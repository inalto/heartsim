//! Procedural adult torso and the standard electrode sites, in the heart frame.
//!
//! The torso is a superellipsoid (rounded-rectangular cross-section, flattened shoulders and
//! pelvis) placed so that the heart frame origin — the ventricular centroid — sits where an adult
//! heart does: about 35 mm left of the midline, in the anterior half of the thorax, at the level of
//! the fifth intercostal space. Electrode sites are found by shooting a ray from the torso axis at
//! the site's height and azimuth and intersecting the surface, so every site lies exactly on the
//! mesh. Standard positions follow the AHA/ACC recommendations (Kligfield et al. 2007): V1/V2 in
//! the 4th intercostal space either side of the sternum, V4 in the 5th space at the mid-clavicular
//! line, V3 midway, V5 anterior axillary and V6 mid-axillary at the V4 level; V7–V9 continue the
//! V6 level posteriorly; V3R/V4R mirror V3/V4. Limb electrodes are at the Mason–Likar torso
//! positions (infraclavicular fossae, iliac crests) which is what a torso forward model needs.

use crate::electrodes::ElectrodeSet;
use crate::implicit::{polygonise, superellipsoid};
use crate::mesh::TriMesh;
use glam::Vec3;

/// Torso surface plus its parameters (so sites can be re-projected after edits).
#[derive(Debug, Clone)]
pub struct Torso {
    pub mesh: TriMesh,
    pub centre_mm: Vec3,
    pub half_extents_mm: Vec3,
    pub electrodes: ElectrodeSet,
}

/// Torso centre in the heart frame (mm): midline is 35 mm to the patient's right of the
/// ventricular centroid and 35 mm posterior; the mid-torso is 60 mm below it.
pub const TORSO_CENTRE_MM: Vec3 = Vec3::new(-35.0, -60.0, -35.0);
/// Half extents: 175 mm half-width, 320 mm half-height, 115 mm half-depth.
pub const TORSO_HALF_EXTENTS_MM: Vec3 = Vec3::new(175.0, 320.0, 115.0);
const CROSS_SECTION_EXPONENT: f32 = 2.5;
const VERTICAL_EXPONENT: f32 = 6.0;

/// Signed implicit of the torso (negative inside).
#[must_use]
pub fn torso_implicit(p: Vec3) -> f32 {
    superellipsoid(
        p,
        TORSO_CENTRE_MM,
        TORSO_HALF_EXTENTS_MM,
        CROSS_SECTION_EXPONENT,
        VERTICAL_EXPONENT,
    )
}

/// Point on the torso surface at height `y_mm` (heart frame) and azimuth `azimuth_deg`
/// (0° = anterior midline, positive towards the patient's left, 180° = posterior).
#[must_use]
pub fn torso_surface_point(y_mm: f32, azimuth_deg: f32) -> Vec3 {
    let (s, c) = azimuth_deg.to_radians().sin_cos();
    let dir = Vec3::new(s, 0.0, c);
    let origin = Vec3::new(TORSO_CENTRE_MM.x, y_mm, TORSO_CENTRE_MM.z);
    // bisection on the ray parameter: f(origin) < 0 inside, f(far) > 0
    let (mut lo, mut hi) = (0.0f32, 2.0 * TORSO_HALF_EXTENTS_MM.max_element());
    for _ in 0..40 {
        let mid = f32::midpoint(lo, hi);
        if torso_implicit(origin + dir * mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    origin + dir * f32::midpoint(lo, hi)
}

/// Standard and additional electrode sites as `(name, height_mm, azimuth_deg)`.
pub const ELECTRODE_SITES: [(&str, f32, f32); 15] = [
    ("RA", 190.0, -60.0),
    ("LA", 190.0, 60.0),
    ("RL", -300.0, -45.0),
    ("LL", -300.0, 45.0),
    ("V1", 15.0, -12.0),
    ("V2", 15.0, 12.0),
    ("V3", 0.0, 32.0),
    ("V4", -15.0, 52.0),
    ("V5", -15.0, 72.0),
    ("V6", -15.0, 90.0),
    ("V7", -15.0, 108.0),
    ("V8", -15.0, 140.0),
    ("V9", -15.0, 168.0),
    ("V3R", 0.0, -32.0),
    ("V4R", -15.0, -52.0),
];

/// Electrode set on the procedural torso.
#[must_use]
pub fn procedural_electrodes() -> ElectrodeSet {
    let mut set = ElectrodeSet {
        frame: "heart frame: mm, origin = ventricular centroid, +x patient left, +y cranial, +z anterior"
            .to_string(),
        sites: std::collections::BTreeMap::new(),
    };
    for (name, y, az) in ELECTRODE_SITES {
        set.set(name, torso_surface_point(y, az));
    }
    set
}

/// Procedural torso mesh at the given voxel size (8–10 mm is plenty for a body surface).
#[must_use]
pub fn procedural_torso(voxel_mm: f32) -> Torso {
    let margin = Vec3::splat(2.0 * voxel_mm);
    let lo = TORSO_CENTRE_MM - TORSO_HALF_EXTENTS_MM - margin;
    let hi = TORSO_CENTRE_MM + TORSO_HALF_EXTENTS_MM + margin;
    let mesh = polygonise(&torso_implicit, lo, hi, voxel_mm);
    Torso {
        mesh,
        centre_mm: TORSO_CENTRE_MM,
        half_extents_mm: TORSO_HALF_EXTENTS_MM,
        electrodes: procedural_electrodes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_lie_on_the_surface_in_sensible_places() {
        let e = procedural_electrodes();
        assert!(e.has_standard_ten());
        for (name, p) in &e.sites {
            assert!(torso_implicit(Vec3::from(*p)).abs() < 1e-3, "{name} off surface");
        }
        let v1 = e.get("V1").unwrap();
        let v2 = e.get("V2").unwrap();
        let v6 = e.get("V6").unwrap();
        let v8 = e.get("V8").unwrap();
        assert!(v1.z > 60.0 && v2.z > 60.0, "V1/V2 anterior");
        assert!(v1.x < v2.x, "V1 right of V2");
        assert!(v6.x > 120.0, "V6 lateral");
        assert!(v8.z < -80.0, "V8 posterior");
        assert!(e.get("LL").unwrap().y < -250.0);
        assert!(e.get("RA").unwrap().x < e.get("LA").unwrap().x);
        assert!(e.get("V3R").unwrap().x < v1.x);
    }

    #[test]
    fn torso_mesh_is_a_closed_body() {
        let t = procedural_torso(20.0);
        assert!(t.mesh.vertex_count() > 300);
        let (lo, hi) = t.mesh.bounds();
        assert!(lo.y < -350.0 && hi.y > 230.0);
    }
}
