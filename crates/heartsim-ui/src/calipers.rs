//! Calipers and pause/scrub for the ECG strips.
//!
//! A caliper pair is two vertical lines measured in milliseconds from the strip's left edge, so a
//! pair stays where the student put it while the paper scrolls (like real calipers on a monitor)
//! and keeps its time meaning when the paper speed changes. The read-out gives Δt, the rate that
//! interval corresponds to, and the trace voltage under each line.

use crate::strip::{StripStyle, StripWindow};
use egui::{Align2, EventFilter, FontId, Id, Key, Pos2, Sense, Stroke, Ui, Vec2};
use heartsim_ecg::LeadRing;
use std::fmt::Write as _;

/// Search half-width when a released caliper line snaps to the nearest deflection, ms.
pub const SNAP_RADIUS_MS: f32 = 40.0;
/// Pointer distance that grabs a caliper line, screen points.
const GRAB_PX: f32 = 6.0;

/// Which part of a caliper pair is selected or being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaliperEnd {
    A,
    B,
    /// The whole pair (dragged by the band between the lines).
    Both,
}

/// Two vertical cursors, ms from the strip's left edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caliper {
    pub a_ms: f32,
    pub b_ms: f32,
}

impl Caliper {
    /// Time between the lines, ms.
    #[must_use]
    pub fn interval_ms(self) -> f32 {
        (self.b_ms - self.a_ms).abs()
    }

    /// Heart rate whose RR equals the interval, beats/min.
    #[must_use]
    pub fn rate_bpm(self) -> Option<f32> {
        let d = self.interval_ms();
        (d >= 1.0).then(|| 60_000.0 / d)
    }
}

/// What a caliper pair measures on a given strip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaliperReading {
    pub interval_ms: f32,
    pub rate_bpm: Option<f32>,
    /// Trace voltage under line A / B, mV (as displayed, i.e. after inversion).
    pub a_mv: Option<f32>,
    pub b_mv: Option<f32>,
}

impl CaliperReading {
    /// `b_mv - a_mv`, mV.
    #[must_use]
    pub fn delta_mv(&self) -> Option<f32> {
        Some(self.b_mv? - self.a_mv?)
    }

    /// One-line read-out, e.g. `"Δt 1000 ms · 60 bpm · ΔV +0.85 mV"`.
    #[must_use]
    pub fn text(&self) -> String {
        let mut s = format!("Δt {:.0} ms", self.interval_ms);
        if let Some(r) = self.rate_bpm {
            let _ = write!(s, " · {r:.0} bpm");
        }
        if let Some(d) = self.delta_mv() {
            let _ = write!(s, " · ΔV {d:+.2} mV");
        }
        s
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    Line {
        pair: usize,
        end: CaliperEnd,
        /// For `Both`: pointer-to-line-A offset at grab time, ms.
        offset_ms: f32,
    },
    Scrub {
        start_scrub_ms: f64,
        origin_x: f32,
    },
}

/// The caliper pairs of one strip plus the interaction state.
#[derive(Debug, Clone, PartialEq)]
pub struct CaliperSet {
    pub pairs: Vec<Caliper>,
    /// Target of keyboard nudges.
    pub selected: Option<(usize, CaliperEnd)>,
    /// Snap a released line to the largest deflection within [`SNAP_RADIUS_MS`].
    pub snap: bool,
    drag: Option<Drag>,
}

impl Default for CaliperSet {
    fn default() -> Self {
        Self {
            pairs: Vec::new(),
            selected: None,
            snap: true,
            drag: None,
        }
    }
}

impl CaliperSet {
    /// Add a pair and select its B line.
    pub fn add_pair(&mut self, a_ms: f32, b_ms: f32) {
        self.pairs.push(Caliper { a_ms, b_ms });
        self.selected = Some((self.pairs.len() - 1, CaliperEnd::B));
    }

    /// Remove every pair.
    pub fn reset(&mut self) {
        self.pairs.clear();
        self.selected = None;
        self.drag = None;
    }

    /// Move the selected line (or pair) by `delta_ms`.
    pub fn nudge(&mut self, delta_ms: f32) {
        if let Some((i, end)) = self.selected
            && let Some(c) = self.pairs.get_mut(i)
        {
            if matches!(end, CaliperEnd::A | CaliperEnd::Both) {
                c.a_ms += delta_ms;
            }
            if matches!(end, CaliperEnd::B | CaliperEnd::Both) {
                c.b_ms += delta_ms;
            }
        }
    }

    /// Measure pair `i` on `ring` as drawn in `win`.
    #[must_use]
    pub fn reading(
        &self,
        i: usize,
        win: &StripWindow,
        ring: &LeadRing,
        invert: bool,
    ) -> Option<CaliperReading> {
        let c = self.pairs.get(i)?;
        let sign = if invert { -1.0 } else { 1.0 };
        let mv = |ms: f32| {
            let s = win.sample_at_x(win.x_from_left_ms(ms))?;
            ring.at(s).map(|v| sign * v)
        };
        Some(CaliperReading {
            interval_ms: c.interval_ms(),
            rate_bpm: c.rate_bpm(),
            a_mv: mv(c.a_ms),
            b_mv: mv(c.b_ms),
        })
    }

    /// Nearest line within grab distance of screen x, else the pair whose band contains x.
    fn hit(&self, win: &StripWindow, x: f32) -> Option<(usize, CaliperEnd)> {
        let mut best: Option<(f32, usize, CaliperEnd)> = None;
        for (i, c) in self.pairs.iter().enumerate() {
            for (end, ms) in [(CaliperEnd::A, c.a_ms), (CaliperEnd::B, c.b_ms)] {
                let d = (win.x_from_left_ms(ms) - x).abs();
                if d <= GRAB_PX && best.is_none_or(|b| d < b.0) {
                    best = Some((d, i, end));
                }
            }
        }
        if let Some((_, i, end)) = best {
            return Some((i, end));
        }
        let ms = win.ms_from_left(x);
        self.pairs
            .iter()
            .position(|c| (c.a_ms.min(c.b_ms)..=c.a_ms.max(c.b_ms)).contains(&ms))
            .map(|i| (i, CaliperEnd::Both))
    }
}

/// Move a caliper line at `ms` to the largest deflection (from the local mean) within
/// [`SNAP_RADIUS_MS`]; unchanged if no trace is under it.
#[must_use]
pub fn snap_ms(win: &StripWindow, ring: &LeadRing, ms: f32) -> f32 {
    let Some(centre) = win.sample_at_x(win.x_from_left_ms(ms)) else {
        return ms;
    };
    let r = (SNAP_RADIUS_MS * 1e-3 * win.sample_rate_hz) as u64;
    let cands: Vec<(u64, f32)> = (centre.saturating_sub(r)..=centre + r)
        .filter(|s| win.x_of_sample(*s).is_some())
        .filter_map(|s| ring.at(s).map(|v| (s, v)))
        .collect();
    if cands.is_empty() {
        return ms;
    }
    let mean = cands.iter().map(|c| c.1).sum::<f32>() / cands.len() as f32;
    cands
        .iter()
        .max_by(|a, b| (a.1 - mean).abs().total_cmp(&(b.1 - mean).abs()))
        .and_then(|(s, _)| win.x_of_sample(*s))
        .map_or(ms, |x| win.ms_from_left(x))
}

/// Pause state and how far the view is scrubbed back from the newest sample.
///
/// While paused the rings are frozen (the app stops stepping the simulation), so "now" does not
/// move; resuming clears the scrub and the simulation carries on from where it stopped.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Playback {
    paused: bool,
    scrub_ms: f64,
}

impl Playback {
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume live display: the view jumps back to "now" (which has not moved while paused).
    pub fn resume(&mut self) {
        self.paused = false;
        self.scrub_ms = 0.0;
    }

    pub fn toggle(&mut self) {
        if self.paused {
            self.resume();
        } else {
            self.pause();
        }
    }

    /// How far behind the newest sample the view's head is, ms (0 when live).
    #[must_use]
    pub fn scrub_ms(&self) -> f64 {
        self.scrub_ms
    }

    /// Set the scrub offset (only while paused), clamped to `0..=max_ms`.
    pub fn set_scrub_ms(&mut self, ms: f64, max_ms: f64) {
        if self.paused {
            self.scrub_ms = ms.clamp(0.0, max_ms.max(0.0));
        }
    }

    /// Scrub offset rounded to whole samples.
    #[must_use]
    pub fn scrub_samples(&self, sample_rate_hz: f32) -> u64 {
        (self.scrub_ms * f64::from(sample_rate_hz) / 1000.0).round() as u64
    }

    /// Absolute index one past the newest sample to show, for a ring that has pushed `pushed`.
    #[must_use]
    pub fn head(&self, pushed: u64, sample_rate_hz: f32) -> u64 {
        pushed.saturating_sub(self.scrub_samples(sample_rate_hz))
    }

    /// Time of the newest shown sample (the scrub cursor), ms since the first sample.
    #[must_use]
    pub fn cursor_time_ms(&self, pushed: u64, sample_rate_hz: f32) -> f64 {
        self.head(pushed, sample_rate_hz).saturating_sub(1) as f64 * 1000.0 / f64::from(sample_rate_hz)
    }

    /// Longest scrub that still fills a strip of `visible` samples from `ring`, ms.
    #[must_use]
    pub fn max_scrub_ms(ring: &LeadRing, visible: u64) -> f64 {
        (ring.len() as u64).saturating_sub(visible) as f64 * 1000.0 / f64::from(ring.sample_rate_hz)
    }
}

/// Pointer and keyboard handling for one strip drawn in `win`:
///
/// * drag a caliper line (or the band between a pair) to move it; it snaps on release if enabled,
/// * shift-click adds a pair (1 s wide) at the pointer,
/// * click selects; ←/→ nudge the selected line 1 sample (shift: 10), Delete/Esc remove all pairs,
/// * while paused, dragging elsewhere scrubs (drag right = back in time, like pulling paper).
pub fn strip_interact(
    ui: &mut Ui,
    id: Id,
    win: &StripWindow,
    ring: &LeadRing,
    calipers: &mut CaliperSet,
    playback: &mut Playback,
) -> egui::Response {
    let resp = ui.interact(win.rect, id, Sense::click_and_drag());
    let (press_origin, shift) = ui.input(|i| (i.pointer.press_origin(), i.modifiers.shift));

    if resp.drag_started()
        && let Some(o) = press_origin
    {
        calipers.drag = match calipers.hit(win, o.x) {
            Some((pair, end)) => {
                calipers.selected = Some((pair, end));
                let offset_ms = win.ms_from_left(o.x) - calipers.pairs[pair].a_ms;
                Some(Drag::Line { pair, end, offset_ms })
            }
            None if playback.is_paused() => Some(Drag::Scrub {
                start_scrub_ms: playback.scrub_ms(),
                origin_x: o.x,
            }),
            None => None,
        };
    }
    if resp.dragged()
        && let Some(p) = resp.interact_pointer_pos()
    {
        match calipers.drag {
            Some(Drag::Line { pair, end, offset_ms }) => {
                if let Some(c) = calipers.pairs.get_mut(pair) {
                    let ms = win.ms_from_left(p.x.clamp(win.rect.left(), win.rect.right()));
                    match end {
                        CaliperEnd::A => c.a_ms = ms,
                        CaliperEnd::B => c.b_ms = ms,
                        CaliperEnd::Both => {
                            let w = c.b_ms - c.a_ms;
                            c.a_ms = ms - offset_ms;
                            c.b_ms = c.a_ms + w;
                        }
                    }
                }
            }
            Some(Drag::Scrub {
                start_scrub_ms,
                origin_x,
            }) => {
                let max = Playback::max_scrub_ms(ring, win.visible_samples);
                let delta_ms = f64::from((p.x - origin_x) / win.px_per_ms());
                playback.set_scrub_ms(start_scrub_ms + delta_ms, max);
            }
            None => {}
        }
    }
    if resp.drag_stopped() {
        if let Some(Drag::Line { pair, end, .. }) = calipers.drag
            && calipers.snap
            && let Some(c) = calipers.pairs.get_mut(pair)
        {
            match end {
                CaliperEnd::A => c.a_ms = snap_ms(win, ring, c.a_ms),
                CaliperEnd::B => c.b_ms = snap_ms(win, ring, c.b_ms),
                CaliperEnd::Both => {}
            }
        }
        calipers.drag = None;
    }
    if resp.clicked() {
        resp.request_focus();
        if let Some(p) = resp.interact_pointer_pos() {
            if shift {
                let a = win.ms_from_left(p.x);
                let b = (a + 1000.0).min(win.ms_from_left(win.rect.right()));
                calipers.add_pair(a, b);
            } else if let Some(hit) = calipers.hit(win, p.x) {
                calipers.selected = Some(hit);
            }
        }
    }
    if resp.has_focus() {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                EventFilter {
                    horizontal_arrows: true,
                    escape: true,
                    ..EventFilter::default()
                },
            );
        });
        let step_ms = 1000.0 / win.sample_rate_hz;
        let (left, right, big, clear) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.modifiers.shift,
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Escape),
            )
        });
        let k = if big { 10.0 } else { 1.0 };
        if left {
            calipers.nudge(-k * step_ms);
        }
        if right {
            calipers.nudge(k * step_ms);
        }
        if clear {
            calipers.reset();
        }
    }
    resp
}

/// Paint the caliper lines, the band between each pair and its read-out.
pub fn draw_calipers(
    ui: &Ui,
    win: &StripWindow,
    ring: &LeadRing,
    invert: bool,
    calipers: &CaliperSet,
    style: &StripStyle,
) {
    let painter = ui.painter_at(win.rect);
    let r = win.rect;
    for (i, c) in calipers.pairs.iter().enumerate() {
        let (xa, xb) = (win.x_from_left_ms(c.a_ms), win.x_from_left_ms(c.b_ms));
        painter.rect_filled(
            egui::Rect::from_x_y_ranges(xa.min(xb)..=xa.max(xb), r.y_range()),
            0.0,
            style.caliper.gamma_multiply(0.08),
        );
        for (end, x) in [(CaliperEnd::A, xa), (CaliperEnd::B, xb)] {
            let sel = calipers.selected == Some((i, end)) || calipers.selected == Some((i, CaliperEnd::Both));
            painter.line_segment(
                [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
                Stroke::new(if sel { 2.0 } else { 1.2 }, style.caliper),
            );
        }
        let y = r.top() + 14.0 + 13.0 * (i % 3) as f32;
        painter.line_segment(
            [Pos2::new(xa, y), Pos2::new(xb, y)],
            Stroke::new(1.0, style.caliper),
        );
        if let Some(rd) = calipers.reading(i, win, ring, invert) {
            let galley = painter.layout_no_wrap(rd.text(), FontId::monospace(11.0), style.caliper);
            let x = f32::midpoint(xa, xb).clamp(r.left() + 60.0, r.right() - 60.0);
            let at = Align2::CENTER_BOTTOM.anchor_size(Pos2::new(x, y - 1.0), galley.size());
            painter.rect_filled(
                at.expand2(Vec2::new(2.0, 0.0)),
                2.0,
                style.paper.gamma_multiply(0.85),
            );
            painter.galley(at.min, galley, style.caliper);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Rect, Vec2};

    #[test]
    fn caliper_rate_is_sixty_thousand_over_interval() {
        let c = Caliper {
            a_ms: 200.0,
            b_ms: 1200.0,
        };
        assert!((c.interval_ms() - 1000.0).abs() < 1e-3);
        assert!((c.rate_bpm().unwrap() - 60.0).abs() < 1e-3);
        assert_eq!(Caliper { a_ms: 5.0, b_ms: 5.0 }.rate_bpm(), None);
    }

    #[test]
    fn nudge_moves_only_the_selected_line() {
        let mut s = CaliperSet::default();
        s.add_pair(100.0, 900.0);
        s.nudge(2.0);
        assert_eq!(
            s.pairs[0],
            Caliper {
                a_ms: 100.0,
                b_ms: 902.0
            }
        );
        s.selected = Some((0, CaliperEnd::Both));
        s.nudge(-10.0);
        assert_eq!(
            s.pairs[0],
            Caliper {
                a_ms: 90.0,
                b_ms: 892.0
            }
        );
        s.reset();
        assert!(s.pairs.is_empty() && s.selected.is_none());
    }

    #[test]
    fn scrub_is_clamped_and_only_while_paused() {
        let mut p = Playback::default();
        p.set_scrub_ms(500.0, 10_000.0);
        assert!(p.scrub_ms().abs() < f64::EPSILON, "live view cannot scrub");
        p.pause();
        p.set_scrub_ms(50_000.0, 10_000.0);
        assert!((p.scrub_ms() - 10_000.0).abs() < 1e-9);
        assert_eq!(p.head(20_000, 500.0), 15_000);
        p.toggle();
        assert!(!p.is_paused() && p.scrub_ms().abs() < f64::EPSILON);
    }

    #[test]
    fn snap_finds_the_spike_within_radius() {
        let mut ring = LeadRing::new(5000, 500.0);
        for k in 0..5000 {
            ring.push(if k == 4000 { 1.5 } else { 0.0 });
        }
        let style = StripStyle::default();
        let win = StripWindow::new(
            Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(750.0, 100.0)),
            &style,
            500.0,
            5000,
        );
        let x_spike = win.x_of_sample(4000).unwrap();
        let near = win.ms_from_left(x_spike) + 30.0;
        let snapped = snap_ms(&win, &ring, near);
        assert!((win.x_from_left_ms(snapped) - x_spike).abs() < 1e-3);
    }
}
