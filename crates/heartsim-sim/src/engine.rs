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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

/// Discrete events emitted by engines — the audit trail the UI uses for beeps, markers and
/// ladder diagrams (a "laddergram" view is a P1 feature).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub at: Millis,
    pub region: Region,
    pub kind: EventKind,
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
