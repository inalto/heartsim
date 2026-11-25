//! How the Simulator page lays out its three parts: the heart's controls, the 3-D view and the QRS
//! axis panel. With room they are one row (controls left, view centre, axis right); on a
//! phone-narrow window ([`crate::responsive::NARROW_PX`]) they are three views of one full-width
//! panel, chosen with a segmented control, because two side panels leave the 3-D view a sliver.
//!
//! The parts themselves belong to the app; this module only decides where they go, so the rule
//! (nothing clipped, nothing overlapping, at 400 and at 1280 points) can be tested without a GPU.

use crate::responsive;
use eframe::egui;
use heartsim_ui::t;

/// The three parts of the Simulator, one at a time on a phone-narrow window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SimulatorView {
    /// The 3-D heart.
    #[default]
    Heart,
    /// The heart's controls (colour map, click tool, layers, wall motion, vector overlay).
    Controls,
    /// The measured QRS axis and the interval read-out.
    Axis,
}

/// What the layout places. The app implements this over its own state.
pub trait SimulatorParts {
    /// Whether there are heart controls to show (none when the anatomy pack did not load).
    fn has_controls(&self) -> bool;
    /// The heart's controls, as collapsible sections in a scroll area.
    fn controls(&mut self, ui: &mut egui::Ui);
    /// The frontal QRS axis panel.
    fn axis(&mut self, ui: &mut egui::Ui);
    /// The 3-D view, filling the space it is given.
    fn viewport(&mut self, ui: &mut egui::Ui);
}

/// Draw the parts into `ui` (the space above the ECG strip): three panels in a row with room,
/// the segmented one-at-a-time view when the window is phone-narrow. `view` is the segment shown
/// in the narrow layout.
pub fn simulator_body(ui: &mut egui::Ui, view: &mut SimulatorView, parts: &mut impl SimulatorParts) {
    if responsive::is_narrow(ui.ctx()) {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut options = vec![(SimulatorView::Heart, t!("app-sim-view-heart"))];
            if parts.has_controls() {
                options.push((SimulatorView::Controls, t!("app-sim-view-controls")));
            }
            options.push((SimulatorView::Axis, t!("app-sim-view-axis")));
            if !options.iter().any(|(v, _)| v == view) {
                *view = SimulatorView::Heart;
            }
            heartsim_ui::segmented(ui, view, &options);
            match *view {
                SimulatorView::Heart => parts.viewport(ui),
                SimulatorView::Controls => parts.controls(ui),
                SimulatorView::Axis => parts.axis(ui),
            }
        });
        return;
    }
    // the side panels keep a remembered width between runs; a maximum keeps a stray wide value (or
    // a drag) from pushing the 3-D view out of the window
    egui::Panel::right("axis")
        .default_size(280.0)
        .max_size(responsive::max_side(ui, 480.0))
        .show(ui, |ui| parts.axis(ui));
    if parts.has_controls() {
        egui::Panel::left("heart")
            .default_size(280.0)
            .max_size(responsive::max_side(ui, 480.0))
            .show(ui, |ui| parts.controls(ui));
    }
    egui::CentralPanel::default().show(ui, |ui| parts.viewport(ui));
}
