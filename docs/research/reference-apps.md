# Reference applications: feature inventory and UX analysis

Research for the cross-platform 3D heart / ECG simulator (Rust). Four reference
applications were examined: an open-source ECG axis trainer, the commercial
Epicardio Simulation platform (including the Google Play "EHS2" Heart Simulator
app and the Windows desktop build present in this repository), the web-based
Cardiac Rhythm Simulator, and the Unity WebGL build of Epicardio.

Date of research: 2026-10-31.

Confidence legend used throughout:

- **verified** – read directly from source code, a downloaded file, an official
  page, or an official PDF.
- **inferred** – deduced from identifiers in binaries/JS bundles or from
  indirect descriptions; not confirmed by documentation.
- **unverified** – claimed by a third party or marketing copy only, or could not
  be reproduced.

Nothing in this document is copied code from the proprietary products; the only
code-level detail reproduced is from the MIT-licensed axis trainer.

---

## Table of contents

1. ECG Axis Trainer (david-shrk/ecgaxistrainer)
2. Epicardio Simulation / Epicardio Heart Simulator (EHS2)
3. Cardiac Rhythm Simulator (cardiacrhythmsim.com)
4. Epicardio Unity WebGL build (epicardiowebgl.s3.amazonaws.com)
5. Feature comparison table
6. Feature superset to implement (P0 / P1 / P2)
7. Exact math and algorithms from the open-source axis trainer
8. URLs consulted

---

## 1. ECG Axis Trainer (david-shrk/ecgaxistrainer)

### 1.1 Purpose

A single-page web app for learning and drilling QRS-axis ("Lagetyp")
determination from the six frontal-plane limb leads. The user rotates a mean
QRS vector on a Cabrera circle and watches the six limb leads redraw, or, in
quiz mode, is shown a random six-lead strip and must name the axis class.
(verified – README: "A small web-app for training of ecg axis deviation.
Available in English, Spanish and German.")

Author: David Schaack, cardiologist / invasive EP fellow (Germany), M.Sc.
medical informatics student (verified from the About text in languages.js).
Spanish translation by Alvaro Pemartin.

### 1.2 Platform and technology (verified)

- Static site on GitHub Pages: https://david-shrk.github.io/ecgaxistrainer/
- Vanilla JavaScript ES modules, no framework, no build step.
- Two stacked HTML `<canvas>` elements for the ECG (grid canvas + trace canvas).
- Cabrera circle drawn with CSS `conic-gradient` sectors, the axis arrow is a
  rotated `<div>`; mouse drag, touch drag and mouse-wheel input.
- Google Analytics tag (G-1RMYQC13N1), Google Fonts (Roboto 300).
- Repository: 33 commits on `main`, 23 stars, 4 forks, 1 open issue at time of
  fetch.
- Files: `index.html` (7.6 KB), `resources/js/{main.js 20 KB, ecgDrawing.js
  11 KB, ecgCalculations.js 3 KB, circleCalculations.js 3.3 KB, languages.js
  4.9 KB}`, `resources/css/{style, cabrera, colors, handle, quiz}.css`,
  `resources/images/{cabrera.svg, cabreraEn.svg, arrow.svg, rotationarrows.svg,
  mail/twitter/github.svg}`.

### 1.3 Feature list (verified from source)

- Three top-level sections switched by nav buttons: **Axis/Cabrera**, **Quiz**,
  **About/Info**.
- Language switch En / De / Es; auto-detected from `navigator.language`
  (`de`/`de-DE` -> German, `es` -> Spanish, else English).
- Two axis-nomenclature systems, tied to the language:
  - German "Lagetyp" system (7 classes): Linkstyp, Indifferenztyp, Steiltyp,
    Rechtstyp, Überdrehter Rechtstyp, Überdrehter Linkstyp, Northwest-Typ.
  - International system (4 classes): Normal QRS axis, Left axis deviation,
    Right axis deviation, Northwest (extreme) axis.
- Interactive Cabrera circle: drag the arrow handle (mouse or touch) or scroll
  the mouse wheel (`angle -= deltaY / 25` degrees, wrapped to 0..360) to set the
  axis continuously; the sector the axis is in is highlighted with
  `filter: saturate(5)`; label shows class name and the angle to one decimal.
- Live six-lead strip (I, II, III, aVR, aVL, aVF) redrawn on every change,
  with 1 mm / 5 mm ECG grid, lead labels, one PQRST complex per "second".
- Responsive layout: if the ECG container is narrower than 300 px the six
  leads are drawn as two columns of three ("side by side"); canvas is scaled by
  `devicePixelRatio` (line width and font scale with DPR).
- Quiz mode: a random axis is generated, the strip is drawn, the user clicks
  one of the class buttons; counters "Correct" / "Wrong" with a CSS animation
  on each answer; a correct answer immediately generates a new axis, a wrong
  answer keeps the current ECG so the user can retry.
- The random generator excludes Northwest axes and a guard band of roughly
  +/-5 deg around every class boundary so that ambiguous strips are never asked
  (see section 7.6).
- About page: author info, sources (LITFL ECG-axis article; the Cabrera-circle
  colours are taken from a Wikimedia Commons GFDL graphic by Sylvère Störmann),
  full MIT licence text.

What it does **not** do (verified by absence in source): no precordial leads,
no P/T-axis, no heart-rate or interval variation, no pathology, no
persistence of scores, no explanation of the correct answer, no 3D.

### 1.4 UI layout (verified)

```
+-------------------------------------------------------------+
| h1 title           [Axis] [Quiz] [Info]        [En][De][Es]  |
+------------------------------+------------------------------+
| LEFT PANE (one of):          | RIGHT PANE: ECG               |
|  * Cabrera circle (square,   |  gridCanvas (1 mm / 5 mm)     |
|    conic-gradient sectors,   |  ecgCanvas (6 leads stacked,  |
|    rotating arrow w/ handle, |  label at x=15 px, one        |
|    class label + angle)      |  complex every 5*factor px)   |
|  * Quiz: 6 (De) or 3 (En/Es) |  or 3+3 columns when narrow   |
|    answer buttons + counters |                               |
|  * About: text + MIT licence | (ECG hidden on About page)    |
+------------------------------+------------------------------+
```

The two panes are a CSS flex "split"; both container divs are forced square
(`height = width`) on resize.

### 1.5 Rhythms / pathologies

None. The only variable is the frontal-plane QRS axis (0..359 deg). P-wave and
T-wave are fixed positive arcs regardless of axis (verified).

### 1.6 How the ECG is produced (verified – see section 7 for formulas)

Pure projection model: each limb lead's QRS "amplitude" is the cosine of the
angle between the mean QRS vector and the lead axis. Lead axes used:
I = 0 deg, II = 60 deg, III = 120 deg, aVR = 210 deg, aVL = 330 deg (-30 deg),
aVF = 90 deg. The scalar amplitude in [-1, 1] is then turned into a stylised
PQRST complex by a piecewise-linear drawing routine (R up to 20 mm * amplitude,
S down to 6 mm * amplitude, with a 0.25 threshold that converts low
amplitudes into equiphasic RS complexes, and a mirrored rS complex for negative
amplitudes).

### 1.7 Licensing / openness

MIT License, Copyright (c) 2022 David Schaack (verified from LICENSE and
index.html). The Cabrera-circle colour scheme is credited to a GFDL image; the
SVG circle graphics in the repo are stated to be the author's own. Fully
reusable; formulas can be re-implemented freely.

---

## 2. Epicardio Simulation / Epicardio Heart Simulator (EHS2)

### 2.1 Company and product family (verified unless noted)

- Vendor: **Epicardio Ltd**, UK company no. 7585521, 13 Marlborough Road,
  Richmond TW10 6JT; a US registration in Minnesota is also listed (VPAT PDF).
- Founder / project lead: Vassilios Hurmusiadis (UKRI Gateway to Research
  entry, Innovate UK project 710066 "Real-time Simulation of Cardiac
  Electrophysiology for Medical Training and Diagnosis Support", Sep 2011 –
  Mar 2013, grant GBP 73,113 of GBP 121,855). Earlier Innovate UK SMART
  grants 2012, 2014, 2016 (R&D page).
- Product timeline (FAQ Q16): Simulation ECG (2018), WebGL version (2018),
  Simulation Pacing (2019), Simulation Cardiac EP (2020), Atrial Fibrillation
  app (2020), Temporary Cardiac Pacing app (2020), Epicardio Heart Simulator
  app (2021). Customised builds: St Jude Medical MultiPoint Pacing
  (2016–2017), Medtronic external temporary pacing "vEPG" (2023–2025),
  Medtronic conduction-system pacing "vCSP" / VirtualCSP (2025), Medtronic
  "VirtualPULSE" app (shop page). Veterinary (canine) version exists (VPAT
  scope list; Liverpool vet school testimonial).
- The VPAT scope list enumerates the app SKUs: ECG for medical student;
  PACING for medical student; CARDIAC EP for medical student; Medical
  Educator/Pro; Atrial Fibrillation; Temporary Cardiac Pacing; Epicardio Heart
  Simulator; ECG for veterinary student; Advanced ECG for veterinary student;
  Veterinary Educator/Pro. The desktop binary contains matching package names:
  "PACING (inc. ECG)", "CARDIAC EP (inc. ECG & PACING)", "EDUCATOR/PRO (inc.
  ECG, PACING & CARDIAC EP)", "VET Entry Level", "VET Advanced Level", "VET
  EDUCATOR/PRO".
- "Patent Pending" and registered trademark are claimed on the site; **no
  granted or published patent by Epicardio Ltd was found** (searches on
  Google Patents / USPTO terms returned only unrelated patents) – unverified.
- Validation claims: "Arrhythmias ECG independently validated by Schiller AG";
  "Pacing ECG independently validated by Medtronic"; FAQ Q17 cites a
  peer-reviewed Journal of Electrocardiology study (Hull Royal Infirmary /
  University of Sheffield) finding no difference vs. lecture teaching – the
  paper could not be located in PubMed/Google; **unverified**.
- Claimed reach: >12k users in >25 countries (brochure).

### 2.2 Platforms (verified)

- Desktop: macOS and Windows standalone (Unity player). The Windows build in
  this repo (`/home/alain/heartsim/epicardio/`) is **Epicardio Simulation
  v4.934, compiled 23/10/2026, Unity 6000.3.6f1 (Unity 6.3), Mono scripting
  backend**, with D3D12 + DirectML.dll shipped, ~2.3 MB `Assembly-CSharp.dll`.
- Web: Unity WebGL in Safari/Chrome/Edge/Firefox, no plug-in (see section 4).
- Mobile: iOS/iPadOS and Android apps – Heart Simulator (`com.Epicardio.EHS2`,
  App Store id 1575071257), Temporary Cardiac Pacing (`com.Epicardio.TPW`,
  App Store id 1499084468), Atrial Fibrillation (`com.Epicardio.AFib`, App
  Store id 1499252850). Landscape only. Apple Pencil / rubber-tip stylus
  recommended.
- Minimum specs (technical-requirements page): macOS Sonoma, i5 2.6 GHz,
  4 GB; Windows 10+, 64-bit, DirectX 12 GPU; iPadOS 18+ / iPad Air 4; Android
  API 34+ (Galaxy Tab S9 FE example). The desktop README in the repo still
  quotes older minimums (Windows XP/7+, OpenGL 2 / DX9, 2 GHz, 2 GB) and
  "Broadband internet connection".
- Internet is required: tutorial text, MCQs, pathology data, ablation data,
  AP data, voice-over clips, EULA, licence/concurrent-user data are all fetched
  from the server at runtime (`WWW*` classes: WWWPathoData, WWWTutoText,
  WWWTutoVoice, WWWEcgQuiz, WWWEcgTest, WWWLicenceData, WWWMOTD, WWWUser,
  WWWVersionData, WWWABLData, WWWAPData …) with "using local data" fallbacks
  and "You will experience limited functionality" messages (inferred from
  strings in the local binary).
- Languages: English, Spanish, German, Greek, Mandarin (site); binary also has
  French and Italian UI strings and Chinese Simplified/Traditional enums.

### 2.3 Pricing / licensing (verified, shop page, EUR excl. VAT)

| Product | Monthly | Yearly | Notes |
|---|---|---|---|
| Med Student | 30 | – | 5-day trial |
| Med Student Advanced | 58 | 459 | |
| Med Educator (Academic Educator) | 76 | 606 | 5-day trial |
| Corporate Trainee | 70 | 551 | |
| Corporate Trainer (Educator) | 114 | 912 | |
| Epicardio Cardiac EP | 66 | – | 5-day trial |
| Temporary Pacing (app, CPD) | 10 | – | 2 CPD points |
| Atrial Fibrillation (app, CPD) | 10 | – | 2 CPD points |
| Understanding ECG (CPD, 10 points) | – | – | "coming soon" |
| Vet Student / Vet Educator / Med School multi-user | – | – | quote |

Heart Simulator mobile app: listed as free on the US App Store page fetched
(other listings show USD 2.99) – inconsistent, unverified. App Store rating 2.3/5
from 3 ratings; one review complains "no options to simulate pathology".

Licence types (FAQ Q15): single-user (one person, one computer, one year),
multi-user (site-limited), campus-wide / IP-address (no login). Each login is
locked to the first device used. EULA: no reverse engineering, no
redistribution, no automated use, training tool only, not for diagnosis.
Closed source; no SDK or API.

### 2.4 Simulation model (what is published)

**Verified – Hurmusiadis, "Virtual Heart: Simulation-Based Cardiac Physiology
for Education", Computing in Cardiology 2007, pp. 65–68 (PDF on epicardio.com,
image-only; read visually):**

- Anatomy: 3D model segmented from the Visible Male dataset (recursive
  region-growing + marching cubes); high-res and simplified meshes.
- Fibre orientation: canine DT-MRI dataset from UCSD Bioengineering lab,
  ~1 M fibre nodes over both ventricles; sub-sampled volumes of 200 K down to
  10 K nodes used to test real-time performance. Fibres drive (a) preferential
  AP propagation direction, (b) contraction direction, (c) transverse passive
  elasticity.
- Specialised conduction system: a 3D conduction tree (SA node, atrial
  pathways, AV node, His, bundle branches, Purkinje) built by hand from
  anatomical descriptions; a velocity model assigns average velocities per
  segment (figure: atrial muscle ~0.5 m/s, AV node ~0.05 m/s, His/bundle
  branches ~2 m/s, Purkinje ~4 m/s, ventricular muscle ~0.5 m/s). SA rate is
  interactively adjustable with immediate feedback.
- Single-cell AP: Luo–Rudy (1991) ventricular model run offline in CESE;
  AP from -86 mV (rest) to +50 mV, 300 ms depolarisation/repolarisation
  cycle; the voltage/time profile is imported as a look-up table.
- Propagation: **cellular automaton** (no differential equations solved at
  run time); regular discrete network with 6- or 26-neighbourhood; each cell
  has a state updated every simulated millisecond; a neighbour is excited when
  the source cell reaches ~30 mV threshold and the neighbour is fully
  repolarised; anisotropic conduction 0.5 m/s along fibre and 0.025 m/s
  transverse (paper text); ~100 K nodes near real time on a 1.6 GHz CPU
  (2007).
- Arrhythmia: generated by changing heartbeat rate and the refractory period
  (plateau duration); "dead cell" state produces block/re-entry around scar.
- Mechanics: Hill–Maxwell muscle model (active contractile + passive elastic
  element in parallel), anisotropic via fibre orientation, solved as
  M d2D/dt2 + C dD/dt + K U = F; cell contracts when AP exceeds ~40 mV,
  contraction peaks ~200 ms after AP peak.
- Patient data: timed cardiac MRI superimposed on the beating model.
- Implementation at the time: C (visualisation) and C++ (simulation),
  OpenGL/GLUT; free camera (tumble, track, drag, tilt, zoom).

**Verified – current marketing (brochure 2026, R&D page):** "Electromechanical
model of the heart, based on FEM, intra-/extra-cellular simulation and neural
cellular automata"; "can be trained to accurately reproduce complex
arrhythmias"; "responds to pacing, ablation and drug administration while
generating action potential wavefront and ECG/EGM signals in real time";
future: VVUQ validation for in-silico device testing, patient-specific
diagnosis support, drug-effect modelling.

**Inferred from identifiers in the local desktop binary (Assembly-CSharp.dll
strings; no code was decompiled):**

- Separate simulation regions/classes: `SimA` (atria), `SimAV` (AV node),
  `SimCT` (conduction tree), `SimV` (ventricles), `SimP` (Purkinje?), with
  per-region `depolarizeCell*`, `cell2cell*`, `paceCell*`, `ablateCell`,
  `blockCell`; per-region APD and CV modifiers (`APDModA/AV/CT/PUR/V`,
  `CVModA/PUR/V`), endo/epi APD (`APDendo`, `APDepi`), restitution/memory
  (`calcRestAPD`, `calcRestCV`, `calcMemAPD`, "Cell Memory", "Rest. APD B/C",
  "Rest CV B/C", "Base APD DI Min/Max"), "Dep. Thresh. (mV)", "Sim. Step (ms)",
  "Cell Density", "Purkinje Range L/R/LA/LP", bundle start/fire cells
  (`RBstart`, `LABstart`, `LPBstart`).
- ECG/EGM generation: electrodes are objects placed on a torso
  (`placeLeads`, `dragLead`, `dropLead`, `readLeadPositionsFromMarkers`,
  `resetLeadsBody`, V1/V2 misplacement checks `leadV1mispF`), per-cell
  distances to electrodes (`calcLeadDistances`, "Cell-Lead Dist",
  `cellDistF`, `cellDistPow`, `cellDensF`, `cellDensPow`, `ecgLeadDistance`,
  `ecgLeadRatios`), standard and precordial lead vectors
  (`setStandardLeadVectors`, `setPrecordialLeadVectors`), 12-lead computation
  (`calcGECG12`, `calcGECG13s`, `calcGEGM13e`, `calcGCSP14`, `calcSignal`,
  `calcSingleGraph`), electrical heart vector (`calcEHV`, `ehvLine`,
  `ehvpDottedLine1..5` = projections of the heart vector onto lead axes),
  catheter bipoles (`calcBipolarAP`, `calcEGMCellBase`), ECG filter and noise
  (`ecgFilterP`, `ecgNoiseAmp`, `ecgNoiseTh`, "EGM Filter"), voltage scaling
  ("mm/mV", `ecgVSF`), speed (12.5/25/50/100/200 mm/s), "S-L scaling".
  The most plausible reading is a distance-weighted sum of cell membrane
  potentials (or their temporal derivative) per electrode, with power-law
  attenuation and a density weighting, in an unbounded homogeneous medium –
  i.e. no finite torso volume conductor. **This is an inference**; the vendor
  does not document the forward model.
- Mechanics/haemodynamics in the current build: `ContractCell`, `ContractVb`,
  "LV Contraction", "RV Contraction", "F Contraction", "Contraction Dur",
  `calcLVEF` (EDV/ESV), blood-flow particles ("Bloodflow DT", `bloodflowL/R`),
  "Respiration Rate (bpm)" and respiration wave (baseline wander), "mmHg/s".
- Maps: "Isochronal Map" (`isoMap*`), "Voltage Map" (`voltMap*`, recorded
  over a window), "Depo Wave Hood" / "Repo Wave Hood" (wavefront surfaces),
  fibre map (`fibreIcon`), AP colour map with user-set voltage range.
- Unity packages present: InputSystem, TextMeshPro, Timeline, Burst,
  Mathematics, Collections, AI Navigation, **Unity InferenceEngine (Sentis)**
  + DirectML, Unity Services (Analytics, Purchasing 4.4.0), Newtonsoft.Json.
  The presence of an inference engine is consistent with the "neural cellular
  automata / can be trained" marketing wording but its actual use is
  unverified.

### 2.5 Heart Simulator app (EHS2) – Google Play / App Store listing (verified)

Google Play `com.Epicardio.EHS2`, updated 6 Feb 2026, PEGI 3; App Store
version 7 (Feb 2026), 130 MB, iOS 15.6+, landscape only, macOS M1 (unverified
for macOS), visionOS 1.0+.

Description (abridged): "interactive real-time 3D exploration to aid learning
of the cardiac rhythm … Utilises electrophysiology simulation and live ECG
signals to simulate the normal heart rhythm."

APP FEATURES as listed:

- Introductory tutorial with voice-over (English voice; text in EN/DE/ES/ZH/EL)
- Electrophysiology parameters: SA node switch on/off; SA node rate
  30–150 bpm; AV node block/unblock; Escape rhythm
- Live single-lead ECG display; Live 12-lead ECG display; Annotated 12-lead
  ECG display
- Pause and scroll ECG with simultaneous action-potential wavefront and 3D
  heart-vector view
- ECG calliper tool
- Simulation speed control: Slow-Mo, Hi-Fi, Fast
- ECG speed control; ECG voltage control
- ECG leads display with heart vector
- DCCV cardioverter/defibrillator
- Temporary pacing device with limited functions: on/off switch, pace and
  sense indicator, interactive placement of the pacing-wire tip; all other
  pacing parameters disabled (full device is in the TPW app)
- Anatomy views with free 3D camera control; choice of displayed anatomy;
  preset views
- Settings: sounds on/off, background colour and brightness, screenshot info,
  tool-tips on/off, language
- Version history highlights: v5 (Jan 2025) "updated 3D conduction system
  model, pause-and-scroll ECG with synchronised wavefront and vector views,
  real-time ECG, new voice-over, higher-quality ECG signals".

Rhythms in this app: **normal sinus rhythm only**, plus what the SA/AV switches
produce (sinus arrest with escape rhythm, complete AV block with escape
rhythm). No pathology library (verified by listing + user review).

### 2.6 Temporary Cardiac Pacing app (`com.Epicardio.TPW`, updated 30 Oct 2025) (verified)

- Step-by-step tutorial with voice-over, MCQ test, interactive exercise with
  different patient parameters each launch, test mode with feedback, user
  guide; CPD certificate (2 points).
- Fully functional temporary pacing device: pacing rate, voltage (output),
  sensitivity, mode; interactive pacing-tip placement including conduction
  system pacing.
- EP parameters: SA node switch and rate; AV node switch; AV retro/antegrade;
  AV node rate; AV node refractory period; AV node conduction velocity; choice
  of escape rhythm.
- Live single-lead ECG (choice of 12 signals), pause-and-scroll with wavefront,
  live 12-lead, calliper, simulation speed, DC cardioverter, free 3D camera,
  choice of anatomy, medical-information links.

### 2.7 Atrial Fibrillation app (`com.Epicardio.AFib`, updated 25 Aug 2026) (verified)

- Tutorial with voice-over covering AF with interactive: drug administration
  (Diltiazem, Digoxin, Flecainide), DC cardioversion, Pace & Ablate (AV-node
  ablation + pacemaker), stroke simulation, PVI ablation; MCQ test; user guide;
  CPD 2 points.
- Same EP parameter set as TPW (SA on/off + rate, AV on/off, AV retro/antegrade,
  AV rate, AV refractory period, AV conduction velocity).
- Live single lead (12 choices), pause-and-scroll with wavefront, live and
  annotated 12-lead, calliper, simulation speed, free camera, anatomy choice,
  preset views.

### 2.8 Full desktop / web platform – feature inventory

Sources: product-overview page, help PDFs (Tutor Guide 2023, Cardiac EP User
Guide 2026, AVRT Tutorial 2026, Case-Based Exercise 2024), tutorial-video
titles, and identifiers in the local binary. Items marked (B) are inferred
from binary identifiers only.

**3D view and anatomy (verified unless (B))**

- Real-time 3D heart with live action-potential wavefront; colour-coded
  transmembrane voltage (2007 paper colour code: blue = repolarised,
  red = fully depolarised, cyan = Purkinje network). A colour-bar control lets
  the user pick the colour range and set the displayed voltage window with two
  sliders (Tutor Guide suggests +10 mV to -60 mV to see T-wave activity).
- Free camera: drag to rotate, zoom +/- buttons or pinch; preset views: AP
  (click again for PA), cranial, caudal; (B) LAO, RAO, LAO-CRA, LAO-CAU,
  RAO-CRA, RAO-CAU, "Fluoro" view, close-up, additional numbered views.
- Anatomy menu (heart icon, bottom-left) to show/hide structures: (B) atria,
  ventricles, septum, valves (aortic, pulmonary, tricuspid, bicuspid),
  papillary muscles, coronary arteries/veins, coronary sinus, crista
  terminalis, foramen ovale, LA appendage, pulmonary veins/trunk, aortic arch,
  vena cava, bones, lungs, diaphragm, phrenic, torso, head, subclavian,
  jugular, femoral, iliac, carotid, cerebrum (stroke scenario), blood-flow
  particles, "Patient MRI Heart", anatomy labels on/off.
- Maps and overlays: AP map on/off, fibre map (myocardial fibre orientation),
  (B) isochronal map, voltage map (record over a time window), depolarisation /
  repolarisation wave "hood" surfaces, scar regions in patient cases
  (`VH case 1-4 scar`), "Normal vs pathology overlays" (marketing).
- (B) Mechanics: LV/RV contraction animation, LVEF read-out, blood-flow
  particles, respiration.

**ECG display (verified)**

- Single-lead strip at top of screen with lead selector (any of the 12
  standard leads) at its left; click on the strip to pause; a grey scroll bar
  with a handle and a yellow cursor line then lets the user scroll backwards
  through completed cycles while the 3D wavefront and heart vector follow.
- 12-lead window: live and annotated variants; formats 6x2, 3x4, 12x1; optional
  ECG-paper layer; full-window or half-window; (B) watermark, snapshot to
  pop-up window, download/export of the 12 signals.
- Heart-vector view: the mean electrical heart vector drawn as a white line in
  3D with thin dotted guide lines projecting it onto the selected lead axis
  (e.g. RA/LL for lead II) and onto the ECG; axis angle markers (B: -90, -30,
  0, 40, 60, 90, 100, 150, 160 deg; "Left Axis bias", "Right Axis bias").
- Electrode placement: pick-drag-drop each limb and chest electrode on the
  torso and watch the ECG change; reset button; (B) V1/V2 misplacement
  detection; canine electrode set for the vet version.
- ECG speed 12.5 / 25 / 50 / 100 / 200 mm/s; voltage gain (mm/mV); ECG filter
  and noise controls (B); simulation speed Slow-Mo / Hi-Fi / Fast; calipers
  (two cursors, delta in ms and bpm); "4D-ECG Test" (B).
- "Patient ECG" button in the Ischaemia tutorial shows a real patient 12-lead
  next to the simulated one (Case-Based Exercise).

**Electrophysiology parameters window ("Tutorial Parameters", cogwheel)**

- SA node on/off and rate; AV node on/off, direction (anterograde /
  retrograde), rate, conduction velocity, refractory period; escape rhythm
  location and rate; (B) AVN fast/slow pathway, 2nd-degree block type I/II
  atrial and ventricular rates, APD endo/epi, CV (m/s), cell density, Dep.
  threshold, simulation step, Purkinje ranges, BBB selectors (LBBB, RBBB, LAFB,
  LPFB, bifascicular), accessory pathway (location: left lateral, left
  posterior, right lateral, septal; direction antegrade/retrograde/both),
  ischaemia region with APD/CV modification, "Left/Right Rotation".

**Therapies**

- Temporary pacing: wire insertion via jugular or femoral route, single and
  dual chamber, rate, A/V output, A/V sensitivity, mode (incl. VVI, DDD,
  asynchronous), A-V interval, A-tracking, pace/sense indicators; (B)
  biventricular, quadripolar LV lead with 10 MultiPoint-Pacing vectors
  (SJM/Abbott), conduction-system pacing with Medtronic 3830 lead and
  C304/C315 sheaths, catheter navigation (push/pull/rotate), penetration depth,
  ring/tip capture, unipolar/bipolar, anodal capture, current of injury (COI),
  perforation and micro-perforation warnings, capture thresholds and
  loss-of-capture, LBBB correction cases ("KC case 1-4"), Medtronic EPG 5392 /
  53401 external pulse generators.
- Cardiac EP: insert diagnostic catheters HRA, HIS (3 bipoles), CS (bipoles
  CS1-2 … CS9-10), RVA and ablation catheter ABL; live EGMs per bipole next to
  the ECG; programmed electrical stimulation box (S1 cycle length x N beats,
  S2, (B) S3, continuous, Stim, output in V); pause/scroll the EGM page;
  calipers on EGMs (A-H interval, TCL, PPI); toggle "12-lead ECG" vs
  "Electrograms" page; accessory-pathway insertion; RF ablation by
  drag-and-drop of the catheter tip with red lesion spheres (AVRT accessory
  pathway, AV-node ablation, PVI, (B) CTI/isthmus); entrainment manoeuvres;
  AVN effective refractory period determination.
- Drugs (B + AFib listing): adenosine (dose/duration, AV CV target),
  digoxin/diltiazem (AV CV steps, max doses), flecainide (atrial APD/CV and AV
  APD/CV steps, max doses).
- Defibrillator/DCCV: charge, synchronised shock on R wave, shock animation
  ("shake"), spike on ECG (B); available in all apps.

**Learning modes (verified)**

- Tutorials: burger menu -> Tutorials -> bundle -> tutorial -> numbered pages;
  the "User Guide" window doubles as the tutorial text window; voice-over;
  pages can execute actions on the simulation (`pathoTextP1..P30act`); "Advance
  to the next tutorial".
- Levels: ECG / Pacing / Cardiac EP (home screen "Levels -> Cardiac EP").
- Patient cases menu (e.g. "AV Reentrant Tachycardia", VH case 1–4 with scar,
  KCL case 1).
- Tests: MCQ ECG test, CPD test (80 % pass, certificate), "set quiz" by
  educators (`WWWEcgQuiz: getQuiz/setQuiz/submitAnswer`), test timer and
  score upload, educator can "administer and monitor student tests" and
  customise tutorials/curriculum (medical-schools page).
- Group-teaching Tutor Guide with exercises, learning objectives and pre/post
  MCQs; case-based discussion PDF (STEMI -> VT/VF).

### 2.9 Rhythm / pathology list

Tutorial bundles (verified from product-overview page and matching binary
strings): Basics (Introduction to the heart, Normal rhythm, Cardiac axis);
Conduction Abnormalities; AV Blocks and Bradycardias; Atrial Arrhythmias;
Supraventricular Tachycardias; Wolff-Parkinson-White Syndrome; Ventricular
Arrhythmias; Ischaemia & Infarction; Misc Abnormalities & Pathology
Combinations; MCQ ECG Test; advanced Pacing and Cardiac EP tutorials.
The binary has tutorial navigation classes `Tuto0nav … Tuto17nav` (18 tutorial
groups) and up to 30 pages per tutorial.

Individual pathologies confirmed by name in official PDFs/listings/video
titles: Left Bundle Branch Block; atrial fibrillation; atrial flutter; 1st,
2nd (type I and II) and 3rd degree AV block; ventricular tachycardia;
ventricular fibrillation; STEMI (anterolateral: ST elevation V2–V6, I, aVL);
AVRT with left-lateral / left-posterior / right-lateral accessory pathway;
WPW; sinus arrest / escape rhythm; AV-nodal block.
Inferred from binary identifiers (B): RBBB, LAFB, LPFB, bifascicular block,
AVNRT (fast/slow pathway), focal atrial tachycardia (FAT), typical and atypical
atrial flutter (TAFL, AAFL), PACs, PVC/ectopic, ischaemia with configurable
region, scar-related VT cases, left/right axis deviation, ventricular
pre-excitation, stroke (AF embolus). Hyperkalaemia, Brugada, long QT,
pericarditis, hypertrophy: **not found** anywhere – unverified / probably
absent.

### 2.10 UI layout summary (desktop/web, verified from guides)

```
+--------------------------------------------------------------------+
| [burger] [T]   single-lead ECG strip (lead selector at left) ...    |
|                (click = pause; grey scroll bar + yellow cursor)     |
+-------------------------------+------------------------------------+
| left margin icon column       |                                    |
|  (shown via heart icon):      |   3D heart + torso                 |
|  zoom +/-, AP/PA, CRA, CAU,   |   AP colour map, wavefront,        |
|  anatomy toggles, colour bar  |   electrodes, catheters, wires,    |
|                               |   heart vector + guide lines       |
|  [cogwheel] Tutorial          |                                    |
|   Parameters window (SA/AV…)  |   right side icon column:          |
|  User Guide / tutorial text   |   ECG tools (12-lead, EGM, speed,  |
|   window (closable, "T")      |   gain, calipers, snapshot,        |
|                               |   leads/torso, pathology menu,     |
|                               |   pacing, defib, drugs, ablation)  |
+-------------------------------+------------------------------------+
| Cardiac EP level: left half = 3D heart; right half = ECG lead on   |
| top + EGM channels (HRA, HIS, CS, RVA, ABL) + PES box + Ablate bar |
+--------------------------------------------------------------------+
```

12-lead and EGM pages can replace the 3D view ("full window") or share it
("half window"); the WebGL canvas is 1200x675.

### 2.11 Licensing / openness

Fully proprietary, subscription, server-tethered content, EULA forbids reverse
engineering. The only open technical description is the 2007 CinC paper.

---

## 3. Cardiac Rhythm Simulator (cardiacrhythmsim.com)

### 3.1 Purpose and vendor (verified)

"A browser-based ECG training tool for medical and nursing students,
educators, and early-career clinicians. It generates simulated heart rhythms,
plays back over 45,000 real clinical recordings from public research datasets,
and tests rhythm recognition through quizzes … built for learning, review, and
teaching — not for diagnosis or patient care decisions."

Vendor: ShadeTree.IT LLC (for-profit), © 2023–2026, support@cardiacrhythmsim.com.
Earlier/alternate host: https://ecg-sim.web.app/ (same product; legal pages
there carry the cardiacrhythmsim.com contact). Note: web search also surfaces
SkillStat's "Six Second ECG" simulator (25 rhythms, lead II, MERLOT 2011) –
that is a **different product** and must not be conflated.

### 3.2 Platform and technology (verified from fetched bundles)

- Static Astro site with SolidJS islands (`/_astro/*.js`, `solid.*.js`,
  `ClientRouter.astro`); synthetic rhythm engine runs in the browser
  (`kind:"worker"` entries suggest a Web Worker); Stripe payments; Microsoft
  Clarity + analytics; cookie consent.
- Languages: English, Spanish, Portuguese (Brazil), Russian, German, Chinese
  (Simplified) – route prefixes `/es`, `/pt-BR`, `/ru`, `/de`, `/zh-CN`.
- Sign-in required for "My Rhythms" (custom rhythm library); otherwise free.
- Pricing: no paywall found; voluntary support subscriptions via the
  "Support" page: Quick boost USD 5/month, Steady rhythm USD 15/month, Full
  support USD 25/month, or the same amounts one-time; cancel anytime.

### 3.3 Sections and features (verified)

- **Simulator** (`/simulator`, page title "ECG Rhythm Generator"): synthetic
  single-channel rhythm strip rendered in the browser. Controls found in the
  bundles: rhythm selector (`rhythm-change`), heart-rate numeric input with
  increase/decrease (`viewer-heart-rate-input`, `bpm-change`), noise level
  (`rhythm-noise-change`), EKG/sweep speed (`set-ekg-speed`, `ekgSpeed`),
  display mode and display time / playback span (`display-mode-change`,
  `display-time-set`, `playbackSpan`), play / pause / loop, "Heart rate
  display" toggle, wave highlighting setting
  (`settings/simulator/wave-highlighting`, "wave-highlight" – highlights the
  P/QRS/T segments on the strip), rhythm "highlights" text panel, link to
  "My Rhythms". The strip is one lead (older descriptions say lead II;
  **lead choice not found** – unverified).
- **Custom rhythm editor** (`/my-rhythms/editor`, help at
  `/rhythm-creation-help`): a rhythm is a list of cardiac cycles, each a list
  of wave segments of type P, PR, F, Q, R, S, ST, T, TP, U; every segment has
  five numbers: amplitude (absolute peak, mV, slider -2..+3), duration (ms,
  0..500), apex position (0..1, default 0.5, 5 % steps), start voltage, end
  voltage (mV; mismatches draw a vertical step). Multiple segments of the same
  type can be stacked (e.g. notched P). Cycles differ from each other and loop.
  Noise adds per-sample jitter. Live preview while dragging sliders; "Save All
  Changes". Saved rhythms can be loaded into the simulator or presented.
- **Explorer** (`/explorer`, "ECG Database: 45,000+ Recordings"): browse
  45,046 real 12-lead recordings (500 Hz) from the PhysioNet "A large scale
  12-lead electrocardiogram database for arrhythmia study" v1.0.0 (CC BY 4.0;
  106 of 45,152 records excluded as unrenderable). Filters: rhythms,
  conditions (match any/all), sex, age ranges 0–20 … 81–100; condition
  code URLs such as `?rhythms=AFIB`, `?conditions=AMI,AnMI,MISW&matchMode=any`.
  "80+ arrhythmia types, 12 leads per record". Users can compare their edited
  synthetic rhythm with real 12-lead cases.
- **Quizzes** (`/quiz`): Rhythm Quiz (identify the rhythm from a strip; score
  and review pages) and Knowledge Quiz (ECG principles, cardiac cycle, heart
  rate calculation); "Measurement and Calculation" quiz planned.
- **Learn** (`/learn`): learning path "coming soon" (10 modules, 78 lessons,
  122 min: ECG orientation, strip reading, rhythm families, conduction
  changes, paced patterns, integrated interpretation); **Conditions Library**
  (`/learn/conditions`) with 88 entries, each "what it is, how to recognise it,
  what it can be confused with".
- **Support/Ask** (`/ask`): donation page and feature summary.
- AI disclosure: explanatory text may be AI-assisted and reviewed for
  readability only.

### 3.4 Rhythm / condition list (verified – Conditions Library, 88 entries; codes in parentheses)

Rhythms (kind = rhythm, 26 in bundle): Accelerated Junctional Rhythm (AJR),
Atrial Arrhythmia (AArr), Atrial Fibrillation (AFIB), Atrial Flutter (AF),
Atrial Rhythm (AR), Atrial Tachycardia (AT), AV Nodal Reentrant Tachycardia
(PPAC), AV Reentrant Tachycardia (AVRT), AV Dissociation (AVD), Bradycardia
(Brady), Ectopic Atrial Tachycardia (EAT), Idioventricular Rhythm (VEsR),
Junctional Escape Rhythm (AVJR), Junctional Tachycardia (JTach), Paced Rhythm
(PR), Sinus Arrhythmia (SA), Sinus Bradycardia (SB), Sinus Rhythm (SR), Sinus
Tachycardia (ST), Supraventricular Tachycardia (SVT), Ventricular Fibrillation
(VF), Ventricular Flutter (VFL), Ventricular Tachycardia (PVT), Wandering
Atrial Pacemaker (SAAWR), plus synthetic-only "Normal Sinus" (R006) and
"Asystole".

Conditions (kind = condition, 70): Abnormal U Wave, Acute MI, Anterior Wall MI,
Atrial Bigeminy, Atrial Escape Beat, AV Block, Blocked PAC, Bundle Branch
Block, Clockwise / Counterclockwise Rotation, Complete LBBB, Complete RBBB,
Early Repolarization, First-Degree AV Block, Fragmented QRS, Incomplete LBBB,
Incomplete RBBB, Junctional Escape Beat, Lateral MI, Left Anterior Fascicular
Block, Left Atrial Enlargement, Left Atrial Hypertrophy, Left Axis Deviation,
Left Posterior Fascicular Block, LV High Voltage, LV Hypertrophy, Low QRS
Voltage, Non-specific IVCD, P Mitrale, P Pulmonale, P Wave Change, Pathologic
Q Wave, Poor R Wave Progression, PACs, Premature Junctional Complex, PVCs,
Prolonged PR, QT Prolongation, QT Shortening, Right Atrial Enlargement, Right
Atrial Hypertrophy, Right Axis Deviation, RBBB, RV Hypertrophy, Second-Degree
AV Block, Shortened PR, Sinoatrial Block, ST Changes / Depression / Elevation
/ Extension, ST-T Change, Supraventricular Ectopic Beat, T Wave Change, T Wave
Inversion, Third-Degree AV Block, Ventricular Bigeminy, Ventricular Escape
Beat, Ventricular Escape Trigeminy, Ventricular Fusion Beat, Ventricular
Preexcitation, Ventricular Premature Beat, Wenckebach AV Block, WPW Syndrome,
LV Strain (bundle only).

### 3.5 Monitor / defibrillator / pacing / 12-lead vs 3-lead

- No bedside-monitor emulation (no alarms, no SpO2/NIBP, no 3-lead/5-lead
  selector) was found in page text or JS bundles.
- No defibrillator, cardioversion or pacing *simulation*; "Paced Rhythm" exists
  only as a rhythm type in the library.
- Synthetic simulator: single strip. Real recordings: full 12-lead.
- No scenario engine (no scripted deterioration, no vitals).
  (verified for the public, logged-out site; logged-in state unverified)

### 3.6 How the ECG is produced (verified from help page and bundle)

Template synthesis: each wave segment is drawn as a smooth hump from start
voltage to the apex (amplitude, at apexPosition x duration) to end voltage;
flat segments use near-zero amplitude; cycles are concatenated and looped;
per-sample noise added. Heart rate is derived, not set per se: cycles are
repeated 20x, RR intervals measured between R waves, BPM = 60000 / mean RR
(if no R wave: mean cycle duration); asystole reports 0. The heart-rate input
in the simulator presumably rescales the TP (and other) durations – the exact
rescaling rule was not visible (unverified). Example built-in "Normal Sinus"
(R006): P 0.15 mV/132 ms, PR 0/135, Q -0.5/46, R 2.0/36, S -0.8/45, ST
0.01/87, T 0.1/192, TP 0/124 (sum 797 ms = 75 bpm), noise 0.05.

### 3.7 UI layout (verified / partly inferred)

Top nav: Learn | Simulator | Explorer | Quizzes | My Rhythms | Support, language
dropdown, profile menu. Simulator page: strip viewer with heart-rate stepper
and rhythm selector in a control panel (`SyntheticControlPanel`), a
"highlights" information panel describing the rhythm, playback controls
(play/pause/loop, speed, display span), noise slider; the editor page adds a
per-wave slider panel with live preview. Explorer: filter sidebar + result
cards, 12-lead record viewer. Quiz: strip + multiple choice, score and review
pages. Mobile-friendly responsive layout.

### 3.8 Licensing / openness

Proprietary site; synthetic rhythm definitions are the vendor's; real ECG data
is CC BY 4.0 (PhysioNet). User-created rhythms remain the user's. No API.

---

## 4. Epicardio Unity WebGL build (epicardiowebgl.s3.amazonaws.com/epi/)

### 4.1 index.html (verified – fetched raw)

- `<title>Epicardio Simulation™</title>`, `lang="en-us"`, standard Unity
  WebGL template ("dark" progress-bar images), canvas `#unity-canvas`
  1200x675, fullscreen button, footer logo links to https://epicard.io/shop.
- Loader config (verbatim values):
  - `buildUrl = "Build"`
  - `loaderUrl = Build/EpicardioSimulation.loader.js`
  - `dataUrl: Build/EpicardioSimulation.data`
  - `frameworkUrl: Build/EpicardioSimulation.framework.js`
  - `codeUrl: Build/EpicardioSimulation.wasm`
  - `streamingAssetsUrl: "StreamingAssets"`
  - `companyName: "Epicardio Ltd"`
  - `productName: "Epicardio Simulation"`
  - `productVersion: "3.9995"`
- Mobile branch adds a viewport meta and switches to `unity-mobile` CSS (fills
  the browser); desktop fixes the canvas at 1200x675 px.
- On fullscreen click: `unityInstance.SetFullscreen(1)` and
  `unityInstance.SendMessage('JSbridge', 'resetScreen', '1')` – i.e. a C#
  GameObject named `JSbridge` handles resolution reset.

### 4.2 Build directory (verified via HTTP HEAD; binaries not downloaded)

| File | Size | Last-Modified |
|---|---|---|
| Build/EpicardioSimulation.loader.js | 18,228 B | 23 Oct 2026 16:58 |
| Build/EpicardioSimulation.framework.js | 441,803 B | 23 Oct 2026 16:58 |
| Build/EpicardioSimulation.data | 50,480,305 B (48.1 MiB) | 23 Oct 2026 16:58 |
| Build/EpicardioSimulation.wasm | 48,706,407 B (46.4 MiB) | 23 Oct 2026 16:58 |
| TemplateData/style.css | 1,266 B | 22 May 2024 |
| StreamingAssets/UnityServicesProjectConfiguration.json | 334 B | 18 Oct 2022 |

- No `.gz` / `.br` variants exist (403) – the build is served uncompressed,
  ~95 MB download before play.
- `UnityServicesProjectConfiguration.json`: cloud-environment "production",
  com.unity.purchasing 4.4.0, services.core 1.4.2, analytics 4.0.1.
- framework.js advertises WebGL2 with OpenGL ES 3.0 (ES 2.0 fallback) and
  Emscripten runtime; it contains no editor version string. The loader exposes
  `productVersion` only. **Unity editor version of the WebGL build:
  unverified**; the desktop build uploaded the same day is Unity 6000.3.6f1,
  so the WebGL build is very likely Unity 6.3 as well (inferred).
- The WebGL `productVersion` 3.9995 differs from the desktop README's
  "Version 4.934" and the VPAT's "v4.81" – the WebGL build appears to use a
  separate version counter (inferred).
- A login (user name) is required after load; content is streamed from the
  Epicardio server as in the desktop build.

---

## 5. Feature comparison table

Legend: Y = present (verified), (i) = inferred, P = partial, N = not found, ? = unknown.

| Feature | Axis Trainer | Epicardio (full platform) | Epicardio EHS2 app | Cardiac Rhythm Sim | Epicardio WebGL |
|---|---|---|---|---|---|
| Platform | Static web (any) | Win/macOS Unity, WebGL, iOS, Android | iOS/Android | Web (Astro/Solid) | Browser WebGL2 |
| Open source | Y (MIT) | N | N | N | N |
| Price | Free | EUR 30–114/mo | Free / 2.99 (?) | Free + donations | subscription login |
| Offline use | Y | N (server content) | ? | P (static) | N |
| 3D heart | N | Y | Y | N | Y |
| Free camera + presets | N | Y | Y | N | Y |
| Anatomy layer toggles | N | Y | Y (choice of anatomy) | N | Y |
| AP / depolarisation colour map on 3D | N | Y (user-set mV window) | Y (wavefront) | N | Y |
| Isochrone / voltage maps | N | (i) Y | N | N | (i) |
| Fibre orientation map | N | Y | N | N | Y |
| Torso with movable electrodes | N | Y | N | N | Y |
| Heart-vector / lead-projection view | N (circle only) | Y (3D vector + guide lines) | Y ("heart vector") | N | Y |
| Cabrera / hexaxial diagram | Y | P (axis markers) | N | N | P |
| Single-lead strip | Y (6 limb leads) | Y (any of 12) | Y | Y (1 strip) | Y |
| 12-lead display | N | Y (6x2, 3x4, 12x1, annotated) | Y | Y (real records only) | Y |
| ECG paper / grid | Y | Y (optional paper layer) | Y | Y | Y |
| Sweep speed control | N | 12.5–200 mm/s | Y | Y | Y |
| Gain control | N | Y (mm/mV) | Y | N | Y |
| Pause + scroll back in sync with 3D | N | Y | Y | play/pause/loop | Y |
| Calipers | N | Y (ms / bpm) | Y | N (quiz planned) | Y |
| Simulation speed (slow-mo) | N | Y | Y (Slow-Mo/Hi-Fi/Fast) | playback speed | Y |
| Noise / filter | N | (i) Y | N | Y (noise) | (i) |
| Heart-rate control | N | SA rate | SA 30–150 | bpm input | Y |
| AV-node parameters | N | on/off, rate, CV, ERP, retro | block/unblock | N | Y |
| Escape rhythm config | N | Y (site + rate) | Y | N | Y |
| Pathology library | N (axis only) | ~9 bundles, dozens of pathologies | N | 88 library entries, 26 rhythm templates | Y |
| Real patient ECGs | N | P (patient ECG in tutorials) | N | 45,046 12-lead records | P |
| Custom rhythm editor | N | educator customisation (server) | N | Y (wave-segment editor) | N |
| Temporary pacing device | N | Y (rate/output/sense/mode/wire) | P (on/off only) | N | Y |
| CRT / MPP / CSP pacing | N | (i) Y (SJM MPP, Medtronic 3830 CSP) | N | N | (i) |
| EP catheters + EGMs | N | Y (HRA/HIS/CS/RVA/ABL) | N | N | Y |
| Programmed stimulation (S1/S2) | N | Y | N | N | Y |
| Ablation | N | Y (RF lesions, AVN, PVI, AP) | N | N | Y |
| Drugs | N | Y (adenosine, digoxin/diltiazem, flecainide) | N | N | Y |
| Defibrillator / DCCV | N | Y | Y | N | Y |
| Stroke / embolus scenario | N | Y (AF app) | N | N | ? |
| Mechanics / EF / blood flow | N | (i) Y | N | N | (i) |
| Guided tutorials + voice-over | N | Y | Y | "coming soon" | Y |
| Quiz / MCQ | Y (axis class) | Y (MCQ, CPD test, educator-set quiz) | N | Y (rhythm + knowledge) | Y |
| Scoring / feedback | counters only | score upload, CPD certificate | N | score + review | Y |
| Educator tools | N | set quiz, monitor tests, custom content | N | present saved libraries | Y |
| Localisation | EN/DE/ES | EN/ES/DE/EL/ZH (+FR/IT strings) | EN/DE/ES/ZH/EL | EN/ES/PT/RU/DE/ZH | EN |
| Accessibility statement | N | WCAG 2.2 VPAT | N | N | N |
| Touch support | Y | Y (stylus recommended) | Y | Y | Y |

---

## 6. Feature superset we should implement

Prioritisation: **P0** = needed for a credible first release that matches the
educational core of all four references; **P1** = differentiating features
present in Epicardio that justify a 3D simulator; **P2** = advanced / niche.

### P0 – core (ship first)

1. **Real-time 3D heart with activation colour map**: per-cell transmembrane
   voltage colour (blue repolarised, red depolarised, distinct colour for the
   conduction system), user-adjustable displayed voltage window and colour
   ramp (Epicardio Tutor Guide pattern: sliders +10 … -60 mV).
2. **Conduction-system model** with SA node, atria, AV node (rate, conduction
   velocity, refractory period, anterograde/retrograde), His, bundle branches,
   fascicles, Purkinje, ventricles; per-region APD and CV; escape pacemakers
   with configurable site and rate; SA on/off, AV block on/off.
3. **ECG forward computation from the 3D source to 12 leads** (limb + augmented
   + V1–V6) from electrode positions on a torso; the Wilson central terminal
   and Goldberger augmented leads computed, not hard-coded; lead I/II/III,
   aVR/aVL/aVF consistent with Einthoven.
4. **Single-lead strip with lead selector + 12-lead page** (3x4, 6x2, 12x1),
   25/50 mm/s and 12.5/100/200 variants, 10 mm/mV gain with 5/20 options, ECG
   paper grid (1 mm / 5 mm), pause, **scroll back through history with the 3D
   view and heart vector synchronised to the cursor** (both Epicardio apps
   treat this as the headline feature).
5. **Heart-vector / axis view**: 3D mean electrical vector, its projection onto
   the selected lead axis with guide lines, and a **Cabrera/hexaxial circle**
   with the computed frontal QRS axis and classification (international:
   normal -30..+90, LAD -90..-30, RAD +90..+180, extreme 180..270; optional
   German 7-class Lagetyp scheme as in section 7.4).
6. **Pathology library (first wave)**: sinus brady/tachy, sinus arrhythmia,
   sinus arrest + junctional/ventricular escape, 1st-degree AVB, 2nd-degree
   type I and II, 3rd-degree AVB, LBBB, RBBB, LAFB, LPFB, PAC, PVC (uni/multi
   focal, bigeminy), AF, atrial flutter (typical), AVNRT, AVRT/WPW (accessory
   pathway location + direction), VT (monomorphic), VF, asystole,
   ischaemia/STEMI by territory. Each with a short explanation and "what it
   can be confused with" (Cardiac Rhythm Sim library pattern).
7. **Free camera + preset views** (AP/PA, LAO/RAO, cranial/caudal, LAO/RAO
   +/- CRA/CAU), zoom, anatomy layer toggles (atria, ventricles, septum,
   valves, coronary sinus, great vessels, torso, bones/lungs), anatomy labels.
8. **Simulation speed**: real time, slow-motion (e.g. 1/10), fast; fixed-step
   deterministic engine (1 ms step like Epicardio / 2007 paper) decoupled from
   render rate.
9. **Calipers** (two cursors, delta ms and equivalent bpm) on ECG.
10. **Heart-rate / interval read-outs**: HR, PR, QRS, QT/QTc, axis.
11. **Quiz mode** at minimum rhythm-identification with scoring, and the axis
    drill (random axis with boundary guard band, section 7.6).
12. **Cross-platform shell**: desktop (Win/macOS/Linux), WebAssembly build,
    touch-first layout for tablets (landscape), keyboard + mouse + touch +
    wheel input, DPR-aware 2D ECG rendering.
13. **Offline-first**: all content bundled (Epicardio's server tethering is a
    documented pain point: "limited functionality" offline).

### P1 – differentiators

1. **Movable electrodes**: drag limb and chest electrodes on the torso; V1/V2
   misplacement, limb-lead reversal detection, canine/vet electrode set as
   option.
2. **Temporary pacing device**: rate, A/V output, A/V sensitivity, modes (VOO,
   VVI, AOO, AAI, DOO, DDD, DDI), AV interval, capture/sense indicators,
   pacing-wire placement (RV apex, RV septum, RA appendage), capture threshold
   search; loss of capture / undersensing scenarios.
3. **Drugs** with region-specific APD/CV effects and dose/duration:
   adenosine, diltiazem/digoxin, flecainide, amiodarone, atropine, adrenaline.
4. **DC cardioversion / defibrillation** with sync on R, energy, shock
   animation and success probability depending on rhythm.
5. **Isochrone map** (local activation time) and **voltage map** recorded over
   a window; fibre-orientation overlay.
6. **Annotated 12-lead** (wave/interval labels) and **wave highlighting** on
   the strip (Cardiac Rhythm Sim "wave-highlight").
7. **Patient-case mode**: scripted scenario with narrative, parameter changes,
   expected diagnosis, MCQs, feedback and score; CPD-style certificate export.
8. **Guided tutorials** with page actions that drive the simulator; optional
   voice-over; tutor-guide style group exercises.
9. **Custom rhythm / pathology editor** (parameter sets stored as data files,
   shareable), and educator features: set quiz, collect scores.
10. **Real ECG comparison**: embed a CC BY 4.0 subset of the PhysioNet 12-lead
    arrhythmia database for "compare simulated vs real" (Cardiac Rhythm Sim
    Explorer pattern); attribution required.
11. **Export**: ECG snapshot PNG, 12-lead PDF on paper layout, CSV of signals,
    screenshot with metadata.
12. **Localisation** framework (EN first; DE/ES/FR/IT/ZH/EL/PT/RU candidates),
    accessibility (keyboard navigation, contrast, tooltips on/off, reduced
    motion), WCAG statement.
13. **Respiration / baseline wander, mains noise, muscle artefact** toggles
    (Epicardio has respiration rate and ECG noise controls).

### P2 – advanced

1. **Cardiac EP lab**: HRA/HIS/CS/RVA/ABL catheters with bipolar EGMs, PES
   (S1xN, S2, S3, continuous), AH/HV/TCL/PPI measurements, entrainment,
   AVN ERP determination, accessory-pathway localisation by CS activation
   sequence.
2. **Ablation**: RF lesions by drag-and-drop tip, AVN ablation, PVI, CTI line,
   accessory pathway; lesion persistence across the session.
3. **CRT / MultiPoint / conduction-system pacing**: LV quadripolar lead
   vectors, 3830-style lead screw-in depth, LBB capture criteria, perforation
   warnings, "fluoro" view.
4. **Electromechanics**: contraction animation, LVEF read-out, blood-flow
   particles, pressure traces; stroke-embolus scenario for AF.
5. **Patient-specific / imaging overlay**: MRI/CT slices on the 3D model,
   scar import for VT cases.
6. **Bedside-monitor and defibrillator emulation** (3/5-lead monitor, alarms,
   SpO2/NIBP, transcutaneous pacing) – none of the four references do this
   well; valuable for ALS/ACLS training.
7. **Extended pathology set**: hyperkalaemia, hypokalaemia, long/short QT,
   Brugada, pericarditis, LVH/RVH, atrial enlargement, dextrocardia, torsades,
   polymorphic VT, AIVR, junctional rhythms, pre-excited AF, bifascicular block,
   atypical flutter, multifocal atrial tachycardia.
8. **Educator analytics dashboard** and multi-user campus licensing (only if
   commercial).
9. **Neural / learned corrections** of the ECG (Epicardio's "neural cellular
   automata" claim) – research item, not a product requirement.

---

## 7. Exact math and algorithms in the open-source axis trainer (verified)

All angles in degrees, ECG/Cabrera convention: 0 deg points to the subject's
left (lead I), positive angles clockwise when viewed from the front (so +90 deg
is inferior = aVF, -30 deg = 330 deg = aVL, -150 deg = 210 deg = aVR).

### 7.1 Lead axes and lead projection

Lead axis angles used by `calculateEcg()`:

| Lead | theta_lead |
|---|---|
| I | 0 |
| II | 60 |
| III | 120 |
| aVR | 210 (= -150) |
| aVL | 330 (= -30) |
| aVF | 90 |

Projection of a unit mean QRS vector at angle `alpha` onto a lead:

```
A_lead(alpha) = cos( (alpha - theta_lead) * pi / 180 )       # in [-1, 1]

calculateEcg(alpha) = [ cos(alpha-0), cos(alpha-60), cos(alpha-120),
                        cos(alpha-210), cos(alpha-330), cos(alpha-90) ]
# order: I, II, III, aVR, aVL, aVF
```

Note that this is the pure dot-product (Einthoven/Goldberger) projection with
unit magnitude; the app never computes the inverse (axis from leads) – the
axis is always the independent variable. For our implementation the inverse
from leads I and aVF is the standard

```
alpha = atan2( A_aVF , A_I )                         # using net QRS areas
```

or, using I and II (more robust when aVF is small):

```
alpha = atan2( (2*A_II - A_I) / sqrt(3) , A_I )
```

(These two inverse formulas are standard textbook relations, not from the
trainer; included for completeness.)

### 7.2 Pointer angle from mouse/touch (circleCalculations.js)

```
center = ( el.offsetLeft + el.offsetWidth/2 , el.offsetTop + el.offsetHeight/2 )
d      = ( pointer.pageX - center.x , pointer.pageY - center.y )
theta  = atan2( d.y , d.x ) * 180/pi          # screen y grows downward
if theta < 0: theta += 360                    # -> [0, 360)
```

Because screen y points down, `atan2(dy, dx)` already yields clockwise-positive
angles with 0 at 3 o'clock, which is exactly the Cabrera convention; no extra
sign flip is required. Touch events use `changedTouches[0]`.

Mouse wheel: `alpha_new = alpha - deltaY / 25`, wrapped into [0, 360].

Drop shadow of the arrow is rotated with the pointer:
`offsetX = 0.3 rem * cos(45 - alpha)`, `offsetY = 0.3 rem * sin(45 - alpha)`.

### 7.3 Cabrera circle rendering (cabrera.css + main.js)

The circle is a square `<div>`; each class is a full-size absolutely
positioned child with a `conic-gradient` that is transparent except for its
sector. CSS conic gradients start at 12 o'clock and run clockwise, so
`conic_deg = alpha_ecg + 90`. Sectors overlap by 5 deg to avoid Gecko seams.

German circle (7 sectors):

| id | conic range | ECG axis range |
|---|---|---|
| uelt (überdrehter Linkstyp) | 0–65 | -90 … -30 |
| lt (Linkstyp) | 60–125 | -30 … +30 |
| it (Indifferenztyp) | 120–155 | +30 … +60 |
| st (Steiltyp) | 150–185 | +60 … +90 |
| rt (Rechtstyp) | 180–215 | +90 … +120 |
| uert (überdrehter Rechtstyp) | 210–275 | +120 … +180 |
| nwt (Northwest) | 270–360 | +180 … +270 |

International circle (4 sectors): enLad 0–65 (-90…-30), enNormal 60–185
(-30…+90), enRad 180–275 (+90…+180), enNwt 270–360 (+180…+270).

The active sector gets `filter: saturate(5)`, others `saturate(1)`; the arrow
`<div id="line">` gets `transform: rotate(alpha deg)`.

### 7.4 Axis classification (ecgCalculations.js `lagetyp`)

German nomenclature, alpha in [0, 360):

```
 30 <= a <  60  -> Indifferenztyp
 60 <= a <  90  -> Steiltyp
 90 <= a < 120  -> Rechtstyp
120 <= a < 180  -> Ueberdrehter Rechtstyp
180 <= a < 270  -> Northwest-Typ
270 <= a < 330  -> Ueberdrehter Linkstyp
a >= 330 or a < 30 -> Linkstyp
```

International nomenclature:

```
 90 <= a < 180  -> Right axis deviation
180 <= a < 270  -> Northwest (extreme) axis
270 <= a < 330  -> Left axis deviation      (-90 … -30)
a >= 330 or a < 90 -> Normal QRS axis       (-30 … +90)
```

Out-of-range input returns the string 'invalid degree'.

### 7.5 ECG strip synthesis (ecgDrawing.js)

Let `f` = pixels per centimetre of ECG paper, computed from the canvas:

```
f = ( canvasHeight / (nLeads + 1) ) * 0.4           # calculateEcgFactor
heightPerLine = floor(canvasHeight) / (nLeads + 1)  # lead i baseline at y = heightPerLine*(i+1)
grid: light lines every f/10 (1 mm, colour #ffd3d3), strong lines every f/2 (5 mm, #ffacac)
```

Time scale implied by the comments: `f` (1 cm) = 200 ms, so one complex is
placed every `5 f` = 1 s (fixed 60 bpm). One PQRST segment starting at
`x0` with scalar amplitude `A` in [-1, 1] and baseline `y`:

```
P wave : half-circle arc, centre (x0 + 0.25 f, y), radius 0.25 f (100 ms wide, 2.5 mm tall)
PQ     : line to (x0 + f, y)                         # P onset to Q = 200 ms
QRS (100 ms = 0.5 f), threshold t = 0.25:
  if A >= t:                  R peak  (x0 + 1.17 f, y - 2 f * A)      # R = 20 mm * A
                              S nadir (x0 + 1.34 f, y + 0.6 f * A)    # S = 6 mm * A
  if 0 <= A < t:              R peak  (x0 + 1.17 f, y - 2 f * t)      # R frozen at 5 mm
                              S nadir (x0 + 1.34 f, y + f * max(0.6 t, 2 (t - A)))   # S grows to 5 mm at A=0
  if -t < A < 0:              r peak  (x0 + 1.17 f, y - f * max(0.6 t, 2 (t + A)))   # mirror image
                              S nadir (x0 + 1.34 f, y + 2 f * t)
  if A <= -t:                 r peak  (x0 + 1.17 f, y + 0.6 f * A)    # (A<0 so this is up, 6 mm*|A|)
                              S nadir (x0 + 1.34 f, y - 2 f * A)      # deep S, 20 mm*|A|
  return to baseline at (x0 + 1.5 f, y)
ST/T   : half-circle arc, centre (x0 + 2.6 f, y), radius 0.4 f  (T always positive; QT = 2 f = 400 ms)
```

The "flatline" variant (unused by default) draws an RS complex proportional to
A without the threshold logic. Lead labels are drawn at x = 15 px with a
20 px serif font (15 px * DPR when DPR >= 2); the trace starts at
`x = 30 + 30*dpr` px. For the two-column layout the canvas is split at
`canvasWidth/2` and the second triplet (aVR, aVL, aVF) is drawn with
`xstart = canvasWidth/2`.

Design note for our implementation: the trainer encodes the morphology rule
"as the axis moves away from a lead, the R shrinks until |A| < 0.25, then an
equiphasic RS appears, then an rS", which is a reasonable pedagogical
approximation of a rotating dipole but is not physiological. A dipole-based
engine should produce this behaviour automatically.

### 7.6 Quiz random-axis generator (main.js `getRandomAxis`)

```
repeat
  a = floor(random() * 360)
until not ( 170 <= a < 280            # exclude Northwest and its borders
         or 320 < a < 340             # +/- ~10 around -30
         or  20 < a <  35             # around +30
         or  55 < a <  65             # around +60
         or  85 < a <  95             # around +90
         or 115 < a < 130 )           # around +120
```

Answer checking: `lagetyp(a, international, lang) === answerString`; correct
-> `answersCorrect++`, new axis; wrong -> `answersWrong++`, same axis retained.

### 7.7 Responsive rules

- Both left/right containers are forced square: `height = offsetWidth`.
- Canvas backing size = CSS width x `ceil(devicePixelRatio)`; `sideBySide =
  width < 300 px`.
- Language auto-select on load; switching language also switches the
  classification scheme (German <-> international) and redraws.

---

## 8. URLs consulted

### Axis trainer
- https://david-shrk.github.io/ecgaxistrainer/
- https://github.com/david-shrk/ecgaxistrainer
- https://api.github.com/repos/david-shrk/ecgaxistrainer/git/trees/main?recursive=1
- https://raw.githubusercontent.com/david-shrk/ecgaxistrainer/main/index.html
- https://raw.githubusercontent.com/david-shrk/ecgaxistrainer/main/README.md
- https://raw.githubusercontent.com/david-shrk/ecgaxistrainer/main/LICENSE
- https://raw.githubusercontent.com/david-shrk/ecgaxistrainer/main/resources/js/main.js
- https://raw.githubusercontent.com/david-shrk/ecgaxistrainer/main/resources/js/ecgDrawing.js
- https://raw.githubusercontent.com/david-shrk/ecgaxistrainer/main/resources/js/ecgCalculations.js
- https://raw.githubusercontent.com/david-shrk/ecgaxistrainer/main/resources/js/circleCalculations.js
- https://raw.githubusercontent.com/david-shrk/ecgaxistrainer/main/resources/js/languages.js
- https://raw.githubusercontent.com/david-shrk/ecgaxistrainer/main/resources/css/cabrera.css
- https://raw.githubusercontent.com/david-shrk/ecgaxistrainer/main/resources/images/cabreraEn.svg

### Epicardio
- https://play.google.com/store/apps/details?id=com.Epicardio.EHS2&hl=it (and &hl=en)
- https://play.google.com/store/apps/details?id=com.Epicardio.TPW&hl=en
- https://play.google.com/store/apps/details?id=com.Epicardio.AFib&hl=en
- https://play.google.com/store/apps/dev?id=7498744027215636337&gl=GB&hl=en
- https://apps.apple.com/US/app/id1575071257
- https://www.epicardio.com (https://epicardio.com)
- https://epicardio.com/product-overview/
- https://epicardio.com/faq/
- https://epicardio.com/technical-requirements/
- https://epicardio.com/video/
- https://epicardio.com/ecg-cpd/
- https://epicardio.com/product/temporary-pacing-monthly/
- https://epicardio.com/product/atrial-fibrillation/
- https://epicardio.com/research-development/
- https://epicardio.com/eula/
- https://epicardio.com/get-epicardio/
- https://epicardio.com/help/
- https://epicardio.com/shop/
- https://epicardio.com/students/
- https://epicardio.com/medical-schools/
- https://epicardio.com/hospitals/
- https://epicardio.com/cgi-data/pdf/Epicardio_Simulation.pdf (brochure, 17 pp.)
- https://epicardio.com/cgi-data/pdf/VirtualHeart_CinC_2007.pdf (Hurmusiadis 2007, 4 pp., image-only)
- https://epicardio.com/cgi-data/pdf/Epicardio_Simulation_Cardiac_EP_User_Guide.pdf (14 pp.)
- https://epicardio.com/cgi-data/pdf/Epicardio_Simulation_AVRT_Tutorial.pdf (16 pp.)
- https://epicardio.com/cgi-data/pdf/Epicardio_Simulation_Tutor_Guide.pdf (30 pp.)
- https://epicardio.com/cgi-data/pdf/Epicardio_Simulation_Case-Based_Exercise.pdf (14 pp.)
- https://epicardio.com/cgi-data/pdf/Epicardio_Simulation_Installation_Guide.pdf (36 pp.)
- https://epicardio.com/cgi-data/pdf/Epicardio_Simulation_VPAT.pdf (4 pp.)
- https://gtr.ukri.org/project/D843161E-F64A-4C75-9521-9CC5A54F3368
- https://www.healthysimulation.com/19929/epicardio-heart-simulator/ (blocked by Cloudflare; content only via search snippets)
- https://www.healthysimulation.com/nl/epicardio-hartsimulator/ (403)
- https://www.medtronicacademy.com/en-xd/content/virtualCSP-simulator (JS-only page, no content retrieved)
- https://apps.apple.com/pl/app/virtualcsp/id6739589166 (404)
- https://core.ac.uk/download/357358791.pdf (returned HTML, not the PDF)
- https://pubmed.ncbi.nlm.nih.gov/?term=Epicardio+simulation+ECG+interpretation (no Epicardio paper found)
- Local build inspected (strings only, no decompilation):
  /home/alain/heartsim/epicardio/EpicardioSimulation_README.txt,
  /home/alain/heartsim/epicardio/EpicardioSimulation_Data/{app.info, boot.config, globalgamemanagers, ScriptingAssemblies.json, Managed/Assembly-CSharp.dll}

### Cardiac Rhythm Simulator
- https://cardiacrhythmsim.com/
- https://cardiacrhythmsim.com/simulator
- https://cardiacrhythmsim.com/explorer
- https://cardiacrhythmsim.com/learn
- https://cardiacrhythmsim.com/learn/conditions
- https://cardiacrhythmsim.com/quiz
- https://cardiacrhythmsim.com/my-rhythms
- https://cardiacrhythmsim.com/ask
- https://cardiacrhythmsim.com/legal/terms
- https://cardiacrhythmsim.com/legal/data-sources
- https://cardiacrhythmsim.com/legal/ai-disclosure
- https://cardiacrhythmsim.com/_astro/SyntheticControlPanel.C1PIMn4-.js, normal-sinus.CkzoO4PD.js, index.DiuhhZ21.js, index.x3zoK8oK.js, index.jGxc30B0.js, store.BC8s42zx.js, routeMap.wLSwdVA2.js and other chunks referenced by the simulator/explorer/quiz pages
- https://ecg-sim.web.app/
- https://ecg-sim.web.app/rhythm-creation-help
- https://ecg-sim.web.app/legal/data-sources
- https://www.merlot.org/merlot/viewMaterial.htm?id=593419 (SkillStat "Six Second ECG" – different product)

### Epicardio WebGL
- https://epicardiowebgl.s3.amazonaws.com/epi/index.html
- https://epicardiowebgl.s3.amazonaws.com/epi/Build/EpicardioSimulation.loader.js
- https://epicardiowebgl.s3.amazonaws.com/epi/Build/EpicardioSimulation.framework.js
- https://epicardiowebgl.s3.amazonaws.com/epi/Build/EpicardioSimulation.data (HEAD only)
- https://epicardiowebgl.s3.amazonaws.com/epi/Build/EpicardioSimulation.wasm (HEAD only)
- https://epicardiowebgl.s3.amazonaws.com/epi/TemplateData/style.css
- https://epicardiowebgl.s3.amazonaws.com/epi/StreamingAssets/UnityServicesProjectConfiguration.json

### Web searches run
- "Epicardio simulation features 3D heart ECG simulator"
- "Epicardio ECG simulator pathologies list"
- "Epicardio Ltd patent heart simulation ECG"
- "cardiac rhythm simulator cardiacrhythmsim.com features rhythms defibrillator pacing"
- "Epicardio 3D heart electrical activation depolarisation visualisation"
- "\"cardiacrhythmsim.com\" OR \"Cardiac Rhythm Simulator\" ShadeTree.IT"
- "Epicardio Simulation cellular automaton / action potential / finite element …"
- "Epicardio Heart Simulator app chapters \"Understanding ECG\" modules list …"
- "Hurmusiadis \"Virtual Heart\" … Computing in Cardiology 2007"
- "Epicardio Journal of Electrocardiology Hull Sheffield ECG interpretation simulation study randomised"
- "\"Epicardio\" simulation ECG teaching medical students randomised …"
- "Hurmusiadis cellular automaton heart model \"Primal Pictures\" …"
- "\"Journal of Electrocardiology\" simulation 3D heart ECG interpretation teaching … Hull Sheffield"
- "Epicardio Simulation review ECG 3D heart simulator tutorial screenshots …"
