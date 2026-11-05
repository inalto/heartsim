//! ECG strip painter: paper grid + min/max-decimated trace, drawn with `egui::Painter` (no
//! egui_plot — see docs/research/rust-stack.md §3 for why).
//!
//! Time on a strip is the absolute sample index of [`LeadRing::pushed`]: sample `k` was taken at
//! `k / fs` seconds. A [`StripWindow`] maps sample indices to screen x for the two display modes
//! (scrolling paper and monitor sweep bar), so the trace, wave labels, calipers and scrubbing all
//! share one mapping and can never disagree.

use crate::annotations::LeadMarks;
use crate::t;
use egui::{Align2, Color32, FontId, Id, Mesh, Pos2, Rect, Shape, Stroke, Ui, Vec2};
use heartsim_ecg::{LeadRing, WaveKind};
use std::sync::Arc;

/// Paper speeds offered in the UI, mm/s (25 is the clinical standard).
pub const SPEEDS_MM_S: [f32; 5] = [12.5, 25.0, 50.0, 100.0, 200.0];
/// Gains offered in the UI, mm/mV (10 is the clinical standard).
pub const GAINS_MM_MV: [f32; 3] = [5.0, 10.0, 20.0];

/// How the trace moves across the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SweepMode {
    /// Paper moves left; the newest sample is always at the right edge.
    #[default]
    Scroll,
    /// Bedside-monitor style: the trace stays put and an erase bar sweeps left → right, overwriting
    /// the previous sweep.
    Sweep,
}

impl SweepMode {
    /// UI label.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Scroll => t!("sweep-scroll"),
            Self::Sweep => t!("sweep-sweep"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StripStyle {
    /// Paper speed, mm/s (25 standard).
    pub mm_per_s: f32,
    /// Gain, mm/mV (10 standard).
    pub mm_per_mv: f32,
    /// Screen points per paper millimetre (calibrate per monitor; 3.78 ≈ a true 96-dpi point).
    /// Points, not physical pixels, so the paper keeps its size on HiDPI screens.
    pub px_per_mm: f32,
    /// Scrolling paper or sweep bar.
    pub mode: SweepMode,
    /// Draw the 1 mm / 5 mm grid.
    pub show_grid: bool,
    pub trace: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub paper: Color32,
    /// Wave labels and interval brackets.
    pub marker: Color32,
    /// Caliper lines and read-outs.
    pub caliper: Color32,
    pub trace_width: f32,
}

impl Default for StripStyle {
    fn default() -> Self {
        Self::paper()
    }
}

impl StripStyle {
    /// Pink ECG paper with a black trace (the printout look).
    #[must_use]
    pub fn paper() -> Self {
        Self {
            mm_per_s: 25.0,
            mm_per_mv: 10.0,
            px_per_mm: 3.0,
            mode: SweepMode::Scroll,
            show_grid: true,
            trace: Color32::from_rgb(20, 20, 20),
            grid_minor: Color32::from_rgb(250, 210, 210),
            grid_major: Color32::from_rgb(240, 150, 150),
            paper: Color32::from_rgb(255, 247, 247),
            marker: Color32::from_rgb(30, 70, 200),
            caliper: Color32::from_rgb(0, 140, 60),
            trace_width: 1.4,
        }
    }

    /// Dark bedside-monitor look: green trace on black, dim grid. Keeps the paper scales.
    #[must_use]
    pub fn monitor(self) -> Self {
        Self {
            trace: Color32::from_rgb(80, 255, 120),
            grid_minor: Color32::from_rgb(18, 34, 24),
            grid_major: Color32::from_rgb(34, 66, 44),
            paper: Color32::from_rgb(6, 10, 8),
            marker: Color32::from_rgb(120, 190, 255),
            caliper: Color32::from_rgb(255, 210, 60),
            ..self
        }
    }

    /// Back to the paper colours, keeping the scales.
    #[must_use]
    pub fn with_paper_colours(self) -> Self {
        let p = Self::paper();
        Self {
            trace: p.trace,
            grid_minor: p.grid_minor,
            grid_major: p.grid_major,
            paper: p.paper,
            marker: p.marker,
            caliper: p.caliper,
            ..self
        }
    }

    /// True for the monitor colour scheme.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.paper.r() < 128
    }
}

/// Mapping between absolute sample indices and screen coordinates for one strip.
///
/// In scroll mode the window is `head - visible .. head`, right-aligned. In sweep mode the strip
/// is split into pages of `visible` samples: the current page `page_start .. head` is drawn from
/// the left edge up to the erase bar, the remainder of the previous page to its right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StripWindow {
    pub rect: Rect,
    pub mode: SweepMode,
    /// Absolute index one past the newest sample shown (right edge, or the sweep bar).
    pub head: u64,
    pub sample_rate_hz: f32,
    /// Horizontal screen points per sample (paper speed × calibration / fs).
    pub px_per_sample: f32,
    /// Whole samples that fit across the strip.
    pub visible_samples: u64,
    /// Vertical screen points per millivolt (gain × calibration).
    pub px_per_mv: f32,
}

impl StripWindow {
    #[must_use]
    pub fn new(rect: Rect, style: &StripStyle, sample_rate_hz: f32, head: u64) -> Self {
        let px_per_sample = style.mm_per_s * style.px_per_mm / sample_rate_hz.max(1.0);
        Self {
            rect,
            mode: style.mode,
            head,
            sample_rate_hz,
            px_per_sample,
            visible_samples: (rect.width().max(0.0) / px_per_sample).floor() as u64,
            px_per_mv: style.mm_per_mv * style.px_per_mm,
        }
    }

    /// Seconds of signal across the strip width.
    #[must_use]
    pub fn seconds_visible(&self) -> f32 {
        self.rect.width() / (self.px_per_sample * self.sample_rate_hz)
    }

    /// Horizontal screen points per millisecond.
    #[must_use]
    pub fn px_per_ms(&self) -> f32 {
        self.px_per_sample * self.sample_rate_hz / 1000.0
    }

    /// Screen y of a voltage; 0 mV is the strip's vertical centre.
    #[must_use]
    pub fn y_of_mv(&self, mv: f32) -> f32 {
        self.rect.center().y - mv * self.px_per_mv
    }

    /// Width of the sweep-mode erase gap, samples (~2.5 % of the strip).
    #[must_use]
    pub fn erase_samples(&self) -> u64 {
        (self.visible_samples / 40).max(1)
    }

    /// Screen x of the sweep bar (sweep mode) or the right edge (scroll mode).
    #[must_use]
    pub fn head_x(&self) -> f32 {
        match self.mode {
            SweepMode::Scroll => self.rect.right(),
            SweepMode::Sweep => {
                let n = self.visible_samples.max(1);
                self.rect.left() + (self.head % n) as f32 * self.px_per_sample
            }
        }
    }

    /// Contiguous runs of samples on screen: `(first, end, x of first)`; at most two (sweep mode).
    pub fn segments(&self) -> impl Iterator<Item = (u64, u64, f32)> + use<> {
        let mut out = [(0, 0, 0.0); 2];
        let mut len = 0;
        let n = self.visible_samples;
        if n > 0 {
            match self.mode {
                SweepMode::Scroll => {
                    let start = self.head.saturating_sub(n);
                    let x0 = self.rect.right() - (self.head - start) as f32 * self.px_per_sample;
                    out[0] = (start, self.head, x0);
                    len = 1;
                }
                SweepMode::Sweep => {
                    let page = self.head - self.head % n;
                    if self.head > page {
                        out[len] = (page, self.head, self.rect.left());
                        len += 1;
                    }
                    if page >= n {
                        let prev = page - n;
                        let start = (self.head + self.erase_samples()).saturating_sub(n).max(prev);
                        if start < page {
                            let x0 = self.rect.left() + (start - prev) as f32 * self.px_per_sample;
                            out[len] = (start, page, x0);
                            len += 1;
                        }
                    }
                }
            }
        }
        out.into_iter().take(len)
    }

    /// Screen x of the centre of sample `abs`, if it is on screen.
    #[must_use]
    pub fn x_of_sample(&self, abs: u64) -> Option<f32> {
        self.segments()
            .find(|(s, e, _)| (*s..*e).contains(&abs))
            .map(|(s, _, x0)| x0 + ((abs - s) as f32 + 0.5) * self.px_per_sample)
    }

    /// Absolute index of the sample drawn at screen x, if any.
    #[must_use]
    pub fn sample_at_x(&self, x: f32) -> Option<u64> {
        self.segments().find_map(|(s, e, x0)| {
            let k = ((x - x0) / self.px_per_sample).floor();
            (k >= 0.0 && s + (k as u64) < e).then(|| s + k as u64)
        })
    }

    /// Milliseconds from the strip's left edge to screen x (caliper coordinates).
    #[must_use]
    pub fn ms_from_left(&self, x: f32) -> f32 {
        (x - self.rect.left()) / self.px_per_ms()
    }

    /// Screen x of a point `ms` milliseconds from the left edge.
    #[must_use]
    pub fn x_from_left_ms(&self, ms: f32) -> f32 {
        self.rect.left() + ms * self.px_per_ms()
    }
}

/// What to draw on one strip besides the paper.
#[derive(Debug, Clone, Copy)]
pub struct StripTrace<'a> {
    /// Lead label drawn at the top left.
    pub label: &'a str,
    /// Draw the lead upside down (−aVR in the Cabrera sequence).
    pub invert: bool,
    /// Absolute index one past the newest sample to show (see [`StripWindow::head`]).
    pub head: u64,
    /// Wave labels and intervals, when enabled.
    pub marks: Option<LeadMarks<'a>>,
    /// Draw P/R/T letters.
    pub show_waves: bool,
    /// Draw PR and RR brackets.
    pub show_intervals: bool,
}

/// Draw one lead into `rect`. The newest sample is at the right edge ("scrolling" mode).
/// Returns the number of seconds visible.
pub fn ecg_strip(ui: &mut Ui, rect: Rect, ring: &LeadRing, style: &StripStyle, label: &str) -> f32 {
    let id = ui.scope_id().with(("ecg_strip", label));
    let style = StripStyle {
        mode: SweepMode::Scroll,
        ..*style
    };
    let trace = StripTrace {
        label,
        invert: false,
        head: ring.pushed(),
        marks: None,
        show_waves: false,
        show_intervals: false,
    };
    draw_strip(ui, id, rect, ring, &style, &trace).seconds_visible()
}

/// Paint paper, trace, calibration pulse, label and (optionally) wave annotations. `id` keys the
/// cached grid mesh and must be stable per strip. Returns the window used, for interaction.
pub fn draw_strip(
    ui: &Ui,
    id: Id,
    rect: Rect,
    ring: &LeadRing,
    style: &StripStyle,
    trace: &StripTrace<'_>,
) -> StripWindow {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, style.paper);
    if style.show_grid {
        painter.add(grid_shape(ui, id, rect, style));
    }
    let win = StripWindow::new(rect, style, ring.sample_rate_hz, trace.head);
    let sign = if trace.invert { -1.0 } else { 1.0 };
    let stroke = Stroke::new(style.trace_width, style.trace);

    let mut buf = Vec::new();
    for (s, e, x0) in win.segments() {
        let first = ring.range(s, e, &mut buf);
        if buf.len() < 2 {
            continue;
        }
        let x_start = x0 + (first - s) as f32 * win.px_per_sample;
        let width = buf.len() as f32 * win.px_per_sample;
        let cols = (width.ceil() as usize).clamp(1, buf.len());
        let mut pts = TracePoints(Vec::with_capacity(cols * 2));
        if cols == buf.len() {
            for (k, v) in buf.iter().enumerate() {
                let x = x_start + (k as f32 + 0.5) * win.px_per_sample;
                pts.push(Pos2::new(x, win.y_of_mv(sign * v)));
            }
        } else {
            // Min/max per pixel column (every sample visited once, so QRS spikes survive).
            let col_w = width / cols as f32;
            for c in 0..cols {
                let col = &buf[c * buf.len() / cols..(c + 1) * buf.len() / cols];
                let (lo, hi) = col
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
                if lo > hi {
                    continue;
                }
                let x = x_start + (c as f32 + 0.5) * col_w;
                let (y_hi, y_lo) = (win.y_of_mv(sign * hi), win.y_of_mv(sign * lo));
                if (y_hi - y_lo).abs() < 0.5 {
                    pts.push(Pos2::new(x, f32::midpoint(y_hi, y_lo)));
                } else if c % 2 == 0 {
                    // alternate max/min so the polyline sweeps the full column
                    pts.push(Pos2::new(x, y_hi));
                    pts.push(Pos2::new(x, y_lo));
                } else {
                    pts.push(Pos2::new(x, y_lo));
                    pts.push(Pos2::new(x, y_hi));
                }
            }
        }
        painter.add(Shape::line(pts.0, stroke));
    }
    if win.mode == SweepMode::Sweep && trace.head > 0 {
        let x = win.head_x();
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, style.trace.gamma_multiply(0.35)),
        );
    }

    if let Some(marks) = trace.marks {
        draw_marks(&painter, &win, ring, style, trace, marks);
    }

    // 1 mV calibration pulse and label
    let cal_h = win.px_per_mv;
    let mm = style.px_per_mm;
    let cx = rect.left() + 4.0;
    let cy = rect.center().y;
    painter.add(Shape::line(
        vec![
            Pos2::new(cx, cy),
            Pos2::new(cx, cy - cal_h),
            Pos2::new(cx + 2.0 * mm, cy - cal_h),
            Pos2::new(cx + 2.0 * mm, cy),
        ],
        Stroke::new(1.0, style.trace),
    ));
    let label = if trace.invert {
        format!("−{}", trace.label)
    } else {
        trace.label.to_owned()
    };
    painter.text(
        rect.left_top() + Vec2::new(6.0, 2.0),
        Align2::LEFT_TOP,
        label,
        FontId::proportional(13.0),
        style.trace,
    );
    win
}

/// Polyline builder that drops a vertex when it lies within 0.2 pt of the straight line from
/// its predecessor to the new point (and is not a turnaround). Isoelectric stretches and smooth
/// P/T slopes collapse to a few vertices, which keeps tessellation inside the frame budget; QRS
/// spikes and min/max columns are never thinned because their vertices are far off any chord.
struct TracePoints(Vec<Pos2>);

impl TracePoints {
    const TOLERANCE_PT: f32 = 0.2;

    fn push(&mut self, p: Pos2) {
        let n = self.0.len();
        if n >= 2 {
            let (a, b) = (self.0[n - 2], self.0[n - 1]);
            let ab = p - a;
            let len = ab.length();
            let ahead = (b - a).dot(ab) > 0.0 && (p - b).dot(ab) > 0.0;
            if ahead && len > 0.0 && ((b - a).x * ab.y - (b - a).y * ab.x).abs() / len < Self::TOLERANCE_PT {
                self.0[n - 1] = p;
                return;
            }
        }
        self.0.push(p);
    }
}

/// P/R/T letters above their peaks, PR brackets under the baseline, RR spans along the top.
fn draw_marks(
    painter: &egui::Painter,
    win: &StripWindow,
    ring: &LeadRing,
    style: &StripStyle,
    trace: &StripTrace<'_>,
    marks: LeadMarks<'_>,
) {
    let sign = if trace.invert { -1.0 } else { 1.0 };
    let font = FontId::proportional(11.0);
    let rect = win.rect;
    if trace.show_waves {
        for m in marks.marks {
            if !matches!(m.kind, WaveKind::P | WaveKind::R | WaveKind::T) {
                continue;
            }
            let abs = marks.first_sample + m.peak as u64;
            let (Some(x), Some(v)) = (win.x_of_sample(abs), ring.at(abs)) else {
                continue;
            };
            let v = sign * v;
            let y = win.y_of_mv(v);
            let (pos, align) = if v >= 0.0 {
                (
                    Pos2::new(x, (y - 3.0).max(rect.top() + 12.0)),
                    Align2::CENTER_BOTTOM,
                )
            } else {
                (
                    Pos2::new(x, (y + 3.0).min(rect.bottom() - 12.0)),
                    Align2::CENTER_TOP,
                )
            };
            painter.text(pos, align, m.kind.label(), font.clone(), style.marker);
        }
    }
    if !trace.show_intervals {
        return;
    }
    let stroke = Stroke::new(1.0, style.marker);
    let bracket = |x0: f32, x1: f32, y: f32, tick: f32, text: String, above: bool| {
        painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], stroke);
        painter.line_segment([Pos2::new(x0, y - tick), Pos2::new(x0, y + tick)], stroke);
        painter.line_segment([Pos2::new(x1, y - tick), Pos2::new(x1, y + tick)], stroke);
        let (dy, align) = if above {
            (-2.0, Align2::CENTER_BOTTOM)
        } else {
            (2.0, Align2::CENTER_TOP)
        };
        painter.text(
            Pos2::new(f32::midpoint(x0, x1), y + dy),
            align,
            text,
            FontId::proportional(10.0),
            style.marker,
        );
    };
    let ms = |samples: usize| samples as f64 * 1000.0 / f64::from(marks.sample_rate_hz);
    let x_of = |idx: usize| win.x_of_sample(marks.first_sample + idx as u64);
    let pr_y = (rect.bottom() - 14.0).max(rect.center().y + 4.0);
    let rr_y = (rect.top() + 28.0).min(rect.center().y - 4.0);
    let mut prev_r: Option<usize> = None;
    for b in marks.beats {
        if let Some(p) = b.p
            && let (Some(x0), Some(x1)) = (x_of(p.onset), x_of(b.qrs_onset))
            && x1 > x0
        {
            let pr = ms(b.qrs_onset.saturating_sub(p.onset));
            bracket(x0, x1, pr_y, 3.0, format!("PR {pr:.0}"), false);
        }
        if let Some(r0) = prev_r
            && let (Some(x0), Some(x1)) = (x_of(r0), x_of(b.r_peak))
            && x1 > x0
        {
            let rr = ms(b.r_peak - r0);
            bracket(x0, x1, rr_y, 3.0, format!("RR {rr:.0}"), true);
        }
        prev_r = Some(b.r_peak);
    }
}

/// The paper grid as one cached mesh (one tessellation-free shape instead of hundreds of line
/// segments). Lines are snapped to physical pixels so 1-px lines stay crisp on HiDPI screens.
fn grid_shape(ui: &Ui, id: Id, rect: Rect, style: &StripStyle) -> Shape {
    type Cached = (([u32; 6], Color32, Color32), Arc<Mesh>);
    let ppp = ui.ctx().pixels_per_point();
    let key = (
        [
            rect.min.x,
            rect.min.y,
            rect.max.x,
            rect.max.y,
            style.px_per_mm,
            ppp,
        ]
        .map(f32::to_bits),
        style.grid_minor,
        style.grid_major,
    );
    let cache_id = id.with("grid_mesh");
    if let Some((k, mesh)) = ui.data(|d| d.get_temp::<Cached>(cache_id))
        && k == key
    {
        return Shape::Mesh(mesh);
    }
    let mesh = Arc::new(grid_mesh(rect, style, ppp));
    ui.data_mut(|d| d.insert_temp::<Cached>(cache_id, (key, mesh.clone())));
    Shape::Mesh(mesh)
}

fn grid_mesh(rect: Rect, style: &StripStyle, ppp: f32) -> Mesh {
    let mut mesh = Mesh::default();
    let mm = style.px_per_mm;
    let px = 1.0 / ppp;
    let snap = |v: f32| (v * ppp).round() / ppp;
    // Minor lines closer than 3 physical pixels turn the paper into a solid tint: drop them.
    let minor = mm * ppp >= 3.0;
    let mut add = |vertical: bool, at: f32, major: bool| {
        if !major && !minor {
            return;
        }
        let w = if major { 2.0 * px } else { px };
        let at = snap(at);
        let r = if vertical {
            Rect::from_min_max(Pos2::new(at, rect.top()), Pos2::new(at + w, rect.bottom()))
        } else {
            Rect::from_min_max(Pos2::new(rect.left(), at), Pos2::new(rect.right(), at + w))
        };
        mesh.add_colored_rect(r, if major { style.grid_major } else { style.grid_minor });
    };
    let mut i = 0;
    let mut x = rect.left();
    while x <= rect.right() {
        add(true, x, i % 5 == 0);
        x += mm;
        i += 1;
    }
    let mut j = 0;
    let mut dy = 0.0;
    while dy <= rect.height() * 0.5 {
        let major = j % 5 == 0;
        add(false, rect.center().y - dy, major);
        if j > 0 {
            add(false, rect.center().y + dy, major);
        }
        dy += mm;
        j += 1;
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(style: &StripStyle, head: u64) -> StripWindow {
        StripWindow::new(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(750.0, 100.0)),
            style,
            500.0,
            head,
        )
    }

    #[test]
    fn doubling_speed_halves_the_seconds_visible() {
        let s25 = StripStyle::default();
        let s50 = StripStyle {
            mm_per_s: 50.0,
            ..s25
        };
        let (a, b) = (win(&s25, 0).seconds_visible(), win(&s50, 0).seconds_visible());
        assert!((a - 2.0 * b).abs() < 1e-4, "{a} vs {b}");
        assert!(
            (a - 10.0).abs() < 1e-4,
            "750 pt at 25 mm/s × 3 pt/mm is 10 s, got {a}"
        );
    }

    #[test]
    fn gain_twenty_doubles_amplitude_in_pixels() {
        let s10 = StripStyle::default();
        let s20 = StripStyle {
            mm_per_mv: 20.0,
            ..s10
        };
        let (w10, w20) = (win(&s10, 0), win(&s20, 0));
        let h10 = w10.y_of_mv(0.0) - w10.y_of_mv(1.0);
        let h20 = w20.y_of_mv(0.0) - w20.y_of_mv(1.0);
        assert!((h20 - 2.0 * h10).abs() < 1e-4);
        assert!((h10 - 30.0).abs() < 1e-4);
    }

    #[test]
    fn scroll_mode_puts_the_newest_sample_at_the_right_edge() {
        let w = win(&StripStyle::default(), 10_000);
        let x = w.x_of_sample(9_999).unwrap();
        assert!((w.rect.right() - x - 0.5 * w.px_per_sample).abs() < 1e-3);
        assert_eq!(w.sample_at_x(x), Some(9_999));
        assert_eq!(w.x_of_sample(10_000), None);
        assert_eq!(w.x_of_sample(10_000 - w.visible_samples - 1), None);
    }

    #[test]
    fn sweep_mode_keeps_samples_in_place_and_erases_ahead_of_the_bar() {
        let style = StripStyle {
            mode: SweepMode::Sweep,
            ..StripStyle::default()
        };
        let n = win(&style, 0).visible_samples;
        let head = 3 * n + n / 2;
        let w = win(&style, head);
        // A sample keeps its x as the bar moves on.
        let s = 3 * n + 10;
        let x1 = w.x_of_sample(s).unwrap();
        let x2 = win(&style, head + 100).x_of_sample(s).unwrap();
        assert!((x1 - x2).abs() < 1e-3);
        // Just right of the bar is erased; further right is the previous sweep.
        assert!((w.head_x() - (w.rect.left() + (n / 2) as f32 * w.px_per_sample)).abs() < 1e-3);
        assert_eq!(w.sample_at_x(w.head_x() + 1.0), None);
        let prev = w.sample_at_x(w.rect.right() - 2.0).unwrap();
        assert!(prev < 3 * n && prev >= 2 * n);
        for (a, b, _) in w.segments() {
            assert!(a < b && b <= head);
        }
    }
}
