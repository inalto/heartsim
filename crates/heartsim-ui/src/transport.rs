//! Run / Pause and simulation-speed controls shared by the simulator toolbars (B19).
//!
//! Both controls carry text labels, so they appear by name in the AccessKit tree and in screen
//! readers; the speed slider is also named, so its value is announced with its name. Run / Pause is
//! the bar's one primary action ([`crate::widgets::primary_button`]); the speed is a
//! [`crate::widgets::slider_row`].

use crate::t;
use egui::{Response, Slider, Ui};

/// Simulation speed range, multiples of real time (log scale on the slider).
pub const SPEED_RANGE: std::ops::RangeInclusive<f64> = 0.05..=2.0;

/// The widgets of a transport bar, so the caller can react to clicks and attach highlights.
#[derive(Debug)]
pub struct TransportResponse {
    /// The Run / Pause button. Clicking it toggles the pause; the caller owns that state.
    pub pause: Response,
    /// The speed slider.
    pub speed: Response,
}

/// Draws the Run / Pause button (labelled "Pause" while running, "Run" while paused) and the
/// simulation speed slider. `speed` is clamped to [`SPEED_RANGE`].
pub fn transport_controls(ui: &mut Ui, paused: bool, speed: &mut f64) -> TransportResponse {
    let label = if paused {
        t!("transport-run")
    } else {
        t!("transport-pause")
    };
    let pause = crate::widgets::primary_button(ui, label);
    let speed_response = crate::widgets::slider_row(
        ui,
        &t!("transport-speed"),
        Slider::new(speed, SPEED_RANGE)
            .logarithmic(true)
            .max_decimals(2)
            .suffix(crate::controls::TIMES),
    );
    TransportResponse {
        pause,
        speed: speed_response,
    }
}
