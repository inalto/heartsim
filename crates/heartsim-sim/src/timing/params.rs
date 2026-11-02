//! Parameters of the conduction-system timing engine.
//!
//! Every field carries its unit in the name (`_ms`, `_bpm`) and its doc comment states the
//! physiological meaning and a typical adult range. `0` (or `false` / `None`) switches an
//! optional mechanism off. All structs round-trip through RON (see `tests/rhythms.rs`).
//!
//! Reference values come from `docs/research/electrophysiology-and-ecg-models.md` §1 and
//! §2.5 (Shrier 1987, Lian 2007, Climent 2011, Bernstein 2002).

use heartsim_core::Region;
use serde::{Deserialize, Serialize};

/// Complete configuration of [`super::TimingEngine`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimingParams {
    /// Sinoatrial node: rate, respiration, pauses, sick-sinus, overdrive suppression.
    pub sinus: SinusParams,
    /// Atrial conduction time from the SA node / atrial activation to the AV-node input, ms.
    /// Typical 30–60 ms (P-wave onset to nodal input).
    pub atrial_delay_ms: f64,
    /// Atrial working-myocardium effective refractory period, ms. Typical 200–270 ms. An
    /// atrial impulse (ectopic, echo, paced) arriving earlier than this after the previous
    /// atrial activation is blocked.
    pub atrial_erp_ms: f64,
    /// Ventricular effective refractory period, ms. Typical 250–300 ms (≈ APD).
    pub ventricular_erp_ms: f64,
    /// Fusion window, ms: a second ventricular wavefront arriving over a *different route*
    /// within this time after the first (His-conducted after a pre-excited front, intrinsic
    /// after a pacing stimulus, escape after a conducted beat) is still emitted as `Conducted`
    /// so Tier 1 can fuse the two; arrivals later than this but before `ventricular_erp_ms`
    /// are `Blocked`. ≈ QRS duration of the slower front: typical 100–140 ms.
    pub fusion_window_ms: f64,
    /// AV node (dual pathways, recovery curve, concealment, Mobitz II, complete block).
    pub av: AvParams,
    /// His bundle and the four bundle-branch / fascicular edges.
    pub his_purkinje: HisPurkinjeParams,
    /// Junctional and ventricular escape pacemakers.
    pub escape: EscapeParams,
    /// Ectopic foci (PAC/PVC, bigeminy, runs, parasystole). Index = `Origin::Focus(id)`.
    pub foci: Vec<Focus>,
    /// Optional accessory pathway (WPW / AVRT).
    pub accessory: Option<AccessoryPathway>,
    /// Optional artificial pacemaker.
    pub pacer: Option<PacerConfig>,
}

/// The `timing` section of a scenario file (`assets/scenarios/*.ron`). The struct is named
/// `Scenario` so it reads the same text as `heartsim_core::Scenario`; every other field of the
/// file is ignored here.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename = "Scenario")]
struct TimingSection {
    #[serde(default)]
    timing: TimingParams,
}

impl TimingParams {
    /// Timing parameters of a scenario file; the healthy default when the file has no
    /// `timing` section.
    pub fn from_scenario_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        Ok(ron::from_str::<TimingSection>(text)?.timing)
    }
}

impl Default for TimingParams {
    fn default() -> Self {
        Self {
            sinus: SinusParams::default(),
            atrial_delay_ms: 40.0,
            atrial_erp_ms: 250.0,
            ventricular_erp_ms: 280.0,
            fusion_window_ms: 120.0,
            av: AvParams::default(),
            his_purkinje: HisPurkinjeParams::default(),
            escape: EscapeParams::default(),
            foci: Vec::new(),
            accessory: None,
            pacer: None,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Sinus node
// ---------------------------------------------------------------------------------------------

/// Sinoatrial node behaviour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SinusParams {
    /// Intrinsic rate, bpm. Normal 60–100; bradycardia < 60, tachycardia > 100 (up to ≈ 180).
    pub rate_bpm: f64,
    /// Respiratory sinus arrhythmia depth as a fraction of the RR interval (0.1 = ±10 %).
    /// Typical 0.05–0.15; 0 = off.
    pub respiration_depth: f64,
    /// Respiration rate, breaths per minute. Typical 8–20.
    pub respiration_rate_bpm: f64,
    /// Sinus arrest / pause: every `pause_every_n`-th sinus cycle is replaced by a pause of
    /// `pause_ms` (the SA clock stops and restarts). 0 = off.
    pub pause_every_n: u32,
    /// Duration of each sinus pause, ms. Typical 2000–4000 ms (> 3 s is "significant").
    pub pause_ms: f64,
    /// SA exit block: every `exit_block_ratio`-th impulse fails to leave the node — the clock
    /// keeps running, so the pause is exactly one multiple of the cycle. 0 or 1 = off.
    pub exit_block_ratio: u32,
    /// Sick sinus (tachy–brady) alternation period, seconds. While > 0 the node alternates
    /// between `tachy_rate_bpm` and `brady_rate_bpm` every period, ignoring `rate_bpm`.
    /// Typical 10–60 s; 0 = off.
    pub tachy_brady_period_s: f64,
    /// Tachycardia-phase rate of the sick sinus, bpm. Typical 100–150.
    pub tachy_rate_bpm: f64,
    /// Bradycardia-phase rate of the sick sinus, bpm. Typical 30–50.
    pub brady_rate_bpm: f64,
    /// Overdrive suppression: after the atria are activated by anything other than the SA node
    /// (ectopic, echo, retrograde, paced), the next sinus cycle is lengthened by this amount,
    /// ms. Typical 50–300 ms; 0 = off.
    pub suppression_ms: f64,
    /// `true` = a non-sinus atrial activation resets the SA clock (phase resetting →
    /// *non-compensatory* pause after a PAC). `false` = the clock is undisturbed; a sinus
    /// impulse falling into refractory atrium is dropped (*compensatory* pause).
    pub reset_on_ectopic: bool,
}

impl Default for SinusParams {
    fn default() -> Self {
        Self {
            rate_bpm: 70.0,
            respiration_depth: 0.0,
            respiration_rate_bpm: 12.0,
            pause_every_n: 0,
            pause_ms: 3000.0,
            exit_block_ratio: 0,
            tachy_brady_period_s: 0.0,
            tachy_rate_bpm: 120.0,
            brady_rate_bpm: 40.0,
            suppression_ms: 0.0,
            reset_on_ectopic: true,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// AV node
// ---------------------------------------------------------------------------------------------

/// One AV-nodal pathway with Shrier's recovery curve
/// `AVCT(RT) = min + A · exp(−(RT − erp) / τ)` for `RT ≥ erp`, blocked for `RT < erp`,
/// where RT is the recovery time = interval from the last His-bundle activation to the arrival
/// of the impulse at the nodal input (the H–A interval).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AvPathway {
    /// Minimum conduction time at full recovery, ms. Fast pathway 60–120, slow 150–250.
    pub min_ms: f64,
    /// Amplitude `A` of the decremental term, ms. Typical 100–400. Larger values give 1st-degree
    /// block at high rates and Wenckebach.
    pub recovery_amplitude_ms: f64,
    /// Time constant `τ` of the recovery curve, ms. Typical 50–150.
    pub recovery_tau_ms: f64,
    /// Minimum H–A recovery time for conduction, ms. The nodal ERP expressed as an A1–A2
    /// interval is ≈ `erp_ms + AVCT(erp_ms)`: 100 ms here ≈ 300 ms A1–A2 (normal fast
    /// pathway), 30 ms ≈ 280 ms for the slow pathway; 250–300 ms here models a diseased node
    /// or the long-ERP fast pathway of AVNRT patients.
    pub erp_ms: f64,
    /// Permanent block of this pathway (ablation, `Action::Block`).
    pub blocked: bool,
}

impl AvPathway {
    /// Normal (fast) pathway defaults.
    #[must_use]
    pub fn fast() -> Self {
        Self {
            min_ms: 80.0,
            recovery_amplitude_ms: 120.0,
            recovery_tau_ms: 80.0,
            erp_ms: 100.0,
            blocked: false,
        }
    }

    /// Slow pathway defaults (long delay, short refractory period).
    #[must_use]
    pub fn slow() -> Self {
        Self {
            min_ms: 150.0,
            recovery_amplitude_ms: 100.0,
            recovery_tau_ms: 100.0,
            erp_ms: 30.0,
            blocked: false,
        }
    }
}

/// AV node and His-level block settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AvParams {
    /// Fast pathway (also the "only" pathway when `dual_pathways` is false).
    pub fast: AvPathway,
    /// Slow pathway, used antegradely when the fast pathway is refractory.
    pub slow: AvPathway,
    /// Enable the slow pathway (substrate for AVNRT).
    pub dual_pathways: bool,
    /// Fast pathway conducts retrogradely (V→A). Enables retrograde P waves after junctional,
    /// ventricular and paced beats, and — with `dual_pathways` — the AVNRT loop.
    pub retrograde_fast: bool,
    /// Retrograde conduction time His → atrium over the fast pathway, ms. Typical 50–150
    /// (the V–A interval of typical AVNRT is ≤ 70 ms, of VA conduction after PVCs ≈ 150).
    pub retrograde_delay_ms: f64,
    /// Time for an atrial echo to re-enter the nodal input, ms (atrial turnaround in AVNRT).
    /// Typical 20–60.
    pub reentry_delay_ms: f64,
    /// Concealed conduction: a blocked impulse still advances the node's recovery clock by this
    /// fraction of the way towards its arrival time (0 = none, 1 = full reset). Typical 0.3–0.7.
    pub concealment_fraction: f64,
    /// Complete (3rd-degree) block: nothing crosses the node in either direction.
    pub complete_block: bool,
    /// Mobitz II (infranodal all-or-none drop): conduct `n − 1` of every `n` beats that reach
    /// the His bundle. 0 or 1 = off; 2 = 2:1, 3 = 3:2 … (ratio of atrial to His events).
    pub mobitz2_ratio: u32,
    /// Beats conducted per Mobitz II cycle of `mobitz2_ratio` beats; the rest are dropped.
    /// 0 = `mobitz2_ratio − 1` (classic n:n−1). `1` with ratio 3 gives high-grade 3:1 block.
    pub mobitz2_conducted: u32,
}

impl Default for AvParams {
    fn default() -> Self {
        Self {
            fast: AvPathway::fast(),
            slow: AvPathway::slow(),
            dual_pathways: false,
            retrograde_fast: false,
            retrograde_delay_ms: 80.0,
            reentry_delay_ms: 50.0,
            concealment_fraction: 0.5,
            complete_block: false,
            mobitz2_ratio: 0,
            mobitz2_conducted: 0,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// His–Purkinje
// ---------------------------------------------------------------------------------------------

/// One edge of the His–Purkinje tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleEdge {
    /// Conduction time along the edge, ms. Typical 10–30 (His–Purkinje CV 2–4 m/s); a
    /// longer value models incomplete block.
    pub delay_ms: f64,
    /// Permanent block (LBBB, RBBB, LAFB, LPFB).
    pub blocked: bool,
    /// Rate-dependent (phase-3) block: the edge fails when the His–His interval is shorter than
    /// `60000 / block_above_rate_bpm`. Typical 100–150 bpm; 0 = off.
    pub block_above_rate_bpm: f64,
}

impl BundleEdge {
    #[must_use]
    pub fn new(delay_ms: f64) -> Self {
        Self {
            delay_ms,
            blocked: false,
            block_above_rate_bpm: 0.0,
        }
    }

    /// `true` if the edge conducts an impulse arriving `his_interval_ms` after the previous
    /// His activation (`f64::INFINITY` for the first beat).
    #[must_use]
    pub fn conducts(&self, his_interval_ms: f64) -> bool {
        if self.blocked {
            return false;
        }
        if self.block_above_rate_bpm > 0.0 && his_interval_ms < 60_000.0 / self.block_above_rate_bpm {
            return false;
        }
        true
    }
}

/// His bundle and bundle branches. Normal total His → earliest ventricular myocardium ≈ 35–55 ms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HisPurkinjeParams {
    /// His bundle proper, from the nodal output to the branching point, ms. Typical 5–15.
    pub his_ms: f64,
    /// His-bundle refractory period, ms: a retrograde impulse from the ventricles arriving
    /// earlier than this after the bundle's own activation is ignored. Typical 200–300.
    pub erp_ms: f64,
    /// Right bundle branch → RV endocardium (Purkinje terminals included), ms. Typical 25–35.
    pub rbb: BundleEdge,
    /// Left bundle branch trunk → bifurcation into fascicles, ms. Typical 5–15.
    pub lbb: BundleEdge,
    /// Left anterior fascicle → anterosuperior LV, ms. Typical 10–20. With the defaults the LV
    /// septum is reached 5 ms before the RV (Durrer 1970).
    pub laf: BundleEdge,
    /// Left posterior fascicle → inferoposterior LV, ms. Typical 10–20.
    pub lpf: BundleEdge,
}

impl HisPurkinjeParams {
    /// His → earliest RV activation, ms.
    #[must_use]
    pub fn right_total_ms(&self) -> f64 {
        self.his_ms + self.rbb.delay_ms
    }

    /// His → earliest LV activation (fastest fascicle), ms.
    #[must_use]
    pub fn left_total_ms(&self) -> f64 {
        self.his_ms + self.lbb.delay_ms + self.laf.delay_ms.min(self.lpf.delay_ms)
    }

    /// His → earliest ventricular activation over any intact branch, ms (normal ≈ 40).
    #[must_use]
    pub fn total_ms(&self) -> f64 {
        self.right_total_ms().min(self.left_total_ms())
    }
}

impl Default for HisPurkinjeParams {
    fn default() -> Self {
        Self {
            his_ms: 10.0,
            erp_ms: 250.0,
            rbb: BundleEdge::new(30.0),
            lbb: BundleEdge::new(10.0),
            laf: BundleEdge::new(15.0),
            lpf: BundleEdge::new(15.0),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Escape pacemakers
// ---------------------------------------------------------------------------------------------

/// Subsidiary pacemakers. Each fires when no ventricular activation has occurred within its
/// cycle; the faster one wins. Overdriven by any ventricular activation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EscapeParams {
    /// AV-junctional escape rate, bpm. Typical 40–60; accelerated junctional 60–100; 0 = none.
    pub junctional_bpm: f64,
    /// Ventricular (Purkinje) escape rate, bpm. Idioventricular 20–40, accelerated
    /// idioventricular (AIVR) 50–110; 0 = none.
    pub ventricular_bpm: f64,
    /// Region where the ventricular escape focus sits (sets `Event::region`).
    pub ventricular_region: Region,
}

impl Default for EscapeParams {
    fn default() -> Self {
        Self {
            junctional_bpm: 45.0,
            ventricular_bpm: 30.0,
            ventricular_region: Region::LeftVentricle,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Ectopic foci
// ---------------------------------------------------------------------------------------------

/// Where an ectopic focus sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusRegion {
    RightAtrium,
    LeftAtrium,
    /// AV junction (fires into the His bundle; junctional premature beats / junctional tachycardia).
    Junctional,
    RightVentricle,
    LeftVentricle,
}

impl FocusRegion {
    #[must_use]
    pub fn region(self) -> Region {
        match self {
            FocusRegion::RightAtrium => Region::RightAtrium,
            FocusRegion::LeftAtrium => Region::LeftAtrium,
            FocusRegion::Junctional => Region::AvNodeCompact,
            FocusRegion::RightVentricle => Region::RightVentricle,
            FocusRegion::LeftVentricle => Region::LeftVentricle,
        }
    }
}

/// How a focus fires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FocusMode {
    /// Re-entrant-type premature beat coupled to the preceding dominant beat of its chamber.
    Coupled {
        /// Coupling interval from the preceding activation of the chamber, ms. PAC typical
        /// 350–600; PVC 400–600 (shorter → R-on-T).
        coupling_ms: f64,
        /// Fire after every `every_n_beats` dominant beats: 1 = bigeminy, 2 = trigeminy, …
        every_n_beats: u32,
        /// Beats per firing: 1 = single, 2 = couplet, ≥ 3 = run (non-sustained tachycardia).
        count: u32,
        /// Cycle length inside a couplet/run, ms. Typical 300–500 (120–200 bpm).
        run_cycle_ms: f64,
    },
    /// Automatic focus with its own clock (parasystole, ectopic atrial / ventricular
    /// tachycardia when faster than the sinus node).
    Automatic {
        /// Intrinsic rate, bpm. Parasystole 30–60; ectopic tachycardia 100–200.
        rate_bpm: f64,
        /// Entrance block: the focus is *not* reset by activations of the surrounding
        /// myocardium (classic parasystole). `false` = reset like any pacemaker.
        entrance_block: bool,
        /// Exit block: every `exit_block_ratio`-th discharge fails to emerge. 0 or 1 = off.
        exit_block_ratio: u32,
    },
}

/// An ectopic focus. Its index in `TimingParams::foci` is the `id` of `Origin::Focus(id)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Focus {
    pub region: FocusRegion,
    pub mode: FocusMode,
    /// Maximum number of discharges in the whole run (0 = unlimited). `1` with `Coupled`
    /// gives a single PAC/PVC.
    #[serde(default)]
    pub max_fires: u32,
}

impl Focus {
    /// Premature atrial contraction after every `every_n` sinus beats.
    #[must_use]
    pub fn pac(coupling_ms: f64, every_n: u32) -> Self {
        Self {
            region: FocusRegion::RightAtrium,
            mode: FocusMode::Coupled {
                coupling_ms,
                every_n_beats: every_n,
                count: 1,
                run_cycle_ms: 400.0,
            },
            max_fires: 0,
        }
    }

    /// Premature ventricular contraction (LV) after every `every_n` ventricular beats.
    #[must_use]
    pub fn pvc(coupling_ms: f64, every_n: u32) -> Self {
        Self {
            region: FocusRegion::LeftVentricle,
            mode: FocusMode::Coupled {
                coupling_ms,
                every_n_beats: every_n,
                count: 1,
                run_cycle_ms: 400.0,
            },
            max_fires: 0,
        }
    }

    /// Ventricular parasystole: fixed-rate focus with entrance block.
    #[must_use]
    pub fn parasystole(rate_bpm: f64) -> Self {
        Self {
            region: FocusRegion::LeftVentricle,
            mode: FocusMode::Automatic {
                rate_bpm,
                entrance_block: true,
                exit_block_ratio: 0,
            },
            max_fires: 0,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Accessory pathway
// ---------------------------------------------------------------------------------------------

/// Which side of the heart the pathway inserts into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
}

/// Accessory atrioventricular (Kent) bundle: non-decremental, fixed delay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessoryPathway {
    /// Conduction time atrium ↔ ventricle, ms. Typical 20–40 (shorter PR, delta wave).
    pub delay_ms: f64,
    /// Refractory period measured from the pathway's last use in either direction, ms.
    /// Typical 250–350; < 250 is the dangerous pre-excited-AF pathway.
    pub erp_ms: f64,
    /// Conducts atrium → ventricle (manifest pre-excitation, antidromic AVRT).
    pub antegrade: bool,
    /// Conducts ventricle → atrium (concealed pathway, orthodromic AVRT).
    pub retrograde: bool,
    /// Ventricular insertion side (left lateral = type A, right/septal = type B).
    pub ventricular_side: Side,
    /// Atrial insertion side.
    pub atrial_side: Side,
}

impl Default for AccessoryPathway {
    fn default() -> Self {
        Self {
            delay_ms: 30.0,
            erp_ms: 300.0,
            antegrade: true,
            retrograde: true,
            ventricular_side: Side::Left,
            atrial_side: Side::Left,
        }
    }
}

impl AccessoryPathway {
    #[must_use]
    pub fn ventricular_region(&self) -> Region {
        match self.ventricular_side {
            Side::Left => Region::LeftVentricle,
            Side::Right => Region::RightVentricle,
        }
    }

    #[must_use]
    pub fn atrial_region(&self) -> Region {
        match self.atrial_side {
            Side::Left => Region::LeftAtrium,
            Side::Right => Region::RightAtrium,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Pacemaker
// ---------------------------------------------------------------------------------------------

/// NBG pacing mode (chamber paced / chamber sensed / response).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacerMode {
    /// Atrial asynchronous.
    Aoo,
    /// Atrial demand (inhibited by sensed atrial events).
    Aai,
    /// Ventricular asynchronous.
    Voo,
    /// Ventricular demand (inhibited by sensed ventricular events).
    Vvi,
    /// Dual-chamber asynchronous (A at LRI, V after AV delay).
    Doo,
    /// Dual-chamber, atrial tracking (sensed P → V pace after AV delay, up to the upper rate).
    Ddd,
    /// Dual-chamber, no atrial tracking (sensed P inhibits A pace only).
    Ddi,
}

impl PacerMode {
    #[must_use]
    pub fn paces_atrium(self) -> bool {
        matches!(
            self,
            PacerMode::Aoo | PacerMode::Aai | PacerMode::Doo | PacerMode::Ddd | PacerMode::Ddi
        )
    }

    #[must_use]
    pub fn paces_ventricle(self) -> bool {
        matches!(
            self,
            PacerMode::Voo | PacerMode::Vvi | PacerMode::Doo | PacerMode::Ddd | PacerMode::Ddi
        )
    }

    #[must_use]
    pub fn senses_atrium(self) -> bool {
        matches!(self, PacerMode::Aai | PacerMode::Ddd | PacerMode::Ddi)
    }

    #[must_use]
    pub fn senses_ventricle(self) -> bool {
        matches!(self, PacerMode::Vvi | PacerMode::Ddd | PacerMode::Ddi)
    }

    /// Sensed atrial events start the AV delay (P-synchronous ventricular pacing).
    #[must_use]
    pub fn tracks_atrium(self) -> bool {
        matches!(self, PacerMode::Ddd)
    }
}

impl PacerMode {
    /// The asynchronous mode a magnet (or the panel's ASYNC button) turns this mode into:
    /// same chambers paced, nothing sensed.
    #[must_use]
    pub fn asynchronous(self) -> Self {
        match self {
            PacerMode::Aoo | PacerMode::Aai => PacerMode::Aoo,
            PacerMode::Voo | PacerMode::Vvi => PacerMode::Voo,
            PacerMode::Doo | PacerMode::Ddd | PacerMode::Ddi => PacerMode::Doo,
        }
    }

    /// NBG code as printed on a device, e.g. `"DDD"`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PacerMode::Aoo => "AOO",
            PacerMode::Aai => "AAI",
            PacerMode::Voo => "VOO",
            PacerMode::Vvi => "VVI",
            PacerMode::Doo => "DOO",
            PacerMode::Ddd => "DDD",
            PacerMode::Ddi => "DDI",
        }
    }

    /// Every mode, in the order of a temporary pacemaker's mode knob.
    pub const ALL: [PacerMode; 7] = [
        PacerMode::Voo,
        PacerMode::Vvi,
        PacerMode::Aoo,
        PacerMode::Aai,
        PacerMode::Doo,
        PacerMode::Ddd,
        PacerMode::Ddi,
    ];
}

/// Output channel of the device. The discriminant is the `n` of `Origin::Pacer(n)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PacerChannel {
    /// Atrial lead.
    Atrial = 0,
    /// (Right) ventricular lead.
    Ventricular = 1,
    /// Left-ventricular lead of a biventricular (CRT) system, via the coronary sinus.
    LeftVentricular = 2,
}

impl PacerChannel {
    pub const ALL: [PacerChannel; 3] = [
        PacerChannel::Atrial,
        PacerChannel::Ventricular,
        PacerChannel::LeftVentricular,
    ];

    /// `Origin::Pacer` index of the channel.
    #[must_use]
    pub fn index(self) -> u8 {
        self as u8
    }

    /// Channel of an `Origin::Pacer(n)` index.
    #[must_use]
    pub fn from_index(n: u8) -> Option<Self> {
        Self::ALL.get(usize::from(n)).copied()
    }

    /// Marker-channel letter: `A` or `V` (both ventricular leads are `V`).
    #[must_use]
    pub fn letter(self) -> char {
        match self {
            PacerChannel::Atrial => 'A',
            _ => 'V',
        }
    }
}

/// Where a pacing lead tip sits. Each site maps to a timing-engine region (which chamber the
/// stimulus enters, which beat template Tier 0 draws) and to anatomy-pack landmarks (where
/// Tier 1 starts the wavefront), so the paced morphology follows from the site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LeadSite {
    /// Right atrial appendage: the standard atrial lead; a normal-axis P after the spike.
    RaAppendage,
    /// Right ventricular apex: LBBB-like QRS with a superior axis (activation spreads from the
    /// apex upwards, through slow myocardium), QRS typically 140–200 ms.
    RvApex,
    /// Right ventricular (mid-)septum: LBBB-like but narrower with an inferior axis.
    RvSeptum,
    /// His-bundle area: captures the conduction system, so the QRS is (nearly) normal.
    HisBundle,
    /// Left ventricular lateral wall via a coronary-sinus branch (CRT lead): RBBB-like QRS,
    /// right-inferior axis.
    LvLateral,
    /// Left ventricular posterolateral wall via a coronary-sinus branch.
    LvPosterior,
}

impl LeadSite {
    pub const ALL: [LeadSite; 6] = [
        LeadSite::RaAppendage,
        LeadSite::RvApex,
        LeadSite::RvSeptum,
        LeadSite::HisBundle,
        LeadSite::LvLateral,
        LeadSite::LvPosterior,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            LeadSite::RaAppendage => "RA appendage",
            LeadSite::RvApex => "RV apex",
            LeadSite::RvSeptum => "RV septum",
            LeadSite::HisBundle => "His bundle",
            LeadSite::LvLateral => "LV lateral (CS)",
            LeadSite::LvPosterior => "LV posterior (CS)",
        }
    }

    /// Region a stimulus at this site is reported in (and enters the timing graph at).
    #[must_use]
    pub fn region(self) -> Region {
        match self {
            LeadSite::RaAppendage => Region::RightAtrium,
            LeadSite::RvApex | LeadSite::RvSeptum => Region::RightVentricle,
            LeadSite::HisBundle => Region::HisBundle,
            LeadSite::LvLateral | LeadSite::LvPosterior => Region::LeftVentricle,
        }
    }

    /// `true` for the atrial site.
    #[must_use]
    pub fn is_atrial(self) -> bool {
        self == LeadSite::RaAppendage
    }

    /// Anatomy-pack landmarks the Tier-1 engine seeds, best first (`docs/anatomy-pack.md`).
    /// `rv_septum` falls back to the right-ventricular cell nearest `lv_septal`.
    #[must_use]
    pub fn landmarks(self) -> &'static [&'static str] {
        match self {
            LeadSite::RaAppendage => &["raa", "ra_appendage", "ra_lateral"],
            LeadSite::RvApex => &["rv_apex"],
            LeadSite::RvSeptum => &["rv_septum", "lv_septal"],
            LeadSite::HisBundle => &["his"],
            LeadSite::LvLateral => &["lv_lateral"],
            LeadSite::LvPosterior => &["lv_posterior"],
        }
    }

    /// Typical chronic capture threshold, mA at 0.5 ms pulse width. Endocardial atrial and
    /// ventricular leads 0.5–1.5; coronary-sinus LV leads are epicardial and higher (1–2.5);
    /// His-bundle capture needs more than myocardium (1–2.5). (Ellenbogen, *Clinical Cardiac
    /// Pacing, Defibrillation and Resynchronization Therapy*, 5th ed., ch. 1–3.)
    #[must_use]
    pub fn typical_threshold_ma(self) -> f64 {
        match self {
            LeadSite::RaAppendage => 1.0,
            LeadSite::RvApex | LeadSite::RvSeptum => 0.8,
            LeadSite::HisBundle | LeadSite::LvLateral | LeadSite::LvPosterior => 1.5,
        }
    }

    /// Typical intrinsic electrogram amplitude at the lead tip, mV: P wave 1.5–5 on an atrial
    /// lead, R wave 5–25 on a ventricular lead, smaller on the His lead (Ellenbogen ch. 1).
    #[must_use]
    pub fn typical_intrinsic_mv(self) -> f64 {
        match self {
            LeadSite::RaAppendage => 2.5,
            LeadSite::RvApex | LeadSite::RvSeptum => 10.0,
            LeadSite::HisBundle => 3.0,
            LeadSite::LvLateral | LeadSite::LvPosterior => 8.0,
        }
    }

    /// Typical T-wave (repolarisation) amplitude at the lead tip, mV: 1–3 on a ventricular
    /// lead, negligible on an atrial lead. Sensed only when the ventricular sensitivity is set
    /// below it (T-wave oversensing).
    #[must_use]
    pub fn typical_t_wave_mv(self) -> f64 {
        if self.is_atrial() { 0.0 } else { 1.5 }
    }
}

/// One pacing lead: where it sits, its capture threshold and what it senses. `None` fields
/// take the site's typical value ([`LeadSite`]), so a scenario states only what differs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeadParams {
    pub site: LeadSite,
    /// Capture threshold at t = 0, mA. Output below the threshold gives a spike without capture.
    #[serde(default)]
    pub threshold_ma: Option<f64>,
    /// Threshold drift, mA per minute of simulated time (exit block after implantation, lead
    /// micro-dislodgement, hyperkalaemia, antiarrhythmics). 0 = stable; negative values recover.
    #[serde(default)]
    pub threshold_drift_ma_per_min: f64,
    /// Ceiling of the drifting threshold, mA (0 = none).
    #[serde(default)]
    pub threshold_max_ma: f64,
    /// Intrinsic electrogram amplitude at the tip, mV. Undersensing when the programmed
    /// sensitivity is above it. Tier 1 replaces it with the amplitude measured on the mesh.
    #[serde(default)]
    pub intrinsic_mv: Option<f64>,
    /// T-wave amplitude at the tip, mV (T-wave oversensing when the sensitivity is below it).
    #[serde(default)]
    pub t_wave_mv: Option<f64>,
}

impl LeadParams {
    /// A lead at `site` with the site's typical values.
    #[must_use]
    pub fn at(site: LeadSite) -> Self {
        Self {
            site,
            threshold_ma: None,
            threshold_drift_ma_per_min: 0.0,
            threshold_max_ma: 0.0,
            intrinsic_mv: None,
            t_wave_mv: None,
        }
    }

    fn atrial() -> Self {
        Self::at(LeadSite::RaAppendage)
    }

    fn ventricular() -> Self {
        Self::at(LeadSite::RvApex)
    }

    /// Capture threshold at simulated time `t_ms`, mA (never below 0).
    #[must_use]
    pub fn threshold_at(&self, t_ms: f64) -> f64 {
        let base = self
            .threshold_ma
            .unwrap_or_else(|| self.site.typical_threshold_ma());
        let mut th = base + self.threshold_drift_ma_per_min * t_ms / 60_000.0;
        if self.threshold_max_ma > 0.0 {
            th = th.min(self.threshold_max_ma.max(base));
        }
        th.max(0.0)
    }

    /// Intrinsic electrogram amplitude, mV.
    #[must_use]
    pub fn intrinsic(&self) -> f64 {
        self.intrinsic_mv
            .unwrap_or_else(|| self.site.typical_intrinsic_mv())
    }

    /// T-wave amplitude, mV.
    #[must_use]
    pub fn t_wave(&self) -> f64 {
        self.t_wave_mv.unwrap_or_else(|| self.site.typical_t_wave_mv())
    }
}

/// Artificial pacemaker configuration: the NBG timing cycles of Bernstein 2002 plus the
/// controls of a temporary external pacemaker (output in mA, sensitivity in mV, pause and
/// asynchronous buttons) and the leads.
///
/// Capture: a stimulus captures when the channel's output is at or above the lead's threshold
/// *and* the tissue at the lead is excitable (timing-engine refractoriness). Sensing: an
/// intrinsic event is sensed when the lead's electrogram amplitude is at or above the
/// channel's sensitivity (a *larger* sensitivity number is a *less* sensitive device).
// The switches are the independent buttons of a temporary pacemaker, not a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PacerConfig {
    pub mode: PacerMode,
    /// Lower rate limit, ppm. Typical 50–80 (temporary pacemakers 30–180). The lower rate
    /// interval LRI = 60000 / lower_rate.
    pub lower_rate_bpm: f64,
    /// Upper tracking rate for DDD, ppm. Typical 110–150. Ventricular paces tracking P waves
    /// are delayed to the upper rate interval (pacemaker Wenckebach).
    pub upper_rate_bpm: f64,
    /// Paced/sensed AV delay, ms. Typical 120–200.
    pub av_delay_ms: f64,
    /// Post-ventricular atrial refractory period, ms: atrial events within PVARP after a
    /// ventricular event are not tracked (prevents tracking retrograde P). Typical 250–350.
    pub pvarp_ms: f64,
    /// Blanking after any pace during which nothing is sensed (crosstalk protection), ms.
    /// Typical 20–60.
    pub blanking_ms: f64,
    /// Ventricular refractory period, ms: ventricular-channel signals within this time after a
    /// ventricular event are not counted (protects against sensing the paced QRS and its
    /// T wave). Typical 200–300.
    pub vrp_ms: f64,
    /// Atrial output, mA (temporary pacemaker range 0.1–20; set 2–3× threshold).
    pub atrial_output_ma: f64,
    /// Ventricular output, mA (both ventricular leads; 0.1–20).
    pub ventricular_output_ma: f64,
    /// Atrial sensitivity, mV (0.5–20; typical 0.5).
    pub atrial_sensitivity_mv: f64,
    /// Ventricular sensitivity, mV (0.5–20; typical 2–3).
    pub ventricular_sensitivity_mv: f64,
    /// `false` forces loss of capture on every channel whatever the output (legacy drill
    /// switch of brief B1; the threshold model is the physiological route).
    pub output_capture: bool,
    /// Atrial lead (RA appendage by default).
    pub atrial_lead: LeadParams,
    /// Ventricular lead (RV apex by default).
    pub ventricular_lead: LeadParams,
    /// Second ventricular lead of a biventricular (CRT) device, paced with every ventricular
    /// pace at `vv_delay_ms`. `None` = single-chamber ventricular pacing.
    pub lv_lead: Option<LeadParams>,
    /// LV-lead pace time relative to the RV pace, ms: negative = LV first (the usual CRT
    /// programming, −20…−40), 0 = simultaneous.
    pub vv_delay_ms: f64,
    /// Asynchronous (magnet) mode: nothing is sensed, the device paces at the lower rate.
    pub asynchronous: bool,
    /// Pacing paused (the temporary pacemaker's PAUSE button): sensing continues, no output.
    pub paused: bool,
    /// PMT intervention: after [`PMT_DETECT_CYCLES`] consecutive tracked ventricular paces at
    /// the upper rate, the next PVARP is extended to `pmt_pvarp_ms` for one cycle, so the
    /// retrograde P of a pacemaker-mediated tachycardia is not tracked and the loop breaks.
    pub pmt_intervention: bool,
    /// Extended PVARP used by the PMT intervention, ms. Typical 400–500.
    pub pmt_pvarp_ms: f64,
}

/// Consecutive upper-rate tracked paces that count as a pacemaker-mediated tachycardia
/// (the common device algorithm detects 8–16).
pub const PMT_DETECT_CYCLES: u32 = 8;

impl Default for PacerConfig {
    fn default() -> Self {
        Self {
            mode: PacerMode::Ddd,
            lower_rate_bpm: 60.0,
            upper_rate_bpm: 120.0,
            av_delay_ms: 150.0,
            pvarp_ms: 250.0,
            blanking_ms: 50.0,
            vrp_ms: 250.0,
            atrial_output_ma: 5.0,
            ventricular_output_ma: 5.0,
            atrial_sensitivity_mv: 0.5,
            ventricular_sensitivity_mv: 2.5,
            output_capture: true,
            atrial_lead: LeadParams::atrial(),
            ventricular_lead: LeadParams::ventricular(),
            lv_lead: None,
            vv_delay_ms: 0.0,
            asynchronous: false,
            paused: false,
            pmt_intervention: false,
            pmt_pvarp_ms: 400.0,
        }
    }
}

impl PacerConfig {
    #[must_use]
    pub fn new(mode: PacerMode, lower_rate_bpm: f64) -> Self {
        Self {
            mode,
            lower_rate_bpm,
            ..Self::default()
        }
    }

    /// Mode in effect (the asynchronous button overrides the programmed mode).
    #[must_use]
    pub fn effective_mode(&self) -> PacerMode {
        if self.asynchronous {
            self.mode.asynchronous()
        } else {
            self.mode
        }
    }

    /// Lead of `channel` (`None` for the LV channel of a single-chamber device).
    #[must_use]
    pub fn lead(&self, channel: PacerChannel) -> Option<&LeadParams> {
        match channel {
            PacerChannel::Atrial => Some(&self.atrial_lead),
            PacerChannel::Ventricular => Some(&self.ventricular_lead),
            PacerChannel::LeftVentricular => self.lv_lead.as_ref(),
        }
    }

    /// Output of `channel`, mA.
    #[must_use]
    pub fn output_ma(&self, channel: PacerChannel) -> f64 {
        match channel {
            PacerChannel::Atrial => self.atrial_output_ma,
            _ => self.ventricular_output_ma,
        }
    }

    /// Sensitivity of `channel`, mV.
    #[must_use]
    pub fn sensitivity_mv(&self, channel: PacerChannel) -> f64 {
        match channel {
            PacerChannel::Atrial => self.atrial_sensitivity_mv,
            _ => self.ventricular_sensitivity_mv,
        }
    }

    /// Implanted-device presets of the pacing panel (brief B13).
    #[must_use]
    pub fn preset(p: PacerPreset) -> Self {
        match p {
            PacerPreset::Vvi60 => Self::new(PacerMode::Vvi, 60.0),
            PacerPreset::Ddd60130 => Self {
                upper_rate_bpm: 130.0,
                ..Self::new(PacerMode::Ddd, 60.0)
            },
            PacerPreset::Biv => Self {
                upper_rate_bpm: 130.0,
                av_delay_ms: 120.0,
                lv_lead: Some(LeadParams::at(LeadSite::LvLateral)),
                vv_delay_ms: -20.0,
                ..Self::new(PacerMode::Ddd, 60.0)
            },
        }
    }
}

/// Implanted-device presets (`assets/scenarios/paced_*.ron` use the same settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacerPreset {
    /// Single-chamber RV-apex VVI at 60 ppm.
    Vvi60,
    /// Dual-chamber DDD, lower rate 60, upper tracking rate 130.
    Ddd60130,
    /// Biventricular (CRT-P) DDD 60/130: RV apex + LV lateral, LV first by 20 ms, AV 120 ms.
    Biv,
}

impl PacerPreset {
    pub const ALL: [PacerPreset; 3] = [PacerPreset::Vvi60, PacerPreset::Ddd60130, PacerPreset::Biv];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PacerPreset::Vvi60 => "VVI 60",
            PacerPreset::Ddd60130 => "DDD 60/130",
            PacerPreset::Biv => "BiV (CRT)",
        }
    }
}
