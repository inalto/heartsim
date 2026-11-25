//! The standard transthoracic views (brief B33): for each view a target built from the heart's own
//! landmarks, a geometric "view found" score, and a reference probe pose found by a small
//! deterministic optimisation from the window's starting pose.
//!
//! **Sources.** What each view must show, where the probe goes, the index-marker direction and the
//! manoeuvre from the previous view are those of Mitchell et al., *Guidelines for Performing a
//! Comprehensive Transthoracic Echocardiographic Examination in Adults*, J Am Soc Echocardiogr
//! 2019;32:1-64, doi:10.1016/j.echo.2018.06.004. The primary PDF
//! (asecho.org/wp-content/uploads/2019/01/2019_Comprehensive-TTE.pdf) was read for this work; its
//! PDF page numbers equal the journal's, and each [`ViewId::citation`] names the pages. The guideline
//! gives no angle for the index marker of the parasternal short-axis, RV inflow and RV outflow views
//! beyond the words quoted there; their marker rules and the tolerances are **designed**
//! ([`ViewTolerances`]) and have not been reviewed by a sonographer.
//!
//! **Targets are computed, not typed.** Every key point comes from the anatomy: the LV and RV apex
//! landmarks, the four valve-disc centres, the chamber centres, the LV long axis (apex to mitral
//! centre), the designed papillary bodies and moderator band, the aortic arch found in the thorax
//! label volume, and the baked ribs and lungs (for the window quality). A different pack gives
//! different, still valid, targets.
//!
//! **The score** is `exp(-cost)`, a product of Gaussians (`cost` is a sum of squared, normalised
//! errors, so the optimiser has a gradient far from the view): the distance of each key landmark from
//! the image plane; each inside the sector and depth; each "must be absent" landmark absent; the
//! marker direction; the plane's orientation to the LV long axis (short-axis views); the
//! foreshortening of the apical views; and the window quality (the share of lines to the key
//! landmarks not hidden by bone or lung). It is a pure function of the anatomy at rest and the pose:
//! the beat and the breath do not enter. The window quality uses the **expiration** anatomy (the
//! lung edge at full retraction), so a target never depends on the breath.
//!
//! **Not reachable is a result.** A view whose best pose within the contact limit does not reach the
//! found threshold, or whose structure the label volumes lack, is reported as such
//! ([`Reach::NotReachable`]) and not faked.

use crate::anatomy::{EchoAnatomy, EchoWindow};
use crate::coords::MotionChamber;
use crate::model::MotionModel;
use crate::probe::{DEFAULT_DEPTH_MM, DEFAULT_SECTOR_DEG, DEPTH_RANGE_MM, ProbeFrame, ProbePose};
use crate::slice::LabelImage;
use glam::Vec3;
use heartsim_ecg::Valve;
use heartsim_geometry::{AnatomyPack, Label, LabelVolume};
use std::collections::BinaryHeap;
use std::sync::OnceLock;

// ---- tolerances -----------------------------------------------------------------------------

/// Every tolerance of the score, the hints and the reference search, in one place. **All designed**
/// (no published tolerance exists for an echo-view score); listed in the user docs for a sonographer
/// to review.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewTolerances {
    /// Gaussian width of a key landmark's distance from the image plane, mm.
    pub plane_sigma_mm: f32,
    /// Gaussian width of how far outside the sector's edge a key landmark lies, degrees.
    pub sector_sigma_deg: f32,
    /// A key landmark counts as inside the sector only this far in from its edge, degrees.
    pub sector_margin_deg: f32,
    /// Gaussian width of how far beyond the imaging depth a key landmark lies, mm.
    pub depth_sigma_mm: f32,
    /// A key landmark must lie this far inside the imaging depth, mm.
    pub depth_margin_mm: f32,
    /// Gaussian width of the "must be absent" landmarks' distance from the plane, mm: a landmark this
    /// close to the plane (and inside the sector) is present.
    pub absent_sigma_mm: f32,
    /// Gaussian width of the angle between the index marker and its target direction (in the image
    /// plane), degrees.
    pub marker_sigma_deg: f32,
    /// The same for views whose marker the guideline describes only in words ([`ViewTarget::marker_loose`]).
    pub marker_loose_sigma_deg: f32,
    /// Short-axis views: Gaussian width of the angle between the image-plane normal and the LV long
    /// axis, degrees. It only excludes long-axis cuts; the roundness of the LV section is judged by
    /// [`Self::circularity_min`].
    pub normal_sigma_deg: f32,
    /// ... and costs nothing up to this angle: only a cut more oblique than this (towards a long-axis
    /// cut) is penalised.
    pub normal_free_deg: f32,
    /// Short-axis levels of the LV: the section of the LV blood in the image plane must be at least
    /// this round (ratio of the minor to the major axis of its extent, 1 = a circle). Clinically the
    /// cut is accepted when the ventricle looks circular, not when the plane is exactly perpendicular
    /// to a computed axis (Mitchell p26: "the ventricle should appear circular").
    pub circularity_min: f32,
    /// Gaussian width of the shortfall below [`Self::circularity_min`] (ratio units).
    pub circularity_sigma: f32,
    /// Foreshortening, apical views: Gaussian width of the true apex's distance from the plane, mm.
    pub foreshortening_sigma_mm: f32,
    /// Foreshortening, apical views: Gaussian width of the angle between the beam and the LV long
    /// axis, degrees.
    pub axis_sigma_deg: f32,
    /// Window quality: Gaussian width of the bone and lung (mm along the line to a key landmark) that
    /// hide it.
    pub occlusion_sigma_mm: f32,
    /// A view is found when the score is at least this.
    pub found_threshold: f32,
    /// ... and has stayed there this long, s.
    pub found_hold_s: f32,
    /// A checklist structure is ticked when at least this share of the image's samples show it
    /// (and at least 8 samples; thin structures a sixteenth of that).
    pub checklist_min_fraction: f32,
    /// Hint: a slide error beyond this is the movement to make first, mm.
    pub hint_slide_mm: f32,
    /// Hint: rotation error that counts as an error, degrees.
    pub hint_rotate_deg: f32,
    /// Hint: tilt error, degrees.
    pub hint_tilt_deg: f32,
    /// Hint: rock error, degrees.
    pub hint_rock_deg: f32,
    /// Hint: depth error, mm.
    pub hint_depth_mm: f32,
    /// Hint: bone along the central line before the heart that is "over a rib", mm.
    pub rib_bone_mm: f32,
    /// Hint: lung along the central line before the heart that is "lung in the way", mm.
    pub lung_mm: f32,
    /// Hint: an index marker this far from its target is on the wrong side, degrees.
    pub wrong_marker_deg: f32,
    /// Hint: the apical view is foreshortened when its foreshortening term is below this and the rest
    /// of the geometry is fine.
    pub foreshortened_below: f32,
    /// Reference search: spacing of the slide seeds around the window's starting point, mm.
    pub seed_step_mm: f32,
    /// A reference pose lies at most this far (mm, straight line) from its window's starting skin
    /// point: the window is a place on the chest, not the whole trunk.
    pub max_slide_mm: f32,
    /// The parasternal window lies at least this far to the patient's left of the sternal midline
    /// (mm): the left sternal border, beside the sternum (Mitchell p23), not over it.
    pub parasternal_left_margin_mm: f32,
}

impl Default for ViewTolerances {
    fn default() -> Self {
        Self {
            plane_sigma_mm: 8.0,
            sector_sigma_deg: 5.0,
            sector_margin_deg: 4.0,
            depth_sigma_mm: 10.0,
            depth_margin_mm: 10.0,
            absent_sigma_mm: 12.0,
            marker_sigma_deg: 15.0,
            marker_loose_sigma_deg: 25.0,
            normal_sigma_deg: 15.0,
            normal_free_deg: 35.0,
            circularity_min: 0.85,
            circularity_sigma: 0.1,
            foreshortening_sigma_mm: 6.0,
            axis_sigma_deg: 12.0,
            occlusion_sigma_mm: 4.0,
            found_threshold: 0.8,
            found_hold_s: 1.0,
            checklist_min_fraction: 0.0004,
            hint_slide_mm: 6.0,
            hint_rotate_deg: 8.0,
            hint_tilt_deg: 4.0,
            hint_rock_deg: 4.0,
            hint_depth_mm: 20.0,
            rib_bone_mm: 4.0,
            lung_mm: 6.0,
            wrong_marker_deg: 110.0,
            foreshortened_below: 0.7,
            seed_step_mm: 12.0,
            max_slide_mm: 60.0,
            parasternal_left_margin_mm: 15.0,
        }
    }
}

// ---- the views --------------------------------------------------------------------------------

/// The standard views, in the order the guideline teaches them (parasternal, apical, subcostal,
/// suprasternal), as in the contents of Mitchell et al. 2019, section IV: the RV outflow view
/// before the RV inflow view (IV.A.2, IV.A.3; Table 2 rows 2.5, 2.6), and the RV-focused view right
/// after the apical four-chamber (IV.C.1, IV.C.2). The string ids are stable (saved progress, tutorials, the CLI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ViewId {
    ParasternalLongAxis,
    RvOutflow,
    RvInflow,
    ShortAxisAorticValve,
    ShortAxisMitralValve,
    ShortAxisPapillary,
    ShortAxisApex,
    ApicalFourChamber,
    RvFocusedFourChamber,
    ApicalFiveChamber,
    ApicalTwoChamber,
    ApicalThreeChamber,
    SubcostalFourChamber,
    SubcostalIvc,
    SuprasternalArch,
}

/// Where a guideline statement was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Citation {
    /// Pages of Mitchell et al. 2019 (journal pages; the PDF's equal them).
    pub pages: &'static str,
    /// Whether the primary document was read for these pages.
    pub verified: bool,
}

impl ViewId {
    /// Every view, in teaching order.
    pub const ALL: [Self; 15] = [
        Self::ParasternalLongAxis,
        Self::RvOutflow,
        Self::RvInflow,
        Self::ShortAxisAorticValve,
        Self::ShortAxisMitralValve,
        Self::ShortAxisPapillary,
        Self::ShortAxisApex,
        Self::ApicalFourChamber,
        Self::RvFocusedFourChamber,
        Self::ApicalFiveChamber,
        Self::ApicalTwoChamber,
        Self::ApicalThreeChamber,
        Self::SubcostalFourChamber,
        Self::SubcostalIvc,
        Self::SuprasternalArch,
    ];

    /// The stable string id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::ParasternalLongAxis => "plax",
            Self::RvInflow => "rv_inflow",
            Self::RvOutflow => "rv_outflow",
            Self::ShortAxisAorticValve => "psax_av",
            Self::ShortAxisMitralValve => "psax_mv",
            Self::ShortAxisPapillary => "psax_papillary",
            Self::ShortAxisApex => "psax_apex",
            Self::ApicalFourChamber => "a4c",
            Self::ApicalFiveChamber => "a5c",
            Self::ApicalTwoChamber => "a2c",
            Self::ApicalThreeChamber => "a3c",
            Self::RvFocusedFourChamber => "a4c_rv",
            Self::SubcostalFourChamber => "sc_4c",
            Self::SubcostalIvc => "sc_ivc",
            Self::SuprasternalArch => "ssn_arch",
        }
    }

    /// The view with string id `id`.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.id() == id)
    }

    /// The window the view is taken from.
    #[must_use]
    pub const fn window(self) -> EchoWindow {
        match self {
            Self::ParasternalLongAxis
            | Self::RvInflow
            | Self::RvOutflow
            | Self::ShortAxisAorticValve
            | Self::ShortAxisMitralValve
            | Self::ShortAxisPapillary
            | Self::ShortAxisApex => EchoWindow::Parasternal,
            Self::ApicalFourChamber
            | Self::ApicalFiveChamber
            | Self::ApicalTwoChamber
            | Self::ApicalThreeChamber
            | Self::RvFocusedFourChamber => EchoWindow::Apical,
            Self::SubcostalFourChamber | Self::SubcostalIvc => EchoWindow::Subcostal,
            Self::SuprasternalArch => EchoWindow::Suprasternal,
        }
    }

    /// Where in Mitchell et al. 2019 the view is described (probe position, index marker,
    /// manoeuvre, structures). Every entry was read on the primary document.
    #[must_use]
    pub const fn citation(self) -> Citation {
        let pages = match self {
            Self::ParasternalLongAxis => "23-25",
            Self::RvOutflow | Self::RvInflow => "25",
            Self::ShortAxisAorticValve
            | Self::ShortAxisMitralValve
            | Self::ShortAxisPapillary
            | Self::ShortAxisApex => "25-26",
            Self::ApicalFourChamber | Self::RvFocusedFourChamber | Self::ApicalFiveChamber => "26",
            Self::ApicalTwoChamber | Self::ApicalThreeChamber => "23, 30",
            Self::SubcostalFourChamber | Self::SubcostalIvc => "24, 31",
            Self::SuprasternalArch => "25, 31",
        };
        Citation {
            pages,
            verified: true,
        }
    }
}

/// A named point of the anatomy a view is built from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landmark {
    /// Stable key (the interface names it `echo-lm-<key>`).
    pub key: &'static str,
    /// Heart frame, mm, at rest.
    pub pos_mm: Vec3,
}

/// Whether the model can show a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// The anatomy has what the view needs.
    Reachable,
    /// It does not (interface key of the reason: `echo-unreachable-<key>`).
    NotReachable(&'static str),
}

/// What a checklist line looks for in the label slice (or in the pose).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemTest {
    /// The label is in the image (not hidden) in at least the designed share of its samples.
    Label(Label),
    /// Leaflets of this valve (a leaflet sample whose rest position lies within the valve's reach).
    Valve(Valve),
    /// Papillary muscle samples of the anterolateral (0) or posteromedial (1) body.
    Papillary(u8),
    /// The landmark (index into [`ViewTarget::marks`]) lies inside the sector.
    InSector(usize),
    /// The landmark lies outside the sector (the apex in the long-axis view).
    NotInSector(usize),
    /// The label is not in the image (fewer than the minimum share of samples).
    LabelAbsent(Label),
}

/// One line of a view's "what it must show" list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChecklistItem {
    /// Interface key `echo-chk-<key>`.
    pub key: &'static str,
    pub test: ItemTest,
}

/// The target of one view.
#[derive(Debug, Clone)]
pub struct ViewTarget {
    pub id: ViewId,
    pub window: EchoWindow,
    pub reach: Reach,
    /// Must lie in the plane and inside the sector.
    pub in_plane: Vec<Landmark>,
    /// Must be absent (outside the sector or off the plane).
    pub absent: Vec<Landmark>,
    /// Target direction of the index marker (any 3-D vector; its in-plane part is compared).
    pub marker_dir: Option<Vec3>,
    /// The guideline gives the marker of this view only in words (no clock position or landmark
    /// pair), so its target direction is looser ([`ViewTolerances::marker_loose_sigma_deg`]).
    pub marker_loose: bool,
    /// The LV long axis, when the plane must be perpendicular to it (short-axis views).
    pub normal_axis: Option<Vec3>,
    /// Short-axis LV levels: the level's landmark, around which the roundness of the LV section is
    /// measured on the rest labels in the image plane.
    pub circular_at: Option<Vec3>,
    /// Apical views: the LV apex and the LV long axis (foreshortening).
    pub apical: Option<(Vec3, Vec3)>,
    /// Where the optimiser aims the beam from the start (landmark or midpoint).
    pub aim_mm: Vec3,
    /// Structures drawn on the image and referenced by [`ItemTest::InSector`].
    pub marks: Vec<Landmark>,
    pub checklist: Vec<ChecklistItem>,
}

// ---- the anatomy's landmarks ---------------------------------------------------------------------

/// The key points of the anatomy at rest, from which every target is built.
#[derive(Debug, Clone)]
pub struct Landmarks {
    pub apex: Vec3,
    pub lv_axis: Vec3,
    pub lv_mid: Vec3,
    pub mitral: Vec3,
    pub tricuspid: Vec3,
    pub aortic: Vec3,
    pub pulmonic: Vec3,
    pub la: Vec3,
    pub ra: Vec3,
    pub rv_apex: Vec3,
    pub lv_blood: Vec3,
    pub rv_blood: Vec3,
    pub pa: Vec3,
    pub ao_asc: Vec3,
    pub pap: [Vec3; 2],
    pub moderator: Vec3,
    /// The aortic arch (ascending end, top, descending end), from the thorax label volume; `None`
    /// when the thorax volume has no blood connecting to the pack's aorta.
    pub arch: Option<[Vec3; 3]>,
    /// Length of the inferior vena cava the labels show below the right atrium, mm.
    pub ivc_length_mm: f32,
    /// A point on the IVC and its direction (toward the right atrium), when the pack has the vein.
    pub ivc: Option<(Vec3, Vec3)>,
}

fn centroid(vol: &LabelVolume, label: Label, keep: impl Fn(Vec3) -> bool) -> Option<Vec3> {
    let b = label as u8;
    let mut s = glam::DVec3::ZERO;
    let mut n = 0usize;
    for (idx, l) in vol.labels.iter().enumerate() {
        if *l == b {
            let [i, j, k] = vol.coords(idx);
            let c = vol.centre(i, j, k);
            if keep(c) {
                s += c.as_dvec3();
                n += 1;
            }
        }
    }
    (n > 0).then(|| (s / n as f64).as_vec3())
}

impl Landmarks {
    /// The landmarks of `pack`'s anatomy. `None` when a needed structure is missing.
    #[must_use]
    pub fn new(pack: &AnatomyPack, model: &MotionModel, anatomy: &EchoAnatomy) -> Option<Self> {
        let frame = model.frame();
        let heart = anatomy.heart_volume();
        let lv = frame.axis(MotionChamber::LeftVentricle);
        let apex = lv.origin_mm;
        let mitral = frame.valve_centres_mm[Valve::Mitral.index()];
        let aortic = frame.valve_centres_mm[Valve::Aortic.index()];
        let aortic_normal = anatomy
            .valves()
            .iter()
            .find(|v| v.valve == Valve::Aortic)
            .map_or(Vec3::Y, |v| v.normal);
        let ao_asc = centroid(heart, Label::AortaLumen, |c| {
            let h = (c - aortic).dot(aortic_normal);
            (15.0..30.0).contains(&h)
        })
        .unwrap_or(aortic + aortic_normal * 22.0);
        let pap = anatomy.papillary_muscles();
        let mid = |c: &crate::anatomy::Capsule| (c.a_mm + c.b_mm) * 0.5;
        let arch = find_arch(anatomy.thorax_volume(), heart, aortic_normal, aortic);
        let (ivc, ivc_length_mm) = ivc_of(pack, anatomy);
        Some(Self {
            apex,
            lv_axis: (mitral - apex).normalize_or(Vec3::Y),
            lv_mid: apex + (mitral - apex) * 0.5,
            mitral,
            tricuspid: frame.valve_centres_mm[Valve::Tricuspid.index()],
            aortic,
            pulmonic: frame.valve_centres_mm[Valve::Pulmonic.index()],
            la: frame.axis(MotionChamber::LeftAtrium).centre_mm,
            ra: frame.axis(MotionChamber::RightAtrium).centre_mm,
            rv_apex: frame.axis(MotionChamber::RightVentricle).origin_mm,
            lv_blood: centroid(heart, Label::LvBlood, |_| true)?,
            rv_blood: centroid(heart, Label::RvBlood, |_| true)?,
            pa: centroid(heart, Label::PulmonaryArteryLumen, |_| true)?,
            ao_asc,
            pap: [mid(&pap[0]), mid(&pap[1])],
            moderator: mid(anatomy.moderator_band()),
            arch,
            ivc_length_mm,
            ivc,
        })
    }

    /// A point on the LV long axis at `height` (0 apex … 1 mitral centre).
    #[must_use]
    pub fn axis_point(&self, height: f32) -> Vec3 {
        self.apex + (self.mitral - self.apex) * height
    }
}

/// The IVC of the pack and how long a blood-filled vein the thorax labels show along it below the
/// right atrium (the pack's vein is a stub; the thorax volume holds none in the abdomen here).
fn ivc_of(pack: &AnatomyPack, anatomy: &EchoAnatomy) -> (Option<(Vec3, Vec3)>, f32) {
    let Some(m) = pack.meshes.iter().find(|m| m.name == "ivc") else {
        return (None, 0.0);
    };
    let pts: Vec<Vec3> = m.mesh.vertices.iter().map(|v| Vec3::from(v.position)).collect();
    if pts.is_empty() {
        return (None, 0.0);
    }
    let c = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
    let top = pts
        .iter()
        .copied()
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    let bottom = pts.iter().copied().fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let dir = (top - bottom).normalize_or(Vec3::Y);
    // march down the axis from the vein's lower end while blood or the pack's vein continues
    let t = anatomy.thorax_volume();
    let h = anatomy.heart_volume();
    let start = pts
        .iter()
        .copied()
        .min_by(|a, b| a.dot(dir).total_cmp(&b.dot(dir)))
        .unwrap_or(c);
    let mut len = start.distance(top);
    let mut s = 0.0;
    loop {
        s += 2.0;
        let p = start - dir * s;
        let blood =
            t.label_at(p) == Label::ThoracicBlood || h.get(p).is_some_and(|l| l == Label::RaBlood as u8);
        if !blood || s > 200.0 {
            break;
        }
        len += 2.0;
    }
    (Some((c, dir)), len)
}

/// The aortic arch in the thorax label volume: the geodesic path through the thoracic blood from the
/// top of the pack's ascending aorta to the descending aorta behind it, and three points on it (the
/// ascending end, the top, the descending end). `None` when the blood does not connect.
fn find_arch(t: &LabelVolume, heart: &LabelVolume, ao_normal: Vec3, aortic: Vec3) -> Option<[Vec3; 3]> {
    // the top of the pack's ascending aorta: its lumen voxels farthest along the valve's axis
    let mut top = None::<(f32, Vec3)>;
    for (idx, l) in heart.labels.iter().enumerate() {
        if *l == Label::AortaLumen as u8 {
            let [i, j, k] = heart.coords(idx);
            let c = heart.centre(i, j, k);
            let h = (c - aortic).dot(ao_normal);
            if top.is_none_or(|(b, _)| h > b) {
                top = Some((h, c));
            }
        }
    }
    let seed_pt = top?.1;
    // the nearest thorax blood voxel to that point (within 12 mm)
    let dims = t.dims.map(i64::from);
    let id = |i: i64, j: i64, k: i64| (i + dims[0] * (j + dims[1] * k)) as usize;
    let blood = |i: i64, j: i64, k: i64| {
        i >= 0
            && j >= 0
            && k >= 0
            && i < dims[0]
            && j < dims[1]
            && k < dims[2]
            && t.labels[id(i, j, k)] == Label::ThoracicBlood as u8
    };
    let [ci, cj, ck] = t.voxel_of(seed_pt)?.map(i64::from);
    let r = (12.0 / t.spacing_mm).ceil() as i64;
    let mut seed = None::<(f32, [i64; 3])>;
    for dk in -r..=r {
        for dj in -r..=r {
            for di in -r..=r {
                if blood(ci + di, cj + dj, ck + dk) {
                    let d = ((di * di + dj * dj + dk * dk) as f32).sqrt();
                    if seed.is_none_or(|(b, _)| d < b) {
                        seed = Some((d, [ci + di, cj + dj, ck + dk]));
                    }
                }
            }
        }
    }
    let seed = seed?.1;
    // Dijkstra over blood voxels, 26-neighbourhood, cost in mm
    let n = t.len();
    let mut dist = vec![f32::INFINITY; n];
    let mut prev = vec![u32::MAX; n];
    let mut heap = BinaryHeap::new();
    let s_id = id(seed[0], seed[1], seed[2]);
    dist[s_id] = 0.0;
    heap.push((std::cmp::Reverse(0u32), s_id as u32));
    let mut neighbours = Vec::new();
    for dk in -1i64..=1 {
        for dj in -1i64..=1 {
            for di in -1i64..=1 {
                if (di, dj, dk) != (0, 0, 0) {
                    neighbours.push((
                        di,
                        dj,
                        dk,
                        ((di * di + dj * dj + dk * dk) as f32).sqrt() * t.spacing_mm,
                    ));
                }
            }
        }
    }
    while let Some((std::cmp::Reverse(dq), u)) = heap.pop() {
        let u = u as usize;
        if (dq as f32) / 16.0 > dist[u] + 1e-3 {
            continue;
        }
        let [ui, uj, uk] = t.coords(u).map(i64::from);
        for &(di, dj, dk, w) in &neighbours {
            let (a, b, c) = (ui + di, uj + dj, uk + dk);
            if !blood(a, b, c) {
                continue;
            }
            let v = id(a, b, c);
            let nd = dist[u] + w;
            if nd < dist[v] {
                dist[v] = nd;
                prev[v] = u as u32;
                heap.push((std::cmp::Reverse((nd * 16.0) as u32), v as u32));
            }
        }
    }
    // the descending limb: the nearest reached voxel at least 30 mm below the seed and at least 40 mm
    // behind it (the descending aorta runs down beside the spine)
    let mut best = None::<(f32, usize)>;
    for (v, d) in dist.iter().enumerate() {
        if d.is_finite() {
            let [i, j, k] = t.coords(v);
            let c = t.centre(i, j, k);
            if c.y < seed_pt.y - 30.0 && c.z < seed_pt.z - 40.0 && best.is_none_or(|(b, _)| *d < b) {
                best = Some((*d, v));
            }
        }
    }
    let (_, end) = best?;
    let mut path = Vec::new();
    let mut v = end;
    while v != usize::MAX && v != s_id {
        let [i, j, k] = t.coords(v);
        path.push(t.centre(i, j, k));
        let p = prev[v];
        v = if p == u32::MAX { usize::MAX } else { p as usize };
    }
    path.push(seed_pt);
    path.reverse();
    if path.len() < 8 {
        return None;
    }
    let top_i = (0..path.len()).max_by(|&a, &b| path[a].y.total_cmp(&path[b].y))?;
    let at = |f: f32| path[((path.len() - 1) as f32 * f).round() as usize];
    // ascending end: where the path is a quarter of the way up to its top; descending: the last point
    let asc = path[top_i / 2];
    let _ = at;
    Some([asc, path[top_i], path[path.len() - 1]])
}

// ---- building the targets ---------------------------------------------------------------------------

fn lm(key: &'static str, pos_mm: Vec3) -> Landmark {
    Landmark { key, pos_mm }
}

fn rotate_about(v: Vec3, axis: Vec3, deg: f32) -> Vec3 {
    glam::Quat::from_axis_angle(axis.normalize_or(Vec3::Y), deg.to_radians()) * v
}

fn perp(v: Vec3, axis: Vec3) -> Vec3 {
    v - axis * v.dot(axis)
}

impl ViewTarget {
    fn new(id: ViewId, aim_mm: Vec3) -> Self {
        Self {
            id,
            window: id.window(),
            reach: Reach::Reachable,
            in_plane: Vec::new(),
            absent: Vec::new(),
            marker_dir: None,
            marker_loose: false,
            normal_axis: None,
            circular_at: None,
            apical: None,
            aim_mm,
            marks: Vec::new(),
            checklist: Vec::new(),
        }
    }

    fn mark(&mut self, key: &'static str, pos: Vec3) -> usize {
        self.marks.push(lm(key, pos));
        self.marks.len() - 1
    }

    fn item(&mut self, key: &'static str, test: ItemTest) {
        self.checklist.push(ChecklistItem { key, test });
    }
}

/// How far a "virtual" in-plane point is placed from the long axis to fix a plane that contains the
/// axis, mm. **Designed** (about the LV's radius).
const VIRTUAL_RADIUS_MM: f32 = 25.0;
/// The RV-focused four-chamber is the four-chamber plane turned counterclockwise by this, degrees
/// ("slightly", Mitchell p26). **Designed**.
const RV_FOCUS_TURN_DEG: f32 = 20.0;
/// Height of the apical short-axis level on the LV long axis (0 apex … 1 mitral): the middle of the
/// apical third (Mitchell p26 "the apical third of the ventricle"). **Designed** from that wording.
const APICAL_LEVEL_HEIGHT: f32 = 1.0 / 6.0;

/// The target of `id` from the anatomy's landmarks.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn build_target(id: ViewId, k: &Landmarks, anatomy: &EchoAnatomy) -> ViewTarget {
    let _ = anatomy;
    let long = k.lv_axis;
    // the A4C marker: toward the LV from the RV, in the plane through the axis and the tricuspid
    let rv_to_lv = perp(k.lv_blood - k.rv_blood, long);
    let tv_dir = perp(k.tricuspid - k.lv_mid, long);
    let a4c_marker = {
        // the plane of the A4C contains the axis and the tricuspid; its in-plane marker points away
        // from the tricuspid side (the left heart on the screen's right, Mitchell p26)
        let m = -tv_dir.normalize_or(Vec3::X);
        if m.dot(rv_to_lv) < 0.0 { -m } else { m }
    };
    let mut v = match id {
        ViewId::ParasternalLongAxis => {
            let mut v = ViewTarget::new(id, (k.mitral + k.aortic) * 0.5);
            v.in_plane = vec![
                lm("aortic", k.aortic),
                lm("mitral", k.mitral),
                lm("lv_mid", k.lv_mid),
            ];
            v.absent = vec![lm("apex", k.apex)];
            // the marker toward the right shoulder, along the long axis toward the base (p23)
            v.marker_dir = Some(k.aortic - k.apex);
            let apex = v.mark("apex", k.apex);
            v.mark("aortic", k.aortic);
            v.mark("mitral", k.mitral);
            v.mark("la", k.la);
            v.item("lv", ItemTest::Label(Label::LvBlood));
            v.item("rv", ItemTest::Label(Label::RvBlood));
            v.item("la", ItemTest::Label(Label::LaBlood));
            v.item("aorta", ItemTest::Label(Label::AortaLumen));
            v.item("mitral-leaflets", ItemTest::Valve(Valve::Mitral));
            v.item("aortic-leaflets", ItemTest::Valve(Valve::Aortic));
            v.item("no-papillary", ItemTest::LabelAbsent(Label::PapillaryMuscle));
            v.item("no-apex", ItemTest::NotInSector(apex));
            v
        }
        ViewId::RvOutflow => {
            let mut v = ViewTarget::new(id, (k.pulmonic + k.pa) * 0.5);
            v.in_plane = vec![lm("pulmonic", k.pulmonic), lm("pa", k.pa), lm("rv", k.rv_blood)];
            v.marker_dir = Some(k.pa - k.rv_blood);
            v.mark("rv", k.rv_blood);
            v.mark("pulmonic", k.pulmonic);
            v.mark("pa", k.pa);
            v.item("rv", ItemTest::Label(Label::RvBlood));
            v.item("pa", ItemTest::Label(Label::PulmonaryArteryLumen));
            v.item("pulmonic-leaflets", ItemTest::Valve(Valve::Pulmonic));
            v
        }
        ViewId::RvInflow => {
            let mut v = ViewTarget::new(id, k.tricuspid);
            v.in_plane = vec![lm("tricuspid", k.tricuspid), lm("ra", k.ra)];
            // the RV on the upper part of the screen, its anterior wall to the right (p25)
            v.marker_dir = Some(perp(Vec3::Z, (k.rv_blood - k.ra).normalize_or(Vec3::X)));
            v.mark("ra", k.ra);
            v.mark("tricuspid", k.tricuspid);
            v.mark("rv", k.rv_blood);
            v.item("ra", ItemTest::Label(Label::RaBlood));
            v.item("rv", ItemTest::Label(Label::RvBlood));
            v.item("tricuspid-leaflets", ItemTest::Valve(Valve::Tricuspid));
            v
        }
        ViewId::ShortAxisAorticValve => {
            let mut v = ViewTarget::new(id, k.aortic);
            v.in_plane = vec![lm("aortic", k.aortic)];
            v.marker_dir = Some(psax_marker(k));
            v.normal_axis = Some(long);
            v.mark("aortic", k.aortic);
            v.mark("tricuspid", k.tricuspid);
            v.mark("la", k.la);
            v.mark("pulmonic", k.pulmonic);
            v.item("aortic-leaflets", ItemTest::Valve(Valve::Aortic));
            v.item("la", ItemTest::Label(Label::LaBlood));
            v.item("rv", ItemTest::Label(Label::RvBlood));
            v
        }
        ViewId::ShortAxisMitralValve => {
            let mut v = ViewTarget::new(id, k.mitral);
            v.in_plane = vec![lm("mitral", k.mitral)];
            v.marker_dir = Some(psax_marker(k));
            v.normal_axis = Some(long);
            v.circular_at = Some(k.mitral);
            v.mark("mitral", k.mitral);
            v.item("mitral-leaflets", ItemTest::Valve(Valve::Mitral));
            v.item("lv", ItemTest::Label(Label::LvBlood));
            v.item("lv-wall", ItemTest::Label(Label::LvMyocardium));
            v.item("rv", ItemTest::Label(Label::RvBlood));
            v
        }
        ViewId::ShortAxisPapillary => {
            let mut v = ViewTarget::new(id, (k.pap[0] + k.pap[1]) * 0.5);
            v.in_plane = vec![lm("pap_al", k.pap[0]), lm("pap_pm", k.pap[1])];
            v.marker_dir = Some(psax_marker(k));
            v.normal_axis = Some(long);
            v.circular_at = Some((k.pap[0] + k.pap[1]) * 0.5);
            v.mark("pap_al", k.pap[0]);
            v.mark("pap_pm", k.pap[1]);
            v.item("lv", ItemTest::Label(Label::LvBlood));
            v.item("lv-wall", ItemTest::Label(Label::LvMyocardium));
            v.item("pap-al", ItemTest::Papillary(0));
            v.item("pap-pm", ItemTest::Papillary(1));
            v.item("rv", ItemTest::Label(Label::RvBlood));
            v
        }
        ViewId::ShortAxisApex => {
            let p = k.axis_point(APICAL_LEVEL_HEIGHT);
            let mut v = ViewTarget::new(id, p);
            v.in_plane = vec![lm("lv_apical", p)];
            v.marker_dir = Some(psax_marker(k));
            v.normal_axis = Some(long);
            v.circular_at = Some(p);
            v.mark("lv_apical", p);
            v.item("lv", ItemTest::Label(Label::LvBlood));
            v.item("lv-wall", ItemTest::Label(Label::LvMyocardium));
            v.item("no-leaflets", ItemTest::LabelAbsent(Label::ValveLeaflet));
            v
        }
        ViewId::ApicalFourChamber => {
            let mut v = ViewTarget::new(id, (k.mitral + k.tricuspid) * 0.5);
            v.in_plane = vec![lm("mitral", k.mitral), lm("tricuspid", k.tricuspid)];
            v.marker_dir = Some(a4c_marker);
            v.apical = Some((k.apex, long));
            v.mark("apex", k.apex);
            v.mark("mitral", k.mitral);
            v.mark("tricuspid", k.tricuspid);
            four_chamber_items(&mut v);
            v
        }
        ViewId::RvFocusedFourChamber => {
            // the four-chamber turned slightly counterclockwise about the LV axis, kept through the
            // LV centre (p26)
            let m = rotate_about(a4c_marker, long, -RV_FOCUS_TURN_DEG);
            let mut v = ViewTarget::new(id, (k.mitral + k.tricuspid) * 0.5);
            v.in_plane = vec![
                lm("tricuspid", k.tricuspid),
                lm("rv_apex", k.rv_apex),
                lm("lv_mid", k.lv_mid),
            ];
            v.marker_dir = Some(m);
            v.mark("rv_apex", k.rv_apex);
            v.mark("tricuspid", k.tricuspid);
            v.mark("ra", k.ra);
            v.item("rv", ItemTest::Label(Label::RvBlood));
            v.item("ra", ItemTest::Label(Label::RaBlood));
            v.item("tricuspid-leaflets", ItemTest::Valve(Valve::Tricuspid));
            v.item("lv", ItemTest::Label(Label::LvBlood));
            v
        }
        ViewId::ApicalFiveChamber => {
            // the four-chamber tilted anteriorly until the LVOT and the aortic valve are in the plane
            let mut v = ViewTarget::new(id, (k.aortic + k.tricuspid) * 0.5);
            v.in_plane = vec![
                lm("aortic", k.aortic),
                lm("tricuspid", k.tricuspid),
                lm("lv_mid", k.lv_mid),
            ];
            v.marker_dir = Some(a4c_marker);
            v.mark("aortic", k.aortic);
            v.mark("ao_asc", k.ao_asc);
            v.mark("tricuspid", k.tricuspid);
            v.item("lv", ItemTest::Label(Label::LvBlood));
            v.item("aortic-leaflets", ItemTest::Valve(Valve::Aortic));
            v.item("aorta", ItemTest::Label(Label::AortaLumen));
            v.item("rv", ItemTest::Label(Label::RvBlood));
            v.item("ra", ItemTest::Label(Label::RaBlood));
            v
        }
        ViewId::ApicalTwoChamber => {
            // the four-chamber rotated 60 degrees counterclockwise (p30, p23)
            let m = rotate_about(a4c_marker, long, -60.0);
            let virt = k.lv_mid + m.normalize_or(Vec3::X) * VIRTUAL_RADIUS_MM;
            let mut v = ViewTarget::new(id, k.mitral);
            v.in_plane = vec![
                lm("mitral", k.mitral),
                lm("lv_mid", k.lv_mid),
                lm("lv_wall", virt),
            ];
            v.absent = vec![lm("tricuspid", k.tricuspid), lm("rv_apex", k.rv_apex)];
            v.marker_dir = Some(m);
            v.apical = Some((k.apex, long));
            v.mark("apex", k.apex);
            v.mark("mitral", k.mitral);
            v.mark("la", k.la);
            v.item("lv", ItemTest::Label(Label::LvBlood));
            v.item("la", ItemTest::Label(Label::LaBlood));
            v.item("mitral-leaflets", ItemTest::Valve(Valve::Mitral));
            v.item("no-rv", ItemTest::LabelAbsent(Label::RvBlood));
            v
        }
        ViewId::ApicalThreeChamber => {
            // a further 60 degrees counterclockwise: the plane through the axis and the aortic valve
            let m = perp(k.aortic - k.mitral, long);
            let mut v = ViewTarget::new(id, (k.mitral + k.aortic) * 0.5);
            v.in_plane = vec![
                lm("mitral", k.mitral),
                lm("aortic", k.aortic),
                lm("lv_mid", k.lv_mid),
            ];
            v.marker_dir = Some(m);
            v.apical = Some((k.apex, long));
            v.mark("apex", k.apex);
            v.mark("mitral", k.mitral);
            v.mark("aortic", k.aortic);
            v.item("lv", ItemTest::Label(Label::LvBlood));
            v.item("la", ItemTest::Label(Label::LaBlood));
            v.item("mitral-leaflets", ItemTest::Valve(Valve::Mitral));
            v.item("aortic-leaflets", ItemTest::Valve(Valve::Aortic));
            v.item("aorta", ItemTest::Label(Label::AortaLumen));
            v
        }
        ViewId::SubcostalFourChamber => {
            // the beam through the four chambers from below; the marker to the patient's left (p31)
            let mut v = ViewTarget::new(id, (k.mitral + k.tricuspid) * 0.5);
            v.in_plane = vec![
                lm("mitral", k.mitral),
                lm("tricuspid", k.tricuspid),
                lm("lv_mid", k.lv_mid),
            ];
            v.marker_dir = Some(Vec3::X);
            v.mark("mitral", k.mitral);
            v.mark("tricuspid", k.tricuspid);
            v.mark("la", k.la);
            v.mark("ra", k.ra);
            four_chamber_items(&mut v);
            v
        }
        ViewId::SubcostalIvc => {
            let (p, d) = k.ivc.unwrap_or((k.ra, Vec3::Y));
            let mut v = ViewTarget::new(id, p);
            v.in_plane = vec![lm("ivc", p), lm("ra", k.ra)];
            v.marker_dir = Some(d);
            v.mark("ivc", p);
            v.mark("ra", k.ra);
            v.item("ra", ItemTest::Label(Label::RaBlood));
            v.item("liver", ItemTest::Label(Label::Liver));
            v.item("ivc", ItemTest::InSector(0));
            if k.ivc_length_mm < IVC_MIN_LENGTH_MM {
                v.reach = Reach::NotReachable("ivc-short");
            }
            v
        }
        ViewId::SuprasternalArch => {
            let arch = k.arch.unwrap_or([k.ao_asc; 3]);
            let mut v = ViewTarget::new(id, arch[1]);
            v.in_plane = vec![
                lm("arch_asc", arch[0]),
                lm("arch_top", arch[1]),
                lm("arch_desc", arch[2]),
            ];
            v.marker_dir = Some(arch[2] - arch[0]);
            v.mark("arch_asc", arch[0]);
            v.mark("arch_top", arch[1]);
            v.mark("arch_desc", arch[2]);
            v.item("arch-asc", ItemTest::InSector(0));
            v.item("arch-top", ItemTest::InSector(1));
            v.item("arch-desc", ItemTest::InSector(2));
            v.item("blood", ItemTest::Label(Label::ThoracicBlood));
            if k.arch.is_none() {
                v.reach = Reach::NotReachable("no-arch");
            }
            v
        }
    };
    v.marker_loose = matches!(
        id,
        ViewId::RvOutflow | ViewId::RvInflow | ViewId::SubcostalIvc | ViewId::SuprasternalArch
    );
    v
}

/// The IVC must be this long in the label volumes for its long-axis view to exist: "for several
/// centimeters" (Mitchell p31); designed as 30 mm.
pub const IVC_MIN_LENGTH_MM: f32 = 30.0;

fn four_chamber_items(v: &mut ViewTarget) {
    v.item("lv", ItemTest::Label(Label::LvBlood));
    v.item("rv", ItemTest::Label(Label::RvBlood));
    v.item("la", ItemTest::Label(Label::LaBlood));
    v.item("ra", ItemTest::Label(Label::RaBlood));
    v.item("mitral-leaflets", ItemTest::Valve(Valve::Mitral));
    v.item("tricuspid-leaflets", ItemTest::Valve(Valve::Tricuspid));
}

/// The short-axis marker: the long-axis marker (apex toward the aortic root) turned 90 degrees
/// clockwise about the beam, which comes from the front (Mitchell p25: "rotating the transducer 90
/// clockwise from the PLAX view"). In this frame that is toward the patient's left shoulder.
fn psax_marker(k: &Landmarks) -> Vec3 {
    (-Vec3::Z).cross(k.aortic - k.apex)
}

// ---- occlusion along a line (expiration anatomy) ---------------------------------------------------

/// Bone and lung along `a → b` that lie outside the heart box, in mm (2 mm steps), at the lung's
/// expiration edge.
#[must_use]
pub fn segment_occlusion(anatomy: &EchoAnatomy, a: Vec3, b: Vec3) -> (f32, f32) {
    // 2 mm steps: the occlusion width the score reads is 4 mm
    let n = ((a.distance(b) / 2.0).ceil() as usize).max(1);
    let heart = anatomy.heart_volume();
    let (mut bone, mut lung) = (0.0f32, 0.0f32);
    let step = a.distance(b) / n as f32;
    for i in 0..n {
        let x = a.lerp(b, (i as f32 + 0.5) / n as f32);
        if heart.get(x).is_some_and(|h| h != Label::Outside as u8) {
            continue;
        }
        match anatomy.thorax_label_retracted(x, 1.0) {
            Label::Bone => bone += step,
            Label::Lung => lung += step,
            _ => {}
        }
    }
    (bone, lung)
}

/// Bone and lung along the beam from `apex` to the first heart-box tissue (or `depth_mm`), mm.
#[must_use]
pub fn beam_occlusion(anatomy: &EchoAnatomy, apex: Vec3, beam: Vec3, depth_mm: f32) -> (f32, f32) {
    let heart = anatomy.heart_volume();
    let (mut bone, mut lung) = (0.0f32, 0.0f32);
    let mut s = 0.5;
    while s < depth_mm {
        let x = apex + beam * s;
        match heart.get(x) {
            Some(h) if h != Label::Outside as u8 && h != Label::Pericardium as u8 => break,
            Some(h) if h == Label::Pericardium as u8 => {}
            _ => match anatomy.thorax_label_retracted(x, 1.0) {
                Label::Bone => bone += 1.0,
                Label::Lung => lung += 1.0,
                _ => {}
            },
        }
        s += 1.0;
    }
    (bone, lung)
}

// ---- the score -------------------------------------------------------------------------------------

/// A landmark as the probe sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    pub key: &'static str,
    /// Signed distance from the image plane, mm.
    pub off_plane_mm: f32,
    /// Angle from the beam toward the marker side, degrees.
    pub angle_deg: f32,
    /// Distance from the sector apex, mm.
    pub range_mm: f32,
    /// Inside the sector and the depth, by the margins.
    pub inside: bool,
}

/// The score of a pose against a view, with its terms.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ViewScore {
    /// The product of the terms, 0 … 1.
    pub total: f32,
    pub contact: bool,
    pub plane: f32,
    pub sector: f32,
    pub absent: f32,
    pub marker: f32,
    pub normal: f32,
    /// Short-axis LV levels: the roundness term and the ratio it was made from (1 when not asked).
    pub circularity: f32,
    pub circularity_ratio: f32,
    pub foreshortening: f32,
    pub window: f32,
    /// Angle between the index marker and its target in the image plane, degrees.
    pub marker_error_deg: f32,
    /// Foreshortening: distance of the true apex from the plane, mm (apical views).
    pub apex_off_plane_mm: f32,
    /// Foreshortening: beam to LV long axis, degrees.
    pub axis_error_deg: f32,
    /// The key landmarks, one reading each.
    pub readings: Vec<Reading>,
    /// The "must be absent" landmarks that are present, as [`Reading`]s.
    pub present_absent: Vec<Reading>,
}

/// Roundness of the LV blood in the image plane around `at` (heart frame, rest anatomy): the ratio of
/// the minor to the major axis of the extent of the LV-blood samples of a 3 mm grid in the plane within
/// 55 mm of the landmark's foot (principal axes of the samples). 0 when the section is under about
/// 250 mm² (a grazing or missed cut).
fn lv_section_roundness(anatomy: &EchoAnatomy, f: &ProbeFrame, at: Vec3) -> f32 {
    const STEP: f32 = 3.0;
    const N: i32 = 18;
    let rel = at - f.apex_mm;
    let (u0, w0) = (rel.dot(f.marker), rel.dot(f.beam));
    let heart = anatomy.heart_volume();
    let (mut n, mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for j in -N..=N {
        for i in -N..=N {
            let (x, y) = (i as f32 * STEP, j as f32 * STEP);
            let p = f.apex_mm + f.marker * (u0 + x) + f.beam * (w0 + y);
            if heart.get(p) == Some(Label::LvBlood as u8) {
                n += 1.0;
                sx += x;
                sy += y;
                sxx += x * x;
                syy += y * y;
                sxy += x * y;
            }
        }
    }
    if n * STEP * STEP < 250.0 {
        return 0.0;
    }
    let (mx, my) = (sx / n, sy / n);
    let (cxx, cyy, cxy) = (sxx / n - mx * mx, syy / n - my * my, sxy / n - mx * my);
    let tr = cxx + cyy;
    let disc = ((cxx - cyy) * 0.5).powi(2) + cxy * cxy;
    let (l1, l2) = (tr * 0.5 + disc.sqrt(), tr * 0.5 - disc.sqrt());
    if l1 <= 1e-6 {
        0.0
    } else {
        (l2.max(0.0) / l1).sqrt()
    }
}

fn gauss_cost(x: f32, sigma: f32) -> f32 {
    (x / sigma).powi(2)
}

/// Rings of slide seeds around the window's starting point (4 = 9 × 9 seeds). **Designed**: fewer
/// seeds search faster; the reference poses of the shipped views are the same as with 5 rings.
const SEED_RINGS: i32 = 4;
const CONTACT_COST: f32 = 40.0;
/// A coarse hidden share of the sector (9 lines, 3 mm steps; [`EchoAnatomy::hidden_share_at_rest`] is
/// the finer one) for the search.
fn hidden_estimate(anatomy: &EchoAnatomy, f: &ProbeFrame, pose: &ProbePose) -> f32 {
    const LINES: usize = 15;
    let heart = anatomy.heart_volume();
    let (mut hidden, mut total) = (0u32, 0u32);
    let steps = (pose.depth_mm / 3.0) as usize;
    for line in 0..LINES {
        let dir = f.line_direction(line, LINES, pose.sector_deg);
        let mut behind = false;
        for k in 0..steps {
            let x = f.apex_mm + dir * (k as f32 * 3.0 + 1.5);
            match heart.get(x) {
                Some(h) if h != Label::Outside as u8 => {
                    if h != Label::Pericardium as u8 {
                        total += 1;
                        hidden += u32::from(behind);
                    }
                }
                _ => behind |= anatomy.thorax_label_retracted(x, 1.0).occludes(),
            }
        }
    }
    if total == 0 {
        1.0
    } else {
        hidden as f32 / total as f32
    }
}

/// Weight of the hidden share of the sector in the reference search (not in the score). **Designed**.
const HIDDEN_WEIGHT: f32 = 3.0;
/// The second phase of the search may give up this much cost (about a 10 % lower score) for a cleaner window.
/// **Designed**.
const HIDDEN_SLACK: f32 = 0.1;
/// ... but never below this score when the first phase reached it. **Designed**.
const MIN_SCORE: f32 = 0.91;

/// The geometry of one landmark in the probe's frame.
fn read(f: &ProbeFrame, pose: &ProbePose, l: &Landmark) -> Reading {
    let rel = l.pos_mm - f.apex_mm;
    let along = rel.dot(f.beam);
    let lat = rel.dot(f.marker);
    Reading {
        key: l.key,
        off_plane_mm: rel.dot(f.elevation),
        angle_deg: lat.atan2(along).to_degrees(),
        range_mm: rel.length(),
        inside: false,
    }
    .with_inside(pose, 0.0, 0.0)
}

impl Reading {
    fn with_inside(mut self, pose: &ProbePose, sector_margin: f32, depth_margin: f32) -> Self {
        self.inside = self.angle_deg.abs() <= pose.sector_deg * 0.5 - sector_margin
            && self.range_mm <= pose.depth_mm - depth_margin;
        self
    }
}

/// The additive cost terms of a pose (its `exp(-sum)` is the score) and everything the hints need.
struct Costs {
    plane: f32,
    sector: f32,
    absent: f32,
    marker: f32,
    normal: f32,
    circ: f32,
    circularity_ratio: f32,
    fore: f32,
    window: f32,
    marker_error_deg: f32,
    apex_off_plane_mm: f32,
    axis_error_deg: f32,
    readings: Vec<Reading>,
    present_absent: Vec<Reading>,
}

fn costs(
    tol: &ViewTolerances,
    anatomy: &EchoAnatomy,
    v: &ViewTarget,
    f: &ProbeFrame,
    pose: &ProbePose,
) -> Costs {
    let mut c = Costs {
        plane: 0.0,
        sector: 0.0,
        absent: 0.0,
        marker: 0.0,
        normal: 0.0,
        circ: 0.0,
        circularity_ratio: 1.0,
        fore: 0.0,
        window: 0.0,
        marker_error_deg: 0.0,
        apex_off_plane_mm: 0.0,
        axis_error_deg: 0.0,
        readings: Vec::with_capacity(v.in_plane.len()),
        present_absent: Vec::new(),
    };
    let half = pose.sector_deg * 0.5;
    let mut clear = 0.0f32;
    for l in &v.in_plane {
        let r = read(f, pose, l);
        c.plane += gauss_cost(r.off_plane_mm, tol.plane_sigma_mm);
        let ang_exc = (r.angle_deg.abs() - (half - tol.sector_margin_deg)).max(0.0);
        let rad_exc = (r.range_mm - (pose.depth_mm - tol.depth_margin_mm)).max(0.0);
        c.sector += gauss_cost(ang_exc, tol.sector_sigma_deg) + gauss_cost(rad_exc, tol.depth_sigma_mm);
        c.readings.push(Reading {
            inside: ang_exc == 0.0 && rad_exc == 0.0,
            ..r
        });
        let (bone, lung) = segment_occlusion(anatomy, f.apex_mm, l.pos_mm);
        clear += (-gauss_cost(bone + lung, tol.occlusion_sigma_mm)).exp();
    }
    if !v.in_plane.is_empty() {
        let q = clear / v.in_plane.len() as f32;
        c.window = -(q.max(1e-4)).ln();
    }
    for l in &v.absent {
        let r = read(f, pose, l);
        let ang_exc = (r.angle_deg.abs() - half).max(0.0);
        let rad_exc = (r.range_mm - pose.depth_mm).max(0.0);
        let inside =
            (-gauss_cost(ang_exc, tol.sector_sigma_deg) - gauss_cost(rad_exc, tol.depth_sigma_mm)).exp();
        let presence = (-gauss_cost(r.off_plane_mm, tol.absent_sigma_mm)).exp() * inside;
        c.absent += -((1.0 - presence).max(1e-4)).ln();
        if presence > 0.3 {
            c.present_absent.push(r);
        }
    }
    if let Some(m) = v.marker_dir {
        let tp = m - f.elevation * m.dot(f.elevation);
        if tp.length() > 1e-3 {
            let a = f.marker.dot(tp.normalize()).clamp(-1.0, 1.0).acos().to_degrees();
            c.marker_error_deg = a;
            c.marker = gauss_cost(
                a,
                if v.marker_loose {
                    tol.marker_loose_sigma_deg
                } else {
                    tol.marker_sigma_deg
                },
            );
        }
    }
    if let Some(ax) = v.normal_axis {
        let a = f.elevation.dot(ax).abs().clamp(0.0, 1.0).acos().to_degrees();
        c.normal = gauss_cost((a - tol.normal_free_deg).max(0.0), tol.normal_sigma_deg);
    }
    if let Some(at) = v.circular_at {
        let r = lv_section_roundness(anatomy, f, at);
        c.circularity_ratio = r;
        c.circ = gauss_cost((tol.circularity_min - r).max(0.0), tol.circularity_sigma);
    }
    if let Some((apex, axis)) = v.apical {
        let rel = apex - f.apex_mm;
        c.apex_off_plane_mm = rel.dot(f.elevation).abs();
        let a = f.beam.dot(axis).abs().clamp(0.0, 1.0).acos().to_degrees();
        c.axis_error_deg = a;
        c.fore =
            gauss_cost(c.apex_off_plane_mm, tol.foreshortening_sigma_mm) + gauss_cost(a, tol.axis_sigma_deg);
    }
    c
}

impl Costs {
    fn sum(&self) -> f32 {
        self.plane
            + self.sector
            + self.absent
            + self.marker
            + self.normal
            + self.circ
            + self.fore
            + self.window
    }
}

/// The scoring and reference-pose machinery of the views of one anatomy. Built once per pack
/// ([`Self::new`] reads only the pack, the motion model and the echo anatomy; it keeps none of them).
#[derive(Debug)]
pub struct ViewTargets {
    pub tolerances: ViewTolerances,
    pub landmarks: Landmarks,
    views: Vec<ViewTarget>,
    refs: Vec<OnceLock<Reference>>,
}

/// The reference pose of a view and what it scores.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    pub pose: ProbePose,
    pub score: ViewScore,
    /// Share of the heart samples of the sector, at rest and in expiration, hidden by bone or lung.
    pub hidden_fraction: f32,
    /// Whether the view is shipped: reachable in the anatomy and found (score at least the found
    /// threshold) at its reference pose.
    pub reachable: bool,
    /// Score evaluations the search used.
    pub evaluations: u32,
}

impl ViewTargets {
    /// Targets of every view of this anatomy. `None` when a needed landmark is missing.
    #[must_use]
    pub fn new(pack: &AnatomyPack, model: &MotionModel, anatomy: &EchoAnatomy) -> Option<Self> {
        let landmarks = Landmarks::new(pack, model, anatomy)?;
        let views = ViewId::ALL
            .iter()
            .map(|id| build_target(*id, &landmarks, anatomy))
            .collect();
        Some(Self {
            tolerances: ViewTolerances::default(),
            landmarks,
            views,
            refs: ViewId::ALL.iter().map(|_| OnceLock::new()).collect(),
        })
    }

    /// The target of `id`.
    #[must_use]
    pub fn target(&self, id: ViewId) -> &ViewTarget {
        &self.views[ViewId::ALL.iter().position(|v| *v == id).unwrap_or(0)]
    }

    /// Every target, in teaching order.
    #[must_use]
    pub fn targets(&self) -> &[ViewTarget] {
        &self.views
    }

    /// The score of `pose` for view `id`: a pure function of the anatomy at rest and the pose.
    #[must_use]
    pub fn score(&self, anatomy: &EchoAnatomy, id: ViewId, pose: &ProbePose) -> ViewScore {
        let v = self.target(id);
        let pose = pose.clamped();
        let f = pose.frame(anatomy.designed.contact_limit_deg);
        if !f.contact {
            return ViewScore {
                contact: false,
                ..ViewScore::default()
            };
        }
        let c = costs(&self.tolerances, anatomy, v, &f, &pose);
        let e = |x: f32| (-x).exp();
        ViewScore {
            total: e(c.sum()).clamp(0.0, 1.0),
            contact: true,
            plane: e(c.plane),
            sector: e(c.sector),
            absent: e(c.absent),
            marker: e(c.marker),
            normal: e(c.normal),
            circularity: e(c.circ),
            circularity_ratio: c.circularity_ratio,
            foreshortening: e(c.fore),
            window: e(c.window),
            marker_error_deg: c.marker_error_deg,
            apex_off_plane_mm: c.apex_off_plane_mm,
            axis_error_deg: c.axis_error_deg,
            readings: c.readings,
            present_absent: c.present_absent,
        }
    }

    /// How far (mm) `skin` lies outside the area the reference search may use for `v`: beyond
    /// [`ViewTolerances::max_slide_mm`] of the window's starting point, or (parasternal) not left of the
    /// sternal midline by [`ViewTolerances::parasternal_left_margin_mm`]. 0 inside.
    #[must_use]
    pub fn outside_window(&self, anatomy: &EchoAnatomy, v: &ViewTarget, skin: Vec3) -> f32 {
        let tol = &self.tolerances;
        let start = anatomy.window_pose(v.window).skin_mm;
        let mut out = (skin.distance(start) - tol.max_slide_mm).max(0.0);
        if v.window == EchoWindow::Parasternal {
            out = out.max(anatomy.midline_x_mm() + tol.parasternal_left_margin_mm - skin.x);
        }
        out
    }

    /// How many lines of `v`'s checklist the rest slice at `pose` leaves unticked.
    fn unticked_at_rest(
        &self,
        anatomy: &EchoAnatomy,
        v: &ViewTarget,
        pose: &ProbePose,
        rest: &crate::slice::SliceState,
        image: &mut LabelImage,
    ) -> usize {
        rest.slice(anatomy, pose, image);
        let stats = ImageStats::of(image, anatomy);
        self.checklist_of(v, &stats, pose, anatomy)
            .iter()
            .filter(|ok| !**ok)
            .count()
    }

    /// The search objective: the cost, and, in the second phase (`cap` = the best cost of the first
    /// plus [`HIDDEN_SLACK`]), the hidden share of the sector as a tie-breaker, with a steep wall
    /// where the cost would pass the cap (the hidden share never buys a worse score).
    fn cost_only(
        &self,
        anatomy: &EchoAnatomy,
        v: &ViewTarget,
        pose: &ProbePose,
        cap: Option<f32>,
        evals: &mut u32,
    ) -> f32 {
        *evals += 1;
        let pose = pose.clamped();
        let f = pose.frame(anatomy.designed.contact_limit_deg);
        if !f.contact {
            return CONTACT_COST + (f.beam_from_normal_deg - anatomy.designed.contact_limit_deg).max(0.0);
        }
        let out = self.outside_window(anatomy, v, pose.skin_mm);
        if out > 0.0 {
            return CONTACT_COST + out;
        }
        let c = costs(&self.tolerances, anatomy, v, &f, &pose).sum();
        match cap {
            None => c,
            Some(cap) => c + HIDDEN_WEIGHT * hidden_estimate(anatomy, &f, &pose) + 30.0 * (c - cap).max(0.0),
        }
    }

    /// The reference pose of `id`, found once (deterministically) by [`Self::search`] and kept.
    #[must_use]
    pub fn reference(&self, anatomy: &EchoAnatomy, id: ViewId) -> &Reference {
        let i = ViewId::ALL.iter().position(|v| *v == id).unwrap_or(0);
        self.refs[i].get_or_init(|| self.search(anatomy, &self.views[i]))
    }

    /// The deterministic reference search: slide seeds on a grid around the window's starting skin
    /// point, each aimed at the view's target; the angles of each refined by a compass search; the
    /// best few refined again in slide; the imaging depth set to fit the key landmarks.
    #[must_use]
    pub fn search(&self, anatomy: &EchoAnatomy, v: &ViewTarget) -> Reference {
        let tol = &self.tolerances;
        let limit = anatomy.designed.contact_limit_deg;
        let start = anatomy.window_pose(v.window);
        let mut evals = 0u32;
        let (c, l) = skin_tangents(&start);
        let marker = v.marker_dir.unwrap_or(Vec3::X);
        let aim = |skin: Vec3, n: Vec3| {
            ProbePose::aimed(
                skin,
                n,
                v.aim_mm,
                marker,
                limit,
                DEFAULT_DEPTH_MM,
                DEFAULT_SECTOR_DEG,
            )
        };
        let mut seeds: Vec<(f32, ProbePose)> = Vec::new();
        for iv in -SEED_RINGS..=SEED_RINGS {
            for iu in -SEED_RINGS..=SEED_RINGS {
                let want =
                    start.skin_mm + c * (iv as f32 * tol.seed_step_mm) + l * (iu as f32 * tol.seed_step_mm);
                let Some((q, n)) = anatomy.skin().place(want) else {
                    continue;
                };
                let (p, cost) = self.refine_angles(anatomy, v, aim(q, n), None, &mut evals);
                seeds.push((cost, p));
            }
        }
        seeds.sort_by(|a, b| a.0.total_cmp(&b.0));
        seeds.dedup_by(|a, b| a.1.skin_mm.distance(b.1.skin_mm) < 0.5);
        seeds.truncate(6);
        // phase A: the best score
        let seed_poses: Vec<ProbePose> = seeds.iter().map(|s| s.1).collect();
        let mut phase_a: Vec<(f32, ProbePose)> = seeds
            .into_iter()
            .map(|(cost, pose)| self.refine_slide(anatomy, v, pose, cost, None, &mut evals))
            .collect();
        phase_a.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some(&(best_cost, best_pose)) = phase_a.first() else {
            let pose = anatomy.window_pose(v.window);
            let score = self.score(anatomy, v.id, &pose);
            return Reference {
                hidden_fraction: anatomy.hidden_share_at_rest(&pose),
                pose,
                score,
                reachable: false,
                evaluations: evals,
            };
        };
        // phase B: among the poses within the slack of that score, the cleanest window
        // (never below a 0.91 score when the first phase reached it)
        let slack = ((-MIN_SCORE.ln()) - best_cost).clamp(0.0, HIDDEN_SLACK);
        let cap = Some(best_cost + slack);
        // the checklist is judged on the rest slice of each finalist (a pose that scores well but
        // shows nothing of what the view must show does not count)
        let rest = crate::slice::SliceState::new(anatomy);
        let mut rest_image = LabelImage::new(128, 512);
        let mut rank = |pose: &ProbePose| {
            let f = pose.frame(limit);
            let plain = costs(tol, anatomy, v, &f, pose).sum();
            let mut fitted = *pose;
            fitted.depth_mm = self.fit_depth(v, pose);
            let missing = self.unticked_at_rest(anatomy, v, &fitted, &rest, &mut rest_image);
            plain
                + HIDDEN_WEIGHT * anatomy.hidden_share_at_rest(pose)
                + 30.0 * (plain - best_cost - slack).max(0.0)
                + 10.0 * missing as f32
        };
        // the best-score pose itself competes with the cleaner windows within the slack
        let mut chosen = (rank(&best_pose), best_pose);
        // (from the best poses and from every seed: another interspace may be cleaner)
        let starts: Vec<ProbePose> = phase_a
            .iter()
            .take(3)
            .map(|a| a.1)
            .chain(seed_poses.into_iter().take(3))
            .collect();
        for p in &starts {
            let (p, cost) = self.refine_angles(anatomy, v, *p, cap, &mut evals);
            let (_, pose) = self.refine_slide(anatomy, v, p, cost, cap, &mut evals);
            let ranked = rank(&pose);
            if ranked < chosen.0 {
                chosen = (ranked, pose);
            }
        }
        let mut pose = chosen.1;
        pose.depth_mm = self.fit_depth(v, &pose);
        let score = self.score(anatomy, v.id, &pose);
        let hidden_fraction = anatomy.hidden_share_at_rest(&pose);
        let missing = self.unticked_at_rest(anatomy, v, &pose, &rest, &mut rest_image);
        let reachable =
            matches!(v.reach, Reach::Reachable) && score.total >= tol.found_threshold && missing == 0;
        Reference {
            pose,
            score,
            hidden_fraction,
            reachable,
            evaluations: evals,
        }
    }

    /// Slide the skin point by compass search (each trial re-optimises the angles).
    fn refine_slide(
        &self,
        anatomy: &EchoAnatomy,
        v: &ViewTarget,
        mut pose: ProbePose,
        mut cost: f32,
        cap: Option<f32>,
        evals: &mut u32,
    ) -> (f32, ProbePose) {
        let mut step = self.tolerances.seed_step_mm * 0.4;
        while step > 0.7 {
            let mut improved = false;
            let (cc, ll) = skin_tangents(&pose);
            for (du, dv) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let want = pose.skin_mm + ll * (du * step) + cc * (dv * step);
                let Some((q, n)) = anatomy.skin().place(want) else {
                    continue;
                };
                let cand = ProbePose {
                    skin_mm: q,
                    normal: n,
                    ..pose
                };
                let (p, cst) = self.refine_angles(anatomy, v, cand, cap, evals);
                if cst < cost - 1e-4 {
                    cost = cst;
                    pose = p;
                    improved = true;
                    break;
                }
            }
            if !improved {
                step *= 0.5;
            }
        }
        (cost, pose)
    }

    /// Compass search over tilt, rock and rotation (slide fixed).
    fn refine_angles(
        &self,
        anatomy: &EchoAnatomy,
        v: &ViewTarget,
        mut pose: ProbePose,
        cap: Option<f32>,
        evals: &mut u32,
    ) -> (ProbePose, f32) {
        let mut cost = self.cost_only(anatomy, v, &pose, cap, evals);
        let mut step = [6.0f32, 6.0, 8.0];
        for _ in 0..80 {
            let mut improved = false;
            for d in 0..3 {
                for sign in [1.0f32, -1.0] {
                    let mut p = pose;
                    match d {
                        0 => p.tilt_deg += sign * step[0],
                        1 => p.rock_deg += sign * step[1],
                        _ => p.rotation_deg += sign * step[2],
                    }
                    let p = p.clamped();
                    let cst = self.cost_only(anatomy, v, &p, cap, evals);
                    if cst < cost - 1e-5 {
                        cost = cst;
                        pose = p;
                        improved = true;
                    }
                }
            }
            if !improved {
                for s in &mut step {
                    *s *= 0.5;
                }
                if step[0] < 0.2 {
                    break;
                }
            }
        }
        (pose, cost)
    }

    /// The smallest depth (10 mm steps) that keeps every key landmark inside the depth margin.
    fn fit_depth(&self, v: &ViewTarget, pose: &ProbePose) -> f32 {
        let far = v
            .in_plane
            .iter()
            .map(|l| l.pos_mm.distance(pose.skin_mm))
            .fold(0.0f32, f32::max);
        let want = far + self.tolerances.depth_margin_mm + 10.0;
        ((want / 10.0).ceil() * 10.0).clamp(80.0, DEPTH_RANGE_MM.1)
    }
}

/// Cranial and patient-left tangents of the skin at `pose`.
fn skin_tangents(pose: &ProbePose) -> (Vec3, Vec3) {
    let n = -pose.normal;
    let c = (Vec3::Y - n * n.dot(Vec3::Y)).normalize_or(Vec3::Z);
    (c, n.cross(c))
}

// ---- the checklist ---------------------------------------------------------------------------------

/// Whether the point lies within `tol_mm` of the image plane and inside the sector.
fn in_plane_and_sector(f: &ProbeFrame, pose: &ProbePose, p: Vec3, tol_mm: f32) -> bool {
    let rel = p - f.apex_mm;
    rel.dot(f.elevation).abs() <= tol_mm
        && rel.dot(f.marker).atan2(rel.dot(f.beam)).to_degrees().abs() <= pose.sector_deg * 0.5
        && rel.length() <= pose.depth_mm
}

/// What a label image shows, counted over its unhidden samples.
#[derive(Debug, Clone)]
pub struct ImageStats {
    counts: [u32; 256],
    valves: [u32; 4],
    pap: [u32; 2],
    samples: u32,
}

impl ImageStats {
    /// Count `image`, assigning leaflet samples to the nearest valve and papillary samples to the
    /// nearest papillary body (by their rest positions).
    #[must_use]
    pub fn of(image: &LabelImage, anatomy: &EchoAnatomy) -> Self {
        let mut s = Self {
            counts: [0; 256],
            valves: [0; 4],
            pap: [0; 2],
            samples: (image.lines * image.samples) as u32,
        };
        if !image.contact {
            return s;
        }
        let valves = anatomy.valves();
        let pap = anatomy.papillary_muscles();
        let pap_mid = [
            (pap[0].a_mm + pap[0].b_mm) * 0.5,
            (pap[1].a_mm + pap[1].b_mm) * 0.5,
        ];
        for line in 0..image.lines {
            let visible = usize::from(image.hidden_from[line]).min(image.samples);
            for sample in 0..visible {
                let at = line * image.samples + sample;
                let b = image.labels[at];
                s.counts[b as usize] += 1;
                if b == Label::ValveLeaflet as u8 && image.from_heart[at] {
                    let r = image.rest_mm[at];
                    let near = valves
                        .iter()
                        .min_by(|a, b| a.centre_mm.distance(r).total_cmp(&b.centre_mm.distance(r)));
                    if let Some(v) = near {
                        s.valves[v.valve.index()] += 1;
                    }
                } else if b == Label::PapillaryMuscle as u8 {
                    let r = image.rest_mm[at];
                    let k = usize::from(pap_mid[1].distance(r) < pap_mid[0].distance(r));
                    s.pap[k] += 1;
                }
            }
        }
        s
    }
}

impl ViewTargets {
    /// Which lines of the view's checklist the image satisfies, in order.
    #[must_use]
    pub fn checklist(
        &self,
        id: ViewId,
        stats: &ImageStats,
        pose: &ProbePose,
        anatomy: &EchoAnatomy,
    ) -> Vec<bool> {
        self.checklist_of(self.target(id), stats, pose, anatomy)
    }

    fn checklist_of(
        &self,
        v: &ViewTarget,
        stats: &ImageStats,
        pose: &ProbePose,
        anatomy: &EchoAnatomy,
    ) -> Vec<bool> {
        let pose = pose.clamped();
        let f = pose.frame(anatomy.designed.contact_limit_deg);
        let min = ((stats.samples as f32 * self.tolerances.checklist_min_fraction) as u32).max(8);
        let sector_of = |i: usize| {
            let r = read(&f, &pose, &v.marks[i]);
            r.angle_deg.abs() <= pose.sector_deg * 0.5 && r.range_mm <= pose.depth_mm
        };
        v.checklist
            .iter()
            .map(|it| {
                if !f.contact {
                    return false;
                }
                match it.test {
                    ItemTest::Label(l) => {
                        // thin structures (leaflets, papillary bodies, the moderator band) need a
                        // smaller share of the image
                        let need = if matches!(
                            l,
                            Label::ModeratorBand | Label::PapillaryMuscle | Label::ValveLeaflet
                        ) {
                            (min / 16).max(2)
                        } else {
                            min
                        };
                        stats.counts[l as usize] >= need
                    }
                    ItemTest::LabelAbsent(l) => stats.counts[l as usize] < min,
                    ItemTest::Valve(valve) => {
                        // leaflets in the slice, or the valve's plane in the image plane (a thin leaflet
                        // can be between two samples or out of the cut at one instant of the beat)
                        stats.valves[valve.index()] >= (min / 16).max(2)
                            || anatomy
                                .valves()
                                .iter()
                                .find(|l| l.valve == valve)
                                .is_some_and(|l| in_plane_and_sector(&f, &pose, l.centre_mm, 8.0))
                    }
                    ItemTest::Papillary(k) => {
                        let c = &anatomy.papillary_muscles()[usize::from(k)];
                        stats.pap[usize::from(k)] >= (min / 16).max(2)
                            || in_plane_and_sector(&f, &pose, (c.a_mm + c.b_mm) * 0.5, c.radius_a_mm)
                    }
                    ItemTest::InSector(i) => sector_of(i),
                    ItemTest::NotInSector(i) => !sector_of(i),
                }
            })
            .collect()
    }
}

// ---- found tracker ---------------------------------------------------------------------------------

/// "Found" is the score held above the threshold for [`ViewTolerances::found_hold_s`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FoundTracker {
    held_s: f32,
    found: bool,
}

impl FoundTracker {
    /// Advance by `dt_s` at `score`; `true` once the threshold has been held long enough (and while
    /// it stays above it: found is sticky until the score drops below the threshold).
    pub fn update(&mut self, tol: &ViewTolerances, score: f32, dt_s: f32) -> bool {
        if score >= tol.found_threshold {
            self.held_s += dt_s.max(0.0);
            if self.held_s + 1e-4 >= tol.found_hold_s {
                self.found = true;
            }
        } else {
            self.held_s = 0.0;
            self.found = false;
        }
        self.found
    }

    /// Seconds held so far.
    #[must_use]
    pub fn held_s(&self) -> f32 {
        self.held_s
    }

    /// Whether the view is found.
    #[must_use]
    pub fn found(&self) -> bool {
        self.found
    }

    /// Start over.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The pose between `a` and `b` (`t` in 0 … 1): the skin point moved along the line and put back on
/// the skin, the angles linear (rotation by the short way), depth and sector linear. What *Show me*
/// plays.
#[must_use]
pub fn lerp_pose(anatomy: &EchoAnatomy, a: &ProbePose, b: &ProbePose, t: f32) -> ProbePose {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: f32, y: f32| x + (y - x) * t;
    let dr = (b.rotation_deg - a.rotation_deg + 180.0).rem_euclid(360.0) - 180.0;
    let base = ProbePose {
        tilt_deg: lerp(a.tilt_deg, b.tilt_deg),
        rock_deg: lerp(a.rock_deg, b.rock_deg),
        rotation_deg: a.rotation_deg + dr * t,
        depth_mm: lerp(a.depth_mm, b.depth_mm),
        sector_deg: lerp(a.sector_deg, b.sector_deg),
        ..*a
    };
    if t >= 1.0 {
        return ProbePose {
            skin_mm: b.skin_mm,
            normal: b.normal,
            ..base
        }
        .clamped();
    }
    anatomy.slide_to(&base, a.skin_mm.lerp(b.skin_mm, t)).clamped()
}
