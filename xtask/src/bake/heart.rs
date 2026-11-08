//! Four-chamber heart from the Rodero et al. 2021 `average` mesh (CC BY 4.0, see
//! `docs/research/anatomy-meshes.md`): render surfaces, cell cloud, fibres, regions, AHA segments,
//! conduction-system cells and landmarks, in the pack's heart frame.
//!
//! The dataset gives tagged tetrahedra, rule-based ventricular fibres and universal ventricular
//! coordinates (ρ transmural, φ rotational, Z apex–base, V LV/RV). Everything it lacks — vessel
//! tubes beyond the cropped inlets, an appendage body, the coronary sinus, the conduction
//! system, atrial sub-regions — is constructed here with the same rules and the same canonical
//! conventions as `heartsim_geometry::procedural::heart`, so the engines see the same landmarks
//! and the pose is the measured one (`APEX_DIRECTION`, `SEPTAL_DIRECTION`).
//!
//! **Canonical frame** (as in the procedural generator): LV long axis along −y (apex at −y), the
//! septum at azimuth 160° in the x–z plane (0° = lateral free wall, 90° = anterior, 270° =
//! inferior), origin at the ventricular centroid. `heart_rotation()` then tilts it into the thorax.

use super::surface::{decimate, faces_to_mesh, merge, swept_ring, swept_tube, taubin_smooth, transform};
use super::tet::{FittedDisc, TetMesh, fit_disc};
use anyhow::{Context, Result, bail};
use glam::{Mat3, Quat, Vec3};
use heartsim_core::Region;
use heartsim_geometry::procedural::heart::{
    LAF_TERMINAL, LPF_TERMINAL, SEPTAL_AZIMUTH_DEG, canonical_aha, heart_rotation, rv_segment,
};
use heartsim_geometry::{CellCloud, MeshKind, NamedMesh, TriMesh};
use std::collections::{BTreeMap, HashMap};

/// Element tags of the Rodero/Strocchi meshes, confirmed on `average.vtk` by adjacency (the
/// mitral plane touches LV and LA, the aortic plane LV and aorta, …) and position (the appendage
/// inlet is the anterior one, the right veins the ones on the RA side, the SVC the superior one).
pub mod tag {
    pub const LV: u16 = 1;
    pub const RV: u16 = 2;
    pub const LA: u16 = 3;
    pub const RA: u16 = 4;
    pub const AORTA: u16 = 5;
    pub const PA: u16 = 6;
    pub const MITRAL: u16 = 7;
    pub const TRICUSPID: u16 = 8;
    pub const AORTIC: u16 = 9;
    pub const PULMONARY: u16 = 10;
    pub const LAA: u16 = 11;
    pub const LSPV: u16 = 12;
    pub const LIPV: u16 = 13;
    pub const RIPV: u16 = 14;
    pub const RSPV: u16 = 15;
    pub const SVC: u16 = 16;
    pub const IVC: u16 = 17;
    /// The ring of atrial wall around each inlet carries the inlet's tag plus this offset.
    pub const BORDER_OFFSET: u16 = 7;
}

/// Tunables of the heart bake.
#[derive(Debug, Clone, Copy)]
pub struct HeartOptions {
    pub cell_spacing_mm: f32,
    pub chamber_triangles: usize,
    pub vessel_triangles: usize,
}

/// Output of [`bake_heart`], in the heart frame.
pub struct BakedHeart {
    pub meshes: Vec<NamedMesh>,
    pub cells: CellCloud,
    pub pmj_seeds: BTreeMap<String, Vec<u32>>,
    pub aha_segments: Vec<u8>,
    /// Mitral-plane centre to the farthest LV node, mm: the size measure the torso is scaled to
    /// (the same measure is taken on the torso subject's own LV).
    pub lv_length_mm: f32,
    pub report: Vec<String>,
}

const CONDUCTION_SPACING_MM: f32 = 1.5;
/// Procedural LV span in canonical y (apex −55 … base +25): the heights of the procedural
/// landmark table are mapped onto the real heart as fractions of this span.
const PROCEDURAL_APEX_Y: f32 = -55.0;
const PROCEDURAL_BASE_Y: f32 = 25.0;

#[derive(Default)]
struct Cells {
    positions: Vec<Vec3>,
    fibres: Vec<Vec3>,
    regions: Vec<Region>,
    depth: Vec<f32>,
    aha: Vec<u8>,
}

impl Cells {
    fn push(&mut self, p: Vec3, f: Vec3, r: Region, d: f32, aha: u8) -> u32 {
        self.positions.push(p);
        self.fibres.push(f.normalize_or_zero());
        self.regions.push(r);
        self.depth.push(d);
        self.aha.push(aha);
        (self.positions.len() - 1) as u32
    }
    fn nearest(&self, p: Vec3, pred: impl Fn(usize) -> bool) -> Option<u32> {
        let mut best: Option<(f32, u32)> = None;
        for (i, q) in self.positions.iter().enumerate() {
            if pred(i) {
                let d = q.distance_squared(p);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, i as u32));
                }
            }
        }
        best.map(|b| b.1)
    }
    fn region(&self, i: usize) -> Region {
        self.regions[i]
    }
}

/// Sample a polyline at `spacing`, returning positions with unit tangents.
fn sample_polyline(points: &[Vec3], spacing: f32) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    let mut carry = 0.0f32;
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = b - a;
        let len = seg.length();
        if len < 1e-6 {
            continue;
        }
        let t = seg / len;
        let mut s = carry;
        while s <= len {
            out.push((a + t * s, t));
            s += spacing;
        }
        carry = s - len;
    }
    if out.is_empty()
        && let (Some(a), Some(b)) = (points.first(), points.last())
    {
        out.push((*a, (*b - *a).normalize_or_zero()));
    }
    out
}

fn distance_to_polyline(p: Vec3, points: &[Vec3]) -> f32 {
    points
        .windows(2)
        .map(|w| {
            let ab = w[1] - w[0];
            let t = ((p - w[0]).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
            p.distance(w[0] + ab * t)
        })
        .fold(f32::INFINITY, f32::min)
}

fn polyline_tangent_at(p: Vec3, points: &[Vec3]) -> Vec3 {
    let mut best = (f32::INFINITY, Vec3::X);
    for w in points.windows(2) {
        let ab = w[1] - w[0];
        let t = ((p - w[0]).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
        let d = p.distance(w[0] + ab * t);
        if d < best.0 {
            best = (d, ab.normalize_or_zero());
        }
    }
    best.1
}

fn blob(centre: Vec3, r: f32, spacing: f32) -> Vec<Vec3> {
    let n = (r / spacing).ceil() as i32;
    let mut out = Vec::new();
    for x in -n..=n {
        for y in -n..=n {
            for z in -n..=n {
                let p = centre + Vec3::new(x as f32, y as f32, z as f32) * spacing;
                if p.distance(centre) <= r {
                    out.push(p);
                }
            }
        }
    }
    if out.is_empty() {
        out.push(centre);
    }
    out
}

fn azimuth_deg(p: Vec3) -> f32 {
    p.z.atan2(p.x).to_degrees().rem_euclid(360.0)
}

fn angle_diff_deg(a: f32, b: f32) -> f32 {
    ((a - b + 180.0).rem_euclid(360.0) - 180.0).abs()
}

/// Rotation taking `from_a → to_a` and `from_b → to_b` (both pairs orthonormalised).
fn rotation_between(from_a: Vec3, from_b: Vec3, to_a: Vec3, to_b: Vec3) -> Quat {
    let fa = from_a.normalize();
    let fb = (from_b - fa * from_b.dot(fa)).normalize();
    let ta = to_a.normalize();
    let tb = (to_b - ta * to_b.dot(ta)).normalize();
    let source = Mat3::from_cols(fa, fb, fa.cross(fb));
    let target = Mat3::from_cols(ta, tb, ta.cross(tb));
    Quat::from_mat3(&(target * source.transpose())).normalize()
}

/// Percentile of a sorted slice.
fn percentile(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f32 * q).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

/// Bounded proximity query: is any of the stored points within `r` (≤ the bin size) of `p`?
struct NearSet {
    cell: f32,
    bins: HashMap<(i32, i32, i32), Vec<Vec3>>,
}

impl NearSet {
    fn new(points: &[Vec3], cell: f32) -> Self {
        let mut bins: HashMap<(i32, i32, i32), Vec<Vec3>> = HashMap::new();
        for p in points {
            bins.entry(Self::key(*p, cell)).or_default().push(*p);
        }
        Self { cell, bins }
    }
    fn key(p: Vec3, cell: f32) -> (i32, i32, i32) {
        let q = (p / cell).floor();
        (q.x as i32, q.y as i32, q.z as i32)
    }
    fn within(&self, p: Vec3, r: f32) -> bool {
        let k = Self::key(p, self.cell);
        let r2 = r * r;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(list) = self.bins.get(&(k.0 + dx, k.1 + dy, k.2 + dz))
                        && list.iter().any(|q| q.distance_squared(p) <= r2)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Point of the inlet discs and valve planes in canonical coordinates.
struct Discs {
    mitral: FittedDisc,
    tricuspid: FittedDisc,
    aortic: FittedDisc,
    pulmonary: FittedDisc,
    laa: FittedDisc,
    pvs: [FittedDisc; 4],
    svc: FittedDisc,
    ivc: FittedDisc,
}

/// Bake the heart. `mesh` is the Rodero `average.vtk` in its file frame (mm, DICOM LPS).
/// Lattice points this close to an atrial tetrahedron's centroid become atrial cells even when
/// they lie outside the tissue (see the sampling notes in `bake_heart`).
const ATRIAL_REACH_MM: f32 = 0.5;

/// Radius of the Purkinje–myocardial junction patches the fascicles and the right bundle seed,
/// mm. A fascicle inserts at the base of its papillary muscle over about a centimetre; from
/// there the network (the engine's fast endocardial layer) spreads over the ventricle, so the
/// endocardium takes ≈ 20–25 ms to activate as in Durrer 1970. The procedural pack used 25 mm
/// patches, which on a real-sized heart light up most of the LV endocardium at once and make
/// the LV's front come and go before the RV's.
const PMJ_PATCH_RADIUS_MM: f32 = 12.0;

pub fn bake_heart(mesh: &TetMesh, opts: HeartOptions) -> Result<BakedHeart> {
    let mut report = Vec::new();
    check_tags(mesh)?;
    let rho = mesh
        .point_scalars
        .get("rho")
        .context("mesh has no RHO point field")?;
    let phi = mesh
        .point_scalars
        .get("phi")
        .context("mesh has no PHI point field")?;
    let zc = mesh.point_scalars.get("z").context("mesh has no Z point field")?;
    if mesh.fibres.len() != mesh.tets.len() {
        bail!("mesh has no per-element fibres");
    }

    // --- frame: file → canonical ------------------------------------------------------------
    let lv_nodes = mesh.nodes_of_tag(tag::LV);
    let mean_of = |nodes: &[u32], pred: &dyn Fn(u32) -> bool| -> Vec3 {
        let (s, n) = nodes
            .iter()
            .filter(|n| pred(**n))
            .fold((Vec3::ZERO, 0usize), |(s, n), i| {
                (s + mesh.points[*i as usize], n + 1)
            });
        s / n.max(1) as f32
    };
    let apex_pt = mean_of(&lv_nodes, &|n| zc[n as usize] < 0.03);
    let base_pt = mean_of(&lv_nodes, &|n| zc[n as usize] > 0.97);
    let apex_dir = (apex_pt - base_pt).normalize();
    let shared = mesh.shared_nodes(tag::LV, tag::RV);
    let sept_pt = mean_of(&shared, &|_| true);
    let (vsum, vn) = (0..mesh.tets.len())
        .filter(|t| matches!(mesh.tags[*t], tag::LV | tag::RV))
        .fold((Vec3::ZERO, 0usize), |(s, n), t| (s + mesh.centroid(t), n + 1));
    let vc_file = vsum / vn as f32;
    let lv_c_file = mesh.tag_centroid(tag::LV);
    let sept_dir = sept_pt - lv_c_file;
    let (s, c) = SEPTAL_AZIMUTH_DEG.to_radians().sin_cos();
    let canonical_septal = Vec3::new(c, 0.0, s);
    let r1 = rotation_between(apex_dir, sept_dir, Vec3::NEG_Y, canonical_septal);
    let to_canon = |p: Vec3| r1 * (p - vc_file);
    {
        let lps_to_heart = |v: Vec3| Vec3::new(v.x, v.z, -v.y);
        let a = lps_to_heart(apex_dir);
        report.push(format!(
            "dataset long axis (LPS → heart frame): frontal {:.0}°, transverse {:.0}° (pose used: 36°/46°); LV base→apex {:.1} mm",
            (-a.y).atan2(a.x).to_degrees(),
            a.z.atan2(a.x).to_degrees(),
            apex_pt.distance(base_pt)
        ));
    }

    // --- discs ------------------------------------------------------------------------------
    let disc = |t: u16, away_from: u16| -> FittedDisc {
        let pts: Vec<Vec3> = mesh
            .nodes_of_tag(t)
            .iter()
            .map(|n| to_canon(mesh.points[*n as usize]))
            .collect();
        fit_disc(&pts, to_canon(mesh.tag_centroid(away_from)))
    };
    let discs = Discs {
        mitral: disc(tag::MITRAL, tag::LV),
        tricuspid: disc(tag::TRICUSPID, tag::RV),
        aortic: disc(tag::AORTIC, tag::LV),
        pulmonary: disc(tag::PULMONARY, tag::RV),
        laa: disc(tag::LAA, tag::LA),
        pvs: [
            disc(tag::LSPV, tag::LA),
            disc(tag::LIPV, tag::LA),
            disc(tag::RIPV, tag::LA),
            disc(tag::RSPV, tag::LA),
        ],
        svc: disc(tag::SVC, tag::RA),
        ivc: disc(tag::IVC, tag::RA),
    };
    let la_c = to_canon(mesh.tag_centroid(tag::LA));
    let ra_c = to_canon(mesh.tag_centroid(tag::RA));
    let rv_c = to_canon(mesh.tag_centroid(tag::RV));
    let lv_length_mm = lv_nodes
        .iter()
        .map(|n| to_canon(mesh.points[*n as usize]).distance(discs.mitral.centre))
        .fold(0.0f32, f32::max);
    report.push(format!(
        "LV mitral centre → farthest epicardial point {lv_length_mm:.0} mm"
    ));

    // --- septum classification --------------------------------------------------------------
    let rv_node_pos: Vec<Vec3> = mesh
        .nodes_of_tag(tag::RV)
        .iter()
        .map(|n| mesh.points[*n as usize])
        .collect();
    let rv_near_set = NearSet::new(&rv_node_pos, 14.0);
    let shared_phi: Vec<f32> = shared.iter().map(|n| phi[*n as usize]).collect();
    let (ps, pc) = shared_phi
        .iter()
        .fold((0.0f32, 0.0f32), |(s, c), p| (s + p.sin(), c + p.cos()));
    let phi_centre = ps.atan2(pc);
    let mut dev: Vec<f32> = shared_phi
        .iter()
        .map(|p| {
            ((p - phi_centre + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI)
                .abs()
        })
        .collect();
    dev.sort_by(f32::total_cmp);
    let phi_half_width = percentile(&dev, 0.975) + 0.05;
    let is_septal = |t: usize, centre: Vec3| -> bool {
        let p = {
            let [a, b, c, d] = mesh.tets[t];
            // circular mean of the four node φ
            let (s, cc) = [a, b, c, d].iter().fold((0.0f32, 0.0f32), |(s, cc), n| {
                (s + phi[*n as usize].sin(), cc + phi[*n as usize].cos())
            });
            s.atan2(cc)
        };
        let d = ((p - phi_centre + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI)
            .abs();
        d <= phi_half_width && rv_near_set.within(centre, 14.0)
    };

    // --- cell sampling: a regular lattice through the myocardium ---------------------------
    // Lattice points *inside* a myocardial tetrahedron become cells (the volume each cell stands
    // for is then unbiased: a 3 mm right-ventricular wall gets 3/2.3 cells per column, not one
    // full cell per voxel it touches, which would double its mass and its share of the ECG).
    // The atria are about as thin as the lattice (2–3 mm), so for them a lattice point also
    // counts when it lies within `ATRIAL_REACH_MM` of an atrial tetrahedron's centroid: that
    // closes the gaps a surface thinner than the spacing would leave while keeping the wall one
    // cell thick almost everywhere. The P wave scales with the wall thickness (front area =
    // thickness × front length): a reach of 1.6 mm made the atria two cells thick and P(II)
    // 0.6 mV; the Tier-1 graph (2.3 spacings) and the Tier-2 voxel stamp (0.9 spacings) both
    // bridge a one-cell wall.
    // A regular lattice (not a Poisson-disc subset) keeps the neighbour graph as even as the
    // procedural pack's, which the eikonal front needs for smooth arrival times.
    let spacing = opts.cell_spacing_mm.max(0.5);
    let mut cells = Cells::default();
    let mut uvc_z: Vec<f32> = Vec::new();
    let la_axis = (la_c - discs.mitral.centre).normalize_or_zero();
    let ra_axis = (ra_c - discs.tricuspid.centre).normalize_or_zero();
    // the AHA span: epicardial apex to the mitral plane, canonical y
    let apex_y = (0..mesh.tets.len())
        .filter(|t| mesh.tags[*t] == tag::LV)
        .map(|t| to_canon(mesh.centroid(t)).y)
        .fold(f32::INFINITY, f32::min);
    let base_y = discs.mitral.centre.y;
    let height = |frac: f32| apex_y + frac * (base_y - apex_y);
    let proc_frac = |y: f32| (y - PROCEDURAL_APEX_Y) / (PROCEDURAL_BASE_Y - PROCEDURAL_APEX_Y);
    // The LV long axis does not pass through the ventricular centroid (the RV pulls it to the
    // right): circumferential angles, AHA segments and the LV landmarks are measured about the LV
    // tag's own centroid, as the procedural model measures them about its LV centre.
    let lv_c = to_canon(lv_c_file);
    let about_lv = |p: Vec3| Vec3::new(p.x - lv_c.x, p.y, p.z - lv_c.z);
    // spatial hash of the myocardial tets by canonical centroid
    let bin = 2.0f32;
    let bin_key = |p: Vec3| {
        let q = (p / bin).floor();
        (q.x as i32, q.y as i32, q.z as i32)
    };
    let mut tet_bins: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for t in 0..mesh.tets.len() {
        if !matches!(mesh.tags[t], tag::LV | tag::RV | tag::LA | tag::RA) {
            continue;
        }
        let c = to_canon(mesh.centroid(t));
        lo = lo.min(c);
        hi = hi.max(c);
        tet_bins.entry(bin_key(c)).or_default().push(t);
    }
    let inside = |p: Vec3, t: usize| -> bool {
        let [a, b, c, d] = mesh.tets[t];
        let (pa, pb, pc, pd) = (
            to_canon(mesh.points[a as usize]),
            to_canon(mesh.points[b as usize]),
            to_canon(mesh.points[c as usize]),
            to_canon(mesh.points[d as usize]),
        );
        let m = Mat3::from_cols(pb - pa, pc - pa, pd - pa);
        if m.determinant().abs() < 1e-9 {
            return false;
        }
        let l = m.inverse() * (p - pa);
        l.min_element() >= -1e-4 && l.x + l.y + l.z <= 1.0 + 1e-4
    };
    // (cell position, tet) per lattice point, in lattice order
    let mut voxels: Vec<((i32, i32, i32), usize)> = Vec::new();
    let k_lo = (lo / spacing).floor() - Vec3::ONE;
    let k_hi = (hi / spacing).ceil() + Vec3::ONE;
    for kx in k_lo.x as i32..=k_hi.x as i32 {
        for ky in k_lo.y as i32..=k_hi.y as i32 {
            for kz in k_lo.z as i32..=k_hi.z as i32 {
                let p = Vec3::new(kx as f32 + 0.5, ky as f32 + 0.5, kz as f32 + 0.5) * spacing;
                let bk = bin_key(p);
                let mut hit: Option<usize> = None;
                let mut nearest_atrial: Option<(f32, usize)> = None;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            let Some(list) = tet_bins.get(&(bk.0 + dx, bk.1 + dy, bk.2 + dz)) else {
                                continue;
                            };
                            for &t in list {
                                let d = to_canon(mesh.centroid(t)).distance(p);
                                if d > 2.0 {
                                    continue;
                                }
                                if hit.is_none_or(|h| t < h) && inside(p, t) {
                                    hit = Some(t);
                                }
                                if matches!(mesh.tags[t], tag::LA | tag::RA)
                                    && d <= ATRIAL_REACH_MM
                                    && nearest_atrial
                                        .is_none_or(|(bd, bt)| d.total_cmp(&bd).then(t.cmp(&bt)).is_lt())
                                {
                                    nearest_atrial = Some((d, t));
                                }
                            }
                        }
                    }
                }
                let chosen = hit.or(nearest_atrial.map(|(_, t)| t));
                if let Some(t) = chosen {
                    voxels.push(((kx, ky, kz), t));
                }
            }
        }
    }
    let voxel_centre =
        |k: (i32, i32, i32)| Vec3::new(k.0 as f32 + 0.5, k.1 as f32 + 0.5, k.2 as f32 + 0.5) * spacing;
    for (k, t) in voxels {
        let g = mesh.tags[t];
        let p = voxel_centre(k);
        let (region, fibre, depth, aha) = match g {
            tag::LV | tag::RV => {
                let d = mesh.scalar_at("rho", t).unwrap_or(0.5).clamp(0.0, 1.0);
                let mut f = r1 * mesh.fibres[t];
                if f.length_squared() < 0.5 {
                    f = Vec3::Y.cross(p).normalize_or_zero();
                }
                if g == tag::LV {
                    let region = if is_septal(t, mesh.centroid(t)) {
                        Region::VentricularSeptum
                    } else {
                        Region::LeftVentricle
                    };
                    (region, f, d, canonical_aha(about_lv(p), apex_y, base_y))
                } else {
                    (
                        Region::RightVentricle,
                        f,
                        d,
                        rv_segment(about_lv(p) - Vec3::Y * lv_c.y),
                    )
                }
            }
            tag::LA => (Region::LeftAtrium, la_axis.cross(p - la_c), 0.5, 0),
            _ => (Region::RightAtrium, ra_axis.cross(p - ra_c), 0.5, 0),
        };
        cells.push(p, fibre, region, depth, aha);
        uvc_z.push(mesh.scalar_at("z", t).unwrap_or(-1.0));
    }
    let _ = rho;
    let n_sampled = cells.positions.len();
    report.push(format!(
        "{n_sampled} myocardial cells at {spacing} mm (LV {}, septum {}, RV {}, LA {}, RA {})",
        cells
            .regions
            .iter()
            .filter(|r| **r == Region::LeftVentricle)
            .count(),
        cells
            .regions
            .iter()
            .filter(|r| **r == Region::VentricularSeptum)
            .count(),
        cells
            .regions
            .iter()
            .filter(|r| **r == Region::RightVentricle)
            .count(),
        cells.regions.iter().filter(|r| **r == Region::LeftAtrium).count(),
        cells
            .regions
            .iter()
            .filter(|r| **r == Region::RightAtrium)
            .count(),
    ));

    // --- helpers on the sampled cloud --------------------------------------------------------
    let is_lv = |r: Region| matches!(r, Region::LeftVentricle | Region::VentricularSeptum);
    // Cell of the LV (or septum) nearest to (azimuth, height, depth) in a scaled metric.
    let lv_at = |cells: &Cells, az: f32, y: f32, depth: f32, pred: &dyn Fn(Region) -> bool| -> Vec3 {
        let mut best = (f32::INFINITY, Vec3::ZERO);
        for i in 0..n_sampled {
            if !pred(cells.regions[i]) {
                continue;
            }
            let p = cells.positions[i];
            let cost = (angle_diff_deg(azimuth_deg(about_lv(p)), az) / 6.0).powi(2)
                + ((p.y - y) / 2.0).powi(2)
                + ((cells.depth[i] - depth) * 20.0).powi(2);
            if cost < best.0 {
                best = (cost, p);
            }
        }
        best.1
    };
    let rv_near = |cells: &Cells, target: Vec3, max_depth: f32| -> Vec3 {
        let mut best = (f32::INFINITY, target);
        for i in 0..n_sampled {
            if cells.regions[i] == Region::RightVentricle && cells.depth[i] <= max_depth {
                let d = cells.positions[i].distance_squared(target);
                if d < best.0 {
                    best = (d, cells.positions[i]);
                }
            }
        }
        best.1
    };
    let atrial_near = |cells: &Cells, target: Vec3, region: Region| -> Vec3 {
        let mut best = (f32::INFINITY, target);
        for i in 0..n_sampled {
            if cells.regions[i] == region {
                let d = cells.positions[i].distance_squared(target);
                if d < best.0 {
                    best = (d, cells.positions[i]);
                }
            }
        }
        best.1
    };
    let snap_path = |cells: &Cells, a: Vec3, b: Vec3, steps: usize, region: Region| -> Vec<Vec3> {
        (0..=steps)
            .map(|i| atrial_near(cells, a.lerp(b, i as f32 / steps as f32), region))
            .collect()
    };

    // --- atrial geometry (canonical) ----------------------------------------------------------
    let up = Vec3::Y;
    let lateral_ra = (ra_c - la_c).normalize();
    let toward_la = -lateral_ra;
    let sa_target = discs.svc.centre - discs.svc.normal * 2.0 + lateral_ra * discs.svc.radius_mm;
    let sa_point = atrial_near(&cells, sa_target, Region::RightAtrium);
    let ivc_lateral = discs.ivc.centre + lateral_ra * discs.ivc.radius_mm;
    let crista = snap_path(
        &cells,
        sa_point,
        atrial_near(&cells, ivc_lateral, Region::RightAtrium),
        10,
        Region::RightAtrium,
    );
    let ivc_to_tv = (discs.tricuspid.centre - discs.ivc.centre).normalize();
    let isthmus_a = atrial_near(
        &cells,
        discs.ivc.centre + ivc_to_tv * discs.ivc.radius_mm,
        Region::RightAtrium,
    );
    let isthmus_b = atrial_near(
        &cells,
        discs.tricuspid.centre - ivc_to_tv * discs.tricuspid.radius_mm + discs.tricuspid.normal * 2.0,
        Region::RightAtrium,
    );
    let isthmus = snap_path(&cells, isthmus_a, isthmus_b, 6, Region::RightAtrium);
    let septal_dir_atria = (discs.mitral.centre - discs.tricuspid.centre).normalize();
    let avn_target = discs.tricuspid.centre
        + septal_dir_atria * (discs.tricuspid.radius_mm * 0.75)
        + discs.tricuspid.normal * 4.0;
    let avn_point = atrial_near(&cells, avn_target, Region::RightAtrium);
    let fast_point = atrial_near(
        &cells,
        avn_point + up * 5.0 + toward_la * 2.0 + Vec3::Z * 3.0,
        Region::RightAtrium,
    );
    let cs_os_target = discs.tricuspid.centre
        + (discs.ivc.centre - discs.tricuspid.centre).normalize() * (discs.tricuspid.radius_mm * 0.8)
        + toward_la * 6.0
        + discs.tricuspid.normal * 3.0;
    let cs_os_point = atrial_near(&cells, cs_os_target, Region::RightAtrium);
    let slow_pathway = snap_path(&cells, avn_point, cs_os_point, 6, Region::RightAtrium);
    let interatrial = to_canon(mean_of(&mesh.shared_nodes(tag::LA, tag::RA), &|_| true));
    let bachmann = vec![
        atrial_near(
            &cells,
            interatrial + Vec3::Z * 18.0 + up * 8.0 + lateral_ra * 8.0,
            Region::RightAtrium,
        ),
        atrial_near(
            &cells,
            interatrial + Vec3::Z * 18.0 + up * 8.0 + toward_la * 12.0,
            Region::LeftAtrium,
        ),
    ];
    // coronary sinus: just outside the mitral annulus on the atrial side, from the septal-inferior
    // rim (near its ostium) through the inferior wall to the lateral wall
    let coronary_sinus: Vec<Vec3> = {
        let m = &discs.mitral;
        let (u, v) = {
            let x = (Vec3::X - m.normal * Vec3::X.dot(m.normal)).normalize();
            (x, m.normal.cross(x))
        };
        let rim = |az_deg: f32| {
            let (s, c) = az_deg.to_radians().sin_cos();
            m.centre + (u * c + v * s) * (m.radius_mm * 1.15) + m.normal * 2.5
        };
        let start_az = azimuth_deg(cs_os_point - m.centre);
        let mut pts = vec![cs_os_point];
        let mut az = start_az;
        // go towards 360° (lateral) through the inferior (270°) side
        let steps = ((360.0 - start_az).rem_euclid(360.0) / 10.0).ceil() as usize;
        for _ in 0..steps.max(1) {
            az = (az + 10.0).min(start_az + (360.0 - start_az).rem_euclid(360.0));
            pts.push(rim(az));
        }
        pts
    };

    // --- atrial sub-regions -------------------------------------------------------------------
    let la_node_pos: Vec<Vec3> = mesh
        .nodes_of_tag(tag::LA)
        .iter()
        .map(|n| to_canon(mesh.points[*n as usize]))
        .collect();
    let ra_node_pos: Vec<Vec3> = mesh
        .nodes_of_tag(tag::RA)
        .iter()
        .map(|n| to_canon(mesh.points[*n as usize]))
        .collect();
    let la_near = NearSet::new(&la_node_pos, 2.5);
    let ra_near = NearSet::new(&ra_node_pos, 2.5);
    for i in 0..n_sampled {
        let p = cells.positions[i];
        let r = cells.regions[i];
        let (in_ra, in_la) = (r == Region::RightAtrium, r == Region::LeftAtrium);
        if !(in_ra || in_la) {
            continue;
        }
        let other_near = if in_ra {
            la_near.within(p, 2.5)
        } else {
            ra_near.within(p, 2.5)
        };
        if other_near {
            cells.regions[i] = Region::AtrialSeptum;
        }
        if distance_to_polyline(p, &bachmann) < 4.0 {
            cells.regions[i] = Region::BachmannBundle;
            cells.fibres[i] = polyline_tangent_at(p, &bachmann);
        } else if in_ra && distance_to_polyline(p, &crista) < 4.0 {
            cells.regions[i] = Region::CristaTerminalis;
            cells.fibres[i] = polyline_tangent_at(p, &crista);
        } else if in_ra && distance_to_polyline(p, &isthmus) < 5.0 {
            cells.regions[i] = Region::CavotricuspidIsthmus;
        } else if in_la && discs.pvs.iter().any(|d| d.centre.distance(p) < d.radius_mm + 4.0) {
            cells.regions[i] = Region::PulmonaryVeinSleeves;
        } else if in_la && discs.laa.centre.distance(p) < discs.laa.radius_mm + 3.0 {
            cells.regions[i] = Region::LeftAtrialAppendage;
        }
    }
    // appendage body: a tapered lobe on the LAA inlet, sampled on a lattice
    let laa_len = 22.0f32;
    let laa_axis = discs.laa.normal;
    let (lu, lv) = {
        let h = if laa_axis.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
        let u = h.cross(laa_axis).normalize();
        (u, laa_axis.cross(u))
    };
    let laa_radius = |t: f32| discs.laa.radius_mm * (0.95 - 0.5 * t);
    {
        let n_ax = (laa_len / spacing).ceil() as i32;
        let n_r = (discs.laa.radius_mm / spacing).ceil() as i32;
        for ia in 0..=n_ax {
            let t = ia as f32 / n_ax as f32;
            let r = laa_radius(t);
            for iu in -n_r..=n_r {
                for iv in -n_r..=n_r {
                    let off = lu * (iu as f32 * spacing) + lv * (iv as f32 * spacing);
                    if off.length() > r - 0.5 * spacing || (off.length() < r - 2.5 * spacing && t < 0.95) {
                        continue; // a shell: the appendage is a thin-walled pouch
                    }
                    let p = discs.laa.centre + laa_axis * (t * laa_len + 1.0) + off;
                    let mut fibre = laa_axis.cross(off).normalize_or_zero();
                    if fibre.length_squared() < 0.5 {
                        fibre = lu; // on the axis the circumferential direction is undefined
                    }
                    cells.push(p, fibre, Region::LeftAtrialAppendage, 0.5, 0);
                    uvc_z.push(-1.0);
                }
            }
        }
    }
    let laa_tip = discs.laa.centre + laa_axis * (laa_len + 1.0);

    // --- ventricular conduction geometry ---------------------------------------------------------
    let septum_top = {
        let max_y = (0..n_sampled)
            .filter(|i| cells.regions[*i] == Region::VentricularSeptum)
            .map(|i| cells.positions[i].y)
            .fold(f32::NEG_INFINITY, f32::max);
        let (s, n) = (0..n_sampled)
            .filter(|i| cells.regions[*i] == Region::VentricularSeptum && cells.positions[*i].y > max_y - 4.0)
            .fold((Vec3::ZERO, 0usize), |(s, n), i| (s + cells.positions[i], n + 1));
        s / n.max(1) as f32
    };
    // the bundle starts a little way from the node's atrial cell so a catheter tip there finds
    // His tissue, not the atrial wall underneath (as in the procedural heart)
    let his = vec![
        avn_point.lerp(septum_top, 0.15),
        avn_point.lerp(septum_top, 0.5) - up,
        septum_top,
    ];
    let lbb: Vec<Vec3> = (0..=6)
        .map(|i| {
            let y = septum_top.y - 3.0 - 17.0 * i as f32 / 6.0;
            lv_at(&cells, SEPTAL_AZIMUTH_DEG, y, 0.0, &is_lv)
        })
        .collect();
    let lbb_end = *lbb.last().expect("lbb");
    let fascicle = |cells: &Cells, az_end: f32, y_end: f32| -> Vec<Vec3> {
        let mut v = vec![lbb_end];
        let y_start = lbb_end.y;
        for i in 1..=8 {
            let t = i as f32 / 8.0;
            let az = SEPTAL_AZIMUTH_DEG + (az_end - SEPTAL_AZIMUTH_DEG) * t;
            let y = y_start + (y_end - y_start) * t;
            v.push(lv_at(cells, az, y, 0.0, &is_lv));
        }
        v
    };
    let laf = fascicle(&cells, LAF_TERMINAL.0, height(proc_frac(LAF_TERMINAL.1)));
    let lpf = fascicle(&cells, LPF_TERMINAL.0, height(proc_frac(LPF_TERMINAL.1)));
    let rbb_septal: Vec<Vec3> = (0..=10)
        .map(|i| {
            let y = septum_top.y - 3.0 + (height(0.1) - septum_top.y + 3.0) * i as f32 / 10.0;
            lv_at(&cells, SEPTAL_AZIMUTH_DEG, y, 1.0, &|r| {
                r == Region::VentricularSeptum
            })
        })
        .collect();
    // the right bundle ends at the anterior papillary muscle: the anterior RV free wall a quarter
    // of the way from the RV apex to the pulmonary valve (the moderator band inserts at the
    // junction of the apical and middle thirds; from there the network has to reach the acute
    // margin and the inflow, 50–60 mm away, so the patch sits low rather than mid-ventricular)
    let rv_apex_target = {
        let mut best = (f32::INFINITY, rv_c);
        for i in 0..n_sampled {
            if cells.regions[i] == Region::RightVentricle
                && cells.depth[i] < 0.5
                && cells.positions[i].y < best.0
            {
                best = (cells.positions[i].y, cells.positions[i]);
            }
        }
        best.1
    };
    let rbb_terminal = rv_near(
        &cells,
        rv_apex_target.lerp(discs.pulmonary.centre, 0.25) + Vec3::Z * 12.0,
        0.3,
    );
    let rbb_band = vec![*rbb_septal.last().expect("rbb"), rbb_terminal];

    // --- explicit structures ------------------------------------------------------------------
    let push_blob = |cells: &mut Cells, centre: Vec3, r: f32, region: Region, fibre: Vec3| {
        for p in blob(centre, r, CONDUCTION_SPACING_MM) {
            cells.push(p, fibre, region, 0.5, 0);
        }
    };
    push_blob(
        &mut cells,
        sa_point,
        3.0,
        Region::SinoatrialNode,
        polyline_tangent_at(sa_point, &crista),
    );
    push_blob(&mut cells, avn_point, 2.5, Region::AvNodeCompact, Vec3::Y);
    push_blob(
        &mut cells,
        fast_point,
        2.0,
        Region::AvNodeFastPathway,
        (avn_point - fast_point).normalize_or_zero(),
    );
    let push_polyline =
        |cells: &mut Cells, points: &[Vec3], region: Region, spacing: f32, depth: f32| -> (u32, u32) {
            let first = cells.positions.len() as u32;
            for (p, t) in sample_polyline(points, spacing) {
                cells.push(p, t, region, depth, 0);
            }
            (first, cells.positions.len() as u32 - 1)
        };
    push_polyline(
        &mut cells,
        &slow_pathway,
        Region::AvNodeSlowPathway,
        CONDUCTION_SPACING_MM,
        0.5,
    );
    let (his_start, _) = push_polyline(&mut cells, &his, Region::HisBundle, CONDUCTION_SPACING_MM, 0.0);
    let (lbb_start, _) = push_polyline(
        &mut cells,
        &lbb,
        Region::LeftBundleBranch,
        CONDUCTION_SPACING_MM,
        0.0,
    );
    let (_, laf_end) = push_polyline(
        &mut cells,
        &laf,
        Region::LeftAnteriorFascicle,
        CONDUCTION_SPACING_MM,
        0.0,
    );
    let (_, lpf_end) = push_polyline(
        &mut cells,
        &lpf,
        Region::LeftPosteriorFascicle,
        CONDUCTION_SPACING_MM,
        0.0,
    );
    let (rbb_start, _) = push_polyline(
        &mut cells,
        &rbb_septal,
        Region::RightBundleBranch,
        CONDUCTION_SPACING_MM,
        1.0,
    );
    let (_, rbb_end) = push_polyline(
        &mut cells,
        &rbb_band,
        Region::RightBundleBranch,
        CONDUCTION_SPACING_MM,
        0.0,
    );
    let (cs_start, cs_end) = push_polyline(&mut cells, &coronary_sinus, Region::CoronarySinus, 2.0, 0.5);
    while uvc_z.len() < cells.positions.len() {
        uvc_z.push(-1.0);
    }

    // --- landmarks --------------------------------------------------------------------------------
    let mut landmarks: BTreeMap<String, u32> = BTreeMap::new();
    let mut put = |name: &str, idx: Option<u32>| {
        if let Some(i) = idx {
            landmarks.insert(name.to_string(), i);
        }
    };
    let vent = |r: Region| r.is_ventricular();
    put(
        "sa_node",
        cells.nearest(sa_point, |i| cells.region(i) == Region::SinoatrialNode),
    );
    put(
        "av_node",
        cells.nearest(avn_point, |i| cells.region(i) == Region::AvNodeCompact),
    );
    put("his", Some(his_start));
    put("lbb", Some(lbb_start));
    put("laf", Some(laf_end));
    put("lpf", Some(lpf_end));
    put("rbb", Some(rbb_start));
    put("rbb_terminal", Some(rbb_end));
    put("cs_os", Some(cs_start));
    put("cs_distal", Some(cs_end));
    let rv = |cells: &Cells, i: usize| cells.region(i) == Region::RightVentricle;
    put("rv_apex", cells.nearest(rv_apex_target, |i| rv(&cells, i)));
    put(
        "rvot",
        cells.nearest(discs.pulmonary.centre - discs.pulmonary.normal * 10.0, |i| {
            rv(&cells, i)
        }),
    );
    put(
        "rv_free_wall",
        cells.nearest(
            rv_near(&cells, rv_c + Vec3::new(-1.0, -0.1, 0.2).normalize() * 45.0, 0.5),
            |i| rv(&cells, i),
        ),
    );
    put(
        "lv_apex",
        cells.nearest(Vec3::new(lv_c.x, apex_y + 4.0, lv_c.z), |i| vent(cells.region(i))),
    );
    put(
        "lv_lateral",
        cells.nearest(lv_at(&cells, 0.0, height(proc_frac(-5.0)), 0.5, &is_lv), |i| {
            vent(cells.region(i))
        }),
    );
    put(
        "lv_posterior",
        cells.nearest(lv_at(&cells, 280.0, height(proc_frac(-15.0)), 0.5, &is_lv), |i| {
            vent(cells.region(i))
        }),
    );
    put(
        "lv_anterior",
        cells.nearest(
            lv_at(&cells, 90.0, height(proc_frac(-5.0)), 0.5, &|r| {
                r == Region::LeftVentricle
            }),
            |i| cells.region(i) == Region::LeftVentricle,
        ),
    );
    put(
        "lv_septal",
        cells.nearest(
            lv_at(&cells, SEPTAL_AZIMUTH_DEG, height(proc_frac(-5.0)), 0.5, &|r| {
                r == Region::VentricularSeptum
            }),
            |i| cells.region(i) == Region::VentricularSeptum,
        ),
    );
    put(
        "bachmann",
        cells.nearest(bachmann[0].lerp(bachmann[1], 0.5), |i| {
            cells.region(i) == Region::BachmannBundle
        }),
    );
    put(
        "ra_lateral",
        cells.nearest(ra_c + lateral_ra * 40.0, |i| {
            cells.region(i) == Region::RightAtrium
        }),
    );
    let la_roof_target = {
        let mut best = (f32::NEG_INFINITY, la_c);
        for i in 0..n_sampled {
            if cells.region(i) == Region::LeftAtrium && cells.positions[i].y > best.0 {
                best = (cells.positions[i].y, cells.positions[i]);
            }
        }
        best.1
    };
    put(
        "la_roof",
        cells.nearest(la_roof_target, |i| cells.region(i) == Region::LeftAtrium),
    );
    put(
        "laa",
        cells.nearest(laa_tip, |i| cells.region(i) == Region::LeftAtrialAppendage),
    );
    // the AF presets pace here: the left atrial wall between the four vein mouths (a sleeve cell
    // instead gave organised wavelets and a regular ventricular response)
    let antrum = discs.pvs.iter().map(|d| d.centre).sum::<Vec3>() / 4.0;
    put(
        "la_pv_antrum",
        cells.nearest(antrum, |i| cells.region(i) == Region::LeftAtrium),
    );

    // --- PMJ seed sets ------------------------------------------------------------------------------
    let rbb_lower: Vec<Vec3> = rbb_septal[5..].to_vec();
    let laf_t = *laf.last().expect("laf");
    let lpf_t = *lpf.last().expect("lpf");
    let mut pmj_seeds = BTreeMap::new();
    pmj_seeds.insert(
        "rbb".to_string(),
        (0..cells.positions.len())
            .filter(|&i| {
                let p = cells.positions[i];
                match cells.regions[i] {
                    // the same patch radius as the left fascicles: the real RV free wall is as large
                    // as the LV's and a 15 mm patch left its base 50 mm from the nearest seed
                    Region::RightVentricle => {
                        cells.depth[i] < 0.5 && p.distance(rbb_terminal) < PMJ_PATCH_RADIUS_MM
                    }
                    Region::VentricularSeptum => {
                        cells.depth[i] > 0.7 && distance_to_polyline(p, &rbb_lower) < 8.0
                    }
                    _ => false,
                }
            })
            .map(|i| i as u32)
            .collect::<Vec<u32>>(),
    );
    for (name, terminal) in [("laf", laf_t), ("lpf", lpf_t)] {
        pmj_seeds.insert(
            name.to_string(),
            (0..cells.positions.len())
                .filter(|&i| {
                    is_lv(cells.regions[i])
                        && cells.depth[i] < 0.3
                        && cells.positions[i].distance(terminal) < PMJ_PATCH_RADIUS_MM
                })
                .map(|i| i as u32)
                .collect(),
        );
    }

    // --- render meshes (canonical) ---------------------------------------------------------------
    let surface_of = |tags: &[u16]| -> TriMesh {
        let faces = mesh.boundary_faces(|t| tags.contains(&mesh.tags[t]));
        let mut m = faces_to_mesh(&mesh.points, &faces);
        transform(&mut m, to_canon, r1);
        m
    };
    let chamber = |tags: &[u16], extra: Vec<TriMesh>| -> TriMesh {
        let mut m = surface_of(tags);
        taubin_smooth(&mut m, 3);
        decimate(&mut m, opts.chamber_triangles);
        if extra.is_empty() {
            m
        } else {
            merge(&std::iter::once(m).chain(extra).collect::<Vec<_>>())
        }
    };
    let vessel_tube = |d: &FittedDisc, len: f32, wall: f32| -> TriMesh {
        let r = d.radius_mm + wall;
        swept_tube(d.centre - d.normal * 1.0, d.normal, len, move |_| r, 6, 24)
    };
    // arteries: the far end of the cropped wall, extended by a tube
    let artery_extension = |wall_tag: u16, valve: &FittedDisc, len: f32| -> TriMesh {
        let nodes = mesh.nodes_of_tag(wall_tag);
        let axis_pts: Vec<(f32, Vec3)> = nodes
            .iter()
            .map(|n| {
                let p = to_canon(mesh.points[*n as usize]);
                ((p - valve.centre).dot(valve.normal), p)
            })
            .collect();
        let far = axis_pts.iter().map(|(s, _)| *s).fold(f32::NEG_INFINITY, f32::max);
        let end: Vec<Vec3> = axis_pts
            .iter()
            .filter(|(s, _)| *s > far - 4.0)
            .map(|(_, p)| *p)
            .collect();
        let d = fit_disc(&end, valve.centre);
        swept_tube(
            d.centre - d.normal * 2.0,
            d.normal,
            len,
            move |t| d.radius_mm * (1.0 - 0.1 * t),
            6,
            24,
        )
    };
    let laa_lobe = swept_tube(
        discs.laa.centre,
        laa_axis,
        laa_len,
        move |t| laa_radius(t) + 1.0,
        8,
        24,
    );
    let mut named: Vec<NamedMesh> = Vec::new();
    let mut add = |name: &str, kind: MeshKind, m: TriMesh| {
        named.push(NamedMesh {
            name: name.to_string(),
            kind,
            mesh: m,
        });
    };
    add("lv", MeshKind::Myocardium, chamber(&[tag::LV], vec![]));
    add("rv", MeshKind::Myocardium, chamber(&[tag::RV], vec![]));
    add("ra", MeshKind::Myocardium, chamber(&[tag::RA], vec![]));
    add("la", MeshKind::Myocardium, chamber(&[tag::LA], vec![laa_lobe]));
    let vessel = |tags: &[u16], extra: TriMesh| -> TriMesh {
        let mut m = surface_of(tags);
        taubin_smooth(&mut m, 2);
        decimate(&mut m, opts.vessel_triangles);
        merge(&[m, extra])
    };
    add(
        "aorta",
        MeshKind::Vessel,
        vessel(&[tag::AORTA], artery_extension(tag::AORTA, &discs.aortic, 28.0)),
    );
    add(
        "pulmonary_artery",
        MeshKind::Vessel,
        vessel(&[tag::PA], artery_extension(tag::PA, &discs.pulmonary, 18.0)),
    );
    add("svc", MeshKind::Vessel, vessel_tube(&discs.svc, 32.0, 1.5));
    add("ivc", MeshKind::Vessel, vessel_tube(&discs.ivc, 26.0, 1.5));
    add(
        "pulmonary_veins",
        MeshKind::Vessel,
        merge(
            &discs
                .pvs
                .iter()
                .map(|d| vessel_tube(d, 22.0, 1.2))
                .collect::<Vec<_>>(),
        ),
    );
    let ring_of = |valve_tag: u16, chambers: [u16; 2], d: &FittedDisc| -> TriMesh {
        let mut rim: Vec<Vec3> = mesh
            .shared_nodes(valve_tag, chambers[0])
            .into_iter()
            .chain(mesh.shared_nodes(valve_tag, chambers[1]))
            .map(|n| to_canon(mesh.points[n as usize]))
            .collect();
        if rim.len() < 24 {
            rim = mesh
                .nodes_of_tag(valve_tag)
                .iter()
                .map(|n| to_canon(mesh.points[*n as usize]))
                .collect();
        }
        let x = (Vec3::X - d.normal * Vec3::X.dot(d.normal)).normalize();
        let y = d.normal.cross(x);
        let bins = 48usize;
        let mut acc = vec![(0.0f32, 0usize); bins];
        for p in &rim {
            let q = *p - d.centre;
            let (a, b) = (q.dot(x), q.dot(y));
            let ang = b.atan2(a).rem_euclid(std::f32::consts::TAU);
            let bin = ((ang / std::f32::consts::TAU) * bins as f32) as usize % bins;
            acc[bin].0 += (a * a + b * b).sqrt();
            acc[bin].1 += 1;
        }
        let mut radii: Vec<f32> = acc
            .iter()
            .map(|(s, n)| if *n > 0 { s / *n as f32 } else { f32::NAN })
            .collect();
        for i in 0..bins {
            if radii[i].is_nan() {
                let (mut l, mut r) = (i, i);
                while radii[l].is_nan() {
                    l = (l + bins - 1) % bins;
                }
                while radii[r].is_nan() {
                    r = (r + 1) % bins;
                }
                radii[i] = f32::midpoint(radii[l], radii[r]);
            }
        }
        let loop_pts: Vec<Vec3> = (0..bins)
            .map(|i| {
                let ang = std::f32::consts::TAU * (i as f32 + 0.5) / bins as f32;
                d.centre + (x * ang.cos() + y * ang.sin()) * radii[i]
            })
            .collect();
        swept_ring(&loop_pts, 1.6, 12)
    };
    add(
        "valve_rings",
        MeshKind::Valve,
        merge(&[
            ring_of(tag::MITRAL, [tag::LV, tag::LA], &discs.mitral),
            ring_of(tag::AORTIC, [tag::LV, tag::AORTA], &discs.aortic),
            ring_of(tag::TRICUSPID, [tag::RV, tag::RA], &discs.tricuspid),
            ring_of(tag::PULMONARY, [tag::RV, tag::PA], &discs.pulmonary),
        ]),
    );

    // --- to the heart frame ----------------------------------------------------------------------
    let (sum, count) = cells
        .positions
        .iter()
        .zip(&cells.regions)
        .filter(|(_, r)| r.is_ventricular())
        .fold((Vec3::ZERO, 0usize), |(s, c), (p, _)| (s + *p, c + 1));
    let centroid = sum / count.max(1) as f32;
    let rotation = heart_rotation();
    let xf = |p: Vec3| rotation * (p - centroid);
    let positions: Vec<Vec3> = cells.positions.iter().map(|p| xf(*p)).collect();
    let fibres: Vec<Vec3> = cells
        .fibres
        .iter()
        .map(|f| (rotation * *f).normalize_or_zero())
        .collect();
    let mut cloud = CellCloud::build(positions, fibres, cells.regions.clone(), cells.depth.clone());
    cloud.landmarks = landmarks;
    for m in &mut named {
        transform(&mut m.mesh, xf, rotation);
        m.mesh.recompute_normals();
    }
    {
        let h = |p: Vec3| xf(p);
        let hd = |d: &FittedDisc| {
            let c = h(d.centre);
            let n = (rotation * d.normal).normalize();
            format!(
                "centre ({:.1}, {:.1}, {:.1}) normal ({:.3}, {:.3}, {:.3}) r {:.1}",
                c.x, c.y, c.z, n.x, n.y, n.z, d.radius_mm
            )
        };
        report.push(format!("heart frame: tricuspid {}", hd(&discs.tricuspid)));
        report.push(format!("heart frame: mitral    {}", hd(&discs.mitral)));
        report.push(format!(
            "heart frame: svc {} | ivc {}",
            hd(&discs.svc),
            hd(&discs.ivc)
        ));
        let ra_h = h(ra_c);
        let la_h = h(la_c);
        report.push(format!(
            "heart frame: RA centroid ({:.1}, {:.1}, {:.1}), LA centroid ({:.1}, {:.1}, {:.1})",
            ra_h.x, ra_h.y, ra_h.z, la_h.x, la_h.y, la_h.z
        ));
        // polar angles about the RA→tricuspid axis, for the flutter preset's bands
        let axis = (h(discs.tricuspid.centre) - ra_h).normalize();
        let polar = |p: Vec3| {
            ((p - ra_h).normalize().dot(axis))
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees()
        };
        let range = |pred: &dyn Fn(usize) -> bool| {
            let (lo, hi) = (0..cells.positions.len())
                .filter(|i| pred(*i))
                .map(|i| polar(h(cells.positions[i])))
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, hh), a| {
                    (l.min(a), hh.max(a))
                });
            format!("{lo:.0}–{hi:.0}°")
        };
        report.push(format!(
            "polar angles about RA→TV (axis ({:.3}, {:.3}, {:.3})): RA wall {}, isthmus {}, septum {}, crista {}, SA node {}, IVC inlet {:.0}°, SVC inlet {:.0}°, CS os {:.0}°, Bachmann {}; RA radius max {:.0} mm",
            axis.x,
            axis.y,
            axis.z,
            range(&|i| cells.regions[i] == Region::RightAtrium),
            range(&|i| cells.regions[i] == Region::CavotricuspidIsthmus),
            range(&|i| cells.regions[i] == Region::AtrialSeptum),
            range(&|i| cells.regions[i] == Region::CristaTerminalis),
            range(&|i| cells.regions[i] == Region::SinoatrialNode),
            polar(h(discs.ivc.centre)),
            polar(h(discs.svc.centre)),
            polar(h(cs_os_point)),
            range(&|i| cells.regions[i] == Region::BachmannBundle),
            (0..cells.positions.len())
                .filter(|i| cells.regions[*i].is_atrial() && cells.regions[*i] != Region::LeftAtrium)
                .map(|i| h(cells.positions[i]).distance(ra_h))
                .fold(0.0f32, f32::max)
        ));
    }
    report.push(format!(
        "{} cells in all ({} conduction/sinus/appendage cells added); landmarks {}",
        cloud.len(),
        cloud.len() - n_sampled,
        cloud.landmarks.len()
    ));
    for m in &named {
        report.push(format!(
            "  mesh {:<17} {:>6} vertices {:>6} triangles",
            m.name,
            m.mesh.vertex_count(),
            m.mesh.triangle_count()
        ));
    }
    Ok(BakedHeart {
        meshes: named,
        cells: cloud,
        pmj_seeds,
        aha_segments: cells.aha,
        lv_length_mm,
        report,
    })
}

/// The tag table is the Strocchi/Rodero one: verify the adjacencies that the bake relies on.
fn check_tags(mesh: &TetMesh) -> Result<()> {
    let touches = |a: u16, b: u16| !mesh.shared_nodes(a, b).is_empty();
    let expect = [
        (tag::MITRAL, tag::LV, tag::LA, "mitral plane"),
        (tag::TRICUSPID, tag::RV, tag::RA, "tricuspid plane"),
        (tag::AORTIC, tag::LV, tag::AORTA, "aortic plane"),
        (tag::PULMONARY, tag::RV, tag::PA, "pulmonary plane"),
    ];
    for (t, a, b, name) in expect {
        if !(touches(t, a) && touches(t, b)) {
            bail!("tag {t} ({name}) does not touch tags {a} and {b}: not the Rodero/Strocchi tag table");
        }
    }
    for t in [tag::LAA, tag::LSPV, tag::LIPV, tag::RIPV, tag::RSPV] {
        if !touches(t, tag::LA) || !touches(t, t + tag::BORDER_OFFSET) {
            bail!(
                "tag {t} is not a left-atrial inlet with its border at {}",
                t + tag::BORDER_OFFSET
            );
        }
    }
    for t in [tag::SVC, tag::IVC] {
        if !touches(t, tag::RA) || !touches(t, t + tag::BORDER_OFFSET) {
            bail!(
                "tag {t} is not a right-atrial inlet with its border at {}",
                t + tag::BORDER_OFFSET
            );
        }
    }
    // positions (file frame is DICOM LPS: +x left, +y posterior, +z superior)
    let c = |t: u16| mesh.tag_centroid(t);
    if c(tag::SVC).z < c(tag::IVC).z {
        bail!("tag {} (SVC) is below tag {} (IVC)", tag::SVC, tag::IVC);
    }
    if c(tag::LAA).y > c(tag::LSPV).y || c(tag::LAA).y > c(tag::LIPV).y {
        bail!(
            "tag {} (appendage inlet) is not anterior to the left pulmonary veins",
            tag::LAA
        );
    }
    if c(tag::RSPV).x > c(tag::LSPV).x || c(tag::RIPV).x > c(tag::LIPV).x {
        bail!("right pulmonary-vein tags are not to the right of the left ones");
    }
    Ok(())
}
