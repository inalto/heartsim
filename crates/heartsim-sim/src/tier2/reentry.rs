//! Re-entry tools (brief B16 §5): S1–S2 cross-field initiation, obstacles and scar, APD
//! heterogeneity patches. The phase-singularity diagnostic is
//! [`Monodomain::phase_singularities`].
//!
//! Regimes verified by the acceptance tests (`tests/tier2.rs`): the default Aliev–Panfilov cell
//! holds one rotor (monomorphic VT); the same cell on [`breakup_substrate`] breaks up into
//! multiple wavelets (VF); a Mitchell–Schaeffer cell holds one stable rotor. The Aliev–Panfilov
//! "break-up" constants of Panfilov 1999 (a = 0.1, μ₁ = 0.07) did *not* break up in our 2-D
//! sheets, so the break-up preset relies on dispersion of refractoriness instead.

use super::calibrate;
use super::model::CellModel;
use super::solver::Monodomain;
use glam::Vec3;
use std::collections::BTreeMap;

/// S1–S2 cross-field protocol (Starmer / Winfree): a planar S1 wave from the low-x face at
/// `now`, then an S2 stimulus over the quadrant `x < X/2, y < Y/2` (all z) at `now + s2_ms`.
/// With S2 timed into the S1 wave's tail the S2 front can only propagate away from the
/// refractory zone, and its free end curls into a rotor. Returns the S2 time.
pub fn s1_s2_cross_field<M: CellModel>(solver: &mut Monodomain<M>, s2_ms: f64, amplitude: f32) -> f64 {
    let g = solver.grid();
    let (nx, ny) = (g.dims[0], g.dims[1]);
    let s1: Vec<u32> = (0..g.len() as u32)
        .filter(|&v| g.ijk[v as usize][0] <= 1)
        .collect();
    let s2: Vec<u32> = (0..g.len() as u32)
        .filter(|&v| {
            let [i, j, _] = g.ijk[v as usize];
            usize::from(i) < nx / 2 && usize::from(j) < ny / 2
        })
        .collect();
    let now = solver.now_ms();
    solver.stimulate(&s1, now, 2.0, amplitude);
    solver.stimulate(&s2, now + s2_ms, 2.0, amplitude);
    now + s2_ms
}

/// Make every voxel within `radius_mm` of `centre` inert (an anatomical obstacle or a
/// transmural scar/ablation lesion). Returns the number of voxels affected.
pub fn obstacle_sphere<M: CellModel>(solver: &mut Monodomain<M>, centre: Vec3, radius_mm: f32) -> usize {
    let vox = solver.grid().voxels_within(centre, radius_mm, None);
    solver.set_conducting(&vox, false);
    vox.len()
}

/// Inert box (mm, inclusive) — e.g. a line of block for anatomical re-entry.
pub fn obstacle_box<M: CellModel>(solver: &mut Monodomain<M>, lo: Vec3, hi: Vec3) -> usize {
    let vox = solver.voxels_in_box(lo, hi, None);
    solver.set_conducting(&vox, false);
    vox.len()
}

/// Scale the APD of every voxel within `radius_mm` of `centre` by `apd_scale` (each tissue
/// class found there gets a scaled twin; diffusivities unchanged). Long-APD islands are the
/// classic substrate of unidirectional block and wavebreak.
pub fn apd_patch<M: CellModel>(
    solver: &mut Monodomain<M>,
    centre: Vec3,
    radius_mm: f32,
    apd_scale: f32,
) -> usize {
    let vox = solver.grid().voxels_within(centre, radius_mm, None);
    scale_voxels(solver, &vox, apd_scale, 1.0);
    vox.len()
}

/// Scale the APD by `apd_scale` and the measured conduction velocity by `cv_scale` on `voxels`
/// ([`calibrate::retarget`]: the diffusivity is re-solved for the new action potential, since
/// shortening the plateau alone would stop conduction on a coarse grid). Each tissue class found
/// there gets a re-targeted twin, so overlapping calls compound.
pub fn scale_voxels<M: CellModel>(solver: &mut Monodomain<M>, voxels: &[u32], apd_scale: f32, cv_scale: f32) {
    let (h, dt) = (solver.grid().h_mm, solver.dt_ms());
    let mut twins: BTreeMap<u16, Vec<u32>> = BTreeMap::new();
    for &v in voxels {
        twins.entry(solver.classes()[v as usize]).or_default().push(v);
    }
    let mut assign = Vec::with_capacity(twins.len());
    for (class, members) in twins {
        let twin = calibrate::retarget(&solver.tissue()[usize::from(class)], apd_scale, cv_scale, h, dt);
        assign.push((solver.add_tissue(twin), members));
    }
    solver.set_classes(&assign);
}

/// Break-up preset: an APD mosaic of 12 mm patches with APD × 0.6 … 1.4 ([`apd_mosaic`]). A
/// rotor in this substrate fragments within a few rotations (heterogeneity-induced wavebreak,
/// one of the break-up mechanisms catalogued by Fenton et al. 2002).
pub fn breakup_substrate<M: CellModel>(solver: &mut Monodomain<M>, seed: u64) {
    apd_mosaic(solver, 12.0, 0.6, 1.4, seed);
}

/// Dispersion of refractoriness: tile the grid with cubes of edge `patch_mm` whose APD is
/// scaled by a factor drawn uniformly from `[lo, hi]` (deterministic in `seed`). Wavefronts
/// meeting a still-refractory patch break, which turns a single Aliev–Panfilov rotor into
/// multiple wavelets (research §2.3.6: "breakup with heterogeneity").
pub fn apd_mosaic<M: CellModel>(solver: &mut Monodomain<M>, patch_mm: f32, lo: f32, hi: f32, seed: u64) {
    apd_mosaic_in(solver, None, patch_mm, lo, hi, seed);
}

/// [`apd_mosaic`] restricted to `voxels` (`None` = every voxel). The patch lattice is that of
/// the whole grid, so the mosaic of a sub-region does not depend on the region's shape.
pub fn apd_mosaic_in<M: CellModel>(
    solver: &mut Monodomain<M>,
    voxels: Option<&[u32]>,
    patch_mm: f32,
    lo: f32,
    hi: f32,
    seed: u64,
) {
    let g = solver.grid();
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        // splitmix64
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
    };
    let n = |d: usize| ((d as f32 * g.h_mm) / patch_mm).ceil().max(1.0) as usize;
    let (px, py, pz) = (n(g.dims[0]), n(g.dims[1]), n(g.dims[2]));
    let scale: Vec<f32> = (0..px * py * pz).map(|_| lo + (hi - lo) * next()).collect();
    let patch_of = |v: usize| {
        let p = |d: usize| ((f32::from(g.ijk[v][d]) * g.h_mm) / patch_mm) as usize;
        p(0) + px * (p(1) + py * p(2))
    };
    // group voxels by (patch, class) and give each group a scaled twin class
    let mut groups: BTreeMap<(usize, u16), Vec<u32>> = BTreeMap::new();
    let all: Vec<u32>;
    let members: &[u32] = if let Some(v) = voxels {
        v
    } else {
        all = (0..g.len() as u32).collect();
        &all
    };
    for &v in members {
        let v = v as usize;
        groups
            .entry((patch_of(v), solver.classes()[v]))
            .or_default()
            .push(v as u32);
    }
    let (h, dt) = (solver.grid().h_mm, solver.dt_ms());
    let mut assign = Vec::with_capacity(groups.len());
    for ((patch, class), members) in groups {
        // quantised to 0.05 so that the (memoised) re-targeting is shared between patches
        let s = (scale[patch] * 20.0).round() / 20.0;
        let twin = calibrate::retarget(&solver.tissue()[usize::from(class)], s, 1.0, h, dt);
        assign.push((solver.add_tissue(twin), members));
    }
    solver.set_classes(&assign);
}
