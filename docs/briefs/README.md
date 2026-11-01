# Implementation briefs

Each brief is a self-contained work package for a Claude subagent (or a human). Read `../../CLAUDE.md` first, then the brief, then
only the documents it points to. Recommended models: **Opus** for numerics/architecture, **Sonnet** for feature work, **Haiku** for
mechanical, fully specified tasks. Order and parallelism: `../ROADMAP.md`.

| id | title | crates | model | milestone |
|---|---|---|---|---|
| B1 | Timing-engine extensions (foci, dual AV pathways, accessory pathway, pacemaker channels) | sim | Opus | M1 |
| B2 | Anatomy pipeline (open mesh → anatomy pack: cells, fibres, hoods, torso, electrodes, landmarks) | geometry, xtask | Opus | M2 |
| B3 | Tier-1 eikonal activation engine | sim | Opus | M2 |
| B4 | ECG analysis: QRS/P/T detection, intervals, QTc, heart vector | ecg | Sonnet | M1 |
| B5 | Forward model: electrode weight matrix, EDL/dipole, electrode moves | ecg | Opus | M2 |
| B6 | Multi-mesh scene: layers, transparency, clip plane, glyphs, lines, screenshots | render | Sonnet | M2 |
| B7 | Strip and 12-lead pages: formats, sweep/gain, pause/scrub, calipers, sweep-bar mode | ui, app | Sonnet | M1 |
| B8 | Cabrera axis wheel, heart-vector view, axis drill quiz, rhythm quiz | ui, app | Sonnet | M1 |
| B9 | Colour maps and picking (V_m, isochrones, APD; click-to-stimulate) | render, ui, app | Sonnet | M2 |
| B10 | Electrode editor on the torso (drag, presets, misplacement detection) | ui, app, ecg | Sonnet | M3 |
| B11 | Tier-0 scenario library (25 rhythms as RON + golden tests) | assets, ecg, sim | Haiku | M1 |
| B12 | Packaging, signing and release CI (cargo-packager, rcodesign, workflows) | xtask, .github | Sonnet | M1 |
| B13 | Pacing devices: timing cycles, temporary pacemaker UI, implanted presets | sim, ui, app | Sonnet | M3 |
| B14 | Drugs, ischaemia by coronary branch, defibrillation | sim, ecg, ui | Sonnet | M3 |
| B15 | Tutorial engine (markdown pages with simulator actions) and scenario timeline editor | app, ui | Sonnet | M3 |
| B16 | Tier-2 reaction–diffusion engine (Aliev–Panfilov, MS, FK; CPU; optional wgpu) | sim | Opus | M4 |
| B17 | Re-entry presets (AF, flutter, VT, VF, torsades) and Tier switching | sim, assets | Sonnet | M4 |
| B18 | Content: 40 scenarios with explanations, 10 tutorials, 3 cases | assets | Haiku/Sonnet | M5 |
| B19 | i18n (Fluent) and accessibility pass | ui, app, assets | Haiku | M5 |
| B20 | Exports (PNG, PDF, CSV, WFDB) and user docs (mdBook) | app, cli, docs | Sonnet | M5 |
| B21 | WASM build (trunk, WebGPU/WebGL2) | app, web | Sonnet | M6 |
| B22 | EP lab: catheters, EGMs, PES, ablation | sim, ecg, render, ui | Opus | M7 |
| B23 | Validation bridge: openCARP/VTK export-import, benchmark report | cli, geometry | Opus | M8 |
