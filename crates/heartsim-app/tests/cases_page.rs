//! B41 acceptance: the Cases page lists the built-in cases, My cases and the online slot; the
//! Tutorials page lists none; a package handed to the page's `open_bytes` appears under My cases,
//! starts and is offered for saving; a rejected file shows its error; an opened rhythm never enters
//! a quiz pool. No GPU, no dialogs (a recording `CaseFileIo` stands in for them), no network.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use heartsim_app::case_io::{CaseFileIo, IoEvent};
use heartsim_app::case_ui::{CaseLibrary, CaseRef, Cases};
use heartsim_app::scenarios::{ScenarioLibrary, picker_groups};
use heartsim_app::tutorial::{TutorialHost, TutorialLibrary, UiAction};
use heartsim_app::tutorial_ui::{self, Tutorials};
use heartsim_casefile::{CasePackage, Manifest};
use heartsim_core::Action;
use heartsim_core::case::{CASE_FORMAT_VERSION, Case, Stage};
use heartsim_core::tutorial::{Question, Step};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// Records what the stages ask for; simulated time stands still.
#[derive(Debug, Default)]
struct Host {
    ui: Vec<UiAction>,
}

impl TutorialHost for Host {
    fn apply_action(&mut self, _action: &Action) {}
    fn apply_ui(&mut self, action: &UiAction) {
        self.ui.push(action.clone());
    }
    fn sim_now_ms(&self) -> f64 {
        0.0
    }
    fn last_qrs_ms(&self) -> f64 {
        -1.0
    }
}

/// The dialogs, recorded: `save` keeps what it was given; the test pushes the dialog's answers.
#[derive(Debug, Default)]
struct Recorded {
    saved: Vec<(String, Vec<u8>)>,
    opens: usize,
    answers: VecDeque<IoEvent>,
}

#[derive(Debug, Clone, Default)]
struct RecordingIo(Rc<RefCell<Recorded>>);

impl CaseFileIo for RecordingIo {
    fn open(&mut self) {
        self.0.borrow_mut().opens += 1;
    }
    fn save(&mut self, file_name: &str, bytes: Vec<u8>) {
        self.0.borrow_mut().saved.push((file_name.to_owned(), bytes));
    }
    fn poll(&mut self) -> Option<IoEvent> {
        self.0.borrow_mut().answers.pop_front()
    }
    fn busy(&self) -> bool {
        false
    }
}

struct Model {
    cases: Cases,
    scenarios: ScenarioLibrary,
    host: Host,
}

fn scenario_library() -> ScenarioLibrary {
    ScenarioLibrary::load_dir(&assets().join("scenarios")).unwrap()
}

fn harness() -> (Harness<'static, Model>, RecordingIo) {
    heartsim_ui::i18n::set_language("en");
    let io = RecordingIo::default();
    let library = CaseLibrary::load_dir(&assets().join("cases")).unwrap();
    let cases = Cases::with_io(
        library,
        Box::new(io.clone()),
        Box::new(heartsim_app::case_io::StorageStore::session()),
    );
    let h = Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(900.0, 6000.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                m.cases.poll_io(&mut m.scenarios);
                if m.cases.is_open() {
                    m.cases.reader_ui(ui);
                } else {
                    m.cases.library_ui(ui, true);
                }
                m.cases.poll(&mut m.host);
            },
            Model {
                cases,
                scenarios: scenario_library(),
                host: Host::default(),
            },
        );
    (h, io)
}

fn frames(h: &mut Harness<'_, Model>) {
    h.run_steps(4);
}

/// A rhythm file of the library under another id.
fn scenario_text(id: &str) -> String {
    std::fs::read_to_string(assets().join("scenarios/sinus_normal.ron"))
        .unwrap()
        .replacen("id: \"sinus_normal\"", &format!("id: \"{id}\""), 1)
}

/// A case whose first stage loads the bundled rhythm `teacher_rhythm`.
fn package(id: &str, title: &str) -> CasePackage {
    let q = |n: usize| Question {
        prompt: format!("Question {n}?"),
        choices: vec!["one".into(), "two".into(), "three".into()],
        answer: n % 3,
        explanation: "Because.".into(),
    };
    let case = Case {
        format_version: CASE_FORMAT_VERSION,
        id: id.into(),
        title: title.into(),
        summary: "A teacher's case.".into(),
        patient: "A synthetic patient".into(),
        references: vec!["Somebody 2026".into()],
        stages: vec![Stage {
            title: "Only".into(),
            markdown: "Look at the strip.".into(),
            actions: vec![Step::Ui(UiAction::LoadScenario("teacher_rhythm".into()))],
            questions: (0..5).map(q).collect(),
        }],
    };
    let manifest = Manifest::for_case(&case, vec!["A Teacher".into()], "CC-BY-4.0", "2026-11-27", "test");
    CasePackage::new(
        manifest,
        case,
        vec![("teacher_rhythm".into(), scenario_text("teacher_rhythm"))],
    )
}

fn buttons_starting(h: &Harness<'_, Model>, prefix: &str) -> Vec<String> {
    h.query_all_by_role(Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.starts_with(prefix))
        .collect()
}

#[test]
fn the_cases_page_lists_every_built_in_case_and_its_three_sections() {
    let (mut h, _) = harness();
    frames(&mut h);
    let titles: Vec<String> = h
        .state()
        .cases
        .library
        .cases
        .iter()
        .map(|c| c.title.clone())
        .collect();
    assert!(titles.len() >= 9, "{titles:?}");
    for t in &titles {
        let _ = h.get_by_role_and_label(Role::Button, &format!("Start {t}"));
    }
    for section in ["Built-in", "My cases", "Online"] {
        let _ = h.get_by_label(section);
    }
    let _ = h.get_by_label_contains("No case files yet");
    assert_eq!(buttons_starting(&h, "Save as").len(), titles.len());
    assert!(
        buttons_starting(&h, "Remove").is_empty(),
        "built-in cases cannot be removed"
    );
}

#[test]
fn the_tutorials_page_lists_no_case_and_links_to_the_cases_page() {
    heartsim_ui::i18n::set_language("en");
    let tutorials = Tutorials::new(
        TutorialLibrary::load_dir(&assets().join("tutorials")).unwrap(),
        None,
    );
    let cases = CaseLibrary::load_dir(&assets().join("cases")).unwrap();
    let mut h = Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(900.0, 6000.0))
        .build_ui_state(
            |ui, (t, go): &mut (Tutorials, bool)| {
                if tutorial_ui::list_page(ui, t) {
                    *go = true;
                }
            },
            (tutorials, false),
        );
    h.run_steps(3);
    for c in &cases.cases {
        assert!(
            h.query_by_label_contains(&c.title).is_none(),
            "the Tutorials page lists the case {}",
            c.title
        );
    }
    h.get_by_label("Open the Cases page").click();
    h.run_steps(3);
    assert!(h.state().1, "the link asks for the Cases page");
}

#[test]
fn an_opened_package_joins_my_cases_starts_and_is_offered_for_saving() {
    let (mut h, io) = harness();
    frames(&mut h);
    let bytes = package("case_teacher", "The teacher's case").write().unwrap();
    {
        let m = h.state_mut();
        let i = m
            .cases
            .open_bytes(&bytes, "teacher.hs", &mut m.scenarios)
            .unwrap();
        assert_eq!(i, 0);
    }
    frames(&mut h);
    let _ = h.get_by_label_contains("Opened The teacher's case");
    let _ = h.get_by_label_contains("Authors: A Teacher · Licence: CC-BY-4.0 · teacher.hs");
    // the bundled rhythm joined the library under "From cases"
    let lib = &h.state().scenarios;
    let i = lib.index_of("teacher_rhythm").expect("added for the session");
    assert!(lib.is_from_case(i));
    assert_eq!(picker_groups(lib).last().unwrap().1, [i]);

    // offered for saving: the user card's Save as… gives back the file as it was opened
    let saves: Vec<_> = h
        .query_all_by_role(Role::Button)
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Save as"))
        })
        .collect();
    saves.last().unwrap().click();
    frames(&mut h);
    {
        let rec = io.0.borrow();
        assert_eq!(rec.saved.len(), 1);
        assert_eq!(rec.saved[0].0, "case_teacher.hs");
        assert_eq!(rec.saved[0].1, bytes);
    }

    // starts: the first stage loads the bundled rhythm
    h.get_by_role_and_label(Role::Button, "Start The teacher's case")
        .click();
    frames(&mut h);
    assert_eq!(h.state().cases.position(), Some((CaseRef::User(0), 0)));
    assert_eq!(
        h.state().host.ui.first(),
        Some(&UiAction::LoadScenario("teacher_rhythm".into()))
    );
    let _ = h.get_by_label("The teacher's case");
}

#[test]
fn a_rejected_file_shows_its_error_text_and_adds_nothing() {
    let (mut h, _) = harness();
    frames(&mut h);
    {
        let m = h.state_mut();
        assert!(
            m.cases
                .open_bytes(b"not a zip", "bad.hs", &mut m.scenarios)
                .is_err()
        );
    }
    frames(&mut h);
    let _ = h.get_by_label_contains("Could not open bad.hs: not a case file");
    assert!(h.state().cases.user.is_empty());
    // a case needing a rhythm this build lacks
    let mut p = package("case_needs", "Needs a rhythm");
    p.scenarios.clear();
    let bytes = p.write().unwrap();
    {
        let m = h.state_mut();
        assert!(m.cases.open_bytes(&bytes, "needs.hs", &mut m.scenarios).is_err());
    }
    frames(&mut h);
    let _ = h.get_by_label_contains("needs the rhythm `teacher_rhythm`");
    assert!(h.state().cases.user.is_empty());
}

#[test]
fn the_open_button_asks_the_dialog_and_its_answer_is_opened() {
    let (mut h, io) = harness();
    frames(&mut h);
    h.get_by_role_and_label(Role::Button, "Open case file…").click();
    frames(&mut h);
    assert_eq!(io.0.borrow().opens, 1);
    let bytes = package("case_teacher", "The teacher's case").write().unwrap();
    io.0.borrow_mut().answers.push_back(IoEvent::Opened {
        name: "picked.hs".into(),
        bytes,
    });
    frames(&mut h);
    assert_eq!(h.state().cases.user.len(), 1);
    let _ = h.get_by_role_and_label(Role::Button, "Start The teacher's case");
}

#[test]
fn a_built_in_case_saved_as_a_file_joins_my_cases_with_its_own_score() {
    let (mut h, io) = harness();
    frames(&mut h);
    let first = h.state().cases.library.cases[0].clone();
    let saves: Vec<_> = h
        .query_all_by_role(Role::Button)
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Save as"))
        })
        .collect();
    saves[0].click();
    frames(&mut h);
    let (name, bytes) = io.0.borrow().saved[0].clone();
    assert_eq!(name, format!("{}.hs", first.id));
    let p = CasePackage::read(&bytes).unwrap();
    assert_eq!(p.case, first);
    assert_eq!(p.manifest.authors, ["heartsim authors"]);
    assert_eq!(p.manifest.licence, "MIT OR Apache-2.0");
    // nothing joins My cases until the dialog reports the file written
    assert!(h.state().cases.user.is_empty());
    io.0.borrow_mut().answers.push_back(IoEvent::Saved {
        name: "/tmp/x.hs".into(),
    });
    frames(&mut h);
    assert_eq!(h.state().cases.user.len(), 1);
    let file_title = format!("{} (file)", first.title);
    let _ = h.get_by_role_and_label(Role::Button, &format!("Start {file_title}"));
    let cases = &h.state().cases;
    assert_ne!(
        cases.score_key(CaseRef::Builtin(0)),
        cases.score_key(CaseRef::User(0)),
        "a file with a built-in case's id keeps its own score"
    );

    // Remove asks first, then removes
    h.get_by_role_and_label(Role::Button, "Remove").click();
    frames(&mut h);
    assert_eq!(h.state().cases.user.len(), 1);
    h.get_by_role_and_label(Role::Button, "Keep it").click();
    frames(&mut h);
    h.get_by_role_and_label(Role::Button, "Remove").click();
    frames(&mut h);
    h.get_by_role_and_label(Role::Button, "Yes, remove").click();
    frames(&mut h);
    assert!(h.state().cases.user.is_empty());
}

#[test]
fn the_quiz_pools_are_unchanged_by_an_opened_case_with_a_bundled_rhythm() {
    let mut scenarios = scenario_library();
    let ecg = scenarios.ecg_quiz_pool();
    let sounds = scenarios.auscultation_pool();
    let n = scenarios.len();
    let mut cases = Cases::new(CaseLibrary::default());
    let bytes = package("case_teacher", "The teacher's case").write().unwrap();
    cases.open_bytes(&bytes, "t.hs", &mut scenarios).unwrap();
    assert_eq!(scenarios.len(), n + 1);
    assert_eq!(scenarios.ecg_quiz_pool(), ecg);
    assert_eq!(scenarios.auscultation_pool(), sounds);
    // the Rhythm Quiz draws only from that pool
    let new = scenarios.index_of("teacher_rhythm").unwrap();
    let mut rng = 7;
    for _ in 0..300 {
        let q = heartsim_app::rhythm_quiz::make_question(&scenarios, &mut rng, None).unwrap();
        assert!(q.answer != new && !q.options.contains(&new));
    }
    // a second file bundling a different rhythm under the same id is refused
    let mut other = package("case_other", "Another case");
    other.scenarios[0].1.push_str("\n// changed\n");
    let e = cases
        .open_bytes(&other.write().unwrap(), "o.hs", &mut scenarios)
        .unwrap_err();
    assert!(e.to_string().contains("teacher_rhythm"), "{e}");
}
