//! Brief B18: the ischaemia scenarios of `assets/scenarios/` show what their teaching text claims.
//!
//! The Tier-0 synthesiser renders a coronary lesion as an ST injury vector (B14). Each scenario is
//! run for 20 s; the ST deviation of every lead is the mean voltage 60 ms after the J point minus
//! the mean voltage of the PR segment, averaged over the beats of the last 10 s. The tutorial
//! `10_ischaemia` says which leads rise and which fall; this test pins those statements.

use heartsim_core::{LEAD_COUNT, Lead};
use heartsim_ecg::Tier0Scenario;
use heartsim_ecg::scenario::{ANALYSIS_RATE_HZ, VALIDATION_RUN_S, VALIDATION_WINDOW_S};
use std::path::PathBuf;

/// ST deviation in mV beyond which a lead counts as elevated or depressed.
const SIGNIFICANT_MV: f64 = 0.05;

fn scenario(id: &str) -> Tier0Scenario {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{id}.ron"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Tier0Scenario::from_ron(&text).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Mean ST deviation per lead (`Lead as usize` order), mV, over the beats of the last
/// `window_s` seconds of a run of `run_s` seconds.
fn st_deviation(s: &Tier0Scenario, run_s: f64, window_s: f64) -> [f64; LEAD_COUNT] {
    let rec = s.record(run_s).unwrap();
    let analysis = rec.analyze_tail(window_s);
    let leads: Vec<Vec<f32>> = (0..LEAD_COUNT)
        .map(|l| rec.tail(l, window_s, ANALYSIS_RATE_HZ))
        .collect();
    let per_ms = ANALYSIS_RATE_HZ / 1000.0;
    let at = |ms: f64| (ms * per_ms).round() as usize;
    let mut sum = [0.0; LEAD_COUNT];
    let mut n = 0usize;
    for b in &analysis.beats {
        let (base_from, base_to) = (
            b.qrs_onset.saturating_sub(at(30.0)),
            b.qrs_onset.saturating_sub(at(10.0)),
        );
        let (st_from, st_to) = (b.qrs_offset + at(50.0), b.qrs_offset + at(70.0));
        if base_to <= base_from || st_to >= leads[0].len() {
            continue;
        }
        let mean = |l: usize, a: usize, z: usize| {
            leads[l][a..=z].iter().map(|v| f64::from(*v)).sum::<f64>() / (z - a + 1) as f64
        };
        for (l, total) in sum.iter_mut().enumerate() {
            *total += mean(l, st_from, st_to) - mean(l, base_from, base_to - 1);
        }
        n += 1;
    }
    assert!(n >= 5, "{}: only {n} beats measured", s.scenario.id);
    sum.map(|v| v / n as f64)
}

fn dev(d: &[f64; LEAD_COUNT], lead: Lead) -> f64 {
    d[lead as usize]
}

fn report(id: &str, d: &[f64; LEAD_COUNT]) {
    let row: Vec<String> = Lead::ALL
        .iter()
        .map(|l| format!("{} {:+.2}", l.label(), dev(d, *l)))
        .collect();
    eprintln!("{id:24} {}", row.join("  "));
}

/// `leads` all above (`sign > 0`) or below (`sign < 0`) the significance threshold.
fn assert_st(id: &str, d: &[f64; LEAD_COUNT], sign: f64, leads: &[Lead]) {
    for &l in leads {
        assert!(
            dev(d, l) * sign > SIGNIFICANT_MV,
            "{id}: lead {} ST {:+.3} mV, expected {} than {SIGNIFICANT_MV}",
            l.label(),
            dev(d, l),
            if sign > 0.0 { "more" } else { "less" }
        );
    }
}

fn measured(id: &str) -> [f64; LEAD_COUNT] {
    let d = st_deviation(&scenario(id), VALIDATION_RUN_S, VALIDATION_WINDOW_S);
    report(id, &d);
    d
}

#[test]
fn a_healthy_heart_has_no_st_deviation() {
    let d = measured("sinus_normal");
    for l in Lead::ALL {
        assert!(
            dev(&d, l).abs() < SIGNIFICANT_MV,
            "{}: {:+.3}",
            l.label(),
            dev(&d, l)
        );
    }
}

#[test]
fn anterior_stemi_raises_the_chest_leads_and_lowers_the_inferior_leads() {
    let d = measured("stemi_anterior");
    assert_st(
        "stemi_anterior",
        &d,
        1.0,
        &[Lead::V1, Lead::V2, Lead::V3, Lead::V4, Lead::I, Lead::AVL],
    );
    assert_st("stemi_anterior", &d, -1.0, &[Lead::III, Lead::AVF]);
}

#[test]
fn inferior_stemi_raises_ii_iii_avf_and_lowers_avl_and_avr() {
    let d = measured("stemi_inferior");
    assert_st("stemi_inferior", &d, 1.0, &[Lead::II, Lead::III, Lead::AVF]);
    assert_st("stemi_inferior", &d, -1.0, &[Lead::AVL, Lead::AVR]);
}

#[test]
fn lateral_stemi_raises_i_avl_and_v4_to_v6_and_lowers_iii() {
    let d = measured("stemi_lateral");
    assert_st(
        "stemi_lateral",
        &d,
        1.0,
        &[Lead::I, Lead::AVL, Lead::V4, Lead::V5, Lead::V6],
    );
    assert_st("stemi_lateral", &d, -1.0, &[Lead::III]);
}

#[test]
fn posterior_stemi_lowers_v1_to_v3() {
    let d = measured("stemi_posterior");
    assert_st("stemi_posterior", &d, -1.0, &[Lead::V1, Lead::V2, Lead::V3]);
}

#[test]
fn subendocardial_ischaemia_lowers_many_leads_and_raises_avr() {
    let d = measured("nstemi_subendocardial");
    assert_st(
        "nstemi_subendocardial",
        &d,
        -1.0,
        &[Lead::II, Lead::AVF, Lead::V4, Lead::V5, Lead::V6],
    );
    assert_st("nstemi_subendocardial", &d, 1.0, &[Lead::AVR]);
}

#[test]
fn pericarditis_raises_most_leads_and_lowers_avr() {
    let d = measured("pericarditis");
    assert_st(
        "pericarditis",
        &d,
        1.0,
        &[
            Lead::I,
            Lead::II,
            Lead::AVF,
            Lead::V2,
            Lead::V3,
            Lead::V4,
            Lead::V5,
            Lead::V6,
        ],
    );
    assert_st("pericarditis", &d, -1.0, &[Lead::AVR]);
}

#[test]
fn an_evolving_infarct_goes_from_depression_to_elevation() {
    let s = scenario("stemi_evolving");
    // beats between the partial narrowing (5 s) and the occlusion (10 s)
    let before = st_deviation(&s, 9.9, 4.4);
    report("stemi_evolving (5-10 s)", &before);
    // a partial narrowing shifts the ST segment only slightly (injury of the inner layer only)
    for l in [Lead::V1, Lead::V2, Lead::V3] {
        assert!(
            dev(&before, l) < -0.03,
            "{}: {:+.3} mV",
            l.label(),
            dev(&before, l)
        );
    }
    let after = st_deviation(&s, VALIDATION_RUN_S, 7.0);
    report("stemi_evolving (13-20 s)", &after);
    assert_st("evolving after", &after, 1.0, &[Lead::V2, Lead::V3, Lead::V4]);
}

#[test]
fn reperfusion_brings_the_st_segment_back() {
    use heartsim_core::Millis;
    use heartsim_core::scenario::{Action, TimedAction};
    let mut s = scenario("stemi_anterior");
    s.scenario.timeline.push(TimedAction {
        at: Millis(8000.0),
        action: Action::Stenosis {
            branch: "all".into(),
            severity: 0.0,
        },
    });
    let d = st_deviation(&s, VALIDATION_RUN_S, VALIDATION_WINDOW_S);
    report("stemi_anterior + reperfusion", &d);
    for l in Lead::ALL {
        assert!(
            dev(&d, l).abs() < SIGNIFICANT_MV,
            "{}: {:+.3}",
            l.label(),
            dev(&d, l)
        );
    }
}
