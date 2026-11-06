# Changelog

All notable changes to heartsim are documented here. Format: Keep a Changelog; versioning: SemVer.

## [Unreleased]

### Added
- The browser version is published on GitHub Pages under `/app/`, next to the guide: `docs.yml` now builds the guide and, on
  `master`, the web app, and deploys both. The guide's introduction links to it.

## [0.4.0] — 2026-11-05
Third tagged release: the re-entry framework with a persistent-AF preset, localisation and accessibility settings, the
WebAssembly build and the validation bridge to scientific solvers (briefs B17, B19, B21, B23).

### Added
- WebAssembly build (brief B21): new `heartsim-web` crate builds the whole app for the browser with `trunk build --release`
  (WebGPU, falling back to WebGL2), with a loading screen, a canvas that fills the window and `localStorage` persistence
  (quiz score, tutorial progress). The wasm carries no anatomy: the page downloads `heartsim-assets.zip` (anatomy pack,
  rhythm library, tutorials; written by `cargo xtask web-assets`) and the app loads it from memory
  (`heartsim_app::AssetBundle`, `HeartSimApp::with_assets`, `AnatomyPack::load_with`).
- `heartsim-app` features: `native` (disk asset discovery, user data directory, plain-file save/load), `export` (the
  Export / Import window and `heartsim-io`) and `parallel` (rayon); all on by default. `heartsim-render` and the
  workspace entries for `heartsim-sim` / `heartsim-ecg` get a matching `parallel` switch. The web build drops `native`,
  `export` and `parallel`: no export or file buttons, no threads (results are identical, only slower).
- `cargo xtask web-assets` and `cargo xtask web-budget` (wasm + assets must stay under 25 MiB); `.github/workflows/web.yml`
  builds, checks the budget and runs a Playwright smoke test on pull requests. It uploads an artifact and does not publish.
- Test that no shader uses storage buffers, compute or `@interpolate(linear)` (WebGL2 fallback); `docs/web.md` with the
  browser test matrix.
- The web build also gets the re-entry presets (`scenarios/reentry/` and the anatomy pack from the downloaded archive; the frame
  budget reads `Date.now()` because `Instant` panics in a browser), the browser's language (`sys-locale` `js`) and the Settings
  choices in `localStorage`.
- Validation bridge to scientific solvers (brief B23; see `docs/validation-bridge.md`). `heartsim-cli export-opencarp` writes a scenario as an openCARP experiment:
  tetrahedral mesh, fibres, region tags, timing-engine stimuli, Aliev-Panfilov or TT06 parameters and a carputils script. `import-vtk` reads VTU, legacy VTK, PVD, IGB and
  activation `.dat` results onto the cell cloud by nearest point. `validation-report` compares them with a Tier-1 or Tier-2 run (activation RMS, per-lead ECG RMSE, QRS and
  T correlation; Markdown, PNG, JSON). `niederer` runs the benchmark slab on Tier 2 and reports its deviations. No solver has been run on these files yet: this is tested
  against the documented formats, Tier 2 is calibrated rather than TT06, and the stored reference is a secondary table, not the paper's supplement.
- Re-entry framework (brief B17): scenario files may carry a `reentry:` section (`heartsim_sim::tier2::ReentrySpec`) with tissue patches
  (APD / CV scaling by region, sphere, box or compartment, inert obstacles, APD mosaics), the initiation (S1–S2–S3 or burst pacing,
  cross-field stimulation, a pre-seeded rotating wave, or a `Tier2Snapshot` state file) and the tier-switch policy.
- `assets/scenarios/reentry/af_persistent.ron`: persistent atrial fibrillation (no P waves, f waves ≈ 0.15 mV in V1, irregularly irregular
  ventricular response at ≈ 100–130/min). Its atrial rate is ≈ 165/min, below clinical AF. Flutter, scar-related VT, torsades and VF
  presets are **not** included: no induction protocol produced a surviving rotor in the heart-shaped model.
- `SwitchingEngine`: runs Tier 1 until the scenario or an action (rapid pacing burst, S2/S3 extrastimuli, an unsynchronised shock) needs
  re-entry, continues the same simulation in Tier 2, and hands back to Tier 1 after 2 s without re-entry (a phase-singularity run of 0.6 s
  or an unexplained tissue activation at the AV-nodal input or lead tip; minimum dwell 3 s), keeping the beat timing and lesions.
- Tier 2 follows the timing engine's drug effects (`calibrate::retarget` re-solves the diffusivity, so short-APD tissue still conducts),
  applies an unsynchronised shock on the T wave of a normal beat as a field stimulus, and saves / restores its state
  (`Tier2Engine::snapshot`, `restore`).
- Pulmonary-vein isolation and cavotricuspid-isthmus ablation lines (`preset::pvi_lesions`, `preset::cti_lesions`).
- Simulator tab: a **Re-entry presets** list runs a preset on the switching engine; the strip, the 12-lead page and the 3-D heart show the
  forward-model ECG and the Tier-2 cells, and the therapy window, click-to-ablate and the timeline act on it. The status line shows the
  tier, the phase-singularity count and the achieved speed.
- User guide page *Re-entry presets*.
- Localisation with Project Fluent (brief B19): every user-visible string of the panels and tabs goes through `t!("key")`
  (`heartsim_ui::i18n`), with catalogues in `assets/locales/<language>/*.ftl` embedded at compile time. Italian is complete;
  German, French and Spanish are stubs that fall back to English. The language follows the operating system and can be
  changed in the new Settings window (the choice is remembered). Domain names (regions, coronary branches, lead sites,
  rhythm classes, limbs) are translated through `heartsim_ui::labels`.
- Settings window: language, high-contrast theme and reduced motion. Reduced motion stops the automatic rotation of the
  3-D heart and the defibrillator's shock flash.
- Keyboard shortcuts: Space pauses or resumes; ←/→ scrub while paused (Shift for 1 s steps); + / − change the simulation
  speed; 1–9 open a tab. They are listed in the Help tab and are off while a text box has the keyboard.
- Labelled transport controls (Run / Pause and the speed slider) so screen readers and AccessKit see their names.
- Tests (`crates/heartsim-ui/tests/i18n.rs`): every English key exists in Italian; stub languages define no unknown keys; the
  embedded catalogue list matches the files; every key used in code is defined in English; no hard-coded label text in the
  panels; switching language relabels the toolbar.

### Changed
- The timing engine no longer vetoes tissue-sensed atrial or ventricular activations with its own ERP, and classifies tissue-sensed
  ventricular activity as VT (cycle ≥ 250 ms) or VF (shorter), so a shock to a re-entry preset is scored as VT/VF rather than SVT.
- Tier-2 sensing: an activation within `sense_window_ms` (now 300 ms, was 150) of a timing event is that event's own conduction;
  `sense_gap_ms` (150 ms) is the new minimum spacing of fed-back activations.

### Not yet translated
- Tutorial pages and scenario texts (`assets/tutorials`, `assets/scenarios`), electrode presets and exercise texts, the
  descriptions of ischaemia presets and the detector's findings, the pacemaker preset codes, the Territory names, and the
  export preset descriptions. These remain English.

## [0.3.0] — 2026-11-04
Second tagged release: the Tier-1 and Tier-2 engines, the forward ECG, the clinical pages, the rhythm library, pacing,
therapy, tutorials and exports (briefs B03–B11, B13–B16, B20).

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

- Tier-1 anisotropic graph-eikonal activation engine (brief B03): wavefronts on the cell cloud driven by `TimingEngine` events,
  refractoriness, restitution, a fast endocardial layer per bundle, ablation; calendar queue with results identical on any thread count.
- ECG analysis (brief B04): Pan-Tompkins QRS detection, P/T boundaries, HR, PR, QRS, QT, QTc (Bazett, Fridericia), axes,
  regularity, wave marks and the heart-vector loop of the last beat.
- Tier-1 forward ECG (brief B05): precomputed electrode weight matrix from the cell cloud, single-electrode updates, per-wave gains.
- Multi-mesh renderer (brief B06): layers with opacity and contours, clip plane, instanced glyphs, CPU picking, PNG capture.
- Real heart in the 3-D view (briefs B08, B09): voltage, activation-time and APD maps with a colour-blind-safe scale and legend,
  anatomy panel, camera presets, click to stimulate, shift-click to ablate; rhythm picker, 3-D scrub, heart vector with
  vectorcardiogram planes, and a Rhythm Quiz tab.
- Electrode editor (brief B10): drag electrodes on the torso, placement presets (standard, Mason-Likar, posterior, right-sided,
  paediatric), misplacement exercises and a detector that explains them.
- Tier-0 scenario library (brief B11): 27 rhythms with teaching notes, sources, expected findings and golden ECG snapshots;
  `heartsim-cli validate-scenarios`.
- Pacing devices (brief B13): lead sites, capture thresholds, sensing, biventricular pacing, pacemaker-mediated tachycardia
  intervention, pacemaker panel and six new paced scenarios.
- Tutorial engine and scenario timeline editor (brief B15): reader panel with knowledge checks, self-study and tutor modes,
  saved progress, and two original tutorials (Basics, Cardiac axis).
- Tier-2 monodomain reaction-diffusion engine on masked voxels (brief B16): Aliev-Panfilov, Mitchell-Schaeffer and Fenton-Karma
  cell models, results independent of thread count, hand-over from Tier 1.

### Changed
- The default left-ventricular free wall has an endo-to-epi APD gradient and the septum a uniform APD, which makes the T wave upright.

### Fixed
- Tier 2 follows the timing engine's shock outcome: a synchronised shock waits for the R wave and a failed shock leaves the tissue alone.
- A heart that has not beaten yet is no longer classed as asystolic, which made an early shock fail.
- LAO and RAO camera presets were swapped; LAO now puts the camera at the patient's left.
- Tier-1 eikonal: a front reaching absolutely refractory tissue is blocked instead of being slowed until the tissue
  recovers (that let it re-excite the heart where short-APD ischaemic cells border slow-conducting ones).

### Known limitations
- Tier 1 takes about 60–85 ms per beat against a 15 ms budget, and Tier 2 at 64³ takes 46–62 ms per frame against 8 ms, so Tier 2 is
  not real time on the dev machine.
- Tier 2 has no GPU path yet and does not apply spatial `Action::SetRegion`.
- Two forward-model acceptance tests stay ignored (R-wave progression and fascicular axes) until the anatomy pack geometry is fixed.

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
