//! Brief B11, engine side: the `timing` section of every preset in `assets/scenarios/` parses,
//! its timeline is accepted by the timing engine, and the ventricular rate counted from engine
//! events alone (no ECG) lies in the file's expected heart-rate range.

use heartsim_core::{Millis, Scenario};
use heartsim_sim::{Chamber, Engine, EventKind, TimingEngine, TimingParams};
use std::path::PathBuf;

/// Ventricular activation times of a `seconds` run (fusion fronts < 150 ms apart merged).
fn ventricular_times(s: &Scenario, p: TimingParams, seconds: f64) -> Vec<f64> {
    let mut e = TimingEngine::new(p);
    let mut timeline = s.timeline.clone();
    timeline.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
    let mut next = 0;
    let mut out: Vec<f64> = Vec::new();
    for _ in 0..(seconds * 1000.0) as usize {
        while let Some(a) = timeline.get(next)
            && a.at.0 <= e.now().0
        {
            e.apply(&a.action, a.at)
                .unwrap_or_else(|err| panic!("{}: {err}", s.id));
            next += 1;
        }
        e.step(Millis(1.0));
        for ev in e.drain_events() {
            if ev.chamber == Chamber::Ventricle
                && ev.kind != EventKind::Blocked
                && out.last().is_none_or(|l| ev.at.0 - l > 150.0)
            {
                out.push(ev.at.0);
            }
        }
    }
    out
}

#[test]
fn every_timing_section_runs_at_its_expected_rate() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios");
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "ron") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let s = Scenario::from_ron(&text).unwrap();
        let p = TimingParams::from_scenario_ron(&text).unwrap();
        // a rhythm with no countable beats (ventricular fibrillation) has no rate to check
        if s.expected.as_ref().is_some_and(|e| !e.rate_is_criterion()) {
            n += 1;
            continue;
        }
        let v = ventricular_times(&s, p, 20.0);
        let last: Vec<f64> = v.iter().copied().filter(|t| *t >= 10_000.0).collect();
        assert!(last.len() >= 3, "{}: {} beats in the last 10 s", s.id, last.len());
        let mean_rr = (last[last.len() - 1] - last[0]) / (last.len() - 1) as f64;
        let hr = 60_000.0 / mean_rr;
        let (lo, hi) = s.expected.as_ref().expect("expected block").hr_bpm;
        // event rate and analyser rate can differ by one beat's worth at the window edges
        assert!(
            (lo - 3.0..=hi + 3.0).contains(&hr),
            "{}: engine rate {hr:.1} bpm outside {lo}..{hi}",
            s.id
        );
        n += 1;
    }
    assert!(n >= 25);
}
