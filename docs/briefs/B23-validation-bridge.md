# B23 — Validation bridge to scientific solvers (P2)
**Crates** `heartsim-cli`, `heartsim-geometry` · **Model** Opus · **Milestone** M8 · **Depends on** B2, B3, B16.

## Goal
Export heartsim geometry/fibres/parameters/stimuli to openCARP (and optionally MonoAlg3D) input formats, import their V_m /
activation-time outputs (VTK/VTU, IGB), and produce a comparison report (activation RMS, QRS/T morphology correlation, ECG RMSE)
so the educational engine can be checked against research-grade monodomain simulations without re-implementing them.

## Read first
`docs/research/user-deep-research-it.md` (openCARP/MonoAlg3D/Chaste roles, licences, formats, Niederer benchmark, FDA problems),
`docs/research/user-deep-research-takeaways.md`, `docs/research/electrophysiology-and-ecg-models.md` §2.3 (model parameters),
openCARP documentation (mesh `.pts/.elem/.lon`, `carputils` experiment scripts, IGB output).

## Scope
1. `heartsim-cli export-opencarp <scenario> --out dir`: tetrahedralise the cell cloud or write a point-cloud mesh the solver accepts;
   `.lon` fibres; region tags; stimulus definitions; parameter mapping to TT06/AP; a generated `carputils` script.
2. `heartsim-cli import-vtk <dir>`: read activation times / V_m series into the cell cloud (nearest-point mapping).
3. Report: Markdown + PNG plots (plotters): activation-time scatter, per-lead ECG overlay, metrics table; Niederer slab benchmark
   runner for Tier 2 (dx 0.5/0.2 mm variants, document deviations).
4. Licence note: openCARP's academic licence applies to *running* it; heartsim only writes/reads files.

## Acceptance tests
Export of the idealised heart validates with openCARP's mesh tools (documented manual step), import roundtrip of a synthetic VTU;
report generated for a slab where Tier 2 vs a reference solution (stored CSV from the Niederer paper's published consensus) has
activation RMS < 3 ms at 0.5 mm.
