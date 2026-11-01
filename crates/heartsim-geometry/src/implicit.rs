//! Implicit (signed-distance-like) shapes and a marching-tetrahedra polygoniser.
//!
//! The procedural anatomy is defined as implicit functions (negative inside, positive outside) so
//! that the same definition drives both cell sampling (point-in-wall tests, transmural depth) and
//! render-mesh extraction. Boolean composition is `min` (union), `max` (intersection) and
//! `max(a, -b)` (difference). The primitives are exact or near-exact distances so that gradients
//! are well behaved for normals.

use crate::mesh::{TriMesh, Vertex};
use glam::Vec3;
use std::collections::HashMap;

/// Exact signed distance to a sphere.
#[inline]
#[must_use]
pub fn sphere(p: Vec3, centre: Vec3, radius_mm: f32) -> f32 {
    (p - centre).length() - radius_mm
}

/// Near-exact signed distance to an axis-aligned ellipsoid (Quilez's bound; exact on the axes,
/// sign-correct everywhere).
#[inline]
#[must_use]
pub fn ellipsoid(p: Vec3, centre: Vec3, radii_mm: Vec3) -> f32 {
    let q = p - centre;
    let k0 = (q / radii_mm).length();
    if k0 < 1e-6 {
        return -radii_mm.min_element();
    }
    let k1 = (q / (radii_mm * radii_mm)).length();
    k0 * (k0 - 1.0) / k1
}

/// Exact signed distance to a capsule (segment `a`–`b` with radius).
#[inline]
#[must_use]
pub fn capsule(p: Vec3, a: Vec3, b: Vec3, radius_mm: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.length_squared()).clamp(0.0, 1.0);
    (pa - ba * h).length() - radius_mm
}

/// Signed distance to a torus with axis `axis` through `centre`.
#[inline]
#[must_use]
pub fn torus(p: Vec3, centre: Vec3, axis: Vec3, major_mm: f32, minor_mm: f32) -> f32 {
    let q = p - centre;
    let along = q.dot(axis);
    let radial = (q - axis * along).length();
    Vec3::new(radial - major_mm, along, 0.0).length() - minor_mm
}

/// Half-space `n·(p − o) ≤ 0` is inside.
#[inline]
#[must_use]
pub fn half_space(p: Vec3, origin: Vec3, normal: Vec3) -> f32 {
    (p - origin).dot(normal)
}

/// Superellipsoid `(|x/a|^r + |z/c|^r)^(t/r) + |y/b|^t − 1` (not a distance; zero set and sign are
/// what matter). `r` rounds the horizontal cross-section (2 = ellipse, →∞ = rectangle), `t`
/// flattens the top and bottom.
#[inline]
#[must_use]
pub fn superellipsoid(p: Vec3, centre: Vec3, half_extents: Vec3, r: f32, t: f32) -> f32 {
    let q = ((p - centre) / half_extents).abs();
    (q.x.powf(r) + q.z.powf(r)).powf(t / r) + q.y.powf(t) - 1.0
}

/// Union.
#[inline]
#[must_use]
pub fn union(a: f32, b: f32) -> f32 {
    a.min(b)
}
/// Intersection.
#[inline]
#[must_use]
pub fn intersect(a: f32, b: f32) -> f32 {
    a.max(b)
}
/// `a` minus `b`.
#[inline]
#[must_use]
pub fn subtract(a: f32, b: f32) -> f32 {
    a.max(-b)
}

/// Finite-difference gradient of `f` at `p`.
#[must_use]
pub fn gradient(f: &dyn Fn(Vec3) -> f32, p: Vec3, eps_mm: f32) -> Vec3 {
    let e = eps_mm;
    Vec3::new(
        f(p + Vec3::X * e) - f(p - Vec3::X * e),
        f(p + Vec3::Y * e) - f(p - Vec3::Y * e),
        f(p + Vec3::Z * e) - f(p - Vec3::Z * e),
    )
}

/// Extract the zero level set of `f` inside the box `[lo, hi]` with marching tetrahedra
/// (Freudenthal 6-tet decomposition of each voxel). Vertices are shared along lattice edges;
/// normals are the normalised implicit gradient, so they are smooth even where the triangulation
/// is coarse; triangles are wound so that normals point towards increasing `f` (outside).
///
/// The box should contain the whole surface with at least one voxel of margin; the sampling lattice
/// is clamped to a minimum of 2 points per axis.
#[must_use]
pub fn polygonise(f: &dyn Fn(Vec3) -> f32, lo: Vec3, hi: Vec3, voxel_mm: f32) -> TriMesh {
    // The six tetrahedra sharing the main diagonal 0–7; corners are bit-coded x=1, y=2, z=4.
    const TETS: [[usize; 4]; 6] = [
        [0, 1, 3, 7],
        [0, 1, 5, 7],
        [0, 2, 3, 7],
        [0, 2, 6, 7],
        [0, 4, 5, 7],
        [0, 4, 6, 7],
    ];

    let voxel = voxel_mm.max(1e-3);
    let n = ((hi - lo) / voxel).ceil().max(Vec3::ONE);
    let (nx, ny, nz) = (n.x as usize + 1, n.y as usize + 1, n.z as usize + 1);
    let at = |i: usize, j: usize, k: usize| lo + Vec3::new(i as f32, j as f32, k as f32) * voxel;
    let mut values = vec![0.0f32; nx * ny * nz];
    let tiny = 1e-4 * voxel;
    let idx = |i: usize, j: usize, k: usize| (i * ny + j) * nz + k;
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                // A sample exactly on the surface would make every edge from that lattice point
                // emit a distinct vertex at the same position (non-manifold); nudge it outside.
                let v = f(at(i, j, k));
                values[idx(i, j, k)] = if v.abs() < tiny { tiny } else { v };
            }
        }
    }

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut edge_cache: HashMap<(u32, u32), u32> = HashMap::new();
    let eps = 0.25 * voxel;

    for i in 0..nx - 1 {
        for j in 0..ny - 1 {
            for k in 0..nz - 1 {
                let mut corner_idx = [0usize; 8];
                let mut corner_val = [0.0f32; 8];
                for c in 0..8 {
                    let (ci, cj, ck) = (i + (c & 1), j + ((c >> 1) & 1), k + ((c >> 2) & 1));
                    corner_idx[c] = idx(ci, cj, ck);
                    corner_val[c] = values[corner_idx[c]];
                }
                // Fast reject: all corners on one side.
                let any_in = corner_val.iter().any(|v| *v < 0.0);
                let any_out = corner_val.iter().any(|v| *v >= 0.0);
                if !(any_in && any_out) {
                    continue;
                }
                for tet in &TETS {
                    let mut inside = [false; 4];
                    let mut count = 0;
                    for (t, &c) in tet.iter().enumerate() {
                        inside[t] = corner_val[c] < 0.0;
                        count += usize::from(inside[t]);
                    }
                    if count == 0 || count == 4 {
                        continue;
                    }
                    let mut edge_vertex = |ca: usize, cb: usize| -> u32 {
                        let (ga, gb) = (corner_idx[ca] as u32, corner_idx[cb] as u32);
                        let key = (ga.min(gb), ga.max(gb));
                        *edge_cache.entry(key).or_insert_with(|| {
                            let (va, vb) = (corner_val[ca], corner_val[cb]);
                            let t = (va / (va - vb)).clamp(0.0, 1.0);
                            let pa = at(i + (ca & 1), j + ((ca >> 1) & 1), k + ((ca >> 2) & 1));
                            let pb = at(i + (cb & 1), j + ((cb >> 1) & 1), k + ((cb >> 2) & 1));
                            let p = pa.lerp(pb, t);
                            let n = gradient(f, p, eps).normalize_or_zero();
                            vertices.push(Vertex {
                                position: p.to_array(),
                                normal: n.to_array(),
                            });
                            (vertices.len() - 1) as u32
                        })
                    };
                    if count == 1 || count == 3 {
                        let lone = (0..4).find(|&t| inside[t] == (count == 1)).unwrap_or(0);
                        let others: Vec<usize> = (0..4).filter(|&t| t != lone).collect();
                        let a = edge_vertex(tet[lone], tet[others[0]]);
                        let b = edge_vertex(tet[lone], tet[others[1]]);
                        let c = edge_vertex(tet[lone], tet[others[2]]);
                        push_oriented(&vertices, &mut indices, [a, b, c]);
                    } else {
                        let ins: Vec<usize> = (0..4).filter(|&t| inside[t]).collect();
                        let outs: Vec<usize> = (0..4).filter(|&t| !inside[t]).collect();
                        let q0 = edge_vertex(tet[ins[0]], tet[outs[0]]);
                        let q1 = edge_vertex(tet[ins[0]], tet[outs[1]]);
                        let q2 = edge_vertex(tet[ins[1]], tet[outs[1]]);
                        let q3 = edge_vertex(tet[ins[1]], tet[outs[0]]);
                        push_oriented(&vertices, &mut indices, [q0, q1, q2]);
                        push_oriented(&vertices, &mut indices, [q0, q2, q3]);
                    }
                }
            }
        }
    }
    TriMesh { vertices, indices }
}

/// Append triangle `[a, b, c]` wound so that its geometric normal agrees with the vertex normals.
fn push_oriented(vertices: &[Vertex], indices: &mut Vec<u32>, tri: [u32; 3]) {
    let [a, b, c] = tri;
    if a == b || b == c || a == c {
        return; // degenerate (two edge vertices coincided on a lattice edge)
    }
    let pa = Vec3::from(vertices[a as usize].position);
    let pb = Vec3::from(vertices[b as usize].position);
    let pc = Vec3::from(vertices[c as usize].position);
    let n = (pb - pa).cross(pc - pa);
    let g = Vec3::from(vertices[a as usize].normal)
        + Vec3::from(vertices[b as usize].normal)
        + Vec3::from(vertices[c as usize].normal);
    if n.dot(g) < 0.0 {
        indices.extend_from_slice(&[a, c, b]);
    } else {
        indices.extend_from_slice(&[a, b, c]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_mesh_is_closed_and_has_the_right_area_and_volume() {
        let r = 10.0;
        let f = |p: Vec3| sphere(p, Vec3::ZERO, r);
        let m = polygonise(&f, Vec3::splat(-12.0), Vec3::splat(12.0), 1.0);
        assert!(m.vertex_count() > 500);
        // Closed manifold: every edge used exactly twice, with opposite orientation.
        let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
        for t in m.indices.as_chunks::<3>().0 {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *edges.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { -1 };
            }
        }
        assert!(
            edges.values().all(|v| *v == 0),
            "mesh is not a closed oriented manifold"
        );
        // Area / volume within a few % of the analytic sphere.
        let (mut area, mut volume) = (0.0f32, 0.0f32);
        for t in m.indices.as_chunks::<3>().0 {
            let a = Vec3::from(m.vertices[t[0] as usize].position);
            let b = Vec3::from(m.vertices[t[1] as usize].position);
            let c = Vec3::from(m.vertices[t[2] as usize].position);
            area += 0.5 * (b - a).cross(c - a).length();
            volume += a.dot(b.cross(c)) / 6.0;
        }
        let area_exact = 4.0 * std::f32::consts::PI * r * r;
        let vol_exact = 4.0 / 3.0 * std::f32::consts::PI * r * r * r;
        assert!(
            (area / area_exact - 1.0).abs() < 0.05,
            "area {area} vs {area_exact}"
        );
        assert!(
            (volume / vol_exact - 1.0).abs() < 0.05,
            "volume {volume} vs {vol_exact}"
        );
        for v in &m.vertices {
            let p = Vec3::from(v.position);
            assert!((p.length() - r).abs() < 0.3);
            assert!(p.normalize().dot(Vec3::from(v.normal)) > 0.99);
        }
    }

    #[test]
    fn ellipsoid_distance_is_exact_on_axes_and_sign_correct() {
        let c = Vec3::new(1.0, 2.0, 3.0);
        let r = Vec3::new(3.0, 5.0, 2.0);
        assert!((ellipsoid(c + Vec3::X * 4.0, c, r) - 1.0).abs() < 1e-4);
        assert!((ellipsoid(c + Vec3::Y * 5.0, c, r)).abs() < 1e-4);
        assert!(ellipsoid(c, c, r) < 0.0);
        assert!(ellipsoid(c + Vec3::new(1.0, 1.0, 0.5), c, r) < 0.0);
    }

    #[test]
    fn csg_helpers() {
        let f = |p: Vec3| subtract(sphere(p, Vec3::ZERO, 10.0), sphere(p, Vec3::ZERO, 8.0));
        assert!(f(Vec3::X * 9.0) < 0.0);
        assert!(f(Vec3::X * 7.0) > 0.0);
        assert!(f(Vec3::X * 11.0) > 0.0);
        assert!(capsule(Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO, Vec3::X * 10.0, 1.0) < 0.0);
        assert!(torus(Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO, Vec3::Y, 5.0, 1.0) < 0.0);
        assert!(superellipsoid(Vec3::ZERO, Vec3::ZERO, Vec3::ONE, 2.5, 6.0) < 0.0);
        assert!(superellipsoid(Vec3::X * 1.1, Vec3::ZERO, Vec3::ONE, 2.5, 6.0) > 0.0);
        assert!(half_space(Vec3::Y, Vec3::ZERO, Vec3::Y) > 0.0);
        assert!(union(1.0, -1.0) < 0.0 && intersect(1.0, -1.0) > 0.0);
    }
}
