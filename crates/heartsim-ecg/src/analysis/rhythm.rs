//! Naming the rhythms that have no stable P–QRS–T to measure, from the 12 leads alone.
//!
//! The engine knows what it is running, but the analyser does not ask: it reads the waveform as a
//! clinician would, so a strip and its name can only disagree if the strip is wrong.
//!
//! **Ventricular** ([`ventricular_chaos`]). Deflections are found on the spatial velocity, which
//! does not vanish when the QRS vector turns away from one lead. Their rate and how far their
//! 12-lead vector turns from each deflection to the next separate the rhythms:
//!
//! * a stable vector is an ordinary rhythm, however fast: not this module's business;
//! * a mean cycle under [`VF_MAX_CYCLE_MS`] with a vector that keeps jumping is VF;
//! * a vector that turns a little each beat, so that a lead flips polarity in runs of several
//!   beats, is torsades de pointes (the twist);
//! * a vector that jumps at a slower rate is polymorphic VT.
//!
//! **Atrial** ([`atrial_activity`]). What lies between the QRS complexes. V1 is read for flutter,
//! because its T wave is small and flutter waves stand out there even at 2:1, when the other wave
//! hides in the ST–T of II: flutter waves keep a fixed spacing, f waves do not, and a sinus P wave
//! in V1 is too small to count. Fibrillation is an irregular rhythm with a restless baseline after
//! the T wave. Fast, broad and irregular is named by [`pre_excited`] once the QRS is measured.
//!
//! The thresholds separate the rhythms this simulator draws. They are not validated against
//! recorded ECGs, and the analyser must not be read as a clinical classifier.

use super::dsp;
use super::{Beat, Regularity};
use heartsim_core::{LEAD_COUNT, Lead, RhythmKind};

/// A mean deflection cycle under this is fibrillation rather than polymorphic VT (over 250/min).
pub const VF_MAX_CYCLE_MS: f64 = 240.0;
/// Below this rate a rhythm is not a polymorphic tachycardia, bpm.
const CHAOS_MIN_RATE_BPM: f64 = 150.0;
/// Median turn between consecutive 12-lead vectors under which the rhythm is monomorphic, degrees.
const STABLE_TURN_DEG: f64 = 12.0;
/// Median turn above which the vector jumps rather than twists, degrees.
const JUMPING_TURN_DEG: f64 = 40.0;
/// Polarity runs of a lead in a twist, beats (the reported 5–20, with a margin for measurement).
const TWIST_RUN_BEATS: (f64, f64) = (4.0, 25.0);
/// Deflections closer than this belong to one complex, ms.
const MIN_DEFLECTION_GAP_MS: f64 = 130.0;
/// Deflections fewer than this say nothing, count.
pub(super) const MIN_DEFLECTIONS: usize = 8;
/// Dominant frequency search band, Hz.
const DOMINANT_BAND_HZ: (f64, f64) = (1.5, 12.0);

/// Baseline extrema smaller than this are ignored when counting f waves, mV.
const MIN_PROMINENCE_MV: f32 = 0.03;
/// A flutter wave in V1 stands at least this far above the troughs beside it, mV (a sinus P wave
/// in V1 does not).
const FLUTTER_PROMINENCE_MV: f32 = 0.1;
/// Flutter waves are looked for from this long after the R wave, ms (the QRS is over)...
const FLUTTER_AFTER_R_MS: f64 = 70.0;
/// ...to this long before the next R wave, ms.
const FLUTTER_BEFORE_R_MS: f64 = 40.0;
/// How far the median spacing of flutter waves within one RR may stray from the cycle.
const FLUTTER_SPACING_TOLERANCE: f64 = 0.25;
/// The restless baseline of fibrillation starts this long after the R wave, ms (the T wave is over
/// by then at these rates).
const BASELINE_AFTER_R_MS: f64 = 300.0;
/// Baseline stops this long before the next QRS, ms.
const BASELINE_BEFORE_QRS_MS: f64 = 40.0;
/// Atrial cycles considered flutter, ms (170–340 is 176–353/min).
const FLUTTER_CYCLE_MS: (f64, f64) = (170.0, 340.0);
/// How well flutter waves must line up on one cycle (0 … 1).
const FLUTTER_COHERENCE: f64 = 0.85;
/// Baseline extrema per second above which the atria are fibrillating (over 2.5 waves/s, where a
/// sinus rhythm shows one P wave per beat).
const FIBRILLATORY_EXTREMA_PER_S: f64 = 5.0;
/// Pre-excited AF is called at or above this rate with a broad QRS, bpm and ms. The QRS limit is
/// the usual 120 ms less the 10 or so that the measurement loses over a restless baseline.
const PREEXCITED_MIN_RATE_BPM: f64 = 150.0;
const PREEXCITED_MIN_QRS_MS: f64 = 110.0;
/// Velocity outside this distance from every R wave counts as the baseline's, ms.
const BASELINE_VELOCITY_REACH_MS: f64 = 120.0;
/// A QRS edge must rise above the velocity this share of the baseline stays under.
const BASELINE_VELOCITY_QUANTILE: f32 = 0.75;

/// How far around a deflection its height is measured, ms.
const QRS_SIZED_REACH_MS: f64 = 60.0;
/// Share of the height of a detected QRS that makes a deflection one too.
const QRS_SIZED_SHARE: f64 = 0.5;

/// A polymorphic or fibrillating ventricular rhythm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VentricularChaos {
    pub rhythm: RhythmKind,
    /// Deflections per minute.
    pub rate_bpm: f64,
    pub dominant_hz: f64,
    pub regularity: Regularity,
}

/// What the atria are doing between the QRS complexes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtrialFinding {
    pub rhythm: RhythmKind,
    pub atrial_rate_bpm: Option<f64>,
    pub av_ratio: Option<u32>,
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(0.0)
}

/// Sample positions of the deflections of a 12-lead window: maxima of the spatial velocity.
pub(super) fn deflections(leads: &[Vec<f32>], fs: f32) -> Vec<usize> {
    let w = ((0.01 * fs) as usize).max(1) | 1;
    let d: Vec<Vec<f32>> = leads
        .iter()
        .map(|x| dsp::derivative_per_ms(&dsp::moving_average(x, w), fs))
        .collect();
    let n = d[0].len();
    let vel: Vec<f32> = (0..n)
        .map(|i| d.iter().map(|l| l[i] * l[i]).sum::<f32>().sqrt())
        .collect();
    let env = dsp::moving_average(&vel, ((0.03 * fs) as usize).max(1) | 1);
    let mut sorted = env.clone();
    sorted.sort_by(f32::total_cmp);
    let p95 = sorted[(0.95 * (n - 1) as f32) as usize];
    if p95 <= 1e-5 {
        return Vec::new();
    }
    let thr = 0.35 * p95;
    let gap = (MIN_DEFLECTION_GAP_MS * 1e-3 * f64::from(fs)) as usize;
    let mut cand: Vec<usize> = (1..n - 1)
        .filter(|&i| env[i] >= thr && env[i] > env[i - 1] && env[i] >= env[i + 1])
        .collect();
    // strongest first; a kept deflection suppresses its neighbours
    cand.sort_by(|&a, &b| env[b].total_cmp(&env[a]));
    let mut kept: Vec<usize> = Vec::new();
    for c in cand {
        if kept.iter().all(|&k| k.abs_diff(c) >= gap) {
            kept.push(c);
        }
    }
    kept.sort_unstable();
    kept
}

/// Largest peak-to-peak of any lead around sample `at`, mV.
fn deflection_height(leads: &[Vec<f32>], at: usize, fs: f32) -> f64 {
    let reach = (QRS_SIZED_REACH_MS * f64::from(fs) / 1000.0) as usize;
    leads
        .iter()
        .map(|l| {
            let w = &l[at.saturating_sub(reach)..(at + reach).min(l.len())];
            let (lo, hi) = w
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
            f64::from(hi - lo)
        })
        .fold(0.0, f64::max)
}

/// The `deflections` as tall as a QRS: their height reaches half of the height around the median
/// R peak in `r_peaks`. T, P, flutter and f waves stay out.
pub(super) fn qrs_sized(leads: &[Vec<f32>], deflections: &[usize], r_peaks: &[usize], fs: f32) -> Vec<usize> {
    let mut heights: Vec<f64> = r_peaks.iter().map(|r| deflection_height(leads, *r, fs)).collect();
    let limit = QRS_SIZED_SHARE * median(&mut heights);
    deflections
        .iter()
        .copied()
        .filter(|d| deflection_height(leads, *d, fs) >= limit)
        .collect()
}

/// Frequency of the largest spectral peak of `x` in [`DOMINANT_BAND_HZ`], Hz (Goertzel scan).
fn dominant_frequency_hz(x: &[f32], fs: f32) -> f64 {
    let mean = x.iter().map(|v| f64::from(*v)).sum::<f64>() / x.len().max(1) as f64;
    let mut best = (0.0, -1.0);
    let mut f = DOMINANT_BAND_HZ.0;
    while f <= DOMINANT_BAND_HZ.1 {
        let w = std::f64::consts::TAU * f / f64::from(fs);
        let c = 2.0 * w.cos();
        let (mut s1, mut s2) = (0.0, 0.0);
        for v in x {
            let s = f64::from(*v) - mean + c * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        let p = s1 * s1 + s2 * s2 - c * s1 * s2;
        if p > best.1 {
            best = (f, p);
        }
        f += 0.1;
    }
    best.0
}

/// The polymorphic or fibrillating rhythm in `leads` (despiked, oldest first), if any.
#[must_use]
pub fn ventricular_chaos(leads: &[Vec<f32>], fs: f32) -> Option<VentricularChaos> {
    let peaks = deflections(leads, fs);
    if peaks.len() < MIN_DEFLECTIONS {
        return None;
    }
    let per_ms = f64::from(fs) / 1000.0;
    let cycles: Vec<f64> = peaks.windows(2).map(|p| (p[1] - p[0]) as f64 / per_ms).collect();
    let mean_cycle = cycles.iter().sum::<f64>() / cycles.len() as f64;
    let rate_bpm = 60_000.0 / mean_cycle;
    if rate_bpm < CHAOS_MIN_RATE_BPM {
        return None;
    }
    // Each deflection's vector: the 12 leads at the largest excursion in the 80 ms before it.
    let reach = (80.0 * per_ms) as usize;
    // Only the taller half: a T wave turns the vector from its QRS, but it is the QRS complexes
    // that keep or lose their axis.
    let mut heights: Vec<f64> = peaks.iter().map(|p| deflection_height(leads, *p, fs)).collect();
    let tall = median(&mut heights);
    let vectors: Vec<[f32; LEAD_COUNT]> = peaks
        .iter()
        .filter(|p| deflection_height(leads, **p, fs) >= tall)
        .map(|&p| {
            let lo = p.saturating_sub(reach);
            let at = (lo..=p)
                .max_by(|&a, &b| {
                    let e = |i: usize| leads.iter().map(|l| l[i] * l[i]).sum::<f32>();
                    e(a).total_cmp(&e(b))
                })
                .unwrap_or(p);
            std::array::from_fn(|l| leads[l][at])
        })
        .collect();
    let mut turns: Vec<f64> = vectors
        .windows(2)
        .map(|v| {
            let dot: f32 = v[0].iter().zip(&v[1]).map(|(a, b)| a * b).sum();
            let na: f32 = v[0].iter().map(|a| a * a).sum::<f32>().sqrt();
            let nb: f32 = v[1].iter().map(|a| a * a).sum::<f32>().sqrt();
            f64::from(dot / (na * nb).max(1e-9)).clamp(-1.0, 1.0).acos().to_degrees()
        })
        .collect();
    let turn = median(&mut turns);
    if turn < STABLE_TURN_DEG {
        return None;
    }
    let reference = &leads[Lead::II as usize];
    let dominant_hz = dominant_frequency_hz(reference, fs);
    let mean = mean_cycle;
    let var = cycles.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / cycles.len() as f64;
    let regularity = if var.sqrt() / mean > 0.1 {
        Regularity::Irregular
    } else {
        Regularity::Regular
    };
    let result = |rhythm| {
        Some(VentricularChaos {
            rhythm,
            rate_bpm,
            dominant_hz,
            regularity,
        })
    };
    if mean_cycle < VF_MAX_CYCLE_MS {
        return (turn >= JUMPING_TURN_DEG)
            .then_some(())
            .and_then(|()| result(RhythmKind::VentricularFibrillation));
    }
    if turn >= JUMPING_TURN_DEG {
        return result(RhythmKind::PolymorphicVentricularTachycardia);
    }
    // A slow orderly turn: the polarity of a lead must flip in runs of several beats.
    let mut runs: Vec<f64> = Vec::new();
    let mut count = 0.0;
    let mut sign = 0.0f32;
    let big = vectors
        .iter()
        .map(|v| v[Lead::II as usize].abs())
        .fold(0.0f32, f32::max);
    for v in &vectors {
        let x = v[Lead::II as usize];
        if x.abs() < 0.1 * big {
            count += 1.0;
            continue;
        }
        if sign != 0.0 && x.signum() != sign {
            runs.push(count);
            count = 0.0;
        }
        sign = x.signum();
        count += 1.0;
    }
    if runs.is_empty() {
        return None;
    }
    let run = median(&mut runs);
    (TWIST_RUN_BEATS.0..=TWIST_RUN_BEATS.1)
        .contains(&run)
        .then_some(())
        .and_then(|()| result(RhythmKind::TorsadesDePointes))
}

/// Local extrema of `x` of at least `prominence` against the opposite extremum before them, as
/// `(index, is_trough)`. The start of `x` is not an extremum, whichever way the signal leaves it.
fn extrema(x: &[f32], prominence: f32) -> Vec<(usize, bool)> {
    let mut out: Vec<(usize, bool)> = Vec::new();
    if x.len() < 3 {
        return out;
    }
    let (mut hi, mut lo) = ((0, x[0]), (0, x[0]));
    // Which way the signal has last moved by at least the prominence.
    let mut going_up: Option<bool> = None;
    for (i, &v) in x.iter().enumerate().skip(1) {
        if v > hi.1 {
            hi = (i, v);
        }
        if v < lo.1 {
            lo = (i, v);
        }
        let fell = hi.1 - v >= prominence;
        let rose = v - lo.1 >= prominence;
        match going_up {
            Some(true) | None if fell => {
                if hi.0 > 0 {
                    out.push((hi.0, false));
                }
                going_up = Some(false);
                lo = (i, v);
            }
            Some(false) | None if rose => {
                if lo.0 > 0 {
                    out.push((lo.0, true));
                }
                going_up = Some(true);
                hi = (i, v);
            }
            _ => {}
        }
    }
    out
}

/// How well `times` (ms) line up on a cycle: the best cycle in `range` and its coherence (0 … 1).
fn best_cycle(times: &[f64], range: (f64, f64)) -> (f64, f64) {
    let mut best = (0.0, 0.0);
    let mut c = range.0;
    while c <= range.1 {
        let (mut re, mut im) = (0.0, 0.0);
        for t in times {
            let phase = std::f64::consts::TAU * t / c;
            re += phase.cos();
            im += phase.sin();
        }
        let r = re.hypot(im) / times.len().max(1) as f64;
        if r > best.1 + 1e-9 {
            best = (c, r);
        }
        c += 1.0;
    }
    best
}

/// The flutter cycle in V1, ms: two or more waves in a good share of the RR intervals, spaced by
/// one cycle and all on the same cycle from one RR to the next.
///
/// The spacing test matters: anything that repeats with the RR also lines up on a short
/// sub-multiple of it, so coherence alone would call a slow regular rhythm flutter.
fn flutter_cycle(v1: &[f32], beats: &[Beat], per_ms: f64) -> Option<f64> {
    let after_r = (FLUTTER_AFTER_R_MS * per_ms) as usize;
    let before_r = (FLUTTER_BEFORE_R_MS * per_ms) as usize;
    let mut waves_ms: Vec<f64> = Vec::new();
    let mut spacings_ms: Vec<f64> = Vec::new();
    let (mut windows, mut with_two) = (0usize, 0usize);
    for pair in beats.windows(2) {
        let from = pair[0].r_peak + after_r;
        let to = pair[1].r_peak.saturating_sub(before_r);
        if to <= from {
            continue;
        }
        windows += 1;
        let waves: Vec<f64> = extrema(&v1[from..=to], FLUTTER_PROMINENCE_MV)
            .iter()
            .filter(|(_, is_trough)| !*is_trough)
            .map(|(i, _)| (from + i) as f64 / per_ms)
            .collect();
        if waves.len() >= 2 {
            with_two += 1;
        }
        spacings_ms.extend(waves.windows(2).map(|w| w[1] - w[0]));
        waves_ms.extend(waves);
    }
    // A wave next to the QRS can be lost in it, so some intervals show only one.
    if waves_ms.len() < 6 || with_two * 4 < windows {
        return None;
    }
    let (cycle, coherence) = best_cycle(&waves_ms, FLUTTER_CYCLE_MS);
    let spacing = median(&mut spacings_ms);
    ((coherence >= FLUTTER_COHERENCE) && (spacing - cycle).abs() <= FLUTTER_SPACING_TOLERANCE * cycle).then_some(cycle)
}

/// The P waves found stand at one distance before their QRS: whatever makes this rhythm irregular
/// (ectopics, runs of VT), the atria are organised. An f wave that happens to look like a P wave
/// has no fixed PR.
fn atria_lead_the_ventricles(beats: &[Beat]) -> bool {
    let mut pr: Vec<f64> = beats.iter().filter_map(|b| b.p.map(|p| (b.qrs_onset - p.onset) as f64)).collect();
    if pr.len() < 3 {
        return false;
    }
    let typical = median(&mut pr);
    let tolerance = 0.1 * typical;
    pr.iter().filter(|p| (*p - typical).abs() <= tolerance).count() * 2 >= pr.len()
}

/// Flutter or fibrillation from the signal between the QRS complexes.
#[must_use]
pub fn atrial_activity(
    leads: &[Vec<f32>],
    beats: &[Beat],
    fs: f32,
    regularity: Regularity,
    hr_bpm: Option<f64>,
) -> Option<AtrialFinding> {
    if beats.len() < 4 {
        return None;
    }
    let per_ms = f64::from(fs) / 1000.0;
    let smooth = ((0.02 * fs) as usize).max(1) | 1;
    let ii = dsp::moving_average(&leads[Lead::II as usize], smooth);
    let v1 = dsp::moving_average(&leads[Lead::V1 as usize], smooth);

    if let Some(cycle_ms) = flutter_cycle(&v1, beats, per_ms) {
        let av_ratio = match (regularity, hr_bpm) {
            (Regularity::Regular, Some(h)) => Some((60_000.0 / h / cycle_ms).round() as u32),
            _ => None,
        };
        return Some(AtrialFinding {
            rhythm: RhythmKind::AtrialFlutter,
            atrial_rate_bpm: Some(60_000.0 / cycle_ms),
            av_ratio,
        });
    }
    if regularity != Regularity::Irregular || atria_lead_the_ventricles(beats) {
        return None;
    }

    let after_r = (BASELINE_AFTER_R_MS * per_ms) as usize;
    let before_qrs = (BASELINE_BEFORE_QRS_MS * per_ms) as usize;
    let (mut baseline_ms, mut seen) = (0.0, 0usize);
    for pair in beats.windows(2) {
        let from = pair[0].r_peak + after_r;
        let to = pair[1].qrs_onset.saturating_sub(before_qrs);
        if to <= from + (100.0 * per_ms) as usize {
            continue;
        }
        baseline_ms += (to - from) as f64 / per_ms;
        seen += extrema(&ii[from..=to], MIN_PROMINENCE_MV)
            .len()
            .max(extrema(&v1[from..=to], MIN_PROMINENCE_MV).len());
    }
    (baseline_ms > 300.0 && seen as f64 / (baseline_ms / 1000.0) >= FIBRILLATORY_EXTREMA_PER_S).then_some(AtrialFinding {
        rhythm: RhythmKind::AtrialFibrillation,
        atrial_rate_bpm: None,
        av_ratio: None,
    })
}

/// Fast, broad and irregular: AF over an accessory pathway. The baseline is not asked, because at
/// this rate there is none left to read.
#[must_use]
pub fn pre_excited(regularity: Regularity, hr_bpm: Option<f64>, qrs_ms: Option<f64>) -> Option<AtrialFinding> {
    (regularity == Regularity::Irregular
        && hr_bpm.is_some_and(|h| h >= PREEXCITED_MIN_RATE_BPM)
        && qrs_ms.is_some_and(|q| q >= PREEXCITED_MIN_QRS_MS))
    .then_some(AtrialFinding {
        rhythm: RhythmKind::PreExcitedAtrialFibrillation,
        atrial_rate_bpm: None,
        av_ratio: None,
    })
}

/// The spatial velocity the baseline itself reaches, away from every QRS. Flutter and f waves are
/// never flat, so a QRS edge has to be told from them and not from a quiet line.
#[must_use]
pub fn baseline_velocity(vel: &[f32], beats: &[Beat], fs: f32) -> f32 {
    let reach = (BASELINE_VELOCITY_REACH_MS * f64::from(fs) / 1000.0) as usize;
    let mut quiet: Vec<f32> = vel
        .iter()
        .enumerate()
        .filter(|(i, _)| beats.iter().all(|b| b.r_peak.abs_diff(*i) > reach))
        .map(|(_, v)| *v)
        .collect();
    if quiet.is_empty() {
        return 0.0;
    }
    quiet.sort_by(f32::total_cmp);
    quiet[(BASELINE_VELOCITY_QUANTILE * (quiet.len() - 1) as f32) as usize]
}
