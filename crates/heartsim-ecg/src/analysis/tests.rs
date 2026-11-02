//! Acceptance tests of brief B04, run on `GaussianSynth` + `TimingEngine` recordings.

use super::*;
use crate::GaussianSynth;
use crate::filter::Noise;
use heartsim_core::Millis;
use heartsim_sim::timing::TimingParams;
use heartsim_sim::{Chamber, Engine, Event, EventKind, TimingEngine};

const FS: f32 = 500.0;

struct Rec {
    leads: Vec<Vec<f32>>,
    events: Vec<Event>,
}

impl Rec {
    fn slices(&self) -> [&[f32]; LEAD_COUNT] {
        std::array::from_fn(|i| self.leads[i].as_slice())
    }
}

fn record_with(params: TimingParams, synth: &mut GaussianSynth, seconds: f64) -> Rec {
    let mut eng = TimingEngine::new(params);
    let mut leads = vec![Vec::new(); LEAD_COUNT];
    let mut events = Vec::new();
    let steps = (seconds * 1000.0) as usize;
    for k in 0..steps {
        eng.step(Millis(1.0));
        let evs = eng.drain_events();
        synth.on_events(&evs);
        events.extend(evs);
        let s = synth.sample(eng.now());
        if k % 2 == 1 {
            for (l, buf) in leads.iter_mut().enumerate() {
                buf.push(s.0[l] as f32);
            }
        }
    }
    Rec { leads, events }
}

fn record(rate_bpm: f64, scale: f64, shift: f64, seconds: f64) -> Rec {
    let mut p = TimingParams::default();
    p.sinus.rate_bpm = rate_bpm;
    // Keep the escape pacemakers slower than the sinus node so 40 bpm stays a sinus rhythm.
    p.escape.junctional_bpm = 25.0;
    p.escape.ventricular_bpm = 15.0;
    let mut s = GaussianSynth::textbook();
    s.qrs_width_scale = scale;
    s.axis_shift_deg = shift;
    record_with(p, &mut s, seconds)
}

/// Width of a Gaussian wave at 5 % of its peak, in σ (the boundary convention of the analyser).
const K5: f64 = 2.447_746_830_680_817;

/// The last `seconds` of every lead.
fn tail(rec: &Rec, seconds: f32) -> Vec<Vec<f32>> {
    let n = (seconds * FS) as usize;
    rec.leads
        .iter()
        .map(|l| l[l.len().saturating_sub(n)..].to_vec())
        .collect()
}

fn analyze_tail(rec: &Rec, seconds: f32) -> Analysis {
    let t = tail(rec, seconds);
    let refs: [&[f32]; LEAD_COUNT] = std::array::from_fn(|i| t[i].as_slice());
    Analyzer::default().analyze(&refs, FS)
}

fn active(e: &Event) -> bool {
    e.kind != EventKind::Blocked
}

/// Times of the ventricular activations (fusion duplicates removed) and of the atrial ones.
fn vent_times(rec: &Rec) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for e in rec
        .events
        .iter()
        .filter(|e| e.chamber == Chamber::Ventricle && active(e))
    {
        if out.last().is_none_or(|l| e.at.0 - l > 150.0) {
            out.push(e.at.0);
        }
    }
    out
}

fn atrial_times(rec: &Rec) -> Vec<f64> {
    rec.events
        .iter()
        .filter(|e| e.chamber == Chamber::Atrium && active(e))
        .map(|e| e.at.0)
        .collect()
}

/// Median engine PR (atrial activation → ventricular activation) over the recording, ms.
fn engine_pr(rec: &Rec) -> f64 {
    let a = atrial_times(rec);
    let mut prs: Vec<f64> = vent_times(rec)
        .iter()
        .filter_map(|v| a.iter().rev().find(|t| **t < *v).map(|t| v - t))
        .collect();
    median_f64(&mut prs).unwrap()
}

/// Generator QRS duration and QT (Q onset → T offset) at the analyser's 5 % convention, ms.
fn generator_qrs_qt(scale: f64) -> (f64, f64) {
    let q_on = 8.0 * scale - K5 * 7.0 * scale;
    let s_end = 48.0 * scale + K5 * 9.0 * scale;
    let t_off = 300.0 + K5 * 55.0;
    (s_end - q_on, t_off - q_on)
}

#[test]
fn heart_rate_within_1_bpm_from_40_to_180() {
    for rate in [40.0, 50.0, 60.0, 75.0, 90.0, 110.0, 140.0, 160.0, 180.0] {
        let rec = record(rate, 1.0, 0.0, 14.0);
        let a = analyze_tail(&rec, 10.0);
        let hr = a
            .hr_bpm
            .unwrap_or_else(|| panic!("no HR at {rate} bpm: {:?}", a.quality));
        assert!((hr - rate).abs() <= 1.0, "rate {rate}: measured {hr}");
        assert_eq!(a.regularity, Regularity::Regular, "rate {rate}");
    }
}

#[test]
fn pr_within_10_ms_of_engine_pr() {
    for rate in [50.0, 60.0, 75.0, 90.0] {
        let rec = record(rate, 1.0, 0.0, 14.0);
        let a = analyze_tail(&rec, 10.0);
        let pr = a.pr_ms.expect("PR");
        let want = engine_pr(&rec);
        assert!(a.p_wave_present);
        assert!((pr - want).abs() <= 10.0, "rate {rate}: PR {pr} vs engine {want}");
    }
}

#[test]
fn qrs_duration_matches_morphology() {
    let a = analyze_tail(&record(75.0, 1.0, 0.0, 14.0), 10.0);
    let q = a.qrs_ms.unwrap();
    assert!((80.0..=100.0).contains(&q), "default QRS {q} ms");
    let a = analyze_tail(&record(75.0, 1.6, 0.0, 14.0), 10.0);
    let q = a.qrs_ms.unwrap();
    assert!((130.0..=170.0).contains(&q), "wide QRS {q} ms");
    // And it tracks the generator closely at the analyser's own boundary convention.
    let want = generator_qrs_qt(1.6).0;
    assert!((q - want).abs() < 25.0, "wide QRS {q} vs generator {want}");
}

#[test]
fn qt_within_20_ms_of_generator_and_qtc_formulas() {
    for rate in [50.0, 60.0, 75.0, 90.0] {
        let a = analyze_tail(&record(rate, 1.0, 0.0, 14.0), 10.0);
        let qt = a.qt_ms.expect("QT");
        let want = generator_qrs_qt(1.0).1;
        assert!(
            (qt - want).abs() <= 20.0,
            "rate {rate}: QT {qt} vs generator {want}"
        );
        let rr_s = 60.0 / rate;
        let bz = a.qtc_bazett_ms.unwrap();
        let fr = a.qtc_fridericia_ms.unwrap();
        assert!((bz - qt / rr_s.sqrt()).abs() < 2.0, "Bazett {bz}");
        assert!((fr - qt / rr_s.cbrt()).abs() < 2.0, "Fridericia {fr}");
    }
}

#[test]
fn qrs_axis_within_5_degrees_of_60_plus_shift() {
    for shift in [-90.0, -60.0, -30.0, 0.0, 30.0, 60.0, 90.0, 120.0] {
        let a = analyze_tail(&record(75.0, 1.0, shift, 14.0), 10.0);
        let got = a.qrs_axis.expect("axis").degrees;
        let err = FrontalAxis::normalise(got - (60.0 + shift)).abs();
        assert!(err <= 5.0, "shift {shift}: axis {got}");
    }
    let a = analyze_tail(&record(75.0, 1.0, 0.0, 14.0), 10.0);
    assert_eq!(a.qrs_axis_class(), Some(AxisClass::Normal));
    let a = analyze_tail(&record(75.0, 1.0, -100.0, 14.0), 10.0);
    assert_eq!(a.qrs_axis_class(), Some(AxisClass::LeftDeviation));
    let a = analyze_tail(&record(75.0, 1.0, 60.0, 14.0), 10.0);
    assert_eq!(a.qrs_axis_class(), Some(AxisClass::RightDeviation));
}

#[test]
fn t_axis_is_concordant_with_the_generator() {
    let a = analyze_tail(&record(75.0, 1.0, 0.0, 14.0), 10.0);
    let t = a.t_axis.expect("T axis").degrees;
    assert!((t - 45.0).abs() <= 8.0, "T axis {t}");
}

/// Ventricular beats at irregular intervals and no atrial activity (AF-like).
fn irregular_rec(seed: u64) -> Rec {
    use heartsim_core::Region;
    use heartsim_sim::Origin;
    let mut state = seed;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut beats = Vec::new();
    let mut t = 500.0;
    while t < 13_000.0 {
        beats.push(t);
        t += 380.0 + 700.0 * next();
    }
    let mut synth = GaussianSynth::textbook();
    let mut leads = vec![Vec::new(); LEAD_COUNT];
    let mut idx = 0;
    for k in 0..14_000 {
        let now = f64::from(k);
        while idx < beats.len() && beats[idx] <= now {
            synth.on_events(&[Event::new(
                Millis(beats[idx]),
                Region::LeftVentricle,
                EventKind::Conducted,
                Origin::Sinus,
            )]);
            idx += 1;
        }
        let s = synth.sample(Millis(now));
        if k % 2 == 1 {
            for (l, buf) in leads.iter_mut().enumerate() {
                buf.push(s.0[l] as f32);
            }
        }
    }
    Rec {
        leads,
        events: Vec::new(),
    }
}

#[test]
fn af_like_irregular_rr_is_flagged_and_has_no_p_waves() {
    for seed in [1, 7, 99] {
        let a = analyze_tail(&irregular_rec(seed), 10.0);
        assert_eq!(a.regularity, Regularity::Irregular, "seed {seed}");
        assert!(!a.p_wave_present, "seed {seed}: P found without atrial activity");
        assert!(a.pr_ms.is_none());
        assert!(a.hr_bpm.is_some());
    }
}

fn add_noise(rec: &mut Rec, amplitude: f32) {
    for (l, lead) in rec.leads.iter_mut().enumerate() {
        let mut n = Noise::new(1000 + l as u64, amplitude);
        for v in lead.iter_mut() {
            *v += n.sample();
        }
    }
}

#[test]
fn half_a_hundredth_millivolt_noise_changes_nothing_material() {
    for (rate, scale) in [(60.0, 1.0), (75.0, 1.0), (90.0, 1.0), (75.0, 1.6)] {
        let clean = analyze_tail(&record(rate, scale, 0.0, 14.0), 10.0);
        let mut rec = record(rate, scale, 0.0, 14.0);
        add_noise(&mut rec, 0.05);
        let noisy = analyze_tail(&rec, 10.0);
        let d = |a: Option<f64>, b: Option<f64>| (a.unwrap() - b.unwrap()).abs();
        let tag = format!("{rate} bpm x{scale}");
        assert!(d(clean.hr_bpm, noisy.hr_bpm) <= 1.0, "{tag}: HR");
        assert!(
            d(clean.qrs_ms, noisy.qrs_ms) <= 15.0,
            "{tag}: QRS {:?} vs {:?}",
            clean.qrs_ms,
            noisy.qrs_ms
        );
        assert!(
            d(clean.pr_ms, noisy.pr_ms) <= 15.0,
            "{tag}: PR {:?} vs {:?}",
            clean.pr_ms,
            noisy.pr_ms
        );
        assert!(
            d(clean.qt_ms, noisy.qt_ms) <= 25.0,
            "{tag}: QT {:?} vs {:?}",
            clean.qt_ms,
            noisy.qt_ms
        );
        let ax = FrontalAxis::normalise(clean.qrs_axis.unwrap().degrees - noisy.qrs_axis.unwrap().degrees);
        assert!(ax.abs() <= 5.0, "{tag}: axis");
        assert_eq!(noisy.regularity, Regularity::Regular, "{tag}");
    }
}

#[test]
fn noise_alone_yields_no_heart_rate() {
    for seed in 1..6 {
        let mut leads = vec![Vec::new(); LEAD_COUNT];
        for (l, lead) in leads.iter_mut().enumerate() {
            let mut n = Noise::new(seed * 100 + l as u64, 0.05);
            lead.extend((0..5000).map(|_| n.sample()));
        }
        let rec = Rec {
            leads,
            events: Vec::new(),
        };
        let a = Analyzer::default().analyze(&rec.slices(), FS);
        assert!(a.hr_bpm.is_none(), "seed {seed}: HR {:?} from noise", a.hr_bpm);
        assert_eq!(a.beats.len(), 0);
    }
    let flat = Rec {
        leads: vec![vec![0.0; 5000]; LEAD_COUNT],
        events: Vec::new(),
    };
    let a = Analyzer::default().analyze(&flat.slices(), FS);
    assert_eq!(a.quality, SignalQuality::NoSignal);
    assert!(a.hr_bpm.is_none());
}

#[test]
fn pacing_spikes_are_ignored() {
    let clean = analyze_tail(&record(75.0, 1.0, 0.0, 14.0), 10.0);
    let mut rec = record(75.0, 1.0, 0.0, 14.0);
    let v = vent_times(&rec);
    for t in v {
        // A bipolar 2 ms stimulus artefact 40 ms before every ventricular activation, on all leads.
        let i = ((t - 40.0) / 2.0) as usize;
        for lead in &mut rec.leads {
            if i + 1 < lead.len() {
                lead[i] += 3.0;
                lead[i + 1] -= 2.0;
            }
        }
    }
    let spiked = analyze_tail(&rec, 10.0);
    let d = |a: Option<f64>, b: Option<f64>| (a.unwrap() - b.unwrap()).abs();
    assert!(d(clean.hr_bpm, spiked.hr_bpm) <= 1.0);
    assert!(
        d(clean.qrs_ms, spiked.qrs_ms) <= 6.0,
        "{:?} vs {:?}",
        clean.qrs_ms,
        spiked.qrs_ms
    );
    assert!(d(clean.qt_ms, spiked.qt_ms) <= 6.0);
}

#[test]
fn heart_vector_loop_follows_the_dipole() {
    let rec = record(75.0, 1.0, 0.0, 14.0);
    let a = analyze_tail(&rec, 10.0);
    assert!(
        a.heart_vector.len() > 200,
        "loop has {} samples",
        a.heart_vector.len()
    );
    // Largest vector = R wave: points along the 60° frontal axis, anteriorly.
    let big = a
        .heart_vector
        .iter()
        .copied()
        .max_by(|x, y| x.length().total_cmp(&y.length()))
        .unwrap();
    let ang = FrontalAxis::from_i_avf(big.x, -big.y).degrees;
    assert!((ang - 60.0).abs() < 8.0, "peak vector axis {ang}");
    assert!(big.length() > 0.8, "{big}");
    assert!(big.z > 0.0);
}

#[test]
fn wave_marks_are_ordered_and_labelled() {
    let a = analyze_tail(&record(75.0, 1.0, 0.0, 14.0), 10.0);
    assert!(a.marks.iter().all(|m| m.lead == Lead::II));
    let beat = &a.beats[3];
    let in_beat: Vec<&WaveMark> = a
        .marks
        .iter()
        .filter(|m| m.onset >= beat.p.unwrap().onset && m.offset <= beat.t.unwrap().offset)
        .collect();
    let kinds: Vec<WaveKind> = in_beat.iter().map(|m| m.kind).collect();
    // The Q wave of the textbook morphology is buried under the rising R in lead II, so it is optional.
    let kinds: Vec<WaveKind> = kinds.into_iter().filter(|k| *k != WaveKind::Q).collect();
    assert_eq!(kinds, vec![WaveKind::P, WaveKind::R, WaveKind::S, WaveKind::T]);
    for m in &a.marks {
        assert!(m.onset <= m.peak && m.peak <= m.offset, "{m:?}");
    }
    for w in in_beat.windows(2) {
        assert!(w[0].peak < w[1].peak);
    }
}

#[test]
fn rings_entry_point_matches_slices() {
    let rec = record(75.0, 1.0, 0.0, 14.0);
    let mut rings: [LeadRing; LEAD_COUNT] = std::array::from_fn(|_| LeadRing::new(5000, FS));
    for (r, lead) in rings.iter_mut().zip(&rec.leads) {
        for v in lead {
            r.push(*v);
        }
    }
    let a = Analyzer::default().analyze_rings(&rings);
    let b = analyze_tail(&rec, 10.0);
    assert_eq!(a.hr_bpm, b.hr_bpm);
    assert_eq!(a.qt_ms, b.qt_ms);
}

#[test]
fn analysis_is_deterministic() {
    let rec = record(75.0, 1.0, 0.0, 14.0);
    let a = analyze_tail(&rec, 10.0);
    let b = analyze_tail(&rec, 10.0);
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}

#[test]
fn ten_seconds_of_twelve_leads_analyse_in_under_5_ms() {
    let rec = record(75.0, 1.0, 0.0, 14.0);
    let t = tail(&rec, 10.0);
    let refs: [&[f32]; LEAD_COUNT] = std::array::from_fn(|i| t[i].as_slice());
    let an = Analyzer::default();
    let mut best = f64::MAX;
    for _ in 0..20 {
        let t0 = std::time::Instant::now();
        let a = an.analyze(&refs, FS);
        best = best.min(t0.elapsed().as_secs_f64() * 1e3);
        assert!(a.hr_bpm.is_some());
    }
    // The 5 ms budget is for optimised builds; unoptimised test builds get 10x headroom.
    let budget = if cfg!(debug_assertions) { 50.0 } else { 5.0 };
    println!("analysis of 10 s x 12 leads: {best:.2} ms (best of 20)");
    assert!(best < budget, "{best:.2} ms");
}
