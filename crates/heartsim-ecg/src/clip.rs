//! Loopable heart-sound clips (brief B28): an offline recording of a scenario at one listening site
//! and chest piece, cut so that it can be played over and over by the Sound Trainer and the
//! auscultation quiz.
//!
//! A [`SoundClip`] is the single source of what those pages play, draw and label: the PCG loop (and
//! its half-speed version), the twelve ECG leads over the same span at 500 Hz, the sound labels and
//! the QRS positions. It is a pure function of the scenario, the listening choice and the
//! breath-hold ([`ClipOptions`]): the same inputs give the same samples on every platform.
//!
//! * **Regular rhythms** (every RR interval within [`REGULAR_TOLERANCE_MS`] of the mean) are cut at a
//!   quiet point in diastole into a whole number of cycles near [`CLIP_TARGET_S`]; the last
//!   [`LOOP_CROSSFADE_MS`] are blended into the samples that preceded the start, so the loop point is
//!   as smooth as any other step of the recording.
//! * **Irregular rhythms** (atrial fibrillation, ectopy, sinus arrhythmia while breathing) get
//!   [`IRREGULAR_CLIP_S`] with a [`IRREGULAR_FADE_MS`] fade at both ends: the loop passes through a
//!   short silence instead of pretending to repeat.
//! * **Half speed** is the slow-motion synthesiser ([`crate::WallTimeSynth`]) run offline over the
//!   same events at 0.5×: each sound keeps its pitch, the intervals double.

use crate::listening::{ListeningChoice, SiteTable};
use crate::pcg::{PCG_PERIOD_MS, PCG_RATE_HZ, SoundLabel};
use crate::scenario::{Recording, STEP_MS, Tier0Error, Tier0Scenario};
use heartsim_core::{Action, BreathPhase, LEAD_COUNT, Millis, TimedAction};
use heartsim_sim::timing::RespiratoryClock;

/// Time the rhythm is given to settle before the clip may start, s. Designed (the rhythm quiz's
/// settling time: Wenckebach cycles and re-entry started by a timed beat have settled by then).
pub const CLIP_SETTLE_S: f64 = 4.0;
/// Length a regular clip aims for, s: the whole number of cycles closest to it (at least
/// [`MIN_CYCLES`]). Designed: about one breath at 12 breaths/min, short enough for many listens.
pub const CLIP_TARGET_S: f64 = 5.0;
/// Fewest cycles in a regular clip. Designed: a single cycle hides the beat-to-beat changes.
pub const MIN_CYCLES: usize = 2;
/// Length of an irregular clip, s. Designed (brief B28: 8–10 s).
pub const IRREGULAR_CLIP_S: f64 = 9.0;
/// Fade at each end of an irregular clip, ms. Designed: long enough not to click, short enough not
/// to swallow a sound.
pub const IRREGULAR_FADE_MS: f64 = 40.0;
/// Blend at the loop point of a regular clip, ms. Designed: a few ms of quiet diastole.
pub const LOOP_CROSSFADE_MS: f64 = 5.0;
/// A rhythm is regular when every RR interval of the window is within this of their mean, ms.
/// Designed: well below the ±10 % of a sinus arrhythmia and above the 1 ms engine step.
pub const REGULAR_TOLERANCE_MS: f64 = 20.0;
/// Sampling rate of the clip's ECG, Hz (the strip's).
pub const CLIP_ECG_RATE_HZ: f32 = 500.0;
/// PCG samples per ECG sample of a clip.
pub const PCG_PER_ECG: usize = (PCG_RATE_HZ / CLIP_ECG_RATE_HZ) as usize;
/// Ventricular events closer than this belong to one beat, ms (the heart-sound model's
/// `same_beat_ms`).
const SAME_BEAT_MS: f64 = 150.0;
/// Speed of the half-speed version.
pub const HALF_SPEED: f64 = 0.5;

/// What a clip is recorded with besides the scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ClipOptions {
    /// Listening site and chest piece (the overview by default).
    pub listening: ListeningChoice,
    /// Hold the breath from the start of the run (`Action::BreathHold` at 0 ms).
    pub breath_hold: Option<BreathPhase>,
    /// Play the sound in small-speaker mode ([`crate::SMALL_SPEAKER_FACTOR`]); the trace stays true.
    pub small_speaker: bool,
}

/// A loopable recording of a scenario's heart sounds with its ECG. See the module documentation.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundClip {
    /// Id of the scenario recorded.
    pub scenario_id: String,
    pub options: ClipOptions,
    /// The loop at [`PCG_RATE_HZ`] as the trace shows it, a multiple of [`PCG_PER_ECG`] samples long.
    pub pcg: Vec<f32>,
    /// The loop as it is played: [`Self::pcg`] itself, or its small-speaker version (same length).
    pub sound: Vec<f32>,
    /// The played loop re-timed at [`HALF_SPEED`] for listening (twice as long, natural pitch).
    pub half_speed: Vec<f32>,
    /// Twelve leads (`Lead as usize`) at [`CLIP_ECG_RATE_HZ`], `pcg.len() / PCG_PER_ECG` samples
    /// each: ECG sample `j` is taken at the same instant as PCG sample `8 j + 7`.
    pub leads: Vec<Vec<f32>>,
    /// Sound labels as `(PCG index, label)`, in time order.
    pub marks: Vec<(u64, SoundLabel)>,
    /// PCG index of every QRS onset (engine ventricular event) in the clip, in order.
    pub qrs: Vec<usize>,
    /// Whether the clip is a whole number of cycles (`false`: fixed length with fades).
    pub regular: bool,
    /// Simulation time of the clip's start, ms.
    pub start_ms: f64,
}

impl SoundClip {
    /// Record `scenario` with `options` and cut the clip.
    ///
    /// # Errors
    /// The scenario cannot be run (see [`Tier0Scenario::record`]).
    pub fn record(scenario: &Tier0Scenario, options: ClipOptions) -> Result<Self, Tier0Error> {
        let mut s = scenario.clone();
        if let Some(phase) = options.breath_hold {
            s.scenario.timeline.push(TimedAction {
                at: Millis(0.0),
                action: Action::BreathHold { phase: Some(phase) },
            });
        }
        let listening = SiteTable::builtin().listening(options.listening);
        let total_s = CLIP_SETTLE_S + IRREGULAR_CLIP_S.max(CLIP_TARGET_S) + 3.0;
        let rec = s.record_at(total_s, Some(&listening))?;
        let qrs_ms = beat_times(&rec, CLIP_SETTLE_S * 1000.0);
        let (start_ms, end_ms, regular) = cut(&rec, &qrs_ms, total_s * 1000.0);
        let k0 = (start_ms / PCG_PERIOD_MS).round() as usize;
        let k1 = (end_ms / PCG_PERIOD_MS).round() as usize;
        let per_ms = f64::from(PCG_RATE_HZ) / 1000.0;
        let cut_loop = |all: &[f32]| {
            let mut out = all[k0..k1].to_vec();
            if regular {
                crossfade(&mut out, &all[..k0], (LOOP_CROSSFADE_MS * per_ms) as usize);
            } else {
                fade_ends(&mut out, (IRREGULAR_FADE_MS * per_ms) as usize);
            }
            out
        };
        let pcg = cut_loop(&rec.pcg);
        let factor = if options.small_speaker {
            crate::pcg::SMALL_SPEAKER_FACTOR
        } else {
            1.0
        };
        let sound = if options.small_speaker {
            cut_loop(&s.record_with(total_s, Some(&listening), factor)?.pcg)
        } else {
            pcg.clone()
        };
        let half_speed = half_speed(&s, &listening, &rec, start_ms, end_ms, regular, options, factor);
        let t0 = (start_ms / STEP_MS).round() as usize;
        let n_ecg = pcg.len() / PCG_PER_ECG;
        let step = (1000.0 / f64::from(CLIP_ECG_RATE_HZ) / STEP_MS).round() as usize;
        let leads = (0..LEAD_COUNT)
            .map(|l| {
                (0..n_ecg)
                    .map(|j| rec.leads[l][t0 + step * j + step - 1])
                    .collect()
            })
            .collect();
        let marks = rec
            .pcg_marks
            .iter()
            .filter_map(|m| {
                // the sample at time `at` has index `at / period − 1` (see `Recording::pcg`)
                let abs = (m.at.0 / PCG_PERIOD_MS).round() - 1.0;
                let k = abs - k0 as f64;
                (k >= 0.0 && k < pcg.len() as f64).then_some((k as u64, m.label))
            })
            .collect();
        let qrs = qrs_ms
            .iter()
            .filter(|&&q| q >= start_ms && q < end_ms)
            .map(|q| ((q - start_ms) / PCG_PERIOD_MS).round() as usize)
            .collect();
        Ok(Self {
            scenario_id: scenario.scenario.id.clone(),
            options,
            pcg,
            sound,
            half_speed,
            leads,
            marks,
            qrs,
            regular,
            start_ms,
        })
    }

    /// Length of the loop, s.
    #[must_use]
    pub fn duration_s(&self) -> f64 {
        self.pcg.len() as f64 / f64::from(PCG_RATE_HZ)
    }

    /// Where in the cardiac cycle PCG index `pos` lies: the cycle (QRS to next QRS, the last one
    /// wrapping round to the first QRS of the next loop) and the fraction of it, `0.0..1.0`. `None`
    /// without a QRS (ventricular fibrillation).
    #[must_use]
    pub fn cycle_phase(&self, pos: usize) -> Option<(usize, f64)> {
        let n = self.qrs.len();
        if n == 0 || self.pcg.is_empty() {
            return None;
        }
        let len = self.pcg.len();
        let pos = pos % len;
        // before the first QRS: the tail of the last cycle, continued from the loop before
        let cycle = self.qrs.iter().rposition(|&q| q <= pos).unwrap_or(n - 1);
        let from = self.qrs[cycle];
        let at = if pos < self.qrs[0] { pos + len } else { pos };
        let next = if cycle + 1 < n {
            self.qrs[cycle + 1]
        } else {
            self.qrs[0] + len
        };
        let length = next.saturating_sub(from).max(1);
        Some((
            cycle,
            ((at - from) as f64 / length as f64).clamp(0.0, 1.0 - 1e-12),
        ))
    }

    /// The PCG index at `fraction` of cycle `cycle` (wrapped onto this clip's cycles): the
    /// counterpart of [`Self::cycle_phase`], so that switching from one clip to another keeps the
    /// point of the cycle (contrast pairs).
    #[must_use]
    pub fn index_at_phase(&self, cycle: usize, fraction: f64) -> usize {
        let n = self.qrs.len();
        let len = self.pcg.len().max(1);
        if n == 0 {
            return ((fraction.clamp(0.0, 1.0) * len as f64) as usize).min(len - 1);
        }
        let c = cycle % n;
        let from = self.qrs[c];
        let next = if c + 1 < n {
            self.qrs[c + 1]
        } else {
            self.qrs[0] + len
        };
        let at = from as f64 + fraction.clamp(0.0, 1.0) * (next - from) as f64;
        (at.round() as usize) % len
    }
}

/// Simulation times of the first ventricular event of every beat at or after `from_ms`.
fn beat_times(rec: &Recording, from_ms: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    let mut last = f64::NEG_INFINITY;
    for e in &rec.events {
        if e.region.is_ventricular() && e.kind != heartsim_sim::EventKind::Blocked {
            if e.at.0 - last > SAME_BEAT_MS && e.at.0 >= from_ms {
                out.push(e.at.0);
            }
            if e.at.0 - last > SAME_BEAT_MS {
                last = e.at.0;
            }
        }
    }
    out
}

/// Round `ms` down to the clip grid (one ECG sample, 2 ms).
fn on_grid(ms: f64) -> f64 {
    let g = 1000.0 / f64::from(CLIP_ECG_RATE_HZ);
    (ms / g).floor() * g
}

/// Start, end and regularity of the clip.
fn cut(rec: &Recording, qrs: &[f64], total_ms: f64) -> (f64, f64, bool) {
    let settle = CLIP_SETTLE_S * 1000.0;
    let window_end = settle + 1000.0 * IRREGULAR_CLIP_S.max(CLIP_TARGET_S) + 1000.0;
    let in_window: Vec<f64> = qrs.iter().copied().filter(|q| *q <= window_end).collect();
    let rr: Vec<f64> = in_window.windows(2).map(|w| w[1] - w[0]).collect();
    let regular = rr.len() > MIN_CYCLES && {
        let mean = rr.iter().sum::<f64>() / rr.len() as f64;
        rr.iter().all(|r| (r - mean).abs() <= REGULAR_TOLERANCE_MS)
    };
    if regular {
        let mean = rr.iter().sum::<f64>() / rr.len() as f64;
        let cycles = ((CLIP_TARGET_S * 1000.0 / mean).round() as usize)
            .max(MIN_CYCLES)
            .min(rr.len() - 1);
        let d = quiet_offset(rec, in_window[0], in_window[1]);
        let start = on_grid(in_window[1] - d);
        let end = on_grid(in_window[1 + cycles] - d);
        return (start, end, true);
    }
    // irregular: a fixed length from a quiet point before the second beat (or from the settled
    // start when there are no beats)
    let start = match (in_window.first(), in_window.get(1)) {
        (Some(&a), Some(&b)) => on_grid(b - quiet_offset(rec, a, b)),
        _ => on_grid(settle),
    };
    let end = on_grid((start + IRREGULAR_CLIP_S * 1000.0).min(total_ms - 1000.0));
    (start, end, false)
}

/// How long before the QRS at `next_ms` the quietest 10 ms of the diastole that ends there lie, ms:
/// searched between 55 % and 12 % of the cycle before the QRS (after S2 and S3, before S4 and any
/// presystolic sound of most rhythms).
fn quiet_offset(rec: &Recording, prev_ms: f64, next_ms: f64) -> f64 {
    let rr = next_ms - prev_ms;
    let half_window = (5.0 / PCG_PERIOD_MS) as i64;
    let energy = |t: f64| -> f64 {
        let k = (t / PCG_PERIOD_MS) as i64;
        (k - half_window..k + half_window)
            .filter_map(|i| usize::try_from(i).ok().and_then(|i| rec.pcg.get(i)))
            .map(|v| f64::from(*v).powi(2))
            .sum()
    };
    let (lo, hi) = (0.12 * rr, 0.55 * rr);
    let mut best = (f64::INFINITY, lo);
    let mut d = lo;
    while d <= hi {
        let e = energy(next_ms - d);
        if e < best.0 {
            best = (e, d);
        }
        d += 2.0;
    }
    best.1
}

/// Raised-cosine weight of position `i` of `n`, rising from near 0 to 1.
fn ramp(i: usize, n: usize) -> f32 {
    let x = (i + 1) as f64 / n.max(1) as f64;
    (0.5 - 0.5 * (std::f64::consts::PI * x).cos()) as f32
}

/// Blend the last `n` samples of `clip` into the `n` samples of the recording that precede it
/// (`before`), so the last sample is the one before the first: the loop point is an ordinary step.
fn crossfade(clip: &mut [f32], before: &[f32], n: usize) {
    let n = n.min(clip.len()).min(before.len());
    let len = clip.len();
    let pre = &before[before.len() - n..];
    for (i, (c, p)) in clip[len - n..].iter_mut().zip(pre).enumerate() {
        let w = ramp(i, n);
        *c = *c * (1.0 - w) + p * w;
    }
}

/// Fade both ends of `clip` over `n` samples.
fn fade_ends(clip: &mut [f32], n: usize) {
    let n = n.min(clip.len() / 2);
    let len = clip.len();
    for i in 0..n {
        let w = ramp(i, n);
        clip[i] *= w;
        clip[len - 1 - i] *= w;
    }
    if let Some(first) = clip.first_mut() {
        *first = 0.0;
    }
    if let Some(last) = clip.last_mut() {
        *last = 0.0;
    }
}

/// The half-speed loop: the slow-motion synthesiser fed the recording's events, keeping up at 1×
/// until a little before the start, then rendering at [`HALF_SPEED`] to the end; cut and blended like
/// the clip itself.
#[allow(clippy::too_many_arguments, reason = "the parts of one clip, computed once")]
fn half_speed(
    scenario: &Tier0Scenario,
    listening: &crate::listening::Listening,
    rec: &Recording,
    start_ms: f64,
    end_ms: f64,
    regular: bool,
    options: ClipOptions,
    frequency_factor: f64,
) -> Vec<f32> {
    let mut wall = scenario.wall_synth();
    wall.set_listening(listening);
    wall.set_frequency_factor(frequency_factor);
    if let Some(phase) = options.breath_hold {
        let mut clock = RespiratoryClock::default();
        clock.hold(Some(phase), scenario.timing.sinus.respiration_rate_bpm, 0.0);
        wall.set_respiration(clock);
    }
    let pre_ms = LOOP_CROSSFADE_MS * 2.0;
    let slow_from = start_ms - pre_ms;
    let mut out = Vec::new();
    let mut next_event = 0;
    let steps = (end_ms / STEP_MS).ceil() as usize;
    let mut slowed = false;
    for i in 1..=steps {
        let t = i as f64 * STEP_MS;
        while let Some(e) = rec.events.get(next_event)
            && e.at.0 <= t
        {
            wall.on_events(std::slice::from_ref(e));
            next_event += 1;
        }
        if t <= slow_from {
            wall.follow(t);
        } else {
            if !slowed {
                wall.set_speed(HALF_SPEED);
                slowed = true;
            }
            wall.render_until_sim(t, &mut out);
        }
    }
    let per_ms = f64::from(PCG_RATE_HZ) / 1000.0 / HALF_SPEED;
    let pre = ((start_ms - slow_from.max(0.0)) * per_ms).round() as usize;
    let len = (((end_ms - start_ms) * per_ms).round() as usize).min(out.len().saturating_sub(pre));
    let (before, rest) = out.split_at(pre.min(out.len()));
    let mut clip = rest[..len.min(rest.len())].to_vec();
    if regular {
        let n = (LOOP_CROSSFADE_MS * per_ms) as usize;
        crossfade(&mut clip, before, n);
    } else {
        fade_ends(&mut clip, (IRREGULAR_FADE_MS * per_ms) as usize);
    }
    clip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ramp_rises_to_one() {
        assert!(ramp(0, 10) < 0.05);
        assert!((ramp(9, 10) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_crossfade_ends_on_the_sample_before_the_start() {
        let before = [0.0, 0.1, 0.2, 0.3];
        let mut clip = vec![1.0; 10];
        crossfade(&mut clip, &before, 4);
        assert!((clip[9] - 0.3).abs() < 1e-6);
        assert!((clip[0] - 1.0).abs() < 1e-6);
    }
}
