//! Tier-1 forward ECG model: per-cell transmembrane potential → electrode potentials → 12 leads.
//!
//! **Physics** (`docs/ARCHITECTURE.md` §4.4, research §3.1). In an unbounded homogeneous volume
//! conductor the extracellular potential at an electrode `r_e` is (Gima & Rudy 2002)
//!
//! ```text
//! φ_e = −K ∫_Ω ∇V_m(x) · ∇_x(1/|r_e − x|) dV = −K ∫_Ω ∇V_m · (r_e − x)/|r_e − x|³ dV     K = σ_i/(4πσ_b)
//! ```
//!
//! — every cell is a current dipole `−σ_i∇V_m·dV` (pointing the way the front moves), so a front
//! approaching the electrode gives a positive deflection. Two discretisations are provided and
//! chosen per cell ([`SourceModel`]):
//!
//! * **Volume** (default for volumetric clouds): `∇V_m` is the least-squares gradient over the
//!   cell's neighbours *in the same electrical compartment* (atria, AV junction, ventricles —
//!   the annulus fibrosus insulates them; scar is silent), and the kernel is
//!   `(r_e − x)/|r_e − x|^p` (`p = 3` is the exact dipole field; [`ForwardOptions::distance_power`]).
//!   Because the gradient is linear in V_m it folds into the weight matrix: uniform V_m (rest, or
//!   the ST plateau) gives exactly zero.
//! * **Equivalent double layer** (surface cells with a [`SurfacePatches`] area vector `A·n`):
//!   `w = −K (A n · (r_e − x))/|r_e − x|^p`, the solid-angle form that ECGSIM uses on closed
//!   surface meshes (van Oosterom 2004). Integration by parts makes the two equal for uniform σ_i,
//!   which [`SurfacePatches::boundary_of`] lets you check on a volumetric cloud.
//!
//! Either way the model is a precomputed matrix `W [9 signal electrodes × n cells]` (row-major
//! f32) and a sample is one matrix–vector product. Limb electrodes default to Einthoven's
//! idealised distal-limb positions ([`LimbPlacement`]); RL is the reference and carries no signal;
//! the Wilson terminal is subtracted by [`ElectrodePotentials::to_leads`]. Extra unipolar chest
//! electrodes (V3R, V4R, V7–V9; brief B10) add rows after the nine signal rows
//! ([`ForwardModel::set_extra_electrodes`], [`ForwardModel::sample_extras`]).
//!
//! **Determinism.** Column chunks are reduced in a fixed order, so results do not depend on the
//! number of threads; there is no I/O and no clock.

use glam::{DMat3, DVec3, Vec3};
use heartsim_core::{ELECTRODE_COUNT, Electrode, ElectrodePotentials, Region};
use heartsim_geometry::{CellCloud, ElectrodeSet, HashGrid};
use heartsim_sim::{CellState, Chamber};
use serde::{Deserialize, Serialize};

/// Electrodes that carry a signal, in row order of `W` (RL is the reference).
pub const SIGNAL_ELECTRODES: [Electrode; 9] = [
    Electrode::RA,
    Electrode::LA,
    Electrode::LL,
    Electrode::V1,
    Electrode::V2,
    Electrode::V3,
    Electrode::V4,
    Electrode::V5,
    Electrode::V6,
];
const ROWS: usize = SIGNAL_ELECTRODES.len();
/// Cells per reduction chunk of [`ForwardModel::sample`] (fixed: it defines the summation order).
const SAMPLE_CHUNK: usize = 4096;
/// Electrodes closer than this fraction of the cell spacing to a cell see it as if at that
/// distance (avoids the 1/r^p singularity for an electrode placed on the heart).
const MIN_DISTANCE_FRACTION: f32 = 0.5;

/// Row of `W` for a signal electrode (`None` for RL).
#[must_use]
pub fn signal_row(e: Electrode) -> Option<usize> {
    SIGNAL_ELECTRODES.iter().position(|s| *s == e)
}

/// Which discretisation each cell uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SourceModel {
    /// Per cell: double layer where a surface patch is given, volume gradient otherwise. Patches
    /// are meant for cells that *are* surface samples (surface-only meshes, shell structures); do
    /// not combine patches on the boundary of a volumetric cloud with its interior, which would
    /// count the same source twice — use [`SourceModel::DoubleLayer`] for that comparison.
    #[default]
    Auto,
    /// Volume gradient for every cell; patches are ignored.
    Volume,
    /// Double layer on patched cells only; the other cells are silent.
    DoubleLayer,
}

/// Where the limb electrodes RA, LA, LL (and RL) sit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LimbPlacement {
    /// Standard distal-limb electrodes. A limb is a linear conductor that carries the potential
    /// of its junction with the trunk, far from the heart, and Einthoven's equilateral triangle
    /// idealises those junctions as equidistant at hexaxial −150° (RA), −30° (LA) and +90° (LL)
    /// in the frontal plane. The limb electrodes therefore see the far field of the heart vector,
    /// `φ = p·d̂ / R²` — the limb leads are exact hexaxial projections of `p`.
    Einthoven {
        /// Effective distance `R`, mm: sets the limb leads' sensitivity relative to the chest
        /// electrodes (point electrodes in the infinite medium). It stands in for the torso
        /// boundary, which makes limb lead vectors as large as precordial ones (Dower 1980);
        /// calibrated on the default pack ([`ForwardOptions`]).
        lead_distance_mm: f32,
    },
    /// Use the electrode set's own limb sites (e.g. Mason–Likar torso positions of the anatomy
    /// pack); shifts the axis rightwards, a classic teaching point.
    Sites,
}

impl Default for LimbPlacement {
    fn default() -> Self {
        Self::Einthoven {
            lead_distance_mm: 125.0,
        }
    }
}

/// Pedagogical per-wave amplitude knobs. They scale the V_m field of the cells that make each
/// wave (atria → P; ventricles during depolarisation → QRS, otherwise → ST–T), so 1.0 is the
/// physical model; anything else is a teaching exaggeration, not physiology.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaveGains {
    pub p: f32,
    pub qrs: f32,
    pub t: f32,
}

impl WaveGains {
    /// Whether the ventricular gain depends on the phase (QRS ≠ T).
    #[must_use]
    pub fn phased(&self) -> bool {
        (self.qrs - self.t).abs() > f32::EPSILON
    }
}

impl Default for WaveGains {
    fn default() -> Self {
        Self {
            p: 1.0,
            qrs: 1.0,
            t: 1.0,
        }
    }
}

/// Whether the ventricles are being activated (QRS gain) or not (ST–T gain).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VentricularPhase {
    Depolarisation,
    #[default]
    Repolarisation,
}

/// Build settings.
#[derive(Debug, Clone, PartialEq)]
pub struct ForwardOptions {
    /// Overall scale `k = σ_i/(4πσ_b) · ΔV_m`, mV per unit of normalised V_m. With σ_i ≈ σ_b and
    /// a 100 mV action potential the physical value is ≈ 8 mV; the default is calibrated so that
    /// the default pack's sinus beat has R(II) ≈ 1.4 mV, S(V1) ≈ 0.7 mV and R(V5) ≈ 2.4 mV (it
    /// absorbs the missing torso boundary).
    pub gain_mv: f32,
    /// Exponent `p` of the distance kernel `(r_e − x)/|r_e − x|^p`; 3 is the exact dipole field.
    pub distance_power: f32,
    /// Per-cell source discretisation.
    pub source: SourceModel,
    /// Limb-electrode positions.
    pub limbs: LimbPlacement,
    /// Per-wave gains (pedagogical; 1.0 = physical).
    pub gains: WaveGains,
    /// Surface patches for double-layer cells (`None` = volumetric cloud).
    pub patches: Option<SurfacePatches>,
    /// The ventricles count as depolarising while some ventricular cell activated less than this
    /// long ago, ms ([`ForwardModel::ventricular_phase_of`]).
    pub depolarisation_window_ms: f32,
}

impl Default for ForwardOptions {
    fn default() -> Self {
        Self {
            gain_mv: 4.5,
            distance_power: 3.0,
            source: SourceModel::Auto,
            limbs: LimbPlacement::default(),
            gains: WaveGains::default(),
            patches: None,
            depolarisation_window_ms: 10.0,
        }
    }
}

/// Area vectors `A·n` (mm², outward normal) of the cells that sample a closed surface; zero for
/// cells that do not.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SurfacePatches {
    pub area_vectors_mm2: Vec<Vec3>,
}

impl SurfacePatches {
    /// Boundary patches of a volumetric cloud, one closed surface per electrical compartment.
    ///
    /// A cell's area vector is `−h³ · M⁻¹ Σ_k d_k` over its 26-neighbourhood (`d_k` = offsets to
    /// neighbours of the same compartment within 1.8 spacings, `M = Σ d dᵀ`): zero for a full
    /// stencil, and exactly `h²·n` for a cell on a flat lattice face. Cells below 0.3 h² are
    /// interior.
    ///
    /// The layer carries the V_m of the boundary *cell centres*, which lie half a spacing inside
    /// the voxel surface the volume source integrates over, so on coarse lattices its amplitude
    /// is lower by roughly `h·S / (2·V)` (≈ 25 % for a 6-spacing ball; S, V = surface, volume).
    #[must_use]
    pub fn boundary_of(cloud: &CellCloud) -> Self {
        let h = spacing_mm(cloud);
        let comp: Vec<Compartment> = cloud.regions.iter().map(|r| Compartment::of(*r)).collect();
        let grid = HashGrid::new(&cloud.positions, h * 2.0);
        let mut found = Vec::with_capacity(32);
        let r2 = (1.8 * h) * (1.8 * h);
        let vol = f64::from(h).powi(3);
        let area_vectors_mm2: Vec<Vec3> = (0..cloud.len())
            .map(|i| {
                if comp[i] == Compartment::Silent {
                    return Vec3::ZERO;
                }
                let p = cloud.positions[i];
                grid.knn(p, 26, Some(i as u32), &mut found);
                let mut m = DMat3::ZERO;
                let mut sum = DVec3::ZERO;
                for &(_, k) in &found {
                    let d = cloud.positions[k as usize] - p;
                    if comp[k as usize] != comp[i] || d.length_squared() > r2 {
                        continue;
                    }
                    let d = d.as_dvec3();
                    m += outer(d, d);
                    sum += d;
                }
                let a = -vol * regularised_inverse(m) * sum;
                if a.length() < 0.3 * f64::from(h * h) {
                    Vec3::ZERO
                } else {
                    a.as_vec3()
                }
            })
            .collect();
        Self { area_vectors_mm2 }
    }

    /// Number of cells that carry a patch.
    #[must_use]
    pub fn count(&self) -> usize {
        self.area_vectors_mm2.iter().filter(|a| **a != Vec3::ZERO).count()
    }
}

/// Weight-matrix statistics of one electrode row, for the validation bridge (brief B23).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeightStats {
    pub electrode: Electrode,
    pub position_mm: [f32; 3],
    /// Σ w (≈ 0: a uniform V_m produces no potential).
    pub sum: f64,
    /// Σ |w|, mV per unit V_m.
    pub abs_sum: f64,
    /// √Σ w².
    pub l2: f64,
    pub min: f32,
    pub max: f32,
    /// Cells with a non-zero weight.
    pub nonzero: usize,
    /// |w|-weighted centroid of the cells the electrode "sees", mm.
    pub centroid_mm: [f32; 3],
    /// Participation ratio (Σ|w|)² / Σw²: how many cells effectively contribute.
    pub effective_cells: f64,
}

/// Construction errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardError {
    /// The electrode set lacks a signal electrode needed with the chosen placement.
    MissingElectrode(Electrode),
    /// `patches` does not have one entry per cell.
    PatchCount { expected: usize, found: usize },
}

impl std::fmt::Display for ForwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingElectrode(e) => write!(f, "electrode set has no {} site", e.label()),
            Self::PatchCount { expected, found } => {
                write!(f, "surface patches: expected {expected} entries, found {found}")
            }
        }
    }
}

impl std::error::Error for ForwardError {}

/// Electrically separate tissue: V_m gradients are never taken across compartments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compartment {
    /// Scar: no intracellular continuity, no source.
    Silent,
    Atrial,
    Junction,
    Ventricular,
}

impl Compartment {
    fn of(region: Region) -> Self {
        if region == Region::Scar {
            return Self::Silent;
        }
        match Chamber::of(region) {
            Chamber::Atrium => Self::Atrial,
            Chamber::AvJunction => Self::Junction,
            Chamber::Ventricle => Self::Ventricular,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    Silent,
    Volume,
    Patch,
}

/// The precomputed forward model. See the module documentation.
#[derive(Debug, Clone)]
pub struct ForwardModel {
    opts: ForwardOptions,
    n: usize,
    positions: Vec<Vec3>,
    kind: Vec<SourceKind>,
    compartment: Vec<Compartment>,
    /// Volume of one cell, mm³.
    cell_volume_mm3: f32,
    min_distance_mm: f32,
    /// Gradient operator transposed (CSR): for cell `j`, the `(i, c_ij)` with `j ∈ N(i)` and
    /// `∇V_i = Σ_j c_ij (V_j − V_i)`.
    in_offsets: Vec<u32>,
    in_source: Vec<u32>,
    in_coef: Vec<Vec3>,
    /// `Σ_j c_ij` per cell.
    coef_sum: Vec<Vec3>,
    /// Area vector of patch cells.
    patch: Vec<Vec3>,
    ventricular_cells: Vec<u32>,
    /// Current site of each of the ten electrodes, mm (nominal for far-field electrodes).
    sites: [Vec3; ELECTRODE_COUNT],
    /// Far-field lead vector `d̂/R²` of Einthoven limb electrodes (`None` = point electrode).
    far_field: [Option<Vec3>; ELECTRODE_COUNT],
    /// Heart-vector weights: `p = Σ_j dipole_w[j]·V_j`, mV·mm².
    dipole_w: Vec<Vec3>,
    /// Extra point electrodes (name, site mm): rows `ROWS..` of `w`.
    extras: Vec<(String, Vec3)>,
    /// Physical weights, `(ROWS + extras) × n`.
    w: Vec<f32>,
    /// `w` with per-wave gains and rest offsets folded in.
    w_eff: Vec<f32>,
    /// Constant potential from the rest offsets per row, mV.
    offset_mv: Vec<f64>,
    rest_offset: Vec<f32>,
    phase: VentricularPhase,
}

impl ForwardModel {
    /// Precompute `W` for `cloud` and the signal sites of `electrodes`.
    pub fn build(
        cloud: &CellCloud,
        electrodes: &ElectrodeSet,
        opts: ForwardOptions,
    ) -> Result<Self, ForwardError> {
        let n = cloud.len();
        if let Some(p) = &opts.patches
            && p.area_vectors_mm2.len() != n
        {
            return Err(ForwardError::PatchCount {
                expected: n,
                found: p.area_vectors_mm2.len(),
            });
        }
        let compartment: Vec<Compartment> = cloud.regions.iter().map(|r| Compartment::of(*r)).collect();
        let patch = opts
            .patches
            .as_ref()
            .map_or_else(|| vec![Vec3::ZERO; n], |p| p.area_vectors_mm2.clone());
        let kind: Vec<SourceKind> = (0..n)
            .map(|i| {
                let has_patch = patch[i] != Vec3::ZERO;
                match (compartment[i], opts.source, has_patch) {
                    (Compartment::Silent, _, _) | (_, SourceModel::DoubleLayer, false) => SourceKind::Silent,
                    (_, SourceModel::Auto | SourceModel::DoubleLayer, true) => SourceKind::Patch,
                    (_, SourceModel::Volume | SourceModel::Auto, _) => SourceKind::Volume,
                }
            })
            .collect();
        let h = spacing_mm(cloud);
        let ventricular_cells: Vec<u32> = (0..n)
            .filter(|&i| compartment[i] == Compartment::Ventricular)
            .map(|i| i as u32)
            .collect();
        let (sites, far_field) = electrode_sites(cloud, electrodes, &ventricular_cells, opts.limbs)?;
        let mut model = Self {
            n,
            positions: cloud.positions.clone(),
            cell_volume_mm3: h * h * h,
            min_distance_mm: h * MIN_DISTANCE_FRACTION,
            in_offsets: Vec::new(),
            in_source: Vec::new(),
            in_coef: Vec::new(),
            coef_sum: vec![Vec3::ZERO; n],
            patch,
            ventricular_cells,
            sites,
            far_field,
            dipole_w: Vec::new(),
            extras: Vec::new(),
            w: vec![0.0; ROWS * n],
            w_eff: vec![0.0; ROWS * n],
            offset_mv: vec![0.0; ROWS],
            rest_offset: vec![0.0; n],
            phase: VentricularPhase::default(),
            kind,
            compartment,
            opts,
        };
        model.build_gradient_operator(cloud);
        model.build_dipole_weights();
        for row in 0..ROWS {
            model.compute_row(row);
        }
        Ok(model)
    }

    /// Least-squares gradient over each volume cell's k-NN neighbours of the same compartment,
    /// stored transposed so a row of `W` is a gather (parallel, deterministic).
    fn build_gradient_operator(&mut self, cloud: &CellCloud) {
        let n = self.n;
        let mut out: Vec<Vec<(u32, Vec3)>> = vec![Vec::new(); n];
        let mut counts = vec![0u32; n];
        for (i, own) in out.iter_mut().enumerate() {
            if self.kind[i] != SourceKind::Volume {
                continue;
            }
            let p = cloud.positions[i];
            let nbrs: Vec<(u32, DVec3)> = cloud.neighbours.get(i).map_or_else(Vec::new, |nb| {
                nb.iter()
                    .filter(|k| {
                        let k = k.index as usize;
                        k < n
                            && self.kind[k] == SourceKind::Volume
                            && self.compartment[k] == self.compartment[i]
                    })
                    .map(|k| (k.index, (cloud.positions[k.index as usize] - p).as_dvec3()))
                    .collect()
            });
            let m = nbrs.iter().fold(DMat3::ZERO, |m, (_, d)| m + outer(*d, *d));
            let inv = regularised_inverse(m);
            let mut sum = Vec3::ZERO;
            for (k, d) in nbrs {
                let c = (inv * d).as_vec3();
                sum += c;
                own.push((k, c));
                counts[k as usize] += 1;
            }
            self.coef_sum[i] = sum;
        }
        let mut offsets = Vec::with_capacity(n + 1);
        offsets.push(0u32);
        for c in &counts {
            offsets.push(offsets.last().copied().unwrap_or(0) + c);
        }
        let total = offsets[n] as usize;
        let mut source = vec![0u32; total];
        let mut coef = vec![Vec3::ZERO; total];
        let mut fill: Vec<u32> = offsets[..n].to_vec();
        for (i, list) in out.iter().enumerate() {
            for &(k, c) in list {
                let slot = fill[k as usize] as usize;
                source[slot] = i as u32;
                coef[slot] = c;
                fill[k as usize] += 1;
            }
        }
        self.in_offsets = offsets;
        self.in_source = source;
        self.in_coef = coef;
    }

    /// `p = −k Σ_i vol·∇V_i` (plus `−k Σ A n V` over patches) folded into per-cell vectors.
    fn build_dipole_weights(&mut self) {
        let k = self.opts.gain_mv;
        let vol = self.cell_volume_mm3;
        let weight = |j: usize| -> Vec3 {
            match self.kind[j] {
                SourceKind::Silent => Vec3::ZERO,
                SourceKind::Patch => -k * self.patch[j],
                SourceKind::Volume => {
                    let range = self.in_offsets[j] as usize..self.in_offsets[j + 1] as usize;
                    let gathered: Vec3 = self.in_coef[range].iter().copied().sum();
                    -k * vol * (gathered - self.coef_sum[j])
                }
            }
        };
        self.dipole_w = par_map(self.n, weight);
    }

    /// `|r|^−p` with the singularity clamped.
    fn kernel_scale(&self, r: Vec3) -> f32 {
        let r2 = r
            .length_squared()
            .max(self.min_distance_mm * self.min_distance_mm);
        let p = self.opts.distance_power;
        if (p - 3.0).abs() < f32::EPSILON {
            1.0 / (r2 * r2.sqrt())
        } else {
            r2.powf(-0.5 * p)
        }
    }

    /// Site and far-field lead vector of a row: a signal electrode, or an extra point electrode.
    fn row_site(&self, row: usize) -> (Vec3, Option<Vec3>) {
        match SIGNAL_ELECTRODES.get(row) {
            Some(&e) => (self.sites[e as usize], self.far_field[e as usize]),
            None => (self.extras[row - ROWS].1, None),
        }
    }

    fn row_count(&self) -> usize {
        ROWS + self.extras.len()
    }

    /// Recompute the physical weights of one row from its electrode's current site, and bake it
    /// (as `rebake_row` would) in the same pass.
    fn compute_row(&mut self, row: usize) {
        let (site, far_field) = self.row_site(row);
        let n = self.n;
        let mut w = std::mem::take(&mut self.w);
        let mut eff = std::mem::take(&mut self.w_eff);
        let rows = (&mut w[row * n..(row + 1) * n], &mut eff[row * n..(row + 1) * n]);
        let gain = |j: usize| self.column_gain(j);
        let constant = if let Some(lead) = far_field {
            fill_row(rows, &self.rest_offset, gain, |j| self.dipole_w[j].dot(lead))
        } else {
            let k = self.opts.gain_mv;
            let vol = self.cell_volume_mm3;
            // u_i = vol · (r_e − x_i)/|r_e − x_i|^p: the dipole kernel at each volume cell.
            let kernel = |i: usize| {
                if self.kind[i] == SourceKind::Volume {
                    let r = site - self.positions[i];
                    r * (vol * self.kernel_scale(r))
                } else {
                    Vec3::ZERO
                }
            };
            let u: Vec<Vec3> = par_map(n, kernel);
            // φ_e = −k Σ_i ∇V_i·u_i = −k Σ_j V_j (Σ_{i: j∈N(i)} c_ij·u_i − (Σ_k c_jk)·u_j).
            let weight = |j: usize| -> f32 {
                match self.kind[j] {
                    SourceKind::Silent => 0.0,
                    SourceKind::Patch => {
                        let r = site - self.positions[j];
                        -k * self.patch[j].dot(r) * self.kernel_scale(r)
                    }
                    SourceKind::Volume => {
                        let range = self.in_offsets[j] as usize..self.in_offsets[j + 1] as usize;
                        let gathered: f32 = self.in_source[range.clone()]
                            .iter()
                            .zip(&self.in_coef[range])
                            .map(|(&i, c)| c.dot(u[i as usize]))
                            .sum();
                        -k * (gathered - self.coef_sum[j].dot(u[j]))
                    }
                }
            };
            fill_row(rows, &self.rest_offset, gain, weight)
        };
        self.w = w;
        self.w_eff = eff;
        self.offset_mv[row] = constant;
    }

    /// Per-cell multiplier from the per-wave gains in the current phase.
    fn column_gain(&self, j: usize) -> f32 {
        let g = self.opts.gains;
        match self.compartment[j] {
            Compartment::Atrial => g.p,
            Compartment::Silent => 0.0,
            Compartment::Junction | Compartment::Ventricular => match self.phase {
                VentricularPhase::Depolarisation => g.qrs,
                VentricularPhase::Repolarisation => g.t,
            },
        }
    }

    /// Fold gains and rest offsets into one row: `w_eff = w·g·(1 − o)`, constant `Σ w·g·o`.
    fn rebake_row(&mut self, row: usize) {
        let n = self.n;
        let mut eff = std::mem::take(&mut self.w_eff);
        let gained = |j: usize| self.w[row * n + j] * self.column_gain(j);
        par_fill(&mut eff[row * n..(row + 1) * n], |j| {
            gained(j) * (1.0 - self.rest_offset[j])
        });
        // fixed-order sum of fixed chunks: deterministic for any thread count
        let partial = |c: usize| -> f64 {
            (c * SAMPLE_CHUNK..((c + 1) * SAMPLE_CHUNK).min(n))
                .map(|j| f64::from(gained(j) * self.rest_offset[j]))
                .sum()
        };
        let constant = par_map(n.div_ceil(SAMPLE_CHUNK), partial).iter().sum();
        self.w_eff = eff;
        self.offset_mv[row] = constant;
    }

    fn rebake_all(&mut self) {
        self.w_eff.resize(self.row_count() * self.n, 0.0);
        for row in 0..self.row_count() {
            self.rebake_row(row);
        }
    }

    // --- sampling -----------------------------------------------------------------------------

    /// Electrode potentials (mV) for the normalised V_m field `vm` (0 rest … 1 peak, one value per
    /// cell). RL is 0. Cost: one `9 × n` matrix–vector product.
    #[must_use]
    pub fn sample(&self, vm: &[f32]) -> ElectrodePotentials {
        assert_eq!(vm.len(), self.n, "V_m has one value per cell");
        let n = self.n;
        let chunks = n.div_ceil(SAMPLE_CHUNK);
        let partial = |c: usize| -> [f32; ROWS] {
            let cols = c * SAMPLE_CHUNK..((c + 1) * SAMPLE_CHUNK).min(n);
            let v = &vm[cols.clone()];
            std::array::from_fn(|row| dot(&self.w_eff[row * n + cols.start..row * n + cols.end], v))
        };
        let partials = par_map(chunks, partial);
        let mut out = ElectrodePotentials::default();
        for (row, e) in SIGNAL_ELECTRODES.iter().enumerate() {
            let sum: f64 = partials.iter().map(|p| f64::from(p[row])).sum();
            out.set(*e, sum + self.offset_mv[row]);
        }
        out
    }

    /// The heart vector: total current dipole `p = −k Σ vol·∇V_m` (mV·mm², heart frame), the
    /// physical quantity behind the vectorcardiogram and the "true" electrical axis. Its
    /// infinite-medium potential at a distant point `r` is `p·r/|r|³`. Per-wave gains and rest
    /// offsets are not applied.
    #[must_use]
    pub fn heart_vector(&self, vm: &[f32]) -> Vec3 {
        assert_eq!(vm.len(), self.n, "V_m has one value per cell");
        let sum: DVec3 = self
            .dipole_w
            .iter()
            .zip(vm)
            .map(|(d, &v)| (*d * v).as_dvec3())
            .sum();
        sum.as_vec3()
    }

    /// Potentials (mV, before the Wilson terminal is subtracted) of the extra electrodes, in the
    /// order of [`ForwardModel::extra_electrodes`]. Cost: one `extras × n` product.
    #[must_use]
    pub fn sample_extras(&self, vm: &[f32]) -> Vec<f64> {
        assert_eq!(vm.len(), self.n, "V_m has one value per cell");
        let n = self.n;
        (ROWS..self.row_count())
            .map(|row| {
                // fixed chunk order, as in `sample`
                (0..n.div_ceil(SAMPLE_CHUNK))
                    .map(|c| {
                        let cols = c * SAMPLE_CHUNK..((c + 1) * SAMPLE_CHUNK).min(n);
                        f64::from(dot(
                            &self.w_eff[row * n + cols.start..row * n + cols.end],
                            &vm[cols],
                        ))
                    })
                    .sum::<f64>()
                    + self.offset_mv[row]
            })
            .collect()
    }

    /// [`ForwardModel::sample`] of an engine state, after switching the ventricular gain phase
    /// from it (only matters when `gains.qrs ≠ gains.t`).
    pub fn sample_state(&mut self, state: &CellState, now_ms: f64) -> ElectrodePotentials {
        if self.opts.gains.phased() {
            let phase = self.ventricular_phase_of(state, now_ms);
            self.set_ventricular_phase(phase);
        }
        self.sample(&state.vm)
    }

    /// Depolarisation while some ventricular cell activated within the last
    /// `depolarisation_window_ms`.
    #[must_use]
    pub fn ventricular_phase_of(&self, state: &CellState, now_ms: f64) -> VentricularPhase {
        let window = f64::from(self.opts.depolarisation_window_ms);
        let active = self.ventricular_cells.iter().any(|&j| {
            let elapsed = now_ms - f64::from(state.activation_ms[j as usize]);
            (0.0..window).contains(&elapsed)
        });
        if active {
            VentricularPhase::Depolarisation
        } else {
            VentricularPhase::Repolarisation
        }
    }

    // --- configuration ------------------------------------------------------------------------

    /// Move one electrode to `pos` and recompute its row of `W` (RL: position only). An
    /// Einthoven limb electrode becomes a point electrode at `pos` (e.g. a Mason–Likar site).
    pub fn move_electrode(&mut self, e: Electrode, pos: Vec3) {
        self.sites[e as usize] = pos;
        self.far_field[e as usize] = None;
        if let Some(row) = signal_row(e) {
            self.compute_row(row);
        }
    }

    /// Put a limb electrode back on its distal limb: a far-field electrode at hexaxial angle
    /// `hexaxial_deg` (−150° right arm, −30° left arm, +90° legs) and the effective distance of
    /// the Einthoven placement (its default when the model was built with torso limb sites).
    /// The legs join the trunk at the pelvis, so both leg electrodes see the same far field.
    pub fn place_on_limb(&mut self, e: Electrode, hexaxial_deg: f32) {
        let r = match self.opts.limbs {
            LimbPlacement::Einthoven { lead_distance_mm } => lead_distance_mm,
            LimbPlacement::Sites => match LimbPlacement::default() {
                LimbPlacement::Einthoven { lead_distance_mm } => lead_distance_mm,
                LimbPlacement::Sites => 125.0,
            },
        };
        let a = hexaxial_deg.to_radians();
        let dir = Vec3::new(a.cos(), -a.sin(), 0.0);
        let centre = centroid_of(&self.positions, &self.ventricular_cells);
        self.sites[e as usize] = centre + dir * r;
        self.far_field[e as usize] = Some(dir / (r * r));
        if let Some(row) = signal_row(e) {
            self.compute_row(row);
        }
    }

    /// Replace the extra point electrodes (name, site mm); each gets its own row of `W`.
    pub fn set_extra_electrodes(&mut self, extras: &[(String, Vec3)]) {
        if self.extras.as_slice() == extras {
            return;
        }
        self.extras = extras.to_vec();
        let rows = self.row_count();
        self.w.resize(rows * self.n, 0.0);
        self.w_eff.resize(rows * self.n, 0.0);
        self.offset_mv.resize(rows, 0.0);
        for row in ROWS..rows {
            self.compute_row(row);
        }
    }

    /// The extra electrodes, in row order.
    #[must_use]
    pub fn extra_electrodes(&self) -> &[(String, Vec3)] {
        &self.extras
    }

    /// Whether an electrode is a far-field (distal-limb) electrode.
    #[must_use]
    pub fn is_far_field(&self, e: Electrode) -> bool {
        self.far_field[e as usize].is_some()
    }

    /// Exchange two electrodes' sites (a cable swap): rows are swapped, nothing is recomputed.
    pub fn swap_electrodes(&mut self, a: Electrode, b: Electrode) {
        self.sites.swap(a as usize, b as usize);
        self.far_field.swap(a as usize, b as usize);
        let n = self.n;
        match (signal_row(a), signal_row(b)) {
            (Some(ra), Some(rb)) if ra != rb => {
                let (lo, hi) = (ra.min(rb), ra.max(rb));
                for buf in [&mut self.w, &mut self.w_eff] {
                    let (head, tail) = buf.split_at_mut(hi * n);
                    head[lo * n..(lo + 1) * n].swap_with_slice(&mut tail[..n]);
                }
                self.offset_mv.swap(lo, hi);
            }
            (Some(r), None) | (None, Some(r)) => {
                self.compute_row(r);
            }
            _ => {}
        }
    }

    /// Replace the per-wave gains.
    pub fn set_gains(&mut self, gains: WaveGains) {
        if gains != self.opts.gains {
            self.opts.gains = gains;
            self.rebake_all();
        }
    }

    /// Select which ventricular gain applies.
    pub fn set_ventricular_phase(&mut self, phase: VentricularPhase) {
        if phase != self.phase {
            self.phase = phase;
            if self.opts.gains.phased() {
                self.rebake_all();
            }
        }
    }

    /// Per-cell resting-potential offset (normalised V_m units: 0.2 ≈ +20 mV for a 100 mV AP),
    /// the source of the ischaemic injury current (brief B14). A cell's potential becomes
    /// `o + (1 − o)·V_m`: rest is raised to `o`, the plateau is unchanged, so the zone and its
    /// neighbours differ during diastole (TQ) but not during the plateau — after baseline
    /// correction, ST elevation over a transmural zone and depression opposite it.
    pub fn set_v_rest_offset(&mut self, offset: &[f32]) {
        assert_eq!(offset.len(), self.n, "one offset per cell");
        self.rest_offset.copy_from_slice(offset);
        self.rebake_all();
    }

    // --- read-out -----------------------------------------------------------------------------

    #[must_use]
    pub fn options(&self) -> &ForwardOptions {
        &self.opts
    }

    #[must_use]
    pub fn cell_count(&self) -> usize {
        self.n
    }

    /// Current site of an electrode, mm.
    #[must_use]
    pub fn electrode_position(&self, e: Electrode) -> Vec3 {
        self.sites[e as usize]
    }

    /// Physical weights of one electrode (mV per unit V_m per cell), before gains and offsets.
    #[must_use]
    pub fn weights(&self, e: Electrode) -> Option<&[f32]> {
        signal_row(e).map(|r| &self.w[r * self.n..(r + 1) * self.n])
    }

    /// Number of cells using each discretisation: `(volume, patch, silent)`.
    #[must_use]
    pub fn source_counts(&self) -> (usize, usize, usize) {
        self.kind.iter().fold((0, 0, 0), |(v, p, s), k| match k {
            SourceKind::Volume => (v + 1, p, s),
            SourceKind::Patch => (v, p + 1, s),
            SourceKind::Silent => (v, p, s + 1),
        })
    }

    /// Statistics of every signal row of `W` (validation bridge, B23).
    #[must_use]
    pub fn weight_stats(&self) -> Vec<WeightStats> {
        SIGNAL_ELECTRODES
            .iter()
            .enumerate()
            .map(|(row, &e)| {
                let w = &self.w[row * self.n..(row + 1) * self.n];
                let (mut sum, mut abs, mut sq) = (0.0f64, 0.0f64, 0.0f64);
                let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                let mut centroid = DVec3::ZERO;
                let mut nonzero = 0;
                for (x, &v) in self.positions.iter().zip(w) {
                    let v64 = f64::from(v);
                    sum += v64;
                    abs += v64.abs();
                    sq += v64 * v64;
                    lo = lo.min(v);
                    hi = hi.max(v);
                    centroid += x.as_dvec3() * v64.abs();
                    nonzero += usize::from(v != 0.0);
                }
                WeightStats {
                    electrode: e,
                    position_mm: self.sites[e as usize].to_array(),
                    sum,
                    abs_sum: abs,
                    l2: sq.sqrt(),
                    min: lo,
                    max: hi,
                    nonzero,
                    centroid_mm: if abs > 0.0 {
                        (centroid / abs).as_vec3().to_array()
                    } else {
                        [0.0; 3]
                    },
                    effective_cells: if sq > 0.0 { abs * abs / sq } else { 0.0 },
                }
            })
            .collect()
    }
}

// --- helpers ------------------------------------------------------------------------------------

/// Sites (and far-field lead vectors) of the ten electrodes for the chosen limb placement.
type Sites = ([Vec3; ELECTRODE_COUNT], [Option<Vec3>; ELECTRODE_COUNT]);

fn electrode_sites(
    cloud: &CellCloud,
    electrodes: &ElectrodeSet,
    ventricular: &[u32],
    limbs: LimbPlacement,
) -> Result<Sites, ForwardError> {
    let mut sites = [Vec3::ZERO; ELECTRODE_COUNT];
    let mut far = [None; ELECTRODE_COUNT];
    let centre = centroid(cloud, ventricular);
    for e in Electrode::ALL {
        let is_limb = matches!(e, Electrode::RA | Electrode::LA | Electrode::LL | Electrode::RL);
        sites[e as usize] = match (limbs, is_limb) {
            (LimbPlacement::Einthoven { lead_distance_mm }, true) => {
                // Hexaxial angle θ (clockwise from +x towards the feet) → frontal direction.
                let deg: f32 = match e {
                    Electrode::RA => -150.0,
                    Electrode::LA => -30.0,
                    Electrode::LL => 90.0,
                    _ => 110.0,
                };
                let r = deg.to_radians();
                let dir = Vec3::new(r.cos(), -r.sin(), 0.0);
                if e != Electrode::RL {
                    far[e as usize] = Some(dir / (lead_distance_mm * lead_distance_mm));
                }
                centre + dir * lead_distance_mm
            }
            _ => match electrodes.standard(e) {
                Some(p) => p,
                None if e == Electrode::RL => centre,
                None => return Err(ForwardError::MissingElectrode(e)),
            },
        };
    }
    Ok((sites, far))
}

/// Centroid of the `cells` among `positions` (all when empty).
fn centroid_of(positions: &[Vec3], cells: &[u32]) -> Vec3 {
    let sum: DVec3 = if cells.is_empty() {
        positions.iter().map(|p| p.as_dvec3()).sum()
    } else {
        cells.iter().map(|&i| positions[i as usize].as_dvec3()).sum()
    };
    let n = if cells.is_empty() {
        positions.len()
    } else {
        cells.len()
    };
    (sum / n.max(1) as f64).as_vec3()
}

/// Centroid of `cells` (all cells when empty).
fn centroid(cloud: &CellCloud, cells: &[u32]) -> Vec3 {
    centroid_of(&cloud.positions, cells)
}

/// Typical cell spacing, mm.
fn spacing_mm(cloud: &CellCloud) -> f32 {
    let h = cloud.mean_nearest_distance_mm();
    if h.is_finite() && h > 0.0 { h } else { 1.0 }
}

fn outer(a: DVec3, b: DVec3) -> DMat3 {
    DMat3::from_cols(a * b.x, a * b.y, a * b.z)
}

/// `(M + ε·tr(M)/3·I)⁻¹`: exact on well-spread stencils, finite on degenerate ones (a strand
/// gives the gradient along the strand only, because every offset lies in M's range).
fn regularised_inverse(m: DMat3) -> DMat3 {
    let tr = m.x_axis.x + m.y_axis.y + m.z_axis.z;
    if tr <= 0.0 {
        return DMat3::ZERO;
    }
    (m + DMat3::from_diagonal(DVec3::splat(1e-6 * tr / 3.0))).inverse()
}

/// Dot product with eight independent lanes (auto-vectorises; fixed order → deterministic).
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = [0.0f32; 8];
    let ((ca, ra), (cb, rb)) = (a.as_chunks::<8>(), b.as_chunks::<8>());
    for (x, y) in ca.iter().zip(cb) {
        for l in 0..8 {
            acc[l] += x[l] * y[l];
        }
    }
    let tail: f32 = ra.iter().zip(rb).map(|(x, y)| x * y).sum();
    acc.iter().sum::<f32>() + tail
}

/// `out[j] = f(j)`, in parallel chunks with the `parallel` feature.
fn par_fill(out: &mut [f32], f: impl Fn(usize) -> f32 + Sync + Send) {
    let fill = |(c, chunk): (usize, &mut [f32])| {
        for (k, v) in chunk.iter_mut().enumerate() {
            *v = f(c * SAMPLE_CHUNK + k);
        }
    };
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        out.par_chunks_mut(SAMPLE_CHUNK).enumerate().for_each(fill);
    }
    #[cfg(not(feature = "parallel"))]
    out.chunks_mut(SAMPLE_CHUNK).enumerate().for_each(fill);
}

/// Fill one row of `W` (`w[j] = weight(j)`) and of `w_eff` (`w·g·(1 − o)`) in one pass and return
/// its constant `Σ w·g·o`, summed per fixed chunk in chunk order (deterministic, and the same
/// expressions as `rebake_row`).
fn fill_row(
    (w, eff): (&mut [f32], &mut [f32]),
    rest_offset: &[f32],
    gain: impl Fn(usize) -> f32 + Sync + Send,
    weight: impl Fn(usize) -> f32 + Sync + Send,
) -> f64 {
    let fill = |(c, (w, eff)): (usize, (&mut [f32], &mut [f32]))| -> f64 {
        let mut partial = 0.0;
        for (k, (w, eff)) in w.iter_mut().zip(eff.iter_mut()).enumerate() {
            let j = c * SAMPLE_CHUNK + k;
            *w = weight(j);
            let gained = *w * gain(j);
            *eff = gained * (1.0 - rest_offset[j]);
            partial += f64::from(gained * rest_offset[j]);
        }
        partial
    };
    #[cfg(feature = "parallel")]
    let partials: Vec<f64> = {
        use rayon::prelude::*;
        w.par_chunks_mut(SAMPLE_CHUNK)
            .zip(eff.par_chunks_mut(SAMPLE_CHUNK))
            .enumerate()
            .map(fill)
            .collect()
    };
    #[cfg(not(feature = "parallel"))]
    let partials: Vec<f64> = w
        .chunks_mut(SAMPLE_CHUNK)
        .zip(eff.chunks_mut(SAMPLE_CHUNK))
        .enumerate()
        .map(fill)
        .collect();
    partials.iter().sum()
}

/// `(0..n).map(f).collect()`, in parallel with the `parallel` feature (same result either way).
fn par_map<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync + Send) -> Vec<T> {
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        (0..n)
            .into_par_iter()
            .with_min_len(1024.min(n.max(1)))
            .map(f)
            .collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        (0..n).map(f).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_core::Lead;

    /// A lattice ball of LV cells at the origin.
    fn ball(radius_mm: f32) -> CellCloud {
        CellCloud::test_sphere(1.0, radius_mm, Region::LeftVentricle)
    }

    fn sites() -> ElectrodeSet {
        let mut s = ElectrodeSet::default();
        for (e, p) in [
            (Electrode::RA, Vec3::new(-170.0, 190.0, 43.0)),
            (Electrode::LA, Vec3::new(100.0, 190.0, 43.0)),
            (Electrode::LL, Vec3::new(64.0, -300.0, 64.0)),
            (Electrode::RL, Vec3::new(-134.0, -300.0, 64.0)),
            (Electrode::V1, Vec3::new(-59.0, 15.0, 80.0)),
            (Electrode::V2, Vec3::new(-11.0, 15.0, 80.0)),
            (Electrode::V3, Vec3::new(34.0, 0.0, 75.0)),
            (Electrode::V4, Vec3::new(85.0, -15.0, 59.0)),
            (Electrode::V5, Vec3::new(129.0, -15.0, 18.0)),
            (Electrode::V6, Vec3::new(140.0, -15.0, -35.0)),
        ] {
            s.set(e.label(), p);
        }
        s
    }

    #[test]
    fn uniform_vm_gives_no_potential() {
        let cloud = ball(5.0);
        let m = ForwardModel::build(&cloud, &sites(), ForwardOptions::default()).unwrap();
        let p = m.sample(&vec![0.7; cloud.len()]);
        for e in SIGNAL_ELECTRODES {
            assert!(p.get(e).abs() < 1e-5, "{}: {}", e.label(), p.get(e));
        }
    }

    #[test]
    fn linear_field_matches_the_analytic_dipole() {
        // V = −x·d/10 inside a 5 mm ball: dipole moment k·vol·(d/10) along d (front moves along d).
        let cloud = ball(5.0);
        let d = Vec3::new(0.3, -0.5, 0.8).normalize();
        let vm: Vec<f32> = cloud.positions.iter().map(|x| 0.5 - x.dot(d) / 10.0).collect();
        let opts = ForwardOptions::default();
        let m = ForwardModel::build(&cloud, &sites(), opts.clone()).unwrap();
        let p = m.sample(&vm);
        let moment = d / 10.0 * (cloud.len() as f32 * opts.gain_mv);
        for e in SIGNAL_ELECTRODES {
            let r = m.electrode_position(e);
            let expect = moment.dot(r) / r.length().powi(3);
            let got = p.get(e) as f32;
            assert!(
                (got - expect).abs() < 0.02 * expect.abs().max(1e-4),
                "{}: {got} vs {expect}",
                e.label()
            );
        }
    }

    #[test]
    fn double_layer_on_the_boundary_agrees_with_the_volume_source() {
        let cloud = ball(6.0);
        let d = Vec3::new(-0.4, 0.2, 0.9).normalize();
        let vm: Vec<f32> = cloud.positions.iter().map(|x| 0.5 + x.dot(d) / 12.0).collect();
        let vol = ForwardModel::build(&cloud, &sites(), ForwardOptions::default()).unwrap();
        let edl = ForwardModel::build(
            &cloud,
            &sites(),
            ForwardOptions {
                source: SourceModel::DoubleLayer,
                patches: Some(SurfacePatches::boundary_of(&cloud)),
                ..ForwardOptions::default()
            },
        )
        .unwrap();
        let (a, b) = (vol.sample(&vm), edl.sample(&vm));
        for e in SIGNAL_ELECTRODES {
            // Same field shape; the layer is ≈ 25 % weaker on this coarse ball (see `boundary_of`).
            let ratio = b.get(e) / a.get(e);
            assert!(
                (0.65..1.05).contains(&ratio),
                "{}: volume {} vs EDL {}",
                e.label(),
                a.get(e),
                b.get(e)
            );
        }
    }

    #[test]
    fn einthoven_limbs_recover_a_central_dipole_axis() {
        let cloud = ball(5.0);
        let m = ForwardModel::build(&cloud, &sites(), ForwardOptions::default()).unwrap();
        for deg in [-60.0f32, 0.0, 60.0, 120.0] {
            let r = deg.to_radians();
            let d = Vec3::new(r.cos(), -r.sin(), 0.0);
            let vm: Vec<f32> = cloud.positions.iter().map(|x| 0.5 - x.dot(d) / 10.0).collect();
            let leads = m.sample(&vm).to_leads();
            let axis = heartsim_core::FrontalAxis::from_limb_leads(&leads).degrees;
            assert!((axis - f64::from(deg)).abs() < 1.0, "{deg}: {axis}");
            assert_eq!(leads.get(Lead::II) > 0.0, (deg - 60.0).abs() < 90.0);
        }
    }

    #[test]
    fn gains_and_offsets_fold_into_the_weights() {
        let cloud = ball(4.0);
        let vm: Vec<f32> = cloud.positions.iter().map(|x| 0.5 - x.x / 10.0).collect();
        let mut m = ForwardModel::build(&cloud, &sites(), ForwardOptions::default()).unwrap();
        let base = m.sample(&vm).get(Electrode::LA);
        m.set_gains(WaveGains {
            p: 1.0,
            qrs: 2.0,
            t: 0.5,
        });
        m.set_ventricular_phase(VentricularPhase::Depolarisation);
        assert!((m.sample(&vm).get(Electrode::LA) - 2.0 * base).abs() < 1e-6 * base.abs().max(1.0));
        m.set_ventricular_phase(VentricularPhase::Repolarisation);
        assert!((m.sample(&vm).get(Electrode::LA) - 0.5 * base).abs() < 1e-6 * base.abs().max(1.0));
        m.set_gains(WaveGains::default());
        // A uniform rest offset only rescales the field: o + (1 − o)·V.
        m.set_v_rest_offset(&vec![0.2; cloud.len()]);
        assert!((m.sample(&vm).get(Electrode::LA) - 0.8 * base).abs() < 1e-5 * base.abs().max(1.0));
    }

    #[test]
    fn engine_state_selects_the_ventricular_gain() {
        let cloud = ball(4.0);
        let mut m = ForwardModel::build(&cloud, &sites(), ForwardOptions::default()).unwrap();
        let mut state = CellState::new(cloud.len());
        state.vm = cloud.positions.iter().map(|x| 0.5 - x.x / 10.0).collect();
        let base = m.sample(&state.vm).get(Electrode::LA);
        m.set_gains(WaveGains {
            p: 1.0,
            qrs: 3.0,
            t: 1.0,
        });
        state.activation_ms[0] = 100.0;
        assert_eq!(
            m.ventricular_phase_of(&state, 105.0),
            VentricularPhase::Depolarisation
        );
        let qrs = m.sample_state(&state, 105.0).get(Electrode::LA);
        assert!((qrs - 3.0 * base).abs() < 1e-5 * base.abs());
        assert_eq!(
            m.ventricular_phase_of(&state, 300.0),
            VentricularPhase::Repolarisation
        );
        let t = m.sample_state(&state, 300.0).get(Electrode::LA);
        assert!((t - base).abs() < 1e-5 * base.abs());
    }

    #[test]
    fn stats_rows_are_balanced() {
        let cloud = ball(5.0);
        let m = ForwardModel::build(&cloud, &sites(), ForwardOptions::default()).unwrap();
        for s in m.weight_stats() {
            assert!(s.sum.abs() < 1e-3 * s.abs_sum, "{s:?}");
            assert!(s.nonzero > 0 && s.effective_cells > 1.0);
        }
    }
}
