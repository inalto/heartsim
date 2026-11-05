//! Defibrillator panel (brief B14): energy selector, SYNC, CHARGE and SHOCK.
//!
//! The panel drives a [`Defibrillator`] (charge timing on simulation time) and returns the
//! `Action::Shock` when the learner discharges it; the timing engine then synchronises it on
//! the R wave and decides the outcome. Delivered shocks come back as [`ShockRecord`]s
//! ([`DefibPanelState::on_shocks`]) for the flash animation, the status line and the
//! [`DefibSound`] events.

use crate::t;
use egui::{Color32, ProgressBar, RichText, Ui};
use heartsim_core::Action;
use heartsim_sim::defib::{DefibState, Defibrillator, ENERGY_STEPS_J, ShockRecord, success_probability};

/// How long the panel flashes after a shock, ms of simulated time.
pub const SHOCK_FLASH_MS: f64 = 400.0;

/// Sounds the app should play (no audio backend yet: they are events).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefibSound {
    /// Charging finished: the "ready" tone.
    Charged,
    /// A shock was delivered.
    Shock,
}

/// Panel state owned by the app.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DefibPanelState {
    pub defib: Defibrillator,
    /// Last shock delivered.
    pub last_shock: Option<ShockRecord>,
    /// Reduced motion (B19 settings): the shock no longer flashes the panel.
    pub reduced_motion: bool,
    flash_until_ms: f64,
}

/// What the panel did this frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DefibResponse {
    /// The shock to hand to the engine.
    pub action: Option<Action>,
    /// Withdraw a synchronised shock still waiting for its R wave.
    pub cancel_armed: bool,
    pub sounds: Vec<DefibSound>,
}

impl DefibPanelState {
    /// Feed the shocks the engine delivered this step; returns the sounds to play.
    pub fn on_shocks(&mut self, shocks: &[ShockRecord]) -> Vec<DefibSound> {
        let mut sounds = Vec::new();
        for s in shocks {
            self.last_shock = Some(*s);
            self.flash_until_ms = s.at.0 + SHOCK_FLASH_MS;
            sounds.push(DefibSound::Shock);
        }
        sounds
    }

    /// The panel flashes after a shock.
    #[must_use]
    pub fn flashing(&self, now_ms: f64) -> bool {
        !self.reduced_motion && now_ms < self.flash_until_ms
    }
}

/// One-line description of a delivered shock.
#[must_use]
pub fn describe_shock(s: &ShockRecord) -> String {
    let mode = if s.synchronised {
        t!("defib-sync")
    } else {
        t!("defib-unsync")
    };
    let outcome = if s.success {
        t!("defib-terminated")
    } else {
        t!("defib-no-effect")
    };
    let mut text = t!(
        "defib-shock-line",
        joules = format!("{:.0}", s.joules),
        mode = mode,
        at = crate::i18n::number(s.at.0 / 1000.0, 1),
        rhythm = crate::labels::rhythm_name(s.rhythm),
        probability = format!("{:.0}", 100.0 * success_probability(s.rhythm, s.joules)),
        outcome = outcome
    );
    if s.vulnerable {
        text = format!("{text} — {}", t!("defib-vulnerable"));
    }
    text
}

/// The defibrillator. `now_ms` is simulation time; `armed` is the engine's
/// `TimingEngine::shock_armed`.
pub fn defib_panel(ui: &mut Ui, state: &mut DefibPanelState, now_ms: f64, armed: bool) -> DefibResponse {
    let mut r = DefibResponse::default();
    if state.defib.update(now_ms) {
        r.sounds.push(DefibSound::Charged);
    }
    let d = &mut state.defib;
    ui.horizontal(|ui| {
        egui::ComboBox::from_label(t!("defib-energy"))
            .selected_text(format!("{:.0} J", d.energy_j))
            .show_ui(ui, |ui| {
                for e in ENERGY_STEPS_J {
                    ui.selectable_value(&mut d.energy_j, e, format!("{e:.0} J"));
                }
            });
        ui.toggle_value(&mut d.synchronised, t!("defib-sync-button"))
            .on_hover_text(t!("defib-sync-hint"));
    });
    ui.horizontal(|ui| {
        let charging = matches!(d.state, DefibState::Charging { .. });
        if ui
            .add_enabled(!charging, egui::Button::new(t!("defib-charge")))
            .on_hover_text(format!(
                "≈ {:.1} s to {:.0} J",
                Defibrillator::charge_time_ms(d.energy_j) / 1000.0,
                d.energy_j
            ))
            .clicked()
        {
            d.charge(now_ms);
        }
        let ready = d.state == DefibState::Ready;
        let shock = egui::Button::new(RichText::new(t!("defib-shock")).strong().color(Color32::WHITE)).fill(
            if ready {
                Color32::from_rgb(200, 30, 30)
            } else {
                Color32::from_rgb(90, 40, 40)
            },
        );
        if ui.add_enabled(ready, shock).clicked() {
            r.action = d.shock();
        }
        if ui.button(t!("defib-disarm")).clicked() {
            d.disarm();
            r.cancel_armed = armed;
        }
    });
    let fraction = d.charge_fraction(now_ms) as f32;
    let label = match d.state {
        DefibState::Idle => t!("defib-idle"),
        DefibState::Charging { .. } => t!("defib-charging", percent = format!("{:.0}", 100.0 * fraction)),
        DefibState::Ready => t!("defib-charged", joules = format!("{:.0}", d.energy_j)),
    };
    ui.add(ProgressBar::new(fraction).text(label));
    if armed {
        ui.colored_label(Color32::YELLOW, t!("defib-sync-waiting"));
    }
    if let Some(s) = &state.last_shock {
        let text = describe_shock(s);
        if state.flashing(now_ms) {
            ui.colored_label(Color32::from_rgb(255, 220, 60), RichText::new(text).strong());
        } else {
            ui.label(text);
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_core::Millis;
    use heartsim_sim::defib::RhythmClass;

    #[test]
    fn shocks_flash_and_sound() {
        let mut s = DefibPanelState::default();
        let rec = ShockRecord {
            at: Millis(1000.0),
            joules: 150.0,
            synchronised: true,
            rhythm: RhythmClass::VentricularTachycardia,
            success: true,
            vulnerable: false,
        };
        assert_eq!(s.on_shocks(&[rec]), vec![DefibSound::Shock]);
        assert!(s.flashing(1200.0) && !s.flashing(1500.0));
        let text = describe_shock(&rec);
        assert!(text.contains("150 J synchronised") && text.contains("VT") && text.contains("terminated"));
    }
}
