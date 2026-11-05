//! Translated display names of domain enums (B19).
//!
//! The domain crates keep their English `label()` methods as stable identifiers; what the user reads
//! comes from these functions, so each name has a catalogue key and a translation.

use crate::t;

/// Translated display name of a [`CoronaryBranch`].
#[must_use]
pub fn branch_name(value: heartsim_sim::ischaemia::CoronaryBranch) -> String {
    #[allow(clippy::enum_glob_use)]
    use heartsim_sim::ischaemia::CoronaryBranch::*;
    match value {
        LeftMain => t!("branch-left-main"),
        Lad => t!("branch-lad"),
        Lcx => t!("branch-lcx"),
        ObtuseMarginal => t!("branch-obtuse-marginal"),
        Posterolateral => t!("branch-posterolateral"),
        Rca => t!("branch-rca"),
        Pda => t!("branch-pda"),
        Global => t!("branch-global"),
        Pericardium => t!("branch-pericardium"),
    }
}

/// Translated display name of a [`IschaemiaPreset`].
#[must_use]
pub fn preset_name(value: heartsim_sim::ischaemia::IschaemiaPreset) -> String {
    #[allow(clippy::enum_glob_use)]
    use heartsim_sim::ischaemia::IschaemiaPreset::*;
    match value {
        AnteriorStemi => t!("preset-anterior-stemi"),
        InferiorStemi => t!("preset-inferior-stemi"),
        LateralStemi => t!("preset-lateral-stemi"),
        PosteriorStemi => t!("preset-posterior-stemi"),
        SubendocardialIschaemia => t!("preset-subendocardial"),
        Pericarditis => t!("preset-pericarditis"),
        OldAnteriorInfarct => t!("preset-old-anterior"),
        OldInferiorInfarct => t!("preset-old-inferior"),
    }
}

/// Translated display name of a [`RhythmClass`].
#[must_use]
pub fn rhythm_name(value: heartsim_sim::defib::RhythmClass) -> String {
    #[allow(clippy::enum_glob_use)]
    use heartsim_sim::defib::RhythmClass::*;
    match value {
        VentricularFibrillation => t!("rhythm-vf"),
        VentricularTachycardia => t!("rhythm-vt"),
        SupraventricularTachycardia => t!("rhythm-svt"),
        AtrialFibrillationOrFlutter => t!("rhythm-af-flutter"),
        Organised => t!("rhythm-organised"),
        Asystole => t!("rhythm-asystole"),
    }
}

/// Translated display name of a [`Limb`].
#[must_use]
pub fn limb_name(value: heartsim_ecg::Limb) -> String {
    #[allow(clippy::enum_glob_use)]
    use heartsim_ecg::Limb::*;
    match value {
        RightArm => t!("limb-right-arm"),
        LeftArm => t!("limb-left-arm"),
        LeftLeg => t!("limb-left-leg"),
        RightLeg => t!("limb-right-leg"),
    }
}

/// Translated display name of a [`LeadSite`].
#[must_use]
pub fn lead_site_name(value: heartsim_sim::timing::params::LeadSite) -> String {
    #[allow(clippy::enum_glob_use)]
    use heartsim_sim::timing::params::LeadSite::*;
    match value {
        RaAppendage => t!("site-ra-appendage"),
        RvApex => t!("site-rv-apex"),
        RvSeptum => t!("site-rv-septum"),
        HisBundle => t!("site-his-bundle"),
        LvLateral => t!("site-lv-lateral"),
        LvPosterior => t!("site-lv-posterior"),
    }
}

/// Translated display name of a [`Region`].
#[must_use]
pub fn region_name(value: heartsim_core::Region) -> String {
    #[allow(clippy::enum_glob_use)]
    use heartsim_core::Region::*;
    match value {
        SinoatrialNode => t!("region-sinoatrial-node"),
        RightAtrium => t!("region-right-atrium"),
        LeftAtrium => t!("region-left-atrium"),
        CristaTerminalis => t!("region-crista-terminalis"),
        BachmannBundle => t!("region-bachmann"),
        AtrialSeptum => t!("region-atrial-septum"),
        CavotricuspidIsthmus => t!("region-cavotricuspid-isthmus"),
        LeftAtrialAppendage => t!("region-la-appendage"),
        CoronarySinus => t!("region-coronary-sinus"),
        PulmonaryVeinSleeves => t!("region-pulmonary-veins"),
        AvNodeFastPathway => t!("region-av-fast"),
        AvNodeSlowPathway => t!("region-av-slow"),
        AvNodeCompact => t!("region-av-compact"),
        HisBundle => t!("region-his-bundle"),
        RightBundleBranch => t!("region-right-bundle-branch"),
        LeftBundleBranch => t!("region-left-bundle-branch"),
        LeftAnteriorFascicle => t!("region-left-anterior-fascicle"),
        LeftPosteriorFascicle => t!("region-left-posterior-fascicle"),
        PurkinjeRight => t!("region-purkinje-right"),
        PurkinjeLeft => t!("region-purkinje-left"),
        RightVentricle => t!("region-right-ventricle"),
        LeftVentricle => t!("region-left-ventricle"),
        VentricularSeptum => t!("region-ventricular-septum"),
        Scar => t!("region-scar"),
        BorderZone => t!("region-border-zone"),
        Ischaemic => t!("region-ischaemic"),
        AccessoryPathway => t!("region-accessory-pathway"),
    }
}
