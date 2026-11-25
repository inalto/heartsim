//! B8 acceptance: the interval drill (measure PR / QRS / QT with calipers, tolerance grading) is
//! reachable through AccessKit nodes. No GPU.

use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use heartsim_app::interval_drill::IntervalDrill;
use heartsim_app::scenarios::ScenarioLibrary;
use heartsim_ui::{StripStyle, i18n};
use std::path::Path;

struct Model {
    drill: IntervalDrill,
    library: ScenarioLibrary,
}

fn harness() -> Harness<'static, Model> {
    i18n::set_language("en");
    let library =
        ScenarioLibrary::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios"))
            .unwrap();
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(1400.0, 800.0))
        .build_ui_state(
            |ui, m: &mut Model| m.drill.ui(ui, &m.library, &StripStyle::default()),
            Model {
                drill: IntervalDrill::new(11),
                library,
            },
        )
}

#[test]
fn measure_with_calipers_then_get_graded_with_an_explanation() {
    let mut h = harness();
    h.run();
    let _ = h.get_by_label_contains("Press New question");

    h.get_by_label("New question").click();
    h.run();
    let q = h.state().drill.question.clone().expect("a question");
    let _ = h.get_by_label_contains("Measure the");
    let _ = h.get_by_label_contains("Place the calipers");
    // the rhythm is not named before the answer
    assert!(h.query_by_label_contains("Rhythm:").is_none());

    // calipers: place a pair (200 ms wide), copy their reading into the answer box
    h.get_by_label("Place calipers").click();
    h.run();
    assert_eq!(h.state().drill.calipers.pairs.len(), 1);
    let _ = h.get_by_label("Calipers: 200 ms");
    h.get_by_label("Use caliper reading").click();
    h.run();
    assert!((h.state().drill.entry_ms - 200.0).abs() < 1e-9);

    // a measurement within the tolerance is correct and is explained
    h.state_mut().drill.entry_ms = (q.truth_ms + q.interval.tolerance_ms() - 2.0).round();
    h.run();
    h.get_by_label("Check answer").click();
    h.run();
    assert_eq!((h.state().drill.asked, h.state().drill.correct), (1, 1));
    let _ = h.get_by_label_contains("Correct: you measured");
    let _ = h.get_by_label_contains("Rhythm:");
    let _ = h.get_by_label_contains("The reference is the simulator's own measurement");

    // next question; a measurement outside the tolerance is wrong and says by how much
    h.get_by_label("Next question").click();
    h.run();
    assert!(h.query_by_label_contains("Rhythm:").is_none());
    let q = h.state().drill.question.clone().unwrap();
    h.state_mut().drill.entry_ms = (q.truth_ms + q.interval.tolerance_ms() + 30.0).round();
    h.run();
    h.get_by_label("Check answer").click();
    h.run();
    assert_eq!(
        (
            h.state().drill.asked,
            h.state().drill.correct,
            h.state().drill.streak
        ),
        (2, 1, 0)
    );
    let _ = h.get_by_label_contains("Not quite: you measured");
}

/// The two buttons that are disabled until the learner has done something say what is missing
/// when hovered, and the strip takes the height of the page.
#[test]
fn disabled_buttons_explain_themselves_on_hover() {
    let mut h = harness();
    h.run();
    h.get_by_label("New question").click();
    h.run();

    h.get_by_label("Use caliper reading").hover();
    h.run();
    h.run();
    let _ = h.get_by_label("Place the calipers first");

    h.get_by_label("Check answer").hover();
    h.run();
    h.run();
    let _ = h.get_by_label("Enter a measurement first");
    assert!(
        h.query_by_label("Place the calipers first").is_none(),
        "only the hovered button explains itself"
    );
}

/// The primary actions on the page (by label, in tree order): the rule of use allows one per panel.
fn primary_names<S>(h: &Harness<'_, S>) -> Vec<String> {
    h.query_all(By::new())
        .filter(|n| n.accesskit_node().class_name() == Some(heartsim_ui::PRIMARY_CLASS))
        .map(|n| n.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// One primary action: New question, then Check answer (grey while there is nothing to check, with
/// the next step in words), then Next question; the disabled-reason texts exist in both languages.
#[test]
fn the_interval_drill_has_one_primary_action_and_says_the_next_step_while_it_is_disabled() {
    let mut h = harness();
    h.run();
    assert_eq!(primary_names(&h), ["New question"]);
    h.get_by_label("New question").click();
    h.run();
    assert_eq!(primary_names(&h), ["Check answer"]);
    assert!(
        h.get_by_label("Check answer").accesskit_node().is_disabled(),
        "nothing to check yet"
    );
    let _ = h.get_by_label_contains("Next: press Place calipers");
    h.get_by_label("Place calipers").click();
    h.run();
    let _ = h.get_by_label_contains("Next: press Use caliper reading");
    h.get_by_label("Use caliper reading").click();
    h.run();
    assert!(!h.get_by_label("Check answer").accesskit_node().is_disabled());
    assert!(h.query_by_label_contains("Next: press").is_none());
    h.get_by_label("Check answer").click();
    h.run();
    assert_eq!(primary_names(&h), ["Next question"]);
    for lang in ["en", "it"] {
        i18n::set_language(lang);
        for key in [
            "interval-calipers-use-disabled",
            "interval-check-disabled",
            "interval-next-calipers",
            "interval-next-reading",
            "interval-def-summary",
        ] {
            assert_ne!(i18n::tr(key), key, "{lang}: {key}");
        }
    }
    i18n::set_language("en");
}
