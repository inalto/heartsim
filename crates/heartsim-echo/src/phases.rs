//! Time courses: one cell's contraction, and each ventricle's volume gated by the valve schedule.
//!
//! **Contraction.** `c(t) = shape(t − t_act − EMD)` over `k · APD` (§2.3): a smoothstep rise over
//! [`MotionParams::rise_ms`], a plateau, and a smoothstep decay over the last
//! [`MotionParams::decay_fraction`] of the contraction.
//!
//! **Volume.** Each ventricular cycle runs from one M1 (T1 on the right) to the next:
//!
//! | phase | from | to | volume |
//! |---|---|---|---|
//! | isovolumic contraction | M1 | M1 + IVCT (valve opens) | held at EDV |
//! | ejection | opening | A2 (P2) | falls to `EDV · (1 − EF)` |
//! | isovolumic relaxation | A2 | A2 + IVRT | held at ESV |
//! | filling | AV valve opens | next M1 | E wave, diastasis, then the atrial push |
//!
//! Volumes are fractions of the rest mesh's cavity (the end-diastolic CT the pack is built from).
//! A beat that does not eject (no A2: a ventricular beat inside `min_ejecting_rr_ms`) holds its
//! volume and refills from where it is once its AV valve would reopen. Fibrillation wavelets are
//! not beats here: the cavity keeps the volume it had. The end-diastolic volume of a cycle is
//! where the previous filling got to, so a premature beat starts from a smaller ventricle; the
//! recursion starts four beats back at the rest volume, which makes the value at `t` a pure
//! function of the record.

use crate::params::MotionParams;
use heartsim_core::math::Deterministic;
use heartsim_ecg::pcg::{Beat, Components, PcgParams, qs2_ms};

/// Smoothstep on `[0, 1]`, clamped.
#[must_use]
pub fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Contraction state of a cell `x_ms` after its activation (`x` already net of nothing: the
/// electromechanical delay `emd_ms` is subtracted here) with action-potential duration `apd_ms`,
/// `0..=1`.
#[must_use]
pub fn contraction(x_ms: f64, emd_ms: f64, apd_ms: f64, p: &MotionParams) -> f64 {
    let x = x_ms - emd_ms;
    let dur = p.apd_fraction * apd_ms;
    if x <= 0.0 || x >= dur || dur <= 0.0 {
        return 0.0;
    }
    let decay_start = dur * (1.0 - p.decay_fraction);
    let rise = smoothstep(x / p.rise_ms.min(decay_start).max(1.0));
    let decay = if x <= decay_start {
        1.0
    } else {
        1.0 - smoothstep((x - decay_start) / (dur - decay_start).max(1.0))
    };
    rise.min(decay)
}

/// Ejected fraction of the stroke volume `τ` (`0..=1`) into ejection. The outflow is
/// `(1 + 3τ)(1 − τ) + k`, normalised, `k` = [`MotionParams::ejection_end_flow`]: it starts at
/// once, peaks a third of the way through and is still `k` at the closure, so the volume stops
/// falling at A2 rather than creeping to it. **Designed** shape.
#[must_use]
pub fn ejection_shape(tau: f64, k: f64) -> f64 {
    let t = tau.clamp(0.0, 1.0);
    (t + t * t - t * t * t + k * t) / (1.0 + k)
}

/// Fraction of the passive filling reached `tau_ms` after the AV valve opened (E wave as a
/// triangular inflow, plus the slow diastasis component).
#[must_use]
pub fn passive_filling(tau_ms: f64, p: &MotionParams) -> f64 {
    if tau_ms <= 0.0 {
        return 0.0;
    }
    let (ta, td) = (p.e_acceleration_ms.max(1.0), p.e_deceleration_ms.max(1.0));
    let peak = 2.0 / (ta + td);
    let tri = if tau_ms <= ta {
        peak * tau_ms * tau_ms / (2.0 * ta)
    } else if tau_ms <= ta + td {
        let u = tau_ms - ta;
        peak * ta / 2.0 + peak * (u - u * u / (2.0 * td))
    } else {
        1.0
    };
    let slow = 1.0 - (-tau_ms / p.diastasis_tau_ms.max(1.0)).exp_d();
    (1.0 - p.diastasis_share) * tri + p.diastasis_share * slow
}

/// Which part of its cycle a ventricle is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardiacPhase {
    /// Before the first beat, or with no organised beat at all.
    Rest,
    IsovolumicContraction,
    Ejection,
    IsovolumicRelaxation,
    Filling,
}

/// The valve times of one beat for one ventricle, ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CycleTimes {
    /// QRS onset (the engine's ventricular event).
    pub qrs_ms: f64,
    /// Closure of the inflow valve (M1 / T1): the cycle starts.
    pub closure_ms: f64,
    /// Opening of the outflow valve and its closure (A2 / P2); `None` for a beat that does not
    /// eject.
    pub ejection_ms: Option<(f64, f64)>,
    /// Opening of the inflow valve: filling starts.
    pub inflow_open_ms: f64,
    /// Ejection fraction of the beat (0 when it does not eject).
    pub ef: f64,
}

/// Which ventricle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

/// The valve times of `beat` (with its `components`) for `side`; `rr_prev_ms` is the preceding
/// beat's RR (post-extrasystolic potentiation) and `ef_scale` the injury factor of the side.
#[must_use]
pub fn cycle_times(
    beat: &Beat,
    components: &Components,
    rr_prev_ms: Option<f64>,
    side: Side,
    p: &MotionParams,
    pcg: &PcgParams,
    ef_scale: f64,
) -> CycleTimes {
    let q = beat.qrs_onset.0;
    let (lv, rv) = beat.ventricle_delays_ms();
    let (closure, out, ivct, ivrt, ef0, delay) = match side {
        Side::Left => (
            components.m1.map_or(q + pcg.q_m1_ms, |c| c.onset.0),
            components.a2.map(|c| c.onset.0),
            p.lv_ivct_ms,
            p.lv_ivrt_ms,
            p.lvef,
            lv,
        ),
        Side::Right => (
            components.t1.map_or(q + pcg.q_m1_ms, |c| c.onset.0),
            components.p2.map(|c| c.onset.0),
            p.rv_ivct_ms,
            p.rv_ivrt_ms,
            p.rvef,
            rv,
        ),
    };
    let ef = (ef0 * p.ef_factor(beat.rr_ms, rr_prev_ms) * ef_scale).clamp(0.0, p.ef_max);
    // a late left ventricle (LBBB, RV pacing) moves M1 by only part of its delay; the aortic valve
    // opens IVCT after the late wall has started, so isovolumic contraction lasts the rest of the
    // delay longer (designed: the pre-ejection period lengthens in LBBB)
    let late = match side {
        Side::Left => (1.0 - pcg.m1_lv_delay_fraction) * lv,
        Side::Right => 0.0,
    };
    if let Some(close) = out {
        let open = (closure + ivct + late).min(close);
        CycleTimes {
            qrs_ms: q,
            closure_ms: closure,
            ejection_ms: Some((open, close)),
            inflow_open_ms: close + ivrt,
            ef,
        }
    } else {
        // no ejection: the inflow valve reopens when relaxation would have ended
        let hr = beat
            .rr_ms
            .filter(|rr| *rr > 0.0)
            .map_or(pcg.first_beat_bpm, |rr| 60_000.0 / rr);
        let virtual_close = q - pcg.qrs_onset_lead_ms + qs2_ms(hr, pcg) + delay;
        CycleTimes {
            qrs_ms: q,
            closure_ms: closure,
            ejection_ms: None,
            inflow_open_ms: virtual_close.max(closure) + ivrt,
            ef: 0.0,
        }
    }
}

/// A ventricle's state at a time: volume fraction, phase, and the current cycle's numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeState {
    /// Cavity volume as a fraction of the rest cavity.
    pub volume: f64,
    pub phase: CardiacPhase,
    /// End-diastolic and end-systolic volume of the current cycle (fractions of rest).
    pub edv: f64,
    pub esv: f64,
    /// The current cycle's times (`None` at rest).
    pub cycle: Option<CycleTimes>,
}

/// How many beats back the volume recursion starts from the rest volume.
pub const LOOKBACK_BEATS: usize = 4;

/// Inputs of the volume computation that do not change within a frame.
#[derive(Debug, Clone, Copy)]
pub struct VolumeInputs<'a> {
    pub beats: &'a [(Beat, Components)],
    /// Organised atrial activations, ms, ascending.
    pub atrial_onsets_ms: &'a [f64],
    pub params: &'a MotionParams,
    pub pcg: &'a PcgParams,
    /// Ejection-fraction factor of each side for the current injury (1 healthy).
    pub ef_scale: [f64; 2],
}

/// Fraction of the atrial push delivered between the inflow valve's opening `open_ms` and `t_ms`:
/// the pushes that start before the opening lose their early part (an atrial contraction against
/// a closed valve fills nothing).
fn atrial_push(open_ms: f64, t_ms: f64, onsets: &[f64], p: &MotionParams) -> f64 {
    let dur = p.atrial_contraction_ms.max(1.0);
    let first = onsets.partition_point(|&a| a + p.emd_atrium_ms + dur <= open_ms);
    let mut total = 0.0;
    for &a in &onsets[first..] {
        let start = a + p.emd_atrium_ms;
        if start > t_ms {
            break;
        }
        let s = |x: f64| smoothstep((x - start) / dur);
        total += (s(t_ms) - s(open_ms.max(start))).max(0.0);
    }
    total.min(1.0)
}

/// Volume during the filling of a cycle that ended systole at `esv` with the inflow valve open at
/// `open_ms`.
fn filling(esv: f64, open_ms: f64, t_ms: f64, inp: &VolumeInputs<'_>) -> f64 {
    let p = inp.params;
    let passive = (1.0 - p.atrial_share) * passive_filling(t_ms - open_ms, p);
    let push = atrial_push(open_ms, t_ms, inp.atrial_onsets_ms, p);
    let g = passive + (1.0 - passive) * push;
    esv + (1.0 - esv) * g
}

/// Volume of a cycle at `t_ms` (inside it) from its end-diastolic volume `edv`.
fn in_cycle(c: &CycleTimes, edv: f64, t_ms: f64, inp: &VolumeInputs<'_>) -> (f64, CardiacPhase, f64) {
    let esv = edv * (1.0 - c.ef);
    match c.ejection_ms {
        Some((open, close)) => {
            if t_ms < open {
                (edv, CardiacPhase::IsovolumicContraction, esv)
            } else if t_ms < close {
                let tau = (t_ms - open) / (close - open).max(1e-9);
                (
                    edv - (edv - esv) * ejection_shape(tau, inp.params.ejection_end_flow),
                    CardiacPhase::Ejection,
                    esv,
                )
            } else if t_ms < c.inflow_open_ms {
                (esv, CardiacPhase::IsovolumicRelaxation, esv)
            } else {
                (
                    filling(esv, c.inflow_open_ms, t_ms, inp),
                    CardiacPhase::Filling,
                    esv,
                )
            }
        }
        None => {
            if t_ms < c.inflow_open_ms {
                (edv, CardiacPhase::IsovolumicContraction, edv)
            } else {
                (
                    filling(edv, c.inflow_open_ms, t_ms, inp),
                    CardiacPhase::Filling,
                    edv,
                )
            }
        }
    }
}

/// The volume state of `side` at `t_ms` (see the module docs). Beats after `t_ms` are ignored.
#[must_use]
pub fn volume_at(t_ms: f64, side: Side, inp: &VolumeInputs<'_>) -> VolumeState {
    let ef_scale = inp.ef_scale[match side {
        Side::Left => 0,
        Side::Right => 1,
    }];
    // organised beats up to t, with the RR of the beat before each (in the full list)
    let upto = inp.beats.partition_point(|(b, _)| b.qrs_onset.0 <= t_ms);
    let mut buf = [None::<CycleTimes>; LOOKBACK_BEATS + 1];
    let mut n = 0;
    let mut j = upto;
    while j > 0 && n < buf.len() {
        j -= 1;
        let (b, c) = &inp.beats[j];
        if !b.organised {
            continue;
        }
        let rr_prev = j.checked_sub(1).and_then(|k| inp.beats[k].0.rr_ms);
        buf[n] = Some(cycle_times(b, c, rr_prev, side, inp.params, inp.pcg, ef_scale));
        n += 1;
    }
    buf[..n].reverse();
    // a beat that closes the inflow valve while the one before is still ejecting (an early PVC)
    // does not cut that ejection short: its cycle starts at the previous closure of the outflow
    // valve, from the previous end-systolic volume, and ejects only after it
    for k in 1..n {
        let (Some(prev), Some(cur)) = (buf[k - 1], buf[k].as_mut()) else {
            continue;
        };
        let Some((_, prev_close)) = prev.ejection_ms else {
            continue;
        };
        if cur.closure_ms < prev_close {
            cur.closure_ms = prev_close;
            cur.ejection_ms = cur
                .ejection_ms
                .map(|(open, close)| (open.max(prev_close), close))
                .filter(|(open, close)| close > open);
            if cur.ejection_ms.is_none() {
                cur.ef = 0.0;
            }
            cur.inflow_open_ms = cur.inflow_open_ms.max(prev_close);
        }
    }
    let cycles = &buf[..n];
    let get = |k: usize| {
        cycles[k].unwrap_or(CycleTimes {
            qrs_ms: 0.0,
            closure_ms: 0.0,
            ejection_ms: None,
            inflow_open_ms: 0.0,
            ef: 0.0,
        })
    };
    // the current cycle: the last whose closure is at or before t
    let Some(cur) = (0..n).rev().find(|&k| get(k).closure_ms <= t_ms) else {
        return VolumeState {
            volume: 1.0,
            phase: CardiacPhase::Rest,
            edv: 1.0,
            esv: 1.0,
            cycle: None,
        };
    };
    let mut edv = 1.0;
    for k in 0..cur {
        edv = in_cycle(&get(k), edv, get(k + 1).closure_ms, inp).0;
    }
    let (volume, phase, esv) = in_cycle(&get(cur), edv, t_ms, inp);
    VolumeState {
        volume,
        phase,
        edv,
        esv,
        cycle: Some(get(cur)),
    }
}
