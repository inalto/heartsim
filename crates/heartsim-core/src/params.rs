//! Electrophysiology parameters. One `RegionParams` per [`Region`], plus global settings.
//! These are the knobs the UI exposes and the pathology files override.

use crate::region::Region;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Per-region tissue parameters (Tier 1 vocabulary; Tier 2 models map these onto their own
/// constants, e.g. Mitchell–Schaeffer τ_close ↔ APD).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RegionParams {
    /// Action potential duration at a long diastolic interval, ms (epicardial value).
    pub apd_ms: f64,
    /// Endocardial APD; blended with `apd_ms` by transmural depth. `None` = same as epi.
    pub apd_endo_ms: Option<f64>,
    /// Conduction velocity along the fibre, m/s.
    pub cv_long_m_s: f64,
    /// Transverse / longitudinal CV ratio (1 = isotropic). Typical myocardium 0.3–0.5.
    pub anisotropy_ratio: f64,
    /// Effective refractory period as a fraction of APD (cells cannot be re-excited before).
    pub erp_fraction: f64,
    /// APD restitution: APD(DI) = apd · (1 − a · exp(−DI / τ)). `a` here.
    pub apd_rest_amplitude: f64,
    /// APD restitution time constant τ, ms.
    pub apd_rest_tau_ms: f64,
    /// CV restitution: CV(DI) = cv · (1 − a · exp(−DI / τ)).
    pub cv_rest_amplitude: f64,
    pub cv_rest_tau_ms: f64,
    /// Intrinsic automaticity rate, bpm (0 = none). SA ≈ 70, AV junction ≈ 45, Purkinje ≈ 30.
    pub automaticity_bpm: f64,
    /// Whether the region conducts at all (false = block / scar).
    pub conducts: bool,
}

impl RegionParams {
    #[must_use]
    pub const fn working_myocardium(apd_ms: f64, cv: f64) -> Self {
        Self {
            apd_ms,
            apd_endo_ms: None,
            cv_long_m_s: cv,
            anisotropy_ratio: 0.4,
            erp_fraction: 0.9,
            apd_rest_amplitude: 0.3,
            apd_rest_tau_ms: 120.0,
            cv_rest_amplitude: 0.1,
            cv_rest_tau_ms: 60.0,
            automaticity_bpm: 0.0,
            conducts: true,
        }
    }
}

/// Global, non-regional settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalParams {
    pub sinus_rate_bpm: f64,
    /// Respiratory sinus arrhythmia depth (fraction of RR) and rate (breaths/min).
    pub respiration_depth: f64,
    pub respiration_rate_bpm: f64,
    /// AV nodal conduction delay at rest, ms, and its decremental slope.
    pub av_delay_ms: f64,
    pub av_decrement_ms_per_ms: f64,
    pub av_erp_ms: f64,
    pub av_retrograde: bool,
    /// Exponent p in the 1/r^p electrode weighting of the Tier-1 pseudo-ECG (2 = dipole far field).
    pub ecg_distance_power: f64,
    pub ecg_noise_mv: f64,
    pub ecg_lowpass_hz: f64,
}

impl Default for GlobalParams {
    fn default() -> Self {
        Self {
            sinus_rate_bpm: 70.0,
            respiration_depth: 0.0,
            respiration_rate_bpm: 12.0,
            av_delay_ms: 90.0,
            av_decrement_ms_per_ms: 0.15,
            av_erp_ms: 300.0,
            av_retrograde: false,
            ecg_distance_power: 2.0,
            ecg_noise_mv: 0.0,
            ecg_lowpass_hz: 40.0,
        }
    }
}

/// The complete parameter set of a simulation state. Serialised inside scenarios.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameters {
    pub global: GlobalParams,
    pub regions: BTreeMap<Region, RegionParams>,
}

impl Default for Parameters {
    /// Textbook healthy adult values (literature ranges; tune in `assets/scenarios/sinus.ron`).
    fn default() -> Self {
        use Region as R;
        let mut regions = BTreeMap::new();
        let mut set = |r: R, p: RegionParams| {
            regions.insert(r, p);
        };
        // Atrial working myocardium is isotropic here. The default pack's atria are one lattice
        // cell thick, and on such a wall the eikonal engine and the monodomain grid disagree
        // about anisotropic conduction (the Tier-2 sinus beat lagged Tier 1 by 26 ms RMS with
        // the fibre tensor and 12 ms without), while the two bundles below keep theirs.
        let atrium = RegionParams {
            anisotropy_ratio: 1.0,
            ..RegionParams::working_myocardium(230.0, 0.8)
        };
        for r in [
            R::RightAtrium,
            R::LeftAtrium,
            R::AtrialSeptum,
            R::CavotricuspidIsthmus,
            R::LeftAtrialAppendage,
            R::CoronarySinus,
            R::PulmonaryVeinSleeves,
        ] {
            set(r, atrium);
        }
        set(
            R::SinoatrialNode,
            RegionParams {
                automaticity_bpm: 70.0,
                cv_long_m_s: 0.05,
                ..atrium
            },
        );
        set(
            R::CristaTerminalis,
            RegionParams {
                cv_long_m_s: 1.2,
                anisotropy_ratio: 0.2,
                ..atrium
            },
        );
        // The coronary-sinus musculature is a sleeve of parallel fibres along the vessel; it
        // conducts the sinus wave from the ostium along the mitral annulus ahead of the left
        // atrial wall it lies on (Chauvin et al. 2000), which is what makes a CS catheter read
        // proximal to distal.
        set(
            R::CoronarySinus,
            RegionParams {
                cv_long_m_s: 1.2,
                anisotropy_ratio: 0.2,
                ..atrium
            },
        );
        set(
            R::BachmannBundle,
            RegionParams {
                cv_long_m_s: 1.5,
                anisotropy_ratio: 0.2,
                ..atrium
            },
        );
        let av = RegionParams {
            apd_ms: 250.0,
            cv_long_m_s: 0.05,
            anisotropy_ratio: 1.0,
            erp_fraction: 1.1,
            apd_rest_amplitude: 0.0,
            cv_rest_amplitude: 0.6,
            cv_rest_tau_ms: 150.0,
            automaticity_bpm: 0.0,
            ..atrium
        };
        set(R::AvNodeFastPathway, av);
        set(
            R::AvNodeSlowPathway,
            RegionParams {
                cv_long_m_s: 0.02,
                erp_fraction: 0.8,
                ..av
            },
        );
        set(
            R::AvNodeCompact,
            RegionParams {
                automaticity_bpm: 45.0,
                ..av
            },
        );
        let purk = RegionParams {
            apd_ms: 330.0,
            cv_long_m_s: 3.0,
            anisotropy_ratio: 1.0,
            apd_rest_amplitude: 0.2,
            cv_rest_amplitude: 0.0,
            automaticity_bpm: 30.0,
            ..atrium
        };
        for r in [
            R::HisBundle,
            R::RightBundleBranch,
            R::LeftBundleBranch,
            R::LeftAnteriorFascicle,
            R::LeftPosteriorFascicle,
            R::PurkinjeRight,
            R::PurkinjeLeft,
        ] {
            set(r, purk);
        }
        // Transmural APD gradient (endo 345 → epi 285 ms, mid-wall 315 ms). It must exceed the
        // endo→epi activation time (≈ 25 ms across the LV free wall at the cross-fibre CV) for the
        // epicardium to repolarise first and the T wave to be concordant (research §3.5; within
        // the transmural dispersion measured in wedge preparations by Yan & Antzelevitch 1998;
        // calibrated with the forward model, brief B05).
        let vent = RegionParams {
            apd_endo_ms: Some(345.0),
            ..RegionParams::working_myocardium(285.0, 0.6)
        };
        set(R::RightVentricle, vent);
        set(R::LeftVentricle, vent);
        // Both septal faces are endocardium (depth 0 = LV side, 1 = RV side), so the septum gets
        // no endo→epi blend: a uniform mid-wall APD.
        set(
            R::VentricularSeptum,
            RegionParams {
                apd_ms: 315.0,
                apd_endo_ms: None,
                ..vent
            },
        );
        set(
            R::Scar,
            RegionParams {
                conducts: false,
                ..vent
            },
        );
        set(
            R::BorderZone,
            RegionParams {
                cv_long_m_s: 0.2,
                apd_ms: 340.0,
                ..vent
            },
        );
        set(
            R::Ischaemic,
            RegionParams {
                cv_long_m_s: 0.35,
                apd_ms: 220.0,
                ..vent
            },
        );
        set(
            R::AccessoryPathway,
            RegionParams {
                cv_long_m_s: 1.5,
                conducts: false,
                erp_fraction: 1.0,
                ..atrium
            },
        );
        Self {
            global: GlobalParams::default(),
            regions,
        }
    }
}

impl Parameters {
    #[must_use]
    pub fn region(&self, r: Region) -> RegionParams {
        self.regions
            .get(&r)
            .copied()
            .unwrap_or_else(|| RegionParams::working_myocardium(300.0, 0.6))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_region_has_defaults() {
        let p = Parameters::default();
        for r in Region::ALL {
            assert!(p.regions.contains_key(&r), "{r:?} missing");
        }
    }

    #[test]
    fn ron_roundtrip() {
        let p = Parameters::default();
        let text = ron::ser::to_string_pretty(&p, ron::ser::PrettyConfig::default()).unwrap();
        let back: Parameters = ron::from_str(&text).unwrap();
        assert_eq!(p, back);
    }
}
