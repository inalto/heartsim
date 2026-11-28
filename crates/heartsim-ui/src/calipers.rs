//! Calipers and pause/scrub for the ECG strips.
//!
//! A caliper pair is two vertical lines measured in milliseconds, so a pair keeps its time meaning
//! when the paper speed changes. On live paper the origin is the strip's left edge, so a pair
//! stays where the student put it while the paper scrolls (like real calipers on a monitor). On a
//! recorded strip ([`StripWindow::record`]) the origin is the **start of the record**
//! ([`StripWindow::caliper_ms_at_x`]), so a line stays on its sample while the record is zoomed
//! or panned and only its screen x moves. The read-out gives Δt, the rate that interval
//! corresponds to, and the trace voltage under each line.
//!
//! Each line has a grip at the top and at the bottom of the strip (the handle, [`handle_rect`])
//! and the band between a pair has a grip on its read-out bar ([`band_grip_rect`]). Targets are
//! generous (see [`CaliperSet::hit`]) and the pointer shows what a press would do.

use crate::strip::{StripStyle, StripWindow};
use egui::{Align2, Color32, CursorIcon, EventFilter, FontId, Id, Key, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use heartsim_ecg::LeadRing;
use std::fmt::Write as _;

/// Search half-width when a released caliper line snaps to the nearest deflection, ms.
pub const SNAP_RADIUS_MS: f32 = 40.0;
/// Pointer distance that grabs a caliper line, screen points.
const GRAB_PX: f32 = 6.0;
/// Drawn size of a line's handle (a rounded tab at the top and at the bottom of the line), points.
pub const HANDLE_SIZE: Vec2 = Vec2::new(12.0, 18.0);
/// Drawn size of the selected line's handle, points.
const HANDLE_SIZE_SELECTED: Vec2 = Vec2::new(16.0, 24.0);
/// Drawn size of the band's grip on the read-out bar, points.
const BAND_GRIP_SIZE: Vec2 = Vec2::new(22.0, 10.0);
/// Smallest side of a handle's or grip's hit rectangle with a mouse, points.
pub const MIN_TARGET: f32 = 24.0;
/// Smallest side of a handle's or grip's hit rectangle while a finger is on the screen, points
/// (the usual 44-point touch target).
pub const TOUCH_TARGET: f32 = 44.0;
/// Distance between the top of the strip and the read-out bar of the first pair, points.
const BAR_TOP: f32 = 14.0;
/// Vertical spacing of the read-out bars of stacked pairs, points.
const BAR_STEP: f32 = 13.0;

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
        /// Pointer-to-line offset at grab time, ms (line A for `Both`): a line grabbed by its
        /// handle, off the line itself, does not jump to the pointer.
        offset_ms: f32,
    },
    Scrub {
        start_scrub_ms: f64,
        origin_x: f32,
    },
    /// Panning a recorded strip: the record window's first sample when the drag began.
    Pan {
        start_first: u64,
        origin_x: f32,
    },
}

/// What a drag on free paper does.
enum Free<'a> {
    /// Scrub the paused live strip.
    Scrub(&'a mut Playback),
    /// Pan a recorded strip: the record window's first sample.
    Pan(&'a mut u64),
}

/// The handle of the caliper line at screen `x`, drawn, at the top (`top`) or the bottom of
/// `win`'s strip, `size` large.
fn tab_rect(win: &StripWindow, x: f32, top: bool, size: Vec2) -> Rect {
    let y = if top {
        win.rect.top()
    } else {
        win.rect.bottom() - size.y
    };
    Rect::from_min_size(Pos2::new(x - size.x / 2.0, y), size)
}

/// The drawn handle of the caliper line at screen `x`, at the top (`top`) or the bottom of the
/// strip: a tab [`HANDLE_SIZE`] large with the line running through it.
#[must_use]
pub fn handle_rect(win: &StripWindow, x: f32, top: bool) -> Rect {
    tab_rect(win, x, top, HANDLE_SIZE)
}

/// `r` enlarged around its centre to at least `min` points on each side.
fn at_least(r: Rect, min: f32) -> Rect {
    Rect::from_center_size(r.center(), Vec2::new(r.width().max(min), r.height().max(min)))
}

/// Where a handle or grip can be grabbed: its drawn rectangle enlarged to at least
/// [`MIN_TARGET`] points square, [`TOUCH_TARGET`] when `touch` (a finger is down).
#[must_use]
pub fn target_rect(drawn: Rect, touch: bool) -> Rect {
    at_least(drawn, if touch { TOUCH_TARGET } else { MIN_TARGET })
}

/// Height of the read-out bar of pair `i`, screen y.
fn bar_y(win: &StripWindow, i: usize) -> f32 {
    win.rect.top() + BAR_TOP + BAR_STEP * (i % 3) as f32
}

/// The drawn grip of the band of pair `c` (number `i`) on its read-out bar: in the middle of the
/// part of the band that is on screen, or `None` when none of it is.
#[must_use]
pub fn band_grip_rect(win: &StripWindow, i: usize, c: Caliper) -> Option<Rect> {
    let (xa, xb) = (win.x_of_caliper_ms(c.a_ms), win.x_of_caliper_ms(c.b_ms));
    let (lo, hi) = (xa.min(xb).max(win.rect.left()), xa.max(xb).min(win.rect.right()));
    (lo < hi).then(|| Rect::from_center_size(Pos2::new(f32::midpoint(lo, hi), bar_y(win, i)), BAND_GRIP_SIZE))
}

/// Luminance-based outline colour that shows against `fill` and against the paper or monitor.
fn outline_for(fill: Color32) -> Color32 {
    let lum = 0.299 * f32::from(fill.r()) + 0.587 * f32::from(fill.g()) + 0.114 * f32::from(fill.b());
    if lum > 140.0 {
        Color32::from_black_alpha(220)
    } else {
        Color32::from_white_alpha(230)
    }
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
            let s = win.sample_at_x(win.x_of_caliper_ms(ms))?;
            ring.at(s).map(|v| sign * v)
        };
        Some(CaliperReading {
            interval_ms: c.interval_ms(),
            rate_bpm: c.rate_bpm(),
            a_mv: mv(c.a_ms),
            b_mv: mv(c.b_ms),
        })
    }

    /// Move the selection to the next (`+1`) or previous (`-1`) line, in the order A, B of pair 0,
    /// then pair 1, and so on, wrapping round. With nothing selected (or a whole pair) `+1` takes
    /// the first line and `-1` the last.
    pub fn select_step(&mut self, dir: i32) {
        let n = self.pairs.len() * 2;
        if n == 0 {
            return;
        }
        let flat = |(i, end): (usize, CaliperEnd)| match end {
            CaliperEnd::A => Some(i * 2),
            CaliperEnd::B => Some(i * 2 + 1),
            CaliperEnd::Both => None,
        };
        let next = match self.selected.and_then(flat) {
            Some(k) if dir >= 0 => (k + 1) % n,
            Some(k) => (k + n - 1) % n,
            None if dir >= 0 => 0,
            None => n - 1,
        };
        let end = if next % 2 == 0 {
            CaliperEnd::A
        } else {
            CaliperEnd::B
        };
        self.selected = Some((next / 2, end));
    }

    /// What a press at `pos` grabs: the single place that decides.
    ///
    /// * A line is grabbed within [`GRAB_PX`] of it along the strip's whole height, or inside its
    ///   top or bottom handle's target ([`target_rect`]: at least 24 points square, 44 with
    ///   `touch`). When several lines qualify the one nearer to `pos.x` wins.
    /// * Only if no line qualifies, the band of the first pair whose lines enclose `pos.x` is
    ///   grabbed (`Both`).
    #[must_use]
    pub fn hit(&self, win: &StripWindow, pos: Pos2, touch: bool) -> Option<(usize, CaliperEnd)> {
        let mut best: Option<(f32, usize, CaliperEnd)> = None;
        for (i, c) in self.pairs.iter().enumerate() {
            for (end, ms) in [(CaliperEnd::A, c.a_ms), (CaliperEnd::B, c.b_ms)] {
                let x = win.x_of_caliper_ms(ms);
                let d = (x - pos.x).abs();
                let on_handle = [true, false]
                    .into_iter()
                    .any(|top| target_rect(handle_rect(win, x, top), touch).contains(pos));
                if (d <= GRAB_PX || on_handle) && best.is_none_or(|b| d < b.0) {
                    best = Some((d, i, end));
                }
            }
        }
        if let Some((_, i, end)) = best {
            return Some((i, end));
        }
        let ms = win.caliper_ms_at_x(pos.x);
        self.pairs
            .iter()
            .position(|c| (c.a_ms.min(c.b_ms)..=c.a_ms.max(c.b_ms)).contains(&ms))
            .map(|i| (i, CaliperEnd::Both))
    }

    /// The pointer shape for hovering at `pos`: the horizontal resize arrows over a line or its
    /// handle, an open hand over a band's grip, nothing elsewhere.
    #[must_use]
    pub fn cursor_at(&self, win: &StripWindow, pos: Pos2, touch: bool) -> Option<CursorIcon> {
        match self.hit(win, pos, touch)? {
            (_, CaliperEnd::A | CaliperEnd::B) => Some(CursorIcon::ResizeHorizontal),
            (i, CaliperEnd::Both) => band_grip_rect(win, i, self.pairs[i])
                .is_some_and(|g| target_rect(g, touch).contains(pos))
                .then_some(CursorIcon::Grab),
        }
    }
}

/// Move a caliper line at `ms` to the largest deflection (from the local mean) within
/// [`SNAP_RADIUS_MS`]; unchanged if no trace is under it.
#[must_use]
pub fn snap_ms(win: &StripWindow, ring: &LeadRing, ms: f32) -> f32 {
    let Some(centre) = win.sample_at_x(win.x_of_caliper_ms(ms)) else {
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
        .map_or(ms, |x| win.caliper_ms_at_x(x))
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

/// Pointer and keyboard handling for one live strip drawn in `win`:
///
/// * drag a caliper line by its line or its handle, or the band between a pair by its grip or the
///   band itself, to move it; it snaps on release if enabled,
/// * shift-click adds a pair (1 s wide) at the pointer,
/// * click selects; ←/→ nudge the selected line 1 sample (shift: 10), `[` and `]` select the
///   previous and the next line, Delete/Esc remove all pairs,
/// * while paused, dragging elsewhere scrubs (drag right = back in time, like pulling paper).
///
/// The pointer shows what a press would do ([`CaliperSet::cursor_at`]) and while two fingers are
/// down (a pinch) no drag starts.
pub fn strip_interact(
    ui: &mut Ui,
    id: Id,
    win: &StripWindow,
    ring: &LeadRing,
    calipers: &mut CaliperSet,
    playback: &mut Playback,
) -> egui::Response {
    interact(ui, id, win, ring, calipers, Free::Scrub(playback))
}

/// [`strip_interact`] for a recorded strip ([`StripWindow::record`]): dragging on free paper pans
/// the record instead of scrubbing. `first` is the record window's first sample, moved here
/// (clamped to the record) so that drag right = back in time.
pub fn record_interact(
    ui: &mut Ui,
    id: Id,
    win: &StripWindow,
    ring: &LeadRing,
    calipers: &mut CaliperSet,
    first: &mut u64,
) -> egui::Response {
    interact(ui, id, win, ring, calipers, Free::Pan(first))
}

fn interact(
    ui: &mut Ui,
    id: Id,
    win: &StripWindow,
    ring: &LeadRing,
    calipers: &mut CaliperSet,
    mut free: Free<'_>,
) -> egui::Response {
    let resp = ui.interact(win.rect, id, Sense::click_and_drag());
    let (press_origin, shift, touch) =
        ui.input(|i| (i.pointer.press_origin(), i.modifiers.shift, i.any_touches()));
    // two fingers on the strip pinch it (zoom); they never start or continue a drag
    let pinch = crate::viewport::pinching(ui);
    if pinch {
        calipers.drag = None;
    }

    if resp.drag_started()
        && !pinch
        && let Some(o) = press_origin
    {
        calipers.drag = match calipers.hit(win, o, touch) {
            Some((pair, end)) => {
                calipers.selected = Some((pair, end));
                let c = calipers.pairs[pair];
                let line_ms = match end {
                    CaliperEnd::A | CaliperEnd::Both => c.a_ms,
                    CaliperEnd::B => c.b_ms,
                };
                Some(Drag::Line {
                    pair,
                    end,
                    offset_ms: win.caliper_ms_at_x(o.x) - line_ms,
                })
            }
            None => match &free {
                Free::Scrub(playback) if playback.is_paused() => Some(Drag::Scrub {
                    start_scrub_ms: playback.scrub_ms(),
                    origin_x: o.x,
                }),
                Free::Pan(first) => Some(Drag::Pan {
                    start_first: **first,
                    origin_x: o.x,
                }),
                Free::Scrub(_) => None,
            },
        };
    }
    if resp.dragged()
        && let Some(p) = resp.interact_pointer_pos()
    {
        match (calipers.drag, &mut free) {
            (Some(Drag::Line { pair, end, offset_ms }), _) => {
                if let Some(c) = calipers.pairs.get_mut(pair) {
                    let ms = win.caliper_ms_at_x(p.x.clamp(win.rect.left(), win.rect.right())) - offset_ms;
                    match end {
                        CaliperEnd::A => c.a_ms = ms,
                        CaliperEnd::B => c.b_ms = ms,
                        CaliperEnd::Both => {
                            let w = c.b_ms - c.a_ms;
                            c.a_ms = ms;
                            c.b_ms = ms + w;
                        }
                    }
                }
            }
            (
                Some(Drag::Scrub {
                    start_scrub_ms,
                    origin_x,
                }),
                Free::Scrub(playback),
            ) => {
                let max = Playback::max_scrub_ms(ring, win.visible_samples);
                let delta_ms = f64::from((p.x - origin_x) / win.px_per_ms());
                playback.set_scrub_ms(start_scrub_ms + delta_ms, max);
            }
            (
                Some(Drag::Pan {
                    start_first,
                    origin_x,
                }),
                Free::Pan(first),
            ) => {
                let moved = f64::from((p.x - origin_x) / win.px_per_sample);
                let target = (start_first as f64 - moved).round().max(0.0) as u64;
                **first = target.min(win.max_record_first());
            }
            _ => {}
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
    // the pointer shape: held during a drag, otherwise what a press here would do
    let cursor = match calipers.drag {
        Some(
            Drag::Line {
                end: CaliperEnd::Both,
                ..
            }
            | Drag::Pan { .. },
        ) => Some(CursorIcon::Grabbing),
        Some(Drag::Line { .. }) => Some(CursorIcon::ResizeHorizontal),
        Some(Drag::Scrub { .. }) => None,
        None => ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| win.rect.contains(*p))
            .and_then(|p| calipers.cursor_at(win, p, touch)),
    };
    if let Some(c) = cursor {
        ui.ctx().set_cursor_icon(c);
    }
    if resp.clicked() {
        resp.request_focus();
        if let Some(p) = resp.interact_pointer_pos() {
            if shift {
                let a = win.caliper_ms_at_x(p.x);
                let b = (a + 1000.0).min(win.caliper_ms_at_x(win.rect.right()));
                calipers.add_pair(a, b);
            } else if let Some(hit) = calipers.hit(win, p, touch) {
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
        let (left, right, big, clear, prev, next) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.modifiers.shift,
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Escape),
                i.key_pressed(Key::OpenBracket),
                i.key_pressed(Key::CloseBracket),
            )
        });
        let k = if big { 10.0 } else { 1.0 };
        if left {
            calipers.nudge(-k * step_ms);
        }
        if right {
            calipers.nudge(k * step_ms);
        }
        if prev {
            calipers.select_step(-1);
        }
        if next {
            calipers.select_step(1);
        }
        if clear {
            calipers.reset();
        }
    }
    resp
}

/// Paint one handle: a rounded tab in the caliper colour with a contrasting outline, the line
/// running through it.
fn paint_handle(painter: &egui::Painter, rect: Rect, fill: Color32) {
    painter.rect(
        rect,
        4.0,
        fill,
        Stroke::new(1.5, outline_for(fill)),
        egui::StrokeKind::Outside,
    );
    // two short notches say "grip"
    let notch = outline_for(fill).gamma_multiply(0.8);
    for dx in [-1.5, 1.5] {
        let x = rect.center().x + dx;
        painter.line_segment(
            [
                Pos2::new(x, rect.center().y - 3.0),
                Pos2::new(x, rect.center().y + 3.0),
            ],
            Stroke::new(1.0, notch),
        );
    }
}

/// Paint the caliper lines, their handles, the band between each pair with its grip, and the
/// read-out.
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
        let (xa, xb) = (win.x_of_caliper_ms(c.a_ms), win.x_of_caliper_ms(c.b_ms));
        painter.rect_filled(
            Rect::from_x_y_ranges(xa.min(xb)..=xa.max(xb), r.y_range()),
            0.0,
            style.caliper.gamma_multiply(0.08),
        );
        for (end, x) in [(CaliperEnd::A, xa), (CaliperEnd::B, xb)] {
            let sel = calipers.selected == Some((i, end)) || calipers.selected == Some((i, CaliperEnd::Both));
            painter.line_segment(
                [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
                Stroke::new(if sel { 2.0 } else { 1.2 }, style.caliper),
            );
            let size = if sel { HANDLE_SIZE_SELECTED } else { HANDLE_SIZE };
            for top in [true, false] {
                paint_handle(&painter, tab_rect(win, x, top, size), style.caliper);
            }
        }
        let y = bar_y(win, i);
        painter.line_segment(
            [Pos2::new(xa, y), Pos2::new(xb, y)],
            Stroke::new(1.0, style.caliper),
        );
        if let Some(g) = band_grip_rect(win, i, *c) {
            let both = calipers.selected == Some((i, CaliperEnd::Both));
            paint_handle(&painter, if both { g.expand(1.5) } else { g }, style.caliper);
        }
        if let Some(rd) = calipers.reading(i, win, ring, invert) {
            let galley = painter.layout_no_wrap(rd.text(), FontId::monospace(11.0), style.caliper);
            let x = f32::midpoint(xa, xb).clamp(r.left() + 60.0, r.right() - 60.0);
            let at = Align2::CENTER_BOTTOM
                .anchor_size(Pos2::new(x, y - 1.0 - BAND_GRIP_SIZE.y / 2.0), galley.size());
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

    fn win_750() -> StripWindow {
        StripWindow::new(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(750.0, 100.0)),
            &StripStyle::default(),
            500.0,
            5000,
        )
    }

    /// A set with one pair whose lines are at screen x `xa` and `xb`.
    fn set_at(win: &StripWindow, xa: f32, xb: f32) -> CaliperSet {
        let mut s = CaliperSet::default();
        s.add_pair(win.caliper_ms_at_x(xa), win.caliper_ms_at_x(xb));
        s
    }

    #[test]
    fn a_line_is_hit_within_six_points_at_mid_height() {
        let win = win_750();
        let s = set_at(&win, 300.0, 600.0);
        let y = win.rect.center().y;
        assert_eq!(s.hit(&win, Pos2::new(305.9, y), false), Some((0, CaliperEnd::A)));
        assert_eq!(s.hit(&win, Pos2::new(294.1, y), false), Some((0, CaliperEnd::A)));
        assert_eq!(s.hit(&win, Pos2::new(594.0, y), false), Some((0, CaliperEnd::B)));
        // seven points away is the band (inside) or nothing (outside)
        assert_eq!(
            s.hit(&win, Pos2::new(307.5, y), false),
            Some((0, CaliperEnd::Both))
        );
        assert_eq!(s.hit(&win, Pos2::new(292.5, y), false), None);
    }

    #[test]
    fn a_handle_is_hit_within_its_enlarged_rect_at_the_top_and_the_bottom() {
        let win = win_750();
        let s = set_at(&win, 300.0, 600.0);
        for top in [true, false] {
            let h = handle_rect(&win, 300.0, top);
            // 11 points beside the line, level with the handle: inside the 24 x 24 target
            let p = Pos2::new(300.0 + 11.0, h.center().y);
            assert_eq!(s.hit(&win, p, false), Some((0, CaliperEnd::A)), "top {top}");
            // 13 points beside it: outside, so the band
            let p = Pos2::new(300.0 + 13.0, h.center().y);
            assert_eq!(s.hit(&win, p, false), Some((0, CaliperEnd::Both)), "top {top}");
            // the same height 11 points beside the line, but mid-strip, is not on a handle
            let p = Pos2::new(300.0 + 11.0, win.rect.center().y);
            assert_eq!(s.hit(&win, p, false), Some((0, CaliperEnd::Both)));
        }
    }

    #[test]
    fn a_touch_target_is_at_least_forty_four_points() {
        let win = win_750();
        let s = set_at(&win, 300.0, 600.0);
        let h = handle_rect(&win, 300.0, true);
        let t = target_rect(h, true);
        assert!(t.width() >= 44.0 && t.height() >= 44.0, "{t:?}");
        assert!(target_rect(h, false).width() >= 24.0);
        let p = Pos2::new(300.0 + 21.0, h.center().y);
        assert_eq!(
            s.hit(&win, p, true),
            Some((0, CaliperEnd::A)),
            "a finger 21 points off"
        );
        assert_eq!(
            s.hit(&win, p, false),
            Some((0, CaliperEnd::Both)),
            "the mouse target is smaller"
        );
    }

    #[test]
    fn of_two_lines_eight_points_apart_the_nearer_wins() {
        let win = win_750();
        let s = set_at(&win, 300.0, 308.0);
        let y = win.rect.center().y;
        assert_eq!(s.hit(&win, Pos2::new(303.0, y), false), Some((0, CaliperEnd::A)));
        assert_eq!(s.hit(&win, Pos2::new(305.5, y), false), Some((0, CaliperEnd::B)));
        let top = handle_rect(&win, 300.0, true).center().y;
        assert_eq!(
            s.hit(&win, Pos2::new(302.0, top), false),
            Some((0, CaliperEnd::A))
        );
        assert_eq!(
            s.hit(&win, Pos2::new(306.0, top), false),
            Some((0, CaliperEnd::B))
        );
    }

    #[test]
    fn the_band_is_hit_only_outside_both_line_targets() {
        let win = win_750();
        let s = set_at(&win, 300.0, 600.0);
        let y = win.rect.center().y;
        assert_eq!(
            s.hit(&win, Pos2::new(450.0, y), false),
            Some((0, CaliperEnd::Both))
        );
        assert_eq!(
            s.hit(&win, Pos2::new(307.0, y), false),
            Some((0, CaliperEnd::Both))
        );
        assert_eq!(s.hit(&win, Pos2::new(303.0, y), false), Some((0, CaliperEnd::A)));
        assert_eq!(s.hit(&win, Pos2::new(700.0, y), false), None);
    }

    #[test]
    fn a_recorded_strip_keeps_a_line_on_its_sample_when_panned() {
        let style = StripStyle::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(750.0, 100.0));
        let w0 = StripWindow::record(rect, &style, 500.0, 0, 50_000);
        let w1 = StripWindow::record(rect, &style, 500.0, 400, 50_000);
        let ms = w0.caliper_ms_at_x(300.0);
        let s0 = w0.sample_at_x(w0.x_of_caliper_ms(ms));
        let x1 = w1.x_of_caliper_ms(ms);
        assert!(x1 < 300.0, "panned later: the line moves left");
        assert_eq!(w1.sample_at_x(x1), s0);
    }

    #[test]
    fn the_selection_steps_through_the_lines_and_wraps() {
        let mut s = CaliperSet::default();
        s.select_step(1);
        assert_eq!(s.selected, None, "no pairs");
        s.add_pair(100.0, 200.0);
        s.add_pair(300.0, 400.0);
        s.selected = None;
        s.select_step(1);
        assert_eq!(s.selected, Some((0, CaliperEnd::A)));
        s.select_step(1);
        s.select_step(1);
        s.select_step(1);
        assert_eq!(s.selected, Some((1, CaliperEnd::B)));
        s.select_step(1);
        assert_eq!(s.selected, Some((0, CaliperEnd::A)), "wraps");
        s.select_step(-1);
        assert_eq!(s.selected, Some((1, CaliperEnd::B)));
        s.selected = None;
        s.select_step(-1);
        assert_eq!(s.selected, Some((1, CaliperEnd::B)));
    }
}
