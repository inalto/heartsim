# heartsim — technical architecture

Status: v0.1 (2026-10-31). Companion to `REQUIREMENTS.md`. Research backing: `research/rust-stack.md`,
`research/electrophysiology-and-ecg-models.md`, `reverse-engineering/EPICARDIO-ANALYSIS.md`.

## 1. Principles
1. **One state, two views.** Beat timing and tissue state are computed once per fixed step; the ECG strip, the colour map, the heart
   vector and the ladder diagram are all *derived* from that state. Nothing in the UI can make them disagree.
2. **Tiers, not one solver.** Tier 0 (template), Tier 1 (graph + eikonal + AP templates, default), Tier 2 (reaction–diffusion for
   re-entry). Same parameter vocabulary, same ECG forward model, same scenario files. (The research-grade monodomain/HPC path is an
   *export bridge* to openCARP/MonoAlg3D, not something we re-implement.)
3. **Data-driven pathologies.** A pathology is a scenario file (parameters, masks, templates, timeline). Adding one needs no code.
4. **Pure core.** `core/geometry/sim/ecg` have no GPU, GUI, I/O or wall-clock dependencies, compile to wasm32, and are what the CLI
   and the tests exercise.
5. **Determinism.** Fixed dt, seeded RNG, fixed-order reductions. Golden snapshots guard every preset.

## 2. Crate map
```
heartsim-core      units (Millis), SimClock, Region, Electrode/Lead + 12-lead derivation, FrontalAxis, Parameters, Scenario (RON)
heartsim-geometry  TriMesh (+glTF/OBJ import), CellCloud (positions, fibres, regions, depth, k-NN with distance & fibre cosine),
                   vertex→cell hoods, landmarks, torso + electrode sites, primitives (icosphere), anatomy pack loader
heartsim-sim       Engine trait, TimingEngine (SA/AV/His/escape; events), ApTemplate, Tier-1 EikonalEngine (activation, V_m),
                   Tier-2 RdEngine (Aliev–Panfilov/MS/FK on voxels), interventions (stimulus, block, ablation, pathway, stenosis,
                   defib, drugs), pacemaker state machines
heartsim-ecg       LeadVectors, Tier-0 GaussianSynth, ForwardModel (weight matrix W[electrode, cell] from positions; EDL/dipole),
                   LeadRing + min/max decimation, filters/noise, interval & wave detection, QTc, heart vector, export (CSV/WFDB)
heartsim-render    wgpu: offscreen scene (colour+depth), per-vertex scalar buffer + LUT textures, clip planes, picking rays, glyphs,
                   lines (vector arrow, catheters), offscreen capture to PNG
heartsim-ui        egui widgets: ecg_strip (painter), twelve-lead page, Viewport3d (egui_wgpu callback), Cabrera wheel, calipers,
                   parameter panels, ladder diagram, scenario browser, quiz views; egui_tiles layout; Fluent i18n
heartsim-app       eframe app: model (clock, engine, synth/forward, rings), settings, persistence, audio (kira), exports, tutorials
heartsim-cli       headless: run scenario, export CSV/PNG/PDF, benchmark, validate presets
xtask              ci, asset pipeline (decimate meshes, bake hoods/fibres/W matrix), release checks
```
Dependency direction is strictly downward in this list.

## 3. Runtime data flow (per frame)
```
winit/eframe frame (dt) ──► SimClock.advance(dt·speed) → n steps
  for each step (1 ms):
     TimingEngine.step ──► events {Automatic, Conducted, Blocked, Stimulus} @ (region, t)
     Tier1.step(events)  ──► activation front (eikonal arrival times per cell, once per beat) + V_m[cell] (template × APD × DI)
     [Tier2.step if active ──► V_m[voxel]]
     Forward.sample(V_m) ──► 10 electrode potentials ──► 12 leads ──► LeadRing[12].push  (at 500 Hz)
  UI:
     strip painter reads rings (min/max per pixel column)
     viewport: scalars = hood(V_m) → queue.write_buffer → offscreen render → blit in egui pass
     axis/vector panels read the last QRS window; ladder diagram reads events
```
Rendering never writes engine state; interactions become `Action`s queued for the next step.

## 4. Simulation design
### 4.1 Timing engine (implemented in `sim::timing`)
Nodes SA → atria → AV (recovery curve `AVCT = min + A·exp(−(RT−ERP)/τ)`, Mobitz II ratio, complete block) → His → ventricles; escape
timers (junctional 45, ventricular 30 bpm) reset by any ventricular activation; respiration modulation of SA cycle. Extensions in
briefs: dual AV pathways (AVNRT loop), accessory pathway edge (WPW/AVRT), ectopic foci with coupling intervals, overdrive
suppression, pacemaker channels (sense/pace timers, blanking, PVARP, upper-rate), concealed conduction.

### 4.2 Tier 1 — graph + anisotropic eikonal + AP templates
* Cells: `CellCloud` (≈ 20–50 k points) with 8-NN edges carrying distance `d_ij` and fibre alignment `c_ij = |cos θ|`.
* Edge delay `τ_ij = d_ij / CV_ij`, `CV_ij = CV_long · sqrt(c² + (ρ·(1−c²)))` (ρ = anisotropy ratio) × CV restitution(DI) × drug/ischaemia
  factors; `CV = 0` for non-conducting regions/ablated cells.
* Activation: event-driven Dijkstra from the seed sites of each timing event (SA cells, His terminals/PMJ sites, pacing site, ectopic
  focus). Arrival `t_j = min(t_i + τ_ij)` subject to refractoriness: a cell accepts activation only if `t ≥ t_act + APD·erp_fraction`.
  Because activations are discrete events, re-activation (e.g. retrograde) is handled naturally; **functional re-entry is not**
  (Tier 2's job).
* V_m: `vm[j](t) = template_region(phase)` with `phase = (t − t_act[j]) / APD_j`, `APD_j = APD(region, depth, DI, drugs, ischaemia)`.
* Cost: Dijkstra 50 k nodes × 8 edges ≈ 5–15 ms once per beat (spread across steps or on a worker thread); per-step V_m update
  O(N) ≈ 0.1 ms. CPU only.

### 4.3 Tier 2 — reaction–diffusion
Masked voxel grid (64³ default, 2 mm) built from the cell cloud; Aliev–Panfilov (`k=8, a=0.15, ε0=0.002, μ1=0.2, μ2=0.3`) with
explicit Euler, dt 0.1–0.25 ms, D ≈ 0.1 mm²/ms scaled per region, fibre tensor optional. rayon over slabs; optional wgpu compute
(storage buffers, ping-pong) behind the `gpu` feature; results sampled back to cells for the forward model. Entered from Tier 1 by
mapping each cell's template phase to `(u, v)`.

### 4.4 Forward ECG
* Precompute `W[e, j]` for the 9 signal electrodes: equivalent-double-layer / dipole form
  `φ_e = Σ_j w_ej · V_m,j`, `w_ej = −(σ_i/4πσ_b) · (A_j n_j · (r_e − x_j)) / |r_e − x_j|³` for surface cells, falling back to `1/|r|^p`
  (p = `ecg_distance_power`) for volumetric clouds. Homogeneous infinite medium first; BEM/lead-field torso (`research` §3.2) later via
  the same `W` interface.
* Electrode moves → recompute the affected row only (cheap). Lead derivation in `core::lead`.
* Per-wave gain knobs (P, QRS, T) kept for pedagogy but default to 1.
* Signal chain: 500 Hz sampling → optional filters → rings. Interval detection (Pan–Tompkins-style QRS, P/T search windows) feeds
  read-outs and quizzes.

### 4.5 Interventions
`core::scenario::Action` is the only way to mutate an engine; the app queues actions with timestamps, so tutorials, timelines and
user clicks go through one path and are replayable. Drugs are parameter-modifier sets with onset/offset curves.

## 5. Rendering
* Offscreen pass (Rgba8UnormSrgb + Depth32Float) sized to the widget in physical pixels, blitted in egui's pass (implemented).
* Per-vertex scalar vertex buffer updated with `queue.write_buffer`; LUT 256×1 textures (V_m, isochrone, APD); WebGL2-safe (no
  storage buffers in VS).
* Multi-mesh scene: chambers, conduction system, vessels, torso (alpha-blended, drawn last), electrodes/catheters as instanced
  spheres/tubes, heart-vector arrow as a line mesh; clip plane via a uniform and `discard`.
* Picking: CPU ray–triangle against `TriMesh` (parry3d optional) → nearest cell via `CellCloud::nearest`.
* Screenshots: copy offscreen texture to a mapped buffer → PNG (`image`).

## 6. UI
egui 0.37; `egui_tiles` for dockable panes (Viewport, Strip, 12-lead, Parameters, Scenarios, Vector/Axis, Ladder, Tutorial, Quiz).
Strip: custom painter (implemented) with min/max decimation; 12-lead page tiles the same painter. Toolbar: run/pause, speed,
scenario picker, presets. Keyboard: space pause, ←/→ scrub, 1–9 views. i18n with Fluent (`assets/locales/*.ftl`). Theme light/dark.

## 7. Files and formats
* `assets/scenarios/*.ron` — `core::Scenario` (versioned).
* `assets/anatomy/<pack>/` — `mesh_*.glb` (render), `cells.bin` (positions f32×3, fibres f32×3, region u8, depth f32),
  `neighbours.bin`, `hoods/<mesh>.bin` (8 × u32 per vertex), `landmarks.ron`, `electrodes.ron`, `torso.glb`, `LICENSE`.
  Built by `cargo xtask bake-anatomy` from the source mesh (Strocchi/Rodero cohort or Fritz/Gerach, CC BY) — see brief B2.
* Exports: CSV (`t_ms, I..V6`), WFDB (header + 16-bit), PNG, PDF (printpdf), RON.
* Settings: `directories` config dir, `settings.ron`.

## 8. Testing strategy
* Unit: lead identities (proptest), axis roundtrip, timing-engine rhythms (counts, PR, Wenckebach), AP template, decimation
  extremes (proptest), kNN sorting/alignment, camera.
* Golden: `insta` CSV snapshots (rounded, 100 Hz) of every preset's 12 leads over 5 s; interval checks (PR 120–200, QRS < 120 for
  sinus; QRS ≥ 120 for BBB; axis ranges for LAFB/LPFB).
* Physics: CV calibration on a slab cloud (measured vs set CV ± 5 %); planar-wave activation times vs analytic; Tier 2 spiral
  persistence test; Tier 1/Tier 2 agreement on a sinus beat (activation RMS < 5 ms).
* Render: offscreen render to PNG on lavapipe in CI, mean-abs-diff threshold.
* GUI: `egui_kittest` smoke tests for panels.
* Bench: criterion for `Tier1::step`, Dijkstra, decimation, forward sample.
* Runner: `cargo nextest`; `cargo xtask ci` mirrors CI.

## 9. Packaging and CI
GitHub Actions (`.github/workflows/ci.yml`): fmt/clippy/deny on Linux; tests on ubuntu-22.04, windows-2025, macos-15; MSRV check;
wasm32 check of pure crates. Release (`release.yml`, brief B12): tag → cargo-packager (MSI+NSIS, .dmg universal via lipo + rcodesign
notarisation, AppImage+.deb) → GitHub Release with SHA256 and `THIRD_PARTY_LICENSES.html` (cargo-about).

## 10. Risks and mitigations
| risk | mitigation |
|---|---|
| Open anatomy lacks atria/conduction detail | Strocchi cohort has four chambers + fibres; conduction system generated procedurally (fractal tree + landmarks); fall back to procedural idealised heart for M1. |
| Tier 1 morphology not "textbook" enough | Per-region APD maps + calibration against PTB-XL NORM medians; per-wave gains as last resort. |
| egui/wgpu version churn | All versions pinned in workspace; upgrade in one PR with the coupling table from `research/rust-stack.md`. |
| CI machines without GPU | lavapipe for render tests; sim/ecg tests are CPU only. |
| Scope creep (EP lab, devices) | P2 behind feature flags; P0/P1 gate releases. |
