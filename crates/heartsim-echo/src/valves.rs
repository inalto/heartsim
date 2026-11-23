//! The four valve-plane discs of a `Valve` mesh, told apart by position (the same rule as the
//! app's valve highlight, so both agree on which disc is which).

use glam::Vec3;
use heartsim_ecg::Valve;
use heartsim_geometry::TriMesh;

/// One valve disc of the `valve_planes` mesh: which valve, the vertex indices of the disc in that
/// mesh, and the disc's centre (mean of its vertices, heart frame, mm).
#[derive(Debug, Clone, PartialEq)]
pub struct ValveDisc {
    pub valve: Valve,
    pub vertices: Vec<u32>,
    pub centre_mm: Vec3,
}

/// The connected components of `mesh`, as vertex lists in ascending index order (components
/// ordered by their smallest vertex index).
#[must_use]
pub fn components(mesh: &TriMesh) -> Vec<Vec<u32>> {
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let n = mesh.vertices.len();
    let mut parent: Vec<usize> = (0..n).collect();
    for tri in mesh.indices.as_chunks::<3>().0 {
        let a = find(&mut parent, tri[0] as usize);
        for &v in &tri[1..] {
            let b = find(&mut parent, v as usize);
            if a != b {
                parent[b] = a;
            }
        }
    }
    let mut roots: Vec<usize> = Vec::new();
    let mut out: Vec<Vec<u32>> = Vec::new();
    for v in 0..n {
        let r = find(&mut parent, v);
        let k = roots.iter().position(|&x| x == r).unwrap_or_else(|| {
            roots.push(r);
            out.push(Vec::new());
            roots.len() - 1
        });
        out[k].push(v as u32);
    }
    out
}

/// The four discs of a valve-plane mesh, or `None` unless it has exactly four components. Told
/// apart by their bounding-box centres in the heart frame: the pulmonic the most anterior and
/// cranial (largest `y + z`), then the tricuspid the furthest to the patient's right (smallest
/// `x`), then the mitral the most posterior (smallest `z`), the aortic the one left — the rule of
/// `heartsim_app::heart_view::split_valves`.
#[must_use]
pub fn split_valve_discs(mesh: &TriMesh) -> Option<Vec<ValveDisc>> {
    let comps = components(mesh);
    if comps.len() != 4 {
        return None;
    }
    let pos = |i: u32| Vec3::from(mesh.vertices[i as usize].position);
    let mut left: Vec<(Vec3, Vec<u32>)> = comps
        .into_iter()
        .map(|c| {
            let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
            for &i in &c {
                lo = lo.min(pos(i));
                hi = hi.max(pos(i));
            }
            ((lo + hi) * 0.5, c)
        })
        .collect();
    let mut take = |key: fn(Vec3) -> f32| {
        let i = (0..left.len())
            .min_by(|&a, &b| key(left[a].0).total_cmp(&key(left[b].0)))
            .unwrap_or(0);
        left.remove(i).1
    };
    let pulmonic = take(|c| -(c.y + c.z));
    let tricuspid = take(|c| c.x);
    let mitral = take(|c| c.z);
    let aortic = take(|c| c.x);
    let disc = |valve: Valve, vertices: Vec<u32>| {
        let sum: Vec3 = vertices.iter().map(|&i| pos(i)).sum();
        let centre_mm = sum / vertices.len().max(1) as f32;
        ValveDisc {
            valve,
            vertices,
            centre_mm,
        }
    };
    Some(vec![
        disc(Valve::Mitral, mitral),
        disc(Valve::Tricuspid, tricuspid),
        disc(Valve::Aortic, aortic),
        disc(Valve::Pulmonic, pulmonic),
    ])
}
