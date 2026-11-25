//! The parts the data lacks (valve leaflets, papillary muscles, moderator band, pericardium) and
//! the probe's designed limits: every dimension and timing in one struct, each with the primary
//! source it was read on and its locator, or the word **designed** (brief B31, item 4).
//!
//! What "read" means here: the number was read on the cited page during the B31 work (full text
//! or abstract, as stated). Nothing was taken from memory or from a secondary website.

use crate::history::MotionHistory;
use crate::params::MotionParams;
use crate::phases::{CardiacPhase, Side, VolumeInputs, smoothstep, volume_at};
use heartsim_ecg::Valve;

/// Dimensions and timings of the designed parts and the probe.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignedParams {
    // --- leaflets ---------------------------------------------------------------------------
    /// Anterior mitral leaflet length, insertion to tip, mm. **25.1 mm**: Tarkiainen et al.,
    /// *J Cardiovasc Magn Reson* 2016;18:33, doi:10.1186/s12968-016-0250-5, Table 1, "archive"
    /// controls with normal CMR (n = 71, 25.1 ± 3.7 mm), 3-chamber cine in diastole (full text).
    pub mitral_anterior_length_mm: f32,
    /// Posterior mitral leaflet length, mm. **14.4 mm**: same table and cohort (14.4 ± 3.6 mm).
    pub mitral_posterior_length_mm: f32,
    /// Overlap of the closed AV leaflets at their tips (the coaptation zone), subtracted from the
    /// lengths before the closed leaflets are made to meet, mm. **Designed** (no source read for
    /// the mitral or tricuspid coaptation length).
    pub av_coaptation_mm: f32,
    /// Tricuspid leaflet height (annulus to free margin), mm, one value for the three leaflets.
    /// **18.9 mm** = mean of anterior 20.71, posterior 18.88 and septal 17.22 mm: Skwarek et al.,
    /// *Folia Morphol* 2006;65(3):200–208, Results (107 formalin-fixed adult hearts; full text).
    /// The three leaflets share one coaptation point, so their individual heights are not kept.
    pub tricuspid_height_mm: f32,
    /// Height of the closed aortic cusps' central coaptation above the annulus, mm. **8.6 mm**
    /// ("effective height", 8.6 ± 1.4 mm): Izawa et al., *Circ J* 2021;85(7):1059–1067,
    /// doi:10.1253/circj.CJ-20-0938, Table 2 (CT, n = 123 normal valves; full text). With the
    /// disc's radius this gives the cusp length (geometric height 14.7 ± 1.3 mm in the same table).
    pub aortic_effective_height_mm: f32,
    /// The same for the pulmonary cusps, mm. **Designed**: equal to the aortic value (no
    /// pulmonary cusp dimension was found).
    pub pulmonary_effective_height_mm: f32,
    /// Leaflet thickness drawn for the mitral and tricuspid valves, mm. **1.6 mm**: anterior
    /// mitral leaflet, ages 20–59, Sahasakul et al., *Am J Cardiol* 1988;62(7):424–430,
    /// doi:10.1016/0002-9149(88)90971-X (autopsy, 200 normal hearts; abstract only).
    pub av_leaflet_thickness_mm: f32,
    /// Semilunar cusp thickness drawn, mm. **1.0 mm** = one heart voxel, the floor the brief sets;
    /// the cusps are thinner (aortic nodule 0.87 mm at 20–59, Sahasakul 1988, abstract).
    pub cusp_thickness_mm: f32,
    /// Angle between a fully open leaflet and the valve axis, leaning toward it, degrees.
    /// **Designed**.
    pub open_angle_from_axis_deg: f32,
    // --- valve timing -----------------------------------------------------------------------
    /// Rapid opening of the semilunar valves, ms. **27.5 ms**: aortic "rapid opening time"
    /// 27.5 ± 6.7 ms, Zhong et al., *J Magn Reson Imaging* 2021;54(4):1246–1254,
    /// doi:10.1002/jmri.27603 (MRI, 12 healthy subjects; abstract only). The pulmonary valve takes
    /// the same (designed).
    pub semilunar_opening_ms: f64,
    /// Rapid closure of the semilunar valves, ending at A2/P2, ms. **43.8 ms**: rapid closing time
    /// 43.8 ± 11.6 ms, same abstract.
    pub semilunar_closing_ms: f64,
    /// Slow systolic closure: fraction of the maximal opening lost over ejection before the rapid
    /// closure. **0.125**: 12.5 ± 6.6 % in normal controls, Leyh et al., *Circulation*
    /// 1999;100(21):2153–2160, doi:10.1161/01.cir.100.21.2153 (TEE M-mode, n = 10; abstract only).
    pub semilunar_slow_closure: f64,
    /// Opening of the AV valves (from the end of isovolumic relaxation, and again with the atrial
    /// contraction), ms. **Designed** (no mitral M-mode timing source was found).
    pub av_opening_ms: f64,
    /// Closure of the AV valves, starting at M1/T1, ms. **Designed**.
    pub av_closing_ms: f64,
    /// Opening of the AV valves through diastasis (the partial mid-diastolic closure), fraction
    /// of full. **Designed**.
    pub av_diastasis_opening: f64,
    /// Duration of that partial closure after the E wave, ms. **Designed**.
    pub av_mid_closure_ms: f64,
    // --- papillary muscles ------------------------------------------------------------------
    /// Distance of the anterolateral and posteromedial muscle tips from the mitral annulus, mm.
    /// **20.7 / 22.75 mm**: "highest apex to annulus distance", median 20.71 mm (superior group)
    /// and 22.75 ± 4.97 mm (inferior group): Çandır et al., *Folia Morphol* 2025;84(1):83–96,
    /// doi:10.5603/fm.99585, Table 8, non-pathologic hearts (n = 89; fresh autopsy; full text).
    pub papillary_tip_to_annulus_mm: [f32; 2],
    /// Diameter of each muscle at its base, mm. **7.98 / 8.44 mm**: width at the origin of the
    /// LV anterior and posterior muscles, Bhadoria et al., *Cureus* 2022;14(2):e22722,
    /// doi:10.7759/cureus.22722, Table 1 (50 formalin-fixed hearts; full text).
    pub papillary_base_diameter_mm: [f32; 2],
    /// Tip diameter as a fraction of the base. **Designed**.
    pub papillary_tip_fraction: f32,
    /// How far the base is sunk into the wall, mm (so the body touches it). **Designed**.
    pub papillary_embed_mm: f32,
    // Position: at mid-ventricular level, on the boundary between AHA segments 11 and 12
    // (anterolateral) and 9 and 10 (posteromedial) — **designed** from the descriptive positions
    // (no AHA-segment or clock-face position was read on a primary source).
    // --- moderator band ---------------------------------------------------------------------
    /// Band thickness (diameter), mm. **4.5 mm**: 4.5 ± 1.8 mm, Loukas et al., *Clin Anat*
    /// 2010;23(4):443–450, doi:10.1002/ca.20968 (100 cadaver hearts; abstract only). Lee & Hur,
    /// *Anat Cell Biol* 2019;52(1):38–42, give 4.6 mm (full text).
    pub moderator_thickness_mm: f32,
    /// Septal origin as a fraction of the tricuspid-valve-to-apex distance. **0.55**: the upper
    /// limit of the most frequent class (45–55 %, 45 of 100 hearts; > 55 % in 39), Loukas 2010
    /// (abstract). The brief asks for a band in the apical third; the source puts the origin at
    /// about half way, and the insertion below is further toward the apex.
    pub moderator_septal_fraction: f32,
    /// The free-wall insertion is the nearest RV free wall across the cavity from the origin,
    /// within ±50° of the direction away from the septum, tilted toward the apex by at most this
    /// (tangent of the tilt). **Designed**. (Round 1 used a fixed insertion at 0.70 of the
    /// tricuspid→apex distance, which made the band 32 mm long against 16 mm in Loukas 2010.)
    pub moderator_max_apical_tilt: f32,
    // --- pericardium ------------------------------------------------------------------------
    /// Shell thickness outside the epicardium, mm. **1.5 mm**: inside the reported normal
    /// ranges, 1.7–1.8 ± 0.5 mm on CT (Choi et al., *J Korean Radiol Soc* 1998;39(2):289–292,
    /// doi:10.3348/jkrs.1998.39.2.289) and 1.2 ± 0.5 mm in diastole on MRI (Sechtem et al.,
    /// *AJR* 1986;147(2):239–244, doi:10.2214/ajr.147.2.239); both abstracts only.
    pub pericardium_mm: f32,
    // --- probe ------------------------------------------------------------------------------
    /// The beam loses contact beyond this angle from the inward skin normal, degrees.
    /// **Designed** (brief B31: "about 60°").
    pub contact_limit_deg: f32,
    /// Air between the probe face and the KCL skin up to this depth is drawn as soft tissue (the
    /// figure's skin and the KCL skin differ by up to 1–2 cm), mm. **Designed**.
    pub skin_gap_mm: f32,
    /// Fraction of the heart's respiratory shift that the thorax volume moves by. **Designed**
    /// (no source for thoracic organ displacement with respiration was read).
    pub thorax_respiratory_fraction: f32,
    /// Footprint radius over which the skin normal is averaged, mm. **Designed** (about half a
    /// phased-array face).
    pub skin_normal_radius_mm: f32,
    // --- lung-edge retraction (the parasternal window) ---------------------------------------
    // In the supine KCL subject a thin layer of left lung covers the heart along the whole left
    // sternal border, so a parasternal image would be hidden entirely. In a real examination the
    // heart is reached there through the cardiac notch of the left lung (the "bare area" of the
    // pericardium), helped by expiration and the left lateral position. **Designed** remedy, applied
    // at lookup time (the baked thorax volume is unchanged): lung lying in front of the pack heart's
    // anterior silhouette, in a band from `lung_retraction_right_mm` right of the midline to
    // `lung_retraction_left_mm` left of it, reads as soft tissue. No source for the notch's extent
    // was read; every value below is designed. Setting both widths to 0 restores the baked lungs.
    /// Width of the retracted band to the patient's left of the thorax midline, mm. **Designed**.
    pub lung_retraction_left_mm: f32,
    /// Width of the band to the patient's right of the midline, mm. **Designed**.
    pub lung_retraction_right_mm: f32,
    /// The heart's anterior silhouette is widened by this much (in x and y) before use, mm.
    /// **Designed**.
    pub lung_retraction_margin_mm: f32,
    /// Lung down to this far behind the heart's front surface is retracted too, mm. **Designed**.
    pub lung_retraction_behind_mm: f32,
    /// Fraction of the band's left width lost at full inspiration (the lung's anterior border
    /// comes back over the heart): left width × (1 − this × inspiration depth); the right edge,
    /// behind the sternum, stays. **Designed** (until B32 step 0: 0.5 on both edges, which hid
    /// the whole parasternal image at full inspiration; 0.15 on the left edge leaves the window
    /// worse in inspiration but open).
    pub lung_retraction_inspiration_loss: f32,
}

impl Default for DesignedParams {
    fn default() -> Self {
        Self {
            mitral_anterior_length_mm: 25.1,
            mitral_posterior_length_mm: 14.4,
            av_coaptation_mm: 4.0,
            tricuspid_height_mm: 18.9,
            aortic_effective_height_mm: 8.6,
            pulmonary_effective_height_mm: 8.6,
            av_leaflet_thickness_mm: 1.6,
            cusp_thickness_mm: 1.0,
            open_angle_from_axis_deg: 10.0,
            semilunar_opening_ms: 27.5,
            semilunar_closing_ms: 43.8,
            semilunar_slow_closure: 0.125,
            av_opening_ms: 40.0,
            av_closing_ms: 30.0,
            av_diastasis_opening: 0.5,
            av_mid_closure_ms: 80.0,
            papillary_tip_to_annulus_mm: [20.7, 22.75],
            papillary_base_diameter_mm: [7.98, 8.44],
            papillary_tip_fraction: 0.6,
            papillary_embed_mm: 2.0,
            moderator_thickness_mm: 4.5,
            moderator_septal_fraction: 0.55,
            moderator_max_apical_tilt: 0.5,
            pericardium_mm: 1.5,
            contact_limit_deg: 60.0,
            skin_gap_mm: 25.0,
            thorax_respiratory_fraction: 0.5,
            skin_normal_radius_mm: 12.0,
            lung_retraction_left_mm: 55.0,
            lung_retraction_right_mm: 15.0,
            lung_retraction_margin_mm: 10.0,
            lung_retraction_behind_mm: 5.0,
            lung_retraction_inspiration_loss: 0.15,
        }
    }
}

/// How far each valve is open at `t_ms` (`0` closed … `1` fully open), indexed by
/// [`Valve::index`]. A pure function of the motion record and the time, from the same valve
/// schedule as the wall motion ([`volume_at`]):
///
/// * **semilunar** (aortic, pulmonic): open only during ejection — a rapid opening over
///   [`DesignedParams::semilunar_opening_ms`], the slow systolic closure, and a rapid closure
///   that ends at A2/P2; a beat that does not eject never opens them;
/// * **AV** (mitral, tricuspid): open from the end of isovolumic relaxation, fully through the E
///   wave, drifting to [`DesignedParams::av_diastasis_opening`] in diastasis, reopening with each
///   organised atrial contraction that falls in the filling (none in fibrillation), and closing
///   over [`DesignedParams::av_closing_ms`] from M1/T1; closed through isovolumic contraction,
///   ejection and relaxation.
///
/// Before the first beat the AV valves stand at their diastasis opening and the semilunar valves
/// are shut. Valve lesions do not change this in this phase.
#[must_use]
pub fn valve_openings(
    history: &MotionHistory,
    motion: &MotionParams,
    ef_scale: [f64; 2],
    t_ms: f64,
    d: &DesignedParams,
) -> [f32; 4] {
    let inp = VolumeInputs {
        beats: history.beats(),
        atrial_onsets_ms: history.atrial_onsets_ms(),
        params: motion,
        pcg: history.pcg_params(),
        ef_scale,
    };
    let mut out = [0.0f32; 4];
    for (side, inflow, outflow) in [
        (Side::Left, Valve::Mitral, Valve::Aortic),
        (Side::Right, Valve::Tricuspid, Valve::Pulmonic),
    ] {
        let (i, o) = side_openings(t_ms, side, &inp, d);
        out[inflow.index()] = i as f32;
        out[outflow.index()] = o as f32;
    }
    out
}

fn side_openings(t: f64, side: Side, inp: &VolumeInputs<'_>, d: &DesignedParams) -> (f64, f64) {
    let st = volume_at(t, side, inp);
    let Some(c) = st.cycle else {
        return (d.av_diastasis_opening, 0.0);
    };
    let outflow = match c.ejection_ms {
        Some((open, close)) if t >= open && t < close => {
            let up = smoothstep((t - open) / d.semilunar_opening_ms.max(1.0));
            let down = smoothstep((close - t) / d.semilunar_closing_ms.max(1.0));
            let tau = ((t - open) / (close - open).max(1e-9)).clamp(0.0, 1.0);
            up.min(down) * (1.0 - d.semilunar_slow_closure * tau)
        }
        _ => 0.0,
    };
    let inflow = if st.phase == CardiacPhase::Filling {
        filling_opening(t, c.inflow_open_ms, inp, d)
    } else if t < c.closure_ms + d.av_closing_ms {
        // closing from where the previous filling had got to at M1/T1
        let before = volume_at(c.closure_ms - 1e-3, side, inp);
        let at_closure = match before.cycle {
            Some(p) if before.phase == CardiacPhase::Filling => {
                filling_opening(c.closure_ms, p.inflow_open_ms, inp, d)
            }
            None => d.av_diastasis_opening,
            _ => 0.0,
        };
        at_closure * (1.0 - smoothstep((t - c.closure_ms) / d.av_closing_ms.max(1.0)))
    } else {
        0.0
    };
    (inflow, outflow)
}

/// AV-valve opening during a filling that began at `open_ms`.
fn filling_opening(t: f64, open_ms: f64, inp: &VolumeInputs<'_>, d: &DesignedParams) -> f64 {
    let p = inp.params;
    let tau = t - open_ms;
    if tau < 0.0 {
        return 0.0;
    }
    let up = smoothstep(tau / d.av_opening_ms.max(1.0));
    let e_end = p.e_acceleration_ms + p.e_deceleration_ms;
    let passive = if tau <= e_end {
        1.0
    } else {
        let k = smoothstep((tau - e_end) / d.av_mid_closure_ms.max(1.0));
        1.0 - (1.0 - d.av_diastasis_opening) * k
    };
    // the atrial reopening: each organised atrial contraction that starts with the valve open
    let dur = p.atrial_contraction_ms.max(2.0);
    let mut atrial = 0.0f64;
    let first = inp
        .atrial_onsets_ms
        .partition_point(|&a| a + p.emd_atrium_ms < open_ms);
    for &a in &inp.atrial_onsets_ms[first..] {
        let start = a + p.emd_atrium_ms;
        if start > t {
            break;
        }
        let x = t - start;
        let rise = smoothstep(x / d.av_opening_ms.max(1.0));
        let fall = 1.0 - smoothstep((x - 0.5 * dur) / (0.5 * dur));
        atrial = atrial.max(rise.min(fall));
    }
    up.min(passive.max(atrial))
}
