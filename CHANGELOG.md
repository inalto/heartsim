# Changelog

All notable changes to heartsim are documented here. Format: Keep a Changelog; versioning: SemVer.

## [Unreleased]

### Changed
- The frontal QRS-axis panel is redesigned after a clinical teaching card: the hexaxial circle shows the four axis classes
  as soft-coloured sectors, and the one under the pointer brightens and shows its name and range (normal −30° to +90°, left deviation −30° to −90° at the upper
  right, right deviation +90° to +180° at the lower left, extreme or indeterminate −90° to ±180° at the upper left; the
  boundaries come from `AxisClass`), degree ticks every 30° in pills, faint dotted inner rings, the limb leads on the ring
  and a thick arrow with a soft drop shadow. A *Measured axis* card shows the angle on a badge in the class colour, an
  *Interpretation* card names the class and its range, lists the net QRS polarity of each limb lead as a signed glyph and
  a word, and draws a small torso schematic with the limb leads at their angles from the electrode geometry; a legend row
  under the circle doubles as the colour key. On the Simulator and 12-Lead pages the cards stack under the circle; on the
  Axis Trainer and the Axis Quiz they sit beside it (the trainer's separate polarity panel is folded into the cards).
- Dragging the axis: over the trainer's circle the pointer becomes a hand (closed while dragging), the arrow brightens
  and thickens, and a tooltip shows the angle; the read-only circles of the Simulator and 12-Lead pages do none of that.
- One palette for the whole interface, in a light and a dark variant (`heartsim_ui::theme`), with card, pill, badge and
  result-box helpers (`heartsim_ui::cards`): the rhythm bar's controls sit in soft groups, the measurement read-outs and
  the Heart Vector values are cards, the Settings and About windows use cards, and the quizzes, drills, tutorials and
  cases show their results in green or rose boxes with a check or an exclamation mark. The high-contrast setting keeps
  its black and white; the ECG paper, strip and 3-D colour maps keep their clinical colours.
- The screenshot tour takes `HEARTSIM_SCREENSHOT_THEME=light|dark` to shoot in one theme.

## [0.7.0] — 2026-11-07
Sixth tagged release, the follow-up to the first look at 0.6.0: the 3-D view opens on an oblique that shows the heart as a
heart, the tab row gives way to a one-line menu bar with the version in view, and the desktop app tells you when a newer
release is on GitHub.

### Added
- Update check: at start the desktop app asks the GitHub releases API once whether a newer release exists and, if so, shows
  a notice under the menu bar and in About with a link to the release page. It sends only the app version, downloads nothing,
  and the new Settings switch *Check for a newer release at start* turns it off (no request at all). Not in the web build.

### Changed
- The 3-D view opens as a right anterior oblique (45°, slightly from above) instead of straight-on AP: since 0.6.0 the heart's long
  axis points left, caudal and anterior as measured, and the AP view looked down that axis, showing the left ventricle as a round
  outline in front of the atria. The oblique puts the long axis in the screen plane with the apex at the lower right.
- The row of eleven tabs is replaced by a one-line menu bar: the program name and version, a **Menu** with every page (the open
  one marked, each with its number-key shortcut) followed by Export / Import, Settings and About, and the name of the open page.
  On a 1280-pixel window the old row clipped Settings and About off the right edge. The version also appears in the window
  title and at the top of the Help page.

## [0.6.0] — 2026-11-07
Fifth tagged release: the wave types that were missing from the strip (flutter, AF, VF, torsades, polymorphic VT, infarct
evolution) with 14 scenarios, the user-interface gaps (first-run disclaimer and About, ECG display filters and artefacts, the
Interval Drill, quiz explanations, the monitor beep), the anatomy pack re-posed on measured values so the R wave progresses
across the chest leads, and the new *Known limitations and open questions* page in the guide with the project wiki.

### Known limitations
- Fascicular blocks shift the frontal axis the right way but not as far as in patients (sinus ≈ +24°, LAFB ≈ +17°, LPFB ≈ +35°; brief B05
  asks for ≤ −45° and ≥ +90°). `fascicular_block_axes` stays ignored; its doc comment records what was tried and why rS in V1 and these
  axes exclude each other in this model.
- The guide's new page [Known limitations and open questions](docs/book/src/known-issues.md) lists what no person has run on real
  hardware yet and the physiology numbers that still need a clinician's check.

### Added
- The guide page *Known limitations and open questions*, a rewritten README, the project wiki (Home, Features, Releases, Known
  issues, Contributing) and a 17-step screenshot tour (`HEARTSIM_SCREENSHOT_DIR`) that now covers every tab, the 6 × 2 and
  clinical 12-lead layouts, flutter, VF, torsades and an anterior STEMI.
- New wave types on the Tier-0 strip, drawn from the timing engine's own events so the strip and the 3-D heart agree: atrial
  flutter (sawtooth at about 300/min with 2:1, 4:1 and variable AV block that follow from the AV-nodal recovery curve), atrial
  fibrillation with f waves, pre-excited AF, coarse and fine ventricular fibrillation, torsades de pointes (twisting axis, started by a
  pause-dependent R-on-T beat on a long QT, ending by itself) and polymorphic VT, and the evolution of an infarct (pathological Q
  waves and T-wave inversion in the leads of its territory). New timing sections `atrial_driver` and `ventricular_driver`, event
  origin `Origin::Circuit`, and the lesion kind `"<branch> evolving"` next to `"<branch> infarct"`. Old files load unchanged, and the
  golden strips of the existing scenarios are unchanged.
- 14 new scenarios with explanations, differentials, references and golden strips: `flutter_2to1`, `flutter_4to1`,
  `flutter_variable`, `atrial_fibrillation`, `af_preexcited`, `vf_coarse`, `vf_fine`, `torsades`, `polymorphic_vt`,
  `mi_anterior_evolved`, `mi_inferior_evolved`, `mi_lateral_evolved`, `mi_old_anterior`, `mi_evolution_stages`. They enter the rhythm
  quiz with the rest of the library.
- The analyser names these rhythms from the waveform alone (`Analysis::rhythm`, `atrial_rate_bpm`, `av_ratio`,
  `dominant_frequency_hz`), and an `expected` block can require a name (`rhythm`) and leave out the rate for a rhythm with no
  countable beats. The thresholds separate what this simulator draws and are not validated on recorded ECGs.
- A shock ends these rhythms through the existing therapy path. Sync mode does not fire in VF, polymorphic VT or torsades, flutter
  needs less energy than AF (new class `RhythmClass::AtrialFlutter`), and polymorphic VT and torsades have their own class.
- Tutorial pages using the new scenarios in `06_atrial`, `08_wpw`, `09_ventricular` and `10_ischaemia`.
- QRS-synchronous monitor beep (cargo feature `audio` of `heartsim-app`, desktop only): a 1 kHz tone synthesised in code and played
  through `kira` at each ventricular activation, with a switch and a volume slider in Settings (off by default; at most one beep per
  180 ms of simulated time; silent without an audio device; never in the browser build).
- First-run medical disclaimer and About (requirement N-09): a modal dialog with the guide's disclaimer must be acknowledged once
  (remembered through the settings, which also work in the browser), and an *About* window shows the same text with the version.
- ECG display filters and artefacts (requirement F-15): diagnostic 0.05–150 Hz and monitor 0.5–40 Hz bandwidths (Butterworth
  cascades, `heartsim_ecg::filter`), and seeded baseline wander, respiration, 50/60 Hz mains and muscle tremor
  (`heartsim_ecg::artefact`), switched on above the strip and the 12-lead page. They are a display stage only: the engine, the
  measurements and the default exports stay unfiltered. An export "as displayed" (and the 12-lead *Save PNG*) states the filter and
  artefacts in the PDF, PNGs, CSV (`# trace:` line), WFDB header and bundle README.
- Quiz gaps of brief B8: the axis drill never asks a north-west axis or one within 7° of a class boundary
  (`heartsim_core::axis::draw_drill_axis`, property-tested over 10 000 draws), shows its streak and explains every answer
  ("lead I positive and lead II positive → normal axis"); a new **Interval Drill** tab (measure PR, QRS or QT with calipers,
  tolerance grading); and the rhythm quiz shows the teaching notes, the "often confused with" list and what the chosen rhythm is
  after every answer.

### Changed
- The default anatomy pack's heart now sits in the thorax as measured: the LV long axis at the CMR population mean (frontal 36°,
  transverse 46°; Engblom 2005, Kayyali 2025) with the septum facing right and anterior (Narumi 2022), so the LV free wall faces left
  and posterior and the "anterior" wall faces cranially. The torso is narrower and the heart further left, the LAF and LPF terminals
  are at Durrer's paraseptal breakthrough sites with 25 mm junction patches, and `ForwardOptions::gain_mv` is recalibrated from 5.0
  to 4.4. The R wave now grows from V1 to V5 (brief B05), and left posterior fascicular block now moves the axis to the right of
  sinus. The coronary territories follow: the LAD now owns the anterior apical part of the RV free wall (`segments.bin` value 18,
  pack format version 2), which is what keeps ST elevation in V2–V4 for an LAD occlusion, and not in an RCA one.
- The atria, caval veins, coronary sinus and pulmonary veins follow the thorax too: the veins now enter the left atrium at or above the
  mid-ventricular level instead of beside the apex, the SA node sits at the SVC–RA junction, the P axis is about +33°, and the AV node
  and His bundle stay at the septal crest. A new landmark, `la_pv_antrum`, marks the posterior left atrium between the veins.
- The persistent-AF presets (`af_persistent`, `ep_af_pvi`) are re-tuned for the new atria: the burst starts at `la_pv_antrum` at 125 ms,
  the atrial refractory period varies from patch to patch, and sensing (`sense_window_ms` 400, `sense_gap_ms` 135) and AV concealment
  (0.85) are set for conduction at 0.3 ×. The typical-flutter preset's circuit moves with the right atrium.
- Tab shortcuts: `0` opens the tenth tab (EP Lab); the new Interval Drill tab sits before Tutorials, so Tutorials is now `9`.

## [0.5.0] — 2026-11-06
Fourth tagged release: the education content (55 scenarios, ten tutorials, three patient cases), the cardiac EP lab with
catheters, electrograms, programmed stimulation and ablation, and the browser version published on GitHub Pages (briefs B18, B22).

### Added
- The browser version is published on GitHub Pages under `/app/`, next to the guide: `docs.yml` now builds the guide and, on
  `master`, the web app, and deploys both. The guide's introduction links to it.
- Education content (brief B18): the rhythm library grows from 33 to 55 scenarios (ventricular tachycardia and its runs, couplets,
  R-on-T and parasystole, blocked PACs and atrial bigeminy, atrial tachycardia, sick sinus, rate-related aberrancy, right-sided WPW,
  accelerated junctional rhythm, Mobitz II 4:3, axis deviations, and anterior, inferior, lateral, posterior, subendocardial,
  pericarditis and evolving ischaemia patterns), each with an explanation, a "confused with" list and cited references; the tutorial list to ten (basics, normal rhythm, cardiac axis, conduction abnormalities, AV blocks and
  bradycardias, atrial arrhythmias, SVT, WPW, ventricular arrhythmias, ischaemia and infarction); and three scripted patient
  cases with eight multiple-choice questions each.
- Patient cases: `assets/cases/*.ron` (format in `heartsim_core::case`, documented in `docs/tutorials.md`), a case list under the
  tutorials and a minimal player in the reader panel (stage vignettes, simulator actions, one answer per question with its
  explanation, score), with English and Italian labels. The web archive carries `cases/`.
- `heartsim-cli run-tutorial` runs every page action of a tutorial headlessly and checks the content rules; `heartsim-cli
  validate-cases` does the same for cases. `validate-scenarios` now also requires references.
- `references` field in scenarios, tutorials and cases (scenarios may still keep a `Sources:` block in `notes`); the tutorial
  action `StartReentry("<preset>")` starts a re-entry preset from a page.
- `crates/heartsim-ecg/tests/ischaemia_scenarios.rs` pins which leads rise and fall in each ischaemia scenario (the statements of
  the tutorial); `crates/heartsim-cli/tests/content.rs` runs the scenario validator, every tutorial, and the cases.
- `typos` spell-check with `_typos.toml` (medical terms), run by `cargo xtask ci` when installed and in the CI workflow.
- Cardiac EP lab (brief B22, F-16, F-33): a new **EP Lab** tab with five procedures in `assets/scenarios/ep/` (baseline study, AVNRT and
  slow-pathway modification, left-lateral accessory pathway, typical flutter and CTI ablation, AF and pulmonary-vein isolation). See the
  user-guide page *The EP lab*.
- Catheters (`heartsim_geometry::catheter`): HRA, His, CS decapolar, RVA and ablation catheters as polylines with electrodes, placed on landmarks
  or along the coronary sinus and held inside chamber cavities computed from the cell cloud.
- Electrograms (`heartsim_ecg::egm`): unipolar and bipolar local potentials from the forward model restricted to nearby cells, displayed at
  100–200 mm/s, with A, H and V deflections labelled by source; AH, HV and cycle-length measurements and the activation sequence along a catheter.
- Programmed stimulation (`heartsim_sim::ep`) from any bipole: continuous, S1×n / S2 / S3, decremental, automatic ERP search with A2–H2 jump
  detection, entrainment with PPI and PPI − TCL. In Tier 1 the timing engine decides capture and only a capture starts the mesh front.
- RF ablation: lesions that grow with a time constant, with temperature and impedance read-outs (a teaching stub, not a thermal model). A lesion
  stops the cells and voxels under it and the nodal, bundle or accessory-pathway structures it touches, and it survives an engine reset.
- Tier-2 patches: `PatchTarget::Band` and `Initiation::PhaseWave { within }`, used to draw a macro-re-entrant circuit on the spherical atria.
  Typical flutter exists only as this drawn, pre-seeded circuit (cycle 235 ms), a procedure of the EP lab; it is not induced by pacing.
- `EikonalParams::cs_os_link_mm`: joins the coronary-sinus sleeve to the right atrium (off by default, so existing rhythms do not change).

### Changed
- Inert tissue from a scenario's Tier-2 patches no longer follows the heart back to Tier 1 as lesions; lesions made during the run still do.
- The tutorial format and page runner moved from `heartsim-app` to `heartsim_core::tutorial` (re-exported by the app) so the CLI
  reads the same files. Tutorial ids are numbered for the teaching order: `basics` is now `01_basics` and `cardiac_axis`
  `03_cardiac_axis` (saved progress of those two starts afresh).
- Tutorial `Sim` actions go through the same path as the therapy window (re-entry engine if one runs, timing engine, then the 3-D
  heart), so a coronary lesion set by a page also reaches the heart view.

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
