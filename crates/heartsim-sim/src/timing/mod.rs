//! Conduction-system timing engine (Tier 0/1 backbone).
//!
//! The heart's rhythm is modelled as a small graph of nodes with automaticity and refractoriness
//! joined by edges with delay and (for the AV node) decremental conduction:
//!
//! ```text
//! SA ──► atria ──► AV node (fast ∥ slow pathway) ──► His ──► RBB ─────────► RV
//!   ▲       │                 ▲    │                        └► LBB ─► LAF/LPF ► LV
//!   │       └──► accessory pathway ─────────────────────────────────────────► V
//!   │                                                                        │
//!   └── retrograde (fast pathway / accessory pathway) ◄──────────────────────┘
//! ectopic foci and pacemaker leads attach to any chamber; escape pacemakers in the junction and
//! the ventricles fire when the ventricles stay silent.
//! ```
//!
//! **Sustained arrhythmias** ([`AtrialDriver`], [`VentricularDriver`]) are two more items in the
//! same queue: every cycle emits an event with `Origin::Circuit`. An atrial cycle overdrives the
//! sinus node and meets the AV node and any accessory pathway as usual, so the ventricular
//! response is the node's own. A ventricular driver blocks every other ventricular front while
//! it runs. Cycle lengths come from a generator seeded by [`TimingEngine::set_shock_seed`], apart
//! from the one that decides shock outcomes, so adding a driver does not change those.
//!
//! AV conduction follows the Shrier recovery curve `AVCT(RT) = min + A·exp(−(RT − ERP)/τ)` with
//! `RT` the H–A recovery time (last His activation → arrival at the node); this single formula
//! yields 1st-degree block, Wenckebach and 2:1 block. Mobitz II is an all-or-none His-level drop.
//!
//! Everything — sinus discharges included — goes through one time-sorted pending queue, so the
//! emitted [`Event`]s are strictly chronological and the engine is deterministic for a given
//! [`TimingParams`]. The queue and all per-structure state are pre-allocated; the only
//! allocation after warm-up is the caller's [`Engine::drain_events`].

mod conduction;
pub mod ladder;
pub mod pacer;
pub mod params;

pub use ladder::{LadderDirection, LadderSample};
pub use pacer::{MarkerKind, PacerMarker, T_WAVE_DELAY_MS};
pub use params::{
    AccessoryPathway, AtrialDriver, AtrialDriverKind, AvParams, AvPathway, BundleEdge, DriverStart,
    EscapeParams, Focus, FocusMode, FocusRegion, HisPurkinjeParams, LeadParams, LeadSite, PMT_DETECT_CYCLES,
    PacerChannel, PacerConfig, PacerMode, PacerPreset, Side, SinusParams, TimingParams, VentricularDriver,
    VentricularDriverKind,
};

use crate::defib::{
    RhythmClass, SYNC_DELAY_MS, ShockRecord, ShockRng, VULNERABLE_WINDOW, success_probability,
};
use crate::engine::{CellState, Chamber, Circuit, Engine, Event, EventKind, Origin};
use crate::ischaemia::IschaemiaState;
use crate::pharma::{DrugEffects, Pharmacology};
use heartsim_core::scenario::{Action, StimSite};
use heartsim_core::{Millis, Region};
use pacer::Pacer;

/// Which route brought an impulse to the His bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HisVia {
    Fast,
    Slow,
    /// Retrograde from the ventricles over the His–Purkinje system.
    Retrograde,
    /// Junctional escape, junctional focus or His stimulus.
    Junctional,
}

/// Which route brought an impulse to the ventricular myocardium.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VentVia {
    HisPurkinje,
    Accessory,
    Focus,
    Escape,
    Pacer,
    /// LV lead of a biventricular device: a front distinct from the RV pace, so the two fuse.
    PacerLv,
    External,
    /// An activation the tissue model (Tier 2 re-entry) produced and sensed back.
    Tissue,
    /// A cycle of the timing engine's own [`VentricularDriver`].
    Circuit,
}

/// Which route brought an impulse to the atria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtrialVia {
    Sinus,
    Focus,
    /// Retrograde over the AV node (echo / VA conduction).
    NodeRetro,
    /// Retrograde over the accessory pathway.
    AccessoryRetro,
    Pacer,
    External,
    /// A re-entrant atrial front the tissue model (Tier 2) sensed at the AV-nodal input.
    Tissue,
    /// A cycle of the timing engine's own [`AtrialDriver`].
    Circuit,
}

/// Bundle-branch / fascicular edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edge {
    Rbb,
    Lbb,
    Laf,
    Lpf,
}

impl Edge {
    pub(crate) fn region(self) -> Region {
        match self {
            Edge::Rbb => Region::RightBundleBranch,
            Edge::Lbb => Region::LeftBundleBranch,
            Edge::Laf => Region::LeftAnteriorFascicle,
            Edge::Lpf => Region::LeftPosteriorFascicle,
        }
    }
}

/// A scheduled internal happening.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Pending {
    /// Sinus node discharge.
    Sinus,
    /// Ectopic focus `id` discharges; `remaining` further beats follow in a couplet/run.
    FocusFire { id: u8, remaining: u32 },
    /// Atrial activation attempt.
    Atrial {
        region: Region,
        kind: EventKind,
        origin: Origin,
        via: AtrialVia,
    },
    /// Impulse reaches the AV-node input.
    AvArrival { origin: Origin },
    /// His bundle activation.
    His { origin: Origin, via: HisVia },
    /// Mobitz II drop at the His level.
    HisBlocked { origin: Origin },
    /// Bundle-branch / fascicle edge event.
    Branch {
        edge: Edge,
        origin: Origin,
        blocked: bool,
    },
    /// Ventricular myocardium activation attempt.
    Ventricular {
        region: Region,
        kind: EventKind,
        origin: Origin,
        via: VentVia,
    },
    /// Retrograde arrival at the His bundle from the ventricles.
    HisRetro { origin: Origin },
    /// Programmed stimulus (`Action::Stimulate` / `PacingTrain`).
    Stimulus { target: StimTarget },
    /// Defibrillator discharge (`Action::Shock`; synchronised shocks are scheduled on the R wave).
    Shock { joules: f64, synchronised: bool },
    /// Next cycle of the [`AtrialDriver`].
    AtrialCircuit,
    /// Next cycle of the [`VentricularDriver`].
    VentricularCircuit,
}

/// Where a programmed stimulus is delivered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum StimTarget {
    Atrium(Region),
    /// A catheter pair in the atria: the front needs `delay_ms` to reach the AV-nodal input from
    /// that site, instead of the sinus node's `atrial_delay_ms`.
    AtrialSite {
        region: Region,
        delay_ms: f64,
    },
    His,
    Ventricle(Region),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Scheduled {
    pub at: f64,
    pub what: Pending,
}

/// Per-focus run-time state.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FocusState {
    /// Dominant beats of the chamber since the focus last fired (coupled mode).
    pub beats: u32,
    /// Discharges so far (for `max_fires` and exit block).
    pub fires: u32,
    /// Terminated by a successful shock (Tier-1 stub of an ectopic tachycardia).
    pub terminated: bool,
}

/// Run-time state of a driver.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CircuitState {
    /// An episode is under way.
    pub running: bool,
    /// Ended for good (shock, or an episode that does not recur).
    pub stopped: bool,
    /// When a self-limiting episode starts to end, ms.
    pub ends_at_ms: Option<f64>,
    /// Slowing beats already played at the end of an episode.
    pub closing_beats: u32,
    /// A pause-dependent start may not fire before this time, ms.
    pub not_before_ms: f64,
}

/// Errors from [`TimingEngine::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    /// The action exists in the scenario vocabulary but the timing engine cannot honour it
    /// (it belongs to the spatial tiers, e.g. `Ablate`, `Stenosis`, `MoveElectrode`).
    Unsupported(&'static str),
    /// `Action::Block` on a region that is not an edge of the timing graph.
    UnsupportedRegion(Region),
    /// Stimulus site that the timing engine cannot map to a chamber.
    UnsupportedSite(String),
    /// Unknown drug name.
    UnknownDrug(String),
    /// Unknown coronary branch in `Action::Stenosis`.
    UnknownBranch(String),
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionError::Unsupported(what) => {
                write!(f, "action `{what}` is not supported by the timing engine")
            }
            ActionError::UnsupportedRegion(r) => write!(f, "region {r:?} is not an edge of the timing graph"),
            ActionError::UnsupportedSite(s) => write!(f, "stimulus site `{s}` cannot be mapped to a chamber"),
            ActionError::UnknownDrug(d) => write!(f, "unknown drug `{d}`"),
            ActionError::UnknownBranch(b) => write!(f, "unknown coronary branch `{b}`"),
        }
    }
}

impl std::error::Error for ActionError {}

/// Capacity reserved for the pending queue; `PacingTrain` may grow it (not a per-step allocation).
const QUEUE_CAPACITY: usize = 128;

/// Default seed of the shock-outcome generator ([`TimingEngine::set_shock_seed`]).
const DEFAULT_SHOCK_SEED: u64 = 0x5EED_DEF1;
/// No ventricular activation for this long: asystole, ms.
const ASYSTOLE_MS: f64 = 3000.0;
/// RR below this is a tachycardia (> 100 bpm), ms.
const TACHYCARDIA_RR_MS: f64 = 600.0;
/// Ventricular cycle below this, sensed from the tissue model, is fibrillation (> 240 bpm), ms.
const VF_CYCLE_MS: f64 = 250.0;
/// Atrial cycle below this is fibrillation / flutter (> 250 bpm), ms.
const FLUTTER_CYCLE_MS: f64 = 240.0;
/// Mixed into the seed of the circuit generator so that it differs from the shock generator.
const CIRCUIT_SEED_MIX: u64 = 0xC1BC_0175_A7F1_0B5E;
/// An RR this many times the coupling interval is the "long" of a short-long-short sequence.
const SHORT_LONG_RATIO: f64 = 1.5;
/// Beats over which a self-limiting episode slows before it ends (Drew et al. 2010: "the last 2
/// to 3 beats"), and the cycle-length increase per beat.
pub(crate) const CLOSING_BEATS: u32 = 3;
pub(crate) const CLOSING_SLOWING: f64 = 0.2;

/// The timing engine. See the module documentation for the model.
#[derive(Debug, Clone)]
pub struct TimingEngine {
    pub params: TimingParams,
    now: f64,
    queue: Vec<Scheduled>,
    events: Vec<Event>,
    /// Index into `events` where the current step's events start (pacer sensing window).
    step_start: usize,
    ladder: Vec<LadderSample>,
    cells: CellState,

    // --- sinus node ---
    /// Sinus discharges so far (pause / exit-block counters).
    sinus_beats: u32,
    /// Overdrive suppression to add to the next sinus cycle, ms.
    sinus_suppression_ms: f64,

    // --- chambers ---
    last_atrial: Option<f64>,
    last_ventricular: Option<f64>,
    last_vent_via: Option<VentVia>,
    /// Escape timers count from here when nothing has happened yet (reset, shock).
    escape_anchor: f64,
    /// Conduction time to the nodal input of the atrial stimulus being dispatched (catheter site).
    site_delay_ms: Option<f64>,

    // --- AV node / His ---
    /// Recovery clock of the node: time of the last nodal output (scheduled His activation).
    last_his_output: Option<f64>,
    /// Time the His bundle actually fired last (His–His interval, His refractoriness).
    last_his_fired: Option<f64>,
    fast_last_use: Option<f64>,
    slow_last_use: Option<f64>,
    his_beats_since_drop: u32,

    // --- drugs, ischaemia, defibrillation (B14) ---
    pharma: Pharmacology,
    /// Combined drug effect, updated at the start of every step.
    drug: DrugEffects,
    ischaemia: IschaemiaState,
    shock_seed: u64,
    rng: ShockRng,
    /// Energy of a synchronised shock waiting for the next R wave, J.
    sync_armed: Option<f64>,
    shocks: Vec<ShockRecord>,
    /// Last ventricular and atrial cycle lengths (rhythm class at a shock), ms.
    last_v_rr_ms: Option<f64>,
    last_a_cycle_ms: Option<f64>,

    // --- accessory pathway ---
    ap_last_use: Option<f64>,

    // --- sustained arrhythmias ---
    pub(crate) atrial_circuit: CircuitState,
    pub(crate) ventricular_circuit: CircuitState,
    /// Draws the cycle lengths and sites of the drivers.
    pub(crate) circuit_rng: ShockRng,
    /// The last three RR intervals, newest first, ms (short-long-short detection).
    pub(crate) rr_history: [f64; 3],

    // --- foci / pacer ---
    foci: Vec<FocusState>,
    pacer: Pacer,
    /// Pacemaker marker channel since the last drain (paces with capture, senses).
    markers: Vec<PacerMarker>,
    /// Electrogram amplitude per pacer channel measured on the mesh (Tier 1), mV.
    measured_amplitude_mv: [Option<f64>; 3],
}

impl TimingEngine {
    #[must_use]
    pub fn new(params: TimingParams) -> Self {
        let n_foci = params.foci.len();
        let mut e = Self {
            params,
            now: 0.0,
            queue: Vec::with_capacity(QUEUE_CAPACITY),
            events: Vec::with_capacity(64),
            step_start: 0,
            ladder: Vec::with_capacity(64),
            cells: CellState::new(0),
            sinus_beats: 0,
            sinus_suppression_ms: 0.0,
            last_atrial: None,
            last_ventricular: None,
            last_vent_via: None,
            escape_anchor: 0.0,
            site_delay_ms: None,
            last_his_output: None,
            last_his_fired: None,
            fast_last_use: None,
            slow_last_use: None,
            his_beats_since_drop: 0,
            pharma: Pharmacology::default(),
            drug: DrugEffects::NONE,
            ischaemia: IschaemiaState::default(),
            shock_seed: DEFAULT_SHOCK_SEED,
            rng: ShockRng::new(DEFAULT_SHOCK_SEED),
            sync_armed: None,
            shocks: Vec::with_capacity(4),
            last_v_rr_ms: None,
            last_a_cycle_ms: None,
            ap_last_use: None,
            atrial_circuit: CircuitState::default(),
            ventricular_circuit: CircuitState::default(),
            circuit_rng: ShockRng::new(DEFAULT_SHOCK_SEED ^ CIRCUIT_SEED_MIX),
            rr_history: [f64::INFINITY; 3],
            foci: vec![FocusState::default(); n_foci],
            pacer: Pacer::default(),
            markers: Vec::with_capacity(16),
            measured_amplitude_mv: [None; 3],
        };
        e.reset();
        e
    }

    /// Ladder-diagram samples produced since the last call (see [`ladder`]).
    pub fn drain_ladder(&mut self) -> Vec<LadderSample> {
        std::mem::take(&mut self.ladder)
    }

    /// Pacemaker marker-channel entries since the last call: every stimulus (with its capture
    /// result) and every sensed signal. Pace markers are the stimulus sites the strip and the
    /// heart view show; they align with the `Origin::Pacer` events of the same step.
    pub fn drain_pacer_markers(&mut self) -> Vec<PacerMarker> {
        std::mem::take(&mut self.markers)
    }

    /// Marker-channel entries since the last drain, without taking them.
    #[must_use]
    pub fn pacer_markers(&self) -> &[PacerMarker] {
        &self.markers
    }

    /// Set the intrinsic electrogram amplitude the lead of `channel` sees, mV (`None` = the
    /// lead's parameter). Tier 1 measures it from the local V_m upstrokes at the lead tip.
    pub fn set_measured_amplitude_mv(&mut self, channel: PacerChannel, mv: Option<f64>) {
        self.measured_amplitude_mv[channel as usize] = mv;
    }

    /// Electrogram amplitudes measured on the mesh, per channel (index = `PacerChannel`).
    #[must_use]
    pub fn measured_amplitudes_mv(&self) -> [Option<f64>; 3] {
        self.measured_amplitude_mv
    }

    /// Feed back an activation that the tissue model produced on its own (Tier 2 functional
    /// re-entry: flutter, AF, VT, VF), so that the timing graph stays the single source of beat
    /// timing. An atrial activation is taken as already at the AV-nodal input (it conducts or
    /// blocks there, overdrives the sinus node); a ventricular one resets the escape timers, is
    /// seen by the pacemaker's sense amplifier and may conduct retrogradely. The resulting
    /// events carry `Origin::Reentry` and `EventKind::Automatic`. Processed at the next step.
    pub fn sense_tissue(&mut self, chamber: Chamber, region: Region, at: Millis) {
        let at = at.0.max(self.now);
        let (kind, origin) = (EventKind::Automatic, Origin::Reentry);
        match chamber {
            Chamber::Atrium => self.schedule(
                at,
                Pending::Atrial {
                    region,
                    kind,
                    origin,
                    via: AtrialVia::Tissue,
                },
            ),
            Chamber::Ventricle => self.schedule(
                at,
                Pending::Ventricular {
                    region,
                    kind,
                    origin,
                    via: VentVia::Tissue,
                },
            ),
            Chamber::AvJunction => {}
        }
    }

    /// A programmed stimulus from a catheter pair touching `region`, at `at` (brief B22): the
    /// atria, the His bundle (any AV-junction structure) or the ventricles capture it unless they
    /// are refractory, which shows as an [`EventKind::Stimulus`] or [`EventKind::Blocked`] event
    /// with [`Origin::External`]. `atrial_delay_ms` is the conduction time from the pair to the
    /// AV-nodal input (`None` = the sinus node's), so the A–H relation holds when pacing from anywhere.
    pub fn stimulate_region(
        &mut self,
        region: Region,
        at: Millis,
        atrial_delay_ms: Option<f64>,
    ) -> Result<(), ActionError> {
        let at = at.0.max(self.now);
        let target = if region.is_atrial() {
            match atrial_delay_ms {
                Some(delay_ms) => StimTarget::AtrialSite { region, delay_ms },
                None => StimTarget::Atrium(region),
            }
        } else if region.is_conduction_system() {
            StimTarget::His
        } else if region == Region::Scar {
            return Err(ActionError::UnsupportedRegion(region));
        } else {
            StimTarget::Ventricle(region)
        };
        self.schedule(at, Pending::Stimulus { target });
        Ok(())
    }

    /// Events produced since the last drain, without taking them.
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    // --- scheduling -------------------------------------------------------------------------

    pub(crate) fn schedule(&mut self, at: f64, what: Pending) {
        let pos = self.queue.partition_point(|s| s.at <= at);
        self.queue.insert(pos, Scheduled { at, what });
    }

    /// Remove every pending item matching `pred` (used for phase resetting).
    pub(crate) fn unschedule(&mut self, pred: impl Fn(&Pending) -> bool) {
        self.queue.retain(|s| !pred(&s.what));
    }

    pub(crate) fn emit(&mut self, at: f64, region: Region, kind: EventKind, origin: Origin) {
        let ev = Event::new(Millis(at), region, kind, origin);
        // A synchronised shock fires on the R wave: the first ventricular activation after it
        // was armed.
        if ev.kind != EventKind::Blocked
            && ev.chamber == Chamber::Ventricle
            && !matches!(ev.origin, Origin::Circuit(c) if c.is_ventricular())
            && let Some(joules) = self.sync_armed.take()
        {
            self.schedule(
                at + SYNC_DELAY_MS,
                Pending::Shock {
                    joules,
                    synchronised: true,
                },
            );
        }
        self.events.push(ev);
    }

    pub(crate) fn ladder_push(&mut self, s: LadderSample) {
        self.ladder.push(s);
    }

    /// Combined drug effect of the current step.
    #[must_use]
    pub fn drug_effects(&self) -> DrugEffects {
        self.drug
    }

    /// Drug administrations and the drug table (drug-tray timers).
    #[must_use]
    pub fn pharmacology(&self) -> &Pharmacology {
        &self.pharma
    }

    /// Mutable pharmacology, e.g. to replace the drug table or speed up the kinetics.
    pub fn pharmacology_mut(&mut self) -> &mut Pharmacology {
        &mut self.pharma
    }

    /// Coronary lesions in effect (`Action::Stenosis`); Tier 0 draws its ST vector from them.
    #[must_use]
    pub fn ischaemia(&self) -> &IschaemiaState {
        &self.ischaemia
    }

    /// Shocks delivered since the last drain, without taking them.
    #[must_use]
    pub fn shocks(&self) -> &[ShockRecord] {
        &self.shocks
    }

    /// Shocks delivered since the last call (animation, sound, strip artefact).
    pub fn drain_shocks(&mut self) -> Vec<ShockRecord> {
        std::mem::take(&mut self.shocks)
    }

    /// A synchronised shock is waiting for the next R wave.
    #[must_use]
    pub fn shock_armed(&self) -> bool {
        self.sync_armed.is_some()
    }

    /// Withdraw a synchronised shock still waiting for its R wave.
    pub fn cancel_armed_shock(&mut self) {
        self.sync_armed = None;
    }

    /// Seed of the shock-outcome generator (a scenario's `seed`); takes effect now and at
    /// every `reset`.
    pub fn set_shock_seed(&mut self, seed: u64) {
        self.shock_seed = seed;
        self.rng = ShockRng::new(seed);
        self.circuit_rng = ShockRng::new(seed ^ CIRCUIT_SEED_MIX);
    }

    /// The atrial and the ventricular circuit under way at this moment.
    #[must_use]
    pub fn running_circuits(&self) -> (Option<Circuit>, Option<Circuit>) {
        let a = self
            .params
            .atrial_driver
            .as_ref()
            .filter(|_| self.atrial_circuit.running)
            .map(|d| match d.kind {
                AtrialDriverKind::Flutter => Circuit::AtrialFlutter,
                AtrialDriverKind::Fibrillation => Circuit::AtrialFibrillation,
            });
        let v = self
            .params
            .ventricular_driver
            .as_ref()
            .filter(|_| self.ventricular_circuit.running)
            .map(|d| match d.kind {
                VentricularDriverKind::Fibrillation => Circuit::VentricularFibrillation,
                VentricularDriverKind::Torsades => Circuit::Torsades,
                VentricularDriverKind::Polymorphic => Circuit::PolymorphicVt,
            });
        (a, v)
    }

    /// Rhythm class at `t` from the last ventricular and atrial cycles (see [`RhythmClass`]).
    #[must_use]
    pub fn rhythm_class(&self, t: Millis) -> RhythmClass {
        let t = t.0;
        // Before the first beat, silence is counted from the start of the simulation.
        if t - self.last_ventricular.unwrap_or(0.0) >= ASYSTOLE_MS {
            return RhythmClass::Asystole;
        }
        match self.running_circuits() {
            (_, Some(Circuit::VentricularFibrillation)) => return RhythmClass::VentricularFibrillation,
            (_, Some(_)) => return RhythmClass::PolymorphicVentricularTachycardia,
            (Some(Circuit::AtrialFlutter), None) => return RhythmClass::AtrialFlutter,
            (Some(_), None) => return RhythmClass::AtrialFibrillationOrFlutter,
            (None, None) => {}
        }
        let flutter = self.last_a_cycle_ms.is_some_and(|c| c < FLUTTER_CYCLE_MS);
        match self.last_v_rr_ms {
            Some(rr) if rr < TACHYCARDIA_RR_MS => match self.last_vent_via {
                Some(VentVia::Focus | VentVia::Escape | VentVia::External) => {
                    RhythmClass::VentricularTachycardia
                }
                Some(VentVia::Pacer | VentVia::PacerLv) => RhythmClass::Organised,
                // sensed back from the tissue model: organised re-entry (VT) or fibrillation
                Some(VentVia::Tissue) if rr < VF_CYCLE_MS => RhythmClass::VentricularFibrillation,
                Some(VentVia::Tissue) => RhythmClass::VentricularTachycardia,
                _ if flutter => RhythmClass::AtrialFibrillationOrFlutter,
                _ => RhythmClass::SupraventricularTachycardia,
            },
            _ if flutter => RhythmClass::AtrialFibrillationOrFlutter,
            _ => RhythmClass::Organised,
        }
    }

    /// Deliver a shock at `t` (see [`crate::defib`]): draw the outcome, and on success
    /// depolarise every structure at once.
    pub(crate) fn deliver_shock(&mut self, t: f64, joules: f64, synchronised: bool) {
        let rhythm = self.rhythm_class(Millis(t));
        let erp = self.ventricular_erp();
        let vulnerable = !synchronised
            && self.last_ventricular.is_some_and(|v| {
                let d = t - v;
                (VULNERABLE_WINDOW.0 * erp..=VULNERABLE_WINDOW.1 * erp).contains(&d)
            });
        let success = self.rng.next_f64() < success_probability(rhythm, joules);
        if success {
            // Every structure depolarised at once: pending wavefronts die (re-entry
            // terminates), all tissue is refractory from `t`, ectopic tachycardia foci stop.
            self.queue.clear();
            self.last_atrial = Some(t);
            self.last_ventricular = Some(t);
            self.last_vent_via = None;
            self.last_v_rr_ms = None;
            self.last_a_cycle_ms = None;
            self.escape_anchor = t;
            self.last_his_output = Some(t);
            self.last_his_fired = Some(t);
            self.fast_last_use = Some(t);
            self.slow_last_use = Some(t);
            self.ap_last_use = Some(t);
            self.sinus_suppression_ms = 0.0;
            self.rr_history = [f64::INFINITY; 3];
            self.stop_circuits_after_shock(t);
            for (id, f) in self.params.foci.iter().enumerate() {
                if let FocusMode::Automatic { rate_bpm, .. } = f.mode
                    && rate_bpm * self.drug.automaticity_scale > 60_000.0 / TACHYCARDIA_RR_MS
                {
                    self.foci[id].terminated = true;
                }
            }
            self.schedule(t + self.sinus_cycle_ms(t), Pending::Sinus);
            self.schedule_automatic_foci(t);
        }
        self.shocks.push(ShockRecord {
            at: Millis(t),
            joules,
            synchronised,
            rhythm,
            success,
            vulnerable,
        });
    }

    /// A successful shock ends both circuits. The substrate of a recurring pause-dependent
    /// arrhythmia stays, so its next start is only delayed.
    fn stop_circuits_after_shock(&mut self, t: f64) {
        self.atrial_circuit.running = false;
        self.atrial_circuit.stopped = true;
        let recur = self.params.ventricular_driver.as_ref().and_then(|d| {
            matches!(d.start, DriverStart::PauseDependentRonT { .. })
                .then_some(d.recur_after_ms)
                .filter(|r| *r > 0.0)
        });
        let vc = &mut self.ventricular_circuit;
        vc.running = false;
        vc.ends_at_ms = None;
        vc.closing_beats = 0;
        match recur {
            Some(r) => vc.not_before_ms = t + r,
            None => vc.stopped = true,
        }
    }

    // --- drug-modified parameters ------------------------------------------------------------

    /// AV-nodal ERP addition from drugs, ms.
    pub(crate) fn av_erp_bonus(&self) -> f64 {
        self.drug.av_erp_add_ms
    }

    /// Atrial conduction time to the AV-node input, ms (drug CV change).
    pub(crate) fn atrial_delay(&self) -> f64 {
        self.params.atrial_delay_ms / self.drug.atrial_cv_scale
    }

    /// Atrial ERP, ms (drug APD change).
    pub(crate) fn atrial_erp(&self) -> f64 {
        self.params.atrial_erp_ms * self.drug.atrial_apd_scale
    }

    /// Ventricular ERP, ms (drug APD and post-repolarisation refractoriness).
    pub(crate) fn ventricular_erp(&self) -> f64 {
        self.params.ventricular_erp_ms * self.drug.ventricular_apd_scale * self.drug.ventricular_erp_scale
    }

    /// His–Purkinje delays with the drug CV change applied.
    pub(crate) fn his_purkinje(&self) -> HisPurkinjeParams {
        let mut hp = self.params.his_purkinje.clone();
        let k = 1.0 / self.drug.his_purkinje_cv_scale;
        if (k - 1.0).abs() > f64::EPSILON {
            hp.his_ms *= k;
            for e in [&mut hp.rbb, &mut hp.lbb, &mut hp.laf, &mut hp.lpf] {
                e.delay_ms *= k;
            }
        }
        hp
    }

    /// Rate of an automatic focus or escape pacemaker, bpm (drug automaticity change).
    pub(crate) fn automatic_rate(&self, rate_bpm: f64) -> f64 {
        rate_bpm * self.drug.automaticity_scale
    }

    // --- scenario actions ------------------------------------------------------------------

    /// Apply a scenario action at simulation time `at` (clamped to the present). Parameter
    /// changes take effect immediately; stimuli are queued.
    pub fn apply(&mut self, action: &Action, at: Millis) -> Result<(), ActionError> {
        let at = at.0.max(self.now);
        match action {
            Action::SetGlobalSinusRate { bpm } => {
                self.params.sinus.rate_bpm = *bpm;
                Ok(())
            }
            Action::Block { region, blocked } => self.apply_block(*region, *blocked),
            Action::Stimulate { site, .. } => {
                let target = Self::stim_target(site)?;
                self.schedule(at, Pending::Stimulus { target });
                Ok(())
            }
            Action::PacingTrain {
                site,
                s1_cycle_ms,
                s1_count,
                s2_ms,
                s3_ms,
            } => {
                let target = Self::stim_target(site)?;
                let mut t = at;
                for i in 0..*s1_count {
                    t = at + f64::from(i) * s1_cycle_ms;
                    self.schedule(t, Pending::Stimulus { target });
                }
                if let Some(s2) = s2_ms {
                    t += s2;
                    self.schedule(t, Pending::Stimulus { target });
                    if let Some(s3) = s3_ms {
                        t += s3;
                        self.schedule(t, Pending::Stimulus { target });
                    }
                }
                Ok(())
            }
            Action::AccessoryPathway {
                enabled,
                antegrade,
                retrograde,
            } => {
                if *enabled {
                    let ap = self
                        .params
                        .accessory
                        .get_or_insert_with(AccessoryPathway::default);
                    ap.antegrade = *antegrade;
                    ap.retrograde = *retrograde;
                } else {
                    self.params.accessory = None;
                }
                Ok(())
            }
            Action::Drug { name, dose } => {
                if self.pharma.administer(name, *dose, at) {
                    Ok(())
                } else {
                    Err(ActionError::UnknownDrug(name.clone()))
                }
            }
            Action::Shock { joules, synchronised } => {
                if *synchronised {
                    self.sync_armed = Some(*joules);
                } else {
                    self.schedule(
                        at,
                        Pending::Shock {
                            joules: *joules,
                            synchronised: false,
                        },
                    );
                }
                Ok(())
            }
            Action::Stenosis { branch, severity } => self
                .ischaemia
                .apply(branch, *severity)
                .map_err(|e| ActionError::UnknownBranch(e.0)),
            Action::SetRegion { .. } => Err(ActionError::Unsupported("SetRegion")),
            Action::Ablate { .. } => Err(ActionError::Unsupported("Ablate")),
            Action::MoveElectrode { .. } => Err(ActionError::Unsupported("MoveElectrode")),
        }
    }

    fn apply_block(&mut self, region: Region, blocked: bool) -> Result<(), ActionError> {
        let hp = &mut self.params.his_purkinje;
        match region {
            Region::AvNodeCompact | Region::HisBundle => self.params.av.complete_block = blocked,
            Region::AvNodeFastPathway => self.params.av.fast.blocked = blocked,
            Region::AvNodeSlowPathway => self.params.av.slow.blocked = blocked,
            Region::RightBundleBranch => hp.rbb.blocked = blocked,
            Region::LeftBundleBranch => hp.lbb.blocked = blocked,
            Region::LeftAnteriorFascicle => hp.laf.blocked = blocked,
            Region::LeftPosteriorFascicle => hp.lpf.blocked = blocked,
            Region::AccessoryPathway => {
                if let Some(ap) = &mut self.params.accessory {
                    ap.antegrade = !blocked;
                    ap.retrograde = !blocked;
                }
            }
            other => return Err(ActionError::UnsupportedRegion(other)),
        }
        Ok(())
    }

    /// Landmarks are matched by prefix: `ra*`/`la*`/`atri*` → atria, `his*` → His bundle,
    /// `rv*`/`lv*`/`vent*` → ventricles. Cell indices and points need the spatial tiers.
    fn stim_target(site: &StimSite) -> Result<StimTarget, ActionError> {
        let StimSite::Landmark(name) = site else {
            return Err(ActionError::UnsupportedSite(format!("{site:?}")));
        };
        let n = name.to_ascii_lowercase();
        if n.starts_with("ra") || n.starts_with("atri") {
            Ok(StimTarget::Atrium(Region::RightAtrium))
        } else if n.starts_with("la") {
            Ok(StimTarget::Atrium(Region::LeftAtrium))
        } else if n.starts_with("his") {
            Ok(StimTarget::His)
        } else if n.starts_with("rv") {
            Ok(StimTarget::Ventricle(Region::RightVentricle))
        } else if n.starts_with("lv") || n.starts_with("vent") {
            Ok(StimTarget::Ventricle(Region::LeftVentricle))
        } else {
            Err(ActionError::UnsupportedSite(name.clone()))
        }
    }

    fn schedule_automatic_foci(&mut self, t: f64) {
        for id in 0..self.params.foci.len() {
            if let FocusMode::Automatic { rate_bpm, .. } = self.params.foci[id].mode
                && rate_bpm > 0.0
                && !self.foci[id].terminated
            {
                self.schedule(
                    t + 60_000.0 / self.automatic_rate(rate_bpm),
                    Pending::FocusFire {
                        id: id as u8,
                        remaining: 1,
                    },
                );
            }
        }
    }
}

impl Engine for TimingEngine {
    fn step(&mut self, dt: Millis) {
        self.now += dt.0;
        self.step_start = self.events.len();
        self.drug = self.pharma.effects_at(self.now);
        while self.queue.first().is_some_and(|s| s.at <= self.now) {
            let s = self.queue.remove(0);
            self.dispatch(s.at, s.what);
        }
        self.check_escape();
        self.run_pacer();
    }

    fn now(&self) -> Millis {
        Millis(self.now)
    }

    fn cells(&self) -> &CellState {
        &self.cells
    }

    fn drain_events(&mut self) -> Vec<Event> {
        self.step_start = 0;
        std::mem::take(&mut self.events)
    }

    fn reset(&mut self) {
        self.now = 0.0;
        self.queue.clear();
        self.events.clear();
        self.ladder.clear();
        self.step_start = 0;
        self.sinus_beats = 0;
        self.sinus_suppression_ms = 0.0;
        self.last_atrial = None;
        self.last_ventricular = None;
        self.last_vent_via = None;
        // Escape timers count from reset, so no escape beat precedes the first sinus beat.
        self.escape_anchor = 0.0;
        self.site_delay_ms = None;
        self.last_his_output = None;
        self.last_his_fired = None;
        self.fast_last_use = None;
        self.slow_last_use = None;
        self.his_beats_since_drop = 0;
        self.pharma.clear();
        self.drug = DrugEffects::NONE;
        self.ischaemia.clear();
        self.rng = ShockRng::new(self.shock_seed);
        self.sync_armed = None;
        self.shocks.clear();
        self.last_v_rr_ms = None;
        self.last_a_cycle_ms = None;
        self.ap_last_use = None;
        self.atrial_circuit = CircuitState::default();
        self.ventricular_circuit = CircuitState::default();
        self.circuit_rng = ShockRng::new(self.shock_seed ^ CIRCUIT_SEED_MIX);
        self.rr_history = [f64::INFINITY; 3];
        self.foci.clear();
        self.foci.resize(self.params.foci.len(), FocusState::default());
        self.pacer.reset(0.0);
        self.markers.clear();
        self.schedule(200.0, Pending::Sinus);
        self.schedule_automatic_foci(0.0);
        if let Some(d) = &self.params.atrial_driver {
            let start = d.start_ms.max(0.0);
            self.schedule(start, Pending::AtrialCircuit);
        }
        if let Some(VentricularDriver {
            start: DriverStart::AtMs(start),
            ..
        }) = self.params.ventricular_driver
        {
            self.schedule(start.max(0.0), Pending::VentricularCircuit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(params: TimingParams, seconds: f64) -> Vec<Event> {
        let mut e = TimingEngine::new(params);
        let mut all = Vec::new();
        let steps = (seconds * 1000.0) as usize;
        for _ in 0..steps {
            e.step(Millis(1.0));
            all.extend(e.drain_events());
        }
        all
    }

    fn count(evs: &[Event], region: Region, kind: EventKind) -> usize {
        evs.iter()
            .filter(|e| e.region == region && e.kind == kind)
            .count()
    }

    #[test]
    fn sinus_rhythm_conducts_one_to_one() {
        let evs = run(TimingParams::default(), 10.0);
        let p = count(&evs, Region::RightAtrium, EventKind::Automatic);
        let v = count(&evs, Region::LeftVentricle, EventKind::Conducted);
        assert!((11..=12).contains(&p), "{p} P waves at 70 bpm in 10 s");
        assert_eq!(p, v);
        // PR = atrial delay + AVCT_min (+ recovery term ≈ 0 at 70 bpm) + His–Purkinje ≈ 160 ms
        let first_p = evs.iter().find(|e| e.region == Region::RightAtrium).unwrap().at;
        let first_v = evs.iter().find(|e| e.region == Region::LeftVentricle).unwrap().at;
        let pr = first_v.0 - first_p.0;
        assert!((150.0..=200.0).contains(&pr), "PR {pr}");
    }

    #[test]
    fn complete_block_gives_junctional_escape() {
        let mut p = TimingParams::default();
        p.av.complete_block = true;
        let evs = run(p, 10.0);
        assert_eq!(
            count(&evs, Region::LeftVentricle, EventKind::Conducted),
            7,
            "45 bpm escape ≈ 7 beats in 10 s"
        );
        assert!(count(&evs, Region::AvNodeCompact, EventKind::Blocked) >= 10);
        assert!(
            evs.iter()
                .filter(|e| e.region == Region::LeftVentricle)
                .all(|e| e.origin == Origin::Escape)
        );
    }

    #[test]
    fn fast_atrial_rate_produces_wenckebach_or_2to1() {
        let mut p = TimingParams::default();
        p.sinus.rate_bpm = 220.0;
        let evs = run(p, 10.0);
        let p = count(&evs, Region::RightAtrium, EventKind::Automatic);
        let v = count(&evs, Region::LeftVentricle, EventKind::Conducted);
        assert!(v < p, "some atrial beats must be blocked: P={p} V={v}");
        assert!(count(&evs, Region::AvNodeCompact, EventKind::Blocked) > 0);
    }

    #[test]
    fn mobitz2_drops_every_third() {
        let mut p = TimingParams::default();
        p.av.mobitz2_ratio = 3;
        let evs = run(p, 30.0);
        let p = count(&evs, Region::RightAtrium, EventKind::Automatic);
        let v = count(&evs, Region::LeftVentricle, EventKind::Conducted);
        assert!((v as f64 / p as f64 - 2.0 / 3.0).abs() < 0.05, "P={p} V={v}");
        assert!(count(&evs, Region::HisBundle, EventKind::Blocked) >= 10);
    }

    #[test]
    fn mobitz2_high_grade_conducts_one_in_three() {
        let mut p = TimingParams::default();
        p.av.mobitz2_ratio = 3;
        p.av.mobitz2_conducted = 1;
        p.escape.junctional_bpm = 0.0;
        p.escape.ventricular_bpm = 0.0;
        let evs = run(p, 30.0);
        let p = count(&evs, Region::RightAtrium, EventKind::Automatic);
        let v = count(&evs, Region::LeftVentricle, EventKind::Conducted);
        assert!((v as f64 / p as f64 - 1.0 / 3.0).abs() < 0.05, "P={p} V={v}");
    }

    #[test]
    fn deterministic() {
        let a = run(TimingParams::default(), 5.0);
        let b = run(TimingParams::default(), 5.0);
        assert_eq!(a, b);
    }

    #[test]
    fn events_are_chronological_and_queue_does_not_grow() {
        let mut p = TimingParams::default();
        p.foci.push(Focus::pvc(500.0, 2));
        p.pacer = Some(PacerConfig::new(PacerMode::Ddd, 60.0));
        let mut e = TimingEngine::new(p);
        let cap = e.queue.capacity();
        let mut last = f64::NEG_INFINITY;
        for _ in 0..20_000 {
            e.step(Millis(1.0));
            for ev in e.drain_events() {
                assert!(ev.at.0 >= last - 1e-9, "event at {} after {last}", ev.at.0);
                last = ev.at.0;
            }
        }
        assert_eq!(
            e.queue.capacity(),
            cap,
            "pending queue must not reallocate during stepping"
        );
    }

    #[test]
    fn unsupported_actions_are_reported() {
        let mut e = TimingEngine::new(TimingParams::default());
        assert_eq!(
            e.apply(
                &Action::Block {
                    region: Region::Scar,
                    blocked: true
                },
                Millis::ZERO
            ),
            Err(ActionError::UnsupportedRegion(Region::Scar))
        );
        assert!(matches!(
            e.apply(
                &Action::Drug {
                    name: "snake oil".into(),
                    dose: 1.0
                },
                Millis::ZERO
            ),
            Err(ActionError::UnknownDrug(_))
        ));
        assert!(matches!(
            e.apply(
                &Action::Stimulate {
                    site: StimSite::Cell(3),
                    strength: 1.0,
                    duration_ms: 2.0
                },
                Millis::ZERO
            ),
            Err(ActionError::UnsupportedSite(_))
        ));
    }
}
