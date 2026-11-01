//! ECG strip painter: paper grid + min/max-decimated trace, drawn with `egui::Painter` (no
//! egui_plot — see docs/research/rust-stack.md §3 for why).

use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};
use heartsim_ecg::{LeadRing, ring::minmax_decimate};

#[derive(Debug, Clone, Copy)]
pub struct StripStyle {
    /// Paper speed, mm/s (25 standard).
    pub mm_per_s: f32,
    /// Gain, mm/mV (10 standard).
    pub mm_per_mv: f32,
    /// Screen pixels per paper millimetre (calibrate per monitor; 3.78 ≈ 96 dpi).
    pub px_per_mm: f32,
    pub trace: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub paper: Color32,
    pub trace_width: f32,
}

impl Default for StripStyle {
    fn default() -> Self {
        Self {
            mm_per_s: 25.0,
            mm_per_mv: 10.0,
            px_per_mm: 3.0,
            trace: Color32::from_rgb(20, 20, 20),
            grid_minor: Color32::from_rgb(250, 210, 210),
            grid_major: Color32::from_rgb(240, 150, 150),
            paper: Color32::from_rgb(255, 247, 247),
            trace_width: 1.4,
        }
    }
}

/// Draw one lead into `rect`. The newest sample is at the right edge ("scrolling" mode).
/// Returns the number of seconds visible.
pub fn ecg_strip(ui: &mut Ui, rect: Rect, ring: &LeadRing, style: &StripStyle, label: &str) -> f32 {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, style.paper);
    let mm = style.px_per_mm;
    // grid: minor every 1 mm, major every 5 mm, aligned to the right edge so it scrolls cleanly
    let mut i = 0;
    let mut x = rect.right();
    while x >= rect.left() {
        let major = i % 5 == 0;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(
                if major { 1.0 } else { 0.5 },
                if major { style.grid_major } else { style.grid_minor },
            ),
        );
        x -= mm;
        i += 1;
    }
    let mut j = 0;
    let mut y = rect.center().y;
    while y >= rect.top() {
        let major = j % 5 == 0;
        let s = Stroke::new(
            if major { 1.0 } else { 0.5 },
            if major { style.grid_major } else { style.grid_minor },
        );
        painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], s);
        let y2 = rect.center().y + (rect.center().y - y);
        if y2 <= rect.bottom() {
            painter.line_segment([Pos2::new(rect.left(), y2), Pos2::new(rect.right(), y2)], s);
        }
        y -= mm;
        j += 1;
    }

    let seconds_visible = rect.width() / (style.mm_per_s * mm);
    let n_samples = (seconds_visible * ring.sample_rate_hz) as usize;
    let columns = rect.width().max(1.0) as usize;
    let mut samples = Vec::with_capacity(n_samples);
    let got = ring.latest(n_samples, &mut samples);
    if got > 1 {
        let px_per_sample = rect.width() / n_samples as f32;
        let used_width = got as f32 * px_per_sample;
        let x0 = rect.right() - used_width;
        let cols = ((used_width as usize).max(1)).min(columns);
        let dec = minmax_decimate(&samples, cols);
        let y_of = |v: f32| rect.center().y - v * style.mm_per_mv * mm;
        let mut pts: Vec<Pos2> = Vec::with_capacity(cols * 2);
        let col_w = used_width / cols as f32;
        for (c, m) in dec.iter().enumerate() {
            if let Some(m) = m {
                let x = x0 + (c as f32 + 0.5) * col_w;
                // alternate max/min so the polyline sweeps the full column
                if c % 2 == 0 {
                    pts.push(Pos2::new(x, y_of(m.max)));
                    pts.push(Pos2::new(x, y_of(m.min)));
                } else {
                    pts.push(Pos2::new(x, y_of(m.min)));
                    pts.push(Pos2::new(x, y_of(m.max)));
                }
            }
        }
        painter.add(egui::Shape::line(
            pts,
            Stroke::new(style.trace_width, style.trace),
        ));
    }
    // 1 mV calibration pulse and label
    let cal_h = style.mm_per_mv * mm;
    let cx = rect.left() + 4.0;
    let cy = rect.center().y;
    painter.line_segment(
        [Pos2::new(cx, cy), Pos2::new(cx, cy - cal_h)],
        Stroke::new(1.0, style.trace),
    );
    painter.line_segment(
        [Pos2::new(cx, cy - cal_h), Pos2::new(cx + 2.0 * mm, cy - cal_h)],
        Stroke::new(1.0, style.trace),
    );
    painter.line_segment(
        [Pos2::new(cx + 2.0 * mm, cy - cal_h), Pos2::new(cx + 2.0 * mm, cy)],
        Stroke::new(1.0, style.trace),
    );
    painter.text(
        rect.left_top() + Vec2::new(6.0, 2.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(13.0),
        style.trace,
    );
    seconds_visible
}
