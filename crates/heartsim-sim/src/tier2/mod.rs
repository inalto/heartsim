//! Tier 2: monodomain reaction–diffusion on a masked voxel grid (brief B16;
//! `docs/ARCHITECTURE.md` §4.3, research §2.3 and §6).
//!
//! * [`grid`] — the masked voxel grid (active voxels in slab order, compartments, fibres) and the
//!   18-neighbour stencil of the fibre tensor `D = D_t I + (D_l − D_t) f fᵀ`.
//! * [`model`] — the [`CellModel`] trait and Aliev–Panfilov (default), Mitchell–Schaeffer and
//!   Fenton–Karma 3V, in ms with a normalised membrane variable.
//! * [`solver`] — [`Monodomain`]: explicit Euler with operator splitting, no-flux masks,
//!   stimuli, activation times, shock, phase-singularity count. Deterministic and thread-count
//!   independent; CPU only (rayon over slab chunks with the `parallel` feature).
//! * [`calibrate`] — `RegionParams` → model: CV ↔ D on a 1-D cable of the solver's own `h`/`dt`,
//!   APD ↔ the model's plateau dial on a single cell.
//! * [`engine`] — [`Tier2Engine`]: timing engine + solver on the voxelised cell cloud, Tier-1 hand-over,
//!   tissue → timing sensing, voxel → cell sampling for the forward ECG and colour maps.
//! * [`reentry`] — S1–S2 cross-field, obstacles, APD patches / mosaic, the break-up preset.
//! * [`preset`] — the `reentry:` section of scenario files (patches, initiation, switch policy), PVI and
//!   CTI ablation lines (brief B17).
//! * [`switching`] — [`SwitchingEngine`]: Tier 1 until re-entry is needed, then Tier 2, and back after 2 s
//!   without it (brief B17).
//!
//! **Numerical guard rails.** The explicit step is stable while
//! `dt·(2·Σ_j w_ij + 1/max_dt) ≤ 2` per voxel and the upstroke spans ≥ 2 steps
//! (`dt ≤ ½ max_dt`); [`Monodomain::new`] asserts both, and the calibration only chooses
//! `(D, excitation)` pairs inside that budget. Stimuli are conductances integrated implicitly, so
//! they can never push u past its peak. The optional wgpu compute path of the brief is not
//! implemented yet (no `gpu` feature).

pub mod calibrate;
pub mod engine;
pub mod grid;
pub mod model;
pub mod preset;
pub mod reentry;
pub mod solver;
pub mod switching;

pub use engine::{SnapshotError, Tier2Engine, Tier2Params, Tier2Snapshot};
pub use grid::{Compartment, VoxelGrid, VoxelSpec};
pub use model::{AlievPanfilov, CellModel, FentonKarma, MitchellSchaeffer};
pub use preset::{
    Initiation, Patch, PatchEffect, PatchTarget, Preset, PresetError, ReentrySpec, SwitchPolicy,
};
pub use solver::{Monodomain, Tissue, TissueModifier, Trajectory};
pub use switching::{SwitchReason, SwitchRecord, SwitchingEngine, Tier, requires_reentry};
