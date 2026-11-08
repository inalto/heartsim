//! Body surface and electrode sites from the KCL whole-torso CT model (Qian & Bishop 2022,
//! Zenodo 7253863, CC BY 4.0): the skin tag's outer boundary, closed at the CT's cut planes,
//! posed in the heart frame with the subject's own ventricular centroid at the origin — so the
//! heart sits where this subject's heart was, in a real chest.
//!
//! The torso is scaled uniformly about that centroid so the subject's LV (mitral centre to the
//! farthest epicardial point) has the length of the baked heart's LV: the body is sized to the
//! heart it contains, and the chest-wall distances the forward model sees are those of an adult of
//! the heart's size rather than of the CT subject.
//!
//! Electrodes are placed as for the procedural torso (`procedural::torso::ELECTRODE_SITES`):
//! a ray from the torso axis at the site's height and azimuth, intersected with the skin, so
//! every site lies on the mesh. The limb sites are clamped inside the CT's vertical extent.

use super::surface::{cap_open_rims, decimate, faces_to_mesh, ray_hit_far, taubin_smooth, transform};
use super::tet::TetMesh;
use anyhow::{Result, bail};
use glam::{Mat3, Quat, Vec3};
use heartsim_geometry::procedural::torso::ELECTRODE_SITES;
use heartsim_geometry::{ElectrodeSet, TriMesh};
use std::collections::HashSet;

/// KCL tags used here (`KCL_TorsoComputationalModels_Info.pdf`).
pub mod tag {
    pub const SKIN: u16 = 2;
    pub const MITRAL: u16 = 16;
    pub const RV_WALL: u16 = 34;
    pub const LV_WALL: u16 = 35;
}

pub struct BakedTorso {
    pub mesh: TriMesh,
    pub electrodes: ElectrodeSet,
    pub report: Vec<String>,
}

/// DICOM LPS (+x left, +y posterior, +z superior) → heart frame (+x left, +y cranial, +z anterior).
fn lps_to_heart(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y)
}

pub fn bake_torso(mesh: &TetMesh, triangles: usize, heart_lv_length_mm: f32) -> Result<BakedTorso> {
    let mut report = Vec::new();
    // the subject's ventricular centroid and long axis
    let (vs, vn) = (0..mesh.tets.len())
        .filter(|t| matches!(mesh.tags[*t], tag::LV_WALL | tag::RV_WALL))
        .fold((Vec3::ZERO, 0usize), |(s, n), t| (s + mesh.centroid(t), n + 1));
    if vn == 0 {
        bail!(
            "torso mesh has no ventricular wall tags {} / {}",
            tag::LV_WALL,
            tag::RV_WALL
        );
    }
    let vc = vs / vn as f32;
    let mv = mesh.tag_centroid(tag::MITRAL);
    let apex = mesh
        .nodes_of_tag(tag::LV_WALL)
        .iter()
        .map(|n| mesh.points[*n as usize])
        .max_by(|a, b| a.distance_squared(mv).total_cmp(&b.distance_squared(mv)))
        .unwrap_or(vc);
    let axis = lps_to_heart((apex - mv).normalize());
    let subject_lv = apex.distance(mv);
    let scale = if heart_lv_length_mm > 0.0 {
        heart_lv_length_mm / subject_lv
    } else {
        1.0
    };
    report.push(format!(
        "subject's LV long axis: frontal {:.0}°, transverse {:.0}° (population mean 36°/46° is the pose used); mitral→apex {subject_lv:.0} mm, heart {heart_lv_length_mm:.0} mm → torso scaled ×{scale:.3}",
        (-axis.y).atan2(axis.x).to_degrees(),
        axis.z.atan2(axis.x).to_degrees(),
    ));

    // outer skin: boundary faces of the skin layer that are not shared with the inner body
    let skin_faces = mesh.boundary_faces(|t| mesh.tags[t] == tag::SKIN);
    let keyed: HashSet<[u32; 3]> = skin_faces
        .iter()
        .map(|f| {
            let mut k = *f;
            k.sort_unstable();
            k
        })
        .collect();
    let mut inner: HashSet<[u32; 3]> = HashSet::new();
    for t in 0..mesh.tets.len() {
        if mesh.tags[t] == tag::SKIN {
            continue;
        }
        let tet = mesh.tets[t];
        for i in 0..4 {
            let mut k = [tet[(i + 1) % 4], tet[(i + 2) % 4], tet[(i + 3) % 4]];
            k.sort_unstable();
            if keyed.contains(&k) {
                inner.insert(k);
            }
        }
    }
    let outer: Vec<[u32; 3]> = skin_faces
        .iter()
        .filter(|f| {
            let mut k = **f;
            k.sort_unstable();
            !inner.contains(&k)
        })
        .copied()
        .collect();
    report.push(format!(
        "skin: {} boundary faces, {} outer after removing the {} shared with inner tissue",
        skin_faces.len(),
        outer.len(),
        inner.len()
    ));
    let mut torso = faces_to_mesh(&mesh.points, &outer);
    cap_open_rims(&mut torso);
    taubin_smooth(&mut torso, 2);
    decimate(&mut torso, triangles);
    let rot = Quat::from_mat3(&Mat3::from_cols(Vec3::X, Vec3::NEG_Z, Vec3::Y)).normalize();
    transform(&mut torso, |p| lps_to_heart((p - vc) * scale), rot);
    torso.recompute_normals();
    let (lo, hi) = torso.bounds();
    report.push(format!(
        "torso in the heart frame: x {:.0}…{:.0}, y {:.0}…{:.0}, z {:.0}…{:.0} mm, {} vertices {} triangles",
        lo.x,
        hi.x,
        lo.y,
        hi.y,
        lo.z,
        hi.z,
        torso.vertex_count(),
        torso.triangle_count()
    ));

    // torso axis at heart level: centre of the skin cross-section around y = 0
    let slice: Vec<Vec3> = torso
        .vertices
        .iter()
        .map(|v| Vec3::from(v.position))
        .filter(|p| p.y.abs() < 15.0)
        .collect();
    let (sx, sz) = slice.iter().fold((f32::INFINITY, f32::INFINITY), |(x, z), p| {
        (x.min(p.x), z.min(p.z))
    });
    let (ex, ez) = slice
        .iter()
        .fold((f32::NEG_INFINITY, f32::NEG_INFINITY), |(x, z), p| {
            (x.max(p.x), z.max(p.z))
        });
    // the same axis `heartsim_ecg::placement::TorsoSurface` uses: the centre of the bounding box
    let axis_point = Vec3::new(f32::midpoint(lo.x, hi.x), 0.0, f32::midpoint(lo.z, hi.z));
    report.push(format!(
        "chest at heart level: {:.0} mm wide, {:.0} mm deep; ventricular centroid {:.0} mm left of the midline, {:.0} mm behind the sternum",
        ex - sx,
        ez - sz,
        -f32::midpoint(sx, ex),
        ez
    ));

    let mut electrodes = ElectrodeSet {
        frame: "heart frame: mm, origin = ventricular centroid, +x patient left, +y cranial, +z anterior"
            .to_string(),
        sites: std::collections::BTreeMap::new(),
    };
    for (name, y, az) in ELECTRODE_SITES {
        let y = y.clamp(lo.y + 20.0, hi.y - 20.0);
        let (s, c) = az.to_radians().sin_cos();
        let dir = Vec3::new(s, 0.0, c);
        let origin = Vec3::new(axis_point.x, y, axis_point.z);
        let Some(p) = ray_hit_far(&torso, origin, dir) else {
            bail!("electrode {name}: the ray at height {y} mm, azimuth {az}° misses the skin");
        };
        electrodes.set(name, p);
    }
    Ok(BakedTorso {
        mesh: torso,
        electrodes,
        report,
    })
}
