//! Tier 1 as one [`Engine`]: the timing engine decides *when* structures fire, the eikonal engine
//! turns each of those events into a wavefront on the mesh in the same step.
//!
//! **Pacing leads** (brief B13). The leads sit at the cells of their [`LeadSite`]s and the
//! engine measures what each lead would sense: a [`LeadProbe`] integrates the positive V_m
//! slope of the cells around the tip, distance-weighted, over each local activation. Healthy
//! tissue depolarises every cell once per beat, so the integral is 1 and the lead reports its
//! nominal electrogram amplitude; scar, ablation or a lead floating off the wall lower it, and
//! the timing engine's pacemaker then undersenses.

use super::EikonalEngine;
use crate::engine::{CellState, Engine, Event};
use crate::timing::{ActionError, LeadSite, PacerChannel, TimingEngine, TimingParams};
use heartsim_core::{Action, Millis};
use heartsim_geometry::CellCloud;

/// Radius of the tissue a lead tip "sees", mm (a unipolar electrogram is dominated by the
/// few millimetres around the electrode).
pub const LEAD_FIELD_RADIUS_MM: f32 = 5.0;
/// A local activation is complete once the V_m slope around the tip has been flat this long, ms.
const PROBE_QUIET_MS: f64 = 40.0;

/// Electrogram amplitude estimate of one pacing lead (see the module docs).
#[derive(Debug, Clone)]
pub struct LeadProbe {
    cells: Vec<u32>,
    /// Weights `1 / (1 + d / 1 mm)`, normalised to sum 1.
    weights: Vec<f32>,
    prev_vm: Vec<f32>,
    /// Weighted positive V_m change accumulated over the current activation (0 … 1).
    acc: f64,
    quiet_ms: f64,
    /// Fraction of the healthy amplitude measured over the last complete activation.
    fraction: Option<f64>,
}

impl LeadProbe {
    /// Probe around the first cell of `tip`, over cells of the tip's own chamber (atrial vs
    /// ventricular, scar included) within [`LEAD_FIELD_RADIUS_MM`].
    #[must_use]
    pub fn new(cloud: &CellCloud, tip: &[u32]) -> Self {
        let mut probe = Self {
            cells: Vec::new(),
            weights: Vec::new(),
            prev_vm: Vec::new(),
            acc: 0.0,
            quiet_ms: 0.0,
            fraction: None,
        };
        let Some(&t) = tip.first() else {
            return probe;
        };
        let p = cloud.positions[t as usize];
        let atrial = cloud.regions[t as usize].is_atrial();
        for (i, q) in cloud.positions.iter().enumerate() {
            let d = q.distance(p);
            if d <= LEAD_FIELD_RADIUS_MM && cloud.regions[i].is_atrial() == atrial {
                probe.cells.push(i as u32);
                probe.weights.push(1.0 / (1.0 + d));
            }
        }
        let sum: f32 = probe.weights.iter().sum();
        for w in &mut probe.weights {
            *w /= sum.max(f32::MIN_POSITIVE);
        }
        probe.prev_vm = vec![0.0; probe.cells.len()];
        probe
    }

    /// Feed the V_m of one step of length `dt_ms`. Returns the new fraction when a local
    /// activation has just completed.
    pub fn update(&mut self, vm: &[f32], dt_ms: f64) -> Option<f64> {
        let mut rise = 0.0f64;
        for ((&c, &w), prev) in self.cells.iter().zip(&self.weights).zip(&mut self.prev_vm) {
            let v = vm.get(c as usize).copied().unwrap_or(0.0);
            rise += f64::from(w * (v - *prev).max(0.0));
            *prev = v;
        }
        if rise > 1e-6 {
            self.acc += rise;
            self.quiet_ms = 0.0;
        } else if self.acc > 0.0 {
            self.quiet_ms += dt_ms;
            if self.quiet_ms >= PROBE_QUIET_MS {
                self.fraction = Some(self.acc.min(1.0));
                self.acc = 0.0;
                return self.fraction;
            }
        }
        None
    }

    /// Fraction of the healthy electrogram measured at the last activation (`None` before the
    /// first one, or when the lead touches no tissue).
    #[must_use]
    pub fn fraction(&self) -> Option<f64> {
        self.fraction
    }
}

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
    /// Electrogram probes of the pacing leads, indexed by [`PacerChannel`].
    probes: [Option<LeadProbe>; 3],
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
        let mut t1 = Self {
            timing: TimingEngine::new(timing),
            eikonal,
            ingested: 0,
            probes: [None, None, None],
        };
        t1.place_leads();
        t1
    }

    /// Move the pacemaker leads to the sites of the timing engine's pacer configuration and
    /// rebuild their electrogram probes. Call after changing `PacerConfig` lead sites.
    pub fn place_leads(&mut self) {
        let Some(cfg) = self.timing.params.pacer.clone() else {
            self.probes = [None, None, None];
            return;
        };
        self.eikonal.set_pacer_leads(&cfg);
        let seeds = self.eikonal.seeds();
        let cloud = self.eikonal.cloud();
        self.probes = PacerChannel::ALL.map(|ch| {
            let site = cfg.lead(ch)?.site;
            let tip = match ch {
                PacerChannel::Atrial => &seeds.atrial_lead,
                PacerChannel::Ventricular => &seeds.ventricular_lead,
                PacerChannel::LeftVentricular => &seeds.lv_lead,
            };
            // a His lead senses the conduction system, not the myocardium around it
            (site != LeadSite::HisBundle).then(|| LeadProbe::new(cloud, tip))
        });
        for ch in PacerChannel::ALL {
            self.timing.set_measured_amplitude_mv(ch, None);
        }
    }

    /// Mutable timing engine, e.g. to reprogram the pacemaker (call [`Tier1Engine::place_leads`]
    /// after moving a lead).
    pub fn timing_mut(&mut self) -> &mut TimingEngine {
        &mut self.timing
    }

    /// Electrogram amplitude each pacing lead measured at its last local activation, mV
    /// (index = [`PacerChannel`]).
    #[must_use]
    pub fn lead_amplitudes_mv(&self) -> [Option<f64>; 3] {
        self.timing.measured_amplitudes_mv()
    }

    fn update_probes(&mut self, dt_ms: f64) {
        let Some(cfg) = self.timing.params.pacer.clone() else {
            return;
        };
        let vm = &self.eikonal.cells().vm;
        for ch in PacerChannel::ALL {
            let Some(probe) = &mut self.probes[ch as usize] else {
                continue;
            };
            if let Some(f) = probe.update(vm, dt_ms)
                && let Some(lead) = cfg.lead(ch)
            {
                self.timing
                    .set_measured_amplitude_mv(ch, Some(f * lead.intrinsic()));
            }
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
        self.update_probes(dt.0);
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
        self.place_leads();
    }
}
