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
//! site's height and azimuth, the figure's skin is within 3 mm of V1–V6, V3R and V4R and within
//! 5 mm of the Mason–Likar arm sites. It is 1.55 m tall. Elsewhere the shapes differ (the figure's
//! back is ~25 mm flatter at V9 and its abdomen ~20 mm fuller at the iliac crests).

use crate::mesh::{TriMesh, Vertex};
use glam::Vec3;

use super::PackError;

/// Millimetres per source unit: the figure's height in the heart frame.
pub const BODY_HEIGHT_MM: f32 = 1551.18;
/// Heart-frame position of the source origin, mm.
const BODY_OFFSET_MM: Vec3 = Vec3::new(-52.94, -341.09, -9.55);

/// Heart-frame wrists of the registered figure (left, right), mm: the narrowest section of each
/// forearm above the hand. Where a distal arm electrode is drawn.
pub const BODY_WRISTS_MM: [Vec3; 2] = [Vec3::new(240.0, -330.0, -10.0), Vec3::new(-345.0, -330.0, -10.0)];
/// Heart-frame ankles of the registered figure (left, right), mm. Where a distal leg electrode is
/// drawn.
pub const BODY_ANKLES_MM: [Vec3; 2] = [Vec3::new(78.0, -1040.0, -35.0), Vec3::new(-183.0, -1040.0, -35.0)];

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
