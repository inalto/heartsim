//! Scenario files (`*.ron`): everything needed to reproduce a simulation — pathology recipe,
//! parameter overrides, interventions on a timeline, electrode placement, tutorial metadata.
//! Versioned so old files keep loading.

use crate::lead::Electrode;
use crate::params::{Parameters, RegionParams};
use crate::region::Region;
use crate::time::Millis;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCENARIO_FORMAT_VERSION: u32 = 1;

/// Which engine tier the scenario needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EngineTier {
    /// Template ECG synthesis only (no 3D propagation).
    Template,
    /// Conduction graph + eikonal activation + AP templates → pseudo-ECG (default).
    #[default]
    Eikonal,
    /// Reaction–diffusion (re-entry, fibrillation).
    ReactionDiffusion,
}

/// An intervention applied at a point in simulated time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// Change a global or regional parameter.
    SetGlobalSinusRate {
        bpm: f64,
    },
    SetRegion {
        region: Region,
        params: RegionParams,
    },
    /// Make a region non-conducting (block) or restore it.
    Block {
        region: Region,
        blocked: bool,
    },
    /// Programmed stimulus at a cell / landmark.
    Stimulate {
        site: StimSite,
        strength: f64,
        duration_ms: f64,
    },
    /// Pacing train: S1×n at `cycle_ms`, then optional S2, S3 coupling intervals.
    PacingTrain {
        site: StimSite,
        s1_cycle_ms: f64,
        s1_count: u32,
        s2_ms: Option<f64>,
        s3_ms: Option<f64>,
    },
    /// Ablate cells within `radius_mm` of a site.
    Ablate {
        site: StimSite,
        radius_mm: f64,
    },
    /// Enable/disable an accessory pathway.
    AccessoryPathway {
        enabled: bool,
        antegrade: bool,
        retrograde: bool,
    },
    /// Coronary branch stenosis (0 = none, 1 = occlusion).
    Stenosis {
        branch: String,
        severity: f64,
    },
    /// Defibrillation shock (resets all excitable tissue), optionally synchronised on R.
    Shock {
        joules: f64,
        synchronised: bool,
    },
    /// Drug effect preset by name (adenosine, verapamil, flecainide, digoxin, …).
    Drug {
        name: String,
        dose: f64,
    },
    /// Move an electrode (torso-surface coordinates, mm).
    MoveElectrode {
        electrode: Electrode,
        position_mm: [f64; 3],
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StimSite {
    /// A named landmark from the anatomy pack (e.g. "rv_apex", "his_prox", "cs_5_6").
    Landmark(String),
    /// Explicit cell index.
    Cell(u32),
    /// Nearest cell to a point in heart coordinates (mm).
    Point([f64; 3]),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedAction {
    pub at: Millis,
    pub action: Action,
}

/// A closed range `(lo, hi)` of an expected measurement.
pub type Range = (f64, f64);

/// The rhythm name the B4 analyser gives a strip when it can name one from the waveform alone
/// (`heartsim_ecg::analysis`). Every other rhythm is described by its measurements instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RhythmKind {
    /// Regular atrial macro-re-entry at about 300/min: sawtooth flutter waves, no isoelectric
    /// baseline, a fixed or variable AV ratio.
    AtrialFlutter,
    /// No P waves, an irregular fibrillatory baseline, irregularly irregular narrow QRS.
    AtrialFibrillation,
    /// AF conducted over an accessory pathway: fast, broad, irregular.
    PreExcitedAtrialFibrillation,
    /// No identifiable QRS complexes: chaotic deflections of varying amplitude.
    VentricularFibrillation,
    /// Polymorphic VT whose QRS axis twists around the baseline, in a long QT setting.
    TorsadesDePointes,
    /// Polymorphic VT with beat-to-beat QRS changes and no orderly twist.
    PolymorphicVentricularTachycardia,
}

/// What the B4 analyser must measure on the scenario's ECG once the rhythm has settled — the
/// textbook diagnostic criteria of the rhythm, so a regression in an engine, the synthesiser or
/// the analyser shows up as a scenario that no longer "looks like" its diagnosis.
/// `None` = not a criterion of this rhythm (e.g. PR in complete AV block).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectedFindings {
    /// Ventricular rate, beats/min. May be left out when the rhythm has no countable beats
    /// (ventricular fibrillation): see [`ExpectedFindings::NO_RATE`].
    #[serde(default = "no_rate")]
    pub hr_bpm: Range,
    /// PR interval (P onset → QRS onset), ms.
    #[serde(default)]
    pub pr_ms: Option<Range>,
    /// QRS duration, ms.
    #[serde(default)]
    pub qrs_ms: Option<Range>,
    /// Frontal QRS axis, degrees. `lo > hi` wraps through ±180° (e.g. `(150, -90)`).
    #[serde(default)]
    pub axis_deg: Option<Range>,
    /// The name the analyser must give the rhythm (`None` = not a criterion).
    #[serde(default)]
    pub rhythm: Option<RhythmKind>,
}

fn no_rate() -> Range {
    ExpectedFindings::NO_RATE
}

impl ExpectedFindings {
    /// `hr_bpm` of a rhythm with no countable beats: the rate is then not a criterion.
    pub const NO_RATE: Range = (0.0, 0.0);

    /// Whether `hr_bpm` is a criterion of this rhythm.
    #[must_use]
    pub fn rate_is_criterion(&self) -> bool {
        self.hr_bpm != Self::NO_RATE
    }

    /// `true` if `deg` lies on the arc from `lo` to `hi` (counter-clockwise on the hexaxial
    /// circle, i.e. increasing angle), so ranges through ±180° work.
    #[must_use]
    pub fn axis_contains(range: Range, deg: f64) -> bool {
        let n = |d: f64| (d % 360.0 + 360.0) % 360.0;
        let width = n(range.1 - range.0);
        n(deg - range.0) <= width
    }
}

/// A complete scenario. Fields marked `#[serde(default)]` may be omitted from the file, so a
/// rhythm preset only states what differs from the healthy default.
///
/// Engine-specific sections (`timing` for `heartsim_sim::TimingParams`, `morphology` for the
/// Tier-0 synthesiser) live in the same file; this crate cannot name their types (layering) and
/// ignores them, `heartsim_sim` / `heartsim_ecg` read them from the same text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub format_version: u32,
    /// Stable id, e.g. "sinus_normal", "lbbb", "af". Equals the file stem.
    pub id: String,
    pub title: String,
    pub description: String,
    pub tier: EngineTier,
    /// Anatomy pack id (mesh + cell cloud + landmarks), see `heartsim-geometry`.
    #[serde(default = "default_anatomy")]
    pub anatomy: String,
    #[serde(default)]
    pub parameters: Parameters,
    /// Cells rendered non-excitable at t = 0 (scar / previous ablation), by index.
    #[serde(default)]
    pub ablated_cells: Vec<u32>,
    #[serde(default)]
    pub timeline: Vec<TimedAction>,
    /// Optional electrode overrides (default positions come from the anatomy pack).
    #[serde(default)]
    pub electrodes_mm: BTreeMap<Electrode, [f64; 3]>,
    /// Additional unipolar chest electrodes that are switched on, by name (`V3R`, `V4R`, `V7`,
    /// `V8`, `V9`), heart frame mm. Each adds one lead (`φ − WCT`) to the recording.
    #[serde(default)]
    pub extra_electrodes_mm: BTreeMap<String, [f64; 3]>,
    /// Deterministic seed for noise, ectopy timing, fibrillation initiation.
    #[serde(default)]
    pub seed: u64,
    /// Teaching text and quiz hooks (markdown), optional.
    #[serde(default)]
    pub notes: Option<String>,
    /// Ids or names of the rhythms a learner most often mistakes this one for (teaching
    /// differential), e.g. `["avb_2nd_mobitz2_3to1", "blocked PACs"]`.
    #[serde(default)]
    pub confused_with: Vec<String>,
    /// Open literature the teaching text was checked against (guideline, textbook chapter, LITFL
    /// page), one citation per entry. Older files keep theirs in a `Sources:` block at the end of
    /// `notes`; [`Scenario::citations`] reads both.
    #[serde(default)]
    pub references: Vec<String>,
    /// Textbook measurement ranges the rhythm must satisfy (golden tests, quiz answers).
    #[serde(default)]
    pub expected: Option<ExpectedFindings>,
}

fn default_anatomy() -> String {
    "default".into()
}

impl Scenario {
    #[must_use]
    pub fn sinus_normal() -> Self {
        Self {
            format_version: SCENARIO_FORMAT_VERSION,
            id: "sinus_normal".into(),
            title: "Sinus rhythm, normal axis".into(),
            description: "Healthy adult heart at 70 bpm.".into(),
            tier: EngineTier::Eikonal,
            anatomy: "default".into(),
            parameters: Parameters::default(),
            ablated_cells: Vec::new(),
            timeline: Vec::new(),
            electrodes_mm: BTreeMap::new(),
            extra_electrodes_mm: BTreeMap::new(),
            seed: 42,
            notes: None,
            confused_with: Vec::new(),
            references: Vec::new(),
            expected: None,
        }
    }

    /// The citations of the teaching text: the `references` field plus the `- ` lines of a
    /// trailing `Sources:` block in `notes` (the layout of the first 33 presets).
    #[must_use]
    pub fn citations(&self) -> Vec<String> {
        let mut out = self.references.clone();
        if let Some(notes) = &self.notes
            && let Some((_, block)) = notes.split_once("\n\nSources:")
        {
            out.extend(
                block
                    .lines()
                    .filter_map(|l| l.trim().strip_prefix("- "))
                    .map(|l| l.trim().to_string()),
            );
        }
        out
    }

    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default().struct_names(true))
    }

    pub fn from_ron(text: &str) -> Result<Self, ScenarioError> {
        let s: Self = ron::from_str(text)?;
        if s.format_version > SCENARIO_FORMAT_VERSION {
            return Err(ScenarioError::Version(s.format_version));
        }
        Ok(s)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScenarioError {
    #[error("scenario parse error: {0}")]
    Parse(#[from] ron::error::SpannedError),
    #[error("scenario format version {0} is newer than supported ({SCENARIO_FORMAT_VERSION})")]
    Version(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ron_roundtrip_with_timeline() {
        let mut s = Scenario::sinus_normal();
        s.timeline.push(TimedAction {
            at: Millis(2000.0),
            action: Action::PacingTrain {
                site: StimSite::Landmark("rv_apex".into()),
                s1_cycle_ms: 600.0,
                s1_count: 8,
                s2_ms: Some(300.0),
                s3_ms: None,
            },
        });
        s.timeline.push(TimedAction {
            at: Millis(9000.0),
            action: Action::Shock {
                joules: 200.0,
                synchronised: true,
            },
        });
        let text = s.to_ron().unwrap();
        let back = Scenario::from_ron(&text).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn minimal_file_uses_defaults_and_ignores_engine_sections() {
        let text = r#"Scenario(
            format_version: 1,
            id: "x",
            title: "X",
            description: "d",
            tier: Template,
            timing: TimingParams(sinus: SinusParams(rate_bpm: 45.0), foci: [Focus(region: LeftVentricle)]),
            confused_with: ["a", "b"],
            expected: Some(ExpectedFindings(hr_bpm: (40.0, 50.0), axis_deg: Some((150.0, -90.0)))),
        )"#;
        let s = Scenario::from_ron(text).unwrap();
        assert_eq!(s.anatomy, "default");
        assert_eq!(s.parameters, Parameters::default());
        assert_eq!(s.confused_with.len(), 2);
        let e = s.expected.unwrap();
        assert_eq!(e.pr_ms, None);
        assert!(ExpectedFindings::axis_contains(e.axis_deg.unwrap(), 180.0));
        assert!(ExpectedFindings::axis_contains(e.axis_deg.unwrap(), -120.0));
        assert!(!ExpectedFindings::axis_contains(e.axis_deg.unwrap(), 0.0));
        assert!(ExpectedFindings::axis_contains((-30.0, 90.0), 60.0));
        assert!(!ExpectedFindings::axis_contains((-30.0, 90.0), 120.0));
    }

    #[test]
    fn a_rhythm_without_countable_beats_may_omit_its_rate() {
        let text = r#"Scenario(
            format_version: 1,
            id: "x",
            title: "X",
            description: "d",
            tier: Template,
            expected: Some(ExpectedFindings(rhythm: Some(VentricularFibrillation))),
        )"#;
        let e = Scenario::from_ron(text).unwrap().expected.unwrap();
        assert!(!e.rate_is_criterion());
        assert_eq!(e.rhythm, Some(RhythmKind::VentricularFibrillation));
    }

    #[test]
    fn citations_come_from_the_field_and_the_notes_block() {
        let mut s = Scenario::sinus_normal();
        assert_eq!(s.citations(), Vec::<String>::new());
        s.notes = Some("Teaching text.\n\nSources:\n- A et al. 2001.\n- B 2002.".into());
        s.references = vec!["C 2003.".into()];
        assert_eq!(s.citations(), ["C 2003.", "A et al. 2001.", "B 2002."]);
    }

    #[test]
    fn rejects_future_version() {
        let mut s = Scenario::sinus_normal();
        s.format_version = SCENARIO_FORMAT_VERSION + 1;
        let text = s.to_ron().unwrap();
        assert!(matches!(
            Scenario::from_ron(&text),
            Err(ScenarioError::Version(_))
        ));
    }
}
