//! Re-entry presets in the Simulator tab (brief B17): atrial fibrillation, flutter, VT, torsades
//! and VF from `assets/scenarios/reentry/`, run by [`heartsim_sim::tier2::SwitchingEngine`].
//!
//! A re-entry scenario replaces the Tier-0 pipeline while it runs: the switching engine (Tier 1,
//! then Tier 2 when the arrhythmia is initiated, and back to Tier 1 two seconds after it has
//! stopped) is the single owner of the heart. The strip and the 12-lead page are the forward
//! model's potentials of its cell state, the 3-D heart shows the same cells
//! ([`crate::heart_view::HeartView::set_external_cells`]) and the therapy window's drugs and
//! shocks, the click-to-ablate tool and the scenario timeline all become actions of that engine,
//! so the strip, the colour map and the beat timing can never disagree. The app's own
//! [`heartsim_sim::TimingEngine`] is kept as a mirror of the engine's, which is what the therapy
//! window reads (drug timers, ischaemia, armed shock).
//!
//! **Speed.** Tier 2 is not real time at 2 mm voxels and only just on a 4 mm grid; each frame
//! runs as many 1 ms steps as fit in a time budget and the controls show the achieved speed.

use super::{HeartSimApp, SAMPLE_RATE_HZ};
use crate::heart_view::default_pack_dir;
use crate::scenarios::{Timeline, default_scenario_dir};
use eframe::egui;
use heartsim_core::{Action, Millis};
use heartsim_ecg::{ForwardModel, ForwardOptions};
use heartsim_geometry::{AnatomyPack, CellCloud};
use heartsim_sim::defib::ShockRecord;
use heartsim_sim::tier2::{Preset, SwitchingEngine, Tier};
use heartsim_sim::{Engine, EventKind};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Wall-clock budget of the simulation steps of one frame.
const FRAME_BUDGET: Duration = Duration::from_millis(14);

/// What a preset needs from the anatomy pack.
#[derive(Debug)]
struct Runtime {
    cells: Arc<CellCloud>,
    sets: BTreeMap<String, Vec<u32>>,
    aha: Vec<u8>,
    forward: ForwardModel,
}

/// A running re-entry scenario.
#[derive(Debug)]
struct Active {
    index: usize,
    engine: SwitchingEngine,
    timeline: Timeline,
    /// Achieved simulation speed (simulated ms per wall-clock ms), smoothed.
    speed: f64,
}

/// The presets and the running one.
#[derive(Debug, Default)]
pub(super) struct Reentry {
    presets: Vec<Preset>,
    runtime: Option<Runtime>,
    active: Option<Active>,
    status: Option<String>,
}

impl Reentry {
    /// Every preset of `assets/scenarios/reentry/` that parses (bad files are logged and skipped).
    pub(super) fn load_default() -> Self {
        let mut presets = Vec::new();
        if let Some(dir) = default_scenario_dir().map(|d| d.join("reentry")) {
            presets = load_dir(&dir);
        }
        Self {
            presets,
            ..Self::default()
        }
    }
}

fn load_dir(dir: &Path) -> Vec<Preset> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<_> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .collect();
    files.sort();
    let mut presets: Vec<Preset> = files
        .iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(p)
                .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                .ok()?;
            Preset::from_ron(&text)
                .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                .ok()
                .flatten()
        })
        .collect();
    presets.sort_by(|a, b| a.scenario.title.cmp(&b.scenario.title));
    presets
}

impl Runtime {
    fn load() -> anyhow::Result<Self> {
        use anyhow::Context as _;
        let dir = default_pack_dir().context("anatomy pack `assets/anatomy/default` not found")?;
        let pack = AnatomyPack::load(&dir).with_context(|| format!("loading {}", dir.display()))?;
        let forward = ForwardModel::build(&pack.cells, &pack.electrodes, ForwardOptions::default())
            .context("forward model")?;
        Ok(Self {
            cells: Arc::new(pack.cells),
            sets: pack.landmarks.sets,
            aha: pack.aha_segments,
            forward,
        })
    }
}

impl HeartSimApp {
    /// A re-entry scenario is running.
    pub(super) fn reentry_active(&self) -> bool {
        self.reentry.active.is_some()
    }

    /// Start preset `index` from t = 0 (speed setting kept).
    fn start_reentry(&mut self, index: usize) {
        if self.reentry.runtime.is_none() {
            match Runtime::load() {
                Ok(rt) => self.reentry.runtime = Some(rt),
                Err(e) => {
                    tracing::warn!("re-entry scenario: {e:#}");
                    self.reentry.status = Some(format!("{e:#}"));
                    return;
                }
            }
        }
        let (Some(rt), Some(preset)) = (&self.reentry.runtime, self.reentry.presets.get(index)) else {
            return;
        };
        let engine = preset.build(Arc::clone(&rt.cells), &rt.sets, &rt.aha);
        let timeline = Timeline::new(preset.timeline());
        let speed = self.clock.speed;
        self.clock = heartsim_core::SimClock::new(Millis(1.0));
        self.clock.speed = speed;
        self.engine = engine.timing().clone();
        self.sample_accumulator = 0.0;
        self.rings.iter_mut().for_each(heartsim_ecg::LeadRing::clear);
        self.last_v_activation = Millis(-1000.0);
        self.scenario = None;
        self.timeline = Timeline::default();
        if self.ecg.is_paused() {
            self.ecg.toggle_pause();
        }
        if let Some(h) = &mut self.heart {
            h.retime(&preset.timing);
            h.set_external_cells(Some(engine.cells()));
        }
        self.reentry.status = None;
        self.reentry.active = Some(Active {
            index,
            engine,
            timeline,
            speed: 1.0,
        });
    }

    /// Leave the re-entry scenario (the 3-D heart goes back to its own engine).
    pub(super) fn stop_reentry(&mut self) {
        if self.reentry.active.take().is_some()
            && let Some(h) = &mut self.heart
        {
            h.set_external_cells(None);
        }
    }

    /// Run the steps of this frame on the switching engine: scenario timeline and clicks on the
    /// heart as actions, one forward-model sample per strip sample.
    pub(super) fn step_reentry(&mut self, steps: u32) {
        let (Some(mut act), Some(rt)) = (self.reentry.active.take(), self.reentry.runtime.as_ref()) else {
            return;
        };
        let dt = self.clock.dt;
        let period = 1000.0 / f64::from(SAMPLE_RATE_HZ);
        let started = Instant::now();
        let t0 = act.engine.now().0;
        for _ in 0..steps {
            let now = act.engine.now();
            let mut actions: Vec<(Action, Millis)> =
                act.timeline.due(now).map(|a| (a.action.clone(), a.at)).collect();
            if let Some(h) = &mut self.heart {
                actions.extend(h.take_pending_actions().into_iter().map(|a| (a, now)));
            }
            for (a, at) in actions {
                if let Err(e) = act.engine.apply(&a, at) {
                    tracing::warn!("re-entry scenario action at {:.0} ms: {e}", at.0);
                }
            }
            act.engine.step(dt);
            let events = act.engine.drain_events();
            for e in &events {
                if e.region.is_ventricular() && e.kind != EventKind::Blocked {
                    self.last_v_activation = e.at;
                }
            }
            if let Some(h) = &mut self.heart {
                h.on_step(&events, dt, act.engine.now());
            }
            self.sample_accumulator += dt.0;
            while self.sample_accumulator >= period {
                self.sample_accumulator -= period;
                let s = rt.forward.sample(&act.engine.cells().vm).to_leads();
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
            act.speed = 0.9 * act.speed + 0.1 * (act.engine.now().0 - t0) / wall_ms;
        }
        // the therapy window reads the timing engine (drug timers, armed shock): keep a mirror
        self.engine = act.engine.timing().clone();
        if let Some(h) = &mut self.heart {
            h.set_external_cells(Some(act.engine.cells()));
        }
        self.record_heart_history();
        self.reentry.active = Some(act);
    }

    /// Give a therapy-window action to the switching engine. `false` when no re-entry scenario
    /// runs (the app's own engines take it).
    pub(super) fn reentry_apply(&mut self, action: &Action) -> bool {
        let Some(act) = &mut self.reentry.active else {
            return false;
        };
        let now = act.engine.now();
        if let Err(e) = act.engine.apply(action, now) {
            tracing::warn!("therapy action: {e}");
        }
        true
    }

    /// Shocks the switching engine delivered since the last call (`None` outside re-entry).
    pub(super) fn reentry_shocks(&mut self) -> Option<Vec<ShockRecord>> {
        self.reentry.active.as_mut().map(|a| a.engine.drain_shocks())
    }

    /// The re-entry picker with the tier read-out, next to the rhythm picker.
    pub(super) fn reentry_controls(&mut self, ui: &mut egui::Ui) {
        if self.reentry.presets.is_empty() {
            return;
        }
        ui.separator();
        let current = self.reentry.active.as_ref().map(|a| a.index);
        let title = current
            .and_then(|i| self.reentry.presets.get(i))
            .map_or("Re-entry presets…", |p| p.scenario.title.as_str());
        let mut picked = None;
        egui::ComboBox::from_id_salt("reentry_picker")
            .selected_text(title)
            .width(280.0)
            .show_ui(ui, |ui| {
                for (i, p) in self.reentry.presets.iter().enumerate() {
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
            self.start_reentry(i);
        }
        if let Some(act) = &self.reentry.active {
            let tier = match act.engine.tier() {
                Tier::One => "Tier 1".to_string(),
                Tier::Two => format!(
                    "Tier 2 · {} phase singularities · quiet {:.1} s",
                    act.engine.phase_singularities(),
                    act.engine.quiet_ms().unwrap_or(0.0).max(0.0) / 1000.0
                ),
            };
            ui.label(format!("{tier} · {:.2}× real time", act.speed))
                .on_hover_text(
                    "Tier 2 is not real time on every machine; the simulation runs as fast as it can.",
                );
            if ui.button("Stop").clicked() {
                self.stop_reentry();
            }
        }
        if let Some(s) = &self.reentry.status {
            ui.colored_label(egui::Color32::LIGHT_RED, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_presets_load_and_name_a_tier_2_scenario() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios/reentry");
        let presets = load_dir(&dir);
        assert!(!presets.is_empty(), "no re-entry presets in {}", dir.display());
        for p in &presets {
            assert_eq!(
                p.scenario.tier,
                heartsim_core::scenario::EngineTier::ReactionDiffusion,
                "{}",
                p.scenario.id
            );
            assert_ne!(p.scenario.title, "");
        }
        assert!(presets.iter().any(|p| p.scenario.id == "af_persistent"));
    }
}
