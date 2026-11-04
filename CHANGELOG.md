# Changelog

All notable changes to heartsim are documented here. Format: Keep a Changelog; versioning: SemVer.

## [Unreleased]

### Added
- Tabbed application: Simulator, 12-Lead ECG, Axis Trainer, Axis Quiz and Help.
- 12-lead page with three layouts (12 x 1, 6 x 2, clinical 3 x 4 plus rhythm strip) and a live hexaxial axis circle with per-lead QRS polarity.
- `heartsim_ecg::estimate_frontal_axis`: measures the QRS axis from leads I and aVF, and `GaussianSynth::qrs_axis_deg` gives the axis implied by the waves.
- Clinical strip and 12-lead pages (brief B07): sweep 12.5–200 mm/s, gain 5/10/20 mm/mV, screen calibration, grid toggle,
  monitor colours, scrolling and sweep-bar modes, Cabrera lead order, consecutive-segment time base on multi-column pages;
  60 s history with pause and drag-to-scrub (3-D view follows via a 10 Hz `SnapshotRing`); draggable calipers with
  Δt/bpm/ΔV read-out, snapping, shift-click to add, arrow-key nudge; P/R/T labels and PR/RR brackets from the B4 analyser;
  HR/PR/QRS/QT/QTc/axis read-out panel; PNG snapshot of the 12-lead page.
- Drugs, ischaemia and defibrillation (brief B14): `assets/drugs.ron` drug table (adenosine, verapamil, diltiazem,
  digoxin, flecainide, amiodarone, atropine, adrenaline, lidocaine, hyper-/hypokalaemia) with onset/hold/half-life
  kinetics (`heartsim_sim::pharma`); `Action::Stenosis` by coronary branch with the wavefront rule, injury current,
  infarct scar and presets (`heartsim_sim::ischaemia`); synchronised / unsynchronised shocks with charge time and
  per-rhythm success probability (`heartsim_sim::defib`); Tier-0 `SynthModulation`; Therapy window with drug tray,
  coronary-tree picker (territory highlight on the heart) and defibrillator.
- Exports and documentation (brief B20): new `heartsim-io` crate shared by the app and the CLI — 12-lead PDF (3 x 4 + lead II
  rhythm strip, A4/Letter landscape, patient-free header), 12-lead page and rhythm-strip PNGs, CSV, WFDB (format 16, 500 Hz,
  1000 units/mV), scenario RON and a deterministic share-bundle zip; Export / Import window in the app (drag-and-drop import
  of CSV, WFDB or bundles into the strip and 12-lead views for review with the calipers, `.ron` scenario files, window screenshot);
  `heartsim-cli run <scenario> --seconds … --out …` writing byte-identical files; mdBook user and developer guide in
  `docs/book` published to GitHub Pages by `.github/workflows/docs.yml`.

### Fixed
- Tier-1 eikonal: a front reaching absolutely refractory tissue is blocked instead of being slowed until the tissue
  recovers (that let it re-excite the heart where short-APD ischaemic cells border slow-conducting ones).

## [0.2.0] — 2026-11-01
First tagged release: M0 foundation plus briefs B01 (timing engine), B02 (anatomy pipeline) and B12 (packaging).

### Added
- Cargo workspace (`core`, `geometry`, `sim`, `ecg`, `render`, `ui`, `app`, `cli`, `xtask`) with pinned toolchain, lints,
  `cargo-deny` policy, CI workflow (Linux/Windows/macOS tests, MSRV, wasm32 check).
- `heartsim-core`: `Millis`/`SimClock`, `Region`, 10-electrode → 12-lead derivation, frontal axis maths and classification,
  per-region `Parameters`, versioned RON `Scenario` with intervention `Action`s.
- `heartsim-geometry`: `TriMesh`, icosphere, `CellCloud` with 8-NN graph (distance + fibre alignment).
- `heartsim-sim`: `Engine` trait, AP templates, `TimingEngine` (SA node with respiration, AV node recovery curve, Mobitz II,
  complete block, junctional/ventricular escape) with rhythm tests.
- `heartsim-ecg`: lead vectors, event-driven Tier-0 12-lead synthesiser, ring buffers, min/max decimation, IIR filters, noise.
- `heartsim-render`: offscreen wgpu scene (colour + depth) with per-vertex scalar LUT colouring and blit into egui.
- `heartsim-ui`: paper-grid ECG strip painter, 3-D viewport widget (egui-wgpu callback).
- `heartsim-app`: eframe desktop app with live strip, 12-lead page, parameter toolbar, viewport.
- `heartsim-cli`: `synth` (CSV export) and `default-scenario`.
- Documentation: requirements, architecture, roadmap, 23 implementation briefs, research reports, Epicardio analysis.
- Packaging and release CI (brief B12): `Packager.toml` (cargo-packager) with app icon (`assets/icon.svg` → PNG/ICO/ICNS via
  `cargo xtask icons`), `.desktop` entry, `.heartsim.ron` association, macOS `Info.plist` extras and hardened-runtime
  entitlements; `.github/workflows/release.yml` (MSI + NSIS, notarised universal `.dmg`, AppImage + `.deb`, `SHA256SUMS`,
  cargo-about `THIRD_PARTY_LICENSES.html`, draft GitHub Release, per-artefact `--version` smoke tests); actionlint job in CI;
  `cargo xtask release-check | release-notes | licenses | prepare-packaging | version`; `heartsim --version`.
- `heartsim-sim` timing engine (brief B01): grouped `TimingParams` (`sinus`, `av`, `his_purkinje`, `escape`, `foci`,
  `accessory`, `pacer`) with documented units/ranges and RON round-trip; sinus pauses, SA exit block, sick sinus,
  overdrive suppression; ectopic foci (PAC/PVC, bigeminy/trigeminy, couplets/runs, parasystole); dual AV-nodal pathways with
  AVNRT loop, concealed conduction, adenosine (`Action::Drug`); accessory pathway with pre-excitation and orthodromic/
  antidromic AVRT; separate RBB/LBB/LAF/LPF edges with fixed and rate-dependent block; junctional rhythm with retrograde P;
  pacemaker channels (AOO/AAI/VOO/VVI/DOO/DDD/DDI with sensing, capture, blanking, PVARP, upper rate); `Event::origin` and
  `Event::chamber`; ladder-diagram `LadderSample` export; `TimingEngine::apply` for scenario actions; 18 acceptance tests in
  `crates/heartsim-sim/tests/rhythms.rs`.
- `heartsim-geometry` anatomy pipeline (brief B02): implicit-surface modelling with a marching-tetrahedra polygoniser
  (`implicit`), exact hash-grid k-NN (`spatial::HashGrid`, now also behind `CellCloud`), procedural idealised four-chamber
  heart (`procedural::idealised_heart`: truncated-ellipsoid ventricles with septum, spherical atria with septum and
  appendage, great-vessel stubs, valve rings; ~17.5 k cells on a 2.3 mm lattice with transmural depth, ±60° helix fibres,
  21 labelled regions, SA/AV node, His, bundle branches and fascicles as cell polylines, 22 landmarks, PMJ seed sets per
  fascicle, AHA 17 segments → coronary territories), procedural superellipsoid torso with the 10 standard + V3R/V4R/V7–V9
  electrode sites in the heart frame, vertex→cell hoods with inverse-distance weights, and the versioned, checksummed
  anatomy-pack format with `AnatomyPack::{procedural, save, load}` (`docs/anatomy-pack.md`).
- `assets/anatomy/default/`: the baked procedural pack (10 MB, loads in ≈ 60 ms) with its `LICENSE`.
- `cargo xtask bake-anatomy` (procedural path done; real-mesh path for the Strocchi/Rodero CC BY 4.0 cohort scaffolded
  with format detection and a stage plan) and `heartsim-cli anatomy-info <pack>`.
- Acceptance tests in `crates/heartsim-geometry/tests/anatomy_pack.rs`.

### Changed
- AV-node recovery time is now the H–A interval (last His output → atrial arrival), so Wenckebach emerges from the recovery
  curve; `erp_ms` is documented in those terms.
