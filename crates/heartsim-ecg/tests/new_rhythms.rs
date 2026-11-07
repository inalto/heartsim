//! The wave types of the sustained arrhythmias and of infarction, measured on the 12-lead strips
//! of `assets/scenarios/`: flutter rate and AV ratio, absent P waves and irregular RR in AF, VF
//! without identifiable QRS and with a dominant frequency in the reported range, the torsades
//! twist and QT, the Q-wave criteria and lead territories of each infarct, T inversion timing.
//! Criteria and sources: `docs/research/tier0-rhythm-criteria.md`.

use heartsim_core::scenario::{Action, RhythmKind, TimedAction};
use heartsim_core::{LEAD_COUNT, Lead, Millis};
use heartsim_ecg::analysis::SignalQuality;
use heartsim_ecg::scenario::{ANALYSIS_RATE_HZ, VALIDATION_RUN_S, VALIDATION_WINDOW_S};
use heartsim_ecg::{Analysis, GaussianSynth, Regularity, Tier0Scenario};
use heartsim_sim::{Chamber, Engine, TimingEngine};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios")
}

fn scenario(id: &str) -> Tier0Scenario {
    let path = dir().join(format!("{id}.ron"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Tier0Scenario::from_ron(&text).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Analysis of the last 10 s of a 20 s run, the same way the scenario validator measures.
fn analysed(id: &str) -> Analysis {
    scenario(id)
        .record(VALIDATION_RUN_S)
        .unwrap()
        .analyze_tail(VALIDATION_WINDOW_S)
}

fn lead_tail(id: &str, lead: Lead) -> Vec<f64> {
    scenario(id)
        .record(VALIDATION_RUN_S)
        .unwrap()
        .tail(lead as usize, VALIDATION_WINDOW_S, ANALYSIS_RATE_HZ)
        .iter()
        .map(|v| f64::from(*v))
        .collect()
}

/// A scenario drawn with only its atrial events, so that the atrial waves can be measured with no
/// QRS or T wave on top: `leads[l][i]` at 1 kHz.
fn atrial_only(id: &str, seconds: f64) -> Vec<Vec<f64>> {
    let s = scenario(id);
    let mut eng = TimingEngine::new(s.timing.clone());
    eng.set_shock_seed(s.scenario.seed);
    let mut synth = GaussianSynth::textbook();
    let mut out = vec![Vec::new(); LEAD_COUNT];
    for _ in 0..(seconds * 1000.0) as usize {
        eng.step(Millis(1.0));
        let evs: Vec<_> = eng
            .drain_events()
            .into_iter()
            .filter(|e| e.chamber == Chamber::Atrium)
            .collect();
        synth.on_events(&evs);
        let x = synth.sample(eng.now());
        for (o, l) in out.iter_mut().zip(Lead::ALL) {
            o.push(x.get(l));
        }
    }
    out
}

/// Frequency of the largest spectral peak in `band`, Hz, for `x` sampled at `fs` Hz.
fn dominant_hz(x: &[f64], fs: f64, band: (f64, f64)) -> f64 {
    let mean = x.iter().sum::<f64>() / x.len() as f64;
    let (mut best, mut f) = ((0.0, -1.0), band.0);
    while f <= band.1 {
        let w = std::f64::consts::TAU * f / fs;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let p = w * i as f64;
            re += (v - mean) * p.cos();
            im += (v - mean) * p.sin();
        }
        let power = re * re + im * im;
        if power > best.1 {
            best = (f, power);
        }
        f += 0.05;
    }
    best.0
}

fn peak_to_peak(x: &[f64]) -> f64 {
    x.iter().copied().fold(f64::MIN, f64::max) - x.iter().copied().fold(f64::MAX, f64::min)
}

// --- atrial flutter ----------------------------------------------------------------------------

#[test]
fn flutter_waves_are_a_300_per_minute_sawtooth_negative_inferiorly_and_positive_in_v1() {
    let leads = atrial_only("flutter_4to1", 8.0);
    let (ii, v1) = (
        &leads[Lead::II as usize][3000..],
        &leads[Lead::V1 as usize][3000..],
    );
    let f = dominant_hz(ii, 1000.0, (2.0, 10.0));
    assert!((f - 5.0).abs() < 0.2, "flutter frequency {f} Hz");
    assert!(
        (0.15..=0.5).contains(&peak_to_peak(ii)),
        "II {}",
        peak_to_peak(ii)
    );
    // No isoelectric baseline: the signal is hardly ever flat.
    let flat = ii.windows(2).filter(|w| (w[1] - w[0]).abs() < 2e-4).count();
    assert!(flat * 10 < ii.len(), "{flat} of {} samples flat", ii.len());
    // Sawtooth: a slow descent then a faster return in II.
    let steepest_up = ii.windows(2).map(|w| w[1] - w[0]).fold(f64::MIN, f64::max);
    let steepest_down = ii.windows(2).map(|w| w[0] - w[1]).fold(f64::MIN, f64::max);
    assert!(
        steepest_up > 1.3 * steepest_down,
        "up {steepest_up}, down {steepest_down}"
    );
    // Polarity: the deepest deflection of II is below its mean, of V1 above it.
    let skew = |x: &[f64]| {
        let m = x.iter().sum::<f64>() / x.len() as f64;
        x.iter().map(|v| (v - m).powi(3)).sum::<f64>()
    };
    assert!(skew(ii) * skew(v1) < 0.0, "II and V1 are mirror images");
    // Level baseline: a train of waves has no net offset.
    let mean = ii.iter().sum::<f64>() / ii.len() as f64;
    assert!(mean.abs() < 0.02, "baseline offset {mean} mV");
}

#[test]
fn the_analyser_names_flutter_with_its_rate_and_ratio() {
    for (id, ratio, hr) in [("flutter_2to1", 2, 150.0), ("flutter_4to1", 4, 75.0)] {
        let a = analysed(id);
        assert_eq!(a.rhythm, Some(RhythmKind::AtrialFlutter), "{id}");
        assert_eq!(a.av_ratio, Some(ratio), "{id}");
        assert!(
            (a.atrial_rate_bpm.unwrap() - 300.0).abs() < 25.0,
            "{id}: {:?}",
            a.atrial_rate_bpm
        );
        assert!((a.hr_bpm.unwrap() - hr).abs() < 6.0, "{id}: {:?}", a.hr_bpm);
        assert!(!a.p_wave_present, "{id}: flutter waves are not P waves");
        assert_eq!(a.pr_ms, None, "{id}");
    }
    let a = analysed("flutter_variable");
    assert_eq!(a.rhythm, Some(RhythmKind::AtrialFlutter));
    assert_eq!(a.av_ratio, None, "the ratio varies");
    assert_eq!(a.regularity, Regularity::Irregular);
}

// --- atrial fibrillation -----------------------------------------------------------------------

#[test]
fn af_has_no_p_waves_an_irregular_rr_and_small_varying_f_waves() {
    let a = analysed("atrial_fibrillation");
    assert_eq!(a.rhythm, Some(RhythmKind::AtrialFibrillation));
    assert_eq!(a.regularity, Regularity::Irregular);
    assert!(!a.p_wave_present);
    assert_eq!(a.pr_ms, None);
    assert!(a.qrs_ms.unwrap() < 120.0, "narrow: {:?}", a.qrs_ms);
    assert!(a.beats.iter().all(|b| b.p.is_none()));

    let leads = atrial_only("atrial_fibrillation", 12.0);
    let v1 = &leads[Lead::V1 as usize][2000..];
    let f = dominant_hz(v1, 1000.0, (2.0, 12.0));
    assert!((3.0..=10.0).contains(&f), "dominant atrial frequency {f} Hz");
    // coarse f waves: ≥ 0.1 mV, none the size of a QRS
    let pp = peak_to_peak(v1);
    assert!((0.1..=0.6).contains(&pp), "f-wave size {pp} mV");
    // size varies from one second to the next, unlike flutter
    let sizes: Vec<f64> = v1.chunks(1000).map(peak_to_peak).collect();
    assert!(peak_to_peak(&sizes) > 0.02, "{sizes:?}");
}

#[test]
fn pre_excited_af_is_fast_broad_and_irregular() {
    let a = analysed("af_preexcited");
    assert_eq!(a.rhythm, Some(RhythmKind::PreExcitedAtrialFibrillation));
    assert_eq!(a.regularity, Regularity::Irregular);
    assert!(a.hr_bpm.unwrap() >= 150.0, "{:?}", a.hr_bpm);
    // the analyser's limit: 120 ms less the 10 or so lost over a restless baseline
    assert!(a.qrs_ms.unwrap() >= 110.0, "{:?}", a.qrs_ms);
    assert!(!a.p_wave_present);
    // the danger sign the notes teach: a shortest pre-excited R-R of 250 ms or less
    let shortest_rr_ms = a
        .beats
        .iter()
        .filter_map(|b| b.rr_before_ms)
        .fold(f64::INFINITY, f64::min);
    assert!(shortest_rr_ms <= 250.0, "shortest R-R {shortest_rr_ms} ms");
}

// --- ventricular fibrillation ------------------------------------------------------------------

#[test]
fn vf_has_no_identifiable_qrs_and_its_dominant_frequency_is_in_the_reported_range() {
    for id in ["vf_coarse", "vf_fine"] {
        let a = analysed(id);
        assert_eq!(a.rhythm, Some(RhythmKind::VentricularFibrillation), "{id}");
        assert_eq!(a.quality, SignalQuality::NoQrs, "{id}");
        assert!(a.beats.is_empty() && a.marks.is_empty(), "{id}");
        assert_eq!(a.hr_bpm, None, "{id}: no rate is reported");
        let f = a.dominant_frequency_hz.unwrap();
        // recorded human VF: about 4–7 Hz (see the criteria file)
        assert!((3.5..=8.0).contains(&f), "{id}: {f} Hz");
    }
}

#[test]
fn coarse_and_fine_vf_differ_by_amplitude_and_vary_in_size() {
    // Peak to peak over one-second stretches: the 0.2 mV line of Weaver et al. 1985.
    let median_pp = |id: &str| {
        let x = lead_tail(id, Lead::II);
        let mut p: Vec<f64> = x.chunks(500).map(peak_to_peak).collect();
        p.sort_by(f64::total_cmp);
        (p[p.len() / 2], peak_to_peak(&p))
    };
    let (coarse, spread) = median_pp("vf_coarse");
    assert!(coarse > 0.2, "coarse VF {coarse} mV");
    assert!(spread > 0.1, "varying amplitude: {spread}");
    let (fine, _) = median_pp("vf_fine");
    assert!(fine <= 0.2, "fine VF {fine} mV");
}

// --- torsades de pointes and polymorphic VT -----------------------------------------------------

/// Peak signed deflection in lead II per beat, from the engine's own beat times.
fn beat_polarity(id: &str) -> Vec<f64> {
    let s = scenario(id);
    let rec = s.record(VALIDATION_RUN_S).unwrap();
    let ii = &rec.leads[Lead::II as usize];
    rec.events
        .iter()
        .filter(|e| {
            matches!(e.origin, heartsim_sim::Origin::Circuit(c) if c.is_ventricular()) && e.at.0 >= 8000.0
        })
        .map(|e| {
            let t = e.at.0 as usize;
            // the first lobe of the beat
            let w = &ii[(t + 60).min(ii.len() - 1)..(t + 150).min(ii.len() - 1)];
            w.iter()
                .copied()
                .max_by(|a, b| a.abs().total_cmp(&b.abs()))
                .map_or(0.0, f64::from)
        })
        .collect()
}

/// Lengths of the runs of equal polarity, leaving out the unfinished first and last run.
fn polarity_runs(x: &[f64]) -> Vec<usize> {
    let mut runs = Vec::new();
    let mut n = 0;
    for w in x.windows(2) {
        n += 1;
        if w[0].signum() != w[1].signum() {
            runs.push(n);
            n = 0;
        }
    }
    if runs.len() > 1 {
        runs[1..].to_vec()
    } else {
        Vec::new()
    }
}

#[test]
fn torsades_twists_with_polarity_runs_in_the_reported_range() {
    let p = beat_polarity("torsades");
    let runs = polarity_runs(&p);
    assert!(!runs.is_empty(), "{p:?}");
    // the QRS changes polarity every 5–20 beats
    assert!(
        runs.iter().all(|r| (5..=20).contains(r)),
        "polarity runs {runs:?}"
    );
    // amplitude waxes and wanes inside a run
    let biggest = p.iter().copied().map(f64::abs).fold(0.0, f64::max);
    let smallest = p.iter().copied().map(f64::abs).fold(f64::MAX, f64::min);
    assert!(smallest < 0.5 * biggest, "spindles: {smallest} to {biggest} mV");

    let a = analysed("torsades");
    assert_eq!(a.rhythm, Some(RhythmKind::TorsadesDePointes));
    let hr = a.hr_bpm.unwrap();
    assert!((160.0..=240.0).contains(&hr), "rate {hr}");
}

#[test]
fn torsades_has_a_long_qt_before_it_starts() {
    let mut s = scenario("torsades");
    // the same heart without its premature beats: QTc > 500 ms (Drew et al. 2010)
    s.timing.foci.clear();
    s.timing.ventricular_driver = None;
    let a = s
        .record(VALIDATION_RUN_S)
        .unwrap()
        .analyze_tail(VALIDATION_WINDOW_S);
    let qtc = a.qtc_bazett_ms.unwrap();
    assert!(qtc > 500.0, "QTc {qtc}");
}

#[test]
fn polymorphic_vt_changes_from_beat_to_beat_without_a_twist() {
    let a = analysed("polymorphic_vt");
    assert_eq!(a.rhythm, Some(RhythmKind::PolymorphicVentricularTachycardia));
    let hr = a.hr_bpm.unwrap();
    assert!((170.0..=260.0).contains(&hr), "rate {hr}");
    let p = beat_polarity("polymorphic_vt");
    // polarity changes often, not in long orderly runs
    let flips = p.windows(2).filter(|w| w[0].signum() != w[1].signum()).count();
    assert!(
        flips * 4 > p.len(),
        "{flips} polarity changes in {} beats",
        p.len()
    );
}

// --- infarction --------------------------------------------------------------------------------

/// A lesion applied from the start on a normal heart at 75 bpm.
fn with_lesion(branch: &str, severity: f64) -> Tier0Scenario {
    let mut s = scenario("sinus_normal");
    s.scenario.timeline = vec![TimedAction {
        at: Millis(0.0),
        action: Action::Stenosis {
            branch: branch.into(),
            severity,
        },
    }];
    s
}

/// One measured beat per lead, at 500 Hz.
struct Measured {
    leads: Vec<Vec<f32>>,
    a: Analysis,
}

impl Measured {
    fn of(s: &Tier0Scenario) -> Self {
        Self::window(s, VALIDATION_RUN_S, VALIDATION_WINDOW_S)
    }

    fn window(s: &Tier0Scenario, run_s: f64, window_s: f64) -> Self {
        let rec = s.record(run_s).unwrap();
        let a = rec.analyze_tail(window_s);
        let leads = (0..LEAD_COUNT)
            .map(|l| rec.tail(l, window_s, ANALYSIS_RATE_HZ))
            .collect();
        Self { leads, a }
    }

    /// Duration (ms) and depth (mV) of the initial negative deflection of the QRS, 0 when the QRS
    /// starts upward. Median over the beats.
    fn q_wave(&self, lead: Lead) -> (f64, f64) {
        let x = &self.leads[lead as usize];
        let mut dur = Vec::new();
        let mut depth = Vec::new();
        for b in &self.a.beats {
            let base = f64::from(x[b.qrs_onset.saturating_sub(10)]);
            let v = |i: usize| f64::from(x[i]) - base;
            let end = (b.qrs_onset + 100).min(x.len() - 1);
            let start = (b.qrs_onset..end).find(|&i| v(i).abs() > 0.02);
            let Some(start) = start else {
                dur.push(0.0);
                depth.push(0.0);
                continue;
            };
            if v(start) > 0.0 {
                dur.push(0.0);
                depth.push(0.0);
                continue;
            }
            let stop = (start..end).find(|&i| v(i) >= 0.0).unwrap_or(end);
            dur.push((stop - start) as f64 * 2.0);
            depth.push(-(start..stop).map(v).fold(0.0, f64::min));
        }
        let med = |v: &mut Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        (med(&mut dur), med(&mut depth))
    }

    /// The largest T deviation from the PR baseline (signed, mV) and when it comes after the QRS
    /// onset (ms). Median over the beats.
    fn t_wave(&self, lead: Lead) -> (f64, f64) {
        let x = &self.leads[lead as usize];
        let (mut amp, mut when) = (Vec::new(), Vec::new());
        for b in &self.a.beats {
            let base = f64::from(x[b.qrs_onset.saturating_sub(10)]);
            let from = b.qrs_onset + 90; // 180 ms: past the QRS
            let to = (b.qrs_onset + 230).min(x.len() - 1); // 460 ms
            if to <= from {
                continue;
            }
            let i = (from..to)
                .max_by(|&p, &q| {
                    (f64::from(x[p]) - base)
                        .abs()
                        .total_cmp(&(f64::from(x[q]) - base).abs())
                })
                .unwrap();
            amp.push(f64::from(x[i]) - base);
            when.push((i - b.qrs_onset) as f64 * 2.0);
        }
        amp.sort_by(f64::total_cmp);
        when.sort_by(f64::total_cmp);
        (amp[amp.len() / 2], when[when.len() / 2])
    }

    fn assert_pathological_q(&self, id: &str, leads: &[Lead]) {
        for &l in leads {
            let (ms, mv) = self.q_wave(l);
            // Thygesen et al. 2018: V2–V3 ≥ 20 ms or QS; elsewhere ≥ 30 ms and ≥ 0.1 mV
            let v2_v3 = matches!(l, Lead::V2 | Lead::V3);
            let (min_ms, min_mv) = if v2_v3 { (20.0, 0.0) } else { (30.0, 0.1) };
            assert!(
                ms >= min_ms && mv >= min_mv,
                "{id}: {} Q {ms} ms, {mv:.3} mV",
                l.label()
            );
        }
    }

    fn assert_no_pathological_q(&self, id: &str, leads: &[Lead]) {
        for &l in leads {
            let (ms, mv) = self.q_wave(l);
            assert!(ms < 30.0 || mv < 0.1, "{id}: {} Q {ms} ms, {mv:.3} mV", l.label());
        }
    }

    fn assert_t_inverted(&self, id: &str, leads: &[Lead]) {
        for &l in leads {
            let (mv, at) = self.t_wave(l);
            assert!(mv < -0.1, "{id}: {} T {mv:.3} mV", l.label());
            // at the T wave's time, not in the ST segment
            assert!((220.0..=420.0).contains(&at), "{id}: {} T at {at} ms", l.label());
        }
    }

    fn assert_t_upright(&self, id: &str, leads: &[Lead]) {
        for &l in leads {
            let (mv, _) = self.t_wave(l);
            assert!(mv > 0.0, "{id}: {} T {mv:.3} mV", l.label());
        }
    }
}

#[test]
fn a_healthy_heart_has_no_pathological_q_waves() {
    let m = Measured::of(&scenario("sinus_normal"));
    // The criterion leaves out aVR and V1 (a normal QS) and III (a normal Q on its own).
    let leads = [
        Lead::I,
        Lead::II,
        Lead::AVL,
        Lead::AVF,
        Lead::V2,
        Lead::V3,
        Lead::V4,
        Lead::V5,
        Lead::V6,
    ];
    m.assert_no_pathological_q("sinus_normal", &leads);
}

#[test]
fn anterior_infarct_q_waves_and_t_inversion_lie_in_v1_to_v4() {
    let m = Measured::of(&scenario("mi_anterior_evolved"));
    let id = "mi_anterior_evolved";
    m.assert_pathological_q(id, &[Lead::V1, Lead::V2, Lead::V3, Lead::V4, Lead::AVL]);
    m.assert_no_pathological_q(id, &[Lead::II, Lead::III, Lead::AVF]);
    m.assert_t_inverted(id, &[Lead::V1, Lead::V2, Lead::V3, Lead::V4]);
    m.assert_t_upright(id, &[Lead::II, Lead::III, Lead::AVF]);
}

#[test]
fn inferior_infarct_q_waves_and_t_inversion_lie_in_ii_iii_avf() {
    let m = Measured::of(&scenario("mi_inferior_evolved"));
    let id = "mi_inferior_evolved";
    m.assert_pathological_q(id, &[Lead::II, Lead::III, Lead::AVF]);
    m.assert_no_pathological_q(
        id,
        &[
            Lead::I,
            Lead::AVL,
            Lead::V2,
            Lead::V3,
            Lead::V4,
            Lead::V5,
            Lead::V6,
        ],
    );
    m.assert_t_inverted(id, &[Lead::III, Lead::AVF]);
    m.assert_t_upright(id, &[Lead::I, Lead::AVL]);
}

#[test]
fn lateral_infarct_q_waves_and_t_inversion_lie_in_i_avl_v5_v6() {
    let m = Measured::of(&scenario("mi_lateral_evolved"));
    let id = "mi_lateral_evolved";
    m.assert_pathological_q(id, &[Lead::I, Lead::AVL, Lead::V5, Lead::V6]);
    m.assert_no_pathological_q(
        id,
        &[Lead::II, Lead::III, Lead::AVF, Lead::V1, Lead::V2, Lead::V3],
    );
    m.assert_t_inverted(id, &[Lead::I, Lead::AVL, Lead::V6]);
    m.assert_t_upright(id, &[Lead::III, Lead::V2]);
}

#[test]
fn posterior_infarct_gives_tall_r_and_upright_t_in_v1_to_v3() {
    let tall = |s: &Tier0Scenario, lead: Lead| {
        let m = Measured::of(s);
        let x = &m.leads[lead as usize];
        let b = m.a.beats[m.a.beats.len() / 2];
        x[b.qrs_onset..b.qrs_offset]
            .iter()
            .copied()
            .fold(f32::MIN, f32::max)
    };
    let normal = scenario("sinus_normal");
    let posterior = with_lesion("PL infarct", 1.0);
    for l in [Lead::V1, Lead::V2] {
        assert!(tall(&posterior, l) > tall(&normal, l) + 0.2, "{}", l.label());
    }
    Measured::of(&with_lesion("PL evolving", 1.0)).assert_t_upright("PL", &[Lead::V1, Lead::V2, Lead::V3]);
}

#[test]
fn a_non_transmural_lesion_draws_no_pathological_q_wave() {
    let m = Measured::of(&with_lesion("LAD evolving", 0.45));
    m.assert_no_pathological_q("partial", &[Lead::V1, Lead::V2, Lead::V3, Lead::V4]);
}

#[test]
fn an_old_infarct_keeps_its_q_waves_with_upright_t_waves() {
    let m = Measured::of(&scenario("mi_old_anterior"));
    m.assert_pathological_q("mi_old_anterior", &[Lead::V1, Lead::V2, Lead::V3, Lead::V4]);
    m.assert_t_upright("mi_old_anterior", &[Lead::V3, Lead::V4, Lead::V5]);
}

#[test]
fn stages_come_in_order_st_then_q_then_t_inversion_then_q_alone() {
    let s = scenario("mi_evolution_stages");
    // windows inside each stage of the timeline
    let stage = |to_s: f64, len_s: f64| Measured::window(&s, to_s, len_s);
    let st_at = |m: &Measured, lead: Lead| {
        let x = &m.leads[lead as usize];
        let mut v: Vec<f64> =
            m.a.beats
                .iter()
                .filter(|b| b.qrs_offset + 30 < x.len())
                .map(|b| f64::from(x[b.qrs_offset + 30]) - f64::from(x[b.qrs_onset.saturating_sub(10)]))
                .collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let acute = stage(4.9, 3.5);
    assert!(st_at(&acute, Lead::V2) > 0.1, "ST {}", st_at(&acute, Lead::V2));
    assert!(
        acute.q_wave(Lead::V4).0 < 30.0,
        "no Q yet: {:?}",
        acute.q_wave(Lead::V4)
    );
    assert!(acute.t_wave(Lead::V5).0 > -0.1);

    let q_stage = stage(9.9, 3.5);
    q_stage.assert_pathological_q("Q stage", &[Lead::V2, Lead::V3, Lead::V4]);
    assert!(st_at(&q_stage, Lead::V4) > 0.05, "ST is still up");

    let t_stage = stage(15.9, 4.5);
    t_stage.assert_pathological_q("T stage", &[Lead::V2, Lead::V3, Lead::V4]);
    t_stage.assert_t_inverted("T stage", &[Lead::V2, Lead::V3, Lead::V4]);
    assert!(
        st_at(&t_stage, Lead::V5) < st_at(&q_stage, Lead::V5),
        "ST settles"
    );

    let old = stage(20.0, 3.5);
    old.assert_pathological_q("old stage", &[Lead::V2, Lead::V3, Lead::V4]);
    old.assert_t_upright("old stage", &[Lead::V4, Lead::V5]);
}

// --- the library as a whole --------------------------------------------------------------------

#[test]
fn limb_leads_keep_einthovens_law_on_every_new_strip() {
    for id in [
        "mi_anterior_evolved",
        "mi_inferior_evolved",
        "vf_coarse",
        "torsades",
        "flutter_variable",
        "af_preexcited",
    ] {
        let rec = scenario(id).record(4.0).unwrap();
        let (i, ii, iii) = (
            &rec.leads[Lead::I as usize],
            &rec.leads[Lead::II as usize],
            &rec.leads[Lead::III as usize],
        );
        for k in 0..ii.len() {
            let gap = f64::from(ii[k]) - f64::from(i[k]) - f64::from(iii[k]);
            assert!(gap.abs() < 1e-3, "{id} at {k} ms: II − I − III = {gap}");
        }
    }
}

#[test]
fn the_analyser_names_a_rhythm_only_where_the_library_expects_one() {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.sort();
    let mut wrong = Vec::new();
    for f in files {
        let id = f.file_stem().unwrap().to_string_lossy().into_owned();
        let s = scenario(&id);
        let want = s.scenario.expected.as_ref().and_then(|e| e.rhythm);
        let got = s
            .record(VALIDATION_RUN_S)
            .unwrap()
            .analyze_tail(VALIDATION_WINDOW_S)
            .rhythm;
        if got != want {
            wrong.push(format!("{id}: analyser says {got:?}, library says {want:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
