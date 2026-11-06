//! EP procedures as scenario files (`assets/scenarios/ep/*.ron`): the common scenario fields, the
//! `timing` section, an optional `reentry` section for Tier-2 arrhythmias, and an `ep` section with
//! the procedure's goal.

use super::lab::{AblationTarget, EpLab};
use crate::tier1::{EikonalEngine, EikonalParams, Tier1Engine};
use crate::tier2::model::AlievPanfilov;
use crate::tier2::preset::{PresetError, ReentrySpec};
use crate::tier2::switching::SwitchingEngine;
use crate::timing::TimingParams;
use heartsim_core::{Scenario, TimedAction};
use heartsim_geometry::CellCloud;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Distance within which the coronary-sinus sleeve joins the right atrium in the EP lab, mm.
const CS_OS_LINK_MM: f32 = 15.0;

/// What the procedure is trying to achieve; [`EpLab::goal_reached`] checks it on the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EpGoal {
    /// A diagnostic study: nothing to ablate.
    #[default]
    Study,
    /// The re-entry stops and sinus rhythm returns (flutter).
    TerminateReentry,
    /// Slow pathway blocked, AV conduction kept (AVNRT).
    SlowPathwayModified,
    /// Accessory pathway gone (AVRT, WPW).
    PathwayAblated,
    /// Every pulmonary vein ostium inside a lesion (AF).
    VeinsIsolated,
}

/// The `ep:` section of a procedure file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EpSpec {
    pub goal: EpGoal,
    /// Ablation sites offered on the panel.
    pub targets: Vec<AblationTarget>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename = "Scenario")]
struct EpSection {
    #[serde(default)]
    ep: Option<EpSpec>,
    #[serde(default)]
    reentry: Option<ReentrySpec>,
}

/// A procedure file.
#[derive(Debug, Clone)]
pub struct EpPreset {
    pub scenario: Scenario,
    pub timing: TimingParams,
    pub spec: ReentrySpec,
    pub ep: EpSpec,
}

impl EpPreset {
    /// Parse a procedure file; `Ok(None)` when it has no `ep` section.
    pub fn from_ron(text: &str) -> Result<Option<Self>, PresetError> {
        let section: EpSection = ron::from_str(text).map_err(PresetError::Section)?;
        let Some(ep) = section.ep else {
            return Ok(None);
        };
        Ok(Some(Self {
            scenario: Scenario::from_ron(text).map_err(PresetError::Scenario)?,
            timing: TimingParams::from_scenario_ron(text).map_err(PresetError::Section)?,
            spec: section.reentry.unwrap_or_default(),
            ep,
        }))
    }

    /// The scenario's timeline plus the initiation, in time order.
    #[must_use]
    pub fn timeline(&self) -> Vec<TimedAction> {
        let mut t = self.scenario.timeline.clone();
        t.extend(self.spec.timeline());
        t.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
        t
    }

    /// The lab: Tier 1 on the cell cloud with the coronary-sinus sleeve joined to the right atrium,
    /// switching to Tier 2 as the `reentry` section says.
    #[must_use]
    pub fn build(
        &self,
        cells: Arc<CellCloud>,
        sets: &BTreeMap<String, Vec<u32>>,
        aha: &[u8],
    ) -> EpLab<AlievPanfilov> {
        let n = cells.len();
        let eik = EikonalParams {
            cs_os_link_mm: CS_OS_LINK_MM,
            ..EikonalParams::default()
        };
        let mut eikonal = EikonalEngine::with_seed_sets(cells, &self.scenario.parameters, eik, sets);
        if aha.len() == n {
            eikonal.set_aha_segments(aha);
        }
        eikonal.set_ablated_cells(&self.scenario.ablated_cells);
        let mut t1 = Tier1Engine::new(self.timing.clone(), eikonal);
        t1.timing_mut().set_shock_seed(self.scenario.seed);
        let engine = SwitchingEngine::new(
            t1,
            AlievPanfilov::default(),
            self.spec.clone(),
            self.scenario.seed,
        );
        EpLab::new(engine, self.ep.goal)
    }
}
