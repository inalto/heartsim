//! Idealised four-chamber heart built from implicit shapes.
//!
//! **Model.** The ventricles are two truncated ellipsoids: the LV a thick-walled (10 mm) ellipsoid
//! of revolution, the RV a thinner (4 mm) ellipsoid offset to the patient's right and anteriorly so
//! that its cavity is the crescent outside the LV; the part of the LV wall inside the RV ellipsoid is
//! the interventricular septum. The atria are 3.5 mm spherical shells above the valve plane whose
//! overlap forms the interatrial septum; the LA carries an appendage. Great vessels are capsules and
//! the four valve rings are tori (render-only). Dimensions are adult averages (LV cavity ≈ 50 mm
//! diameter, ≈ 80 mm long axis; atrial diameter ≈ 48 mm) from standard echocardiographic normal
//! ranges (Lang et al., *J Am Soc Echocardiogr* 2015).
//!
//! **Cells.** A regular lattice (default 2.3 mm) is classified against the implicit shapes. Each
//! ventricular cell gets a transmural depth (0 endocardium → 1 epicardium, septum measured from the
//! LV side) by bisecting the family of ellipsoids between the endocardial and epicardial surfaces,
//! and a fibre direction following the rule-based helix (helix angle +60° at the endocardium to −60°
//! at the epicardium relative to the circumferential direction — Streeter's human values, see
//! `docs/research/electrophysiology-and-ecg-models.md` §5.3). Atrial fibres run circumferentially
//! around each chamber's axis. The conduction system (SA node, AV node with fast/slow inputs, His,
//! bundle branches, fascicles) and the coronary sinus are added as explicit cell blobs/polylines on
//! the endocardium; atrial sub-regions (crista terminalis, cavotricuspid isthmus, Bachmann's bundle,
//! pulmonary-vein sleeves, appendage, septum) are labelled by position.
//!
//! **Frame.** Everything is modelled with the LV long axis vertical ("canonical" coordinates) and
//! finally rotated so that the apex points left, inferior and anterior (≈ 45° in the frontal
//! plane, 20° anterior tilt) and translated so the ventricular centroid is the origin — the heart
//! frame of `docs/ARCHITECTURE.md` §7.

use crate::cloud::CellCloud;
use crate::implicit::{capsule, ellipsoid, half_space, polygonise, sphere, subtract, torus, union};
use crate::mesh::TriMesh;
use crate::territory::aha_segment;
use glam::{Quat, Vec3};
use heartsim_core::Region;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Sampling resolution of the procedural model.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Resolution {
    /// Voxel size of the marching-tetrahedra render meshes.
    pub mesh_voxel_mm: f32,
    /// Lattice spacing of the myocardial cell cloud.
    pub cell_spacing_mm: f32,
}

impl Resolution {
    /// Default pack: ~15–20 k vertices per ventricle, ~18 k cells.
    pub const DEFAULT: Resolution = Resolution {
        mesh_voxel_mm: 2.5,
        cell_spacing_mm: 2.3,
    };
    /// Fast fixture for unit tests (a few thousand cells).
    pub const COARSE: Resolution = Resolution {
        mesh_voxel_mm: 4.0,
        cell_spacing_mm: 4.0,
    };
}

impl Default for Resolution {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// What a render mesh represents (decides whether it gets hoods and colour from cell state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshKind {
    /// Excitable tissue: coloured from cell state through hoods.
    Myocardium,
    /// Great vessels: drawn with a flat material.
    Vessel,
    /// Valve annuli.
    Valve,
}

/// A render mesh with its structure name (`lv`, `rv`, `ra`, `la`, `aorta`, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedMesh {
    pub name: String,
    pub kind: MeshKind,
    pub mesh: TriMesh,
}

impl NamedMesh {
    /// Whether the mesh is coloured from cell state (and therefore needs hoods).
    #[must_use]
    pub fn conducting(&self) -> bool {
        self.kind == MeshKind::Myocardium
    }
}

/// Output of [`idealised_heart`].
#[derive(Debug, Clone)]
pub struct IdealisedHeart {
    pub meshes: Vec<NamedMesh>,
    pub cells: CellCloud,
    /// Purkinje–myocardial junction seed cells per fascicle: `rbb`, `laf`, `lpf`.
    pub pmj_seeds: BTreeMap<String, Vec<u32>>,
    /// AHA 17-segment index per cell (0 = not part of the LV model).
    pub aha_segments: Vec<u8>,
    /// Rotation from canonical (long axis = −y) to heart-frame coordinates.
    pub rotation: Quat,
}

/// Regions the procedural model populates (a subset of `Region::ALL`; pathological overlays and
/// Purkinje trees are added by scenarios / later briefs).
pub const IDEALISED_REGIONS: [Region; 21] = [
    Region::SinoatrialNode,
    Region::RightAtrium,
    Region::LeftAtrium,
    Region::CristaTerminalis,
    Region::BachmannBundle,
    Region::AtrialSeptum,
    Region::CavotricuspidIsthmus,
    Region::LeftAtrialAppendage,
    Region::CoronarySinus,
    Region::PulmonaryVeinSleeves,
    Region::AvNodeFastPathway,
    Region::AvNodeSlowPathway,
    Region::AvNodeCompact,
    Region::HisBundle,
    Region::RightBundleBranch,
    Region::LeftBundleBranch,
    Region::LeftAnteriorFascicle,
    Region::LeftPosteriorFascicle,
    Region::RightVentricle,
    Region::LeftVentricle,
    Region::VentricularSeptum,
];

/// Landmarks every procedural pack defines.
pub const LANDMARKS: [&str; 22] = [
    "sa_node",
    "av_node",
    "his",
    "rbb",
    "lbb",
    "laf",
    "lpf",
    "rbb_terminal",
    "rv_apex",
    "rvot",
    "rv_free_wall",
    "lv_apex",
    "lv_lateral",
    "lv_posterior",
    "lv_anterior",
    "lv_septal",
    "cs_os",
    "cs_distal",
    "bachmann",
    "ra_lateral",
    "la_roof",
    "laa",
];

// ----------------------------------------------------------------------------------------------
// Canonical geometry (mm). Long axis = y, apex at −y, +x = patient's left, +z = anterior.
// ----------------------------------------------------------------------------------------------

const LV_CENTRE: Vec3 = Vec3::ZERO;
const LV_ENDO: Vec3 = Vec3::new(25.0, 45.0, 25.0);
const LV_EPI: Vec3 = Vec3::new(35.0, 55.0, 35.0);
const LV_BASE_Y: f32 = 25.0;

const RV_CENTRE: Vec3 = Vec3::new(-30.0, -2.0, 10.0);
const RV_ENDO: Vec3 = Vec3::new(36.0, 44.0, 32.0);
const RV_EPI: Vec3 = Vec3::new(40.0, 48.0, 36.0);
const RV_BASE_Y: f32 = 22.0;

const RA_CENTRE: Vec3 = Vec3::new(-26.0, 51.0, -14.0);
const LA_CENTRE: Vec3 = Vec3::new(16.0, 50.0, -30.0);
const ATRIUM_EPI_R: f32 = 24.0;
const ATRIUM_ENDO_R: f32 = 20.5;
const ATRIUM_MID_R: f32 = 22.25;

const LAA_CENTRE: Vec3 = Vec3::new(32.0, 59.0, -10.0);
const LAA_EPI_R: f32 = 9.5;
const LAA_ENDO_R: f32 = 6.5;

const AORTA_A: Vec3 = Vec3::new(10.0, 25.0, 10.0);
const AORTA_B: Vec3 = Vec3::new(4.0, 72.0, 11.0);
const AORTA_ARCH: Vec3 = Vec3::new(16.0, 84.0, -4.0);
const AORTA_R: f32 = 13.0;
const PA_A: Vec3 = Vec3::new(-32.0, 22.0, 26.0);
const PA_B: Vec3 = Vec3::new(-10.0, 65.0, 24.0);
const PA_LEFT: Vec3 = Vec3::new(12.0, 72.0, 14.0);
const PA_RIGHT: Vec3 = Vec3::new(-34.0, 72.0, 12.0);
const PA_R: f32 = 12.0;
const SVC_A: Vec3 = Vec3::new(-28.0, 71.0, -14.0);
const SVC_B: Vec3 = Vec3::new(-28.0, 98.0, -14.0);
const IVC_A: Vec3 = Vec3::new(-30.0, 33.0, -18.0);
const IVC_B: Vec3 = Vec3::new(-34.0, 8.0, -36.0);
const VENA_CAVA_R: f32 = 10.0;
const PV_R: f32 = 6.0;
const PV_DIRS: [Vec3; 4] = [
    Vec3::new(0.6, 0.3, -0.74),
    Vec3::new(0.6, -0.3, -0.74),
    Vec3::new(-0.6, 0.3, -0.74),
    Vec3::new(-0.6, -0.3, -0.74),
];

const MITRAL_CENTRE: Vec3 = Vec3::new(3.0, 25.5, -10.0);
const AORTIC_CENTRE: Vec3 = Vec3::new(10.0, 25.5, 10.0);
const TRICUSPID_CENTRE: Vec3 = Vec3::new(-50.0, 22.5, 4.0);
const PULMONARY_CENTRE: Vec3 = Vec3::new(-32.0, 22.5, 26.0);

/// Azimuth (degrees, atan2(z, x)) of the septal direction from the LV axis.
const SEPTAL_AZIMUTH_DEG: f32 = 160.0;

/// Apex direction in the heart frame: left, inferior, anterior.
const APEX_DIRECTION: Vec3 = Vec3::new(0.60, -0.70, 0.38);

const CONDUCTION_SPACING_MM: f32 = 1.5;

// ---------------------------------------------------------------- implicit surfaces ----------

fn lv_endo(p: Vec3) -> f32 {
    ellipsoid(p, LV_CENTRE, LV_ENDO)
}
fn lv_epi(p: Vec3) -> f32 {
    ellipsoid(p, LV_CENTRE, LV_EPI)
}
fn rv_endo(p: Vec3) -> f32 {
    ellipsoid(p, RV_CENTRE, RV_ENDO)
}
fn rv_epi(p: Vec3) -> f32 {
    ellipsoid(p, RV_CENTRE, RV_EPI)
}
fn ra_epi(p: Vec3) -> f32 {
    sphere(p, RA_CENTRE, ATRIUM_EPI_R)
}
fn ra_endo(p: Vec3) -> f32 {
    sphere(p, RA_CENTRE, ATRIUM_ENDO_R)
}
fn la_epi(p: Vec3) -> f32 {
    sphere(p, LA_CENTRE, ATRIUM_EPI_R)
}
fn la_endo(p: Vec3) -> f32 {
    sphere(p, LA_CENTRE, ATRIUM_ENDO_R)
}
fn laa_epi(p: Vec3) -> f32 {
    sphere(p, LAA_CENTRE, LAA_EPI_R)
}
fn laa_endo(p: Vec3) -> f32 {
    sphere(p, LAA_CENTRE, LAA_ENDO_R)
}

/// LV wall (including the septum), cut at the base plane.
fn lv_wall(p: Vec3) -> f32 {
    let shell = subtract(lv_epi(p), lv_endo(p));
    shell.max(half_space(p, Vec3::Y * LV_BASE_Y, Vec3::Y))
}
/// RV free wall: the RV shell outside the LV, cut at its base plane.
fn rv_wall(p: Vec3) -> f32 {
    let shell = subtract(rv_epi(p), rv_endo(p));
    shell
        .max(-(lv_epi(p) + 0.3))
        .max(half_space(p, Vec3::Y * RV_BASE_Y, Vec3::Y))
}
/// RA wall: inside the RA outer sphere, outside its cavity (the cavity excludes the LA).
fn ra_wall(p: Vec3) -> f32 {
    let cavity = subtract(ra_endo(p), la_epi(p));
    subtract(ra_epi(p), cavity)
}
/// LA wall with appendage; the septal lens belongs to the RA mesh.
fn la_wall(p: Vec3) -> f32 {
    let outer = union(la_epi(p), laa_epi(p));
    let cavity = union(subtract(la_endo(p), ra_epi(p)), laa_endo(p));
    subtract(outer, cavity).max(-(ra_epi(p) + 0.3))
}
fn aorta(p: Vec3) -> f32 {
    union(
        capsule(p, AORTA_A, AORTA_B, AORTA_R),
        capsule(p, AORTA_B, AORTA_ARCH, AORTA_R * 0.95),
    )
}
fn pulmonary_artery(p: Vec3) -> f32 {
    capsule(p, PA_A, PA_B, PA_R)
        .min(capsule(p, PA_B, PA_LEFT, PA_R * 0.7))
        .min(capsule(p, PA_B, PA_RIGHT, PA_R * 0.7))
}
fn svc(p: Vec3) -> f32 {
    capsule(p, SVC_A, SVC_B, VENA_CAVA_R)
}
fn ivc(p: Vec3) -> f32 {
    capsule(p, IVC_A, IVC_B, VENA_CAVA_R)
}
fn pulmonary_veins(p: Vec3) -> f32 {
    PV_DIRS
        .iter()
        .map(|d| {
            let n = d.normalize();
            capsule(
                p,
                LA_CENTRE + n * (ATRIUM_ENDO_R + 1.0),
                LA_CENTRE + n * (ATRIUM_EPI_R + 14.0),
                PV_R,
            )
        })
        .fold(f32::INFINITY, f32::min)
}
fn valve_rings(p: Vec3) -> f32 {
    torus(p, MITRAL_CENTRE, Vec3::Y, 12.0, 1.8)
        .min(torus(p, AORTIC_CENTRE, Vec3::Y, 9.0, 1.5))
        .min(torus(p, TRICUSPID_CENTRE, Vec3::Y, 13.0, 1.8))
        .min(torus(p, PULMONARY_CENTRE, Vec3::Y, 9.0, 1.5))
}

/// Normalised transmural coordinate of `p` between the ellipsoids `inner` and `outer` (shared
/// centre): 0 on the inner surface, 1 on the outer. Monotone in the scale, so bisection is exact.
fn ellipsoid_depth(p: Vec3, centre: Vec3, inner: Vec3, outer: Vec3) -> f32 {
    let q = p - centre;
    let level = |lambda: f32| (q / inner.lerp(outer, lambda)).length() - 1.0;
    if level(0.0) <= 0.0 {
        return 0.0;
    }
    if level(1.0) >= 0.0 {
        return 1.0;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = f32::midpoint(lo, hi);
        if level(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    f32::midpoint(lo, hi)
}

/// Rule-based helix fibre at depth `d` on the ellipsoid family `(centre, inner→outer)`.
fn helix_fibre(p: Vec3, centre: Vec3, inner: Vec3, outer: Vec3, depth: f32) -> Vec3 {
    let radii = inner.lerp(outer, depth);
    let q = p - centre;
    let n = (q / (radii * radii)).normalize_or_zero(); // outward normal of the local ellipsoid
    let mut circ = Vec3::Y.cross(n);
    if circ.length_squared() < 1e-4 {
        circ = Vec3::X.cross(n); // at the apex the circumferential direction is undefined
    }
    let circ = circ.normalize();
    let long = n.cross(circ).normalize(); // apex → base
    let alpha = (60.0 * (1.0 - 2.0 * depth)).to_radians();
    (circ * alpha.cos() + long * alpha.sin()).normalize()
}

/// Circumferential fibre around a chamber's vertical axis.
fn circumferential_fibre(p: Vec3, centre: Vec3) -> Vec3 {
    let q = p - centre;
    let mut f = Vec3::Y.cross(q);
    if f.length_squared() < 1e-4 {
        f = Vec3::X.cross(q);
    }
    f.normalize_or_zero()
}

// ----------------------------------------------------------------- helper geometry -----------

/// Point on the LV endocardium (scaled by `scale` of the wall thickness outward) at azimuth and height.
fn lv_surface_point(azimuth_deg: f32, y: f32, radii: Vec3, scale: f32) -> Vec3 {
    let r = radii.x * (1.0 - (y / radii.y).powi(2)).max(0.0).sqrt() * scale;
    let (s, c) = azimuth_deg.to_radians().sin_cos();
    Vec3::new(r * c, y, r * s)
}

/// Point on the RV endocardial ellipsoid in direction `dir` from its centre.
fn rv_endo_point(dir: Vec3, scale: f32) -> Vec3 {
    let k = (dir / RV_ENDO).length();
    RV_CENTRE + dir / k * scale
}

/// Point on an atrial mid-wall sphere in direction `dir`.
fn on_sphere(centre: Vec3, dir: Vec3, r: f32) -> Vec3 {
    centre + dir.normalize() * r
}

/// Spherical arc on a sphere between two directions, as a polyline.
fn sphere_arc(centre: Vec3, from: Vec3, to: Vec3, r: f32, steps: usize) -> Vec<Vec3> {
    let (a, b) = (from.normalize(), to.normalize());
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let d = a.lerp(b, t).normalize();
            centre + d * r
        })
        .collect()
}

/// Resample a polyline at `spacing`, returning positions with unit tangents.
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
        .map(|w| capsule(p, w[0], w[1], 0.0))
        .fold(f32::INFINITY, f32::min)
}

fn polyline_tangent_at(p: Vec3, points: &[Vec3]) -> Vec3 {
    let mut best = (f32::INFINITY, Vec3::X);
    for w in points.windows(2) {
        let d = capsule(p, w[0], w[1], 0.0);
        if d < best.0 {
            best = (d, (w[1] - w[0]).normalize_or_zero());
        }
    }
    best.1
}

/// Lattice of points at `spacing` inside a sphere (small blobs: nodes).
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

// ---------------------------------------------------------------------- builder --------------

#[derive(Default)]
struct Cells {
    positions: Vec<Vec3>,
    fibres: Vec<Vec3>,
    regions: Vec<Region>,
    depth: Vec<f32>,
    aha: Vec<u8>,
}

impl Cells {
    fn push(&mut self, p: Vec3, f: Vec3, r: Region, d: f32, aha: u8) {
        self.positions.push(p);
        self.fibres.push(f);
        self.regions.push(r);
        self.depth.push(d);
        self.aha.push(aha);
    }
}

/// A render structure: name, kind, implicit function and canonical bounding box.
type Shape<'a> = (&'a str, MeshKind, &'a dyn Fn(Vec3) -> f32, Vec3, Vec3);

/// Conduction-system geometry (canonical coordinates), kept to derive landmarks and seeds.
struct Conduction {
    sa_point: Vec3,
    avn_point: Vec3,
    his: Vec<Vec3>,
    lbb: Vec<Vec3>,
    laf: Vec<Vec3>,
    lpf: Vec<Vec3>,
    rbb_septal: Vec<Vec3>,
    rbb_band: Vec<Vec3>,
    slow_pathway: Vec<Vec3>,
    coronary_sinus: Vec<Vec3>,
    bachmann: Vec<Vec3>,
    crista: Vec<Vec3>,
    isthmus: Vec<Vec3>,
    pv_ostia: Vec<Vec3>,
}

fn conduction_geometry() -> Conduction {
    let sa_point = on_sphere(RA_CENTRE, Vec3::new(-0.4, 0.85, -0.35), ATRIUM_MID_R);
    let avn_point = on_sphere(RA_CENTRE, Vec3::new(0.7, -0.7, 0.15), ATRIUM_MID_R);
    let septum_top = lv_surface_point(
        SEPTAL_AZIMUTH_DEG,
        LV_BASE_Y - 2.0,
        LV_ENDO.lerp(LV_EPI, 0.5),
        1.0,
    );
    let his = vec![
        avn_point.lerp(septum_top, 0.15),
        avn_point.lerp(septum_top, 0.5) + Vec3::new(0.0, -1.0, 0.0),
        septum_top,
    ];

    let endo_scale = 1.03;
    let lbb: Vec<Vec3> = (0..=6)
        .map(|i| {
            lv_surface_point(
                SEPTAL_AZIMUTH_DEG,
                22.0 - 17.0 * i as f32 / 6.0,
                LV_ENDO,
                endo_scale,
            )
        })
        .collect();
    let lbb_end = *lbb.last().expect("lbb has points");
    let fascicle = |az_end: f32, y_end: f32| -> Vec<Vec3> {
        let mut v = vec![lbb_end];
        for i in 1..=8 {
            let t = i as f32 / 8.0;
            let az = SEPTAL_AZIMUTH_DEG + (az_end - SEPTAL_AZIMUTH_DEG) * t;
            let y = 5.0 + (y_end - 5.0) * t;
            v.push(lv_surface_point(az, y, LV_ENDO, endo_scale));
        }
        v
    };
    let laf = fascicle(95.0, -15.0);
    let lpf = fascicle(235.0, -25.0);

    let rbb_septal: Vec<Vec3> = (0..=10)
        .map(|i| lv_surface_point(SEPTAL_AZIMUTH_DEG, 22.0 - 47.0 * i as f32 / 10.0, LV_EPI, 1.02))
        .collect();
    let rbb_terminal = rv_endo_point(Vec3::new(-0.5, -0.6, 0.6), 0.97);
    let rbb_band = vec![*rbb_septal.last().expect("rbb has points"), rbb_terminal];

    let slow_pathway = sphere_arc(
        RA_CENTRE,
        avn_point - RA_CENTRE,
        Vec3::new(0.25, -0.85, -0.5),
        ATRIUM_MID_R,
        6,
    );
    let coronary_sinus = vec![
        Vec3::new(-10.0, 26.0, -32.0),
        Vec3::new(6.0, 27.0, -35.0),
        Vec3::new(22.0, 27.0, -27.0),
    ];
    let bachmann = vec![
        on_sphere(RA_CENTRE, Vec3::new(0.35, 0.6, 0.7), ATRIUM_MID_R),
        on_sphere(LA_CENTRE, Vec3::new(-0.35, 0.6, 0.7), ATRIUM_MID_R),
    ];
    let crista = sphere_arc(
        RA_CENTRE,
        Vec3::new(-0.55, 0.75, -0.35),
        Vec3::new(-0.45, -0.8, -0.4),
        ATRIUM_MID_R,
        10,
    );
    let isthmus = sphere_arc(
        RA_CENTRE,
        Vec3::new(-0.25, -0.85, -0.45),
        Vec3::new(0.45, -0.85, 0.25),
        ATRIUM_MID_R,
        6,
    );
    let pv_ostia = PV_DIRS
        .iter()
        .map(|d| on_sphere(LA_CENTRE, *d, ATRIUM_EPI_R))
        .collect();
    Conduction {
        sa_point,
        avn_point,
        his,
        lbb,
        laf,
        lpf,
        rbb_septal,
        rbb_band,
        slow_pathway,
        coronary_sinus,
        bachmann,
        crista,
        isthmus,
        pv_ostia,
    }
}

fn ventricular_aha(p: Vec3) -> u8 {
    let apex_base = ((p.y + LV_EPI.y) / (LV_EPI.y + LV_BASE_Y)).clamp(0.0, 1.0);
    aha_segment(apex_base, p.z.atan2(p.x).to_degrees())
}

/// Classify one lattice point; `None` outside the myocardium.
fn classify_lattice_point(p: Vec3, con: &Conduction) -> Option<(Region, Vec3, f32, u8)> {
    if lv_wall(p) < 0.0 {
        let d = ellipsoid_depth(p, LV_CENTRE, LV_ENDO, LV_EPI);
        let f = helix_fibre(p, LV_CENTRE, LV_ENDO, LV_EPI, d);
        // Septum = the LV wall that faces the RV cavity: inside the RV ellipsoid and within ±60°
        // of the septal direction (the anterior / inferior interventricular grooves).
        let azimuth = p.z.atan2(p.x).to_degrees();
        let from_septal = (azimuth - SEPTAL_AZIMUTH_DEG + 180.0).rem_euclid(360.0) - 180.0;
        let region = if rv_epi(p) < 0.0 && from_septal.abs() <= 60.0 {
            Region::VentricularSeptum
        } else {
            Region::LeftVentricle
        };
        return Some((region, f, d, ventricular_aha(p)));
    }
    if rv_wall(p) < 0.0 {
        let d = ellipsoid_depth(p, RV_CENTRE, RV_ENDO, RV_EPI);
        let f = helix_fibre(p, RV_CENTRE, RV_ENDO, RV_EPI, d);
        return Some((Region::RightVentricle, f, d, 0));
    }
    let in_ra = ra_wall(p) < 0.0;
    let in_la = la_wall(p) < 0.0;
    if !(in_ra || in_la) {
        return None;
    }
    let centre = if in_ra { RA_CENTRE } else { LA_CENTRE };
    let mut fibre = circumferential_fibre(p, centre);
    let mut region = if in_ra {
        Region::RightAtrium
    } else {
        Region::LeftAtrium
    };
    if in_la && laa_epi(p) < 0.0 && la_epi(p) >= 0.0 {
        region = Region::LeftAtrialAppendage;
        fibre = circumferential_fibre(p, LAA_CENTRE);
    }
    if ra_epi(p) < 0.0 && la_epi(p) < 0.0 {
        region = Region::AtrialSeptum;
    }
    if distance_to_polyline(p, &con.bachmann) < 4.0 {
        region = Region::BachmannBundle;
        fibre = polyline_tangent_at(p, &con.bachmann);
    } else if in_ra && distance_to_polyline(p, &con.crista) < 4.0 {
        region = Region::CristaTerminalis;
        fibre = polyline_tangent_at(p, &con.crista);
    } else if in_ra && distance_to_polyline(p, &con.isthmus) < 5.0 {
        region = Region::CavotricuspidIsthmus;
    } else if in_la && con.pv_ostia.iter().any(|o| o.distance(p) < 6.5) {
        region = Region::PulmonaryVeinSleeves;
    }
    Some((region, fibre, 0.5, 0))
}

fn push_polyline(cells: &mut Cells, points: &[Vec3], region: Region, spacing: f32, depth: f32) {
    for (p, t) in sample_polyline(points, spacing) {
        cells.push(p, t, region, depth, 0);
    }
}

fn push_blob(cells: &mut Cells, centre: Vec3, r: f32, region: Region, fibre: Vec3) {
    for p in blob(centre, r, CONDUCTION_SPACING_MM) {
        cells.push(p, fibre, region, 0.5, 0);
    }
}

/// Index of the cell nearest to `p` among those satisfying `pred`.
fn nearest_cell(cells: &Cells, p: Vec3, pred: impl Fn(Region) -> bool) -> Option<u32> {
    let mut best: Option<(f32, u32)> = None;
    for (i, q) in cells.positions.iter().enumerate() {
        if pred(cells.regions[i]) {
            let d = q.distance_squared(p);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, i as u32));
            }
        }
    }
    best.map(|b| b.1)
}

/// Build the idealised heart at `resolution`.
#[must_use]
pub fn idealised_heart(resolution: Resolution) -> IdealisedHeart {
    let spacing = resolution.cell_spacing_mm.max(0.5);
    let con = conduction_geometry();
    let mut cells = Cells::default();

    // --- myocardial lattice ---------------------------------------------------------------
    let lo = Vec3::new(-72.0, -60.0, -62.0);
    let hi = Vec3::new(62.0, 80.0, 60.0);
    let n = ((hi - lo) / spacing).ceil();
    for i in 0..=n.x as i32 {
        for j in 0..=n.y as i32 {
            for k in 0..=n.z as i32 {
                let p = lo + Vec3::new(i as f32, j as f32, k as f32) * spacing;
                if let Some((region, fibre, depth, aha)) = classify_lattice_point(p, &con) {
                    cells.push(p, fibre, region, depth, aha);
                }
            }
        }
    }

    // --- explicit structures (nodes, bundles, coronary sinus) ----------------------------
    push_blob(
        &mut cells,
        con.sa_point,
        3.0,
        Region::SinoatrialNode,
        polyline_tangent_at(con.sa_point, &con.crista),
    );
    push_blob(&mut cells, con.avn_point, 2.5, Region::AvNodeCompact, Vec3::Y);
    let fast = con.avn_point + Vec3::new(1.0, 5.0, 4.0);
    let fast = on_sphere(RA_CENTRE, fast - RA_CENTRE, ATRIUM_MID_R);
    push_blob(
        &mut cells,
        fast,
        2.0,
        Region::AvNodeFastPathway,
        (con.avn_point - fast).normalize(),
    );
    push_polyline(
        &mut cells,
        &con.slow_pathway,
        Region::AvNodeSlowPathway,
        CONDUCTION_SPACING_MM,
        0.5,
    );
    let his_start = cells.positions.len() as u32;
    push_polyline(
        &mut cells,
        &con.his,
        Region::HisBundle,
        CONDUCTION_SPACING_MM,
        0.0,
    );
    let lbb_start = cells.positions.len() as u32;
    push_polyline(
        &mut cells,
        &con.lbb,
        Region::LeftBundleBranch,
        CONDUCTION_SPACING_MM,
        0.0,
    );
    push_polyline(
        &mut cells,
        &con.laf,
        Region::LeftAnteriorFascicle,
        CONDUCTION_SPACING_MM,
        0.0,
    );
    let laf_end = cells.positions.len() as u32 - 1;
    push_polyline(
        &mut cells,
        &con.lpf,
        Region::LeftPosteriorFascicle,
        CONDUCTION_SPACING_MM,
        0.0,
    );
    let lpf_end = cells.positions.len() as u32 - 1;
    let rbb_start = cells.positions.len() as u32;
    push_polyline(
        &mut cells,
        &con.rbb_septal,
        Region::RightBundleBranch,
        CONDUCTION_SPACING_MM,
        1.0,
    );
    push_polyline(
        &mut cells,
        &con.rbb_band,
        Region::RightBundleBranch,
        CONDUCTION_SPACING_MM,
        0.0,
    );
    let rbb_end = cells.positions.len() as u32 - 1;
    let cs_start = cells.positions.len() as u32;
    push_polyline(&mut cells, &con.coronary_sinus, Region::CoronarySinus, 2.0, 0.5);
    let cs_end = cells.positions.len() as u32 - 1;

    // --- landmarks (canonical) -------------------------------------------------------------
    let vent = |r: Region| r.is_ventricular();
    let mut landmarks: BTreeMap<String, u32> = BTreeMap::new();
    let mut put = |name: &str, idx: Option<u32>| {
        if let Some(i) = idx {
            landmarks.insert(name.to_string(), i);
        }
    };
    put(
        "sa_node",
        nearest_cell(&cells, con.sa_point, |r| r == Region::SinoatrialNode),
    );
    put(
        "av_node",
        nearest_cell(&cells, con.avn_point, |r| r == Region::AvNodeCompact),
    );
    put("his", Some(his_start));
    put("lbb", Some(lbb_start));
    put("laf", Some(laf_end));
    put("lpf", Some(lpf_end));
    put("rbb", Some(rbb_start));
    put("rbb_terminal", Some(rbb_end));
    put("cs_os", Some(cs_start));
    put("cs_distal", Some(cs_end));
    let rv = |r: Region| r == Region::RightVentricle;
    put(
        "rv_apex",
        nearest_cell(&cells, rv_endo_point(Vec3::new(-0.2, -1.0, 0.3), 1.0), rv),
    );
    put(
        "rvot",
        nearest_cell(&cells, rv_endo_point(Vec3::new(-0.5, 0.45, 0.75), 1.0), rv),
    );
    put(
        "rv_free_wall",
        nearest_cell(&cells, rv_endo_point(Vec3::new(-1.0, -0.1, 0.2), 1.0), rv),
    );
    let mid = LV_ENDO.lerp(LV_EPI, 0.5);
    put("lv_apex", nearest_cell(&cells, Vec3::new(0.0, -mid.y, 0.0), vent));
    put(
        "lv_lateral",
        nearest_cell(&cells, lv_surface_point(0.0, -5.0, mid, 1.0), vent),
    );
    put(
        "lv_posterior",
        nearest_cell(&cells, lv_surface_point(-80.0, -15.0, mid, 1.0), vent),
    );
    put(
        "lv_anterior",
        nearest_cell(&cells, lv_surface_point(90.0, -5.0, mid, 1.0), |r| {
            r == Region::LeftVentricle
        }),
    );
    put(
        "lv_septal",
        nearest_cell(
            &cells,
            lv_surface_point(SEPTAL_AZIMUTH_DEG, -5.0, mid, 1.0),
            |r| r == Region::VentricularSeptum,
        ),
    );
    let bach_mid = con.bachmann[0].lerp(con.bachmann[1], 0.5);
    put(
        "bachmann",
        nearest_cell(&cells, bach_mid, |r| r == Region::BachmannBundle),
    );
    put(
        "ra_lateral",
        nearest_cell(
            &cells,
            on_sphere(RA_CENTRE, Vec3::new(-1.0, 0.0, 0.2), ATRIUM_MID_R),
            |r| r == Region::RightAtrium,
        ),
    );
    put(
        "la_roof",
        nearest_cell(
            &cells,
            on_sphere(LA_CENTRE, Vec3::new(0.2, 1.0, -0.2), ATRIUM_MID_R),
            |r| r == Region::LeftAtrium,
        ),
    );
    put(
        "laa",
        nearest_cell(&cells, LAA_CENTRE, |r| r == Region::LeftAtrialAppendage),
    );

    // --- PMJ seed sets ---------------------------------------------------------------------
    let seeds = |pred: &dyn Fn(usize) -> bool| -> Vec<u32> {
        (0..cells.positions.len())
            .filter(|i| pred(*i))
            .map(|i| i as u32)
            .collect()
    };
    let rbb_terminal = con.rbb_band[1];
    let rbb_lower: Vec<Vec3> = con.rbb_septal[5..].to_vec();
    let laf_t = *con.laf.last().expect("laf");
    let lpf_t = *con.lpf.last().expect("lpf");
    let mut pmj_seeds = BTreeMap::new();
    pmj_seeds.insert(
        "rbb".to_string(),
        seeds(&|i| {
            let p = cells.positions[i];
            match cells.regions[i] {
                Region::RightVentricle => cells.depth[i] < 0.5 && p.distance(rbb_terminal) < 15.0,
                Region::VentricularSeptum => {
                    cells.depth[i] > 0.7 && distance_to_polyline(p, &rbb_lower) < 8.0
                }
                _ => false,
            }
        }),
    );
    for (name, terminal) in [("laf", laf_t), ("lpf", lpf_t)] {
        pmj_seeds.insert(
            name.to_string(),
            seeds(&|i| {
                let p = cells.positions[i];
                matches!(
                    cells.regions[i],
                    Region::LeftVentricle | Region::VentricularSeptum
                ) && cells.depth[i] < 0.3
                    && p.distance(terminal) < 15.0
            }),
        );
    }

    // --- transform to the heart frame ------------------------------------------------------
    let (sum, count) = cells
        .positions
        .iter()
        .zip(&cells.regions)
        .filter(|(_, r)| r.is_ventricular())
        .fold((Vec3::ZERO, 0usize), |(s, c), (p, _)| (s + *p, c + 1));
    let centroid = if count > 0 { sum / count as f32 } else { Vec3::ZERO };
    let rotation = Quat::from_rotation_arc(Vec3::NEG_Y, APEX_DIRECTION.normalize());
    let xf = |p: Vec3| rotation * (p - centroid);
    let positions: Vec<Vec3> = cells.positions.iter().map(|p| xf(*p)).collect();
    let fibres: Vec<Vec3> = cells
        .fibres
        .iter()
        .map(|f| (rotation * *f).normalize_or_zero())
        .collect();
    let mut cloud = CellCloud::build(positions, fibres, cells.regions.clone(), cells.depth.clone());
    cloud.landmarks = landmarks;

    // --- render meshes ---------------------------------------------------------------------
    let v = resolution.mesh_voxel_mm.max(0.25);
    let m = Vec3::splat(2.0 * v);
    let shapes: [Shape<'_>; 10] = [
        (
            "lv",
            MeshKind::Myocardium,
            &lv_wall,
            LV_CENTRE - LV_EPI,
            Vec3::new(LV_EPI.x, LV_BASE_Y, LV_EPI.z),
        ),
        (
            "rv",
            MeshKind::Myocardium,
            &rv_wall,
            RV_CENTRE - RV_EPI,
            RV_CENTRE + Vec3::new(RV_EPI.x, RV_BASE_Y, RV_EPI.z),
        ),
        (
            "ra",
            MeshKind::Myocardium,
            &ra_wall,
            RA_CENTRE - Vec3::splat(ATRIUM_EPI_R),
            RA_CENTRE + Vec3::splat(ATRIUM_EPI_R),
        ),
        (
            "la",
            MeshKind::Myocardium,
            &la_wall,
            LA_CENTRE.min(LAA_CENTRE) - Vec3::splat(ATRIUM_EPI_R),
            LA_CENTRE.max(LAA_CENTRE) + Vec3::splat(ATRIUM_EPI_R),
        ),
        (
            "aorta",
            MeshKind::Vessel,
            &aorta,
            AORTA_A.min(AORTA_B).min(AORTA_ARCH) - Vec3::splat(AORTA_R),
            AORTA_A.max(AORTA_B).max(AORTA_ARCH) + Vec3::splat(AORTA_R),
        ),
        (
            "pulmonary_artery",
            MeshKind::Vessel,
            &pulmonary_artery,
            PA_A.min(PA_B).min(PA_LEFT).min(PA_RIGHT) - Vec3::splat(PA_R),
            PA_A.max(PA_B).max(PA_LEFT).max(PA_RIGHT) + Vec3::splat(PA_R),
        ),
        (
            "svc",
            MeshKind::Vessel,
            &svc,
            SVC_A.min(SVC_B) - Vec3::splat(VENA_CAVA_R),
            SVC_A.max(SVC_B) + Vec3::splat(VENA_CAVA_R),
        ),
        (
            "ivc",
            MeshKind::Vessel,
            &ivc,
            IVC_A.min(IVC_B) - Vec3::splat(VENA_CAVA_R),
            IVC_A.max(IVC_B) + Vec3::splat(VENA_CAVA_R),
        ),
        (
            "pulmonary_veins",
            MeshKind::Vessel,
            &pulmonary_veins,
            LA_CENTRE - Vec3::splat(ATRIUM_EPI_R + 14.0 + PV_R),
            LA_CENTRE + Vec3::splat(ATRIUM_EPI_R + 14.0 + PV_R),
        ),
        (
            "valve_rings",
            MeshKind::Valve,
            &valve_rings,
            Vec3::new(-68.0, LV_BASE_Y - 3.0, -30.0),
            Vec3::new(25.0, LV_BASE_Y + 3.0, 42.0),
        ),
    ];
    let meshes = shapes
        .iter()
        .map(|(name, kind, f, lo, hi)| {
            let mut mesh = polygonise(*f, *lo - m, *hi + m, v);
            for vert in &mut mesh.vertices {
                vert.position = xf(Vec3::from(vert.position)).to_array();
                vert.normal = (rotation * Vec3::from(vert.normal)).to_array();
            }
            NamedMesh {
                name: (*name).to_string(),
                kind: *kind,
                mesh,
            }
        })
        .collect();

    IdealisedHeart {
        meshes,
        cells: cloud,
        pmj_seeds,
        aha_segments: cells.aha,
        rotation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_is_zero_on_endo_and_one_on_epi() {
        let on_endo = Vec3::new(LV_ENDO.x, 0.0, 0.0);
        let on_epi = Vec3::new(0.0, 0.0, LV_EPI.z);
        assert!(ellipsoid_depth(on_endo, LV_CENTRE, LV_ENDO, LV_EPI) < 1e-3);
        assert!(ellipsoid_depth(on_epi, LV_CENTRE, LV_ENDO, LV_EPI) > 1.0 - 1e-3);
        let mid = Vec3::new(30.0, 0.0, 0.0);
        assert!((ellipsoid_depth(mid, LV_CENTRE, LV_ENDO, LV_EPI) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn helix_fibre_is_tangent_and_rotates_with_depth() {
        let p = Vec3::new(30.0, 0.0, 0.0);
        let n = Vec3::X;
        for d in [0.0f32, 0.5, 1.0] {
            let f = helix_fibre(p, LV_CENTRE, LV_ENDO, LV_EPI, d);
            assert!((f.length() - 1.0).abs() < 1e-5);
            assert!(f.dot(n).abs() < 1e-3, "fibre tangent to wall");
        }
        let endo = helix_fibre(p, LV_CENTRE, LV_ENDO, LV_EPI, 0.0);
        let epi = helix_fibre(p, LV_CENTRE, LV_ENDO, LV_EPI, 1.0);
        let midw = helix_fibre(p, LV_CENTRE, LV_ENDO, LV_EPI, 0.5);
        assert!(
            midw.dot(Vec3::Y).abs() < 1e-3,
            "mid-wall fibre is circumferential"
        );
        assert!(endo.y > 0.8 && epi.y < -0.8, "±60° helix");
    }

    #[test]
    fn coarse_heart_builds_quickly_with_all_regions() {
        let h = idealised_heart(Resolution::COARSE);
        assert!(h.cells.len() > 1000);
        assert_eq!(h.meshes.len(), 10);
        for r in IDEALISED_REGIONS {
            assert!(h.cells.regions.contains(&r), "{r:?} missing at coarse resolution");
        }
        for name in LANDMARKS {
            assert!(h.cells.landmarks.contains_key(name), "landmark {name}");
        }
    }
}
