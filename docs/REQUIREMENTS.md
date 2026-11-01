# heartsim — product requirements

Status: v0.1 (2026-10-31). Sources: user brief; the private reference-product analysis notes (not published); `docs/research/reference-apps.md`
§6 (feature superset); `docs/research/electrophysiology-and-ecg-models.md`; `docs/research/user-deep-research-takeaways.md`.

## 1. Vision
A free, open-source, offline-first desktop (and later browser) application that teaches electrocardiography by letting the learner
*see* electrical activation spread over a 3-D heart while the 12-lead ECG it produces draws in real time, and lets them change the
physiology (rates, blocks, pathways, foci, ischaemia, electrodes, pacing, drugs) and immediately see the consequences in both views.
Reference bar: Epicardio Simulation (3-D + 12-lead + EP + pacing), cardiacrhythmsim.com (rhythm library + quiz), ECG axis trainer
(hexaxial drill), ECGSIM (scientific forward model).

## 2. Users and jobs
| user | job to be done |
|---|---|
| Medical / nursing / paramedic student | Understand P-QRS-T genesis, axis, blocks, arrhythmias; drill rhythm recognition. |
| Cardiology / EP fellow | Explore conduction physiology, re-entry, pacing modes, EP study logic. |
| Educator | Build scenarios and quizzes, present on a projector, export strips. |
| Developer / researcher (secondary) | Reuse the engine from the CLI, validate against openCARP/ECGSIM, add pathology packs. |

## 3. Functional requirements

IDs: **F-xx** functional, **N-xx** non-functional. Priority P0 = first release (M1–M4), P1 = v1.0, P2 = later.

### 3.1 Simulation engine
| id | requirement | prio |
|---|---|---|
| F-01 | Fixed-step (1 ms) deterministic engine decoupled from frame rate; speed 0.05×–2× (slow motion for teaching), pause, step, scrub last ≥ 30 s. | P0 |
| F-02 | Conduction-system model: SA node (rate, respiration modulation, on/off), atria, AV node (delay, decremental conduction, ERP, retrograde, fast/slow pathways), His, RBB, LBB, LAF, LPF, Purkinje, ventricles; escape pacemakers (junctional, ventricular) with rates. | P0 |
| F-03 | Tier 1 spatial activation on a cell cloud (≥ 20 k cells) with anisotropic conduction (fibre direction), per-region APD (epi/endo gradient), CV, restitution and refractoriness; AP templates per region; produces V_m per cell per step. | P0 |
| F-04 | Tier 0 template ECG (timing engine + Gaussian/McSharry synthesis) for rhythm-only mode and WASM fallback. | P0 |
| F-05 | Tier 2 reaction–diffusion (Aliev–Panfilov default; Mitchell–Schaeffer, Fenton–Karma optional) on a masked voxel grid for re-entry: VT, VF, torsades, AF wavelets, flutter; initialised from Tier 1 state; CPU (rayon) first, wgpu compute optional. | P1 |
| F-06 | Interventions on a timeline: parameter edits, block/unblock region, stimulate (site, strength, duration), pacing trains S1×n/S2/S3, ablation (radius), accessory pathway (site × direction), coronary stenosis (branch, severity), defibrillation (energy, sync), drugs (adenosine, verapamil/diltiazem, digoxin, flecainide, amiodarone, atropine, adrenaline), electrode moves. | P0 (subset) / P1 |
| F-07 | Pathology library ≥ 30 presets as data files: sinus brady/tachy/arrhythmia, sinus arrest + escapes, AVB 1°/2° I/2° II/3°, LBBB, RBBB, LAFB, LPFB, bifascicular, PAC/PVC (uni/multifocal, bigeminy), AF, typical flutter, atypical flutter, AVNRT, AVRT/WPW (≥ 3 pathway sites), VT monomorphic, VF, asystole, idioventricular, junctional, STEMI by territory (ant/inf/lat/post), ischaemia/ST depression, LQT, torsades, Brugada, hyper/hypokalaemia, LVH/RVH, pericarditis, paced rhythms (AAI/VVI/DDD/BiV). | P0 (first 25) / P1 (rest) |
| F-08 | Every preset is a RON scenario (parameters + ablation set + optional AP templates + timeline) that users can edit, save, share and import. | P0 |
| F-09 | Deterministic seed per scenario; identical output on every platform (tested). | P0 |

### 3.2 ECG
| id | requirement | prio |
|---|---|---|
| F-10 | 12-lead ECG computed from the 3-D source: electrode potentials from the equivalent-double-layer / dipole-sum of cell V_m (precomputed weight matrix, 1/r² homogeneous torso first; lead-field later), Wilson central terminal, Einthoven and Goldberger leads derived, not hard-coded. | P0 |
| F-11 | Electrodes placed on a torso mesh; drag-and-drop; presets; misplacement and limb-reversal exercises with detection feedback; V3R/V4R/V7–V9 optional. | P1 |
| F-12 | Strip display: single lead with lead selector; 12-lead pages 3×4, 6×2, 12×1 (+ rhythm strip); sweep 12.5/25/50/100/200 mm/s; gain 5/10/20 mm/mV; 1 mm/5 mm paper grid; 1 mV calibration pulse; monitor-style (sweep bar) and scrolling modes; ≥ 60 fps with min/max decimation; HiDPI-correct. | P0 |
| F-13 | Pause and scrub with the 3-D view, heart vector and cursor synchronised; calipers (ms / bpm); interval read-outs HR, PR, QRS, QT, QTc (Bazett, Fridericia), frontal axis + classification. | P0 |
| F-14 | Electrical heart vector (3-D arrow + loop) and Cabrera/hexaxial wheel with projection guide lines; axis drill quiz (random axis with guard bands). | P0 |
| F-15 | Filters (diagnostic 0.05–150 Hz, monitor 0.5–40 Hz), baseline wander, mains hum, muscle artefact, respiration toggles. | P1 |
| F-16 | Intracardiac electrograms (HRA, His 1–3, CS 1–10, RVA, ablation tip) as bipolar differences of local potentials; AH/HV/CL measurements. | P2 |
| F-17 | Export: PNG snapshot, 12-lead PDF on paper layout, CSV/WFDB of signals, scenario RON. | P1 |

### 3.3 3-D visualisation
| id | requirement | prio |
|---|---|---|
| F-20 | Four-chamber heart (open licence) with conduction system, coronary arteries, great vessels; torso with electrodes; optional ribs/lungs. Layer toggles, transparency, cut-away/clip plane, labels. | P0 (heart, torso) / P1 (rest) |
| F-21 | Colour maps: V_m (adjustable window and ramp), activation isochrones, APD/repolarisation map, wavefront isosurface/outline, fibre glyphs, ischaemic/scar overlay, conduction-system highlight. | P0 (V_m, isochrones) / P1 |
| F-22 | Orbit camera; presets AP, PA, LAO, RAO, cranial, caudal, left lateral; LAO−90…RAO+90 slider; auto-rotate; zoom; screenshots. | P0 |
| F-23 | Picking on the heart/torso: stimulate, place block/ablation, place electrode, query cell parameters. | P0 (stimulate) / P1 |
| F-24 | Device models: pacing wires (RA appendage, RV apex/septum, LV lateral/posterior), EP catheters (HRA/HIS/CS/RVA/ABL), 3830-style CSP lead — visual placement only. | P2 |
| F-25 | Cosmetic contraction animation driven by activation (template-based vertex displacement); LVEF read-out. | P2 |

### 3.4 Devices and procedures
| id | requirement | prio |
|---|---|---|
| F-30 | Temporary pacemaker UI: rate, A/V output, A/V sensitivity, modes VOO/VVI/AOO/AAI/DOO/DDD/DDI, AV interval, upper rate, PVARP, capture/sense indicators, loss-of-capture & undersensing scenarios. | P1 |
| F-31 | Implanted pacing presets: single, dual, BiV/CRT with LV lead site choice. | P1 |
| F-32 | DC cardioversion/defibrillation: charge, sync-on-R, energy, outcome probability per rhythm, animation and sound. | P1 |
| F-33 | EP lab: catheter placement, programmed stimulation, EGMs, entrainment, ERP determination; ablation (AVN, PVI, CTI line, AP). | P2 |

### 3.5 Education
| id | requirement | prio |
|---|---|---|
| F-40 | Rhythm-recognition quiz with scoring and explanations; axis drill; interval-measurement drill. | P0 |
| F-41 | Guided tutorials: markdown pages with actions that drive the simulator (load scenario, set parameter, focus camera, highlight tool); progress tracking. | P1 |
| F-42 | Scripted patient cases: narrative, timed events, expected diagnosis/treatment, MCQ, feedback, exportable score. | P1 |
| F-43 | Real-ECG comparison panel with a bundled CC BY subset of PhysioNet 12-lead records (PTB-XL). | P2 |
| F-44 | Localisation (EN first; IT, DE, FR, ES, ZH next) via Fluent; accessibility: keyboard navigation, high-contrast theme, tooltips, reduced motion. | P1 |

### 3.6 Validation and research bridge
| id | requirement | prio |
|---|---|---|
| F-50 | CLI: run a scenario headless, export signals, benchmark; JSON/CSV output for CI. | P0 |
| F-51 | Golden-ECG regression tests per preset; interval checks against textbook ranges; PTB-XL statistical comparison of lead morphologies for NORM/LBBB/RBBB/MI. | P0 (golden) / P1 |
| F-52 | Export geometry + fibres + parameters + stimuli to openCARP/MonoAlg3D inputs and import their V_m/activation results (VTK/VTU) for side-by-side validation. Niederer-benchmark-style activation comparison on a slab. | P2 |

## 4. Non-functional requirements
| id | requirement |
|---|---|
| N-01 | Platforms: Windows 10+ (x64), macOS 12+ (arm64 + x64 universal), Linux glibc ≥ 2.35 (x64; X11 and Wayland); WASM (WebGPU, WebGL2 fallback with Tier 0/1 only) as stretch. |
| N-02 | Performance: 60 fps UI with Tier 1 on a 2019 laptop iGPU; Tier 1 step ≤ 0.5 ms for 50 k cells; Tier 2 64³ ≤ 8 ms/frame on 4 cores; cold start < 3 s; binary < 60 MB; RAM < 1 GB. |
| N-03 | Determinism and reproducibility: fixed-step, seeded, no wall-clock in engines; scenario files record engine version. |
| N-04 | Licensing: code MIT OR Apache-2.0; assets CC BY/CC0/OFL only, listed in `ASSETS-LICENSES.md`; `cargo deny` enforced. No proprietary Epicardio material. |
| N-05 | Offline-first; no telemetry; no accounts. Optional update check off by default. |
| N-06 | Accessibility: AccessKit via egui; keyboard operable; colour maps have colour-blind-safe alternatives. |
| N-07 | Quality gates: fmt, clippy pedantic, tests on three OSes, MSRV check, wasm check, cargo-deny, golden snapshots; coverage of `sim`/`ecg` ≥ 80 %. |
| N-08 | Packaging: signed installers (MSI/NSIS, .dmg notarised, AppImage/.deb) via cargo-packager; reproducible release workflow on tags. |
| N-09 | Medical disclaimer in About and first run: educational use only, not for diagnosis. |

## 5. Out of scope (v1)
True electromechanics, haemodynamics, patient-specific imaging import, bedside monitor emulation (SpO2/NIBP), multi-user/campus
licensing, learned/neural ECG corrections, mobile touch-first layouts (tablet works via desktop layout).

## 6. Glossary
APD action potential duration · CV conduction velocity · ERP effective refractory period · DI diastolic interval · EDL equivalent
double layer · WCT Wilson central terminal · PES programmed electrical stimulation · PVI pulmonary vein isolation · CSP
conduction-system pacing · UVC universal ventricular coordinates · LDRB Laplace–Dirichlet rule-based fibre method.
