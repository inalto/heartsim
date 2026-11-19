//! The human figure shown around the heart in the 3-D views, registered to the heart frame.
//!
//! The figure (`human.glb`, a full standing adult, ~8 k vertices) is a display mesh only: electrode
//! sites, picking and the forward model stay on the pack's own `torso` surface, so the ECG never
//! depends on this file. The registration therefore only has to make the figure look right around
//! the pack torso and the heart.
//!
//! Source frame: +y up, the body faces +x, its left-right axis is z, height 1 unit. A standing body
//! facing +x with +y up has its left on −z (left = up × forward), so
//! `heart = (−z, y, x) × scale` — a proper rotation. The scale makes the chest as deep as the pack
//! torso's (a 1.6 m figure; the pack torso is a smaller-than-average adult), and the figure is
//! shifted so the nipple line (72 % of the height) is at heart-frame y = 0, the sternum is flush
//! with the pack torso's anterior surface and the body midline coincides with the pack torso's.

use crate::mesh::{TriMesh, Vertex};
use glam::Vec3;

use super::PackError;

/// Millimetres per source unit: the figure's height in the heart frame.
pub const BODY_HEIGHT_MM: f32 = 1616.0;
/// Fraction of the body height (from the soles) of the heart-frame origin, the 4th–5th intercostal
/// space at the nipple line.
const HEART_LEVEL: f32 = 0.72;

/// Read the GLB `bytes` of the human figure and place it in the heart frame around `torso`.
///
/// # Errors
/// When the file is not a readable GLB mesh.
pub fn read_human_body(bytes: &[u8], torso: &TriMesh) -> Result<TriMesh, PackError> {
    let mut mesh = super::read_glb(bytes)?;
    let rot = |p: [f32; 3]| [-p[2], p[1], p[0]];
    let (lo, hi) = mesh.bounds();
    let level = lo.y + (hi.y - lo.y) * HEART_LEVEL;
    let scale = BODY_HEIGHT_MM / (hi.y - lo.y);
    for v in &mut mesh.vertices {
        let p = Vec3::from(rot([v.position[0], v.position[1] - level, v.position[2]])) * scale;
        *v = Vertex {
            position: p.into(),
            normal: rot(v.normal),
        };
    }
    // anchor on the chest slice (heart level ± 6 mm): midline of the torso's width, and its front
    // against the figure's front at its own midline (arms and belly excluded)
    let chest = |m: &TriMesh| {
        m.vertices
            .iter()
            .map(|v| Vec3::from(v.position))
            .filter(|p| p.y.abs() < 6.0)
            .collect::<Vec<_>>()
    };
    let t = chest(torso);
    let b: Vec<Vec3> = chest(&mesh).into_iter().filter(|p| p.x.abs() < 20.0).collect();
    if !t.is_empty() && !b.is_empty() {
        let fold = |pts: &[Vec3], f: fn(f32, f32) -> f32, init: f32, axis: usize| {
            pts.iter().map(|p| p[axis]).fold(init, f)
        };
        let mid_x = f32::midpoint(
            fold(&t, f32::min, f32::INFINITY, 0),
            fold(&t, f32::max, f32::NEG_INFINITY, 0),
        );
        let dz = fold(&t, f32::max, f32::NEG_INFINITY, 2) - fold(&b, f32::max, f32::NEG_INFINITY, 2);
        for v in &mut mesh.vertices {
            v.position[0] += mid_x;
            v.position[2] += dz;
        }
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::icosphere;

    /// A stand-in figure: a unit-high capsule-ish blob facing +x must end up 1.6 m tall in the
    /// heart frame, with +x of the source on the heart-frame anterior axis.
    #[test]
    fn registration_rotates_scales_and_anchors() {
        let mut figure = icosphere(2);
        for v in &mut figure.vertices {
            let p = Vec3::from(v.position);
            v.position = Vec3::new(p.x * 0.1, p.y * 0.5, p.z * 0.2).into();
        }
        figure.recompute_normals();
        let torso = icosphere(2); // unit sphere at the origin: front at z = 1, midline x = 0
        let glb = super::super::write_glb(&figure, "body");
        let body = read_human_body(&glb, &torso).unwrap();
        let (lo, hi) = body.bounds();
        assert!(((hi.y - lo.y) - BODY_HEIGHT_MM).abs() < 1.0, "{lo} {hi}");
        // source z (left–right, 0.2) became heart x, source x (depth, 0.1) heart z
        assert!((hi.x - lo.x) > (hi.z - lo.z), "{lo} {hi}");
    }
}
