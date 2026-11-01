# B3 — Tier-1 eikonal activation engine
**Crate** `heartsim-sim` (`src/tier1/*.rs`) · **Model** Opus · **Milestone** M2 · **Depends on** B2 (pack API; use the procedural
heart meanwhile), B1 (events with origin).

## Goal
Given timing events and a `CellCloud`, compute activation times by anisotropic graph-eikonal propagation and emit V_m per cell per
step so that (a) the heart colour map shows a physiological wavefront and (b) the forward model (B5) yields correct QRS/T.

## Read first
`docs/ARCHITECTURE.md` §4.2; `docs/research/electrophysiology-and-ecg-models.md` §2.2 (eikonal, Pezzuto 2017, Wallman 2012),
§2.5.2/2.5.5 (BBB, refractoriness, aberrancy), §3.5 (how P/QRS/T morphology arises — APD gradients), §6 (budgets);
the private reference-product analysis notes (not published) §4 (the reference product's parameters: APD epi/endo, CV per region, restitution
coefficients, Purkinje range, hood smoothing). `crates/heartsim-sim/src/{engine.rs,ap.rs,timing.rs}`.

## Scope
1. `EikonalEngine::new(cloud: Arc<CellCloud>, params: &Parameters)`; precompute per-edge base delay
   `τ_ij = d_ij / (CV_long · sqrt(c_ij² + ρ(1−c_ij²)))`.
2. Seeds from events: SA → `sa_node` landmark cells; His/RBB/LBB/LAF/LPF events → corresponding PMJ seed sets with fascicle delays;
   focus/stimulus → nearest cell to site (`StimSite`); accessory pathway → its ventricular insertion cells.
3. Propagation: Dijkstra (binary heap) per seed group, lazily advanced each step up to `now` (so a beat's front appears
   progressively, not all at once); a cell accepts activation if `t ≥ t_last + APD·erp_fraction` and the region conducts; in the
   relative refractory window (`erp_fraction..1.0`) CV is scaled by `cv_rest` restitution (aberrancy).
4. Per step: `vm[j] = template_region(phase)`, APD from `apd_ms`/`apd_endo_ms` blended by depth, times restitution
   `APD(DI) = apd·(1 − a·exp(−DI/τ))`, times region modifiers (ischaemia, drugs — hook for B14). Separate templates: atrial,
   ventricular, Purkinje (longer), nodal (slow upstroke).
5. Emit `Event`s for mesh-level activations the timing engine needs (e.g. atrial front reaching AV node region → `Conducted` into
   AV; ventricular activation reaching an accessory pathway insertion → retrograde), so re-entrant loops across the graph are
   possible even in Tier 1.
6. Ablated cells (`Scenario::ablated_cells`) and `Action::Ablate` → `conducts = false`.
7. Performance: 50 k cells, 8 edges, Dijkstra ≤ 15 ms per beat spread over steps; per-step V_m update ≤ 0.2 ms (rayon optional,
   fixed-order reductions for determinism).

## Acceptance tests
* Slab cloud (B2 `test_sphere` or a 60×20×10 mm lattice) with fibres along x: measured CV along x = set CV ± 3 %, across = ρ·CV ± 5 %.
* Planar wave on the lattice vs analytic arrival times: RMS < 1 ms.
* Idealised heart sinus beat: atrial activation 80–120 ms total, ventricular activation (QRS) 70–100 ms, apex-before-base,
  endocardium-before-epicardium; LBBB (block LBB edge) → ventricular activation 140–180 ms, LV last; RBBB mirror; LAFB → superior
  LV activation delayed (used by B5 to show axis −45°).
* PVC from `rv_apex` → total activation > 130 ms; pacing from `rv_apex` ditto.
* Premature stimulus inside ERP is ignored; just after ERP conducts with slowed CV (aberrancy).
* Golden snapshot of activation times (hashed histogram) for the default sinus beat.
