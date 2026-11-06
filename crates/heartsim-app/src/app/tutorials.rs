//! The Tutorials tab, the tutorial reader beside every tab, and the scenario timeline editor
//! (brief B15). A child module of `app` so the page actions reach the model's fields while
//! `app.rs` keeps one-line hooks.
//!
//! Page actions go through [`TutorialHost`]: scenario [`Action`]s take the same path as timeline
//! entries (the timing engine at the current instant), so any intervention the engines support is
//! available to tutorials without a change here.

use super::{HeartSimApp, Tab};
use eframe::egui;
use heartsim_core::Action;
use heartsim_render::CameraPreset;
use heartsim_sim::Engine as _;
use heartsim_ui::t;
use heartsim_ui::{CameraCommand, StripWindow, TimelineEditor, highlight, timeline_editor};
#[cfg(feature = "native")]
use std::path::PathBuf;

use crate::tutorial::{CameraView, Panel, TutorialHost, UiAction};

/// Timeline-editor state: which library scenario it edits and where it saves.
#[derive(Debug, Default)]
pub(super) struct TimelineTab {
    /// Library index of the scenario being edited.
    editing: Option<usize>,
    editor: TimelineEditor,
    #[cfg(feature = "native")]
    save_path: String,
    status: Option<String>,
    /// Show the editor instead of the tutorial list.
    pub(super) open: bool,
}

impl TutorialHost for HeartSimApp {
    fn apply_action(&mut self, action: &Action) {
        // the therapy path: re-entry engine if one runs, timing engine, then the 3-D heart (a
        // coronary lesion is spatial), so the strip and the colour map follow the same action
        self.apply_therapy(action);
    }

    fn apply_ui(&mut self, action: &UiAction) {
        match action {
            UiAction::LoadScenario(id) => {
                if let Some(i) = self.library.index_of(id) {
                    self.load_scenario(i);
                } else {
                    tracing::warn!("tutorial: no scenario {id}");
                }
            }
            UiAction::StartReentry(id) => self.start_reentry_by_id(id),
            UiAction::FocusPanel(p) => {
                self.tab = match p {
                    Panel::Simulator => Tab::Simulator,
                    Panel::TwelveLead => Tab::TwelveLead,
                    Panel::HeartVector => Tab::HeartVector,
                    Panel::AxisTrainer => Tab::AxisTrainer,
                    Panel::AxisQuiz => Tab::Quiz,
                    Panel::Electrodes => Tab::Electrodes,
                    Panel::RhythmQuiz => Tab::RhythmQuiz,
                    Panel::Tutorials => Tab::Tutorials,
                };
            }
            UiAction::Highlight(tool) => self.tutorials.highlight = Some(tool.clone()),
            UiAction::CameraPreset(v) => {
                let c = match v {
                    CameraView::Ap => CameraCommand::Preset(CameraPreset::Ap),
                    CameraView::Pa => CameraCommand::Preset(CameraPreset::Pa),
                    CameraView::Lao => CameraCommand::Preset(CameraPreset::Lao),
                    CameraView::Rao => CameraCommand::Preset(CameraPreset::Rao),
                    CameraView::Left => CameraCommand::Preset(CameraPreset::Left),
                    CameraView::Cranial => CameraCommand::Preset(CameraPreset::Cranial),
                    CameraView::Caudal => CameraCommand::Preset(CameraPreset::Caudal),
                    CameraView::Frame => CameraCommand::Frame,
                };
                if let Some(h) = &mut self.heart {
                    h.command_camera(c);
                }
            }
            UiAction::SetSpeed(s) => self.clock.speed = s.clamp(0.05, 2.0),
            UiAction::Pause => {
                if !self.ecg.is_paused() {
                    self.ecg.toggle_pause();
                }
            }
            UiAction::Resume => {
                if self.ecg.is_paused() {
                    self.ecg.toggle_pause();
                }
            }
            UiAction::SetQrsAxis(deg) => self.set_qrs_axis(*deg),
            UiAction::SelectLead(l) => self.selected_lead = *l,
            // handled by the page runner
            UiAction::WaitForEvent(_) => {}
        }
    }

    fn sim_now_ms(&self) -> f64 {
        self.engine.now().0
    }

    fn last_qrs_ms(&self) -> f64 {
        self.last_v_activation.0
    }
}

impl HeartSimApp {
    /// Once per frame, before the tab: run the open page's due actions and show the reader as a
    /// right-hand panel (on every tab, so a page can bring another panel forward).
    pub(super) fn tutorial_frame(&mut self, ui: &mut egui::Ui) {
        if let Some(mut run) = self.tutorials.take_run() {
            run.poll(self);
            self.tutorials.restore_run(run);
        }
        if let Some(mut run) = self.cases.take_run() {
            run.poll(self);
            self.cases.restore_run(run);
        }
        if self.tutorials.is_open() {
            egui::Panel::right("tutorial_reader")
                .resizable(true)
                .default_size(360.0)
                .show(ui, |ui| self.tutorials.reader_ui(ui));
        } else if self.cases.is_open() {
            crate::tutorial_ui::publish_highlight(ui.ctx(), None);
            egui::Panel::right("case_reader")
                .resizable(true)
                .default_size(360.0)
                .show(ui, |ui| self.cases.reader_ui(ui));
        } else {
            crate::tutorial_ui::publish_highlight(ui.ctx(), None);
        }
    }

    /// The Tutorials tab: the tutorial list, or the scenario timeline editor.
    pub(super) fn tutorials_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("tutorials_mode").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.timeline_tab.open, false, t!("tab-tutorials"));
                let r = ui.selectable_value(&mut self.timeline_tab.open, true, t!("timeline-editor-mode"));
                highlight::mark(&r, "timeline_editor");
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.timeline_tab.open {
                    self.timeline_editor_ui(ui);
                } else {
                    self.tutorials.library_ui(ui);
                    if self.tutorials.is_open() {
                        self.cases.close();
                    }
                    ui.add_space(12.0);
                    ui.separator();
                    self.cases.library_ui(ui);
                    if self.cases.is_open() {
                        self.tutorials.close();
                    }
                }
            });
        });
    }

    fn timeline_editor_ui(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.scenario else {
            ui.label(t!("timeline-load-first"));
            return;
        };
        let tt = &mut self.timeline_tab;
        if tt.editing != Some(index) {
            let s = &self.library.entries[index].scenario;
            tt.editor = TimelineEditor::new(&s.timeline);
            tt.editing = Some(index);
            #[cfg(feature = "native")]
            {
                tt.save_path = crate::scenario_file::user_scenario_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(format!("{}.ron", s.id))
                    .display()
                    .to_string();
            }
            tt.status = None;
        }
        ui.heading(&self.library.entries[index].scenario.title);
        ui.label(
            "Interventions applied while the scenario runs. Each action is written in the scenario-file \
             syntax; the kind list under the table gives a starting point.",
        );
        let now = self.engine.now().0;
        if tt.editor.ui(ui, Some(now)) {
            tt.status = None;
        }
        ui.separator();
        let timeline = tt.editor.timeline();
        if timeline.is_none() {
            ui.colored_label(egui::Color32::LIGHT_RED, t!("timeline-row-error"));
        }
        let mut restart = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(timeline.is_some(), egui::Button::new(t!("timeline-apply")))
                .on_hover_text(t!("timeline-apply-hint"))
                .clicked()
                && let Some(t) = &timeline
            {
                self.library.entries[index].scenario.timeline.clone_from(t);
                restart = true;
            }
            if ui.button(t!("timeline-revert")).clicked() {
                tt.editing = None;
            }
        });
        #[cfg(feature = "native")]
        ui.horizontal(|ui| {
            ui.label(t!("timeline-file"));
            ui.add(egui::TextEdit::singleline(&mut tt.save_path).desired_width(420.0));
            if ui
                .add_enabled(timeline.is_some(), egui::Button::new(t!("timeline-save")))
                .clicked()
                && let Some(t) = timeline
            {
                let mut s = self.library.entries[index].clone();
                s.scenario.timeline = t;
                tt.status = Some(match save_scenario(&s, &PathBuf::from(&tt.save_path)) {
                    Ok(()) => t!("export-saved", path = tt.save_path.clone()),
                    Err(e) => t!("timeline-not-saved", error = e.to_string()),
                });
            }
        });
        #[cfg(not(feature = "native"))]
        ui.weak(t!("timeline-save-desktop-only"));
        if let Some(s) = &tt.status {
            ui.weak(s);
        }
        if restart {
            self.load_scenario(index);
        }
    }

    /// The running scenario's timeline actions on the strip.
    pub(super) fn draw_timeline_marks(&self, painter: &egui::Painter, window: &StripWindow) {
        let Some(s) = self.scenario.and_then(|i| self.library.entries.get(i)) else {
            return;
        };
        let newest = self.rings[0].pushed().saturating_sub(1);
        timeline_editor::draw_strip_markers(
            painter,
            window,
            &s.scenario.timeline,
            self.engine.now().0,
            newest,
        );
    }
}

#[cfg(feature = "native")]
fn save_scenario(s: &heartsim_ecg::Tier0Scenario, path: &std::path::Path) -> anyhow::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, crate::scenario_file::to_ron(s)?)?;
    Ok(())
}
