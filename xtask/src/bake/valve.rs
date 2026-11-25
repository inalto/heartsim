//! The tricuspid valve of the default pack (brief B35): the Texas TriValve 1.1 healthy
//! quasi-static model (Mathur et al.; CC BY 4.0), decimated and fitted to this heart's tricuspid
//! disc, as an [`AnimatedValve`].
//!
//! Steps (each reported):
//! 1. read the Abaqus model (`.inp`: nodes, the three leaflets' S4R shells, the T3D2 chordae, the
//!    annulus and free-edge node sets) and the 21 result frames (`.vtu`, LZMA blocks decoded here);
//!    check that frame 0 is the model's nodes and that every frame has the model's leaflet cells;
//! 2. quads to triangles, decimated by half-edge collapse (each kept vertex is a source node, so
//!    every frame keeps its shape exactly at the kept nodes) under the summed quadric error of
//!    several frames, keeping the annulus, the free edge and the seams between leaflets; chordae
//!    chains thinned to every third node;
//! 3. fit: a similarity (the donor annulus onto the disc rim: centre, plane, size, and the septal
//!    leaflet turned toward the septum), then per frame a warp that puts each annulus vertex on the
//!    rim and fades to zero toward the free edge; the chordae origins (the papillary-muscle tips
//!    the source pins) moved to the nearest right-ventricular wall, the move fading along each
//!    chord to its insertion;
//! 4. keyframes: the frames whose free edge comes monotonically closer to the coaptation point,
//!    thinned where a frame is the interpolation of its neighbours; leaflet vertices that leave
//!    the right heart's blood pushed back in by a smooth correction;
//! 5. quantised offsets from the rest (open) shape.

use super::abaqus;
use super::fetch::{PinnedRepo, TEXAS_TRIVALVE_1_1};
use anyhow::{Context, Result, bail, ensure};
use glam::{DMat3, DVec3, Mat3, Vec3};
use heartsim_geometry::bridge::vtk::parse_vtu_with;
use heartsim_geometry::valve::{DiscRim, ValveKeyframe, smallest_eigenvector, split_discs};
use heartsim_geometry::{AnatomyPack, AnimatedValve, Label, LabelVolume, MeshKind};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::Path;

/// Target triangle count of the leaflet surface (brief: about 3 000).
pub const TARGET_TRIANGLES: usize = 3_000;
/// Keep one chordae node in this many along a chain.
const CHORD_STRIDE: usize = 3;
/// A frame is dropped when its neighbours' interpolation reproduces it within this, mm.
const KEYFRAME_TOLERANCE_MM: f32 = 0.25;
/// Successive keyframes must bring the free edge at least this much nearer the coaptation point,
/// mm: the late frames of the load sequence (the shut valve bulging under rising pressure) barely
/// change the closure, and between two such shapes the interpolated closure need not be monotone.
const MIN_CLOSURE_STEP_MM: f32 = 0.05;
/// Samples along each keyframe segment for the monotone-closure check.
const SEGMENT_SAMPLES: usize = 200;
/// The containment correction pushes a leaflet vertex until it is at most this far outside the
/// right heart's blood, mm (the acceptance test allows 1 mm).
const CONTAIN_TARGET_MM: f32 = 0.6;
const FRAMES: usize = 21;
const LEAFLETS: [&str; 3] = ["Anterior", "Septal", "Posterior"];

/// What the bake produced and the numbers the brief asks to report.
#[derive(Debug)]
pub struct ValveBake {
    pub valve: AnimatedValve,
    pub report: Vec<String>,
}

/// The attribution the CC BY 4.0 licence of the valve source requires, appended to the pack's
/// `LICENSE`.
pub const VALVE_LICENSE: &str = "TRICUSPID VALVE\n\
Mathur M, Meador WD, Malinowski M, Jazwiec T, Timek TA, Rausch MK. Texas TriValve 1.0: a\n\
reverse-engineered, open model of the human tricuspid valve. Engineering with Computers 2022.\n\
doi:10.1007/s00366-022-01659-w. Model files: Texas TriValve 1.1 (a second donor valve), Soft Tissue\n\
Biomechanics Laboratory, https://github.com/SoftTissueBiomechanicsLab/Texas_TriValve_1.1, commit\n\
cfbdad5ab8f2e02934f317dde1c2566963c98f4b, folder Healthy_QuasiStatic (Healthy_QuasiStatic.inp and\n\
the 21 frames Healthy_QuasiStatic_0..20.vtu). Licence: Creative Commons Attribution 4.0\n\
International, https://creativecommons.org/licenses/by/4.0/\n\
Changes by heartsim (modified): decimated, fitted to another heart's annulus, re-timed. The leaflet\n\
midsurface was triangulated and decimated and the chordae thinned; the valve was scaled, turned\n\
and warped onto the pack heart's tricuspid annulus, its chordae origins moved to the right\n\
ventricular wall and small excursions outside the right heart's blood pushed back; a subset of the\n\
result frames (a load sequence from open to shut, not a timed recording) is replayed against\n\
heartsim's designed valve timing (valve_tricuspid.bin). One donor's valve fitted to another\n\
heart; not reviewed by a clinician.\n";

/// Fetch the source (pinned commit, checked) and bake the valve for `pack`.
pub fn bake_for_pack(source_root: &Path, offline: bool, pack: &AnatomyPack) -> Result<ValveBake> {
    let dir = super::fetch::ensure_repo(source_root, &TEXAS_TRIVALVE_1_1, offline)?;
    bake(&dir, pack, &TEXAS_TRIVALVE_1_1)
}

fn lzma_block(name: &str, block: &[u8]) -> Result<Vec<u8>, String> {
    if name != "vtkLZMADataCompressor" {
        return Err(format!("unsupported compressor `{name}`"));
    }
    let mut out = Vec::new();
    lzma_rust2::XzReader::new(block, false)
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// Read one result frame: positions (in the `.inp` node order) and the leaflet cells.
fn read_frame(path: &Path) -> Result<heartsim_geometry::bridge::vtk::VtkGrid> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse_vtu_with(&bytes, &lzma_block).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
}

/// The source, read and checked (step 1).
struct Source {
    /// All nodes in `.inp` order (leaflet nodes first, then chordae nodes).
    frames: Vec<Vec<DVec3>>,
    /// Leaflet quads (node indices) and their leaflet ids, in the `.inp` order.
    quads: Vec<[usize; 4]>,
    quad_leaflet: Vec<u8>,
    leaflet_nodes: usize,
    annulus: BTreeSet<usize>,
    free_edge: BTreeSet<usize>,
    /// Chordae segments (node indices) and their leaflet group.
    chords: Vec<([usize; 2], u8)>,
    chord_area_mm2: f64,
}

fn bytecount_of(v: &[u8], x: u8) -> usize {
    v.iter().fold(0, |n, b| n + usize::from(*b == x))
}

fn read_source(dir: &Path, report: &mut Vec<String>) -> Result<Source> {
    let inp_path = dir.join("Healthy_QuasiStatic/Healthy_QuasiStatic.inp");
    let model = abaqus::parse(
        &std::fs::read_to_string(&inp_path).with_context(|| format!("reading {}", inp_path.display()))?,
    )?;
    let index = model.node_index();
    let n = model.nodes.len();
    for (k, (id, _)) in model.nodes.iter().enumerate() {
        ensure!(
            *id as usize == k + 1,
            "node ids are not 1..{n} in order (node {id} at {k})"
        );
    }
    let leaflet_nodes = model.nsets.get("Leaflet_Nodes").map_or(0, Vec::len);
    ensure!(leaflet_nodes > 0, "no Leaflet_Nodes set");
    let mut quads = Vec::new();
    let mut quad_leaflet = Vec::new();
    for (l, name) in LEAFLETS.iter().enumerate() {
        let (ty, els) = model
            .element_blocks
            .get(&format!("{name}_Leaflet_Elements"))
            .with_context(|| format!("no {name}_Leaflet_Elements block"))?;
        ensure!(
            ty.eq_ignore_ascii_case("S4R"),
            "{name} leaflet elements are {ty}, not S4R"
        );
        for (_, nodes) in els {
            ensure!(nodes.len() == 4, "a leaflet element without four nodes");
            let q = [0, 1, 2, 3].map(|i| index[&nodes[i]]);
            ensure!(
                q.iter().all(|&i| i < leaflet_nodes),
                "a leaflet element on a chordae node"
            );
            quads.push(q);
            quad_leaflet.push(l as u8);
        }
    }
    let set = |name: &str| -> Result<BTreeSet<usize>> {
        Ok(model
            .nsets
            .get(name)
            .with_context(|| format!("no {name} set"))?
            .iter()
            .map(|id| index[id])
            .collect())
    };
    let annulus = set("Annular_Nodes")?;
    let free_edge = set("Free_Edge_Nodes")?;
    let (ty, chord_els) = model
        .element_blocks
        .get("Chordae_Elements")
        .context("no Chordae_Elements block")?;
    ensure!(
        ty.eq_ignore_ascii_case("T3D2"),
        "chordae elements are {ty}, not T3D2"
    );
    let mut group_of: HashMap<u32, u8> = HashMap::new();
    let mut areas = Vec::new();
    for (l, name) in LEAFLETS.iter().enumerate() {
        let set = format!("{name}_Chordae");
        for id in model.elsets.get(&set).with_context(|| format!("no {set} set"))? {
            group_of.insert(*id, l as u8);
        }
        if let Some(a) = model.sections.get(&set) {
            areas.push(*a);
        }
    }
    let mut chords = Vec::new();
    for (id, nodes) in chord_els {
        ensure!(nodes.len() == 2, "a chordae element without two nodes");
        let g = *group_of
            .get(id)
            .with_context(|| format!("chordae element {id} in no chordae set"))?;
        chords.push(([index[&nodes[0]], index[&nodes[1]]], g));
    }
    let chord_area_mm2 = areas.iter().sum::<f64>() / areas.len().max(1) as f64;
    let inp_nodes: Vec<DVec3> = model.nodes.iter().map(|(_, x)| DVec3::from_array(*x)).collect();
    report.push(format!(
        "source: {n} nodes ({leaflet_nodes} leaflet), {} leaflet quads (anterior {}, septal {}, posterior {}), \
         {} chordae segments, annulus {} and free edge {} nodes, chord area {chord_area_mm2:.3} mm²",
        quads.len(),
        bytecount_of(&quad_leaflet, 0),
        bytecount_of(&quad_leaflet, 1),
        bytecount_of(&quad_leaflet, 2),
        chords.len(),
        annulus.len(),
        free_edge.len()
    ));
    // the frames
    let mut frames = Vec::with_capacity(FRAMES);
    let mut leaflet_code: [Option<f64>; 3] = [None; 3];
    for k in 0..FRAMES {
        let path = dir.join(format!("Healthy_QuasiStatic/Healthy_QuasiStatic_{k}.vtu"));
        let g = read_frame(&path)?;
        ensure!(
            g.points.len() == n,
            "frame {k}: {} points, the model has {n} nodes",
            g.points.len()
        );
        ensure!(
            g.cell_count() == quads.len(),
            "frame {k}: {} cells, the model has {} leaflet shells",
            g.cell_count(),
            quads.len()
        );
        for (c, q) in quads.iter().enumerate() {
            let cell = &g.connectivity[g.offsets[c] as usize..g.offsets[c + 1] as usize];
            ensure!(
                cell.len() == 4 && cell.iter().zip(q).all(|(a, b)| *a as usize == *b),
                "frame {k}: cell {c} is not the model's leaflet shell {c}"
            );
        }
        if let Some(sets) = g.cell_fields.iter().find(|f| f.name == "element_sets") {
            for (c, l) in quad_leaflet.iter().enumerate() {
                let code = sets.data[c];
                let slot = &mut leaflet_code[usize::from(*l)];
                ensure!(
                    slot.is_none_or(|s| s.to_bits() == code.to_bits()),
                    "frame {k}: element_sets is not constant on the {} leaflet",
                    LEAFLETS[usize::from(*l)]
                );
                *slot = Some(code);
            }
        }
        frames.push(g.points.iter().map(|p| DVec3::from_array(*p)).collect::<Vec<_>>());
    }
    // frame 0 is the model's nodes (the frames store f32, the .inp 7 significant digits)
    let mut worst = 0.0f64;
    let mut worst_rel = 0.0f64;
    for (a, b) in frames[0].iter().zip(&inp_nodes) {
        let d = (*a - *b).abs().max_element();
        worst = worst.max(d);
        worst_rel = worst_rel.max(d / b.abs().max_element().max(1.0));
    }
    ensure!(
        worst_rel < 2e-6,
        "frame 0 differs from the .inp nodes by {worst:.2e} mm (relative {worst_rel:.2e})"
    );
    // which nodes move: the leaflets and the chordae; the chordae origins stay pinned
    let moved = |range: std::ops::Range<usize>| -> f64 {
        range
            .map(|i| {
                frames
                    .iter()
                    .map(|f| f[i].distance(frames[0][i]))
                    .fold(0.0, f64::max)
            })
            .fold(0.0, f64::max)
    };
    report.push(format!(
        "frames: {FRAMES} × {n} points and {} leaflet cells (VTK quads), cell connectivity equal to the .inp \
         shells in every frame; element_sets codes anterior {:?}, septal {:?}, posterior {:?}; frame 0 = .inp \
         nodes within {worst:.1e} mm (relative {worst_rel:.1e}); largest move over the frames: leaflet nodes \
         {:.2} mm, chordae nodes {:.2} mm",
        quads.len(),
        leaflet_code[0],
        leaflet_code[1],
        leaflet_code[2],
        moved(0..leaflet_nodes),
        moved(leaflet_nodes..n)
    ));
    Ok(Source {
        frames,
        quads,
        quad_leaflet,
        leaflet_nodes,
        annulus,
        free_edge,
        chords,
        chord_area_mm2,
    })
}

// --- decimation ---------------------------------------------------------------------------------

/// A symmetric 4×4 quadric (Garland & Heckbert 1997), upper triangle.
#[derive(Debug, Clone, Copy, Default)]
struct Quadric([f64; 10]);

impl Quadric {
    fn plane(n: DVec3, d: f64, w: f64) -> Self {
        let (a, b, c) = (n.x, n.y, n.z);
        Self(
            [
                a * a,
                a * b,
                a * c,
                a * d,
                b * b,
                b * c,
                b * d,
                c * c,
                c * d,
                d * d,
            ]
            .map(|x| x * w),
        )
    }
    fn add(&mut self, o: &Self) {
        for (a, b) in self.0.iter_mut().zip(o.0) {
            *a += b;
        }
    }
    fn error(&self, p: DVec3) -> f64 {
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
}

#[derive(Debug, PartialEq)]
struct Candidate {
    cost: f64,
    /// Removed vertex and the vertex it collapses into.
    u: u32,
    v: u32,
    stamp: u64,
}
impl Eq for Candidate {}
impl Ord for Candidate {
    fn cmp(&self, o: &Self) -> Ordering {
        // min-heap on cost, ties by vertex ids (deterministic)
        o.cost
            .total_cmp(&self.cost)
            .then_with(|| o.u.cmp(&self.u))
            .then_with(|| o.v.cmp(&self.v))
    }
}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// The decimated leaflet surface: which source vertices are kept, where each removed vertex went,
/// and the triangles over the source vertex indices.
struct Decimated {
    /// For every source leaflet vertex, the kept vertex it was collapsed into (itself if kept).
    rep: Vec<usize>,
    triangles: Vec<[usize; 3]>,
    triangle_leaflet: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tag {
    Interior,
    Seam,
    Annulus,
    FreeEdge,
    OtherBoundary,
}

/// Half-edge collapse of the leaflet surface to `target` triangles (see the module docs).
#[allow(clippy::too_many_lines)]
fn decimate(
    frames: &[&[DVec3]],
    tris_in: &[[usize; 3]],
    tri_leaflet: &[u8],
    nv: usize,
    annulus: &BTreeSet<usize>,
    free_edge: &BTreeSet<usize>,
    target: usize,
) -> Decimated {
    let mut tris: Vec<[usize; 3]> = tris_in.to_vec();
    let mut alive = vec![true; tris.len()];
    let mut vt: Vec<Vec<usize>> = vec![Vec::new(); nv];
    for (t, tri) in tris.iter().enumerate() {
        for &v in tri {
            vt[v].push(t);
        }
    }
    // edges, boundary and seams
    let mut edge_tris: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (t, tri) in tris.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            edge_tris.entry((a.min(b), a.max(b))).or_default().push(t);
        }
    }
    let feature = |e: &(usize, usize), ts: &Vec<usize>| -> Option<bool> {
        // Some(true) = boundary, Some(false) = seam
        let _ = e;
        if ts.len() == 1 {
            Some(true)
        } else if ts.len() == 2 && tri_leaflet[ts[0]] != tri_leaflet[ts[1]] {
            Some(false)
        } else {
            None
        }
    };
    let mut feature_edges: BTreeMap<(usize, usize), bool> = BTreeMap::new();
    let mut fdeg = vec![0usize; nv];
    for (e, ts) in &edge_tris {
        if let Some(b) = feature(e, ts) {
            feature_edges.insert(*e, b);
            fdeg[e.0] += 1;
            fdeg[e.1] += 1;
        }
    }
    let tag: Vec<Tag> = (0..nv)
        .map(|v| {
            let on_boundary = feature_edges.iter().any(|(e, b)| *b && (e.0 == v || e.1 == v));
            if annulus.contains(&v) {
                Tag::Annulus
            } else if free_edge.contains(&v) {
                Tag::FreeEdge
            } else if on_boundary {
                Tag::OtherBoundary
            } else if fdeg[v] > 0 {
                Tag::Seam
            } else {
                Tag::Interior
            }
        })
        .collect();
    // a vertex where features meet (a commissure, the end of a seam, a change of boundary set)
    // never moves
    let locked: Vec<bool> = (0..nv)
        .map(|v| {
            if tag[v] == Tag::Interior {
                return false;
            }
            if fdeg[v] != 2 || (annulus.contains(&v) && free_edge.contains(&v)) {
                return true;
            }
            let kinds: Vec<(bool, Tag)> = feature_edges
                .iter()
                .filter(|(e, _)| e.0 == v || e.1 == v)
                .map(|(e, b)| (*b, tag[if e.0 == v { e.1 } else { e.0 }]))
                .collect();
            kinds
                .iter()
                .any(|(b, t)| *b != kinds[0].0 || (*t != tag[v] && *t != Tag::Interior))
        })
        .collect();
    // quadrics per frame
    let nf = frames.len();
    let mut quad = vec![Quadric::default(); nv * nf];
    for (fi, pos) in frames.iter().enumerate() {
        for tri in &tris {
            let [a, b, c] = tri.map(|i| pos[i]);
            let n = (b - a).cross(c - a);
            let area = n.length() * 0.5;
            if area <= 1e-12 {
                continue;
            }
            let n = n / (2.0 * area);
            let q = Quadric::plane(n, -n.dot(a), area);
            for &v in tri {
                quad[v * nf + fi].add(&q);
            }
        }
        // feature edges: a heavy plane through the edge, perpendicular to a face beside it
        for (e, ts) in &edge_tris {
            if !feature_edges.contains_key(e) {
                continue;
            }
            let t = &tris[ts[0]];
            let [a, b, c] = t.map(|i| pos[i]);
            let fnrm = (b - a).cross(c - a).normalize_or_zero();
            let dir = (pos[e.1] - pos[e.0]).normalize_or_zero();
            let pn = fnrm.cross(dir).normalize_or_zero();
            let q = Quadric::plane(pn, -pn.dot(pos[e.0]), 100.0);
            quad[e.0 * nf + fi].add(&q);
            quad[e.1 * nf + fi].add(&q);
        }
    }
    let p0 = frames[0];
    let mut stamp = vec![0u64; nv];
    let mut removed = vec![false; nv];
    let mut rep: Vec<usize> = (0..nv).collect();
    let neighbours =
        |vt: &Vec<Vec<usize>>, tris: &Vec<[usize; 3]>, alive: &Vec<bool>, u: usize| -> BTreeSet<usize> {
            let mut s = BTreeSet::new();
            for &t in &vt[u] {
                if alive[t] {
                    for &x in &tris[t] {
                        if x != u {
                            s.insert(x);
                        }
                    }
                }
            }
            s
        };
    let cost = |quad: &Vec<Quadric>, u: usize, v: usize| -> f64 {
        let mut c = 0.0;
        for (fi, pos) in frames.iter().enumerate() {
            let mut q = quad[u * nf + fi];
            q.add(&quad[v * nf + fi]);
            c += q.error(pos[v]).max(0.0);
        }
        c + 1e-3 * p0[u].distance_squared(p0[v])
    };
    let allowed = |u: usize, v: usize, fe: &BTreeMap<(usize, usize), bool>| -> bool {
        if locked[u] {
            return false;
        }
        match tag[u] {
            Tag::Interior => true,
            t => {
                let along = fe.contains_key(&(u.min(v), u.max(v)));
                along && (tag[v] == t || locked[v])
            }
        }
    };
    let mut heap = BinaryHeap::new();
    for e in edge_tris.keys() {
        for (u, v) in [(e.0, e.1), (e.1, e.0)] {
            if allowed(u, v, &feature_edges) {
                heap.push(Candidate {
                    cost: cost(&quad, u, v),
                    u: u as u32,
                    v: v as u32,
                    stamp: stamp[u] + stamp[v],
                });
            }
        }
    }
    let mut alive_count = tris.len();
    while alive_count > target {
        let Some(c) = heap.pop() else { break };
        let (u, v) = (c.u as usize, c.v as usize);
        if removed[u] || removed[v] || c.stamp != stamp[u] + stamp[v] {
            continue;
        }
        let shared: Vec<usize> = vt[u]
            .iter()
            .copied()
            .filter(|&t| alive[t] && tris[t].contains(&v))
            .collect();
        if shared.is_empty() {
            continue;
        }
        // link condition: the common neighbours are the opposite vertices of the shared triangles
        let nu = neighbours(&vt, &tris, &alive, u);
        let nvv = neighbours(&vt, &tris, &alive, v);
        let common: BTreeSet<usize> = nu.intersection(&nvv).copied().collect();
        let opposite: BTreeSet<usize> = shared
            .iter()
            .flat_map(|&t| tris[t].iter().copied())
            .filter(|&x| x != u && x != v)
            .collect();
        if common != opposite {
            continue;
        }
        // no triangle around u flips or degenerates in any frame
        let mut ok = true;
        'frames: for pos in frames {
            for &t in &vt[u] {
                if !alive[t] || shared.contains(&t) {
                    continue;
                }
                let tri = tris[t];
                let before = (pos[tri[1]] - pos[tri[0]]).cross(pos[tri[2]] - pos[tri[0]]);
                let p = |x: usize| if x == u { pos[v] } else { pos[x] };
                let after = (p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0]));
                let longest = [(0, 1), (1, 2), (2, 0)]
                    .iter()
                    .map(|&(i, j)| p(tri[i]).distance_squared(p(tri[j])))
                    .fold(0.0, f64::max);
                if before.dot(after) <= 0.0 || after.length() < 0.05 * longest {
                    ok = false;
                    break 'frames;
                }
            }
        }
        if !ok {
            continue;
        }
        // collapse u into v
        for &t in &shared {
            alive[t] = false;
            alive_count -= 1;
        }
        let ut = std::mem::take(&mut vt[u]);
        for t in ut {
            if !alive[t] {
                continue;
            }
            for x in &mut tris[t] {
                if *x == u {
                    *x = v;
                }
            }
            vt[v].push(t);
        }
        vt[v].retain(|t| alive[*t]);
        removed[u] = true;
        rep[u] = v;
        for fi in 0..nf {
            let q = quad[u * nf + fi];
            quad[v * nf + fi].add(&q);
        }
        // feature edges at u move to v
        let moved: Vec<((usize, usize), bool)> = feature_edges
            .iter()
            .filter(|(e, _)| e.0 == u || e.1 == u)
            .map(|(e, b)| (*e, *b))
            .collect();
        for (e, b) in moved {
            feature_edges.remove(&e);
            let o = if e.0 == u { e.1 } else { e.0 };
            if o != v {
                feature_edges.insert((o.min(v), o.max(v)), b);
            }
        }
        stamp[v] += 1;
        for x in neighbours(&vt, &tris, &alive, v) {
            for (a, b) in [(x, v), (v, x)] {
                if allowed(a, b, &feature_edges) {
                    heap.push(Candidate {
                        cost: cost(&quad, a, b),
                        u: a as u32,
                        v: b as u32,
                        stamp: stamp[a] + stamp[b],
                    });
                }
            }
        }
    }
    // path-compress the representatives
    for i in 0..nv {
        let mut r = rep[i];
        while rep[r] != r {
            r = rep[r];
        }
        rep[i] = r;
    }
    let mut triangles = Vec::new();
    let mut triangle_leaflet = Vec::new();
    for (t, tri) in tris.iter().enumerate() {
        if alive[t] && tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2] {
            triangles.push(*tri);
            triangle_leaflet.push(tri_leaflet[t]);
        }
    }
    Decimated {
        rep,
        triangles,
        triangle_leaflet,
    }
}

// --- geometry helpers -----------------------------------------------------------------------------

/// Multi-source Dijkstra over an undirected weighted graph: distance and nearest source per node.
fn dijkstra(adj: &[Vec<(usize, f64)>], sources: &[usize]) -> (Vec<f64>, Vec<usize>) {
    #[derive(PartialEq)]
    struct Item(f64, usize);
    impl Eq for Item {}
    impl Ord for Item {
        fn cmp(&self, o: &Self) -> Ordering {
            o.0.total_cmp(&self.0).then_with(|| o.1.cmp(&self.1))
        }
    }
    impl PartialOrd for Item {
        fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
            Some(self.cmp(o))
        }
    }
    let n = adj.len();
    let mut dist = vec![f64::INFINITY; n];
    let mut src = vec![usize::MAX; n];
    let mut heap = BinaryHeap::new();
    for &s in sources {
        dist[s] = 0.0;
        src[s] = s;
        heap.push(Item(0.0, s));
    }
    while let Some(Item(d, a)) = heap.pop() {
        if d > dist[a] {
            continue;
        }
        for &(b, w) in &adj[a] {
            let nd = d + w;
            if nd < dist[b] {
                dist[b] = nd;
                src[b] = src[a];
                heap.push(Item(nd, b));
            }
        }
    }
    (dist, src)
}

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Distance from `p` to the nearest voxel (as a box) whose label passes `want`, searching up to
/// `reach_mm`; `0` inside such a voxel, `None` when none is within reach. Also the nearest point
/// of that voxel.
pub fn distance_to_label(
    vol: &LabelVolume,
    p: Vec3,
    reach_mm: f32,
    want: &dyn Fn(u8) -> bool,
) -> Option<(f32, Vec3)> {
    if let Some(l) = vol.get(p)
        && want(l)
    {
        return Some((0.0, p));
    }
    let s = vol.spacing_mm;
    let r = (reach_mm / s).ceil() as i64;
    let rel = (p - vol.origin_mm) / s;
    let (ci, cj, ck) = (rel.x.floor() as i64, rel.y.floor() as i64, rel.z.floor() as i64);
    let [nx, ny, nz] = vol.dims.map(i64::from);
    let mut best: Option<(f32, Vec3)> = None;
    for k in (ck - r).max(0)..=(ck + r).min(nz - 1) {
        for j in (cj - r).max(0)..=(cj + r).min(ny - 1) {
            for i in (ci - r).max(0)..=(ci + r).min(nx - 1) {
                let l = vol.labels[vol.index(i as u32, j as u32, k as u32)];
                if !want(l) {
                    continue;
                }
                let lo = vol.origin_mm + Vec3::new(i as f32, j as f32, k as f32) * s;
                let q = p.clamp(lo, lo + Vec3::splat(s));
                let d = q.distance(p);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, q));
                }
            }
        }
    }
    best.filter(|(d, _)| *d <= reach_mm)
}

/// Whether a label byte is right-heart blood (the blood pool the tricuspid leaflets must stay in).
#[must_use]
pub fn right_heart_blood(l: u8) -> bool {
    l == Label::RvBlood as u8 || l == Label::RaBlood as u8
}

fn centroid_of(vol: &LabelVolume, label: Label) -> Option<Vec3> {
    let mut s = DVec3::ZERO;
    let mut n = 0usize;
    for (idx, l) in vol.labels.iter().enumerate() {
        if *l == label as u8 {
            let [i, j, k] = vol.coords(idx);
            s += vol.centre(i, j, k).as_dvec3();
            n += 1;
        }
    }
    (n > 0).then(|| (s / n as f64).as_vec3())
}

fn plane_dir(d: DVec3, n: DVec3) -> DVec3 {
    (d - n * d.dot(n)).normalize_or_zero()
}

fn frame_of(a: DVec3, b: DVec3) -> DMat3 {
    let b = plane_dir(b, a);
    DMat3::from_cols(a, b, a.cross(b))
}

// --- the bake ---------------------------------------------------------------------------------------

/// Bake the valve from the source directory `dir` for `pack` (which needs its valve-plane mesh and
/// its label volumes).
#[allow(clippy::too_many_lines)]
pub fn bake(dir: &Path, pack: &AnatomyPack, repo: &PinnedRepo) -> Result<ValveBake> {
    let mut report = vec![format!(
        "tricuspid valve: Texas TriValve 1.1 at {} ({})",
        &repo.commit[..12],
        dir.display()
    )];
    let src = read_source(dir, &mut report)?;
    let nl = src.leaflet_nodes;

    // 2. triangles (the shorter diagonal of each quad at frame 0) and the decimation
    let f0 = &src.frames[0];
    let mut tris = Vec::with_capacity(src.quads.len() * 2);
    let mut tri_leaflet = Vec::with_capacity(src.quads.len() * 2);
    for (q, l) in src.quads.iter().zip(&src.quad_leaflet) {
        let [a, b, c, d] = *q;
        if f0[a].distance_squared(f0[c]) <= f0[b].distance_squared(f0[d]) {
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        } else {
            tris.push([a, b, d]);
            tris.push([b, c, d]);
        }
        tri_leaflet.extend([*l, *l]);
    }
    let source_area: Vec<f32> = (0..3u8)
        .map(|l| {
            tris.iter()
                .zip(&tri_leaflet)
                .filter(|(_, x)| **x == l)
                .map(|(t, _)| (f0[t[1]] - f0[t[0]]).cross(f0[t[2]] - f0[t[0]]).length() * 0.5)
                .sum::<f64>() as f32
        })
        .collect();
    let quadric_frames: Vec<&[DVec3]> = [0usize, 2, 3, 4, 8, 12, 16, 20]
        .iter()
        .map(|&k| &src.frames[k][..nl])
        .collect();
    let dec = decimate(
        &quadric_frames,
        &tris,
        &tri_leaflet,
        nl,
        &src.annulus,
        &src.free_edge,
        TARGET_TRIANGLES,
    );
    // kept leaflet vertices in source order, then the chordae
    let mut kept_leaflet: Vec<usize> = (0..nl).filter(|&i| dec.rep[i] == i).collect();
    let used: BTreeSet<usize> = dec.triangles.iter().flatten().copied().collect();
    kept_leaflet.retain(|i| used.contains(i));
    let mut new_index: HashMap<usize, usize> =
        kept_leaflet.iter().enumerate().map(|(k, i)| (*i, k)).collect();
    report.push(format!(
        "decimation: {} → {} leaflet triangles, {} → {} leaflet vertices",
        tris.len(),
        dec.triangles.len(),
        nl,
        kept_leaflet.len()
    ));

    // chordae graph over source nodes, insertions mapped to the kept leaflet vertex
    let map_node = |i: usize| if i < nl { dec.rep[i] } else { i };
    let mut chord_adj: BTreeMap<usize, BTreeSet<(usize, u8)>> = BTreeMap::new();
    for ([a, b], g) in &src.chords {
        let (a, b) = (map_node(*a), map_node(*b));
        if a != b {
            chord_adj.entry(a).or_default().insert((b, *g));
            chord_adj.entry(b).or_default().insert((a, *g));
        }
    }
    let degree = |i: usize| chord_adj.get(&i).map_or(0, BTreeSet::len);
    // chains between "anchor" nodes (leaflet insertions, branch points, ends) thinned
    let is_anchor = |i: usize| i < nl || degree(i) != 2;
    let mut chord_segments: BTreeSet<(usize, usize, u8)> = BTreeSet::new();
    let mut seen_edge: BTreeSet<(usize, usize)> = BTreeSet::new();
    let anchors: Vec<usize> = chord_adj.keys().copied().filter(|&i| is_anchor(i)).collect();
    for &a in &anchors {
        for &(first, g) in &chord_adj[&a] {
            if seen_edge.contains(&(a.min(first), a.max(first))) {
                continue;
            }
            let mut chain = vec![a, first];
            seen_edge.insert((a.min(first), a.max(first)));
            while !is_anchor(*chain.last().expect("chain")) {
                let cur = *chain.last().expect("chain");
                let prev = chain[chain.len() - 2];
                let Some(&(next, _)) = chord_adj[&cur].iter().find(|(x, _)| *x != prev) else {
                    break;
                };
                seen_edge.insert((cur.min(next), cur.max(next)));
                chain.push(next);
            }
            let last = chain.len() - 1;
            let mut keep: Vec<usize> = (0..=last)
                .filter(|k| *k == 0 || *k == last || k % CHORD_STRIDE == 0)
                .collect();
            // never leave a last piece shorter than one source segment's stride
            if keep.len() > 2 && last - keep[keep.len() - 2] < CHORD_STRIDE / 2 + 1 {
                keep.remove(keep.len() - 2);
            }
            for w in keep.windows(2) {
                let (x, y) = (chain[w[0]], chain[w[1]]);
                if x != y {
                    chord_segments.insert((x.min(y), x.max(y), g));
                }
            }
        }
    }
    let chord_nodes: BTreeSet<usize> = chord_segments
        .iter()
        .flat_map(|s| [s.0, s.1])
        .filter(|&i| i >= nl)
        .collect();
    for (k, &i) in chord_nodes.iter().enumerate() {
        new_index.insert(i, kept_leaflet.len() + k);
    }
    let order: Vec<usize> = kept_leaflet
        .iter()
        .copied()
        .chain(chord_nodes.iter().copied())
        .collect();
    let nv = order.len();
    let chordae: Vec<[u32; 2]> = chord_segments
        .iter()
        .map(|s| [new_index[&s.0] as u32, new_index[&s.1] as u32])
        .collect();
    let chord_group: Vec<u8> = chord_segments.iter().map(|s| s.2).collect();
    // origins: chordae-only nodes with one segment (the pinned papillary tips)
    let mut seg_count = vec![0usize; nv];
    for s in &chordae {
        seg_count[s[0] as usize] += 1;
        seg_count[s[1] as usize] += 1;
    }
    let origins: Vec<usize> = (kept_leaflet.len()..nv).filter(|&v| seg_count[v] == 1).collect();
    report.push(format!(
        "chordae: {} → {} segments over {} nodes, {} origins",
        src.chords.len(),
        chordae.len(),
        chord_nodes.len(),
        origins.len()
    ));
    let nlk = kept_leaflet.len();
    let triangles: Vec<[u32; 3]> = dec
        .triangles
        .iter()
        .map(|t| t.map(|i| new_index[&i] as u32))
        .collect();
    let triangle_leaflet = dec.triangle_leaflet.clone();
    let annulus: Vec<u32> = kept_leaflet
        .iter()
        .enumerate()
        .filter(|(_, i)| src.annulus.contains(i))
        .map(|(k, _)| k as u32)
        .collect();
    let free_edge: Vec<u32> = kept_leaflet
        .iter()
        .enumerate()
        .filter(|(_, i)| src.free_edge.contains(i))
        .map(|(k, _)| k as u32)
        .collect();
    // per frame positions of the kept vertices
    let frames: Vec<Vec<DVec3>> = src
        .frames
        .iter()
        .map(|f| order.iter().map(|&i| f[i]).collect())
        .collect();

    // 3. the fit
    let vp = pack
        .meshes
        .iter()
        .find(|m| m.kind == MeshKind::Valve && m.name == "valve_planes")
        .context("the pack has no valve_planes mesh")?;
    let discs = split_discs(&vp.mesh).context("valve_planes does not split into four discs")?;
    let disc_pts: Vec<Vec3> = discs[1]
        .iter()
        .map(|&i| Vec3::from(vp.mesh.vertices[i as usize].position))
        .collect();
    let pulmonic: Vec3 = discs[3]
        .iter()
        .map(|&i| Vec3::from(vp.mesh.vertices[i as usize].position))
        .sum::<Vec3>()
        / discs[3].len() as f32;
    let vol = &pack
        .volumes
        .as_ref()
        .context("the pack has no label volumes")?
        .heart;
    let rv_c = centroid_of(vol, Label::RvBlood).context("no RV blood")?;
    let lv_c = centroid_of(vol, Label::LvBlood).context("no LV blood")?;
    let rim = DiscRim::new(&disc_pts, rv_c).context("degenerate tricuspid disc")?;
    let rim_pts = rim.rim_points(&disc_pts);
    let rim_c = (rim_pts.iter().copied().sum::<Vec3>() / rim_pts.len() as f32).as_dvec3();
    let n = rim.normal.as_dvec3();
    let rim_radius = rim_pts
        .iter()
        .map(|p| {
            let d = p.as_dvec3() - rim_c;
            (d - n * d.dot(n)).length()
        })
        .sum::<f64>()
        / rim_pts.len() as f64;
    // donor annulus at frame 0
    let ann_full: Vec<DVec3> = src.annulus.iter().map(|&i| f0[i]).collect();
    let ca = ann_full.iter().copied().sum::<DVec3>() / ann_full.len() as f64;
    let mut cov = Mat3::ZERO;
    for p in &ann_full {
        let q = (*p - ca).as_vec3();
        cov += Mat3::from_cols(q * q.x, q * q.y, q * q.z);
    }
    let mut nd = smallest_eigenvector(cov).as_dvec3();
    let leaflet_c = f0[..nl].iter().copied().sum::<DVec3>() / nl as f64;
    if nd.dot(leaflet_c - ca) < 0.0 {
        nd = -nd;
    }
    let donor_radius = ann_full
        .iter()
        .map(|p| plane_dir(*p - ca, nd).dot(*p - ca))
        .sum::<f64>()
        / ann_full.len() as f64;
    let leaflet_dir = |l: u8, centre: DVec3, normal: DVec3, pts: &dyn Fn(usize) -> DVec3| -> DVec3 {
        // the leaflet's annulus nodes' mean direction from the centre
        let ids: BTreeSet<usize> = src
            .quads
            .iter()
            .zip(&src.quad_leaflet)
            .filter(|(_, x)| **x == l)
            .flat_map(|(q, _)| q.iter().copied())
            .filter(|i| src.annulus.contains(i))
            .collect();
        let m = ids.iter().map(|&i| pts(i)).sum::<DVec3>() / ids.len().max(1) as f64;
        plane_dir(m - centre, normal)
    };
    let d_sept = leaflet_dir(1, ca, nd, &|i| f0[i]);
    let d_ant = leaflet_dir(0, ca, nd, &|i| f0[i]);
    let sept = plane_dir(lv_c.as_dvec3() - rim_c, n);
    let rvot = plane_dir(pulmonic.as_dvec3() - rim_c, n);
    let rot = frame_of(n, sept) * frame_of(nd, d_sept).transpose();
    ensure!(
        (rot.determinant() - 1.0).abs() < 1e-6,
        "the fit's rotation is not proper"
    );
    let scale = rim_radius / donor_radius;
    let sim = |p: DVec3| rim_c + rot * (p - ca) * scale;
    let signed_angle = |a: DVec3, b: DVec3, ax: DVec3| a.cross(b).dot(ax).atan2(a.dot(b)).to_degrees();
    let ant_now = rot * d_ant;
    report.push(format!(
        "fit: similarity scale {scale:.3} (disc rim mean radius {rim_radius:.2} mm / donor annulus {donor_radius:.2} mm); \
         orientation: donor annulus normal onto the disc normal (toward the RV blood centroid), the septal leaflet's \
         annulus direction onto the septum (disc centre → LV blood centroid, in the disc plane); the anterior leaflet \
         then lies {:.0}° from the outflow direction (→ pulmonary valve) and {:.0}° beyond the septum in the same \
         sense ({:.0}° from septum to outflow)",
        signed_angle(ant_now, rvot, n).abs(),
        signed_angle(sept, ant_now, n),
        signed_angle(sept, rvot, n)
    ));
    // the warp: annulus vertices onto the rim at their own azimuth, faded by geodesic position
    let rest_sim: Vec<Vec3> = frames[0].iter().map(|p| sim(*p).as_vec3()).collect();
    let phi: Vec<f32> = rest_sim[..nlk].iter().map(|p| rim.param(&rim_pts, *p)).collect();
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nlk];
    for t in &triangles {
        for k in 0..3 {
            let (a, b) = (t[k] as usize, t[(k + 1) % 3] as usize);
            let w = frames[0][a].distance(frames[0][b]);
            adj[a].push((b, w));
            adj[b].push((a, w));
        }
    }
    let ann_idx: Vec<usize> = annulus.iter().map(|&a| a as usize).collect();
    let fe_idx: Vec<usize> = free_edge.iter().map(|&a| a as usize).collect();
    let (da, _) = dijkstra(&adj, &ann_idx);
    let (df, _) = dijkstra(&adj, &fe_idx);
    let mut follow_weight = vec![0.0f32; nv];
    for v in 0..nlk {
        let t = if da[v].is_finite() && df[v].is_finite() {
            da[v] / (da[v] + df[v]).max(1e-9)
        } else {
            1.0
        };
        follow_weight[v] = (1.0 - smoothstep(t)) as f32;
    }
    for &a in &ann_idx {
        follow_weight[a] = 1.0;
    }
    let mut ann_by_phi: Vec<usize> = ann_idx.clone();
    ann_by_phi.sort_by(|a, b| phi[*a].total_cmp(&phi[*b]).then(a.cmp(b)));
    let warp_of = |pos_sim: &[Vec3]| -> Vec<Vec3> {
        // δ at each annulus vertex, as a function of φ
        let deltas: Vec<(f32, Vec3)> = ann_by_phi
            .iter()
            .map(|&a| (phi[a], DiscRim::rim_at(&rim_pts, phi[a]) - pos_sim[a]))
            .collect();
        let at = |f: f32| -> Vec3 {
            let m = deltas.len();
            let k = deltas.partition_point(|(p, _)| *p <= f);
            let (i, j) = ((k + m - 1) % m, k % m);
            let (pi, pj) = (deltas[i].0, deltas[j].0);
            let span = (pj - pi).rem_euclid(heartsim_geometry::valve::RIM_BINS as f32);
            let off = (f - pi).rem_euclid(heartsim_geometry::valve::RIM_BINS as f32);
            let t = if span > 1e-9 {
                (off / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            deltas[i].1 * (1.0 - t) + deltas[j].1 * t
        };
        let mut out = pos_sim.to_vec();
        for v in 0..nlk {
            if follow_weight[v] > 0.0 {
                out[v] += at(phi[v]) * follow_weight[v];
            }
        }
        for &a in &ann_idx {
            out[a] = DiscRim::rim_at(&rim_pts, phi[a]);
        }
        out
    };
    let mut fitted: Vec<Vec<Vec3>> = frames
        .iter()
        .map(|f| {
            let s: Vec<Vec3> = f.iter().map(|p| sim(*p).as_vec3()).collect();
            warp_of(&s)
        })
        .collect();
    let ann_shift = ann_idx
        .iter()
        .map(|&a| rest_sim[a].distance(fitted[0][a]))
        .fold(0.0f32, f32::max);
    // the chordae origins to the nearest RV wall (an RV blood voxel beside myocardium)
    let wall = |l: u8| -> bool { l == Label::RvMyocardium as u8 || l == Label::LvMyocardium as u8 };
    let near_wall = |p: Vec3| -> Option<(f32, Vec3)> {
        let s = vol.spacing_mm;
        let mut best: Option<(f32, Vec3)> = None;
        let r = (30.0 / s).ceil() as i64;
        let rel = (p - vol.origin_mm) / s;
        let (ci, cj, ck) = (rel.x.floor() as i64, rel.y.floor() as i64, rel.z.floor() as i64);
        let [nx, ny, nz] = vol.dims.map(i64::from);
        for k in (ck - r).max(1)..=(ck + r).min(nz - 2) {
            for j in (cj - r).max(1)..=(cj + r).min(ny - 2) {
                for i in (ci - r).max(1)..=(ci + r).min(nx - 2) {
                    let at = |a: i64, b: i64, c: i64| vol.labels[vol.index(a as u32, b as u32, c as u32)];
                    if at(i, j, k) != Label::RvBlood as u8 {
                        continue;
                    }
                    let beside = [
                        (1, 0, 0),
                        (-1, 0, 0),
                        (0, 1, 0),
                        (0, -1, 0),
                        (0, 0, 1),
                        (0, 0, -1),
                    ]
                    .iter()
                    .any(|(a, b, c)| wall(at(i + a, j + b, k + c)));
                    if !beside {
                        continue;
                    }
                    let q = vol.centre(i as u32, j as u32, k as u32);
                    let d = q.distance(p);
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, q));
                    }
                }
            }
        }
        best
    };
    let mut origin_move = Vec::with_capacity(origins.len());
    let mut chord_adj_new: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nv];
    for s in &chordae {
        let (a, b) = (s[0] as usize, s[1] as usize);
        let w = f64::from(fitted[0][a].distance(fitted[0][b]));
        chord_adj_new[a].push((b, w));
        chord_adj_new[b].push((a, w));
    }
    let mut origin_delta: HashMap<usize, Vec3> = HashMap::new();
    for &o in &origins {
        let p = fitted[0][o];
        let (d, q) = near_wall(p).with_context(|| format!("chordae origin {o}: no RV wall within 30 mm"))?;
        origin_delta.insert(o, q - p);
        origin_move.push(d);
    }
    let insertions: Vec<usize> = (0..nlk).filter(|v| !chord_adj_new[*v].is_empty()).collect();
    let (d_o, s_o) = dijkstra(&chord_adj_new, &origins);
    let (d_l, _) = dijkstra(&chord_adj_new, &insertions);
    for f in &mut fitted {
        for v in nlk..nv {
            if !(d_o[v].is_finite() && d_l[v].is_finite()) || s_o[v] == usize::MAX {
                continue;
            }
            let w = (d_l[v] / (d_o[v] + d_l[v]).max(1e-9)) as f32;
            f[v] += origin_delta[&s_o[v]] * w;
        }
    }
    origin_move.sort_by(f32::total_cmp);
    let median = origin_move.get(origin_move.len() / 2).copied().unwrap_or(0.0);
    report.push(format!(
        "warp: annulus vertices moved onto the rim by at most {ann_shift:.2} mm at rest; chordae origins moved to the \
         nearest RV wall by min {:.1} / median {median:.1} / max {:.1} mm (each: {})",
        origin_move.first().copied().unwrap_or(0.0),
        origin_move.last().copied().unwrap_or(0.0),
        origin_move.iter().map(|d| format!("{d:.1}")).collect::<Vec<_>>().join(" ")
    ));

    // 4. keyframes: monotone closure, then thinned
    let free_mean =
        |f: &[Vec3], q: Vec3| fe_idx.iter().map(|&i| f[i].distance(q)).sum::<f32>() / fe_idx.len() as f32;
    let coapt = fe_idx.iter().map(|&i| fitted[FRAMES - 1][i]).sum::<Vec3>() / fe_idx.len() as f32;
    let metric: Vec<f32> = fitted.iter().map(|f| free_mean(f, coapt)).collect();
    let mut kept = vec![0usize];
    for k in 1..FRAMES {
        if metric[k] < metric[*kept.last().expect("kept")] - MIN_CLOSURE_STEP_MM {
            kept.push(k);
        }
    }
    if *kept.last().expect("kept") != FRAMES - 1 {
        kept.retain(|&k| metric[k] > metric[FRAMES - 1] + MIN_CLOSURE_STEP_MM);
        kept.push(FRAMES - 1);
    }
    let monotone = kept.clone();
    let interp_error = |a: usize, j: usize, b: usize| -> f32 {
        let t = (metric[a] - metric[j]) / (metric[a] - metric[b]).max(1e-9);
        fitted[j]
            .iter()
            .zip(fitted[a].iter().zip(&fitted[b]))
            .map(|(p, (pa, pb))| p.distance(pa.lerp(*pb, t)))
            .fold(0.0f32, f32::max)
    };
    loop {
        let best = (1..kept.len().saturating_sub(1))
            .map(|i| (interp_error(kept[i - 1], kept[i], kept[i + 1]), i))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        match best {
            Some((e, i)) if e < KEYFRAME_TOLERANCE_MM => {
                kept.remove(i);
            }
            _ => break,
        }
    }
    report.push(format!(
        "keyframes: closure metric (mean free-edge distance to the coaptation point) per frame {}; monotone frames \
         {monotone:?} (each at least {MIN_CLOSURE_STEP_MM} mm nearer); kept {kept:?} ({} shapes; a dropped frame is \
         its neighbours' interpolation within {KEYFRAME_TOLERANCE_MM} mm)",
        metric.iter().map(|m| format!("{m:.2}")).collect::<Vec<_>>().join(" "),
        kept.len()
    ));
    let mut shapes: Vec<Vec<Vec3>> = kept.iter().map(|&k| fitted[k].clone()).collect();

    // containment: leaflet vertices back into the right heart's blood
    let mut contain_report = Vec::new();
    for (si, shape) in shapes.iter_mut().enumerate() {
        let mut moved_max = 0.0f32;
        let mut before = 0.0f32;
        for pass in 0..40 {
            let mut push = vec![Vec3::ZERO; nlk];
            let mut worst = 0.0f32;
            for v in 0..nlk {
                if follow_weight[v] >= 1.0 {
                    continue;
                }
                if let Some((d, q)) = distance_to_label(vol, shape[v], 8.0, &right_heart_blood)
                    && d > CONTAIN_TARGET_MM
                {
                    worst = worst.max(d);
                    push[v] = (q - shape[v]).normalize_or_zero() * (d - CONTAIN_TARGET_MM + 0.05);
                }
            }
            if pass == 0 {
                before = worst;
            }
            if worst <= CONTAIN_TARGET_MM {
                break;
            }
            // spread the push a little so the surface moves smoothly (keep each vertex's own)
            let mut smooth = push.clone();
            for _ in 0..2 {
                let prev = smooth.clone();
                for v in 0..nlk {
                    if follow_weight[v] >= 1.0 {
                        continue;
                    }
                    let (mut s, mut c) = (Vec3::ZERO, 0.0f32);
                    for (b, _) in &adj[v] {
                        s += prev[*b];
                        c += 1.0;
                    }
                    let avg = if c > 0.0 { s / c } else { Vec3::ZERO };
                    smooth[v] = if push[v].length_squared() > avg.length_squared() {
                        push[v]
                    } else {
                        avg * 0.5 + prev[v] * 0.5
                    };
                }
            }
            for v in 0..nlk {
                if follow_weight[v] < 1.0 {
                    shape[v] += smooth[v];
                    moved_max = moved_max.max(smooth[v].length());
                }
            }
        }
        contain_report.push(format!("shape {si}: worst {before:.2} mm before"));
        let _ = moved_max;
    }

    // 5. quantise: offsets from the rest shape
    let rest: Vec<Vec3> = shapes[0].clone();
    let max_off = shapes
        .iter()
        .flat_map(|s| s.iter().zip(&rest).map(|(p, r)| (*p - *r).abs().max_element()))
        .fold(0.0f32, f32::max);
    let quantum_mm = (1.0f32 / 1024.0).max(max_off / 32_000.0);
    let mut deq: Vec<Vec<Vec3>> = vec![rest.clone()];
    let mut offsets_all: Vec<Vec<[i16; 3]>> = Vec::new();
    for s in &shapes[1..] {
        let off: Vec<[i16; 3]> = s
            .iter()
            .zip(&rest)
            .map(|(p, r)| ((*p - *r) / quantum_mm).round().to_array().map(|x| x as i16))
            .collect();
        deq.push(
            rest.iter()
                .zip(&off)
                .map(|(r, o)| *r + Vec3::new(f32::from(o[0]), f32::from(o[1]), f32::from(o[2])) * quantum_mm)
                .collect(),
        );
        offsets_all.push(off);
    }
    let coaptation_mm = fe_idx.iter().map(|&i| deq[deq.len() - 1][i]).sum::<Vec3>() / fe_idx.len() as f32;
    let m: Vec<f32> = deq.iter().map(|s| free_mean(s, coaptation_mm)).collect();
    let (m0, m1) = (m[0], m[m.len() - 1]);
    ensure!(
        m0 > m1 + 1.0,
        "the shut shape's free edge is not nearer the coaptation point ({m0:.2} vs {m1:.2})"
    );
    // keep only shapes between which the interpolated closure is monotone: the pose interpolates
    // positions linearly, and between two nearly shut shapes that differ by bulging the free edge
    // need not approach the coaptation point steadily
    let mut idx: Vec<usize> = (0..deq.len()).collect();
    let mut lerp = vec![Vec3::ZERO; nv];
    loop {
        let mut bad = None;
        for w in 0..idx.len() - 1 {
            let (a, b) = (&deq[idx[w]], &deq[idx[w + 1]]);
            let mut prev_m = f32::INFINITY;
            for step in 0..=SEGMENT_SAMPLES {
                let t = step as f32 / SEGMENT_SAMPLES as f32;
                for ((o, pa), pb) in lerp.iter_mut().zip(a).zip(b) {
                    *o = *pa * (1.0 - t) + *pb * t;
                }
                let mm = free_mean(&lerp, coaptation_mm);
                if mm > prev_m + 1e-5 {
                    bad = Some(w);
                    break;
                }
                prev_m = mm;
            }
            if bad.is_some() {
                break;
            }
        }
        let Some(w) = bad else { break };
        // drop the segment's inner end (never the rest shape or the shut one)
        let drop = if w + 1 == idx.len() - 1 { w } else { w + 1 };
        ensure!(drop != 0, "the first segment of the closure is not monotone");
        report.push(format!(
            "keyframe of frame {} dropped: the closure is not monotone between frames {} and {}",
            kept[idx[drop]],
            kept[idx[w]],
            kept[idx[w + 1]]
        ));
        idx.remove(drop);
    }
    let mut keyframes = Vec::new();
    let mut prev = 0.0f32;
    let mut offsets_all: Vec<Option<Vec<[i16; 3]>>> = offsets_all.into_iter().map(Some).collect();
    for &k in &idx[1..] {
        let c = if k + 1 == deq.len() {
            1.0
        } else {
            (m0 - m[k]) / (m0 - m1)
        };
        ensure!(c > prev && c <= 1.0, "closure {c} after {prev}");
        prev = c;
        keyframes.push(ValveKeyframe {
            closure: c,
            offsets: offsets_all[k - 1].take().expect("each keyframe once"),
        });
    }
    report.push(format!(
        "final keyframes: frames {:?}, closures {}",
        idx.iter().map(|&k| kept[k]).collect::<Vec<_>>(),
        keyframes
            .iter()
            .map(|k| format!("{:.3}", k.closure))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    // per-vertex leaflet ids
    let mut vertex_leaflet = vec![u8::MAX; nv];
    for (t, l) in triangles.iter().zip(&triangle_leaflet) {
        for &i in t {
            if vertex_leaflet[i as usize] == u8::MAX {
                vertex_leaflet[i as usize] = *l;
            }
        }
    }
    for (s, g) in chordae.iter().zip(&chord_group) {
        for &i in s {
            if vertex_leaflet[i as usize] == u8::MAX {
                vertex_leaflet[i as usize] = *g;
            }
        }
    }
    for l in &mut vertex_leaflet {
        if *l == u8::MAX {
            *l = 0;
        }
    }
    let valve = AnimatedValve {
        name: "tricuspid".into(),
        leaflet_names: LEAFLETS.iter().map(|s| s.to_ascii_lowercase()).collect(),
        rest_mm: rest,
        leaflet_vertices: nlk as u32,
        triangles,
        triangle_leaflet,
        vertex_leaflet,
        chordae,
        chordae_radius_mm: ((src.chord_area_mm2 / std::f64::consts::PI).sqrt() * scale) as f32,
        annulus,
        free_edge,
        follow_weight,
        coaptation_mm,
        quantum_mm,
        fit_scale: scale as f32,
        source_leaflet_area_mm2: source_area.clone(),
        keyframes,
    };
    // the numbers the brief asks for
    let areas = valve.leaflet_areas_mm2(&valve.rest_mm);
    let mut line = String::from("leaflet areas at rest (fitted / source / ratio ÷ scale²):");
    for (k, name) in valve.leaflet_names.iter().enumerate() {
        write!(
            line,
            " {name} {:.0} / {:.0} mm² / {:.2}",
            areas[k],
            source_area[k],
            f64::from(areas[k]) / f64::from(source_area[k]) / (scale * scale)
        )
        .expect("string write");
    }
    report.push(line);
    let mut worst = (0.0f32, 0usize, 0usize, Vec3::ZERO);
    let mut pos = Vec::new();
    for k in 0..valve.keyframe_count() {
        valve.keyframe_positions(k, &mut pos);
        for (v, p) in pos[..nlk].iter().enumerate() {
            if let Some((d, _)) = distance_to_label(vol, *p, 12.0, &right_heart_blood)
                && d > worst.0
            {
                worst = (d, k, v, *p);
            }
        }
    }
    report.push(format!(
        "containment: {}; after: worst leaflet vertex {:.2} mm outside the right heart's blood (shape {}, vertex {}, \
         at {:.1?}, label there {:?})",
        contain_report.join(", "),
        worst.0,
        worst.1,
        worst.2,
        worst.3,
        vol.get(worst.3).and_then(Label::from_u8)
    ));
    let bytes = valve.encode().len();
    report.push(format!(
        "valve file: {} vertices ({} leaflet), {} triangles, {} chordae segments, {} shapes, quantum {:.5} mm, {} bytes",
        valve.rest_mm.len(),
        valve.leaflet_vertices,
        valve.triangles.len(),
        valve.chordae.len(),
        valve.keyframe_count(),
        valve.quantum_mm,
        bytes
    ));
    if valve.keyframes.is_empty() {
        bail!("no keyframe survived");
    }
    Ok(ValveBake { valve, report })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The LZMA payloads of a VTU frame decode through the geometry crate's reader with this
    /// crate's block decoder. The fixture is hand-made (`fixtures/lzma_quad.vtu`: one quad, in the
    /// layout VTK writes with `vtkLZMADataCompressor`, generated with Python's `lzma` module), not
    /// taken from any dataset; without the decoder the reader refuses the file.
    #[test]
    fn lzma_frames_decode() {
        let bytes = include_bytes!("fixtures/lzma_quad.vtu");
        let g = parse_vtu_with(bytes, &lzma_block).expect("decodes");
        assert_eq!(g.points.len(), 4);
        assert_eq!(g.points[2], [1.0, 1.0, 0.5]);
        assert_eq!(g.connectivity, vec![0, 1, 2, 3]);
        assert_eq!(g.cell_count(), 1);
        assert_eq!(g.cell_fields[0].name, "element_sets");
        assert_eq!(g.cell_fields[0].data, vec![2.0]);
        assert!(heartsim_geometry::bridge::vtk::parse_vtu(bytes).is_err());
        assert!(lzma_block("vtkLZ4DataCompressor", &[]).is_err());
    }

    /// A small surface: the decimation keeps the boundary and the seam between two leaflets,
    /// never flips a triangle, and every kept triangle keeps its leaflet.
    #[test]
    fn decimation_keeps_boundaries_seams_and_leaflets() {
        // a 21 × 11 grid, columns 0..10 leaflet 0 and 10..20 leaflet 1, gently curved
        let (nx, ny) = (21usize, 11usize);
        let pos: Vec<DVec3> = (0..ny)
            .flat_map(|j| {
                (0..nx).map(move |i| {
                    let (x, y) = (i as f64, j as f64);
                    DVec3::new(x, y, 0.02 * (x - 10.0).powi(2))
                })
            })
            .collect();
        let id = |i: usize, j: usize| j * nx + i;
        let mut tris = Vec::new();
        let mut leaf = Vec::new();
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let l = u8::from(i >= 10);
                tris.push([id(i, j), id(i + 1, j), id(i + 1, j + 1)]);
                tris.push([id(i, j), id(i + 1, j + 1), id(i, j + 1)]);
                leaf.extend([l, l]);
            }
        }
        let annulus: BTreeSet<usize> = (0..nx).map(|i| id(i, 0)).collect();
        let free: BTreeSet<usize> = (0..nx).map(|i| id(i, ny - 1)).collect();
        let d = decimate(&[&pos], &tris, &leaf, pos.len(), &annulus, &free, 120);
        assert!(
            d.triangles.len() <= 140 && d.triangles.len() >= 60,
            "{}",
            d.triangles.len()
        );
        let kept: BTreeSet<usize> = d.triangles.iter().flatten().copied().collect();
        // the four corners and the seam's ends never move
        for c in [
            id(0, 0),
            id(nx - 1, 0),
            id(0, ny - 1),
            id(nx - 1, ny - 1),
            id(10, 0),
            id(10, ny - 1),
        ] {
            assert!(kept.contains(&c), "corner {c} removed");
        }
        // boundary vertices only collapse along their own boundary: the annulus row stays at y = 0
        for (v, r) in d.rep.iter().enumerate() {
            if annulus.contains(&v) {
                assert!(
                    annulus.contains(r),
                    "annulus vertex {v} collapsed off the annulus"
                );
            }
            if v % nx == 10 {
                assert_eq!(*r % nx, 10, "seam vertex {v} collapsed off the seam");
            }
        }
        for (t, l) in d.triangles.iter().zip(&d.triangle_leaflet) {
            let n = (pos[t[1]] - pos[t[0]]).cross(pos[t[2]] - pos[t[0]]);
            assert!(n.z > 0.0, "a triangle flipped");
            let centre_x = t.iter().map(|&i| pos[i].x).sum::<f64>() / 3.0;
            assert_eq!(*l, u8::from(centre_x > 10.0), "a triangle changed leaflet");
        }
    }

    /// Brief B35: two bakes from the same pinned source give the same bytes, and the committed
    /// default pack holds them. Skipped when the sources are not on disk; ignored by default:
    /// `cargo test -p xtask --release -- --ignored valve_bake`.
    #[test]
    #[ignore = "reads assets/anatomy-src/texas-trivalve-1.1 (download with `cargo xtask bake-valve`)"]
    fn valve_bake_is_reproducible_and_committed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let src = root.join("assets/anatomy-src");
        if !src.join(TEXAS_TRIVALVE_1_1.dir).is_dir() {
            eprintln!("sources not on disk: skipped");
            return;
        }
        let pack = AnatomyPack::load(&root.join("assets/anatomy/default")).expect("default pack");
        let a = bake_for_pack(&src, true, &pack).expect("bake").valve.encode();
        let b = bake_for_pack(&src, true, &pack).expect("bake").valve.encode();
        assert!(a == b, "two valve bakes differ");
        let committed =
            std::fs::read(root.join("assets/anatomy/default/valve_tricuspid.bin")).expect("committed valve");
        assert!(a == committed, "the committed valve is not what the bake gives");
    }
}
