# Running tests and benchmarks

## The CI entry point

```bash
cargo xtask ci
```

runs what CI runs: `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` (pedantic lints are errors),
the whole test suite and `cargo deny check` (licences and advisories). Run it before every pull request.

The GUI crates take several minutes to build from cold; while iterating on physiology, test the pure crates first:

```bash
cargo test -p heartsim-core -p heartsim-geometry -p heartsim-sim -p heartsim-ecg
```

## Golden snapshots

`crates/heartsim-ecg/tests/golden.rs` compares simulated ECGs with stored `insta` snapshots, and the tests in
`crates/heartsim-sim/tests/` pin beat timing and intervals for every rhythm. A change that alters a waveform fails them; if the
change is intended, review the diff with `cargo insta review` (from `cargo install cargo-insta`) and commit the updated
snapshot together with the change and the reason.

## Rendering tests

`cargo test -p heartsim-render --test gpu` renders offscreen. Without a GPU it uses Mesa's software Vulkan (lavapipe):

```bash
WGPU_BACKEND=vulkan cargo test -p heartsim-render --test gpu
```

When no adapter is found the render tests skip.

## Benchmarks

The engines have `criterion` benchmarks:

```bash
cargo bench -p heartsim-sim --bench tier1
```

```bash
cargo bench -p heartsim-ecg --bench forward
```

`tier1` measures the eikonal activation (Dijkstra per beat) and the per-step voltage update on the default anatomy pack;
`forward` measures the forward model (cell voltages to electrode potentials). Reports go to `target/criterion/`
(open `report/index.html`). Compare against a baseline with `-- --save-baseline before` and `-- --baseline before`.
Benchmarks use the `bench` profile (release with debug info), so they also work under `perf` and flame-graph tools.

## Screenshots for the documentation

The screenshots in this guide are taken by the app itself:

```bash
HEARTSIM_SCREENSHOT_DIR=docs/images WGPU_BACKEND=vulkan xvfb-run -a -s "-screen 0 1280x800x24" cargo run -p heartsim-app
```
