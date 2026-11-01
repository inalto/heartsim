# B16 — Tier-2 reaction–diffusion engine
**Crate** `heartsim-sim` (`src/tier2/*.rs`, feature `gpu` optional) · **Model** Opus · **Milestone** M4 · **Depends on** B2, B3, B5.

## Goal
A masked-voxel monodomain solver with phenomenological cell models that can sustain functional re-entry (spiral waves, break-up)
for AF/flutter/VT/VF/torsades, initialised from Tier-1 state and sampled back to cells for the forward model and colour maps.

## Read first
`docs/research/electrophysiology-and-ecg-models.md` §2.3 (equations + parameter tables for Aliev–Panfilov, Mitchell–Schaeffer,
Fenton–Karma, BOCF; which model does what; wavefront-width rule), §6 (dt/dx stability, substeps per frame, CPU vs GPU),
`docs/research/user-deep-research-takeaways.md` (validation protocol, openCARP as oracle), `docs/ARCHITECTURE.md` §4.3,
`docs/research/rust-stack.md` §4.1–4.2.

## Scope
1. Voxelise the cell cloud: grid spacing h (default 2 mm), mask of active voxels, per-voxel region/depth/fibre (nearest cell),
   diffusion tensor `D = D_l f fᵀ + D_t (I − f fᵀ)` or isotropic fallback; boundary = no-flux (mirror).
2. Models behind a trait `CellModel { fn step(&self, state: &mut [f32], i_stim: f32, dt) ; fn vm(&self, state) -> f32 }`:
   Aliev–Panfilov (default), Mitchell–Schaeffer, Fenton–Karma 3V; parameter sets per region mapped from `RegionParams` (APD ↔ τ_close
   / k,a; CV ↔ D).
3. Time stepping: operator splitting, explicit Euler with substeps (dt ≤ h²/(6D) and model-specific limits), rayon over z-slabs,
   fixed reduction order; optional wgpu compute path (ping-pong storage buffers, WGSL kernels) behind `gpu`.
4. Coupling: `from_tier1(cells)` maps each cell's template phase to model state; `to_cells(vm)` samples voxel V_m back to cells
   (trilinear); timing-engine events become stimulus currents at seed voxels; sensing feeds activations back to the timing engine.
5. Re-entry tools: S1–S2 cross-field initiation, obstacle/scar masks, APD heterogeneity patches, "break-up" parameter presets;
   phase-singularity count as a diagnostic (for tests and the UI).
6. Budget: 64³ grid, AP model, dt 0.25 ms → ≤ 8 ms per 16.7 ms frame on 4 cores (benchmark in CI, loose threshold).

## Acceptance tests
* 1-D cable: CV vs D matches the analytic/known relation within 10 %; APD restitution curve of MS matches its analytic formula.
* 2-D sheet: S1–S2 protocol creates a spiral that persists ≥ 2 s (phase-singularity count ≥ 1); AP with break-up parameters →
  PS count grows; MS with long τ_close → single stable rotor.
* 3-D idealised heart: sinus beat activation times agree with Tier 1 within RMS 8 ms; after `Shock`, all voxels at rest within
  50 ms.
* Determinism across runs; `gpu` path (if available) matches CPU within 1e-3 after 100 steps.
