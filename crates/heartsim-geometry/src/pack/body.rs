//! The human figure around the heart in the 3-D views and the Electrodes tab, in the heart frame.
//!
//! The figure (`human.glb`, a standing adult, ~8 k vertices) is the surface electrodes are shown
//! and dragged on. The standard sites in `electrodes.ron`, and with them the forward model, stay
//! those of the pack torso, so the ECG never depends on this file; the registration is chosen so
//! that the two agree where it matters.
//!
//! Source frame: +y up, the body faces +x, its left–right axis is z, height 1 unit. A standing body
//! facing +x with +y up has its left on −z (left = up × forward), so the rotation is
//! `heart = (−z, y, x)`, a proper rotation. Scale and offset were fitted, uniform scale plus
//! translation, to the default pack's KCL torso: on rays from the torso axis at each standard
//! site's height and azimuth, the figure's skin is within 3 mm of V1–V6, V3R, V4R, RA and LA. It
//! is 1.55 m tall. Elsewhere the shapes differ, and those standard sites are *not* on the figure's
//! skin: LL is 10 mm off, RL 22 mm inside the figure (the abdomen is fuller at the iliac crests),
//! V7 27 mm, V8 7 mm and V9 19 mm off (the back is flatter). The stored sites stay as they are;
//! the app draws those markers at the nearest skin point instead.

use crate::mesh::{TriMesh, Vertex};
use glam::Vec3;

use super::PackError;

/// Millimetres per source unit: the figure's height in the heart frame.
pub const BODY_HEIGHT_MM: f32 = 1551.18;
/// Heart-frame position of the source origin, mm.
const BODY_OFFSET_MM: Vec3 = Vec3::new(-52.94, -341.09, -9.55);

/// Heart-frame wrists of the registered figure (left, right), mm: points on the anterior skin of
/// each forearm above the hand. Where a distal arm electrode is drawn when it was not dropped at a
/// particular spot.
pub const BODY_WRISTS_MM: [Vec3; 2] = [Vec3::new(237.0, -330.0, 23.3), Vec3::new(-343.0, -330.0, 24.4)];
/// Heart-frame ankles of the registered figure (left, right), mm: points on the anterior skin just
/// above the malleoli. Where a distal leg electrode is drawn when it was not dropped at a
/// particular spot.
pub const BODY_ANKLES_MM: [Vec3; 2] = [Vec3::new(78.0, -1000.0, -12.9), Vec3::new(-185.0, -1000.0, -13.2)];

/// Sagittal midline of the registered figure (heart-frame x), mm.
pub const BODY_MIDLINE_X_MM: f32 = BODY_OFFSET_MM.x;

/// Which part of the figure a point on its skin belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyRegion {
    /// Head, neck, chest, abdomen and pelvis: where point electrodes record the local potential.
    Trunk,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

/// Lateral distance from the midline beyond which a point at height `y_mm` is on an arm, mm:
/// piecewise linear through the measured (y, d) pairs, constant outside them. Below about
/// y = −80 the arms hang free of the trunk and this runs down the middle of the gap; above it
/// it cuts off the deltoid and upper arm lateral to the chest wall.
#[must_use]
pub fn arm_boundary_mm(y_mm: f32) -> f32 {
    const B: [(f32, f32); 6] = [
        (-480.0, 234.0),
        (-400.0, 218.0),
        (-280.0, 202.0),
        (-200.0, 170.0),
        (-120.0, 156.0),
        (200.0, 156.0),
    ];
    if y_mm <= B[0].0 {
        return B[0].1;
    }
    for w in B.windows(2) {
        let ((y0, b0), (y1, b1)) = (w[0], w[1]);
        if y_mm <= y1 {
            return b0 + (b1 - b0) * (y_mm - y0) / (y1 - y0);
        }
    }
    B[5].1
}

/// The part of the registered figure a point `p_mm` of its skin is on.
///
/// A limb is a linear conductor: an electrode anywhere on an arm or leg records the potential of
/// that limb's junction with the trunk (Einthoven), so a drop on the hand, forearm, upper arm,
/// shin or thigh is a limb electrode, not a point on the chest. Rule (measured on the figure):
/// an arm is farther from the midline than [`arm_boundary_mm`] between y = −520 and 200; below
/// y = −440 (the groin is near −410; fingertips reach −500 but are caught by the arm rule
/// first) it is a leg; everything else, head included, is trunk. Left is the patient's left
/// (+x of the midline).
#[must_use]
pub fn body_region(p_mm: Vec3) -> BodyRegion {
    let left = p_mm.x > BODY_MIDLINE_X_MM;
    let d = (p_mm.x - BODY_MIDLINE_X_MM).abs();
    if p_mm.y > -520.0 && p_mm.y < 200.0 && d > arm_boundary_mm(p_mm.y) {
        if left {
            BodyRegion::LeftArm
        } else {
            BodyRegion::RightArm
        }
    } else if p_mm.y < -440.0 {
        if left {
            BodyRegion::LeftLeg
        } else {
            BodyRegion::RightLeg
        }
    } else {
        BodyRegion::Trunk
    }
}

/// Read the GLB `bytes` of the human figure and place it in the heart frame.
///
/// # Errors
/// When the file is not a readable GLB mesh.
pub fn read_human_body(bytes: &[u8]) -> Result<TriMesh, PackError> {
    let mut mesh = super::read_glb(bytes)?;
    let rot = |p: [f32; 3]| Vec3::new(-p[2], p[1], p[0]);
    for v in &mut mesh.vertices {
        *v = Vertex {
            position: (rot(v.position) * BODY_HEIGHT_MM + BODY_OFFSET_MM).into(),
            normal: rot(v.normal).into(),
        };
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::icosphere;

    /// Source +x (the way the figure faces) becomes heart-frame anterior, source −z the patient's
    /// left, and one source unit the figure's height.
    #[test]
    fn registration_rotates_and_scales() {
        let mut figure = icosphere(2);
        for v in &mut figure.vertices {
            let p = Vec3::from(v.position);
            v.position = Vec3::new(p.x * 0.1, p.y * 0.5, p.z * 0.2).into();
        }
        figure.recompute_normals();
        let body = read_human_body(&super::super::write_glb(&figure, "body")).unwrap();
        let (lo, hi) = body.bounds();
        assert!(((hi.y - lo.y) - BODY_HEIGHT_MM).abs() < 1.0, "{lo} {hi}");
        assert!(
            (hi.x - lo.x) > (hi.z - lo.z),
            "source z (0.2) is heart x: {lo} {hi}"
        );
        let front = body
            .vertices
            .iter()
            .max_by(|a, b| a.position[2].total_cmp(&b.position[2]))
            .unwrap();
        assert!(front.normal[2] > 0.9, "normals rotate with the positions");
    }
}
