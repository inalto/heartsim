//! The Heart Vector tab (brief B8 §2): the vectorcardiogram loop of the last beat and the
//! instantaneous heart vector at the strip cursor, on the frontal and horizontal planes, above the
//! strip they were computed from.

use super::HeartSimApp;
use eframe::egui;
use heartsim_core::Lead;
use heartsim_ui::{VectorPlane, VectorViewStyle, cards, full_scale_mv, t, vector_plane};

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
                egui::ScrollArea::vertical().show(ui, |ui| {
                    cards::card(ui, |ui| {
                        cards::heading(ui, t!("vector-heading"));
                        ui.label(t!("vector-intro"));
                        if let Some(d) = self.cursor_dipole() {
                            ui.add_space(4.0);
                            ui.monospace(format!("|v| {:.2} mV", d.length()));
                            ui.monospace(t!(
                                "vector-x",
                                value = format!("{:+.2}", d.x),
                                side = t!("vector-left")
                            ));
                            ui.monospace(t!(
                                "vector-y",
                                value = format!("{:+.2}", d.y),
                                side = t!("vector-cranial")
                            ));
                            ui.monospace(t!(
                                "vector-z",
                                value = format!("{:+.2}", d.z),
                                side = t!("vector-anterior")
                            ));
                        }
                        ui.add_space(4.0);
                        ui.weak(t!("vector-scrub-hint"));
                    });
                    cards::card(ui, |ui| {
                        cards::heading(ui, t!("readout-heading"));
                        self.ecg.readout(ui);
                    });
                });
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
