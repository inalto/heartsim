# heartsim — technical architecture

Status: v0.1 (2026-10-31). Companion to `REQUIREMENTS.md`. Research backing: `research/rust-stack.md`,
`research/electrophysiology-and-ecg-models.md`, the private reference-product analysis notes (not published).

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
                   LeadRing + min/max decimation, filters/noise, interval & wave detection, QTc, heart vector
heartsim-render    wgpu: offscreen scene (colour+depth), per-vertex scalar buffer + LUT textures, clip planes, picking rays, glyphs,
                   lines (vector arrow, catheters), offscreen capture to PNG
heartsim-ui        egui widgets: ecg_strip (painter), twelve-lead page, Viewport3d (egui_wgpu callback), Cabrera wheel, calipers,
                   parameter panels, ladder diagram, scenario browser, quiz views; egui_tiles layout; Fluent i18n
heartsim-io        exports and imports (B20): EcgRecord excerpt, 12-lead PDF (printpdf), page/strip PNG (ui's CPU rasteriser),
                   CSV, WFDB format 16, scenario RON, deterministic share-bundle zip; CSV/WFDB/zip import for review
heartsim-app       eframe app: model (clock, engine, synth/forward, rings), settings, persistence, audio (kira), exports, tutorials
heartsim-cli       headless: `run` a scenario and write the app's exports byte for byte, benchmark, validate presets, openCARP bridge
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
* Implementation (`sim::tier1`, brief B3): one shared lazily-advanced Dijkstra for all fronts (first front to reach a cell
  wins → collisions annihilate). The graph is a radius neighbourhood (≈ 2.3 spacings, 56 neighbours on a lattice; the 8-NN
  graph has up to 70 % metrication error), atria and ventricles insulated, conduction-system cells driven by timing events
  only. The Purkinje tree is a thin fast endocardial layer (1.4 m/s) used only by fronts of its own bundle, so BBB widens the
  QRS through myocardium. Calibrated on the default pack: P ≈ 105 ms (atrial CVs × 1.7 for the spherical atria), QRS ≈ 80 ms,
  LBBB ≈ 150 ms, RBBB ≈ 170 ms (wider than the clinical 120–150 ms: no retrograde Purkinje engagement), LAFB ≈ 85 ms. Measured on a 2013 Xeon VM: 17.5 k cells ≈ 23 ms Dijkstra per beat, 50 k cells ≈ 95 ms (42 edges/cell —
  `EikonalParams::neighbourhood_factor` 1.5 halves it at ≈ 2× the metrication error); V_m update 50 k cells ≈ 0.06 ms.

### 4.3 Tier 2 — reaction–diffusion
Masked voxel grid (64³ default, 2 mm) built from the cell cloud; Aliev–Panfilov (`k=8, a=0.15, ε0=0.002, μ1=0.2, μ2=0.3`) with
explicit Euler, dt 0.1–0.25 ms, D ≈ 0.1 mm²/ms scaled per region, fibre tensor optional. rayon over slabs; optional wgpu compute
(storage buffers, ping-pong) behind the `gpu` feature; results sampled back to cells for the forward model. Entered from Tier 1 by
mapping each cell's template phase to `(u, v)`.

* Implementation (`sim::tier2`, brief B16): active voxels in slab order with an 18-neighbour symmetric graph Laplacian that encodes
  the fibre tensor (no-flux masks, scar, obstacles and the AV ring by construction); cell models behind `CellModel` (Aliev–Panfilov
  default, Mitchell–Schaeffer, Fenton–Karma 3V) with an APD dial and an excitation dial. `RegionParams` map to `(D_l, D_t,
  excitation)` by bisection on a 1-D cable of the solver's own `h`/`dt` (D_l ≈ 2.5 mm²/ms for 0.6 m/s: the slow AP upstroke needs
  far more than the biophysical 0.1) and to the APD dial on a single cell. The explicit step is
  guarded by `dt·(2·load + 1/max_dt) ≤ 2` and `dt ≤ ½ max_dt`; stimuli are conductances integrated by backward Euler (bounded,
  libm-free). On a 2 mm grid the slow AP front is ≈ 6 mm wide, so seed stimuli cover a 5 mm radius (critical nucleus ≈ 2D/CV)
  and fast tissue (Bachmann, crista, the 1.4 m/s Purkinje layer) cannot be carried as tissue within the stability budget: the
  fast endocardial layer is a *driver* (voxel Dijkstra from each bundle's PMJ voxels, stimuli at those delays). Timing events reach
  the tissue through the Tier-1 seed router; re-entrant activations at the AV-nodal input and the RV lead tip are fed back with
  `TimingEngine::sense_tissue`. Sinus beat on the default pack vs Tier 1: activation RMS 6.4 ms (atria 6.4, ventricles 6.5).
* Measured (release, 2013 Xeon E5-2470 v2 VM, 8 threads unless noted, **never on a quiet machine**: load average 8–70 from other
  builds, so treat these as upper bounds; `cargo bench -p heartsim-sim --bench tier2`). Default pack, 34.7 k voxels: build and
  calibration 0.44 s; one 1 ms step (4 substeps) 2.8–3.3 ms in steady running; one 857 ms sinus beat 2.4–2.8 s, i.e. ≈ 0.3–0.36×
  real time. 64³ budget case (84 k-voxel shell, AP, dt 0.25 ms): 46–62 ms per 16.7 ms frame, 61 ms on 4 threads (8–11 ns per
  voxel-update), **6–8× over the 8 ms target**, so Tier 2 is not real-time on this CPU yet. Full 262 k box: 140–368 ms. Not done:
  the wgpu path (`gpu` feature) and SIMD/dense-layout kernels. The test `tier2_budget` does not check the target: it only catches
  a kernel some 20× slower (fastest frame at opt-level 1: 75–480 ms depending on machine load, limit 1500 ms).
* Regimes (tests): default AP keeps one rotor ≥ 2 s; the same cell on the APD-mosaic break-up preset fragments into 3–23 phase
  singularities (the Panfilov 1999 a = 0.1, μ₁ = 0.07 constants did *not* break up in 2-D); Mitchell–Schaeffer holds one stable
  rotor with τ_close = 100 < τ_open (restitution slope ∝ τ_close/τ_open; τ_close = 300 breaks up).

* Re-entry presets and tier switching (brief B17, `sim::tier2::{preset, switching}`): a scenario file's `reentry:` section lists tissue
  patches, an initiation and a switch policy; `SwitchingEngine` runs Tier 1 and continues the same simulation in Tier 2 when the scenario
  or an action (burst pacing, S2/S3, unsynchronised shock) needs re-entry, handing back after 2 s without a ≥ 0.6 s phase-singularity run or
  an unexplained activation at the AV-nodal input / lead tip. Changing APD or CV of calibrated tissue re-solves D on the solver's grid
  (`calibrate::retarget`): scaling them independently stops conduction on a coarse grid. Measured: the short-APD atrial front fails to
  conduct through the narrow atrial bridges at 3–4 mm voxels, so the AF preset runs at 2 mm; flutter, scar VT, torsades and VF could not be
  made to survive in the heart-shaped model with any initiation tried (S1–S2, bursts, cross-field, phase-seeded wave), and are not shipped as
  re-entry presets. (B22 gets flutter only by drawing the circuit: an inert annulus and outer barrier around a seeded wave.)

* EP lab (brief B22, `sim::ep`): `EpLab` wraps `SwitchingEngine` and adds no physiology. A protocol (`ProtocolRun`) is a pure state machine fed
  by timing events and the paced cell's activation time. A catheter stimulus goes to `TimingEngine::stimulate_region` and the mesh front starts
  only on the capture event; in Tier 2 the tissue decides. Lesions go to cells (`ablate_cells_permanently`), voxels, and the timing graph (`Block`,
  `AccessoryPathway`), so they persist across `reset`. Electrograms (`ecg::egm`) are the forward model's dipole kernel on the cells within 15 mm of
  each electrode, summed per source so A, H and V can be labelled exactly. Flutter (`ep_flutter.ron`): the spherical atria have no annulus, so
  `PatchTarget::Band` draws one, plus an outer barrier inside the septal contact with the left atrium and with one exit gap, leaving the rest of the
  atria a dead end. With the barrier further out, outside the septal contact, a periodic wave entered the ring from that side and collided with itself in the
  isthmus (apparently a loop through the left atrium), which a CTI line would not stop. Measured cycle 235 ms. Patches of this kind stay in Tier 2 (`accept_tissue_as_baseline`), so they are not handed back as lesions.

### 4.4 Forward ECG
* Precompute `W[e, j]` for the 9 signal electrodes: equivalent-double-layer / dipole form
  `φ_e = Σ_j w_ej · V_m,j`, `w_ej = −(σ_i/4πσ_b) · (A_j n_j · (r_e − x_j)) / |r_e − x_j|³` for surface cells, falling back to `1/|r|^p`
  (p = `ecg_distance_power`) for volumetric clouds. Homogeneous infinite medium first; BEM/lead-field torso (`research` §3.2) later via
  the same `W` interface.
* Electrode moves → recompute the affected row only (cheap). Lead derivation in `core::lead`.
* Per-wave gain knobs (P, QRS, T) kept for pedagogy but default to 1.
* Implementation (`ecg::forward`, brief B5): volumetric clouds use the exact infinite-medium dipole form
  `φ_e = −k Σ vol·∇V_m·(r_e − x)/|r_e − x|³`, with `∇V_m` the least-squares gradient over k-NN neighbours of the same
  electrical compartment (atria / AV junction / ventricles insulated, scar silent). The gradient is linear in V_m, so it folds
  into `W` and uniform V_m gives exactly 0. Cells with an `A·n` surface patch use the EDL form instead (`SourceModel`);
  `SurfacePatches::boundary_of` builds closed boundary patches for comparison. Limb electrodes default to Einthoven's
  idealisation: far field of the heart vector at hexaxial −150°/−30°/+90°, `φ = p·d̂/R²`, with `R` = 125 mm calibrated so
  that limb and chest amplitudes match. `LimbPlacement::Sites` uses the pack's Mason–Likar sites. With `k` = 5 mV the default
  sinus beat has R(II) ≈ 1.15 mV, S(V1) ≈ 1.4 mV and R(V5) ≈ 0.9 mV. Per-wave gains and the per-cell rest offset
  `o + (1 − o)·V_m` (ischaemic injury current, B14) are folded into the weights. Measured with 52 k cells: `sample` ≈ 0.12 ms, `move_electrode` ≈ 0.7 ms.
  The transmural APD gradient was widened to endo 345 / epi 285 ms (septum uniform 315 ms, since both of its faces are
  endocardium). With the old 30 ms gradient, LV repolarisation was transmurally uniform and the T wave was discordant.
  Known activation-driven gaps (B3): sinus axis ≈ −2°, R amplitude peaks in V2–V3, LAFB/LPFB shift the axis only −14°/−1°.
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
* `assets/anatomy/<pack>/` — `manifest.ron` (format version, provenance, per-file CRC-32), `mesh_*.glb` (render),
  `cells.bin` (positions f32×3, fibres f32×3, region u8, depth f32), `neighbours.bin` (8 × index/distance/alignment),
  `segments.bin` (AHA segment u8), `hoods/<mesh>.bin` (8 × u32 cell + 8 × f32 weight per vertex), `landmarks.ron`
  (named cells + PMJ seed sets), `electrodes.ron`, `torso.glb`, `LICENSE`. Loader: `heartsim_geometry::AnatomyPack`.
  Built by `cargo xtask bake-anatomy` (procedural by default; `--source` for the Strocchi/Rodero cohort, CC BY) —
  see `docs/anatomy-pack.md` and brief B2.
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
GitHub Actions (`.github/workflows/ci.yml`): fmt/clippy/deny on Linux; actionlint over the workflows; tests on ubuntu-22.04,
windows-2025, macos-15; MSRV check; wasm32 check of pure crates.

### 9.1 Release pipeline (`release.yml`, brief B12)
Tag `vX.Y.Z` → `preflight` (`cargo xtask release-check --tag vX.Y.Z --require-tools`) → three native build jobs → `release`, which
attaches everything to a **draft** GitHub Release (publish it by hand after checking). A manual run (`workflow_dispatch`) is the
dry run: same builds, signing and smoke tests, artefacts kept 7 days, no release.

| job | runner | artefacts | smoke test |
|---|---|---|---|
| `linux` | ubuntu-22.04 (glibc 2.35) | `heartsim_X_amd64.deb`, `heartsim_X_x86_64.AppImage` | clean `ubuntu:22.04` container: `apt install ./*.deb` (proves the `depends` list resolves) and AppImage, each running `heartsim --version`; `desktop-file-validate` |
| `windows` | windows-2025 | `HeartSim_X_x64_en-US.msi`, `HeartSim_X_x64-setup.exe` (NSIS, per-user) | PE subsystem is GUI (no console), no `VCRUNTIME140` import (static CRT via `.cargo/config.toml`); silent MSI and NSIS install → `--version` → uninstall |
| `macos` | macos-15 | `HeartSim_X_universal.dmg` | arm64 + x86_64 built with `MACOSX_DEPLOYMENT_TARGET=12.0`, merged with `lipo`; dmg mounted, `lipo -archs`, `--version`, Info.plist (min OS 12.0, `NSHighResolutionCapable`), `codesign --verify`; with secrets also `stapler validate` and `spctl` |
| `release` | ubuntu-22.04 | `SHA256SUMS`, `THIRD_PARTY_LICENSES.html` (cargo-about), CHANGELOG section as release notes | – |

Packaging is configured in `Packager.toml` (cargo-packager 0.11.8): product metadata, icons, `.heartsim.ron` file association, the
`.desktop` template and shared-mime-info XML (`packaging/linux/`), `Info.plist` extras and hardened-runtime entitlements
(`packaging/macos/`). Icons are rasterised from `assets/icon.svg` by `cargo xtask icons` (PNG sizes, `icon.ico`, `icon.icns`);
`cargo xtask prepare-packaging` (the packager's `beforePackagingCommand`) also writes `THIRD_PARTY_LICENSES.html`, so locally
`cargo build --release -p heartsim-app && cargo packager` works once `cargo-packager` and `cargo-about` are installed. The macOS
`.dmg` is built with `hdiutil` instead of cargo-packager's AppleScript-based dmg, which needs a Finder session that CI runners lack.

`cargo xtask release-check [--tag vX.Y.Z] [--allow-unreleased] [--require-tools]` fails unless the workspace version, every
literal crate version, `Packager.toml`, the git tag and a non-empty `## [X.Y.Z]` CHANGELOG section agree, the version is
installer-safe (WiX needs a numeric-only pre-release such as `0.2.0-1`, not `-rc.1`), then runs `cargo deny check` and a dry
`cargo about generate`. **Release procedure:** bump `[workspace.package].version` and `Packager.toml`, rename the CHANGELOG
`[Unreleased]` heading to `[X.Y.Z] — date`, run `cargo xtask release-check`, tag `vX.Y.Z`, push the tag, review the draft.

### 9.2 Secrets (repository → Settings → Secrets and variables → Actions)
Every signing step is skipped, and the build still succeeds, when its secrets are absent (unsigned MSI/NSIS; ad-hoc-signed,
un-notarised dmg that Gatekeeper warns about). Nothing is required to build.

| secret | used by | content |
|---|---|---|
| `WINDOWS_CERT_PFX_BASE64` | windows | Authenticode code-signing certificate + key as `.pfx`, base64 (`base64 -w0 cert.pfx`) |
| `WINDOWS_CERT_PASSWORD` | windows | password of that `.pfx` |
| `APPLE_CERT_P12_BASE64` | macos | "Developer ID Application" certificate + key exported as `.p12`, base64 |
| `APPLE_CERT_PASSWORD` | macos | password of that `.p12` |
| `APPLE_API_KEY_P8_BASE64` | macos | App Store Connect API key (`AuthKey_XXXX.p8`), base64 — role *Developer* is enough for notarisation |
| `APPLE_API_KEY_ID` | macos | the key's 10-character ID |
| `APPLE_API_ISSUER_ID` | macos | the issuer UUID shown on the App Store Connect *Keys* page |

macOS signing runs from `rcodesign` 0.29.0 (apple-codesign; downloaded and checked against a pinned SHA-256) rather than Xcode's
`codesign`, so no keychain is created on the runner: `rcodesign sign --code-signature-flags runtime` with
`packaging/macos/entitlements.plist` (deliberately empty — no JIT, no plug-ins, no microphone), then `notary-submit --wait --staple`
for the `.app` and again for the `.dmg`. Apple Silicon requires a valid signature to run at all, hence the ad-hoc fallback.
Windows signing uses `signtool` from the runner's Windows SDK (`packaging/windows/sign.ps1`, DigiCert timestamp unless
`WINDOWS_TIMESTAMP_URL` is set) on `heartsim.exe` before packaging and on both installers after. An EV or Azure Trusted Signing
certificate is better against SmartScreen; swap the body of `sign.ps1` and keep the call sites.

### 9.3 Known limits
* **`.heartsim.ron` association.** Windows and macOS match the last dot-separated extension only, so the compound suffix cannot be
  registered reliably there (registering plain `.ron` would hijack every RON file). It works on Linux through the shared-mime-info
  glob `*.heartsim.ron`. Moving to a unique single extension (e.g. `.hsim`) would make it work everywhere; it needs a
  `core::Scenario` file-format decision, so it is left open.
* **`--version` on Windows.** The release exe uses the GUI subsystem (no console window), so `heartsim --version` prints only when
  stdout is redirected (`heartsim --version > v.txt`); the smoke test does exactly that.
* **Clean-container AppImage test.** Only `libgl1` and `libvulkan1` are installed in the container, mirroring the AppImage
  excludelist; anything else the binary links at load time must come from the bundle. Window creation (dlopen of
  libxkbcommon / wayland / X11) is not exercised on CI — there is no display.
* The macOS dmg has no custom background/window layout.

## 10. Risks and mitigations
| risk | mitigation |
|---|---|
| Open anatomy lacks atria/conduction detail | Strocchi cohort has four chambers + fibres; conduction system generated procedurally (fractal tree + landmarks); fall back to procedural idealised heart for M1. |
| Tier 1 morphology not "textbook" enough | Per-region APD maps + calibration against PTB-XL NORM medians; per-wave gains as last resort. |
| egui/wgpu version churn | All versions pinned in workspace; upgrade in one PR with the coupling table from `research/rust-stack.md`. |
| CI machines without GPU | lavapipe for render tests; sim/ecg tests are CPU only. |
| Scope creep (EP lab, devices) | P2 behind feature flags; P0/P1 gate releases. |
