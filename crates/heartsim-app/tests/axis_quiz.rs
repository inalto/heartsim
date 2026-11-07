//! B8 acceptance: the axis drill flow (new question → answer → feedback with explanation → next)
//! is reachable through AccessKit nodes, and every question respects the guard bands. No GPU.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::axis_quiz::{AxisQuiz, TOLERANCE_DEG};
use heartsim_core::axis::drill_axis_allowed;
use heartsim_ui::i18n;

struct Model {
    quiz: AxisQuiz,
    /// Axes the app would dial into the synthesiser.
    applied: Vec<f64>,
}

fn harness() -> Harness<'static, Model> {
    i18n::set_language("en");
    Harness::builder()
        .with_size(egui::vec2(900.0, 900.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                egui::Panel::left("quizdial").default_size(430.0).show(ui, |ui| {
                    if let Some(t) = m.quiz.panel(ui) {
                        m.applied.push(t);
                    }
                });
            },
            Model {
                quiz: AxisQuiz::new(5),
                applied: Vec::new(),
            },
        )
}

#[test]
fn drill_flow_new_question_answer_explanation_next() {
    let mut h = harness();
    h.run();
    let _ = h.get_by_label_contains("Press New question");

    h.get_by_label("New question").click();
    h.run();
    let target = h.state().quiz.target_deg.expect("a hidden axis");
    assert_eq!(
        h.state().applied,
        vec![target],
        "the app is told to dial the axis in"
    );
    assert!(drill_axis_allowed(target), "{target}");
    let _ = h.get_by_label_contains("Click the circle");

    // a correct answer: score, colour line and the explanation
    h.state_mut().quiz.set_guess(target + TOLERANCE_DEG - 2.0);
    h.run();
    h.get_by_label("Check answer").click();
    h.run();
    assert_eq!((h.state().quiz.asked, h.state().quiz.correct), (1, 1));
    let _ = h.get_by_label_contains("you chose");
    let _ = h.get_by_label_contains("net QRS vector");
    let _ = h.get_by_label("Score 1/1");

    // next question clears the explanation; a wrong answer scores nothing and resets the streak
    h.get_by_label("New question").click();
    h.run();
    assert!(h.query_by_label_contains("net QRS vector").is_none());
    assert_eq!(h.state().applied.len(), 2);
    let target = h.state().quiz.target_deg.unwrap();
    h.state_mut().quiz.set_guess(target + 90.0);
    h.run();
    h.get_by_label("Check answer").click();
    h.run();
    assert_eq!(
        (
            h.state().quiz.asked,
            h.state().quiz.correct,
            h.state().quiz.streak
        ),
        (2, 1, 0)
    );
    let _ = h.get_by_label_contains("net QRS vector");
}

#[test]
fn the_explanation_is_available_in_italian() {
    let mut h = harness();
    h.run();
    h.get_by_label("New question").click();
    h.run();
    let target = h.state().quiz.target_deg.unwrap();
    h.state_mut().quiz.set_guess(target);
    h.run();
    i18n::set_language("it");
    h.run();
    h.get_by_label("Verifica risposta").click();
    h.run();
    let _ = h.get_by_label_contains("vettore QRS netto");
    i18n::set_language("en");
}
