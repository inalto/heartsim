//! Brief B18 acceptance: the shipped teaching content passes the headless checks.
//!
//! * `heartsim-cli validate-scenarios assets/scenarios` passes (40+ scenarios with explanations,
//!   differentials and references);
//! * `heartsim-cli run-tutorial` loads every tutorial and executes every page action without error;
//! * `heartsim-cli validate-cases assets/cases` passes (≥ 5 questions per case, with explanations);
//! * the checks do fail on broken content.

use heartsim_core::case::Case;
use heartsim_core::tutorial::{Step, Tutorial, UiAction};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_heartsim-cli"))
        .args(args)
        .output()
        .expect("runs heartsim-cli")
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// A scratch folder under the target directory, removed by the caller.
fn fresh_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("content-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn tutorial_files() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(assets().join("tutorials"))
        .unwrap()
        .map(|e| e.unwrap().path().join("tutorial.ron"))
        .filter(|p| p.is_file())
        .collect();
    v.sort();
    v
}

#[test]
fn every_scenario_passes_the_validator() {
    let dir = assets().join("scenarios");
    let o = cli(&["validate-scenarios", dir.to_str().unwrap()]);
    let out = text(&o);
    assert!(o.status.success(), "{out}");
    let count = std::fs::read_dir(&dir)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().extension().is_some_and(|x| x == "ron"))
        .count();
    // 40+ rhythms, plus the re-entry preset in `scenarios/reentry/`
    assert!(count >= 40, "only {count} scenarios");
}

#[test]
fn ten_tutorials_run_headless() {
    let files = tutorial_files();
    assert!(files.len() >= 10, "only {} tutorials", files.len());
    let o = cli(&["run-tutorial", "all"]);
    let out = text(&o);
    assert!(o.status.success(), "{out}");
    assert!(out.contains(&format!("{} tutorials", files.len())), "{out}");
    // the ten of the brief, in teaching order
    let ids: Vec<String> = files
        .iter()
        .map(|f| {
            Tutorial::from_ron(&std::fs::read_to_string(f).unwrap())
                .unwrap()
                .id
        })
        .collect();
    for (i, topic) in [
        "basics",
        "normal_rhythm",
        "cardiac_axis",
        "conduction",
        "av_blocks",
        "atrial",
        "svt",
        "wpw",
        "ventricular",
        "ischaemia",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(ids[i], format!("{:02}_{topic}", i + 1), "{ids:?}");
    }
}

#[test]
fn one_tutorial_can_be_run_by_id() {
    let o = cli(&["run-tutorial", "07_svt"]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("1 tutorials"));
}

#[test]
fn every_page_changes_the_simulation_and_names_a_known_rhythm() {
    let scenarios: Vec<String> = std::fs::read_dir(assets().join("scenarios"))
        .unwrap()
        .filter_map(|e| {
            e.unwrap()
                .path()
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .collect();
    for f in tutorial_files() {
        let t = Tutorial::from_ron(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert!(t.pages.len() >= 5, "{}: {} pages", t.id, t.pages.len());
        for (i, p) in t.pages.iter().enumerate() {
            assert!(
                p.actions.iter().any(Step::changes_simulation),
                "{} page {}: no action that changes the simulation",
                t.id,
                i + 1
            );
            for a in &p.actions {
                if let Step::Ui(UiAction::LoadScenario(id)) = a {
                    assert!(
                        scenarios.contains(id),
                        "{} page {}: unknown rhythm {id}",
                        t.id,
                        i + 1
                    );
                }
            }
        }
    }
}

#[test]
fn three_cases_with_five_questions_each_validate() {
    let dir = assets().join("cases");
    let o = cli(&["validate-cases", dir.to_str().unwrap()]);
    let out = text(&o);
    assert!(o.status.success(), "{out}");
    let cases: Vec<Case> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .map(|p| Case::from_ron(&std::fs::read_to_string(p).unwrap()).unwrap())
        .collect();
    assert!(cases.len() >= 3, "only {} cases", cases.len());
    for c in &cases {
        assert!(
            c.question_count() >= 5,
            "{}: {} questions",
            c.id,
            c.question_count()
        );
        assert!(
            c.stages
                .iter()
                .flat_map(|s| &s.questions)
                .all(|q| !q.explanation.trim().is_empty()),
            "{}: a question has no explanation",
            c.id
        );
    }
}

#[test]
fn broken_tutorials_and_cases_fail() {
    let dir = fresh_dir("broken");
    // an unknown highlight, a page that changes nothing, and a drug the table does not know
    let bad = r#"Tutorial(
        format_version: 1, id: "bad", title: "Bad", summary: "s", references: ["r"],
        pages: [
            Page(title: "a", markdown: "m", actions: [Ui(LoadScenario("sinus_normal")), Ui(Highlight("no_such_tool"))]),
            Page(title: "b", markdown: "m", actions: [Ui(Pause)]),
            Page(title: "c", markdown: "m", actions: [Sim(Drug(name: "no_such_drug", dose: 1.0))]),
        ],
    )"#;
    std::fs::create_dir_all(dir.join("bad")).unwrap();
    std::fs::write(dir.join("bad/tutorial.ron"), bad).unwrap();
    let o = cli(&["run-tutorial", dir.join("bad/tutorial.ron").to_str().unwrap()]);
    let out = text(&o);
    assert!(!o.status.success(), "{out}");
    for needle in [
        "unknown highlight",
        "no action that changes the simulation",
        "no_such_drug",
    ] {
        assert!(out.contains(needle), "missing `{needle}` in {out}");
    }

    let short = r#"Case(
        format_version: 1, id: "short", title: "T", summary: "s", patient: "p", references: ["r"],
        stages: [Stage(title: "a", markdown: "m", actions: [Ui(LoadScenario("lbbb"))],
            questions: [Question(prompt: "?", choices: ["a", "b", "c"], answer: 0, explanation: "e")])],
    )"#;
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    std::fs::write(cases.join("short.ron"), short).unwrap();
    let o = cli(&["validate-cases", cases.to_str().unwrap()]);
    assert!(!o.status.success());
    assert!(text(&o).contains("1 questions (need"), "{}", text(&o));
    std::fs::remove_dir_all(&dir).unwrap();
}
