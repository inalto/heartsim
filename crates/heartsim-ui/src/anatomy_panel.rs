//! Anatomy panel: which structures are drawn and how, the cut-away plane and the camera. The
//! widget only edits [`AnatomyPanelState`] (and returns one-shot camera commands); the app copies
//! the state into the renderer's layers each frame, so the panel is testable without a GPU.

use crate::t;
use egui::Ui;
use heartsim_render::CameraPreset;

/// Display state of one scene layer as the user sets it.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerToggle {
    /// Layer name shown in the list (`lv`, `aorta`, `torso`, …).
    pub name: String,
    /// Draw the layer at all.
    pub visible: bool,
    /// Opacity 0–1; below 1 the structures behind show through (e.g. the torso over the heart).
    pub alpha: f32,
}

/// Everything the anatomy panel edits.
#[derive(Debug, Clone, PartialEq)]
// The layer, clipping and label switches are independent toggles, not a state machine.
#[allow(clippy::struct_excessive_bools)]
pub struct AnatomyPanelState {
    /// One entry per scene layer, in scene order.
    pub layers: Vec<LayerToggle>,
    /// Cut the heart with a plane facing the camera, to see the septum and the endocardium.
    pub clip_enabled: bool,
    /// Position of the cut plane: −1 (behind the heart, nothing removed) … +1 (in front, all
    /// removed); 0 cuts through the centre of the heart.
    pub clip_depth: f32,
    /// Show anatomical labels.
    pub labels: bool,
    /// Last value of the oblique slider, degrees: −90 = LAO 90 (left lateral), 0 = AP,
    /// +90 = RAO 90.
    pub lao_rao_deg: f32,
    /// Spin the camera slowly around the cranio-caudal axis.
    pub auto_rotate: bool,
    /// Reduced motion (B19 settings): the automatic rotation cannot run, so its checkbox is off and disabled.
    pub reduced_motion: bool,
}

impl AnatomyPanelState {
    /// All layers visible and opaque, no cut, labels off.
    #[must_use]
    pub fn new(layers: impl IntoIterator<Item = (String, f32)>) -> Self {
        Self {
            layers: layers
                .into_iter()
                .map(|(name, alpha)| LayerToggle {
                    name,
                    visible: true,
                    alpha,
                })
                .collect(),
            clip_enabled: false,
            clip_depth: 0.0,
            labels: false,
            lao_rao_deg: 0.0,
            auto_rotate: false,
            reduced_motion: false,
        }
    }

    /// The toggle of the layer called `name`, if any.
    #[must_use]
    pub fn layer(&self, name: &str) -> Option<&LayerToggle> {
        self.layers.iter().find(|l| l.name == name)
    }
}

/// A one-shot camera change requested by the panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraCommand {
    /// Jump to a standard view.
    Preset(CameraPreset),
    /// Turn to this angle on the LAO (−) … RAO (+) axis, degrees, level with the heart.
    Oblique(f32),
    /// Re-frame the camera on the visible layers.
    Frame,
}

/// Draw the panel. Returns the camera change the user asked for this frame, if any.
pub fn anatomy_panel(ui: &mut Ui, state: &mut AnatomyPanelState) -> Option<CameraCommand> {
    let mut command = None;
    ui.strong(t!("anatomy-layers"));
    egui::Grid::new("anatomy_layers").num_columns(2).show(ui, |ui| {
        for l in &mut state.layers {
            ui.checkbox(&mut l.visible, l.name.as_str());
            ui.add_enabled(
                l.visible,
                egui::Slider::new(&mut l.alpha, 0.0..=1.0)
                    .show_value(false)
                    .text(t!("anatomy-opacity")),
            );
            ui.end_row();
        }
    });
    ui.checkbox(&mut state.labels, t!("anatomy-labels"));
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.clip_enabled, t!("anatomy-cut-away"));
        ui.add_enabled(
            state.clip_enabled,
            egui::Slider::new(&mut state.clip_depth, -1.0..=1.0)
                .show_value(false)
                .text(t!("anatomy-depth")),
        );
    });
    ui.separator();
    ui.strong(t!("anatomy-view"));
    ui.horizontal_wrapped(|ui| {
        for p in CameraPreset::ALL {
            if ui.button(p.label()).clicked() {
                command = Some(CameraCommand::Preset(p));
            }
        }
        if ui.button(t!("anatomy-fit")).clicked() {
            command = Some(CameraCommand::Frame);
        }
    });
    if ui
        .add(
            egui::Slider::new(&mut state.lao_rao_deg, -90.0..=90.0)
                .step_by(5.0)
                .custom_formatter(|v, _| match v {
                    v if v < -0.5 => format!("LAO {:.0}°", -v),
                    v if v > 0.5 => format!("RAO {v:.0}°"),
                    _ => "AP".to_string(),
                })
                .text(t!("anatomy-oblique")),
        )
        .changed()
    {
        command = Some(CameraCommand::Oblique(state.lao_rao_deg));
    }
    ui.add_enabled_ui(!state.reduced_motion, |ui| {
        ui.checkbox(&mut state.auto_rotate, t!("anatomy-auto-rotate"));
    });
    command
}
