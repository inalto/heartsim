//! Acceptance tests of brief B27, part A: gallops, clicks, the snap, the rub and the murmurs.
//!
//! * frozen output: an empty `auscultation` section equals none, sample for sample, and the section
//!   never changes the ECG;
//! * golden PCG envelopes, one per new scenario;
//! * determinism and random access: one call equals frame-sized chunks for every new scenario, and
//!   the wall-time synthesiser at 1× equals `PcgSynth` on a gallop, a systolic and a diastolic murmur;
//! * the timing rules (S3, S4, summation, snap), the rhythm rules (no S4 and no presystolic accent in
//!   AF, S4 after the P waves in complete block, an ejection murmur louder after a PVC while MR is
//!   not), the shapes (aortic stenosis by severity, aortic regurgitation, mitral regurgitation), the
//!   respiratory rules (TR, the fixed split of ASD);
//! * every new scenario's `expected` auscultation block matches what its schedule contains.

use heartsim_core::{Auscultation, ExtraSound, LesionKind, MurmurTiming, Severity, SplitMode};
use heartsim_ecg::Tier0Scenario;
use heartsim_ecg::auscultation::{Murmur, MurmurClass, SoundGains};
use heartsim_ecg::pcg::{
    BeatAssembler, ComponentKind, PCG_PERIOD_MS, PcgParams, PcgSynth, ScheduledSound, SoundLabel, schedule,
    schedule_with,
};
use heartsim_ecg::scenario::{Recording, STEP_MS};
use heartsim_sim::{Engine, EventKind, Origin, TimingEngine};
use std::fmt::Write as _;
use std::path::PathBuf;

/// The scenarios of brief B27 (the valve and gallop presets), and the two rhythm presets that gained
/// a section.
const NEW: [&str; 15] = [
    "heart_failure_s3",
    "lvh_s4",
    "summation_gallop",
    "aortic_stenosis_mild",
    "aortic_stenosis_severe",
    "aortic_regurgitation",
    "mitral_regurgitation",
    "mitral_stenosis_sinus",
    "mitral_stenosis_af",
    "mitral_valve_prolapse",
    "tricuspid_regurgitation",
    "pulmonary_stenosis",
    "ventricular_septal_defect",
    "atrial_septal_defect",
    "innocent_murmur",
];
const WITH_SECTION: [&str; 2] = ["pericarditis", "stemi_anterior"];

fn path(stem: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{stem}.ron"))
}

fn text(stem: &str) -> String {
    std::fs::read_to_string(path(stem)).unwrap_or_else(|e| panic!("{stem}: {e}"))
}

fn scenario(stem: &str) -> Tier0Scenario {
    Tier0Scenario::from_ron(&text(stem)).unwrap_or_else(|e| panic!("{stem}: {e}"))
}

fn with_model(stem: &str, model: Auscultation) -> Tier0Scenario {
    let mut s = scenario(stem);
    s.scenario.auscultation = model;
    s
}

/// Time of PCG sample `k`, ms (as `Recording::pcg`).
fn pcg_time(k: usize) -> f64 {
    (k + 1) as f64 * PCG_PERIOD_MS
}

/// Run `s` for `seconds` as `Tier0Scenario::record` does (timeline applied), with every beat and
/// sound kept and the given gains. Returns the trace and the synthesiser.
fn run(s: &Tier0Scenario, seconds: f64, gains: Option<SoundGains>) -> (Vec<f32>, PcgSynth) {
    let mut eng = TimingEngine::new(s.timing.clone());
    eng.set_shock_seed(s.scenario.seed);
    let mut timeline = s.scenario.timeline.clone();
    timeline.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
    let mut next = 0;
    let mut synth = s.pcg_synth();
    synth.keep_beats(true);
    if let Some(g) = gains {
        synth.set_gains(g);
    }
    let mut out = Vec::new();
    for _ in 0..(seconds * 1000.0 / STEP_MS).round() as usize {
        while let Some(a) = timeline.get(next)
            && a.at.0 <= eng.now().0
        {
            eng.apply(&a.action, a.at).unwrap();
            next += 1;
        }
        eng.step(heartsim_core::Millis(STEP_MS));
        synth.on_events(&eng.drain_events());
        for i in (0..4).rev() {
            out.push(synth.sample(heartsim_core::Millis(eng.now().0 - f64::from(i) * PCG_PERIOD_MS)));
        }
    }
    (out, synth)
}

fn rms_between(pcg: &[f32], from_ms: f64, to_ms: f64) -> f64 {
    let v: Vec<f64> = pcg
        .iter()
        .enumerate()
        .filter(|(k, _)| (from_ms..to_ms).contains(&pcg_time(*k)))
        .map(|(_, v)| f64::from(*v))
        .collect();
    (v.iter().map(|x| x * x).sum::<f64>() / v.len().max(1) as f64).sqrt()
}

fn peak_between(pcg: &[f32], from_ms: f64, to_ms: f64) -> f32 {
    pcg.iter()
        .enumerate()
        .filter(|(k, _)| (from_ms..to_ms).contains(&pcg_time(*k)))
        .fold(0.0f32, |a, (_, v)| a.max(v.abs()))
}

fn sounds(synth: &PcgSynth, kind: ComponentKind) -> Vec<ScheduledSound> {
    synth
        .scheduled_sounds()
        .iter()
        .filter(|s| s.kind == kind)
        .copied()
        .collect()
}

fn murmurs(synth: &PcgSynth, class: MurmurClass) -> Vec<Murmur> {
    synth
        .scheduled_murmurs()
        .iter()
        .filter(|m| m.class == class)
        .cloned()
        .collect()
}

/// Only the murmurs of `class` are heard.
fn only(class: MurmurClass) -> SoundGains {
    let mut g = SoundGains::murmurs_only();
    for c in MurmurClass::ALL {
        g.murmurs[c.index()] = if c == class { 1.0 } else { 0.0 };
    }
    g
}

/// Max |PCG| over each `bin_ms` of the last `seconds`, as `t_ms,env` text (the B24 golden format).
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

// --- frozen output ---------------------------------------------------------------------------------

#[test]
fn an_empty_section_equals_none_and_the_section_never_changes_the_ecg() {
    let plain = text("sinus_normal");
    let empty = plain.replacen(
        "    timing: TimingParams(",
        "    auscultation: (),\n    timing: TimingParams(",
        1,
    );
    assert_ne!(plain, empty);
    let a = Tier0Scenario::from_ron(&plain).unwrap().record(10.0).unwrap();
    let b = Tier0Scenario::from_ron(&empty).unwrap().record(10.0).unwrap();
    assert!(a.pcg.iter().zip(&b.pcg).all(|(x, y)| x.to_bits() == y.to_bits()));
    assert_eq!(a.pcg.len(), b.pcg.len());
    // the loudest sections: the ECG and the events are those of the scenario without the section
    for stem in ["mitral_stenosis_sinus", "pericarditis", "aortic_stenosis_severe"] {
        let s = scenario(stem);
        let bare = with_model(stem, Auscultation::default());
        let (x, y) = (s.record(8.0).unwrap(), bare.record(8.0).unwrap());
        assert_eq!(x.leads, y.leads, "{stem}: ECG");
        assert_eq!(x.events, y.events, "{stem}: events");
        assert_ne!(x.pcg, y.pcg, "{stem}: the section is heard");
    }
}

#[test]
fn golden_pcg_envelopes_of_the_new_scenarios() {
    for stem in NEW.iter().chain(&WITH_SECTION) {
        let rec = scenario(stem).record(20.0).unwrap();
        insta::assert_snapshot!(format!("ausc_{stem}"), envelope_csv(&rec, 5.0, 10.0));
    }
}

// --- determinism and random access -----------------------------------------------------------------

#[test]
fn every_new_scenario_samples_the_same_in_one_call_or_in_chunks() {
    for stem in NEW.iter().chain(&WITH_SECTION) {
        let s = scenario(stem);
        let seconds = 6.0;
        let a = s.record(seconds).unwrap();
        let mut eng = TimingEngine::new(s.timing.clone());
        eng.set_shock_seed(s.scenario.seed);
        let mut timeline = s.scenario.timeline.clone();
        timeline.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
        let mut next = 0;
        let mut synth = s.pcg_synth();
        let mut chunked = Vec::new();
        let mut pending: Vec<f64> = Vec::new();
        for step in 0..(seconds * 1000.0) as usize {
            while let Some(act) = timeline.get(next)
                && act.at.0 <= eng.now().0
            {
                eng.apply(&act.action, act.at).unwrap();
                next += 1;
            }
            eng.step(heartsim_core::Millis(STEP_MS));
            synth.on_events(&eng.drain_events());
            for i in (0..4).rev() {
                pending.push(eng.now().0 - f64::from(i) * PCG_PERIOD_MS);
            }
            if step % 17 == 16 {
                chunked.extend(pending.drain(..).map(|t| synth.sample(heartsim_core::Millis(t))));
            }
        }
        chunked.extend(pending.drain(..).map(|t| synth.sample(heartsim_core::Millis(t))));
        let first = a
            .pcg
            .iter()
            .zip(&chunked)
            .position(|(x, y)| x.to_bits() != y.to_bits());
        assert_eq!(first, None, "{stem}: chunked sampling differs at {first:?}");
        assert_eq!(a.pcg.len(), chunked.len());
    }
}

#[test]
fn at_speed_one_the_wall_time_synth_equals_the_trace_with_gallops_and_murmurs() {
    for stem in [
        "summation_gallop",
        "aortic_stenosis_severe",
        "mitral_stenosis_af",
        "mitral_stenosis_sinus",
        "pericarditis",
    ] {
        let s = scenario(stem);
        let mut eng = TimingEngine::new(s.timing.clone());
        eng.set_shock_seed(s.scenario.seed);
        let mut trace_synth = s.pcg_synth();
        let mut wall = s.wall_synth();
        let (mut trace, mut sound) = (Vec::new(), Vec::new());
        for _ in 0..10_000 {
            eng.step(heartsim_core::Millis(STEP_MS));
            let evs = eng.drain_events();
            trace_synth.on_events(&evs);
            wall.on_events(&evs);
            for i in (0..4).rev() {
                trace.push(
                    trace_synth.sample(heartsim_core::Millis(eng.now().0 - f64::from(i) * PCG_PERIOD_MS)),
                );
            }
            wall.render_until_sim(eng.now().0, &mut sound);
        }
        assert_eq!(trace.len(), sound.len(), "{stem}");
        let first = trace
            .iter()
            .zip(&sound)
            .position(|(a, b)| a.to_bits() != b.to_bits());
        assert_eq!(first, None, "{stem}: differs at sample {first:?}");
    }
}

#[test]
fn in_slow_motion_a_murmur_keeps_its_band_and_its_envelope_stretches() {
    let s = scenario("mitral_regurgitation");
    let mut eng = TimingEngine::new(s.timing.clone());
    let mut wall = s.wall_synth();
    let mut only_mr = only(MurmurClass::MitralRegurgitation);
    only_mr.components = [0.0; ComponentKind::ALL.len()];
    wall.set_gains(only_mr.clone());
    wall.set_speed(0.25);
    let (_, synth) = run(&s, 3.0, Some(only_mr));
    let mut sound = Vec::new();
    for _ in 0..3000 {
        eng.step(heartsim_core::Millis(STEP_MS));
        wall.on_events(&eng.drain_events());
        wall.render_until_sim(eng.now().0, &mut sound);
    }
    let mr = murmurs(&synth, MurmurClass::MitralRegurgitation);
    let m = &mr[0];
    let (sim_len, wall_start) = (m.end_ms - m.start_ms, m.start_ms / 0.25);
    // the envelope lasts four times as long: sound from the wall start for 4 × its length
    let wall_of = |ms: f64| (ms / PCG_PERIOD_MS) as usize;
    let inside = &sound[wall_of(wall_start + 40.0)..wall_of(wall_start + 4.0 * sim_len - 40.0)];
    let after =
        &sound[wall_of(wall_start + 4.0 * sim_len + 10.0)..wall_of(wall_start + 4.0 * sim_len + 100.0)];
    let rms = |v: &[f32]| (v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
    assert!(rms(inside) > 0.05, "{}", rms(inside));
    assert!(rms(after) < 1e-9, "{}", rms(after));
    // the band stays: zero crossings per second as at 1× (within 20 %)
    let zc =
        |v: &[f32]| v.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count() as f64 / v.len() as f64;
    let (one_x, _) = run(
        &s,
        3.0,
        Some({
            let mut g = only(MurmurClass::MitralRegurgitation);
            g.components = [0.0; ComponentKind::ALL.len()];
            g
        }),
    );
    let at_one = &one_x[wall_of(m.start_ms + 10.0)..wall_of(m.end_ms - 10.0)];
    let ratio = zc(inside) / zc(at_one);
    assert!((0.8..1.25).contains(&ratio), "zero-crossing rate ratio {ratio}");
}

// --- the extra sounds --------------------------------------------------------------------------------

#[test]
fn s3_follows_a2_by_120_to_180_ms_and_s4_lies_between_the_atrial_activation_and_m1() {
    let (_, hf) = run(&scenario("heart_failure_s3"), 10.0, None);
    let mut n = 0;
    for (_, c) in hf.beats() {
        if let (Some(a2), Some(s3)) = (c.a2, c.s3) {
            let d = s3.onset.0 - a2.onset.0;
            assert!((120.0..=180.0).contains(&d), "A2 → S3 {d}");
            n += 1;
        }
    }
    assert!(n >= 10, "{n} S3");
    assert_eq!(sounds(&hf, ComponentKind::S4), Vec::new());

    let (_, lvh) = run(&scenario("lvh_s4"), 10.0, None);
    let s4 = sounds(&lvh, ComponentKind::S4);
    assert!(s4.len() >= 10, "{} S4", s4.len());
    let m1s: Vec<f64> = lvh
        .beats()
        .iter()
        .filter_map(|(_, c)| c.m1.map(|m| m.onset.0))
        .collect();
    for s in &s4 {
        let next_m1 = m1s
            .iter()
            .copied()
            .find(|m| *m > s.key_ms)
            .unwrap_or(f64::INFINITY);
        let end = s.onset_ms + PcgParams::default().auscultation.s4.duration_ms;
        assert!(
            s.onset_ms > s.key_ms && end <= next_m1,
            "S4 {s:?} vs M1 {next_m1}"
        );
    }
    assert_eq!(sounds(&lvh, ComponentKind::S3), Vec::new());
}

#[test]
fn at_140_bpm_s3_and_s4_overlap_and_their_sum_is_louder_than_either() {
    let both = scenario("summation_gallop");
    let s3_only = with_model(
        "summation_gallop",
        Auscultation {
            s3: true,
            ..Auscultation::default()
        },
    );
    let s4_only = with_model(
        "summation_gallop",
        Auscultation {
            s4: true,
            ..Auscultation::default()
        },
    );
    let mut gallops = SoundGains::default();
    for k in ComponentKind::ALL {
        gallops.components[k as usize] =
            f64::from(u8::from(matches!(k, ComponentKind::S3 | ComponentKind::S4)));
    }
    let (sum, synth) = run(&both, 8.0, Some(gallops.clone()));
    let (a, _) = run(&s3_only, 8.0, Some(gallops.clone()));
    let (b, _) = run(&s4_only, 8.0, Some(gallops));
    let p = PcgParams::default().auscultation;
    let s3 = sounds(&synth, ComponentKind::S3);
    let s4 = sounds(&synth, ComponentKind::S4);
    let mut checked = 0;
    for x in &s3 {
        let Some(y) = s4.iter().find(|y| (y.onset_ms - x.onset_ms).abs() < 60.0) else {
            continue;
        };
        let (lo, hi) = (
            x.onset_ms.max(y.onset_ms),
            (x.onset_ms + p.s3.duration_ms).min(y.onset_ms + p.s4.duration_ms),
        );
        assert!(
            hi > lo,
            "the envelopes overlap: S3 {} S4 {}",
            x.onset_ms,
            y.onset_ms
        );
        let (from, to) = (x.onset_ms.min(y.onset_ms), x.onset_ms.max(y.onset_ms) + 90.0);
        let (ps, pa, pb) = (
            peak_between(&sum, from, to),
            peak_between(&a, from, to),
            peak_between(&b, from, to),
        );
        assert!(ps > pa && ps > pb, "summed peak {ps} vs S3 {pa}, S4 {pb}");
        checked += 1;
    }
    assert!(checked >= 10, "{checked} summation beats");
}

#[test]
fn no_s4_and_no_presystolic_accent_in_af_and_the_sinus_rumble_rises_before_s1() {
    // AF, even when the section asks for an S4
    let mut model = scenario("mitral_stenosis_af").scenario.auscultation;
    model.s4 = true;
    let af = with_model("mitral_stenosis_af", model);
    let (sig, synth) = run(&af, 15.0, Some(SoundGains::murmurs_only()));
    assert_eq!(sounds(&synth, ComponentKind::S4), Vec::new(), "no S4 in AF");
    assert!(
        murmurs(&synth, MurmurClass::MitralPresystolic).is_empty(),
        "no presystolic accent in AF"
    );
    // the loudness contour (every murmur's envelope × gain) in the last 50 ms before S1 against the
    // contour 150–100 ms earlier, for each cycle long enough to have both after the snap
    let rise = |synth: &PcgSynth| -> Vec<(f64, f64)> {
        let level = |t: f64| {
            synth
                .scheduled_murmurs()
                .iter()
                .map(|m| m.level_at(t))
                .sum::<f64>()
        };
        let beats = synth.beats();
        beats
            .windows(2)
            .filter_map(|w| {
                let next_m1 = w[1].1.m1?.onset.0;
                let snap = w[0].1.opening_snap?.onset.0;
                (next_m1 - 200.0 > snap + 60.0).then(|| {
                    let late = (0..10)
                        .map(|i| level(next_m1 - 50.0 + 5.0 * f64::from(i)))
                        .sum::<f64>()
                        / 10.0;
                    let mid = (0..10)
                        .map(|i| level(next_m1 - 200.0 + 5.0 * f64::from(i)))
                        .sum::<f64>()
                        / 10.0;
                    (late, mid)
                })
            })
            .collect()
    };
    let af_rise = rise(&synth);
    assert!(af_rise.len() >= 4, "{} long AF cycles", af_rise.len());
    assert!(
        af_rise.iter().all(|(late, mid)| late <= mid),
        "AF: the rumble does not rise before S1 {af_rise:?}"
    );
    // and what is heard: no more sound just before S1 than in mid-diastole
    let m1s: Vec<f64> = synth
        .beats()
        .iter()
        .filter_map(|(_, c)| c.m1.map(|m| m.onset.0))
        .collect();
    let (late, mid): (f64, f64) = m1s
        .windows(2)
        .filter(|w| w[1] - w[0] > 650.0)
        .map(|w| {
            (
                rms_between(&sig, w[1] - 100.0, w[1] - 5.0),
                rms_between(&sig, w[1] - 300.0, w[1] - 205.0),
            )
        })
        .fold((0.0, 0.0), |a, x| (a.0 + x.0, a.1 + x.1));
    assert!(
        late < 1.15 * mid,
        "AF: heard rumble {late:.3} late vs {mid:.3} mid"
    );

    let (sig, synth) = run(
        &scenario("mitral_stenosis_sinus"),
        10.0,
        Some(SoundGains::murmurs_only()),
    );
    assert_ne!(murmurs(&synth, MurmurClass::MitralPresystolic), Vec::new());
    let sinus_rise = rise(&synth);
    assert!(sinus_rise.len() >= 8);
    assert!(
        sinus_rise.iter().all(|(late, mid)| *late > 1.5 * mid),
        "sinus: the rumble rises before S1 {sinus_rise:?}"
    );
    let m1s: Vec<f64> = synth
        .beats()
        .iter()
        .filter_map(|(_, c)| c.m1.map(|m| m.onset.0))
        .collect();
    let (late, mid): (f64, f64) = m1s
        .windows(2)
        .map(|w| {
            (
                rms_between(&sig, w[1] - 100.0, w[1] - 5.0),
                rms_between(&sig, w[1] - 300.0, w[1] - 205.0),
            )
        })
        .fold((0.0, 0.0), |a, x| (a.0 + x.0, a.1 + x.1));
    assert!(
        late > 1.5 * mid,
        "sinus: heard rumble {late:.3} late vs {mid:.3} mid"
    );
}

#[test]
fn in_complete_block_the_s4_follows_the_p_waves() {
    let s = with_model(
        "avb_3rd_ventricular",
        Auscultation {
            s4: true,
            ..Auscultation::default()
        },
    );
    let rec = s.record(20.0).unwrap();
    let (_, synth) = run(&s, 20.0, None);
    let atrial: Vec<f64> = rec
        .events
        .iter()
        .filter(|e| {
            e.region.is_atrial() && e.kind != EventKind::Blocked && !matches!(e.origin, Origin::Circuit(_))
        })
        .map(|e| e.at.0)
        .collect();
    let s4 = sounds(&synth, ComponentKind::S4);
    let beats = synth.beats().len();
    let p = PcgParams::default().auscultation;
    assert!(
        s4.len() > beats,
        "{} S4 for {beats} beats: more P waves than QRS",
        s4.len()
    );
    for x in &s4 {
        assert!(
            atrial.iter().any(|a| a.to_bits() == x.key_ms.to_bits()),
            "S4 keyed to a P wave"
        );
        assert!((x.onset_ms - x.key_ms - p.atrial_s4_ms).abs() < 1e-9);
    }
    // its distance from the preceding QRS varies widely: it is not tied to the ventricles
    let qrs: Vec<f64> = synth.beats().iter().map(|(b, _)| b.qrs_onset.0).collect();
    let gaps: Vec<f64> = s4
        .iter()
        .filter_map(|x| {
            qrs.iter()
                .rev()
                .find(|q| **q < x.onset_ms)
                .map(|q| x.onset_ms - q)
        })
        .collect();
    let (lo, hi) = gaps
        .iter()
        .fold((f64::MAX, 0.0f64), |(l, h), g| (l.min(*g), h.max(*g)));
    assert!(hi - lo > 300.0, "QRS → S4 {lo:.0}..{hi:.0}");
    // a P wave in ventricular systole has no S4
    assert!(
        s4.len() < atrial.len(),
        "{} S4 of {} P waves",
        s4.len(),
        atrial.len()
    );
}

#[test]
fn the_opening_snap_follows_a2_by_30_to_140_ms_and_is_earlier_when_severe() {
    let p = PcgParams::default();
    let s = scenario("mitral_stenosis_sinus");
    let rec = s.record(6.0).unwrap();
    let beats = BeatAssembler::assemble(&rec.events, &p);
    let b = beats.iter().find(|b| b.rr_ms.is_some()).unwrap();
    let mut prev = f64::INFINITY;
    for sev in [Severity::Mild, Severity::Moderate, Severity::Severe] {
        let model = Auscultation {
            lesions: vec![(LesionKind::MitralStenosis, sev)],
            ..Auscultation::default()
        };
        let c = schedule_with(b, &p, 0.5, &model);
        let d = c.opening_snap.unwrap().onset.0 - c.a2.unwrap().onset.0;
        assert!((30.0..=140.0).contains(&d), "{sev:?}: A2 → OS {d}");
        assert!(d < prev, "{sev:?} earlier");
        prev = d;
    }
    assert!(prev < 70.0, "severe under 70 ms (Nemani & Pechetty)");
    // and in the running synthesiser, every beat of both presets
    for stem in ["mitral_stenosis_sinus", "mitral_stenosis_af"] {
        let (_, synth) = run(&scenario(stem), 8.0, None);
        let n = synth
            .beats()
            .iter()
            .filter_map(|(_, c)| Some(c.opening_snap?.onset.0 - c.a2?.onset.0))
            .inspect(|d| assert!((30.0..=140.0).contains(d), "{stem}: {d}"))
            .count();
        assert!(n >= 6, "{stem}: {n} snaps");
    }
}

// --- murmurs and the rhythm ---------------------------------------------------------------------------

/// Sinus rhythm at 75 bpm (beats at 355 + 800 k ms) with one ventricular stimulus 600 ms after the
/// sixth beat.
fn with_pvc(lesion: LesionKind) -> Tier0Scenario {
    let text = format!(
        r#"Scenario(format_version: 1, id: "pvc_case", title: "T", description: "d", tier: Template, seed: 42,
            auscultation: (lesions: [({lesion:?}, Moderate)]),
            timeline: [TimedAction(at: 4955.0, action: Stimulate(site: Landmark("rv_apex"), strength: 1.0, duration_ms: 2.0))],
            timing: TimingParams(sinus: SinusParams(rate_bpm: 75.0, respiration_depth: 0.0)))"#
    );
    Tier0Scenario::from_ron(&text).unwrap()
}

#[test]
fn after_a_compensatory_pause_the_ejection_murmur_grows_and_mitral_regurgitation_does_not() {
    let mut ratios = Vec::new();
    for (lesion, class) in [
        (LesionKind::AorticStenosis, MurmurClass::AorticStenosis),
        (LesionKind::MitralRegurgitation, MurmurClass::MitralRegurgitation),
    ] {
        let s = with_pvc(lesion);
        let (sig, synth) = run(&s, 9.0, Some(only(class)));
        let beats: Vec<_> = synth.beats().iter().map(|(b, _)| *b).collect();
        let pvc = beats
            .iter()
            .position(|b| b.qrs_onset.0 > 4900.0 && b.qrs_onset.0 < 5000.0)
            .expect("the stimulus captured");
        let rr = |i: usize| beats[i].rr_ms.unwrap();
        // the case: a premature beat and a fully compensatory pause
        let normal = rr(pvc - 1);
        assert!(rr(pvc) < 0.8 * normal, "premature: {} vs {normal}", rr(pvc));
        assert!(
            (rr(pvc) + rr(pvc + 1) - 2.0 * normal).abs() < 0.1 * normal,
            "compensatory: {} + {} vs 2 × {normal}",
            rr(pvc),
            rr(pvc + 1)
        );
        let list = murmurs(&synth, class);
        let rms_of = |q: f64| {
            let m = list
                .iter()
                .find(|m| (m.key_ms - q).abs() < 1.0)
                .expect("murmur of the beat");
            rms_between(&sig, m.start_ms, m.stop_ms())
        };
        let before: f64 = (pvc - 3..pvc).map(|i| rms_of(beats[i].qrs_onset.0)).sum::<f64>() / 3.0;
        let after = rms_of(beats[pvc + 1].qrs_onset.0);
        eprintln!("{lesion:?}: before {before:.4} after the pause {after:.4}");
        ratios.push(after / before);
    }
    assert!(
        ratios[0] >= 1.2,
        "aortic stenosis after the pause: × {:.3}",
        ratios[0]
    );
    assert!(
        (ratios[1] - 1.0).abs() < 0.1,
        "mitral regurgitation after the pause: × {:.3}",
        ratios[1]
    );
}

#[test]
fn aortic_stenosis_peaks_later_when_severe_after_a_silent_gap_and_softens_a2() {
    let shape = |stem: &str| {
        let (sig, synth) = run(&scenario(stem), 10.0, Some(only(MurmurClass::AorticStenosis)));
        let list = murmurs(&synth, MurmurClass::AorticStenosis);
        assert!(list.len() >= 8, "{stem}");
        let mut peak_fraction = 0.0;
        let mut a2_gain = 0.0;
        let mut n = 0.0;
        for (b, c) in synth.beats().iter().filter(|(b, _)| b.rr_ms.is_some()) {
            let Some(m) = list
                .iter()
                .find(|m| m.key_ms.to_bits() == b.qrs_onset.0.to_bits())
            else {
                continue;
            };
            let (m1, a2) = (c.m1.unwrap(), c.a2.unwrap());
            // a silent gap after S1: the murmur starts well after M1, and nothing of it before
            assert!(
                m.start_ms - m1.onset.0 >= 60.0,
                "{stem}: gap {}",
                m.start_ms - m1.onset.0
            );
            assert!(
                peak_between(&sig, m1.onset.0, m.start_ms) < 1e-6,
                "{stem}: silent after S1"
            );
            assert!(m.stop_ms() <= a2.onset.0, "{stem}: ends before A2");
            // where the envelope peaks, as a fraction of S1 → A2
            let (t, _) = (0..1000)
                .map(|i| m.start_ms + (m.stop_ms() - m.start_ms) * f64::from(i) / 1000.0)
                .map(|t| (t, m.level_at(t)))
                .fold((0.0, 0.0), |acc, x| if x.1 > acc.1 { x } else { acc });
            peak_fraction += (t - m1.onset.0) / (a2.onset.0 - m1.onset.0);
            a2_gain += a2.gain;
            n += 1.0;
        }
        (peak_fraction / n, a2_gain / n)
    };
    let (mild_peak, mild_a2) = shape("aortic_stenosis_mild");
    let (severe_peak, severe_a2) = shape("aortic_stenosis_severe");
    eprintln!("peak at {mild_peak:.2} (mild) vs {severe_peak:.2} (severe) of systole");
    assert!(severe_peak > mild_peak + 0.1, "later peak when severe");
    assert!(severe_a2 < 0.5 * mild_a2, "A2 {mild_a2:.2} → {severe_a2:.2}");
}

#[test]
fn aortic_regurgitation_starts_at_a2_and_decreases_and_mitral_regurgitation_is_a_plateau_from_m1_through_a2()
{
    let p = PcgParams::default().auscultation;
    let (_, ar) = run(&scenario("aortic_regurgitation"), 8.0, None);
    let list = murmurs(&ar, MurmurClass::AorticRegurgitation);
    assert!(list.len() >= 6);
    for (b, c) in ar.beats() {
        let Some(m) = list
            .iter()
            .find(|m| m.key_ms.to_bits() == b.qrs_onset.0.to_bits())
        else {
            continue;
        };
        let a2 = c.a2.unwrap().onset.0;
        assert!(
            (0.0..=10.0).contains(&(m.start_ms - a2)),
            "starts at A2: {}",
            m.start_ms - a2
        );
        // non-increasing after its attack, to its end or cut
        let from = m.start_ms + p.decrescendo_attack_ms;
        let mut prev = f64::INFINITY;
        let mut t = from;
        while t < m.stop_ms() {
            let v = m.level_at(t);
            assert!(v <= prev + 1e-12, "AR rises at {t}");
            prev = v;
            t += 1.0;
        }
    }
    let (_, mr) = run(&scenario("mitral_regurgitation"), 8.0, None);
    let list = murmurs(&mr, MurmurClass::MitralRegurgitation);
    assert!(list.len() >= 6);
    for (b, c) in mr.beats() {
        let Some(m) = list
            .iter()
            .find(|m| m.key_ms.to_bits() == b.qrs_onset.0.to_bits())
        else {
            continue;
        };
        let (m1, a2) = (c.m1.unwrap().onset.0, c.a2.unwrap().onset.0);
        assert!(m.start_ms - m1 <= 15.0, "from S1");
        assert!(m.stop_ms() >= a2 + 10.0, "through A2");
        // plateau: between the ramps the envelope varies by less than 1 %
        let (lo, hi) = (m.start_ms + p.plateau_ramp_ms, m.stop_ms() - p.plateau_ramp_ms);
        let levels: Vec<f64> = (0..=100)
            .map(|i| m.level_at(lo + (hi - lo) * f64::from(i) / 100.0))
            .collect();
        let (mn, mx) = levels
            .iter()
            .fold((f64::MAX, 0.0f64), |(a, b), v| (a.min(*v), b.max(*v)));
        assert!(mx - mn < 0.01 * mx, "MR plateau {mn}..{mx}");
    }
}

#[test]
fn tricuspid_regurgitation_grows_on_inspiration_and_the_asd_split_is_fixed() {
    let s = scenario("tricuspid_regurgitation");
    let (sig, synth) = run(&s, 20.0, Some(only(MurmurClass::TricuspidRegurgitation)));
    let rate = s.timing.sinus.respiration_rate_bpm;
    let list = murmurs(&synth, MurmurClass::TricuspidRegurgitation);
    let by_phase: Vec<(f64, f64)> = list
        .iter()
        .map(|m| {
            let depth = heartsim_sim::timing::inspiration_depth(heartsim_sim::timing::respiratory_phase(
                rate, m.key_ms,
            ));
            (depth, rms_between(&sig, m.start_ms, m.stop_ms()))
        })
        .collect();
    let insp = by_phase
        .iter()
        .copied()
        .fold((0.0, 0.0), |a, x| if x.0 > a.0 { x } else { a });
    let exp = by_phase
        .iter()
        .copied()
        .fold((2.0, 0.0), |a, x| if x.0 < a.0 { x } else { a });
    assert!(
        insp.0 > 0.8 && exp.0 < 0.2,
        "the run spans the breathing cycle {insp:?} {exp:?}"
    );
    assert!(
        insp.1 > 1.2 * exp.1,
        "TR louder on inspiration: {insp:?} vs {exp:?}"
    );

    let p = PcgParams::default();
    let change = |stem: &str| {
        let s = scenario(stem);
        let rec = s.record(6.0).unwrap();
        BeatAssembler::assemble(&rec.events, &p)
            .iter()
            .filter(|b| b.rr_ms.is_some())
            .map(|b| {
                let m = &s.scenario.auscultation;
                let i = schedule_with(b, &p, 0.25, m).split_ms().unwrap();
                let e = schedule_with(b, &p, 0.75, m).split_ms().unwrap();
                (i - e).abs()
            })
            .fold(0.0f64, f64::max)
    };
    let asd = change("atrial_septal_defect");
    let normal = change("sinus_normal");
    assert!(asd < 20.0, "ASD split changes by {asd} ms");
    assert!(normal >= 20.0, "normal split changes by {normal} ms");
}

// --- the scenarios' expected auscultation ---------------------------------------------------------

/// The S2 split mode of a scenario's beats (inspiration = phase 0.25, expiration = 0.75).
fn split_mode(s: &Tier0Scenario) -> SplitMode {
    let p = PcgParams::default();
    let rec = s.record(6.0).unwrap();
    let m = &s.scenario.auscultation;
    let (mut insp, mut exp, mut n) = (0.0, 0.0, 0.0);
    for b in BeatAssembler::assemble(&rec.events, &p)
        .iter()
        .filter(|b| b.rr_ms.is_some())
    {
        insp += schedule_with(b, &p, 0.25, m).split_ms().unwrap();
        exp += schedule_with(b, &p, 0.75, m).split_ms().unwrap();
        n += 1.0;
    }
    let (i, e) = (insp / n, exp / n);
    if i.abs() < 20.0 && e.abs() < 20.0 {
        SplitMode::Single
    } else if e < 0.0 && i.abs() < e.abs() {
        SplitMode::Paradoxical
    } else if (i - e).abs() < 20.0 {
        SplitMode::Fixed
    } else if e >= 30.0 {
        SplitMode::Wide
    } else {
        SplitMode::Physiological
    }
}

#[test]
fn every_scenario_with_auscultation_findings_produces_them() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "ron") {
            continue;
        }
        let s = Tier0Scenario::from_ron(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let Some(e) = s.scenario.expected.clone() else {
            continue;
        };
        let stem = s.scenario.id.clone();
        if s.scenario.auscultation.is_empty() {
            assert!(
                e.extra_sounds.is_none() && e.murmurs.is_none(),
                "{stem}: findings without a section"
            );
            continue;
        }
        let (_, synth) = run(&s, 10.0, None);
        let mut extra: Vec<ExtraSound> = synth
            .scheduled_sounds()
            .iter()
            .filter_map(|x| match x.kind {
                ComponentKind::S3 => Some(ExtraSound::S3),
                ComponentKind::S4 => Some(ExtraSound::S4),
                ComponentKind::AorticClick | ComponentKind::PulmonaryClick => Some(ExtraSound::EjectionClick),
                ComponentKind::MidSystolicClick => Some(ExtraSound::MidSystolicClick),
                ComponentKind::OpeningSnap => Some(ExtraSound::OpeningSnap),
                _ => None,
            })
            .collect();
        if synth
            .scheduled_murmurs()
            .iter()
            .any(|m| m.class == MurmurClass::FrictionRub)
        {
            extra.push(ExtraSound::FrictionRub);
        }
        extra.sort();
        extra.dedup();
        let mut timing: Vec<MurmurTiming> = synth
            .scheduled_murmurs()
            .iter()
            .filter_map(|m| m.class.timing())
            .collect();
        timing.sort();
        timing.dedup();
        if let Some(mut want) = e.extra_sounds.clone() {
            want.sort();
            assert_eq!(extra, want, "{stem}: extra sounds");
        }
        if let Some(mut want) = e.murmurs.clone() {
            want.sort();
            assert_eq!(timing, want, "{stem}: murmurs");
        }
        if let Some(want) = e.s2_split {
            assert_eq!(split_mode(&s), want, "{stem}: S2 split");
        }
        checked += 1;
    }
    assert_eq!(checked, NEW.len() + WITH_SECTION.len());
}

#[test]
fn the_synth_labels_the_extra_sounds_from_its_schedule() {
    for (stem, label, kind) in [
        ("heart_failure_s3", SoundLabel::S3, ComponentKind::S3),
        ("lvh_s4", SoundLabel::S4, ComponentKind::S4),
        (
            "mitral_stenosis_sinus",
            SoundLabel::OpeningSnap,
            ComponentKind::OpeningSnap,
        ),
        (
            "pulmonary_stenosis",
            SoundLabel::EjectionClick,
            ComponentKind::PulmonaryClick,
        ),
        (
            "aortic_stenosis_mild",
            SoundLabel::EjectionClick,
            ComponentKind::AorticClick,
        ),
        (
            "mitral_valve_prolapse",
            SoundLabel::Click,
            ComponentKind::MidSystolicClick,
        ),
    ] {
        let (_, mut synth) = run(&scenario(stem), 6.0, None);
        let marks = synth.take_all_marks();
        let list = sounds(&synth, kind);
        assert!(!list.is_empty(), "{stem}");
        for x in list {
            assert!(
                marks
                    .iter()
                    .any(|m| m.label == label && m.at.0.to_bits() == x.onset_ms.to_bits()),
                "{stem}: {label:?} at {}",
                x.onset_ms
            );
        }
        // murmurs are not labelled: only discrete sounds carry labels
        assert!(marks.iter().all(|m| m.label.text().len() <= 2));
    }
    // the plain schedule of a beat has none of the extra sounds
    let s = scenario("sinus_normal");
    let rec = s.record(3.0).unwrap();
    let p = PcgParams::default();
    for b in BeatAssembler::assemble(&rec.events, &p) {
        let c = schedule(&b, &p, 0.5);
        assert!(c.s3.is_none() && c.opening_snap.is_none() && c.aortic_click.is_none());
    }
}
