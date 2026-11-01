//! Scenario files (`*.ron`): everything needed to reproduce a simulation — pathology recipe,
//! parameter overrides, interventions on a timeline, electrode placement, tutorial metadata.
//! Versioned so old files keep loading.

use crate::lead::Electrode;
use crate::params::{Parameters, RegionParams};
use crate::region::Region;
use crate::time::Millis;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCENARIO_FORMAT_VERSION: u32 = 1;

/// Which engine tier the scenario needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EngineTier {
    /// Template ECG synthesis only (no 3D propagation).
    Template,
    /// Conduction graph + eikonal activation + AP templates → pseudo-ECG (default).
    #[default]
    Eikonal,
    /// Reaction–diffusion (re-entry, fibrillation).
    ReactionDiffusion,
}

/// An intervention applied at a point in simulated time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// Change a global or regional parameter.
    SetGlobalSinusRate {
        bpm: f64,
    },
    SetRegion {
        region: Region,
        params: RegionParams,
    },
    /// Make a region non-conducting (block) or restore it.
    Block {
        region: Region,
        blocked: bool,
    },
    /// Programmed stimulus at a cell / landmark.
    Stimulate {
        site: StimSite,
        strength: f64,
        duration_ms: f64,
    },
    /// Pacing train: S1×n at `cycle_ms`, then optional S2, S3 coupling intervals.
    PacingTrain {
        site: StimSite,
        s1_cycle_ms: f64,
        s1_count: u32,
        s2_ms: Option<f64>,
        s3_ms: Option<f64>,
    },
    /// Ablate cells within `radius_mm` of a site.
    Ablate {
        site: StimSite,
        radius_mm: f64,
    },
    /// Enable/disable an accessory pathway.
    AccessoryPathway {
        enabled: bool,
        antegrade: bool,
        retrograde: bool,
    },
    /// Coronary branch stenosis (0 = none, 1 = occlusion).
    Stenosis {
        branch: String,
        severity: f64,
    },
    /// Defibrillation shock (resets all excitable tissue), optionally synchronised on R.
    Shock {
        joules: f64,
        synchronised: bool,
    },
    /// Drug effect preset by name (adenosine, verapamil, flecainide, digoxin, …).
    Drug {
        name: String,
        dose: f64,
    },
    /// Move an electrode (torso-surface coordinates, mm).
    MoveElectrode {
        electrode: Electrode,
        position_mm: [f64; 3],
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StimSite {
    /// A named landmark from the anatomy pack (e.g. "rv_apex", "his_prox", "cs_5_6").
    Landmark(String),
    /// Explicit cell index.
    Cell(u32),
    /// Nearest cell to a point in heart coordinates (mm).
    Point([f64; 3]),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedAction {
    pub at: Millis,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub format_version: u32,
    /// Stable id, e.g. "sinus_normal", "lbbb", "af".
    pub id: String,
    pub title: String,
    pub description: String,
    pub tier: EngineTier,
    /// Anatomy pack id (mesh + cell cloud + landmarks), see `heartsim-geometry`.
    pub anatomy: String,
    pub parameters: Parameters,
    /// Cells rendered non-excitable at t = 0 (scar / previous ablation), by index.
    pub ablated_cells: Vec<u32>,
    pub timeline: Vec<TimedAction>,
    /// Optional electrode overrides (default positions come from the anatomy pack).
    pub electrodes_mm: BTreeMap<Electrode, [f64; 3]>,
    /// Deterministic seed for noise, ectopy timing, fibrillation initiation.
    pub seed: u64,
    /// Teaching text and quiz hooks (markdown), optional.
    pub notes: Option<String>,
}

impl Scenario {
    #[must_use]
    pub fn sinus_normal() -> Self {
        Self {
            format_version: SCENARIO_FORMAT_VERSION,
            id: "sinus_normal".into(),
            title: "Sinus rhythm, normal axis".into(),
            description: "Healthy adult heart at 70 bpm.".into(),
            tier: EngineTier::Eikonal,
            anatomy: "default".into(),
            parameters: Parameters::default(),
            ablated_cells: Vec::new(),
            timeline: Vec::new(),
            electrodes_mm: BTreeMap::new(),
            seed: 42,
            notes: None,
        }
    }

    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default().struct_names(true))
    }

    pub fn from_ron(text: &str) -> Result<Self, ScenarioError> {
        let s: Self = ron::from_str(text)?;
        if s.format_version > SCENARIO_FORMAT_VERSION {
            return Err(ScenarioError::Version(s.format_version));
        }
        Ok(s)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScenarioError {
    #[error("scenario parse error: {0}")]
    Parse(#[from] ron::error::SpannedError),
    #[error("scenario format version {0} is newer than supported ({SCENARIO_FORMAT_VERSION})")]
    Version(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ron_roundtrip_with_timeline() {
        let mut s = Scenario::sinus_normal();
        s.timeline.push(TimedAction {
            at: Millis(2000.0),
            action: Action::PacingTrain {
                site: StimSite::Landmark("rv_apex".into()),
                s1_cycle_ms: 600.0,
                s1_count: 8,
                s2_ms: Some(300.0),
                s3_ms: None,
            },
        });
        s.timeline.push(TimedAction {
            at: Millis(9000.0),
            action: Action::Shock {
                joules: 200.0,
                synchronised: true,
            },
        });
        let text = s.to_ron().unwrap();
        let back = Scenario::from_ron(&text).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn rejects_future_version() {
        let mut s = Scenario::sinus_normal();
        s.format_version = SCENARIO_FORMAT_VERSION + 1;
        let text = s.to_ron().unwrap();
        assert!(matches!(
            Scenario::from_ron(&text),
            Err(ScenarioError::Version(_))
        ));
    }
}
