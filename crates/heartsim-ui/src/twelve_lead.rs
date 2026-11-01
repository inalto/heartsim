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

/// Where each cell goes: `(lead, column, row, columns spanned)` on a grid of `cols × rows`.
#[must_use]
pub fn layout_cells(layout: TwelveLeadLayout) -> (usize, usize, Vec<(Lead, usize, usize, usize)>) {
    match layout {
        TwelveLeadLayout::Stacked => (
            1,
            12,
            Lead::ALL.iter().enumerate().map(|(i, l)| (*l, 0, i, 1)).collect(),
        ),
        TwelveLeadLayout::TwoColumn => (
            2,
            6,
            Lead::ALL
                .iter()
                .enumerate()
                .map(|(i, l)| (*l, i / 6, i % 6, 1))
                .collect(),
        ),
        TwelveLeadLayout::Clinical => {
            let mut cells: Vec<_> = Lead::ALL
                .iter()
                .enumerate()
                .map(|(i, l)| (*l, i / 3, i % 3, 1))
                .collect();
            cells.push((Lead::II, 0, 3, 4));
            (4, 4, cells)
        }
    }
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
}
