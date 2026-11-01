# Takeaways from the user's deep-research document (Italian, `user-deep-research-it.md`)

Source: "Ricerca approfondita: simulatore 3D fisiologicamente accurato del cuore con ECG e propagazione elettrica" (ChatGPT deep
research, 2026). Full Markdown conversion in `user-deep-research-it.md`. This file distils what it changes for `heartsim`.

## What the document recommends
1. **Multiscale chain**: anatomy + fibres → cell electrophysiology → tissue propagation → conduction system → extracardiac sources →
   torso forward model/ECG → visualisation. Keep the layers separable (openCARP's structure).
2. **Three fidelity levels**: scientific (monodomain + O'Hara-Rudy / Courtemanche + His-Purkinje), fast (eikonal / reaction-eikonal or
   Aliev–Panfilov / Bueno-Orovio), bidomain only when the extracellular field matters.
3. **"Charge movement" is not physical**: show depolarisation fronts, V_m maps, isochrones, current-density vectors, (optionally)
   specific ionic currents — never animated "electrons".
4. **Do not write the PDE solver first**: use openCARP / MonoAlg3D (MIT) / Chaste (BSD) as scientific oracle; export VTK/HDF5;
   build the interactive frontend separately ("simulation truth" vs "interactive surrogate").
5. **Anatomy**: Fritz/Gerach four-chamber model (Zenodo), Rodero/Strocchi 24-heart cohort, 2025 50-model CT cohort,
   InSilicoHeartGen 2026; fibres via Laplace–Dirichlet rule-based methods; Purkinje via published generators.
6. **Numerics**: 0.1–0.2 mm and 0.01–0.02 ms for research-grade monodomain; pre-pace cells to limit cycle; calibrate conductivities
   to target CV; operator splitting, Rush–Larsen.
7. **ECG forward model**: lead-field / pseudo-bidomain for many surface points, point recovery for few electrodes; homogeneous torso
   first, compartments later; ECGSIM as independent reference.
8. **Validation protocol**: single-cell traces, spatial/temporal convergence, Niederer 2011 N-version benchmark, FDA analytic
   verification problems (Pathmanathan & Gray), CV longitudinal/transverse, APD90 and restitution, activation/repolarisation sequences,
   QRS duration/morphology, synthetic ECG vs ECGSIM/experimental, regression tests per release; ASME V&V 40 for anything clinical.
9. **Licensing**: openCARP = academic licence (commercial needs agreement); life^x-ep CC BY-NC-ND; Chaste BSD; MonoAlg3D MIT;
   ECGSIM GPLv3; VTK/ParaView BSD.
10. **Timeline** for a research platform: 12–18 months / 3–5 people on existing solvers; 24–36+ months with a proprietary HPC solver.

## How this maps onto `heartsim` (decisions)
* Our product goal is **real-time education on a laptop** (Epicardio / cardiacrhythmsim class), not an HPC research platform. We
  therefore adopt the document's *fast level* as the product core and treat the *scientific level* as an **external validation path**,
  not as something we re-implement in Rust in v1:
  * **Tier 0** – template ECG synthesiser (McSharry-style) for rhythm-strip training (instant, no 3D needed).
  * **Tier 1 (core)** – conduction-system graph + anisotropic eikonal / fast-marching activation on a cell cloud or surface mesh +
    phenomenological AP (Mitchell–Schaeffer / Aliev–Panfilov with restitution) → pseudo-ECG by current-dipole summation /
    precomputed lead field. 60 fps on CPU.
  * **Tier 2** – reaction–diffusion monodomain with a phenomenological model (Aliev–Panfilov, Bueno-Orovio minimal) on a coarse
    grid / surface for re-entry, spiral break-up, VF; CPU (rayon) first, wgpu compute later.
  * **Tier 3 (bridge, not a solver)** – export geometry, fibres, parameters and stimuli to openCARP / MonoAlg3D formats and import
    their V_m / activation-time results (VTK/VTU, HDF5) for side-by-side validation and for generating high-fidelity "ground truth"
    animations. This is how we honour "truth vs surrogate" without the licence and HPC burden.
* Visualisation vocabulary follows point 3: V_m colour map, activation isochrones, repolarisation/APD maps, wavefront isosurface,
  current-density / heart-vector arrows, fibre glyphs, Purkinje tree, electrodes; "charges" in the UI means the **electrical heart
  vector and dipole field**, explained as such in tutorials.
* The validation protocol (point 8) becomes our CI test matrix, scaled to the tier: unit tests for single-cell models against
  published traces, CV calibration tests, Niederer-benchmark-style activation-time comparisons (coarse), golden-ECG snapshot tests
  for every pathology, QRS/PR/QT interval checks against textbook ranges, PTB-XL statistical comparison for lead morphologies.
* Anatomy: start from the Strocchi/Rodero cohort or Fritz/Gerach model (CC BY) decimated to a render mesh + a ~20–50 k node
  simulation cloud, with LDRB fibres and a generated Purkinje tree; keep an importer for openCARP/VTK meshes.
* Reproducibility (point 4/architecture): every scenario file records engine version, mesh hash, model id, parameters, dt, seed.
* Electromechanics: deferred to a later milestone as a cosmetic contraction (template-driven vertex displacement) first, true
  mechanics never in scope for v1.
