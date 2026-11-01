//! CPU ray casting against layer meshes. Brute force over triangles with an AABB early-out per
//! layer: at the < 100 k triangles of the anatomy pack this is well under a millisecond, and it
//! runs only on a mouse event, never per frame.

use glam::Vec3;

/// Result of [`crate::SceneRenderer::pick`]: the nearest visible surface point under the cursor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// Layer that was hit.
    pub layer: crate::LayerId,
    /// Index of the triangle within the layer's index buffer (`indices[3 * triangle..]`).
    pub triangle: u32,
    /// World-space (heart frame, mm) hit position.
    pub position: Vec3,
    /// Unit geometric normal of the triangle, flipped to face the viewer.
    pub normal: Vec3,
    /// Distance along the ray from its origin.
    pub distance: f32,
}

/// Möller–Trumbore ray/triangle intersection, double sided. Returns the ray parameter `t > 0`.
#[must_use]
pub fn ray_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    const EPS: f32 = 1e-9;
    let e1 = b - a;
    let e2 = c - a;
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < EPS {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 1e-6).then_some(t)
}

/// Slab test: does the ray touch the box `[lo, hi]` at some `t > 0`?
#[must_use]
pub fn ray_aabb(origin: Vec3, dir: Vec3, lo: Vec3, hi: Vec3) -> bool {
    let inv = Vec3::ONE / dir;
    let t0 = (lo - origin) * inv;
    let t1 = (hi - origin) * inv;
    let tmin = t0.min(t1).max_element();
    let tmax = t0.max(t1).min_element();
    tmax >= tmin.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_hit_and_miss() {
        let (a, b, c) = (
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let t = ray_triangle(Vec3::new(0.0, 0.0, 5.0), -Vec3::Z, a, b, c).expect("hit");
        assert!((t - 5.0).abs() < 1e-5);
        assert!(ray_triangle(Vec3::new(2.0, 0.0, 5.0), -Vec3::Z, a, b, c).is_none());
        assert!(
            ray_triangle(Vec3::new(0.0, 0.0, 5.0), Vec3::Z, a, b, c).is_none(),
            "triangle behind the ray origin"
        );
    }

    #[test]
    fn aabb_slab() {
        let (lo, hi) = (Vec3::splat(-1.0), Vec3::splat(1.0));
        assert!(ray_aabb(Vec3::new(0.0, 0.0, 5.0), -Vec3::Z, lo, hi));
        assert!(!ray_aabb(Vec3::new(3.0, 0.0, 5.0), -Vec3::Z, lo, hi));
        assert!(!ray_aabb(Vec3::new(0.0, 0.0, 5.0), Vec3::Z, lo, hi));
    }
}
