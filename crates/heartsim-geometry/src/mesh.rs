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

    /// Positions as a flat `Vec<Vec3>` (for spatial queries).
    #[must_use]
    pub fn positions(&self) -> Vec<Vec3> {
        self.vertices.iter().map(|v| Vec3::from(v.position)).collect()
    }
}
