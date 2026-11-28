//! B8 acceptance: the rhythm-quiz flow (start → answer → feedback → next) is reachable through
//! AccessKit nodes, without a GPU.

use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
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
        .with_context_setup(heartsim_ui::theme::apply_to_context)
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

/// After an answer the right option says "(correct)" and a wrong choice says "(your answer)" in
/// words (the tick and the cross are painted beside them), the other options stay as they were,
/// and none of the four can be answered again.
#[test]
fn answered_options_are_marked_without_relying_on_colour() {
    let mut h = harness();
    h.get_by_label("New question").click();
    h.run();
    let q = h.state().quiz.question.clone().unwrap();
    let wrong = *q.options.iter().find(|&&i| i != q.answer).unwrap();
    h.get_by_label(&title(&h, wrong)).click();
    h.run();
    let _ = h.get_by_label(&format!("{}  (correct)", title(&h, q.answer)));
    let _ = h.get_by_label(&format!("{}  (your answer)", title(&h, wrong)));
    let others = q.options.iter().filter(|&&i| i != q.answer && i != wrong);
    for &i in others {
        assert!(
            h.query_by_label(&title(&h, i)).is_some(),
            "plain option kept its name"
        );
    }
    // a second click on any option changes nothing
    h.get_by_label(&format!("{}  (correct)", title(&h, q.answer)))
        .click();
    h.run();
    assert_eq!((h.state().quiz.asked, h.state().quiz.correct), (1, 0));
    // the score says what it counts
    let _ = h.get_by_label("Score: 0 of 1 correct");
}

/// The primary actions on the page (by label, in tree order): the rule of use allows one per panel.
fn primary_names<S>(h: &Harness<'_, S>) -> Vec<String> {
    h.query_all(By::new())
        .filter(|n| n.accesskit_node().class_name() == Some(heartsim_ui::PRIMARY_CLASS))
        .map(|n| n.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// New question first, none while the choices are the action, Next question after the answer.
#[test]
fn the_rhythm_quiz_has_one_primary_action_at_a_time() {
    let mut h = harness();
    h.run();
    assert_eq!(primary_names(&h), ["New question"]);
    h.get_by_label("New question").click();
    h.run();
    assert!(primary_names(&h).is_empty(), "the answers are the action");
    let q = h.state().quiz.question.clone().unwrap();
    h.get_by_label(&title(&h, q.answer)).click();
    h.run();
    assert_eq!(primary_names(&h), ["Next question"]);
}

/// B40: a finished recording starts at the left edge of the strip, whatever the paper speed and
/// the width of the window (it was right-aligned like live paper, leaving the left empty).
#[test]
fn a_recorded_strip_starts_at_the_left_edge() {
    let library =
        ScenarioLibrary::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios"))
            .unwrap();
    let mut quiz = RhythmQuiz::new(5);
    quiz.next_question(&library);
    for mm_per_s in [12.5, 25.0, 50.0] {
        for width in [400.0_f32, 750.0, 1200.0, 2400.0] {
            let style = StripStyle {
                mm_per_s,
                ..StripStyle::default()
            };
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 300.0));
            let win = quiz.strip_window(rect, &style).expect("a recording");
            let x = win
                .x_of_sample(0)
                .unwrap_or_else(|| panic!("{mm_per_s} mm/s, {width} pt: sample 0 is not on screen"));
            assert!(
                (x - win.rect.left()).abs() <= win.px_per_sample,
                "{mm_per_s} mm/s, {width} pt: sample 0 drawn at x = {x}, strip starts at {}",
                win.rect.left()
            );
        }
    }
}

/// B40: nothing is carried from one question to the next; each recording starts at the left.
#[test]
fn the_second_question_also_starts_at_the_left_in_both_layouts() {
    let library =
        ScenarioLibrary::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios"))
            .unwrap();
    let mut quiz = RhythmQuiz::new(11);
    let style = StripStyle::default();
    // a narrow cell of the 12-lead page and the full strip of the two-lead layout
    for width in [300.0_f32, 1200.0] {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 300.0));
        let mut lens = Vec::new();
        for _ in 0..3 {
            quiz.next_question(&library);
            let win = quiz.strip_window(rect, &style).unwrap();
            let x = win.x_of_sample(0).expect("sample 0 on screen");
            assert!(
                (x - win.rect.left()).abs() <= win.px_per_sample,
                "{width} pt: x = {x}"
            );
            assert_eq!(win.sample_at_x(win.rect.left() + 0.01), Some(0));
            lens.push(quiz.rings()[0].pushed());
        }
        assert!(
            lens.iter().all(|&n| n == lens[0]),
            "same length every question: {lens:?}"
        );
    }
}
