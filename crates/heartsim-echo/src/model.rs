//! The motion model on the anatomy pack's meshes: construction once, evaluation per drawn frame.

use crate::coords::{
    CardiacFrame, CellCoords, ChamberAxis, FrameError, MotionChamber, cardiac_frame, cell_chambers,
    working_proxies,
};
use crate::history::MotionHistory;
use crate::params::MotionParams;
use crate::phases::{CardiacPhase, Side, VolumeInputs, VolumeState, contraction, smoothstep, volume_at};
use crate::valves::{ValveDisc, split_valve_discs};
use glam::{DVec3, Mat3, Vec3, Vec3A, Vec4};
use heartsim_core::math::Deterministic;
use heartsim_ecg::Valve;
use heartsim_geometry::{
    AnatomyPack, CellCloud, HashGrid, Hood, K_NEIGHBOURS, Landmarks, MeshKind, NamedMesh, TriMesh, Vertex,
    compute_hoods,
};
use std::collections::BTreeMap;

/// Largest exaggeration [`MotionInput::exaggeration`] accepts. Mesh safety is tested at 1× only
/// (2× is reported by the safety test); a large factor can fold the thin walls.
pub const MAX_EXAGGERATION: f32 = 3.0;

/// A disc vertex's annulus anchors: ventricle-mesh vertices within this distance of the disc, mm.
const ANCHOR_RADIUS_MM: f32 = 3.0;
/// Fewest anchors a disc needs; with fewer the nearest ventricle vertices are taken.
const MIN_ANCHORS: usize = 24;
/// Parent vertices an attached vertex interpolates (modified Shepard weights).
const FOLLOW_K: usize = 8;
/// Bin size of the spatial grids built at construction, mm.
const FOLLOW_GRID_MM: f32 = 4.0;
/// Passes of neighbour averaging of the per-vertex depth and chamber shares (construction): the
/// hoods take the eight nearest cells, so neighbouring vertices can mix different cells and the raw
/// fields jump; a jump times a 15 mm excursion folds the small triangles of the decimated meshes.
/// Designed.
const TEMPLATE_SMOOTHING_PASSES: usize = 24;
/// Passes of diffusion of the cells' chamber membership over the neighbour graph before it is
/// interpolated to the vertices (≈ 1 cm): the LV and RV templates differ by centimetres, and a sharp
/// blend between them folds the junction. Designed.
const SHARE_DIFFUSION_PASSES: usize = 160;
/// The transmural depth of a vertex: Gaussian over this many nearest cells with this width. Designed.
const DEPTH_CELLS: usize = 128;
const DEPTH_SIGMA_MM: f32 = 3.0;
/// Passes of diffusion of the injury over the cells' same-chamber neighbours before it scales
/// the wall motion (the border of an akinetic territory is tethered to it). Designed.
const INJURY_SPREAD_PASSES: usize = 30;
/// Passes of averaging of the kinesis field over each mesh (when the injury is set). Designed.
const KINESIS_SMOOTHING_PASSES: usize = 10;
/// Passes of averaging of a vessel's displacement over its mesh (every frame). Numerical.
const FOLLOWER_SMOOTHING_PASSES: usize = 3;
/// Half-width of the rounded corners of the height clamp. Designed.
const SOFT_CLAMP: f32 = 0.1;
/// Bounds of the solved global amplitude.
const BETA_RANGE: (f64, f64) = (-3.0, 3.0);
/// A vertex within this distance of the parts it rests on lies on them (its contact), mm: the pack's
/// meshes touch (≤ 1.5 mm) or are separated by a little fat. A vessel moves in full within it of
/// its root. Designed.
const ATTACH_INNER_MM: f32 = 3.0;
/// Away from its contact, the correction that attaches a part to its parents fades to 0 over this
/// distance, mm, per part (see [`attach_fade_mm`]). Designed: the RV free wall reaches its own
/// long-axis and inward motion about 3 cm from the septum and its insertion lines, the atrial wall
/// its own about 3 cm above the AV junction; a disc's inner part moves with the affine fit of its
/// annulus; a vessel follows its own root beyond its contacts.
const RV_ATTACH_FADE_MM: f32 = 35.0;
const ATRIUM_ATTACH_FADE_MM: f32 = 30.0;
const DISC_ATTACH_FADE_MM: f32 = 6.0;
const VESSEL_ATTACH_FADE_MM: f32 = 15.0;
/// Spacing of the control vertices the attaching correction is computed on, mm; passes of
/// neighbour averaging over their graph (every frame: the exact correction carries the parent
/// mesh's vertex-to-vertex variation, which folds the sub-millimetre triangles where two meshes
/// meet); controls each vertex interpolates. Numerical.
const CONTROL_MM: f32 = 2.0;
/// The valve discs and the vessels (small, thin, between several parents) use every vertex as a
/// control, averaged this many passes. Numerical.
const ATTACH_SMOOTHING_PASSES: usize = 12;
const CONTROL_SMOOTHING_PASSES: usize = 4;
const INTERP_K: usize = 8;
/// Passes of neighbour averaging of the share a vertex takes of the extended correction
/// (construction): the share then passes smoothly round the thin wedges where a wall's two faces
/// meet at a contact line. Numerical.
const TAKE_SMOOTHING_PASSES: usize = 40;
/// Smallest cell of the grid a contact correction is extended through space on, mm (a fifth of the
/// fade when that is larger). Numerical.
const CONTACT_GRID_MM: f32 = 5.0;
/// Regularisation of the interpolation weights of nearest vertices, mm: the field a
/// sub-millimetre triangle of one mesh reads from another mesh's vertices is that mesh's local
/// mean, not one vertex's own value. Numerical.
const SHEPARD_EPS_MM: f32 = 1.0;
/// Within the first distance of its parents a vertex takes the exact correction (their
/// displacement), passing to the extended one by the second, mm: the RV, on the LV. Numerical.
const ATTACH_EXACT_MM: f32 = 1.0;
const ATTACH_DIRECT_MM: f32 = 3.0;
/// The same band for the valve discs, whose rim lies 1–3 mm from their annulus. Designed.
const DISC_EXACT_BAND_MM: (f32, f32) = (2.5, 4.0);
/// The same band for the atria and the vessels, which keep only [`THIN_EXACT_SHARE`] of the exact
/// correction (the rest is the extended one): their thin walls meet the ventricles and one another
/// at sub-millimetre triangles that the full local shear of the wall they rest on folds (an LA
/// triangle on the LV base in atrial fibrillation, the aortic root between the outflow tracts).
/// Designed: the largest share that folds none in the safety test's rhythms.
const THIN_EXACT_BAND_MM: (f32, f32) = (1.0, 3.0);
const THIN_EXACT_SHARE: f32 = 0.3;

/// Why a [`MotionModel`] could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotionError {
    Frame(FrameError),
    /// The pack has no `lv` myocardium mesh.
    NoLeftVentricle,
    /// The LV mesh has no recognisable endocardial surface.
    NoCavity,
}

impl std::fmt::Display for MotionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MotionError::Frame(e) => write!(f, "cardiac frame: {e}"),
            MotionError::NoLeftVentricle => write!(f, "no `lv` myocardium mesh"),
            MotionError::NoCavity => write!(f, "no LV endocardial surface found"),
        }
    }
}

impl std::error::Error for MotionError {}

impl From<FrameError> for MotionError {
    fn from(e: FrameError) -> Self {
        MotionError::Frame(e)
    }
}

/// Per-vertex template of a hooded (myocardium) mesh, fixed at construction.
#[derive(Debug, Clone, Copy, Default)]
struct VertexTemplate {
    /// Full LV / RV excursion at amplitude 1, already weighted by the vertex's share of that
    /// ventricle's cells.
    u_lv: Vec3,
    u_rv: Vec3,
    /// The long-axis part of `u_rv` (before the TAPSE normalisation, see
    /// [`MotionModel::rv_long_scale`]).
    u_rv_long: Vec3,
    /// The radial (inward) part of `u_lv`: what an injured LV wall loses.
    u_lv_rad: Vec3,
    /// The ventricular template the amplitude multiplies: `u_lv` less the inward motion an
    /// injured wall loses, plus `u_rv` (set with the injury).
    u_eff: Vec3,
    /// Rotation about the LV axis: the offset from the axis, its cross product with the axis,
    /// and the full end-systolic angle (radians, weighted by the LV share).
    rot_r: Vec3,
    rot_b: Vec3,
    rot_angle: f32,
    /// Atrial part: how much of the AV-plane displacement the vertex follows, and its
    /// contraction excursion at full contraction.
    atr_follow: f32,
    atr_contract: Vec3,
    /// LV share of the ventricular part (`shares[0] / (shares[0] + shares[1])`).
    lv_share: f32,
}

impl Owner {
    /// The chambers whose contraction a vertex reads, in slot order (LV, RV, atrium); `None`
    /// slots repeat the first.
    fn slots(self) -> [Option<MotionChamber>; 3] {
        use MotionChamber::{LeftAtrium, LeftVentricle, RightVentricle};
        match self {
            Owner::Chamber(c) => [Some(c), None, None],
            Owner::Ventricles => [Some(LeftVentricle), Some(RightVentricle), None],
            Owner::Any => [Some(LeftVentricle), Some(RightVentricle), Some(LeftAtrium)],
        }
    }
}

/// The chambers a mesh may draw its motion from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Ventricles,
    /// One chamber only.
    Chamber(MotionChamber),
    Any,
}

#[derive(Debug, Clone)]
struct Hooded {
    hoods: Vec<Hood>,
    /// Per vertex and per slot of [`Owner::slots`], the trilinear weights of the vertex on its
    /// chamber's coupling voxels: the contraction of each chamber reaches the vertex from that
    /// chamber's own voxel field, so it varies smoothly in space and does not jump where cells of
    /// another chamber come near.
    ctrl: Vec<DenseSample>,
    owner: Owner,
    templates: Vec<VertexTemplate>,
    adjacency: Adjacency,
    /// Per vertex, the fraction of its inward LV motion the wall keeps: `1 − injury`, spread,
    /// interpolated and smoothed (1 healthy). An injured region is hypo- or akinetic in every
    /// phase.
    kinesis: Vec<f32>,
    /// Compact per-frame copies of the templates (see [`VentHot`], [`AtrHot`]); empty when the
    /// part has no ventricular or no atrial share.
    vent: Vec<VentHot>,
    atr: Vec<AtrHot>,
}

/// What the per-frame ventricular fill reads per vertex (32 bytes).
#[derive(Debug, Clone, Copy, Default)]
struct VentHot {
    u_eff: Vec3,
    rot_r: Vec3,
    rot_angle: f32,
    lv_share: f32,
}

/// What the per-frame atrial fill reads per vertex (16 bytes).
#[derive(Debug, Clone, Copy, Default)]
struct AtrHot {
    contract: Vec3,
    follow: f32,
}

/// Vertex neighbours of a triangle mesh (compressed rows, ascending, no duplicates).
#[derive(Debug, Clone, Default)]
struct Adjacency {
    offsets: Vec<u32>,
    neighbours: Vec<u32>,
}

impl Adjacency {
    fn new(n: usize, indices: &[u32]) -> Self {
        let mut lists: Vec<Vec<u32>> = vec![Vec::new(); n];
        for t in indices.as_chunks::<3>().0 {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                lists[a as usize].push(b);
                lists[b as usize].push(a);
            }
        }
        let mut offsets = Vec::with_capacity(n + 1);
        let mut neighbours = Vec::new();
        offsets.push(0);
        for mut l in lists {
            l.sort_unstable();
            l.dedup();
            neighbours.extend(l);
            offsets.push(neighbours.len() as u32);
        }
        Self { offsets, neighbours }
    }

    fn of(&self, v: usize) -> &[u32] {
        &self.neighbours[self.offsets[v] as usize..self.offsets[v + 1] as usize]
    }

    /// `passes` of half-weight neighbour averaging of `x` (`tmp` is scratch).
    fn smooth<T>(&self, x: &mut Vec<T>, tmp: &mut Vec<T>, passes: usize)
    where
        T: Copy + Default + std::ops::Add<Output = T> + std::ops::Mul<f32, Output = T>,
    {
        for _ in 0..passes {
            tmp.clear();
            for v in 0..x.len() {
                let nb = self.of(v);
                if nb.is_empty() {
                    tmp.push(x[v]);
                    continue;
                }
                let mut s = T::default();
                for &u in nb {
                    s = s + x[u as usize];
                }
                tmp.push(x[v] * 0.5 + s * (0.5 / nb.len() as f32));
            }
            std::mem::swap(x, tmp);
        }
    }
}

/// Neighbours of every cell from the cloud's neighbour graph, all chambers.
fn cell_graph(cells: &CellCloud) -> Adjacency {
    let mut offsets = Vec::with_capacity(cells.len() + 1);
    let mut neighbours = Vec::new();
    offsets.push(0);
    for (i, nb) in cells.neighbours.iter().enumerate() {
        let mut l: Vec<u32> = nb
            .iter()
            .map(|n| n.index)
            .filter(|&j| (j as usize) < cells.len() && j as usize != i)
            .collect();
        l.sort_unstable();
        l.dedup();
        neighbours.extend(l);
        offsets.push(neighbours.len() as u32);
    }
    Adjacency { offsets, neighbours }
}

/// Where a vertex reads a chamber's dense coupling lattice.
#[derive(Debug, Clone, Copy, Default)]
struct DenseSample {
    base: u32,
    fr: [f32; 3],
}

/// Mechanical coupling of the contraction: each chamber's cells are binned into voxels, the voxel
/// means are averaged over their face neighbours of the same chamber, and every cell takes the
/// trilinear interpolation of the voxel field back. A few passes on the coarse grid reach as far
/// as many passes over the cells' own neighbour graph, at a fraction of the cost.
#[derive(Debug, Clone, Default)]
struct Coupling {
    /// Lattice origin and spacing, and every voxel by (chamber, ix, iy, iz).
    lo: Vec3,
    h: f32,
    index: BTreeMap<(usize, i32, i32, i32), u32>,
    /// Voxel centres of each chamber (for the nearest-voxel fallback) and their indices.
    chamber_voxels: Vec<(Vec<Vec3>, Vec<u32>)>,
    /// The dense lattice: origin (lattice index), size, the voxel each node copies (per chamber
    /// block) and the values this frame.
    dense_lo: (i32, i32, i32),
    dims: (usize, usize, usize),
    dense_src: Vec<u32>,
    dense: Vec<f32>,
    /// Voxel of each cell (for the restriction).
    own: Vec<u32>,
    adjacency: Adjacency,
    sum: Vec<f32>,
    /// `1 / (working cells in the voxel)`.
    count: Vec<f32>,
    field: Vec<f32>,
    /// The dense-lattice nodes some vertex reads (the others are never refreshed).
    used: Vec<u32>,
}

impl Coupling {
    /// Voxels of the `working` cells (the others move with them and are left out).
    fn new(cells: &CellCloud, chambers: &[MotionChamber], working: &[u32], h: f32) -> Self {
        let h = h.max(0.5);
        let n = cells.len();
        let mut lo = Vec3::splat(f32::INFINITY);
        for p in &cells.positions {
            lo = lo.min(*p);
        }
        let key = |p: Vec3| {
            let f = (p - lo) / h;
            (f.x.floor() as i32, f.y.floor() as i32, f.z.floor() as i32)
        };
        let mut index: BTreeMap<(usize, i32, i32, i32), u32> = BTreeMap::new();
        let mut own = vec![u32::MAX; n];
        for &i in working {
            let (x, y, z) = key(cells.positions[i as usize]);
            let next = index.len() as u32;
            own[i as usize] = *index
                .entry((chambers[i as usize].index(), x, y, z))
                .or_insert(next);
        }
        let voxels = index.len();
        let mut count = vec![0.0f32; voxels];
        for &o in own.iter().filter(|&&o| o != u32::MAX) {
            count[o as usize] += 1.0;
        }
        // face neighbours
        let mut lists: Vec<Vec<u32>> = vec![Vec::new(); voxels];
        for (&(ch, x, y, z), &v) in &index {
            for (dx, dy, dz) in [
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                if let Some(&u) = index.get(&(ch, x + dx, y + dy, z + dz)) {
                    lists[v as usize].push(u);
                }
            }
        }
        let mut offsets = vec![0u32];
        let mut neighbours = Vec::new();
        for mut l in lists {
            l.sort_unstable();
            neighbours.extend(l);
            offsets.push(neighbours.len() as u32);
        }
        let mut chamber_voxels: Vec<(Vec<Vec3>, Vec<u32>)> = vec![(Vec::new(), Vec::new()); 4];
        for (&(ch, x, y, z), &v) in &index {
            let c = lo + (Vec3::new(x as f32, y as f32, z as f32) + Vec3::splat(0.5)) * h;
            chamber_voxels[ch].0.push(c);
            chamber_voxels[ch].1.push(v);
        }
        let mut out = Self {
            lo,
            h,
            index,
            chamber_voxels,
            dense_lo: (0, 0, 0),
            dims: (0, 0, 0),
            dense_src: Vec::new(),
            dense: Vec::new(),
            used: Vec::new(),
            own,
            adjacency: Adjacency { offsets, neighbours },
            sum: vec![0.0; voxels],
            count: count.iter().map(|&c| 1.0 / c.max(1.0)).collect(),
            field: vec![0.0; voxels],
        };
        out.build_dense();
        out
    }

    /// The coupled voxel field of the per-cell values `c` (restriction to the voxels, then
    /// `passes` of averaging), in [`Self::field`].
    fn couple_field(&mut self, tmp: &mut Vec<f32>, passes: usize) {
        self.field.clear();
        self.field
            .extend(self.sum.iter().zip(&self.count).map(|(s, k)| s * k));
        let mut field = std::mem::take(&mut self.field);
        self.adjacency.smooth(&mut field, tmp, passes);
        self.field = field;
        self.sum.fill(0.0);
    }

    /// Build the dense per-chamber lattice the vertices read (see [`Self::dense`]): every lattice
    /// node of the cells' bounding box (two voxels of margin) takes the value of the nearest voxel
    /// of that chamber, so the field is defined, and continuous under trilinear interpolation,
    /// everywhere a vertex can be.
    fn build_dense(&mut self) {
        let margin = 2;
        let (mut lo_i, mut hi_i) = ((i32::MAX, i32::MAX, i32::MAX), (i32::MIN, i32::MIN, i32::MIN));
        for &(_, x, y, z) in self.index.keys() {
            lo_i = (lo_i.0.min(x), lo_i.1.min(y), lo_i.2.min(z));
            hi_i = (hi_i.0.max(x), hi_i.1.max(y), hi_i.2.max(z));
        }
        if self.index.is_empty() {
            return;
        }
        self.dense_lo = (lo_i.0 - margin, lo_i.1 - margin, lo_i.2 - margin);
        self.dims = (
            (hi_i.0 - lo_i.0 + 1 + 2 * margin) as usize,
            (hi_i.1 - lo_i.1 + 1 + 2 * margin) as usize,
            (hi_i.2 - lo_i.2 + 1 + 2 * margin) as usize,
        );
        let per = self.dims.0 * self.dims.1 * self.dims.2;
        self.dense_src = vec![0; per * 4];
        for ch in 0..4 {
            let (centres, ids) = &self.chamber_voxels[ch];
            if ids.is_empty() {
                continue;
            }
            let grid = HashGrid::new(centres, self.h * 2.0);
            for (z, zi) in (0..self.dims.2).zip(self.dense_lo.2..) {
                for (y, yi) in (0..self.dims.1).zip(self.dense_lo.1..) {
                    for (x, xi) in (0..self.dims.0).zip(self.dense_lo.0..) {
                        let key = (ch, xi, yi, zi);
                        let src = self.index.get(&key).copied().unwrap_or_else(|| {
                            let c = self.lo
                                + (Vec3::new(key.1 as f32, key.2 as f32, key.3 as f32) + Vec3::splat(0.5))
                                    * self.h;
                            grid.nearest(c).map_or(ids[0], |(_, j)| ids[j as usize])
                        });
                        self.dense_src[ch * per + (z * self.dims.1 + y) * self.dims.0 + x] = src;
                    }
                }
            }
        }
        self.dense = vec![0.0; per * 4];
    }

    /// Where the point `p` reads `chamber`'s dense lattice: the linear index of its lower corner
    /// and the fractions toward the upper one.
    fn sample_at(&self, p: Vec3, chamber: MotionChamber) -> DenseSample {
        let f = (p - self.lo) / self.h - Vec3::splat(0.5);
        let rel = [
            f.x - self.dense_lo.0 as f32,
            f.y - self.dense_lo.1 as f32,
            f.z - self.dense_lo.2 as f32,
        ];
        let dims = [self.dims.0, self.dims.1, self.dims.2];
        let mut base = [0usize; 3];
        let mut fr = [0f32; 3];
        for k in 0..3 {
            let top = (dims[k].max(2) - 2) as f32;
            let r = rel[k].clamp(0.0, top + 0.999_9);
            let b = r.floor().min(top);
            base[k] = b as usize;
            fr[k] = r - b;
        }
        let per = self.dims.0 * self.dims.1 * self.dims.2;
        DenseSample {
            base: (chamber.index() * per + (base[2] * self.dims.1 + base[1]) * self.dims.0 + base[0]) as u32,
            fr,
        }
    }

    /// Fill the used nodes of the dense lattice from the coupled voxel field.
    fn refresh_dense(&mut self) {
        for &u in &self.used {
            self.dense[u as usize] = self.field[self.dense_src[u as usize] as usize];
        }
    }

    /// Note the nodes a sample reads.
    fn mark_used(&self, s: &DenseSample, mark: &mut [bool]) {
        let (nx, nxy) = (self.dims.0, self.dims.0 * self.dims.1);
        let b = s.base as usize;
        for o in [0, 1, nx, nx + 1, nxy, nxy + 1, nxy + nx, nxy + nx + 1] {
            mark[b + o] = true;
        }
    }

    /// Trilinear value of the dense lattice at a sample.
    fn read(&self, s: &DenseSample) -> f32 {
        let (nx, nxy) = (self.dims.0, self.dims.0 * self.dims.1);
        let b = s.base as usize;
        let d = &self.dense;
        let [fx, fy, fz] = s.fr;
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let c00 = lerp(d[b], d[b + 1], fx);
        let c10 = lerp(d[b + nx], d[b + nx + 1], fx);
        let c01 = lerp(d[b + nxy], d[b + nxy + 1], fx);
        let c11 = lerp(d[b + nxy + nx], d[b + nxy + nx + 1], fx);
        lerp(lerp(c00, c10, fy), lerp(c01, c11, fy), fz)
    }
}

/// The triangles incident on each vertex of a triangle list, as compressed rows.
fn incident_faces(n: usize, indices: &[u32]) -> Adjacency {
    let mut deg = vec![0u32; n + 1];
    for &v in indices {
        deg[v as usize + 1] += 1;
    }
    for k in 0..n {
        deg[k + 1] += deg[k];
    }
    let mut fill = deg.clone();
    let mut neighbours = vec![0u32; indices.len()];
    for (f, t) in indices.as_chunks::<3>().0.iter().enumerate() {
        for &v in t {
            neighbours[fill[v as usize] as usize] = f as u32;
            fill[v as usize] += 1;
        }
    }
    Adjacency {
        offsets: deg,
        neighbours,
    }
}

/// Same-chamber neighbours of every cell from the cloud's neighbour graph.
fn cell_adjacency(cells: &CellCloud, chambers: &[MotionChamber]) -> Adjacency {
    let mut offsets = Vec::with_capacity(cells.len() + 1);
    let mut neighbours = Vec::new();
    offsets.push(0);
    for (i, nb) in cells.neighbours.iter().enumerate() {
        let mut l: Vec<u32> = nb
            .iter()
            .map(|n| n.index)
            .filter(|&j| (j as usize) < cells.len() && j as usize != i && chambers[j as usize] == chambers[i])
            .collect();
        l.sort_unstable();
        l.dedup();
        neighbours.extend(l);
        offsets.push(neighbours.len() as u32);
    }
    Adjacency { offsets, neighbours }
}

/// Refresh the effective ventricular templates of a hooded part from its kinesis.
fn refresh_effective(h: &mut Hooded) {
    for (t, &g) in h.templates.iter_mut().zip(&h.kinesis) {
        t.u_eff = t.u_lv - t.u_lv_rad * (1.0 - g) + t.u_rv;
    }
    let has_vent = h
        .templates
        .iter()
        .any(|t| t.u_eff != Vec3::ZERO || t.rot_angle != 0.0);
    let has_atr = h
        .templates
        .iter()
        .any(|t| t.atr_follow != 0.0 || t.atr_contract != Vec3::ZERO);
    h.vent = if has_vent {
        h.templates
            .iter()
            .map(|t| VentHot {
                u_eff: t.u_eff,
                rot_r: t.rot_r,
                rot_angle: t.rot_angle,
                lv_share: t.lv_share,
            })
            .collect()
    } else {
        Vec::new()
    };
    h.atr = if has_atr {
        h.templates
            .iter()
            .map(|t| AtrHot {
                contract: t.atr_contract,
                follow: t.atr_follow,
            })
            .collect()
    } else {
        Vec::new()
    };
}

/// Six scalars smoothed together (the LV and RV transmural depth and the four chamber shares).
#[derive(Debug, Clone, Copy, Default)]
struct S5([f32; 6]);

impl std::ops::Add for S5 {
    type Output = S5;
    fn add(self, o: S5) -> S5 {
        S5(std::array::from_fn(|i| self.0[i] + o.0[i]))
    }
}

impl std::ops::Mul<f32> for S5 {
    type Output = S5;
    fn mul(self, k: f32) -> S5 {
        S5(self.0.map(|x| x * k))
    }
}

/// One valve disc's affine fit to its annulus anchors.
#[derive(Debug, Clone)]
struct DiscFit {
    disc: ValveDisc,
    /// The ventricle part and its vertices the disc hangs on.
    anchor_part: usize,
    anchors: Vec<u32>,
    anchor_centre: Vec3,
    /// Rest offsets `q` of the anchors from their centre.
    anchor_q: Vec<Vec3>,
    /// `(Σ q qᵀ + λ I)⁻¹` of the anchors' offsets `q` from their centre.
    gram_inv: Mat3,
    /// The lateral point of the rim (TAPSE, MAPSE are read at the rim), mm.
    lateral_rim_mm: Vec3,
}

/// How a vessel follows its root (see [`vessel_root`]): per vertex, its nearest root vertices with
/// interpolation weights and the share `take` of their displacement it takes (1 within
/// [`ATTACH_INNER_MM`] of the root, fading to 0 over [`MotionParams::vessel_fade_mm`]); the
/// result is averaged over the vessel's mesh (`passes`).
#[derive(Debug, Clone, Default)]
struct Attach {
    /// The region the correction lives on: the attached vertices (take > 0) and their
    /// neighbours, ascending, as part vertex indices.
    region: Vec<u32>,
    /// Neighbours of each region vertex in region-local indices; `region.len()` stands for a
    /// vertex outside the region (correction 0).
    adjacency: Adjacency,
    /// Per attached vertex: its region-local index, parents, weights and share.
    local: Vec<u32>,
    parents: Vec<[(u32, u32); FOLLOW_K]>,
    weights: Vec<[f32; FOLLOW_K]>,
    take: Vec<f32>,
    passes: usize,
}

impl Attach {
    /// Blend part `k`'s own displacement (already in `disp[k]`) with its parents'.
    fn apply(&self, k: usize, disp: &mut [Vec<Vec3>], corr: &mut Vec<Vec3>, tmp: &mut Vec<Vec3>) {
        if self.local.is_empty() {
            return;
        }
        let mut mine = std::mem::take(&mut disp[k]);
        corr.clear();
        corr.resize(self.region.len() + 1, Vec3::ZERO);
        for (((&l, ps), ws), &take) in self
            .local
            .iter()
            .zip(&self.parents)
            .zip(&self.weights)
            .zip(&self.take)
        {
            let mut d = Vec3::ZERO;
            for (&(pp, pv), &w) in ps.iter().zip(ws) {
                d += disp[pp as usize][pv as usize] * w;
            }
            let v = self.region[l as usize] as usize;
            corr[l as usize] = (d - mine[v]) * take;
        }
        self.adjacency.smooth(corr, tmp, self.passes);
        for (&v, c) in self.region.iter().zip(corr.iter()) {
            mine[v as usize] += *c;
        }
        disp[k] = mine;
    }
}

/// The share of its parents' displacement a vertex `d_mm` from the nearest parent vertex takes:
/// 1 within [`ATTACH_INNER_MM`], then a smoothstep down to 0 over `fade_mm`.
fn attach_take(d_mm: f32, fade_mm: f32) -> f32 {
    1.0 - smoothstep(f64::from((d_mm - ATTACH_INNER_MM).max(0.0) / fade_mm.max(1e-3))) as f32
}

/// How a part rests on the parts moved before it (its parents, [`seniority`]). Near them (within
/// an exact band, [`ATTACH_EXACT_MM`] for the RV) a vertex takes the parents' displacement
/// exactly; the correction `parents − own` found on the contact (within [`ATTACH_INNER_MM`]) is
/// also extended smoothly through space (normalised convolution on a coarse grid,
/// [`CONTACT_GRID_MM`]) and fades with the distance to the contact ([`attach_fade_mm`]); across the
/// band a vertex passes from the exact to the extended correction (thin parts keep only
/// [`THIN_EXACT_SHARE`] of the exact). No vertex switches abruptly between rules (the decimated
/// meshes have sub-millimetre triangles that a step of a few tenths folds), and far from the
/// contact a vertex takes a smooth extension of it, not whichever parent vertices happen to be
/// nearest (an atrial wall a centimetre from both the LV and the aortic root).
///
/// The correction is smooth on the scale of millimetres, so on the RV and the atria it is computed
/// on control vertices about [`CONTROL_MM`] apart (one per occupied cell of that size), averaged
/// over their graph, and interpolated to every vertex of the region from its nearest controls;
/// on the discs and the vessels every vertex is a control.
#[derive(Debug, Clone, Default)]
struct Contact {
    /// Controls within the exact band of the parents: index into the control lists, parent
    /// vertices and weights, and how much of the exact correction they keep.
    direct: Vec<u32>,
    parents: Vec<[(u32, u32); FOLLOW_K]>,
    weights: Vec<[f32; FOLLOW_K]>,
    keep: Vec<f32>,
    /// The contact (indices into `direct`) and its trilinear splat on the grid.
    touching: Vec<u32>,
    splat_cell: Vec<Trilinear>,
    /// The grid: cells per axis and passes of the [1 2 1] blur.
    dims: [usize; 3],
    blur_passes: usize,
    /// The controls: part vertex, grid sample, share of the extended correction, and their
    /// neighbours (control indices).
    control: Vec<u32>,
    control_cell: Vec<Trilinear>,
    control_take: Vec<f32>,
    control_adjacency: Adjacency,
    control_passes: usize,
    /// The region the correction lives on (vertices with a share, and their neighbours) with
    /// each vertex's nearest controls and weights.
    region: Vec<u32>,
    interp_from: Vec<[u32; INTERP_K]>,
    interp_w: Vec<[f32; INTERP_K]>,
}

/// Trilinear weights of a point on a grid (8 cells).
#[derive(Debug, Clone, Copy, Default)]
struct Trilinear {
    cell: [u32; 8],
    w: [f32; 8],
}

impl Trilinear {
    fn at(p: Vec3, lo: Vec3, h: f32, dims: [usize; 3]) -> Self {
        let g = (p - lo) / h;
        let mut t = Trilinear::default();
        let base = [g.x.floor(), g.y.floor(), g.z.floor()];
        let f = [g.x - base[0], g.y - base[1], g.z - base[2]];
        for c in 0..8 {
            let mut idx = 0usize;
            let mut w = 1.0f32;
            for a in 0..3 {
                let bit = (c >> a) & 1;
                let step = if bit == 1 { 1.0 } else { 0.0 };
                let i = (base[a] + step).clamp(0.0, (dims[a] - 1) as f32) as usize;
                idx = idx * dims[a] + i;
                w *= if bit == 1 { f[a] } else { 1.0 - f[a] };
            }
            t.cell[c] = idx as u32;
            t.w[c] = w;
        }
        t
    }
}

/// Scratch of [`Contact::apply`].
#[derive(Debug, Clone, Default)]
struct ContactScratch {
    exact: Vec<Vec3>,
    /// The grid: correction (xyz) and weight (w).
    val: Vec<Vec4>,
    tmp_val: Vec<Vec4>,
    corr: Vec<Vec3A>,
    tmp: Vec<Vec3A>,
}

/// One pass of the [1 2 1] / 4 blur along every axis (cells outside count as 0).
fn blur3(x: &mut Vec<Vec4>, tmp: &mut Vec<Vec4>, dims: [usize; 3]) {
    let strides = [dims[1] * dims[2], dims[2], 1];
    for a in 0..3 {
        tmp.clear();
        tmp.resize(x.len(), Vec4::ZERO);
        let (n, st) = (dims[a], strides[a]);
        // the cells come in blocks of `n · st`: within one, step `st` along the axis
        for (xb, ob) in x.chunks_exact(n * st).zip(tmp.chunks_exact_mut(n * st)) {
            for k in 0..n {
                let lo = k * st;
                for j in 0..st {
                    let i = lo + j;
                    let mut v = xb[i] * 0.5;
                    if k > 0 {
                        v += xb[i - st] * 0.25;
                    }
                    if k + 1 < n {
                        v += xb[i + st] * 0.25;
                    }
                    ob[i] = v;
                }
            }
        }
        std::mem::swap(x, tmp);
    }
}

impl Contact {
    /// Correct part `k`'s own displacement (already in `disp[k]`) toward its parents'.
    fn apply(&self, k: usize, disp: &mut [Vec<Vec3>], sc: &mut ContactScratch) {
        if self.touching.is_empty() {
            return;
        }
        let mut mine = std::mem::take(&mut disp[k]);
        // the exact correction near the parents
        sc.exact.clear();
        sc.exact.extend(
            self.direct
                .iter()
                .zip(&self.parents)
                .zip(&self.weights)
                .map(|((&j, ps), ws)| {
                    let mut d = Vec3::ZERO;
                    for (&(pp, pv), &w) in ps.iter().zip(ws) {
                        d += disp[pp as usize][pv as usize] * w;
                    }
                    d - mine[self.control[j as usize] as usize]
                }),
        );
        // the contact's correction extended through space: splat, blur, normalise
        let cells = self.dims[0] * self.dims[1] * self.dims[2];
        sc.val.clear();
        sc.val.resize(cells, Vec4::ZERO);
        for (&j, t) in self.touching.iter().zip(&self.splat_cell) {
            let c = sc.exact[j as usize];
            for (&i, &w) in t.cell.iter().zip(&t.w) {
                sc.val[i as usize] += c.extend(1.0) * w;
            }
        }
        for _ in 0..self.blur_passes {
            blur3(&mut sc.val, &mut sc.tmp_val, self.dims);
        }
        sc.corr.clear();
        sc.corr.extend(
            self.control_cell
                .iter()
                .zip(&self.control_take)
                .map(|(t, &take)| {
                    let mut v = Vec4::ZERO;
                    for (&i, &x) in t.cell.iter().zip(&t.w) {
                        v += sc.val[i as usize] * x;
                    }
                    if v.w > 1e-6 {
                        Vec3A::from(v.truncate() * (take / v.w))
                    } else {
                        Vec3A::ZERO
                    }
                }),
        );
        // near the parents, toward the exact correction
        for ((&j, e), &keep) in self.direct.iter().zip(&sc.exact).zip(&self.keep) {
            let s = &mut sc.corr[j as usize];
            *s += (Vec3A::from(*e) - *s) * keep;
        }
        smooth_a(
            &self.control_adjacency,
            &mut sc.corr,
            &mut sc.tmp,
            self.control_passes,
        );
        for ((&v, from), ws) in self.region.iter().zip(&self.interp_from).zip(&self.interp_w) {
            let mut c = Vec3A::ZERO;
            for (&j, &w) in from.iter().zip(ws) {
                c += sc.corr[j as usize] * w;
            }
            mine[v as usize] += Vec3::from(c);
        }
        disp[k] = mine;
    }
}

/// `passes` of half-weight neighbour averaging of `x` over `adj` ([`Adjacency::smooth`] for
/// SIMD vectors; `tmp` is scratch).
fn smooth_a(adj: &Adjacency, x: &mut Vec<Vec3A>, tmp: &mut Vec<Vec3A>, passes: usize) {
    for _ in 0..passes {
        tmp.clear();
        tmp.extend(adj.offsets.windows(2).zip(x.iter()).map(|(o, &xv)| {
            let nb = &adj.neighbours[o[0] as usize..o[1] as usize];
            if nb.is_empty() {
                return xv;
            }
            let mut s = Vec3A::ZERO;
            for &u in nb {
                s += x[u as usize];
            }
            xv * 0.5 + s * (0.5 / nb.len() as f32)
        }));
        std::mem::swap(x, tmp);
    }
}

/// Modified Shepard weights of the `FOLLOW_K` nearest of `found` (sorted, distances in mm, one
/// more than `FOLLOW_K` if there are): `((R − d) / (R (d + ε)))²`, `R` the next neighbour's
/// distance, normalised. A point's weight reaches 0 as it leaves the nearest set, so the
/// interpolated field has no jumps where the set changes.
fn shepard(found: &[(f32, u32)]) -> ([u32; FOLLOW_K], [f32; FOLLOW_K]) {
    let r = found
        .get(FOLLOW_K)
        .or(found.last())
        .map_or(1.0, |x| x.0)
        .max(1e-3);
    let mut id = [0u32; FOLLOW_K];
    let mut ws = [0.0f32; FOLLOW_K];
    let mut sum = 0.0;
    for (j, &(d, i)) in found.iter().take(FOLLOW_K).enumerate() {
        id[j] = i;
        let x = (r - d).max(0.0) / (r * (d + SHEPARD_EPS_MM));
        ws[j] = x * x;
        sum += ws[j];
    }
    let m = found.len().min(FOLLOW_K);
    for j in m..FOLLOW_K {
        id[j] = id[0];
    }
    if sum > 0.0 {
        for w in &mut ws {
            *w /= sum;
        }
    } else {
        for w in &mut ws[..m] {
            *w = 1.0 / m.max(1) as f32;
        }
    }
    (id, ws)
}

/// The contact attachment of part `k` to the parent vertices `from`, fading over `fade_mm`.
fn contact_for(
    parts: &[Part],
    k: usize,
    from: &[(u32, u32)],
    fade_mm: f32,
    exact: f32,
    band: (f32, f32),
    control_mm: f32,
) -> Contact {
    let (parents, weights, nearest) = nearest_parents(parts, k, from);
    let rest = &parts[k].rest_pos;
    let n = rest.len();
    let mut c = Contact::default();
    let pts: Vec<Vec3> = (0..n)
        .filter(|&v| nearest[v] <= ATTACH_INNER_MM)
        .map(|v| rest[v])
        .collect();
    if pts.is_empty() {
        return c;
    }
    let grid = HashGrid::new(&pts, FOLLOW_GRID_MM);
    // the share of the extended correction: 1 on the contact, fading with the distance to it,
    // smoothed over the mesh
    let full = Adjacency::new(n, &parts[k].indices);
    let mut t: Vec<f32> = (0..n)
        .map(|v| {
            if nearest[v] <= ATTACH_INNER_MM {
                return 1.0;
            }
            let d0 = grid.nearest(rest[v]).map_or(f32::INFINITY, |x| x.0);
            1.0 - smoothstep(f64::from(d0 / fade_mm.max(1e-3))) as f32
        })
        .collect();
    full.smooth(&mut t, &mut Vec::new(), TAKE_SMOOTHING_PASSES);
    let keep =
        |d: f32| exact * (1.0 - smoothstep(f64::from((d - band.0).max(0.0) / (band.1 - band.0))) as f32);
    // the region: vertices with a share, and their neighbours
    let mut in_region = vec![false; n];
    for v in 0..n {
        if t[v] > 1e-4 || nearest[v] < band.1 {
            in_region[v] = true;
            for &u in full.of(v) {
                in_region[u as usize] = true;
            }
        }
    }
    c.region = (0..n as u32).filter(|&v| in_region[v as usize]).collect();
    // controls: per occupied cell of `control_mm`, the vertex nearest the cell's centroid (every
    // region vertex when it is 0)
    let mut cells: BTreeMap<(i64, i64, i64), (Vec3, u32, Vec<u32>)> = BTreeMap::new();
    let per_vertex = control_mm <= 0.0;
    for &v in &c.region {
        if per_vertex {
            cells.insert((i64::from(v), 0, 0), (rest[v as usize], 1, vec![v]));
            continue;
        }
        let q = (rest[v as usize] / control_mm).floor();
        let e = cells
            .entry((q.x as i64, q.y as i64, q.z as i64))
            .or_insert((Vec3::ZERO, 0, Vec::new()));
        e.0 += rest[v as usize];
        e.1 += 1;
        e.2.push(v);
    }
    let mut cluster = vec![u32::MAX; n];
    for (sum, count, members) in cells.values() {
        let centre = *sum / *count as f32;
        let best = members
            .iter()
            .copied()
            .min_by(|&a, &b| {
                rest[a as usize]
                    .distance_squared(centre)
                    .total_cmp(&rest[b as usize].distance_squared(centre))
                    .then(a.cmp(&b))
            })
            .unwrap_or(members[0]);
        for &m in members {
            cluster[m as usize] = c.control.len() as u32;
        }
        c.control.push(best);
    }
    // the controls' graph: clusters joined by a mesh edge
    let mut lists: Vec<Vec<u32>> = vec![Vec::new(); c.control.len()];
    for &v in &c.region {
        let a = cluster[v as usize];
        for &u in full.of(v as usize) {
            let b = cluster[u as usize];
            if b != u32::MAX && b != a {
                lists[a as usize].push(b);
            }
        }
    }
    let mut offsets = Vec::with_capacity(lists.len() + 1);
    let mut neighbours = Vec::new();
    offsets.push(0);
    for mut l in lists {
        l.sort_unstable();
        l.dedup();
        neighbours.extend(l);
        offsets.push(neighbours.len() as u32);
    }
    c.control_adjacency = Adjacency { offsets, neighbours };
    c.control_passes = if per_vertex {
        ATTACH_SMOOTHING_PASSES
    } else {
        CONTROL_SMOOTHING_PASSES
    };
    // the controls' shares and exact corrections
    for (j, &v) in c.control.iter().enumerate() {
        let v = v as usize;
        c.control_take.push(t[v].max(0.0));
        if nearest[v] < band.1 {
            if nearest[v] <= ATTACH_INNER_MM {
                c.touching.push(c.direct.len() as u32);
            }
            c.direct.push(j as u32);
            c.parents.push(parents[v]);
            c.weights.push(weights[v]);
            c.keep.push(keep(nearest[v]));
        }
    }
    // the grid over the region, a cell of margin, blurred far enough to reach the fade
    let h = CONTACT_GRID_MM.max(fade_mm / 5.0);
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for &v in &c.region {
        lo = lo.min(rest[v as usize]);
        hi = hi.max(rest[v as usize]);
    }
    lo -= Vec3::splat(h);
    hi += Vec3::splat(h);
    let size = ((hi - lo) / h).ceil();
    c.dims = [size.x as usize + 1, size.y as usize + 1, size.z as usize + 1];
    c.blur_passes = (fade_mm / h).ceil() as usize + 1;
    c.splat_cell = c
        .touching
        .iter()
        .map(|&j| {
            Trilinear::at(
                rest[c.control[c.direct[j as usize] as usize] as usize],
                lo,
                h,
                c.dims,
            )
        })
        .collect();
    c.control_cell = c
        .control
        .iter()
        .map(|&v| Trilinear::at(rest[v as usize], lo, h, c.dims))
        .collect();
    // every region vertex from its nearest controls (compact weights, zero at the next one)
    let cpos: Vec<Vec3> = c.control.iter().map(|&v| rest[v as usize]).collect();
    let cgrid = HashGrid::new(&cpos, FOLLOW_GRID_MM);
    let mut found = Vec::with_capacity(INTERP_K + 1);
    for &v in &c.region {
        if per_vertex {
            let mut from = [cluster[v as usize]; INTERP_K];
            from[0] = cluster[v as usize];
            let mut ws = [0.0f32; INTERP_K];
            ws[0] = 1.0;
            c.interp_from.push(from);
            c.interp_w.push(ws);
            continue;
        }
        cgrid.knn(rest[v as usize], INTERP_K + 1, None, &mut found);
        let r = found.last().map_or(1.0, |x| x.0).max(1e-3);
        let mut from = [0u32; INTERP_K];
        let mut ws = [0.0f32; INTERP_K];
        let mut sum = 0.0;
        for (j, &(d, i)) in found.iter().take(INTERP_K).enumerate() {
            let q = 1.0 - (d / r) * (d / r);
            from[j] = i;
            ws[j] = q * q;
            sum += ws[j];
        }
        for w in &mut ws {
            *w /= sum.max(f32::MIN_POSITIVE);
        }
        c.interp_from.push(from);
        c.interp_w.push(ws);
    }
    c
}

/// Fade distance of a part's attachment to its parents, mm (`None` for the LV, which every other
/// part rests on).
fn attach_fade_mm(p: &Part) -> Option<f32> {
    match (p.kind, p.name.as_str()) {
        (_, "lv") => None,
        (MeshKind::Valve, _) => Some(DISC_ATTACH_FADE_MM),
        (MeshKind::Vessel, _) => Some(VESSEL_ATTACH_FADE_MM),
        (MeshKind::Myocardium, "rv") => Some(RV_ATTACH_FADE_MM),
        _ => Some(ATRIUM_ATTACH_FADE_MM),
    }
}

/// The order parts are moved in: each rests on the parts before it. The LV first (it alone is
/// solved on its cavity), then the RV (no septum of its own: it lies on the LV), the valve discs
/// and the aortic and pulmonary roots (the fibrous skeleton between the ventricles and the
/// vessels it carries), the left then the right atrium (wrapped around the roots), other
/// myocardium, then the other vessels in pack order.
fn seniority(p: &Part) -> u8 {
    match (p.kind, p.name.as_str()) {
        (_, "lv") => 0,
        (MeshKind::Myocardium, "rv") => 1,
        (MeshKind::Valve, _) => 2,
        (MeshKind::Vessel, "aorta") => 3,
        (MeshKind::Vessel, "pulmonary_artery") => 4,
        (MeshKind::Myocardium, "la") => 5,
        (MeshKind::Myocardium, "ra") => 6,
        (MeshKind::Myocardium, _) => 7,
        _ => 8,
    }
}

/// Every vertex of the parts `from`, as `(part, vertex)`.
fn whole_parts(parts: &[Part], from: &[usize]) -> Vec<(u32, u32)> {
    from.iter()
        .flat_map(|&p| (0..parts[p].rest_pos.len() as u32).map(move |v| (p as u32, v)))
        .collect()
}

/// For every vertex of part `k`, its [`FOLLOW_K`] nearest vertices among the parent vertices
/// `ids` (inverse-distance weights, summing to 1) and the distance to the nearest.
#[allow(clippy::type_complexity)]
fn nearest_parents(
    parts: &[Part],
    k: usize,
    ids: &[(u32, u32)],
) -> (Vec<[(u32, u32); FOLLOW_K]>, Vec<[f32; FOLLOW_K]>, Vec<f32>) {
    let pos: Vec<Vec3> = ids
        .iter()
        .map(|&(p, v)| parts[p as usize].rest_pos[v as usize])
        .collect();
    let n = parts[k].rest_pos.len();
    let (mut parents, mut weights, mut nearest) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    if ids.is_empty() {
        parents.resize(n, [(k as u32, 0); FOLLOW_K]);
        weights.resize(n, [0.0; FOLLOW_K]);
        nearest.resize(n, f32::INFINITY);
        return (parents, weights, nearest);
    }
    let grid = HashGrid::new(&pos, FOLLOW_GRID_MM);
    let mut found = Vec::with_capacity(FOLLOW_K);
    for &p in &parts[k].rest_pos {
        grid.knn(p, FOLLOW_K + 1, None, &mut found);
        let (local, ws) = shepard(&found);
        let id = local.map(|i| ids[i as usize]);
        parents.push(id);
        weights.push(ws);
        nearest.push(found.first().map_or(f32::INFINITY, |x| x.0));
    }
    (parents, weights, nearest)
}

/// The attachment of part `k` to the parent vertices `from`, with `passes` of averaging; `take`
/// gives each vertex's share from its index and its distance to the nearest parent vertex.
fn attach_for(
    parts: &[Part],
    k: usize,
    from: &[(u32, u32)],
    take: impl Fn(usize, f32) -> f32,
    passes: usize,
) -> Attach {
    let (parents, weights, nearest) = nearest_parents(parts, k, from);
    let n = parts[k].rest_pos.len();
    let full = Adjacency::new(n, &parts[k].indices);
    let take: Vec<f32> = nearest.iter().enumerate().map(|(v, &d)| take(v, d)).collect();
    let mut in_region = vec![false; n];
    for v in 0..n {
        if take[v] > 0.0 {
            in_region[v] = true;
            for &u in full.of(v) {
                in_region[u as usize] = true;
            }
        }
    }
    let region: Vec<u32> = (0..n as u32).filter(|&v| in_region[v as usize]).collect();
    let mut local_of = vec![u32::MAX; n];
    for (l, &v) in region.iter().enumerate() {
        local_of[v as usize] = l as u32;
    }
    let outside = region.len() as u32;
    let mut offsets = Vec::with_capacity(region.len() + 2);
    let mut neighbours = Vec::new();
    offsets.push(0);
    for &v in &region {
        neighbours.extend(full.of(v as usize).iter().map(|&u| {
            let l = local_of[u as usize];
            if l == u32::MAX { outside } else { l }
        }));
        offsets.push(neighbours.len() as u32);
    }
    offsets.push(neighbours.len() as u32); // the outside slot: no neighbours, stays 0
    let mut a = Attach {
        adjacency: Adjacency { offsets, neighbours },
        passes,
        ..Attach::default()
    };
    for (v, ((p, w), t)) in parents.into_iter().zip(weights).zip(take).enumerate() {
        if t > 0.0 {
            a.local.push(local_of[v]);
            a.parents.push(p);
            a.weights.push(w);
            a.take.push(t);
        }
    }
    a.region = region;
    a
}

#[derive(Debug, Clone)]
enum Mapping {
    Hooded(Hooded),
    Discs(Vec<DiscFit>),
    /// A vessel: no motion of its own, it follows the parts before it (see [`Part::attach`]).
    Follow,
}

#[derive(Debug, Clone)]
struct Part {
    name: String,
    kind: MeshKind,
    rest: Vec<Vertex>,
    indices: Vec<u32>,
    /// `indices` as triangles.
    tris: Vec<[u32; 3]>,
    /// The triangles incident on each vertex (compressed rows).
    incident: Adjacency,
    /// Rest positions.
    rest_pos: Vec<Vec3>,
    mapping: Mapping,
    /// A vessel's own motion: it follows its root in full, fading to rest over
    /// [`MotionParams::vessel_fade_mm`] (the aorta the aortic disc, the pulmonary artery the
    /// pulmonic disc, the caval veins the right atrium, the pulmonary veins the left atrium).
    follow: Option<Attach>,
    /// How the part rests on the parts moved before it (`None` for the LV).
    attach: Option<Contact>,
}

/// The LV cavity: the endocardial faces of the LV mesh, closed by a cone over each opening.
#[derive(Debug, Clone)]
struct Cavity {
    /// Each opening: its boundary edges as they run in `tris`, and its distinct vertices.
    caps: Vec<(Vec<(u32, u32)>, Vec<u32>)>,
    /// Distinct LV-mesh vertices the cavity uses.
    verts: Vec<u32>,
    /// Position of each LV-mesh vertex in `verts` (u32::MAX if unused).
    slot: Vec<u32>,
    /// The LV mesh's endocardial triangles (oriented as in the mesh, normals into the cavity) and
    /// the openings, in slot indices (positions in `verts`).
    stris: Vec<[u32; 3]>,
    scaps: Vec<(Vec<(u32, u32)>, Vec<u32>)>,
    /// Reference point the volumes are summed about (rest centroid of `verts`).
    origin: DVec3,
    /// Rest position of every LV-mesh vertex relative to `origin` (f32; the cavity sums in f64).
    rest_local: Vec<Vec3>,
    rest_volume_mm3: f64,
}

/// Inputs of one evaluation.
#[derive(Debug, Clone, Copy)]
pub struct MotionInput<'a> {
    /// The record of beats and cell activations.
    pub history: &'a MotionHistory,
    /// Simulation time to draw, ms (the strip's time: engine time minus the display delay and the
    /// scrub).
    pub t_ms: f64,
    /// Respiratory phase at `t_ms` (`0..1`, `TimingEngine::respiratory_phase`).
    pub resp_phase: f64,
    /// Wall motion on; `false` gives the rest mesh bit for bit.
    pub enabled: bool,
    /// Multiplies the wall displacement for teaching (1 = the model); clamped to
    /// `0..=`[`MAX_EXAGGERATION`]. The read-outs are always the model's.
    pub exaggeration: f32,
}

/// The numbers the read-out card shows, all of the model at 1×.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MotionReadout {
    /// Cavity volume of the deformed LV mesh at the drawn time, mL.
    pub lv_volume_ml: f64,
    /// End-diastolic and end-systolic volume of the current beat, mL (the deformed mesh's cavity at
    /// those instants).
    pub lv_edv_ml: f64,
    pub lv_esv_ml: f64,
    /// Ejection fraction of the current beat, `0..1` (`None` before the first beat).
    pub lvef: Option<f64>,
    /// Mitral and tricuspid annular plane systolic excursion of the current beat, mm, each read
    /// between its own ventricle's end-diastole and end-systole (MAPSE at aortic, TAPSE at
    /// pulmonic closure; `None` before that end-systole), on the ventricles' own motion (the
    /// attachment of the RV to the LV is not applied for the read-outs).
    pub mapse_mm: Option<f64>,
    pub tapse_mm: Option<f64>,
    /// Phase of the left ventricle.
    pub lv_phase: Option<CardiacPhase>,
    /// Mean contraction state of each chamber (`MotionChamber::ALL` order), `0..1`.
    pub chamber_activity: [f32; 4],
}

/// One part's vertices this frame, in the pack mesh's vertex order and GPU layout.
#[derive(Debug, Clone)]
pub struct PartFrame {
    pub name: String,
    pub vertices: Vec<Vertex>,
}

/// The output buffers of [`MotionModel::evaluate`], allocated once by [`MotionModel::new_frame`]
/// and refilled every frame.
#[derive(Debug, Clone)]
pub struct MotionFrame {
    pub parts: Vec<PartFrame>,
    pub readout: MotionReadout,
}

impl MotionFrame {
    /// The part called `name`.
    #[must_use]
    pub fn part(&self, name: &str) -> Option<&PartFrame> {
        self.parts.iter().find(|p| p.name == name)
    }
}

/// Wall displacement of every part at one time (model, 1×), plus what the read-outs need.
#[derive(Debug, Clone, Default)]
struct WallState {
    disp: Vec<Vec<Vec3>>,
    lv: Option<VolumeState>,
    lv_volume_mm3: f64,
    activity: [f32; 4],
}

/// MAPSE / TAPSE / EDV / ESV of one beat, cached by its QRS time.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BeatMetrics {
    qrs_bits: u64,
    mapse_mm: Option<f64>,
    tapse_mm: Option<f64>,
    /// End-systole of the right ventricle (pulmonic closure), when TAPSE is read.
    rv_es_ms: Option<f64>,
    edv_mm3: f64,
    esv_mm3: f64,
}

/// The kinematic wall-motion model (see the crate docs).
#[derive(Debug, Clone)]
pub struct MotionModel {
    params: MotionParams,
    frame: CardiacFrame,
    coords: CellCoords,
    aha: Vec<u8>,
    /// The working cells (their own proxies), the only ones whose contraction is computed.
    working: Vec<u32>,
    /// Same-chamber neighbours of each cell (compressed rows).
    cell_adjacency: Adjacency,
    /// The voxel grid the contraction is coupled on.
    coupling: Coupling,
    c_tmp: Vec<f32>,
    injury: Vec<f32>,
    ef_scale: [f64; 2],
    parts: Vec<Part>,
    /// The parts in the order they are moved (see [`seniority`]); the LV first.
    order: Vec<usize>,
    lv_part: usize,
    discs_part: Option<usize>,
    cavity: Cavity,
    /// LV endocardial radial excursion at full contraction, fraction of the radius (solved).
    lv_radial_fraction: f32,
    /// Factor on the RV long-axis template (solved, see [`Self::rv_long_scale`]).
    rv_long_scale: f32,
    /// The tricuspid disc's annulus fit: the RV's amplitude is set at its lateral rim.
    rv_annulus: Option<DiscFit>,
    // --- scratch, reused every frame --------------------------------------------------------
    cav_a: Vec<DVec3>,
    cav_b: Vec<DVec3>,
    vertex_c: Vec<Vec<Vec3>>,
    vertex_tmp: Vec<Vec3>,
    attach_corr: Vec<Vec3>,
    contact_scratch: ContactScratch,
    state: WallState,
    metrics_state: WallState,
    metrics_scratch: WallState,
    normal_acc: Vec<Vec3A>,
    pos_scratch: Vec<Vec3A>,
    metrics: Option<BeatMetrics>,
}

fn v3(v: &Vertex) -> Vec3 {
    Vec3::from(v.position)
}

/// `cos θ − 1` and `sin θ` by their series (exact to f32 for |θ| ≤ 1; deterministic arithmetic).
fn rot_terms(t: f32) -> (f32, f32) {
    let t2 = t * t;
    let cm1 = -t2 / 2.0 * (1.0 - t2 / 12.0 * (1.0 - t2 / 30.0 * (1.0 - t2 / 56.0)));
    let s = t * (1.0 - t2 / 6.0 * (1.0 - t2 / 20.0 * (1.0 - t2 / 42.0)));
    (cm1, s)
}

fn owner_of(name: &str) -> Owner {
    match name {
        // the septum is LV tissue and moves with the LV; only the RV mesh blends the two
        "lv" => Owner::Chamber(MotionChamber::LeftVentricle),
        "rv" => Owner::Ventricles,
        "la" => Owner::Chamber(MotionChamber::LeftAtrium),
        "ra" => Owner::Chamber(MotionChamber::RightAtrium),
        _ => Owner::Any,
    }
}

/// Chamber weights `w` restricted to the chambers a mesh owned by `owner` may use (sum 1).
fn restrict_shares(w: [f32; 4], owner: Owner) -> [f32; 4] {
    match owner {
        Owner::Chamber(ch) => {
            let mut out = [0.0; 4];
            out[ch.index()] = 1.0;
            out
        }
        Owner::Ventricles => {
            let s = w[0] + w[1];
            if s > 0.0 {
                [w[0] / s, w[1] / s, 0.0, 0.0]
            } else {
                [1.0, 0.0, 0.0, 0.0]
            }
        }
        Owner::Any => {
            let s: f32 = w.iter().sum();
            if s > 0.0 {
                w.map(|x| x / s)
            } else {
                [1.0, 0.0, 0.0, 0.0]
            }
        }
    }
}

/// Transmural depth of chamber `chamber` at `p`: Gaussian-weighted ([`DEPTH_SIGMA_MM`]) over that
/// chamber's cells among the [`DEPTH_CELLS`] nearest, so that it varies smoothly in space where a
/// crumpled patch of the mesh brings far-apart vertices close (0.5 if none is near).
fn spatial_depth(
    p: Vec3,
    grid: &HashGrid<'_>,
    coords: &CellCoords,
    chamber: MotionChamber,
    found: &mut Vec<(f32, u32)>,
) -> f32 {
    grid.knn(p, DEPTH_CELLS, None, found);
    let (mut s, mut w) = (0.0f32, 0.0f32);
    for &(d, j) in found.iter() {
        if coords.chamber[j as usize] == chamber {
            let k = (-(d * d) / (2.0 * DEPTH_SIGMA_MM * DEPTH_SIGMA_MM)).exp_d() + 1e-12;
            s += k * coords.depth[j as usize];
            w += k;
        }
    }
    if w > 0.0 { s / w } else { 0.5 }
}

/// Signed-volume polynomial coefficients `(c0, c1, c2, c3)` of the cavity for positions
/// `a + β b` (mm³; positive for the cavity).
fn cavity_poly(cav: &Cavity, a: &[DVec3], b: &[DVec3]) -> [f64; 4] {
    let mut c = [0.0f64; 4];
    // det(a + βb …) expanded with six cross products per triangle
    let mut tri = |aa: DVec3, ba: DVec3, ab: DVec3, bb: DVec3, ac: DVec3, bc: DVec3| {
        let x = ab.cross(ac);
        let y = ac.cross(aa);
        let z = aa.cross(ab);
        let p = bb.cross(bc);
        let q = bc.cross(ba);
        let r = ba.cross(bb);
        c[0] += aa.dot(x);
        c[1] += ba.dot(x) + bb.dot(y) + bc.dot(z);
        c[2] += aa.dot(p) + ab.dot(q) + ac.dot(r);
        c[3] += ba.dot(p);
    };
    for t in &cav.stris {
        let [i, j, k] = [t[0] as usize, t[1] as usize, t[2] as usize];
        tri(a[i], b[i], a[j], b[j], a[k], b[k]);
    }
    for (edges, verts) in &cav.scaps {
        let n = verts.len().max(1) as f64;
        let (mut ca, mut cb) = (DVec3::ZERO, DVec3::ZERO);
        for &v in verts {
            ca += a[v as usize];
            cb += b[v as usize];
        }
        let (aa, ba) = (ca / n, cb / n);
        for &(p, q) in edges {
            // the cap runs each opening edge backwards
            let (p, q) = (p as usize, q as usize);
            tri(a[q], b[q], a[p], b[p], aa, ba);
        }
    }
    // the faces' normals point into the cavity: its volume is minus their signed sum
    c.map(|x| -x / 6.0)
}

fn eval_poly(c: &[f64; 4], x: f64) -> f64 {
    c[0] + x * (c[1] + x * (c[2] + x * c[3]))
}

/// The root of `poly(x) = target` in [`BETA_RANGE`] nearest `guess` (Newton, then bisection).
fn solve_cubic(c: &[f64; 4], target: f64, guess: f64) -> f64 {
    let f = |x: f64| eval_poly(c, x) - target;
    let df = |x: f64| c[1] + x * (2.0 * c[2] + 3.0 * x * c[3]);
    let mut x = guess.clamp(BETA_RANGE.0, BETA_RANGE.1);
    for _ in 0..12 {
        let d = df(x);
        if d.abs() < 1e-12 {
            break;
        }
        let step = f(x) / d;
        x = (x - step).clamp(BETA_RANGE.0, BETA_RANGE.1);
        if step.abs() < 1e-10 {
            return x;
        }
    }
    if f(x).abs() <= 1e-6 * target.abs().max(1.0) {
        return x;
    }
    // bisection on the decreasing branch
    let (mut lo, mut hi) = BETA_RANGE;
    if f(lo) < 0.0 || f(hi) > 0.0 {
        return x;
    }
    for _ in 0..80 {
        let mid = f64::midpoint(lo, hi);
        if f(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    f64::midpoint(lo, hi)
}

/// Extract the LV cavity from the LV mesh (see [`Cavity`]).
fn build_cavity(mesh: &TriMesh, axis: &ChamberAxis, depth: &[f32]) -> Option<Cavity> {
    fn find(parent: &mut BTreeMap<u32, u32>, x: u32) -> u32 {
        let mut r = x;
        while let Some(&p) = parent.get(&r) {
            if p == r {
                break;
            }
            r = p;
        }
        parent.insert(x, r);
        r
    }
    let n = mesh.vertices.len();
    let endo: Vec<bool> = mesh
        .vertices
        .iter()
        .zip(depth)
        .map(|(v, &rho)| {
            let p = v3(v);
            let h = axis.coords(p).height.clamp(0.25, 0.9);
            let d = Vec3::from(v.normal).dot((p - axis.point_at(h)).normalize_or_zero());
            if d < -0.25 {
                true
            } else if d > 0.25 {
                false
            } else {
                rho < 0.5
            }
        })
        .collect();
    let tris: Vec<[u32; 3]> = mesh
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|t| t.iter().all(|&v| endo[v as usize]))
        .copied()
        .collect();
    // largest edge-connected component
    let mut edge_faces: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    for (f, t) in tris.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            edge_faces.entry((a.min(b), a.max(b))).or_default().push(f);
        }
    }
    let mut comp = vec![usize::MAX; tris.len()];
    let mut sizes = Vec::new();
    for start in 0..tris.len() {
        if comp[start] != usize::MAX {
            continue;
        }
        let id = sizes.len();
        let mut stack = vec![start];
        comp[start] = id;
        let mut size = 0;
        while let Some(f) = stack.pop() {
            size += 1;
            let t = tris[f];
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                for &g in &edge_faces[&(a.min(b), a.max(b))] {
                    if comp[g] == usize::MAX {
                        comp[g] = id;
                        stack.push(g);
                    }
                }
            }
        }
        sizes.push(size);
    }
    let best = (0..sizes.len()).max_by_key(|&i| (sizes[i], usize::MAX - i))?;
    let tris: Vec<[u32; 3]> = tris
        .iter()
        .zip(&comp)
        .filter(|(_, c)| **c == best)
        .map(|(t, _)| *t)
        .collect();
    // boundary edges, as they run in the faces
    let mut count: BTreeMap<(u32, u32), (u32, (u32, u32))> = BTreeMap::new();
    for t in &tris {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let e = count.entry((a.min(b), a.max(b))).or_insert((0, (a, b)));
            e.0 += 1;
        }
    }
    let boundary: Vec<(u32, u32)> = count.values().filter(|(c, _)| *c == 1).map(|(_, e)| *e).collect();
    // group the boundary edges into openings
    let mut parent: BTreeMap<u32, u32> = BTreeMap::new();
    for &(a, b) in &boundary {
        parent.entry(a).or_insert(a);
        parent.entry(b).or_insert(b);
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent.insert(rb, ra);
        }
    }
    let mut groups: BTreeMap<u32, (Vec<(u32, u32)>, Vec<u32>)> = BTreeMap::new();
    for &(a, b) in &boundary {
        let r = find(&mut parent, a);
        groups.entry(r).or_default().0.push((a, b));
    }
    for (edges, verts) in groups.values_mut() {
        let mut vs: Vec<u32> = edges.iter().flat_map(|&(a, b)| [a, b]).collect();
        vs.sort_unstable();
        vs.dedup();
        *verts = vs;
    }
    let caps: Vec<(Vec<(u32, u32)>, Vec<u32>)> = groups.into_values().collect();
    let mut verts: Vec<u32> = tris.iter().flatten().copied().collect();
    verts.sort_unstable();
    verts.dedup();
    let mut slot = vec![u32::MAX; n];
    for (k, &v) in verts.iter().enumerate() {
        slot[v as usize] = k as u32;
    }
    let origin = verts
        .iter()
        .map(|&v| v3(&mesh.vertices[v as usize]).as_dvec3())
        .sum::<DVec3>()
        / verts.len().max(1) as f64;
    // rounded to f32 so that the per-frame f32 offsets and this origin agree exactly
    let origin = origin.as_vec3().as_dvec3();
    let stris = tris.iter().map(|t| t.map(|v| slot[v as usize])).collect();
    let scaps = caps
        .iter()
        .map(|(e, v)| {
            (
                e.iter()
                    .map(|&(p, q)| (slot[p as usize], slot[q as usize]))
                    .collect(),
                v.iter().map(|&x| slot[x as usize]).collect(),
            )
        })
        .collect();
    let origin_f = origin.as_vec3();
    let rest_local = mesh.vertices.iter().map(|v| v3(v) - origin_f).collect();
    let mut cav = Cavity {
        rest_local,
        caps,
        stris,
        scaps,
        verts,
        slot,
        origin,
        rest_volume_mm3: 0.0,
    };
    let a: Vec<DVec3> = cav
        .verts
        .iter()
        .map(|&v| v3(&mesh.vertices[v as usize]).as_dvec3() - origin)
        .collect();
    let b = vec![DVec3::ZERO; a.len()];
    cav.rest_volume_mm3 = cavity_poly(&cav, &a, &b)[0];
    (cav.rest_volume_mm3 > 0.0).then_some(cav)
}

/// Per-cell mechanical injury `0..1` of the eikonal engine's coronary lesions: the larger of each
/// lesion's injury `M` (Reimer–Jennings wavefront, `heartsim_sim::ischaemia::Lesion::injury` — not
/// its electrical expression) and 1 in the infarct core. Recompute when the lesions change.
#[must_use]
pub fn mechanical_injury(engine: &heartsim_sim::EikonalEngine, aha_segments: &[u8]) -> Vec<f32> {
    use heartsim_sim::ischaemia::{territory_weight, transmural_depth};
    use heartsim_sim::tier1::CellKind;
    let cloud = engine.cloud();
    let n = cloud.len();
    let mut out = vec![0.0f32; n];
    if aha_segments.len() != n {
        return out;
    }
    let kinds = engine.kinds();
    for lesion in engine.ischaemia().lesions() {
        let w = territory_weight(cloud, engine.graph(), kinds, aha_segments, lesion.branch);
        for i in 0..n {
            if kinds[i] != CellKind::Ventricular || w[i] <= 0.0 {
                continue;
            }
            let rho = transmural_depth(cloud.regions[i], cloud.depth[i]);
            out[i] = out[i].max(lesion.injury(w[i], rho));
        }
    }
    for (o, &scar) in out.iter_mut().zip(&engine.injury().scar) {
        if scar {
            *o = 1.0;
        }
    }
    out
}

impl MotionModel {
    /// Build the model on an anatomy pack.
    pub fn new(pack: &AnatomyPack, params: MotionParams) -> Result<Self, MotionError> {
        Self::from_parts(
            &pack.meshes,
            &pack.hoods,
            &pack.cells,
            &pack.landmarks,
            &pack.aha_segments,
            params,
        )
    }

    /// Build the model from the parts of a pack: render meshes, their hoods (computed here for a
    /// myocardium mesh that has none), the cell cloud, landmarks and AHA segments.
    pub fn from_parts(
        meshes: &[NamedMesh],
        hoods: &BTreeMap<String, Vec<Hood>>,
        cells: &CellCloud,
        landmarks: &Landmarks,
        aha_segments: &[u8],
        params: MotionParams,
    ) -> Result<Self, MotionError> {
        let discs = meshes
            .iter()
            .find(|m| m.kind == MeshKind::Valve)
            .and_then(|m| split_valve_discs(&m.mesh))
            .ok_or(FrameError::NoValveDiscs)?;
        let proxies = working_proxies(cells);
        let chambers = cell_chambers(cells, &proxies);
        let frame = cardiac_frame(cells, &chambers, landmarks, &discs)?;
        let coords_chamber_tmp = chambers.clone();
        let working: Vec<u32> = (0..cells.len() as u32)
            .filter(|&i| proxies[i as usize] == i)
            .collect();
        let mut coupling = Coupling::new(cells, &chambers, &working, params.coupling_voxel_mm as f32);

        let coords = CellCoords::new(cells, chambers, &frame);
        let lv_part = meshes
            .iter()
            .position(|m| m.name == "lv" && m.kind == MeshKind::Myocardium)
            .ok_or(MotionError::NoLeftVentricle)?;

        // hooded parts first: templates at LV radial fraction 0 and 1, smoothed over the mesh
        let mut parts: Vec<Part> = Vec::with_capacity(meshes.len());
        let mut lv_depth = Vec::new();
        let mut radial_pairs: Vec<(Vec<VertexTemplate>, Vec<VertexTemplate>)> = Vec::new();
        // chamber membership of every cell, diffused over the neighbour graph
        let mut cell_shares: Vec<S5> = coords
            .chamber
            .iter()
            .map(|ch| {
                let mut x = [0.0; 6];
                x[ch.index()] = 1.0;
                S5(x)
            })
            .collect();
        let full_graph = cell_graph(cells);
        let cell_grid = HashGrid::new(&cells.positions, FOLLOW_GRID_MM);

        let mut found = Vec::with_capacity(DEPTH_CELLS);
        full_graph.smooth(&mut cell_shares, &mut Vec::new(), SHARE_DIFFUSION_PASSES);
        for (k, m) in meshes.iter().enumerate() {
            let mapping = if m.kind == MeshKind::Myocardium {
                let h = match hoods.get(&m.name) {
                    Some(h) if h.len() == m.mesh.vertex_count() => h.clone(),
                    _ => compute_hoods(&m.mesh, cells),
                };
                let owner = owner_of(&m.name);
                let adjacency = Adjacency::new(m.mesh.vertex_count(), &m.mesh.indices);
                // the hood-derived scalars (depth, chamber shares) smoothed over the mesh; the
                // templates are then exact functions of each vertex's own position
                let mut sc: Vec<S5> = h
                    .iter()
                    .zip(&m.mesh.vertices)
                    .map(|(hood, v)| {
                        // each ventricle's own depth convention (the septum is measured from the
                        // LV side, the RV wall from the RV cavity): blending them would jump
                        let rho_lv = spatial_depth(
                            v3(v),
                            &cell_grid,
                            &coords,
                            MotionChamber::LeftVentricle,
                            &mut found,
                        );
                        let rho_rv = spatial_depth(
                            v3(v),
                            &cell_grid,
                            &coords,
                            MotionChamber::RightVentricle,
                            &mut found,
                        );
                        let mut sh = [0.0f32; 4];
                        for (c, w) in hood.cells.iter().zip(hood.weights) {
                            for (k, x) in sh.iter_mut().enumerate() {
                                *x += w * cell_shares[*c as usize].0[k];
                            }
                        }
                        let sh = restrict_shares(sh, owner);
                        S5([rho_lv, rho_rv, sh[0], sh[1], sh[2], sh[3]])
                    })
                    .collect();
                adjacency.smooth(&mut sc, &mut Vec::new(), TEMPLATE_SMOOTHING_PASSES);
                let mut t0 = Vec::with_capacity(h.len());
                let mut t1 = Vec::with_capacity(h.len());
                for (v, S5(x)) in m.mesh.vertices.iter().zip(&sc) {
                    let p = v3(v);
                    let shares = [x[2], x[3], x[4], x[5]];
                    t0.push(vertex_template(p, [x[0], x[1]], &shares, &frame, &params, 0.0));
                    t1.push(vertex_template(p, [x[0], x[1]], &shares, &frame, &params, 1.0));
                    if k == lv_part {
                        lv_depth.push(x[0]);
                    }
                }
                radial_pairs.push((t0.clone(), t1));
                let chamber_hoods = m
                    .mesh
                    .vertices
                    .iter()
                    .flat_map(|v| {
                        owner
                            .slots()
                            .into_iter()
                            .flatten()
                            .map(|ch| coupling.sample_at(v3(v), ch))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                Mapping::Hooded(Hooded {
                    hoods: h,
                    ctrl: chamber_hoods,
                    owner,
                    kinesis: vec![1.0; m.mesh.vertex_count()],
                    vent: Vec::new(),
                    atr: Vec::new(),
                    templates: t0,
                    adjacency,
                })
            } else {
                radial_pairs.push((Vec::new(), Vec::new()));
                Mapping::Follow
            };
            parts.push(Part {
                name: m.name.clone(),
                kind: m.kind,
                rest: m.mesh.vertices.clone(),
                indices: m.mesh.indices.clone(),
                tris: m.mesh.indices.as_chunks::<3>().0.to_vec(),
                incident: incident_faces(m.mesh.vertex_count(), &m.mesh.indices),
                rest_pos: m.mesh.positions(),
                mapping,
                follow: None,
                attach: None,
            });
        }
        {
            let mut mark = vec![false; coupling.dense.len()];
            for p in &parts {
                if let Mapping::Hooded(h) = &p.mapping {
                    for smp in &h.ctrl {
                        coupling.mark_used(smp, &mut mark);
                    }
                }
            }
            coupling.used = (0..mark.len() as u32).filter(|&i| mark[i as usize]).collect();
        }
        let cavity = build_cavity(
            &meshes[lv_part].mesh,
            frame.axis(MotionChamber::LeftVentricle),
            &lv_depth,
        )
        .ok_or(MotionError::NoCavity)?;

        // solve the LV radial fraction so that the steady beat ejects `lvef` at amplitude 1
        let lv_radial_fraction = {
            let (t0, t1) = &radial_pairs[lv_part];
            let rest = &parts[lv_part].rest;
            let mut a = Vec::with_capacity(cavity.verts.len());
            let mut b = Vec::with_capacity(cavity.verts.len());
            for &v in &cavity.verts {
                let (z, o) = (&t0[v as usize], &t1[v as usize]);
                let p = v3(&rest[v as usize]);
                let (cm1, s) = rot_terms(z.rot_angle);
                let full = z.u_lv + z.u_rv + z.rot_r * cm1 + z.rot_b * s;
                a.push((p + full).as_dvec3() - cavity.origin);
                b.push((o.u_lv - z.u_lv).as_dvec3());
            }
            let c = cavity_poly(&cavity, &a, &b);
            solve_cubic(&c, cavity.rest_volume_mm3 * (1.0 - params.lvef), 0.3) as f32
        };
        for (p, (t0, t1)) in parts.iter_mut().zip(&radial_pairs) {
            let Mapping::Hooded(h) = &mut p.mapping else {
                continue;
            };
            for ((t, z), o) in h.templates.iter_mut().zip(t0).zip(t1) {
                t.u_lv_rad = (o.u_lv - z.u_lv) * lv_radial_fraction;
                t.u_lv = z.u_lv + t.u_lv_rad;
            }
            refresh_effective(h);
        }

        // valve discs on their annulus anchors
        let rv_part = meshes
            .iter()
            .position(|m| m.name == "rv" && m.kind == MeshKind::Myocardium);
        let discs_part = meshes.iter().position(|m| m.kind == MeshKind::Valve);
        if let Some(dp) = discs_part {
            let fits: Vec<DiscFit> = discs
                .iter()
                .map(|d| {
                    let anchor_part = match d.valve {
                        Valve::Mitral | Valve::Aortic => lv_part,
                        Valve::Tricuspid | Valve::Pulmonic => rv_part.unwrap_or(lv_part),
                    };
                    disc_fit(d, &parts[dp].rest, anchor_part, &parts[anchor_part].rest, &frame)
                })
                .collect();
            parts[dp].mapping = Mapping::Discs(fits);
        }
        // the RV long-axis template scaled so that the lateral tricuspid annulus descends by
        // `tapse_mm` between the RV's end-diastole and its full ejection (amplitude 0 to 1)
        let rv_long_scale = discs_part
            .and_then(|dp| rv_long_scale(&parts, dp, &frame, params.tapse_mm as f32))
            .unwrap_or(1.0);
        let rv_annulus: Option<DiscFit> = discs_part
            .and_then(|dp| match &parts[dp].mapping {
                Mapping::Discs(fits) => fits.iter().find(|f| f.disc.valve == Valve::Tricuspid),
                _ => None,
            })
            .filter(|f| matches!(parts[f.anchor_part].mapping, Mapping::Hooded(_)))
            .cloned();
        for p in &mut parts {
            let Mapping::Hooded(h) = &mut p.mapping else {
                continue;
            };
            for t in &mut h.templates {
                t.u_rv += t.u_rv_long * (rv_long_scale - 1.0);
            }
            refresh_effective(h);
        }

        // every part rests on the parts moved before it; vessels follow them
        let mut order: Vec<usize> = (0..parts.len()).collect();
        order.sort_by_key(|&k| (seniority(&parts[k]), k));
        for (i, &k) in order.iter().enumerate() {
            let before = &order[..i];
            let solid: Vec<usize> = before
                .iter()
                .copied()
                .filter(|&p| !matches!(parts[p].mapping, Mapping::Follow))
                .collect();
            if matches!(parts[k].mapping, Mapping::Follow) {
                // a vessel follows its root, fading to rest
                let (from, root) = vessel_root(&parts, k, discs_part, &solid);
                let fade = params.vessel_fade_mm as f32;
                parts[k].follow = Some(attach_for(
                    &parts,
                    k,
                    &from,
                    |v, _| attach_take(root[v], fade),
                    FOLLOWER_SMOOTHING_PASSES,
                ));
            }
            // and rests on what it touches
            if let Some(fade) = attach_fade_mm(&parts[k]) {
                let from = whole_parts(&parts, before);
                let exact = if matches!(parts[k].name.as_str(), "la" | "ra")
                    || matches!(parts[k].mapping, Mapping::Follow)
                {
                    THIN_EXACT_SHARE
                } else {
                    1.0
                };
                let band = match (parts[k].kind, parts[k].name.as_str()) {
                    (MeshKind::Valve, _) => DISC_EXACT_BAND_MM,
                    (MeshKind::Myocardium, "rv") => (ATTACH_EXACT_MM, ATTACH_DIRECT_MM),
                    _ => THIN_EXACT_BAND_MM,
                };
                // the discs and the vessels are small and sit between several parents: every
                // vertex is a control
                let control_mm = match parts[k].kind {
                    MeshKind::Myocardium => CONTROL_MM,
                    MeshKind::Valve | MeshKind::Vessel => 0.0,
                };
                let c = contact_for(&parts, k, &from, fade, exact, band, control_mm);
                parts[k].attach = Some(c);
            }
        }

        let n = cells.len();
        let mut model = Self {
            params,
            frame,
            coords,
            aha: aha_segments.to_vec(),
            cell_adjacency: cell_adjacency(cells, &coords_chamber_tmp),
            coupling,
            c_tmp: Vec::new(),
            working,
            injury: vec![0.0; n],
            ef_scale: [1.0, 1.0],
            order,
            lv_part,
            discs_part,
            cavity,
            lv_radial_fraction,
            rv_long_scale,
            rv_annulus,
            cav_a: Vec::new(),
            cav_b: Vec::new(),
            vertex_c: Vec::new(),
            vertex_tmp: Vec::new(),
            attach_corr: Vec::new(),
            contact_scratch: ContactScratch::default(),
            state: WallState::default(),
            metrics_state: WallState::default(),
            metrics_scratch: WallState::default(),
            normal_acc: Vec::new(),
            pos_scratch: Vec::new(),
            metrics: None,
            parts,
        };
        model.state.disp = model
            .parts
            .iter()
            .map(|p| vec![Vec3::ZERO; p.rest.len()])
            .collect();
        model.metrics_state.disp = model.state.disp.clone();
        Ok(model)
    }

    /// The parameters.
    #[must_use]
    pub fn params(&self) -> &MotionParams {
        &self.params
    }

    /// The cardiac frame (axes, valve centres).
    #[must_use]
    pub fn frame(&self) -> &CardiacFrame {
        &self.frame
    }

    /// Per-cell cardiac coordinates.
    #[must_use]
    pub fn coords(&self) -> &CellCoords {
        &self.coords
    }

    /// The LV endocardial radial excursion at full contraction, fraction of the distance to the
    /// axis, solved at construction so that the steady beat ejects [`MotionParams::lvef`].
    #[must_use]
    pub fn lv_radial_fraction(&self) -> f32 {
        self.lv_radial_fraction
    }

    /// The factor on the RV long-axis template, solved at construction so that the lateral
    /// tricuspid annulus descends by [`MotionParams::tapse_mm`] over the RV's full ejection.
    #[must_use]
    pub fn rv_long_scale(&self) -> f32 {
        self.rv_long_scale
    }

    /// LV cavity volume of the rest mesh (the end-diastolic CT), mL.
    #[must_use]
    pub fn rest_lv_volume_ml(&self) -> f64 {
        self.cavity.rest_volume_mm3 / 1000.0
    }

    /// The cavity volume of an LV mesh with these vertices (the `lv` part of a [`MotionFrame`], or
    /// any deformation of the rest mesh), mL: its endocardial faces closed over the mitral and
    /// aortic openings, as the model measures it.
    #[must_use]
    pub fn lv_cavity_volume_ml(&self, lv_vertices: &[Vertex]) -> f64 {
        if lv_vertices.len() != self.parts[self.lv_part].rest.len() {
            return 0.0;
        }
        let a: Vec<DVec3> = self
            .cavity
            .verts
            .iter()
            .map(|&v| v3(&lv_vertices[v as usize]).as_dvec3() - self.cavity.origin)
            .collect();
        let b = vec![DVec3::ZERO; a.len()];
        cavity_poly(&self.cavity, &a, &b)[0] / 1000.0
    }

    /// Number of openings the LV cavity is closed over (the mitral and aortic orifices, plus any
    /// small hole of the endocardial selection).
    #[must_use]
    pub fn lv_cavity_openings(&self) -> usize {
        self.cavity.caps.len()
    }

    /// The kind of part `name`.
    #[must_use]
    pub fn part_kind(&self, name: &str) -> Option<MeshKind> {
        self.parts.iter().find(|p| p.name == name).map(|p| p.kind)
    }

    /// Names of the parts, in the pack's mesh order (the order of [`MotionFrame::parts`]).
    pub fn part_names(&self) -> impl Iterator<Item = &str> {
        self.parts.iter().map(|p| p.name.as_str())
    }

    /// The four valve discs of the `Valve` mesh, each with its vertex indices in that mesh in
    /// ascending order (the order `heartsim_app::heart_view::split_valves` gives each disc).
    #[must_use]
    pub fn valve_discs(&self) -> Vec<&ValveDisc> {
        self.discs_part
            .and_then(|k| match &self.parts[k].mapping {
                Mapping::Discs(d) => Some(d.iter().map(|f| &f.disc).collect()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// The triangles (vertex indices) of part `name`.
    #[must_use]
    pub fn part_indices(&self, name: &str) -> Option<&[u32]> {
        self.parts
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.indices.as_slice())
    }

    /// The rest vertices of part `name`.
    #[must_use]
    pub fn part_rest(&self, name: &str) -> Option<&[Vertex]> {
        self.parts
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.rest.as_slice())
    }

    /// AHA segment (1–17, 0 outside the LV model) of each vertex of part `name`: the segment of
    /// the cell with the largest hood weight. Empty for parts without hoods.
    #[must_use]
    pub fn vertex_segments(&self, name: &str) -> Vec<u8> {
        let Some(p) = self.parts.iter().find(|p| p.name == name) else {
            return Vec::new();
        };
        let Mapping::Hooded(h) = &p.mapping else {
            return Vec::new();
        };
        h.hoods
            .iter()
            .map(|hood| {
                let k = (0..K_NEIGHBOURS)
                    .max_by(|&a, &b| hood.weights[a].total_cmp(&hood.weights[b]).then(b.cmp(&a)))
                    .unwrap_or(0);
                self.aha.get(hood.cells[k] as usize).copied().unwrap_or(0)
            })
            .collect()
    }

    /// Whether each vertex of the LV mesh belongs to the cavity (endocardial) surface.
    #[must_use]
    pub fn lv_endocardial_vertices(&self) -> Vec<bool> {
        (0..self.parts[self.lv_part].rest.len())
            .map(|v| self.cavity.slot[v] != u32::MAX)
            .collect()
    }

    /// Set the per-cell mechanical injury (`0..1`, see [`mechanical_injury`]): the wall over an
    /// injured LV cell keeps `1 − injury` of its inward motion in every phase (it still translates
    /// along the long axis and twists with its neighbours), and the LV ejection fraction falls by
    /// the cavity volume its injured wall no longer moves, so the remote wall moves as before. The
    /// right ventricle (the LAD's share of its free wall) keeps its own motion: its inward
    /// direction is not normal to its wall, and scaling it folded the mesh; where it lies on the
    /// LV it moves with the injured LV wall. An empty or mis-sized slice clears it.
    pub fn set_injury(&mut self, injury: &[f32]) {
        if injury.len() == self.injury.len() {
            self.injury.copy_from_slice(injury);
        } else {
            self.injury.fill(0.0);
        }
        self.metrics = None;
        // the wall is tethered: an akinetic territory drags its border, so the injury is spread
        // over the neighbour graph (about a centimetre) before it reaches the vertices
        let mut spread = self.injury.clone();
        let mut ctmp = Vec::new();
        self.cell_adjacency
            .smooth(&mut spread, &mut ctmp, INJURY_SPREAD_PASSES);
        let mut tmp = Vec::new();
        for p in &mut self.parts {
            let Mapping::Hooded(h) = &mut p.mapping else {
                continue;
            };
            h.kinesis = h
                .hoods
                .iter()
                .map(|hood| 1.0 - hood.interpolate(&spread))
                .collect();
            h.adjacency
                .smooth(&mut h.kinesis, &mut tmp, KINESIS_SMOOTHING_PASSES);
            refresh_effective(h);
        }
        // LV: the end-systolic cavity of the steady beat with each vertex at its kinesis
        let lv = &self.parts[self.lv_part];
        let Mapping::Hooded(h) = &lv.mapping else { return };
        let mut a = Vec::with_capacity(self.cavity.verts.len());
        let mut b = Vec::with_capacity(self.cavity.verts.len());
        for &v in &self.cavity.verts {
            let t = &h.templates[v as usize];
            let g = h.kinesis[v as usize];
            let p = v3(&lv.rest[v as usize]);
            let (cm1, s) = rot_terms(t.rot_angle);
            let d = t.u_lv - t.u_lv_rad * (1.0 - g) + t.u_rv + t.rot_r * cm1 + t.rot_b * s;
            a.push((p + d).as_dvec3() - self.cavity.origin);
            b.push(DVec3::ZERO);
        }
        let v_inj = cavity_poly(&self.cavity, &a, &b)[0];
        let full = self.cavity.rest_volume_mm3 * self.params.lvef;
        let lv_scale = ((self.cavity.rest_volume_mm3 - v_inj) / full).clamp(0.0, 1.0);
        // RV: its wall keeps its motion (see `wall_state`), so its ejection fraction too
        let rv_scale = 1.0;
        self.ef_scale = [lv_scale, rv_scale];
    }

    /// Ejection-fraction factors `[left, right]` of the current injury (1 healthy).
    #[must_use]
    pub fn ef_scale(&self) -> [f64; 2] {
        self.ef_scale
    }

    /// Output buffers for [`Self::evaluate`] (the rest mesh).
    #[must_use]
    pub fn new_frame(&self) -> MotionFrame {
        MotionFrame {
            parts: self
                .parts
                .iter()
                .map(|p| PartFrame {
                    name: p.name.clone(),
                    vertices: p.rest.clone(),
                })
                .collect(),
            readout: MotionReadout::default(),
        }
    }

    /// Fill `out` with every part's vertices (positions and recomputed normals) at
    /// `input.t_ms`, and the read-outs. A pure function of the inputs: no state carries from one
    /// call to the next except a cache of the current beat's MAPSE/TAPSE, which holds the same
    /// values whenever it is computed. With `enabled = false` the vertices are the rest mesh's,
    /// bit for bit.
    pub fn evaluate(&mut self, input: &MotionInput<'_>, out: &mut MotionFrame) {
        if out.parts.len() != self.parts.len() {
            *out = self.new_frame();
        }
        if !input.enabled {
            for (o, p) in out.parts.iter_mut().zip(&self.parts) {
                o.vertices.copy_from_slice(&p.rest);
            }
            out.readout = MotionReadout::default();
            return;
        }
        let mut state = std::mem::take(&mut self.state);
        self.wall_state(input.history, input.t_ms, &mut state, true);
        let e = input.exaggeration.clamp(0.0, MAX_EXAGGERATION);
        let shift = Vec3::new(
            0.0,
            -(self.params.respiratory_shift_mm * heartsim_sim::timing::inspiration_depth(input.resp_phase))
                as f32,
            0.0,
        );
        for (k, (o, p)) in out.parts.iter_mut().zip(&self.parts).enumerate() {
            let pos = &mut self.pos_scratch;
            pos.clear();
            pos.extend(
                p.rest_pos
                    .iter()
                    .zip(&state.disp[k])
                    .map(|(rv, d)| Vec3A::from(*rv + *d * e + shift)),
            );
            normals_csr(&mut o.vertices, pos, &p.tris, &p.incident, &mut self.normal_acc);
        }
        let r = self.readout_of(input, &state);
        out.readout = r;
        self.state = state;
    }

    /// The read-outs of a wall state at `input.t_ms`.
    fn readout_of(&mut self, input: &MotionInput<'_>, state: &WallState) -> MotionReadout {
        let mut r = MotionReadout {
            lv_volume_ml: state.lv_volume_mm3 / 1000.0,
            chamber_activity: state.activity,
            ..MotionReadout::default()
        };
        if let Some(lv) = state.lv {
            r.lv_phase = Some(lv.phase);
            if let Some(cycle) = lv.cycle {
                let m = self.beat_metrics(input.history, &cycle, &lv, input.t_ms);
                r.lv_edv_ml = m.edv_mm3 / 1000.0;
                r.lv_esv_ml = m.esv_mm3 / 1000.0;
                r.lvef = Some(if m.edv_mm3 > 0.0 {
                    1.0 - m.esv_mm3 / m.edv_mm3
                } else {
                    0.0
                });
                r.mapse_mm = m.mapse_mm;
                r.tapse_mm = m.tapse_mm;
            }
        }
        r
    }

    /// The read-outs alone at `input.t_ms` (the card with wall motion switched off): both
    /// ventricles and the LV volume are evaluated, no vertex is written. Equal to the read-out of
    /// [`Self::evaluate`] with the motion on.
    pub fn readout(&mut self, input: &MotionInput<'_>) -> MotionReadout {
        let mut state = std::mem::take(&mut self.metrics_scratch);
        self.wall_state(input.history, input.t_ms, &mut state, false);
        let r = self.readout_of(input, &state);
        self.metrics_scratch = state;
        r
    }

    /// MAPSE, TAPSE, EDV and ESV of the beat whose cycle is `cycle`, from the deformed mesh at its
    /// end-diastole (the inflow valve's closure) and end-systole (the outflow valve's closure);
    /// cached per beat. MAPSE/TAPSE are `None` until end-systole has passed.
    fn beat_metrics(
        &mut self,
        history: &MotionHistory,
        cycle: &crate::phases::CycleTimes,
        lv: &VolumeState,
        t_ms: f64,
    ) -> BeatMetrics {
        let key = cycle.qrs_ms.to_bits();
        let es_ms = cycle.ejection_ms.map(|(_, close)| close);
        let es_passed = es_ms.is_some_and(|es| t_ms >= es);
        if let Some(m) = self.metrics
            && m.qrs_bits == key
            && (m.mapse_mm.is_some() || !es_passed)
            && (m.tapse_mm.is_some() || !m.rv_es_ms.is_some_and(|es| t_ms >= es))
        {
            return m;
        }
        // the right ventricle's own cycle of this beat (tricuspid closure, pulmonic closure):
        // TAPSE is read at its end-systole, which moves with the respiratory splitting of S2
        let rv_cycle = {
            let inp = VolumeInputs {
                beats: history.beats(),
                atrial_onsets_ms: history.atrial_onsets_ms(),
                params: &self.params,
                pcg: history.pcg_params(),
                ef_scale: self.ef_scale,
            };
            volume_at(es_ms.unwrap_or(cycle.closure_ms), Side::Right, &inp)
                .cycle
                .filter(|c| c.qrs_ms.to_bits() == key)
        };
        let rv_es_ms = rv_cycle.and_then(|c| c.ejection_ms.map(|(_, close)| close));
        let rest = self.cavity.rest_volume_mm3;
        let mut m = BeatMetrics {
            qrs_bits: key,
            mapse_mm: None,
            tapse_mm: None,
            rv_es_ms,
            edv_mm3: lv.edv * rest,
            esv_mm3: lv.esv * rest,
        };
        let mut st = std::mem::take(&mut self.metrics_state);
        let lv_axis = self.frame.axis(MotionChamber::LeftVentricle).axis;
        let rv_axis = self.frame.axis(MotionChamber::RightVentricle).axis;
        self.wall_state(history, cycle.closure_ms, &mut st, false);
        m.edv_mm3 = st.lv_volume_mm3;
        let mitral_ed = self.annulus_points(&st)[0];
        if let Some(es) = es_ms {
            self.wall_state(history, es, &mut st, false);
            m.esv_mm3 = st.lv_volume_mm3;
            if es_passed {
                let mitral_es = self.annulus_points(&st)[0];
                m.mapse_mm = Some(f64::from((mitral_ed - mitral_es).dot(lv_axis)));
            }
        } else {
            m.esv_mm3 = m.edv_mm3;
        }
        if let (Some(rc), Some(es)) = (rv_cycle, rv_es_ms)
            && t_ms >= es
        {
            self.wall_state(history, rc.closure_ms, &mut st, false);
            let ed = self.annulus_points(&st)[1];
            self.wall_state(history, es, &mut st, false);
            let es_pt = self.annulus_points(&st)[1];
            m.tapse_mm = Some(f64::from((ed - es_pt).dot(rv_axis)));
        }
        self.metrics_state = st;
        self.metrics = Some(m);
        m
    }

    /// Displacement of the mitral annulus (the disc's centre: the mean of its septal and lateral
    /// rim) and of the lateral tricuspid annulus, mm.
    fn annulus_points(&self, st: &WallState) -> [Vec3; 2] {
        let Some(dp) = self.discs_part else {
            return [Vec3::ZERO; 2];
        };
        let Mapping::Discs(fits) = &self.parts[dp].mapping else {
            return [Vec3::ZERO; 2];
        };
        let mut out = [Vec3::ZERO; 2];
        for f in fits {
            let (t, l) = disc_affine(f, &st.disp[f.anchor_part]);
            match f.disc.valve {
                Valve::Mitral => out[0] = t + l * (f.disc.centre_mm - f.anchor_centre),
                Valve::Tricuspid => out[1] = t + l * (f.lateral_rim_mm - f.anchor_centre),
                _ => {}
            }
        }
        out
    }

    /// Per-cell effective contraction at `t_ms`, restricted to the coupling voxels and coupled
    /// into the dense lattice; returns the mean per chamber.
    fn cell_contraction(&mut self, history: &MotionHistory, t_ms: f64) -> [f32; 4] {
        let p = &self.params;
        let beats = history.beats();
        let k = beats.partition_point(|(b, _)| b.qrs_onset.0 <= t_ms);
        let ventricles_organised = k == 0 || beats[k - 1].0.organised;
        let v_gate = if ventricles_organised {
            1.0
        } else {
            p.vf_quiver_fraction
        } as f32;
        let a_gate = if history.atria_disorganised_at(t_ms) {
            p.atrial_quiver_fraction
        } else {
            1.0
        } as f32;
        let cells = history.cells();
        let mut sum = [0.0f32; 4];
        let mut count = [0usize; 4];
        for &wi in &self.working {
            let i = wi as usize;
            let ch = self.coords.chamber[i];
            let (emd, gate) = if ch.is_ventricle() {
                (p.emd_ventricle_ms, v_gate)
            } else {
                (p.emd_atrium_ms, a_gate)
            };
            let mut c = 0.0f64;
            if i < cells.len() {
                // the newest activation at or before t and the one before it (which can still be
                // relaxing at very short cycles); older ones have ended (NaN never compares ≤)
                let rec = cells.record_of(i);
                if let Some(k) = rec.iter().position(|&(a, _)| f64::from(a) <= t_ms) {
                    for &(act, apd) in &rec[k..rec.len().min(k + 2)] {
                        if f64::from(act) <= t_ms {
                            c = c.max(contraction(t_ms - f64::from(act), emd, f64::from(apd), p));
                        }
                    }
                }
            }
            let v = c as f32 * gate;
            self.coupling.sum[self.coupling.own[i] as usize] += v;
            sum[ch.index()] += v;
            count[ch.index()] += 1;
        }
        self.coupling
            .couple_field(&mut self.c_tmp, self.params.coupling_passes);
        self.coupling.refresh_dense();
        let mut mean = [0.0f32; 4];
        for k in 0..4 {
            if count[k] > 0 {
                mean[k] = sum[k] / count[k] as f32;
            }
        }
        mean
    }

    /// The wall displacement of every part at `t_ms` (model, 1×, no respiration).
    /// `full = false` computes only what the read-outs need: both ventricles and the LV volume
    /// (no atria, valve discs or vessels).
    fn wall_state(&mut self, history: &MotionHistory, t_ms: f64, st: &mut WallState, full: bool) {
        if st.disp.len() != self.parts.len() {
            st.disp = self
                .parts
                .iter()
                .map(|p| vec![Vec3::ZERO; p.rest.len()])
                .collect();
        }
        let activity = self.cell_contraction(history, t_ms);
        st.activity = activity;
        let inp = VolumeInputs {
            beats: history.beats(),
            atrial_onsets_ms: history.atrial_onsets_ms(),
            params: &self.params,
            pcg: history.pcg_params(),
            ef_scale: self.ef_scale,
        };
        let lv = volume_at(t_ms, Side::Left, &inp);
        let rv = volume_at(t_ms, Side::Right, &inp);
        st.lv = Some(lv);
        // right ventricle: its mean amplitude follows its volume curve
        let s_rv = (1.0 - rv.volume) / self.params.rvef.max(1e-6);

        // ventricular terms of every hooded part; the LV's β solved on the cavity
        let lv_mean = f64::from(activity[MotionChamber::LeftVentricle.index()]);
        let mut beta_lv =
            ((1.0 - lv.volume) / (self.params.lvef * self.ef_scale[0]).max(1e-6) - lv_mean) as f32;
        // per-vertex contraction of each hooded part (LV, RV, own atrium), smoothed over the mesh
        let mut vc = std::mem::take(&mut self.vertex_c);
        vc.resize_with(self.parts.len(), Vec::new);
        for (k, part) in self.parts.iter().enumerate() {
            let Mapping::Hooded(h) = &part.mapping else {
                continue;
            };
            if !full && k != self.lv_part && h.owner != Owner::Ventricles {
                continue;
            }
            let out = &mut vc[k];
            out.clear();
            let cp = &self.coupling;
            match h.owner {
                Owner::Chamber(_) => out.extend(h.ctrl.iter().map(|x| Vec3::splat(cp.read(x)))),
                Owner::Ventricles => out.extend(h.ctrl.as_chunks::<2>().0.iter().map(|x| {
                    let l = cp.read(&x[0]);
                    Vec3::new(l, cp.read(&x[1]), l)
                })),
                Owner::Any => out.extend(
                    h.ctrl
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .map(|x| Vec3::new(cp.read(&x[0]), cp.read(&x[1]), cp.read(&x[2]))),
                ),
            }
        }
        // right ventricle: the amplitude of its tricuspid annulus follows its volume curve (its
        // contraction there plus β), so TAPSE is the annulus template's excursion over the RV's
        // full ejection
        let annulus_c = match &self.rv_annulus {
            Some(f) if !vc[f.anchor_part].is_empty() => {
                // the anchors' RV contraction, affine-fitted and read at the lateral rim
                let c: Vec<f32> = f
                    .anchors
                    .iter()
                    .map(|&v| vc[f.anchor_part][v as usize].y)
                    .collect();
                let mean = c.iter().sum::<f32>() / c.len().max(1) as f32;
                let mut g = Vec3::ZERO;
                for (&ci, q) in c.iter().zip(&f.anchor_q) {
                    g += *q * (ci - mean);
                }
                let g = f.gram_inv.transpose() * g;
                f64::from(mean + g.dot(f.lateral_rim_mm - f.anchor_centre))
            }
            _ => f64::from(activity[MotionChamber::RightVentricle.index()]),
        };
        let beta_rv = (s_rv / self.ef_scale[1].max(1e-3) - annulus_c) as f32;
        // the ventricular amplitude of a vertex: its own contraction plus the global β of its
        // ventricle (blended on the RV mesh, see `Owner::Ventricles`)
        let lv_axis = self.frame.axis(MotionChamber::LeftVentricle).axis;
        let one = |t: &VentHot, c: Vec3, lv_only: bool, beta_lv: f32, beta_rv: f32| -> Vec3 {
            let a = if lv_only {
                c.x + beta_lv
            } else {
                t.lv_share * (c.x + beta_lv) + (1.0 - t.lv_share) * (c.y + beta_rv)
            };
            let mut v = t.u_eff * a;
            if t.rot_angle != 0.0 {
                let (cm1, s) = rot_terms(t.rot_angle * c.x);
                v += t.rot_r * cm1 + lv_axis.cross(t.rot_r) * s;
            }
            v
        };
        let fill = |h: &Hooded, vc: &[Vec3], disp: &mut [Vec3], beta_lv: f32, beta_rv: f32| {
            if h.vent.is_empty() {
                disp.fill(Vec3::ZERO);
                return;
            }
            let lv_only = h.owner == Owner::Chamber(MotionChamber::LeftVentricle);
            for ((d, c), t) in disp.iter_mut().zip(vc).zip(&h.vent) {
                *d = one(t, *c, lv_only, beta_lv, beta_rv);
            }
        };
        // the LV cavity with β = 0, β solved on it, then the whole LV with that β
        {
            let Mapping::Hooded(h) = &self.parts[self.lv_part].mapping else {
                unreachable!("the lv part is hooded")
            };
            let lv_only = h.owner == Owner::Chamber(MotionChamber::LeftVentricle);
            let vcl = &vc[self.lv_part];
            self.cav_a.clear();
            self.cav_b.clear();
            for &v in &self.cavity.verts {
                let i = v as usize;
                let t = &h.vent[i];
                let d0 = one(t, vcl[i], lv_only, 0.0, beta_rv);
                self.cav_a.push((self.cavity.rest_local[i] + d0).as_dvec3());
                self.cav_b.push(t.u_eff.as_dvec3());
            }
            let c = cavity_poly(&self.cavity, &self.cav_a, &self.cav_b);
            let target = self.cavity.rest_volume_mm3 * lv.volume;
            beta_lv = solve_cubic(&c, target, f64::from(beta_lv)) as f32;
            st.lv_volume_mm3 = eval_poly(&c, f64::from(beta_lv));
            fill(h, vcl, &mut st.disp[self.lv_part], beta_lv, beta_rv);
        }
        // every other part in order, each on the parts moved before it: its own motion, then
        // blended into theirs where it rests on them
        let mut annulus = [Vec3::ZERO; 2]; // mitral, tricuspid: mean disc displacement
        for &k in &self.order {
            if k == self.lv_part {
                continue;
            }
            let part = &self.parts[k];
            match &part.mapping {
                Mapping::Hooded(h) => {
                    if !full && h.owner != Owner::Ventricles {
                        continue;
                    }
                    fill(h, &vc[k], &mut st.disp[k], beta_lv, beta_rv);
                    if full && !h.atr.is_empty() {
                        // atria: their own contraction and the AV-plane displacement they follow
                        let av = match h.owner {
                            Owner::Chamber(MotionChamber::RightAtrium) => annulus[1],
                            _ => annulus[0],
                        };
                        for ((d, c), t) in st.disp[k].iter_mut().zip(&vc[k]).zip(&h.atr) {
                            *d += t.contract * c.z + av * t.follow;
                        }
                    }
                }
                Mapping::Discs(fits) => {
                    if !full {
                        continue;
                    }
                    // the affine fit of each disc to its annulus anchors
                    let rest = &part.rest_pos;
                    let mut mine = std::mem::take(&mut st.disp[k]);
                    for f in fits {
                        let (t, l) = disc_affine(f, &st.disp[f.anchor_part]);
                        let mut mean = Vec3::ZERO;
                        for &v in &f.disc.vertices {
                            let d = t + l * (rest[v as usize] - f.anchor_centre);
                            mine[v as usize] = d;
                            mean += d;
                        }
                        mean /= f.disc.vertices.len().max(1) as f32;
                        match f.disc.valve {
                            Valve::Mitral => annulus[0] = mean,
                            Valve::Tricuspid => annulus[1] = mean,
                            _ => {}
                        }
                    }
                    st.disp[k] = mine;
                }
                Mapping::Follow => {
                    if !full {
                        continue;
                    }
                    st.disp[k].fill(Vec3::ZERO);
                    if let Some(f) = &part.follow {
                        f.apply(k, &mut st.disp, &mut self.attach_corr, &mut self.vertex_tmp);
                    }
                }
            }
            // the read-outs need no attachment: TAPSE is read on the RV's own motion (the
            // correction toward the LV moves the lateral tricuspid annulus by < 0.1 mm)
            if full && let Some(a) = &part.attach {
                a.apply(k, &mut st.disp, &mut self.contact_scratch);
            }
        }
        self.vertex_c = vc;
    }
}

/// The inward vector of a ventricular point: to the long axis, perpendicular, above `centre`
/// (a height, [`MotionParams::apical_centre_height`]); below it, to the axis point at that height,
/// so the apical cap closes toward the cavity (and thickens) instead of only narrowing. The switch
/// is rounded (C¹).
fn inward(ax: &ChamberAxis, p: Vec3, height: f32, centre: f32) -> Vec3 {
    let w = 0.05;
    let x = height - centre;
    let ramp = if x <= -w {
        0.0
    } else if x >= w {
        x
    } else {
        (x + w) * (x + w) / (4.0 * w)
    };
    ax.point_at(centre + ramp) - p
}

/// `clamp(h, 0, 1)` with its corners rounded over ±[`SOFT_CLAMP`] (C¹), so that the long-axis
/// template has no kink at the apex or the valve plane.
fn soft_unit(h: f32) -> f32 {
    let w = SOFT_CLAMP;
    let lo = |x: f32| {
        if x <= -w {
            0.0
        } else if x >= w {
            x
        } else {
            (x + w) * (x + w) / (4.0 * w)
        }
    };
    1.0 - lo(1.0 - lo(h))
}

/// Template of one hooded vertex at `p` with LV and RV transmural depth `rho` and chamber `shares`, for the
/// LV radial fraction `lv_radial` (the template is affine in it).
fn vertex_template(
    p: Vec3,
    rho: [f32; 2],
    shares: &[f32; 4],
    frame: &CardiacFrame,
    params: &MotionParams,
    lv_radial: f32,
) -> VertexTemplate {
    let mut t = VertexTemplate::default();
    let epi = params.epi_radial_ratio as f32;
    let transmural = |r: f32| 1.0 - r.clamp(0.0, 1.0) * (1.0 - epi);
    let (from, basal) = (
        params.radial_taper_from as f32,
        params.basal_radial_fraction as f32,
    );
    let (still, centre) = (
        params.apical_still_height as f32,
        params.apical_centre_height as f32,
    );
    let taper = |h: f32| {
        let x = ((h - from) / (1.0 - from)).clamp(0.0, 1.0);
        1.0 - (1.0 - basal) * x * x * (3.0 - 2.0 * x)
    };
    // left ventricle
    let vs = shares[0] + shares[1];
    t.lv_share = if vs > 0.0 { shares[0] / vs } else { 1.0 };
    if shares[0] > 0.0 {
        let ax = frame.axis(MotionChamber::LeftVentricle);
        let c = ax.coords(p);
        let h = soft_unit((c.height - still) / (1.0 - still));
        let long = -ax.axis * (params.mapse_mm as f32 * h);
        let lv_radial_unit =
            inward(ax, p, c.height, centre) * (transmural(rho[0]) * taper(c.height)) * shares[0];
        t.u_lv = long * shares[0] + lv_radial_unit * lv_radial;
        // rotation, right-handed about the apex-to-base axis
        let (lo, hi) = (
            params.rotation_apex_level as f32,
            params.rotation_base_level as f32,
        );
        let x = ((c.height - lo) / (hi - lo).max(1e-3)).clamp(0.0, 1.0);
        let deg = params.apical_rotation_deg
            + (params.basal_rotation_deg - params.apical_rotation_deg) * f64::from(x);
        let r = c.radial * c.radius_mm;
        t.rot_r = r;
        t.rot_b = ax.axis.cross(r);
        t.rot_angle = (deg.to_radians() as f32) * shares[0];
    }
    // right ventricle
    if shares[1] > 0.0 {
        let ax = frame.axis(MotionChamber::RightVentricle);
        let c = ax.coords(p);
        let h = soft_unit((c.height - still) / (1.0 - still));
        // the inward excursion saturates far from the inflow axis, and the long-axis excursion
        // fades there: the infundibulum lies up to 7 cm from the axis and does not descend with
        // the tricuspid annulus
        let x = c.radius_mm / params.rv_radius_cap_mm as f32;
        let cap = 1.0 / (1.0 + x * x * x * x).sqrt().sqrt();
        let xl = c.radius_mm / params.rv_long_fade_mm as f32;
        let long = -ax.axis * (params.tapse_mm as f32 * h / (1.0 + xl * xl * xl * xl).sqrt());
        let radial = inward(ax, p, c.height, centre)
            * (cap * transmural(rho[1]) * taper(c.height) * params.rv_radial_fraction as f32);
        t.u_rv = (long + radial) * shares[1];
        t.u_rv_long = long * shares[1];
    }
    // atria
    for ch in [MotionChamber::LeftAtrium, MotionChamber::RightAtrium] {
        let s = shares[ch.index()];
        if s <= 0.0 {
            continue;
        }
        let ax = frame.axis(ch);
        let h = ax.coords(p).height.clamp(0.0, 1.0);
        t.atr_follow += s * (1.0 - h);
        t.atr_contract += (ax.centre_mm - p) * (params.atrial_radial_fraction as f32 * s);
    }
    t
}

/// The factor on the RV long-axis template that makes the lateral tricuspid annulus (the
/// tricuspid fit's lateral rim) descend by `tapse_mm` along the RV axis at ventricular amplitude 1
/// (the RV's full ejection), or `None` without a tricuspid fit on a hooded part. The excursion is
/// affine in the factor; the radial template and the LV share near the septum contribute too.
fn rv_long_scale(parts: &[Part], dp: usize, frame: &CardiacFrame, tapse_mm: f32) -> Option<f32> {
    let Mapping::Discs(fits) = &parts[dp].mapping else {
        return None;
    };
    let f = fits.iter().find(|f| f.disc.valve == Valve::Tricuspid)?;
    let Mapping::Hooded(h) = &parts[f.anchor_part].mapping else {
        return None;
    };
    let lv_axis = frame.axis(MotionChamber::LeftVentricle).axis;
    let rv_axis = frame.axis(MotionChamber::RightVentricle).axis;
    let excursion = |k: f32| -> f32 {
        let disp: Vec<Vec3> = h
            .templates
            .iter()
            .map(|t| {
                let (cm1, s) = rot_terms(t.rot_angle);
                t.u_eff + t.u_rv_long * (k - 1.0) + t.rot_r * cm1 + lv_axis.cross(t.rot_r) * s
            })
            .collect();
        let (tr, l) = disc_affine(f, &disp);
        -(tr + l * (f.lateral_rim_mm - f.anchor_centre)).dot(rv_axis)
    };
    let (e0, e1) = (excursion(0.0), excursion(1.0));
    ((e1 - e0).abs() > 1e-3).then(|| (tapse_mm - e0) / (e1 - e0))
}

/// The affine annulus fit of a disc for the anchors' displacements `disp`: translation `t` and
/// gradient `l`, so that a point `x` moves by `t + l (x − anchor_centre)`.
fn disc_affine(f: &DiscFit, disp: &[Vec3]) -> (Vec3, Mat3) {
    let n = f.anchors.len().max(1) as f32;
    let t: Vec3 = f.anchors.iter().map(|&a| disp[a as usize]).sum::<Vec3>() / n;
    let mut r = Mat3::ZERO;
    for (&a, q) in f.anchors.iter().zip(&f.anchor_q) {
        let u = disp[a as usize] - t;
        r += Mat3::from_cols(u * q.x, u * q.y, u * q.z);
    }
    (t, r * f.gram_inv)
}

fn disc_fit(
    disc: &ValveDisc,
    disc_rest: &[Vertex],
    anchor_part: usize,
    anchor_rest: &[Vertex],
    frame: &CardiacFrame,
) -> DiscFit {
    let pts: Vec<Vec3> = disc
        .vertices
        .iter()
        .map(|&v| v3(&disc_rest[v as usize]))
        .collect();
    let grid = HashGrid::new(&pts, ANCHOR_RADIUS_MM);
    let reach = pts
        .iter()
        .map(|p| p.distance(disc.centre_mm))
        .fold(0.0f32, f32::max)
        + ANCHOR_RADIUS_MM;
    let mut anchors: Vec<u32> = (0..anchor_rest.len() as u32)
        .filter(|&i| {
            let q = v3(&anchor_rest[i as usize]);
            q.distance(disc.centre_mm) <= reach && grid.nearest(q).is_some_and(|(d, _)| d <= ANCHOR_RADIUS_MM)
        })
        .collect();
    if anchors.len() < MIN_ANCHORS {
        let mut by_d: Vec<(f32, u32)> = (0..anchor_rest.len() as u32)
            .map(|i| (v3(&anchor_rest[i as usize]).distance(disc.centre_mm), i))
            .collect();
        by_d.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        anchors = by_d.iter().take(MIN_ANCHORS).map(|x| x.1).collect();
        anchors.sort_unstable();
    }
    let n = anchors.len().max(1) as f32;
    let centre: Vec3 = anchors
        .iter()
        .map(|&a| v3(&anchor_rest[a as usize]))
        .sum::<Vec3>()
        / n;
    let anchor_q: Vec<Vec3> = anchors
        .iter()
        .map(|&a| v3(&anchor_rest[a as usize]) - centre)
        .collect();
    let mut g = Mat3::ZERO;
    for q in &anchor_q {
        g += Mat3::from_cols(*q * q.x, *q * q.y, *q * q.z);
    }
    let lambda = (g.x_axis.x + g.y_axis.y + g.z_axis.z) * 1e-3 + 1e-6;
    let gram_inv = (g + Mat3::from_diagonal(Vec3::splat(lambda))).inverse();
    // lateral rim: the disc point furthest from the LV axis' side, for the tricuspid; furthest
    // from the septum (the other AV valve) in general
    let other = match disc.valve {
        Valve::Tricuspid => frame.valve_centres_mm[Valve::Mitral.index()],
        _ => frame.valve_centres_mm[Valve::Tricuspid.index()],
    };
    let away = (disc.centre_mm - other).normalize_or_zero();
    let lateral_rim_mm = pts
        .iter()
        .copied()
        .max_by(|a, b| {
            (*a - disc.centre_mm)
                .dot(away)
                .total_cmp(&(*b - disc.centre_mm).dot(away))
        })
        .unwrap_or(disc.centre_mm);
    DiscFit {
        disc: disc.clone(),
        anchor_part,
        anchors,
        anchor_centre: centre,
        anchor_q,
        gram_inv,
        lateral_rim_mm,
    }
}

/// What vessel `k` follows (its root), and the distance of each of its vertices to it, mm: the
/// aorta the aortic disc, the pulmonary artery the pulmonic disc, the caval veins the right
/// atrium, the pulmonary veins the left atrium, any other vessel the parts `solid`. A vessel
/// following a neighbouring structure it does not spring from (the pulmonary trunk the aortic
/// root) would shear its thin wall between two motions; where it touches one, its contact
/// attachment ([`Contact`]) corrects it.
fn vessel_root(
    parts: &[Part],
    k: usize,
    discs_part: Option<usize>,
    solid: &[usize],
) -> (Vec<(u32, u32)>, Vec<f32>) {
    let by_name = |n: &str| solid.iter().copied().find(|&p| parts[p].name == n);
    let disc = |valve: Valve| -> Option<Vec<(u32, u32)>> {
        let dp = discs_part?;
        let Mapping::Discs(f) = &parts[dp].mapping else {
            return None;
        };
        let d = f.iter().find(|x| x.disc.valve == valve)?;
        Some(d.disc.vertices.iter().map(|&v| (dp as u32, v)).collect())
    };
    let own = |root: Vec<(u32, u32)>| (root.clone(), root);
    let atrium = |n: &str| {
        by_name(n)
            .map(|p| whole_parts(parts, &[p]))
            .map(|r| (r.clone(), r))
    };
    let (from, root) = match parts[k].name.as_str() {
        "aorta" => disc(Valve::Aortic).map(own),
        "pulmonary_artery" => disc(Valve::Pulmonic).map(own),
        "svc" | "ivc" => atrium("ra"),
        "pulmonary_veins" => atrium("la"),
        _ => None,
    }
    .unwrap_or_else(|| {
        let all = whole_parts(parts, solid);
        (all.clone(), all)
    });
    let pts: Vec<Vec3> = root
        .iter()
        .map(|&(p, v)| parts[p as usize].rest_pos[v as usize])
        .collect();
    let dist = if pts.is_empty() {
        vec![f32::INFINITY; parts[k].rest_pos.len()]
    } else {
        let grid = HashGrid::new(&pts, FOLLOW_GRID_MM);
        parts[k]
            .rest_pos
            .iter()
            .map(|&p| grid.nearest(p).map_or(f32::INFINITY, |x| x.0))
            .collect()
    };
    (from, dist)
}

/// The positions `pos` and their area-weighted vertex normals, into `vertices`: the
/// face normals in one sequential pass, then each vertex sums its incident faces (`incident`, a
/// compressed list) — no scattered read-modify-writes.
fn normals_csr(
    vertices: &mut [Vertex],
    pos: &[Vec3A],
    tris: &[[u32; 3]],
    incident: &Adjacency,
    faces: &mut Vec<Vec3A>,
) {
    faces.clear();
    faces.extend(tris.iter().map(|t| {
        let (pa, pb, pc) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
        (pb - pa).cross(pc - pa)
    }));
    for (v, vert) in vertices.iter_mut().enumerate() {
        let mut n = Vec3A::ZERO;
        for &f in incident.of(v) {
            n += faces[f as usize];
        }
        let l2 = n.length_squared();
        *vert = Vertex {
            position: pos[v].to_array(),
            normal: if l2 > 0.0 {
                (n / l2.sqrt()).to_array()
            } else {
                [0.0; 3]
            },
        };
    }
}
