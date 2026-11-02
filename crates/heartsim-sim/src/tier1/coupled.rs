//! Tier 1 as one [`Engine`]: the timing engine decides *when* structures fire, the eikonal engine
//! turns each of those events into a wavefront on the mesh in the same step.

use super::EikonalEngine;
use crate::engine::{CellState, Engine, Event};
use crate::timing::{ActionError, TimingEngine, TimingParams};
use heartsim_core::{Action, Millis};

/// Timing engine + eikonal engine, stepped together. Events returned by
/// [`Engine::drain_events`] are the timing engine's (the single source of beat timing); the
/// mesh-level events of the eikonal engine (front reaching the AV-node input or an
/// accessory-pathway insertion) are kept apart in [`Tier1Engine::drain_mesh_events`].
#[derive(Debug, Clone)]
pub struct Tier1Engine {
    timing: TimingEngine,
    eikonal: EikonalEngine,
    /// Timing events already handed to the eikonal engine (index into `timing.events()`).
    ingested: usize,
}

impl Tier1Engine {
    /// Couple a timing engine built from `timing` with `eikonal`; the eikonal engine inherits
    /// the His–Purkinje delays and the accessory-pathway sides of `timing`.
    #[must_use]
    pub fn new(timing: TimingParams, mut eikonal: EikonalEngine) -> Self {
        eikonal.set_his_purkinje(timing.his_purkinje.clone());
        eikonal.set_accessory(
            timing
                .accessory
                .as_ref()
                .map(|ap| (ap.ventricular_side, ap.atrial_side)),
        );
        Self {
            timing: TimingEngine::new(timing),
            eikonal,
            ingested: 0,
        }
    }

    /// The timing engine (beat timing, ladder diagram).
    #[must_use]
    pub fn timing(&self) -> &TimingEngine {
        &self.timing
    }

    /// The eikonal engine (activation times, V_m).
    #[must_use]
    pub fn eikonal(&self) -> &EikonalEngine {
        &self.eikonal
    }

    /// Mutable eikonal engine, for configuration (focus sites, leads, modifiers).
    pub fn eikonal_mut(&mut self) -> &mut EikonalEngine {
        &mut self.eikonal
    }

    /// Apply an action to both engines. It succeeds if either engine honours it (e.g. `Ablate`
    /// is spatial only, `Drug { adenosine }` timing only); otherwise the timing engine's error is
    /// returned. A `Stimulate` at a cell index or point reaches the mesh only, because the timing
    /// engine cannot map it to a chamber.
    pub fn apply(&mut self, action: &Action, at: Millis) -> Result<(), ActionError> {
        let t = self.timing.apply(action, at);
        let e = self.eikonal.apply(action, at);
        if matches!(action, Action::AccessoryPathway { .. }) {
            self.eikonal.set_accessory(
                self.timing
                    .params
                    .accessory
                    .as_ref()
                    .map(|ap| (ap.ventricular_side, ap.atrial_side)),
            );
        }
        match (t, e) {
            (Ok(()), _) | (_, Ok(())) => Ok(()),
            (Err(err), Err(_)) => Err(err),
        }
    }

    /// Mesh-level events since the last call.
    pub fn drain_mesh_events(&mut self) -> Vec<Event> {
        self.eikonal.drain_events()
    }
}

impl Engine for Tier1Engine {
    fn step(&mut self, dt: Millis) {
        self.timing.step(dt);
        let evs = self.timing.events();
        self.eikonal.ingest(&evs[self.ingested..]);
        self.ingested = evs.len();
        self.eikonal.step(dt);
    }

    fn now(&self) -> Millis {
        self.timing.now()
    }

    fn cells(&self) -> &CellState {
        self.eikonal.cells()
    }

    fn drain_events(&mut self) -> Vec<Event> {
        self.ingested = 0;
        self.timing.drain_events()
    }

    fn reset(&mut self) {
        self.timing.reset();
        self.eikonal.reset();
        self.ingested = 0;
    }
}
