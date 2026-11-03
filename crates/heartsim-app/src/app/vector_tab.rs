//! The Heart Vector tab (brief B8 §2): the vectorcardiogram loop of the last beat and the
//! instantaneous heart vector at the strip cursor, on the frontal and horizontal planes, above the
//! strip they were computed from.

use super::HeartSimApp;
use eframe::egui;
use heartsim_core::Lead;
use heartsim_ui::{VectorPlane, VectorViewStyle, full_scale_mv, vector_plane};

impl HeartSimApp {
    pub(super) fn vector_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("vector_strip")
            .resizable(true)
            .default_size(200.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for l in Lead::ALL {
                        ui.selectable_value(&mut self.selected_lead, l, l.label());
                    }
                });
                self.paper_controls(ui);
                let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
                self.ecg.strip(ui, rect, &self.rings, self.selected_lead);
                ui.allocate_rect(rect, egui::Sense::hover());
            });
        egui::Panel::right("vector_readout")
            .default_size(260.0)
            .show(ui, |ui| {
                ui.strong("Heart vector");
                ui.label(
                    "The arrow is the cardiac dipole at the strip cursor, recovered from all twelve leads. \
                     The loop is the path of its tip through the last complete beat (blue to pink).",
                );
                if let Some(d) = self.cursor_dipole() {
                    ui.separator();
                    ui.monospace(format!("|v| {:.2} mV", d.length()));
                    ui.monospace(format!("x {:+.2}  (left)", d.x));
                    ui.monospace(format!("y {:+.2}  (cranial)", d.y));
                    ui.monospace(format!("z {:+.2}  (anterior)", d.z));
                }
                ui.separator();
                ui.label("Pause and scrub the strip to step the arrow through the P, QRS and T loops.");
                ui.separator();
                self.ecg.readout(ui);
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let now = self.cursor_dipole();
            let loop_mv = self
                .ecg
                .analysis()
                .map(|a| a.heart_vector.as_slice())
                .unwrap_or_default();
            let scale = full_scale_mv(loop_mv, now);
            let style = VectorViewStyle::default();
            let avail = ui.available_size();
            let side = (avail.x / 2.0 - 8.0).min(avail.y - 8.0).max(160.0);
            ui.horizontal(|ui| {
                for plane in VectorPlane::ALL {
                    vector_plane(ui, side, plane, loop_mv, now, scale, &style);
                }
            });
        });
    }
}
