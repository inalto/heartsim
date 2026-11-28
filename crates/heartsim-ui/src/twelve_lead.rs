//! All twelve leads on one page, either stacked in a single column (one row per lead, the
//! teaching view) or in the clinical 3 × 4 grid with a lead II rhythm strip underneath.

use crate::strip::{StripStyle, ecg_record_strip, ecg_strip};
use crate::t;
use egui::{Rect, Ui, Vec2};
use heartsim_core::Lead;
use heartsim_ecg::LeadRing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TwelveLeadLayout {
    /// 12 rows × 1 column, all leads time-aligned.
    #[default]
    Stacked,
    /// 6 rows × 2 columns: limb leads on the left, chest leads on the right.
    TwoColumn,
    /// Clinical printout: columns I/II/III, aVR/aVL/aVF, V1–V3, V4–V6, plus a lead II rhythm strip.
    Clinical,
}

impl TwelveLeadLayout {
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Stacked => t!("layout-12x1"),
            Self::TwoColumn => t!("layout-6x2"),
            Self::Clinical => t!("layout-3x4"),
        }
    }
}

/// Order of the limb leads on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeadOrder {
    /// I, II, III, aVR, aVL, aVF.
    #[default]
    Standard,
    /// Cabrera (hexaxial) sequence aVL, I, −aVR, II, aVF, III: the limb leads in frontal-plane
    /// angle order from −30° to +120°, with aVR inverted so it reads at +30°.
    Cabrera,
}

impl LeadOrder {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Cabrera => "Cabrera",
        }
    }

    /// The twelve leads in page order, each with "draw inverted".
    #[must_use]
    pub fn leads(self) -> [(Lead, bool); 12] {
        let limb: [(Lead, bool); 6] = match self {
            Self::Standard => [
                (Lead::I, false),
                (Lead::II, false),
                (Lead::III, false),
                (Lead::AVR, false),
                (Lead::AVL, false),
                (Lead::AVF, false),
            ],
            Self::Cabrera => [
                (Lead::AVL, false),
                (Lead::I, false),
                (Lead::AVR, true),
                (Lead::II, false),
                (Lead::AVF, false),
                (Lead::III, false),
            ],
        };
        std::array::from_fn(|i| if i < 6 { limb[i] } else { (Lead::ALL[i], false) })
    }
}

/// Where each cell goes: `(lead, column, row, columns spanned)` on a grid of `cols × rows`.
#[must_use]
pub fn layout_cells(layout: TwelveLeadLayout) -> (usize, usize, Vec<(Lead, usize, usize, usize)>) {
    let (cols, rows, cells) = layout_cells_ordered(layout, LeadOrder::Standard);
    (
        cols,
        rows,
        cells.into_iter().map(|(l, _, c, r, s)| (l, c, r, s)).collect(),
    )
}

/// Like [`layout_cells`] in a given lead order: `(lead, inverted, column, row, columns spanned)`.
#[must_use]
pub fn layout_cells_ordered(
    layout: TwelveLeadLayout,
    order: LeadOrder,
) -> (usize, usize, Vec<(Lead, bool, usize, usize, usize)>) {
    let leads = order.leads();
    match layout {
        TwelveLeadLayout::Stacked => (
            1,
            12,
            leads
                .iter()
                .enumerate()
                .map(|(i, (l, inv))| (*l, *inv, 0, i, 1))
                .collect(),
        ),
        TwelveLeadLayout::TwoColumn => (
            2,
            6,
            leads
                .iter()
                .enumerate()
                .map(|(i, (l, inv))| (*l, *inv, i / 6, i % 6, 1))
                .collect(),
        ),
        TwelveLeadLayout::Clinical => {
            let mut cells: Vec<_> = leads
                .iter()
                .enumerate()
                .map(|(i, (l, inv))| (*l, *inv, i / 3, i % 3, 1))
                .collect();
            cells.push((Lead::II, false, 0, 3, 4));
            (4, 4, cells)
        }
    }
}

/// Whether cell `index` of `count` on a page of `layout` carries the wave letters and the PR / RR
/// read-outs. They crowd a page of twelve small strips, so the clinical grid keeps them on its
/// rhythm strip (the last cell) only. The one-column and two-column layouts draw them in every
/// cell, where the painter drops the ones that would touch their neighbour.
#[must_use]
pub fn cell_is_annotated(layout: TwelveLeadLayout, index: usize, count: usize) -> bool {
    layout != TwelveLeadLayout::Clinical || index + 1 == count
}

/// Fewest screen points per beat at which the PR / RR read-outs go into the cells of a one- or
/// two-column page: the "RR 832" label is about 40 points wide and should take well under half of
/// the span it measures, or twelve rows of them read as noise. At the usual 25 mm/s and 3 pt/mm a
/// beat is 60 to 75 points, so they stay off until the paper is sped up or the rate is slow.
pub const MIN_BEAT_PX_FOR_INTERVALS: f32 = 110.0;

/// Whether the interval read-outs fit the cells of a page: `rr_ms` is the measured RR interval
/// (when unknown they are left to the painter, which drops the ones that overlap) and `px_per_s`
/// the paper speed in points per second.
#[must_use]
pub fn intervals_fit_cells(rr_ms: Option<f64>, px_per_s: f32) -> bool {
    rr_ms.is_none_or(|rr| (rr * f64::from(px_per_s) / 1000.0) as f32 >= MIN_BEAT_PX_FOR_INTERVALS)
}

/// One strip of a 12-lead page in screen space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageCell {
    pub lead: Lead,
    pub invert: bool,
    pub rect: Rect,
    /// How many cell-widths this strip's time window ends before the page's head. Columns of a
    /// multi-column page are consecutive time segments, like a printout: the last column ends at
    /// "now", the first starts `cols` segments earlier, and the full-width rhythm strip spans them
    /// all. This is the shared time base.
    pub columns_before_head: usize,
}

/// Cell rectangles for `layout` in `rect` (1-point gutters).
#[must_use]
pub fn page_cells(rect: Rect, layout: TwelveLeadLayout, order: LeadOrder) -> Vec<PageCell> {
    let (cols, rows, cells) = layout_cells_ordered(layout, order);
    let cell = Vec2::new(rect.width() / cols as f32, rect.height() / rows as f32);
    cells
        .into_iter()
        .map(|(lead, invert, c, r, span)| {
            let min = rect.min + Vec2::new(c as f32 * cell.x, r as f32 * cell.y);
            PageCell {
                lead,
                invert,
                rect: Rect::from_min_size(min, Vec2::new(cell.x * span as f32, cell.y)).shrink(1.0),
                columns_before_head: cols - c - span,
            }
        })
        .collect()
}

/// Paint the 12 leads into `rect`.
pub fn twelve_lead_page(
    ui: &mut Ui,
    rect: Rect,
    rings: &[LeadRing],
    style: &StripStyle,
    layout: TwelveLeadLayout,
) {
    let (cols, rows, cells) = layout_cells(layout);
    let cell = Vec2::new(rect.width() / cols as f32, rect.height() / rows as f32);
    for (lead, c, r, span) in cells {
        let min = rect.min + Vec2::new(c as f32 * cell.x, r as f32 * cell.y);
        let cell_rect = Rect::from_min_size(min, Vec2::new(cell.x * span as f32, cell.y)).shrink(1.0);
        ecg_strip(ui, cell_rect, &rings[lead as usize], style, lead.label());
    }
    ui.allocate_rect(rect, egui::Sense::hover());
}

/// [`twelve_lead_page`] for a finished recording: every cell, the narrow ones and the full-width
/// rhythm strip, shows the record from absolute sample `first` at its own left edge, so all cells
/// share one time origin (the beat under the left edge is the same in every lead) and a record
/// shorter than a cell leaves empty paper on its right. The live page's right-aligned window would
/// show the tail of the record in each cell and a blank left part on a wide one.
pub fn twelve_lead_record_page(
    ui: &mut Ui,
    rect: Rect,
    rings: &[LeadRing],
    style: &StripStyle,
    layout: TwelveLeadLayout,
    first: u64,
) {
    let (cols, rows, cells) = layout_cells(layout);
    let cell = Vec2::new(rect.width() / cols as f32, rect.height() / rows as f32);
    for (lead, c, r, span) in cells {
        let min = rect.min + Vec2::new(c as f32 * cell.x, r as f32 * cell.y);
        let cell_rect = Rect::from_min_size(min, Vec2::new(cell.x * span as f32, cell.y)).shrink(1.0);
        ecg_record_strip(ui, cell_rect, &rings[lead as usize], style, lead.label(), first);
    }
    ui.allocate_rect(rect, egui::Sense::hover());
}

/// Paint the given leads as one column of rows in `rect` (used by the axis trainer and quiz).
pub fn lead_column(ui: &mut Ui, rect: Rect, rings: &[LeadRing], style: &StripStyle, leads: &[Lead]) {
    let row_h = rect.height() / leads.len().max(1) as f32;
    for (i, lead) in leads.iter().enumerate() {
        let min = rect.min + Vec2::new(0.0, i as f32 * row_h);
        let r = Rect::from_min_size(min, Vec2::new(rect.width(), row_h)).shrink(1.0);
        ecg_strip(ui, r, &rings[*lead as usize], style, lead.label());
    }
    ui.allocate_rect(rect, egui::Sense::hover());
}

/// [`lead_column`] for a finished recording: sample `first` at the left edge of every row.
pub fn record_column(
    ui: &mut Ui,
    rect: Rect,
    rings: &[LeadRing],
    style: &StripStyle,
    leads: &[Lead],
    first: u64,
) {
    let row_h = rect.height() / leads.len().max(1) as f32;
    for (i, lead) in leads.iter().enumerate() {
        let min = rect.min + Vec2::new(0.0, i as f32 * row_h);
        let r = Rect::from_min_size(min, Vec2::new(rect.width(), row_h)).shrink(1.0);
        ecg_record_strip(ui, r, &rings[*lead as usize], style, lead.label(), first);
    }
    ui.allocate_rect(rect, egui::Sense::hover());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lead_appears_in_each_grid_inside_its_bounds() {
        for layout in [
            TwelveLeadLayout::Stacked,
            TwelveLeadLayout::TwoColumn,
            TwelveLeadLayout::Clinical,
        ] {
            let (cols, rows, cells) = layout_cells(layout);
            for l in Lead::ALL {
                assert!(cells.iter().any(|c| c.0 == l), "{layout:?}: {l:?}");
            }
            assert!(cells.iter().all(|(_, c, r, s)| c + s <= cols && *r < rows));
        }
    }

    #[test]
    fn the_clinical_grid_annotates_its_rhythm_strip_only() {
        let (_, _, cells) = layout_cells(TwelveLeadLayout::Clinical);
        let n = cells.len();
        let annotated: Vec<usize> = (0..n)
            .filter(|&i| cell_is_annotated(TwelveLeadLayout::Clinical, i, n))
            .collect();
        assert_eq!(annotated, [n - 1]);
        assert_eq!(cells[n - 1].3, 4, "and that is the full-width strip");
        assert!(cell_is_annotated(TwelveLeadLayout::TwoColumn, 0, 12));
    }

    #[test]
    fn interval_read_outs_wait_for_enough_room_per_beat() {
        let px_per_s = 25.0 * 3.0;
        assert!(!intervals_fit_cells(Some(800.0), px_per_s), "60 pt per beat");
        assert!(
            intervals_fit_cells(Some(800.0), 50.0 * 3.0),
            "120 pt per beat at 50 mm/s"
        );
        assert!(intervals_fit_cells(Some(1600.0), px_per_s), "slow rate: 120 pt");
        assert!(intervals_fit_cells(None, px_per_s));
    }

    #[test]
    fn cabrera_order_runs_through_the_hexaxial_angles() {
        let leads = LeadOrder::Cabrera.leads();
        let angles: Vec<f64> = leads[..6]
            .iter()
            .map(|(l, inv)| l.frontal_angle_deg().unwrap() + if *inv { 180.0 } else { 0.0 })
            .map(heartsim_core::FrontalAxis::normalise)
            .collect();
        assert!(
            angles.windows(2).all(|w| (w[1] - w[0] - 30.0).abs() < 1e-9),
            "{angles:?}"
        );
        assert_eq!(leads[6], (Lead::V1, false));
    }

    #[test]
    fn clinical_columns_are_consecutive_time_segments() {
        let rect = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 400.0));
        let cells = page_cells(rect, TwelveLeadLayout::Clinical, LeadOrder::Standard);
        let of = |l: Lead| {
            cells
                .iter()
                .find(|c| c.lead == l && c.rect.width() < 300.0)
                .unwrap()
        };
        assert_eq!(of(Lead::I).columns_before_head, 3);
        assert_eq!(of(Lead::V4).columns_before_head, 0);
        assert_eq!(
            cells.last().unwrap().columns_before_head,
            0,
            "rhythm strip ends at now"
        );
    }
}
