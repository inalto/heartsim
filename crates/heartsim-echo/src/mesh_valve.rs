//! The measured tricuspid valve (brief B35): the pack's [`AnimatedValve`] posed at the valve
//! opening the schedule gives ([`crate::valve_openings`]) and carried by the deformed valve disc,
//! for the 3-D heart and for the echo cross-section alike, so both show the same valve open or
//! shut at the same instant.
//!
//! * **Pose** ([`MeshValve::pose_rest`]): `opening` (`1` open … `0` shut) selects and interpolates
//!   the keyframes (the closure `1 − opening`, piecewise linear between the two keyframes whose
//!   closures bracket it). No clock and no engine state of its own: the opening is the one value
//!   the heart sounds, the wall motion and the echo already share.
//! * **Ride** ([`MeshValve::pose`]): the rest-frame shape carried by the tricuspid disc of the
//!   evaluated motion frame (its affine motion plus the rim's own, which the annulus takes in
//!   full; see [`DiscCarrier`]), so the annulus stays on the beating disc. Without a moving frame
//!   (wall motion off, reduced motion) the valve keeps the rest disc and still opens and shuts.
//! * **Echo** ([`PosedMeshValve::contains`]): a rest point is leaflet when it lies within half the
//!   designed leaflet thickness of the posed midsurface (the cross-section works in rest
//!   coordinates, like the designed cones it replaces for this valve).
//!
//! It is one donor's valve fitted to this heart and moved by a load sequence replayed against the
//! designed timing; no pathology is implied.

use crate::designed::DesignedParams;
use crate::model::{MotionFrame, MotionModel};
use glam::Vec3;
use heartsim_ecg::Valve;
use heartsim_geometry::mesh::closest_point_on_triangle;
use heartsim_geometry::{AnatomyPack, AnimatedValve, DiscCarrier};

/// Cell size of the posed valve's triangle grid, mm.
const GRID_MM: f32 = 1.5;

/// A pack valve ready to pose (see the module docs).
#[derive(Debug, Clone)]
pub struct MeshValve {
    pub valve_id: Valve,
    pub data: AnimatedValve,
    carrier: DiscCarrier,
    /// The disc's vertex indices in the `valve_planes` part.
    disc_vertices: Vec<u32>,
    /// Half the designed leaflet thickness, mm.
    half_thickness_mm: f32,
    /// Bounds of every keyframe's leaflets (rest frame), widened by the half thickness.
    lo: Vec3,
    hi: Vec3,
}

impl MeshValve {
    /// The pack's tricuspid valve on the motion model's tricuspid disc; `None` when the pack has
    /// no valve or the model no discs.
    #[must_use]
    pub fn tricuspid(pack: &AnatomyPack, model: &MotionModel, designed: &DesignedParams) -> Option<Self> {
        let data = pack.valve("tricuspid")?.clone();
        Self::new(data, Valve::Tricuspid, model, designed)
    }

    /// `data` on the disc of `valve_id`.
    #[must_use]
    pub fn new(
        data: AnimatedValve,
        valve_id: Valve,
        model: &MotionModel,
        designed: &DesignedParams,
    ) -> Option<Self> {
        let disc = model.valve_discs().into_iter().find(|d| d.valve == valve_id)?;
        let rest = model.part_rest("valve_planes")?;
        let disc_rest: Vec<Vec3> = disc
            .vertices
            .iter()
            .map(|&i| rest.get(i as usize).map(|v| Vec3::from(v.position)))
            .collect::<Option<Vec<Vec3>>>()?;
        // downstream: the coaptation point lies on the ventricular side of the annulus (the same
        // side as the RV blood centroid the bake oriented the rim by)
        let carrier = DiscCarrier::new(&data, &disc_rest, data.coaptation_mm)?;
        let h = designed.av_leaflet_thickness_mm * 0.5;
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        let mut pos = Vec::new();
        for k in 0..data.keyframe_count() {
            data.keyframe_positions(k, &mut pos);
            for p in &pos[..data.leaflet_vertices as usize] {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
        }
        Some(Self {
            valve_id,
            carrier,
            disc_vertices: disc.vertices.clone(),
            half_thickness_mm: h,
            lo: lo - Vec3::splat(h),
            hi: hi + Vec3::splat(h),
            data,
        })
    }

    /// Bounds of the leaflets at any opening (rest frame, mm), widened by the half thickness.
    #[must_use]
    pub fn bounds_mm(&self) -> (Vec3, Vec3) {
        (self.lo, self.hi)
    }

    /// Half the designed leaflet thickness, mm.
    #[must_use]
    pub fn half_thickness_mm(&self) -> f32 {
        self.half_thickness_mm
    }

    /// The carrier (tests: the rim it puts the annulus on).
    #[must_use]
    pub fn carrier(&self) -> &DiscCarrier {
        &self.carrier
    }

    /// The disc's vertex indices in the `valve_planes` part.
    #[must_use]
    pub fn disc_vertices(&self) -> &[u32] {
        &self.disc_vertices
    }

    /// The rest-frame shape at `opening` ([`AnimatedValve::pose_rest`]).
    pub fn pose_rest(&self, opening: f32, out: &mut Vec<Vec3>) {
        self.data.pose_rest(opening, out);
    }

    /// The disc's current vertices in `frame` (its `valve_planes` part), or `None` without one.
    #[must_use]
    pub fn disc_now(&self, frame: &MotionFrame) -> Option<Vec<Vec3>> {
        let part = frame.part("valve_planes")?;
        self.disc_vertices
            .iter()
            .map(|&i| part.vertices.get(i as usize).map(|v| Vec3::from(v.position)))
            .collect()
    }

    /// The valve at `opening` riding the disc of `frame` (heart frame, mm), into `out`; with
    /// `frame == None` (the wall at rest) the rest-frame shape itself. A pure function of the
    /// rest data, the opening and the frame: the same instant gives the same bits.
    pub fn pose(
        &self,
        opening: f32,
        frame: Option<&MotionFrame>,
        scratch: &mut Vec<Vec3>,
        out: &mut Vec<Vec3>,
    ) {
        match frame.and_then(|f| self.disc_now(f)) {
            Some(disc) => {
                self.pose_rest(opening, scratch);
                self.carrier.carry(scratch, &disc, out);
            }
            None => self.pose_rest(opening, out),
        }
    }
}

/// The valve posed at one instant for the cross-section: its rest-frame shape and a grid of its
/// triangles for [`Self::contains`]. Rebuilt by [`Self::update`] whenever the opening changes.
#[derive(Debug, Clone, Default)]
pub struct PosedMeshValve {
    opening_bits: Option<u32>,
    positions: Vec<Vec3>,
    origin: Vec3,
    dims: [usize; 3],
    /// Compressed rows: `cell_start[c]..cell_start[c + 1]` in `cell_tris`.
    cell_start: Vec<u32>,
    cell_tris: Vec<u32>,
    /// Per triangle, its first vertex and unit normal: a point farther than the half thickness
    /// from the plane is rejected before the closest-point test.
    planes: Vec<(Vec3, Vec3)>,
    lo: Vec3,
    hi: Vec3,
    half_thickness_mm: f32,
}

impl PosedMeshValve {
    /// Pose `valve` at `opening` (no work when the opening is unchanged).
    pub fn update(&mut self, valve: &MeshValve, opening: f32) {
        if self.opening_bits == Some(opening.to_bits()) && !self.positions.is_empty() {
            return;
        }
        self.opening_bits = Some(opening.to_bits());
        valve.pose_rest(opening, &mut self.positions);
        let h = valve.half_thickness_mm;
        self.half_thickness_mm = h;
        let tris = &valve.data.triangles;
        self.planes.clear();
        self.planes.extend(tris.iter().map(|t| {
            let [a, b, c] = t.map(|i| self.positions[i as usize]);
            (a, (b - a).cross(c - a).normalize_or_zero())
        }));
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for t in tris {
            for &i in t {
                lo = lo.min(self.positions[i as usize]);
                hi = hi.max(self.positions[i as usize]);
            }
        }
        lo -= Vec3::splat(h);
        hi += Vec3::splat(h);
        self.lo = lo;
        self.hi = hi;
        self.origin = lo;
        let ext = ((hi - lo) / GRID_MM).ceil().max(Vec3::ONE);
        self.dims = [ext.x as usize, ext.y as usize, ext.z as usize];
        let cells = self.dims[0] * self.dims[1] * self.dims[2];
        let cell_range = |a: Vec3, b: Vec3, o: Vec3, d: [usize; 3]| {
            let c = |p: Vec3| {
                let r = ((p - o) / GRID_MM).floor().max(Vec3::ZERO);
                [0, 1, 2].map(|k| (r[k] as usize).min(d[k] - 1))
            };
            (c(a), c(b))
        };
        let mut counts = vec![0u32; cells + 1];
        let mut ranges = Vec::with_capacity(tris.len());
        for t in tris {
            let [a, b, c] = t.map(|i| self.positions[i as usize]);
            let (r0, r1) = cell_range(
                a.min(b).min(c) - Vec3::splat(h),
                a.max(b).max(c) + Vec3::splat(h),
                self.origin,
                self.dims,
            );
            ranges.push((r0, r1));
            for z in r0[2]..=r1[2] {
                for y in r0[1]..=r1[1] {
                    for x in r0[0]..=r1[0] {
                        counts[x + self.dims[0] * (y + self.dims[1] * z) + 1] += 1;
                    }
                }
            }
        }
        for c in 1..counts.len() {
            counts[c] += counts[c - 1];
        }
        let mut fill = counts.clone();
        self.cell_tris.clear();
        self.cell_tris.resize(*counts.last().unwrap_or(&0) as usize, 0);
        for (ti, (r0, r1)) in ranges.iter().enumerate() {
            for z in r0[2]..=r1[2] {
                for y in r0[1]..=r1[1] {
                    for x in r0[0]..=r1[0] {
                        let c = x + self.dims[0] * (y + self.dims[1] * z);
                        self.cell_tris[fill[c] as usize] = ti as u32;
                        fill[c] += 1;
                    }
                }
            }
        }
        self.cell_start = counts;
    }

    /// The posed rest-frame positions.
    #[must_use]
    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    /// Bounds of the posed leaflets widened by the half thickness (rest frame).
    #[must_use]
    pub fn bounds_mm(&self) -> (Vec3, Vec3) {
        (self.lo, self.hi)
    }

    /// Whether rest point `x` lies within half the designed thickness of a posed leaflet triangle.
    #[must_use]
    pub fn contains(&self, valve: &MeshValve, x: Vec3) -> bool {
        if self.positions.is_empty() || !(x.cmpge(self.lo).all() && x.cmple(self.hi).all()) {
            return false;
        }
        let rel = ((x - self.origin) / GRID_MM).floor();
        let idx = |k: usize, v: f32| (v.max(0.0) as usize).min(self.dims[k] - 1);
        let c = idx(0, rel.x) + self.dims[0] * (idx(1, rel.y) + self.dims[1] * idx(2, rel.z));
        let h = self.half_thickness_mm;
        let h2 = h * h;
        let tris = &valve.data.triangles;
        self.cell_tris[self.cell_start[c] as usize..self.cell_start[c + 1] as usize]
            .iter()
            .any(|&t| {
                let (p, n) = self.planes[t as usize];
                // a degenerate triangle (zero normal) always goes on to the exact test
                if (x - p).dot(n).abs() > h {
                    return false;
                }
                let [a, b, cc] = tris[t as usize].map(|i| self.positions[i as usize]);
                closest_point_on_triangle(x, a, b, cc).distance_squared(x) <= h2
            })
    }
}
