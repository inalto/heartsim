//! Anatomical / electrophysiological regions. Every simulation cell belongs to exactly one
//! primary region (and may carry secondary tags such as `Epicardial`).
//!
//! The list deliberately mirrors the vocabulary clinicians use (and that commercial simulators
//! expose as parameter groups): parameters such as APD, CV and restitution are set per region.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Region {
    // --- atria ---
    SinoatrialNode,
    RightAtrium,
    LeftAtrium,
    CristaTerminalis,
    BachmannBundle,
    AtrialSeptum,
    CavotricuspidIsthmus,
    LeftAtrialAppendage,
    CoronarySinus,
    PulmonaryVeinSleeves,
    // --- AV junction ---
    AvNodeFastPathway,
    AvNodeSlowPathway,
    AvNodeCompact,
    HisBundle,
    // --- His–Purkinje ---
    RightBundleBranch,
    LeftBundleBranch,
    LeftAnteriorFascicle,
    LeftPosteriorFascicle,
    PurkinjeRight,
    PurkinjeLeft,
    // --- ventricles ---
    RightVentricle,
    LeftVentricle,
    VentricularSeptum,
    // --- pathological tissue ---
    Scar,
    BorderZone,
    Ischaemic,
    AccessoryPathway,
}

impl Region {
    pub const ALL: [Region; 27] = [
        Region::SinoatrialNode,
        Region::RightAtrium,
        Region::LeftAtrium,
        Region::CristaTerminalis,
        Region::BachmannBundle,
        Region::AtrialSeptum,
        Region::CavotricuspidIsthmus,
        Region::LeftAtrialAppendage,
        Region::CoronarySinus,
        Region::PulmonaryVeinSleeves,
        Region::AvNodeFastPathway,
        Region::AvNodeSlowPathway,
        Region::AvNodeCompact,
        Region::HisBundle,
        Region::RightBundleBranch,
        Region::LeftBundleBranch,
        Region::LeftAnteriorFascicle,
        Region::LeftPosteriorFascicle,
        Region::PurkinjeRight,
        Region::PurkinjeLeft,
        Region::RightVentricle,
        Region::LeftVentricle,
        Region::VentricularSeptum,
        Region::Scar,
        Region::BorderZone,
        Region::Ischaemic,
        Region::AccessoryPathway,
    ];

    #[must_use]
    pub fn is_atrial(self) -> bool {
        matches!(
            self,
            Region::SinoatrialNode
                | Region::RightAtrium
                | Region::LeftAtrium
                | Region::CristaTerminalis
                | Region::BachmannBundle
                | Region::AtrialSeptum
                | Region::CavotricuspidIsthmus
                | Region::LeftAtrialAppendage
                | Region::CoronarySinus
                | Region::PulmonaryVeinSleeves
        )
    }

    #[must_use]
    pub fn is_conduction_system(self) -> bool {
        matches!(
            self,
            Region::AvNodeFastPathway
                | Region::AvNodeSlowPathway
                | Region::AvNodeCompact
                | Region::HisBundle
                | Region::RightBundleBranch
                | Region::LeftBundleBranch
                | Region::LeftAnteriorFascicle
                | Region::LeftPosteriorFascicle
                | Region::PurkinjeRight
                | Region::PurkinjeLeft
        )
    }

    #[must_use]
    pub fn is_ventricular(self) -> bool {
        matches!(
            self,
            Region::RightVentricle | Region::LeftVentricle | Region::VentricularSeptum
        )
    }

    /// Human-readable label (English; UI localises through fluent keys named after `Debug`).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Region::SinoatrialNode => "SA node",
            Region::RightAtrium => "Right atrium",
            Region::LeftAtrium => "Left atrium",
            Region::CristaTerminalis => "Crista terminalis",
            Region::BachmannBundle => "Bachmann's bundle",
            Region::AtrialSeptum => "Atrial septum",
            Region::CavotricuspidIsthmus => "Cavotricuspid isthmus",
            Region::LeftAtrialAppendage => "LA appendage",
            Region::CoronarySinus => "Coronary sinus",
            Region::PulmonaryVeinSleeves => "Pulmonary vein sleeves",
            Region::AvNodeFastPathway => "AV node – fast pathway",
            Region::AvNodeSlowPathway => "AV node – slow pathway",
            Region::AvNodeCompact => "AV node – compact",
            Region::HisBundle => "Bundle of His",
            Region::RightBundleBranch => "Right bundle branch",
            Region::LeftBundleBranch => "Left bundle branch",
            Region::LeftAnteriorFascicle => "Left anterior fascicle",
            Region::LeftPosteriorFascicle => "Left posterior fascicle",
            Region::PurkinjeRight => "Purkinje (right)",
            Region::PurkinjeLeft => "Purkinje (left)",
            Region::RightVentricle => "Right ventricle",
            Region::LeftVentricle => "Left ventricle",
            Region::VentricularSeptum => "Ventricular septum",
            Region::Scar => "Scar",
            Region::BorderZone => "Border zone",
            Region::Ischaemic => "Ischaemic tissue",
            Region::AccessoryPathway => "Accessory pathway",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_is_exclusive() {
        for r in Region::ALL {
            let n =
                u8::from(r.is_atrial()) + u8::from(r.is_conduction_system()) + u8::from(r.is_ventricular());
            assert!(n <= 1, "{r:?} classified twice");
        }
    }
}
