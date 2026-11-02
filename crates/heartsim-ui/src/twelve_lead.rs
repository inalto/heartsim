//! All twelve leads on one page, either stacked in a single column (one row per lead, the
//! teaching view) or in the clinical 3 × 4 grid with a lead II rhythm strip underneath.

use crate::strip::{StripStyle, ecg_strip};
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
    pub fn label(self) -> &'static str {
        match self {
            Self::Stacked => "12 × 1",
            Self::TwoColumn => "6 × 2",
            Self::Clinical => "3 × 4 + rhythm",
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
