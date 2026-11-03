//! History of the 3-D heart for scrubbing (brief B9 §4): while the strip is paused and scrubbed,
//! the heart shows the colour map at the strip's cursor time, so the two never disagree.
//!
//! Every [`HeartHistory::PERIOD_MS`] of simulation time the per-cell values of the current colour
//! mode are recorded, quantised to one byte over the mode's legend range (0.6 mV for voltage,
//! 2 ms for activation time): 17.5 k cells × 25 Hz × 60 s ≈ 26 MB. Voltage is interpolated linearly
//! between the two bracketing snapshots, so the wavefront moves smoothly under a slow scrub;
//! activation time and APD are step-wise (they change per beat, and interpolating across a beat
//! boundary would invent colours).

use crate::heart_view::{ColourMode, HeartView};
use crate::snapshot_ring::SnapshotRing;
use heartsim_core::Millis;

/// One recorded frame: per-cell values of `mode`, quantised over `mode.range()`.
#[derive(Debug, Clone)]
pub struct HeartFrame {
    pub mode: ColourMode,
    pub cells: Vec<u8>,
}

/// One-byte code of `v` in `(lo, hi)` (clamped).
#[must_use]
pub fn quantise(v: f32, (lo, hi): (f32, f32)) -> u8 {
    let x = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    (x * 255.0).round() as u8
}

/// Value of the code `q` in `(lo, hi)`; inverse of [`quantise`] within `(hi − lo) / 510`.
#[must_use]
pub fn dequantise(q: u8, (lo, hi): (f32, f32)) -> f32 {
    lo + (hi - lo) * f32::from(q) / 255.0
}

/// Recorded heart frames plus the decode buffer.
#[derive(Debug, Clone)]
pub struct HeartHistory {
    ring: SnapshotRing<HeartFrame>,
    decoded: Vec<f32>,
}

impl Default for HeartHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl HeartHistory {
    /// Recording period, ms of simulation time (25 Hz: two to three frames per QRS).
    pub const PERIOD_MS: f64 = 40.0;
    /// Span kept, ms (the strip history length).
    pub const SPAN_MS: f64 = 60_000.0;

    #[must_use]
    pub fn new() -> Self {
        Self {
            ring: SnapshotRing::new(Millis(Self::PERIOD_MS), Millis(Self::SPAN_MS)),
            decoded: Vec::new(),
        }
    }

    /// Record `heart`'s live state at `now` if a period has passed (cheap otherwise). Time
    /// running backwards (scenario loaded) clears the history.
    pub fn record(&mut self, now: Millis, heart: &mut HeartView) {
        let mode = heart.mode;
        self.ring.record(now, || {
            let range = mode.range();
            HeartFrame {
                mode,
                cells: heart.cell_values().iter().map(|&v| quantise(v, range)).collect(),
            }
        });
    }

    /// Per-cell values of `mode` at `t`, if the history covers it in that mode.
    pub fn at(&mut self, t: Millis, mode: ColourMode) -> Option<&[f32]> {
        let ((ta, a), (tb, b)) = self.ring.around(t)?;
        if a.mode != mode {
            return None;
        }
        let range = mode.range();
        self.decoded.clear();
        if mode == ColourMode::Voltage && b.mode == mode && tb.0 > ta.0 && b.cells.len() == a.cells.len() {
            let w = (((t.0 - ta.0) / (tb.0 - ta.0)).clamp(0.0, 1.0)) as f32;
            self.decoded
                .extend(a.cells.iter().zip(&b.cells).map(|(&qa, &qb)| {
                    let (va, vb) = (dequantise(qa, range), dequantise(qb, range));
                    va + (vb - va) * w
                }));
        } else {
            self.decoded.extend(a.cells.iter().map(|&q| dequantise(q, range)));
        }
        Some(&self.decoded)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantisation_round_trips_within_half_a_step() {
        for mode in ColourMode::ALL {
            let (lo, hi) = mode.range();
            let tol = (hi - lo) / 510.0 + 1e-4;
            for i in 0..=100 {
                let v = lo + (hi - lo) * i as f32 / 100.0;
                let back = dequantise(quantise(v, (lo, hi)), (lo, hi));
                assert!((back - v).abs() <= tol, "{mode:?}: {v} → {back}");
            }
            assert_eq!(quantise(lo - 50.0, (lo, hi)), 0, "clamped low");
            assert_eq!(quantise(hi + 50.0, (lo, hi)), 255, "clamped high");
        }
    }

    /// Scrubbing back to a recorded instant shows what the heart showed live at that instant
    /// (B9: "strip and heart agree during scrub"), and the wave is really moving then.
    #[test]
    fn scrubbed_heart_equals_the_live_heart_at_that_time() {
        use heartsim_geometry::AnatomyPack;
        use heartsim_sim::{Engine, TimingEngine, TimingParams};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
        let pack = AnatomyPack::load(&dir).expect("default pack");
        let timing = TimingParams::default();
        let mut engine = TimingEngine::new(timing.clone());
        let mut heart = HeartView::from_pack(pack, &timing);
        let mut history = HeartHistory::new();
        let mut live: Vec<(f64, Vec<f32>)> = Vec::new();
        let dt = Millis(1.0);
        for _ in 0..2000 {
            engine.step(dt);
            let events = engine.drain_events();
            heart.on_step(&events, dt, engine.now());
            let before = history.len();
            history.record(engine.now(), &mut heart);
            if history.len() > before {
                live.push((engine.now().0, heart.cell_values().to_vec()));
            }
        }
        let range = ColourMode::Voltage.range();
        let tol = (range.1 - range.0) / 510.0 + 1e-3;
        let mut depolarised_frames = 0;
        for (t, cells) in &live {
            let got = history.at(Millis(*t), ColourMode::Voltage).expect("covered");
            let err = got
                .iter()
                .zip(cells)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f32::max);
            assert!(err <= tol, "t = {t} ms: max error {err} mV");
            if cells.iter().any(|&v| v > 0.0) {
                depolarised_frames += 1;
            }
        }
        assert!(
            depolarised_frames > 3,
            "a beat was recorded ({depolarised_frames} frames)"
        );
        assert!(
            history.at(Millis(500.0), ColourMode::Apd).is_none(),
            "recorded in voltage mode"
        );
    }
}
