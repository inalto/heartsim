//! B15: the timeline editor's add / delete through AccessKit nodes, and the strip-time mapping.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_core::{Action, Millis, TimedAction};
use heartsim_ui::TimelineEditor;
use heartsim_ui::timeline_editor::{action_kind, sample_of_time, templates};

#[test]
fn add_and_delete_rows() {
    let start = vec![TimedAction {
        at: Millis(1000.0),
        action: Action::SetGlobalSinusRate { bpm: 80.0 },
    }];
    let mut h = Harness::builder()
        .with_size(egui::vec2(900.0, 400.0))
        .build_ui_state(
            |ui, ed: &mut TimelineEditor| {
                ed.ui(ui, Some(0.0));
            },
            TimelineEditor::new(&start),
        );
    h.run();
    let _ = h.get_by_label("SetGlobalSinusRate");
    h.get_by_label("Add").click();
    h.run();
    assert_eq!(h.state().len(), 2);
    assert!(h.state().is_valid());
    // the new row (template 0 at 0 ms) sorts first; delete it
    h.get_all_by_label("Delete").next().unwrap().click();
    h.run();
    assert_eq!(h.state().timeline().unwrap(), start);
}

#[test]
fn kinds_are_variant_names() {
    let kinds: Vec<String> = templates().iter().map(action_kind).collect();
    assert!(kinds.contains(&"Shock".to_owned()));
    assert!(kinds.contains(&"PacingTrain".to_owned()));
    assert!(
        kinds
            .iter()
            .all(|k| !k.is_empty() && k.chars().all(char::is_alphanumeric))
    );
}

#[test]
fn strip_time_mapping() {
    // newest sample 999 taken at 2000 ms, 500 Hz: 1000 ms earlier is 500 samples back
    assert_eq!(sample_of_time(1000.0, 2000.0, 999, 500.0), Some(499));
    assert_eq!(sample_of_time(2000.0, 2000.0, 999, 500.0), Some(999));
    assert_eq!(sample_of_time(-10.0, 2000.0, 999, 500.0), None);
}
