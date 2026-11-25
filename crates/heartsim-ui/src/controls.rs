//! Labelled sliders in one style: the label before the control and the unit as the suffix of the
//! value ("SA rate [slider] 72 bpm"), the way a lab instrument labels its dials.
//!
//! Units are not translated (see the note at the top of `assets/locales/en/ui.ftl`), so they are
//! constants here and not message keys.

use egui::{Response, Slider, Ui};

/// Beats per minute.
pub const BPM: &str = " bpm";
/// Pulses per minute (the pacemaker's upper rate).
pub const PPM: &str = " ppm";
/// Milliseconds.
pub const MS: &str = " ms";
/// Seconds.
pub const S: &str = " s";
/// Millimetres.
pub const MM: &str = " mm";
/// Millivolts.
pub const MV: &str = " mV";
/// Milliamperes.
pub const MA: &str = " mA";
/// Watts.
pub const W: &str = " W";
/// Millimetres per millivolt (the vector ring's scale).
pub const MM_PER_MV: &str = " mm/mV";
/// Screen points per millimetre.
pub const PT_PER_MM: &str = " pt/mm";
/// Degrees.
pub const DEG: &str = "°";
/// A multiple ("1.50×").
pub const TIMES: &str = "×";

/// Draws `slider` as a [`crate::widgets::slider_row`] (label left, rail, value box) with `unit` as
/// the suffix of its value. The slider is named by the label for screen readers.
pub fn slider(ui: &mut Ui, label: &str, slider: Slider<'_>, unit: &str) -> Response {
    crate::widgets::slider_row(ui, label, slider.suffix(unit))
}
