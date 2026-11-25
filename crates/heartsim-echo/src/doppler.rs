//! Annular tissue Doppler (brief B34, echo simulator phase 5): the velocity of one annulus point
//! of the model along the beam, and its S′, e′ and a′.
//!
//! **Tissue, not blood.** There is no blood Doppler here, spectral or colour, and no control that
//! suggests one: the only thing a gate can be placed on is an atrioventricular **annulus** — the
//! septal or lateral mitral annulus, or the lateral tricuspid annulus — whose motion the model
//! prescribes ([`crate::model::MotionModel::annulus_position`]: the rim point of the valve disc,
//! carried by the same fit to the ventricle that moves the disc in the 3-D heart). The velocity
//! is the time derivative of that motion along the beam, so it is exactly as honest as the motion
//! model: nothing is drawn from a template.
//!
//! **What a sample is.** Every [`DOPPLER_STEP_MS`] the annulus point's position (heart frame,
//! respiration included) is recorded; the velocity at step `k` is the central difference of the
//! positions at `k − 1` and `k + 1` projected on the beam, positive **toward the transducer**
//! (the beam points away from it): for an apical probe the annulus moves toward the apex in
//! systole, so S′ is positive and e′ and a′, when the annulus moves back, negative. The beam is
//! the line from the apex through the gate when it was placed, and **the velocity is not
//! corrected for the angle** between the beam and the motion; the angle is reported
//! ([`DopplerScan::angle_deg`]) so that its effect can be read: at 60° the peak is half.
//!
//! **What is designed.** The thin spectral band the page draws around the curve is for the look
//! only (a real spectrum has a width); it is drawn from the curve itself and labelled designed.
//!
//! The waves are marked from the model's own timing ([`crate::model::BeatTimes`]): S′ the peak
//! toward the probe from QRS onset to the closure of the outflow valve, e′ the peak away from it
//! after that, a′ the peak away from it after the atrial contraction — absent when there is none
//! (atrial fibrillation, flutter, or atria silent).
//!
//! Convention (Mitchell et al. 2019, p. 44): lateral and medial mitral and lateral tricuspid
//! annulus; the angle of interrogation as parallel as possible to the beam; a sample volume of 5
//! to 10 mm (the gate snaps to the annulus within [`GATE_SNAP_MM`]); s′ systole, e′ early
//! diastole, a′ atrial contraction; a sweep speed of 100 mm/s.

use crate::history::MotionHistory;
use crate::measure::Plane2;
use crate::mmode::StepBuffer;
use crate::model::{AnnulusSite, BeatTimes, MotionInput, MotionModel};
use crate::probe::ProbeFrame;
use glam::Vec3;

/// Time between two samples, ms. Designed: fine enough for the central difference to follow the
/// model's fastest annular motion within 0.2 cm/s, coarse enough to be 100 evaluations a second
/// of the ventricles' own motion (about a fifth of a full evaluation each).
pub const DOPPLER_STEP_MS: f64 = 10.0;

/// Samples kept (12 s).
pub const DOPPLER_CAPACITY: usize = 1200;

/// Samples the display may fall behind the clock before [`DopplerScan::advance`] skips ahead.
pub const DOPPLER_MAX_LAG_STEPS: i64 = 40;

/// How near a gate must be to an annulus point to snap to it, mm in the image plane ("within a few
/// millimetres"; the sample volume is 5 to 10 mm, Mitchell 2019 p. 44).
pub const GATE_SNAP_MM: f32 = 8.0;

/// How far from the image plane an annulus point may be to be gated, mm.
pub const GATE_PLANE_MM: f32 = 10.0;

/// A tissue-Doppler gate: the annulus it is on and the beam through it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gate {
    pub site: AnnulusSite,
    /// Unit vector from the transducer through the gate, heart frame.
    pub beam: Vec3,
    /// Where the gate sits in the sector plane when it was placed, mm.
    pub at: Plane2,
}

/// Snap a gate placed at the plane point `p` of the sector of `frame` to the nearest annulus
/// point of the model at `input`'s instant: within [`GATE_SNAP_MM`] in the plane and
/// [`GATE_PLANE_MM`] from it. `None` where there is no annulus — tissue Doppler is offered only
/// at the two annuli.
pub fn snap_gate(
    model: &mut MotionModel,
    input: &MotionInput<'_>,
    frame: &ProbeFrame,
    p: Plane2,
) -> Option<Gate> {
    let mut best: Option<(f32, Gate)> = None;
    for site in AnnulusSite::ALL {
        let Some(pos) = model.annulus_position(input, site) else {
            continue;
        };
        let (q, out_of_plane) = Plane2::of_world(frame, pos);
        let d = q.distance(p);
        if d > GATE_SNAP_MM || out_of_plane.abs() > GATE_PLANE_MM {
            continue;
        }
        let beam = (pos - frame.apex_mm).normalize_or_zero();
        if beam == Vec3::ZERO {
            continue;
        }
        if best.as_ref().is_none_or(|b| d < b.0) {
            best = Some((d, Gate { site, beam, at: q }));
        }
    }
    best.map(|b| b.1)
}

/// One recorded sample: where the annulus point was, heart frame, mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DopplerSample {
    pub step: i64,
    pub t_ms: f64,
    pub position_mm: Vec3,
}

/// Which wave of the annular velocity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WaveKind {
    /// Systolic (s′).
    S,
    /// Early diastolic (e′).
    E,
    /// Late diastolic, atrial (a′).
    A,
}

impl WaveKind {
    /// The wave's name with its prime, for the page (an ASCII apostrophe: the interface font has no
    /// prime sign, and the calculators write e' the same way).
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::S => "S'",
            Self::E => "e'",
            Self::A => "a'",
        }
    }
}

/// A marked peak of the annular velocity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    pub kind: WaveKind,
    pub t_ms: f64,
    /// The velocity there, cm/s, positive toward the transducer.
    pub cm_s: f32,
}

/// What a tissue-Doppler scan needs, other than the model.
#[derive(Debug, Clone, Copy)]
pub struct DopplerContext<'a> {
    pub history: &'a MotionHistory,
    pub enabled: bool,
    pub exaggeration: f32,
}

/// A tissue-Doppler trace being recorded.
#[derive(Debug, Clone)]
pub struct DopplerScan {
    pub buffer: StepBuffer<DopplerSample>,
    gate: Option<Gate>,
    /// Samples computed since creation (tests, the cost report).
    pub computed: u64,
}

impl Default for DopplerScan {
    fn default() -> Self {
        Self::new()
    }
}

impl DopplerScan {
    /// An empty scan without a gate.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: StepBuffer::new(DOPPLER_STEP_MS, DOPPLER_CAPACITY),
            gate: None,
            computed: 0,
        }
    }

    /// The gate.
    #[must_use]
    pub fn gate(&self) -> Option<Gate> {
        self.gate
    }

    /// Put the gate somewhere (the trace restarts) or remove it.
    pub fn set_gate(&mut self, gate: Option<Gate>) {
        if self.gate != gate {
            self.buffer.clear();
        }
        self.gate = gate;
    }

    /// Bring the trace up to `until_ms`, computing at most `max` samples; returns how many it
    /// computed. Nothing without a gate. A sample is a pure function of the record and its step.
    pub fn advance(
        &mut self,
        model: &mut MotionModel,
        ctx: &DopplerContext<'_>,
        until_ms: f64,
        max: usize,
    ) -> usize {
        let Some(gate) = self.gate else { return 0 };
        let steps = self.buffer.plan(until_ms, max, DOPPLER_MAX_LAG_STEPS);
        let mut done = 0;
        for k in steps {
            let t_ms = k as f64 * DOPPLER_STEP_MS;
            let input = MotionInput {
                history: ctx.history,
                t_ms,
                resp_phase: ctx.history.respiratory_phase(t_ms),
                enabled: ctx.enabled,
                exaggeration: ctx.exaggeration,
            };
            let Some(position_mm) = model.annulus_position(&input, gate.site) else {
                break;
            };
            self.buffer.push(
                k,
                DopplerSample {
                    step: k,
                    t_ms,
                    position_mm,
                },
            );
            self.computed += 1;
            done += 1;
        }
        done
    }

    /// Forget the trace, keep the gate.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// The velocity of the annulus along the beam at `step`, cm/s, positive toward the
    /// transducer: the central difference of the positions at the steps either side. `None`
    /// where either is missing.
    #[must_use]
    pub fn velocity_cm_s(&self, step: i64) -> Option<f32> {
        let g = self.gate?;
        let (a, b) = (self.buffer.get(step - 1)?, self.buffer.get(step + 1)?);
        let dt_ms = b.t_ms - a.t_ms;
        // mm per ms is m/s; ×100 is cm/s; toward the transducer is against the beam
        Some(-(b.position_mm - a.position_mm).dot(g.beam) / dt_ms as f32 * 100.0)
    }

    /// The depth of the annulus along the beam at `step`, mm from the apex.
    #[must_use]
    pub fn depth_mm(&self, step: i64, apex: Vec3) -> Option<f32> {
        let g = self.gate?;
        Some((self.buffer.get(step)?.position_mm - apex).dot(g.beam))
    }

    /// The angle between the beam and the annulus's motion at `step`, degrees (`None` where the
    /// annulus does not move or a neighbour is missing). The velocity is **not** corrected for
    /// it: read at the peak of S′ it tells how much of the motion the beam sees.
    #[must_use]
    pub fn angle_deg(&self, step: i64) -> Option<f32> {
        let g = self.gate?;
        let w = self.buffer.get(step + 1)?.position_mm - self.buffer.get(step - 1)?.position_mm;
        (w.length() > 1e-4).then(|| {
            (w.dot(g.beam).abs() / w.length())
                .clamp(0.0, 1.0)
                .acos()
                .to_degrees()
        })
    }

    /// The velocity as a series over `t0_ms..=t1_ms` (every step that has one).
    #[must_use]
    pub fn series(&self, t0_ms: f64, t1_ms: f64) -> Vec<(f64, f32)> {
        let (a, b) = (self.buffer.step_at(t0_ms), self.buffer.step_at(t1_ms));
        (a..=b)
            .filter_map(|k| self.velocity_cm_s(k).map(|v| (self.buffer.time_of(k), v)))
            .collect()
    }

    /// The marked waves of the beat `beat` describes, from the part of the trace recorded.
    #[must_use]
    pub fn markers(&self, beat: &BeatTimes) -> Vec<Marker> {
        let series = |t0: f64, t1: f64| self.series(t0, t1);
        let peak = |v: &[(f64, f32)], sign: f32| {
            v.iter()
                .copied()
                .max_by(|a, b| (a.1 * sign).total_cmp(&(b.1 * sign)))
                .filter(|p| p.1 * sign > MIN_PEAK_CM_S)
        };
        let mut out = Vec::new();
        let close = beat.ejection_ms.map_or(beat.inflow_open_ms, |(_, c)| c);
        if let Some(p) = peak(&series(beat.qrs_ms, close + 30.0), 1.0) {
            out.push(Marker {
                kind: WaveKind::S,
                t_ms: p.0,
                cm_s: p.1,
            });
        }
        let e_end = beat
            .atrial_onset_ms
            .or(beat.next_qrs_ms)
            .unwrap_or(f64::INFINITY)
            .min(beat.inflow_open_ms + E_WINDOW_MS);
        if let Some(p) = peak(&series(close, e_end), -1.0) {
            out.push(Marker {
                kind: WaveKind::E,
                t_ms: p.0,
                cm_s: p.1,
            });
        }
        if let Some(a0) = beat.atrial_onset_ms {
            let a1 = beat.next_qrs_ms.unwrap_or(a0 + A_WINDOW_MS).min(a0 + A_WINDOW_MS);
            if let Some(p) = peak(&series(a0, a1), -1.0) {
                out.push(Marker {
                    kind: WaveKind::A,
                    t_ms: p.0,
                    cm_s: p.1,
                });
            }
        }
        out
    }
}

/// A peak smaller than this, cm/s, is noise of the finite difference, not a wave.
const MIN_PEAK_CM_S: f32 = 0.5;
/// How long after the inflow valve opens e′ is looked for, ms.
const E_WINDOW_MS: f64 = 300.0;
/// How long after the atrial activation a′ is looked for at most, ms.
const A_WINDOW_MS: f64 = 300.0;
