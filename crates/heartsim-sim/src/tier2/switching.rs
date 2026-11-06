//! Automatic Tier 1 ↔ Tier 2 switching (brief B17).
//!
//! Tier 1 (graph eikonal) is cheap and cannot sustain functional re-entry; Tier 2 (monodomain)
//! can and is not real time. [`SwitchingEngine`] runs Tier 1 until something needs re-entry,
//! then takes over in Tier 2, and hands back when the re-entry has stopped:
//!
//! * **Tier 1 → Tier 2** when the scenario's initiation time has come, or when an action
//!   arrives that can start re-entry ([`requires_reentry`]: programmed stimulation with
//!   coupled extrastimuli or a rapid burst, an unsynchronised shock that may land on the T
//!   wave). The Tier-1 simulation is *continued* ([`Tier2Engine::from_tier1`]: each voxel starts
//!   at the phase of its nearest cell), the timing engine is carried over unchanged, and the
//!   scenario's patches ([`super::preset::ReentrySpec`]) are laid on the tissue.
//! * **Tier 2 → Tier 1** after [`SwitchPolicy::exit_quiet_ms`] (2 s) in which no re-entry was
//!   seen. *Re-entry* is a phase singularity in the tissue, **or** a tissue activation at a
//!   sensing site that no timing event explains (a flutter circuit around an obstacle has no
//!   singularity, but it activates the AV-nodal input every cycle). The hand-back resets every
//!   cell of Tier 1 to rest, keeps the timing engine (beat timing is continuous) and carries
//!   lesions over.
//!
//! The hysteresis is the 2 s of quiet plus a minimum dwell in Tier 2 ([`SwitchPolicy`]), so a
//! rotor that drifts off a sensing site, a pause between fibrillatory wavelets, or an initiation
//! that has not yet taken hold does not bounce the engine between tiers.
//!
//! Beat timing stays single-sourced: both tiers are driven by the same [`TimingEngine`], and
//! the events this engine returns are that timing engine's.

use super::engine::{Tier2Engine, Tier2Params, Tier2Snapshot};
use super::model::{AlievPanfilov, CellModel};
use super::preset::{ReentrySpec, SwitchPolicy};
use crate::engine::{CellState, Engine, Event};
use crate::tier1::Tier1Engine;
use crate::timing::ActionError;
use heartsim_core::{Action, Millis};

/// Which tier is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    One,
    Two,
}

/// Why the engine switched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchReason {
    /// The scenario's initiation time.
    Scenario,
    /// An action that can start re-entry ([`requires_reentry`]).
    Action,
    /// No re-entry for [`SwitchPolicy::exit_quiet_ms`].
    Quiescent,
}

/// One tier switch, for the audit trail and the status line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchRecord {
    pub at: Millis,
    pub to: Tier,
    pub reason: SwitchReason,
}

/// Whether `action` can start re-entry and so needs Tier 2: programmed stimulation with
/// coupled extrastimuli (S2/S3) or a rapid burst (≥ 6 stimuli faster than 400 ms), and an
/// unsynchronised shock (which may land on the T wave of a normal beat). Everything else —
/// single stimuli, ablation, drugs, rate changes — Tier 1 handles.
#[must_use]
pub fn requires_reentry(action: &Action) -> bool {
    match action {
        Action::PacingTrain {
            s1_cycle_ms,
            s1_count,
            s2_ms,
            ..
        } => s2_ms.is_some() || (*s1_count >= 6 && *s1_cycle_ms < 400.0),
        Action::Shock { synchronised, .. } => !synchronised,
        _ => false,
    }
}

/// Phase singularities must be present this long to count as re-entry, ms.
const PS_PERSIST_MS: f64 = 600.0;

/// Tier 1 and Tier 2 behind one [`Engine`]. See the module documentation.
#[derive(Debug, Clone)]
pub struct SwitchingEngine<M: CellModel = AlievPanfilov> {
    t1: Tier1Engine,
    t2: Option<Tier2Engine<M>>,
    model: M,
    spec: ReentrySpec,
    scenario_seed: u64,
    seed_state: Option<Tier2Snapshot>,
    /// Events of the tier that was handed back, not yet drained.
    pending: Vec<Event>,
    entered_ms: f64,
    /// Last time re-entry was seen in Tier 2: a phase singularity that lasted, or a sensed activation.
    last_evidence_ms: f64,
    /// Start of the current run of checks that found phase singularities (`None` = none now).
    ps_since_ms: Option<f64>,
    last_check_ms: f64,
    /// Phase-singularity count at the last check.
    ps: usize,
    switches: Vec<SwitchRecord>,
    /// The scenario's initiation has been taken care of (entered Tier 2 for it once).
    initiated: bool,
}

impl<M: CellModel> SwitchingEngine<M> {
    /// Run `t1` and switch to Tier 2 as `spec` describes; `model` is the Tier-2 cell model.
    #[must_use]
    pub fn new(t1: Tier1Engine, model: M, spec: ReentrySpec, scenario_seed: u64) -> Self {
        Self {
            t1,
            t2: None,
            model,
            spec,
            scenario_seed,
            seed_state: None,
            pending: Vec::new(),
            entered_ms: 0.0,
            last_evidence_ms: 0.0,
            ps_since_ms: None,
            last_check_ms: f64::NEG_INFINITY,
            ps: 0,
            switches: Vec::new(),
            initiated: false,
        }
    }

    /// The state a [`super::preset::Initiation::SpiralState`] scenario starts Tier 2 from
    /// (read by the caller from the file the scenario names).
    pub fn set_seed_state(&mut self, state: Tier2Snapshot) {
        self.seed_state = Some(state);
    }

    // --- read-out ---------------------------------------------------------------------------

    /// The running tier.
    #[must_use]
    pub fn tier(&self) -> Tier {
        if self.t2.is_some() { Tier::Two } else { Tier::One }
    }

    /// The Tier-2 engine while it runs.
    #[must_use]
    pub fn tier2(&self) -> Option<&Tier2Engine<M>> {
        self.t2.as_ref()
    }

    /// Mutable Tier-2 engine while it runs.
    pub fn tier2_mut(&mut self) -> Option<&mut Tier2Engine<M>> {
        self.t2.as_mut()
    }

    /// The Tier-1 engine (stale while Tier 2 runs).
    #[must_use]
    pub fn tier1(&self) -> &Tier1Engine {
        &self.t1
    }

    /// Mutable Tier-1 engine, for configuration before the run starts.
    pub fn tier1_mut(&mut self) -> &mut Tier1Engine {
        &mut self.t1
    }

    /// The scenario's re-entry section.
    #[must_use]
    pub fn spec(&self) -> &ReentrySpec {
        &self.spec
    }

    /// The timing engine of the running tier (the single source of beat timing).
    #[must_use]
    pub fn timing(&self) -> &crate::timing::TimingEngine {
        self.t2
            .as_ref()
            .map_or_else(|| self.t1.timing(), Tier2Engine::timing)
    }

    /// Switches so far, oldest first.
    #[must_use]
    pub fn switches(&self) -> &[SwitchRecord] {
        &self.switches
    }

    /// Phase-singularity count at the last evaluation (0 in Tier 1).
    #[must_use]
    pub fn phase_singularities(&self) -> usize {
        if self.t2.is_some() { self.ps } else { 0 }
    }

    /// How long Tier 2 has seen no re-entry, ms (`None` in Tier 1).
    #[must_use]
    pub fn quiet_ms(&self) -> Option<f64> {
        let t2 = self.t2.as_ref()?;
        Some(t2.now().0 - self.last_evidence_ms)
    }

    /// Switch policy in force.
    #[must_use]
    pub fn policy(&self) -> &SwitchPolicy {
        &self.spec.switching
    }

    /// Tier-2 settings in force.
    #[must_use]
    pub fn tier2_params(&self) -> &Tier2Params {
        &self.spec.tier2
    }

    // --- switching --------------------------------------------------------------------------

    /// Continue the simulation in Tier 2. No effect when it is already running.
    pub fn enter_tier2(&mut self, reason: SwitchReason) {
        if self.t2.is_some() {
            return;
        }
        // events of the finished steps stay with this engine; the Tier-2 timing engine starts
        // with an empty buffer
        let evs = self.t1.drain_events();
        self.pending.extend(evs);
        let mut t2 = Tier2Engine::from_tier1(&self.t1, &self.model, self.spec.tier2.clone());
        self.spec.apply_patches(&mut t2, self.scenario_seed);
        if let super::preset::Initiation::CrossField {
            compartment,
            start_ms,
            s2_ms,
        } = self.spec.initiation
        {
            t2.cross_field(compartment.into(), start_ms, s2_ms);
        }
        if let super::preset::Initiation::PhaseWave {
            compartment,
            centre_mm,
            axis,
            span,
            within,
            ..
        } = self.spec.initiation
        {
            t2.seed_phase_wave(
                compartment.into(),
                centre_mm.map(glam::Vec3::from),
                glam::Vec3::from(axis),
                span,
                within,
            );
        }
        if let Some(state) = &self.seed_state {
            // a state from another grid is ignored: the scenario then starts from rest
            let _ = t2.restore(state);
        }
        let now = t2.now();
        self.entered_ms = now.0;
        self.last_evidence_ms = now.0;
        self.ps_since_ms = None;
        self.last_check_ms = f64::NEG_INFINITY;
        self.ps = 0;
        self.t2 = Some(t2);
        self.switches.push(SwitchRecord {
            at: now,
            to: Tier::Two,
            reason,
        });
    }

    /// Hand the heart back to Tier 1 (see the module documentation).
    fn leave_tier2(&mut self, reason: SwitchReason) {
        let Some(mut t2) = self.t2.take() else { return };
        let now = t2.now();
        let lesions = t2.lesion_cells();
        self.pending.extend(t2.drain_events());
        self.t1.resume_from(t2.timing().clone(), &lesions);
        self.ps = 0;
        self.switches.push(SwitchRecord {
            at: now,
            to: Tier::One,
            reason,
        });
    }

    /// Evaluate re-entry in the tissue and switch back after the quiet period.
    fn check_exit(&mut self) {
        let policy = self.spec.switching.clone();
        let Some(t2) = &self.t2 else { return };
        let now = t2.now().0;
        if now - self.last_check_ms < policy.check_every_ms {
            return;
        }
        self.last_check_ms = now;
        self.ps = t2.solver().phase_singularities();
        // A normal beat on curved, heterogeneous tissue shows phase singularities while its
        // front passes (a few hundred ms); a rotor shows them for as long as it turns. Only a run
        // of PS_PERSIST_MS counts as re-entry.
        if self.ps > 0 {
            let since = *self.ps_since_ms.get_or_insert(now);
            if now - since >= PS_PERSIST_MS {
                self.last_evidence_ms = now;
            }
        } else {
            self.ps_since_ms = None;
        }
        // an activation the timing engine did not cause, sensed at the atrial or the
        // ventricular site: re-entry without a singularity (flutter around an obstacle)
        self.last_evidence_ms = self.last_evidence_ms.max(t2.last_sensed_ms());
        if now - self.last_evidence_ms >= policy.exit_quiet_ms && now - self.entered_ms >= policy.min_dwell_ms
        {
            self.leave_tier2(SwitchReason::Quiescent);
        }
    }

    /// Shocks delivered since the last call (animation, sound, defibrillator panel).
    pub fn drain_shocks(&mut self) -> Vec<crate::defib::ShockRecord> {
        match &mut self.t2 {
            Some(t2) => t2.drain_shocks(),
            None => self.t1.drain_shocks(),
        }
    }

    /// A catheter stimulus at `cell` (brief B22). In Tier 1 the timing engine decides capture
    /// ([`Tier1Engine::stimulate_cell`]). In Tier 2 the tissue decides: its refractoriness is real
    /// there, and the activation it produces reaches the timing engine through the sensing
    /// sites, so a stimulus never moves the simulation between tiers by itself.
    pub fn stimulate_cell(&mut self, cell: u32, at: Millis) -> Result<(), ActionError> {
        match &mut self.t2 {
            Some(t2) => t2.apply(
                &Action::Stimulate {
                    site: heartsim_core::StimSite::Cell(cell),
                    strength: 1.0,
                    duration_ms: 2.0,
                },
                at,
            ),
            None => self.t1.stimulate_cell(cell, at),
        }
    }

    /// A permanent lesion (brief B22): `cells` and the voxels within `radius_mm` of `centre` stop
    /// conducting, in the running tier and the idle one, and stay so across [`Engine::reset`].
    pub fn ablate_permanently(&mut self, cells: &[u32], centre: glam::Vec3, radius_mm: f32) {
        self.t1.eikonal_mut().ablate_cells_permanently(cells);
        if let Some(t2) = &mut self.t2 {
            t2.ablate_sphere_permanently(centre, radius_mm);
        }
    }

    /// Apply a scenario action to the running tier. An action that can start re-entry
    /// ([`requires_reentry`]) first moves the simulation to Tier 2.
    pub fn apply(&mut self, action: &Action, at: Millis) -> Result<(), ActionError> {
        if self.t2.is_none() && requires_reentry(action) {
            self.enter_tier2(SwitchReason::Action);
        }
        match &mut self.t2 {
            Some(t2) => t2.apply(action, at),
            None => self.t1.apply(action, at),
        }
    }
}

impl<M: CellModel> Engine for SwitchingEngine<M> {
    fn step(&mut self, dt: Millis) {
        if !self.initiated
            && let Some(start) = self.spec.initiation.start_ms()
            && self.t1.now().0 + dt.0 >= start
        {
            self.initiated = true;
            if self.t2.is_none() {
                self.enter_tier2(SwitchReason::Scenario);
            }
        }
        match &mut self.t2 {
            Some(t2) => {
                t2.step(dt);
                self.check_exit();
            }
            None => self.t1.step(dt),
        }
    }

    fn now(&self) -> Millis {
        self.t2.as_ref().map_or_else(|| self.t1.now(), Engine::now)
    }

    fn cells(&self) -> &CellState {
        self.t2.as_ref().map_or_else(|| self.t1.cells(), Engine::cells)
    }

    fn drain_events(&mut self) -> Vec<Event> {
        let mut out = std::mem::take(&mut self.pending);
        match &mut self.t2 {
            Some(t2) => out.extend(t2.drain_events()),
            None => out.extend(self.t1.drain_events()),
        }
        out
    }

    fn reset(&mut self) {
        self.t1.reset();
        self.t2 = None;
        self.pending.clear();
        self.ps_since_ms = None;
        self.last_evidence_ms = 0.0;
        self.last_check_ms = f64::NEG_INFINITY;
        self.ps = 0;
        self.switches.clear();
        self.initiated = false;
    }
}
