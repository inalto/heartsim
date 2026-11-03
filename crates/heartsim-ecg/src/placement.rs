//! Electrode placement (brief B10): where the electrode on the end of each of the ten cables sits,
//! the optional extra chest electrodes, presets and misplacement exercises, snapping to chest-wall
//! landmarks, and how a placement changes the leads of both tiers.
//!
//! **Positions are the whole state.** A cable's electrode is either on the torso (a point
//! electrode) or on a distal limb. Distal-limb electrodes are stored at a nominal wrist / ankle
//! position outside the torso ([`Limb::anchor_mm`]), so a cable reversal — "the RA cable is on
//! the left wrist" — is just two exchanged positions and the scenario file needs nothing but
//! `electrodes_mm`.
//!
//! **Tier 1** re-places the forward model's electrodes ([`Placement::apply_to_forward`]): torso
//! sites become point electrodes, distal limbs far-field electrodes at their Einthoven angle.
//!
//! **Tier 0** has no cells, only a heart dipole projected on lead vectors, so a placement is turned
//! into lead vectors ([`Tier0Field`]): every electrode site gets an electrode vector `E(r)` such
//! that its potential is `E(r)·p`. Distal limbs give Einthoven's `d̂/√3`; a torso point gives the
//! dipole field `k (r − c)/|r − c|³` (`c` = ventricular centroid) plus a correction that makes
//! the six standard precordial sites reproduce the textbook lead vectors exactly and fades out
//! within a few centimetres. Leads then follow from the usual Einthoven/Goldberger/Wilson algebra,
//! so the standard placement gives exactly [`LeadVectors::standard`] and moving or swapping an
//! electrode changes the strip the way the physics says it should.
//!
//! Everything here is pure and deterministic; the UI only edits a [`Placement`].

use crate::forward::ForwardModel;
use crate::leadvec::LeadVectors;
use crate::misplacement::Finding;
use glam::{DMat3, DVec3, Vec3};
use heartsim_core::{ELECTRODE_COUNT, Electrode, LEAD_COUNT, Lead, Scenario};
use heartsim_geometry::{ElectrodeSet, TriMesh};
use std::collections::BTreeMap;

/// Effective distance of the distal-limb junctions from the heart, mm — the Einthoven default of
/// the forward model ([`crate::LimbPlacement`]); it fixes the Tier-0 field scale `k = R²/√3`.
pub const EINTHOVEN_DISTANCE_MM: f64 = 125.0;
/// A position within this distance of a [`Limb::anchor_mm`] is "on that limb", mm.
pub const LIMB_CAPTURE_MM: f32 = 50.0;
/// Radius of the Tier-0 precordial correction, mm: beyond about two of these from every standard
/// precordial site an electrode sees the plain dipole field.
const CORRECTION_RADIUS_MM: f64 = 60.0;
/// Torso electrodes closer to the heart centre than this see it as if at this distance, mm.
const MIN_RADIUS_MM: f64 = 20.0;

/// The precordial electrodes in order.
pub const PRECORDIAL: [Electrode; 6] = [
    Electrode::V1,
    Electrode::V2,
    Electrode::V3,
    Electrode::V4,
    Electrode::V5,
    Electrode::V6,
];

// --- limbs ---------------------------------------------------------------------------------------

/// A distal limb. A limb is a linear conductor: an electrode anywhere on it records the potential
/// of the limb's junction with the trunk (Einthoven), so where on the wrist or ankle it sits does
/// not matter, and the two legs — joined at the pelvis — record the same potential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Limb {
    RightArm,
    LeftArm,
    LeftLeg,
    RightLeg,
}

impl Limb {
    pub const ALL: [Limb; 4] = [Limb::RightArm, Limb::LeftArm, Limb::LeftLeg, Limb::RightLeg];

    /// The limb a limb electrode belongs on (`None` for chest electrodes).
    #[must_use]
    pub fn of(e: Electrode) -> Option<Limb> {
        match e {
            Electrode::RA => Some(Limb::RightArm),
            Electrode::LA => Some(Limb::LeftArm),
            Electrode::LL => Some(Limb::LeftLeg),
            Electrode::RL => Some(Limb::RightLeg),
            _ => None,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Limb::RightArm => "right wrist",
            Limb::LeftArm => "left wrist",
            Limb::LeftLeg => "left ankle",
            Limb::RightLeg => "right ankle",
        }
    }

    /// Hexaxial angle of the limb's junction with the trunk: −150° (right arm), −30° (left arm),
    /// +90° (both legs) — the corners of Einthoven's triangle.
    #[must_use]
    pub fn hexaxial_deg(self) -> f32 {
        match self {
            Limb::RightArm => -150.0,
            Limb::LeftArm => -30.0,
            Limb::LeftLeg | Limb::RightLeg => 90.0,
        }
    }

    /// Nominal electrode position (wrists beside the hips and ankles, anatomical position), heart
    /// frame mm. Outside the torso; it only stores "on this limb" as a position.
    #[must_use]
    pub fn anchor_mm(self) -> Vec3 {
        match self {
            Limb::RightArm => Vec3::new(-265.0, -330.0, -35.0),
            Limb::LeftArm => Vec3::new(195.0, -330.0, -35.0),
            Limb::LeftLeg => Vec3::new(40.0, -1150.0, -35.0),
            Limb::RightLeg => Vec3::new(-110.0, -1150.0, -35.0),
        }
    }

    /// The limb whose anchor is within [`LIMB_CAPTURE_MM`] of `p`.
    #[must_use]
    pub fn at(p: Vec3) -> Option<Limb> {
        Limb::ALL
            .into_iter()
            .find(|l| l.anchor_mm().distance(p) <= LIMB_CAPTURE_MM)
    }

    /// Unit direction of the junction in the frontal plane.
    fn direction(self) -> DVec3 {
        LeadVectors::frontal(f64::from(self.hexaxial_deg()))
    }
}

// --- extra electrodes ----------------------------------------------------------------------------

/// Additional unipolar chest electrodes: right-sided (V3R, V4R — right-ventricular infarction,
/// children) and posterior (V7–V9 — posterior infarction). Each gives a lead `φ − WCT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExtraLead {
    V3R,
    V4R,
    V7,
    V8,
    V9,
}

impl ExtraLead {
    pub const ALL: [ExtraLead; 5] = [
        ExtraLead::V3R,
        ExtraLead::V4R,
        ExtraLead::V7,
        ExtraLead::V8,
        ExtraLead::V9,
    ];

    /// Site name in the anatomy pack's electrode set (`heartsim_geometry::electrodes::EXTRA_SITES`).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ExtraLead::V3R => "V3R",
            ExtraLead::V4R => "V4R",
            ExtraLead::V7 => "V7",
            ExtraLead::V8 => "V8",
            ExtraLead::V9 => "V9",
        }
    }

    #[must_use]
    pub fn from_label(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|x| x.label() == name)
    }
}

/// An electrode the editor can move: one of the ten cables or an extra chest electrode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cable {
    Standard(Electrode),
    Extra(ExtraLead),
}

impl Cable {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Cable::Standard(e) => e.label(),
            Cable::Extra(x) => x.label(),
        }
    }
}

// --- placement -----------------------------------------------------------------------------------

/// Position of `name` in `sites`, falling back to the procedural torso's site.
fn site(sites: &ElectrodeSet, name: &str) -> Vec3 {
    sites
        .get(name)
        .or_else(|| heartsim_geometry::procedural::procedural_electrodes().get(name))
        .unwrap_or(Vec3::ZERO)
}

/// Where each electrode is. See the module documentation.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// Site of the electrode on each cable, heart frame mm, indexed by `Electrode as usize`.
    /// Distal-limb electrodes sit at a [`Limb::anchor_mm`].
    pub positions: [Vec3; ELECTRODE_COUNT],
    /// Extra chest electrodes that are switched on.
    pub extras: BTreeMap<ExtraLead, Vec3>,
}

impl Placement {
    /// The diagnostic 12-lead placement (AHA/ACC, Kligfield 2007): limb electrodes on the wrists
    /// and ankles, chest electrodes at the anatomy pack's standard sites, no extras.
    #[must_use]
    pub fn standard(sites: &ElectrodeSet) -> Self {
        let positions = Electrode::ALL.map(|e| match Limb::of(e) {
            Some(l) => l.anchor_mm(),
            None => site(sites, e.label()),
        });
        Self {
            positions,
            extras: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn position(&self, e: Electrode) -> Vec3 {
        self.positions[e as usize]
    }

    pub fn set(&mut self, e: Electrode, p: Vec3) {
        self.positions[e as usize] = p;
    }

    /// Position of any movable electrode (`None` for an extra that is off).
    #[must_use]
    pub fn cable_position(&self, c: Cable) -> Option<Vec3> {
        match c {
            Cable::Standard(e) => Some(self.position(e)),
            Cable::Extra(x) => self.extras.get(&x).copied(),
        }
    }

    /// Move any electrode (an extra that is off is switched on).
    pub fn set_cable(&mut self, c: Cable, p: Vec3) {
        match c {
            Cable::Standard(e) => self.set(e, p),
            Cable::Extra(x) => {
                self.extras.insert(x, p);
            }
        }
    }

    /// The distal limb an electrode is on, if it is not on the torso.
    #[must_use]
    pub fn limb_of(&self, e: Electrode) -> Option<Limb> {
        Limb::at(self.position(e))
    }

    /// Exchange the electrodes of two cables (a cable reversal).
    pub fn swap(&mut self, a: Electrode, b: Electrode) {
        self.positions.swap(a as usize, b as usize);
    }

    /// Switch an extra electrode on at its standard site, or off.
    pub fn set_extra(&mut self, x: ExtraLead, on: bool, sites: &ElectrodeSet) {
        if on {
            self.extras.entry(x).or_insert_with(|| site(sites, x.label()));
        } else {
            self.extras.remove(&x);
        }
    }

    /// Store the placement in a scenario: the cables whose electrode is not at its standard site
    /// in `electrodes_mm`, the extras that are on in `extra_electrodes_mm`.
    pub fn write_scenario(&self, scenario: &mut Scenario, standard: &Placement) {
        scenario.electrodes_mm = Electrode::ALL
            .into_iter()
            .filter(|&e| self.position(e).distance(standard.position(e)) > 1e-3)
            .map(|e| (e, self.position(e).as_dvec3().to_array()))
            .collect();
        scenario.extra_electrodes_mm = self
            .extras
            .iter()
            .map(|(x, p)| (x.label().to_string(), p.as_dvec3().to_array()))
            .collect();
    }

    /// The placement a scenario describes: the standard one with the scenario's overrides.
    /// Unknown extra names are ignored.
    #[must_use]
    pub fn from_scenario(scenario: &Scenario, sites: &ElectrodeSet) -> Self {
        let mut p = Self::standard(sites);
        for (e, pos) in &scenario.electrodes_mm {
            p.set(*e, DVec3::from_array(*pos).as_vec3());
        }
        for (name, pos) in &scenario.extra_electrodes_mm {
            if let Some(x) = ExtraLead::from_label(name) {
                p.extras.insert(x, DVec3::from_array(*pos).as_vec3());
            }
        }
        p
    }

    /// Re-place the forward model's electrodes: distal limbs as far-field electrodes at their
    /// Einthoven angle, everything else as point electrodes. Rows whose electrode did not move
    /// are not recomputed.
    pub fn apply_to_forward(&self, model: &mut ForwardModel) {
        for e in Electrode::ALL {
            let p = self.position(e);
            if let Some(l) = Limb::at(p) {
                model.place_on_limb(e, l.hexaxial_deg());
            } else if model.is_far_field(e) || model.electrode_position(e) != p {
                model.move_electrode(e, p);
            }
        }
        let extras: Vec<(String, Vec3)> = self
            .extras
            .iter()
            .map(|(x, p)| (x.label().to_string(), *p))
            .collect();
        model.set_extra_electrodes(&extras);
    }
}

// --- presets and exercises -----------------------------------------------------------------------

/// Named placements a teacher reaches for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Preset {
    /// Diagnostic 12-lead: limbs on wrists and ankles.
    Standard,
    /// Limb electrodes on the torso (infraclavicular fossae, iliac crests; Mason & Likar 1966):
    /// exercise testing and monitoring. Shifts the axis rightwards and enlarges the inferior leads.
    MasonLikar,
    /// Standard plus V7–V9 at the V6 level posteriorly (posterior infarction).
    Posterior,
    /// Standard plus V3R and V4R, mirror images of V3/V4 (right-ventricular infarction).
    RightSided,
    /// Standard plus V4R, recorded routinely in children because of right-ventricular dominance.
    Paediatric,
}

impl Preset {
    pub const ALL: [Preset; 5] = [
        Preset::Standard,
        Preset::MasonLikar,
        Preset::Posterior,
        Preset::RightSided,
        Preset::Paediatric,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Preset::Standard => "Standard 12-lead",
            Preset::MasonLikar => "Mason–Likar",
            Preset::Posterior => "Posterior (V7–V9)",
            Preset::RightSided => "Right-sided (V3R, V4R)",
            Preset::Paediatric => "Paediatric (+V4R)",
        }
    }

    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Preset::Standard => {
                "Limb electrodes on the wrists and ankles; V1/V2 in the 4th intercostal space at the sternal borders, V4 in the 5th space at the mid-clavicular line, V3 between, V5/V6 at the V4 level in the anterior and mid-axillary lines."
            }
            Preset::MasonLikar => {
                "Limb electrodes moved onto the torso (below the clavicles, iliac crests) to avoid movement artefact during exercise tests and monitoring. Not equivalent to the diagnostic 12-lead: the axis shifts rightwards and the inferior leads grow."
            }
            Preset::Posterior => {
                "Adds V7 (posterior axillary line), V8 (scapular line) and V9 (paraspinal) at the V6 level: ST elevation there confirms a posterior infarction suspected from ST depression in V1–V3."
            }
            Preset::RightSided => {
                "Adds V3R and V4R, the mirror images of V3 and V4 on the right chest: ST elevation in V4R points to right-ventricular infarction."
            }
            Preset::Paediatric => {
                "Adds V4R, recorded routinely in children whose right ventricle is relatively larger."
            }
        }
    }

    /// The placement of this preset on the pack's `sites`.
    #[must_use]
    pub fn build(self, sites: &ElectrodeSet) -> Placement {
        let mut p = Placement::standard(sites);
        let extras: &[ExtraLead] = match self {
            Preset::Standard | Preset::MasonLikar => &[],
            Preset::Posterior => &[ExtraLead::V7, ExtraLead::V8, ExtraLead::V9],
            Preset::RightSided => &[ExtraLead::V3R, ExtraLead::V4R],
            Preset::Paediatric => &[ExtraLead::V4R],
        };
        for x in extras {
            p.set_extra(*x, true, sites);
        }
        if self == Preset::MasonLikar {
            for e in [Electrode::RA, Electrode::LA, Electrode::LL, Electrode::RL] {
                p.set(e, site(sites, e.label()));
            }
        }
        p
    }
}

/// A misplacement for the learner to recognise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Exercise {
    RaLaReversed,
    RaLlReversed,
    LaLlReversed,
    RaRlReversed,
    LaRlReversed,
    V1V2Reversed,
    V1V2TooHigh,
    V5V6Posterior,
    LimbsOnTorso,
}

impl Exercise {
    pub const ALL: [Exercise; 9] = [
        Exercise::RaLaReversed,
        Exercise::RaLlReversed,
        Exercise::LaLlReversed,
        Exercise::RaRlReversed,
        Exercise::LaRlReversed,
        Exercise::V1V2Reversed,
        Exercise::V1V2TooHigh,
        Exercise::V5V6Posterior,
        Exercise::LimbsOnTorso,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Exercise::RaLaReversed => "RA/LA reversed",
            Exercise::RaLlReversed => "RA/LL reversed",
            Exercise::LaLlReversed => "LA/LL reversed",
            Exercise::RaRlReversed => "RA/RL (neutral) reversed",
            Exercise::LaRlReversed => "LA/RL (neutral) reversed",
            Exercise::V1V2Reversed => "V1/V2 cables reversed",
            Exercise::V1V2TooHigh => "V1/V2 one space too high",
            Exercise::V5V6Posterior => "V5/V6 too posterior",
            Exercise::LimbsOnTorso => "Limb leads on the torso",
        }
    }

    /// Whether the detector's findings name this misplacement (the first finding must; a
    /// displaced V1/V2 or V5/V6 pair counts when either electrode of the pair is reported).
    #[must_use]
    pub fn recognised(self, findings: &[Finding]) -> bool {
        use Electrode::{LA, LL, RA, RL, V1, V2, V5, V6};
        let displaced_within = |allowed: &[Electrode]| {
            findings.iter().any(|f| {
                matches!(f, Finding::PrecordialDisplaced(v)
                    if !v.is_empty() && v.iter().all(|e| allowed.contains(e)))
            })
        };
        match self {
            Exercise::RaLaReversed => findings.first() == Some(&Finding::CableSwap(RA, LA)),
            Exercise::RaLlReversed => findings.first() == Some(&Finding::CableSwap(RA, LL)),
            Exercise::LaLlReversed => findings.first() == Some(&Finding::CableSwap(LA, LL)),
            Exercise::RaRlReversed => findings.first() == Some(&Finding::CableSwap(RA, RL)),
            Exercise::LaRlReversed => findings.first() == Some(&Finding::CableSwap(LA, RL)),
            Exercise::V1V2Reversed => findings.first() == Some(&Finding::CableSwap(V1, V2)),
            Exercise::V1V2TooHigh => findings.len() == 1 && displaced_within(&[V1, V2]),
            Exercise::V5V6Posterior => findings.len() == 1 && displaced_within(&[V5, V6]),
            Exercise::LimbsOnTorso => matches!(findings.first(), Some(Finding::LimbsOnTorso { .. })),
        }
    }

    /// Apply the misplacement to a placement (normally the standard one). Moves along the chest
    /// wall use `surface` when given, otherwise they shift the site and leave it slightly off the
    /// skin.
    pub fn apply(self, p: &mut Placement, sites: &ElectrodeSet, surface: Option<&TorsoSurface>) {
        let swap = |p: &mut Placement, a, b| p.swap(a, b);
        match self {
            Exercise::RaLaReversed => swap(p, Electrode::RA, Electrode::LA),
            Exercise::RaLlReversed => swap(p, Electrode::RA, Electrode::LL),
            Exercise::LaLlReversed => swap(p, Electrode::LA, Electrode::LL),
            Exercise::RaRlReversed => swap(p, Electrode::RA, Electrode::RL),
            Exercise::LaRlReversed => swap(p, Electrode::LA, Electrode::RL),
            Exercise::V1V2Reversed => swap(p, Electrode::V1, Electrode::V2),
            Exercise::V1V2TooHigh => {
                for e in [Electrode::V1, Electrode::V2] {
                    let q = p.position(e) + Vec3::Y * ICS_SPACING_MM;
                    p.set(e, surface.and_then(|s| s.project(q)).unwrap_or(q));
                }
            }
            Exercise::V5V6Posterior => {
                for e in [Electrode::V5, Electrode::V6] {
                    let q = p.position(e);
                    let moved = surface.and_then(|s| s.at(q.y, s.azimuth_deg(q) + 25.0));
                    p.set(e, moved.unwrap_or(q + Vec3::new(0.0, 0.0, -40.0)));
                }
            }
            Exercise::LimbsOnTorso => {
                for e in [Electrode::RA, Electrode::LA, Electrode::LL, Electrode::RL] {
                    p.set(e, site(sites, e.label()));
                }
            }
        }
    }
}

// --- the torso surface and its landmarks ---------------------------------------------------------

/// Vertical distance between adjacent intercostal spaces on the anterior chest of the default
/// torso, mm (V1/V2 in the 4th space at +15 mm, V4 in the 5th at −15 mm in the heart frame).
pub const ICS_SPACING_MM: f32 = 30.0;

/// Intercostal spaces (anterior chest) and their height in the heart frame, mm.
pub const INTERCOSTAL_SPACES: [(&str, f32); 5] = [
    ("2nd intercostal space", 75.0),
    ("3rd intercostal space", 45.0),
    ("4th intercostal space", 15.0),
    ("5th intercostal space", -15.0),
    ("6th intercostal space", -45.0),
];

/// Vertical reference lines of the chest wall and their azimuth around the torso axis, degrees
/// (0° = anterior midline, positive towards the patient's left; the standard sites of
/// `heartsim_geometry::procedural::torso` lie on them).
pub const VERTICAL_LINES: [(&str, f32); 13] = [
    ("right posterior axillary line", -108.0),
    ("right mid-axillary line", -90.0),
    ("right anterior axillary line", -72.0),
    ("right mid-clavicular line", -52.0),
    (
        "between right sternal border and right mid-clavicular line",
        -32.0,
    ),
    ("right sternal border", -12.0),
    ("left sternal border", 12.0),
    ("between left sternal border and left mid-clavicular line", 32.0),
    ("left mid-clavicular line", 52.0),
    ("left anterior axillary line", 72.0),
    ("left mid-axillary line", 90.0),
    ("left posterior axillary line", 108.0),
    ("left mid-scapular line", 140.0),
];

/// Snap to an intercostal space within this height, mm.
const SNAP_HEIGHT_MM: f32 = 9.0;
/// Snap to a vertical line within this azimuth, degrees.
const SNAP_AZIMUTH_DEG: f32 = 7.0;

/// The body surface as a set of triangles, for constraining electrodes to the skin.
#[derive(Debug, Clone)]
pub struct TorsoSurface {
    triangles: Vec<[Vec3; 3]>,
    /// Torso axis (x, z): centre of the bounding box.
    axis: (f32, f32),
    lo: Vec3,
    hi: Vec3,
}

/// A snapped electrode site.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapped {
    pub position: Vec3,
    /// The landmark it snapped to, e.g. "4th intercostal space, left sternal border".
    pub landmark: Option<String>,
}

impl TorsoSurface {
    #[must_use]
    pub fn new(mesh: &TriMesh) -> Self {
        let (lo, hi) = mesh.bounds();
        let p = |i: u32| Vec3::from(mesh.vertices[i as usize].position);
        Self {
            triangles: mesh
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| [p(t[0]), p(t[1]), p(t[2])])
                .collect(),
            axis: (f32::midpoint(lo.x, hi.x), f32::midpoint(lo.z, hi.z)),
            lo,
            hi,
        }
    }

    /// Azimuth of `p` around the torso axis, degrees (0° anterior, + towards the patient's left).
    #[must_use]
    pub fn azimuth_deg(&self, p: Vec3) -> f32 {
        (p.x - self.axis.0).atan2(p.z - self.axis.1).to_degrees()
    }

    /// Nearest surface point along the ray (`dir` need not be normalised), if it hits.
    #[must_use]
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3) -> Option<Vec3> {
        let mut best: Option<f32> = None;
        for [a, b, c] in &self.triangles {
            if let Some(t) = ray_triangle(origin, dir, *a, *b, *c)
                && best.is_none_or(|b| t < b)
            {
                best = Some(t);
            }
        }
        best.map(|t| origin + dir * t)
    }

    /// Surface point at height `y_mm` and azimuth `azimuth_deg`, found by a horizontal ray from
    /// the torso axis.
    #[must_use]
    pub fn at(&self, y_mm: f32, azimuth_deg: f32) -> Option<Vec3> {
        if !(self.lo.y..=self.hi.y).contains(&y_mm) {
            return None;
        }
        let (s, c) = azimuth_deg.to_radians().sin_cos();
        self.ray_hit(Vec3::new(self.axis.0, y_mm, self.axis.1), Vec3::new(s, 0.0, c))
    }

    /// The surface point at the height and azimuth of `p`.
    #[must_use]
    pub fn project(&self, p: Vec3) -> Option<Vec3> {
        self.at(p.y, self.azimuth_deg(p))
    }

    /// Snap a surface point to the nearest intercostal space and vertical line when it is close
    /// to them (each independently), and name the landmark.
    #[must_use]
    pub fn snap(&self, p: Vec3) -> Snapped {
        let az = self.azimuth_deg(p);
        let space = nearest(&INTERCOSTAL_SPACES, p.y).filter(|(_, d)| *d <= SNAP_HEIGHT_MM);
        let line = nearest(&VERTICAL_LINES, az).filter(|(_, d)| *d <= SNAP_AZIMUTH_DEG);
        let y = space.map_or(p.y, |(i, _)| INTERCOSTAL_SPACES[i].1);
        let a = line.map_or(az, |(i, _)| VERTICAL_LINES[i].1);
        let position = if space.is_some() || line.is_some() {
            self.at(y, a).unwrap_or(p)
        } else {
            p
        };
        let landmark = match (space, line) {
            (Some((i, _)), Some((j, _))) => {
                Some(format!("{}, {}", INTERCOSTAL_SPACES[i].0, VERTICAL_LINES[j].0))
            }
            (Some((i, _)), None) => Some(INTERCOSTAL_SPACES[i].0.to_string()),
            (None, Some((j, _))) => Some(VERTICAL_LINES[j].0.to_string()),
            (None, None) => None,
        };
        Snapped { position, landmark }
    }

    /// Where `p` is in words: the nearest intercostal level and vertical line.
    #[must_use]
    pub fn describe(&self, p: Vec3) -> String {
        let (i, dy) = nearest(&INTERCOSTAL_SPACES, p.y).unwrap_or((0, f32::INFINITY));
        let (j, da) = nearest(&VERTICAL_LINES, self.azimuth_deg(p)).unwrap_or((0, f32::INFINITY));
        let near = |d: f32, tol: f32| if d <= tol { "" } else { "near " };
        format!(
            "{}{}, {}{}",
            near(dy, SNAP_HEIGHT_MM),
            INTERCOSTAL_SPACES[i].0,
            near(da, SNAP_AZIMUTH_DEG),
            VERTICAL_LINES[j].0
        )
    }
}

/// Index of the entry nearest to `x` and its distance.
fn nearest(table: &[(&str, f32)], x: f32) -> Option<(usize, f32)> {
    table
        .iter()
        .enumerate()
        .map(|(i, (_, v))| (i, (v - x).abs()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Möller–Trumbore ray/triangle intersection, double sided; the ray parameter `t > 0`.
fn ray_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 1e-6).then_some(t)
}

// --- Tier 0 --------------------------------------------------------------------------------------

/// Electrode-vector field of the Tier-0 heart dipole. See the module documentation.
#[derive(Debug, Clone, PartialEq)]
pub struct Tier0Field {
    /// `k = R²/√3`, mm²: makes a torso electrode at the Einthoven distance as sensitive as a limb.
    k_mm2: f64,
    /// Standard precordial sites and the correction `E_std − k ℓ(r)` there.
    nodes: [(DVec3, DVec3); 6],
}

impl Tier0Field {
    /// Field anchored on the standard precordial sites of `sites`.
    #[must_use]
    pub fn new(sites: &ElectrodeSet) -> Self {
        let k_mm2 = EINTHOVEN_DISTANCE_MM * EINTHOVEN_DISTANCE_MM / 3f64.sqrt();
        let standard = LeadVectors::standard();
        let nodes = std::array::from_fn(|i| {
            let r = site(sites, PRECORDIAL[i].label()).as_dvec3();
            let target = standard.vectors[Lead::V1 as usize + i];
            (r, target - k_mm2 * dipole_field(r))
        });
        Self { k_mm2, nodes }
    }

    /// Electrode vector `E(r)`: an electrode at `p` records `E(r)·p_heart` (WCT not removed).
    #[must_use]
    pub fn electrode_vector(&self, p: Vec3) -> DVec3 {
        if let Some(l) = Limb::at(p) {
            return l.direction() / 3f64.sqrt();
        }
        let r = p.as_dvec3();
        self.k_mm2 * dipole_field(r) + self.correction(r)
    }

    /// Shepard interpolation of the node corrections, faded with the distance to the nearest
    /// node: exact at the nodes, continuous, zero far away.
    fn correction(&self, r: DVec3) -> DVec3 {
        let mut num = DVec3::ZERO;
        let mut den = 0.0;
        let mut dmin = f64::INFINITY;
        for (node, c) in &self.nodes {
            let d = node.distance(r);
            if d < 1e-6 {
                return *c;
            }
            dmin = dmin.min(d);
            let w = 1.0 / (d * d);
            num += *c * w;
            den += w;
        }
        let fade = (-(dmin / CORRECTION_RADIUS_MM).powi(2)).exp();
        num / den * fade
    }

    /// Lead vectors of the twelve leads for a placement.
    #[must_use]
    pub fn lead_vectors(&self, p: &Placement) -> LeadVectors {
        let e = p.positions.map(|x| self.electrode_vector(x));
        lead_vectors_of(&e)
    }

    /// Lead vectors (`E − WCT`) of the extra electrodes that are on.
    #[must_use]
    pub fn extra_vectors(&self, p: &Placement) -> Vec<(ExtraLead, DVec3)> {
        let wct = self.wct(p);
        p.extras
            .iter()
            .map(|(x, pos)| (*x, self.electrode_vector(*pos) - wct))
            .collect()
    }

    fn wct(&self, p: &Placement) -> DVec3 {
        [Electrode::RA, Electrode::LA, Electrode::LL]
            .iter()
            .map(|e| self.electrode_vector(p.position(*e)))
            .sum::<DVec3>()
            / 3.0
    }
}

/// `(r − c)/|r − c|³` with `c` the ventricular centroid (the heart-frame origin).
fn dipole_field(r: DVec3) -> DVec3 {
    let d = r.length().max(MIN_RADIUS_MM);
    r / (d * d * d)
}

/// The twelve lead vectors from ten electrode vectors (Einthoven, Goldberger, Wilson).
///
/// The augmented leads are scaled by `2/√3`: Tier 0 uses the hexaxial convention in which all six
/// limb lead vectors have the same length (so `atan2(aVF, I)` is the axis), whereas Goldberger's
/// leads are `√3/2` of that. A fixed gain, so no pattern changes.
#[must_use]
pub fn lead_vectors_of(e: &[DVec3; ELECTRODE_COUNT]) -> LeadVectors {
    let aug = 2.0 / 3f64.sqrt();
    let g = |x: Electrode| e[x as usize];
    let (ra, la, ll) = (g(Electrode::RA), g(Electrode::LA), g(Electrode::LL));
    let wct = (ra + la + ll) / 3.0;
    let mut v = [DVec3::ZERO; LEAD_COUNT];
    v[Lead::I as usize] = la - ra;
    v[Lead::II as usize] = ll - ra;
    v[Lead::III as usize] = ll - la;
    v[Lead::AVR as usize] = (ra - (la + ll) * 0.5) * aug;
    v[Lead::AVL as usize] = (la - (ra + ll) * 0.5) * aug;
    v[Lead::AVF as usize] = (ll - (ra + la) * 0.5) * aug;
    for (i, x) in PRECORDIAL.iter().enumerate() {
        v[Lead::V1 as usize + i] = g(*x) - wct;
    }
    LeadVectors { vectors: v }
}

/// Least-squares recovery of the Tier-0 heart dipole from twelve lead values:
/// `p = (LᵀL)⁻¹ Lᵀ v`. Exact for Tier-0 samples (they are `L p`), so the same dipole can be
/// projected onto other lead vectors (the standard placement, extra leads).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DipoleSolver {
    rows: [DVec3; LEAD_COUNT],
}

impl DipoleSolver {
    #[must_use]
    pub fn new(leads: &LeadVectors) -> Self {
        let m = leads.vectors.iter().fold(DMat3::ZERO, |m, l| {
            m + DMat3::from_cols(*l * l.x, *l * l.y, *l * l.z)
        });
        let tr = m.x_axis.x + m.y_axis.y + m.z_axis.z;
        let inv = (m + DMat3::from_diagonal(DVec3::splat(1e-9 * tr.max(1e-12)))).inverse();
        Self {
            rows: leads.vectors.map(|l| inv * l),
        }
    }

    #[must_use]
    pub fn dipole(&self, leads: &[f64; LEAD_COUNT]) -> DVec3 {
        self.rows.iter().zip(leads).map(|(r, v)| *r * *v).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_geometry::procedural::procedural_electrodes;

    #[test]
    fn standard_placement_reproduces_the_textbook_lead_vectors() {
        let sites = procedural_electrodes();
        let field = Tier0Field::new(&sites);
        let lv = field.lead_vectors(&Placement::standard(&sites));
        let std = LeadVectors::standard();
        for l in Lead::ALL {
            let d = (lv.vectors[l as usize] - std.vectors[l as usize]).length();
            assert!(d < 1e-9, "{}: {d}", l.label());
        }
    }

    #[test]
    fn ra_la_swap_inverts_lead_i_and_exchanges_ii_iii() {
        let sites = procedural_electrodes();
        let field = Tier0Field::new(&sites);
        let mut p = Placement::standard(&sites);
        let a = field.lead_vectors(&p);
        p.swap(Electrode::RA, Electrode::LA);
        let b = field.lead_vectors(&p);
        let v = |lv: &LeadVectors, l: Lead| lv.vectors[l as usize];
        assert!((v(&b, Lead::I) + v(&a, Lead::I)).length() < 1e-12);
        assert!((v(&b, Lead::II) - v(&a, Lead::III)).length() < 1e-12);
        assert!((v(&b, Lead::AVR) - v(&a, Lead::AVL)).length() < 1e-12);
        assert!((v(&b, Lead::V4) - v(&a, Lead::V4)).length() < 1e-12);
    }

    #[test]
    fn moving_an_electrode_changes_its_vector_smoothly() {
        let sites = procedural_electrodes();
        let field = Tier0Field::new(&sites);
        let v1 = sites.get("V1").unwrap();
        let e0 = field.electrode_vector(v1);
        let e1 = field.electrode_vector(v1 + Vec3::new(0.0, 0.5, 0.0));
        let e30 = field.electrode_vector(v1 + Vec3::new(0.0, 30.0, 0.0));
        assert!((e1 - e0).length() < 0.03, "continuous at the node");
        assert!((e30 - e0).length() > 0.1, "a space higher is a different lead");
    }

    #[test]
    fn dipole_solver_inverts_the_projection() {
        let sites = procedural_electrodes();
        let field = Tier0Field::new(&sites);
        let mut p = Placement::standard(&sites);
        p.swap(Electrode::RA, Electrode::RL);
        let lv = field.lead_vectors(&p);
        let d = DVec3::new(0.3, -1.1, 0.4);
        let back = DipoleSolver::new(&lv).dipole(&lv.project(d));
        assert!((back - d).length() < 1e-6);
    }

    #[test]
    fn limbs_are_found_by_their_anchor() {
        for l in Limb::ALL {
            assert_eq!(Limb::at(l.anchor_mm() + Vec3::splat(10.0)), Some(l));
        }
        let sites = procedural_electrodes();
        assert_eq!(
            Limb::at(sites.get("RA").unwrap()),
            None,
            "Mason–Likar site is torso"
        );
    }
}
