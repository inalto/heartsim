//! Displacement at any point (brief B31, item 5): the wall motion of a [`MotionFrame`] spread onto
//! a coarse rest-space grid over the heart box, extended into the cavities and fading to the
//! thorax's own (respiratory) motion, and inverted by fixed-point iteration.
//!
//! **Filled from the frame the 3-D view already evaluated.** No second evaluation of the motion
//! model: each grid node is a fixed weighted sum of rest-vertex displacements (`frame − rest`,
//! exaggeration and respiration included, exactly what the 3-D heart draws), so the slice and the
//! mesh can never disagree and the cost per frame is one pass over fixed weights.
//!
//! * **Near nodes** (within [`NEAR_MM`] of a rest vertex of any part): Shepard average of up to
//!   [`NEAR_K`] nearest vertices.
//! * **Far nodes** (cavities, wall interiors, the space around the heart): rays along 14 grid
//!   directions to the first near node or to the grid's edge; weight `1/d²`. In a cavity every ray
//!   ends on a wall, so the field is the walls' motion interpolated across it; outside the heart
//!   half the rays end on the edge, whose value is the thorax's motion, so the field fades to it.
//!
//! The field `U` maps rest to current position, `x = X + U(X)`; a sample `x` is taken back to
//! rest by `X ← x − U(X)` ([`DisplacementField::rest_of`], [`INVERSE_ITERATIONS`] steps), which
//! converges because `|∇U| < 1` for the wall strains of the model.

use crate::model::{MotionFrame, MotionModel};
use glam::Vec3;
use heartsim_geometry::{Label, LabelVolume};
use std::collections::BTreeMap;

/// Grid spacing, mm (brief: about 3 mm).
pub const FIELD_SPACING_MM: f32 = 3.0;
/// A node is "near" when a rest vertex lies within this distance, mm.
pub const NEAR_MM: f32 = 3.0;
/// Vertices averaged per near node.
pub const NEAR_K: usize = 8;
/// Fixed-point steps of the inverse map always taken (after the first guess).
pub const INVERSE_ITERATIONS: usize = 2;
/// The iteration stops once a step is shorter than this, mm.
pub const INVERSE_TOLERANCE_MM: f32 = 0.05;
/// At most this many steps (the heart's large basal and atrial motion needs more than two).
pub const INVERSE_MAX_ITERATIONS: usize = 16;

/// Far nodes farther than this from the rest heart are never interpolated: they keep the edge
/// value. Each update interpolates fewer (see [`DisplacementField::update`]); this caps the
/// precomputed rays for an exaggerated motion.
const FAR_NODE_LIMIT_MM: f32 = 60.0;

const EDGE: u32 = u32::MAX;
const DIRS: [[i32; 3]; 14] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
    [1, 1, 1],
    [1, 1, -1],
    [1, -1, 1],
    [1, -1, -1],
    [-1, 1, 1],
    [-1, 1, -1],
    [-1, -1, 1],
    [-1, -1, -1],
];

/// The rest-space displacement grid over the heart (see the module docs).
#[derive(Debug, Clone)]
pub struct DisplacementField {
    origin: Vec3,
    h: f32,
    dims: [usize; 3],
    /// Near nodes: (node, first entry) into `near_src`/`near_w`.
    near_nodes: Vec<(u32, u32)>,
    /// (part, vertex) of each near entry.
    near_src: Vec<(u16, u32)>,
    near_w: Vec<f32>,
    /// Far nodes: (node, first entry) into `far_src`/`far_w`; source `EDGE` = the edge value.
    far_nodes: Vec<(u32, u32)>,
    far_src: Vec<u32>,
    far_w: Vec<f32>,
    /// Rest positions per part, the frame's order.
    rest: Vec<Vec<Vec3>>,
    /// Current displacement per node.
    values: Vec<Vec3>,
    /// The value beyond the grid (the thorax's motion).
    edge: Vec3,
    /// Per node, the distance to the nearest heart-box tissue at rest, mm (0 inside the heart).
    heart_dist: Vec<f32>,
    /// Largest displacement magnitude of the current instant, mm.
    max_disp: f32,
    /// `heart_dist` of each far node, ascending (the order of `far_nodes`).
    far_dist: Vec<f32>,
    /// Far nodes interpolated by the last update.
    far_updated: usize,
}

impl DisplacementField {
    /// The grid over `[lo, hi]` (rest space, mm) for the parts of `model`; `heart` is the rest
    /// heart box (what the inverse map can land in), used to skip the inverse far from the heart.
    #[must_use]
    #[allow(clippy::cast_possible_wrap)] // grid dimensions are a few dozen
    pub fn new(model: &MotionModel, heart: &LabelVolume, lo: Vec3, hi: Vec3) -> Self {
        let h = FIELD_SPACING_MM;
        let lo = (lo / h).floor() * h;
        let n = ((hi - lo) / h).ceil() + Vec3::ONE;
        let dims = [n.x as usize, n.y as usize, n.z as usize];
        let rest: Vec<Vec<Vec3>> = model
            .part_names()
            .map(|name| {
                model
                    .part_rest(name)
                    .unwrap_or_default()
                    .iter()
                    .map(|v| Vec3::from(v.position))
                    .collect()
            })
            .collect();
        // vertex hash on the node grid
        let key = |p: Vec3| {
            let q = ((p - lo) / h).floor();
            (q.x as i32, q.y as i32, q.z as i32)
        };
        let mut bins: BTreeMap<(i32, i32, i32), Vec<(u16, u32)>> = BTreeMap::new();
        for (k, part) in rest.iter().enumerate() {
            for (v, p) in part.iter().enumerate() {
                bins.entry(key(*p)).or_default().push((k as u16, v as u32));
            }
        }
        let count = dims[0] * dims[1] * dims[2];
        let node_pos = |idx: usize| {
            let i = idx % dims[0];
            let j = (idx / dims[0]) % dims[1];
            let k = idx / (dims[0] * dims[1]);
            lo + Vec3::new(i as f32, j as f32, k as f32) * h
        };
        // a node is "in the heart" when a heart-box tissue voxel lies in one of its eight cells,
        // so thin walls between nodes are not missed; the chamfer distance from those nodes is a
        // safe (over-)estimate of the distance to the heart, see `may_reach_heart`
        let mut inside = vec![false; count];
        for (vi, l) in heart.labels.iter().enumerate() {
            if *l == Label::Outside as u8 {
                continue;
            }
            let [a, b, c] = heart.coords(vi);
            let q = ((heart.centre(a, b, c) - lo) / h).floor();
            if q.x < 0.0 || q.y < 0.0 || q.z < 0.0 {
                continue;
            }
            let (i, j, k) = (q.x as usize, q.y as usize, q.z as usize);
            for (di, dj, dk) in [
                (0, 0, 0),
                (1, 0, 0),
                (0, 1, 0),
                (1, 1, 0),
                (0, 0, 1),
                (1, 0, 1),
                (0, 1, 1),
                (1, 1, 1),
            ] {
                let (x, y, z) = (i + di, j + dj, k + dk);
                if x < dims[0] && y < dims[1] && z < dims[2] {
                    inside[x + dims[0] * (y + dims[1] * z)] = true;
                }
            }
        }
        let heart_dist = node_distance(dims, h, |idx| inside[idx]);
        let mut near_nodes = Vec::new();
        let mut near_src = Vec::new();
        let mut near_w = Vec::new();
        let mut is_near = vec![false; count];
        let mut found: Vec<(f32, u16, u32)> = Vec::new();
        for (idx, near) in is_near.iter_mut().enumerate() {
            let p = node_pos(idx);
            let kk = key(p);
            found.clear();
            for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if let Some(list) = bins.get(&(kk.0 + dx, kk.1 + dy, kk.2 + dz)) {
                            for &(part, v) in list {
                                let d = rest[part as usize][v as usize].distance(p);
                                if d <= NEAR_MM {
                                    found.push((d, part, v));
                                }
                            }
                        }
                    }
                }
            }
            if found.is_empty() {
                continue;
            }
            found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
            found.truncate(NEAR_K);
            *near = true;
            near_nodes.push((idx as u32, near_src.len() as u32));
            let ws: Vec<f32> = found.iter().map(|(d, _, _)| 1.0 / (d * d + 0.25)).collect();
            let sum: f32 = ws.iter().sum();
            for ((_, part, v), w) in found.iter().zip(ws) {
                near_src.push((*part, *v));
                near_w.push(w / sum);
            }
        }
        let mut far_nodes = Vec::new();
        let mut far_src = Vec::new();
        let mut far_w = Vec::new();
        let (nx, ny, nz) = (dims[0] as i32, dims[1] as i32, dims[2] as i32);
        // far nodes nearest the heart first: an update interpolates only as far out as the
        // instant's displacement needs (see `update`)
        let mut order: Vec<usize> = (0..count)
            .filter(|&i| !is_near[i] && heart_dist[i] <= FAR_NODE_LIMIT_MM)
            .collect();
        order.sort_by(|&a, &b| heart_dist[a].total_cmp(&heart_dist[b]).then(a.cmp(&b)));
        let mut far_dist = Vec::with_capacity(order.len());
        for idx in order {
            far_dist.push(heart_dist[idx]);
            let i0 = (idx % dims[0]) as i32;
            let j0 = ((idx / dims[0]) % dims[1]) as i32;
            let k0 = (idx / (dims[0] * dims[1])) as i32;
            far_nodes.push((idx as u32, far_src.len() as u32));
            let start = far_w.len();
            for d in DIRS {
                let step = ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as f32).sqrt() * h;
                let (mut i, mut j, mut k) = (i0, j0, k0);
                let mut s = 0usize;
                loop {
                    i += d[0];
                    j += d[1];
                    k += d[2];
                    s += 1;
                    if i < 0 || j < 0 || k < 0 || i >= nx || j >= ny || k >= nz {
                        let len = s as f32 * step;
                        far_src.push(EDGE);
                        far_w.push(1.0 / (len * len));
                        break;
                    }
                    let n = i as usize + dims[0] * (j as usize + dims[1] * k as usize);
                    if is_near[n] {
                        let len = s as f32 * step;
                        far_src.push(n as u32);
                        far_w.push(1.0 / (len * len));
                        break;
                    }
                }
            }
            // merge the rays that end on the same node (and every ray that leaves the grid into
            // one edge term): fewer terms per update, the same weighted sum
            let mut terms: Vec<(u32, f32)> = far_src[start..]
                .iter()
                .copied()
                .zip(far_w[start..].iter().copied())
                .collect();
            terms.sort_by_key(|t| t.0);
            far_src.truncate(start);
            far_w.truncate(start);
            for (src, w) in terms {
                if far_src.len() > start && far_src[far_src.len() - 1] == src {
                    *far_w.last_mut().expect("a term") += w;
                } else {
                    far_src.push(src);
                    far_w.push(w);
                }
            }
            let sum: f32 = far_w[start..].iter().sum();
            for w in &mut far_w[start..] {
                *w /= sum;
            }
        }
        Self {
            heart_dist,
            far_dist,
            far_updated: 0,
            max_disp: 0.0,
            origin: lo,
            h,
            dims,
            near_nodes,
            near_src,
            near_w,
            far_nodes,
            far_src,
            far_w,
            rest,
            values: vec![Vec3::ZERO; count],
            edge: Vec3::ZERO,
        }
    }

    /// Rest-space box the grid covers, mm.
    #[must_use]
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let n = Vec3::new(
            (self.dims[0] - 1) as f32,
            (self.dims[1] - 1) as f32,
            (self.dims[2] - 1) as f32,
        );
        (self.origin, self.origin + n * self.h)
    }

    /// Number of grid nodes (near, far).
    #[must_use]
    pub fn node_counts(&self) -> (usize, usize) {
        (self.near_nodes.len(), self.far_nodes.len())
    }

    /// Largest displacement of the current instant, mm (walls and the thorax's shift).
    #[must_use]
    pub fn max_displacement_mm(&self) -> f32 {
        self.max_disp
    }

    /// Far nodes the last [`Self::update`] interpolated (the rest keep the edge value).
    #[must_use]
    pub fn far_nodes_updated(&self) -> usize {
        self.far_updated
    }

    /// Zero motion everywhere (the rest anatomy) with `edge` beyond the heart.
    pub fn set_rest(&mut self, edge: Vec3) {
        self.far_updated = 0;
        self.edge = edge;
        self.values.fill(edge);
        self.max_disp = edge.length();
    }

    /// Whether the rest position of current point `x` can lie in the heart box's tissue: false
    /// when `x` is farther from the rest heart than the largest displacement of the instant (plus
    /// a grid cell), so the slice can skip the inverse map there.
    #[must_use]
    pub fn may_reach_heart(&self, x: Vec3) -> bool {
        let q = ((x - self.origin) / self.h).round();
        let [nx, ny, nz] = self.dims;
        if !(q.x >= 0.0 && q.y >= 0.0 && q.z >= 0.0 && q.x < nx as f32 && q.y < ny as f32 && q.z < nz as f32)
        {
            return false;
        }
        let n = q.x as usize + nx * (q.y as usize + ny * q.z as usize);
        self.heart_dist[n] <= self.reach_limit()
    }

    /// Node distance (chamfer, from the heart's node cells) within which a sample may map into
    /// the heart: a sample `x` lies within 0.87 h of its nearest node, a heart node within
    /// 1.73 h of heart tissue, and the inverse moves `x` by at most the largest displacement, so a
    /// node farther than `max_disp + 2.6 h + 1 mm` cannot be reached (the chamfer never
    /// underestimates).
    fn reach_limit(&self) -> f32 {
        self.max_disp + 2.6 * self.h + 1.0
    }

    /// Fill the grid from `frame` (the evaluated vertices, in the model's part order) with `edge`
    /// the thorax's displacement. A frame with the wrong parts leaves the field at rest.
    pub fn update(&mut self, frame: &MotionFrame, edge: Vec3) {
        self.edge = edge;
        if frame.parts.len() != self.rest.len()
            || frame
                .parts
                .iter()
                .zip(&self.rest)
                .any(|(p, r)| p.vertices.len() != r.len())
        {
            self.set_rest(edge);
            return;
        }
        // nodes that are neither near nor interpolated keep the edge value
        self.values.fill(edge);
        for (a, &(node, start)) in self.near_nodes.iter().enumerate() {
            let end = self
                .near_nodes
                .get(a + 1)
                .map_or(self.near_src.len(), |x| x.1 as usize);
            let mut u = Vec3::ZERO;
            for e in start as usize..end {
                let (part, v) = self.near_src[e];
                let cur = Vec3::from(frame.parts[part as usize].vertices[v as usize].position);
                u += (cur - self.rest[part as usize][v as usize]) * self.near_w[e];
            }
            self.values[node as usize] = u;
        }
        // the far nodes are convex combinations of the near ones and the edge, so this bounds the
        // whole field; interpolate only the far nodes an inverse can touch: within one cell of a
        // reachable node, plus the displacement the iteration can move by
        self.max_disp = edge.length();
        for &(node, _) in &self.near_nodes {
            self.max_disp = self.max_disp.max(self.values[node as usize].length());
        }
        // a reachable sample's trilinear cell has its corners within one diagonal step of the
        // reachable node; the fixed-point iterates of a rest point inside the heart stay inside
        // that region
        let limit = self.reach_limit() + 2.0 * self.h;
        self.far_updated = 0;
        for (a, &(node, start)) in self.far_nodes.iter().enumerate() {
            if self.far_dist[a] > limit {
                break;
            }
            self.far_updated += 1;
            let end = self
                .far_nodes
                .get(a + 1)
                .map_or(self.far_src.len(), |x| x.1 as usize);
            let mut u = Vec3::ZERO;
            for e in start as usize..end {
                let s = self.far_src[e];
                let v = if s == EDGE { edge } else { self.values[s as usize] };
                u += v * self.far_w[e];
            }
            self.values[node as usize] = u;
        }
    }

    /// Displacement at rest point `p` (trilinear; the edge value beyond the grid).
    #[must_use]
    pub fn at(&self, p: Vec3) -> Vec3 {
        let q = (p - self.origin) / self.h;
        let [nx, ny, nz] = self.dims;
        if !(q.x >= 0.0
            && q.y >= 0.0
            && q.z >= 0.0
            && q.x <= (nx - 1) as f32
            && q.y <= (ny - 1) as f32
            && q.z <= (nz - 1) as f32)
        {
            return self.edge;
        }
        let i = (q.x as usize).min(nx - 2);
        let j = (q.y as usize).min(ny - 2);
        let k = (q.z as usize).min(nz - 2);
        let f = q - Vec3::new(i as f32, j as f32, k as f32);
        let at = |a: usize, b: usize, c: usize| self.values[a + nx * (b + ny * c)];
        let x00 = at(i, j, k).lerp(at(i + 1, j, k), f.x);
        let x10 = at(i, j + 1, k).lerp(at(i + 1, j + 1, k), f.x);
        let x01 = at(i, j, k + 1).lerp(at(i + 1, j, k + 1), f.x);
        let x11 = at(i, j + 1, k + 1).lerp(at(i + 1, j + 1, k + 1), f.x);
        x00.lerp(x10, f.y).lerp(x01.lerp(x11, f.y), f.z)
    }

    /// The rest position of current point `x`: `X ← x − U(X)` from `x − U(x)`, see
    /// [`Self::solve_rest`].
    #[must_use]
    pub fn rest_of(&self, x: Vec3) -> Vec3 {
        self.solve_rest(x, x - self.at(x))
    }

    /// The fixed point of `X ← x − U(X)` from the guess `start`: at least
    /// [`INVERSE_ITERATIONS`] plain steps, then on until a step moves less than
    /// [`INVERSE_TOLERANCE_MM`] (the step is the residual of the previous iterate), damped by half
    /// after four steps (where the field is steep the plain iteration oscillates), at most
    /// [`INVERSE_MAX_ITERATIONS`] steps.
    #[must_use]
    pub fn solve_rest(&self, x: Vec3, start: Vec3) -> Vec3 {
        let mut r = start;
        for i in 0..INVERSE_MAX_ITERATIONS {
            let next = x - self.at(r);
            let d = next - r;
            r = if i < 4 { next } else { r + d * 0.5 };
            if i + 1 >= INVERSE_ITERATIONS && d.length_squared() < INVERSE_TOLERANCE_MM * INVERSE_TOLERANCE_MM
            {
                break;
            }
        }
        r
    }
}

/// Distance (mm) from every node of a `dims` grid of spacing `h` to the nearest node where
/// `inside` holds: two-pass chamfer with weights 1, √2, √3 (× h).
#[allow(clippy::cast_possible_wrap)] // grid dimensions are a few dozen
fn node_distance(dims: [usize; 3], h: f32, inside: impl Fn(usize) -> bool) -> Vec<f32> {
    let [nx, ny, nz] = dims.map(|d| d as isize);
    let count = dims[0] * dims[1] * dims[2];
    let mut d: Vec<f32> = (0..count)
        .map(|i| if inside(i) { 0.0 } else { f32::INFINITY })
        .collect();
    let mut fwd = Vec::new();
    for dz in -1..=1isize {
        for dy in -1..=1isize {
            for dx in -1..=1isize {
                if dz < 0 || (dz == 0 && (dy < 0 || (dy == 0 && dx < 0))) {
                    fwd.push((dx, dy, dz, ((dx * dx + dy * dy + dz * dz) as f32).sqrt() * h));
                }
            }
        }
    }
    let at = |i: isize, j: isize, k: isize| (i + nx * (j + ny * k)) as usize;
    let inb = |i: isize, j: isize, k: isize| i >= 0 && j >= 0 && k >= 0 && i < nx && j < ny && k < nz;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let mut b = d[at(i, j, k)];
                for &(dx, dy, dz, w) in &fwd {
                    if inb(i + dx, j + dy, k + dz) {
                        b = b.min(d[at(i + dx, j + dy, k + dz)] + w);
                    }
                }
                d[at(i, j, k)] = b;
            }
        }
    }
    for k in (0..nz).rev() {
        for j in (0..ny).rev() {
            for i in (0..nx).rev() {
                let mut b = d[at(i, j, k)];
                for &(dx, dy, dz, w) in &fwd {
                    if inb(i - dx, j - dy, k - dz) {
                        b = b.min(d[at(i - dx, j - dy, k - dz)] + w);
                    }
                }
                d[at(i, j, k)] = b;
            }
        }
    }
    d
}
