//! Label volumes for the echo cross-section (brief B31, `docs/anatomy-pack.md` *Label volumes*):
//! the **heart box** from the Rodero tetrahedra at 1 mm and the **thorax box** from the KCL torso
//! at 2 mm, both in the pack's heart frame (the transforms the meshes went through).
//!
//! Everything is serial and ordered, so the same sources give the same bytes.
//!
//! **Heart box.** Each myocardial or vessel-wall tet claims the voxels whose centres it contains;
//! the thin parts (atria and their inlet borders, vessel walls, valve-plane discs, inlet lids) also
//! claim every voxel one of their sub-sample points falls in, so that a sub-voxel wall still
//! separates two cavities for a 6-connected fill. The aorta and pulmonary artery are open where the
//! dataset crops them: each crop plane is found as the largest planar cluster of the wall's free
//! boundary faces and capped with a disc. Blood is then what a 6-connected flood fill from the box
//! boundary does not reach, each enclosed component named after the wall that surrounds most of it;
//! the valve discs become the blood beside them, the lids atrial wall, the caps vessel wall.
//!
//! **Thorax box.** The KCL tags are verified by volume and position before use (the information
//! sheet's numbers are off by one around the vessels), rasterised by voxel centre, and resolved by
//! priority: pack heart (dilated 2 mm, becomes soft tissue: the heart box is sampled first) > bone >
//! lung > liver > KCL great-vessel blood > KCL heart (soft tissue) > every other tissue.

use super::tet::{TetMesh, fit_disc};
use super::torso::lps_to_heart;
use anyhow::{Result, bail};
use glam::{DVec3, Mat3, Vec3};
use heartsim_core::Region;
use heartsim_geometry::{CellCloud, Label, LabelVolume, LabelVolumes};
use std::collections::BTreeMap;

/// Edge of a heart-box voxel, mm.
pub const HEART_VOXEL_MM: f32 = 1.0;
/// Edge of a thorax-box voxel, mm.
pub const THORAX_VOXEL_MM: f32 = 2.0;
/// Margin around the Rodero mesh in the heart box, mm (room for the pericardium and the leaflets).
const HEART_MARGIN_MM: f32 = 6.0;
/// The pack heart takes over the thorax box this far around its voxels, mm.
const PACK_HEART_DILATION_MM: f32 = 2.0;
/// Accepted range of the LV wall thickness medians per AHA level, mm (brief B31: the bake fails
/// outside it).
const LV_WALL_RANGE_MM: (f32, f32) = (5.0, 13.0);

/// Rodero tags (`heart::tag`), repeated for the classes here.
mod rt {
    pub const AORTA: u16 = 5;
    pub const PA: u16 = 6;
}

/// KCL torso 1 tags as verified by [`verify_kcl_tags`] (volumes and positions in the bake report).
pub mod kcl {
    pub const INNER_BODY: u16 = 1;
    pub const SKIN: u16 = 2;
    pub const BONE: u16 = 3;
    pub const LIVER: u16 = 5;
    pub const LUNGS: u16 = 8;
    pub const SVC_WALL: u16 = 9;
    pub const SVC_POOL: u16 = 10;
    pub const PA_WALL: u16 = 12;
    pub const PA_POOL: u16 = 13;
    pub const AORTA_WALL: u16 = 14;
    pub const AORTA_POOL: u16 = 15;
    pub const LA_POOL: u16 = 20;
    pub const LA_WALL: u16 = 21;
    pub const RA_POOL: u16 = 22;
    pub const RA_WALL: u16 = 23;
    pub const RV_POOL: u16 = 24;
    pub const LV_POOL: u16 = 25;
    pub const RV_WALL: u16 = 34;
    pub const LV_WALL: u16 = 35;
    /// The subject's own heart: chamber walls and pools, valve planes (16–19), the SVC plane (11).
    pub const HEART: [u16; 15] = [11, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 34, 35, 9, 12];
}

/// Report a line now (so a failing bake shows how far it got) and keep it.
fn say(report: &mut Vec<String>, line: String) {
    eprintln!("  {line}");
    report.push(line);
}

/// Regular grid description.
#[derive(Debug, Clone, Copy)]
struct Grid {
    origin: Vec3,
    h: f32,
    dims: [usize; 3],
}

impl Grid {
    fn covering(lo: Vec3, hi: Vec3, h: f32) -> Self {
        let lo = (lo / h).floor() * h;
        let hi = (hi / h).ceil() * h;
        let n = ((hi - lo) / h).round();
        Self {
            origin: lo,
            h,
            dims: [n.x as usize, n.y as usize, n.z as usize],
        }
    }
    fn len(&self) -> usize {
        self.dims[0] * self.dims[1] * self.dims[2]
    }
    fn idx(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.dims[0] * (j + self.dims[1] * k)
    }
    fn ijk(&self, idx: usize) -> [usize; 3] {
        let (nx, ny) = (self.dims[0], self.dims[1]);
        [idx % nx, (idx / nx) % ny, idx / (nx * ny)]
    }
    fn centre(&self, i: usize, j: usize, k: usize) -> Vec3 {
        self.origin + (Vec3::new(i as f32, j as f32, k as f32) + 0.5) * self.h
    }
    fn voxel_of(&self, p: Vec3) -> Option<[usize; 3]> {
        let q = (p - self.origin) / self.h;
        let mut out = [0usize; 3];
        for a in 0..3 {
            if !(q[a] >= 0.0 && q[a] < self.dims[a] as f32) {
                return None;
            }
            out[a] = q[a] as usize;
        }
        Some(out)
    }
    /// Voxel index range whose centres can lie in `[lo, hi]`.
    fn range(&self, lo: Vec3, hi: Vec3) -> Option<[(usize, usize); 3]> {
        let mut r = [(0usize, 0usize); 3];
        for a in 0..3 {
            let a0 = ((lo[a] - self.origin[a]) / self.h - 0.5).ceil().max(0.0);
            let a1 = ((hi[a] - self.origin[a]) / self.h - 0.5)
                .floor()
                .min(self.dims[a] as f32 - 1.0);
            if a1 < a0 {
                return None;
            }
            r[a] = (a0 as usize, a1 as usize);
        }
        Some(r)
    }
    /// The 6 face neighbours of `idx` inside the grid.
    fn neighbours6(&self, idx: usize, out: &mut Vec<usize>) {
        out.clear();
        let [i, j, k] = self.ijk(idx);
        let [nx, ny, nz] = self.dims;
        if i > 0 {
            out.push(idx - 1);
        }
        if i + 1 < nx {
            out.push(idx + 1);
        }
        if j > 0 {
            out.push(idx - nx);
        }
        if j + 1 < ny {
            out.push(idx + nx);
        }
        if k > 0 {
            out.push(idx - nx * ny);
        }
        if k + 1 < nz {
            out.push(idx + nx * ny);
        }
    }
    fn on_boundary(&self, idx: usize) -> bool {
        let [i, j, k] = self.ijk(idx);
        i == 0 || j == 0 || k == 0 || i + 1 == self.dims[0] || j + 1 == self.dims[1] || k + 1 == self.dims[2]
    }
}

/// Rasterise the tets of `mesh` (points through `xf`) into `out`: a tet of class `c = class_of(tag)`
/// (0 = skip) claims each voxel whose centre it contains, and with `conservative(c)` also each
/// voxel one of its sub-sample points (spacing ≤ h/2) falls in. A voxel keeps the class of higher
/// `prio`; ties keep the first tet.
fn rasterise(
    mesh: &TetMesh,
    xf: &dyn Fn(Vec3) -> Vec3,
    grid: &Grid,
    class_of: &dyn Fn(u16) -> u8,
    conservative: &dyn Fn(u8) -> bool,
    prio: &[u8; 256],
    out: &mut [u8],
) {
    let pts: Vec<Vec3> = mesh.points.iter().map(|p| xf(*p)).collect();
    let claim = |idx: usize, c: u8, out: &mut [u8]| {
        let cur = out[idx];
        if cur == 0 || prio[c as usize] > prio[cur as usize] {
            out[idx] = c;
        }
    };
    for (t, tet) in mesh.tets.iter().enumerate() {
        let c = class_of(mesh.tags[t]);
        if c == 0 {
            continue;
        }
        let p = tet.map(|i| pts[i as usize]);
        let lo = p[0].min(p[1]).min(p[2]).min(p[3]);
        let hi = p[0].max(p[1]).max(p[2]).max(p[3]);
        if let Some(r) = grid.range(lo, hi) {
            let m = Mat3::from_cols(p[1] - p[0], p[2] - p[0], p[3] - p[0]);
            if m.determinant().abs() > 1e-12 {
                let inv = m.inverse();
                for k in r[2].0..=r[2].1 {
                    for j in r[1].0..=r[1].1 {
                        for i in r[0].0..=r[0].1 {
                            let l = inv * (grid.centre(i, j, k) - p[0]);
                            if l.min_element() >= -1e-5 && l.x + l.y + l.z <= 1.0 + 1e-5 {
                                claim(grid.idx(i, j, k), c, out);
                            }
                        }
                    }
                }
            }
        }
        if conservative(c) {
            let edge = [
                p[0].distance(p[1]),
                p[0].distance(p[2]),
                p[0].distance(p[3]),
                p[1].distance(p[2]),
                p[1].distance(p[3]),
                p[2].distance(p[3]),
            ]
            .into_iter()
            .fold(0.0f32, f32::max);
            let n = ((edge / (0.5 * grid.h)).ceil() as usize).clamp(1, 16);
            for a in 0..=n {
                for b in 0..=n - a {
                    for cc in 0..=n - a - b {
                        let d = n - a - b - cc;
                        let q = (p[0] * a as f32 + p[1] * b as f32 + p[2] * cc as f32 + p[3] * d as f32)
                            / n as f32;
                        if let Some([i, j, k]) = grid.voxel_of(q) {
                            claim(grid.idx(i, j, k), c, out);
                        }
                    }
                }
            }
        }
    }
}

/// Enclosed components smaller than this are pockets inside a wall, not a cavity (the Rodero
/// pulmonary-artery stub encloses only about 2 mL).
const MIN_CAVITY_VOXELS: usize = 300;
/// Length of the distal tube a cut end is fitted to, mm.
const END_RING_MM: f32 = 6.0;
/// Thickness of the plug that caps a cut vessel, mm (it shortens the lumen by about this much).
const CAP_PLUG_MM: f32 = 2.0;

/// Where the dataset crops a vessel wall: centre, unit outward normal, outer radius and half
/// thickness of the end annulus (heart frame).
#[derive(Debug, Clone, Copy)]
struct Crop {
    centre: Vec3,
    normal: Vec3,
    radius: f32,
    half_thickness: f32,
    area_mm2: f32,
}

/// The cut end of the wall `tag`: the free boundary faces (not on another tag) whose outward normal
/// points along the vessel (within 60° of the valve → wall-centroid direction) and that lie in the
/// distal half of the stub. The averaged SSM mesh's cut is not exactly planar, so the cap is a
/// plug as thick as the end faces' spread.
fn vessel_end(mesh: &TetMesh, xf: &dyn Fn(Vec3) -> Vec3, tag: u16, valve: Vec3) -> Option<Crop> {
    let mut other = vec![false; mesh.points.len()];
    let mut wall_c = DVec3::ZERO;
    let mut wall_n = 0usize;
    for (t, tet) in mesh.tets.iter().enumerate() {
        if mesh.tags[t] == tag {
            wall_c += xf(mesh.centroid(t)).as_dvec3();
            wall_n += 1;
        } else {
            for &n in tet {
                other[n as usize] = true;
            }
        }
    }
    let axis = ((wall_c / wall_n.max(1) as f64).as_vec3() - valve).normalize_or_zero();
    let idx_faces: Vec<[u32; 3]> = mesh
        .boundary_faces(|t| mesh.tags[t] == tag)
        .into_iter()
        .filter(|f| !f.iter().all(|n| other[*n as usize]))
        .collect();
    let faces: Vec<[Vec3; 3]> = idx_faces
        .iter()
        .map(|f| f.map(|n| xf(mesh.points[n as usize])))
        .collect();
    // the distal few millimetres of the tube: a short ring, whose smallest principal axis is the
    // vessel's direction at the cut (the averaged SSM's cut is neither planar nor sharp-edged);
    // refined a few times because the stub bends (the chord from the valve is not the end's axis)
    let verts: Vec<Vec3> = faces.iter().flat_map(|f| f.iter().copied()).collect();
    let mut dir = axis;
    let mut disc = None;
    for _ in 0..8 {
        let reach = verts
            .iter()
            .map(|v| (*v - valve).dot(dir))
            .fold(f32::NEG_INFINITY, f32::max);
        let pts: Vec<Vec3> = verts
            .iter()
            .copied()
            .filter(|v| (*v - valve).dot(dir) > reach - END_RING_MM)
            .collect();
        if pts.len() < 12 {
            return None;
        }
        let d = fit_disc(&pts, valve);
        dir = d.normal;
        disc = Some((d, pts));
    }
    let (disc, pts) = disc?;
    let (centre, normal) = (disc.centre, disc.normal);
    let (mut radius, mut far) = (0.0f32, f32::NEG_INFINITY);
    for v in &pts {
        let r = *v - centre;
        let along = r.dot(normal);
        far = far.max(along);
        radius = radius.max((r - normal * along).length());
    }
    let area = std::f32::consts::PI * radius * radius;
    let half = 0.5 * (CAP_PLUG_MM);
    // the plug sits at the very end: its centre `half` inside the most distal point
    let centre = centre + normal * (far - half);
    Some(Crop {
        centre,
        normal,
        radius,
        half_thickness: half,
        area_mm2: area,
    })
}

/// Component marks of the heart-box flood fill.
const OUT: u32 = u32::MAX;
const UNSEEN: u32 = u32::MAX - 1;

/// Heart-box classes.
mod hc {
    pub const LV: u8 = 1;
    pub const RV: u8 = 2;
    pub const LA: u8 = 3;
    pub const RA: u8 = 4;
    pub const AO: u8 = 5;
    pub const PA: u8 = 6;
    pub const MITRAL: u8 = 7;
    pub const TRICUSPID: u8 = 8;
    pub const AORTIC: u8 = 9;
    pub const PULMONARY: u8 = 10;
    pub const LID_LA: u8 = 11;
    pub const LID_RA: u8 = 12;
    pub const CAP_AO: u8 = 13;
    pub const CAP_PA: u8 = 14;
}

fn heart_class(tag: u16) -> u8 {
    match tag {
        1 => hc::LV,
        2 => hc::RV,
        3 | 18..=22 => hc::LA,
        4 | 23 | 24 => hc::RA,
        5 => hc::AO,
        6 => hc::PA,
        7 => hc::MITRAL,
        8 => hc::TRICUSPID,
        9 => hc::AORTIC,
        10 => hc::PULMONARY,
        11..=15 => hc::LID_LA,
        16 | 17 => hc::LID_RA,
        _ => 0,
    }
}

/// Bake both label volumes. `to_pack` maps Rodero file points into the heart frame, `cells` and
/// `aha` are the pack's cell cloud (for the wall-thickness check); `torso` is the KCL mesh and
/// `subject_centre`/`scale` its transform (as the skin's).
pub fn bake_volumes(
    heart_mesh: &TetMesh,
    to_pack: &dyn Fn(Vec3) -> Vec3,
    cells: &CellCloud,
    aha: &[u8],
    torso: &TetMesh,
    subject_centre: Vec3,
    scale: f32,
) -> Result<LabelVolumes> {
    let mut report = Vec::new();
    let heart = bake_heart_box(heart_mesh, to_pack, cells, aha, &mut report)?;
    let kcl_xf = move |p: Vec3| lps_to_heart((p - subject_centre) * scale);
    let thorax = bake_thorax_box(torso, &kcl_xf, scale, &heart, &mut report)?;
    Ok(LabelVolumes { heart, thorax })
}

#[allow(clippy::too_many_lines)]
fn bake_heart_box(
    mesh: &TetMesh,
    xf: &dyn Fn(Vec3) -> Vec3,
    cells: &CellCloud,
    aha: &[u8],
    report: &mut Vec<String>,
) -> Result<LabelVolume> {
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for p in &mesh.points {
        let q = xf(*p);
        lo = lo.min(q);
        hi = hi.max(q);
    }
    let grid = Grid::covering(lo - HEART_MARGIN_MM, hi + HEART_MARGIN_MM, HEART_VOXEL_MM);
    say(
        report,
        format!(
            "heart box: {}×{}×{} voxels of {} mm, origin ({:.0}, {:.0}, {:.0}) mm",
            grid.dims[0], grid.dims[1], grid.dims[2], grid.h, grid.origin.x, grid.origin.y, grid.origin.z
        ),
    );
    let mut cls = vec![0u8; grid.len()];
    let mut prio = [0u8; 256];
    for (c, p) in [
        (hc::LV, 1),
        (hc::RV, 1),
        (hc::LA, 2),
        (hc::RA, 2),
        (hc::AO, 2),
        (hc::PA, 2),
        (hc::MITRAL, 3),
        (hc::TRICUSPID, 3),
        (hc::AORTIC, 3),
        (hc::PULMONARY, 3),
        (hc::LID_LA, 3),
        (hc::LID_RA, 3),
        (hc::CAP_AO, 3),
        (hc::CAP_PA, 3),
    ] {
        prio[c as usize] = p;
    }
    // the ventricles by voxel centre (unbiased wall thickness and cavity volume), the thin parts
    // conservatively
    rasterise(mesh, xf, &grid, &heart_class, &|c| c >= hc::LA, &prio, &mut cls);

    // caps where the dataset crops the great arteries
    let disc_centre = |tag: u16| -> Vec3 {
        let nodes = mesh.nodes_of_tag(tag);
        nodes.iter().map(|n| xf(mesh.points[*n as usize])).sum::<Vec3>() / nodes.len().max(1) as f32
    };
    for (wall, valve, cap, name) in [
        (rt::AORTA, 9u16, hc::CAP_AO, "aorta"),
        (rt::PA, 10u16, hc::CAP_PA, "pulmonary artery"),
    ] {
        let Some(c) = vessel_end(mesh, xf, wall, disc_centre(valve)) else {
            bail!("{name}: no cut end found on the free boundary of its wall (tag {wall})");
        };
        if c.area_mm2 < 30.0 {
            bail!("{name}: the cut end found is only {:.0} mm²", c.area_mm2);
        }
        for c in [c] {
            say(
                report,
                format!(
                    "{name} cut end: centre ({:.1}, {:.1}, {:.1}) normal ({:.2}, {:.2}, {:.2}) radius {:.1} mm, spread ±{:.1} mm, {:.0} mm²: capped",
                    c.centre.x,
                    c.centre.y,
                    c.centre.z,
                    c.normal.x,
                    c.normal.y,
                    c.normal.z,
                    c.radius,
                    c.half_thickness,
                    c.area_mm2
                ),
            );
            let r = Vec3::splat(c.radius + grid.h);
            if let Some(rg) = grid.range(c.centre - r, c.centre + r) {
                for k in rg[2].0..=rg[2].1 {
                    for j in rg[1].0..=rg[1].1 {
                        for i in rg[0].0..=rg[0].1 {
                            let d = grid.centre(i, j, k) - c.centre;
                            let along = d.dot(c.normal);
                            let radial = (d - c.normal * along).length();
                            let idx = grid.idx(i, j, k);
                            if along.abs() <= c.half_thickness + 0.8 * grid.h
                                && radial <= c.radius + 0.5 * grid.h
                                && cls[idx] == 0
                            {
                                cls[idx] = cap;
                            }
                        }
                    }
                }
            }
        }
    }

    // flood fill of the outside (6-connected) from the box boundary
    let mut comp = vec![UNSEEN; grid.len()];
    let mut stack: Vec<usize> = Vec::new();
    let mut nb = Vec::with_capacity(6);
    for idx in 0..grid.len() {
        if cls[idx] == 0 && grid.on_boundary(idx) && comp[idx] == UNSEEN {
            comp[idx] = OUT;
            stack.push(idx);
            while let Some(v) = stack.pop() {
                grid.neighbours6(v, &mut nb);
                for &w in &nb {
                    if cls[w] == 0 && comp[w] == UNSEEN {
                        comp[w] = OUT;
                        stack.push(w);
                    }
                }
            }
        }
    }
    // enclosed components
    let mut comps: Vec<(usize, [usize; 15])> = Vec::new(); // size, neighbour class histogram
    for idx in 0..grid.len() {
        if cls[idx] != 0 || comp[idx] != UNSEEN {
            continue;
        }
        let id = comps.len() as u32;
        let mut size = 0usize;
        let mut hist = [0usize; 15];
        comp[idx] = id;
        stack.push(idx);
        while let Some(v) = stack.pop() {
            size += 1;
            grid.neighbours6(v, &mut nb);
            for &w in &nb {
                let c = cls[w];
                if c == 0 {
                    if comp[w] == UNSEEN {
                        comp[w] = id;
                        stack.push(w);
                    }
                } else {
                    hist[c as usize] += 1;
                }
            }
        }
        comps.push((size, hist));
    }
    let chamber_label = [
        (hc::LV, Label::LvBlood, Label::LvMyocardium, "LV"),
        (hc::RV, Label::RvBlood, Label::RvMyocardium, "RV"),
        (hc::LA, Label::LaBlood, Label::LaMyocardium, "LA"),
        (hc::RA, Label::RaBlood, Label::RaMyocardium, "RA"),
        (hc::AO, Label::AortaLumen, Label::AortaWall, "aorta"),
        (
            hc::PA,
            Label::PulmonaryArteryLumen,
            Label::PulmonaryArteryWall,
            "pulmonary artery",
        ),
    ];
    // a cap counts as its vessel's wall when naming a component
    let wall_votes = |h: &[usize; 15]| -> [usize; 7] {
        let mut v = [0usize; 7];
        v[1..=6].copy_from_slice(&h[1..=6]);
        v[hc::AO as usize] += h[hc::CAP_AO as usize];
        v[hc::PA as usize] += h[hc::CAP_PA as usize];
        v
    };
    let mut comp_label: Vec<Label> = Vec::with_capacity(comps.len());
    let mut big_found = [0usize; 7];
    let mut small = 0usize;
    for (size, hist) in &comps {
        let v = wall_votes(hist);
        let best = (1..=6).max_by_key(|&c| (v[c], std::cmp::Reverse(c))).unwrap_or(1);
        let total: usize = v.iter().sum();
        let (_, blood, wall, name) = chamber_label[best - 1];
        if *size >= MIN_CAVITY_VOXELS {
            let share = v[best] as f64 / total.max(1) as f64;
            let second =
                (1..=6).filter(|&c| c != best).map(|c| v[c]).max().unwrap_or(0) as f64 / total.max(1) as f64;
            say(
                report,
                format!(
                    "cavity of {size} voxels ({:.1} mL): {name} ({:.0} % of its wall contacts, next {:.0} %)",
                    *size as f64 * f64::from(grid.h).powi(3) / 1000.0,
                    share * 100.0,
                    second * 100.0
                ),
            );
            if second > 0.35 {
                bail!(
                    "a cavity of {size} voxels touches two chambers' walls about equally ({name} {:.0} %, next {:.0} %): the fill leaked through a wall or valve",
                    share * 100.0,
                    second * 100.0
                );
            }
            big_found[best] += 1;
            comp_label.push(blood);
        } else {
            // pockets inside a wall become that wall
            small += size;
            comp_label.push(wall);
        }
    }
    for (c, _, _, name) in chamber_label {
        if big_found[c as usize] == 0 {
            bail!(
                "no enclosed {name} cavity: the flood fill leaked out of it (missing cap or a hole in its wall)"
            );
        }
    }
    say(
        report,
        format!("{small} voxels of small wall pockets filled with their wall"),
    );

    // labels
    let mut labels = vec![Label::Outside as u8; grid.len()];
    for idx in 0..grid.len() {
        let c = cls[idx];
        labels[idx] = match c {
            0 => {
                if comp[idx] == OUT {
                    Label::Outside as u8
                } else {
                    comp_label[comp[idx] as usize] as u8
                }
            }
            hc::LV => Label::LvMyocardium as u8,
            hc::RV => Label::RvMyocardium as u8,
            hc::LA | hc::LID_LA => Label::LaMyocardium as u8,
            hc::RA | hc::LID_RA => Label::RaMyocardium as u8,
            hc::AO | hc::CAP_AO => Label::AortaWall as u8,
            hc::PA | hc::CAP_PA => Label::PulmonaryArteryWall as u8,
            _ => 255, // valve discs: resolved below
        };
    }
    // valve-disc voxels take the blood beside them (the leaflets are drawn on top at run time);
    // grown in passes from the blood, each pass reading the previous one; the rest is wall
    let disc_side = |c: u8| -> [Label; 2] {
        match c {
            hc::MITRAL => [Label::LvBlood, Label::LaBlood],
            hc::TRICUSPID => [Label::RvBlood, Label::RaBlood],
            hc::AORTIC => [Label::AortaLumen, Label::LvBlood],
            _ => [Label::PulmonaryArteryLumen, Label::RvBlood],
        }
    };
    for _pass in 0..8 {
        let prev = labels.clone();
        let mut changed = 0usize;
        for idx in 0..grid.len() {
            if prev[idx] != 255 {
                continue;
            }
            let sides = disc_side(cls[idx]);
            grid.neighbours6(idx, &mut nb);
            let mut votes = [0usize; 2];
            for &w in &nb {
                for (s, l) in sides.iter().enumerate() {
                    if prev[w] == *l as u8 {
                        votes[s] += 1;
                    }
                }
            }
            if votes[0] + votes[1] > 0 {
                labels[idx] = if votes[0] >= votes[1] { sides[0] } else { sides[1] } as u8;
                changed += 1;
            }
        }
        if changed == 0 {
            break;
        }
    }
    let mut disc_wall = 0usize;
    for idx in 0..grid.len() {
        if labels[idx] == 255 {
            disc_wall += 1;
            labels[idx] = match cls[idx] {
                hc::MITRAL | hc::AORTIC => Label::LvMyocardium,
                _ => Label::RvMyocardium,
            } as u8;
        }
    }
    say(
        report,
        format!("valve discs: {disc_wall} voxels not reached from blood became ventricular wall"),
    );
    let vol = LabelVolume {
        origin_mm: grid.origin,
        spacing_mm: grid.h,
        dims: grid.dims.map(|d| d as u32),
        labels,
    };
    let ml = |l: Label| vol.count(l) as f64 * vol.voxel_volume_mm3() / 1000.0;
    say(
        report,
        format!(
            "heart box volumes (mL): blood LV {:.1} RV {:.1} LA {:.1} RA {:.1} aorta {:.1} PA {:.1}; myocardium LV {:.1} RV {:.1} LA {:.1} RA {:.1}",
            ml(Label::LvBlood),
            ml(Label::RvBlood),
            ml(Label::LaBlood),
            ml(Label::RaBlood),
            ml(Label::AortaLumen),
            ml(Label::PulmonaryArteryLumen),
            ml(Label::LvMyocardium),
            ml(Label::RvMyocardium),
            ml(Label::LaMyocardium),
            ml(Label::RaMyocardium)
        ),
    );
    // no blood voxel may touch the outside
    let mut leaks = 0usize;
    for idx in 0..grid.len() {
        if Label::from_u8(vol.labels[idx]).is_some_and(Label::is_blood) {
            grid.neighbours6(idx, &mut nb);
            if nb.iter().any(|&w| vol.labels[w] == Label::Outside as u8) {
                leaks += 1;
            }
        }
    }
    if leaks > 0 {
        bail!("{leaks} blood voxels touch the outside: the fill leaked");
    }
    lv_wall_thickness(&vol, cells, aha, report)?;
    Ok(vol)
}

/// LV wall thickness per AHA level, measured on the volume through the mid-wall LV cells along the
/// short-axis radial direction (blood to outside or to RV blood); medians per level must lie in
/// [`LV_WALL_RANGE_MM`].
fn lv_wall_thickness(
    vol: &LabelVolume,
    cells: &CellCloud,
    aha: &[u8],
    report: &mut Vec<String>,
) -> Result<()> {
    // LV axis: principal direction of the LV blood, oriented apex → base by the apex landmark
    let blood: Vec<Vec3> = (0..vol.len())
        .filter(|&i| vol.labels[i] == Label::LvBlood as u8)
        .map(|i| {
            let [a, b, c] = vol.coords(i);
            vol.centre(a, b, c)
        })
        .collect();
    let c: Vec3 = blood.iter().sum::<Vec3>() / blood.len().max(1) as f32;
    let mut cov = [[0.0f64; 3]; 3];
    for p in &blood {
        let d = (*p - c).as_dvec3();
        for (a, row) in cov.iter_mut().enumerate() {
            for (b, x) in row.iter_mut().enumerate() {
                *x += d[a] * d[b];
            }
        }
    }
    // power iteration
    let mut axis = DVec3::new(0.3, 0.6, 0.7);
    for _ in 0..100 {
        let a = DVec3::new(
            cov[0][0] * axis.x + cov[0][1] * axis.y + cov[0][2] * axis.z,
            cov[1][0] * axis.x + cov[1][1] * axis.y + cov[1][2] * axis.z,
            cov[2][0] * axis.x + cov[2][1] * axis.y + cov[2][2] * axis.z,
        );
        axis = a.normalize();
    }
    let mut axis = axis.as_vec3();
    if let Some(&apex) = cells.landmarks.get("lv_apex")
        && (cells.positions[apex as usize] - c).dot(axis) > 0.0
    {
        axis = -axis; // apex → base
    }
    let lv = |p: Vec3| vol.get(p) == Some(Label::LvMyocardium as u8);
    let mut levels: [Vec<f32>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (i, p) in cells.positions.iter().enumerate() {
        let seg = aha.get(i).copied().unwrap_or(0);
        let level = match seg {
            1..=6 => 0,
            7..=12 => 1,
            13..=16 => 2,
            _ => continue,
        };
        if !matches!(
            cells.regions[i],
            Region::LeftVentricle | Region::VentricularSeptum
        ) || !(0.35..=0.65).contains(&cells.depth[i])
            || !lv(*p)
        {
            continue;
        }
        let r = (*p - c) - axis * (*p - c).dot(axis);
        if r.length() < 5.0 {
            continue;
        }
        let dir = r.normalize();
        let step = 0.25f32;
        let walk = |sign: f32| -> (f32, Option<u8>) {
            let mut s = 0.0;
            while s < 30.0 {
                s += step;
                let q = *p + dir * (sign * s);
                match vol.get(q) {
                    Some(l) if l == Label::LvMyocardium as u8 => {}
                    other => return (s, other),
                }
            }
            (s, None)
        };
        let (a, inner) = walk(-1.0);
        let (b, _) = walk(1.0);
        if inner == Some(Label::LvBlood as u8) {
            levels[level].push(a + b - step);
        }
    }
    let mut medians = [0.0f32; 3];
    for (k, v) in levels.iter_mut().enumerate() {
        v.sort_by(f32::total_cmp);
        medians[k] = if v.is_empty() { f32::NAN } else { v[v.len() / 2] };
    }
    say(
        report,
        format!(
            "LV wall thickness, median along the short-axis radius through mid-wall cells: basal {:.1} mm (n {}), mid {:.1} mm (n {}), apical {:.1} mm (n {})",
            medians[0],
            levels[0].len(),
            medians[1],
            levels[1].len(),
            medians[2],
            levels[2].len()
        ),
    );
    for (m, name) in medians.iter().zip(["basal", "mid", "apical"]) {
        if !(LV_WALL_RANGE_MM.0..=LV_WALL_RANGE_MM.1).contains(m) {
            bail!(
                "LV wall thickness at the {name} level is {m:.1} mm, outside {}–{} mm",
                LV_WALL_RANGE_MM.0,
                LV_WALL_RANGE_MM.1
            );
        }
    }
    Ok(())
}

/// Volume (mL), centroid and vertical extent of one KCL tag in the heart frame.
#[derive(Debug, Clone, Copy, Default)]
struct TagStats {
    ml: f64,
    centroid: Vec3,
    y_lo: f32,
    y_hi: f32,
}

fn tag_stats(mesh: &TetMesh, xf: &dyn Fn(Vec3) -> Vec3) -> BTreeMap<u16, TagStats> {
    let mut acc: BTreeMap<u16, (f64, DVec3, f32, f32)> = BTreeMap::new();
    for (t, tet) in mesh.tets.iter().enumerate() {
        let p = tet.map(|i| xf(mesh.points[i as usize]));
        let v = f64::from(((p[1] - p[0]).cross(p[2] - p[0]).dot(p[3] - p[0])).abs() / 6.0);
        let c = (p[0] + p[1] + p[2] + p[3]) * 0.25;
        let e = acc
            .entry(mesh.tags[t])
            .or_insert((0.0, DVec3::ZERO, f32::INFINITY, f32::NEG_INFINITY));
        e.0 += v;
        e.1 += c.as_dvec3() * v;
        e.2 = e.2.min(c.y);
        e.3 = e.3.max(c.y);
    }
    acc.into_iter()
        .map(|(t, (v, c, lo, hi))| {
            (
                t,
                TagStats {
                    ml: v / 1000.0,
                    centroid: (c / v.max(1e-9)).as_vec3(),
                    y_lo: lo,
                    y_hi: hi,
                },
            )
        })
        .collect()
}

/// Check every KCL tag the thorax bake uses against stated expectations; fail loudly otherwise.
/// `midline_x` is the skin's sagittal midline at heart level.
fn verify_kcl_tags(s: &BTreeMap<u16, TagStats>, midline_x: f32, report: &mut Vec<String>) -> Result<()> {
    let get = |t: u16| s.get(&t).copied().unwrap_or_default();
    let mut fails: Vec<String> = Vec::new();
    let mut check = |ok: bool, what: String| {
        if ok {
            say(report, format!("KCL tag check ok: {what}"));
        } else {
            fails.push(what);
        }
    };
    let bone = get(kcl::BONE);
    check(
        (0.4..=3.0).contains(&(bone.ml / 1000.0)) && bone.y_hi - bone.y_lo > 250.0 && bone.centroid.z < -20.0,
        format!(
            "bone {}: {:.0} mL (0.4–3 L), spans {:.0} mm vertically (> 250), centroid z {:.0} mm (posterior: the spine)",
            kcl::BONE,
            bone.ml,
            bone.y_hi - bone.y_lo,
            bone.centroid.z
        ),
    );
    let lung = get(kcl::LUNGS);
    check(
        (1.5..=7.0).contains(&(lung.ml / 1000.0)) && lung.centroid.y > 0.0,
        format!(
            "lungs {}: {:.0} mL (1.5–7 L), centroid y {:.0} mm (above the ventricular centroid)",
            kcl::LUNGS,
            lung.ml,
            lung.centroid.y
        ),
    );
    let liver = get(kcl::LIVER);
    check(
        (0.8..=2.5).contains(&(liver.ml / 1000.0))
            && liver.centroid.x < midline_x
            && liver.centroid.y < -20.0,
        format!(
            "liver {}: {:.0} mL (0.8–2.5 L), centroid {:.0} mm right of the midline, y {:.0} mm (below the heart)",
            kcl::LIVER,
            liver.ml,
            midline_x - liver.centroid.x,
            liver.centroid.y
        ),
    );
    for (wall, pool, name) in [
        (kcl::AORTA_WALL, kcl::AORTA_POOL, "aorta"),
        (kcl::PA_WALL, kcl::PA_POOL, "pulmonary artery"),
        (kcl::SVC_WALL, kcl::SVC_POOL, "SVC"),
        (kcl::LA_WALL, kcl::LA_POOL, "LA"),
        (kcl::RA_WALL, kcl::RA_POOL, "RA"),
    ] {
        let (w, p) = (get(wall), get(pool));
        check(
            p.ml > w.ml && p.centroid.distance(w.centroid) < 15.0,
            format!(
                "{name}: pool {pool} {:.1} mL > wall {wall} {:.1} mL, centroids {:.0} mm apart (< 15)",
                p.ml,
                w.ml,
                p.centroid.distance(w.centroid)
            ),
        );
    }
    let ao = get(kcl::AORTA_POOL);
    check(
        ao.y_hi - ao.y_lo > 250.0,
        format!(
            "aorta pool {} spans {:.0} mm vertically (ascending, arch, descending: > 250)",
            kcl::AORTA_POOL,
            ao.y_hi - ao.y_lo
        ),
    );
    let svc = get(kcl::SVC_POOL);
    check(
        svc.centroid.x < midline_x && svc.centroid.y > 20.0,
        format!(
            "SVC pool {}: {:.0} mm right of the midline, y {:.0} mm (above the heart)",
            kcl::SVC_POOL,
            midline_x - svc.centroid.x,
            svc.centroid.y
        ),
    );
    let pa = get(kcl::PA_POOL);
    check(
        pa.centroid.y > 20.0,
        format!(
            "PA pool {}: y {:.0} mm (above the ventricles)",
            kcl::PA_POOL,
            pa.centroid.y
        ),
    );
    for (pool, wall, name) in [
        (kcl::LV_POOL, kcl::LV_WALL, "LV"),
        (kcl::RV_POOL, kcl::RV_WALL, "RV"),
    ] {
        let (p, w) = (get(pool), get(wall));
        check(
            p.centroid.distance(w.centroid) < 30.0 && p.centroid.length() < 60.0,
            format!(
                "{name} pool {pool}: {:.0} mL, {:.0} mm from its wall {wall}, {:.0} mm from the ventricular centroid",
                p.ml,
                p.centroid.distance(w.centroid),
                p.centroid.length()
            ),
        );
    }
    let la = get(kcl::LA_POOL);
    let ra = get(kcl::RA_POOL);
    check(
        la.centroid.z < get(kcl::RV_POOL).centroid.z && ra.centroid.x < la.centroid.x,
        format!(
            "LA pool {} posterior to the RV pool, RA pool {} right of the LA pool",
            kcl::LA_POOL,
            kcl::RA_POOL
        ),
    );
    if !fails.is_empty() {
        bail!(
            "KCL tag verification failed (the tag table is not what the bake expects):\n  {}",
            fails.join("\n  ")
        );
    }
    Ok(())
}

/// Thorax classes.
mod tc {
    pub const SOFT: u8 = 1;
    pub const KCL_HEART: u8 = 2;
    pub const BLOOD: u8 = 3;
    pub const LIVER: u8 = 4;
    pub const LUNG: u8 = 5;
    pub const BONE: u8 = 6;
}

#[allow(clippy::too_many_lines)]
fn bake_thorax_box(
    mesh: &TetMesh,
    xf: &dyn Fn(Vec3) -> Vec3,
    scale: f32,
    heart: &LabelVolume,
    report: &mut Vec<String>,
) -> Result<LabelVolume> {
    // skin extent and midline at heart level
    let skin_nodes = mesh.nodes_of_tag(kcl::SKIN);
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let (mut mx0, mut mx1) = (f32::INFINITY, f32::NEG_INFINITY);
    for n in &skin_nodes {
        let p = xf(mesh.points[*n as usize]);
        lo = lo.min(p);
        hi = hi.max(p);
        if p.y.abs() < 15.0 {
            mx0 = mx0.min(p.x);
            mx1 = mx1.max(p.x);
        }
    }
    let midline_x = f32::midpoint(mx0, mx1);
    let stats = tag_stats(mesh, xf);
    for (t, s) in &stats {
        say(
            report,
            format!(
                "KCL tag {t:>2}: {:>7.1} mL, centroid ({:>5.0}, {:>5.0}, {:>5.0}), y {:>5.0}…{:<5.0} mm (heart frame, torso scaled ×{scale:.3})",
                s.ml, s.centroid.x, s.centroid.y, s.centroid.z, s.y_lo, s.y_hi
            ),
        );
    }
    verify_kcl_tags(&stats, midline_x, report)?;

    let grid = Grid::covering(lo - THORAX_VOXEL_MM, hi + THORAX_VOXEL_MM, THORAX_VOXEL_MM);
    say(
        report,
        format!(
            "thorax box: {}×{}×{} voxels of {} mm, origin ({:.0}, {:.0}, {:.0}) mm, {:.0}×{:.0}×{:.0} mm; skin midline at heart level x = {midline_x:.0} mm",
            grid.dims[0],
            grid.dims[1],
            grid.dims[2],
            grid.h,
            grid.origin.x,
            grid.origin.y,
            grid.origin.z,
            grid.dims[0] as f32 * grid.h,
            grid.dims[1] as f32 * grid.h,
            grid.dims[2] as f32 * grid.h
        ),
    );
    let class_of = |tag: u16| -> u8 {
        match tag {
            kcl::BONE => tc::BONE,
            kcl::LUNGS => tc::LUNG,
            kcl::LIVER => tc::LIVER,
            kcl::AORTA_POOL | kcl::PA_POOL | kcl::SVC_POOL => tc::BLOOD,
            t if kcl::HEART.contains(&t) => tc::KCL_HEART,
            // inner body, skin, organs, vessel walls outside the heart, the ICD hardware (26–33)
            _ => tc::SOFT,
        }
    };
    let mut prio = [0u8; 256];
    for (c, p) in [
        (tc::SOFT, 1),
        (tc::KCL_HEART, 2),
        (tc::BLOOD, 3),
        (tc::LIVER, 4),
        (tc::LUNG, 5),
        (tc::BONE, 6),
    ] {
        prio[c as usize] = p;
    }
    let mut cls = vec![0u8; grid.len()];
    rasterise(mesh, xf, &grid, &class_of, &|_| false, &prio, &mut cls);
    let _ = kcl::INNER_BODY;

    // the pack heart on this grid: any heart-box heart voxel within the dilation of the centre
    let is_pack_heart = |p: Vec3| -> bool {
        let r = PACK_HEART_DILATION_MM + 0.5 * THORAX_VOXEL_MM;
        let steps = (r / heart.spacing_mm).ceil() as i32;
        for dz in -steps..=steps {
            for dy in -steps..=steps {
                for dx in -steps..=steps {
                    let q = p + Vec3::new(dx as f32, dy as f32, dz as f32) * heart.spacing_mm;
                    if q.distance(p) > r {
                        continue;
                    }
                    if heart.get(q).is_some_and(|l| l != Label::Outside as u8) {
                        return true;
                    }
                }
            }
        }
        false
    };
    let mut pack = vec![false; grid.len()];
    let (hlo, hhi) = (heart.origin_mm, heart.max_mm());
    let mut exact_pack = vec![false; grid.len()];
    for idx in 0..grid.len() {
        let [i, j, k] = grid.ijk(idx);
        let p = grid.centre(i, j, k);
        if p.cmplt(hlo - 4.0).any() || p.cmpgt(hhi + 4.0).any() {
            continue;
        }
        exact_pack[idx] = heart.get(p).is_some_and(|l| l != Label::Outside as u8);
        pack[idx] = exact_pack[idx] || is_pack_heart(p);
    }
    // the gap between the two hearts' outlines (before the overrides)
    let kcl_heart: Vec<bool> = cls.iter().map(|c| *c == tc::KCL_HEART).collect();
    let d_to_pack = distance_field(&grid, &exact_pack);
    let d_to_kcl = distance_field(&grid, &kcl_heart);
    let worst = |mask: &[bool], d: &[f32]| -> f32 {
        mask.iter()
            .zip(d)
            .filter(|(m, _)| **m)
            .map(|(_, d)| *d)
            .fold(0.0f32, f32::max)
    };
    let kcl_beyond = worst(&kcl_heart, &d_to_pack);
    let pack_beyond = worst(&exact_pack, &d_to_kcl);
    let overlap_lung = (0..grid.len()).filter(|&i| pack[i] && cls[i] == tc::LUNG).count();
    say(
        report,
        format!(
            "heart outlines: the KCL subject's heart reaches {kcl_beyond:.0} mm beyond the pack heart, the pack heart {pack_beyond:.0} mm beyond the KCL heart (worst gap {:.0} mm); {:.1} mL of KCL lung inside the dilated pack heart became soft tissue",
            kcl_beyond.max(pack_beyond),
            overlap_lung as f64 * f64::from(grid.h).powi(3) / 1000.0
        ),
    );

    let mut labels = vec![Label::Outside as u8; grid.len()];
    for idx in 0..grid.len() {
        labels[idx] = if pack[idx] && cls[idx] != 0 && cls[idx] != tc::BONE {
            Label::SoftTissue
        } else {
            match cls[idx] {
                0 => Label::Outside,
                tc::BONE => Label::Bone,
                tc::LUNG => Label::Lung,
                tc::LIVER => Label::Liver,
                tc::BLOOD => Label::ThoracicBlood,
                _ => Label::SoftTissue,
            }
        } as u8;
    }
    let vol = LabelVolume {
        origin_mm: grid.origin,
        spacing_mm: grid.h,
        dims: grid.dims.map(|d| d as u32),
        labels,
    };
    // lungs left and right of the midline
    let (mut l_left, mut l_right) = (0usize, 0usize);
    for idx in 0..vol.len() {
        if vol.labels[idx] == Label::Lung as u8 {
            let [i, j, k] = vol.coords(idx);
            if vol.centre(i, j, k).x > midline_x {
                l_left += 1;
            } else {
                l_right += 1;
            }
        }
    }
    let v = vol.voxel_volume_mm3() / 1e6;
    let ml = |l: Label| vol.count(l) as f64 * vol.voxel_volume_mm3() / 1000.0;
    say(
        report,
        format!(
            "thorax box volumes: bone {:.0} mL, lung {:.2} L (left {:.2} L, right {:.2} L), liver {:.0} mL, thoracic blood {:.0} mL, soft tissue {:.2} L",
            ml(Label::Bone),
            ml(Label::Lung) / 1000.0,
            l_left as f64 * v,
            l_right as f64 * v,
            ml(Label::Liver),
            ml(Label::ThoracicBlood),
            ml(Label::SoftTissue) / 1000.0
        ),
    );
    for (n, side) in [(l_left, "left"), (l_right, "right")] {
        let litres = n as f64 * v;
        if !(0.8..=3.5).contains(&litres) {
            bail!("{side} lung {litres:.2} L outside 0.8–3.5 L");
        }
    }
    Ok(vol)
}

/// Distance (mm) from every voxel to the nearest `true` voxel: two-pass chamfer with weights 1,
/// √2, √3 (times the spacing): exact along axes and diagonals, up to about 12 % over in between.
fn distance_field(grid: &Grid, mask: &[bool]) -> Vec<f32> {
    let [nx, ny, nz] = grid.dims.map(|d| isize::try_from(d).unwrap_or(isize::MAX));
    let mut d: Vec<f32> = mask
        .iter()
        .map(|m| if *m { 0.0 } else { f32::INFINITY })
        .collect();
    let w = |dx: isize, dy: isize, dz: isize| ((dx * dx + dy * dy + dz * dz) as f32).sqrt() * grid.h;
    let fwd: Vec<(isize, isize, isize, f32)> = {
        let mut v = Vec::new();
        for dz in -1..=1isize {
            for dy in -1..=1isize {
                for dx in -1..=1isize {
                    let before = dz < 0 || (dz == 0 && (dy < 0 || (dy == 0 && dx < 0)));
                    if before {
                        v.push((dx, dy, dz, w(dx, dy, dz)));
                    }
                }
            }
        }
        v
    };
    let at = |i: isize, j: isize, k: isize| (i + nx * (j + ny * k)) as usize;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let mut best = d[at(i, j, k)];
                for &(dx, dy, dz, ww) in &fwd {
                    let (a, b, c) = (i + dx, j + dy, k + dz);
                    if a >= 0 && b >= 0 && c >= 0 && a < nx && b < ny && c < nz {
                        best = best.min(d[at(a, b, c)] + ww);
                    }
                }
                d[at(i, j, k)] = best;
            }
        }
    }
    for k in (0..nz).rev() {
        for j in (0..ny).rev() {
            for i in (0..nx).rev() {
                let mut best = d[at(i, j, k)];
                for &(dx, dy, dz, ww) in &fwd {
                    let (a, b, c) = (i - dx, j - dy, k - dz);
                    if a >= 0 && b >= 0 && c >= 0 && a < nx && b < ny && c < nz {
                        best = best.min(d[at(a, b, c)] + ww);
                    }
                }
                d[at(i, j, k)] = best;
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chamfer_distance_is_close_to_euclidean() {
        let g = Grid {
            origin: Vec3::ZERO,
            h: 2.0,
            dims: [21, 21, 21],
        };
        let mut m = vec![false; g.len()];
        m[g.idx(10, 10, 10)] = true;
        let d = distance_field(&g, &m);
        assert_eq!(d[g.idx(10, 10, 10)], 0.0);
        assert!((d[g.idx(15, 10, 10)] - 10.0).abs() < 1e-4);
        let e = g.centre(17, 13, 11).distance(g.centre(10, 10, 10));
        // a 3-4-5-style chamfer overestimates off-axis distances by up to about 12 %
        let rel = (d[g.idx(17, 13, 11)] - e) / e;
        assert!((0.0..0.13).contains(&rel), "{rel}");
    }
}
