//! Window-width rules that keep the app usable on a phone-sized screen.

use eframe::egui;

/// Below this window width (logical px) the app uses its compact layout.
pub const NARROW_PX: f32 = 700.0;

/// Whether the window is phone-narrow.
#[must_use]
pub fn is_narrow(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < NARROW_PX
}

/// The widest a side panel may grow: `wide_cap` px on a desktop window, but never more than half
/// the window (two fifths when narrow), so the central view keeps room instead of being pushed
/// out of the window by a panel with a remembered or default width.
#[must_use]
pub fn max_side(ui: &egui::Ui, wide_cap: f32) -> f32 {
    let w = ui.ctx().content_rect().width();
    let frac = if w < NARROW_PX { 0.4 } else { 0.5 };
    (w * frac).min(wide_cap).max(120.0)
}
