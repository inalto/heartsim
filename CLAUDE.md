# heartsim — guidance for Claude Code sessions and subagents

heartsim is a cross-platform (Windows / macOS / Linux, WASM later) **3-D heart electrophysiology and 12-lead ECG simulator for
medical education**, written in Rust. Read `docs/README.md` for the document map; the architecture is in `docs/ARCHITECTURE.md`,
work packages in `docs/briefs/`.

## Non-negotiables
1. **Legal**: `epicardio/` and `epicardio-extracted/` contain a proprietary third-party program and data extracted from it. They are
   git-ignored and exist only for study. Never copy meshes, textures, parameter tables, text, icons, sounds or code from them into the
   product. Re-derive physiology parameters from the cited literature (`docs/research/`). Only CC-BY / MIT / BSD / Apache assets ship.
2. **Determinism**: engines in `heartsim-sim` must be deterministic for a given scenario + seed, fixed-step, free of I/O and GPU calls.
   Rendering interpolates; it never drives the simulation.
3. **One source of truth**: beat timing comes from `heartsim_sim::TimingEngine` events; the ECG strip and the 3-D colour map are both
   derived from engine state so they can never disagree.
4. **Layering** (`core` → `geometry` → `sim` → `ecg` → `render` → `ui` → `app`/`cli`): lower crates never depend on higher ones;
   `core`, `geometry`, `sim`, `ecg` compile without wgpu/egui and on `wasm32`.
5. **Units in names**: `_ms`, `_mv`, `_mm`, `_bpm`, `_m_s`. Heart coordinates are millimetres, +x = patient's left, +y = cranial,
   +z = anterior.
6. **Tests before "done"**: every brief lists its acceptance tests; run `cargo xtask ci` (fmt, clippy -D warnings, tests, deny) before
   reporting completion. Golden-ECG snapshot tests live in `crates/heartsim-ecg/tests/` and `crates/heartsim-sim/tests/`.

## Toolchain
* Rust pinned in `rust-toolchain.toml` (1.99.1); MSRV 1.95. `cargo xtask ci` is the CI entry point.
* Stack: wgpu 30 · eframe/egui 0.37 · egui-wgpu 0.37 · winit 0.30 (do **not** upgrade one without the others; see
  `docs/research/rust-stack.md` §1.2). GUI docking: `egui_tiles` (not `egui_dock`). Audio: `kira`. Packaging: `cargo-packager`.
* Dependencies are declared once in the root `[workspace.dependencies]`; crates use `dep.workspace = true`.
* `cargo build` of the GUI crates takes several minutes cold; run pure-crate tests first:
  `cargo test -p heartsim-core -p heartsim-geometry -p heartsim-sim -p heartsim-ecg`.
* No GPU on the dev box: GUI smoke tests use `WGPU_BACKEND=vulkan` with lavapipe if present, otherwise skip render tests.

## Conventions
* Edition 2024, `clippy::pedantic` warnings are errors in CI; allow-list in root `Cargo.toml`.
* Public items get a doc comment that states the physiology or the invariant, not the obvious.
* Scenario and parameter files are RON under `assets/scenarios/`; they carry `format_version`.
* Commit messages: Conventional Commits (`feat(sim): …`, `fix(ui): …`). Do not commit unless asked.
* When a brief says "model: haiku/sonnet/opus", that is the recommended Claude model for the implementer; the brief is self-contained.
