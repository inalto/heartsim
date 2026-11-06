//! The procedure: the engine, the protocol being delivered, the RF generator and the lesions.

use super::lesion::{Lesion, LesionBook, RfApplication, RfParams, RfReadout};
use super::preset::EpGoal;
use super::protocol::{Protocol, ProtocolOutcome, ProtocolRun, ProtocolStatus};
use crate::engine::{CellState, Engine, Event};
use crate::tier1::graph::CellKind;
use crate::tier2::model::{AlievPanfilov, CellModel};
use crate::tier2::preset::{cti_lesions, pvi_lesions};
use crate::tier2::switching::{SwitchingEngine, Tier};
use crate::timing::{ActionError, TimingEngine};
use glam::Vec3;
use heartsim_core::{Action, Millis, Region, StimSite};
use heartsim_geometry::CellCloud;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Lesion growth below this is not applied to the tissue yet, mm (re-marking cells costs one scan).
const GROWTH_STEP_MM: f32 = 0.25;
/// Radius of one lesion of a pre-planned ablation line, mm: overlapping into a continuous line on
/// the 2 mm voxel grid.
const LINE_LESION_RADIUS_MM: f64 = 4.0;
/// A pair more than this from any tissue does not capture, mm.
const CAPTURE_RANGE_MM: f32 = 6.0;
/// Fraction of the vein-sleeve junction that must lie in lesions for the veins to count as isolated.
const VEINS_ISOLATED_FRACTION: f64 = 0.95;

/// An ablation target of a procedure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AblationTarget {
    /// Compact AV node: complete heart block (the "ablate and pace" treatment of AF).
    AvNode,
    /// The slow pathway's atrial insertion, inferior to Koch's triangle (AVNRT).
    SlowPathway,
    /// A line across the cavotricuspid isthmus (typical flutter).
    CtiLine,
    /// A ring around the pulmonary-vein ostia (AF).
    PulmonaryVeins,
    /// The accessory pathway's atrial insertion, where the CS activation points.
    AccessoryPathway,
}

/// A catheter stimulus delivered, for the stimulus marks on the electrograms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StimRecord {
    pub at: Millis,
    pub cell: u32,
}

/// The EP lab on one heart. See [`crate::ep`].
#[derive(Debug, Clone)]
pub struct EpLab<M: CellModel = AlievPanfilov> {
    engine: SwitchingEngine<M>,
    goal: EpGoal,
    lesions: LesionBook,
    /// Timing-graph structures the lesions have destroyed, re-applied after a reset.
    blocks: Vec<Action>,
    rf_params: RfParams,
    rf: Option<RfApplication>,
    /// Radius the tissue has received of the application in progress, mm.
    rf_applied_mm: f32,
    run: Option<ProtocolRun>,
    /// The cell the protocol paces.
    run_cell: u32,
    outcome: Option<ProtocolOutcome>,
    stimuli: Vec<StimRecord>,
    events: Vec<Event>,
    /// Actions the engine refused during this step's protocol, for the status line.
    last_error: Option<ActionError>,
}

impl<M: CellModel> EpLab<M> {
    /// A lab around `engine`.
    #[must_use]
    pub fn new(engine: SwitchingEngine<M>, goal: EpGoal) -> Self {
        Self {
            engine,
            goal,
            lesions: LesionBook::default(),
            blocks: Vec::new(),
            rf_params: RfParams::default(),
            rf: None,
            rf_applied_mm: 0.0,
            run: None,
            run_cell: 0,
            outcome: None,
            stimuli: Vec::with_capacity(8),
            events: Vec::with_capacity(32),
            last_error: None,
        }
    }

    // --- read-out ---------------------------------------------------------------------------

    /// The tier-switching engine.
    #[must_use]
    pub fn engine(&self) -> &SwitchingEngine<M> {
        &self.engine
    }

    /// Mutable engine, for scenario actions that are not the lab's business.
    pub fn engine_mut(&mut self) -> &mut SwitchingEngine<M> {
        &mut self.engine
    }

    /// The timing engine of the running tier.
    #[must_use]
    pub fn timing(&self) -> &TimingEngine {
        self.engine.timing()
    }

    /// The cell cloud.
    #[must_use]
    pub fn cloud(&self) -> &Arc<CellCloud> {
        self.engine.tier1().eikonal().cloud()
    }

    /// The lesions so far.
    #[must_use]
    pub fn lesions(&self) -> &LesionBook {
        &self.lesions
    }

    /// The procedure's goal.
    #[must_use]
    pub fn goal(&self) -> EpGoal {
        self.goal
    }

    /// Whether the goal has been met on the engine's state: a structure really blocked, a re-entry
    /// really handed back, not a lesion count.
    #[must_use]
    pub fn goal_reached(&self) -> bool {
        let timing = &self.timing().params;
        match self.goal {
            EpGoal::Study => false,
            EpGoal::TerminateReentry => {
                self.engine.tier() == Tier::One
                    && self.engine.switches().len() >= 2
                    && !self.lesions.is_empty()
            }
            EpGoal::SlowPathwayModified => timing.av.slow.blocked && !timing.av.complete_block,
            EpGoal::PathwayAblated => timing.accessory.is_none() && !self.lesions.is_empty(),
            EpGoal::VeinsIsolated => self.vein_junction_covered() >= VEINS_ISOLATED_FRACTION,
        }
    }

    fn vein_junction_covered(&self) -> f64 {
        let cloud = self.cloud();
        let junction: Vec<Vec3> = (0..cloud.len())
            .filter(|&c| {
                cloud.regions[c] == Region::PulmonaryVeinSleeves
                    && cloud.neighbours[c].iter().any(|n| {
                        n.index != u32::MAX
                            && matches!(
                                cloud.regions[n.index as usize],
                                Region::LeftAtrium | Region::LeftAtrialAppendage
                            )
                    })
            })
            .map(|c| cloud.positions[c])
            .collect();
        if junction.is_empty() {
            return 0.0;
        }
        let inside = junction.iter().filter(|p| self.lesions.contains(**p)).count();
        inside as f64 / junction.len() as f64
    }

    /// Progress of the protocol.
    #[must_use]
    pub fn protocol_status(&self) -> ProtocolStatus {
        self.run
            .as_ref()
            .map_or(ProtocolStatus::Finished, ProtocolRun::status)
    }

    /// The result of the last protocol that finished.
    #[must_use]
    pub fn protocol_outcome(&self) -> Option<&ProtocolOutcome> {
        self.outcome.as_ref()
    }

    /// The generator read-out while RF is on.
    #[must_use]
    pub fn rf_readout(&self) -> Option<RfReadout> {
        let now = self.engine.now().0;
        self.rf.as_ref().map(|rf| rf.readout(now))
    }

    /// Generator settings.
    pub fn rf_params_mut(&mut self) -> &mut RfParams {
        &mut self.rf_params
    }

    /// Stimuli delivered since the last call.
    pub fn take_stimuli(&mut self) -> Vec<StimRecord> {
        std::mem::take(&mut self.stimuli)
    }

    /// Last refusal of the engine, once.
    pub fn take_error(&mut self) -> Option<ActionError> {
        self.last_error.take()
    }

    // --- stimulation ------------------------------------------------------------------------

    /// The cell a catheter pair at `at_mm` stimulates: the nearest tissue that can be excited, if
    /// within capture range. A pair floating in the blood pool captures nothing.
    #[must_use]
    pub fn capture_cell(&self, at_mm: Vec3) -> Option<u32> {
        let eik = self.engine.tier1().eikonal();
        let (cloud, kinds, dead) = (eik.cloud(), eik.kinds(), eik.ablated());
        let mut best: Option<(f32, u32)> = None;
        for (i, p) in cloud.positions.iter().enumerate() {
            // nodal tissue is not captured by a pacing pulse; the His bundle and the bundles are
            let nodal = matches!(
                cloud.regions[i],
                Region::AvNodeCompact | Region::AvNodeFastPathway | Region::AvNodeSlowPathway
            );
            if dead[i] || nodal || matches!(kinds[i], CellKind::SinusNode) {
                continue;
            }
            let d = p.distance_squared(at_mm);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, i as u32));
            }
        }
        best.filter(|b| b.0 <= CAPTURE_RANGE_MM * CAPTURE_RANGE_MM)
            .map(|b| b.1)
    }

    /// One stimulus at `cell` now (and on the stimulus marks).
    pub fn stimulate(&mut self, cell: u32) -> Result<(), ActionError> {
        let at = self.engine.now();
        self.deliver(cell, at)
    }

    fn deliver(&mut self, cell: u32, at: Millis) -> Result<(), ActionError> {
        self.engine.stimulate_cell(cell, at)?;
        self.stimuli.push(StimRecord { at, cell });
        Ok(())
    }

    /// Start `protocol` from `cell`, replacing the one running. Stimuli start at the next step.
    pub fn start_protocol(&mut self, cell: u32, protocol: Protocol) {
        self.outcome = None;
        let mut run = ProtocolRun::new(protocol, self.engine.now().0 + 1.0);
        // activations before the start are not return beats
        run.observe(self.engine.now().0, &[], self.site_activation(cell));
        self.run = Some(run);
        self.run_cell = cell;
    }

    /// Stop the protocol; its partial result is kept.
    pub fn stop_protocol(&mut self) {
        if let Some(mut run) = self.run.take() {
            run.stop();
            self.outcome = run.outcome().cloned();
        }
    }

    fn site_activation(&self, cell: u32) -> f64 {
        self.engine
            .cells()
            .activation_ms
            .get(cell as usize)
            .map_or(f64::NEG_INFINITY, |t| {
                if t.is_nan() {
                    f64::NEG_INFINITY
                } else {
                    f64::from(*t)
                }
            })
    }

    // --- ablation ---------------------------------------------------------------------------

    /// RF on with the tip at `tip_mm`. The contact stub is the distance to the nearest tissue that
    /// still conducts, so burning over an old lesion heats nothing new.
    pub fn rf_on(&mut self, tip_mm: Vec3) {
        let eik = self.engine.tier1().eikonal();
        let (cloud, dead) = (eik.cloud(), eik.ablated());
        let distance = cloud
            .positions
            .iter()
            .enumerate()
            .filter(|(i, _)| !dead[*i])
            .map(|(_, p)| p.distance(tip_mm))
            .fold(f32::INFINITY, f32::min);
        let now = self.engine.now().0;
        self.rf = Some(RfApplication::new(tip_mm, now, distance, self.rf_params));
        self.rf_applied_mm = 0.0;
        self.lesions.add(Lesion {
            centre_mm: tip_mm.to_array(),
            radius_mm: 0.0,
            at_ms: now,
        });
    }

    /// RF off: the lesion keeps the size it has reached; an application too short or too poorly
    /// in contact to reach the injury temperature leaves none.
    pub fn rf_off(&mut self) {
        if self.rf.take().is_some() {
            self.lesions.drop_last_if_empty();
        }
    }

    /// A finished lesion at once (a line planned ahead, a scenario's previous procedure).
    pub fn ablate_now(&mut self, centre_mm: Vec3, radius_mm: f32) {
        let now = self.engine.now().0;
        self.lesions.add(Lesion {
            centre_mm: centre_mm.to_array(),
            radius_mm,
            at_ms: now,
        });
        self.apply_lesion(centre_mm, radius_mm);
    }

    /// Where the ablation catheter goes for `target`. A line target gets all its stops; a target the
    /// heart does not have (no accessory pathway, no sleeve cells) gets none.
    #[must_use]
    pub fn target_sites(&self, target: AblationTarget) -> Vec<Vec3> {
        let cloud = self.cloud();
        let of_actions = |actions: Vec<heartsim_core::TimedAction>| -> Vec<Vec3> {
            actions
                .iter()
                .filter_map(|a| match &a.action {
                    Action::Ablate {
                        site: StimSite::Point(p),
                        ..
                    } => Some(Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)),
                    _ => None,
                })
                .collect()
        };
        match target {
            AblationTarget::AvNode => cloud
                .landmarks
                .get("av_node")
                .map(|c| cloud.positions[*c as usize])
                .into_iter()
                .collect(),
            AblationTarget::SlowPathway => (0..cloud.len())
                .rev()
                .find(|&c| cloud.regions[c] == Region::AvNodeSlowPathway)
                .map(|c| cloud.positions[c])
                .into_iter()
                .collect(),
            AblationTarget::CtiLine => of_actions(cti_lesions(cloud, LINE_LESION_RADIUS_MM, Millis::ZERO)),
            AblationTarget::PulmonaryVeins => {
                of_actions(pvi_lesions(cloud, LINE_LESION_RADIUS_MM, Millis::ZERO))
            }
            AblationTarget::AccessoryPathway => {
                self.accessory_insertions().first().copied().into_iter().collect()
            }
        }
    }

    /// Lay the whole of a target at once: every stop of a line, the single site otherwise, with a
    /// lesion of the line radius. Returns the number of lesions.
    pub fn ablate_target(&mut self, target: AblationTarget) -> usize {
        let sites = self.target_sites(target);
        for &p in &sites {
            self.ablate_now(p, LINE_LESION_RADIUS_MM as f32);
        }
        sites.len()
    }

    /// Atrial and ventricular insertions of the accessory pathway, atrial first.
    fn accessory_insertions(&self) -> Vec<Vec3> {
        let Some(ap) = &self.timing().params.accessory else {
            return Vec::new();
        };
        let eik = self.engine.tier1().eikonal();
        let side = crate::tier1::side_index(ap.atrial_side);
        let v_side = crate::tier1::side_index(ap.ventricular_side);
        let seeds = eik.seeds();
        seeds.ap_atrial[side]
            .iter()
            .chain(&seeds.ap_ventricular[v_side])
            .map(|&c| eik.cloud().positions[c as usize])
            .collect()
    }

    /// Destroy what lies within `radius_mm` of `centre`: myocardium stops conducting; a nodal or
    /// conducting structure, or an accessory-pathway insertion, leaves the timing graph.
    fn apply_lesion(&mut self, centre: Vec3, radius_mm: f32) {
        if radius_mm <= 0.0 {
            return;
        }
        let r2 = radius_mm * radius_mm;
        let cloud = Arc::clone(self.cloud());
        let cells: Vec<u32> = cloud
            .positions
            .iter()
            .enumerate()
            .filter(|(_, p)| p.distance_squared(centre) <= r2)
            .map(|(i, _)| i as u32)
            .collect();
        self.engine.ablate_permanently(&cells, centre, radius_mm);

        let hit = |region: Region| cells.iter().any(|&c| cloud.regions[c as usize] == region);
        let mut blocks = Vec::new();
        if hit(Region::AvNodeCompact) || hit(Region::HisBundle) {
            blocks.push(Region::AvNodeCompact);
        }
        for region in [
            Region::AvNodeFastPathway,
            Region::AvNodeSlowPathway,
            Region::RightBundleBranch,
            Region::LeftBundleBranch,
            Region::LeftAnteriorFascicle,
            Region::LeftPosteriorFascicle,
        ] {
            if hit(region) {
                blocks.push(region);
            }
        }
        for region in blocks {
            let action = Action::Block {
                region,
                blocked: true,
            };
            if !self.blocks.contains(&action) {
                self.apply_block(&action);
                self.blocks.push(action);
            }
        }
        if self
            .accessory_insertions()
            .iter()
            .any(|p| p.distance_squared(centre) <= r2)
        {
            let action = Action::AccessoryPathway {
                enabled: false,
                antegrade: false,
                retrograde: false,
            };
            self.apply_block(&action);
            self.blocks.push(action);
        }
    }

    fn apply_block(&mut self, action: &Action) {
        let now = self.engine.now();
        if let Err(e) = self.engine.apply(action, now) {
            self.last_error = Some(e);
        }
    }

    fn grow_rf(&mut self) {
        let now = self.engine.now().0;
        let Some(rf) = self.rf else { return };
        let radius = rf.readout(now).lesion_radius_mm;
        self.lesions.grow_last(radius);
        if radius >= self.rf_applied_mm + GROWTH_STEP_MM {
            self.rf_applied_mm = radius;
            self.apply_lesion(rf.tip_mm, radius);
        }
    }

    // --- actions ----------------------------------------------------------------------------

    /// Apply a scenario action (timeline, drugs, shocks) to the running tier.
    pub fn apply(&mut self, action: &Action, at: Millis) -> Result<(), ActionError> {
        self.engine.apply(action, at)
    }
}

impl<M: CellModel> Engine for EpLab<M> {
    fn step(&mut self, dt: Millis) {
        let now = self.engine.now().0;
        self.grow_rf();
        if let Some(mut run) = self.run.take() {
            while let Some(t) = run.next_stimulus(now + dt.0) {
                if let Err(e) = self.deliver(self.run_cell, Millis(t)) {
                    self.last_error = Some(e);
                }
            }
            self.run = Some(run);
        }
        self.engine.step(dt);
        let events = self.engine.drain_events();
        let site = self.site_activation(self.run_cell);
        let now = self.engine.now().0;
        if let Some(mut run) = self.run.take() {
            run.observe(now, &events, site);
            if let Some(outcome) = run.outcome() {
                self.outcome = Some(outcome.clone());
            } else {
                self.run = Some(run);
            }
        }
        self.events.extend(events);
    }

    fn now(&self) -> Millis {
        self.engine.now()
    }

    fn cells(&self) -> &CellState {
        self.engine.cells()
    }

    fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Back to t = 0 with the same heart: the protocol and RF stop, the rhythm restarts, and the
    /// lesions are still there (scar does not heal).
    fn reset(&mut self) {
        self.engine.reset();
        self.run = None;
        self.outcome = None;
        self.rf = None;
        self.rf_applied_mm = 0.0;
        self.stimuli.clear();
        self.events.clear();
        self.last_error = None;
        let blocks = self.blocks.clone();
        for action in &blocks {
            self.apply_block(action);
        }
    }
}
