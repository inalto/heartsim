//! Simulation engines. See `docs/ARCHITECTURE.md` §4 for the tier design.
//!
//! * [`engine::Engine`] — the trait every tier implements: fixed-step, deterministic, no I/O.
//! * [`timing`] — the conduction-system timing engine (SA node, AV node with decremental
//!   conduction and refractoriness, escape pacemakers). It is the *single source of truth* for
//!   beat timing so that the ECG strip and the 3D activation always agree.
//! * [`ap`] — action-potential templates (shape × APD) shared by Tier 0/1.
//! * Eikonal activation and reaction–diffusion live in briefs B3/B5 and are not yet implemented.

pub mod ap;
pub mod engine;
pub mod timing;

pub use engine::{CellState, Engine, Event, EventKind};
pub use timing::{TimingEngine, TimingParams};
