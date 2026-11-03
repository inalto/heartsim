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
    AccessoryPathway, AvParams, AvPathway, BundleEdge, EscapeParams, Focus, FocusMode, FocusRegion,
    HisPurkinjeParams, LeadParams, LeadSite, PMT_DETECT_CYCLES, PacerChannel, PacerConfig, PacerMode,
    PacerPreset, Side, SinusParams, TimingParams,
};

use crate::engine::{CellState, Engine, Event, EventKind, Origin};
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
}

/// Where a programmed stimulus is delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StimTarget {
    Atrium(Region),
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
        }
    }
}

impl std::error::Error for ActionError {}

/// Capacity reserved for the pending queue; `PacingTrain` may grow it (not a per-step allocation).
const QUEUE_CAPACITY: usize = 128;

/// Adenosine: transient AV-nodal ERP increase, ms per unit dose, and its duration, ms.
const ADENOSINE_ERP_BONUS_MS_PER_DOSE: f64 = 300.0;
const ADENOSINE_DURATION_MS: f64 = 10_000.0;

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

    // --- AV node / His ---
    /// Recovery clock of the node: time of the last nodal output (scheduled His activation).
    last_his_output: Option<f64>,
    /// Time the His bundle actually fired last (His–His interval, His refractoriness).
    last_his_fired: Option<f64>,
    fast_last_use: Option<f64>,
    slow_last_use: Option<f64>,
    his_beats_since_drop: u32,
    /// Adenosine: ERP bonus and expiry.
    av_erp_bonus_ms: f64,
    av_erp_bonus_until: f64,

    // --- accessory pathway ---
    ap_last_use: Option<f64>,

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
            last_his_output: None,
            last_his_fired: None,
            fast_last_use: None,
            slow_last_use: None,
            his_beats_since_drop: 0,
            av_erp_bonus_ms: 0.0,
            av_erp_bonus_until: f64::NEG_INFINITY,
            ap_last_use: None,
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
        self.events.push(Event::new(Millis(at), region, kind, origin));
    }

    pub(crate) fn ladder_push(&mut self, s: LadderSample) {
        self.ladder.push(s);
    }

    /// Effective AV-nodal ERP addition from drugs at `t`, ms.
    pub(crate) fn av_erp_bonus(&self, t: f64) -> f64 {
        if t < self.av_erp_bonus_until {
            self.av_erp_bonus_ms
        } else {
            0.0
        }
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
            Action::Drug { name, dose } => self.apply_drug(name, *dose, at),
            Action::Shock { .. } => {
                // Every structure depolarised at once: pending wavefronts die (re-entry
                // terminates) and all tissue is refractory from `at`.
                self.queue.clear();
                self.last_atrial = Some(at);
                self.last_ventricular = Some(at);
                self.escape_anchor = at;
                self.last_his_output = Some(at);
                self.last_his_fired = Some(at);
                self.fast_last_use = Some(at);
                self.slow_last_use = Some(at);
                self.ap_last_use = Some(at);
                self.sinus_suppression_ms = 0.0;
                self.schedule(at + self.sinus_cycle_ms(at), Pending::Sinus);
                self.schedule_automatic_foci(at);
                Ok(())
            }
            Action::SetRegion { .. } => Err(ActionError::Unsupported("SetRegion")),
            Action::Ablate { .. } => Err(ActionError::Unsupported("Ablate")),
            Action::Stenosis { .. } => Err(ActionError::Unsupported("Stenosis")),
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

    /// Drugs the timing engine knows. Adenosine: transient (10 s) AV-nodal ERP increase of
    /// 300 ms × dose — enough to break AVNRT/AVRT and cause brief AV block.
    fn apply_drug(&mut self, name: &str, dose: f64, at: f64) -> Result<(), ActionError> {
        match name.to_ascii_lowercase().as_str() {
            "adenosine" => {
                self.av_erp_bonus_ms = ADENOSINE_ERP_BONUS_MS_PER_DOSE * dose.max(0.0);
                self.av_erp_bonus_until = at + ADENOSINE_DURATION_MS;
                Ok(())
            }
            other => Err(ActionError::UnknownDrug(other.to_string())),
        }
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
            {
                self.schedule(
                    t + 60_000.0 / rate_bpm,
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
        self.last_his_output = None;
        self.last_his_fired = None;
        self.fast_last_use = None;
        self.slow_last_use = None;
        self.his_beats_since_drop = 0;
        self.av_erp_bonus_ms = 0.0;
        self.av_erp_bonus_until = f64::NEG_INFINITY;
        self.ap_last_use = None;
        self.foci.clear();
        self.foci.resize(self.params.foci.len(), FocusState::default());
        self.pacer.reset(0.0);
        self.markers.clear();
        self.schedule(200.0, Pending::Sinus);
        self.schedule_automatic_foci(0.0);
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
                    name: "digoxin".into(),
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
