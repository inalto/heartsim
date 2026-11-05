//! The therapy window of the Simulator tab (brief B14): drug tray, ischaemia panel with the
//! coronary tree, and the defibrillator.
//!
//! Every control produces scenario `Action`s that go to the timing engine — the owner of the
//! pharmacology, the coronary lesions and the defibrillator — exactly as timeline actions do;
//! `Stenosis` also reaches the 3-D heart's eikonal engine, which computes the per-cell injury.
//! After each step the engine's combined drug effect and ST vector reach the Tier-0
//! synthesiser and the heart, and successful shocks depolarise the heart, so the strip and the
//! 3-D view show the same therapy.

use super::HeartSimApp;
use eframe::egui;
use heartsim_core::Action;
use heartsim_ecg::SynthModulation;
use heartsim_sim::Engine;
use heartsim_sim::ischaemia::CoronaryBranch;
use heartsim_ui::{
    DefibPanelState, IschaemiaPanelState, defib_panel, describe_shock, drug_tray, ischaemia_panel,
};

/// Every n-th territory cell gets a highlight sphere.
const TERRITORY_GLYPH_STRIDE: usize = 7;
const TERRITORY_GLYPH_RADIUS_MM: f32 = 0.9;

/// Therapy UI state owned by the app.
#[derive(Debug, Default)]
pub(super) struct Therapy {
    /// The therapy window is shown.
    pub open: bool,
    ischaemia: IschaemiaPanelState,
    defib: DefibPanelState,
    /// Branch whose territory is highlighted on the heart, and its glyphs.
    highlight: Option<(CoronaryBranch, Vec<heartsim_render::GlyphInstance>)>,
}

impl HeartSimApp {
    /// Hand an action to the 3-D heart where it is spatial (coronary lesions); the timing engine
    /// gets it separately (timeline, therapy window).
    pub(super) fn forward_to_heart(&mut self, action: &Action) {
        if let (Action::Stenosis { .. }, Some(h)) = (action, &mut self.heart)
            && let Err(e) = h.engine_mut().apply(action, self.engine.now())
        {
            tracing::warn!("3-D heart: {e}");
        }
    }

    /// Apply a therapy action now: timing engine first, then the heart.
    fn apply_therapy(&mut self, action: &Action) {
        if self.reentry_apply(action) {
            return;
        }
        let now = self.engine.now();
        if let Err(e) = self.engine.apply(action, now) {
            tracing::warn!("therapy action: {e}");
            return;
        }
        self.forward_to_heart(action);
    }

    /// After one engine step: drug effect and ST vector to the synthesiser and the heart,
    /// delivered shocks to the heart and the defibrillator panel.
    pub(super) fn therapy_step(&mut self) {
        let fx = self.engine.drug_effects();
        self.synth.modulation = SynthModulation::from_state(&fx, self.engine.ischaemia());
        let shocks = self
            .reentry_shocks()
            .unwrap_or_else(|| self.engine.drain_shocks());
        if let Some(h) = &mut self.heart {
            h.engine_mut().set_drug_effects(&fx);
            for s in shocks.iter().filter(|s| s.success) {
                h.engine_mut().shock(s.at);
            }
        }
        for s in &shocks {
            tracing::info!("shock: {}", describe_shock(s));
        }
        for sound in self.therapy.defib.on_shocks(&shocks) {
            tracing::debug!("defibrillator sound {sound:?}");
        }
    }

    /// The floating therapy window, and the territory highlight it drives on the heart.
    pub(super) fn therapy_window(&mut self, ctx: &egui::Context) {
        let mut open = self.therapy.open;
        let mut actions = Vec::new();
        let mut highlight = None;
        let now = self.engine.now().0;
        egui::Window::new("Therapy")
            .open(&mut open)
            .default_width(320.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::CollapsingHeader::new("Drugs")
                        .default_open(true)
                        .show(ui, |ui| {
                            let active = self.engine.pharmacology().active(now);
                            let table = std::sync::Arc::clone(self.engine.pharmacology().table());
                            actions.extend(drug_tray(ui, &table, &active));
                        });
                    egui::CollapsingHeader::new("Ischaemia").show(ui, |ui| {
                        let lesions = self.engine.ischaemia().lesions().to_vec();
                        let r = ischaemia_panel(ui, &mut self.therapy.ischaemia, &lesions);
                        actions.extend(r.actions);
                        highlight = r.highlight;
                    });
                    egui::CollapsingHeader::new("Defibrillator").show(ui, |ui| {
                        let r = defib_panel(ui, &mut self.therapy.defib, now, self.engine.shock_armed());
                        if r.cancel_armed {
                            self.engine.cancel_armed_shock();
                        }
                        for sound in r.sounds {
                            tracing::debug!("defibrillator sound {sound:?}");
                        }
                        actions.extend(r.action);
                    });
                });
            });
        self.therapy.open = open;
        for a in &actions {
            self.apply_therapy(a);
        }
        self.sync_territory_highlight(if open { highlight } else { None });
    }

    /// Highlight `branch`'s territory on the heart (recomputed only when the branch changes).
    fn sync_territory_highlight(&mut self, branch: Option<CoronaryBranch>) {
        let Some(h) = &mut self.heart else { return };
        let current = self.therapy.highlight.as_ref().map(|(b, _)| *b);
        if current != branch {
            self.therapy.highlight = branch.map(|b| {
                let w = h.territory_weight(b);
                let glyphs = heartsim_ui::territory_glyphs(
                    &h.engine().cloud().positions,
                    &w,
                    TERRITORY_GLYPH_STRIDE,
                    TERRITORY_GLYPH_RADIUS_MM,
                );
                (b, glyphs)
            });
        }
        h.set_overlay_glyphs(
            self.therapy
                .highlight
                .as_ref()
                .map(|(_, g)| g.clone())
                .unwrap_or_default(),
        );
    }
}
