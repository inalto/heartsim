//! Tier-1-specific settings that are not per-region tissue parameters (those live in
//! `heartsim_core::Parameters`).

use serde::{Deserialize, Serialize};

/// Settings of the eikonal engine. Defaults are calibrated on the default anatomy pack so that a
/// sinus beat gives P ≈ 105 ms and QRS ≈ 80 ms (Durrer et al. 1970 ranges, see
/// `docs/research/electrophysiology-and-ecg-models.md` §1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EikonalParams {
    /// Graph neighbourhood radius as a multiple of the median cell spacing. 2.3 gives the
    /// 56-neighbourhood on a cubic lattice (metrication error of a few per cent).
    pub neighbourhood_factor: f32,
    /// Upper bound of edges per cell (nearest kept).
    pub max_edges: usize,
    /// Fast endocardial ("Purkinje") layer: ventricular cells with transmural depth ≤ this
    /// value (0 = endocardium) conduct at `purkinje_layer_cv_m_s` for fronts delivered by their
    /// own bundle branch. Stand-in for an explicit Purkinje tree (the fast-endocardium
    /// recipe of Gillette et al. 2021 and Cardone-Noott et al. 2016). 0 disables the layer.
    pub purkinje_layer_depth: f32,
    /// Isotropic CV inside the fast endocardial layer, m/s. Literature 1–3 m/s.
    pub purkinje_layer_cv_m_s: f64,
    /// Apex–base coordinate (0 apex … 1 base) above which the endocardium has no Purkinje
    /// coverage: the base and the RV outflow tract activate last (Durrer 1970).
    pub purkinje_layer_base_limit: f32,
    /// Multiplier on atrial CVs (all atrial regions). The idealised pack's atria are spheres
    /// whose SA-node → lateral-LA path is ≈ 1.7× longer than in a real heart, so literature CVs
    /// (0.8 m/s working myocardium, 1.5 m/s Bachmann's bundle) give a 170 ms P wave there; this
    /// calibrates the default pack to 80–120 ms. Use 1.0 for anatomical meshes.
    pub atrial_cv_scale: f64,
    /// Apicobasal APD gradient, ms: APD(base) − APD(apex). Shorter apical APD is one of the
    /// gradients that makes the T wave concordant (Keller 2012; Okada 2011; research §3.5).
    pub apd_apicobasal_ms: f64,
    /// APD offset of the RV free wall relative to the LV, ms (RV APD slightly shorter).
    pub apd_rv_offset_ms: f64,
    /// Lower bound of the CV restitution factor (very premature fronts still crawl, they do not
    /// stop), dimensionless.
    pub min_cv_factor: f64,
    /// Lower bound of APD after restitution, as a fraction of the cell's base APD.
    pub min_apd_fraction: f64,
    /// A mesh probe (AV-node input, accessory-pathway insertion) re-arms this long after it
    /// last fired, ms.
    pub probe_rearm_ms: f64,
    /// Distance from the AV-node cells within which atrial cells count as the nodal input, mm.
    pub av_input_radius_mm: f32,
    /// Join the coronary-sinus sleeve to the nearest right-atrial cell at the ostium, if one lies
    /// within this distance, mm (0 = off). The sleeve is continuous with the atrial muscle at the
    /// ostium (Chauvin et al. 2000), which is what makes the CS activate proximal to distal in
    /// sinus rhythm; the idealised pack leaves its sinus ostium ≈ 11 mm from the right atrium, so
    /// without this link the sleeve fills from the left atrium at its middle. The EP lab turns
    /// it on; off by default so that the rhythm library's ECGs do not depend on it.
    #[serde(default)]
    pub cs_os_link_mm: f32,
}

impl Default for EikonalParams {
    fn default() -> Self {
        Self {
            neighbourhood_factor: 2.3,
            max_edges: 64,
            purkinje_layer_depth: 0.3,
            purkinje_layer_cv_m_s: 1.4,
            purkinje_layer_base_limit: 0.8,
            atrial_cv_scale: 1.7,
            apd_apicobasal_ms: 20.0,
            apd_rv_offset_ms: -10.0,
            min_cv_factor: 0.1,
            min_apd_fraction: 0.3,
            probe_rearm_ms: 200.0,
            av_input_radius_mm: 6.0,
            cs_os_link_mm: 0.0,
        }
    }
}
