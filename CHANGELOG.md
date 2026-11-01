# Changelog

All notable changes to heartsim are documented here. Format: Keep a Changelog; versioning: SemVer.

## [Unreleased] — M2 (2026-11-01)
### Added
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

## [Unreleased] — M1 (2026-10-31)
### Added
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
### Changed
- AV-node recovery time is now the H–A interval (last His output → atrial arrival), so Wenckebach emerges from the recovery
  curve; `erp_ms` is documented in those terms.

## [Unreleased] — M0 foundation (2026-10-31)
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
