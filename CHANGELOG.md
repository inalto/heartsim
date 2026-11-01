# Changelog

All notable changes to heartsim are documented here. Format: Keep a Changelog; versioning: SemVer.

## [Unreleased] — M1 (2026-10-31)
### Added
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
