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

/// Artificial pacemaker configuration (Bernstein 2002 timing cycles).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PacerConfig {
    pub mode: PacerMode,
    /// Lower rate limit, ppm. Typical 50–80. The lower rate interval LRI = 60000 / lower_rate.
    pub lower_rate_bpm: f64,
    /// Upper tracking rate for DDD, ppm. Typical 110–150. Ventricular paces tracking P waves
    /// are delayed to the upper rate interval (pacemaker Wenckebach).
    pub upper_rate_bpm: f64,
    /// Paced/sensed AV delay, ms. Typical 120–200.
    pub av_delay_ms: f64,
    /// Post-ventricular atrial refractory period, ms: atrial events within PVARP after a
    /// ventricular event are not sensed (prevents tracking retrograde P). Typical 250–350.
    pub pvarp_ms: f64,
    /// Blanking after any pace during which nothing is sensed (crosstalk protection), ms.
    /// Typical 20–60.
    pub blanking_ms: f64,
    /// `true` = stimuli capture excitable tissue; `false` = output below threshold (loss of
    /// capture drill: spikes without activation).
    pub output_capture: bool,
    /// Atrial lead position.
    pub atrial_lead: Region,
    /// Ventricular lead position (RV apex by default).
    pub ventricular_lead: Region,
}

impl Default for PacerConfig {
    fn default() -> Self {
        Self {
            mode: PacerMode::Ddd,
            lower_rate_bpm: 60.0,
            upper_rate_bpm: 120.0,
            av_delay_ms: 150.0,
            pvarp_ms: 250.0,
            blanking_ms: 50.0,
            output_capture: true,
            atrial_lead: Region::RightAtrium,
            ventricular_lead: Region::RightVentricle,
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
}
