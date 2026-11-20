use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// GPU-ready vertex layout (position, normal). Per-vertex scalar state (V_m, activation time)
/// lives in a separate buffer so it can be updated every frame without touching geometry.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, Serialize, Deserialize, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TriMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl TriMesh {
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Axis-aligned bounds (min, max).
    #[must_use]
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for v in &self.vertices {
            let p = Vec3::from(v.position);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        (lo, hi)
    }

    /// Recompute smooth vertex normals from triangle geometry (area-weighted).
    pub fn recompute_normals(&mut self) {
        let mut acc = vec![Vec3::ZERO; self.vertices.len()];
        for t in self.indices.as_chunks::<3>().0 {
            let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
            let pa = Vec3::from(self.vertices[a].position);
            let pb = Vec3::from(self.vertices[b].position);
            let pc = Vec3::from(self.vertices[c].position);
            let n = (pb - pa).cross(pc - pa);
            acc[a] += n;
            acc[b] += n;
            acc[c] += n;
        }
        for (v, n) in self.vertices.iter_mut().zip(acc) {
            v.normal = n.normalize_or_zero().to_array();
        }
    }

    /// The point of the surface nearest to `p` (brute force over the triangles; `None` for a mesh
    /// without triangles). For snapping a point to the skin of a body mesh a few thousand
    /// triangles large; not for per-frame use on big meshes.
    #[must_use]
    pub fn closest_point(&self, p: Vec3) -> Option<Vec3> {
        let v = |i: u32| Vec3::from(self.vertices[i as usize].position);
        self.indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| closest_point_on_triangle(p, v(t[0]), v(t[1]), v(t[2])))
            .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
    }

    /// Positions as a flat `Vec<Vec3>` (for spatial queries).
    #[must_use]
    pub fn positions(&self) -> Vec<Vec3> {
        self.vertices.iter().map(|v| Vec3::from(v.position)).collect()
    }
}

/// The point of triangle `abc` nearest to `p` (Ericson, *Real-Time Collision Detection* §5.1.5):
/// the vertex, edge or interior point, whichever Voronoi region of the triangle `p` falls in.
#[must_use]
pub fn closest_point_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
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
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return b + (c - b) * w;
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> TriMesh {
        let v = |x: f32, y: f32| Vertex {
            position: [x, y, 0.0],
            normal: [0.0, 0.0, 1.0],
        };
        TriMesh {
            vertices: vec![v(0.0, 0.0), v(10.0, 0.0), v(10.0, 10.0), v(0.0, 10.0)],
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }

    /// Above the interior the nearest point is the foot of the perpendicular; beyond an edge or a
    /// corner it is on that edge or at that corner; an empty mesh has none.
    #[test]
    fn closest_point_regions() {
        let m = square();
        let near = |p: Vec3, want: Vec3| {
            let got = m.closest_point(p).unwrap();
            assert!(got.distance(want) < 1e-4, "{p}: {got} vs {want}");
        };
        near(Vec3::new(3.0, 4.0, 5.0), Vec3::new(3.0, 4.0, 0.0));
        near(Vec3::new(5.0, -7.0, 2.0), Vec3::new(5.0, 0.0, 0.0));
        near(Vec3::new(14.0, 13.0, -1.0), Vec3::new(10.0, 10.0, 0.0));
        near(Vec3::new(-2.0, 5.0, 0.0), Vec3::new(0.0, 5.0, 0.0));
        assert_eq!(TriMesh::default().closest_point(Vec3::ZERO), None);
    }
}
