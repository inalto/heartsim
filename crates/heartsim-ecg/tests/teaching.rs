//! Acceptance tests of brief B28 in the signal layer:
//!
//! * clips: a regular rhythm loops with no step at the loop point larger than its neighbours; the
//!   same scenario, site and seed give the same clip; irregular rhythms get the fixed length with
//!   fades; the half-speed loop is twice as long; the cycle phase survives a switch between clips;
//! * breath-hold: at end-expiration the normal split stays under 30 ms beat after beat, at peak
//!   inspiration within 30–60 ms; the RR interval stops following the breath while held;
//! * small-speaker mode: the frequency factor moves the spectral centroid of S3 by the factor while
//!   the trace synthesiser is untouched.

use heartsim_core::{Action, BreathPhase, Millis};
use heartsim_ecg::clip::{CLIP_SETTLE_S, IRREGULAR_CLIP_S, PCG_PER_ECG};
use heartsim_ecg::pcg::{ComponentKind, PCG_PERIOD_MS, PCG_RATE_HZ, SMALL_SPEAKER_FACTOR};
use heartsim_ecg::scenario::STEP_MS;
use heartsim_ecg::{ChestPiece, ClipOptions, ListeningChoice, ListeningSite, SoundClip, Tier0Scenario};
use heartsim_sim::{Engine, EventKind, TimingEngine};
use std::path::PathBuf;

fn scenario(stem: &str) -> Tier0Scenario {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{stem}.ron"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Tier0Scenario::from_ron(&text).unwrap()
}

fn max_step(x: &[f32]) -> f32 {
    x.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max)
}

#[test]
fn a_regular_clip_loops_without_a_jump_and_is_whole_cycles() {
    for stem in [
        "sinus_normal",
        "lbbb",
        "aortic_regurgitation",
        "mitral_stenosis_sinus",
        "heart_failure_s3",
    ] {
        let s = scenario(stem);
        let clip = SoundClip::record(&s, ClipOptions::default()).unwrap();
        assert!(clip.regular, "{stem}: a regular rhythm");
        let n = clip.pcg.len();
        assert_eq!(n % PCG_PER_ECG, 0, "{stem}");
        assert!(clip.leads.iter().all(|l| l.len() == n / PCG_PER_ECG));
        assert!(
            (3.0..8.0).contains(&clip.duration_s()),
            "{stem}: {} s",
            clip.duration_s()
        );
        // the loop point: no step larger than the steps of the 10 samples on either side
        let wrap = (clip.pcg[0] - clip.pcg[n - 1]).abs();
        let mut around: Vec<f32> = clip.pcg[n - 10..].to_vec();
        around.extend_from_slice(&clip.pcg[..10]);
        let neighbours = max_step(&around);
        assert!(
            wrap <= neighbours + 1e-6,
            "{stem}: loop step {wrap} > neighbouring steps {neighbours}"
        );
        // a whole number of cycles: the gap from the last QRS round to the first equals the RR
        let q = &clip.qrs;
        assert!(q.len() >= 2, "{stem}");
        let rr = q[1] - q[0];
        let wrap_rr = q[0] + n - q[q.len() - 1];
        assert!(
            wrap_rr.abs_diff(rr) <= 16,
            "{stem}: RR {rr} vs round the loop {wrap_rr} samples"
        );
        // the clip carries its labels
        assert!(clip.marks.iter().any(|(_, l)| l.text() == "S1"), "{stem}");
        // half speed: twice as long, also smooth at its loop point
        let h = &clip.half_speed;
        assert!(h.len().abs_diff(2 * n) <= 2, "{stem}: {} vs {}", h.len(), 2 * n);
        let wrap = (h[0] - h[h.len() - 1]).abs();
        let mut around: Vec<f32> = h[h.len() - 10..].to_vec();
        around.extend_from_slice(&h[..10]);
        assert!(wrap <= max_step(&around) + 1e-6, "{stem}: half-speed loop step");
    }
}

#[test]
fn the_same_scenario_site_and_seed_give_the_same_clip() {
    let s = scenario("mitral_regurgitation");
    let o = ClipOptions {
        listening: ListeningChoice {
            site: ListeningSite::Mitral,
            chest: ChestPiece::Diaphragm,
        },
        breath_hold: None,
        small_speaker: false,
    };
    let a = SoundClip::record(&s, o).unwrap();
    let b = SoundClip::record(&s, o).unwrap();
    assert_eq!(a, b);
    // another site is another clip of the same span
    let c = SoundClip::record(
        &s,
        ClipOptions {
            listening: ListeningChoice {
                site: ListeningSite::Aortic,
                chest: ChestPiece::Diaphragm,
            },
            ..o
        },
    )
    .unwrap();
    assert_eq!(c.start_ms.to_bits(), a.start_ms.to_bits());
    assert_ne!(c.pcg, a.pcg);
    assert_eq!(c.leads, a.leads, "the ECG does not depend on the site");
    // small-speaker mode changes the sound played, not the trace
    let d = SoundClip::record(
        &s,
        ClipOptions {
            small_speaker: true,
            ..o
        },
    )
    .unwrap();
    assert_eq!(d.pcg, a.pcg, "the trace is the true signal");
    assert_eq!(d.sound.len(), a.sound.len());
    assert_ne!(d.sound, a.sound);
    assert_eq!(a.sound, a.pcg);
}

#[test]
fn an_irregular_rhythm_gets_the_fixed_length_with_fades() {
    for stem in ["atrial_fibrillation", "sinus_arrhythmia"] {
        let clip = SoundClip::record(&scenario(stem), ClipOptions::default()).unwrap();
        assert!(!clip.regular, "{stem}");
        assert!(
            (clip.duration_s() - IRREGULAR_CLIP_S).abs() < 0.01,
            "{stem}: {} s",
            clip.duration_s()
        );
        let n = clip.pcg.len();
        assert_eq!(clip.pcg[0], 0.0);
        assert_eq!(clip.pcg[n - 1], 0.0);
        // the fade: the first and last few ms are scaled down, smoothly
        assert!(clip.pcg[..40].iter().all(|v| v.abs() < 0.5));
        assert!(clip.start_ms >= CLIP_SETTLE_S * 1000.0);
    }
}

#[test]
fn switching_between_clips_keeps_the_point_of_the_cycle() {
    let a = SoundClip::record(&scenario("innocent_murmur"), ClipOptions::default()).unwrap();
    let b = SoundClip::record(&scenario("aortic_stenosis_severe"), ClipOptions::default()).unwrap();
    for pos in (0..a.pcg.len()).step_by(997) {
        let (cycle, f) = a.cycle_phase(pos).unwrap();
        let there = b.index_at_phase(cycle, f);
        let (c2, f2) = b.cycle_phase(there).unwrap();
        assert_eq!(c2, cycle % b.qrs.len(), "at {pos}");
        let one_sample = 1.0 / (b.qrs[1] - b.qrs[0]) as f64;
        assert!((f2 - f).abs() <= 1.5 * one_sample, "at {pos}: {f} → {f2}");
    }
}

/// Split (P2 − A2, ms) and RR of every beat after `from_ms`, from a run of `stem` with `actions`.
fn splits_and_rr(stem: &str, seconds: f64, actions: &[(f64, Action)], from_ms: f64) -> (Vec<f64>, Vec<f64>) {
    let s = scenario(stem);
    let mut eng = TimingEngine::new(s.timing.clone());
    eng.set_shock_seed(s.scenario.seed);
    let mut pcg = s.pcg_synth();
    pcg.keep_beats(true);
    let mut pending: Vec<(f64, Action)> = actions.to_vec();
    let mut qrs = Vec::new();
    let mut last = f64::NEG_INFINITY;
    for _ in 0..(seconds * 1000.0 / STEP_MS) as usize {
        pending.retain(|(at, a)| {
            if *at <= eng.now().0 {
                eng.apply(a, Millis(*at)).unwrap();
                pcg.set_respiration(eng.respiration());
                false
            } else {
                true
            }
        });
        eng.step(Millis(STEP_MS));
        let evs = eng.drain_events();
        for e in &evs {
            if e.region.is_ventricular() && e.kind != EventKind::Blocked && e.at.0 - last > 150.0 {
                last = e.at.0;
                if e.at.0 >= from_ms {
                    qrs.push(e.at.0);
                }
            }
        }
        pcg.on_events(&evs);
        let _ = pcg.sample(eng.now());
    }
    let splits = pcg
        .beats()
        .iter()
        .filter(|(b, _)| b.qrs_onset.0 >= from_ms)
        .filter_map(|(_, c)| Some(c.p2?.onset.0 - c.a2?.onset.0))
        .collect();
    let rr = qrs.windows(2).map(|w| w[1] - w[0]).collect();
    (splits, rr)
}

#[test]
fn a_breath_hold_freezes_the_split_beat_after_beat() {
    let hold = |p| [(2000.0, Action::BreathHold { phase: Some(p) })];
    let (exp, _) = splits_and_rr("sinus_normal", 20.0, &hold(BreathPhase::EndExpiration), 3000.0);
    assert!(exp.len() > 15);
    assert!(exp.iter().all(|s| (0.0..30.0).contains(s)), "{exp:?}");
    let (insp, _) = splits_and_rr("sinus_normal", 20.0, &hold(BreathPhase::PeakInspiration), 3000.0);
    assert!(insp.len() > 15);
    assert!(insp.iter().all(|s| (30.0..=60.0).contains(s)), "{insp:?}");
    // without the hold the split follows the breath through both ranges
    let (free, _) = splits_and_rr("sinus_normal", 20.0, &[], 3000.0);
    let spread =
        free.iter().copied().fold(f64::MIN, f64::max) - free.iter().copied().fold(f64::MAX, f64::min);
    assert!(spread > 20.0, "{free:?}");
}

#[test]
fn a_breath_hold_stops_the_sinus_arrhythmia_and_a_release_restarts_it() {
    let (_, free) = splits_and_rr("sinus_arrhythmia", 30.0, &[], 3000.0);
    let range =
        |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max) - v.iter().copied().fold(f64::MAX, f64::min);
    assert!(range(&free) > 100.0, "breathing moves the RR: {free:?}");
    let actions = [
        (
            3000.0,
            Action::BreathHold {
                phase: Some(BreathPhase::EndExpiration),
            },
        ),
        (20_000.0, Action::BreathHold { phase: None }),
    ];
    let (_, held) = splits_and_rr("sinus_arrhythmia", 19_000.0 / 1000.0, &actions, 5000.0);
    assert!(held.len() > 10);
    assert!(range(&held) <= 1.0, "held RR: {held:?}");
    let (_, released) = splits_and_rr("sinus_arrhythmia", 40.0, &actions, 21_000.0);
    assert!(range(&released) > 100.0, "breathing again: {released:?}");
}

/// Spectral centroid of `x` at the PCG rate, Hz (power-weighted, zero-padded DFT up to 1 kHz).
fn centroid_hz(x: &[f32]) -> f64 {
    let fs = f64::from(PCG_RATE_HZ);
    let n = 8192usize;
    let (mut num, mut den) = (0.0, 0.0);
    for bin in 1..(n * 1000 / fs as usize) {
        let f = bin as f64 * fs / n as f64;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let ph = std::f64::consts::TAU * f * i as f64 / fs;
            re += f64::from(*v) * ph.cos();
            im += f64::from(*v) * ph.sin();
        }
        let p = re * re + im * im;
        num += f * p;
        den += p;
    }
    num / den
}

#[test]
fn small_speaker_mode_raises_the_s3_by_the_factor_and_leaves_the_trace_alone() {
    let s = scenario("heart_failure_s3");
    let run = |factor: f64| {
        let mut eng = TimingEngine::new(s.timing.clone());
        let mut pcg = s.pcg_synth();
        pcg.set_frequency_factor(factor);
        pcg.keep_beats(true);
        let mut out = Vec::new();
        for _ in 0..6000 {
            eng.step(Millis(STEP_MS));
            pcg.on_events(&eng.drain_events());
            for i in (0..4).rev() {
                out.push(pcg.sample(Millis(eng.now().0 - f64::from(i) * PCG_PERIOD_MS)));
            }
        }
        let s3 = pcg
            .scheduled_sounds()
            .iter()
            .find(|c| c.kind == ComponentKind::S3 && c.onset_ms > 3000.0)
            .copied()
            .unwrap();
        (out, s3.onset_ms)
    };
    let (plain, onset) = run(1.0);
    let (scaled, onset2) = run(SMALL_SPEAKER_FACTOR);
    assert_eq!(onset.to_bits(), onset2.to_bits(), "same schedule");
    // the S3 window: from its onset for 80 ms (nothing else sounds then in this scenario)
    let k = (onset / PCG_PERIOD_MS) as usize;
    let w = (80.0 / PCG_PERIOD_MS) as usize;
    let c1 = centroid_hz(&plain[k..k + w]);
    let c3 = centroid_hz(&scaled[k..k + w]);
    let ratio = c3 / c1;
    assert!(
        (ratio / SMALL_SPEAKER_FACTOR - 1.0).abs() < 0.15,
        "centroid {c1:.1} Hz → {c3:.1} Hz, ratio {ratio:.2}"
    );
    // factor 1 is the trace, bit for bit
    let (again, _) = run(1.0);
    assert!(plain.iter().zip(&again).all(|(a, b)| a.to_bits() == b.to_bits()));
}
