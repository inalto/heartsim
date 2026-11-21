//! Acceptance tests of brief B24: the silent phonocardiogram.
//!
//! * golden snapshots of the PCG envelope (5 s, 10 ms bins, 3 decimals) for eight rhythms;
//! * determinism: the same scenario gives the same samples, in one call or in frame-sized chunks;
//! * timing against the B4 analysis: A2 near the T-wave end, M1 40 ms after QRS onset;
//! * S1 loudness from the atrium-to-ventricle interval, S2 splitting from the activation order;
//! * silence in ventricular fibrillation and asystole.
//!
//! The ECG goldens (`golden.rs`) are untouched by the heart sounds: they never feed back.

use heartsim_core::Millis;
use heartsim_ecg::Tier0Scenario;
use heartsim_ecg::pcg::{
    BeatAssembler, BeatOrigin, Components, PCG_PERIOD_MS, PcgParams, PcgSynth, s1_gain_for_av, schedule,
};
use heartsim_ecg::scenario::{ANALYSIS_RATE_HZ, Recording, STEP_MS};
use heartsim_sim::{Engine, TimingEngine};
use std::fmt::Write as _;
use std::path::PathBuf;

fn scenario(stem: &str) -> Tier0Scenario {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{stem}.ron"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Tier0Scenario::from_ron(&text).unwrap()
}

/// Time of PCG sample `k`, ms.
fn pcg_time(k: usize) -> f64 {
    (k + 1) as f64 * PCG_PERIOD_MS
}

/// Max |PCG| over each `bin_ms` of the last `seconds`, as `t_ms,env` text.
fn envelope_csv(rec: &Recording, seconds: f64, bin_ms: f64) -> String {
    let per_bin = (bin_ms / PCG_PERIOD_MS).round() as usize;
    let want = (seconds * 1000.0 / PCG_PERIOD_MS).round() as usize;
    let tail = &rec.pcg[rec.pcg.len().saturating_sub(want)..];
    let mut out = String::from("t_ms,env\n");
    for (i, chunk) in tail.chunks(per_bin).enumerate() {
        let peak = chunk.iter().fold(0.0f32, |a, v| a.max(v.abs()));
        let _ = writeln!(out, "{:.0},{peak:.3}", i as f64 * bin_ms);
    }
    out
}

/// Every beat of a recording with its components (respiratory phase from the scenario's clock).
fn schedules(s: &Tier0Scenario, rec: &Recording) -> Vec<(heartsim_ecg::pcg::Beat, Components)> {
    let p = PcgParams::default();
    BeatAssembler::assemble(&rec.events, &p)
        .into_iter()
        .map(|b| {
            let phase =
                heartsim_sim::timing::respiratory_phase(s.timing.sinus.respiration_rate_bpm, b.qrs_onset.0);
            (b, schedule(&b, &p, phase))
        })
        .collect()
}

/// Peak |PCG| in `[from, from + len)` ms.
fn peak_between(rec: &Recording, from_ms: f64, len_ms: f64) -> f32 {
    rec.pcg
        .iter()
        .enumerate()
        .filter(|(k, _)| (from_ms..from_ms + len_ms).contains(&pcg_time(*k)))
        .fold(0.0f32, |a, (_, v)| a.max(v.abs()))
}

#[test]
fn golden_pcg_envelopes() {
    for stem in [
        "sinus_normal",
        "avb_1st",
        "avb_3rd_ventricular",
        "rbbb",
        "lbbb",
        "atrial_fibrillation",
        "paced_vvi",
        "vf_coarse",
    ] {
        let s = scenario(stem);
        let rec = s.record(20.0).unwrap();
        assert_eq!(
            rec.pcg.len(),
            4 * rec.len(),
            "{stem}: four PCG samples per ECG sample"
        );
        insta::assert_snapshot!(format!("pcg_{stem}"), envelope_csv(&rec, 5.0, 10.0));
    }
}

#[test]
fn the_same_scenario_gives_the_same_samples_in_one_call_or_in_chunks() {
    for stem in ["sinus_arrhythmia", "atrial_fibrillation", "paced_biv"] {
        let s = scenario(stem);
        let a = s.record(6.0).unwrap();
        let b = s.record(6.0).unwrap();
        assert_eq!(a.pcg, b.pcg, "{stem}: two runs");
        // the app's way: events fed step by step, samples taken in frame-sized blocks afterwards
        let mut eng = TimingEngine::new(s.timing.clone());
        eng.set_shock_seed(s.scenario.seed);
        let mut synth = s.pcg_synth();
        let mut chunked = Vec::new();
        let frame_steps = 17;
        let mut pending: Vec<f64> = Vec::new();
        for step in 0..6000 {
            eng.step(Millis(STEP_MS));
            synth.on_events(&eng.drain_events());
            for i in (0..4).rev() {
                pending.push(eng.now().0 - f64::from(i) * PCG_PERIOD_MS);
            }
            if step % frame_steps == frame_steps - 1 {
                chunked.extend(pending.drain(..).map(|t| synth.sample(Millis(t))));
            }
        }
        chunked.extend(pending.drain(..).map(|t| synth.sample(Millis(t))));
        assert!(
            s.scenario.timeline.is_empty(),
            "{stem}: the loop above applies no timeline"
        );
        assert_eq!(a.pcg, chunked, "{stem}: chunked sampling");
    }
}

/// Sinus scenarios at 50, 75 and 100 bpm: A2 within ±40 ms of the T end of the B4 analysis, and
/// M1 40 ± 5 ms after its QRS onset.
#[test]
fn a2_meets_the_t_end_and_m1_follows_the_qrs_onset() {
    let mut failures = Vec::new();
    for bpm in [50.0, 75.0, 100.0] {
        let mut s = scenario("sinus_normal");
        s.timing.sinus.rate_bpm = bpm;
        let rec = s.record(20.0).unwrap();
        let window_s = 10.0;
        let a = rec.analyze_tail(window_s);
        // analysis sample j is recording index first + 2 j, taken at (index + 1) · STEP_MS
        let every = (1000.0 / ANALYSIS_RATE_HZ / STEP_MS).round() as usize;
        let from = rec.len() - (window_s * 1000.0 / STEP_MS) as usize;
        let first = (from..from + every).find(|i| (i + 1) % every == 0).unwrap();
        let time_of = |j: usize| ((first + j * every) + 1) as f64 * STEP_MS;
        let sched = schedules(&s, &rec);
        let mut checked = 0;
        for beat in &a.beats {
            let (Some(t), onset) = (beat.t, time_of(beat.qrs_onset)) else {
                continue;
            };
            let Some((_, c)) = sched.iter().find(|(b, _)| (b.qrs_onset.0 - onset).abs() < 30.0) else {
                continue;
            };
            let t_end = time_of(t.offset);
            let a2 = c.a2.unwrap().onset.0;
            let m1 = c.m1.unwrap().onset.0;
            eprintln!(
                "{bpm} bpm: QRS onset {onset:.0}, M1 −QRS {:.1} ms, A2 − T end {:.1} ms",
                m1 - onset,
                a2 - t_end
            );
            if (a2 - t_end).abs() > 40.0 {
                failures.push(format!("{bpm} bpm: A2 {a2:.0} vs T end {t_end:.0}"));
            }
            if (m1 - onset - 40.0).abs() > 5.0 {
                failures.push(format!("{bpm} bpm: QRS {onset:.0} → M1 {m1:.0}"));
            }
            checked += 1;
        }
        assert!(checked >= 5, "{bpm} bpm: only {checked} beats measured");
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn s1_softens_with_the_av_interval_and_varies_in_complete_block_only() {
    let p = PcgParams::default();
    let mut prev = f64::INFINITY;
    for av in 120..=300 {
        let g = s1_gain_for_av(f64::from(av), &p);
        assert!(g <= prev, "S1 gain rises at AV {av} ms");
        prev = g;
    }
    assert!(s1_gain_for_av(100.0, &p) > s1_gain_for_av(250.0, &p) * 2.0);

    let s1_peaks = |stem: &str, seconds: f64| {
        let s = scenario(stem);
        let rec = s.record(seconds).unwrap();
        schedules(&s, &rec)
            .iter()
            .filter_map(|(b, c)| {
                let m1 = c.m1?.onset.0;
                // whole S1 inside the recording, and not the first beat (no preceding RR)
                (b.rr_ms.is_some() && m1 + 130.0 < rec.len() as f64 * STEP_MS)
                    .then(|| peak_between(&rec, m1, 130.0))
            })
            .collect::<Vec<f32>>()
    };
    let block = s1_peaks("avb_3rd_ventricular", 45.0);
    assert!(block.len() >= 20, "{} beats", block.len());
    let block = &block[..20];
    let (lo, hi) = (
        block.iter().copied().fold(f32::MAX, f32::min),
        block.iter().copied().fold(0.0, f32::max),
    );
    assert!(hi >= 2.0 * lo, "complete block: S1 {lo:.2}..{hi:.2}");
    let sinus = s1_peaks("sinus_normal", 20.0);
    let (lo, hi) = (
        sinus.iter().copied().fold(f32::MAX, f32::min),
        sinus.iter().copied().fold(0.0, f32::max),
    );
    assert!(sinus.len() >= 15 && hi < 1.1 * lo, "sinus: S1 {lo:.3}..{hi:.3}");
    assert!((0.9..1.15).contains(&hi), "normal S1 peak ≈ 1, got {hi}");
}

/// Splits of every beat of `stem` at peak inspiration and in expiration, ms.
fn splits(stem: &str) -> Vec<(f64, f64)> {
    let s = scenario(stem);
    let rec = s.record(10.0).unwrap();
    let p = PcgParams::default();
    BeatAssembler::assemble(&rec.events, &p)
        .iter()
        .filter(|b| b.rr_ms.is_some() && !b.replaces_previous)
        .filter_map(|b| {
            Some((
                schedule(b, &p, 0.25).split_ms()?,
                schedule(b, &p, 0.75).split_ms()?,
            ))
        })
        .collect()
}

#[test]
fn s2_splits_by_the_order_of_ventricular_activation() {
    for (insp, exp) in splits("sinus_normal") {
        assert!(exp < 30.0 && exp > 0.0, "normal expiration {exp}");
        assert!((30.0..=60.0).contains(&insp), "normal inspiration {insp}");
    }
    let rbbb = splits("rbbb");
    assert!(rbbb.len() > 5, "{} RBBB beats", rbbb.len());
    for (insp, exp) in rbbb {
        assert!(
            exp > 0.0 && insp > exp,
            "RBBB: P2 after A2, wider in inspiration ({exp}, {insp})"
        );
    }
    for stem in ["lbbb", "paced_vvi"] {
        let v = splits(stem);
        assert!(v.len() > 5, "{stem}: {} beats", v.len());
        for (insp, exp) in v {
            assert!(exp < 0.0, "{stem}: P2 before A2 in expiration ({exp})");
            assert!(
                insp.abs() < exp.abs(),
                "{stem}: the paradoxical split narrows on inspiration ({exp}, {insp})"
            );
        }
    }
    // the paced case of the research note: 100 ms in expiration, 70 ms in inspiration
    let (insp, exp) = splits("paced_vvi")[0];
    assert!(
        (exp + 100.0).abs() < 1e-6 && (insp + 70.0).abs() < 1e-6,
        "{exp} {insp}"
    );
}

#[test]
fn ventricular_fibrillation_and_asystole_are_silent() {
    let p = PcgParams::default();
    let normal_s1 = 1.0f32;
    // vf_coarse: the last organised beat, then nothing above 1 % of a normal S1
    let s = scenario("vf_coarse");
    let rec = s.record(20.0).unwrap();
    let beats = schedules(&s, &rec);
    let last_sound_end = beats
        .iter()
        .filter(|(b, _)| b.organised)
        .filter_map(|(_, c)| {
            [c.m1, c.t1, c.a2, c.p2]
                .into_iter()
                .flatten()
                .map(|x| x.onset.0 + 100.0)
                .reduce(f64::max)
        })
        .reduce(f64::max)
        .unwrap();
    assert!(
        beats.iter().any(|(b, _)| !b.organised),
        "vf_coarse has fibrillation wavelets"
    );
    assert!(
        beats
            .iter()
            .filter(|(b, _)| !b.organised)
            .all(|(_, c)| *c == Components::default())
    );
    let after: f32 = rec
        .pcg
        .iter()
        .enumerate()
        .filter(|(k, _)| pcg_time(*k) > last_sound_end)
        .fold(0.0, |a, (_, v)| a.max(v.abs()));
    assert!(rec.len() as f64 * STEP_MS > last_sound_end + 5000.0, "VF runs on");
    assert!(after < 0.01 * normal_s1, "VF: {after}");

    // asystole: sinus arrest with every escape off — no ventricular event, no sound
    let mut s = scenario("sinus_normal");
    s.timing.sinus.pause_every_n = 3;
    s.timing.sinus.pause_ms = 6000.0;
    s.timing.escape.junctional_bpm = 0.0;
    s.timing.escape.ventricular_bpm = 0.0;
    let rec = s.record(12.0).unwrap();
    let beats = BeatAssembler::assemble(&rec.events, &p);
    let last = beats.last().unwrap().qrs_onset.0;
    let quiet_from = last + 700.0;
    let next_ms = rec.len() as f64 * STEP_MS;
    assert!(next_ms - quiet_from > 1000.0, "a pause to listen to");
    let during: f32 = rec
        .pcg
        .iter()
        .enumerate()
        .filter(|(k, _)| pcg_time(*k) > quiet_from)
        .fold(0.0, |a, (_, v)| a.max(v.abs()));
    assert!(during < 0.01 * normal_s1, "asystole: {during}");
}

#[test]
fn a_very_early_ventricular_beat_has_no_second_sound() {
    let p = PcgParams::default();
    let s = scenario("pvc_r_on_t");
    let rec = s.record(10.0).unwrap();
    let beats = BeatAssembler::assemble(&rec.events, &p);
    let mut early = 0;
    for b in &beats {
        let c = schedule(b, &p, 0.75);
        if b.origin == BeatOrigin::Ventricular && b.rr_ms.is_some_and(|rr| rr < p.min_ejecting_rr_ms) {
            assert!(c.m1.is_some() && c.a2.is_none() && c.p2.is_none(), "{b:?}");
            early += 1;
        } else if b.organised {
            assert!(c.a2.is_some(), "{b:?}");
        }
    }
    eprintln!("{early} early ventricular beats");
}

#[test]
fn the_synth_labels_match_its_schedule() {
    let s = scenario("rbbb");
    let mut synth: PcgSynth = s.pcg_synth();
    synth.keep_beats(true);
    let mut eng = TimingEngine::new(s.timing.clone());
    for _ in 0..5000 {
        eng.step(Millis(1.0));
        synth.on_events(&eng.drain_events());
    }
    let marks = synth.take_all_marks();
    let beats = synth.beats();
    assert!(beats.len() >= 5);
    for (_, c) in beats {
        let s1 = c.m1.unwrap().onset;
        assert!(
            marks
                .iter()
                .any(|m| m.at == s1 && m.label == heartsim_ecg::SoundLabel::S1)
        );
        // RBBB splits by more than 20 ms in both phases: A2 and P2 are labelled
        let a2 = c.a2.unwrap().onset;
        assert!(
            marks
                .iter()
                .any(|m| m.at == a2 && m.label == heartsim_ecg::SoundLabel::A2)
        );
    }
}
