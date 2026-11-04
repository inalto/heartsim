# Architecture in brief

heartsim is a Cargo workspace of small crates with a strict downward dependency order. The full design is in
[`docs/ARCHITECTURE.md`](https://github.com/inalto/heartsim/blob/master/docs/ARCHITECTURE.md); this page is the map.

```text
heartsim-core      units (Millis), SimClock, regions, electrodes and the 12-lead derivation, axis maths, Scenario (RON)
heartsim-geometry  meshes, the cell cloud (positions, fibres, regions, neighbours), anatomy packs, torso and electrode sites
heartsim-sim       the timing engine (SA, AV, His, escape, pathways, pacemakers; emits events), Tier-1 eikonal engine,
                   Tier-2 reaction-diffusion, interventions
heartsim-ecg       Tier-0 Gaussian synthesiser, forward model (cells → electrodes), ring buffers, analysis (QRS/P/T,
                   intervals, axis, heart vector), scenario runs
heartsim-render    wgpu: offscreen heart rendering, colour maps, picking, glyphs, capture
heartsim-ui        egui widgets: strip, 12-lead page, calipers, axis dial, vector view, panels, CPU snapshots
heartsim-io        exports and imports: 12-lead PDF, PNG, CSV, WFDB, scenario RON, share bundles
heartsim-app       the desktop application (eframe): the model, tabs, tutorials, quizzes
heartsim-cli       headless runs and exports, scenario validation
xtask              CI entry point, asset pipeline, release checks
```

`core`, `geometry`, `sim` and `ecg` contain no GPU, GUI, file or clock access and compile for `wasm32`; they are what the tests
and the CLI exercise.

## Three rules that shape the code

1. **One source of truth.** Beat timing comes from the timing engine's events. The ECG strip and the colours on the heart are
   both derived from engine state in the same step, so they cannot disagree. Scrubbing reads recorded history; it never
   re-simulates.
2. **Determinism.** Engines are fixed-step (1 ms) and seeded; a scenario run is a pure function of the scenario file. Golden
   ECG snapshots in `crates/heartsim-ecg/tests/` and `crates/heartsim-sim/tests/` catch any change. Exports inherit this: the
   app and the CLI write identical files for the same run.
3. **Data, not code, for physiology.** A rhythm is a scenario file; a drug is an entry in a data table. Parameters come
   from the literature cited in `docs/research/`, with sources in each file's notes.

## A frame of the app

```text
frame dt ──► SimClock: n fixed 1 ms steps
  each step: TimingEngine.step → events → synthesiser / Tier-1 engine → 12 lead samples → strip rings (500 Hz)
UI: strip and 12-lead page read the rings; the 3-D view reads per-cell voltage; panels read the analysis of the rings
```

Rendering interpolates and never drives the simulation; user interactions become `Action`s applied at the next step.

## Units and coordinates

Names carry units: `_ms`, `_mv`, `_mm`, `_bpm`, `_m_s`. Heart coordinates are millimetres with +x to the patient's left,
+y cranial and +z anterior.
