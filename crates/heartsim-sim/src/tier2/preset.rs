//! Re-entry presets (brief B17): the `reentry:` section of a scenario file and the tissue
//! substrate it describes.
//!
//! A preset is a scenario with `tier: ReactionDiffusion` whose file carries, next to `timing`
//! and `morphology`, a [`ReentrySpec`]:
//!
//! * **patches** — where the tissue differs: short-APD pulmonary-vein sleeves (atrial remodelling,
//!   the AF trigger zone), a slow cavotricuspid isthmus (typical flutter), scar with a slow
//!   border zone (monomorphic VT), long-APD tissue with wide dispersion (torsades), an APD
//!   mosaic (VF). Region-wide parameter changes (`parameters:` of the scenario) reach the engine
//!   through the Tier-1 router it is built from, so only what the parameter table cannot say —
//!   geometry — is a patch;
//! * **initiation** — how the arrhythmia is started: a programmed-stimulation protocol (expanded
//!   to a `PacingTrain` timeline action), or a pre-seeded state file ([`Tier2Snapshot`]);
//! * **switching** — when [`super::switching::SwitchingEngine`] hands the heart back to Tier 1.
//!
//! Interventions are ordinary timeline actions (`Ablate` lines, `Shock`, `Drug`).
//! [`pvi_lesions`] and [`cti_lesions`] generate the ablation lines the EP procedures use.

use super::engine::{Tier2Engine, Tier2Params};
use super::grid::Compartment;
use super::model::{AlievPanfilov, CellModel};
use super::reentry;
use super::switching::SwitchingEngine;
use crate::tier1::{EikonalEngine, EikonalParams, Tier1Engine};
use crate::timing::TimingParams;
use glam::Vec3;
use heartsim_core::scenario::ScenarioError;
use heartsim_core::{Action, Millis, Region, Scenario, StimSite, TimedAction};
use heartsim_geometry::CellCloud;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Which voxels a [`Patch`] covers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatchTarget {
    /// Voxels whose nearest cell belongs to this region.
    Region(Region),
    /// Voxels within `radius_mm` of a point (heart frame).
    Sphere { centre_mm: [f32; 3], radius_mm: f32 },
    /// Voxels within `radius_mm` of a named landmark.
    Landmark { name: String, radius_mm: f32 },
    /// Voxels in a box (heart frame, inclusive).
    Box { lo_mm: [f32; 3], hi_mm: [f32; 3] },
    /// Every voxel of a compartment.
    Compartment(PatchCompartment),
    /// A band of a spherical wall: voxels within `radius_mm` of `centre_mm` whose direction from
    /// the centre makes an angle in `from_deg..=to_deg` with `axis`, except those within `gap_mm`
    /// of `gap_at_mm`. It draws a tricuspid-annulus opening or an insulating barrier on the
    /// spherical atria of the idealised pack (they have no anatomical holes of their own).
    Band {
        compartment: PatchCompartment,
        centre_mm: [f32; 3],
        radius_mm: f32,
        axis: [f32; 3],
        from_deg: f32,
        to_deg: f32,
        #[serde(default)]
        gap_at_mm: Option<[f32; 3]>,
        #[serde(default)]
        gap_mm: f32,
    },
}

/// Serializable twin of [`Compartment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchCompartment {
    Atrial,
    Ventricular,
}

impl From<PatchCompartment> for Compartment {
    fn from(c: PatchCompartment) -> Self {
        match c {
            PatchCompartment::Atrial => Compartment::Atrial,
            PatchCompartment::Ventricular => Compartment::Ventricular,
        }
    }
}

/// What a [`Patch`] does to its voxels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatchEffect {
    /// Scale the APD (and so the refractory period) and the conduction velocity of the tissue;
    /// the CV factor is capped at 1 (the diffusivity is already at the explicit stability
    /// limit wherever the CV target needed it).
    Scale { apd: f32, cv: f32 },
    /// Inert tissue: scar, a line of block, an ablation lesion.
    Obstacle,
    /// Tile the target with cubes of `patch_mm` whose APD is scaled by a factor drawn
    /// uniformly from `[lo, hi]` (deterministic in the scenario seed): dispersion of
    /// refractoriness, the substrate of wavebreak.
    Mosaic { patch_mm: f32, lo: f32, hi: f32 },
}

/// A region of tissue with different properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub target: PatchTarget,
    pub effect: PatchEffect,
}

/// The conducting voxels a [`Initiation::PhaseWave`] seeds: within `radius_mm` of its centre and at
/// an angle from its axis in `polar_deg`. The rest of the chamber is left at rest.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhaseWaveLimit {
    pub radius_mm: f32,
    pub polar_deg: (f32, f32),
}

/// How the arrhythmia is started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Initiation {
    /// Nothing: the substrate alone (a timeline action — a burst, a shock on T — starts it).
    #[default]
    None,
    /// Programmed electrical stimulation: `s1_count` stimuli at `s1_cycle_ms` from `site`, then
    /// the optional S2 and S3 at their coupling intervals (`Action::PacingTrain`), starting at
    /// `start_ms` of the scenario.
    Protocol {
        site: StimSite,
        start_ms: f64,
        s1_cycle_ms: f64,
        s1_count: u32,
        s2_ms: Option<f64>,
        s3_ms: Option<f64>,
    },
    /// Cross-field stimulation (Starmer / Winfree) over the tissue of one compartment: a planar S1
    /// from its low-x end at `start_ms`, then an S2 over the low-x, low-y quadrant `s2_ms`
    /// later. Timed so that the quadrant has recovered behind the S1 wave but its far side has
    /// not, the S2 front can only go one way and its free end curls into a rotor
    /// ([`super::reentry::s1_s2_cross_field`] is the same protocol on a sheet). It is the
    /// reliable way to start a functional rotor in a smooth substrate, where pacing from one
    /// site meets no unidirectional block.
    CrossField {
        compartment: PatchCompartment,
        start_ms: f64,
        s2_ms: f64,
    },
    /// A pre-seeded rotating wave: every voxel of the compartment is put at the phase of its
    /// action potential given by its azimuth around an axis through `centre_mm` (the
    /// compartment's centroid when `None`), `span` action-potential durations over a full
    /// turn. The seam where the phase jumps from rested to just-excited is a wave front, the
    /// gradient behind it its tail (the phase-distribution method of starting a spiral wave).
    /// The wave then runs by itself; whether it persists is up to the tissue.
    PhaseWave {
        compartment: PatchCompartment,
        start_ms: f64,
        centre_mm: Option<[f32; 3]>,
        axis: [f32; 3],
        span: f32,
        /// Seed only part of the compartment (`None` = all of it): a wave confined to one circuit.
        #[serde(default)]
        within: Option<PhaseWaveLimit>,
    },
    /// Start from a pre-seeded state ([`super::Tier2Snapshot`] file, relative to the scenario
    /// directory) instead of waiting for the initiation to develop. The caller reads the file
    /// ([`super::switching::SwitchingEngine::set_seed_state`]); the simulation crates do no I/O.
    SpiralState { file: String, start_ms: f64 },
}

impl Initiation {
    /// Time at which the scenario needs Tier 2, ms.
    #[must_use]
    pub fn start_ms(&self) -> Option<f64> {
        match self {
            Initiation::None => None,
            Initiation::Protocol { start_ms, .. }
            | Initiation::CrossField { start_ms, .. }
            | Initiation::PhaseWave { start_ms, .. }
            | Initiation::SpiralState { start_ms, .. } => Some(*start_ms),
        }
    }

    /// The timeline action of the protocol, if any.
    #[must_use]
    pub fn action(&self) -> Option<TimedAction> {
        match self {
            Initiation::Protocol {
                site,
                start_ms,
                s1_cycle_ms,
                s1_count,
                s2_ms,
                s3_ms,
            } => Some(TimedAction {
                at: Millis(*start_ms),
                action: Action::PacingTrain {
                    site: site.clone(),
                    s1_cycle_ms: *s1_cycle_ms,
                    s1_count: *s1_count,
                    s2_ms: *s2_ms,
                    s3_ms: *s3_ms,
                },
            }),
            _ => None,
        }
    }
}

/// When the heart goes back to Tier 1 (see [`super::switching::SwitchingEngine`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SwitchPolicy {
    /// Tier 2 hands the heart back after this long without any re-entry (no phase singularity
    /// and no tissue activation sensed that the timing engine did not cause), ms. 2 s is long
    /// enough that a rotor drifting off a sensing site or a pause between wavelets does not
    /// bounce the engine between tiers, and short enough to feel immediate after a shock.
    pub exit_quiet_ms: f64,
    /// Tier 2 is kept at least this long after entering (the initiation needs time to take
    /// hold; without it a protocol that has not yet produced a rotor would end at once), ms.
    pub min_dwell_ms: f64,
    /// Re-entry is evaluated at this interval (a phase-singularity scan visits every voxel), ms.
    pub check_every_ms: f64,
}

impl Default for SwitchPolicy {
    fn default() -> Self {
        Self {
            exit_quiet_ms: 2000.0,
            min_dwell_ms: 3000.0,
            check_every_ms: 20.0,
        }
    }
}

/// The `reentry:` section of a scenario file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReentrySpec {
    /// Tier-2 numerics (voxel size, time step, stimulus).
    pub tier2: Tier2Params,
    pub patches: Vec<Patch>,
    pub initiation: Initiation,
    pub switching: SwitchPolicy,
    /// Seed of the random tissue (mosaic patches); the scenario seed when `None`.
    pub seed: Option<u64>,
}

impl Default for ReentrySpec {
    fn default() -> Self {
        Self {
            tier2: Tier2Params::default(),
            patches: Vec::new(),
            initiation: Initiation::None,
            switching: SwitchPolicy::default(),
            seed: None,
        }
    }
}

/// Reads the `reentry` section of a scenario file (every other field is ignored, as for the
/// `timing` section).
#[derive(Debug, Default, Deserialize)]
#[serde(rename = "Scenario")]
struct ReentrySection {
    #[serde(default)]
    reentry: Option<ReentrySpec>,
}

impl ReentrySpec {
    /// The `reentry` section of a scenario file, `None` when it has none.
    pub fn from_scenario_ron(text: &str) -> Result<Option<Self>, ron::error::SpannedError> {
        Ok(ron::from_str::<ReentrySection>(text)?.reentry)
    }

    /// Timeline actions the initiation adds to the scenario's own.
    #[must_use]
    pub fn timeline(&self) -> Vec<TimedAction> {
        self.initiation.action().into_iter().collect()
    }

    /// Build the tissue substrate on a fresh Tier-2 engine: every [`Patch`] in order.
    pub fn apply_patches<M: CellModel>(&self, engine: &mut Tier2Engine<M>, scenario_seed: u64) {
        let seed = self.seed.unwrap_or(scenario_seed);
        for (i, patch) in self.patches.iter().enumerate() {
            let voxels = select(engine, &patch.target);
            let solver = engine.solver_mut();
            match patch.effect {
                PatchEffect::Obstacle => solver.set_conducting(&voxels, false),
                PatchEffect::Scale { apd, cv } => reentry::scale_voxels(solver, &voxels, apd, cv),
                PatchEffect::Mosaic { patch_mm, lo, hi } => {
                    reentry::apd_mosaic_in(
                        solver,
                        Some(&voxels),
                        patch_mm,
                        lo,
                        hi,
                        seed.wrapping_add(i as u64),
                    );
                }
            }
        }
    }
}

/// Voxels of `target`, ascending.
fn select<M: CellModel>(engine: &Tier2Engine<M>, target: &PatchTarget) -> Vec<u32> {
    let g = engine.solver().grid();
    let cloud = engine.router().cloud();
    match target {
        PatchTarget::Region(r) => (0..g.len() as u32)
            .filter(|&v| cloud.regions[engine.voxel_cells()[v as usize] as usize] == *r)
            .collect(),
        PatchTarget::Sphere { centre_mm, radius_mm } => {
            g.voxels_within(Vec3::from(*centre_mm), *radius_mm, None)
        }
        PatchTarget::Landmark { name, radius_mm } => cloud
            .landmarks
            .get(name)
            .map(|&c| g.voxels_within(cloud.positions[c as usize], *radius_mm, None))
            .unwrap_or_default(),
        PatchTarget::Box { lo_mm, hi_mm } => {
            engine
                .solver()
                .voxels_in_box(Vec3::from(*lo_mm), Vec3::from(*hi_mm), None)
        }
        PatchTarget::Compartment(c) => {
            let c = Compartment::from(*c);
            (0..g.len() as u32)
                .filter(|&v| g.compartment[v as usize] == c)
                .collect()
        }
        PatchTarget::Band {
            compartment,
            centre_mm,
            radius_mm,
            axis,
            from_deg,
            to_deg,
            gap_at_mm,
            gap_mm,
        } => {
            let c = Compartment::from(*compartment);
            let centre = Vec3::from(*centre_mm);
            let axis = Vec3::from(*axis).normalize_or_zero();
            let gap = gap_at_mm.map(Vec3::from);
            (0..g.len() as u32)
                .filter(|&v| {
                    if g.compartment[v as usize] != c {
                        return false;
                    }
                    let p = g.position(v);
                    let r = p - centre;
                    let len = r.length();
                    if len > *radius_mm || len < 1e-3 {
                        return false;
                    }
                    let angle = (r.dot(axis) / len).clamp(-1.0, 1.0).acos().to_degrees();
                    (*from_deg..=*to_deg).contains(&angle) && gap.is_none_or(|q| p.distance(q) > *gap_mm)
                })
                .collect()
        }
    }
}

// --- ablation lines ----------------------------------------------------------------------------

/// Ablation lesions as timeline actions: one `Ablate` of `radius_mm` at each of `points`, at
/// `at`.
fn lesion_actions(points: &[Vec3], radius_mm: f64, at: Millis) -> Vec<TimedAction> {
    points
        .iter()
        .map(|p| TimedAction {
            at,
            action: Action::Ablate {
                site: StimSite::Point([f64::from(p.x), f64::from(p.y), f64::from(p.z)]),
                radius_mm,
            },
        })
        .collect()
}

/// Greedy subsample of `points` (in index order) keeping every pick at least `gap_mm` from the
/// previous ones.
fn ring_picks(points: &[Vec3], gap_mm: f32) -> Vec<Vec3> {
    let mut picks: Vec<Vec3> = Vec::new();
    for &p in points {
        if picks.iter().all(|q| q.distance(p) >= gap_mm) {
            picks.push(p);
        }
    }
    picks
}

/// Pulmonary-vein isolation (PVI): circumferential lesions around the sleeve–atrium junction,
/// as `Ablate` actions at `at`. The junction is the set of sleeve cells that have a left-atrial
/// (body) neighbour; lesions of `radius_mm` are laid at least `radius_mm` apart along it, which
/// overlaps them into a continuous line on the voxel grid.
#[must_use]
pub fn pvi_lesions(cloud: &CellCloud, radius_mm: f64, at: Millis) -> Vec<TimedAction> {
    let junction: Vec<Vec3> = (0..cloud.len())
        .filter(|&c| cloud.regions[c] == Region::PulmonaryVeinSleeves)
        .filter(|&c| {
            cloud.neighbours[c].iter().any(|n| {
                n.index != u32::MAX
                    && matches!(
                        cloud.regions[n.index as usize],
                        Region::LeftAtrium | Region::LeftAtrialAppendage
                    )
            })
        })
        .map(|c| cloud.positions[c])
        .collect();
    lesion_actions(&ring_picks(&junction, radius_mm as f32), radius_mm, at)
}

/// Cavotricuspid-isthmus (CTI) line: lesions along every cell of the isthmus region, from the
/// tricuspid annulus to the inferior vena cava, as `Ablate` actions at `at`.
#[must_use]
pub fn cti_lesions(cloud: &CellCloud, radius_mm: f64, at: Millis) -> Vec<TimedAction> {
    let isthmus: Vec<Vec3> = (0..cloud.len())
        .filter(|&c| cloud.regions[c] == Region::CavotricuspidIsthmus)
        .map(|c| cloud.positions[c])
        .collect();
    lesion_actions(&ring_picks(&isthmus, radius_mm as f32), radius_mm, at)
}

// --- a loaded preset -----------------------------------------------------------------------------

/// Why a preset file cannot be loaded.
#[derive(Debug)]
pub enum PresetError {
    /// The common scenario fields failed to parse (or the version is too new).
    Scenario(ScenarioError),
    /// The `timing` or `reentry` section failed to parse.
    Section(ron::error::SpannedError),
}

impl std::fmt::Display for PresetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PresetError::Scenario(e) => write!(f, "{e}"),
            PresetError::Section(e) => write!(f, "engine section: {e}"),
        }
    }
}

impl std::error::Error for PresetError {}

/// A scenario file with a `reentry` section: the common fields, the timing parameters and the
/// re-entry specification, all read from the same text.
#[derive(Debug, Clone)]
pub struct Preset {
    pub scenario: Scenario,
    pub timing: TimingParams,
    pub spec: ReentrySpec,
}

impl Preset {
    /// Parse a scenario file; `Ok(None)` when it has no `reentry` section.
    pub fn from_ron(text: &str) -> Result<Option<Self>, PresetError> {
        let Some(spec) = ReentrySpec::from_scenario_ron(text).map_err(PresetError::Section)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            scenario: Scenario::from_ron(text).map_err(PresetError::Scenario)?,
            timing: TimingParams::from_scenario_ron(text).map_err(PresetError::Section)?,
            spec,
        }))
    }

    /// The scenario's timeline plus the initiation protocol, in time order.
    #[must_use]
    pub fn timeline(&self) -> Vec<TimedAction> {
        let mut t = self.scenario.timeline.clone();
        t.extend(self.spec.timeline());
        t.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
        t
    }

    /// The engine: Tier 1 on the cell cloud (with the scenario's tissue parameters and
    /// lesions), switching to Tier 2 as the spec says. `sets` are the pack's seed sets and
    /// `aha` its coronary segments (empty = none).
    #[must_use]
    pub fn build(
        &self,
        cells: Arc<CellCloud>,
        sets: &BTreeMap<String, Vec<u32>>,
        aha: &[u8],
    ) -> SwitchingEngine<AlievPanfilov> {
        let n = cells.len();
        let mut eik =
            EikonalEngine::with_seed_sets(cells, &self.scenario.parameters, EikonalParams::default(), sets);
        if aha.len() == n {
            eik.set_aha_segments(aha);
        }
        eik.set_ablated_cells(&self.scenario.ablated_cells);
        let mut t1 = Tier1Engine::new(self.timing.clone(), eik);
        t1.timing_mut().set_shock_seed(self.scenario.seed);
        SwitchingEngine::new(
            t1,
            AlievPanfilov::default(),
            self.spec.clone(),
            self.scenario.seed,
        )
    }
}
