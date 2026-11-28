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
    /// Breath-hold (brief B28): hold the respiratory phase at `phase`, or breathe again from the
    /// held phase with `None`. While held, the sinus node's respiratory modulation and the S2 split
    /// stop changing with the breath, so a split can be studied beat after beat. Only the timing
    /// engine honours it (the respiratory clock lives there); a scenario that never uses it runs
    /// bit for bit as before.
    BreathHold {
        phase: Option<BreathPhase>,
    },
}

/// Where a breath-hold stops the respiratory cycle (`Action::BreathHold`). Other manoeuvres
/// (Valsalva, squatting, handgrip) change venous return and afterload, which the timing model does
/// not have, so they are not offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BreathPhase {
    /// After a breath out: the lungs at their resting volume, the S2 split at its narrowest and the
    /// sinus cycle at its longest.
    EndExpiration,
    /// At the top of a breath in: the S2 split at its widest and the sinus cycle at its shortest.
    PeakInspiration,
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

/// A valve or shunt lesion of the `auscultation` section (brief B27). Each one adds its murmur and
/// changes the valve sounds the way the cited texts describe (`docs/research/heart-sounds.md`); the
/// ECG is not touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LesionKind {
    /// Crescendo–decrescendo ejection murmur peaking later with severity; soft, single A2 when severe.
    AorticStenosis,
    /// High-pitched decrescendo diastolic murmur from A2.
    AorticRegurgitation,
    /// Plateau (holosystolic) murmur from S1 through A2, little change with the cycle length.
    MitralRegurgitation,
    /// Loud S1, opening snap, low mid-diastolic rumble with presystolic accentuation in sinus rhythm.
    MitralStenosis,
    /// Mid-systolic click followed by a late systolic murmur to A2.
    MitralValveProlapse,
    /// Plateau murmur from S1 to P2, louder in inspiration.
    TricuspidRegurgitation,
    /// Ejection murmur of the right side; P2 soft and late.
    PulmonaryStenosis,
    /// Plateau, high-pitched, holosystolic murmur.
    VentricularSeptalDefect,
    /// Soft pulmonary flow (ejection) murmur; the fixed split is the scenario's `fixed_split`.
    AtrialSeptalDefect,
    /// Innocent (Still's) murmur: short, early-to-mid systolic, low and musical; S2 normal.
    InnocentMurmur,
}

/// How severe a lesion is: sets the murmur's gain and, where the sources describe it, its shape
/// (later peak of aortic stenosis, longer rumble and earlier snap of mitral stenosis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Severity {
    Mild,
    #[default]
    Moderate,
    Severe,
}

/// The optional `auscultation` section of a scenario (brief B27): what is wrong with the valves or
/// the ventricles. Every field defaults to "normal", so a file without the section, or with an
/// empty one, sounds exactly as the rhythm alone makes it (the heart sounds of brief B24). It never
/// changes the ECG.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent findings of the scenario file, each a plain yes/no in the RON text"
)]
pub struct Auscultation {
    /// Third heart sound after every beat that ejected (heart failure).
    pub s3: bool,
    /// Fourth heart sound after every organised atrial activation that falls in ventricular
    /// diastole (never in atrial fibrillation or with silent atria).
    pub s4: bool,
    /// Pericardial friction rub: up to three scratchy components per cycle.
    pub rub: bool,
    /// Wide S2 split that hardly changes with respiration (atrial septal defect).
    pub fixed_split: bool,
    /// Valve and shunt lesions, each with its severity.
    pub lesions: Vec<(LesionKind, Severity)>,
    /// The ECG itself is the lesson and the sounds accompany it (pericarditis with its rub, an
    /// infarction with an S4): the scenario stays in the rhythm list and in the ECG quizzes. Without
    /// it a scenario with a non-empty section is an auscultation case, whose ordinary ECG would be a
    /// wrong quiz question ([`Scenario::is_auscultation_case`]).
    pub ecg_diagnostic: bool,
}

impl Auscultation {
    /// No extra sound and no lesion: the heart sounds are the rhythm's alone.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.s3 && !self.s4 && !self.rub && !self.fixed_split && self.lesions.is_empty()
    }

    /// The severity of `kind` if the section lists it (the first entry when listed twice).
    #[must_use]
    pub fn lesion(&self, kind: LesionKind) -> Option<Severity> {
        self.lesions.iter().find(|(k, _)| *k == kind).map(|(_, s)| *s)
    }
}

/// An extra heart sound a scenario must produce (`ExpectedFindings::extra_sounds`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExtraSound {
    S3,
    S4,
    /// Aortic or pulmonary ejection click.
    EjectionClick,
    /// The mid-systolic click of mitral valve prolapse.
    MidSystolicClick,
    OpeningSnap,
    FrictionRub,
}

/// Where in the cycle a murmur lies (`ExpectedFindings::murmurs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MurmurTiming {
    /// Crescendo–decrescendo between S1 and S2, with a gap after S1 (ejection murmurs).
    MidSystolic,
    /// Short and early-to-mid systolic (the innocent murmur).
    EarlySystolic,
    /// A plateau from S1 to or through S2.
    Holosystolic,
    /// From a mid-systolic click to S2.
    LateSystolic,
    /// Decrescendo from S2.
    EarlyDiastolic,
    /// After the opening snap, in mid-diastole.
    MidDiastolic,
    /// A crescendo into S1 after the atrial contraction.
    Presystolic,
}

/// How S2 splits over the respiratory cycle (`ExpectedFindings::s2_split`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SplitMode {
    /// Narrow in expiration, wider in inspiration, A2 first.
    Physiological,
    /// Wide in both phases and wider in inspiration (late P2).
    Wide,
    /// Wide and changing by less than 20 ms with respiration.
    Fixed,
    /// P2 before A2 in expiration, narrowing in inspiration.
    Paradoxical,
    /// Within 20 ms in both phases: heard as one sound.
    Single,
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
    /// Extra heart sounds the phonocardiogram must contain (brief B27; `None` = not a criterion).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_sounds: Option<Vec<ExtraSound>>,
    /// The timing class of every murmur the phonocardiogram must contain (`Some(vec![])`: none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub murmurs: Option<Vec<MurmurTiming>>,
    /// How S2 splits over the respiratory cycle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s2_split: Option<SplitMode>,
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
    /// Extra heart sounds and valve lesions (brief B27). Left out of a file, and of a written file
    /// when empty, so the rhythm presets keep their text.
    #[serde(default, skip_serializing_if = "Auscultation::is_empty")]
    pub auscultation: Auscultation,
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
            auscultation: Auscultation::default(),
        }
    }

    /// An auscultation case: a non-empty `auscultation` section whose ECG is not itself the
    /// diagnosis ([`Auscultation::ecg_diagnostic`]). Its strip is ordinary (sinus rhythm in the valve
    /// scenarios), so it is never drawn by the ECG quizzes and drills, and the scenario picker lists
    /// it under its own heading. The one predicate every pool uses.
    #[must_use]
    pub fn is_auscultation_case(&self) -> bool {
        !self.auscultation.is_empty() && !self.auscultation.ecg_diagnostic
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
        ron::ser::to_string_pretty(
            self,
            ron::ser::PrettyConfig::default()
                .struct_names(true)
                .new_line("\n"),
        )
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
    fn the_auscultation_section_is_optional_and_reads_lesions() {
        let base = r#"Scenario(format_version: 1, id: "x", title: "X", description: "d", tier: Template"#;
        let none = Scenario::from_ron(&format!("{base})")).unwrap();
        assert!(none.auscultation.is_empty());
        assert!(!none.is_auscultation_case());
        let empty = Scenario::from_ron(&format!("{base}, auscultation: ())")).unwrap();
        assert_eq!(empty, none);
        let text = format!(
            "{base}, auscultation: (s4: true, lesions: [(AorticStenosis, Severe), (MitralRegurgitation, Mild)]),
             expected: Some(ExpectedFindings(hr_bpm: (60.0, 80.0), extra_sounds: Some([S4]),
               murmurs: Some([MidSystolic, Holosystolic]), s2_split: Some(Single))))"
        );
        let s = Scenario::from_ron(&text).unwrap();
        assert!(s.auscultation.s4 && !s.auscultation.s3);
        assert_eq!(
            s.auscultation.lesion(LesionKind::AorticStenosis),
            Some(Severity::Severe)
        );
        assert_eq!(s.auscultation.lesion(LesionKind::MitralStenosis), None);
        assert!(s.is_auscultation_case());
        let e = s.expected.clone().unwrap();
        assert_eq!(e.s2_split, Some(SplitMode::Single));
        assert_eq!(e.murmurs.unwrap().len(), 2);
        let back = Scenario::from_ron(&s.to_ron().unwrap()).unwrap();
        assert_eq!(back, s);
        // the ECG is the lesson: the sounds ride along, the scenario stays an ECG case
        let diag = Scenario::from_ron(&format!(
            "{base}, auscultation: (rub: true, ecg_diagnostic: true))"
        ))
        .unwrap();
        assert!(!diag.auscultation.is_empty() && !diag.is_auscultation_case());
        // an empty section is not written back
        assert!(
            !Scenario::sinus_normal()
                .to_ron()
                .unwrap()
                .contains("auscultation")
        );
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
