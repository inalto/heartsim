//! Tier 0 synthesis: each timing event spawns a set of Gaussian waves (McSharry et al. 2003
//! morphology) whose dipole directions are projected onto the 12 leads. Because the waves are
//! triggered by the shared timing engine, the strip stays consistent with any 3-D view.
//!
//! Each event picks a beat [`Template`] from what the engine reports: sinus, ectopic, retrograde
//! or paced P; normally conducted, bundle-branch-blocked (from the `Blocked` branch events of the
//! same beat), pre-excited, ectopic or paced QRS–T. Scenario files tune the shapes through
//! [`MorphologyOverrides`]; drugs, electrolytes and coronary lesions of the timing engine reach
//! every new beat through [`SynthModulation`] (brief B14).
//!
//! **Sustained arrhythmias.** A cycle of the timing engine's atrial or ventricular driver
//! (`Origin::Circuit`) spawns one wave, so the strip follows the engine event by event:
//!
//! * flutter: a sawtooth, negative in II, III and aVF and positive in V1 (counter-clockwise
//!   cavotricuspid flutter);
//! * atrial fibrillation: a small biphasic f wave of varying size and direction;
//! * ventricular fibrillation: a biphasic deflection in a random direction, no QRS or T wave;
//! * torsades de pointes: a large wide deflection whose direction turns a little with every beat,
//!   so a lead sees the complexes shrink, flip and grow again;
//! * polymorphic VT: the same deflection in a new random direction every beat.
//!
//! The biphasic waves have no net area, so a train of them keeps the baseline where it was. Their
//! randomness is a pure function of the event's time ([`event_noise`]), and that time comes from
//! the seeded engine, so a scenario and its seed fix the strip.
//!
//! **Infarction.** A necrotic lesion adds three per-lead waves to each conducted beat: a Q wave,
//! the loss of R, and the inversion of T, in the leads over the territory
//! (`heartsim_sim::ischaemia::InfarctPattern`).

use crate::leadvec::LeadVectors;
use glam::DVec3;
use heartsim_core::{FrontalAxis, LEAD_COUNT, Lead, Millis, Region, TwelveLead};
use heartsim_sim::defib::ShockRng;
use heartsim_sim::ischaemia::{InfarctForces, IschaemiaState};
use heartsim_sim::pharma::DrugEffects;
use heartsim_sim::timing::{MarkerKind, PacerMarker};
use heartsim_sim::{Circuit, Event, EventKind, Origin};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One Gaussian component: `a · exp(−((t − t0 − offset)² / (2 b²)))` along direction `dir`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaveSpec {
    pub name: &'static str,
    /// Peak dipole magnitude (mV in lead units when projected onto a unit lead vector).
    pub amplitude: f64,
    /// Standard deviation, ms.
    pub width_ms: f64,
    /// Delay of the peak after the triggering event, ms.
    pub offset_ms: f64,
    /// Frontal-plane angle of the dipole, degrees (hexaxial).
    pub axis_deg: f64,
    /// Anterior (+) / posterior (−) component, same units as the frontal unit vector.
    pub anterior: f64,
}

impl WaveSpec {
    #[must_use]
    pub fn direction(&self) -> DVec3 {
        (LeadVectors::frontal(self.axis_deg) + DVec3::new(0.0, 0.0, self.anterior)).normalize()
    }
}

/// Default morphology for a healthy adult (QRS axis +60°, T concordant).
#[must_use]
pub fn textbook_waves() -> (Vec<WaveSpec>, Vec<WaveSpec>) {
    let atrial = vec![WaveSpec {
        name: "P",
        amplitude: 0.15,
        width_ms: 22.0,
        offset_ms: 45.0,
        axis_deg: 60.0,
        anterior: 0.3,
    }];
    let ventricular = vec![
        WaveSpec {
            name: "Q",
            amplitude: -0.12,
            width_ms: 7.0,
            offset_ms: 8.0,
            axis_deg: 60.0,
            anterior: -0.6,
        },
        WaveSpec {
            name: "R",
            amplitude: 1.3,
            width_ms: 11.0,
            offset_ms: 28.0,
            axis_deg: 60.0,
            anterior: 0.45,
        },
        WaveSpec {
            name: "S",
            amplitude: -0.35,
            width_ms: 9.0,
            offset_ms: 48.0,
            axis_deg: 60.0,
            anterior: -0.9,
        },
        T_NORMAL,
    ];
    (atrial, ventricular)
}

const T_NORMAL: WaveSpec = WaveSpec {
    name: "T",
    amplitude: 0.35,
    width_ms: 55.0,
    offset_ms: 300.0,
    axis_deg: 45.0,
    anterior: 0.6,
};

const fn wave(
    name: &'static str,
    amplitude: f64,
    width_ms: f64,
    offset_ms: f64,
    axis_deg: f64,
    anterior: f64,
) -> WaveSpec {
    WaveSpec {
        name,
        amplitude,
        width_ms,
        offset_ms,
        axis_deg,
        anterior,
    }
}

/// The beat morphologies the synthesiser chooses between. The choice is made per timing event
/// from the event's region, kind and origin and from the bundle-branch block events of the same
/// beat, so the ECG always shows what the timing engine did (no separate "BBB switch").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Template {
    /// Sinus P wave (also atrial pacing). Stored in [`GaussianSynth::atrial`].
    SinusP,
    /// P wave of an ectopic atrial focus (PAC): lower, narrower, shifted axis.
    EctopicP,
    /// Retrograde P (VA conduction over the node or an accessory pathway): superior axis,
    /// negative in II, III and aVF.
    RetrogradeP,
    /// QRS–T conducted over an intact His–Purkinje system. Stored in
    /// [`GaussianSynth::ventricular`]; fascicular blocks rotate its QRS.
    Normal,
    /// Right bundle-branch block: normal initial forces plus a slow terminal right-anterior R′
    /// (rsR′ in V1, wide S in I and V6), QRS ≥ 120 ms, T discordant in V1.
    Rbbb,
    /// Left bundle-branch block: no septal q, slow leftward-posterior forces (QS in V1, broad
    /// notched R in I and V6), QRS ≥ 120 ms, T discordant.
    Lbbb,
    /// Delta wave of a left-sided accessory pathway (negative in I/aVL, positive in V1: "type A").
    DeltaLv,
    /// Delta wave of a right-sided accessory pathway (positive in I, negative in V1: "type B").
    DeltaRv,
    /// Beat arising in the left ventricle (PVC, escape, LV pacing): RBBB-like, inferior axis.
    EctopicLv,
    /// Beat arising in the right ventricle / RV apex (PVC, escape, RV pacing): LBBB-like,
    /// superior axis.
    EctopicRv,
    /// Pacing stimulus artefact (< 2 ms), added at every pacemaker output (captured or not)
    /// and every programmed stimulus.
    Spike,
    /// Biventricular (CRT) paced QRS–T: the RV-apex and LV-lateral fronts fuse, so the QRS is
    /// narrower than RV pacing, with a dominant R in V1 and a negative lead I (right-superior
    /// axis); T discordant (Barold & Herweg, *Cardiol J* 2011;18:610–628).
    Biventricular,
    /// One turn of a typical atrial-flutter circuit: the slow descending ramp and steep rise of
    /// the sawtooth in II, III and aVF, with no isoelectric segment between waves. Zero net area,
    /// so a run of them does not shift the baseline.
    FlutterWave,
    /// One fibrillatory atrial wavelet: a low biphasic deflection (f wave) in place of a P wave.
    FibrillatoryWave,
    /// One ventricular fibrillation wavelet: a biphasic deflection with no QRS and no T; each
    /// event varies in size and axis, so the strip has no repeating complex.
    VfWave,
    /// One beat of a polymorphic ventricular tachycardia or of torsades: a broad biphasic complex
    /// whose T is merged into the next beat. The event sets its size and axis.
    PolymorphicBeat,
    /// Fully pre-excited QRS–T of a left-sided accessory pathway: delta, broad R, discordant T.
    PreexcitedLv,
    /// Fully pre-excited QRS–T of a right-sided accessory pathway: delta, broad R, discordant T.
    PreexcitedRv,
}

/// Rotation of the QRS forces in left anterior fascicular block, degrees: the unopposed
/// late activation of the anterosuperior LV moves the axis to about −60° (criterion −45° to −90°,
/// Surawicz et al. 2009).
pub const LAFB_ROTATION_DEG: f64 = -120.0;
/// Rotation of the QRS forces in left posterior fascicular block, degrees: axis about +120°
/// (criterion +90° to +180°, Surawicz et al. 2009).
pub const LPFB_ROTATION_DEG: f64 = 60.0;
/// QRS widening of an isolated fascicular block (QRS stays < 120 ms).
const FASCICULAR_WIDTH_SCALE: f64 = 1.1;
/// Fridericia's exponent (QT ∝ RR^⅓), the default rate adaptation of scenario runs.
pub const FRIDERICIA_EXPONENT: f64 = 1.0 / 3.0;

/// Built-in waves of the non-default templates. Amplitudes in mV, widths are Gaussian σ.
#[must_use]
pub fn template_waves(t: Template) -> Vec<WaveSpec> {
    match t {
        Template::SinusP | Template::Normal => {
            let (a, v) = textbook_waves();
            if t == Template::SinusP { a } else { v }
        }
        Template::EctopicP => vec![wave("P", 0.12, 18.0, 45.0, 20.0, 0.1)],
        Template::RetrogradeP => vec![wave("P", 0.12, 16.0, 45.0, -90.0, 0.2)],
        Template::Rbbb => vec![
            wave("Q", -0.12, 7.0, 8.0, 60.0, -0.6),
            wave("R", 1.3, 11.0, 28.0, 60.0, -0.25),
            wave("R'", 0.6, 13.0, 88.0, 180.0, 1.6),
            wave("T", 0.3, 55.0, 310.0, 45.0, -0.6),
        ],
        Template::Lbbb => vec![
            wave("R", 0.8, 11.0, 40.0, 10.0, -0.3),
            wave("R2", 1.0, 13.0, 85.0, -20.0, -0.9),
            wave("T", 0.35, 55.0, 330.0, 170.0, 0.8),
        ],
        Template::DeltaLv => vec![
            wave("D", 0.5, 14.0, 50.0, 110.0, 0.7),
            wave("T", -0.12, 50.0, 300.0, 110.0, 0.7),
        ],
        Template::DeltaRv => vec![
            wave("D", 0.5, 14.0, 50.0, -10.0, -0.6),
            wave("T", -0.12, 50.0, 300.0, -10.0, -0.6),
        ],
        // Wide complexes are two overlapping slow components (slurred, sometimes notched) rather
        // than one very broad Gaussian, whose long tails would read as a 200+ ms QRS. Ectopic
        // complexes are larger than conducted ones (Goldberger 2017 ch. 16).
        Template::EctopicLv => vec![
            wave("R", 1.6, 13.0, 48.0, 100.0, 0.8),
            wave("R2", 1.6, 13.0, 85.0, 110.0, 0.9),
            wave("T", -0.5, 55.0, 330.0, 100.0, 0.8),
        ],
        Template::EctopicRv => vec![
            wave("R", 1.6, 13.0, 48.0, -60.0, -0.8),
            wave("R2", 1.6, 13.0, 85.0, -75.0, -0.9),
            wave("T", -0.5, 55.0, 330.0, -65.0, -0.8),
        ],
        Template::Spike => vec![wave("spike", 2.0, 0.6, 0.0, -60.0, -0.5)],
        Template::Biventricular => vec![
            wave("R", 1.2, 12.0, 40.0, -150.0, 0.7),
            wave("R2", 0.8, 12.0, 72.0, -110.0, 0.6),
            wave("T", -0.4, 55.0, 320.0, -130.0, 0.6),
        ],
        // Direction: superior and anterior, so II, III and aVF see it inverted and V1 upright.
        // The three components have no net area (a train of them has a level baseline) and start
        // ≥ 3 σ after their event, so a new wave enters without a step.
        Template::FlutterWave => vec![
            wave("F", 0.17, 34.0, 110.0, -90.0, 0.9),
            wave("F2", 0.07, 12.0, 160.0, -90.0, 0.9),
            wave("F3", -0.2546, 26.0, 200.0, -90.0, 0.9),
        ],
        // Coarse by default: over 0.1 mV in V1, the usual dividing line. The event picks the direction.
        Template::FibrillatoryWave => vec![
            wave("f", 0.11, 20.0, 65.0, 40.0, 0.9),
            wave("f2", -0.11, 20.0, 125.0, 40.0, 0.9),
        ],
        // Coarse by default: over 0.2 mV peak to peak (Weaver et al. 1985). The event picks the
        // direction and scales the size.
        Template::VfWave => vec![
            wave("VF", 0.4, 24.0, 75.0, 60.0, 0.0),
            wave("VF2", -0.4, 24.0, 150.0, 60.0, 0.0),
        ],
        // Wide and large; the two lobes are QRS and T together, as they are at this rate.
        Template::PolymorphicBeat => vec![
            wave("R", 1.3, 34.0, 105.0, 60.0, 0.25),
            wave("R2", -1.3, 34.0, 210.0, 60.0, 0.25),
        ],
        Template::PreexcitedLv => vec![
            wave("D", 0.8, 22.0, 48.0, 110.0, 0.7),
            wave("R", 1.3, 28.0, 115.0, 110.0, 0.7),
            wave("T", -0.35, 50.0, 320.0, 110.0, 0.7),
        ],
        Template::PreexcitedRv => vec![
            wave("D", 0.8, 22.0, 48.0, -10.0, -0.6),
            wave("R", 1.3, 28.0, 115.0, -10.0, -0.6),
            wave("T", -0.35, 50.0, 320.0, -10.0, -0.6),
        ],
    }
}

impl Template {
    /// Templates whose waves are ventricular (QRS width scale, axis shift and QT adaptation apply).
    #[must_use]
    pub fn is_ventricular(self) -> bool {
        !matches!(
            self,
            Template::SinusP
                | Template::EctopicP
                | Template::RetrogradeP
                | Template::Spike
                | Template::FlutterWave
                | Template::FibrillatoryWave
        )
    }
}

/// Override of one wave of one template (absolute values; `None` keeps the built-in value).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveOverride {
    pub template: Template,
    /// Wave name within the template ("P", "Q", "R", "R'", "R2", "S", "T", "D").
    pub wave: String,
    #[serde(default)]
    pub amplitude_mv: Option<f64>,
    #[serde(default)]
    pub width_ms: Option<f64>,
    #[serde(default)]
    pub offset_ms: Option<f64>,
    #[serde(default)]
    pub axis_deg: Option<f64>,
    #[serde(default)]
    pub anterior: Option<f64>,
}

/// Scenario-level Tier-0 morphology knobs (the `morphology` section of a scenario file).
/// Everything here changes the *shape* of the beats; *which* beats occur and when is decided by
/// the timing engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MorphologyOverrides {
    /// QRS width scale of every ventricular template (1 = built-in).
    pub qrs_width_scale: f64,
    /// Rotation of every ventricular wave in the frontal plane, degrees (e.g. a vertical or
    /// horizontal heart position).
    pub axis_shift_deg: f64,
    /// QT rate adaptation exponent `k` in `QT ∝ (RR / 1 s)^k`; `None` keeps the synthesiser's
    /// setting. Scenario runs default to Fridericia (⅓).
    pub qt_rate_exponent: Option<f64>,
    /// Per-wave overrides.
    pub waves: Vec<WaveOverride>,
}

impl Default for MorphologyOverrides {
    fn default() -> Self {
        Self {
            qrs_width_scale: 1.0,
            axis_shift_deg: 0.0,
            qt_rate_exponent: None,
            waves: Vec::new(),
        }
    }
}

/// A [`WaveOverride`] names a wave its template does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownWave {
    pub template: Template,
    pub wave: String,
}

impl std::fmt::Display for UnknownWave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "template {:?} has no wave `{}`", self.template, self.wave)
    }
}

impl std::error::Error for UnknownWave {}

/// Tier-0 rendering of the timing engine's drug and ischaemia state (brief B14). The Tier-1
/// mesh produces the same changes from first principles (CV → QRS width, APD → QT, AP shape →
/// T shape, injury current → ST); here they are morphology rules applied to every beat spawned
/// after the modulation is set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SynthModulation {
    /// QRS width multiplier (ventricular conduction slowing: class Ic, hyperkalaemia).
    pub qrs_width_scale: f64,
    /// P width and amplitude multipliers (atrial conduction slowing flattens and widens the P).
    pub p_width_scale: f64,
    pub p_amplitude_scale: f64,
    /// QT multiplier (T offset; ventricular APD).
    pub qt_scale: f64,
    /// T amplitude and width multipliers (peaked narrow T of hyperkalaemia, flat broad T of
    /// hypokalaemia).
    pub t_amplitude_scale: f64,
    pub t_width_scale: f64,
    /// Depth of the digoxin ST "scoop" (sagging depression opposite the T), mV.
    pub st_scoop_mv: f64,
    /// U-wave amplitude along the T direction, mV.
    pub u_wave_mv: f64,
    /// ST injury vector (heart frame, mV): ST deviation from J to T in every lead is its
    /// projection.
    pub st_vector_mv: DVec3,
    /// Necrotic lesions in effect: Q waves, loss of R and T inversion of conducted beats.
    pub infarct: InfarctForces,
}

impl Default for SynthModulation {
    fn default() -> Self {
        Self::NONE
    }
}

impl SynthModulation {
    /// No drug, no lesion.
    pub const NONE: Self = Self {
        qrs_width_scale: 1.0,
        p_width_scale: 1.0,
        p_amplitude_scale: 1.0,
        qt_scale: 1.0,
        t_amplitude_scale: 1.0,
        t_width_scale: 1.0,
        st_scoop_mv: 0.0,
        u_wave_mv: 0.0,
        st_vector_mv: DVec3::ZERO,
        infarct: InfarctForces {
            q_limb: [0.0; 2],
            q_precordial: [0.0; 6],
            t_limb: [0.0; 2],
            t_precordial: [0.0; 6],
        },
    };

    /// The morphology of a drug effect and a set of coronary lesions (research §3.5 table).
    #[must_use]
    pub fn from_state(drug: &DrugEffects, ischaemia: &IschaemiaState) -> Self {
        let slope = drug.repolarisation_slope;
        let st = ischaemia.st_vector_mv();
        Self {
            qrs_width_scale: 1.0 / drug.ventricular_cv_scale,
            p_width_scale: 1.0 / drug.atrial_cv_scale,
            p_amplitude_scale: drug.atrial_cv_scale.min(1.0),
            qt_scale: drug.ventricular_apd_scale,
            t_amplitude_scale: (1.0 + if slope > 0.0 { 1.0 } else { 0.6 } * slope).max(0.1),
            t_width_scale: (1.0 - if slope > 0.0 { 0.35 } else { 0.3 } * slope).max(0.3),
            st_scoop_mv: 0.12 * drug.plateau_sag,
            u_wave_mv: 0.15 * drug.u_wave,
            st_vector_mv: DVec3::new(st[0], st[1], st[2]),
            infarct: ischaemia.infarct_forces(),
        }
    }
}

/// Hexaxial angle and anterior component of a heart-frame direction (the inverse of
/// [`WaveSpec::direction`] up to length).
fn wave_direction(v: DVec3) -> (f64, f64) {
    let frontal = v.x.hypot(v.y);
    if frontal < 1e-9 * v.length().max(1e-12) {
        return (0.0, 1e6 * v.z.signum());
    }
    ((-v.y).atan2(v.x).to_degrees(), v.z / frontal)
}

#[derive(Debug, Clone, Copy)]
struct ActiveWave {
    spec: WaveSpec,
    t0: Millis,
}

/// A Gaussian with its own gain per lead: what a regional lesion adds to a beat. The limb gains
/// come from a frontal vector, so Einthoven's law holds; the precordial leads are near-field and
/// carry a weight each.
#[derive(Debug, Clone, Copy)]
struct LeadWave {
    t0: Millis,
    offset_ms: f64,
    width_ms: f64,
    gains: [f64; LEAD_COUNT],
}

/// Depth of a pathological Q wave at full weight, mV, and its timing. σ 14 ms at 24 ms makes the
/// initial negative deflection last over 40 ms where the R is lost with it, so the criteria of
/// Thygesen et al. 2018 (≥ 20 ms in V2–V3; ≥ 30 ms and ≥ 0.1 mV elsewhere) are met.
const INFARCT_Q_MV: f64 = 0.45;
const INFARCT_Q_WIDTH_MS: f64 = 14.0;
const INFARCT_Q_OFFSET_MS: f64 = 24.0;
/// Fraction of the R wave lost in a lead that faces the infarct (Q or QS pattern).
const INFARCT_R_LOSS: f64 = 0.9;
/// R wave gained in a lead that sees the infarct from behind (posterior infarct in V1–V3), mV.
const INFARCT_TALL_R_MV: f64 = 0.5;
/// Depth of an inverted T wave at full weight, mV, and how much narrower than the T it replaces
/// (the symmetric "coronary" T).
const INFARCT_T_MV: f64 = 0.4;
const INFARCT_T_WIDTH_SCALE: f64 = 0.8;
/// How long ago the last torsades beat must be for a new episode to restart the twist, ms.
const TWIST_RESTART_MS: f64 = 1500.0;
/// Frontal angle a torsades episode starts from, degrees.
const TWIST_START_DEG: f64 = 60.0;
/// Mean turn of the QRS vector per torsades beat, degrees: half a turn, and so one polarity run in
/// a lead, in about 10 beats (reported range 5–20 beats; see `docs/research/tier0-rhythm-criteria.md`).
const TWIST_STEP_DEG: f64 = 18.0;
/// An AP beat within this time of an atrial circuit event is pre-excited AF: the pathway activates
/// the ventricles alone (fully pre-excited QRS), ms.
const PREEXCITED_AF_WINDOW_MS: f64 = 500.0;

/// Random numbers in `[0, 1)` tied to one event: the same event always draws the same wave. Only
/// IEEE arithmetic on the time's bits, so it is the same on every platform.
fn event_noise<const N: usize>(at_ms: f64) -> [f64; N] {
    let mut rng = ShockRng::new(at_ms.to_bits());
    std::array::from_fn(|_| rng.next_f64())
}

/// Bundle-branch block events seen since the last His activation (bit set).
const BLOCK_RBB: u8 = 1;
const BLOCK_LBB: u8 = 2;
const BLOCK_LAF: u8 = 4;
const BLOCK_LPF: u8 = 8;

/// Ventricular activations closer than this belong to the same beat (fusion fronts), ms.
const SAME_BEAT_MS: f64 = 150.0;
/// A conducted front arriving this long after a captured pace finds the ventricles mostly
/// activated by the paced front (pseudo-fusion on the surface ECG), ms.
const PACED_CAPTURE_MS: f64 = 30.0;

/// Event-driven 12-lead synthesiser.
#[derive(Debug, Clone)]
pub struct GaussianSynth {
    pub leads: LeadVectors,
    /// Sinus P wave ([`Template::SinusP`]).
    pub atrial: Vec<WaveSpec>,
    /// Normally conducted QRS–T ([`Template::Normal`]).
    pub ventricular: Vec<WaveSpec>,
    /// Waves of every other [`Template`].
    pub templates: BTreeMap<Template, Vec<WaveSpec>>,
    /// QRS width scale (1 = 90 ms-ish; 1.5 ≈ bundle-branch block).
    pub qrs_width_scale: f64,
    /// Global axis rotation added to every ventricular wave, degrees.
    pub axis_shift_deg: f64,
    /// QT rate adaptation exponent: T offset and width scale with `(RR / 1000 ms)^k`, RR being
    /// the interval from the previous ventricular beat. 0 = fixed QT (the textbook default).
    pub qt_rate_exponent: f64,
    /// Drug / ischaemia morphology of new beats.
    pub modulation: SynthModulation,
    active: Vec<ActiveWave>,
    branch_blocks: u8,
    last_beat_ms: Option<f64>,
    last_rr_ms: Option<f64>,
    /// Last paced ventricular beat: time and pacer channel (biventricular fusion).
    last_paced_v: Option<(Millis, u8)>,
    /// Time of the last captured ventricular pace, ms.
    last_pace_ms: Option<f64>,
    /// Per-lead waves in flight.
    active_lead: Vec<LeadWave>,
    /// Time of the last atrial-driver cycle, ms.
    last_atrial_circuit_ms: Option<f64>,
    /// Time of the last fully pre-excited beat, ms.
    last_preexcited_ms: Option<f64>,
    /// Frontal angle of the QRS vector in torsades, degrees.
    twist_deg: f64,
    /// Time of the last torsades beat, ms.
    last_twist_ms: Option<f64>,
}

impl GaussianSynth {
    #[must_use]
    pub fn textbook() -> Self {
        let (atrial, ventricular) = textbook_waves();
        let templates = [
            Template::EctopicP,
            Template::RetrogradeP,
            Template::Rbbb,
            Template::Lbbb,
            Template::DeltaLv,
            Template::DeltaRv,
            Template::EctopicLv,
            Template::EctopicRv,
            Template::Spike,
            Template::Biventricular,
            Template::FlutterWave,
            Template::FibrillatoryWave,
            Template::VfWave,
            Template::PolymorphicBeat,
            Template::PreexcitedLv,
            Template::PreexcitedRv,
        ]
        .into_iter()
        .map(|t| (t, template_waves(t)))
        .collect();
        Self {
            leads: LeadVectors::standard(),
            atrial,
            ventricular,
            templates,
            qrs_width_scale: 1.0,
            axis_shift_deg: 0.0,
            qt_rate_exponent: 0.0,
            modulation: SynthModulation::NONE,
            active: Vec::new(),
            branch_blocks: 0,
            last_beat_ms: None,
            last_rr_ms: None,
            last_paced_v: None,
            last_pace_ms: None,
            active_lead: Vec::new(),
            last_atrial_circuit_ms: None,
            last_preexcited_ms: None,
            twist_deg: TWIST_START_DEG,
            last_twist_ms: None,
        }
    }

    /// Waves of a template.
    #[must_use]
    pub fn waves(&self, t: Template) -> &[WaveSpec] {
        match t {
            Template::SinusP => &self.atrial,
            Template::Normal => &self.ventricular,
            other => self.templates.get(&other).map_or(&[], Vec::as_slice),
        }
    }

    fn waves_mut(&mut self, t: Template) -> &mut Vec<WaveSpec> {
        match t {
            Template::SinusP => &mut self.atrial,
            Template::Normal => &mut self.ventricular,
            other => self
                .templates
                .entry(other)
                .or_insert_with(|| template_waves(other)),
        }
    }

    /// Apply a scenario's morphology section.
    pub fn apply_overrides(&mut self, o: &MorphologyOverrides) -> Result<(), UnknownWave> {
        self.qrs_width_scale = o.qrs_width_scale;
        self.axis_shift_deg = o.axis_shift_deg;
        if let Some(k) = o.qt_rate_exponent {
            self.qt_rate_exponent = k;
        }
        for w in &o.waves {
            let spec = self
                .waves_mut(w.template)
                .iter_mut()
                .find(|s| s.name == w.wave)
                .ok_or_else(|| UnknownWave {
                    template: w.template,
                    wave: w.wave.clone(),
                })?;
            if let Some(v) = w.amplitude_mv {
                spec.amplitude = v;
            }
            if let Some(v) = w.width_ms {
                spec.width_ms = v;
            }
            if let Some(v) = w.offset_ms {
                spec.offset_ms = v;
            }
            if let Some(v) = w.axis_deg {
                spec.axis_deg = v;
            }
            if let Some(v) = w.anterior {
                spec.anterior = v;
            }
        }
        Ok(())
    }

    /// Feed timing-engine events.
    pub fn on_events(&mut self, events: &[Event]) {
        for ev in events {
            if ev.kind == EventKind::Blocked {
                self.branch_blocks |= match ev.region {
                    Region::RightBundleBranch => BLOCK_RBB,
                    Region::LeftBundleBranch => BLOCK_LBB,
                    Region::LeftAnteriorFascicle => BLOCK_LAF,
                    Region::LeftPosteriorFascicle => BLOCK_LPF,
                    _ => 0,
                };
                continue;
            }
            match ev.region {
                Region::HisBundle => self.branch_blocks = 0,
                r if r.is_atrial() => {
                    if let Origin::Circuit(c) = ev.origin {
                        self.on_atrial_circuit(c, ev);
                        continue;
                    }
                    let t = match (ev.kind, ev.origin) {
                        (EventKind::Automatic, Origin::Sinus) => Template::SinusP,
                        (EventKind::Automatic, _) => Template::EctopicP,
                        (EventKind::Stimulus, _) => {
                            self.spawn(Template::Spike, ev.at, 0.0, 1.0);
                            Template::SinusP
                        }
                        _ => Template::RetrogradeP,
                    };
                    self.spawn(t, ev.at, 0.0, 1.0);
                }
                Region::LeftVentricle | Region::RightVentricle | Region::VentricularSeptum => {
                    self.on_ventricular(ev);
                }
                _ => {}
            }
        }
    }

    /// One cycle of the atrial driver.
    fn on_atrial_circuit(&mut self, c: Circuit, ev: &Event) {
        self.last_atrial_circuit_ms = Some(ev.at.0);
        match c {
            Circuit::AtrialFibrillation => {
                // Mostly along the mean f-wave direction (best seen in V1 and II), varying in size.
                let [size, axis, anterior] = event_noise::<3>(ev.at.0);
                self.spawn_circuit(
                    Template::FibrillatoryWave,
                    ev.at,
                    0.6 + 0.8 * size,
                    40.0 + 120.0 * (axis - 0.5),
                    0.9 + anterior - 0.5,
                );
            }
            _ => self.spawn(Template::FlutterWave, ev.at, 0.0, 1.0),
        }
    }

    /// One cycle of the ventricular driver.
    fn on_ventricular_circuit(&mut self, c: Circuit, ev: &Event) {
        let t = ev.at.0;
        self.last_rr_ms = self.last_beat_ms.map(|l| t - l);
        self.last_beat_ms = Some(t);
        self.branch_blocks = 0;
        let [a, b, size] = event_noise::<3>(t);
        match c {
            Circuit::Torsades => {
                if self.last_twist_ms.is_none_or(|l| t - l > TWIST_RESTART_MS) {
                    self.twist_deg = TWIST_START_DEG;
                }
                self.last_twist_ms = Some(t);
                self.twist_deg += TWIST_STEP_DEG * (0.7 + 0.6 * a);
                let axis = self.twist_deg;
                self.spawn_circuit(Template::PolymorphicBeat, ev.at, 0.85 + 0.3 * size, axis, 0.25);
            }
            Circuit::PolymorphicVt => {
                self.spawn_circuit(
                    Template::PolymorphicBeat,
                    ev.at,
                    0.7 + 0.6 * size,
                    360.0 * a,
                    2.0 * b - 1.0,
                );
            }
            _ => {
                // Slowly waxing and waning size on top of the wave-to-wave variation.
                let envelope = 0.85 + 0.3 * (t / 1900.0 * std::f64::consts::TAU).sin();
                self.spawn_circuit(
                    Template::VfWave,
                    ev.at,
                    envelope * (0.6 + 0.8 * size),
                    360.0 * a,
                    2.0 * b - 1.0,
                );
            }
        }
    }

    /// Spawn every wave of `tpl` with its size scaled and its direction replaced.
    fn spawn_circuit(&mut self, tpl: Template, t0: Millis, size: f64, axis_deg: f64, anterior: f64) {
        let specs = self.waves(tpl).to_vec();
        for w in specs {
            self.active.push(ActiveWave {
                spec: WaveSpec {
                    amplitude: w.amplitude * size,
                    axis_deg,
                    anterior,
                    ..w
                },
                t0,
            });
        }
    }

    fn on_ventricular(&mut self, ev: &Event) {
        if let Origin::Circuit(c) = ev.origin
            && c.is_ventricular()
        {
            self.on_ventricular_circuit(c, ev);
            return;
        }
        let t = ev.at.0;
        match self.last_beat_ms {
            Some(last) if t - last < SAME_BEAT_MS => {}
            last => {
                self.last_rr_ms = last.map(|l| t - l);
                self.last_beat_ms = Some(t);
            }
        }
        let ectopic = if ev.region == Region::RightVentricle {
            Template::EctopicRv
        } else {
            Template::EctopicLv
        };
        match (ev.kind, ev.origin) {
            // Pre-excited AF: nothing to fuse with on most beats, so the pathway's front is the
            // whole QRS. A front arriving just after an earlier one of the same beat adds nothing.
            (EventKind::Conducted, Origin::AccessoryPathway)
                if self
                    .last_atrial_circuit_ms
                    .is_some_and(|a| t - a < PREEXCITED_AF_WINDOW_MS) =>
            {
                let same_beat = self.beat_started_before(t);
                self.branch_blocks = 0;
                if !same_beat {
                    self.last_preexcited_ms = Some(t);
                    let tpl = if ev.region == Region::RightVentricle {
                        Template::PreexcitedRv
                    } else {
                        Template::PreexcitedLv
                    };
                    self.spawn(tpl, ev.at, 0.0, 1.0);
                }
            }
            (EventKind::Conducted, _)
                if self
                    .last_preexcited_ms
                    .is_some_and(|p| (0.0..SAME_BEAT_MS).contains(&(t - p))) =>
            {
                self.branch_blocks = 0;
            }
            (EventKind::Conducted, Origin::AccessoryPathway) => {
                let d = if ev.region == Region::RightVentricle {
                    Template::DeltaRv
                } else {
                    Template::DeltaLv
                };
                self.spawn(d, ev.at, 0.0, 1.0);
            }
            // Intrinsic conduction reaching ventricles a paced front has already largely
            // captured adds nothing visible (true fusion only when the two start together).
            (EventKind::Conducted, _)
                if self
                    .last_pace_ms
                    .is_some_and(|p| (PACED_CAPTURE_MS..SAME_BEAT_MS).contains(&(t - p))) =>
            {
                self.branch_blocks = 0;
            }
            (EventKind::Conducted, _) => {
                let b = std::mem::take(&mut self.branch_blocks);
                let lbbb = b & BLOCK_LBB != 0 || b & (BLOCK_LAF | BLOCK_LPF) == BLOCK_LAF | BLOCK_LPF;
                let (rotation, width) = if lbbb {
                    (0.0, 1.0)
                } else if b & BLOCK_LAF != 0 {
                    (LAFB_ROTATION_DEG, FASCICULAR_WIDTH_SCALE)
                } else if b & BLOCK_LPF != 0 {
                    (LPFB_ROTATION_DEG, FASCICULAR_WIDTH_SCALE)
                } else {
                    (0.0, 1.0)
                };
                let tpl = if lbbb {
                    Template::Lbbb
                } else if b & BLOCK_RBB != 0 {
                    Template::Rbbb
                } else {
                    Template::Normal
                };
                self.spawn(tpl, ev.at, rotation, width);
            }
            (EventKind::Stimulus, Origin::Pacer(ch)) => {
                self.spawn(Template::Spike, ev.at, 0.0, 1.0);
                if self.last_pace_ms.is_none_or(|p| t - p >= SAME_BEAT_MS) {
                    self.last_pace_ms = Some(t);
                }
                match self.last_paced_v {
                    // second lead of a biventricular pace: the two fronts make one fused beat
                    Some((t0, other)) if other != ch && t - t0.0 < SAME_BEAT_MS => {
                        self.active
                            .retain(|w| w.t0 != t0 || !matches!(w.spec.name, "R" | "R2" | "T"));
                        self.spawn(Template::Biventricular, t0, 0.0, 1.0);
                        self.last_paced_v = None;
                    }
                    _ => {
                        self.spawn(ectopic, ev.at, 0.0, 1.0);
                        self.last_paced_v = Some((ev.at, ch));
                    }
                }
            }
            (EventKind::Stimulus, _) => {
                self.spawn(Template::Spike, ev.at, 0.0, 1.0);
                self.spawn(ectopic, ev.at, 0.0, 1.0);
            }
            _ => self.spawn(ectopic, ev.at, 0.0, 1.0),
        }
    }

    /// `true` when the beat at `t` already began before `t` (`on_ventricular` has recorded when
    /// the current beat started), so the event is a later front of the same beat.
    fn beat_started_before(&self, t: f64) -> bool {
        self.last_beat_ms.is_some_and(|b| b < t)
    }

    /// Feed the pacemaker marker channel: a stimulus that did not capture still leaves its
    /// spike on the ECG (captured paces get theirs from the `Stimulus` event, which also
    /// starts the paced beat).
    pub fn on_pacer_markers(&mut self, markers: &[PacerMarker]) {
        for m in markers {
            if m.kind == (MarkerKind::Pace { captured: false }) {
                self.spawn(Template::Spike, m.at, 0.0, 1.0);
            }
        }
    }

    /// Start the waves of `tpl` at `t0`. Ventricular waves get the global QRS width scale and
    /// axis shift; the QRS waves Q/R/S additionally get the fascicular `rotation_deg` and
    /// `width`; T waves are rate-adapted.
    fn spawn(&mut self, tpl: Template, t0: Millis, rotation_deg: f64, width: f64) {
        let ventricular = tpl.is_ventricular();
        let qt = match self.last_rr_ms {
            Some(rr) if self.qt_rate_exponent != 0.0 => {
                (rr / 1000.0).powf(self.qt_rate_exponent).clamp(0.6, 1.4)
            }
            _ => 1.0,
        };
        let m = self.modulation;
        let (scale, shift) = (
            self.qrs_width_scale * width * m.qrs_width_scale,
            self.axis_shift_deg,
        );
        let mut new = Vec::new();
        let mut t_wave = None;
        for w in self.waves(tpl) {
            let mut s = *w;
            if ventricular {
                s.axis_deg += shift;
                if s.name == "T" {
                    s.offset_ms *= qt * m.qt_scale;
                    s.width_ms *= qt * m.qt_scale * m.t_width_scale;
                    s.amplitude *= m.t_amplitude_scale;
                    t_wave = Some(s);
                } else {
                    s.offset_ms *= scale;
                    s.width_ms *= scale;
                    if matches!(s.name, "Q" | "R" | "S") {
                        s.axis_deg += rotation_deg;
                    }
                }
            } else if s.name == "P" {
                s.width_ms *= m.p_width_scale;
                s.offset_ms *= m.p_width_scale;
                s.amplitude *= m.p_amplitude_scale;
            }
            new.push(ActiveWave { spec: s, t0 });
        }
        if let Some(t) = t_wave {
            // ST injury: a broad component from the J point to the T peak along the injury
            // vector (σ 80 ms centred at 0.6 of the T offset: ≈ 0.9 of its peak 60 ms after J).
            let v = m.st_vector_mv;
            if v.length() > 1e-6 {
                let (axis_deg, anterior) = wave_direction(v);
                let spec = WaveSpec {
                    name: "ST",
                    amplitude: v.length() / 0.9,
                    width_ms: 80.0 * m.qt_scale,
                    offset_ms: t.offset_ms * 0.6,
                    axis_deg,
                    anterior,
                };
                new.push(ActiveWave { spec, t0 });
            }
            if m.st_scoop_mv > 0.0 {
                let spec = WaveSpec {
                    name: "scoop",
                    amplitude: -m.st_scoop_mv * t.amplitude.signum(),
                    width_ms: 0.8 * t.width_ms,
                    offset_ms: t.offset_ms - 1.2 * t.width_ms,
                    ..t
                };
                new.push(ActiveWave { spec, t0 });
            }
            if m.u_wave_mv > 0.0 {
                let spec = WaveSpec {
                    name: "U",
                    amplitude: m.u_wave_mv * t.amplitude.signum(),
                    width_ms: 25.0,
                    offset_ms: t.offset_ms + 2.0 * t.width_ms + 40.0,
                    ..t
                };
                new.push(ActiveWave { spec, t0 });
            }
        }
        if matches!(tpl, Template::Normal | Template::Rbbb) && !m.infarct.is_none() {
            self.spawn_infarct(&new, t0, scale);
        }
        self.active.extend(new);
    }

    /// The Q wave, loss of R and T inversion a necrotic lesion adds to one conducted beat, from
    /// the beat's own R and T waves: each lead loses what it had.
    fn spawn_infarct(&mut self, beat: &[ActiveWave], t0: Millis, qrs_scale: f64) {
        let forces = self.modulation.infarct;
        let vector = |name: &str| {
            beat.iter()
                .find(|w| w.spec.name == name)
                .map(|w| w.spec)
        };
        // Sum over the leads of `gain · (frontal part along v) + weight · (precordial part)`.
        let gains = |limb: [f64; 2], limb_along: &dyn Fn(DVec3) -> f64, precordial: &dyn Fn(usize) -> f64| {
            let v = DVec3::new(limb[0], limb[1], 0.0);
            let length = v.length();
            let along = if length > 0.0 { limb_along(v / length) } else { 0.0 };
            let mut out = [0.0; LEAD_COUNT];
            for (i, g) in out.iter_mut().enumerate() {
                *g = if i < 6 {
                    self.leads.vectors[i].dot(v) * along
                } else {
                    precordial(i - 6)
                };
            }
            out
        };
        let q = gains(forces.q_limb, &|_| -INFARCT_Q_MV, &|i| {
            -INFARCT_Q_MV * forces.q_precordial[i]
        });
        let mut waves = vec![LeadWave {
            t0,
            offset_ms: INFARCT_Q_OFFSET_MS * qrs_scale,
            width_ms: INFARCT_Q_WIDTH_MS * qrs_scale,
            gains: q,
        }];
        if let Some(r) = vector("R") {
            let r_vec = r.direction() * r.amplitude;
            let r_leads = self.leads.project(r_vec);
            let g = gains(
                forces.q_limb,
                &|u| -INFARCT_R_LOSS * r_vec.dot(u).max(0.0),
                &|i| {
                    let w = forces.q_precordial[i];
                    if w >= 0.0 {
                        -INFARCT_R_LOSS * w * r_leads[i + 6]
                    } else {
                        -w * INFARCT_TALL_R_MV
                    }
                },
            );
            waves.push(LeadWave {
                t0,
                offset_ms: r.offset_ms,
                width_ms: r.width_ms,
                gains: g,
            });
        }
        if let Some(t) = vector("T") {
            let t_vec = t.direction() * t.amplitude;
            let t_leads = self.leads.project(t_vec);
            let g = gains(
                forces.t_limb,
                &|u| -(t_vec.dot(u).max(0.0) + INFARCT_T_MV),
                &|i| {
                    let w = forces.t_precordial[i];
                    if w >= 0.0 {
                        -w * (t_leads[i + 6].max(0.0) + INFARCT_T_MV)
                    } else {
                        -w * 0.5 * INFARCT_T_MV
                    }
                },
            );
            waves.push(LeadWave {
                t0,
                offset_ms: t.offset_ms,
                width_ms: t.width_ms * INFARCT_T_WIDTH_SCALE,
                gains: g,
            });
        }
        self.active_lead.extend(waves);
    }

    /// Sample the 12 leads at time `t` and drop finished waves.
    pub fn sample(&mut self, t: Millis) -> TwelveLead {
        let mut dipole = DVec3::ZERO;
        self.active.retain(|w| {
            let dt = t.0 - w.t0.0 - w.spec.offset_ms;
            let width = w.spec.width_ms;
            if dt > 5.0 * width {
                return false;
            }
            if dt > -5.0 * width {
                let g = w.spec.amplitude * (-(dt * dt) / (2.0 * width * width)).exp();
                dipole += w.spec.direction() * g;
            }
            true
        });
        let mut leads = self.leads.project(dipole);
        self.active_lead.retain(|w| {
            let dt = t.0 - w.t0.0 - w.offset_ms;
            if dt > 5.0 * w.width_ms {
                return false;
            }
            if dt > -5.0 * w.width_ms {
                let g = (-(dt * dt) / (2.0 * w.width_ms * w.width_ms)).exp();
                for (lead, gain) in leads.iter_mut().zip(w.gains) {
                    *lead += gain * g;
                }
            }
            true
        });
        TwelveLead(leads)
    }

    /// Mean frontal QRS axis implied by the configured waves, degrees on the hexaxial circle.
    ///
    /// It is the direction of the summed Q+R+S area vectors (amplitude × width), i.e. the net
    /// ventricular depolarisation dipole. This is the quantity an observer estimates from the
    /// net QRS deflections in leads I and aVF, so a measured axis must agree with it.
    #[must_use]
    pub fn qrs_axis_deg(&self) -> f64 {
        let (mut x, mut y) = (0.0, 0.0);
        for w in self
            .ventricular
            .iter()
            .filter(|w| matches!(w.name, "Q" | "R" | "S"))
        {
            let spec = WaveSpec {
                axis_deg: w.axis_deg + self.axis_shift_deg,
                ..*w
            };
            let d = spec.direction();
            let area = spec.amplitude * spec.width_ms;
            x += d.x * area;
            y += -d.y * area;
        }
        FrontalAxis::normalise(y.atan2(x).to_degrees())
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}

/// Convenience: index helper for callers that keep `[f64; 12]` arrays.
#[must_use]
pub fn lead_index(l: Lead) -> usize {
    l as usize
}

#[allow(dead_code)]
const _: () = assert!(LEAD_COUNT == 12);

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_core::{FrontalAxis, SimClock};
    use heartsim_sim::timing::{AtrialDriver, AtrialDriverKind};
    use heartsim_sim::{Engine, TimingEngine, TimingParams};

    #[test]
    fn a_train_of_flutter_waves_keeps_the_baseline_level() {
        let area: f64 = template_waves(Template::FlutterWave)
            .iter()
            .map(|w| w.amplitude * w.width_ms)
            .sum();
        assert!(area.abs() < 1e-2, "net area {area}");
        for t in [Template::FibrillatoryWave, Template::VfWave, Template::PolymorphicBeat] {
            let area: f64 = template_waves(t).iter().map(|w| w.amplitude * w.width_ms).sum();
            assert!(area.abs() < 1e-9, "{t:?}: net area {area}");
        }
    }

    #[test]
    fn the_same_engine_events_draw_the_same_strip() {
        let run = || {
            let mut eng = TimingEngine::new(TimingParams {
                atrial_driver: Some(AtrialDriver {
                    kind: AtrialDriverKind::Fibrillation,
                    cycle_ms: 160.0,
                    cycle_jitter: 0.3,
                    start_ms: 0.0,
                }),
                ..TimingParams::default()
            });
            let mut s = GaussianSynth::textbook();
            (0..3000)
                .map(|_| {
                    eng.step(Millis(1.0));
                    s.on_events(&eng.drain_events());
                    s.sample(eng.now()).get(Lead::V1)
                })
                .collect::<Vec<f64>>()
        };
        assert_eq!(run(), run());
    }

    /// Run timing + synth for `secs` seconds at 1 kHz; returns lead II samples and all leads.
    fn run(secs: f64, synth: &mut GaussianSynth) -> Vec<TwelveLead> {
        let mut eng = TimingEngine::new(TimingParams::default());
        let mut clock = SimClock::new(Millis(1.0));
        let mut out = Vec::new();
        for _ in 0..(secs * 1000.0) as usize {
            clock.advance(Millis(1.0));
            eng.step(Millis(1.0));
            synth.on_events(&eng.drain_events());
            out.push(synth.sample(eng.now()));
        }
        out
    }

    #[test]
    fn lead_ii_has_upright_p_qrs_t_and_physiological_amplitudes() {
        let mut s = GaussianSynth::textbook();
        let v = run(3.0, &mut s);
        let ii: Vec<f64> = v.iter().map(|x| x.get(Lead::II)).collect();
        let max = ii.iter().copied().fold(f64::MIN, f64::max);
        let min = ii.iter().copied().fold(f64::MAX, f64::min);
        assert!((0.8..2.0).contains(&max), "R in II = {max} mV");
        assert!(min > -0.6 && min < -0.05, "S in II = {min} mV");
        // aVR must be predominantly negative
        let avr_sum: f64 = v.iter().map(|x| x.get(Lead::AVR)).sum();
        assert!(avr_sum < 0.0);
    }

    #[test]
    fn qrs_axis_matches_spec_and_shift() {
        for shift in [0.0, -60.0, 60.0] {
            let mut s = GaussianSynth::textbook();
            s.axis_shift_deg = shift;
            let v = run(2.0, &mut s);
            // take the sample with the largest |R| in lead with max energy: use net QRS area ≈ R peak sample
            let peak = v
                .iter()
                .max_by(|a, b| a.get(Lead::II).abs().partial_cmp(&b.get(Lead::II).abs()).unwrap())
                .unwrap();
            let ax = FrontalAxis::from_limb_leads(peak).degrees;
            let expected = 60.0 + shift;
            assert!((ax - expected).abs() < 3.0, "axis {ax} vs {expected}");
        }
    }

    #[test]
    fn modulation_widens_qrs_peaks_t_and_adds_st_vector() {
        // the first beat's T wave (QRS ≈ 355 ms, next QRS ≈ 1210 ms)
        let tail_max = |v: &[TwelveLead], lead: Lead, from: usize| {
            v[from..1100].iter().map(|x| x.get(lead)).fold(f64::MIN, f64::max)
        };
        let mut base = GaussianSynth::textbook();
        let normal = run(2.0, &mut base);
        // hyperkalaemia: taller T, earlier (shorter QT)
        let mut s = GaussianSynth::textbook();
        let mut hyper_k = DrugEffects::NONE;
        hyper_k.repolarisation_slope = 1.0;
        hyper_k.ventricular_apd_scale = 0.85;
        s.modulation = SynthModulation::from_state(&hyper_k, &IschaemiaState::default());
        let peaked = run(2.0, &mut s);
        // first beat: QRS at ≈ 355 ms, T well after 450 ms
        assert!(tail_max(&peaked, Lead::V3, 450) > 1.4 * tail_max(&normal, Lead::V3, 450));
        // anterior STEMI: ST up in V2, down in III, 60 ms after the J point (QRS ≈ 355–415 ms)
        let mut s = GaussianSynth::textbook();
        let mut isch = IschaemiaState::default();
        isch.apply("LAD", 1.0).unwrap();
        s.modulation = SynthModulation::from_state(&DrugEffects::NONE, &isch);
        let stemi = run(2.0, &mut s);
        let st = |v: &[TwelveLead], lead: Lead| v[505].get(lead) - v[150].get(lead);
        assert!(st(&stemi, Lead::V2) - st(&normal, Lead::V2) > 0.15);
        assert!(st(&stemi, Lead::III) - st(&normal, Lead::III) < -0.05);
        let (axis, anterior) = wave_direction(DVec3::new(0.0, -1.0, 0.0));
        assert!((axis - 90.0).abs() < 1e-9 && anterior.abs() < 1e-9);
    }

    #[test]
    fn waves_are_released() {
        let mut s = GaussianSynth::textbook();
        let _ = run(5.0, &mut s);
        assert!(
            s.active_count() <= 10,
            "waves must not accumulate: {}",
            s.active_count()
        );
    }
}
