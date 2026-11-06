//! Keyboard shortcuts (B19): Space pauses, ←/→ scrub while paused, +/− change speed, 1–9 open tabs.
//!
//! Shortcuts are ignored while a text box or any other widget has keyboard focus, so typing and
//! Tab/Space on a focused button keep their usual meaning.

use super::{HeartSimApp, Tab};
use eframe::egui::{self, Key};

/// Simulation speed change per `+`/`−` press.
const SPEED_STEP: f64 = 1.25;
/// Scrub step per arrow press, and with Shift.
const SCRUB_STEP_MS: f64 = 100.0;
const SCRUB_STEP_SHIFT_MS: f64 = 1000.0;

const TAB_KEYS: [Key; 9] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];

impl HeartSimApp {
    pub(super) fn keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() || ctx.memory(egui::Memory::focused).is_some() {
            return;
        }
        let (space, left, right, faster, slower, shift, tab) = ctx.input(|i| {
            (
                i.key_pressed(Key::Space),
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals),
                i.key_pressed(Key::Minus),
                i.modifiers.shift,
                TAB_KEYS.iter().position(|k| i.key_pressed(*k)),
            )
        });
        if space {
            self.ecg.toggle_pause();
        }
        if self.ecg.is_paused() {
            let step = if shift { SCRUB_STEP_SHIFT_MS } else { SCRUB_STEP_MS };
            // scrubbing back in time is a positive offset, so ← goes back and → comes forward
            if left {
                self.ecg.scrub_back_by(step, &self.rings);
            }
            if right {
                self.ecg.scrub_back_by(-step, &self.rings);
            }
        }
        if faster {
            self.clock.speed = (self.clock.speed * SPEED_STEP).min(2.0);
        }
        if slower {
            self.clock.speed = (self.clock.speed / SPEED_STEP).max(0.05);
        }
        if let Some(i) = tab {
            self.tab = Tab::ALL[i];
        }
    }
}
