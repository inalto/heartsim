//! Time zoom of the ECG strips: the gesture reading and the arithmetic that keeps a sample, or a
//! caliper line, under the finger while the paper speed changes.
//!
//! Zoom is a drawing matter. The strips are drawn at an effective paper speed (the chosen preset
//! times the zoom factor) exactly as if that speed had been chosen, so the grid stays truthful: a
//! 1 mm square keeps its size and stands for another time. Nothing in the engines changes.

use crate::strip::StripWindow;
use crate::viewport::{pointer_or_pinch_over, zoom_input};
use egui::{Rect, Ui};

/// Slowest effective paper speed zooming reaches, mm/s (the slowest preset, halved once).
pub const MIN_EFFECTIVE_MM_S: f32 = 6.25;
/// Fastest effective paper speed zooming reaches, mm/s (the fastest preset, doubled once).
pub const MAX_EFFECTIVE_MM_S: f32 = 400.0;
/// What one mouse-wheel notch does to the time zoom when a strip takes the plain wheel: about 10 %.
pub const WHEEL_NOTCH_FACTOR: f32 = 1.1;

/// `zoom` limited so that the effective speed `base_mm_s × zoom` stays within
/// [`MIN_EFFECTIVE_MM_S`]..=[`MAX_EFFECTIVE_MM_S`]. A zoom that is not a positive finite number
/// becomes 1.
#[must_use]
pub fn clamp_time_zoom(base_mm_s: f32, zoom: f32) -> f32 {
    let usable = zoom.is_finite() && zoom > 0.0 && base_mm_s.is_finite() && base_mm_s > 0.0;
    if !usable {
        return 1.0;
    }
    let lo = MIN_EFFECTIVE_MM_S / base_mm_s;
    let hi = MAX_EFFECTIVE_MM_S / base_mm_s;
    // a preset outside the range (none today) keeps its own speed: zoom 1
    if lo > hi || lo > 1.0 || hi < 1.0 {
        return 1.0;
    }
    zoom.clamp(lo, hi)
}

/// The zoom a strip is asked for this frame.
///
/// Reads the gestures over `rect` and returns `(factor, anchor_x)`: the multiplicative change of
/// the time zoom (above 1 = faster paper, wider samples) and the screen x the gesture is centred
/// on (the pointer, or the middle of the two fingers), or `None` when nothing zooms.
///
/// * a two-finger pinch, a trackpad magnify and Ctrl (⌘ on macOS) + wheel always count;
/// * the plain vertical wheel counts only when `plain_wheel` is set, one notch being
///   [`WHEEL_NOTCH_FACTOR`]; the strip then consumes the scroll so that nothing behind it scrolls.
///   Pass `false` for a strip inside a scroll area that the wheel would otherwise scroll.
///
/// Reusable by any strip drawn elsewhere (the interval drill), which then applies the factor to
/// its own paper speed.
pub fn strip_zoom_factor(ui: &mut Ui, rect: Rect, plain_wheel: bool) -> Option<(f32, f32)> {
    let over = pointer_or_pinch_over(ui, rect);
    if !over {
        return None;
    }
    let anchor_x = ui.input(|i| {
        i.multi_touch()
            .map(|t| t.center_pos.x)
            .or_else(|| i.pointer.hover_pos().map(|p| p.x))
    })?;
    let z = ui.input(|i| zoom_input(i, true));
    let mut factor = z.factor;
    if plain_wheel && z.wheel != 0.0 {
        factor *= WHEEL_NOTCH_FACTOR.powf(z.wheel);
        ui.input_mut(|i| i.smooth_scroll_delta.y = 0.0);
    }
    ((factor - 1.0).abs() > 1e-6 && factor.is_finite()).then_some((factor, anchor_x))
}

/// Continuous sample position drawn at screen x in a scrolling window: sample `k` is centred on
/// `k + 0.5`. Linear, so it also works outside the strip.
#[must_use]
pub fn sample_pos_at_x(win: &StripWindow, x: f32) -> f64 {
    win.head as f64 - f64::from(win.rect.right() - x) / f64::from(win.px_per_sample)
}

/// Screen x of the continuous sample position `pos` in a scrolling window.
#[must_use]
pub fn x_at_sample_pos(win: &StripWindow, pos: f64) -> f32 {
    win.rect.right() - ((win.head as f64 - pos) * f64::from(win.px_per_sample)) as f32
}

/// The window head (one past the newest sample) at which the sample position `pos` is drawn at
/// screen x in a scrolling window shaped like `win`.
#[must_use]
pub fn head_keeping(win: &StripWindow, pos: f64, x: f32) -> f64 {
    pos + f64::from(win.rect.right() - x) / f64::from(win.px_per_sample)
}

/// A caliper position, ms from the strip's left edge, moved from the window `old` to the window
/// `new` (both scrolling, on the same rect) so that the line stays on the sample it was on.
#[must_use]
pub fn rebase_caliper_ms(ms: f32, old: &StripWindow, new: &StripWindow) -> f32 {
    let pos = sample_pos_at_x(old, old.x_from_left_ms(ms));
    new.ms_from_left(x_at_sample_pos(new, pos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strip::StripStyle;
    use egui::{Pos2, Vec2};

    fn win(zoom: f32, head: u64) -> StripWindow {
        let style = StripStyle {
            mm_per_s: 25.0 * zoom,
            ..StripStyle::default()
        };
        StripWindow::new(
            Rect::from_min_size(Pos2::new(10.0, 0.0), Vec2::new(1000.0, 200.0)),
            &style,
            500.0,
            head,
        )
    }

    #[test]
    fn the_zoom_is_clamped_to_the_effective_speed_range() {
        assert!(
            (clamp_time_zoom(25.0, 100.0) - 16.0).abs() < 1e-5,
            "25 × 16 = 400"
        );
        assert!(
            (clamp_time_zoom(25.0, 0.001) - 0.25).abs() < 1e-5,
            "25 × 0.25 = 6.25"
        );
        assert!((clamp_time_zoom(200.0, 100.0) - 2.0).abs() < 1e-5);
        assert!((clamp_time_zoom(12.5, 0.001) - 0.5).abs() < 1e-5);
        assert!(
            (clamp_time_zoom(25.0, 1.5) - 1.5).abs() < 1e-6,
            "inside the range: unchanged"
        );
        assert!((clamp_time_zoom(25.0, f32::NAN) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_caliper_keeps_its_sample_when_the_window_changes() {
        let old = win(1.0, 20_000);
        let new = win(2.0, 19_000);
        for ms in [100.0_f32, 450.0, 1800.0] {
            let pos = sample_pos_at_x(&old, old.x_from_left_ms(ms));
            let ms2 = rebase_caliper_ms(ms, &old, &new);
            let pos2 = sample_pos_at_x(&new, new.x_from_left_ms(ms2));
            assert!((pos - pos2).abs() < 1e-3, "{ms} ms moved from {pos} to {pos2}");
        }
    }

    #[test]
    fn head_keeping_puts_the_sample_back_under_x() {
        let old = win(1.0, 20_000);
        let x = 600.0;
        let pos = sample_pos_at_x(&old, x);
        let mut new = win(3.0, 0);
        new.head = head_keeping(&new, pos, x).round() as u64;
        assert!((x_at_sample_pos(&new, pos) - x).abs() < new.px_per_sample);
    }
}
