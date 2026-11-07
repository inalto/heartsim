//! B20 acceptance: `heartsim-cli run` and the app's Export button write byte-identical files for
//! the same scenario and moment.
//!
//! The app side fills strip rings with the app's per-step pipeline (timeline actions before the
//! step, events and pacer markers to the synthesiser, a 500 Hz sample every 2 ms; the order of
//! `HeartSimApp::step_simulation`, which needs a GPU to construct) and then calls
//! `heartsim_app::export_panel::export_from_rings`, the function behind the Export button.

use heartsim_app::export_panel::{ExportRequest, export_from_rings, newest_sample_ms};
use heartsim_app::scenarios::Timeline;
use heartsim_core::{LEAD_COUNT, Lead, Millis};
use heartsim_ecg::{LeadRing, Tier0Scenario};
use heartsim_io::{Formats, Paper};
use heartsim_sim::{Engine, TimingEngine};
use heartsim_ui::{LeadOrder, StripStyle};
use std::path::{Path, PathBuf};

const DATE: &str = "2026-01-15";

fn scenario_file(id: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{id}.ron"))
}

/// Strip rings after `seconds` of the app running scenario `s` (loaded after `warmup` samples of
/// something else, as when the user picks a rhythm while another one runs).
fn app_rings(s: &Tier0Scenario, seconds: f64, warmup: usize) -> (Vec<LeadRing>, f64) {
    let mut rings: Vec<LeadRing> = (0..LEAD_COUNT).map(|_| LeadRing::new(30_000, 500.0)).collect();
    for r in &mut rings {
        (0..warmup).for_each(|i| r.push(i as f32));
        r.clear();
    }
    let mut engine = TimingEngine::new(s.timing.clone());
    let mut synth = s.synth().unwrap();
    let mut timeline = Timeline::new(s.scenario.timeline.clone());
    let mut acc = 0.0;
    for _ in 0..(seconds * 1000.0).round() as usize {
        let now = engine.now();
        for a in timeline.due(now) {
            engine.apply(&a.action, a.at).unwrap();
        }
        engine.step(Millis(1.0));
        let events = engine.drain_events();
        synth.on_events(&events);
        synth.on_pacer_markers(&engine.drain_pacer_markers());
        acc += 1.0;
        while acc >= 2.0 {
            acc -= 2.0;
            let x = synth.sample(engine.now());
            for (r, v) in rings.iter_mut().zip(x.0) {
                r.push(v as f32);
            }
        }
    }
    (rings, engine.now().0)
}

fn cli(args: &[&str]) {
    let st = std::process::Command::new(env!("CARGO_BIN_EXE_heartsim-cli"))
        .args(args)
        .status()
        .unwrap();
    assert!(st.success(), "heartsim-cli {args:?}");
}

fn read_dir(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("heartsim-parity-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// The CLI run of `seconds` vs the app paused `scrub_samples` back from `seconds + scrub` s.
fn check(id: &str, seconds: f64, scrub_samples: u64, style: StripStyle, paper: Paper, order: LeadOrder) {
    let path = scenario_file(id);
    let s = Tier0Scenario::from_ron(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let run_s = seconds + scrub_samples as f64 / 500.0;
    let (rings, now_ms) = app_rings(&s, run_s, 777);
    let pushed = rings[0].pushed();
    let head = pushed - scrub_samples;
    let req = ExportRequest {
        scenario: &s,
        style: &style,
        order,
        strip_lead: Lead::V1,
        window_s: 8.0,
        paper,
        formats: Formats::default(),
        date: DATE.into(),
        trace_note: None,
    };
    let app_files =
        export_from_rings(&rings, head, newest_sample_ms(now_ms, pushed, head, 500.0), &req).unwrap();
    let app_dir = tmp(&format!("{id}-app"));
    heartsim_io::write_files(&app_dir, &app_files).unwrap();

    let cli_dir = tmp(&format!("{id}-cli"));
    let mut args = vec![
        "run".to_string(),
        path.display().to_string(),
        "--seconds".into(),
        seconds.to_string(),
        "--window".into(),
        "8".into(),
        "--speed".into(),
        style.mm_per_s.to_string(),
        "--gain".into(),
        style.mm_per_mv.to_string(),
        "--paper".into(),
        paper.label().to_string(),
        "--date".into(),
        DATE.into(),
        "--strip-lead".into(),
        "V1".into(),
        "--out".into(),
        cli_dir.display().to_string(),
    ];
    if order == LeadOrder::Cabrera {
        args.push("--cabrera".into());
    }
    cli(&args.iter().map(String::as_str).collect::<Vec<_>>());

    let (a, c) = (read_dir(&app_dir), read_dir(&cli_dir));
    let names: Vec<&str> = a.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, c.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>());
    assert_eq!(a.len(), 8, "{names:?}");
    for ((name, x), (_, y)) in a.iter().zip(&c) {
        assert!(
            x == y,
            "{id}: {name} differs between app and CLI ({} vs {} bytes)",
            x.len(),
            y.len()
        );
    }
    let _ = std::fs::remove_dir_all(app_dir);
    let _ = std::fs::remove_dir_all(cli_dir);
}

#[test]
fn cli_and_app_exports_are_byte_identical() {
    check(
        "sinus_normal",
        12.0,
        0,
        StripStyle::paper(),
        Paper::A4,
        LeadOrder::Standard,
    );
}

#[test]
fn identical_after_scrubbing_back_with_a_timeline_and_other_settings() {
    let style = StripStyle {
        mm_per_s: 50.0,
        mm_per_mv: 20.0,
        ..StripStyle::paper().monitor()
    };
    // paused 1.5 s back: the app's excerpt ends 750 samples before its newest one
    check("avnrt_150", 9.0, 750, style, Paper::Letter, LeadOrder::Cabrera);
}

#[test]
fn library_ids_and_bad_arguments() {
    let out = tmp("ids");
    cli(&[
        "run",
        "lbbb",
        "--seconds",
        "3",
        "--formats",
        "csv,wfdb",
        "--date",
        DATE,
        "--out",
        &out.display().to_string(),
    ]);
    let names: Vec<String> = read_dir(&out).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        ["heartsim_lbbb.csv", "heartsim_lbbb.dat", "heartsim_lbbb.hea"]
    );
    let _ = std::fs::remove_dir_all(out);
    let bad = std::process::Command::new(env!("CARGO_BIN_EXE_heartsim-cli"))
        .args(["run", "lbbb", "--formats", "gif", "--out", "/nonexistent/x"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
}
