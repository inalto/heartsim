//! The EP Lab tab (brief B22): procedures from `assets/scenarios/ep/`, run by
//! [`heartsim_sim::ep::EpLab`].
//!
//! A procedure replaces the Tier-0 pipeline as a re-entry scenario does: the lab's engine owns the
//! heart, the strip rows are the forward model's leads of its cells, the electrograms are the local
//! potentials of the same cells, and the 3-D heart shows them. So a stimulus seen on the HRA
//! electrogram, the beat on lead II and the colour wave cannot disagree.

use super::reentry::{FRAME_BUDGET, Runtime, Stopwatch};
use super::{HeartSimApp, SAMPLE_RATE_HZ};
use crate::assets::AssetBundle;
#[cfg(feature = "native")]
use crate::scenarios::default_scenario_dir;
use crate::scenarios::Timeline;
use eframe::egui;
use glam::Vec3;
use heartsim_core::{Action, Lead, Millis};
use heartsim_ecg::{EgmOptions, EgmRecorder, LeadRing};
use heartsim_geometry::{Catheter, CatheterAnatomy, CatheterKind, CatheterSite};
use heartsim_sim::Engine;
use heartsim_sim::ep::{EpGoal, EpLab, EpPreset, RfParams};
use heartsim_sim::tier2::Tier;
use heartsim_ui::ep_panel::{
    EpCommand, EpPanelState, SurfaceRow, ablation_panel, catheter_glyphs, catheter_panel, egm_controls,
    egm_stack, interval_panel, pes_panel,
};
use heartsim_ui::i18n::number;
use heartsim_ui::t;
use std::sync::Arc;

/// Surface leads above the electrograms, as on an EP recording system.
const SURFACE_LEADS: [Lead; 3] = [Lead::I, Lead::AVF, Lead::V1];

/// A procedure in progress.
#[derive(Debug)]
struct Active {
    index: usize,
    lab: EpLab,
    timeline: Timeline,
    anatomy: CatheterAnatomy,
    catheters: Vec<Catheter>,
    recorder: EgmRecorder,
    speed: f64,
}

/// The tab's state.
#[derive(Debug, Default)]
pub(super) struct EpLabTab {
    presets: Vec<EpPreset>,
    runtime: Option<Runtime>,
    active: Option<Active>,
    panel: EpPanelState,
    status: Option<String>,
    bundle: Option<AssetBundle>,
}

impl EpLabTab {
    /// Every procedure of `assets/scenarios/ep/` that parses (bad files are logged and skipped).
    pub(super) fn load_default() -> Self {
        #[cfg(feature = "native")]
        {
            let mut presets = Vec::new();
            if let Some(dir) = default_scenario_dir().map(|d| d.join("ep")) {
                presets = load_dir(&dir);
            }
            Self {
                presets,
                ..Self::default()
            }
        }
        #[cfg(not(feature = "native"))]
        {
            Self::default()
        }
    }

    /// Every procedure of `scenarios/ep/` in a downloaded bundle; its anatomy pack is kept for the
    /// first time a procedure starts.
    pub(super) fn from_bundle(bundle: &AssetBundle) -> Self {
        let mut presets: Vec<EpPreset> = bundle
            .files_in("scenarios/ep", ".ron")
            .filter_map(|p| {
                EpPreset::from_ron(&bundle.text(p)?)
                    .inspect_err(|e| tracing::warn!("{p}: {e}"))
                    .ok()
                    .flatten()
            })
            .collect();
        presets.sort_by(|a, b| a.scenario.id.cmp(&b.scenario.id));
        Self {
            presets,
            bundle: Some(bundle.subset(&[crate::assets::PACK_DIR])),
            ..Self::default()
        }
    }
}

#[cfg(feature = "native")]
fn load_dir(dir: &std::path::Path) -> Vec<EpPreset> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<_> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .collect();
    files.sort();
    let mut presets: Vec<EpPreset> = files
        .iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(p)
                .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                .ok()?;
            EpPreset::from_ron(&text)
                .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                .ok()
                .flatten()
        })
        .collect();
    presets.sort_by(|a, b| a.scenario.id.cmp(&b.scenario.id));
    presets
}

impl HeartSimApp {
    /// A procedure is running.
    pub(super) fn ep_active(&self) -> bool {
        self.ep.active.is_some()
    }

    /// Start procedure `index` from t = 0: standard catheters, empty recording, no lesions.
    fn start_ep(&mut self, index: usize) {
        self.stop_reentry();
        if self.ep.runtime.is_none() {
            match Runtime::load(self.ep.bundle.as_ref()) {
                Ok(rt) => self.ep.runtime = Some(rt),
                Err(e) => {
                    tracing::warn!("EP lab: {e:#}");
                    self.ep.status = Some(format!("{e:#}"));
                    return;
                }
            }
        }
        let (Some(rt), Some(preset)) = (&self.ep.runtime, self.ep.presets.get(index)) else {
            return;
        };
        let lab = preset.build(Arc::clone(&rt.cells), &rt.sets, &rt.aha);
        let anatomy = CatheterAnatomy::from_cloud(&rt.cells);
        let catheters = anatomy.standard_set();
        let recorder = EgmRecorder::new(&rt.cells, &catheters, EgmOptions::default());
        let speed = self.clock.speed;
        self.clock = heartsim_core::SimClock::new(Millis(1.0));
        self.clock.speed = speed;
        self.engine = lab.timing().clone();
        self.sample_accumulator = 0.0;
        self.rings.iter_mut().for_each(LeadRing::clear);
        self.last_v_activation = Millis(-1000.0);
        self.scenario = None;
        self.timeline = Timeline::default();
        if self.ecg.is_paused() {
            self.ecg.toggle_pause();
        }
        if let Some(h) = &mut self.heart {
            h.retime(&preset.timing);
            h.set_external_cells(Some(lab.cells()));
            h.set_pick_mode(true);
        }
        self.ep.panel.caliper = None;
        self.ep.status = None;
        self.ep.active = Some(Active {
            index,
            lab,
            timeline: Timeline::new(preset.timeline()),
            anatomy,
            catheters,
            recorder,
            speed: 1.0,
        });
    }

    /// Leave the procedure: the 3-D heart goes back to its own engine.
    pub(super) fn stop_ep(&mut self) {
        if self.ep.active.take().is_some()
            && let Some(h) = &mut self.heart
        {
            h.set_external_cells(None);
            h.set_pick_mode(false);
            h.set_catheter_glyphs(Vec::new(), Vec::new());
        }
    }

    /// Run the steps of this frame: timeline actions, one electrogram sample per step, one
    /// forward-model sample per strip sample.
    pub(super) fn step_ep(&mut self, steps: u32) {
        let (Some(mut act), Some(rt)) = (self.ep.active.take(), self.ep.runtime.as_ref()) else {
            return;
        };
        let dt = self.clock.dt;
        let period = 1000.0 / f64::from(SAMPLE_RATE_HZ);
        let started = Stopwatch::start();
        let t0 = act.lab.now().0;
        for _ in 0..steps {
            let now = act.lab.now();
            let due: Vec<(Action, Millis)> = act.timeline.due(now).map(|a| (a.action.clone(), a.at)).collect();
            for (a, at) in due {
                if let Err(e) = act.lab.apply(&a, at) {
                    tracing::warn!("EP lab action at {:.0} ms: {e}", at.0);
                }
            }
            act.lab.step(dt);
            for s in act.lab.take_stimuli() {
                act.recorder.mark_stimulus(s.at.0);
            }
            if let Some(e) = act.lab.take_error() {
                tracing::warn!("EP lab: {e}");
            }
            let events = act.lab.drain_events();
            for e in &events {
                if e.region.is_ventricular() && e.kind != heartsim_sim::EventKind::Blocked {
                    self.last_v_activation = e.at;
                }
            }
            if let Some(h) = &mut self.heart {
                h.on_step(&events, dt, act.lab.now());
            }
            act.recorder.push(&act.lab.cells().vm, act.lab.now().0);
            self.sample_accumulator += dt.0;
            while self.sample_accumulator >= period {
                self.sample_accumulator -= period;
                let s = rt.forward.sample(&act.lab.cells().vm).to_leads();
                for (ring, v) in self.rings.iter_mut().zip(s.0) {
                    ring.push(v as f32);
                }
            }
            if started.elapsed() > FRAME_BUDGET {
                break;
            }
        }
        let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        if wall_ms > 0.0 {
            act.speed = 0.9 * act.speed + 0.1 * (act.lab.now().0 - t0) / wall_ms;
        }
        self.engine = act.lab.timing().clone();
        if let Some(h) = &mut self.heart {
            h.set_external_cells(Some(act.lab.cells()));
        }
        self.record_heart_history();
        self.ep.active = Some(act);
    }

    /// Give a therapy-window action to the lab. `false` when no procedure runs.
    pub(super) fn ep_apply(&mut self, action: &Action) -> bool {
        let Some(act) = &mut self.ep.active else {
            return false;
        };
        let now = act.lab.now();
        if let Err(e) = act.lab.apply(action, now) {
            tracing::warn!("therapy action: {e}");
        }
        true
    }

    /// Shocks the lab's engine delivered since the last call (`None` outside a procedure).
    pub(super) fn ep_shocks(&mut self) -> Option<Vec<heartsim_sim::defib::ShockRecord>> {
        self.ep.active.as_mut().map(|a| a.lab.engine_mut().drain_shocks())
    }

    fn apply_ep_commands(&mut self, commands: Vec<EpCommand>) {
        let tau_ms = self.ep.panel.rf_tau_s * 1000.0;
        let power_w = self.ep.panel.power_w;
        let Some(act) = &mut self.ep.active else {
            return;
        };
        *act.lab.rf_params_mut() = RfParams {
            power_w,
            tau_ms,
            ..*act.lab.rf_params_mut()
        };
        let mut moved = false;
        for command in commands {
            match command {
                EpCommand::MoveCatheter { catheter, site } => {
                    if let Some(c) = act.catheters.get_mut(catheter) {
                        *c = act.anatomy.place(c.kind, &site);
                        moved = true;
                    }
                }
                EpCommand::StartProtocol { channel, protocol } => {
                    match pair_cell(&act.lab, &act.catheters, &act.recorder, channel) {
                        Ok(cell) => {
                            self.ep.status = None;
                            act.lab.start_protocol(cell, protocol);
                        }
                        Err(text) => self.ep.status = Some(text),
                    }
                }
                EpCommand::StopProtocol => act.lab.stop_protocol(),
                EpCommand::PaceOnce { channel } => {
                    match pair_cell(&act.lab, &act.catheters, &act.recorder, channel) {
                        Ok(cell) => {
                            self.ep.status = None;
                            if let Err(e) = act.lab.stimulate(cell) {
                                self.ep.status = Some(e.to_string());
                            }
                        }
                        Err(text) => self.ep.status = Some(text),
                    }
                }
                EpCommand::RfOn => {
                    if let Some(tip) = ablation_tip(&act.catheters) {
                        act.lab.rf_on(tip);
                    }
                }
                EpCommand::RfOff => act.lab.rf_off(),
                EpCommand::GoToTarget(target) => {
                    if let (Some(p), Some(c)) = (
                        act.lab.target_sites(target).first().copied(),
                        act.catheters.iter_mut().find(|c| c.kind == CatheterKind::Abl),
                    ) {
                        *c = act.anatomy.place(CatheterKind::Abl, &CatheterSite::Point(p.to_array()));
                        moved = true;
                    }
                }
                EpCommand::AblateTarget(target) => {
                    act.lab.ablate_target(target);
                }
            }
        }
        if moved {
            act.recorder.set_catheters(&act.catheters);
        }
    }

    /// Procedure picker, tier read-out and leave button.
    fn ep_top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let current = self.ep.active.as_ref().map(|a| a.index);
            let title = current
                .and_then(|i| self.ep.presets.get(i))
                .map_or_else(|| t!("ep-picker"), |p| p.scenario.title.clone());
            let mut picked = None;
            egui::ComboBox::from_id_salt("ep_picker")
                .selected_text(title)
                .width(320.0)
                .show_ui(ui, |ui| {
                    for (i, p) in self.ep.presets.iter().enumerate() {
                        if ui
                            .selectable_label(current == Some(i), &p.scenario.title)
                            .on_hover_text(&p.scenario.description)
                            .clicked()
                        {
                            picked = Some(i);
                        }
                    }
                });
            if let Some(i) = picked {
                self.start_ep(i);
            }
            if let Some(i) = current {
                if ui.button(t!("ep-restart")).clicked() {
                    self.start_ep(i);
                }
                if ui.button(t!("ep-stop-procedure")).clicked() {
                    self.stop_ep();
                }
            }
            if self.ep_active() {
                let paused = self.ecg.is_paused();
                if ui
                    .button(if paused { t!("ep-resume") } else { t!("ep-pause") })
                    .clicked()
                {
                    self.ecg.toggle_pause();
                }
            }
            if let Some(act) = &self.ep.active {
                let tier = match act.lab.engine().tier() {
                    Tier::One => t!("ep-status-tier-1"),
                    Tier::Two => t!("ep-status-tier-2"),
                };
                ui.label(t!("ep-speed", tier = tier, speed = number(act.speed, 2)));
            }
            if let Some(s) = &self.ep.status {
                ui.colored_label(egui::Color32::LIGHT_RED, s);
            }
        });
    }

    /// The tab: controls on the left, heart in the middle, electrograms on the right.
    pub(super) fn ep_lab_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("ep_top").show(ui, |ui| self.ep_top_bar(ui));
        if !self.ep_active() {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.label(t!("ep-intro"));
            });
            return;
        }
        let mut commands = Vec::new();
        let picked = self.heart.as_mut().and_then(crate::heart_view::HeartView::take_pick);
        let Some(act) = &mut self.ep.active else {
            return;
        };
        if let Some(p) = picked {
            let abl = act.catheters.iter().position(|c| c.kind == CatheterKind::Abl);
            commands.extend(abl.map(|catheter| EpCommand::MoveCatheter {
                catheter,
                site: CatheterSite::Point(p.to_array()),
            }));
        }
        let preset = &self.ep.presets[act.index];
        let channels: Vec<String> = act.recorder.channels().iter().map(|c| c.label.clone()).collect();
        let landmarks: Vec<String> = act.lab.cloud().landmarks.keys().cloned().collect();
        let panel = &mut self.ep.panel;
        egui::Panel::left("ep_controls").default_size(320.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                catheter_panel(
                    ui,
                    &act.catheters,
                    &landmarks,
                    act.anatomy.cs_length_mm(),
                    &mut commands,
                );
                ui.separator();
                let status = act.lab.protocol_status();
                pes_panel(
                    ui,
                    panel,
                    &channels,
                    status,
                    act.lab.protocol_outcome(),
                    act.lab.engine().tier() == Tier::Two,
                    &mut commands,
                );
                ui.separator();
                ablation_panel(
                    ui,
                    panel,
                    &preset.ep.targets,
                    act.lab.rf_readout(),
                    act.lab.lesions().len(),
                    goal_text(act.lab.goal()),
                    act.lab.goal_reached(),
                    &mut commands,
                );
            });
        });
        let surface: Vec<SurfaceRow<'_>> = SURFACE_LEADS
            .iter()
            .filter_map(|l| {
                let i = Lead::ALL.iter().position(|x| x == l)?;
                Some((l.label(), self.rings.get(i)?))
            })
            .collect();
        let now_ms = act.lab.now().0;
        let style = self.ecg.style;
        egui::Panel::right("ep_egm").default_size(620.0).show(ui, |ui| {
            egm_controls(ui, panel, act.recorder.conduction_intervals());
            interval_panel(ui, &act.recorder, act.lab.protocol_outcome());
            egm_stack(ui, panel, &surface, &act.recorder, &style, now_ms);
        });
        // catheters and lesions on the heart
        let pace_midpoint = act
            .recorder
            .channels()
            .get(panel.pace_channel)
            .and_then(|c| act.catheters.get(c.catheter).map(|k| k.pair_midpoint(c.pair)));
        let (spheres, tubes) = catheter_glyphs(&act.catheters, act.lab.lesions().lesions(), pace_midpoint);
        if let Some(h) = &mut self.heart {
            h.set_catheter_glyphs(spheres, tubes);
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(h) = &mut self.heart {
                h.viewport(ui, &mut self.engine);
            }
        });
        self.apply_ep_commands(commands);
    }
}

/// The cell pair `channel` would stimulate, or the message to show.
fn pair_cell(
    lab: &EpLab,
    catheters: &[Catheter],
    recorder: &EgmRecorder,
    channel: usize,
) -> Result<u32, String> {
    let ch = recorder.channels().get(channel).ok_or_else(String::new)?;
    let at: Vec3 = catheters
        .get(ch.catheter)
        .map_or(Vec3::ZERO, |c| c.pair_midpoint(ch.pair));
    lab.capture_cell(at)
        .ok_or_else(|| t!("ep-no-tissue", pair = ch.label.clone()))
}

fn ablation_tip(catheters: &[Catheter]) -> Option<Vec3> {
    catheters.iter().find(|c| c.kind == CatheterKind::Abl).map(Catheter::tip)
}

fn goal_text(goal: EpGoal) -> Option<String> {
    Some(match goal {
        EpGoal::Study => t!("ep-goal-study"),
        EpGoal::TerminateReentry => t!("ep-goal-terminate"),
        EpGoal::SlowPathwayModified => t!("ep-goal-slow-pathway"),
        EpGoal::PathwayAblated => t!("ep-goal-pathway"),
        EpGoal::VeinsIsolated => t!("ep-goal-veins"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "native")]
    #[test]
    fn the_shipped_procedures_load_and_have_a_goal() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios/ep");
        let presets = load_dir(&dir);
        let ids: Vec<&str> = presets.iter().map(|p| p.scenario.id.as_str()).collect();
        for id in ["ep_baseline", "ep_avnrt", "ep_ap_left_lateral", "ep_af_pvi", "ep_flutter"] {
            assert!(ids.contains(&id), "{id} missing from {ids:?}");
        }
        for p in presets.iter().filter(|p| p.ep.goal != EpGoal::Study) {
            assert!(!p.ep.targets.is_empty(), "{} has nothing to ablate", p.scenario.id);
        }
    }

    /// The web build offers the procedures the desktop build offers.
    #[cfg(feature = "native")]
    #[test]
    fn bundle_procedures_match_the_disk_procedures() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let disk = load_dir(&root.join("scenarios/ep"));
        let bundle = AssetBundle::from_dir(&root).expect("assets dir");
        let mem = EpLabTab::from_bundle(&bundle);
        let ids = |p: &[EpPreset]| p.iter().map(|p| p.scenario.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&mem.presets), ids(&disk));
    }
}
