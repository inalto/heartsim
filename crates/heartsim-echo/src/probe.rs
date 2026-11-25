//! The transducer on the figure's skin: its pose, the local frame and the image plane it cuts
//! (brief B31, item 6; research note §4.2).
//!
//! **Pose.** A skin point and its outward normal, then three angles (Mitchell et al. 2019's
//! probe terms) and the sector:
//!
//! * the **base frame** at the skin point: `n` the inward normal, `c` the head-ward tangent (the
//!   cranial direction projected on the skin plane), `l = n × c` (toward the patient's left on
//!   the front of the chest);
//! * **rotation** φ turns the index marker about the beam: at 0 it points to the head (12
//!   o'clock), at +90° to the patient's left (3 o'clock seen on the front of the chest);
//!   marker `a₁ = c cos φ + l sin φ`, elevation `e₁ = n × a₁`;
//! * **tilt** ψ (fan) about the array axis `a₁`: the beam leaves the normal toward `e₁`, the image
//!   plane moves perpendicular to itself;
//! * **rock** ρ in the image plane, about the elevation axis: the beam swings toward the marker.
//!
//! The beam is `b = cos ρ (n cos ψ + e₁ sin ψ) + a₁ sin ρ`, the marker `a = a₁ cos ρ − (n cos ψ +
//! e₁ sin ψ) sin ρ`, the elevation `e = a × b`… made orthonormal by construction. Contact is lost
//! when `b` leaves [`DesignedParams::contact_limit_deg`] of `n`: the image is then empty.
//!
//! **Image lines** fan from the skin point in the plane (b, a): line `i` of `L` at angle
//! `θᵢ = ((i + ½)/L − ½) · sector` from the beam, positive toward the marker, so the marker side is
//! the last line (the screen's right in the usual adult display).
//!
//! The patient is **supine** in this phase (left lateral decubitus is a later decision).

use crate::designed::DesignedParams;
use glam::Vec3;
use heartsim_geometry::TriMesh;

/// Default imaging depth, mm. **Designed** (16 cm shows the whole adult heart from the apex).
pub const DEFAULT_DEPTH_MM: f32 = 160.0;
/// Default sector width, degrees. **Designed** (phased arrays reach about 90°).
pub const DEFAULT_SECTOR_DEG: f32 = 75.0;
/// Depth range, mm.
pub const DEPTH_RANGE_MM: (f32, f32) = (40.0, 300.0);
/// Sector-width range, degrees.
pub const SECTOR_RANGE_DEG: (f32, f32) = (30.0, 90.0);
/// Tilt and rock are clamped to ± this, degrees (contact is lost long before).
pub const ANGLE_LIMIT_DEG: f32 = 85.0;

/// Where the transducer is and how it is held. Any values are accepted; [`Self::clamped`] (applied
/// by every consumer) brings them into range and replaces non-finite numbers, so a pose never
/// yields NaN.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbePose {
    /// The point of contact on the figure's skin, heart frame, mm.
    pub skin_mm: Vec3,
    /// Outward unit normal of the skin there.
    pub normal: Vec3,
    /// Fan about the array axis, degrees (positive toward the elevation side `n × marker`).
    pub tilt_deg: f32,
    /// Swing in the image plane, degrees (positive toward the marker).
    pub rock_deg: f32,
    /// Index-marker rotation about the beam, degrees: 0 toward the head, +90 toward the patient's
    /// left (clockwise seen from outside on the front of the chest). Wrapped to `[-180, 180)`.
    pub rotation_deg: f32,
    /// Imaging depth, mm.
    pub depth_mm: f32,
    /// Sector width, degrees.
    pub sector_deg: f32,
}

impl Default for ProbePose {
    fn default() -> Self {
        Self {
            skin_mm: Vec3::new(0.0, 0.0, 100.0),
            normal: Vec3::Z,
            tilt_deg: 0.0,
            rock_deg: 0.0,
            rotation_deg: 0.0,
            depth_mm: DEFAULT_DEPTH_MM,
            sector_deg: DEFAULT_SECTOR_DEG,
        }
    }
}

/// The probe's orthonormal frame and the plane it images.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbeFrame {
    /// Sector apex (the skin point), mm.
    pub apex_mm: Vec3,
    /// Central beam direction, into the body.
    pub beam: Vec3,
    /// In-plane direction of the index marker (perpendicular to the beam).
    pub marker: Vec3,
    /// Normal of the image plane (`marker × beam`).
    pub elevation: Vec3,
    /// Inward skin normal at the apex.
    pub inward: Vec3,
    /// Angle between the beam and the inward normal, degrees.
    pub beam_from_normal_deg: f32,
    /// Whether the probe has contact (the beam within the contact limit).
    pub contact: bool,
}

fn finite_or(x: f32, d: f32) -> f32 {
    if x.is_finite() { x } else { d }
}

/// Head-ward tangent of the skin plane with inward normal `n`.
fn cranial_tangent(n: Vec3) -> Vec3 {
    let t = Vec3::Y - n * n.dot(Vec3::Y);
    if t.length_squared() > 1e-6 {
        return t.normalize();
    }
    // on a horizontal skin patch (top of the shoulder) "head-ward" falls back to anterior
    let t = Vec3::Z - n * n.dot(Vec3::Z);
    t.normalize_or(Vec3::X)
}

impl ProbePose {
    /// The pose with every field finite and in range: unit normal (else +z), tilt and rock in
    /// ±[`ANGLE_LIMIT_DEG`], rotation wrapped to `[-180, 180)`, depth and sector clamped.
    #[must_use]
    pub fn clamped(&self) -> Self {
        // a normal already unit to within float rounding is kept as is, so that clamping is
        // idempotent (normalising a unit vector again can move it by an ulp)
        let normal = if self.normal.is_finite() {
            if (self.normal.length_squared() - 1.0).abs() <= 1e-6 {
                self.normal
            } else {
                self.normal.normalize_or(Vec3::Z)
            }
        } else {
            Vec3::Z
        };
        let skin_mm = if self.skin_mm.is_finite() {
            self.skin_mm
        } else {
            Self::default().skin_mm
        };
        let rot = finite_or(self.rotation_deg, 0.0);
        Self {
            skin_mm,
            normal,
            tilt_deg: finite_or(self.tilt_deg, 0.0).clamp(-ANGLE_LIMIT_DEG, ANGLE_LIMIT_DEG),
            rock_deg: finite_or(self.rock_deg, 0.0).clamp(-ANGLE_LIMIT_DEG, ANGLE_LIMIT_DEG),
            // wrapped only when outside the range, so clamping twice changes nothing
            rotation_deg: if (-180.0..180.0).contains(&rot) {
                rot
            } else {
                (rot + 180.0).rem_euclid(360.0) - 180.0
            },
            depth_mm: finite_or(self.depth_mm, DEFAULT_DEPTH_MM).clamp(DEPTH_RANGE_MM.0, DEPTH_RANGE_MM.1),
            sector_deg: finite_or(self.sector_deg, DEFAULT_SECTOR_DEG)
                .clamp(SECTOR_RANGE_DEG.0, SECTOR_RANGE_DEG.1),
        }
    }

    /// The probe frame (of the clamped pose) with the contact limit `contact_limit_deg`.
    #[must_use]
    pub fn frame(&self, contact_limit_deg: f32) -> ProbeFrame {
        let p = self.clamped();
        let n = -p.normal;
        let c = cranial_tangent(n);
        let l = n.cross(c);
        let (sf, cf) = p.rotation_deg.to_radians().sin_cos();
        let a1 = c * cf + l * sf;
        let e1 = n.cross(a1);
        let (st, ct) = p.tilt_deg.to_radians().sin_cos();
        let b2 = n * ct + e1 * st;
        let (sr, cr) = p.rock_deg.to_radians().sin_cos();
        let beam = (b2 * cr + a1 * sr).normalize();
        let marker = (a1 * cr - b2 * sr).normalize();
        let elevation = marker.cross(beam).normalize();
        let angle = beam.dot(n).clamp(-1.0, 1.0).acos().to_degrees();
        ProbeFrame {
            apex_mm: p.skin_mm,
            beam,
            marker,
            elevation,
            inward: n,
            beam_from_normal_deg: angle,
            contact: angle <= contact_limit_deg,
        }
    }

    /// The marker's clock position on the chest, hours in `[0, 12)` (0 = 12 o'clock = toward the
    /// head, 3 = toward the patient's left).
    #[must_use]
    pub fn clock_hours(&self) -> f32 {
        (self.clamped().rotation_deg / 30.0).rem_euclid(12.0)
    }

    /// The pose at `skin_mm` (outward `normal`) whose beam points at `target_mm` (or as close as
    /// the contact limit allows, keeping 5° in hand) and whose marker turns toward `marker_dir`
    /// projected on the skin, with this pose's depth and sector.
    #[must_use]
    pub fn aimed(
        skin_mm: Vec3,
        normal: Vec3,
        target_mm: Vec3,
        marker_dir: Vec3,
        contact_limit_deg: f32,
        depth_mm: f32,
        sector_deg: f32,
    ) -> Self {
        let n = -normal.normalize_or(Vec3::Z);
        let c = cranial_tangent(n);
        let l = n.cross(c);
        // rotation from the marker direction on the skin plane
        let m = marker_dir - n * marker_dir.dot(n);
        let rot = m.dot(l).atan2(m.dot(c));
        let a1 = c * rot.cos() + l * rot.sin();
        let e1 = n.cross(a1);
        // beam, kept inside the contact limit
        let mut b = (target_mm - skin_mm).normalize_or(n);
        let max = (contact_limit_deg - 5.0).max(0.0).to_radians();
        let ang = b.dot(n).clamp(-1.0, 1.0).acos();
        if ang > max {
            let side = (b - n * b.dot(n)).normalize_or(e1);
            b = n * max.cos() + side * max.sin();
        }
        let rock = b.dot(a1).clamp(-1.0, 1.0).asin();
        let b2 = (b - a1 * rock.sin()) / rock.cos().max(1e-6);
        let tilt = b2.dot(e1).atan2(b2.dot(n));
        Self {
            skin_mm,
            normal: -n,
            tilt_deg: tilt.to_degrees(),
            rock_deg: rock.to_degrees(),
            rotation_deg: rot.to_degrees(),
            depth_mm,
            sector_deg,
        }
        .clamped()
    }
}

impl ProbeFrame {
    /// Unit direction of image line `i` of `lines` for a sector of `sector_deg`.
    #[must_use]
    pub fn line_direction(&self, i: usize, lines: usize, sector_deg: f32) -> Vec3 {
        let f = (i as f32 + 0.5) / lines.max(1) as f32 - 0.5;
        let (s, c) = (f * sector_deg).to_radians().sin_cos();
        self.beam * c + self.marker * s
    }
}

/// The skin the probe slides on: the pack's human figure when it has one, else its torso.
#[derive(Debug, Clone)]
pub struct Skin {
    tris: Vec<[Vec3; 3]>,
    normals: Vec<Vec3>,
    areas: Vec<f32>,
    centroids: Vec<Vec3>,
    normal_radius_mm: f32,
}

impl Skin {
    /// The skin of `mesh` (outward-wound triangles).
    #[must_use]
    pub fn new(mesh: &TriMesh, d: &DesignedParams) -> Self {
        let pos = |i: u32| Vec3::from(mesh.vertices[i as usize].position);
        let mut tris = Vec::with_capacity(mesh.indices.len() / 3);
        let mut normals = Vec::new();
        let mut areas = Vec::new();
        let mut centroids = Vec::new();
        for t in mesh.indices.as_chunks::<3>().0 {
            let tri = [pos(t[0]), pos(t[1]), pos(t[2])];
            let c = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
            let a = c.length() * 0.5;
            if a < 1e-9 {
                continue;
            }
            tris.push(tri);
            normals.push(c / (2.0 * a));
            areas.push(a);
            centroids.push((tri[0] + tri[1] + tri[2]) / 3.0);
        }
        Self {
            tris,
            normals,
            areas,
            centroids,
            normal_radius_mm: d.skin_normal_radius_mm,
        }
    }

    /// The nearest skin point to `p_mm` and the outward normal there (averaged over the probe's
    /// footprint, so the facets of the mesh do not jerk the frame). `None` for an empty mesh.
    #[must_use]
    pub fn place(&self, p_mm: Vec3) -> Option<(Vec3, Vec3)> {
        let mut best: Option<(f32, Vec3)> = None;
        for tri in &self.tris {
            let q = closest_on_triangle(p_mm, tri);
            let d = q.distance_squared(p_mm);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, q));
            }
        }
        let q = best?.1;
        Some((q, self.normal_at(q)))
    }

    /// Outward normal at skin point `q`, area-weighted over the footprint.
    #[must_use]
    pub fn normal_at(&self, q: Vec3) -> Vec3 {
        let r2 = self.normal_radius_mm * self.normal_radius_mm;
        let mut acc = Vec3::ZERO;
        let mut nearest = (f32::INFINITY, Vec3::Z);
        for ((c, n), a) in self.centroids.iter().zip(&self.normals).zip(&self.areas) {
            let d = c.distance_squared(q);
            if d <= r2 {
                acc += *n * *a;
            }
            if d < nearest.0 {
                nearest = (d, *n);
            }
        }
        acc.normalize_or(nearest.1)
    }

    /// The farthest crossing of the ray `origin + t·dir` (t > 0) with the skin, and its normal:
    /// from inside the body, the skin point the ray leaves through.
    #[must_use]
    pub fn ray_exit(&self, origin: Vec3, dir: Vec3) -> Option<(Vec3, Vec3)> {
        let mut best: Option<f32> = None;
        for tri in &self.tris {
            if let Some(t) = ray_triangle(origin, dir, tri)
                && best.is_none_or(|b| t > b)
            {
                best = Some(t);
            }
        }
        let q = origin + dir * best?;
        Some((q, self.normal_at(q)))
    }

    /// The nearest crossing (t > 0), e.g. from outside the body inward.
    #[must_use]
    pub fn ray_first(&self, origin: Vec3, dir: Vec3) -> Option<(Vec3, Vec3)> {
        let mut best: Option<f32> = None;
        for tri in &self.tris {
            if let Some(t) = ray_triangle(origin, dir, tri)
                && best.is_none_or(|b| t < b)
            {
                best = Some(t);
            }
        }
        let q = origin + dir * best?;
        Some((q, self.normal_at(q)))
    }
}

/// Möller–Trumbore, both faces, `t > 1e-4`.
fn ray_triangle(o: Vec3, d: Vec3, tri: &[Vec3; 3]) -> Option<f32> {
    let e1 = tri[1] - tri[0];
    let e2 = tri[2] - tri[0];
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - tri[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 1e-4).then_some(t)
}

/// Closest point of a triangle to `p` (Ericson, *Real-Time Collision Detection* §5.1.5).
fn closest_on_triangle(p: Vec3, t: &[Vec3; 3]) -> Vec3 {
    let (a, b, c) = (t[0], t[1], t[2]);
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_is_orthonormal_and_rotation_wraps() {
        let normals = [
            Vec3::Z,
            Vec3::new(0.3, 0.2, 0.9),
            Vec3::new(-0.7, 0.1, 0.4),
            Vec3::Y,
        ];
        for n in normals {
            for tilt in [-80.0, -30.0, 0.0, 17.0, 60.0] {
                for rock in [-70.0, 0.0, 25.0] {
                    for rot in [-170.0, -45.0, 0.0, 90.0, 400.0] {
                        let p = ProbePose {
                            normal: n,
                            tilt_deg: tilt,
                            rock_deg: rock,
                            rotation_deg: rot,
                            ..ProbePose::default()
                        };
                        let f = p.frame(60.0);
                        for v in [f.beam, f.marker, f.elevation] {
                            assert!((v.length() - 1.0).abs() < 1e-4, "{v}");
                        }
                        assert!(f.beam.dot(f.marker).abs() < 1e-4);
                        assert!(f.beam.dot(f.elevation).abs() < 1e-4);
                        assert!(f.marker.dot(f.elevation).abs() < 1e-4);
                        let q = ProbePose {
                            rotation_deg: rot + 360.0,
                            ..p
                        }
                        .frame(60.0);
                        assert!(q.beam.distance(f.beam) < 1e-4 && q.marker.distance(f.marker) < 1e-4);
                    }
                }
            }
        }
    }

    #[test]
    fn contact_limit_and_nan() {
        let p = ProbePose {
            tilt_deg: 59.0,
            ..ProbePose::default()
        };
        assert!(p.frame(60.0).contact);
        let p = ProbePose {
            tilt_deg: 61.0,
            ..ProbePose::default()
        };
        assert!(!p.frame(60.0).contact);
        let p = ProbePose {
            skin_mm: Vec3::NAN,
            normal: Vec3::ZERO,
            tilt_deg: f32::NAN,
            rock_deg: f32::INFINITY,
            rotation_deg: f32::NAN,
            depth_mm: f32::NAN,
            sector_deg: -5.0,
        };
        let f = p.frame(60.0);
        assert!(f.beam.is_finite() && f.marker.is_finite() && f.apex_mm.is_finite());
        let c = p.clamped();
        assert_eq!(c.depth_mm, DEFAULT_DEPTH_MM);
        assert_eq!(c.sector_deg, SECTOR_RANGE_DEG.0);
    }

    #[test]
    fn conventions() {
        // on the front of the chest: rotation 0 → marker to the head, 90 → patient's left
        let f = ProbePose::default().frame(60.0);
        assert!(f.beam.distance(-Vec3::Z) < 1e-5);
        assert!(f.marker.distance(Vec3::Y) < 1e-5);
        let f = ProbePose {
            rotation_deg: 90.0,
            ..ProbePose::default()
        }
        .frame(60.0);
        assert!(f.marker.distance(Vec3::X) < 1e-5, "{}", f.marker);
        // rock swings the beam toward the marker
        let f = ProbePose {
            rock_deg: 20.0,
            ..ProbePose::default()
        }
        .frame(60.0);
        assert!(f.beam.y > 0.3);
        // aiming reproduces the target direction and the marker side
        let target = Vec3::new(20.0, -30.0, 0.0);
        let p = ProbePose::aimed(
            Vec3::new(0.0, 0.0, 100.0),
            Vec3::Z,
            target,
            Vec3::X,
            60.0,
            160.0,
            75.0,
        );
        let f = p.frame(60.0);
        assert!(f.beam.dot((target - Vec3::new(0.0, 0.0, 100.0)).normalize()) > 0.9999);
        assert!(f.marker.x > 0.9);
    }
}
