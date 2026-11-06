//! EP-lab catheters (brief B22): polyline bodies with electrodes, placed on landmarks or dragged
//! along a vessel, and kept inside the chambers.
//!
//! **Electrodes.** Spacings are those of common diagnostic catheters: quadripolar 2-5-2 mm for the
//! high right atrium and the RV apex, hexapolar 2-5-2-5-2 mm for the His (three bipoles, "His 1–3"
//! of F-16), decapolar for the coronary sinus. A real CS decapolar is usually 2-8-2 mm over ≈ 50 mm;
//! the pack's coronary sinus is only ≈ 34 mm long, so 2-6-2 mm is used to keep all ten electrodes in
//! the vessel. By clinical convention electrode 1 is the most distal one, so `CS 1-2` is distal and
//! `CS 9-10` sits at the ostium.
//!
//! **Chamber interiors.** A chamber's cavity is computed from the cell cloud alone, so it works on
//! any anatomy pack: a voxel belongs to the cavity if it holds no myocardium and at least five of
//! the six axis rays from it hit the chamber's own tissue (five, not six, because the ventricles
//! are open at the base). That also handles the crescent-shaped RV, which no centroid-based test
//! could. A tip may go [`CONTACT_MM`] past the last cavity voxel so it can touch the endocardium.

use crate::cloud::CellCloud;
use glam::Vec3;
use heartsim_core::Region;
use serde::{Deserialize, Serialize};

/// Cavity voxel edge, mm (just over the 2.3 mm cell spacing of the default pack, so that a wall
/// has no holes at this resolution).
const CAVITY_VOXEL_MM: f32 = 3.0;
/// How far a catheter tip may reach beyond the last cavity voxel, mm: electrode against the
/// endocardium.
pub const CONTACT_MM: f32 = 4.0;
/// Rays of the six axis directions that must hit the chamber wall for a voxel to count as cavity.
const MIN_WALL_HITS: usize = 5;
/// How far below the lowest right-atrial cavity point the body starts, mm (femoral approach: a
/// purely visual stretch of the inferior vena cava).
const ENTRY_STRETCH_MM: f32 = 60.0;

/// The diagnostic and ablation catheters of a standard EP study.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CatheterKind {
    /// High right atrium, quadripolar: atrial activation near the sinus node.
    Hra,
    /// His-bundle catheter across the tricuspid valve, hexapolar: atrial, His and ventricular
    /// deflections on one electrogram (AH and HV intervals).
    His,
    /// Coronary-sinus decapolar: left-atrial and left-ventricular activation along the mitral
    /// annulus (the sequence that localises a left-sided accessory pathway).
    Cs,
    /// Right-ventricular apex, quadripolar.
    Rva,
    /// Ablation catheter: tip and three rings; can be steered anywhere.
    Abl,
}

impl CatheterKind {
    /// Every catheter, in the top-to-bottom order of an EP-lab display.
    pub const ALL: [CatheterKind; 5] = [
        CatheterKind::Hra,
        CatheterKind::His,
        CatheterKind::Cs,
        CatheterKind::Rva,
        CatheterKind::Abl,
    ];

    /// Label stem of the electrograms. Clinical abbreviations, identical in every language.
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            CatheterKind::Hra => "HRA",
            CatheterKind::His => "His",
            CatheterKind::Cs => "CS",
            CatheterKind::Rva => "RVA",
            CatheterKind::Abl => "ABL",
        }
    }

    /// Electrode positions along the shaft, mm from the tip.
    #[must_use]
    pub fn electrode_offsets_mm(self) -> &'static [f32] {
        match self {
            CatheterKind::Hra | CatheterKind::Rva => &[0.0, 2.0, 7.0, 9.0],
            CatheterKind::His => &[0.0, 2.0, 7.0, 9.0, 14.0, 16.0],
            CatheterKind::Cs => &[0.0, 2.0, 8.0, 10.0, 16.0, 18.0, 24.0, 26.0, 32.0, 34.0],
            // 4 mm tip: the tip electrode's centre sits 2 mm behind the end of the catheter
            CatheterKind::Abl => &[0.0, 4.0, 9.0, 11.0],
        }
    }

    /// Display colour of the shaft (linear RGB).
    #[must_use]
    pub fn colour(self) -> [f32; 3] {
        match self {
            CatheterKind::Hra => [0.95, 0.75, 0.2],
            CatheterKind::His => [0.3, 0.85, 0.35],
            CatheterKind::Cs => [0.3, 0.6, 1.0],
            CatheterKind::Rva => [0.9, 0.35, 0.9],
            CatheterKind::Abl => [1.0, 0.25, 0.2],
        }
    }
}

/// A heart chamber a catheter can sit in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardiacChamber {
    RightAtrium,
    LeftAtrium,
    RightVentricle,
    LeftVentricle,
}

impl CardiacChamber {
    const ALL: [CardiacChamber; 4] = [
        CardiacChamber::RightAtrium,
        CardiacChamber::LeftAtrium,
        CardiacChamber::RightVentricle,
        CardiacChamber::LeftVentricle,
    ];

    /// The wall of this chamber. The septum belongs to both ventricles.
    fn wall(self, r: Region) -> bool {
        match self {
            CardiacChamber::RightAtrium => matches!(
                r,
                Region::RightAtrium
                    | Region::CristaTerminalis
                    | Region::CavotricuspidIsthmus
                    | Region::AtrialSeptum
                    | Region::BachmannBundle
            ),
            CardiacChamber::LeftAtrium => matches!(
                r,
                Region::LeftAtrium
                    | Region::LeftAtrialAppendage
                    | Region::PulmonaryVeinSleeves
                    | Region::AtrialSeptum
                    | Region::BachmannBundle
            ),
            CardiacChamber::RightVentricle => {
                matches!(r, Region::RightVentricle | Region::VentricularSeptum)
            }
            CardiacChamber::LeftVentricle => matches!(r, Region::LeftVentricle | Region::VentricularSeptum),
        }
    }

    /// The chamber a tip touching `region` is in; conduction-system cells on the right side of the
    /// septum are reached from the right atrium.
    #[must_use]
    pub fn of_region(region: Region) -> Self {
        match region {
            Region::LeftAtrium | Region::LeftAtrialAppendage | Region::PulmonaryVeinSleeves => {
                CardiacChamber::LeftAtrium
            }
            Region::RightVentricle | Region::RightBundleBranch | Region::PurkinjeRight => {
                CardiacChamber::RightVentricle
            }
            Region::LeftVentricle
            | Region::VentricularSeptum
            | Region::LeftBundleBranch
            | Region::LeftAnteriorFascicle
            | Region::LeftPosteriorFascicle
            | Region::PurkinjeLeft => CardiacChamber::LeftVentricle,
            _ => CardiacChamber::RightAtrium,
        }
    }
}

/// The inside of a chamber as a voxel mask (see the module documentation).
#[derive(Debug, Clone)]
pub struct Cavity {
    lo: Vec3,
    dims: [usize; 3],
    inside: Vec<bool>,
    /// Centres of the cavity voxels, ascending voxel index (nearest-point queries).
    points: Vec<Vec3>,
    /// A cavity point near the middle of the chamber (waypoint of the catheter bodies).
    centre: Vec3,
}

impl Cavity {
    /// Cavity of `chamber` on `cloud`.
    #[must_use]
    pub fn build(cloud: &CellCloud, chamber: CardiacChamber) -> Self {
        let h = CAVITY_VOXEL_MM;
        let wall: Vec<Vec3> = cloud
            .positions
            .iter()
            .zip(&cloud.regions)
            .filter(|(_, r)| chamber.wall(**r))
            .map(|(p, _)| *p)
            .collect();
        let (lo, hi) = wall.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(a, b), p| (a.min(*p), b.max(*p)),
        );
        if wall.is_empty() {
            return Self {
                lo: Vec3::ZERO,
                dims: [0; 3],
                inside: Vec::new(),
                points: Vec::new(),
                centre: Vec3::ZERO,
            };
        }
        let lo = lo - Vec3::splat(h);
        let n = ((hi - lo) / h).ceil() + Vec3::ONE;
        let dims = [n.x as usize, n.y as usize, n.z as usize];
        let at = |i: usize, j: usize, k: usize| i + dims[0] * (j + dims[1] * k);
        let total = dims[0] * dims[1] * dims[2];
        let voxel_of = |p: Vec3| -> Option<usize> {
            let q = (p - lo) / h;
            if q.min_element() < 0.0 {
                return None;
            }
            let (i, j, k) = (q.x as usize, q.y as usize, q.z as usize);
            (i < dims[0] && j < dims[1] && k < dims[2]).then(|| at(i, j, k))
        };
        let mut own = vec![false; total];
        for p in &wall {
            if let Some(v) = voxel_of(*p) {
                own[v] = true;
            }
        }
        // any myocardium blocks a cavity voxel, whichever chamber it belongs to
        let mut solid = vec![false; total];
        for (p, r) in cloud.positions.iter().zip(&cloud.regions) {
            if (r.is_atrial() || r.is_ventricular())
                && let Some(v) = voxel_of(*p)
            {
                solid[v] = true;
            }
        }
        let mut inside = vec![false; total];
        let mut points = Vec::new();
        for k in 0..dims[2] {
            for j in 0..dims[1] {
                for i in 0..dims[0] {
                    let v = at(i, j, k);
                    if solid[v] {
                        continue;
                    }
                    let hits = [
                        (0..i).any(|a| own[at(a, j, k)]),
                        (i + 1..dims[0]).any(|a| own[at(a, j, k)]),
                        (0..j).any(|a| own[at(i, a, k)]),
                        (j + 1..dims[1]).any(|a| own[at(i, a, k)]),
                        (0..k).any(|a| own[at(i, j, a)]),
                        (k + 1..dims[2]).any(|a| own[at(i, j, a)]),
                    ];
                    if hits.iter().filter(|h| **h).count() >= MIN_WALL_HITS {
                        inside[v] = true;
                        points.push(lo + (Vec3::new(i as f32, j as f32, k as f32) + Vec3::splat(0.5)) * h);
                    }
                }
            }
        }
        let mean = points.iter().copied().sum::<Vec3>() / points.len().max(1) as f32;
        let centre = nearest_of(&points, mean).unwrap_or(mean);
        Self {
            lo,
            dims,
            inside,
            points,
            centre,
        }
    }

    /// `p` lies in a cavity voxel.
    #[must_use]
    pub fn contains(&self, p: Vec3) -> bool {
        let q = (p - self.lo) / CAVITY_VOXEL_MM;
        if q.min_element() < 0.0 {
            return false;
        }
        let (i, j, k) = (q.x as usize, q.y as usize, q.z as usize);
        i < self.dims[0]
            && j < self.dims[1]
            && k < self.dims[2]
            && self.inside[i + self.dims[0] * (j + self.dims[1] * k)]
    }

    /// Bring `p` inside: unchanged if it is in the cavity, otherwise on the way from the nearest
    /// cavity voxel towards `p`, at most [`CONTACT_MM`] past that voxel (the wall of the chamber).
    #[must_use]
    pub fn constrain(&self, p: Vec3) -> Vec3 {
        if self.contains(p) {
            return p;
        }
        match nearest_of(&self.points, p) {
            Some(q) => q + (p - q).clamp_length_max(CONTACT_MM),
            None => p,
        }
    }

    /// Distance from `p` to the cavity, mm (0 inside).
    #[must_use]
    pub fn distance(&self, p: Vec3) -> f32 {
        if self.contains(p) {
            return 0.0;
        }
        nearest_of(&self.points, p).map_or(f32::INFINITY, |q| q.distance(p))
    }

    /// A cavity point near the middle of the chamber.
    #[must_use]
    pub fn centre(&self) -> Vec3 {
        self.centre
    }

    /// Number of cavity voxels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// The pack gave no cavity for this chamber.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

fn nearest_of(points: &[Vec3], p: Vec3) -> Option<Vec3> {
    points
        .iter()
        .copied()
        .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
}

/// Where a catheter is put.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CatheterSite {
    /// The usual position of this kind of catheter.
    Default,
    /// A named landmark of the anatomy pack.
    Landmark(String),
    /// A point in heart coordinates (mm), e.g. a pick on the heart or a drag target.
    Point([f32; 3]),
    /// Along the coronary sinus: distance from the ostium to the tip, mm.
    CsDepth(f32),
}

/// One catheter in place. Everything is in heart coordinates (mm).
#[derive(Debug, Clone, PartialEq)]
pub struct Catheter {
    pub kind: CatheterKind,
    pub site: CatheterSite,
    /// Shaft from the entry point to the tip.
    pub body: Vec<Vec3>,
    /// Electrode centres; index 0 is electrode 1, the most distal.
    pub electrodes: Vec<Vec3>,
}

impl Catheter {
    /// Number of bipoles: electrodes pair up from the tip (1-2, 3-4, …).
    #[must_use]
    pub fn pair_count(&self) -> usize {
        self.electrodes.len() / 2
    }

    /// Electrode indices of bipole `k` (0 = distal).
    #[must_use]
    pub fn pair(&self, k: usize) -> (usize, usize) {
        (2 * k, 2 * k + 1)
    }

    /// Midpoint of bipole `k`: where the pair records from and where it stimulates.
    #[must_use]
    pub fn pair_midpoint(&self, k: usize) -> Vec3 {
        let (a, b) = self.pair(k);
        (self.electrodes[a] + self.electrodes[b]) * 0.5
    }

    /// Clinical name of bipole `k`, e.g. `CS 1-2` (distal), `His 3-4`. Not translated.
    #[must_use]
    pub fn pair_label(&self, k: usize) -> String {
        format!("{} {}-{}", self.kind.prefix(), 2 * k + 1, 2 * k + 2)
    }

    /// The tip, mm.
    #[must_use]
    pub fn tip(&self) -> Vec3 {
        self.electrodes.first().copied().unwrap_or(Vec3::ZERO)
    }
}

/// Everything needed to place catheters on one anatomy pack.
#[derive(Debug, Clone)]
pub struct CatheterAnatomy {
    cavities: [Cavity; 4],
    /// Coronary-sinus centre line from the ostium to the distal end.
    cs_rail: Vec<Vec3>,
    /// Where the bodies start (below the heart: femoral venous access).
    entry: Vec3,
    landmarks: std::collections::BTreeMap<String, Vec3>,
    positions: Vec<Vec3>,
    regions: Vec<Region>,
}

impl CatheterAnatomy {
    /// Derive the cavities and the coronary-sinus rail from the cell cloud.
    #[must_use]
    pub fn from_cloud(cloud: &CellCloud) -> Self {
        let cavities = CardiacChamber::ALL.map(|c| Cavity::build(cloud, c));
        let mut cs_rail: Vec<Vec3> = cloud
            .positions
            .iter()
            .zip(&cloud.regions)
            .filter(|(_, r)| **r == Region::CoronarySinus)
            .map(|(p, _)| *p)
            .collect();
        // Packs without explicit sinus cells: a straight rail between the landmarks.
        if cs_rail.len() < 2
            && let (Some(&a), Some(&b)) = (cloud.landmarks.get("cs_os"), cloud.landmarks.get("cs_distal"))
        {
            cs_rail = vec![cloud.positions[a as usize], cloud.positions[b as usize]];
        }
        let ra = &cavities[0];
        let lowest = ra
            .points
            .iter()
            .copied()
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .unwrap_or(Vec3::ZERO);
        let entry = lowest - Vec3::Y * ENTRY_STRETCH_MM;
        Self {
            cavities,
            cs_rail,
            entry,
            landmarks: cloud
                .landmarks
                .iter()
                .map(|(k, v)| (k.clone(), cloud.positions[*v as usize]))
                .collect(),
            positions: cloud.positions.clone(),
            regions: cloud.regions.clone(),
        }
    }

    /// The cavity of `chamber`.
    #[must_use]
    pub fn cavity(&self, chamber: CardiacChamber) -> &Cavity {
        match chamber {
            CardiacChamber::RightAtrium => &self.cavities[0],
            CardiacChamber::LeftAtrium => &self.cavities[1],
            CardiacChamber::RightVentricle => &self.cavities[2],
            CardiacChamber::LeftVentricle => &self.cavities[3],
        }
    }

    /// Length of the coronary-sinus rail, mm.
    #[must_use]
    pub fn cs_length_mm(&self) -> f32 {
        self.cs_rail.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    /// Depth along the coronary sinus of the rail point nearest to `p`, mm (dragging the
    /// catheter along the vessel).
    #[must_use]
    pub fn cs_depth_of(&self, p: Vec3) -> f32 {
        let mut s = 0.0f32;
        let mut best = (f32::INFINITY, 0.0f32);
        for w in self.cs_rail.windows(2) {
            let seg = w[1] - w[0];
            let len = seg.length().max(1e-6);
            let t = ((p - w[0]).dot(seg) / (len * len)).clamp(0.0, 1.0);
            let d = (w[0] + seg * t).distance_squared(p);
            if d < best.0 {
                best = (d, s + t * len);
            }
            s += len;
        }
        best.1
    }

    fn nearest_region(&self, p: Vec3) -> Region {
        let mut best = (f32::INFINITY, Region::RightAtrium);
        for (q, r) in self.positions.iter().zip(&self.regions) {
            let d = q.distance_squared(p);
            if d < best.0 && (r.is_atrial() || r.is_ventricular() || r.is_conduction_system()) {
                best = (d, *r);
            }
        }
        best.1
    }

    fn landmark_or(&self, name: &str, fallback: Vec3) -> Vec3 {
        self.landmarks.get(name).copied().unwrap_or(fallback)
    }

    /// Put a catheter of `kind` at `site`. Non-CS tips are constrained to the interior of their
    /// chamber; the CS catheter stays on its rail.
    #[must_use]
    pub fn place(&self, kind: CatheterKind, site: &CatheterSite) -> Catheter {
        if kind == CatheterKind::Cs {
            return self.place_cs(site);
        }
        let ra_centre = self.cavity(CardiacChamber::RightAtrium).centre();
        let target = match site {
            CatheterSite::Default => match kind {
                CatheterKind::Hra => self.landmark_or("sa_node", ra_centre),
                CatheterKind::His => self.landmark_or("his", ra_centre),
                CatheterKind::Rva => self.landmark_or("rv_apex", ra_centre),
                _ => self.landmark_or("ra_lateral", ra_centre),
            },
            CatheterSite::Landmark(name) => self.landmark_or(name, ra_centre),
            CatheterSite::Point(p) => Vec3::from(*p),
            CatheterSite::CsDepth(d) => self.rail_point(*d).0,
        };
        let chamber = match kind {
            CatheterKind::Hra | CatheterKind::His => CardiacChamber::RightAtrium,
            CatheterKind::Rva => CardiacChamber::RightVentricle,
            _ => CardiacChamber::of_region(self.nearest_region(target)),
        };
        let cavity = self.cavity(chamber);
        let tip = if cavity.is_empty() {
            target
        } else {
            cavity.constrain(target)
        };
        let mut waypoints = vec![self.entry, ra_centre];
        match chamber {
            CardiacChamber::RightAtrium => {}
            CardiacChamber::RightVentricle => {
                waypoints.push(cavity.centre());
            }
            CardiacChamber::LeftAtrium => {
                waypoints.push(cavity.centre());
            }
            CardiacChamber::LeftVentricle => {
                waypoints.push(self.cavity(CardiacChamber::LeftAtrium).centre());
                waypoints.push(cavity.centre());
            }
        }
        waypoints.push(tip);
        // the straight distal part must carry the electrodes: only the approach is rounded
        let last = waypoints.len() - 1;
        let approach = chaikin(&waypoints[..last], 2);
        let mut body = approach;
        body.push(tip);
        let electrodes = lay_electrodes(&body, kind.electrode_offsets_mm());
        Catheter {
            kind,
            site: site.clone(),
            body,
            electrodes,
        }
    }

    /// Rail point at `depth_mm` from the ostium, and the rail vertices up to it.
    fn rail_point(&self, depth_mm: f32) -> (Vec3, Vec<Vec3>) {
        let mut out = Vec::new();
        let Some(&first) = self.cs_rail.first() else {
            return (Vec3::ZERO, out);
        };
        out.push(first);
        let mut s = 0.0f32;
        for w in self.cs_rail.windows(2) {
            let len = w[0].distance(w[1]);
            if s + len >= depth_mm {
                let p = w[0].lerp(w[1], ((depth_mm - s) / len.max(1e-6)).clamp(0.0, 1.0));
                out.push(p);
                return (p, out);
            }
            s += len;
            out.push(w[1]);
        }
        (*out.last().unwrap_or(&first), out)
    }

    fn place_cs(&self, site: &CatheterSite) -> Catheter {
        let full = self.cs_length_mm();
        let depth = match site {
            CatheterSite::CsDepth(d) => d.clamp(0.0, full),
            CatheterSite::Point(p) => self.cs_depth_of(Vec3::from(*p)),
            CatheterSite::Landmark(name) => self
                .landmarks
                .get(name)
                .map_or(full, |p| self.cs_depth_of(*p)),
            CatheterSite::Default => full,
        };
        let ra_centre = self.cavity(CardiacChamber::RightAtrium).centre();
        let (_, rail) = self.rail_point(depth);
        let mut body = chaikin(&[self.entry, ra_centre, rail[0]], 2);
        body.extend_from_slice(&rail[1..]);
        let electrodes = lay_electrodes(&body, CatheterKind::Cs.electrode_offsets_mm());
        Catheter {
            kind: CatheterKind::Cs,
            site: CatheterSite::CsDepth(depth),
            body,
            electrodes,
        }
    }

    /// The five catheters of a standard study at their usual sites.
    #[must_use]
    pub fn standard_set(&self) -> Vec<Catheter> {
        CatheterKind::ALL
            .iter()
            .map(|k| self.place(*k, &CatheterSite::Default))
            .collect()
    }
}

/// Corner cutting; the end points stay.
fn chaikin(points: &[Vec3], rounds: usize) -> Vec<Vec3> {
    let mut p = points.to_vec();
    for _ in 0..rounds {
        if p.len() < 3 {
            return p;
        }
        let mut q = Vec::with_capacity(2 * p.len());
        q.push(p[0]);
        for w in p.windows(2) {
            q.push(w[0].lerp(w[1], 0.25));
            q.push(w[0].lerp(w[1], 0.75));
        }
        q.push(p[p.len() - 1]);
        p = q;
    }
    p
}

/// Points at `offsets` (mm back from the end of the polyline).
fn lay_electrodes(body: &[Vec3], offsets: &[f32]) -> Vec<Vec3> {
    offsets.iter().map(|&s| point_back_from_tip(body, s)).collect()
}

fn point_back_from_tip(body: &[Vec3], back_mm: f32) -> Vec3 {
    let mut left = back_mm;
    for w in body.windows(2).rev() {
        let len = w[0].distance(w[1]);
        if left <= len {
            return w[1].lerp(w[0], left / len.max(1e-6));
        }
        left -= len;
    }
    body.first().copied().unwrap_or(Vec3::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn electrodes_are_laid_back_from_the_tip_along_the_shaft() {
        let body = [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 0.0)];
        let e = lay_electrodes(&body, &[0.0, 2.0, 12.0]);
        assert_eq!(e[0], Vec3::new(10.0, 10.0, 0.0));
        assert!(e[1].distance(Vec3::new(10.0, 8.0, 0.0)) < 1e-5);
        assert!(e[2].distance(Vec3::new(8.0, 0.0, 0.0)) < 1e-5);
    }

    #[test]
    fn chaikin_keeps_the_end_points() {
        let p = [Vec3::ZERO, Vec3::X, Vec3::ONE];
        let q = chaikin(&p, 2);
        assert_eq!(q[0], p[0]);
        assert_eq!(q[q.len() - 1], p[2]);
        assert!(q.len() > 3);
    }

    #[test]
    fn a_sphere_shell_has_its_inside_as_cavity() {
        // a thin shell labelled right atrium, radius 20 mm
        let mut c = CellCloud::test_sphere(2.0, 22.0, Region::RightAtrium);
        let keep: Vec<usize> = (0..c.len()).filter(|&i| c.positions[i].length() > 18.5).collect();
        c.positions = keep.iter().map(|&i| c.positions[i]).collect();
        c.regions = vec![Region::RightAtrium; keep.len()];
        let cavity = Cavity::build(&c, CardiacChamber::RightAtrium);
        assert!(cavity.contains(Vec3::ZERO));
        assert!(!cavity.contains(Vec3::new(30.0, 0.0, 0.0)));
        let out = cavity.constrain(Vec3::new(60.0, 0.0, 0.0));
        assert!(out.length() < 24.0, "pulled back to the wall: {out}");
        assert_eq!(cavity.constrain(Vec3::new(1.0, 1.0, 1.0)), Vec3::new(1.0, 1.0, 1.0));
    }
}
