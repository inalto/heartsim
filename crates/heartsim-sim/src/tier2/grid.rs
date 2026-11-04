//! Masked voxel grid: a box of `dims` voxels of edge `h_mm`, of which only the *active* ones
//! (inside myocardium) are stored, in z-slab-major order (z, then y, then x) so that a
//! contiguous range of active voxels is a stack of slabs.

use glam::Vec3;

/// Marker for "no active voxel here" in the dense index.
pub const NO_VOXEL: u32 = u32::MAX;

/// Electrical compartment of a voxel. Diffusion never crosses compartments: the atria and the
/// ventricles are insulated by the fibrous AV ring, exactly as in Tier 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Compartment {
    Atrial = 1,
    Ventricular = 2,
}

/// What a voxel holds when the grid is built (see [`VoxelGrid::from_fn`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelSpec {
    pub compartment: Compartment,
    /// Unit fibre direction, or zero for isotropic tissue.
    pub fibre: Vec3,
}

/// The 9 "positive" lattice directions of the 18-neighbour stencil (6 faces + 12 edges, each
/// edge pair stored once); the negative ones are their opposites.
pub const DIRECTIONS: [[i32; 3]; 9] = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, 1, 0],
    [1, -1, 0],
    [1, 0, 1],
    [1, 0, -1],
    [0, 1, 1],
    [0, 1, -1],
];

/// Active voxels of a box, with a dense index for neighbour lookups.
#[derive(Debug, Clone)]
pub struct VoxelGrid {
    pub dims: [usize; 3],
    pub h_mm: f32,
    /// Centre of voxel (0, 0, 0), mm.
    pub origin: Vec3,
    /// Dense box → active voxel id (`NO_VOXEL` outside the tissue).
    index: Vec<u32>,
    /// Lattice coordinates of each active voxel.
    pub ijk: Vec<[u16; 3]>,
    pub compartment: Vec<Compartment>,
    /// Unit fibre direction per active voxel (zero = isotropic).
    pub fibre: Vec<Vec3>,
}

impl VoxelGrid {
    /// Build a grid by asking `spec` about every voxel centre (`None` = outside the tissue).
    /// Dimensions are capped at `u16::MAX` per axis.
    pub fn from_fn(
        dims: [usize; 3],
        h_mm: f32,
        origin: Vec3,
        mut spec: impl FnMut([usize; 3], Vec3) -> Option<VoxelSpec>,
    ) -> Self {
        assert!(dims.iter().all(|&d| d > 0 && d < usize::from(u16::MAX)));
        let mut g = Self {
            dims,
            h_mm,
            origin,
            index: vec![NO_VOXEL; dims[0] * dims[1] * dims[2]],
            ijk: Vec::new(),
            compartment: Vec::new(),
            fibre: Vec::new(),
        };
        for k in 0..dims[2] {
            for j in 0..dims[1] {
                for i in 0..dims[0] {
                    let p = origin + Vec3::new(i as f32, j as f32, k as f32) * h_mm;
                    if let Some(s) = spec([i, j, k], p) {
                        g.index[i + dims[0] * (j + dims[1] * k)] = g.ijk.len() as u32;
                        g.ijk.push([i as u16, j as u16, k as u16]);
                        g.compartment.push(s.compartment);
                        g.fibre.push(s.fibre);
                    }
                }
            }
        }
        g
    }

    /// A one-voxel-thick `nx × ny` sheet of ventricular tissue, fibres along x.
    #[must_use]
    pub fn sheet(nx: usize, ny: usize, h_mm: f32) -> Self {
        Self::block([nx, ny, 1], h_mm, Vec3::X)
    }

    /// A full box of ventricular tissue with uniform fibres (`Vec3::ZERO` = isotropic).
    #[must_use]
    pub fn block(dims: [usize; 3], h_mm: f32, fibre: Vec3) -> Self {
        Self::from_fn(dims, h_mm, Vec3::ZERO, |_, _| {
            Some(VoxelSpec {
                compartment: Compartment::Ventricular,
                fibre,
            })
        })
    }

    /// Number of active voxels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ijk.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ijk.is_empty()
    }

    /// Active voxel at lattice coordinates (signed, so callers can step off the box).
    #[must_use]
    pub fn voxel_at(&self, i: i64, j: i64, k: i64) -> Option<u32> {
        let [nx, ny, nz] = self.dims.map(|d| i64::try_from(d).unwrap_or(i64::MAX));
        if i < 0 || j < 0 || k < 0 || i >= nx || j >= ny || k >= nz {
            return None;
        }
        let v = self.index[(i + nx * (j + ny * k)) as usize];
        (v != NO_VOXEL).then_some(v)
    }

    /// Centre of an active voxel, mm.
    #[must_use]
    pub fn position(&self, v: u32) -> Vec3 {
        let [i, j, k] = self.ijk[v as usize];
        self.origin + Vec3::new(f32::from(i), f32::from(j), f32::from(k)) * self.h_mm
    }

    /// Continuous lattice coordinates of a point (voxel centres at integers).
    #[must_use]
    pub fn lattice(&self, p: Vec3) -> Vec3 {
        (p - self.origin) / self.h_mm
    }

    /// Active voxels whose centre lies within `radius_mm` of `p`, optionally restricted to one
    /// compartment, in index order.
    #[must_use]
    pub fn voxels_within(&self, p: Vec3, radius_mm: f32, compartment: Option<Compartment>) -> Vec<u32> {
        let c = self.lattice(p);
        let r = (radius_mm / self.h_mm).ceil() as i64;
        let (ci, cj, ck) = (c.x.round() as i64, c.y.round() as i64, c.z.round() as i64);
        let mut out = Vec::new();
        for k in ck - r..=ck + r {
            for j in cj - r..=cj + r {
                for i in ci - r..=ci + r {
                    if let Some(v) = self.voxel_at(i, j, k)
                        && compartment.is_none_or(|cp| self.compartment[v as usize] == cp)
                        && self.position(v).distance(p) <= radius_mm
                    {
                        out.push(v);
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Nearest active voxel to `p` within `max_mm` (ties → lower index).
    #[must_use]
    pub fn nearest(&self, p: Vec3, max_mm: f32, compartment: Option<Compartment>) -> Option<u32> {
        self.voxels_within(p, max_mm, compartment)
            .into_iter()
            .map(|v| (self.position(v).distance_squared(p), v))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, v)| v)
    }

    /// Stencil coefficients of the diffusion tensor `D = D_t I + (D_l − D_t) f fᵀ` along the 9
    /// [`DIRECTIONS`], mm²/ms: `Σ_k c_k e_k e_kᵀ = D` when D is diagonally dominant; negative
    /// face coefficients (strongly anisotropic oblique fibres) are clipped to 0 so that the
    /// scheme stays monotone (maximum principle), at the cost of a little extra cross-fibre
    /// diffusion there.
    #[must_use]
    pub fn stencil(fibre: Vec3, d_long: f32, d_trans: f32) -> [f32; 9] {
        let f = if fibre.length_squared() > 0.0 {
            fibre.normalize()
        } else {
            Vec3::ZERO
        };
        let dd = d_long - d_trans;
        let d = |a: usize, b: usize| dd * f[a] * f[b] + if a == b { d_trans } else { 0.0 };
        let (xy, xz, yz) = (d(0, 1), d(0, 2), d(1, 2));
        [
            (d(0, 0) - xy.abs() - xz.abs()).max(0.0),
            (d(1, 1) - xy.abs() - yz.abs()).max(0.0),
            (d(2, 2) - xz.abs() - yz.abs()).max(0.0),
            xy.max(0.0),
            (-xy).max(0.0),
            xz.max(0.0),
            (-xz).max(0.0),
            yz.max(0.0),
            (-yz).max(0.0),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn stencil_reproduces_the_tensor() {
        // oblique in-plane fibre: diagonally dominant, exact
        let f = Vec3::new(1.0, 1.0, 0.0).normalize();
        let c = VoxelGrid::stencil(f, 1.0, 0.25);
        let mut d = [[0.0f32; 3]; 3];
        for (k, e) in DIRECTIONS.iter().enumerate() {
            for a in 0..3 {
                for b in 0..3 {
                    d[a][b] += c[k] * (e[a] * e[b]) as f32;
                }
            }
        }
        let want = |a: usize, b: usize| 0.75 * f[a] * f[b] + if a == b { 0.25 } else { 0.0 };
        for a in 0..3 {
            for b in 0..3 {
                assert!((d[a][b] - want(a, b)).abs() < 1e-6, "D[{a}][{b}]");
            }
        }
    }

    #[test]
    fn grid_lookup_and_neighbourhoods() {
        let g = VoxelGrid::block([4, 3, 2], 2.0, Vec3::X);
        assert_eq!(g.len(), 24);
        assert_eq!(g.voxel_at(1, 2, 1), Some(1 + 4 * (2 + 3)));
        assert_eq!(g.voxel_at(-1, 0, 0), None);
        assert_eq!(g.nearest(Vec3::new(2.1, 0.0, 0.0), 2.0, None), Some(1));
        assert_eq!(g.voxels_within(Vec3::ZERO, 2.0, None), vec![0, 1, 4, 12]);
    }
}
