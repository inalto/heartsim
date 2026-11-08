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
    /// The same threshold for the right-ventricular free wall. Its wall is 3–4 mm thick, about
    /// 1.5 cells of the default pack's 2.3 mm lattice, so the innermost cell of a column sits at
    /// depth ≈ 0.5 wherever the wall is one cell thick; 0.5 makes that cell part of the layer and
    /// gives the RV the same ≈ 2 mm of fast endocardium the LV gets from 0.3 of a 10 mm wall.
    /// With 0.3 the RV layer had holes, the free wall activated from a few patches by myocardial
    /// conduction and finished 30 ms after the LV, which put a large rightward terminal vector
    /// into every QRS.
    pub purkinje_layer_depth_rv: f32,
    /// Isotropic CV inside the fast endocardial layer, m/s. Purkinje fibres conduct at 2–4 m/s
    /// (Durrer 1970); the layer stands in for a network that also has to branch and cross
    /// junctions, so 2.0 reproduces Durrer's timing: from 12 mm seed patches at the papillary
    /// muscles it activates the default pack's LV endocardium in ≈ 25 ms and reaches the far
    /// RV free wall, 50–60 mm from its one patch, in ≈ 28 ms, so both free walls depolarise
    /// together and the QRS lasts ≈ 85 ms.
    pub purkinje_layer_cv_m_s: f64,
    /// Apex–base coordinate (0 apex … 1 base) above which the left-ventricular endocardium
    /// (free wall and septum) has no Purkinje coverage: the base activates last (Durrer 1970).
    pub purkinje_layer_base_limit: f32,
    /// The right-ventricular free wall's Purkinje-free zone: cells within this distance of the
    /// `rvot` landmark (10 mm below the pulmonary valve in the default pack), i.e. the
    /// infundibulum, the smooth-walled outflow that activates last of all (Durrer 1970); 15 mm
    /// is the conus proper, ≈ 150 cells (25 mm took in the upper free wall and gave a terminal
    /// rightward vector that put an r' into aVR). The
    /// rest of the RV free wall, down to the tricuspid annulus, carries the network: the RV has
    /// two bases, and an apex–base limit measured towards either valve would cut the other's
    /// inflow or outflow wall out of the layer, leaving a third of the RV to slow myocardial
    /// conduction and a large rightward terminal QRS vector.
    pub purkinje_free_rvot_mm: f32,
    /// Multiplier on atrial CVs (all atrial regions). The default pack's atria are the Rodero
    /// 2021 average: large (RA to LA centroid 48 mm, appendages and vein sleeves included), one
    /// lattice cell thick and isotropic, so the sinus wave follows the wall with no transmural
    /// shortcut and the literature 0.8 m/s gives a P wave of ≈ 130 ms; 1.25 brings the atrial
    /// activation to ≈ 105 ms (normal P ≤ 120 ms). The procedural pack's spherical atria needed
    /// 1.7 (a path 1.7× longer than a real heart's, and anisotropic walls).
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
            purkinje_layer_depth_rv: 0.5,
            purkinje_layer_cv_m_s: 2.0,
            purkinje_layer_base_limit: 0.8,
            purkinje_free_rvot_mm: 15.0,
            atrial_cv_scale: 1.25,
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
