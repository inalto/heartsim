//! B42 acceptance: the case editor on the Cases page. New case, fill the fields, the problems panel
//! empties and Save as… produces a file the reader accepts and that equals the draft; Edit a copy
//! leaves the built-in case alone; an action row that does not parse disables Save and is named in
//! the panel; Try it plays the draft from the selected stage and closing the reader returns to the
//! editor; the draft survives leaving the page; the layout stacks on a phone-width window. No GPU, no
//! dialogs (a recording `CaseFileIo`), no network. The pure parts (the id, the examples, the draft's
//! operations) are unit tests of `case_edit`.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use heartsim_app::case_io::{CaseFileIo, IoEvent, StorageStore};
use heartsim_app::case_ui::{CaseLibrary, CaseRef, Cases};
use heartsim_app::scenarios::ScenarioLibrary;
use heartsim_app::tutorial::{TutorialHost, UiAction};
use heartsim_casefile::CasePackage;
use heartsim_core::Action;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

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

#[derive(Debug, Default)]
struct Recorded {
    saved: Vec<(String, Vec<u8>)>,
    answers: VecDeque<IoEvent>,
}

#[derive(Debug, Clone, Default)]
struct RecordingIo(Rc<RefCell<Recorded>>);

impl CaseFileIo for RecordingIo {
    fn open(&mut self) {}
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

fn harness_sized(width: f32) -> (Harness<'static, Model>, RecordingIo) {
    heartsim_ui::i18n::set_language("en");
    let io = RecordingIo::default();
    let library = CaseLibrary::load_dir(&assets().join("cases")).unwrap();
    let cases = Cases::with_io(library, Box::new(io.clone()), Box::new(StorageStore::session()));
    let scenarios = ScenarioLibrary::load_dir(&assets().join("scenarios")).unwrap();
    let h = Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(width, 3000.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                m.cases.poll_io(&mut m.scenarios);
                if m.cases.is_open() {
                    m.cases.reader_ui(ui);
                } else {
                    m.cases.page_ui(ui, true, &m.scenarios);
                }
                m.cases.poll(&mut m.host);
            },
            Model {
                cases,
                scenarios,
                host: Host::default(),
            },
        );
    (h, io)
}

fn harness() -> (Harness<'static, Model>, RecordingIo) {
    harness_sized(1100.0)
}

fn frames(h: &mut Harness<'_, Model>) {
    h.run_steps(4);
}

/// Everything the form asks for, except the title (the test types that one).
fn fill(h: &mut Harness<'_, Model>) {
    let d = h.state_mut().cases.draft_mut().expect("a draft");
    d.summary = "A walk to the shop.".into();
    d.patient = "62-year-old man".into();
    d.language = "en".into();
    d.authors = "A. Teacher".into();
    d.licence = "CC-BY-4.0".into();
    d.references = "Somebody 2026".into();
    d.stages[0].title = "At the door".into();
    d.stages[0].markdown = "He **fell**.".into();
    d.insert_load_rhythm(0, "sinus_normal");
    for n in 0..5 {
        let q = d.add_question(0);
        d.stages[0].questions[q].prompt = format!("Question {n}?");
        for (i, c) in d.stages[0].questions[q].choices.iter_mut().enumerate() {
            *c = format!("choice {i}");
        }
        d.stages[0].questions[q].explanation = "Because.".into();
        d.set_answer(0, q, n % 3);
    }
}

fn save_enabled(h: &Harness<'_, Model>) -> bool {
    !h.get_by_role_and_label(Role::Button, "Save as…")
        .accesskit_node()
        .is_disabled()
}

fn click_button(h: &mut Harness<'_, Model>, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    frames(h);
}

/// Type `text` into the `n`th single-line text field of the page (0 is the title).
fn type_into(h: &mut Harness<'_, Model>, n: usize, text: &str) {
    h.query_all_by_role(Role::TextInput).nth(n).unwrap().click();
    frames(h);
    h.query_all_by_role(Role::TextInput)
        .nth(n)
        .unwrap()
        .type_text(text);
    frames(h);
}

#[test]
fn a_new_case_is_filled_in_saved_and_the_file_equals_the_draft() {
    let (mut h, io) = harness();
    frames(&mut h);
    click_button(&mut h, "New case");
    assert!(h.state().cases.is_editing());
    let _ = h.get_by_label("Case editor");
    // an empty case breaks no rule of the file: the editorial rules are advice only
    assert!(h.query_by_label_contains("No problems").is_none());
    assert!(h.query_all_by_label("Advice").count() > 0);
    assert!(h.query_by_label("Must fix").is_none());
    assert!(
        h.query_by_label("not saved").is_none(),
        "an empty draft is not unsaved work"
    );

    type_into(&mut h, 0, "Chest pain at the fair");
    assert_eq!(h.state().cases.draft().unwrap().title, "Chest pain at the fair");
    assert_eq!(h.state().cases.draft().unwrap().id, "chest_pain_at_the_fair");
    let _ = h.get_by_label("not saved");

    fill(&mut h);
    frames(&mut h);
    let _ = h.get_by_label_contains("No problems");
    assert!(save_enabled(&h));
    click_button(&mut h, "Save as…");

    let (name, bytes) = io.0.borrow().saved[0].clone();
    assert_eq!(name, "chest_pain_at_the_fair.hs");
    let draft = h.state().cases.draft().unwrap().clone();
    let read = CasePackage::read(&bytes).expect("the reader accepts the file");
    assert_eq!(read.case, draft.to_case());
    assert_eq!(read.manifest.id, draft.id);
    assert_eq!(read.manifest.title, draft.title);
    assert_eq!(read.manifest.authors, ["A. Teacher"]);
    assert_eq!(read.manifest.licence, "CC-BY-4.0");
    assert_eq!(read.manifest.language, "en");
    assert_eq!(read.scenarios, draft.bundled);
    // nothing joins My cases, and "not saved" stays, until the dialog reports the file written
    assert!(h.state().cases.user.is_empty());
    let _ = h.get_by_label("not saved");
    io.0.borrow_mut().answers.push_back(IoEvent::Saved {
        name: "/tmp/x.hs".into(),
    });
    frames(&mut h);
    assert_eq!(h.state().cases.user.len(), 1);
    assert_eq!(h.state().cases.user[0].bytes, bytes);
    assert!(h.query_by_label("not saved").is_none());
    let _ = h.get_by_label_contains("It is now under My cases");
}

#[test]
fn editing_a_copy_of_a_built_in_case_leaves_it_unchanged() {
    let (mut h, io) = harness();
    frames(&mut h);
    let before = h.state().cases.library.cases.clone();
    let first = before[0].clone();
    let copy_buttons: Vec<_> = h
        .query_all_by_role(Role::Button)
        .filter(|n| n.accesskit_node().label().is_some_and(|l| l == "Edit a copy"))
        .collect();
    assert_eq!(copy_buttons.len(), before.len(), "every card offers it");
    copy_buttons[0].click();
    frames(&mut h);
    let d = h.state().cases.draft().unwrap().clone();
    assert_eq!(d.id, format!("{}_copy", first.id));
    assert_eq!(d.title, format!("Copy of {}", first.title));
    assert_eq!(d.to_case().stages, first.stages, "the copy has the text");
    assert!(
        h.query_by_label("not saved").is_none(),
        "an unchanged copy is not unsaved work"
    );

    // change the copy, save it
    h.state_mut().cases.draft_mut().unwrap().stages[0].title = "Changed in the copy".into();
    frames(&mut h);
    let _ = h.get_by_label("not saved");
    assert!(
        save_enabled(&h),
        "{:?}",
        h.state().cases.draft().map(|d| d.problems(&h.state().scenarios))
    );
    click_button(&mut h, "Save as…");
    let bytes = io.0.borrow().saved[0].1.clone();
    let p = CasePackage::read(&bytes).unwrap();
    assert_eq!(p.case.stages[0].title, "Changed in the copy");
    assert_eq!(p.case.id, d.id);
    assert_eq!(
        h.state().cases.library.cases,
        before,
        "the built-in cases are untouched"
    );

    // leaving the page keeps the draft; the list offers to continue or discard it
    click_button(&mut h, "Back to cases");
    assert!(!h.state().cases.is_editing());
    let _ = h.get_by_label_contains("Draft in progress: Copy of");
    assert_eq!(
        h.state().cases.draft().unwrap().stages[0].title,
        "Changed in the copy"
    );
    // a new case asks before it replaces unsaved work
    click_button(&mut h, "New case");
    assert!(!h.state().cases.is_editing());
    let _ = h.get_by_label_contains("Replace the draft you are writing");
    click_button(&mut h, "Keep the draft");
    assert!(h.query_by_label_contains("Replace the draft").is_none());
    click_button(&mut h, "Continue editing");
    assert!(h.state().cases.is_editing());
    assert_eq!(
        h.state().cases.draft().unwrap().stages[0].title,
        "Changed in the copy"
    );
    click_button(&mut h, "Back to cases");
    click_button(&mut h, "Discard draft");
    click_button(&mut h, "Yes, discard");
    assert!(h.state().cases.draft().is_none());
}

#[test]
fn an_action_row_that_does_not_parse_disables_save_and_is_named_in_the_panel() {
    let (mut h, io) = harness();
    frames(&mut h);
    click_button(&mut h, "New case");
    type_into(&mut h, 0, "A case with a typo");
    fill(&mut h);
    frames(&mut h);
    assert!(save_enabled(&h));

    h.state_mut().cases.draft_mut().unwrap().stages[0].rows[0].set_text("Ui(LoadScenari(\"x\"))".into());
    frames(&mut h);
    assert!(!save_enabled(&h));
    let _ = h.get_by_label_contains("Stage 1, action 1");
    let _ = h.get_by_label_contains("Does not parse");
    let _ = h.get_by_label("Must fix");
    // clicking the disabled button saves nothing
    h.get_by_role_and_label(Role::Button, "Save as…").click();
    frames(&mut h);
    assert_eq!(io.0.borrow().saved.len(), 0);

    // a rhythm that does not exist is marked too
    h.state_mut().cases.draft_mut().unwrap().stages[0].rows[0]
        .set_text("Ui(LoadScenario(\"no_such_rhythm\"))".into());
    frames(&mut h);
    assert!(!save_enabled(&h));
    let _ = h.get_by_label_contains("there is no rhythm `no_such_rhythm`");

    // fixing it empties the panel again
    h.state_mut().cases.draft_mut().unwrap().stages[0].rows[0]
        .set_text("Ui(LoadScenario(\"sinus_normal\"))".into());
    frames(&mut h);
    assert!(save_enabled(&h));
    let _ = h.get_by_label_contains("No problems");

    // an id that breaks the rule of the format is a problem too
    h.state_mut().cases.draft_mut().unwrap().set_id("Bad Id");
    frames(&mut h);
    assert!(!save_enabled(&h));
    let _ = h.get_by_label_contains("The case id must be 1 to 64 characters");
}

#[test]
fn try_it_plays_the_draft_from_the_selected_stage_and_closing_returns_to_the_editor() {
    let (mut h, _) = harness();
    frames(&mut h);
    click_button(&mut h, "New case");
    type_into(&mut h, 0, "Two stages");
    fill(&mut h);
    {
        let d = h.state_mut().cases.draft_mut().unwrap();
        let b = d.add_stage();
        d.stages[b].title = "Second".into();
        d.stages[b].markdown = "Later.".into();
        d.add_step(
            b,
            &heartsim_core::tutorial::Step::Ui(UiAction::SelectLead(heartsim_core::Lead::V1)),
        );
    }
    frames(&mut h);
    let before = h.state().cases.draft().unwrap().clone();
    h.get_by_label("2. Second").click();
    frames(&mut h);
    click_button(&mut h, "Try it");
    assert_eq!(h.state().cases.position(), Some((CaseRef::Draft, 1)));
    assert_eq!(
        h.state().host.ui,
        [UiAction::SelectLead(heartsim_core::Lead::V1)],
        "the stage's own actions ran, not the first stage's"
    );
    let _ = h.get_by_label("Two stages");
    // closing the reader returns to the editor, whose draft is as it was
    click_button(&mut h, "Close case");
    assert!(h.state().cases.is_editing());
    assert!(!h.state().cases.is_open());
    let _ = h.get_by_label("Case editor");
    assert_eq!(h.state().cases.draft().unwrap(), &before);
    // a trial keeps no score
    assert!(h.state().cases.best(CaseRef::Draft).is_none());
}

#[test]
fn the_helpers_insert_rows_that_parse() {
    let (mut h, _) = harness();
    frames(&mut h);
    click_button(&mut h, "New case");
    click_button(&mut h, "Add a wait");
    click_button(&mut h, "Insert example");
    let d = h.state().cases.draft().unwrap();
    let rows = &d.stages[0].rows;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].text, "Ui(WaitForEvent(ElapsedMs(3000.0)))");
    assert!(rows.iter().all(|r| r.parsed.is_ok()), "{rows:?}");
    // the new row is shown, with its own buttons
    let _ = h.get_by_label("Action 2");
}

#[test]
fn a_rhythm_added_in_the_session_is_offered_for_bundling_and_travels_with_the_file() {
    let (mut h, io) = harness();
    {
        let m = h.state_mut();
        let mut s = m.scenarios.entries[m.scenarios.index_of("sinus_normal").unwrap()].clone();
        s.scenario.id = "teachers_own".into();
        s.scenario.title = "Teacher's own rhythm".into();
        heartsim_app::export_panel::add_to_library(&mut m.scenarios, s);
    }
    frames(&mut h);
    click_button(&mut h, "New case");
    type_into(&mut h, 0, "With my rhythm");
    fill(&mut h);
    h.state_mut()
        .cases
        .draft_mut()
        .unwrap()
        .insert_load_rhythm(0, "teachers_own");
    frames(&mut h);
    assert!(!save_enabled(&h), "the rhythm is not bundled yet");
    let _ = h.get_by_label_contains("bundle it, or the file will not open");
    click_button(&mut h, "Bundle");
    assert!(save_enabled(&h));
    assert_eq!(h.state().cases.draft().unwrap().bundled.len(), 1);
    let _ = h.get_by_label_contains("teachers_own");
    click_button(&mut h, "Save as…");
    let bytes = io.0.borrow().saved[0].1.clone();
    let p = CasePackage::read(&bytes).unwrap();
    assert_eq!(p.manifest.scenarios, ["teachers_own"]);
    io.0.borrow_mut()
        .answers
        .push_back(IoEvent::Saved { name: "x.hs".into() });
    frames(&mut h);
    assert_eq!(h.state().cases.user.len(), 1, "{:?}", h.state().cases.status());
}

#[test]
fn on_a_phone_width_window_the_stage_list_is_above_the_stage() {
    let (mut h, _) = harness_sized(380.0);
    frames(&mut h);
    click_button(&mut h, "New case");
    fill(&mut h);
    frames(&mut h);
    let list = h.get_by_label("1. At the door").rect();
    let title = h.get_by_label("Stage title").rect();
    assert!(
        list.bottom() <= title.top() + 1.0,
        "the list {list:?} is not above the stage's title {title:?}"
    );
    let _ = h.get_by_label("Preview");
    assert!(save_enabled(&h));

    // on a wide window it is beside it
    let (mut w, _) = harness_sized(1100.0);
    frames(&mut w);
    click_button(&mut w, "New case");
    fill(&mut w);
    frames(&mut w);
    let list = w.get_by_label("1. At the door").rect();
    let title = w.get_by_label("Stage title").rect();
    assert!(list.right() <= title.left() + 1.0, "{list:?} {title:?}");
}
