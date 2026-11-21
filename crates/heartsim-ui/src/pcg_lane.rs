//! The heart-sound lane under the strip (brief B24): the phonocardiogram drawn as a filled envelope
//! on the strip's own time axis, with `S1` / `S2` labels (and `A2` `P2` when S2 is split by more than
//! 20 ms) placed from the model's schedule, not found by peak picking.
//!
//! The lane's window is the strip's window rescaled to the PCG rate ([`StripWindow::rescaled`]), so
//! scrolling, sweeping, pausing and scrubbing move both by the same sample-exact mapping, and a
//! caliper line (milliseconds from the left edge) is at the same x in both.

use crate::strip::{StripStyle, StripWindow, SweepMode, grid_shape, labels_that_fit};
use crate::t;
use egui::{Align2, Color32, FontId, Id, Mesh, Pos2, Rect, Stroke, Ui, Vec2};
use heartsim_ecg::{LeadRing, SoundLabel};

/// What the heart-sound audio is doing (B25), as the Display popup and the Settings window report
/// it. The app's player decides; the interface only words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeartSoundStatus {
    /// The "Play heart sounds" switch is off.
    #[default]
    Off,
    /// Live at 1×: the ring as drawn, a constant delay behind the engine.
    Playing,
    /// Below 1×: re-timed for wall time, each sound at its own pitch.
    SlowMotion,
    /// Above 1×: muted (the monitor beep still sounds).
    MutedFast,
    /// An imported recording is in the strip: its beats are not the engine's.
    MutedReview,
    /// The simulation is paused.
    Paused,
    /// A slice of the ring is playing while paused.
    PlayingSlice,
    /// The sync check is running.
    Calibrating,
    /// No audio device could be opened (or this build has none).
    NoDevice,
    /// The browser's audio waits for a click or a key press (autoplay policy, B26): a normal state.
    WaitingForClick,
    /// This browser has no usable Web Audio (B26).
    Unsupported,
}

impl HeartSoundStatus {
    /// A short note on why nothing (or something unusual) is heard, if any.
    #[must_use]
    pub fn note(self) -> Option<String> {
        match self {
            HeartSoundStatus::SlowMotion => Some(t!("ecg-sound-slow")),
            HeartSoundStatus::MutedFast => Some(t!("ecg-sound-muted-fast")),
            HeartSoundStatus::MutedReview => Some(t!("ecg-sound-muted-review")),
            HeartSoundStatus::Paused => Some(t!("ecg-sound-paused")),
            HeartSoundStatus::Calibrating => Some(t!("ecg-sound-calibrating")),
            HeartSoundStatus::NoDevice => Some(t!("ecg-sound-no-device")),
            HeartSoundStatus::WaitingForClick => Some(t!("ecg-sound-click-to-enable")),
            HeartSoundStatus::Unsupported => Some(t!("ecg-sound-unsupported")),
            HeartSoundStatus::Off | HeartSoundStatus::Playing | HeartSoundStatus::PlayingSlice => None,
        }
    }
}

/// What the learner asked the heart-sound lane's button for, in PCG ring indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneRequest {
    /// Play ring samples `start..end` once.
    Play {
        start: u64,
        end: u64,
    },
    Stop,
}

/// What the lane draws: the PCG ring and the labels as `(absolute sample index of the ring, label)`.
#[derive(Debug, Clone, Copy)]
pub struct PcgLane<'a> {
    pub ring: &'a LeadRing,
    pub marks: &'a [(u64, SoundLabel)],
}

impl PcgLane<'_> {
    /// How many PCG samples the lane has per sample of `ecg`, when the two rings run in step (the
    /// PCG rate an integer multiple of the ECG rate and as many samples pushed per ECG sample). `None`
    /// when they do not, e.g. while an imported recording is in the strip rings: the lane is then
    /// left out, not drawn blank.
    #[must_use]
    pub fn factor(&self, ecg: &LeadRing) -> Option<u64> {
        let ratio = self.ring.sample_rate_hz / ecg.sample_rate_hz.max(1.0);
        let factor = ratio.round();
        if factor < 1.0 || (ratio - factor).abs() > 1e-3 {
            return None;
        }
        let factor = factor as u64;
        (self.ring.pushed() == ecg.pushed() * factor && !self.ring.is_empty()).then_some(factor)
    }
}

/// Height of the lane in a strip area `total` points high: about a third, within 40–90 points.
#[must_use]
pub fn lane_height(total: f32) -> f32 {
    (total * 0.34).clamp(40.0, 90.0).min(total * 0.5)
}

/// Fraction of the lane's half-height that a PCG value of 1 (a normal S1) reaches.
const FULL_SCALE: f32 = 0.8;

/// Paint the lane into `rect` with the PCG window `win` (the strip's window rescaled to the PCG
/// rate, laid on `rect`). Returns nothing: interaction goes through the caller, which owns the
/// calipers.
pub fn draw_pcg_lane(ui: &Ui, id: Id, win: &StripWindow, lane: &PcgLane<'_>, style: &StripStyle) {
    let rect = win.rect;
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, style.paper);
    if style.show_grid {
        painter.add(grid_shape(ui, id, rect, style));
    }
    let mid = rect.center().y;
    let half = rect.height() * 0.5 - 12.0;
    let y_of = |v: f32| mid - (v * FULL_SCALE * half).clamp(-half - 6.0, half + 6.0);
    // baseline
    painter.line_segment(
        [Pos2::new(rect.left(), mid), Pos2::new(rect.right(), mid)],
        Stroke::new(0.6, style.trace.gamma_multiply(0.45)),
    );
    let mut mesh = Mesh::default();
    let mut buf = Vec::new();
    for (s, e, x0) in win.segments() {
        let first = lane.ring.range(s, e, &mut buf);
        if buf.is_empty() {
            continue;
        }
        let x_start = x0 + (first - s) as f32 * win.px_per_sample;
        let width = buf.len() as f32 * win.px_per_sample;
        let cols = (width.ceil() as usize).clamp(1, buf.len());
        let col_w = width / cols as f32;
        for c in 0..cols {
            let col = &buf[c * buf.len() / cols..(c + 1) * buf.len() / cols];
            let (lo, hi) = col
                .iter()
                .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
            if lo > hi || (hi - lo) * half < 0.3 {
                continue;
            }
            let x = x_start + c as f32 * col_w;
            let r = Rect::from_min_max(Pos2::new(x, y_of(hi)), Pos2::new(x + col_w.max(0.6), y_of(lo)));
            mesh.add_colored_rect(r, style.trace);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
    if win.mode == SweepMode::Sweep && win.head > 0 {
        let x = win.head_x();
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, style.trace.gamma_multiply(0.35)),
        );
    }
    let title = painter.text(
        rect.left_top() + Vec2::new(6.0, 2.0),
        Align2::LEFT_TOP,
        t!("ecg-heart-sounds-lane"),
        FontId::proportional(11.0),
        style.trace,
    );
    draw_labels(&painter, win, lane, style.marker, title.right());
}

/// `S1` and `S2` along the top of the lane (right of the title, which ends at `title_right`), `A2`
/// and `P2` along the bottom (joined into one label when too close to stand apart), each row thinned
/// with [`labels_that_fit`] so that labels never overlap.
fn draw_labels(
    painter: &egui::Painter,
    win: &StripWindow,
    lane: &PcgLane<'_>,
    colour: Color32,
    title_right: f32,
) {
    let font = FontId::proportional(10.0);
    let rect = win.rect;
    for (row, y, align, left) in [
        (
            &[SoundLabel::S1, SoundLabel::S2][..],
            rect.top() + 2.0,
            Align2::CENTER_TOP,
            title_right + 2.0,
        ),
        (
            &[SoundLabel::A2, SoundLabel::P2][..],
            rect.bottom() - 2.0,
            Align2::CENTER_BOTTOM,
            rect.left(),
        ),
    ] {
        let mut items: Vec<(f32, SoundLabel)> = lane
            .marks
            .iter()
            .filter(|(_, l)| row.contains(l))
            .filter_map(|(abs, l)| Some((win.x_of_sample(*abs)?, *l)))
            .filter(|(x, _)| x - 7.0 >= left)
            .collect();
        items.sort_by(|a, b| a.0.total_cmp(&b.0));
        let items = merge_close(&items, LABEL_WIDTH_PX + 2.0);
        let spans: Vec<(f32, f32)> = items.iter().map(|(x, _, width)| (*x, *width)).collect();
        for ((x, text, _), keep) in items.into_iter().zip(labels_that_fit(&spans, 2.0)) {
            if keep {
                painter.text(Pos2::new(x, y), align, text, font.clone(), colour);
            }
        }
    }
}

/// Width allowed for one two-character sound label at the lane's font size.
const LABEL_WIDTH_PX: f32 = 14.0;

/// Join two different labels that sit closer than `min_gap_px` into one, named in the order the
/// sounds occur (`A2·P2` for a physiological split, `P2·A2` for a paradoxical one) and centred
/// between them, so a narrow split never shows one component without the other. `items` is sorted
/// by x; the result is `(centre x, text, width)`.
fn merge_close(items: &[(f32, SoundLabel)], min_gap_px: f32) -> Vec<(f32, String, f32)> {
    let mut out = Vec::with_capacity(items.len());
    let mut i = 0;
    while i < items.len() {
        let (x, label) = items[i];
        match items.get(i + 1) {
            Some(&(next_x, next)) if next != label && next_x - x < min_gap_px => {
                let text = format!("{}\u{b7}{}", label.text(), next.text());
                out.push((x.midpoint(next_x), text, 2.0 * LABEL_WIDTH_PX + 4.0));
                i += 2;
            }
            _ => {
                out.push((x, label.text().to_owned(), LABEL_WIDTH_PX));
                i += 1;
            }
        }
    }
    out
}

/// The playhead of a slice being played: a vertical line at PCG ring index `pcg_index` in `win`
/// (a lane window, or a strip window when `factor` PCG samples make one strip sample).
pub fn draw_playhead(ui: &Ui, win: &StripWindow, pcg_index: u64, factor: u64, style: &StripStyle) {
    let Some(x) = win.x_of_sample(pcg_index / factor.max(1)) else {
        return;
    };
    let r = win.rect;
    ui.painter_at(r).line_segment(
        [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
        Stroke::new(2.0, style.marker),
    );
}

/// Caliper lines and bands of `calipers` carried down into the lane, without a second read-out
/// (the strip above shows it).
pub fn draw_caliper_lines(ui: &Ui, win: &StripWindow, calipers: &crate::CaliperSet, style: &StripStyle) {
    let painter = ui.painter_at(win.rect);
    let r = win.rect;
    for c in &calipers.pairs {
        let (xa, xb) = (win.x_from_left_ms(c.a_ms), win.x_from_left_ms(c.b_ms));
        painter.rect_filled(
            Rect::from_x_y_ranges(xa.min(xb)..=xa.max(xb), r.y_range()),
            0.0,
            style.caliper.gamma_multiply(0.08),
        );
        for x in [xa, xb] {
            painter.line_segment(
                [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
                Stroke::new(1.2, style.caliper),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_narrow_split_names_both_components_in_their_order() {
        use SoundLabel::{A2, P2};
        // physiological split, 8 px apart: one joined label between the two
        let joined = merge_close(&[(100.0, A2), (108.0, P2)], 16.0);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].1, "A2\u{b7}P2");
        assert!((joined[0].0 - 104.0).abs() < 1e-3);
        // paradoxical split keeps the order of occurrence
        assert_eq!(merge_close(&[(100.0, P2), (108.0, A2)], 16.0)[0].1, "P2\u{b7}A2");
        // far enough apart: two labels; the same label twice is never joined
        assert_eq!(merge_close(&[(100.0, A2), (130.0, P2)], 16.0).len(), 2);
        assert_eq!(merge_close(&[(100.0, A2), (108.0, A2)], 16.0).len(), 2);
        // a pair is joined once, the next beat's pair separately
        let two = merge_close(&[(100.0, A2), (108.0, P2), (300.0, A2), (309.0, P2)], 16.0);
        assert_eq!(two.len(), 2);
    }

    #[test]
    fn the_lane_runs_only_in_step_with_the_strip() {
        let mut ecg = LeadRing::new(100, 500.0);
        let mut pcg = LeadRing::new(800, 4000.0);
        let lane = |pcg: &LeadRing, ecg: &LeadRing| {
            PcgLane {
                ring: pcg,
                marks: &[],
            }
            .factor(ecg)
        };
        assert_eq!(lane(&pcg, &ecg), None, "empty");
        for _ in 0..10 {
            ecg.push(0.0);
            for _ in 0..8 {
                pcg.push(0.0);
            }
        }
        assert_eq!(lane(&pcg, &ecg), Some(8));
        pcg.push(0.0);
        assert_eq!(lane(&pcg, &ecg), None, "out of step");
        let other = LeadRing::new(100, 250.0);
        assert_eq!(
            PcgLane {
                ring: &other,
                marks: &[]
            }
            .factor(&ecg),
            None
        );
        assert!((lane_height(200.0) - 68.0).abs() < 1e-3);
    }
}
