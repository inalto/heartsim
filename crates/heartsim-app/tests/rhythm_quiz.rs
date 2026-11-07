//! B8 acceptance: the rhythm-quiz flow (start → answer → feedback → next) is reachable through
//! AccessKit nodes, without a GPU.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::rhythm_quiz::RhythmQuiz;
use heartsim_app::scenarios::ScenarioLibrary;
use heartsim_ui::StripStyle;
use std::path::Path;

struct Model {
    quiz: RhythmQuiz,
    library: ScenarioLibrary,
    opened: Option<usize>,
}

fn harness() -> Harness<'static, Model> {
    let library =
        ScenarioLibrary::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios"))
            .unwrap();
    Harness::builder()
        .with_size(egui::vec2(1200.0, 700.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                if let Some(i) = m.quiz.ui(ui, &m.library, &StripStyle::default()) {
                    m.opened = Some(i);
                }
            },
            Model {
                quiz: RhythmQuiz::new(5),
                library,
                opened: None,
            },
        )
}

fn title(h: &Harness<'_, Model>, i: usize) -> String {
    h.state().library.entries[i].scenario.title.clone()
}

#[test]
fn quiz_flow_start_answer_feedback_next() {
    let mut h = harness();
    h.get_by_label("New question").click();
    h.run();
    let q = h.state().quiz.question.clone().expect("a question");
    for &i in &q.options {
        let _ = h.get_by_label(&title(&h, i));
    }

    // right answer → "Correct."
    h.get_by_label(&title(&h, q.answer)).click();
    h.run();
    assert_eq!(h.state().quiz.correct, 1);
    let _ = h.get_by_label("Correct.");
    // the explanation is shown, not folded away: the teaching text and the differential
    let right = h.state().library.entries[q.answer].scenario.clone();
    let _ = h.get_by_label("Why");
    let teaching = right
        .notes
        .as_deref()
        .unwrap()
        .split("\n\nSources:")
        .next()
        .unwrap()
        .trim()
        .to_owned();
    let _ = h.get_by_label_contains(teaching.lines().next().unwrap());
    let _ = h.get_by_label_contains("Often confused with:");

    // next question, wrong answer → names the right one
    h.get_by_label("Next question").click();
    h.run();
    let q2 = h.state().quiz.question.clone().unwrap();
    assert_eq!(h.state().quiz.chosen, None, "fresh question is unanswered");
    let wrong = *q2.options.iter().find(|&&i| i != q2.answer).unwrap();
    h.get_by_label(&title(&h, wrong)).click();
    h.run();
    assert_eq!((h.state().quiz.correct, h.state().quiz.asked), (1, 2));
    let _ = h.get_by_label(&format!("Not quite: this is {}.", title(&h, q2.answer)));
    // a wrong answer also says what the chosen rhythm is
    let _ = h.get_by_label(&format!("You chose: {}", title(&h, wrong)));
    let _ = h.get_by_label(&h.state().library.entries[wrong].scenario.description.clone());

    h.get_by_label("Open in simulator").click();
    h.run();
    assert_eq!(h.state().opened, Some(q2.answer));
}
