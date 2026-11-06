//! Simulation engines. See `docs/ARCHITECTURE.md` §4 for the tier design.
//!
//! * [`engine::Engine`] — the trait every tier implements: fixed-step, deterministic, no I/O.
//! * [`timing`] — the conduction-system timing engine (SA node, AV node with decremental
//!   conduction and refractoriness, escape pacemakers). It is the *single source of truth* for
//!   beat timing so that the ECG strip and the 3D activation always agree.
//! * [`ap`] — action-potential templates (shape × APD) shared by Tier 0/1.
//! * [`tier1`] — Tier-1 anisotropic graph-eikonal activation on the cell cloud with AP
//!   templates, driven by timing events ([`Tier1Engine`] couples the two).
//! * [`tier2`] — Tier-2 monodomain reaction–diffusion on a masked voxel grid (re-entry, VT/VF,
//!   flutter/AF), driven by the same timing engine ([`tier2::Tier2Engine`]).
//! * [`pharma`] — drugs and electrolyte disturbances as parameter-modifier sets with onset /
//!   offset kinetics (`assets/drugs.ron`), [`ischaemia`] — coronary stenosis and infarction by
//!   territory, [`defib`] — synchronised cardioversion / defibrillation outcomes (brief B14).
//! * [`ep`] — the EP lab (brief B22): programmed stimulation, ERP search, entrainment and RF
//!   lesions on top of the tier-switching engine.

pub mod ap;
pub mod defib;
pub mod engine;
pub mod ep;
pub mod ischaemia;
pub mod pharma;
pub mod tier1;
pub mod tier2;
pub mod timing;

pub use engine::{CellState, Chamber, Engine, Event, EventKind, Origin};
pub use tier1::{EikonalEngine, EikonalParams, SeedMap, Tier1Engine};
pub use timing::{ActionError, LadderSample, TimingEngine, TimingParams};
