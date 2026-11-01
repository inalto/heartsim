//! Procedural test geometry. The icosphere stands in for the heart until anatomy packs land, and
//! stays useful for unit tests and benchmarks (known vertex count, closed, convex).

use crate::mesh::{TriMesh, Vertex};
use glam::Vec3;
use std::collections::HashMap;

/// Unit icosphere with `subdivisions` levels (0 → 12 verts, 3 → 642, 5 → 10 242).
#[must_use]
pub fn icosphere(subdivisions: u32) -> TriMesh {
    let t = f32::midpoint(1.0, 5.0f32.sqrt());
    let mut pos: Vec<Vec3> = [
        (-1.0, t, 0.0),
        (1.0, t, 0.0),
        (-1.0, -t, 0.0),
        (1.0, -t, 0.0),
        (0.0, -1.0, t),
        (0.0, 1.0, t),
        (0.0, -1.0, -t),
        (0.0, 1.0, -t),
        (t, 0.0, -1.0),
        (t, 0.0, 1.0),
        (-t, 0.0, -1.0),
        (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| Vec3::new(x, y, z).normalize())
    .collect();
    let mut idx: Vec<u32> = vec![
        0, 11, 5, 0, 5, 1, 0, 1, 7, 0, 7, 10, 0, 10, 11, 1, 5, 9, 5, 11, 4, 11, 10, 2, 10, 7, 6, 7, 1, 8, 3,
        9, 4, 3, 4, 2, 3, 2, 6, 3, 6, 8, 3, 8, 9, 4, 9, 5, 2, 4, 11, 6, 2, 10, 8, 6, 7, 9, 8, 1,
    ];
    for _ in 0..subdivisions {
        let mut cache: HashMap<(u32, u32), u32> = HashMap::new();
        let mut next = Vec::with_capacity(idx.len() * 4);
        let mut midpoint = |a: u32, b: u32, pos: &mut Vec<Vec3>| -> u32 {
            let key = (a.min(b), a.max(b));
            *cache.entry(key).or_insert_with(|| {
                let m = ((pos[a as usize] + pos[b as usize]) * 0.5).normalize();
                pos.push(m);
                (pos.len() - 1) as u32
            })
        };
        for tri in idx.as_chunks::<3>().0 {
            let (a, b, c) = (tri[0], tri[1], tri[2]);
            let ab = midpoint(a, b, &mut pos);
            let bc = midpoint(b, c, &mut pos);
            let ca = midpoint(c, a, &mut pos);
            next.extend_from_slice(&[a, ab, ca, b, bc, ab, c, ca, bc, ab, bc, ca]);
        }
        idx = next;
    }
    let vertices = pos
        .into_iter()
        .map(|p| Vertex {
            position: p.to_array(),
            normal: p.to_array(),
        })
        .collect();
    TriMesh {
        vertices,
        indices: idx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icosphere_counts_follow_euler() {
        for (s, v, f) in [(0, 12, 20), (1, 42, 80), (3, 642, 1280)] {
            let m = icosphere(s);
            assert_eq!(m.vertex_count(), v);
            assert_eq!(m.triangle_count(), f);
            // closed genus-0 surface: V − E + F = 2 with E = 3F/2
            let (v, f) = (
                i64::try_from(m.vertex_count()).unwrap(),
                i64::try_from(f).unwrap(),
            );
            assert_eq!(v - (3 * f) / 2 + f, 2);
        }
    }

    #[test]
    fn normals_point_outward() {
        let mut m = icosphere(2);
        m.recompute_normals();
        for v in &m.vertices {
            let p = Vec3::from(v.position);
            assert!(p.dot(Vec3::from(v.normal)) > 0.95);
        }
    }
}
