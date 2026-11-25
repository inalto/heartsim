//! An animated heart valve in the anatomy pack (brief B35): one donor valve's leaflet midsurface
//! and chordae, fitted to the pack heart's annulus at bake time, with a few keyframes of its shape
//! between open and shut.
//!
//! **What the data are.** The leaflets are the midsurface of a finite-element model (triangles,
//! zero thickness: the thickness is given at render time and by the echo's contains test); the
//! chordae are line segments. Vertex `0..leaflet_vertices` lie on the leaflets, the rest on the
//! chordae only. The keyframes are shapes of one *load* sequence (pressure rising from open to
//! shut), not a timed recording: [`AnimatedValve::pose_rest`] maps the engine's valve opening
//! (`1` open … `0` shut, from the valve schedule) onto them. Nothing here keeps a clock.
//!
//! **Frames.** Every position is in the pack's heart frame at rest (mm). The valve rides the
//! beating heart through [`DiscCarrier`]: the affine motion of its valve disc plus the disc
//! rim's own non-affine motion, which the annulus takes in full and the free edge not at all.
//!
//! The file is `valve_<name>.bin` (`HSAV` version 1, little-endian), listed in the manifest with
//! its size and CRC-32 like every other pack file.

use crate::mesh::{TriMesh, Vertex};
use glam::{Mat3, Vec3};

/// Magic of a valve file.
pub const VALVE_MAGIC: &[u8; 4] = b"HSAV";
/// Version of the valve record.
pub const VALVE_VERSION: u32 = 1;
/// Azimuth bins of a valve disc's rim (the same count as the echo's designed leaflets).
pub const RIM_BINS: usize = 72;

/// Why a valve file did not decode.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValveError {
    #[error("valve file: bad magic")]
    Magic,
    #[error("valve file: version {0}, this build reads {VALVE_VERSION}")]
    Version(u32),
    #[error("valve file: truncated")]
    Truncated,
    #[error("valve file: {0}")]
    Invalid(String),
}

/// One keyframe after the rest shape: how far along the closure it is (`0` = the rest shape, open;
/// `1` = shut) and each vertex's offset from the rest position in units of
/// [`AnimatedValve::quantum_mm`].
#[derive(Debug, Clone, PartialEq)]
pub struct ValveKeyframe {
    /// Closure of this shape, `(0, 1]`, strictly increasing over the keyframes; the last is `1`.
    /// Defined at bake time as the fraction of the free edge's approach to the coaptation point
    /// (see [`AnimatedValve::closure_metric_mm`]) between the rest shape and the shut one.
    pub closure: f32,
    pub offsets: Vec<[i16; 3]>,
}

/// An animated valve (see the module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct AnimatedValve {
    /// Which valve: `"tricuspid"` (the only one baked so far), `"mitral"`, `"aortic"`, `"pulmonary"`.
    pub name: String,
    /// Leaflet names, indexed by the leaflet ids below (tricuspid: anterior, septal, posterior).
    pub leaflet_names: Vec<String>,
    /// Rest shape = the first keyframe (the open valve), heart frame, mm.
    pub rest_mm: Vec<Vec3>,
    /// Vertices `0..leaflet_vertices` are on the leaflet surface; the rest on the chordae only.
    pub leaflet_vertices: u32,
    /// Leaflet midsurface triangles (indices below `leaflet_vertices`), consistently wound.
    pub triangles: Vec<[u32; 3]>,
    /// Leaflet id of each triangle.
    pub triangle_leaflet: Vec<u8>,
    /// Leaflet id of each vertex: the leaflet of the first triangle that uses it (a vertex on the
    /// seam between two leaflets belongs to one of them); for a chordae vertex, the leaflet its
    /// chord belongs to.
    pub vertex_leaflet: Vec<u8>,
    /// Chordae segments (vertex index pairs; an end may be a leaflet vertex, its insertion).
    pub chordae: Vec<[u32; 2]>,
    /// Drawn radius of a chord, mm (from the source's truss cross-section, scaled by the fit).
    pub chordae_radius_mm: f32,
    /// The annulus ring (leaflet vertices on the rim of the valve disc).
    pub annulus: Vec<u32>,
    /// The free edge (leaflet vertices on the leaflets' free margin).
    pub free_edge: Vec<u32>,
    /// Share of the disc rim's non-affine motion each vertex takes when the heart moves: 1 on the
    /// annulus, fading to 0 toward the free edge; 0 on the chordae.
    pub follow_weight: Vec<f32>,
    /// Coaptation point: the mean free-edge position of the shut keyframe, rest frame, mm.
    pub coaptation_mm: Vec3,
    /// Size of one offset step of the keyframes, mm.
    pub quantum_mm: f32,
    /// Uniform scale of the similarity part of the fit (pack annulus / donor annulus).
    pub fit_scale: f32,
    /// Area of each leaflet in the source (donor) model before decimation and fitting, mm².
    pub source_leaflet_area_mm2: Vec<f32>,
    /// The keyframes after the rest shape, closure ascending.
    pub keyframes: Vec<ValveKeyframe>,
}

impl AnimatedValve {
    /// Number of shapes including the rest shape.
    #[must_use]
    pub fn keyframe_count(&self) -> usize {
        self.keyframes.len() + 1
    }

    /// Closure of shape `k` (`0` for the rest shape).
    #[must_use]
    pub fn closure_of(&self, k: usize) -> f32 {
        if k == 0 {
            0.0
        } else {
            self.keyframes[k - 1].closure
        }
    }

    /// The positions of shape `k` (0 = rest, open; `keyframe_count() − 1` = shut).
    pub fn keyframe_positions(&self, k: usize, out: &mut Vec<Vec3>) {
        out.clear();
        if k == 0 {
            out.extend_from_slice(&self.rest_mm);
            return;
        }
        let q = self.quantum_mm;
        let kf = &self.keyframes[k - 1];
        out.extend(self.rest_mm.iter().zip(&kf.offsets).map(|(r, o)| {
            let off = Vec3::new(f32::from(o[0]), f32::from(o[1]), f32::from(o[2]));
            *r + off * q
        }));
    }

    /// The rest-frame shape at `opening` (`1` = fully open … `0` = shut, as
    /// `heartsim_echo::valve_openings` gives it). The closure `c = 1 − opening` selects the two
    /// keyframes whose closures bracket it and interpolates their positions linearly, so the map
    /// is continuous and piecewise linear in `c`: `opening = 1` is the rest shape and `opening = 0`
    /// the shut keyframe, bit for bit. Out-of-range openings are clamped; NaN counts as shut.
    pub fn pose_rest(&self, opening: f32, out: &mut Vec<Vec3>) {
        let c = if opening.is_nan() {
            1.0
        } else {
            (1.0 - opening).clamp(0.0, 1.0)
        };
        if c <= 0.0 || self.keyframes.is_empty() {
            out.clear();
            out.extend_from_slice(&self.rest_mm);
            return;
        }
        let j = self
            .keyframes
            .iter()
            .position(|k| k.closure >= c)
            .unwrap_or(self.keyframes.len() - 1);
        let (ci, oi) = if j == 0 {
            (0.0, None)
        } else {
            (
                self.keyframes[j - 1].closure,
                Some(&self.keyframes[j - 1].offsets),
            )
        };
        let b = &self.keyframes[j];
        let t = ((c - ci) / (b.closure - ci).max(1e-12)).clamp(0.0, 1.0);
        let q = self.quantum_mm;
        out.clear();
        let o3 = |o: &[i16; 3]| Vec3::new(f32::from(o[0]), f32::from(o[1]), f32::from(o[2]));
        match oi {
            None => out.extend(
                self.rest_mm
                    .iter()
                    .zip(&b.offsets)
                    .map(|(r, ob)| *r + (o3(ob) * t) * q),
            ),
            Some(a) => out.extend(
                self.rest_mm
                    .iter()
                    .zip(a.iter().zip(&b.offsets))
                    .map(|(r, (oa, ob))| *r + (o3(oa) * (1.0 - t) + o3(ob) * t) * q),
            ),
        }
    }

    /// Mean distance of the free-edge vertices of `positions` from [`Self::coaptation_mm`], mm:
    /// the closure measure the keyframes are ordered by (it falls as the valve shuts).
    #[must_use]
    pub fn closure_metric_mm(&self, positions: &[Vec3]) -> f32 {
        let n = self.free_edge.len().max(1) as f32;
        self.free_edge
            .iter()
            .map(|&i| positions[i as usize].distance(self.coaptation_mm))
            .sum::<f32>()
            / n
    }

    /// Area of each leaflet of `positions`, mm² (indexed by leaflet id).
    #[must_use]
    pub fn leaflet_areas_mm2(&self, positions: &[Vec3]) -> Vec<f32> {
        let mut out = vec![0.0f32; self.leaflet_names.len()];
        for (t, &l) in self.triangles.iter().zip(&self.triangle_leaflet) {
            let [a, b, c] = t.map(|i| positions[i as usize]);
            if let Some(o) = out.get_mut(usize::from(l)) {
                *o += (b - a).cross(c - a).length() * 0.5;
            }
        }
        out
    }

    /// The `HSAV` file.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let n = self.rest_mm.len();
        let mut w = Vec::with_capacity(64 + n * (12 + 5 + 6 * self.keyframes.len()));
        w.extend_from_slice(VALVE_MAGIC);
        put_u32(&mut w, VALVE_VERSION);
        put_str(&mut w, &self.name);
        put_u32(&mut w, self.leaflet_names.len() as u32);
        for s in &self.leaflet_names {
            put_str(&mut w, s);
        }
        for c in [
            n,
            self.leaflet_vertices as usize,
            self.triangles.len(),
            self.chordae.len(),
            self.annulus.len(),
            self.free_edge.len(),
            self.keyframes.len(),
        ] {
            put_u32(&mut w, c as u32);
        }
        for x in [self.quantum_mm, self.fit_scale, self.chordae_radius_mm] {
            put_f32(&mut w, x);
        }
        for x in self.coaptation_mm.to_array() {
            put_f32(&mut w, x);
        }
        for a in &self.source_leaflet_area_mm2 {
            put_f32(&mut w, *a);
        }
        for p in &self.rest_mm {
            for x in p.to_array() {
                put_f32(&mut w, x);
            }
        }
        w.extend_from_slice(&self.vertex_leaflet);
        for x in &self.follow_weight {
            put_f32(&mut w, *x);
        }
        for t in &self.triangles {
            for i in t {
                put_u32(&mut w, *i);
            }
        }
        w.extend_from_slice(&self.triangle_leaflet);
        for s in &self.chordae {
            for i in s {
                put_u32(&mut w, *i);
            }
        }
        for i in self.annulus.iter().chain(&self.free_edge) {
            put_u32(&mut w, *i);
        }
        for k in &self.keyframes {
            put_f32(&mut w, k.closure);
            for o in &k.offsets {
                for x in o {
                    w.extend_from_slice(&x.to_le_bytes());
                }
            }
        }
        w
    }

    /// Read an `HSAV` file; every index, count and closure is checked, so a valve that decodes
    /// can be posed and drawn without bounds failures.
    ///
    /// # Errors
    /// [`ValveError`] on a bad magic, version, truncation or inconsistent content.
    pub fn decode(bytes: &[u8]) -> Result<Self, ValveError> {
        let mut r = Rd { b: bytes, pos: 0 };
        if r.take(4)? != VALVE_MAGIC {
            return Err(ValveError::Magic);
        }
        let version = r.u32()?;
        if version != VALVE_VERSION {
            return Err(ValveError::Version(version));
        }
        let name = r.string()?;
        let leaflets = r.u32()? as usize;
        if leaflets > 16 {
            return Err(ValveError::Invalid(format!("{leaflets} leaflets")));
        }
        let leaflet_names = (0..leaflets).map(|_| r.string()).collect::<Result<Vec<_>, _>>()?;
        let mut c = [0usize; 7];
        for x in &mut c {
            *x = r.u32()? as usize;
        }
        let [n, nl, nt, nc, na, nf, nk] = c;
        // a coarse plausibility bound before allocating
        if n > 1 << 22 || nt > 1 << 23 || nc > 1 << 22 || nk > 64 || nl > n {
            return Err(ValveError::Invalid(format!("counts {c:?}")));
        }
        let quantum_mm = r.f32()?;
        let fit_scale = r.f32()?;
        let chordae_radius_mm = r.f32()?;
        let coaptation_mm = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
        let source_leaflet_area_mm2 = (0..leaflets).map(|_| r.f32()).collect::<Result<Vec<_>, _>>()?;
        let mut rest_mm = Vec::with_capacity(n);
        for _ in 0..n {
            rest_mm.push(Vec3::new(r.f32()?, r.f32()?, r.f32()?));
        }
        let vertex_leaflet = r.take(n)?.to_vec();
        let follow_weight = (0..n).map(|_| r.f32()).collect::<Result<Vec<_>, _>>()?;
        let mut triangles = Vec::with_capacity(nt);
        for _ in 0..nt {
            triangles.push([r.u32()?, r.u32()?, r.u32()?]);
        }
        let triangle_leaflet = r.take(nt)?.to_vec();
        let mut chordae = Vec::with_capacity(nc);
        for _ in 0..nc {
            chordae.push([r.u32()?, r.u32()?]);
        }
        let annulus = (0..na).map(|_| r.u32()).collect::<Result<Vec<_>, _>>()?;
        let free_edge = (0..nf).map(|_| r.u32()).collect::<Result<Vec<_>, _>>()?;
        let mut keyframes = Vec::with_capacity(nk);
        for _ in 0..nk {
            let closure = r.f32()?;
            let raw = r.take(n * 6)?;
            let offsets = raw
                .as_chunks::<6>()
                .0
                .iter()
                .map(|b| {
                    [
                        i16::from_le_bytes([b[0], b[1]]),
                        i16::from_le_bytes([b[2], b[3]]),
                        i16::from_le_bytes([b[4], b[5]]),
                    ]
                })
                .collect();
            keyframes.push(ValveKeyframe { closure, offsets });
        }
        if r.pos != bytes.len() {
            return Err(ValveError::Invalid("trailing bytes".into()));
        }
        let bad = |what: &str| Err(ValveError::Invalid(what.to_string()));
        if !(quantum_mm.is_finite() && quantum_mm > 0.0 && fit_scale.is_finite()) {
            return bad("quantum or scale");
        }
        if rest_mm.iter().any(|p| !p.is_finite()) || follow_weight.iter().any(|w| !w.is_finite()) {
            return bad("non-finite position or weight");
        }
        if triangles.iter().flatten().any(|&i| i as usize >= nl) {
            return bad("triangle index past the leaflet vertices");
        }
        if chordae.iter().flatten().any(|&i| i as usize >= n) {
            return bad("chordae index out of range");
        }
        if annulus.iter().chain(&free_edge).any(|&i| i as usize >= nl) {
            return bad("annulus or free-edge index out of range");
        }
        if triangle_leaflet
            .iter()
            .chain(&vertex_leaflet)
            .any(|&l| usize::from(l) >= leaflets.max(1))
        {
            return bad("leaflet id out of range");
        }
        let mut prev = 0.0f32;
        for k in &keyframes {
            if !(k.closure > prev && k.closure <= 1.0) {
                return bad("keyframe closures are not strictly increasing in (0, 1]");
            }
            prev = k.closure;
        }
        if !keyframes.is_empty() && (prev - 1.0).abs() > 1e-6 {
            return bad("the last keyframe is not shut");
        }
        Ok(Self {
            name,
            leaflet_names,
            rest_mm,
            leaflet_vertices: nl as u32,
            triangles,
            triangle_leaflet,
            vertex_leaflet,
            chordae,
            chordae_radius_mm,
            annulus,
            free_edge,
            follow_weight,
            coaptation_mm,
            quantum_mm,
            fit_scale,
            source_leaflet_area_mm2,
            keyframes,
        })
    }
}

fn put_u32(w: &mut Vec<u8>, x: u32) {
    w.extend_from_slice(&x.to_le_bytes());
}
fn put_f32(w: &mut Vec<u8>, x: f32) {
    w.extend_from_slice(&x.to_le_bytes());
}
fn put_str(w: &mut Vec<u8>, s: &str) {
    put_u32(w, s.len() as u32);
    w.extend_from_slice(s.as_bytes());
}

struct Rd<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ValveError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|e| *e <= self.b.len())
            .ok_or(ValveError::Truncated)?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, ValveError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self) -> Result<f32, ValveError> {
        self.u32().map(f32::from_bits)
    }
    fn string(&mut self) -> Result<String, ValveError> {
        let n = self.u32()? as usize;
        if n > 256 {
            return Err(ValveError::Invalid("string too long".into()));
        }
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| ValveError::Invalid("string not UTF-8".into()))
    }
}

// --- the valve discs ------------------------------------------------------------------------------

/// The connected components of `mesh`, as vertex lists in ascending index order (components
/// ordered by their smallest vertex index).
#[must_use]
pub fn components(mesh: &TriMesh) -> Vec<Vec<u32>> {
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let n = mesh.vertices.len();
    let mut parent: Vec<usize> = (0..n).collect();
    for tri in mesh.indices.as_chunks::<3>().0 {
        let a = find(&mut parent, tri[0] as usize);
        for &v in &tri[1..] {
            let b = find(&mut parent, v as usize);
            if a != b {
                parent[b] = a;
            }
        }
    }
    let mut root_slot: Vec<usize> = vec![usize::MAX; n];
    let mut out: Vec<Vec<u32>> = Vec::new();
    for v in 0..n {
        let r = find(&mut parent, v);
        if root_slot[r] == usize::MAX {
            root_slot[r] = out.len();
            out.push(Vec::new());
        }
        out[root_slot[r]].push(v as u32);
    }
    out
}

/// The vertex lists of the four discs of a valve-plane mesh in the order mitral, tricuspid,
/// aortic, pulmonic, or `None` unless it has exactly four components. The rule of
/// `heartsim_echo::split_valve_discs` and `heartsim_app::heart_view::split_valves` (bounding-box
/// centres: pulmonic the largest `y + z`, then tricuspid the smallest `x`, then mitral the smallest
/// `z`, the aortic the one left), so the bake and the app agree on which disc is which.
#[must_use]
pub fn split_discs(mesh: &TriMesh) -> Option<[Vec<u32>; 4]> {
    let comps = components(mesh);
    if comps.len() != 4 {
        return None;
    }
    let pos = |i: u32| Vec3::from(mesh.vertices[i as usize].position);
    let mut left: Vec<(Vec3, Vec<u32>)> = comps
        .into_iter()
        .map(|c| {
            let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
            for &i in &c {
                lo = lo.min(pos(i));
                hi = hi.max(pos(i));
            }
            ((lo + hi) * 0.5, c)
        })
        .collect();
    let mut take = |key: fn(Vec3) -> f32| {
        let i = (0..left.len())
            .min_by(|&a, &b| key(left[a].0).total_cmp(&key(left[b].0)))
            .unwrap_or(0);
        left.remove(i).1
    };
    let pulmonic = take(|c| -(c.y + c.z));
    let tricuspid = take(|c| c.x);
    let mitral = take(|c| c.z);
    let aortic = take(|c| c.x);
    Some([mitral, tricuspid, aortic, pulmonic])
}

/// The rim of a valve disc (a thin closed slab in the pack): per azimuth bin about the disc's
/// centre and normal, the disc vertex furthest from the centre in the disc plane; the rim curve is
/// those vertices smoothed with binomial weights over seven bins, so every rim point is a fixed
/// linear combination of disc vertices and moves exactly with them.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscRim {
    /// Mean of the disc vertices, mm.
    pub centre_mm: Vec3,
    /// Unit normal, pointing downstream (toward the point given to [`Self::new`]).
    pub normal: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    /// Per bin, the index (into the point list given to [`Self::new`]) of its outermost vertex.
    pub rim_index: Vec<u32>,
}

/// Binomial smoothing weights of the rim over bins `−3..=3` (sum 64).
const RIM_SMOOTH: [f32; 7] = [1.0, 6.0, 15.0, 20.0, 15.0, 6.0, 1.0];

impl DiscRim {
    /// The rim of the disc whose vertices are `points`, the normal oriented toward `downstream_mm`.
    /// `None` for fewer than three points.
    #[must_use]
    pub fn new(points: &[Vec3], downstream_mm: Vec3) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }
        let c = points.iter().copied().sum::<Vec3>() / points.len() as f32;
        let mut m = Mat3::ZERO;
        for p in points {
            let q = *p - c;
            m += Mat3::from_cols(q * q.x, q * q.y, q * q.z);
        }
        let mut n = smallest_eigenvector(m);
        if n.dot(downstream_mm - c) < 0.0 {
            n = -n;
        }
        let (u, v) = n.any_orthonormal_pair();
        let mut best: Vec<Option<(f32, u32)>> = vec![None; RIM_BINS];
        for (i, p) in points.iter().enumerate() {
            let d = *p - c;
            let (x, y) = (d.dot(u), d.dot(v));
            let r = x.hypot(y);
            let b = bin_of(y.atan2(x));
            if best[b].is_none_or(|(br, _)| r > br) {
                best[b] = Some((r, i as u32));
            }
        }
        // an empty bin takes the nearest filled one (searching both ways)
        let filled: Vec<usize> = (0..RIM_BINS).filter(|b| best[*b].is_some()).collect();
        if filled.is_empty() {
            return None;
        }
        let rim_index = (0..RIM_BINS)
            .map(|b| {
                let near = filled
                    .iter()
                    .copied()
                    .min_by_key(|f| {
                        let d = (*f + RIM_BINS - b) % RIM_BINS;
                        d.min(RIM_BINS - d)
                    })
                    .unwrap_or(b);
                best[near].map_or(0, |x| x.1)
            })
            .collect();
        Some(Self {
            centre_mm: c,
            normal: n,
            u,
            v,
            rim_index,
        })
    }

    /// The rim curve of the disc whose current vertices are `points` (same order as at
    /// construction): [`RIM_BINS`] points.
    #[must_use]
    pub fn rim_points(&self, points: &[Vec3]) -> Vec<Vec3> {
        (0..RIM_BINS)
            .map(|b| {
                let mut s = Vec3::ZERO;
                for (k, w) in RIM_SMOOTH.iter().enumerate() {
                    let j = (b + RIM_BINS + k - 3) % RIM_BINS;
                    s += points[self.rim_index[j] as usize] * *w;
                }
                s / 64.0
            })
            .collect()
    }

    /// The rim parameter of a point: `b + t` such that the point of the rim polyline `rim` at
    /// [`Self::rim_at`]`(rim, b + t)` lies at the same azimuth about the centre and normal as `p`
    /// (the in-plane ray from the centre through `p` meets segment `b → b + 1` at fraction `t`).
    /// The rim is star-shaped about its disc's centre, so the segment is found by azimuth.
    #[must_use]
    pub fn param(&self, rim: &[Vec3], p: Vec3) -> f32 {
        let to2 = |x: Vec3| {
            let d = x - self.centre_mm;
            glam::Vec2::new(d.dot(self.u), d.dot(self.v))
        };
        let dir = to2(p).normalize_or(glam::Vec2::X);
        let mut best: Option<(f32, f32)> = None;
        for b in 0..RIM_BINS {
            let (a, c) = (to2(rim[b]), to2(rim[(b + 1) % RIM_BINS]));
            // solve a + (c − a) t = s · dir with s > 0, t ∈ [0, 1]
            let e = c - a;
            let den = dir.perp_dot(e);
            if den.abs() < 1e-12 {
                continue;
            }
            let t = -dir.perp_dot(a) / den;
            let s = a.perp_dot(e) / den;
            if s > 0.0 && (-1e-4..=1.0 + 1e-4).contains(&t) {
                let t = t.clamp(0.0, 1.0);
                // the nearest crossing to the centre wins (a non-star rim stays well defined)
                if best.is_none_or(|(bs, _)| s < bs) {
                    best = Some((s, b as f32 + t));
                }
            }
        }
        let f = best.map_or(0.0, |x| x.1);
        if f >= RIM_BINS as f32 {
            f - RIM_BINS as f32
        } else {
            f
        }
    }

    /// The point of the rim curve `rim` at parameter `phi` (linear between bins).
    #[must_use]
    pub fn rim_at(rim: &[Vec3], phi: f32) -> Vec3 {
        let i = (phi.floor() as usize) % RIM_BINS;
        let j = (i + 1) % RIM_BINS;
        let t = phi - phi.floor();
        rim[i] * (1.0 - t) + rim[j] * t
    }
}

fn bin_of(angle: f32) -> usize {
    ((angle / std::f32::consts::TAU).rem_euclid(1.0) * RIM_BINS as f32) as usize % RIM_BINS
}

/// Unit eigenvector of the smallest eigenvalue of a symmetric 3×3 matrix (cyclic Jacobi;
/// deterministic).
#[must_use]
pub fn smallest_eigenvector(m: Mat3) -> Vec3 {
    let mut a = [
        [m.x_axis.x, m.y_axis.x, m.z_axis.x],
        [m.x_axis.y, m.y_axis.y, m.z_axis.y],
        [m.x_axis.z, m.y_axis.z, m.z_axis.z],
    ]
    .map(|r| r.map(f64::from));
    let mut v = [[1.0f64, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let off = a[0][1].abs() + a[0][2].abs() + a[1][2].abs();
        if off < 1e-15 {
            break;
        }
        for (p, q) in [(0usize, 1usize), (0, 2), (1, 2)] {
            if a[p][q].abs() < 1e-300 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let t = if theta == 0.0 { 1.0 } else { t };
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for row in &mut a {
                let (akp, akq) = (row[p], row[q]);
                row[p] = c * akp - s * akq;
                row[q] = s * akp + c * akq;
            }
            let (rp, rq) = (a[p], a[q]);
            for (k, (apk, aqk)) in rp.into_iter().zip(rq).enumerate() {
                a[p][k] = c * apk - s * aqk;
                a[q][k] = s * apk + c * aqk;
            }
            for row in &mut v {
                let (vp, vq) = (row[p], row[q]);
                row[p] = c * vp - s * vq;
                row[q] = s * vp + c * vq;
            }
        }
    }
    let k = (0..3).min_by(|&i, &j| a[i][i].total_cmp(&a[j][j])).unwrap_or(0);
    Vec3::new(v[0][k] as f32, v[1][k] as f32, v[2][k] as f32).normalize_or(Vec3::Z)
}

// --- riding the beating heart ---------------------------------------------------------------------

/// Carries a valve's rest-frame shape with its valve disc as the heart moves: the least-squares
/// affine map of the disc's rest vertices onto their current positions, plus the rim's residual
/// (current rim minus the affine image of the rest rim) at each vertex's rim parameter, weighted
/// by [`AnimatedValve::follow_weight`]. An annulus vertex that lies on the rest rim therefore lands
/// exactly on the current rim; the free edge and the chordae move with the affine part only. A
/// pure function of (rest data, posed shape, current disc).
#[derive(Debug, Clone)]
pub struct DiscCarrier {
    rim: DiscRim,
    rest_disc: Vec<Vec3>,
    rest_centre: Vec3,
    gram_inv: Mat3,
    rest_rim: Vec<Vec3>,
    /// Rim parameter of each valve vertex at rest.
    phi: Vec<f32>,
    weight: Vec<f32>,
}

impl DiscCarrier {
    /// For `valve` on the disc whose rest vertices are `disc_rest` (normal toward `downstream_mm`).
    /// `None` when the disc is degenerate.
    #[must_use]
    pub fn new(valve: &AnimatedValve, disc_rest: &[Vec3], downstream_mm: Vec3) -> Option<Self> {
        let rim = DiscRim::new(disc_rest, downstream_mm)?;
        let rest_centre = disc_rest.iter().copied().sum::<Vec3>() / disc_rest.len() as f32;
        let mut g = Mat3::ZERO;
        for p in disc_rest {
            let q = *p - rest_centre;
            g += Mat3::from_cols(q * q.x, q * q.y, q * q.z);
        }
        let lambda = (g.x_axis.x + g.y_axis.y + g.z_axis.z) * 1e-8 + 1e-12;
        let gram_inv = (g + Mat3::from_diagonal(Vec3::splat(lambda))).inverse();
        if !gram_inv.is_finite() {
            return None;
        }
        let rest_rim = rim.rim_points(disc_rest);
        let phi = valve.rest_mm.iter().map(|p| rim.param(&rest_rim, *p)).collect();
        Some(Self {
            rim,
            rest_disc: disc_rest.to_vec(),
            rest_centre,
            gram_inv,
            rest_rim,
            phi,
            weight: valve.follow_weight.clone(),
        })
    }

    /// The disc rim this carrier uses.
    #[must_use]
    pub fn rim(&self) -> &DiscRim {
        &self.rim
    }

    /// The rest rim curve.
    #[must_use]
    pub fn rest_rim(&self) -> &[Vec3] {
        &self.rest_rim
    }

    /// `posed` (a rest-frame shape from [`AnimatedValve::pose_rest`]) carried by the disc whose
    /// current vertices are `disc_now` (same order and count as at construction) into `out`.
    pub fn carry(&self, posed: &[Vec3], disc_now: &[Vec3], out: &mut Vec<Vec3>) {
        let n = disc_now.len().max(1) as f32;
        let now_centre = disc_now.iter().copied().sum::<Vec3>() / n;
        let mut c = Mat3::ZERO;
        for (y, x) in disc_now.iter().zip(&self.rest_disc) {
            let (dy, dx) = (*y - now_centre, *x - self.rest_centre);
            c += Mat3::from_cols(dy * dx.x, dy * dx.y, dy * dx.z);
        }
        let a = c * self.gram_inv;
        let now_rim = self.rim.rim_points(disc_now);
        let res: Vec<Vec3> = now_rim
            .iter()
            .zip(&self.rest_rim)
            .map(|(r, r0)| *r - (now_centre + a * (*r0 - self.rest_centre)))
            .collect();
        out.clear();
        out.extend(
            posed
                .iter()
                .zip(self.phi.iter().zip(&self.weight))
                .map(|(p, (phi, w))| {
                    let base = now_centre + a * (*p - self.rest_centre);
                    if *w > 0.0 {
                        base + DiscRim::rim_at(&res, *phi) * *w
                    } else {
                        base
                    }
                }),
        );
    }
}

// --- drawing ----------------------------------------------------------------------------------------

/// Sides of a drawn chord (a prism).
const CHORD_SIDES: usize = 4;

/// The drawn form of a valve: the leaflet midsurface as a two-sided shell (one sheet offset by
/// half the leaflet thickness along each side's normal, the back sheet wound the other way, so
/// back-face culling shows each sheet from its own side), and each chord segment as a thin
/// four-sided prism. The topology is fixed; [`Self::fill`] writes positions and normals for a
/// posed shape (no new shader feature: an ordinary triangle mesh).
#[derive(Debug, Clone)]
pub struct ValveRenderMesh {
    leaflet_vertices: usize,
    triangles: Vec<[u32; 3]>,
    chordae: Vec<[u32; 2]>,
    half_thickness_mm: f32,
    chord_radius_mm: f32,
    /// The mesh with the rest shape (indices fixed; positions rewritten by `fill`).
    pub mesh: TriMesh,
    normal_acc: Vec<Vec3>,
}

impl ValveRenderMesh {
    /// The drawn form of `valve` with leaflets `thickness_mm` thick.
    #[must_use]
    pub fn new(valve: &AnimatedValve, thickness_mm: f32) -> Self {
        let nl = valve.leaflet_vertices as usize;
        let mut indices =
            Vec::with_capacity(valve.triangles.len() * 6 + valve.chordae.len() * CHORD_SIDES * 6);
        for t in &valve.triangles {
            indices.extend_from_slice(t);
        }
        for t in &valve.triangles {
            indices.extend([t[0], t[2], t[1]].map(|i| i + nl as u32));
        }
        let base = 2 * nl as u32;
        for s in 0..valve.chordae.len() as u32 {
            let o = base + s * 2 * CHORD_SIDES as u32;
            for k in 0..CHORD_SIDES as u32 {
                let k1 = (k + 1) % CHORD_SIDES as u32;
                let (a0, a1) = (o + k, o + k1);
                let (b0, b1) = (o + CHORD_SIDES as u32 + k, o + CHORD_SIDES as u32 + k1);
                indices.extend([a0, a1, b1, a0, b1, b0]);
            }
        }
        let nv = 2 * nl + valve.chordae.len() * 2 * CHORD_SIDES;
        let mut me = Self {
            leaflet_vertices: nl,
            triangles: valve.triangles.clone(),
            chordae: valve.chordae.clone(),
            half_thickness_mm: thickness_mm * 0.5,
            chord_radius_mm: valve.chordae_radius_mm,
            mesh: TriMesh {
                vertices: vec![
                    Vertex {
                        position: [0.0; 3],
                        normal: [0.0, 0.0, 1.0],
                    };
                    nv
                ],
                indices,
            },
            normal_acc: Vec::new(),
        };
        let mut v = std::mem::take(&mut me.mesh.vertices);
        me.fill(&valve.rest_mm, &mut v);
        me.mesh.vertices = v;
        me
    }

    /// Number of drawn vertices.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.mesh.vertices.len()
    }

    /// Write the drawn vertices of the shape `positions` (all valve vertices, as posed) into
    /// `out` (resized to [`Self::vertex_count`]).
    pub fn fill(&mut self, positions: &[Vec3], out: &mut Vec<Vertex>) {
        let nl = self.leaflet_vertices;
        out.resize(
            2 * nl + self.chordae.len() * 2 * CHORD_SIDES,
            Vertex {
                position: [0.0; 3],
                normal: [0.0, 0.0, 1.0],
            },
        );
        self.normal_acc.clear();
        self.normal_acc.resize(nl, Vec3::ZERO);
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| positions[i as usize]);
            let n = (b - a).cross(c - a);
            for i in t {
                self.normal_acc[*i as usize] += n;
            }
        }
        let h = self.half_thickness_mm;
        for (i, acc) in self.normal_acc.iter().enumerate() {
            let n = acc.normalize_or(Vec3::Z);
            let p = positions[i];
            out[i] = Vertex {
                position: (p + n * h).to_array(),
                normal: n.to_array(),
            };
            out[nl + i] = Vertex {
                position: (p - n * h).to_array(),
                normal: (-n).to_array(),
            };
        }
        let r = self.chord_radius_mm;
        for (s, seg) in self.chordae.iter().enumerate() {
            let (a, b) = (positions[seg[0] as usize], positions[seg[1] as usize]);
            let d = (b - a).normalize_or(Vec3::Z);
            let (e1, e2) = d.any_orthonormal_pair();
            let o = 2 * nl + s * 2 * CHORD_SIDES;
            for k in 0..CHORD_SIDES {
                let ang = std::f32::consts::TAU * k as f32 / CHORD_SIDES as f32;
                let radial = e1 * ang.cos() + e2 * ang.sin();
                let normal = radial.to_array();
                out[o + k] = Vertex {
                    position: (a + radial * r).to_array(),
                    normal,
                };
                out[o + CHORD_SIDES + k] = Vertex {
                    position: (b + radial * r).to_array(),
                    normal,
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-triangle "valve" with one chord and two keyframes.
    fn toy() -> AnimatedValve {
        let rest_mm = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 10.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(5.0, 5.0, -10.0),
        ];
        let off = |dz: i16| vec![[0, 0, 0], [0, 0, 0], [0, 0, dz], [0, 0, dz], [0, 0, 0]];
        AnimatedValve {
            name: "tricuspid".into(),
            leaflet_names: vec!["a".into(), "b".into()],
            rest_mm,
            leaflet_vertices: 4,
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            triangle_leaflet: vec![0, 1],
            vertex_leaflet: vec![0, 0, 0, 1, 1],
            chordae: vec![[4, 2]],
            chordae_radius_mm: 0.4,
            annulus: vec![0, 1],
            free_edge: vec![2, 3],
            follow_weight: vec![1.0, 1.0, 0.0, 0.0, 0.0],
            coaptation_mm: Vec3::new(5.0, 10.0, -2.0),
            quantum_mm: 0.01,
            fit_scale: 1.0,
            source_leaflet_area_mm2: vec![50.0, 50.0],
            keyframes: vec![
                ValveKeyframe {
                    closure: 0.25,
                    offsets: off(-100),
                },
                ValveKeyframe {
                    closure: 1.0,
                    offsets: off(-200),
                },
            ],
        }
    }

    #[test]
    fn encode_decode_round_trip_and_rejects_damage() {
        let v = toy();
        let bytes = v.encode();
        assert_eq!(AnimatedValve::decode(&bytes).expect("decodes"), v);
        assert_eq!(
            AnimatedValve::decode(&bytes[..bytes.len() - 1]),
            Err(ValveError::Truncated)
        );
        let mut bad = bytes.clone();
        bad[0] = b'X';
        assert_eq!(AnimatedValve::decode(&bad), Err(ValveError::Magic));
        let mut v2 = v.clone();
        v2.keyframes[1].closure = 0.2;
        assert!(
            AnimatedValve::decode(&v2.encode()).is_err(),
            "closures must increase"
        );
        let mut v3 = v;
        v3.triangles[0][0] = 4;
        assert!(
            AnimatedValve::decode(&v3.encode()).is_err(),
            "a triangle on a chordae vertex"
        );
    }

    #[test]
    fn opening_one_and_zero_are_the_first_and_last_keyframes() {
        let v = toy();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        v.pose_rest(1.0, &mut a);
        v.keyframe_positions(0, &mut b);
        assert_eq!(a, b);
        v.pose_rest(0.0, &mut a);
        v.keyframe_positions(2, &mut b);
        assert_eq!(a, b);
        // between: piecewise linear in the closure
        v.pose_rest(0.875, &mut a);
        assert!((a[2].z + 0.5).abs() < 1e-5, "{}", a[2].z);
        v.pose_rest(0.375, &mut a);
        assert!((a[2].z + 1.5).abs() < 1e-5, "{}", a[2].z);
        v.pose_rest(f32::NAN, &mut a);
        v.keyframe_positions(2, &mut b);
        assert_eq!(a, b, "NaN counts as shut");
    }

    #[test]
    fn jacobi_finds_the_smallest_axis() {
        let m = Mat3::from_diagonal(Vec3::new(9.0, 1.0, 4.0));
        let r = glam::Quat::from_rotation_y(0.4) * glam::Quat::from_rotation_x(0.3);
        let rm = Mat3::from_quat(r);
        let n = smallest_eigenvector(rm * m * rm.transpose());
        assert!((n.dot(rm * Vec3::Y).abs() - 1.0).abs() < 1e-5, "{n}");
    }

    #[test]
    fn carrier_puts_rim_points_on_the_moved_rim_and_is_rigid_for_a_rigid_motion() {
        // a flat disc of points
        let mut disc = Vec::new();
        for r in [2.0f32, 6.0, 10.0] {
            for k in 0..144 {
                let a = std::f32::consts::TAU * k as f32 / 144.0;
                disc.push(Vec3::new(r * a.cos(), r * a.sin(), 0.0));
                disc.push(Vec3::new(r * a.cos(), r * a.sin(), 1.0));
            }
        }
        let rim = DiscRim::new(&disc, Vec3::new(0.0, 0.0, -10.0)).unwrap();
        assert!(rim.normal.z < -0.99);
        let rr = rim.rim_points(&disc);
        let mut v = toy();
        // put vertex 0 on the rim at parameter 10.5
        v.rest_mm[0] = DiscRim::rim_at(&rr, 10.5);
        assert!((rim.param(&rr, v.rest_mm[0]) - 10.5).abs() < 1e-3);
        let car = DiscCarrier::new(&v, &disc, Vec3::new(0.0, 0.0, -10.0)).unwrap();
        // a rigid motion moves everything rigidly
        let q = glam::Quat::from_rotation_z(0.3);
        let t = Vec3::new(3.0, -2.0, 5.0);
        let moved: Vec<Vec3> = disc.iter().map(|p| q * *p + t).collect();
        let mut out = Vec::new();
        car.carry(&v.rest_mm, &moved, &mut out);
        for (o, p) in out.iter().zip(&v.rest_mm) {
            assert!(o.distance(q * *p + t) < 1e-3, "{o} vs {}", q * *p + t);
        }
        // a non-affine bulge of the rim: vertex 0 (weight 1) follows the rim exactly
        let bulged: Vec<Vec3> = disc
            .iter()
            .map(|p| *p + Vec3::new(0.0, 0.0, 0.2 * (p.x * 0.3).sin() * p.length() / 10.0))
            .collect();
        car.carry(&v.rest_mm, &bulged, &mut out);
        let now_rim = car.rim().rim_points(&bulged);
        let target = DiscRim::rim_at(&now_rim, car.rim().param(car.rest_rim(), v.rest_mm[0]));
        assert!(out[0].distance(target) < 1e-4, "{} vs {target}", out[0]);
    }

    #[test]
    fn render_mesh_has_both_sheets_and_the_chords() {
        let v = toy();
        let mut r = ValveRenderMesh::new(&v, 1.6);
        assert_eq!(r.vertex_count(), 2 * 4 + 2 * CHORD_SIDES);
        assert_eq!(r.mesh.indices.len(), 2 * 2 * 3 + CHORD_SIDES * 6);
        assert!(r.mesh.indices.iter().all(|i| (*i as usize) < r.vertex_count()));
        let mut out = Vec::new();
        r.fill(&v.rest_mm, &mut out);
        // the two sheets lie half the thickness either side of the midsurface
        assert!((out[0].position[2] - out[4].position[2]).abs() - 1.6 < 1e-5);
    }
}
