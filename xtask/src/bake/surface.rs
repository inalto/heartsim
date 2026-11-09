//! Triangle-surface tools for the bake: compacting boundary faces into a `TriMesh`, Taubin
//! smoothing, quadric-error decimation (Garland & Heckbert 1997), swept tubes and lobes for the
//! vessels, capping of open rims, ray casting for electrode placement.

use glam::{Mat4, Vec3, Vec4};
use heartsim_geometry::{TriMesh, Vertex};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Build a mesh from oriented faces over a shared node array, keeping only referenced nodes.
#[must_use]
pub fn faces_to_mesh(points: &[Vec3], faces: &[[u32; 3]]) -> TriMesh {
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut vertices = Vec::new();
    let mut indices = Vec::with_capacity(faces.len() * 3);
    for f in faces {
        for n in f {
            let idx = *map.entry(*n).or_insert_with(|| {
                vertices.push(Vertex {
                    position: points[*n as usize].to_array(),
                    normal: [0.0; 3],
                });
                (vertices.len() - 1) as u32
            });
            indices.push(idx);
        }
    }
    let mut mesh = TriMesh { vertices, indices };
    mesh.recompute_normals();
    mesh
}

/// Concatenate meshes.
#[must_use]
pub fn merge(meshes: &[TriMesh]) -> TriMesh {
    let mut out = TriMesh::default();
    for m in meshes {
        let base = out.vertices.len() as u32;
        out.vertices.extend_from_slice(&m.vertices);
        out.indices.extend(m.indices.iter().map(|i| i + base));
    }
    out
}

/// Apply a point transform to positions and a rotation to normals.
pub fn transform(mesh: &mut TriMesh, f: impl Fn(Vec3) -> Vec3, rot: glam::Quat) {
    for v in &mut mesh.vertices {
        v.position = f(Vec3::from(v.position)).to_array();
        v.normal = (rot * Vec3::from(v.normal)).normalize_or_zero().to_array();
    }
}

fn adjacency(mesh: &TriMesh) -> Vec<Vec<u32>> {
    let mut adj = vec![Vec::new(); mesh.vertices.len()];
    for t in mesh.indices.as_chunks::<3>().0 {
        for i in 0..3 {
            let a = t[i] as usize;
            let b = t[(i + 1) % 3];
            if !adj[a].contains(&b) {
                adj[a].push(b);
            }
            let b = b as usize;
            if !adj[b].contains(&t[i]) {
                adj[b].push(t[i]);
            }
        }
    }
    adj
}

/// Taubin λ|μ smoothing: removes the facet noise of a 1 mm tetrahedral boundary without
/// shrinking (λ = 0.5, μ = −0.53, `passes` × 2 steps).
pub fn taubin_smooth(mesh: &mut TriMesh, passes: usize) {
    let adj = adjacency(mesh);
    let mut pos: Vec<Vec3> = mesh.positions();
    for _ in 0..passes {
        for factor in [0.5f32, -0.53] {
            let next: Vec<Vec3> = pos
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    if adj[i].is_empty() {
                        return *p;
                    }
                    let mean = adj[i].iter().map(|j| pos[*j as usize]).sum::<Vec3>() / adj[i].len() as f32;
                    *p + (mean - *p) * factor
                })
                .collect();
            pos = next;
        }
    }
    for (v, p) in mesh.vertices.iter_mut().zip(pos) {
        v.position = p.to_array();
    }
    mesh.recompute_normals();
}

// ---------------------------------------------------------------- quadric decimation ----------

#[derive(Clone, Copy, Default)]
struct Quadric([f32; 10]); // symmetric 4×4: a² ab ac ad b² bc bd c² cd d²

impl Quadric {
    fn plane(n: Vec3, d: f32, w: f32) -> Self {
        let (a, b, c) = (n.x, n.y, n.z);
        Self([
            a * a * w,
            a * b * w,
            a * c * w,
            a * d * w,
            b * b * w,
            b * c * w,
            b * d * w,
            c * c * w,
            c * d * w,
            d * d * w,
        ])
    }
    fn add(&self, o: &Self) -> Self {
        let mut q = [0.0; 10];
        for (x, (a, b)) in q.iter_mut().zip(self.0.iter().zip(&o.0)) {
            *x = a + b;
        }
        Self(q)
    }
    fn error(&self, p: Vec3) -> f32 {
        let q = &self.0;
        let (x, y, z) = (p.x, p.y, p.z);
        q[0] * x * x
            + 2.0 * q[1] * x * y
            + 2.0 * q[2] * x * z
            + 2.0 * q[3] * x
            + q[4] * y * y
            + 2.0 * q[5] * y * z
            + 2.0 * q[6] * y
            + q[7] * z * z
            + 2.0 * q[8] * z
            + q[9]
    }
    /// Position minimising the error, if the quadric is not degenerate.
    fn optimum(&self) -> Option<Vec3> {
        let q = &self.0;
        let m = Mat4::from_cols(
            Vec4::new(q[0], q[1], q[2], 0.0),
            Vec4::new(q[1], q[4], q[5], 0.0),
            Vec4::new(q[2], q[5], q[7], 0.0),
            Vec4::new(q[3], q[6], q[8], 1.0),
        );
        let det = m.determinant();
        if det.abs() < 1e-9 || !det.is_finite() {
            return None;
        }
        let v = m.inverse() * Vec4::new(0.0, 0.0, 0.0, 1.0);
        let p = v.truncate();
        p.is_finite().then_some(p)
    }
}

#[derive(PartialEq)]
struct Candidate {
    cost: f32,
    a: u32,
    b: u32,
    stamp: u32,
    target: Vec3,
}
impl Eq for Candidate {}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Candidate {
    fn cmp(&self, o: &Self) -> Ordering {
        // min-heap on cost, deterministic tie-break
        o.cost
            .total_cmp(&self.cost)
            .then_with(|| o.a.cmp(&self.a))
            .then_with(|| o.b.cmp(&self.b))
    }
}

/// Collapse edges until the mesh has at most `target_triangles` triangles. Boundary edges are
/// held by heavy perpendicular planes, collapses that flip a triangle are rejected, and the
/// result is compacted and re-normalled. Deterministic.
pub fn decimate(mesh: &mut TriMesh, target_triangles: usize) {
    let nv = mesh.vertices.len();
    let mut pos: Vec<Vec3> = mesh.positions();
    let mut faces: Vec<[u32; 3]> = mesh.indices.as_chunks::<3>().0.to_vec();
    if faces.len() <= target_triangles || nv == 0 {
        return;
    }
    let mut alive_face = vec![true; faces.len()];
    let mut vfaces: Vec<Vec<u32>> = vec![Vec::new(); nv];
    for (fi, f) in faces.iter().enumerate() {
        for v in f {
            vfaces[*v as usize].push(fi as u32);
        }
    }
    // quadrics
    let mut quad = vec![Quadric::default(); nv];
    let mut edge_count: HashMap<(u32, u32), u32> = HashMap::new();
    for f in &faces {
        let (a, b, c) = (pos[f[0] as usize], pos[f[1] as usize], pos[f[2] as usize]);
        let n = (b - a).cross(c - a);
        let area = n.length() * 0.5;
        if area <= 0.0 {
            continue;
        }
        let n = n / (2.0 * area);
        let q = Quadric::plane(n, -n.dot(a), area);
        for v in f {
            quad[*v as usize] = quad[*v as usize].add(&q);
        }
        for i in 0..3 {
            let (x, y) = (f[i], f[(i + 1) % 3]);
            *edge_count.entry((x.min(y), x.max(y))).or_default() += 1;
        }
    }
    // boundary edges: add a plane perpendicular to the (single) face, through the edge
    for (&(x, y), &n) in &edge_count {
        if n != 1 {
            continue;
        }
        let Some(&fi) = vfaces[x as usize]
            .iter()
            .find(|fi| faces[**fi as usize].contains(&y))
        else {
            continue;
        };
        let f = faces[fi as usize];
        let (a, b, c) = (pos[f[0] as usize], pos[f[1] as usize], pos[f[2] as usize]);
        let fn_ = (b - a).cross(c - a).normalize_or_zero();
        let e = (pos[y as usize] - pos[x as usize]).normalize_or_zero();
        let pn = fn_.cross(e).normalize_or_zero();
        let q = Quadric::plane(pn, -pn.dot(pos[x as usize]), 1000.0);
        quad[x as usize] = quad[x as usize].add(&q);
        quad[y as usize] = quad[y as usize].add(&q);
    }
    let mut stamp = vec![0u32; nv];
    let mut heap = BinaryHeap::new();
    let push =
        |heap: &mut BinaryHeap<Candidate>, a: u32, b: u32, quad: &[Quadric], pos: &[Vec3], stamp: &[u32]| {
            let q = quad[a as usize].add(&quad[b as usize]);
            let (pa, pb) = (pos[a as usize], pos[b as usize]);
            let mid = (pa + pb) * 0.5;
            let mut best = (q.error(mid), mid);
            for c in [pa, pb] {
                let e = q.error(c);
                if e < best.0 {
                    best = (e, c);
                }
            }
            if let Some(o) = q.optimum() {
                let e = q.error(o);
                // the optimum of an ill-conditioned quadric can fly away: keep it near the edge
                if e < best.0 && o.distance(mid) < 2.0 * pa.distance(pb) + 1e-3 {
                    best = (e, o);
                }
            }
            heap.push(Candidate {
                cost: best.0.max(0.0),
                a,
                b,
                stamp: stamp[a as usize] + stamp[b as usize],
                target: best.1,
            });
        };
    for &(x, y) in edge_count.keys() {
        push(&mut heap, x, y, &quad, &pos, &stamp);
    }
    let mut alive_count = faces.len();
    let mut removed_vertex = vec![false; nv];
    while alive_count > target_triangles {
        let Some(c) = heap.pop() else { break };
        let (a, b) = (c.a as usize, c.b as usize);
        if removed_vertex[a] || removed_vertex[b] || c.stamp != stamp[a] + stamp[b] {
            continue;
        }
        // the edge must still exist
        if !vfaces[a]
            .iter()
            .any(|fi| alive_face[*fi as usize] && faces[*fi as usize].contains(&(b as u32)))
        {
            continue;
        }
        // reject flips: every surviving face around a or b keeps its orientation after the move
        let mut flips = false;
        for &fi in vfaces[a].iter().chain(&vfaces[b]) {
            let f = faces[fi as usize];
            if !alive_face[fi as usize] || (f.contains(&(a as u32)) && f.contains(&(b as u32))) {
                continue;
            }
            let p = |v: u32| {
                if v as usize == a || v as usize == b {
                    c.target
                } else {
                    pos[v as usize]
                }
            };
            let before =
                (pos[f[1] as usize] - pos[f[0] as usize]).cross(pos[f[2] as usize] - pos[f[0] as usize]);
            let after = (p(f[1]) - p(f[0])).cross(p(f[2]) - p(f[0]));
            if before.dot(after) <= 1e-12 * before.length_squared() {
                flips = true;
                break;
            }
        }
        if flips {
            continue;
        }
        // collapse b into a
        pos[a] = c.target;
        quad[a] = quad[a].add(&quad[b]);
        removed_vertex[b] = true;
        let b_faces = std::mem::take(&mut vfaces[b]);
        for fi in b_faces {
            let f = &mut faces[fi as usize];
            if !alive_face[fi as usize] {
                continue;
            }
            if f.contains(&(a as u32)) {
                alive_face[fi as usize] = false;
                alive_count -= 1;
                // drop this face from the third vertex's list lazily (checked via alive_face)
                continue;
            }
            for v in f.iter_mut() {
                if *v == b as u32 {
                    *v = a as u32;
                }
            }
            vfaces[a].push(fi);
        }
        vfaces[a].retain(|fi| alive_face[*fi as usize]);
        stamp[a] += 1;
        // re-queue the edges around a
        let mut seen: HashSet<u32> = HashSet::new();
        for &fi in &vfaces[a] {
            for v in faces[fi as usize] {
                if v as usize != a && seen.insert(v) {
                    push(&mut heap, a as u32, v, &quad, &pos, &stamp);
                }
            }
        }
    }
    // compact
    let mut remap = vec![u32::MAX; nv];
    let mut vertices = Vec::new();
    let mut indices = Vec::with_capacity(alive_count * 3);
    for (fi, f) in faces.iter().enumerate() {
        if !alive_face[fi] || f[0] == f[1] || f[1] == f[2] || f[0] == f[2] {
            continue;
        }
        for v in f {
            if remap[*v as usize] == u32::MAX {
                remap[*v as usize] = vertices.len() as u32;
                vertices.push(Vertex {
                    position: pos[*v as usize].to_array(),
                    normal: [0.0; 3],
                });
            }
            indices.push(remap[*v as usize]);
        }
    }
    *mesh = TriMesh { vertices, indices };
    mesh.recompute_normals();
}

// ---------------------------------------------------------------------- primitives ------------
// Swept primitives are test fixtures only since the default pack draws nothing but dataset parts.

#[cfg(test)]
fn frame(axis: Vec3) -> (Vec3, Vec3) {
    let a = axis.normalize_or_zero();
    let helper = if a.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let u = helper.cross(a).normalize();
    let v = a.cross(u);
    (u, v)
}

#[cfg(test)]
/// A tube from `start` along `axis` with a radius profile `radius(t)` for `t ∈ [0, 1]` and a
/// rounded end (closed at the start too). `rings` samples along the length, `segments` around.
#[must_use]
pub fn swept_tube(
    start: Vec3,
    axis: Vec3,
    length_mm: f32,
    radius: impl Fn(f32) -> f32,
    rings: usize,
    segments: usize,
) -> TriMesh {
    let a = axis.normalize_or_zero();
    let (u, v) = frame(a);
    let r_end = radius(1.0);
    let mut profile: Vec<(Vec3, f32)> = Vec::new(); // (centre, radius)
    for i in 0..=rings {
        let t = i as f32 / rings as f32;
        profile.push((start + a * (length_mm * t), radius(t)));
    }
    // hemispherical cap
    let cap_steps = 6;
    for i in 1..=cap_steps {
        let ang = std::f32::consts::FRAC_PI_2 * i as f32 / cap_steps as f32;
        profile.push((start + a * (length_mm + r_end * ang.sin()), r_end * ang.cos()));
    }
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    // start cap centre
    vertices.push(Vertex {
        position: start.to_array(),
        normal: (-a).to_array(),
    });
    for (c, r) in &profile {
        for s in 0..segments {
            let ang = std::f32::consts::TAU * s as f32 / segments as f32;
            let dir = u * ang.cos() + v * ang.sin();
            vertices.push(Vertex {
                position: (*c + dir * *r).to_array(),
                normal: dir.to_array(),
            });
        }
    }
    let ring = |i: usize, s: usize| (1 + i * segments + (s % segments)) as u32;
    for s in 0..segments {
        indices.extend_from_slice(&[0, ring(0, s + 1), ring(0, s)]);
    }
    for i in 0..profile.len() - 1 {
        for s in 0..segments {
            indices.extend_from_slice(&[ring(i, s), ring(i, s + 1), ring(i + 1, s)]);
            indices.extend_from_slice(&[ring(i, s + 1), ring(i + 1, s + 1), ring(i + 1, s)]);
        }
    }
    // end centre
    let tip = start + a * (length_mm + r_end);
    vertices.push(Vertex {
        position: tip.to_array(),
        normal: a.to_array(),
    });
    let tip_i = (vertices.len() - 1) as u32;
    let last = profile.len() - 1;
    for s in 0..segments {
        indices.extend_from_slice(&[ring(last, s), ring(last, s + 1), tip_i]);
    }
    let mut mesh = TriMesh { vertices, indices };
    mesh.recompute_normals();
    orient_outward(&mut mesh);
    mesh
}

#[cfg(test)]
/// A torus-like tube swept along a closed polyline (a valve annulus).
#[must_use]
pub fn swept_ring(loop_points: &[Vec3], minor_mm: f32, segments: usize) -> TriMesh {
    let n = loop_points.len();
    let mut vertices = Vec::with_capacity(n * segments);
    let mut indices = Vec::with_capacity(n * segments * 6);
    let centre = loop_points.iter().copied().sum::<Vec3>() / n as f32;
    for i in 0..n {
        let p = loop_points[i];
        let tangent = (loop_points[(i + 1) % n] - loop_points[(i + n - 1) % n]).normalize_or_zero();
        let outward = (p - centre - tangent * (p - centre).dot(tangent)).normalize_or_zero();
        let up = tangent.cross(outward).normalize_or_zero();
        for s in 0..segments {
            let ang = std::f32::consts::TAU * s as f32 / segments as f32;
            let dir = outward * ang.cos() + up * ang.sin();
            vertices.push(Vertex {
                position: (p + dir * minor_mm).to_array(),
                normal: dir.to_array(),
            });
        }
    }
    let at = |i: usize, s: usize| ((i % n) * segments + (s % segments)) as u32;
    for i in 0..n {
        for s in 0..segments {
            indices.extend_from_slice(&[at(i, s), at(i, s + 1), at(i + 1, s)]);
            indices.extend_from_slice(&[at(i, s + 1), at(i + 1, s + 1), at(i + 1, s)]);
        }
    }
    let mut mesh = TriMesh { vertices, indices };
    orient_outward(&mut mesh);
    mesh
}

#[cfg(test)]
/// Signed volume by the divergence theorem (positive for outward counter-clockwise winding).
#[must_use]
pub fn signed_volume(m: &TriMesh) -> f32 {
    m.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let a = Vec3::from(m.vertices[t[0] as usize].position);
            let b = Vec3::from(m.vertices[t[1] as usize].position);
            let c = Vec3::from(m.vertices[t[2] as usize].position);
            a.dot(b.cross(c)) / 6.0
        })
        .sum()
}

#[cfg(test)]
/// Flip every triangle of a closed mesh whose winding faces inward (the renderer culls back
/// faces), and recompute the normals.
pub fn orient_outward(mesh: &mut TriMesh) {
    if signed_volume(mesh) < 0.0 {
        for t in mesh.indices.as_chunks_mut::<3>().0 {
            t.swap(1, 2);
        }
        mesh.recompute_normals();
    }
}

/// Close every open rim of `mesh` with a fan from the rim's centroid (cut planes of a torso).
pub fn cap_open_rims(mesh: &mut TriMesh) {
    let mut edge_count: HashMap<(u32, u32), u32> = HashMap::new();
    let mut directed: HashSet<(u32, u32)> = HashSet::new();
    for t in mesh.indices.as_chunks::<3>().0 {
        for i in 0..3 {
            let (x, y) = (t[i], t[(i + 1) % 3]);
            *edge_count.entry((x.min(y), x.max(y))).or_default() += 1;
            directed.insert((x, y));
        }
    }
    // boundary edges keep the face's winding (x → y); the cap uses the reverse to face outward
    let mut next: HashMap<u32, u32> = HashMap::new();
    for (&(x, y), &n) in &edge_count {
        if n == 1 {
            let (from, to) = if directed.contains(&(x, y)) {
                (x, y)
            } else {
                (y, x)
            };
            next.insert(from, to);
        }
    }
    let mut visited: HashSet<u32> = HashSet::new();
    let starts: Vec<u32> = {
        let mut v: Vec<u32> = next.keys().copied().collect();
        v.sort_unstable();
        v
    };
    for start in starts {
        if visited.contains(&start) {
            continue;
        }
        let mut ring = vec![start];
        visited.insert(start);
        let mut cur = start;
        while let Some(&n) = next.get(&cur) {
            if n == start || visited.contains(&n) {
                break;
            }
            ring.push(n);
            visited.insert(n);
            cur = n;
        }
        if ring.len() < 3 {
            continue;
        }
        let centre = ring
            .iter()
            .map(|i| Vec3::from(mesh.vertices[*i as usize].position))
            .sum::<Vec3>()
            / ring.len() as f32;
        mesh.vertices.push(Vertex {
            position: centre.to_array(),
            normal: [0.0; 3],
        });
        let ci = (mesh.vertices.len() - 1) as u32;
        for i in 0..ring.len() {
            let (x, y) = (ring[i], ring[(i + 1) % ring.len()]);
            mesh.indices.extend_from_slice(&[y, x, ci]);
        }
    }
    mesh.recompute_normals();
}

/// Farthest intersection along the ray (the outer skin when the origin is inside the body).
#[must_use]
pub fn ray_hit_far(mesh: &TriMesh, origin: Vec3, dir: Vec3) -> Option<Vec3> {
    let d = dir.normalize_or_zero();
    let mut best: Option<f32> = None;
    for t in mesh.indices.as_chunks::<3>().0 {
        let a = Vec3::from(mesh.vertices[t[0] as usize].position);
        let b = Vec3::from(mesh.vertices[t[1] as usize].position);
        let c = Vec3::from(mesh.vertices[t[2] as usize].position);
        let (e1, e2) = (b - a, c - a);
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-9 {
            continue;
        }
        let inv = 1.0 / det;
        let s = origin - a;
        let u = s.dot(p) * inv;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = s.cross(e1);
        let v = d.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let tt = e2.dot(q) * inv;
        if tt > 1e-4 && best.is_none_or(|b| tt > b) {
            best = Some(tt);
        }
    }
    best.map(|t| origin + d * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_geometry::primitives::icosphere;

    #[test]
    fn primitives_are_wound_outward() {
        let tube = swept_tube(
            Vec3::new(3.0, 1.0, 2.0),
            Vec3::new(0.3, 1.0, -0.2),
            20.0,
            |_| 5.0,
            4,
            16,
        );
        assert!(
            signed_volume(&tube) > 1000.0,
            "tube volume {}",
            signed_volume(&tube)
        );
        let ring: Vec<Vec3> = (0..48)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 48.0;
                Vec3::new(10.0 * a.cos(), 0.0, 10.0 * a.sin())
            })
            .collect();
        let torus = swept_ring(&ring, 1.5, 12);
        assert!(
            signed_volume(&torus) > 100.0,
            "torus volume {}",
            signed_volume(&torus)
        );
        let mut cut = TriMesh {
            vertices: icosphere(3).vertices,
            ..TriMesh::default()
        };
        let m = icosphere(3);
        for t in m.indices.as_chunks::<3>().0 {
            if t.iter().all(|i| m.vertices[*i as usize].position[1] < 0.3) {
                cut.indices.extend_from_slice(t);
            }
        }
        cap_open_rims(&mut cut);
        assert!(
            signed_volume(&cut) > 2.0,
            "capped sphere volume {}",
            signed_volume(&cut)
        );
        let mut dec = icosphere(4);
        decimate(&mut dec, 500);
        assert!(
            signed_volume(&dec) > 3.5,
            "decimated sphere volume {}",
            signed_volume(&dec)
        );
    }

    #[test]
    fn decimation_keeps_a_sphere_round() {
        let mut m = icosphere(4); // 5120 triangles
        let before = m.triangle_count();
        decimate(&mut m, 600);
        assert!(
            m.triangle_count() <= 600 && m.triangle_count() > 300,
            "{} → {}",
            before,
            m.triangle_count()
        );
        for v in &m.vertices {
            let r = Vec3::from(v.position).length();
            assert!((r - 1.0).abs() < 0.05, "radius {r}");
        }
        // still closed: every edge used twice
        let mut count: HashMap<(u32, u32), u32> = HashMap::new();
        for t in m.indices.as_chunks::<3>().0 {
            for i in 0..3 {
                let (x, y) = (t[i], t[(i + 1) % 3]);
                *count.entry((x.min(y), x.max(y))).or_default() += 1;
            }
        }
        assert!(count.values().all(|n| *n == 2));
    }

    #[test]
    fn tube_is_closed_and_rays_hit_it() {
        let m = swept_tube(Vec3::ZERO, Vec3::Y, 20.0, |_| 5.0, 4, 16);
        let mut count: HashMap<(u32, u32), u32> = HashMap::new();
        for t in m.indices.as_chunks::<3>().0 {
            for i in 0..3 {
                let (x, y) = (t[i], t[(i + 1) % 3]);
                *count.entry((x.min(y), x.max(y))).or_default() += 1;
            }
        }
        assert!(count.values().all(|n| *n == 2), "open edges");
        let hit = ray_hit_far(&m, Vec3::new(0.0, 10.0, 0.0), Vec3::X).expect("hit");
        assert!((hit.x - 5.0).abs() < 1e-3);
    }

    #[test]
    fn capping_closes_a_cut_sphere() {
        let m = icosphere(3);
        let mut cut = TriMesh {
            vertices: m.vertices.clone(),
            ..TriMesh::default()
        };
        for t in m.indices.as_chunks::<3>().0 {
            if t.iter().all(|i| m.vertices[*i as usize].position[1] < 0.3) {
                cut.indices.extend_from_slice(t);
            }
        }
        cap_open_rims(&mut cut);
        let mut count: HashMap<(u32, u32), u32> = HashMap::new();
        for t in cut.indices.as_chunks::<3>().0 {
            for i in 0..3 {
                let (x, y) = (t[i], t[(i + 1) % 3]);
                *count.entry((x.min(y), x.max(y))).or_default() += 1;
            }
        }
        assert!(count.values().all(|n| *n == 2), "cap left open edges");
    }
}
