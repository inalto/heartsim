//! Conforming tetrahedral meshes for solvers that need elements.
//!
//! A heartsim cell cloud is a lattice of points plus a few explicit structures, and walls can be a
//! single cell thick, so tetrahedra spanning cell centres would lose the atria. [`voxel_mesh`]
//! therefore gives every cell its own cube, centred on the cell, and splits each cube into the six
//! Kuhn (Freudenthal) tetrahedra around its main diagonal. That split is translation invariant, so
//! neighbouring cubes agree on their shared face diagonals and the mesh is conforming, and each
//! element inherits region and fibre from exactly one cell.
//!
//! The cubes follow the cloud's own lattice, which need not be axis-aligned: the procedural heart is
//! sampled on a lattice and then rotated into the heart frame, and axis-aligned cubes of the same size
//! would alias against it, leaving about one cube in six empty and the wall full of holes.
//! [`lattice_frame`] finds the lattice's orientation from the cloud's own point spacing.
//!
//! Electrically separate tissue must stay separate: corners are shared only between cubes of the same
//! group (the atria and the ventricles, insulated by the fibrous AV ring), so a solver cannot conduct
//! across the ring even where the two walls touch.
//!
//! Elements are ordered with positive signed volume, `(v1−v0) × (v2−v0) · (v3−v0) > 0`, the VTK
//! convention for tetrahedra.

use crate::spatial::HashGrid;
use glam::{DVec3, Mat3, Quat, Vec3};
use std::collections::{BTreeMap, HashMap};

/// A tetrahedral mesh with per-element tag and fibre, in heart coordinates (mm).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TetMesh {
    pub points_mm: Vec<Vec3>,
    pub tets: Vec<[u32; 4]>,
    /// Region tag per element (non-negative, as the `.elem` format requires).
    pub tags: Vec<u32>,
    /// Unit fibre direction per element (zero = isotropic).
    pub fibres: Vec<Vec3>,
}

/// One input cell of [`voxel_mesh`]. Cells with `group == None` (conduction-system cells whose delays
/// the timing engine owns) are not tissue and get no elements.
#[derive(Debug, Clone, Copy)]
pub struct CellInput<'a> {
    pub positions: &'a [Vec3],
    pub fibres: &'a [Vec3],
    pub tags: &'a [u32],
    pub groups: &'a [Option<u8>],
}

/// Output of [`voxel_mesh`].
#[derive(Debug, Clone, Default)]
pub struct VoxelMesh {
    pub mesh: TetMesh,
    /// Group of every node (a node never belongs to two groups).
    pub node_group: Vec<u8>,
    /// Source cell of every element.
    pub tet_cell: Vec<u32>,
    /// Orientation of the lattice found in the cloud: lattice coordinates → heart frame.
    pub frame: Quat,
    /// Lattice origin found in the cloud, in lattice coordinates, mm.
    pub origin_mm: Vec3,
    /// Tissue cells with a cube of their own.
    pub cells_meshed: usize,
    /// Tissue cells that fell into an occupied cube (off-lattice structures); the cell nearest the cube
    /// centre keeps it.
    pub cells_merged: usize,
    /// Non-tissue cells (`group == None`).
    pub cells_excluded: usize,
}

impl VoxelMesh {
    /// Nodes of `group` within `radius_mm` of `p`, in index order: a stimulus electrode.
    #[must_use]
    pub fn nodes_within(&self, p: Vec3, radius_mm: f32, group: u8) -> Vec<u32> {
        self.mesh
            .points_mm
            .iter()
            .zip(&self.node_group)
            .enumerate()
            .filter(|(_, (q, g))| **g == group && q.distance(p) <= radius_mm)
            .map(|(i, _)| i as u32)
            .collect()
    }
}

/// The three axis permutations of the six tetrahedra of a cube; corners are numbered by bit (x = 1,
/// y = 2, z = 4), so every tetrahedron is the monotone path from corner 0 to corner 7.
const KUHN_PATHS: [[u32; 3]; 6] = [[1, 2, 4], [1, 4, 2], [2, 1, 4], [2, 4, 1], [4, 1, 2], [4, 2, 1]];

fn kuhn_tets() -> [[u32; 4]; 6] {
    KUHN_PATHS.map(|[a, b, c]| [0, a, a + b, a + b + c])
}

/// Signed volume of a tetrahedron, mm³.
#[must_use]
pub fn signed_volume_mm3(p: [Vec3; 4]) -> f64 {
    let [a, b, c, d] = p.map(Vec3::as_dvec3);
    (b - a).cross(c - a).dot(d - a) / 6.0
}

fn oriented(mut t: [u32; 4], points: &[Vec3]) -> [u32; 4] {
    let v = signed_volume_mm3(t.map(|i| points[i as usize]));
    if v < 0.0 {
        t.swap(2, 3);
    }
    t
}

/// Most common fractional lattice coordinate of `values / spacing`, as a coordinate offset in mm.
/// Lattice cells all share it; off-lattice structures are a minority and spread out.
fn lattice_offset(values: impl Iterator<Item = f32>, spacing_mm: f32) -> f32 {
    const BINS: usize = 128;
    let fracts: Vec<f32> = values.map(|v| (v / spacing_mm).rem_euclid(1.0)).collect();
    let mut hist = [0usize; BINS];
    for f in &fracts {
        hist[((f * BINS as f32) as usize).min(BINS - 1)] += 1;
    }
    let score = |i: usize| hist[(i + BINS - 1) % BINS] + hist[i] + hist[(i + 1) % BINS];
    let best = (0..BINS)
        .max_by_key(|&i| (score(i), std::cmp::Reverse(i)))
        .unwrap_or(0);
    let centre = (best as f32 + 0.5) / BINS as f32;
    let wrap = |d: f32| d - d.round();
    let near: Vec<f32> = fracts
        .iter()
        .map(|f| wrap(f - centre))
        .filter(|d| d.abs() <= 1.5 / BINS as f32)
        .collect();
    let mean = near.iter().sum::<f32>() / near.len().max(1) as f32;
    (centre + mean).rem_euclid(1.0) * spacing_mm
}

/// Orientation of the lattice `positions` were sampled on, lattice coordinates → heart frame: the two
/// most common perpendicular directions among point pairs `spacing_mm` apart. The identity when the
/// points show no lattice (then cubes are simply axis-aligned).
#[must_use]
pub fn lattice_frame(positions: &[Vec3], spacing_mm: f32) -> Quat {
    const MAX_DIRECTIONS: usize = 64;
    const PARALLEL: f32 = 0.995;
    const PERPENDICULAR: f32 = 0.05;
    let grid = HashGrid::with_auto_cell(positions);
    // direction (sign-aligned sum of unit edges) and how many edges fell on it
    let mut directions: Vec<(Vec3, u32)> = Vec::new();
    let mut found = Vec::with_capacity(6);
    for (i, p) in positions.iter().enumerate() {
        grid.knn(*p, 6, Some(i as u32), &mut found);
        for (d, j) in &found {
            if (d - spacing_mm).abs() > 0.02 * spacing_mm {
                continue;
            }
            let u = (positions[*j as usize] - *p) / *d;
            let known = directions
                .iter()
                .position(|(c, _)| c.normalize().dot(u).abs() > PARALLEL);
            match known {
                Some(k) => {
                    let (c, n) = &mut directions[k];
                    *c += u * c.dot(u).signum();
                    *n += 1;
                }
                None if directions.len() < MAX_DIRECTIONS => directions.push((u, 1)),
                None => {}
            }
        }
    }
    // stable sort: ties keep first-seen order, so the frame is deterministic
    directions.sort_by_key(|d| std::cmp::Reverse(d.1));
    let Some(a) = directions.first().map(|d| d.0.normalize()) else {
        return Quat::IDENTITY;
    };
    let Some(b) = directions
        .iter()
        .map(|d| d.0.normalize())
        .find(|b| a.dot(*b).abs() < PERPENDICULAR)
    else {
        return Quat::IDENTITY;
    };
    let b = (b - a * a.dot(b)).normalize();
    Quat::from_mat3(&Mat3::from_cols(a, b, a.cross(b))).normalize()
}

/// Mesh the tissue cells of a cloud as cubes of edge `spacing_mm` centred on the lattice the cloud
/// was sampled on. See the module documentation.
///
/// # Panics
/// If the input slices differ in length.
#[must_use]
pub fn voxel_mesh(cells: CellInput<'_>, spacing_mm: f32) -> VoxelMesh {
    let n = cells.positions.len();
    assert!(cells.fibres.len() == n && cells.tags.len() == n && cells.groups.len() == n);
    let tissue = || (0..n).filter(|&i| cells.groups[i].is_some());
    let tissue_points: Vec<Vec3> = tissue().map(|i| cells.positions[i]).collect();
    let frame = lattice_frame(&tissue_points, spacing_mm);
    let to_lattice = frame.inverse();
    let local: Vec<Vec3> = cells.positions.iter().map(|p| to_lattice * *p).collect();
    let origin = Vec3::new(
        lattice_offset(tissue().map(|i| local[i].x), spacing_mm),
        lattice_offset(tissue().map(|i| local[i].y), spacing_mm),
        lattice_offset(tissue().map(|i| local[i].z), spacing_mm),
    );

    // cube (group, i, j, k) → (distance² to its centre, cell); ordered, so numbering is deterministic
    let mut cubes: BTreeMap<(u8, i32, i32, i32), (f32, u32)> = BTreeMap::new();
    let mut merged = 0;
    for i in tissue() {
        let p = local[i];
        let l = ((p - origin) / spacing_mm).round();
        let key = (cells.groups[i].unwrap_or(0), l.x as i32, l.y as i32, l.z as i32);
        let d2 = (origin + l * spacing_mm).distance_squared(p);
        cubes
            .entry(key)
            .and_modify(|e| {
                merged += 1;
                if d2 < e.0 {
                    *e = (d2, i as u32);
                }
            })
            .or_insert((d2, i as u32));
    }

    // corners (group, i, j, k), at half-integer lattice positions stored as the lower-index cube
    let mut corners: BTreeMap<(u8, i32, i32, i32), u32> = BTreeMap::new();
    for &(g, i, j, k) in cubes.keys() {
        for b in 0..8i32 {
            corners.insert((g, i + (b & 1), j + ((b >> 1) & 1), k + ((b >> 2) & 1)), 0);
        }
    }
    let mut points = Vec::with_capacity(corners.len());
    let mut node_group = Vec::with_capacity(corners.len());
    for (id, (key, slot)) in corners.iter_mut().enumerate() {
        *slot = id as u32;
        let (g, i, j, k) = *key;
        let lattice = Vec3::new(i as f32 - 0.5, j as f32 - 0.5, k as f32 - 0.5);
        points.push(frame * (origin + lattice * spacing_mm));
        node_group.push(g);
    }

    let kuhn = kuhn_tets();
    let mut out = TetMesh {
        points_mm: points,
        tets: Vec::with_capacity(cubes.len() * 6),
        tags: Vec::with_capacity(cubes.len() * 6),
        fibres: Vec::with_capacity(cubes.len() * 6),
    };
    let mut tet_cell = Vec::with_capacity(cubes.len() * 6);
    for (&(g, i, j, k), &(_, cell)) in &cubes {
        let corner = |b: u32| {
            let b = b.cast_signed();
            corners[&(g, i + (b & 1), j + ((b >> 1) & 1), k + ((b >> 2) & 1))]
        };
        for t in kuhn {
            out.tets.push(oriented(t.map(corner), &out.points_mm));
            out.tags.push(cells.tags[cell as usize]);
            out.fibres.push(cells.fibres[cell as usize].normalize_or_zero());
            tet_cell.push(cell);
        }
    }
    VoxelMesh {
        mesh: out,
        node_group,
        tet_cell,
        frame,
        origin_mm: origin,
        cells_meshed: cubes.len(),
        cells_merged: merged,
        cells_excluded: n - tissue().count(),
    }
}

/// A regular box of `dims` nodes per axis, spacing `h_mm`, first node at `origin_mm`, all elements with
/// one tag and fibre. Nodes lie exactly on the box corners, as the Niederer slab's reference points
/// require.
///
/// # Panics
/// If any dimension has fewer than two nodes.
#[must_use]
pub fn box_mesh(dims: [usize; 3], h_mm: f32, origin_mm: Vec3, fibre: Vec3, tag: u32) -> TetMesh {
    assert!(
        dims.iter().all(|&d| d >= 2),
        "a box needs at least one element per axis"
    );
    let [nx, ny, nz] = dims;
    let id = |i: usize, j: usize, k: usize| (i + nx * (j + ny * k)) as u32;
    let mut points = Vec::with_capacity(nx * ny * nz);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                points.push(origin_mm + Vec3::new(i as f32, j as f32, k as f32) * h_mm);
            }
        }
    }
    let kuhn = kuhn_tets();
    let elements = (nx - 1) * (ny - 1) * (nz - 1) * 6;
    let mut tets = Vec::with_capacity(elements);
    for k in 0..nz - 1 {
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let corner = |b: u32| {
                    id(
                        i + (b & 1) as usize,
                        j + ((b >> 1) & 1) as usize,
                        k + ((b >> 2) & 1) as usize,
                    )
                };
                for t in kuhn {
                    tets.push(oriented(t.map(corner), &points));
                }
            }
        }
    }
    TetMesh {
        points_mm: points,
        tags: vec![tag; tets.len()],
        fibres: vec![fibre.normalize_or_zero(); tets.len()],
        tets,
    }
}

/// What a mesh checker needs to know: the same properties `meshtool` reports, computed here so that an
/// export can be checked without it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshCheck {
    pub nodes: usize,
    pub elements: usize,
    /// Node indices at or past the end of the node list.
    pub bad_indices: usize,
    /// Elements repeating a node.
    pub repeated_nodes: usize,
    /// Nodes no element uses (solvers reject or mis-solve these).
    pub orphan_nodes: usize,
    /// Elements with signed volume ≤ 0.
    pub inverted_elements: usize,
    /// Faces used by more than two elements, or twice with the same winding (not conforming).
    pub bad_faces: usize,
    pub boundary_faces: usize,
    /// Connected pieces of the node graph.
    pub components: usize,
    /// Total volume, mm³.
    pub volume_mm3: f64,
    pub min_element_mm3: f64,
    pub max_element_mm3: f64,
    /// Elements with a fibre whose length is neither 0 nor 1.
    pub bad_fibres: usize,
    /// Elements per tag, ascending.
    pub tag_counts: BTreeMap<u32, usize>,
}

impl MeshCheck {
    /// Conforming, positively oriented, with no unused or out-of-range nodes.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.elements > 0
            && self.bad_indices == 0
            && self.repeated_nodes == 0
            && self.orphan_nodes == 0
            && self.inverted_elements == 0
            && self.bad_faces == 0
            && self.bad_fibres == 0
    }
}

fn find(parent: &mut [u32], mut x: u32) -> u32 {
    while parent[x as usize] != x {
        let up = parent[parent[x as usize] as usize];
        parent[x as usize] = up;
        x = up;
    }
    x
}

/// Check a mesh's indices, orientation, conformity and fibres.
#[must_use]
pub fn check_mesh(m: &TetMesh) -> MeshCheck {
    let n = m.points_mm.len();
    let mut c = MeshCheck {
        nodes: n,
        elements: m.tets.len(),
        min_element_mm3: f64::INFINITY,
        max_element_mm3: f64::NEG_INFINITY,
        ..MeshCheck::default()
    };
    let mut used = vec![false; n];
    let mut parent: Vec<u32> = (0..n as u32).collect();
    // sorted face → (uses, directed uses by parity of the sorting permutation)
    let mut faces: HashMap<[u32; 3], [u32; 2]> = HashMap::with_capacity(m.tets.len() * 2);
    for t in &m.tets {
        if t.iter().any(|&i| i as usize >= n) {
            c.bad_indices += 1;
            continue;
        }
        let mut sorted = *t;
        sorted.sort_unstable();
        if sorted.windows(2).any(|w| w[0] == w[1]) {
            c.repeated_nodes += 1;
        }
        for &i in t {
            used[i as usize] = true;
            let (a, b) = (find(&mut parent, t[0]), find(&mut parent, i));
            parent[a as usize] = b;
        }
        let v = signed_volume_mm3(t.map(|i| m.points_mm[i as usize]));
        if v <= 0.0 {
            c.inverted_elements += 1;
        }
        c.volume_mm3 += v;
        c.min_element_mm3 = c.min_element_mm3.min(v);
        c.max_element_mm3 = c.max_element_mm3.max(v);
        // outward faces of a positive tetrahedron
        for f in [
            [t[1], t[2], t[3]],
            [t[0], t[3], t[2]],
            [t[0], t[1], t[3]],
            [t[0], t[2], t[1]],
        ] {
            let (key, parity) = sorted_with_parity(f);
            faces.entry(key).or_default()[parity] += 1;
        }
    }
    for [even, odd] in faces.values() {
        match (*even, *odd) {
            (1, 0) | (0, 1) => c.boundary_faces += 1,
            (1, 1) => {}
            _ => c.bad_faces += 1,
        }
    }
    c.orphan_nodes = used.iter().filter(|u| !**u).count();
    c.components = (0..n as u32)
        .filter(|&i| used[i as usize] && find(&mut parent, i) == i)
        .count();
    c.bad_fibres = m
        .fibres
        .iter()
        .filter(|f| {
            let l = DVec3::new(f64::from(f.x), f64::from(f.y), f64::from(f.z)).length();
            l != 0.0 && (l - 1.0).abs() > 1e-4
        })
        .count();
    for t in &m.tags {
        *c.tag_counts.entry(*t).or_default() += 1;
    }
    if c.elements == 0 {
        c.min_element_mm3 = 0.0;
        c.max_element_mm3 = 0.0;
    }
    c
}

/// Sort three indices; the second value is 0 for an even permutation of the input and 1 for an odd one.
fn sorted_with_parity(f: [u32; 3]) -> ([u32; 3], usize) {
    let mut a = f;
    let mut parity = 0;
    for (i, j) in [(0, 1), (1, 2), (0, 1)] {
        if a[i] > a[j] {
            a.swap(i, j);
            parity ^= 1;
        }
    }
    (a, parity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn box_mesh_is_conforming_and_fills_the_box() {
        let m = box_mesh([5, 4, 3], 0.5, Vec3::ZERO, Vec3::X, 1);
        let c = check_mesh(&m);
        assert!(c.is_valid(), "{c:?}");
        assert_eq!(c.components, 1);
        assert_eq!(c.elements, 4 * 3 * 2 * 6);
        assert_abs_diff_eq!(c.volume_mm3, 2.0 * 1.5 * 1.0, epsilon = 1e-5);
        // two triangles on each boundary square
        assert_eq!(c.boundary_faces, 2 * 2 * (4 * 3 + 4 * 2 + 3 * 2));
    }

    #[test]
    fn check_mesh_reports_broken_meshes() {
        let mut m = box_mesh([2, 2, 2], 1.0, Vec3::ZERO, Vec3::X, 1);
        m.tets[0].swap(0, 1);
        let c = check_mesh(&m);
        assert_eq!(c.inverted_elements, 1);
        assert!(c.bad_faces > 0, "reversed winding must not look conforming");

        let mut m = box_mesh([2, 2, 2], 1.0, Vec3::ZERO, Vec3::X, 1);
        m.points_mm.push(Vec3::splat(9.0));
        assert_eq!(check_mesh(&m).orphan_nodes, 1);

        let mut m = box_mesh([2, 2, 2], 1.0, Vec3::ZERO, Vec3::X, 1);
        m.tets[1] = m.tets[0];
        assert!(check_mesh(&m).bad_faces > 0);
        m.tets[1] = [0, 1, 2, 99];
        assert_eq!(check_mesh(&m).bad_indices, 1);
    }

    fn lattice(group_of: impl Fn(i32) -> Option<u8>) -> (Vec<Vec3>, Vec<Vec3>, Vec<u32>, Vec<Option<u8>>) {
        let (mut p, mut g) = (Vec::new(), Vec::new());
        for i in 0..4 {
            for j in 0..3 {
                for k in 0..2 {
                    p.push(Vec3::new(i as f32, j as f32, k as f32) * 2.5 + Vec3::new(1.0, -7.3, 0.4));
                    g.push(group_of(i));
                }
            }
        }
        let n = p.len();
        (p, vec![Vec3::Y; n], (0..n as u32).collect(), g)
    }

    #[test]
    fn every_cell_gets_six_elements_carrying_its_tag_and_fibre() {
        let (positions, fibres, tags, groups) = lattice(|_| Some(1));
        let v = voxel_mesh(
            CellInput {
                positions: &positions,
                fibres: &fibres,
                tags: &tags,
                groups: &groups,
            },
            2.5,
        );
        assert_eq!(v.cells_meshed, positions.len());
        assert_eq!(v.cells_merged, 0);
        let c = check_mesh(&v.mesh);
        assert!(c.is_valid(), "{c:?}");
        assert_eq!(c.components, 1);
        assert_abs_diff_eq!(
            c.volume_mm3,
            positions.len() as f64 * 2.5f64.powi(3),
            epsilon = 1e-2
        );
        assert_eq!(c.tag_counts.len(), positions.len());
        assert!(c.tag_counts.values().all(|n| *n == 6));
        // element centroids stay inside their own cell's cube
        for (t, cell) in v.mesh.tets.iter().zip(&v.tet_cell) {
            let centroid = t.iter().map(|i| v.mesh.points_mm[*i as usize]).sum::<Vec3>() / 4.0;
            let d = (centroid - positions[*cell as usize]).abs().max_element();
            assert!(d < 1.25, "{d}");
        }
    }

    #[test]
    fn a_rotated_lattice_gets_rotated_cubes_and_no_holes() {
        // the procedural heart's case: axis-aligned cubes would alias against this lattice
        let (flat, fibres, tags, groups) = {
            let (mut p, mut g) = (Vec::new(), Vec::new());
            for i in 0..12 {
                for j in 0..12 {
                    for k in 0..12 {
                        p.push(Vec3::new(i as f32, j as f32, k as f32) * 2.3);
                        g.push(Some(1));
                    }
                }
            }
            let n = p.len();
            (p, vec![Vec3::Y; n], (0..n as u32).collect::<Vec<_>>(), g)
        };
        let q = Quat::from_euler(glam::EulerRot::XYZ, 0.4, -0.7, 0.25);
        let positions: Vec<Vec3> = flat.iter().map(|p| q * *p).collect();
        let v = voxel_mesh(
            CellInput {
                positions: &positions,
                fibres: &fibres,
                tags: &tags,
                groups: &groups,
            },
            2.3,
        );
        assert_eq!(v.cells_merged, 0);
        assert_eq!(v.cells_meshed, positions.len());
        let c = check_mesh(&v.mesh);
        assert!(c.is_valid(), "{c:?}");
        assert_eq!(c.components, 1);
        assert_abs_diff_eq!(
            c.volume_mm3,
            positions.len() as f64 * 2.3f64.powi(3),
            epsilon = 5.0
        );
        // every cell is the centre of its own eight corners
        for (t, cell) in v.mesh.tets.iter().zip(&v.tet_cell) {
            let centroid = t.iter().map(|i| v.mesh.points_mm[*i as usize]).sum::<Vec3>() / 4.0;
            assert!(centroid.distance(positions[*cell as usize]) < 2.3 * 3f32.sqrt() / 2.0);
        }
    }

    #[test]
    fn scattered_points_fall_back_to_axis_aligned_cubes() {
        let p = [Vec3::ZERO, Vec3::new(9.1, 3.3, 1.0), Vec3::new(-4.2, 7.7, 2.9)];
        assert_eq!(lattice_frame(&p, 2.0), Quat::IDENTITY);
    }

    #[test]
    fn groups_share_no_nodes_so_the_av_ring_insulates() {
        let (positions, fibres, tags, groups) = lattice(|i| Some(u8::from(i >= 2)));
        let v = voxel_mesh(
            CellInput {
                positions: &positions,
                fibres: &fibres,
                tags: &tags,
                groups: &groups,
            },
            2.5,
        );
        let c = check_mesh(&v.mesh);
        assert!(c.is_valid(), "{c:?}");
        assert_eq!(c.components, 2, "the two groups must be separate pieces");
        // the shared plane exists twice, once per group
        assert_eq!(v.mesh.points_mm.len(), 2 * 3 * 4 * 3);
    }

    #[test]
    fn off_lattice_and_conduction_cells_are_counted_not_lost_silently() {
        let (mut positions, mut fibres, mut tags, mut groups) = lattice(|_| Some(1));
        positions.push(positions[0] + Vec3::splat(0.2)); // falls into cell 0's cube
        fibres.push(Vec3::Z);
        tags.push(99);
        groups.push(Some(1));
        positions.push(positions[0]); // conduction system
        fibres.push(Vec3::Z);
        tags.push(98);
        groups.push(None);
        let v = voxel_mesh(
            CellInput {
                positions: &positions,
                fibres: &fibres,
                tags: &tags,
                groups: &groups,
            },
            2.5,
        );
        assert_eq!(v.cells_merged, 1);
        assert_eq!(v.cells_excluded, 1);
        assert_eq!(v.cells_meshed, 24);
        assert!(!v.mesh.tags.contains(&99) && !v.mesh.tags.contains(&98));
    }
}
