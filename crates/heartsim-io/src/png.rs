//! PNG snapshots of an [`EcgRecord`], drawn by the strip's own CPU rasteriser
//! (`heartsim_ui::snapshot`) on pink paper at a fixed 5 px/mm, whatever the screen calibration, so
//! an export looks the same from every machine. The page is the paper's 250 mm wide (10 s at
//! 25 mm/s) and the 12-lead page uses the PDF's 3 × 4 + rhythm layout.

use crate::ExportInfo;
use crate::record::EcgRecord;
use heartsim_core::Lead;
use heartsim_ui::snapshot::{encode_png, render_page, render_strip, stamp_caption};
use heartsim_ui::{LeadOrder, StripStyle, SweepMode, TwelveLeadLayout};

/// Raster resolution of exports, px/mm.
pub const PNG_PX_PER_MM: f32 = 5.0;
/// Page and strip width, mm (the PDF grid width).
const WIDTH_MM: f32 = crate::pdf::GRID_WIDTH_MM;

/// The paper style of exports at the header's speed and gain.
#[must_use]
pub fn export_style(info: &ExportInfo) -> StripStyle {
    StripStyle {
        mm_per_s: info.mm_per_s,
        mm_per_mv: info.mm_per_mv,
        px_per_mm: PNG_PX_PER_MM,
        mode: SweepMode::Scroll,
        ..StripStyle::paper()
    }
}

fn px(mm: f32) -> u32 {
    (mm * PNG_PX_PER_MM).round() as u32
}

/// The 12-lead page (3 × 4 + lead II rhythm strip), ending at the record's last sample.
pub fn page_png(rec: &EcgRecord, info: &ExportInfo, order: LeadOrder) -> Result<Vec<u8>, image::ImageError> {
    let rings = rec.to_rings(1);
    let size = [
        px(WIDTH_MM),
        px(crate::pdf::ROWS as f32 * crate::pdf::ROW_HEIGHT_MM),
    ];
    let mut img = render_page(
        &rings,
        &export_style(info),
        TwelveLeadLayout::Clinical,
        order,
        rec.len() as u64,
        size,
        1.0,
    );
    if let Some(note) = &info.trace_note {
        stamp_caption(&mut img, &export_style(info), note);
    }
    encode_png(&img)
}

/// One lead across the page width (a rhythm strip), ending at the record's last sample.
pub fn strip_png(rec: &EcgRecord, info: &ExportInfo, lead: Lead) -> Result<Vec<u8>, image::ImageError> {
    let rings = rec.to_rings(1);
    let size = [px(WIDTH_MM), px(crate::pdf::ROW_HEIGHT_MM)];
    let mut img = render_strip(
        &rings[lead as usize],
        &export_style(info),
        lead.label(),
        rec.len() as u64,
        size,
        1.0,
    );
    if let Some(note) = &info.trace_note {
        stamp_caption(&mut img, &export_style(info), note);
    }
    encode_png(&img)
}
