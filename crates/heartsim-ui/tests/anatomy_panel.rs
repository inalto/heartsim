//! B9 acceptance: the anatomy panel's toggles exist and flip the layer flags in the model.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_render::CameraPreset;
use heartsim_ui::{AnatomyPanelState, CameraCommand, anatomy_panel};

struct Model {
    panel: AnatomyPanelState,
    last: Option<CameraCommand>,
}

fn harness() -> Harness<'static, Model> {
    let panel = AnatomyPanelState::new(["lv", "rv", "aorta", "torso"].map(|n| (n.to_string(), 1.0)));
    Harness::new_ui_state(
        |ui, m: &mut Model| {
            if let Some(c) = anatomy_panel(ui, &mut m.panel) {
                m.last = Some(c);
            }
        },
        Model { panel, last: None },
    )
}

#[test]
fn layer_toggles_flip_visibility_flags() {
    let mut h = harness();
    for name in ["lv", "rv", "aorta", "torso"] {
        assert!(h.state().panel.layer(name).unwrap().visible);
    }
    h.get_by_label("rv").click();
    h.run();
    assert!(!h.state().panel.layer("rv").unwrap().visible, "rv hidden");
    assert!(
        h.state().panel.layer("lv").unwrap().visible,
        "other layers untouched"
    );
    h.get_by_label("rv").click();
    h.run();
    assert!(h.state().panel.layer("rv").unwrap().visible, "rv shown again");
}

#[test]
fn cut_away_labels_and_auto_rotate_toggles_exist() {
    let mut h = harness();
    for label in ["Cut-away", "Labels", "Auto-rotate"] {
        h.get_by_label(label).click();
    }
    h.run();
    let p = &h.state().panel;
    assert!(p.clip_enabled && p.labels && p.auto_rotate);
}

#[test]
fn preset_buttons_issue_camera_commands() {
    let mut h = harness();
    for p in CameraPreset::ALL {
        h.get_by_label(p.label()).click();
        h.run();
        assert_eq!(h.state().last, Some(CameraCommand::Preset(p)));
    }
}
