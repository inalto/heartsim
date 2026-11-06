//! B18 acceptance: a patient case can be played through the reader (stage vignettes, one answer per
//! question with feedback, score), and its stage actions reach the host. No GPU.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::case_ui::{CaseLibrary, Cases};
use heartsim_app::tutorial::{TutorialHost, UiAction};
use heartsim_core::Action;
use std::path::{Path, PathBuf};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// Records what the stages ask for; simulated time stands still, so waits never end.
#[derive(Debug, Default)]
struct Host {
    actions: Vec<Action>,
    ui: Vec<UiAction>,
}

impl TutorialHost for Host {
    fn apply_action(&mut self, action: &Action) {
        self.actions.push(action.clone());
    }
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

struct Model {
    cases: Cases,
    host: Host,
}

fn harness() -> Harness<'static, Model> {
    let library = CaseLibrary::load_dir(&assets().join("cases")).unwrap();
    Harness::builder()
        .with_size(egui::vec2(900.0, 1000.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                if m.cases.is_open() {
                    m.cases.reader_ui(ui);
                } else {
                    m.cases.library_ui(ui);
                }
                m.cases.poll(&mut m.host);
            },
            Model {
                cases: Cases::new(library),
                host: Host::default(),
            },
        )
}

fn frames(h: &mut Harness<'_, Model>) {
    h.run_steps(4);
}

fn click(h: &mut Harness<'_, Model>, label: &str) {
    h.get_by_label(label).click();
    frames(h);
}

fn button(h: &mut Harness<'_, Model>, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    frames(h);
}

#[test]
fn the_three_cases_are_listed_and_playable() {
    let mut h = harness();
    frames(&mut h);
    assert!(h.state().cases.library.cases.len() >= 3);
    let case = h.state().cases.library.cases[0].clone();
    let total = case.question_count();

    button(&mut h, &format!("Start {}", case.title));
    assert_eq!(h.state().cases.position(), Some((0, 0)));
    assert!(
        matches!(h.state().host.ui.first(), Some(UiAction::LoadScenario(_))),
        "the first stage opens a rhythm"
    );

    // the first question of the first stage is answered wrongly, every other one correctly
    let mut wrong_first = true;
    for stage in &case.stages {
        for q in &stage.questions {
            let pick = if wrong_first {
                (0..q.choices.len()).find(|&i| i != q.answer).unwrap()
            } else {
                q.answer
            };
            // continuing is blocked until the question is answered
            click(&mut h, &q.choices[pick]);
            if pick == q.answer {
                let _ = h.get_by_label("Correct.");
            } else {
                let _ = h.get_by_label(format!("Not quite. The answer is: {}", q.choices[q.answer]).as_str());
            }
            wrong_first = false;
            let next = if h.query_by_role_and_label(Role::Button, "Finish case").is_some() {
                "Finish case"
            } else {
                "Continue"
            };
            button(&mut h, next);
        }
    }
    let (correct, answered) = h.state().cases.score().expect("case still open on the summary");
    assert_eq!((correct, answered), (total - 1, total));
    let _ = h.get_by_label("Case finished");
    button(&mut h, "Close case");
    assert!(!h.state().cases.is_open());
}
