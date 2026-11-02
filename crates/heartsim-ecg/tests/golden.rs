//! Acceptance tests of brief B11: the Tier-0 scenario library in `assets/scenarios/`.
//!
//! * every file loads, has teaching content, ≥ 2 differentials and a unique id equal to its stem;
//! * a 20 s run of every scenario, analysed by B4 over its last 10 s, meets the file's `expected`
//!   block (textbook criteria of the rhythm);
//! * a 5 s, 100 Hz, 3-decimal CSV of the 12 leads is pinned as an `insta` snapshot per scenario
//!   (`cargo insta review` after an intended morphology or timing change).

use heartsim_ecg::Tier0Scenario;
use heartsim_ecg::scenario::VALIDATION_RUN_S;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn library_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios")
}

/// `(file stem, scenario)` for every `*.ron` in the library, sorted by stem.
fn library() -> Vec<(String, Tier0Scenario)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(library_dir())
        .expect("assets/scenarios")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).unwrap();
            let s = Tier0Scenario::from_ron(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            (p.file_stem().unwrap().to_string_lossy().into_owned(), s)
        })
        .collect()
}

#[test]
fn library_loads_and_is_well_formed() {
    let lib = library();
    assert!(lib.len() >= 25, "only {} scenarios", lib.len());
    let mut ids = BTreeSet::new();
    for (stem, s) in &lib {
        assert_eq!(&s.scenario.id, stem, "id must equal the file stem");
        assert!(ids.insert(s.scenario.id.clone()), "duplicate id {stem}");
        let problems = s.content_problems();
        assert!(problems.is_empty(), "{stem}: {problems:?}");
    }
}

#[test]
fn every_scenario_meets_its_expected_findings() {
    let mut failures = Vec::new();
    for (stem, s) in library() {
        let (a, v) = s.validate_run().unwrap_or_else(|e| panic!("{stem}: {e}"));
        eprintln!(
            "{stem:32} HR {:>6.1?} PR {:>6.1?} QRS {:>6.1?} axis {:>6.1?}",
            a.hr_bpm,
            a.pr_ms,
            a.qrs_ms,
            a.qrs_axis.map(|x| x.degrees)
        );
        failures.extend(v.into_iter().map(|m| format!("{stem}: {m}")));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn golden_csv_snapshots() {
    for (stem, s) in library() {
        let rec = s.record(VALIDATION_RUN_S).unwrap();
        let csv = rec.csv_tail(5.0, 100.0, 3);
        insta::assert_snapshot!(stem, csv);
    }
}

#[test]
fn runs_are_deterministic() {
    for (stem, s) in library().into_iter().take(5) {
        let a = s.record(3.0).unwrap();
        let b = s.record(3.0).unwrap();
        assert_eq!(a.leads, b.leads, "{stem}");
        assert_eq!(a.events, b.events, "{stem}");
    }
}
