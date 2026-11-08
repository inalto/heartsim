//! Coronary ischaemia and infarction (brief B14).
//!
//! `Action::Stenosis { branch, severity }` narrows a coronary branch (`severity` 0 = open …
//! 1 = occluded). The branch perfuses a set of AHA segments (Cerqueira et al. 2002, the
//! assignment of [`heartsim_geometry::Territory`], refined to the major side branches); a
//! cell's ischaemic drive is `q = w · severity`, with `w` its territory weight (1 inside,
//! smoothed over a few millimetres at the territory border: the lateral border zone).
//!
//! **Transmural extent — the wavefront phenomenon** (Reimer & Jennings 1979). The
//! subendocardium is the most vulnerable layer, so a cell at transmural depth ρ (0 endocardium
//! … 1 epicardium) is injured once `q` exceeds `θ(ρ) = 0.4 + 0.3 ρ`; its injury
//! `M = clamp((q − θ(ρ)) / 0.1, 0, 1)`. A severe stenosis (≥ 0.8) injures the full wall
//! (transmural → ST elevation, "STEMI"); a moderate one (≈ 0.55–0.65) only the inner layers
//! (subendocardial → ST depression); below 0.4 there is no injury (flow reserve).
//!
//! **Acute injury** (research §3.5 recipe; Janse & Kléber 1981): resting potential +20 mV · E
//! (the diastolic injury current), plateau −15 mV · E, APD −60 ms · E, CV × (1 − 0.5 M). In
//! normalised V_m (≈ 100 mV per unit) a cell plays `o + (1 − o − p) · template` with
//! `o = 0.2 E`, `p = 0.15 E` ([`InjuryField`]). The forward model then sees the injury current
//! directly in V_m.
//!
//! **Electrical expression** `E = M · (0.25 + 0.75 ρ)`. Epicardial myocytes carry the denser
//! ATP-sensitive K⁺ current, so the same ischaemia shortens their APD and depolarises them more
//! than endocardial ones (Furukawa et al., *Circ Res* 1991;68:1693–1702); and in vivo the
//! highly conductive blood pool short-circuits the endocardial side of an injured zone, which an
//! infinite-medium forward model cannot represent. Without that asymmetry a uniformly injured
//! transmural zone is a closed double layer whose potential nearly cancels outside it. The net
//! transmural injury dipole is then set by `E(epi) − E(endo)`: positive over a transmural zone
//! (ST elevation, reciprocal depression opposite), negative over a subendocardial one (ST
//! depression). Necrosis (infarct core) follows `M`, not `E`.
//!
//! **Infarction** (branch suffix `infarct`, e.g. `"LAD infarct"`): necrotic core (`M ≥ 0.6`)
//! is non-excitable and electrically silent — the "window" that makes pathological Q waves —
//! surrounded by a border zone with slow conduction and long APD (the defaults of
//! `Region::BorderZone`: 0.2 m/s vs 0.6, 340 ms vs 285). No injury current (healed scar).
//!
//! **Evolving infarct** (branch suffix `evolving`, e.g. `"LAD evolving"`): the same necrotic core
//! on the mesh, a small residual ST elevation, and on the Tier-0 strip the T waves of the
//! territory inverted as well as the Q waves ([`IschaemiaState::infarct_forces`]). The acute
//! lesion, this one and the healed infarct are the stages a timeline walks through.
//!
//! **Pericarditis** (`"pericardium"`): diffuse injury of the outer 20 % of the ventricular wall
//! (all territories) at `M = 0.6 · severity` → concave ST elevation in most leads, depression
//! in aVR. **Global subendocardial ischaemia** (`"global"`): demand ischaemia of the whole LV
//! with the same wavefront rule → diffuse ST depression with ST elevation in aVR.

use crate::tier1::{CellKind, Graph};
use heartsim_core::math::Deterministic;
use heartsim_core::{Action, Region};
use heartsim_geometry::CellCloud;
use heartsim_geometry::territory::RV_ANTERIOR_SEGMENT;
use serde::{Deserialize, Serialize};

/// Normalised resting-potential rise at full injury (20 mV of a ≈ 100 mV AP).
pub const INJURY_REST_OFFSET: f32 = 0.2;
/// Normalised plateau loss at full injury (15 mV).
pub const INJURY_PLATEAU_LOSS: f32 = 0.15;
/// APD shortening at full injury, as a fraction (60 ms of 285 ms).
pub const INJURY_APD_SHORTENING: f32 = 0.21;
/// CV loss at full injury, as a fraction.
pub const INJURY_CV_LOSS: f32 = 0.5;
/// Infarct border zone: CV and APD multipliers (`Region::BorderZone` vs ventricular defaults).
pub const BORDER_ZONE_CV_SCALE: f32 = 0.33;
pub const BORDER_ZONE_APD_SCALE: f32 = 1.19;
/// Wavefront rule: injury threshold at the endocardium and its rise to the epicardium.
const THRESHOLD_ENDO: f32 = 0.4;
const THRESHOLD_RISE: f32 = 0.3;
/// Width of the injury ramp in units of ischaemic drive.
const INJURY_RAMP: f32 = 0.1;
/// Infarct core: injury at which the tissue is necrotic.
const INFARCT_CORE: f32 = 0.6;
/// Electrical expression of acute injury at the endocardium (1 at the epicardium).
const ENDOCARDIAL_EXPRESSION: f32 = 0.25;
/// Pericarditis: outer fraction of the wall and its injury per unit severity.
const PERICARDIAL_DEPTH: f32 = 0.8;
const PERICARDIAL_INJURY: f32 = 0.6;
/// Jacobi smoothing passes of the territory membership over the propagation graph (≈ one graph
/// radius each: a border zone of several millimetres).
const TERRITORY_SMOOTHING_PASSES: usize = 2;

/// A coronary branch (or diffuse pattern) a lesion can sit in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CoronaryBranch {
    /// Left main: LAD + LCx territories.
    LeftMain,
    /// Left anterior descending, occluded proximally (before the first diagonal): anterior wall,
    /// anterior septum, apex (AHA 1, 2, 7, 8, 13, 14, 17) and the diagonal-supplied anterolateral
    /// wall (6, 12 — "LAD or LCx" in Cerqueira et al. 2002), plus the anterior apical RV free wall
    /// its right ventricular branches supply. The superior-leftward extent is what gives the
    /// reciprocal inferior ST depression of proximal LAD occlusion (Birnbaum et al., *Am Heart J*
    /// 1993;126:1273).
    Lad,
    /// Left circumflex: lateral and inferolateral wall (5, 6, 11, 12, 16).
    Lcx,
    /// Obtuse marginal (LCx): anterolateral wall (6, 12, 16) — lateral infarction.
    ObtuseMarginal,
    /// Posterolateral branch (LCx/RCA): inferolateral ("posterior") wall (5, 11).
    Posterolateral,
    /// Right coronary: inferior wall, inferior septum and the RV free wall other than the LAD's zone
    /// (3, 4, 9, 10, 15 + RV).
    Rca,
    /// Posterior descending (RCA in right dominance): inferior wall and inferior septum, no RV.
    Pda,
    /// Diffuse subendocardial (demand) ischaemia of the whole LV and septum.
    Global,
    /// Diffuse subepicardial injury of both ventricles (pericarditis).
    Pericardium,
}

impl CoronaryBranch {
    pub const ALL: [CoronaryBranch; 9] = [
        CoronaryBranch::LeftMain,
        CoronaryBranch::Lad,
        CoronaryBranch::Lcx,
        CoronaryBranch::ObtuseMarginal,
        CoronaryBranch::Posterolateral,
        CoronaryBranch::Rca,
        CoronaryBranch::Pda,
        CoronaryBranch::Global,
        CoronaryBranch::Pericardium,
    ];

    /// Id used in `Action::Stenosis { branch }`.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            CoronaryBranch::LeftMain => "LM",
            CoronaryBranch::Lad => "LAD",
            CoronaryBranch::Lcx => "LCx",
            CoronaryBranch::ObtuseMarginal => "OM",
            CoronaryBranch::Posterolateral => "PL",
            CoronaryBranch::Rca => "RCA",
            CoronaryBranch::Pda => "PDA",
            CoronaryBranch::Global => "global",
            CoronaryBranch::Pericardium => "pericardium",
        }
    }

    /// Display name.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            CoronaryBranch::LeftMain => "Left main",
            CoronaryBranch::Lad => "Left anterior descending",
            CoronaryBranch::Lcx => "Left circumflex",
            CoronaryBranch::ObtuseMarginal => "Obtuse marginal",
            CoronaryBranch::Posterolateral => "Posterolateral",
            CoronaryBranch::Rca => "Right coronary",
            CoronaryBranch::Pda => "Posterior descending",
            CoronaryBranch::Global => "Global subendocardial",
            CoronaryBranch::Pericardium => "Pericardium (diffuse)",
        }
    }

    /// Case-insensitive lookup by [`CoronaryBranch::id`].
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.id().eq_ignore_ascii_case(id))
    }

    /// Whether this branch perfuses a ventricular cell of `region` in AHA segment `segment`
    /// (0 = none; the RV free wall has no segment).
    #[must_use]
    pub fn perfuses(self, region: Region, segment: u8) -> bool {
        let rv = region == Region::RightVentricle;
        match self {
            CoronaryBranch::LeftMain => {
                CoronaryBranch::Lad.perfuses(region, segment) || CoronaryBranch::Lcx.perfuses(region, segment)
            }
            CoronaryBranch::Lad => {
                if rv {
                    segment == RV_ANTERIOR_SEGMENT
                } else {
                    matches!(segment, 1 | 2 | 6 | 7 | 8 | 12 | 13 | 14 | 17)
                }
            }
            CoronaryBranch::Lcx => !rv && matches!(segment, 5 | 6 | 11 | 12 | 16),
            CoronaryBranch::ObtuseMarginal => !rv && matches!(segment, 6 | 12 | 16),
            CoronaryBranch::Posterolateral => !rv && matches!(segment, 5 | 11),
            CoronaryBranch::Rca => {
                if rv {
                    segment != RV_ANTERIOR_SEGMENT
                } else {
                    matches!(segment, 3 | 4 | 9 | 10 | 15)
                }
            }
            CoronaryBranch::Pda => !rv && matches!(segment, 3 | 4 | 9 | 10 | 15),
            CoronaryBranch::Global => !rv && segment != 0,
            CoronaryBranch::Pericardium => region != Region::VentricularSeptum,
        }
    }

    /// Direction (heart frame: +x left, +y cranial, +z anterior) of the ST injury vector of a
    /// transmural lesion of this branch: it points from the heart's centre into the injured
    /// territory (Tier-0 synthesis, which has no mesh). Unit length.
    #[must_use]
    pub fn injury_direction(self) -> [f64; 3] {
        let v: [f64; 3] = match self {
            // anterior, slightly leftward and superior: V1–V4 (+ I, aVL) up, II/III/aVF down
            CoronaryBranch::Lad => [0.3, 0.35, 0.9],
            CoronaryBranch::LeftMain => [0.6, 0.4, 0.5],
            CoronaryBranch::Lcx => [0.85, -0.1, -0.5],
            CoronaryBranch::ObtuseMarginal => [1.0, 0.3, 0.0],
            // posterior: V1–V3 down (reciprocal of V7–V9 elevation)
            CoronaryBranch::Posterolateral => [0.3, -0.2, -1.0],
            // inferior and rightward: III > II, I/aVL down
            CoronaryBranch::Rca => [-0.3, -1.0, 0.15],
            CoronaryBranch::Pda => [0.0, -1.0, -0.15],
            // global subendocardial: away from the apex towards the base (aVR up)
            CoronaryBranch::Global => [-0.5, 0.75, -0.2],
            // pericarditis: towards the apex (diffuse elevation, aVR down)
            CoronaryBranch::Pericardium => [0.5, -0.6, 0.6],
        };
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l, v[1] / l, v[2] / l]
    }

    /// The leads an infarct of this branch faces. `None` for the diffuse patterns, which have no
    /// necrotic territory. Lead groups: anterior V1–V4 plus I and aVL when proximal; inferior II,
    /// III, aVF; lateral I, aVL, V5, V6; posterior the mirror image in V1–V3 (Wagner et al. 2009).
    #[must_use]
    pub fn infarct_pattern(self) -> Option<InfarctPattern> {
        let (frontal_axis_deg, frontal_gain, precordial) = match self {
            CoronaryBranch::LeftMain => (-20.0, 0.7, [1.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
            CoronaryBranch::Lad => (-30.0, 0.5, [1.0, 1.0, 1.0, 1.0, 0.5, 0.25]),
            CoronaryBranch::Lcx => (-10.0, 1.0, [-0.3, -0.3, 0.0, 0.0, 0.6, 1.0]),
            CoronaryBranch::ObtuseMarginal => (-15.0, 1.0, [0.0, 0.0, 0.0, 0.2, 0.8, 1.0]),
            CoronaryBranch::Posterolateral => (0.0, 0.0, [-1.0, -1.0, -0.8, 0.0, 0.0, 0.0]),
            CoronaryBranch::Rca => (85.0, 1.0, [0.0; 6]),
            CoronaryBranch::Pda => (90.0, 1.0, [0.0; 6]),
            CoronaryBranch::Global | CoronaryBranch::Pericardium => return None,
        };
        Some(InfarctPattern {
            frontal_axis_deg,
            frontal_gain,
            precordial,
        })
    }
}

/// Acute ischaemia, an evolving infarct or a healed infarct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LesionKind {
    Acute,
    /// Healed: Q waves, no ST shift, T waves nearly normal.
    Infarct,
    /// Hours to days old: Q waves, inverted T waves, a little residual ST elevation.
    Evolving,
}

impl LesionKind {
    /// `true` when the core of the lesion is necrotic.
    #[must_use]
    pub fn is_necrotic(self) -> bool {
        matches!(self, LesionKind::Infarct | LesionKind::Evolving)
    }

    /// How far the T waves of the territory are inverted (0 = not at all, 1 = fully).
    fn t_inversion(self) -> f64 {
        match self {
            LesionKind::Acute => 0.0,
            LesionKind::Infarct => 0.25,
            LesionKind::Evolving => 1.0,
        }
    }
}

/// Which leads an infarct of one territory faces, for the Tier-0 strip (which has no mesh). The
/// surface pattern of an old or evolving infarct is regional: pathological Q waves and T inversion
/// appear in the contiguous leads over the territory (Wagner et al., *Circulation* 2009;119:e262;
/// Thygesen et al. 2018), so the precordial leads carry their own weights instead of one dipole.
/// The limb leads stay a frontal dipole, so that Einthoven's law keeps holding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfarctPattern {
    /// Hexaxial angle in the frontal plane pointing into the territory, degrees.
    pub frontal_axis_deg: f64,
    /// How strongly the limb leads see it (0 = not at all).
    pub frontal_gain: f64,
    /// Weight of V1…V6: 1 = faces the infarct; negative = sees it from behind (the tall R and
    /// upright T of a posterior infarct in V1–V3).
    pub precordial: [f64; 6],
}

/// What the necrotic lesions in effect do to the Q wave, R wave and T wave of a conducted beat,
/// per lead group, for Tier-0 synthesis (see [`IschaemiaState::infarct_forces`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct InfarctForces {
    /// Frontal vector into the infarct, heart frame `[x, y]`; its length is the weight (0 … 1).
    pub q_limb: [f64; 2],
    /// Weight of V1…V6 for the Q wave and the loss of R.
    pub q_precordial: [f64; 6],
    /// Same as `q_limb` for the T inversion.
    pub t_limb: [f64; 2],
    /// Same as `q_precordial` for the T inversion.
    pub t_precordial: [f64; 6],
}

impl InfarctForces {
    /// `true` when no lesion changes the beat.
    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == Self::default()
    }
}

/// One lesion: a branch, its kind, and its severity (0 … 1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Lesion {
    pub branch: CoronaryBranch,
    pub kind: LesionKind,
    pub severity: f64,
}

impl Lesion {
    /// Injury of a cell with territory weight `w` at transmural depth `rho` (see module docs).
    #[must_use]
    pub fn injury(&self, w: f32, rho: f32) -> f32 {
        let s = self.severity as f32;
        if self.branch == CoronaryBranch::Pericardium {
            return if rho >= PERICARDIAL_DEPTH {
                (PERICARDIAL_INJURY * s * w).clamp(0.0, 1.0)
            } else {
                0.0
            };
        }
        let threshold = THRESHOLD_ENDO + THRESHOLD_RISE * rho.clamp(0.0, 1.0);
        ((w * s - threshold) / INJURY_RAMP).clamp(0.0, 1.0)
    }

    /// Electrical expression of this lesion's injury at transmural depth `rho` (see module
    /// docs): epicardial cells express the injury fully, endocardial ones at 25 %.
    #[must_use]
    pub fn expression(rho: f32) -> f32 {
        ENDOCARDIAL_EXPRESSION + (1.0 - ENDOCARDIAL_EXPRESSION) * rho.clamp(0.0, 1.0)
    }

    /// Fraction of the wall (from the endocardium) a fully perfused cell column has injured.
    #[must_use]
    pub fn transmural_fraction(&self) -> f64 {
        if self.branch == CoronaryBranch::Pericardium {
            return 0.0;
        }
        ((self.severity - f64::from(THRESHOLD_ENDO)) / f64::from(THRESHOLD_RISE)).clamp(0.0, 1.0)
    }
}

/// Error of [`IschaemiaState::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownBranch(pub String);

impl std::fmt::Display for UnknownBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown coronary branch `{}`", self.0)
    }
}

impl std::error::Error for UnknownBranch {}

/// What a `Stenosis` branch string addresses.
enum Target {
    /// Every lesion (`"all"`; only meaningful with severity 0, which clears them).
    All,
    One(CoronaryBranch, LesionKind),
}

/// Parse a `Stenosis` branch string: `"<id>"` (acute), `"<id> infarct"`, `"<id> evolving"` or
/// `"all"`.
fn parse_target(s: &str) -> Option<Target> {
    let mut words = s.split_whitespace();
    let id = words.next()?;
    if id.eq_ignore_ascii_case("all") || id.eq_ignore_ascii_case("none") {
        return Some(Target::All);
    }
    let branch = CoronaryBranch::from_id(id)?;
    let kind = match words.next() {
        None => LesionKind::Acute,
        Some(w) if w.eq_ignore_ascii_case("infarct") || w.eq_ignore_ascii_case("old") => LesionKind::Infarct,
        Some(w) if w.eq_ignore_ascii_case("evolving") => LesionKind::Evolving,
        Some(_) => return None,
    };
    words.next().is_none().then_some(Target::One(branch, kind))
}

/// The lesions in effect, one per (branch, kind), sorted (deterministic iteration).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IschaemiaState {
    lesions: Vec<Lesion>,
}

impl IschaemiaState {
    /// Apply `Action::Stenosis { branch, severity }`: set (or with severity ≤ 0 remove) the
    /// lesion; `"all"` with severity 0 removes every lesion.
    pub fn apply(&mut self, branch: &str, severity: f64) -> Result<(), UnknownBranch> {
        let target = parse_target(branch).ok_or_else(|| UnknownBranch(branch.to_string()))?;
        let severity = severity.clamp(0.0, 1.0);
        match target {
            Target::All => {
                if severity <= 0.0 {
                    self.lesions.clear();
                }
            }
            Target::One(b, kind) => {
                self.lesions.retain(|l| (l.branch, l.kind) != (b, kind));
                if severity > 0.0 {
                    self.lesions.push(Lesion {
                        branch: b,
                        kind,
                        severity,
                    });
                    self.lesions.sort_by_key(|l| (l.branch, l.kind));
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn lesions(&self) -> &[Lesion] {
        &self.lesions
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lesions.is_empty()
    }

    pub fn clear(&mut self) {
        self.lesions.clear();
    }

    /// ST injury vector for Tier-0 synthesis, heart frame, mV: per acute lesion, its branch
    /// direction × 0.3 mV × severity drive, pointing *into* the territory when transmural
    /// (ST elevation facing it) and away from it when only the subendocardium is injured
    /// (ST depression); global subendocardial ischaemia and pericarditis carry their own
    /// sign in the direction. An evolving infarct adds a small residual elevation; a healed one
    /// adds none.
    #[must_use]
    pub fn st_vector_mv(&self) -> [f64; 3] {
        const TRANSMURAL_MV: f64 = 0.3;
        const SUBENDOCARDIAL_MV: f64 = 0.15;
        const RESIDUAL_MV: f64 = 0.05;
        let mut v = [0.0; 3];
        for l in self.lesions.iter().filter(|l| l.kind != LesionKind::Infarct) {
            let d = l.branch.injury_direction();
            let mag = match l.branch {
                _ if l.kind == LesionKind::Evolving => RESIDUAL_MV * l.transmural_fraction(),
                CoronaryBranch::Pericardium => 0.6 * TRANSMURAL_MV * l.severity,
                CoronaryBranch::Global => 2.0 * SUBENDOCARDIAL_MV * l.transmural_fraction().min(0.5),
                _ => {
                    let f = l.transmural_fraction();
                    if f >= 0.99 {
                        TRANSMURAL_MV
                    } else {
                        -SUBENDOCARDIAL_MV * (f / 0.5).min(1.0)
                    }
                }
            };
            for (o, di) in v.iter_mut().zip(d) {
                *o += mag * di;
            }
        }
        v
    }

    /// What the necrotic lesions in effect do to conducted beats on the Tier-0 strip. The depth of
    /// the necrosis follows the wavefront rule ([`Lesion::transmural_fraction`]): Q waves belong to
    /// transmural infarcts, so a non-transmural lesion draws proportionally less. This is the same
    /// lesion the mesh turns into a silent core, so the strip and the heart cannot disagree.
    #[must_use]
    pub fn infarct_forces(&self) -> InfarctForces {
        let mut f = InfarctForces::default();
        for l in self.lesions.iter().filter(|l| l.kind.is_necrotic()) {
            let Some(p) = l.branch.infarct_pattern() else {
                continue;
            };
            let depth = l.transmural_fraction();
            let stage = depth * l.kind.t_inversion();
            let r = p.frontal_axis_deg.to_radians();
            let axis = [r.cos_d(), -r.sin_d()];
            for (i, a) in axis.iter().enumerate() {
                f.q_limb[i] += a * p.frontal_gain * depth;
                f.t_limb[i] += a * p.frontal_gain * stage;
            }
            for i in 0..6 {
                f.q_precordial[i] = (f.q_precordial[i] + p.precordial[i] * depth).clamp(-1.0, 1.0);
                f.t_precordial[i] = (f.t_precordial[i] + p.precordial[i] * stage).clamp(-1.0, 1.0);
            }
        }
        f
    }
}

/// Teaching presets: each is a short list of `Stenosis` actions (first clearing every lesion).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IschaemiaPreset {
    AnteriorStemi,
    InferiorStemi,
    LateralStemi,
    PosteriorStemi,
    SubendocardialIschaemia,
    Pericarditis,
    OldAnteriorInfarct,
    OldInferiorInfarct,
}

impl IschaemiaPreset {
    pub const ALL: [IschaemiaPreset; 8] = [
        IschaemiaPreset::AnteriorStemi,
        IschaemiaPreset::InferiorStemi,
        IschaemiaPreset::LateralStemi,
        IschaemiaPreset::PosteriorStemi,
        IschaemiaPreset::SubendocardialIschaemia,
        IschaemiaPreset::Pericarditis,
        IschaemiaPreset::OldAnteriorInfarct,
        IschaemiaPreset::OldInferiorInfarct,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            IschaemiaPreset::AnteriorStemi => "Anterior STEMI (LAD)",
            IschaemiaPreset::InferiorStemi => "Inferior STEMI (RCA)",
            IschaemiaPreset::LateralStemi => "Lateral STEMI (OM)",
            IschaemiaPreset::PosteriorStemi => "Posterior STEMI (PL)",
            IschaemiaPreset::SubendocardialIschaemia => "Subendocardial ischaemia",
            IschaemiaPreset::Pericarditis => "Pericarditis",
            IschaemiaPreset::OldAnteriorInfarct => "Old anterior infarct",
            IschaemiaPreset::OldInferiorInfarct => "Old inferior infarct",
        }
    }

    /// What the learner should look for.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            IschaemiaPreset::AnteriorStemi => {
                "Occluded LAD: ST elevation V1–V4 (± I, aVL), reciprocal ST depression in II, III, aVF."
            }
            IschaemiaPreset::InferiorStemi => {
                "Occluded RCA: ST elevation II, III, aVF (III > II), reciprocal depression in I and aVL; RV involvement shows in V4R."
            }
            IschaemiaPreset::LateralStemi => "Occluded obtuse marginal: ST elevation I, aVL, V5–V6.",
            IschaemiaPreset::PosteriorStemi => {
                "Occluded posterolateral branch: ST depression V1–V3 (the mirror image), ST elevation V7–V9."
            }
            IschaemiaPreset::SubendocardialIschaemia => {
                "Demand ischaemia of the inner LV wall: diffuse ST depression, ST elevation in aVR."
            }
            IschaemiaPreset::Pericarditis => {
                "Diffuse epicardial injury: concave ST elevation in most leads, ST depression in aVR, no reciprocal territory."
            }
            IschaemiaPreset::OldAnteriorInfarct => {
                "Healed LAD infarct: non-conducting scar with a slow border zone, Q waves V1–V4, no ST elevation."
            }
            IschaemiaPreset::OldInferiorInfarct => "Healed RCA infarct: Q waves in II, III, aVF.",
        }
    }

    /// The `Stenosis` actions of this preset (the first one clears every lesion).
    #[must_use]
    pub fn actions(self) -> Vec<Action> {
        let s = |branch: &str, severity: f64| Action::Stenosis {
            branch: branch.to_string(),
            severity,
        };
        let lesion = match self {
            IschaemiaPreset::AnteriorStemi => s("LAD", 1.0),
            IschaemiaPreset::InferiorStemi => s("RCA", 1.0),
            IschaemiaPreset::LateralStemi => s("OM", 1.0),
            IschaemiaPreset::PosteriorStemi => s("PL", 1.0),
            IschaemiaPreset::SubendocardialIschaemia => s("global", 0.6),
            IschaemiaPreset::Pericarditis => s("pericardium", 1.0),
            IschaemiaPreset::OldAnteriorInfarct => s("LAD infarct", 1.0),
            IschaemiaPreset::OldInferiorInfarct => s("RCA infarct", 1.0),
        };
        vec![s("all", 0.0), lesion]
    }
}

/// Per-cell result of the lesions on a cell cloud (see the module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct InjuryField {
    /// Resting-potential rise, normalised V_m.
    pub rest_offset: Vec<f32>,
    /// Plateau (AP amplitude) loss, normalised V_m.
    pub plateau_loss: Vec<f32>,
    /// APD multiplier.
    pub apd_scale: Vec<f32>,
    /// CV multiplier.
    pub cv_scale: Vec<f32>,
    /// Necrotic, non-excitable infarct core.
    pub scar: Vec<bool>,
    /// Largest injury of any lesion (0 … 1; acute lesions: electrical expression), for display.
    pub injury: Vec<f32>,
}

impl InjuryField {
    /// No lesion.
    #[must_use]
    pub fn healthy(n: usize) -> Self {
        Self {
            rest_offset: vec![0.0; n],
            plateau_loss: vec![0.0; n],
            apd_scale: vec![1.0; n],
            cv_scale: vec![1.0; n],
            scar: vec![false; n],
            injury: vec![0.0; n],
        }
    }
}

/// Transmural depth from the endocardium, 0 … 1. Both septal faces are endocardium (cloud depth
/// 0 = LV side, 1 = RV side), so the septum's "epicardium" is its mid-wall.
#[must_use]
pub fn transmural_depth(region: Region, depth: f32) -> f32 {
    let d = depth.clamp(0.0, 1.0);
    if region == Region::VentricularSeptum {
        1.0 - (2.0 * d - 1.0).abs()
    } else {
        d
    }
}

/// Territory weight of `branch` per cell: membership smoothed over the ventricular graph.
#[must_use]
pub fn territory_weight(
    cloud: &CellCloud,
    graph: &Graph,
    kinds: &[CellKind],
    aha_segments: &[u8],
    branch: CoronaryBranch,
) -> Vec<f32> {
    let n = cloud.len();
    let vent = |i: usize| kinds[i] == CellKind::Ventricular;
    let mut w: Vec<f32> = (0..n)
        .map(|i| {
            let seg = aha_segments.get(i).copied().unwrap_or(0);
            f32::from(u8::from(vent(i) && branch.perfuses(cloud.regions[i], seg)))
        })
        .collect();
    if matches!(branch, CoronaryBranch::Pericardium) {
        return w;
    }
    let mut next = w.clone();
    for _ in 0..TERRITORY_SMOOTHING_PASSES {
        for i in 0..n {
            if !vent(i) {
                continue;
            }
            let (mut sum, mut count) = (w[i], 1.0f32);
            for e in graph.edges(i) {
                let j = graph.targets[e] as usize;
                if vent(j) {
                    sum += w[j];
                    count += 1.0;
                }
            }
            next[i] = sum / count;
        }
        std::mem::swap(&mut w, &mut next);
    }
    w
}

/// The per-cell effect of `lesions` (see the module docs). Only ventricular myocardium is
/// affected; overlapping lesions take the larger injury.
#[must_use]
pub fn injury_field(
    cloud: &CellCloud,
    graph: &Graph,
    kinds: &[CellKind],
    aha_segments: &[u8],
    lesions: &[Lesion],
) -> InjuryField {
    let n = cloud.len();
    let mut acute = vec![0.0f32; n];
    let mut infarct = vec![0.0f32; n];
    for l in lesions {
        let w = territory_weight(cloud, graph, kinds, aha_segments, l.branch);
        let target = if l.kind.is_necrotic() {
            &mut infarct
        } else {
            &mut acute
        };
        for i in 0..n {
            if kinds[i] != CellKind::Ventricular || w[i] <= 0.0 {
                continue;
            }
            let rho = transmural_depth(cloud.regions[i], cloud.depth[i]);
            let m = l.injury(w[i], rho);
            let e = if l.kind.is_necrotic() {
                m
            } else {
                m * Lesion::expression(rho)
            };
            target[i] = target[i].max(e);
        }
    }
    let mut f = InjuryField::healthy(n);
    for i in 0..n {
        let (m, inf) = (acute[i], infarct[i]);
        if inf >= INFARCT_CORE {
            f.scar[i] = true;
        } else if inf > 0.0 {
            f.cv_scale[i] = BORDER_ZONE_CV_SCALE;
            f.apd_scale[i] = BORDER_ZONE_APD_SCALE;
        }
        if m > 0.0 && !f.scar[i] {
            f.rest_offset[i] = INJURY_REST_OFFSET * m;
            f.plateau_loss[i] = INJURY_PLATEAU_LOSS * m;
            f.apd_scale[i] *= 1.0 - INJURY_APD_SHORTENING * m;
            f.cv_scale[i] *= 1.0 - INJURY_CV_LOSS * m;
        }
        f.injury[i] = m.max(inf);
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn necrotic_lesions_draw_q_waves_and_only_an_evolving_one_inverts_t_fully() {
        let forces = |branch: &str| {
            let mut s = IschaemiaState::default();
            s.apply(branch, 1.0).unwrap();
            s.infarct_forces()
        };
        assert!(forces("LAD").is_none(), "acute lesions have no necrosis");
        assert!(forces("global infarct").is_none(), "no territory");
        let healed = forces("LAD infarct");
        let evolving = forces("LAD evolving");
        assert_eq!(healed.q_precordial, evolving.q_precordial);
        assert_eq!(evolving.t_precordial[1], 1.0);
        assert!(healed.t_precordial[1] < 0.5);
        // a non-transmural lesion draws less than a transmural one
        let mut s = IschaemiaState::default();
        s.apply("LAD evolving", 0.5).unwrap();
        assert!(s.infarct_forces().q_precordial[1] < 0.5);
    }

    #[test]
    fn an_evolving_infarct_keeps_less_st_than_an_acute_lesion() {
        let st = |branch: &str| {
            let mut s = IschaemiaState::default();
            s.apply(branch, 1.0).unwrap();
            s.st_vector_mv()[2]
        };
        assert!(st("LAD evolving") > 0.0 && st("LAD evolving") < 0.3 * st("LAD"));
        assert_eq!(st("LAD infarct"), 0.0);
    }

    #[test]
    fn wavefront_rule() {
        let l = |severity| Lesion {
            branch: CoronaryBranch::Lad,
            kind: LesionKind::Acute,
            severity,
        };
        // occlusion: the whole wall
        assert_eq!(l(0.9).injury(1.0, 0.0), 1.0);
        assert!(l(0.9).injury(1.0, 1.0) > 0.99);
        assert!((l(0.9).transmural_fraction() - 1.0).abs() < 1e-9);
        // moderate stenosis: subendocardium only
        assert!(l(0.55).injury(1.0, 0.0) > 0.9);
        assert_eq!(l(0.55).injury(1.0, 0.8), 0.0);
        assert!(l(0.55).transmural_fraction() < 0.6);
        assert!(l(0.8).injury(1.0, 1.0) > 0.99, "0.8 is already transmural");
        // mild: nothing
        assert_eq!(l(0.4).injury(1.0, 0.0), 0.0);
        // at the territory border only the subendocardium is injured
        assert!(l(0.9).injury(0.6, 1.0) < 0.5);
        // expressed most at the epicardium
        assert!(Lesion::expression(1.0) > 2.0 * Lesion::expression(0.0));
    }

    #[test]
    fn branch_targets_parse() {
        let mut s = IschaemiaState::default();
        s.apply("lad", 0.9).unwrap();
        s.apply("RCA infarct", 1.0).unwrap();
        s.apply("RCA", 0.5).unwrap();
        assert_eq!(s.lesions().len(), 3);
        s.apply("RCA", 0.0).unwrap();
        assert_eq!(s.lesions().len(), 2);
        assert!(s.apply("LAD sideways", 1.0).is_err());
        assert!(s.apply("septal perforator", 1.0).is_err());
        s.apply("all", 0.0).unwrap();
        assert!(s.is_empty());
        // presets clear first
        for p in IschaemiaPreset::ALL {
            let mut s = IschaemiaState::default();
            s.apply("LCx", 1.0).unwrap();
            for a in p.actions() {
                let Action::Stenosis { branch, severity } = a else {
                    panic!("preset action is a stenosis")
                };
                s.apply(&branch, severity).unwrap();
            }
            assert_eq!(s.lesions().len(), 1, "{p:?}");
        }
    }

    #[test]
    fn territories_follow_the_aha_assignment() {
        let v = Region::LeftVentricle;
        assert!(CoronaryBranch::Lad.perfuses(v, 17));
        assert!(CoronaryBranch::Rca.perfuses(Region::RightVentricle, 0));
        assert!(!CoronaryBranch::Pda.perfuses(Region::RightVentricle, 0));
        assert!(CoronaryBranch::LeftMain.perfuses(v, 6));
        assert!(!CoronaryBranch::Lcx.perfuses(v, 1));
        for seg in 1..=17u8 {
            let owners = [CoronaryBranch::Lad, CoronaryBranch::Lcx, CoronaryBranch::Rca]
                .into_iter()
                .filter(|b| b.perfuses(v, seg))
                .count();
            // the anterolateral segments are shared by the diagonals and the marginals
            let expected = if matches!(seg, 6 | 12) { 2 } else { 1 };
            assert_eq!(owners, expected, "major arteries of segment {seg}");
        }
    }

    #[test]
    fn st_vector_points_into_a_transmural_territory_and_away_from_a_subendocardial_one() {
        let mut s = IschaemiaState::default();
        s.apply("LAD", 1.0).unwrap();
        assert!(s.st_vector_mv()[2] > 0.2, "anterior");
        s.apply("LAD", 0.55).unwrap();
        assert!(s.st_vector_mv()[2] < 0.0, "subendocardial: reversed");
        s.apply("LAD", 0.0).unwrap();
        s.apply("RCA", 1.0).unwrap();
        assert!(s.st_vector_mv()[1] < -0.2, "inferior");
        s.apply("RCA", 0.0).unwrap();
        s.apply("RCA infarct", 1.0).unwrap();
        assert_eq!(s.st_vector_mv(), [0.0; 3], "healed infarct: no injury current");
    }
}
