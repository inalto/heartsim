//! Simulation engines. See `docs/ARCHITECTURE.md` §4 for the tier design.
//!
//! * [`engine::Engine`] — the trait every tier implements: fixed-step, deterministic, no I/O.
//! * [`timing`] — the conduction-system timing engine (SA node, AV node with decremental
//!   conduction and refractoriness, escape pacemakers). It is the *single source of truth* for
//!   beat timing so that the ECG strip and the 3D activation always agree.
//! * [`ap`] — action-potential templates (shape × APD) shared by Tier 0/1.
//! * [`tier1`] — Tier-1 anisotropic graph-eikonal activation on the cell cloud with AP
//!   templates, driven by timing events ([`Tier1Engine`] couples the two).
//! * [`pharma`] — drugs and electrolyte disturbances as parameter-modifier sets with onset /
//!   offset kinetics (`assets/drugs.ron`), [`ischaemia`] — coronary stenosis and infarction by
//!   territory, [`defib`] — synchronised cardioversion / defibrillation outcomes (brief B14).
//! * Reaction–diffusion (Tier 2) lives in brief B16 and is not yet implemented.

pub mod ap;
pub mod defib;
pub mod engine;
pub mod ischaemia;
pub mod pharma;
pub mod tier1;
pub mod timing;

pub use engine::{CellState, Chamber, Engine, Event, EventKind, Origin};
pub use tier1::{EikonalEngine, EikonalParams, SeedMap, Tier1Engine};
pub use timing::{ActionError, LadderSample, TimingEngine, TimingParams};
