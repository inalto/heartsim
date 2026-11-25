//! Everything the cross-section samples that does not change with time: the pack's two label
//! volumes (the heart box with the designed pericardium added), the designed parts in rest
//! coordinates (leaflets, papillary muscles, moderator band), the skin the probe slides on, the
//! displacement-grid layout and the four windows' starting poses (brief B31, items 4–6).

use crate::coords::MotionChamber;
use crate::designed::DesignedParams;
use crate::field::DisplacementField;
use crate::mesh_valve::{MeshValve, PosedMeshValve};
use crate::model::MotionModel;
use crate::probe::{DEFAULT_DEPTH_MM, DEFAULT_SECTOR_DEG, ProbePose, Skin};
use glam::{Vec2, Vec3};
use heartsim_core::Region;
use heartsim_ecg::Valve;
use heartsim_geometry::{AnatomyPack, Label, LabelVolume};

/// Why [`EchoAnatomy::new`] failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EchoError {
    /// The pack has no label volumes (format 3 adds them; the procedural pack has none).
    NoVolumes,
    /// The motion model has no four valve discs.
    NoValves,
    /// A designed part could not be placed on this heart (what and why).
    Placement(String),
}

impl std::fmt::Display for EchoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoVolumes => write!(f, "the anatomy pack has no label volumes (re-bake it: format 3)"),
            Self::NoValves => write!(f, "the anatomy has no four valve discs"),
            Self::Placement(s) => write!(f, "designed part not placed: {s}"),
        }
    }
}

impl std::error::Error for EchoError {}

/// The four acoustic windows the probe can start from (supine patient).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EchoWindow {
    /// Left sternal border, an intercostal space at the level of the left-heart valves; marker
    /// toward the right shoulder (a long-axis plane).
    Parasternal,
    /// Over the LV apex; marker toward the patient's left (a four-chamber plane).
    Apical,
    /// Below the xiphoid on the midline, aimed at the heart; marker to the patient's left.
    Subcostal,
    /// Above the manubrium, aimed down at the aortic valve; marker toward the head.
    Suprasternal,
}

impl EchoWindow {
    /// Every window, in [`EchoAnatomy::window_pose`] order.
    pub const ALL: [Self; 4] = [
        Self::Parasternal,
        Self::Apical,
        Self::Subcostal,
        Self::Suprasternal,
    ];

    /// Index in [`Self::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }
}

/// One valve's leaflets in rest coordinates: an oblique cone of segments from the disc's rim to a
/// common coaptation point `q` when closed, each segment swinging about its hinge toward the valve
/// axis as the valve opens. Evaluated in the half-plane through `q`'s axis line, so the closed
/// surface is watertight.
#[derive(Debug, Clone)]
pub struct LeafletValve {
    pub valve: Valve,
    /// Disc centre and downstream unit normal (atrium → ventricle, ventricle → artery).
    pub centre_mm: Vec3,
    pub normal: Vec3,
    u: Vec3,
    v: Vec3,
    /// Coaptation point: in-plane offset from the centre (u, v) and height downstream, mm.
    q_off: Vec2,
    q_h: f32,
    /// Rim distance from `q`'s axis per azimuth bin about it.
    rim: Vec<f32>,
    half_thickness: f32,
    open_angle: f32,
    /// No leaflet point lies downstream of this height or farther than `rho_max` from the
    /// coaptation axis (any opening).
    h_max: f32,
    rho_max: f32,
}

const RIM_BINS: usize = 72;
/// The subcostal window lies this far below the lowest midline sternal bone, mm. **Designed**.
const SUBCOSTAL_BELOW_XIPHOID_MM: f32 = 35.0;
/// The apical start pose is the candidate nearest the LV long axis among those whose start
/// sector has at most this share (in 2 % steps: 5 = 10 %) of its heart hidden by bone or lung, if
/// any (else the least hidden). **Designed** (B32 step 0: the long axis's own exit lies over a
/// rib, which hid about half the image).
const APICAL_CLEAR_SHARE: i32 = 5;
/// The suprasternal window lies at least this far above the highest midline sternal bone, mm
/// (searched up to 35 mm more, see `compute_windows`). **Designed**.
const SUPRASTERNAL_ABOVE_MANUBRIUM_MM: f32 = 15.0;

impl LeafletValve {
    /// Whether rest point `x` lies in a leaflet of this valve at `opening` (0 shut … 1 open).
    #[must_use]
    pub fn contains(&self, x: Vec3, opening: f32) -> bool {
        self.contains_posed(x, &self.pose(opening))
    }

    /// The leaflets at `opening`: per rim bin, the vector from the hinge to the tip in the
    /// half-plane coordinates (ρ outward from the coaptation axis, h downstream). Computed once
    /// per valve and instant; [`Self::contains_posed`] then needs one `atan2` per sample.
    #[must_use]
    pub fn pose(&self, opening: f32) -> Vec<Vec2> {
        let a_open = std::f32::consts::FRAC_PI_2 + self.open_angle;
        self.rim
            .iter()
            .map(|&r| {
                let closed = Vec2::new(-r, self.q_h);
                let len = closed.length();
                let a_closed = closed.y.atan2(closed.x);
                let a = a_closed + (a_open - a_closed) * opening.clamp(0.0, 1.0);
                Vec2::new(a.cos(), a.sin()) * len
            })
            .collect()
    }

    /// [`Self::contains`] with the leaflets already posed by [`Self::pose`].
    #[must_use]
    pub fn contains_posed(&self, x: Vec3, segs: &[Vec2]) -> bool {
        let d = x - self.centre_mm;
        let h = d.dot(self.normal);
        if h < -self.half_thickness || h > self.h_max {
            return false;
        }
        let w = Vec2::new(d.dot(self.u), d.dot(self.v)) - self.q_off;
        let rho = w.length();
        if rho > self.rho_max {
            return false;
        }
        let f = (w.y.atan2(w.x) / std::f32::consts::TAU).rem_euclid(1.0) * RIM_BINS as f32;
        let i = f as usize % RIM_BINS;
        let j = (i + 1) % RIM_BINS;
        let t = f - f.floor();
        let r = self.rim[i] * (1.0 - t) + self.rim[j] * t;
        let seg = segs[i] * (1.0 - t) + segs[j] * t;
        let hinge = Vec2::new(r, 0.0);
        let p = Vec2::new(rho, h);
        let k = ((p - hinge).dot(seg) / seg.length_squared().max(1e-9)).clamp(0.0, 1.0);
        (hinge + seg * k).distance_squared(p) <= self.half_thickness * self.half_thickness
    }

    /// Radius about the disc centre that holds every leaflet point at any opening, mm.
    #[must_use]
    pub fn bound_radius_mm(&self) -> f32 {
        self.q_off.length()
            + (self.rho_max * self.rho_max + self.h_max * self.h_max).sqrt()
            + self.half_thickness
    }

    /// The coaptation point of the closed valve, rest coordinates.
    #[must_use]
    pub fn coaptation_mm(&self) -> Vec3 {
        self.centre_mm + self.u * self.q_off.x + self.v * self.q_off.y + self.normal * self.q_h
    }

    /// Largest rim radius about the coaptation axis plus the longest leaflet, mm: no leaflet
    /// point is farther than this (plus half the thickness) from the coaptation axis's foot.
    #[must_use]
    pub fn reach_mm(&self) -> f32 {
        let rmax = self.rim.iter().copied().fold(0.0f32, f32::max);
        rmax + (rmax * rmax + self.q_h * self.q_h).sqrt()
    }

    /// The coaptation axis's foot on the disc plane (the centre moved by the in-plane offset).
    #[must_use]
    pub fn axis_foot_mm(&self) -> Vec3 {
        self.centre_mm + self.u * self.q_off.x + self.v * self.q_off.y
    }

    /// Mean rim radius about the disc centre, mm.
    #[must_use]
    pub fn mean_rim_mm(&self) -> f32 {
        self.rim.iter().sum::<f32>() / self.rim.len() as f32
    }
}

/// The designed parts of one instant: each valve's cone posed ([`LeafletValve::pose`], in
/// [`EchoAnatomy::valves`] order) and the measured valve posed ([`PosedMeshValve`]).
#[derive(Debug, Clone, Default)]
pub struct Posed {
    pub cones: Vec<Vec<Vec2>>,
    pub mesh: PosedMeshValve,
}

/// A tapered capsule (papillary muscle, moderator band) in rest coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    pub a_mm: Vec3,
    pub b_mm: Vec3,
    pub radius_a_mm: f32,
    pub radius_b_mm: f32,
}

impl Capsule {
    /// Whether rest point `x` is inside.
    #[must_use]
    pub fn contains(&self, x: Vec3) -> bool {
        let ab = self.b_mm - self.a_mm;
        let t = ((x - self.a_mm).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
        let r = self.radius_a_mm + (self.radius_b_mm - self.radius_a_mm) * t;
        (self.a_mm + ab * t).distance_squared(x) <= r * r
    }

    /// Length between the end centres, mm.
    #[must_use]
    pub fn length_mm(&self) -> f32 {
        self.a_mm.distance(self.b_mm)
    }
}

/// The static part of the echo cross-section (see the module docs). Built once per pack and
/// motion model; cheap to share (`&`), never mutated.
#[derive(Debug, Clone)]
pub struct EchoAnatomy {
    pub designed: DesignedParams,
    /// The heart box with the designed pericardium written into its outside voxels.
    heart: LabelVolume,
    thorax: LabelVolume,
    valves: Vec<LeafletValve>,
    /// The pack's measured valve (the tricuspid, brief B35), which replaces that valve's designed
    /// cone in the labels; `None` for a pack without one.
    mesh_valve: Option<MeshValve>,
    /// Anterolateral, posteromedial.
    papillary: [Capsule; 2],
    moderator: Capsule,
    skin: Skin,
    windows: [ProbePose; 4],
    field_template: DisplacementField,
    respiratory_shift_mm: f64,
    /// Sagittal midline of the thorax at heart level (x, mm).
    midline_x_mm: f32,
    /// Bounding box of every leaflet at any opening (rest coordinates): blood samples outside
    /// it skip the leaflet tests.
    valves_lo: Vec3,
    valves_hi: Vec3,
    /// Per thorax (x, y) column, the front (largest z) of the pack heart widened by the
    /// retraction margin, mm; −∞ where the heart is not behind (see
    /// [`DesignedParams::lung_retraction_left_mm`]).
    lung_front: Vec<f32>,
}

/// Centroid of the voxels of `label`.
fn centroid_of(vol: &LabelVolume, label: Label) -> Option<Vec3> {
    let b = label as u8;
    let mut s = glam::DVec3::ZERO;
    let mut n = 0usize;
    for (idx, l) in vol.labels.iter().enumerate() {
        if *l == b {
            let [i, j, k] = vol.coords(idx);
            s += vol.centre(i, j, k).as_dvec3();
            n += 1;
        }
    }
    (n > 0).then(|| (s / n as f64).as_vec3())
}

/// March from `from` along `dir` (≤ `max_mm`, 0.25 mm steps) through voxels that pass `through`;
/// the first point that does not, with its label byte.
fn march(
    vol: &LabelVolume,
    from: Vec3,
    dir: Vec3,
    max_mm: f32,
    through: impl Fn(u8) -> bool,
) -> Option<(Vec3, u8)> {
    let mut s = 0.0;
    while s <= max_mm {
        let p = from + dir * s;
        let l = vol.get(p)?;
        if !through(l) {
            return Some((p, l));
        }
        s += 0.25;
    }
    None
}

impl EchoAnatomy {
    /// Build from the pack (which must carry its label volumes) and the motion model built from
    /// the same pack. The skin is the pack's human figure when present, else its torso.
    ///
    /// # Errors
    /// [`EchoError`] when the pack has no volumes, the model no valve discs, or a designed part
    /// cannot be placed.
    pub fn new(pack: &AnatomyPack, model: &MotionModel, designed: DesignedParams) -> Result<Self, EchoError> {
        let vols = pack.volumes.as_ref().ok_or(EchoError::NoVolumes)?;
        let mut heart = vols.heart.clone();
        let thorax = vols.thorax.clone();
        add_pericardium(&mut heart, designed.pericardium_mm);
        let blood = |l: Label| {
            centroid_of(&heart, l).ok_or_else(|| EchoError::Placement(format!("no {} voxels", l.name())))
        };
        let lv_c = blood(Label::LvBlood)?;
        let rv_c = blood(Label::RvBlood)?;
        let ao_c = blood(Label::AortaLumen)?;
        let pa_c = blood(Label::PulmonaryArteryLumen)?;
        let valves = leaflet_valves(model, &designed, [lv_c, rv_c, ao_c, pa_c])?;
        let mesh_valve = MeshValve::tricuspid(pack, model, &designed);
        let (mut valves_lo, mut valves_hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for v in &valves {
            let r = Vec3::splat(v.bound_radius_mm());
            valves_lo = valves_lo.min(v.centre_mm - r);
            valves_hi = valves_hi.max(v.centre_mm + r);
        }
        if let Some(m) = &mesh_valve {
            let (lo, hi) = m.bounds_mm();
            valves_lo = valves_lo.min(lo);
            valves_hi = valves_hi.max(hi);
        }
        let papillary = papillary_muscles(pack, model, &heart, &designed)?;
        let moderator = moderator_band(model, &heart, lv_c, &designed)?;
        let skin_mesh = pack.body.as_ref().unwrap_or(&pack.torso);
        let skin = Skin::new(skin_mesh, &designed);
        let (lo, hi) = (heart.origin_mm, heart.max_mm());
        let field_template = DisplacementField::new(model, &heart, lo, hi);
        let midline_x_mm = thorax_midline(&thorax);
        let lung_front = heart_front(&heart, &thorax, designed.lung_retraction_margin_mm);
        let mut me = Self {
            designed,
            heart,
            thorax,
            valves,
            mesh_valve,
            papillary,
            moderator,
            skin,
            windows: [ProbePose::default(); 4],
            field_template,
            respiratory_shift_mm: model.params().respiratory_shift_mm,
            midline_x_mm,
            lung_front,
            valves_lo,
            valves_hi,
        };
        me.windows = me.compute_windows(pack, model, lv_c, rv_c)?;
        Ok(me)
    }

    /// The heart-box labels at rest (with the pericardium).
    #[must_use]
    pub fn heart_volume(&self) -> &LabelVolume {
        &self.heart
    }

    /// The thorax-box labels at rest.
    #[must_use]
    pub fn thorax_volume(&self) -> &LabelVolume {
        &self.thorax
    }

    /// The leaflets of each valve, in [`Valve::ALL`] order (the designed cones; for the valve the
    /// pack has a measured mesh of, the cone still gives the valve's centre, axis and reach but is
    /// not drawn: [`Self::mesh_valve`] is).
    #[must_use]
    pub fn valves(&self) -> &[LeafletValve] {
        &self.valves
    }

    /// The measured valve that replaces its designed cone in the labels (brief B35), if any.
    #[must_use]
    pub fn mesh_valve(&self) -> Option<&MeshValve> {
        self.mesh_valve.as_ref()
    }

    /// The designed parts posed at `openings` (per [`Valve::index`]): what
    /// [`Self::heart_label_posed`] reads.
    #[must_use]
    pub fn pose(&self, openings: &[f32; 4]) -> Posed {
        let mut p = Posed::default();
        self.repose(openings, &mut p);
        p
    }

    /// [`Self::pose`] into an existing value (the mesh valve is re-posed only when its opening
    /// changed).
    pub fn repose(&self, openings: &[f32; 4], posed: &mut Posed) {
        posed.cones.clear();
        posed
            .cones
            .extend(self.valves.iter().map(|v| v.pose(openings[v.valve.index()])));
        if let Some(m) = &self.mesh_valve {
            posed.mesh.update(m, openings[m.valve_id.index()]);
        }
    }

    /// The anterolateral and posteromedial papillary muscles (rest coordinates).
    #[must_use]
    pub fn papillary_muscles(&self) -> &[Capsule; 2] {
        &self.papillary
    }

    /// The moderator band (rest coordinates).
    #[must_use]
    pub fn moderator_band(&self) -> &Capsule {
        &self.moderator
    }

    /// The skin the probe slides on.
    #[must_use]
    pub fn skin(&self) -> &Skin {
        &self.skin
    }

    /// Sagittal midline of the thorax at heart level, x in mm.
    #[must_use]
    pub fn midline_x_mm(&self) -> f32 {
        self.midline_x_mm
    }

    /// The starting pose of a window (computed from the landmarks and the baked ribs).
    #[must_use]
    pub fn window_pose(&self, w: EchoWindow) -> ProbePose {
        self.windows[w.index()]
    }

    /// The pose with its skin point moved to the skin point nearest `p_mm` (and the normal
    /// there), keeping its angles, depth and sector: what dragging the probe does.
    #[must_use]
    pub fn slide_to(&self, pose: &ProbePose, p_mm: Vec3) -> ProbePose {
        match self.skin.place(p_mm) {
            Some((q, n)) => ProbePose {
                skin_mm: q,
                normal: n,
                ..*pose
            },
            None => *pose,
        }
    }

    pub(crate) fn field_template(&self) -> &DisplacementField {
        &self.field_template
    }

    pub(crate) fn respiratory_shift_mm(&self) -> f64 {
        self.respiratory_shift_mm
    }

    /// The label at rest point `x` from the heart box and the designed parts (`openings` per
    /// [`Valve::index`]), or `None` where the heart box has nothing (the thorax decides there).
    #[must_use]
    pub fn heart_label_at(&self, x: Vec3, openings: &[f32; 4]) -> Option<Label> {
        self.heart_label_posed(x, &self.pose(openings))
    }

    /// [`Self::heart_label_at`] with the leaflets already posed ([`Self::pose`]).
    #[must_use]
    pub fn heart_label_posed(&self, x: Vec3, posed: &Posed) -> Option<Label> {
        let l = self.heart.get(x)?;
        if l == Label::Outside as u8 {
            return None;
        }
        let label = Label::from_u8(l)?;
        if label.is_blood()
            && x.cmpge(self.valves_lo).all()
            && x.cmple(self.valves_hi).all()
            && self.leaflet_at(x, posed)
        {
            return Some(Label::ValveLeaflet);
        }
        if label.is_blood() {
            if label == Label::LvBlood && self.papillary.iter().any(|c| c.contains(x)) {
                return Some(Label::PapillaryMuscle);
            }
            if label == Label::RvBlood && self.moderator.contains(x) {
                return Some(Label::ModeratorBand);
            }
        }
        Some(label)
    }

    /// Whether rest point `x` is in a posed leaflet: the measured valve's mesh for its valve, the
    /// designed cones for the others (no blood or bounds check).
    #[must_use]
    pub fn leaflet_at(&self, x: Vec3, posed: &Posed) -> bool {
        let skip = self.mesh_valve.as_ref().map(|m| m.valve_id);
        for (v, segs) in self.valves.iter().zip(&posed.cones) {
            if Some(v.valve) != skip && v.contains_posed(x, segs) {
                return true;
            }
        }
        self.mesh_valve
            .as_ref()
            .is_some_and(|m| posed.mesh.contains(m, x))
    }

    /// The baked thorax label at `x` (already moved back by the thorax's displacement), without
    /// the lung-edge retraction.
    #[must_use]
    pub fn thorax_label_at(&self, x: Vec3) -> Label {
        self.thorax.label_at(x)
    }

    /// The thorax label at `x` with the designed lung retraction at `scale` (1 = expiration;
    /// smaller in inspiration, see [`DesignedParams::lung_retraction_inspiration_loss`]; 0 = the
    /// baked lungs): lung reads as soft tissue where it lies in front of the heart's anterior
    /// silhouette in the parasternal band (the cardiac notch), whose left edge moves to
    /// `scale ×` [`DesignedParams::lung_retraction_left_mm`] (its right edge, behind the
    /// sternum, stays). Bone is never changed.
    #[must_use]
    pub fn thorax_label_retracted(&self, x: Vec3, scale: f32) -> Label {
        let l = self.thorax.label_at(x);
        if l != Label::Lung || scale <= 0.0 {
            return l;
        }
        if self.in_retraction_band(x, scale) {
            Label::SoftTissue
        } else {
            l
        }
    }

    /// Whether `x` lies in the parasternal band (in front of the heart's anterior silhouette).
    fn in_retraction_band(&self, x: Vec3, scale: f32) -> bool {
        let d = &self.designed;
        if d.lung_retraction_left_mm + d.lung_retraction_right_mm <= 0.0 {
            return false;
        }
        // the band's left edge (the lung's anterior border over the heart) comes back over the
        // heart in inspiration; its right edge, behind the sternum, does not move
        let dx = x.x - self.midline_x_mm;
        if dx < -d.lung_retraction_right_mm || dx > d.lung_retraction_left_mm * scale {
            return false;
        }
        let t = &self.thorax;
        let q = (x - t.origin_mm) / t.spacing_mm;
        let (i, j) = (q.x as usize, q.y as usize);
        if q.x < 0.0 || q.y < 0.0 || i >= t.dims[0] as usize || j >= t.dims[1] as usize {
            return false;
        }
        x.z > self.lung_front[i + t.dims[0] as usize * j] - d.lung_retraction_behind_mm
    }

    /// Whether the straight segment `a → b` passes through `label` in the thorax box (1 mm steps).
    #[must_use]
    pub fn segment_crosses(&self, a: Vec3, b: Vec3, label: Label) -> bool {
        let n = (a.distance(b).ceil() as usize).max(1);
        (0..=n).any(|i| self.thorax.label_at(a.lerp(b, i as f32 / n as f32)) == label)
    }

    /// 1 mm samples of `label` on `a → b`, with the lung retraction at full width (the windows
    /// are placed in expiration).
    fn samples_of(&self, a: Vec3, b: Vec3, label: Label) -> usize {
        let n = (a.distance(b).ceil() as usize).max(1);
        (0..=n)
            .filter(|&i| self.thorax_label_retracted(a.lerp(b, i as f32 / n as f32), 1.0) == label)
            .count()
    }

    /// Heart samples of `pose`'s sector at rest that lie behind the first bone or lung sample of
    /// their line, and all its heart samples (32 lines, 1 mm steps, band retraction at full width;
    /// the coarse form of [`crate::SliceState::slice`]'s occlusion, used to place the windows).
    fn sector_hidden_at_rest(&self, pose: &ProbePose) -> (usize, usize) {
        const LINES: usize = 32;
        let f = pose.frame(self.designed.contact_limit_deg);
        if !f.contact {
            return (0, 0);
        }
        let (mut hidden, mut total) = (0usize, 0usize);
        let steps = pose.depth_mm as usize;
        for line in 0..LINES {
            let dir = f.line_direction(line, LINES, pose.sector_deg);
            let mut behind = false;
            for k in 0..steps {
                let x = f.apex_mm + dir * (k as f32 + 0.5);
                match self.heart.get(x) {
                    Some(h) if h != Label::Outside as u8 => {
                        if h != Label::Pericardium as u8 {
                            total += 1;
                            hidden += usize::from(behind);
                        }
                    }
                    _ => behind |= self.thorax_label_retracted(x, 1.0).occludes(),
                }
            }
        }
        (hidden, total)
    }

    /// The share (0 … 1) of the heart samples of `pose`'s sector, at rest and with the lung edge at
    /// its expiration position, that lie behind bone or lung (the respiration-independent window
    /// quality of the view targets, brief B33). 0 without contact.
    #[must_use]
    pub fn hidden_share_at_rest(&self, pose: &ProbePose) -> f32 {
        let (hidden, total) = self.sector_hidden_at_rest(&pose.clamped());
        if total == 0 {
            0.0
        } else {
            hidden as f32 / total as f32
        }
    }

    #[allow(clippy::too_many_lines)]
    fn compute_windows(
        &self,
        pack: &AnatomyPack,
        model: &MotionModel,
        lv_c: Vec3,
        rv_c: Vec3,
    ) -> Result<[ProbePose; 4], EchoError> {
        let d = &self.designed;
        let limit = d.contact_limit_deg;
        let frame = model.frame();
        let mitral = frame.valve_centres_mm[Valve::Mitral.index()];
        let aortic = frame.valve_centres_mm[Valve::Aortic.index()];
        let apex = frame.axis(MotionChamber::LeftVentricle).origin_mm;
        let mid = self.midline_x_mm;
        let pose = |skin: Vec3, n: Vec3, target: Vec3, marker: Vec3| {
            ProbePose::aimed(
                skin,
                n,
                target,
                marker,
                limit,
                DEFAULT_DEPTH_MM,
                DEFAULT_SECTOR_DEG,
            )
        };
        // the anterior skin point straight in front of (x, y)
        let front = |x: f32, y: f32| self.skin.ray_exit(Vec3::new(x, y, -40.0), Vec3::Z);
        // what the straight path to the target crosses (1 mm samples of bone, then of lung)
        let wall = |skin: Vec3, target: Vec3| {
            (
                self.samples_of(skin, target, Label::Bone),
                self.samples_of(skin, target, Label::Lung),
            )
        };

        // parasternal: left sternal border, the rib-free spot nearest the valves' level
        let target = (mitral + aortic) * 0.5;
        let mut best: Option<((usize, usize, i32), Vec3, Vec3)> = None;
        for dx in [15.0f32, 25.0, 35.0, 45.0] {
            for k in -12..=12 {
                let y = target.y + 3.0 * k as f32;
                let Some((p, n)) = front(mid + dx, y) else {
                    continue;
                };
                let (bone, lung) = wall(p, target);
                let key = (bone, lung, (y - target.y).abs().round() as i32);
                if best.as_ref().is_none_or(|b| key < b.0) {
                    best = Some((key, p, n));
                }
            }
        }
        let (_, p, n) =
            best.ok_or_else(|| EchoError::Placement("parasternal: no skin in front of the heart".into()))?;
        // long-axis plane: the marker toward the base and the right shoulder
        let parasternal = pose(p, n, target, aortic - apex);

        // apical: where the LV long axis leaves the chest, moved to the nearest rib-free spot
        let axis = (apex - mitral).normalize();
        let (e1, e2) = axis.any_orthonormal_pair();
        let mut best: Option<((i32, usize, usize, i32), ProbePose)> = None;
        for a in -5..=5 {
            for b in -5..=5 {
                let off = e1 * (5.0 * a as f32) + e2 * (5.0 * b as f32);
                let dir = (apex + off - mitral).normalize();
                let Some((p, n)) = self.skin.ray_exit(mitral, dir) else {
                    continue;
                };
                // four-chamber plane: the marker toward the left ventricle's lateral side
                let cand = pose(p, n, mitral, lv_c - rv_c);
                // the share of the whole start sector's heart that bone or lung hide (in 2 %
                // steps, 10 % or less counting as clear), then the central line's bone and lung,
                // then the nearest to the long axis
                let (hidden, total) = self.sector_hidden_at_rest(&cand);
                if total == 0 {
                    continue;
                }
                let share = i32::try_from(50 * hidden / total)
                    .unwrap_or(i32::MAX)
                    .max(APICAL_CLEAR_SHARE);
                let (bone, lung) = wall(p, mitral);
                let key = (share, bone, lung, a * a + b * b);
                if best.as_ref().is_none_or(|bb| key < bb.0) {
                    best = Some((key, cand));
                }
            }
        }
        let (_, apical) =
            best.ok_or_else(|| EchoError::Placement("apical: the long axis does not reach the skin".into()))?;

        // sternum: midline bone in front of the heart; xiphoid its lowest point, manubrium top
        let mut sternum: Option<(f32, f32)> = None;
        let heart_front = pack
            .cells
            .positions
            .iter()
            .map(|p| p.z)
            .fold(f32::NEG_INFINITY, f32::max);
        let t = &self.thorax;
        for idx in 0..t.len() {
            if t.labels[idx] != Label::Bone as u8 {
                continue;
            }
            let [i, j, k] = t.coords(idx);
            let c = t.centre(i, j, k);
            if (c.x - mid).abs() < 12.0 && c.z > heart_front - 10.0 {
                sternum = Some(sternum.map_or((c.y, c.y), |(lo, hi)| (lo.min(c.y), hi.max(c.y))));
            }
        }
        let (xiphoid_y, manubrium_y) = sternum
            .ok_or_else(|| EchoError::Placement("no sternum on the midline in front of the heart".into()))?;
        let heart_c = (lv_c + rv_c) * 0.5;
        // subcostal: below the xiphoid (designed: 35 mm, through the left lobe of the liver),
        // aimed at the heart, marker to the patient's left
        let (p, n) = front(mid, xiphoid_y - SUBCOSTAL_BELOW_XIPHOID_MM)
            .ok_or_else(|| EchoError::Placement("subcostal: no skin below the xiphoid".into()))?;
        let subcostal = pose(p, n, heart_c, Vec3::X);
        // suprasternal: above the manubrium, aimed down at the aortic valve, marker to the head;
        // the height (and a centimetre either side) whose actual beam crosses the least bone,
        // then lung, preferring the designed height
        let mut best: Option<((usize, usize, i32), ProbePose)> = None;
        for k in 0..8 {
            let above = SUPRASTERNAL_ABOVE_MANUBRIUM_MM + 5.0 * k as f32;
            for dx in [-10.0f32, 0.0, 10.0] {
                let Some((p, n)) = front(mid + dx, manubrium_y + above) else {
                    continue;
                };
                let cand = pose(p, n, aortic, Vec3::Y);
                let f = cand.frame(limit);
                let end = p + f.beam * p.distance(aortic);
                let key = (
                    self.samples_of(p, end, Label::Bone),
                    self.samples_of(p, end, Label::Lung),
                    k + dx.abs() as i32,
                );
                if best.as_ref().is_none_or(|b| key < b.0) {
                    best = Some((key, cand));
                }
            }
        }
        let (_, suprasternal) =
            best.ok_or_else(|| EchoError::Placement("suprasternal: no skin above the manubrium".into()))?;
        Ok([parasternal, apical, subcostal, suprasternal])
    }
}

/// Per thorax (x, y) column, the largest z of any heart-box voxel that is not outside, widened in
/// x and y by `margin_mm`; −∞ where there is none.
#[allow(clippy::cast_possible_wrap)] // grid dimensions are a few hundred
fn heart_front(heart: &LabelVolume, thorax: &LabelVolume, margin_mm: f32) -> Vec<f32> {
    let [nx, ny] = [thorax.dims[0] as usize, thorax.dims[1] as usize];
    let mut front = vec![f32::NEG_INFINITY; nx * ny];
    for idx in 0..heart.len() {
        if heart.labels[idx] == Label::Outside as u8 {
            continue;
        }
        let [i, j, k] = heart.coords(idx);
        let c = heart.centre(i, j, k);
        let q = (c - thorax.origin_mm) / thorax.spacing_mm;
        if q.x < 0.0 || q.y < 0.0 || q.x as usize >= nx || q.y as usize >= ny {
            continue;
        }
        let col = q.x as usize + nx * q.y as usize;
        front[col] = front[col].max(c.z);
    }
    let r = (margin_mm / thorax.spacing_mm).ceil() as isize;
    let r2 = (margin_mm / thorax.spacing_mm).powi(2);
    let mut out = front.clone();
    for j in 0..ny as isize {
        for i in 0..nx as isize {
            let mut m = f32::NEG_INFINITY;
            for dj in -r..=r {
                for di in -r..=r {
                    if (di * di + dj * dj) as f32 > r2 + 1e-4 {
                        continue;
                    }
                    let (a, b) = (i + di, j + dj);
                    if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny {
                        m = m.max(front[a as usize + nx * b as usize]);
                    }
                }
            }
            out[i as usize + nx * j as usize] = m;
        }
    }
    out
}

/// Thorax midline at heart level: the centre of the inside-the-skin extent on the row y ≈ 0.
fn thorax_midline(t: &LabelVolume) -> f32 {
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for idx in 0..t.len() {
        if t.labels[idx] == Label::Outside as u8 {
            continue;
        }
        let [i, j, k] = t.coords(idx);
        let c = t.centre(i, j, k);
        if c.y.abs() < 10.0 {
            lo = lo.min(c.x);
            hi = hi.max(c.x);
        }
    }
    if lo.is_finite() {
        f32::midpoint(lo, hi)
    } else {
        0.0
    }
}

/// Write the pericardium (designed, `thickness_mm`) into the outside voxels of the heart box that
/// lie within that distance of heart tissue.
#[allow(clippy::cast_possible_wrap)] // grid dimensions are a few hundred
fn add_pericardium(heart: &mut LabelVolume, thickness_mm: f32) {
    let r = thickness_mm / heart.spacing_mm;
    let k = r.ceil() as i32;
    let mut offsets: Vec<(i32, i32, i32)> = Vec::new();
    for dz in -k..=k {
        for dy in -k..=k {
            for dx in -k..=k {
                let d2 = (dx * dx + dy * dy + dz * dz) as f32;
                if d2 > 0.0 && d2 <= r * r + 1e-4 {
                    offsets.push((dx, dy, dz));
                }
            }
        }
    }
    let [nx, ny, nz] = heart.dims.map(|d| d as i32);
    let src = heart.labels.clone();
    let outside = Label::Outside as u8;
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let idx = (x + nx * (y + ny * z)) as usize;
                if src[idx] != outside {
                    continue;
                }
                let touches = offsets.iter().any(|&(dx, dy, dz)| {
                    let (a, b, c) = (x + dx, y + dy, z + dz);
                    a >= 0
                        && b >= 0
                        && c >= 0
                        && a < nx
                        && b < ny
                        && c < nz
                        && src[(a + nx * (b + ny * c)) as usize] != outside
                });
                if touches {
                    heart.labels[idx] = Label::Pericardium as u8;
                }
            }
        }
    }
}

/// Leaflets on the B30 valve discs; `blood` = LV, RV, aorta, PA lumen centroids (downstream sides).
fn leaflet_valves(
    model: &MotionModel,
    d: &DesignedParams,
    blood: [Vec3; 4],
) -> Result<Vec<LeafletValve>, EchoError> {
    let discs = model.valve_discs();
    let rest = model.part_rest("valve_planes").ok_or(EchoError::NoValves)?;
    if discs.len() != 4 {
        return Err(EchoError::NoValves);
    }
    let centres = model.frame().valve_centres_mm;
    let mut out = Vec::with_capacity(4);
    for valve in Valve::ALL {
        let disc = discs
            .iter()
            .find(|x| x.valve == valve)
            .ok_or(EchoError::NoValves)?;
        let pts: Vec<Vec3> = disc
            .vertices
            .iter()
            .map(|&i| Vec3::from(rest[i as usize].position))
            .collect();
        let c = disc.centre_mm;
        let downstream = match valve {
            Valve::Mitral => blood[0],
            Valve::Tricuspid => blood[1],
            Valve::Aortic => blood[2],
            Valve::Pulmonic => blood[3],
        };
        let mut n = smallest_axis(&pts, c);
        if n.dot(downstream - c) < 0.0 {
            n = -n;
        }
        let (u, v) = n.any_orthonormal_pair();
        let to2 = |p: Vec3| Vec2::new((p - c).dot(u), (p - c).dot(v));
        // the coaptation point
        let rim_about = |o: Vec2| -> Vec<f32> {
            let mut rim = vec![0.0f32; RIM_BINS];
            for p in &pts {
                let w = to2(*p) - o;
                let b = ((w.y.atan2(w.x) / std::f32::consts::TAU).rem_euclid(1.0) * RIM_BINS as f32) as usize
                    % RIM_BINS;
                rim[b] = rim[b].max(w.length());
            }
            // fill empty bins from their neighbours
            for _ in 0..RIM_BINS {
                let snapshot = rim.clone();
                for b in 0..RIM_BINS {
                    if rim[b] == 0.0 {
                        let l = snapshot[(b + RIM_BINS - 1) % RIM_BINS];
                        let r = snapshot[(b + 1) % RIM_BINS];
                        rim[b] = l.max(r);
                    }
                }
            }
            rim
        };
        let centred = rim_about(Vec2::ZERO);
        let at = |rim: &[f32], dir: Vec2| {
            let b = ((dir.y.atan2(dir.x) / std::f32::consts::TAU).rem_euclid(1.0) * RIM_BINS as f32) as usize
                % RIM_BINS;
            rim[b]
        };
        let (q_off, q_h, half) = match valve {
            Valve::Mitral => {
                // anterior leaflet on the aortic side; solve the closed triangle in that plane
                let ant = to2(centres[Valve::Aortic.index()]).normalize_or(Vec2::X);
                let (ra, rp) = (at(&centred, ant), at(&centred, -ant));
                let la = (d.mitral_anterior_length_mm - d.av_coaptation_mm).max(1.0);
                let lp = (d.mitral_posterior_length_mm - d.av_coaptation_mm).max(1.0);
                let dd = ra + rp;
                let x = (lp * lp - la * la + dd * dd) / (2.0 * dd);
                let h2 = lp * lp - x * x;
                let (x, h) = if h2 > 0.0 && x > 0.0 && x < dd {
                    (x, h2.sqrt())
                } else {
                    (dd * lp / (la + lp), 0.0)
                };
                (ant * (x - rp), h, d.av_leaflet_thickness_mm * 0.5)
            }
            Valve::Tricuspid => {
                let r = centred.iter().sum::<f32>() / RIM_BINS as f32;
                let l = (d.tricuspid_height_mm - d.av_coaptation_mm).max(1.0);
                (
                    Vec2::ZERO,
                    (l * l - r * r).max(0.0).sqrt(),
                    d.av_leaflet_thickness_mm * 0.5,
                )
            }
            Valve::Aortic => (
                Vec2::ZERO,
                d.aortic_effective_height_mm,
                d.cusp_thickness_mm * 0.5,
            ),
            Valve::Pulmonic => (
                Vec2::ZERO,
                d.pulmonary_effective_height_mm,
                d.cusp_thickness_mm * 0.5,
            ),
        };
        let rim = rim_about(q_off);
        let rmax = rim.iter().copied().fold(0.0f32, f32::max);
        let len_max = (rmax * rmax + q_h * q_h).sqrt();
        out.push(LeafletValve {
            valve,
            centre_mm: c,
            normal: n,
            u,
            v,
            q_off,
            q_h,
            rim,
            half_thickness: half.max(0.5),
            open_angle: d.open_angle_from_axis_deg.to_radians(),
            h_max: len_max + half.max(0.5),
            rho_max: rmax + half.max(0.5),
        });
    }
    Ok(out)
}

/// Unit eigenvector of the smallest spread of `pts` about `c` (inverse power iteration).
fn smallest_axis(pts: &[Vec3], c: Vec3) -> Vec3 {
    let mut m = glam::Mat3::ZERO;
    for p in pts {
        let q = *p - c;
        m += glam::Mat3::from_cols(q * q.x, q * q.y, q * q.z);
    }
    let shift = m.col(0).x + m.col(1).y + m.col(2).z + 1.0;
    let a = glam::Mat3::from_diagonal(Vec3::splat(shift)) - m;
    let mut v = Vec3::new(0.577, 0.577, 0.577);
    for _ in 0..200 {
        v = (a * v).normalize_or(Vec3::Z);
    }
    v
}

/// Two papillary muscles: base on the endocardium at mid level on the AHA 11|12 and 9|10 seams,
/// tip at the sourced distance from the mitral annulus.
fn papillary_muscles(
    pack: &AnatomyPack,
    model: &MotionModel,
    heart: &LabelVolume,
    d: &DesignedParams,
) -> Result<[Capsule; 2], EchoError> {
    let ax = model.frame().axis(MotionChamber::LeftVentricle);
    let mitral = model.frame().valve_centres_mm[Valve::Mitral.index()];
    let (e1, e2) = ax.axis.any_orthonormal_pair();
    let cells = &pack.cells;
    let angle_mean = |segs: &[u8]| -> Option<f32> {
        let (mut s, mut c, mut n) = (0.0f32, 0.0f32, 0usize);
        for (i, p) in cells.positions.iter().enumerate() {
            if segs.contains(&pack.aha_segments.get(i).copied().unwrap_or(0))
                && matches!(
                    cells.regions[i],
                    Region::LeftVentricle | Region::VentricularSeptum
                )
            {
                let r = ax.coords(*p).radial;
                let a = r.dot(e2).atan2(r.dot(e1));
                s += a.sin();
                c += a.cos();
                n += 1;
            }
        }
        (n > 0).then(|| s.atan2(c))
    };
    let mid_height = {
        let (mut s, mut n) = (0.0f32, 0usize);
        for (i, p) in cells.positions.iter().enumerate() {
            if (7..=12).contains(&pack.aha_segments.get(i).copied().unwrap_or(0)) {
                s += ax.coords(*p).height;
                n += 1;
            }
        }
        if n == 0 {
            return Err(EchoError::Placement(
                "papillary muscles: no mid-level AHA segments".into(),
            ));
        }
        s / n as f32
    };
    let seam = |a: u8, b: u8| -> Option<f32> {
        let (x, y) = (angle_mean(&[a])?, angle_mean(&[b])?);
        Some((x.sin() + y.sin()).atan2(x.cos() + y.cos()))
    };
    let mut out = [Capsule {
        a_mm: Vec3::ZERO,
        b_mm: Vec3::ZERO,
        radius_a_mm: 0.0,
        radius_b_mm: 0.0,
    }; 2];
    for (k, (a, b)) in [(11u8, 12u8), (9u8, 10u8)].into_iter().enumerate() {
        let ang = seam(a, b)
            .ok_or_else(|| EchoError::Placement(format!("papillary muscles: no cells in AHA {a}/{b}")))?;
        let dir = e1 * ang.cos() + e2 * ang.sin();
        let from = ax.point_at(mid_height);
        let (base, l) = march(heart, from, dir, 60.0, |l| l == Label::LvBlood as u8)
            .ok_or_else(|| EchoError::Placement("papillary muscle: the radius leaves the box".into()))?;
        if l != Label::LvMyocardium as u8 {
            return Err(EchoError::Placement(format!(
                "papillary muscle {k}: the radius at mid level meets {:?}, not the LV wall",
                Label::from_u8(l)
            )));
        }
        let tip = mitral + (base - mitral).normalize() * d.papillary_tip_to_annulus_mm[k];
        let r = d.papillary_base_diameter_mm[k] * 0.5;
        out[k] = Capsule {
            a_mm: base + dir * d.papillary_embed_mm,
            b_mm: tip,
            radius_a_mm: r,
            radius_b_mm: r * d.papillary_tip_fraction,
        };
    }
    Ok(out)
}

/// The moderator band: from the septum at the sourced fraction of the tricuspid→apex distance to
/// the free wall further toward the apex.
fn moderator_band(
    model: &MotionModel,
    heart: &LabelVolume,
    lv_c: Vec3,
    d: &DesignedParams,
) -> Result<Capsule, EchoError> {
    let ax = model.frame().axis(MotionChamber::RightVentricle);
    let tv = ax.centre_mm;
    let apex = ax.origin_mm;
    let at = |f: f32| tv + (apex - tv) * f;
    let axis = (tv - apex).normalize();
    let toward_lv = {
        let p = at(d.moderator_septal_fraction);
        let w = lv_c - p;
        (w - axis * w.dot(axis)).normalize_or(Vec3::X)
    };
    let in_rv = |l: u8| l == Label::RvBlood as u8;
    let (s, ls) = march(heart, at(d.moderator_septal_fraction), toward_lv, 60.0, in_rv)
        .ok_or_else(|| EchoError::Placement("moderator band: no septum".into()))?;
    let wall = |l: u8| l == Label::RvMyocardium as u8 || l == Label::LvMyocardium as u8;
    if !wall(ls) {
        return Err(EchoError::Placement(format!(
            "moderator band starts on {:?}, not on the septum",
            Label::from_u8(ls)
        )));
    }
    // the insertion: the nearest RV free wall across the cavity from the origin, within ±50° of
    // the direction away from the septum and tilted toward the apex by up to
    // `moderator_max_apical_tilt` (the band runs down to the anterior papillary muscle)
    let away = -toward_lv;
    let side = axis.cross(away).normalize_or(Vec3::Y);
    let start = s - toward_lv * 0.5;
    let mut best: Option<(f32, Vec3)> = None;
    for k in 0..=4 {
        let tilt = d.moderator_max_apical_tilt * k as f32 / 4.0;
        for a in -5..=5 {
            let th = (10.0 * a as f32).to_radians();
            let dir = (away * th.cos() + side * th.sin() - axis * tilt).normalize();
            if let Some((p, l)) = march(heart, start + dir * 0.5, dir, 60.0, in_rv)
                && l == Label::RvMyocardium as u8
            {
                let len = p.distance(s);
                if best.is_none_or(|b| len < b.0) {
                    best = Some((len, p));
                }
            }
        }
    }
    let (_, w) =
        best.ok_or_else(|| EchoError::Placement("moderator band: no RV free wall across the cavity".into()))?;
    let r = d.moderator_thickness_mm * 0.5;
    let dir = (w - s).normalize_or(Vec3::X);
    Ok(Capsule {
        a_mm: s - dir * 1.0,
        b_mm: w + dir * 1.0,
        radius_a_mm: r,
        radius_b_mm: r,
    })
}
