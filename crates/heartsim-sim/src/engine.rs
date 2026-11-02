use heartsim_core::{Millis, Region};
use serde::{Deserialize, Serialize};

/// Per-cell state exposed to renderers and the ECG forward model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CellState {
    /// Transmembrane potential, normalised 0 (rest) … 1 (peak); multiply by ~100 mV for display.
    pub vm: Vec<f32>,
    /// Time of last activation, ms (NaN = never).
    pub activation_ms: Vec<f32>,
    /// Current APD assigned at the last activation, ms.
    pub apd_ms: Vec<f32>,
}

impl CellState {
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self {
            vm: vec![0.0; n],
            activation_ms: vec![f32::NAN; n],
            apd_ms: vec![0.0; n],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
    /// A pacemaker region fired spontaneously.
    Automatic,
    /// Activation handed over from another structure (e.g. atria → AV node → His).
    Conducted,
    /// External stimulus (pacing, PES, user click).
    Stimulus,
    /// An impulse was blocked (refractory tissue or a block setting).
    Blocked,
}

/// Where an impulse originated. Conducted events carry the origin of the impulse they
/// propagate, so a ladder diagram can colour a whole beat by its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Origin {
    /// Sinoatrial node.
    Sinus,
    /// Ectopic focus number `id` in `TimingParams::foci`.
    Focus(u8),
    /// Impulse travelled over the accessory (Kent) bundle: pre-excited ventricular activation
    /// or retrograde atrial activation in AVRT.
    AccessoryPathway,
    /// A beat sustained by a re-entrant loop (AVNRT / AVRT echo and every later cycle).
    Reentry,
    /// Junctional or ventricular escape pacemaker.
    Escape,
    /// Artificial pacemaker channel (`timing::PacerChannel`: `0` = atrial, `1` = (right)
    /// ventricular, `2` = left-ventricular lead of a biventricular device).
    Pacer(u8),
    /// Programmed stimulus or user click (`Action::Stimulate`, `Action::PacingTrain`).
    External,
}

/// Coarse anatomical level of an event, i.e. the row of a ladder diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Chamber {
    Atrium,
    /// AV node, His bundle, bundle branches, fascicles, Purkinje and accessory pathway.
    AvJunction,
    Ventricle,
}

impl Chamber {
    /// Every [`Region`] maps to exactly one chamber; pathological tissue tags default to
    /// `Ventricle` (scar, border zone and ischaemia are ventricular in this product).
    #[must_use]
    pub fn of(region: Region) -> Self {
        if region.is_atrial() {
            Chamber::Atrium
        } else if region.is_conduction_system() || region == Region::AccessoryPathway {
            Chamber::AvJunction
        } else {
            Chamber::Ventricle
        }
    }
}

/// Discrete events emitted by engines — the audit trail the UI uses for beeps, markers and
/// ladder diagrams. `region` is the precise structure, `chamber` the ladder row and `origin`
/// the source of the impulse being propagated.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub at: Millis,
    pub region: Region,
    pub kind: EventKind,
    pub origin: Origin,
    pub chamber: Chamber,
}

impl Event {
    /// Build an event, deriving `chamber` from `region`.
    #[must_use]
    pub fn new(at: Millis, region: Region, kind: EventKind, origin: Origin) -> Self {
        Self {
            at,
            region,
            kind,
            origin,
            chamber: Chamber::of(region),
        }
    }
}

/// Every engine tier implements this. Engines must be deterministic given the same inputs and
/// seed, must not allocate per step after warm-up, and must never touch the GPU or the file
/// system (the app layer does that).
pub trait Engine {
    /// Advance by exactly one fixed step.
    fn step(&mut self, dt: Millis);
    fn now(&self) -> Millis;
    /// Cell state after the last step (may be empty for Tier 0).
    fn cells(&self) -> &CellState;
    /// Events produced during the last step, in order.
    fn drain_events(&mut self) -> Vec<Event>;
    /// Reset to t = 0 with the same configuration.
    fn reset(&mut self);
}
