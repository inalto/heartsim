//! Every number of the wall-motion model, in one place.
//!
//! Each field names its source in `docs/research/echo-simulator.md` §2.2–2.3 (author, year, the
//! value) or says **designed**: a choice made for this model inside what the sources describe
//! qualitatively. The designed values are what a clinician review should look at first.

/// Parameters of the kinematic wall-motion model (brief B30).
#[derive(Debug, Clone, PartialEq)]
pub struct MotionParams {
    // --- contraction of one cell -----------------------------------------------------------------
    /// Ventricular electromechanical delay, activation to the start of shortening, ms. **Designed**:
    /// no healthy local value was found (§2.2); short enough that the earliest regions are
    /// shortening at M1 = QRS + 40 ms (`PcgParams::q_m1_ms`).
    pub emd_ventricle_ms: f64,
    /// Atrial electromechanical delay, activation to the start of atrial shortening, ms.
    /// **Designed**, inside the 50–80 ms of §2.2.
    pub emd_atrium_ms: f64,
    /// Rise of a cell's contraction from 0 to its plateau, ms. **Designed**: "a smooth rise over
    /// about 80 ms" (§2.3).
    pub rise_ms: f64,
    /// Contraction lasts this fraction of the cell's APD (`k` of §2.3). **Designed** (≈ 0.9) so
    /// that shortening ends near A2; anchored on T-end ≈ S2 − 5 ms (Sbrollini 2017, cited in
    /// `docs/research/heart-sounds.md`).
    pub apd_fraction: f64,
    /// The decay takes this last fraction of the contraction. **Designed**: §2.3 proposes the last
    /// third; half is used because the steeper decay of a third, times the regional spread of
    /// activation and APD, folded triangles of the RV outflow tract in the mesh-safety test.
    pub decay_fraction: f64,

    // --- valve-gated phases ----------------------------------------------------------------------
    /// Left isovolumic contraction time, M1 to aortic opening, ms. Alhakak et al. 2023
    /// (doi:10.1007/s00392-023-02269-2, abstract): IVCT 40 ± 10 ms, n = 1969.
    pub lv_ivct_ms: f64,
    /// Left isovolumic relaxation time, A2 to mitral opening, ms. **Designed** inside the sources:
    /// Nagueh 2016 (JASE 29:277, p282) "≤ 70 msec in normal subjects", Alhakak 2023 96 ± 19 ms;
    /// §2.2 suggests 70–90 ms.
    pub lv_ivrt_ms: f64,
    /// Right isovolumic contraction time, T1 to pulmonary opening, ms. **Designed** (shorter than
    /// the left: lower afterload; no number was reached).
    pub rv_ivct_ms: f64,
    /// Right isovolumic relaxation time, P2 to tricuspid opening, ms. **Designed** (as above).
    pub rv_ivrt_ms: f64,
    /// Rapid filling: acceleration from mitral opening to the peak filling rate, ms. **Designed**
    /// (no reached source gave the durations of the filling phases, §2.2).
    pub e_acceleration_ms: f64,
    /// Rapid filling: peak filling rate to its end, ms. Lang 2015 (EHJ-CVI 16:233, p254 Table 10):
    /// deceleration time 180 ± 31 ms.
    pub e_deceleration_ms: f64,
    /// Share of the passive filling that arrives slowly, in diastasis. **Designed**.
    pub diastasis_share: f64,
    /// Time constant of the slow (diastasis) filling, ms. **Designed**.
    pub diastasis_tau_ms: f64,
    /// Share of the filling the atrial contraction brings; it is lost when the atria do not
    /// contract (fibrillation, flutter, silent atria, or an atrial contraction against closed AV
    /// valves). **Designed**: the sources reached give E/A = 1.4 ± 0.3 as velocities (Lang 2015),
    /// not volumes.
    pub atrial_share: f64,
    /// Duration of an atrial contraction's filling push, ms. **Designed**.
    pub atrial_contraction_ms: f64,

    // --- beat-to-beat ejection fraction ----------------------------------------------------------
    /// LVEF of a steady sinus beat. Lang 2015 (p239 Table 2): men 62 ± 5 %, women 64 ± 5 %.
    pub lvef: f64,
    /// RVEF of a steady sinus beat. **Designed** (the sources reached give FAC 49 ± 7 %, Lang 2015
    /// Table 10, not a volume fraction); only the right ventricle's mean excursion follows it.
    pub rvef: f64,
    /// Below this preceding RR the stroke volume falls with the RR; above it the RR no longer
    /// matters, ms. Iwase et al. 1988 (doi:10.1007/bf01797720, abstract): in AF the stroke volume
    /// rises with the preceding RR only below about 600 ms.
    pub ef_rr_plateau_ms: f64,
    /// Exponent of that fall, `(RR / plateau)^exponent`. **Designed**.
    pub ef_rr_exponent: f64,
    /// Post-extrasystolic potentiation: gain per unit of prematurity of the pre-preceding beat
    /// (`1 + gain · (1 − RR_prevprev / RR_prev)`). **Designed** to give ≈ 72 % after a 50 %-premature
    /// PVC with a full compensatory pause, near the canine 74 ± 6 % of Kowlgi et al. 2020
    /// (doi:10.1093/europace/euaa025); Iwase 1988: a shorter pre-preceding RR gives a larger
    /// stroke volume.
    pub post_extrasystolic_gain: f64,
    /// Ceiling of any beat's ejection fraction. **Designed**.
    pub ef_max: f64,

    // --- the displacement field ------------------------------------------------------------------
    /// Mitral annular plane systolic excursion of a steady sinus beat, mm (the long-axis
    /// component at the annulus). Matos 2012 (doi:10.1016/j.echo.2012.06.011, abstract): normal-EF
    /// cut-off ≥ 13 mm in men, ≥ 11 mm in women; 12–15 mm typical (**unverified**, §2.2). 14 mm.
    pub mapse_mm: f64,
    /// Tricuspid annular plane systolic excursion of a steady sinus beat, mm. Lang 2015 (p254 Table
    /// 10): 24 ± 3.5 mm, abnormal < 17.
    pub tapse_mm: f64,
    /// Epicardial radial excursion as a fraction of the endocardial one, at the same place. Below
    /// 1 the wall thickens. **Designed**; the endocardial excursion itself is solved at
    /// construction so that the steady beat ejects [`Self::lvef`] with [`Self::mapse_mm`]
    /// (§2.2: ≈ 33 % thickening and ≈ 35 % fractional shortening are inferences, not sources).
    pub epi_radial_ratio: f64,
    /// Right-ventricular endocardial radial excursion at full contraction, fraction of the
    /// distance to the RV axis. **Designed** (RV FAC 49 ± 7 %, Lang 2015, is an area fraction of
    /// one view; the pack's right ventricle has no closed cavity to solve on).
    pub rv_radial_fraction: f64,
    /// LV rotation at the apex at end-systole, degrees, right-handed about the apex-to-base axis
    /// (negative = counter-clockwise seen from the apex). Helle-Valle et al. 2005
    /// (doi:10.1161/circulationaha.104.531558, abstract): apex −11.6 ± 3.8° (MRI); sign as viewed
    /// from the apex (Sengupta 2008).
    pub apical_rotation_deg: f64,
    /// LV rotation at the base at end-systole, degrees (same convention). Helle-Valle 2005:
    /// +4.8 ± 1.7°.
    pub basal_rotation_deg: f64,
    /// Atrial wall excursion toward the atrium's centre at full contraction, fraction of the
    /// distance. **Designed**.
    pub atrial_radial_fraction: f64,
    /// Contraction amplitude left to fibrillating or fluttering atria (a quiver), fraction of an
    /// organised contraction. **Designed** (§2.3: "designed small atrial quiver").
    pub atrial_quiver_fraction: f64,
    /// Contraction amplitude left to the ventricles in fibrillation (incoherent, volume held),
    /// fraction of an organised contraction. **Designed** (§2.3: "low-amplitude incoherent wall
    /// motion").
    pub vf_quiver_fraction: f64,
    /// Height (fraction of the LV long axis) at and below which the apical rotation applies.
    /// **Designed**: the apical short-axis level the rotation sources measure at.
    pub rotation_apex_level: f64,
    /// Height at and above which the basal rotation applies (linear in between). **Designed**:
    /// the basal short-axis level.
    pub rotation_base_level: f64,
    /// The radial excursion is full up to this height and tapers above it. **Designed**.
    pub radial_taper_from: f64,
    /// Radial excursion at the valve plane as a fraction of the mid-ventricular one: the annulus
    /// narrows less than the cavity. **Designed**.
    pub basal_radial_fraction: f64,
    /// The apical cap below this height does not move along the long axis (the apex is nearly
    /// still; compressing the cap along the axis would thin it). **Designed**.
    pub apical_still_height: f64,
    /// Below this height the inward motion converges on one point of the axis, so the apical cap
    /// closes toward the cavity and thickens. **Designed**.
    pub apical_centre_height: f64,
    /// The RV's inward excursion stops growing with the distance from its axis beyond about this,
    /// mm (the infundibulum lies up to 7 cm from the inflow axis). **Designed**.
    pub rv_radius_cap_mm: f64,
    /// The RV's long-axis excursion halves (about) at this distance from its axis, mm (the
    /// infundibulum does not descend with the tricuspid annulus). **Designed**.
    pub rv_long_fade_mm: f64,
    /// Mechanical coupling of the contraction: the cells' contraction is averaged on a voxel grid
    /// of this spacing, mm, per chamber. Myocardium is mechanically coupled, so neighbouring cells
    /// do not shorten independently. **Designed**.
    pub coupling_voxel_mm: f64,
    /// Passes of neighbour averaging on that grid. **Designed**: enough that the scatter of
    /// activation and APD from cell to cell does not fold the mesh; regional differences over
    /// centimetres (LBBB) remain.
    pub coupling_passes: usize,
    /// Outflow left at the closure of the outflow valve, relative to the flow at its opening (the
    /// ejection curve `(1 + 3τ)(1 − τ) + k`). **Designed**.
    pub ejection_end_flow: f64,
    /// A great vessel follows its root (annulus or atrial wall) fully at the root and not at all
    /// this far from it, mm. **Designed**.
    pub vessel_fade_mm: f64,
    /// Caudal shift of the whole heart at peak inspiration, mm. **Designed** (§2.3: "a few
    /// millimetres"; no source reached).
    pub respiratory_shift_mm: f64,
}

impl Default for MotionParams {
    fn default() -> Self {
        Self {
            emd_ventricle_ms: 20.0,
            emd_atrium_ms: 60.0,
            rise_ms: 80.0,
            apd_fraction: 0.9,
            decay_fraction: 0.5,
            lv_ivct_ms: 40.0,
            lv_ivrt_ms: 80.0,
            rv_ivct_ms: 25.0,
            rv_ivrt_ms: 50.0,
            e_acceleration_ms: 80.0,
            e_deceleration_ms: 180.0,
            diastasis_share: 0.1,
            diastasis_tau_ms: 300.0,
            atrial_share: 0.2,
            atrial_contraction_ms: 100.0,
            lvef: 0.62,
            rvef: 0.55,
            ef_rr_plateau_ms: 600.0,
            ef_rr_exponent: 0.5,
            post_extrasystolic_gain: 0.25,
            ef_max: 0.8,
            mapse_mm: 14.0,
            tapse_mm: 24.0,
            epi_radial_ratio: 0.2,
            rv_radial_fraction: 0.25,
            apical_rotation_deg: -11.6,
            basal_rotation_deg: 4.8,
            atrial_radial_fraction: 0.12,
            atrial_quiver_fraction: 0.05,
            vf_quiver_fraction: 0.06,
            rotation_apex_level: 0.2,
            rotation_base_level: 0.85,
            radial_taper_from: 0.7,
            basal_radial_fraction: 0.7,
            apical_still_height: 0.15,
            apical_centre_height: 0.3,
            rv_radius_cap_mm: 30.0,
            rv_long_fade_mm: 45.0,
            coupling_voxel_mm: 7.0,
            coupling_passes: 12,
            ejection_end_flow: 0.25,
            vessel_fade_mm: 50.0,
            respiratory_shift_mm: 4.0,
        }
    }
}

impl MotionParams {
    /// Ejection-fraction multiplier of a beat from its preceding RR (`rr_ms`) and the one before
    /// (`rr_prev_ms`): a premature beat ejects less, the beat after a short cycle more (see
    /// [`Self::ef_rr_plateau_ms`], [`Self::post_extrasystolic_gain`]). 1 for a steady rhythm at or
    /// below 100 bpm.
    #[must_use]
    pub fn ef_factor(&self, rr_ms: Option<f64>, rr_prev_ms: Option<f64>) -> f64 {
        use heartsim_core::math::Deterministic;
        let Some(rr) = rr_ms.filter(|r| *r > 0.0) else {
            return 1.0;
        };
        let restitution = (rr / self.ef_rr_plateau_ms).min(1.0).powf_d(self.ef_rr_exponent);
        let potentiation = rr_prev_ms.filter(|p| *p > 0.0).map_or(1.0, |p| {
            1.0 + self.post_extrasystolic_gain * (1.0 - p / rr).max(0.0)
        });
        restitution * potentiation
    }
}
