//! Electrode sites on the torso, in the heart frame (mm).

use glam::Vec3;
use heartsim_core::Electrode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Named electrode positions. The ten standard sites use the `Electrode::label()` names
/// (`RA, LA, LL, RL, V1…V6`); extra sites are `V3R, V4R, V7, V8, V9`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ElectrodeSet {
    /// Human-readable description of the frame the positions are in.
    pub frame: String,
    pub sites: BTreeMap<String, [f32; 3]>,
}

/// Names of the additional (non-standard) sites every pack provides.
pub const EXTRA_SITES: [&str; 5] = ["V3R", "V4R", "V7", "V8", "V9"];

impl ElectrodeSet {
    /// Position of a standard electrode, if present.
    #[must_use]
    pub fn standard(&self, e: Electrode) -> Option<Vec3> {
        self.get(e.label())
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Vec3> {
        self.sites.get(name).map(|p| Vec3::from(*p))
    }

    pub fn set(&mut self, name: &str, p: Vec3) {
        self.sites.insert(name.to_string(), p.to_array());
    }

    /// True when all ten standard sites are present.
    #[must_use]
    pub fn has_standard_ten(&self) -> bool {
        Electrode::ALL.iter().all(|e| self.sites.contains_key(e.label()))
    }

    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default().struct_names(true))
    }

    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }
}
