//! The Heart Vector tab (brief B8 §2): the vectorcardiogram loop of the last beat and the
//! instantaneous heart vector at the strip cursor, on the frontal and horizontal planes, above the
//! strip they were computed from.

use super::HeartSimApp;
use crate::narrow::{Narrow, Pane, PaneSpec, Side};
use eframe::egui;
use heartsim_ui::{VectorPlane, VectorViewStyle, cards, full_scale_mv, t, vector_plane};

impl HeartSimApp {
    pub(super) fn vector_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("vector_strip")
            .resizable(true)
            .default_size(200.0)
            .min_size(crate::responsive::strip_min_height(ui.ctx()))
            .show(ui, |ui| {
                self.ecg.controls(ui, &self.rings, Some(&mut self.selected_lead));
                let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
                self.ecg.strip(ui, rect, &self.rings, self.selected_lead);
                ui.allocate_rect(rect, egui::Sense::hover());
            });
        let spec = PaneSpec {
            id: "heart_vector",
            left: None,
            right: Some(Side {
                id: "vector_readout",
                default_px: 260.0,
                cap_px: 100_000.0,
                name: "",
            }),
            centre: "",
            // on a phone the two planes stay on top and the read-outs scroll below them
            narrow: Narrow::Stacked { centre_height: 230.0 },
        };
        crate::narrow::panes(ui, &spec, |ui, pane| match pane {
            Pane::Right => {
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
                        heartsim_ui::helper_text(ui, t!("vector-scrub-hint"));
                    });
                    cards::card(ui, |ui| {
                        cards::heading(ui, t!("readout-heading"));
                        self.ecg.readout(ui);
                    });
                });
            }
            Pane::Centre => {
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
            }
            Pane::Left => {}
        });
    }
}
