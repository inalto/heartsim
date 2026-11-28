# Known limitations and open questions

This page collects what the people and tools that built heartsim could **not** verify, what they left out, and which numbers
they could not source well enough. It is written for whoever uses heartsim in teaching and for contributors who can close an
item: each entry says what is uncertain and what would settle it. The lists come from the implementation reports of the work
packages (`docs/briefs/`) and are kept current with each release; an item leaves the page when somebody has checked it.

Nothing here makes heartsim unsafe to use as a teaching aid, because it is [not a medical device](user/disclaimer.md) and never
reads or produces a patient's data. It does mean that **a teacher should read the numbers and the clinical statements below
with a textbook open** before relying on them in class.

## 1. Never run by a person, or on real hardware

| What | Status | What would settle it |
|---|---|---|
| The desktop program on a real screen and GPU | Every screenshot and user-interface test was made headless, under software rendering (Mesa lavapipe) on a Linux server with no display. No person has used the EP lab, the interval drill, the first-run disclaimer dialog, the filter and artefact controls or the Settings window in a running program. | Install a release on a laptop with a GPU, work through [the tour of the window](user/ui-tour.md) and report what looks wrong. |
| Windows and macOS | The installers are built and the test suite passes on GitHub's Windows and macOS runners, which have no display either. The first person to open the Windows build (0.7.0) saw a blue sphere and an empty rhythm list: no installer up to 0.7.0 shipped the `assets/` tree, and the smoke tests only ran `--version`. Fixed in 0.7.2 (the packages carry the assets, `heartsim --assets-dir` is checked after every install, and the app says so when they are missing); the rest of the window has still not been used by a person on either system. | Same as above, on each system. The installers are **unsigned**: Windows SmartScreen and macOS Gatekeeper warn before they run. |
| The browser version | It compiles, the size budget holds and an automated smoke test loads the page in headless Chromium. Nobody has used it in a real browser with WebGPU or WebGL2; the browser table in `docs/web.md` is empty. | Open [the web app](app/) in Chrome, Edge, Firefox and Safari; fill in the table in `docs/web.md` (browser, GPU API, frame rate). |
| The monitor beep | The audio path is compiled on the CI runners and in the release builds, but no sound has been heard anywhere: the build server has no sound device or ALSA headers, so the beep was only type-checked there. The AppImage does not carry `libasound`: it needs the host's `libasound2`, which the release smoke test now installs (the 0.6.0 build failed once without it). | Turn the beep on in Settings on each system and listen. |
| The heart sounds (B25) | Never heard. The build server has no sound device and no ALSA headers, so the `audio` build (kira stream, shared device) could not even be compiled there: its kira code was compiled and unit-tested against kira's mock backend only, and the CI's `--features audio` clippy job is its first real compile. Everything else (the player, the resampler, the fades, the limiter, the drift correction, the display delay) is tested with a fake sink. The real output latency, the default 60 ms offset and the sync check have not been tried on any machine. | On Linux, Windows and macOS: switch the heart sounds on, run *Check sync* in Settings, and note the offset at which click and flash coincide. |
| Pinch zoom and the strip's time zoom (B38) | Not seen on a real touch screen or trackpad: the build machine has no display. The gestures are tested with synthetic egui events only (a zoom event, two touch points moving apart, a Ctrl + wheel event), so the real feel (speed, direction, smoothing, the browser's own pinch handling on a phone) is unknown. The plain wheel zooms every strip of the app because none sits in a scroll area. | On a tablet or phone (the browser build) and on a laptop with a trackpad: pinch the 3-D views and the strips, scroll the page, and report anything that jumps, zooms the wrong view or scrolls the page behind. |
| Printed and exported ECGs | PDF, PNG, CSV and WFDB exports are checked by tests that read them back, not by a person looking at a printout or opening the WFDB record in another program. | Print a PDF at 25 mm/s and measure the grid; open the WFDB record in WFDB tools or PhysioNet's LightWAVE. |
| The validation bridge | **No solver has been run** on the openCARP exports. The element node order, the `carputils` calls in `run.py` and the `im_param` syntax were taken from documentation. | Run the exported case in openCARP and compare with `heartsim-cli validation-report` (see `docs/validation-bridge.md`). |
| The Echo page on a real GPU | Its cross-section shader ran only under software rendering (Mesa lavapipe). Keyboard control and handle dragging have no automated test. | Open the Echo page on a laptop with a GPU, move the probe with the keys and the handles, and check the cross-section against the legend. |
| The re-tuned anatomy in the 3-D view | The 0.6.0 pack (heart tilted as measured, atria and veins at anatomical levels, narrower torso) was checked by the automated tests and the screenshot tour only; nobody has looked at the chambers, vessels and torso layers from every angle. The default view is now a right anterior oblique so the long axis lies in the screen; the straight-on AP preset looks down the long axis and shows the left ventricle as a round outline in front of the atria, which is correct but easy to misread. | Open the Simulator, turn each layer on and rotate the heart; report anything that looks misplaced. |
| AF induction on Windows and macOS | The persistent-AF presets depend on a narrow stimulation window (S1 125 ms induces AF, 120 and 130 do not) and on the seed; the presets passed on Linux and on GitHub's runners for 0.6.0, but a failure on another machine or a later seed change would show as "no AF" after the burst. | Run `af_persistent` and the EP lab's "AF with PVI" on each system and confirm the atria fibrillate. |

## 2. Numbers and statements that need a clinician's or a physiologist's check

The implementers cited these from memory, from secondary sources, or chose them so that the teaching material reads well.
The simulator and its texts use them as written.

### The EP lab

| Value | Used for | Status |
|---|---|---|
| RF lesion time constant ≈ 8 s | How fast an ablation lesion grows | Recalled from Haines & Watson 1989; the half-time was not confirmed |
| 50 °C lesion threshold | When tissue under the catheter stops conducting | Nath 1993 not found; a review gives > 50 °C held for 60 s |
| Impedance falls during RF delivery | The impedance read-out | A stub; the only source found describes a rise on boiling |
| ERP ranges: atrium 170–300, AV node 230–425, ventricle 170–290 ms | ERP search and the protocol texts | Josephson, recalled |
| AH 50–120 ms, HV 35–55 ms | The baseline study's normal ranges | Sources vary for AH (50–120, 50–140, 55–150); HV agreed |
| PPI − TCL ≤ 30 ms means "in the circuit" | Entrainment read-out | Stevenson 1993 and Cosío 1993, recalled |
| 115 ms (Michaud 2001) | Entrainment teaching text | A commentary mentions the number without confirming the attribution |
| AH jump ≥ 50 ms | Dual AV-nodal physiology | Not confirmed |
| Citations Chauvin 2000, Jackman 1991/1992, Waldo 1977, Saoudi 2001, Verma 2015, Narula 1971 | Scenario notes in `assets/scenarios/ep/` | From memory |

### Rhythms and infarct evolution

| Value | Used for | Status |
|---|---|---|
| Beats per twist in torsades (the simulator uses the 5–20 range) | The torsades generator and its test | Teaching sources say 5–20, one says 180° in 10–12 beats; Dessertenne's paper was not read |
| Flutter rate 250–350/min | The flutter scenarios | Secondary sources only |
| Upper bound of the ventricular rate in AF | The AF scenarios | Not firmly sourced |
| Why synchronised cardioversion is unreliable in polymorphic VT | Defibrillator behaviour and its text | ACLS teaching material only |
| Time course of T-wave inversion after infarction | The evolving-infarct scenarios | Sources disagree; the simulator keeps it qualitative |
| Shortest pre-excited RR ≤ 250 ms as the danger line | Pre-excited AF scenario; the accessory pathway's refractory period is set to 200 ms to sit under it | ESC 2016 via slides |
| Pathological Q-wave criteria (Thygesen) | Q-wave tests | Read from reproductions, not the original |
| Q 0.45 mV, T 0.4 mV, flutter-wave amplitude, +85° axis of the inferior Q wave, conduction at 0.3× normal on the fibrillating heart | Teaching waveforms | Chosen by the implementer so the strips meet the textbook criteria; no literature number |
| Rhythm-analyser thresholds (rates, QRS widths, polymorphism) | The read-out that names the rhythm | They separate what this simulator draws and are not validated on recorded ECGs |

### The anatomy pack and the forward model

| Value | Used for | Status |
|---|---|---|
| LV long axis 36° in the frontal and 46° in the transverse plane; septum facing right and anterior | How the heart sits in the thorax, which sets the R-wave progression and the frontal axis | Measured values (Engblom 2005, Kayyali 2025, Narumi 2022) |
| Fascicle breakthrough sites | Where the anterior and posterior fascicles end | Durrer 1970 |
| Atrial and venous positions | The atria, the veins and the AF presets | From the Rodero 2021 average mesh (real chambers and inlets); the vein tubes beyond the inlets, the appendage lobe and the landmark cells on them are placed by rule |
| Heart position in the chest (53 mm left of the midline, 84 mm behind the sternum) and the torso's scale (×0.869) | Electrode distances, hence the precordial amplitudes | Measured on the KCL CT torso 1 (its own ventricular centroid), one subject; the torso is scaled uniformly so its LV has the baked heart's length |
| `ForwardOptions::gain_mv` 4.5 | Scales the forward-model ECG | Recalibrated on the baked pack for R(II) ≈ 1.0 mV; it cannot go higher because P(II) is at its 0.25 mV limit with one-cell atria; not a physical constant |
| AV concealment 0.75 ± 0.25 in the AF presets | How irregular the ventricular response is (≈ 100 bpm, RR CV ≈ 0.2) | The fraction is at the top of the 0.3–0.7 range the documentation cites, and the beat-to-beat scatter (`concealment_spread`, from the scenario seed) stands in for the changing penetration of fibrillatory wavefronts that the organised atrial wave does not provide (Cohen, Berger & Dushane 1983; Lian, Müssig & Lang 2006). Without the scatter the node settles into a fixed conduction ratio and the response is regular; it is 0 in every other preset, so flutter keeps its 2:1 and 4:1. |
| Sinus axis ≈ +43°, P ≈ 105 ms and P(II) ≈ 0.24 mV, QRS ≈ 90 ms, AH 83 ms, HV 43 ms | The forward model's normal sinus beat | Measured on the baked pack; in range, but not compared with a recorded ECG |
| RBBB frontal axis | The axis read-out during RBBB | The net-area axis over the QRS reads about 170° while the peak amplitudes suggest about +25°; the wide terminal S is the likely cause and was not confirmed |

### Statements in the tutorials and cases

The content author flagged these for a clinician to confirm before they are taught as facts: the pacing advice for Mobitz II
and for AV block in inferior infarction, the 48-hour rule and the 120–200 J energy for AF cardioversion, the warning against
nitrates in right-ventricular infarction, the ST-elevation thresholds for STEMI, and the sentence on reciprocal changes in
III and aVF.

The six cases of brief B44 (`assets/cases/case_wide_complex_tachycardia.ron`, `case_anterior_stemi_arrest.ron`,
`case_long_qt_torsades.ron`, `case_flutter_2to1.ron`, `case_pacemaker_malfunction.ron`, `case_pericarditis_or_stemi.ron`) were
written from the research note `docs/research/patient-cases.md`. The tables with the class and level of every guideline
recommendation could not be read, so **no class or level is stated or checked**, and no guideline dose is given. **No clinician
has reviewed these cases.** A clinician should check, by case and stage:

* *Wide-complex tachycardia, stage 1.* That a wide-complex tachycardia of unclear origin is treated as VT (ESC SVT guideline
  running text); stage 2: that AV dissociation and a scar from an old infarct favour VT; stage 3: that the AV-nodal drug
  warning (verapamil) is worded safely (from a case report, not from a guideline table); stage 4: that synchronised
  cardioversion is the answer for an unstable patient with a pulse, and the reason given for synchronisation (the T-wave
  vulnerable period); stage 5: the sentence on recurrence and work-up. The 100 J energy is the simulator's own value. In
  stage 3 the wide QRS of the aberrancy rhythm is clearest in V1 and subtle in lead II; its width was not measured.
* *Anterior STEMI and arrest, stage 1.* The territory and artery wording and the sentence that elevation reaching I and aVL
  suggests a proximal occlusion (taken from the scenario notes; the ACS guideline was not read for this case). Stage 2: that
  early ventricular arrhythmias are a reason for monitoring and do not delay reperfusion (the research's wording; not from a
  read guideline), and that an R-on-T ectopic beat has started VF (a case report). Stage 3: that VF is shocked
  unsynchronised, and that the simulator's synchronised mode refuses to fire in VF. Stages 4 to 6: ST resolution as a
  marker of reperfusion; the meaning of Q waves.
* *Long QT and torsades, stage 1.* The low-potassium ECG description (low T, U wave, long QT(U)) and the 2.6 mmol/L value
  (the simulator's drug dose is about 2.5). Stage 2: the short-long-short start, the statement that QT-lengthening drugs
  should be avoided (the ESC 2022 section on QT-prolonging drugs and the AHA/ACCF 2010 statement were **not read**), and the
  remark about amiodarone. Stage 3: unsynchronised shock for polymorphic VT (teaching material only, see the existing item on
  synchronised cardioversion in polymorphic VT). Stage 4: potassium and magnesium correction and the mention of pacing, from
  the case report's pattern; no doses.
* *Flutter 2:1, stage 1.* That a fixed rate near 150 suggests flutter and the 250 to 350 atrial rate (secondary sources).
  Stage 2: that adenosine does not interrupt flutter (ESC SVT guideline running text) and the warning of chest tightness.
  Stage 3: the effect of AV-nodal slowing on the ratio (the scenario files, not a guideline), told with `flutter_4to1`
  because the drug takes two minutes. Stage 4: the baseline difference between variable flutter and AF. Stage 5: that
  stroke risk with flutter is like AF and the team considers anticoagulation (told; the ESC and ACC/AHA AF guidelines were
  **not read**). Stage 6: the cavotricuspid isthmus ablation sentence.
* *Pacemaker malfunction, stages 1 to 5.* All device statements (why the paced QRS is wide, loss of capture, undersensing,
  functional non-capture, T-wave oversensing, the causes listed) come from the simulator's scenario notes and general
  device teaching; the NBG code paper and the pacing text books were **not read**, and the one paper read (a case report on
  a lead problem after a generator change) does not describe pacing spikes. The line that devices are interrogated for
  threshold and impedance is general knowledge. The case does not say what to reprogram.
* *Pericarditis or STEMI, stages 1 to 5.* The description of the ECG, the sentence that uncertain ST elevation is treated as
  possible ACS until excluded (scenario notes), the PR-segment depression (told, the simulator does not draw it: the
  strip does not show it), the rub description, the two-of-four diagnostic rule and aspirin or an NSAID with low-dose colchicine
  (ESC 2015 running text; class and level not read), and the echo statement (the simulator cannot show an effusion).
* *All six.* The invented patients, their ages, vital signs and laboratory values are made up to read plausibly and were not
  checked against any source. The simulated shocks succeed on the first try with the shipped scenario seeds in our headless
  run, but a shock outcome is drawn from a seeded generator and the stage texts say so; the next stage loads the rhythm
  that follows whatever happened.

### The echo calculators

The 49 verified calculators, among them the 16 guideline decision tools, were copied from the guideline tables and figures by
the implementer and tested at every boundary and on every branch, but no clinician has reviewed them for this program. Values
that fall in a gap between two printed limits are put in the more abnormal band. Where a guideline's figure and its text give
different limits (several do), the figure is followed; the list is in the research note. The way empty fields are handled
(indeterminate only when they could change the result) and the reading of "most parameters" in the mitral prosthesis table
(three of five) are the implementer's, not the guideline's.
The body surface area formulas of Mosteller and Du Bois could not be read in their original papers (the Haycock formula
was). Details per calculator: `docs/research/echo-calculators.md`.

### Translation

Tutorial pages, scenario texts, electrode presets and exercise texts, ischaemia preset descriptions, the detector's findings,
pacemaker preset codes, territory names and export preset descriptions are English only; the Italian locale covers the
interface. The echo calculators are written in English and Italian; in German, French and Spanish they appear in English, and
their source locators are in English in every language.

## 3. Behaviour that is approximate or missing

### The heart model

* The anatomy is now a **real four-chamber heart** (the Rodero 2021 average healthy mesh) in a **real chest** (the KCL CT
  torso 1), see `docs/anatomy-pack.md`. What is still constructed by rule rather than taken from a measurement: the vessel
  tubes beyond the dataset's cropped inlets, the appendage body, the coronary sinus path, the whole conduction system and
  the atrial sub-regions (SA node, crista, isthmus, Bachmann's bundle, vein sleeves), the electrode sites (ray-cast onto the
  skin at the procedural heights and azimuths) and the torso's uniform scaling to the heart's size. The torso is one
  subject; the heart is a population mean, so the two were never in the same body.
* **V4 over the apex.** On the baked heart V4 lies 30 mm from the LV apex, which finishes depolarising about 15 ms
  before the lateral and inferior walls, and the forward model (an unbounded homogeneous conductor) then shows V4 the back
  of the receding fronts: V4 has a QS complex and the R wave does not progress through it, while V3 and V5 are normal.
  Moving the electrode, rolling the heart about its long axis, a Brody-effect scaling of in-plane sources and every
  endocardial timing tried left R(V4) under 0.1 mV; a bounded torso (boundary-element forward model) is the likely remedy.
  The tests that measured V4 are ignored with this note (section 4); the pattern is wrong in the Simulator's V4 and in the
  pericarditis and T-inversion presets' V3–V4.
* Fascicular blocks: on the baked heart a left anterior block shifts the net-area axis the wrong way (sinus ≈ +43°, LAFB ≈
  +78°, LPFB ≈ +92°); the procedural pack had sinus ≈ +24°, LAFB ≈ +17°, LPFB ≈ +35°, already short of the textbook ≤ −45°
  and ≥ +90°. Both fascicular-block axis tests stay ignored with the target values (section 4).
* After flecainide ends an induced AF, the model stays on the Tier-2 grid with sinus sensed instead of returning to Tier 1.
* **AF on the baked atria is organised.** The smooth, isotropic, one-cell atrial walls hold one or two stable wavelets
  rather than fibrillation; the AV node is shown a fast regular input (its sensing site re-arms every 135 ms) and only the
  scattered concealment (`concealment_spread`, above) makes the ventricular response irregular, so the strip reads as AF, but amiodarone does not slow the shown
  rate and, with the presets' pacing site on the atrial side of the vein mouths, isolating the veins does not prevent the induction (section 4). Stronger and
  finer dispersion mosaics and a shorter wavelength were tried; a rough, anisotropic atrial wall (the real one) is what
  the substrate lacks.
* Tier 2 (the reaction–diffusion engine used by the re-entry presets and the EP lab's flutter and AF) runs on the CPU and is
  **not real time**: about 46–62 ms per frame at 64³ against an 8 ms budget on the development machine, and Tier 1 takes 60–85 ms
  per beat against 15 ms. There is no GPU path yet.
* Fibrillating tissue on the 3-D heart conducts at 0.3× normal speed so that the ventricles stop sweeping together; this is a
  display choice, not a measured value.
* No rotors, no anatomical flutter circuit and no drawn twist on the 3-D heart for torsades: those rhythms are drawn by the
  timing engine for the strip, and the heart shows a schematic version.

### Wall motion (the beating heart)

Brief B30. A kinematic model of wall motion, not electromechanics; see [the page](user/wall-motion.md). Nobody has compared it
with a real echocardiogram and **no clinician has reviewed it**: the look, the designed parameters and the read-outs need that
review before they are taught as more than a schematic.

* Many parameters are **designed**, not sourced (contraction timing, filling phases, the shape of the movement, the quivers,
  the respiratory shift); `crates/heartsim-echo/src/params.rs` marks each one.
* Valve leaflets in the 3-D heart are shown from start-up (B37); a user who switches a valve layer off sees the flat ring again (see *The tricuspid valve (B35)* and *The designed mitral,
  aortic and pulmonary valves (B36)* below). On the Echo page the leaflets are drawn whether or not the layers are shown. The
  tricuspid leaflets are measured; the other three are designed. No papillary muscles or pericardium. No pressure, flow or Doppler.
* The right ventricle has no closed cavity, so there is no RV volume or RVEF read-out; ischaemia reduces the left ventricle's
  inward motion only.
* No dilated or hypertrophied chamber shapes.
* One small fold of the RV surface near the tricuspid annulus can appear briefly in LBBB; thin walls (atria, vessel roots) may
  slip a few millimetres against their neighbours.
* Scrubbing back more than about 6 s shows the motion without regional detail.
* The motion is evaluated up to 30 times a second on the desktop and 20 times in the browser. One evaluation takes about 18 ms on
  one core of the development machine (a virtual machine; the bench is `cargo bench -p heartsim-echo`); its cost in the browser has not
  been measured.

### The tricuspid valve (B35)

Brief B35. The tricuspid leaflets in the 3-D heart and on the Echo page are one donor's valve, fitted to another heart. See
[the page](user/wall-motion.md#the-tricuspid-valve-layer). It is for education only, and **no clinician has reviewed it**.

* One donor valve (Texas TriValve 1.1, a healthy valve, CC BY 4.0), fitted to this heart's tricuspid ring by a scale, a rotation
  and a warp. The donor's shape is a finite-element result; the fit is a method choice. Nobody has compared it with a real valve.
* The source is a load sequence of seven shapes, not a timed recording. The valve is posed by the engine's valve schedule. The
  last keyframe segment holds about 6 mm of motion in the final 1.3 % of the closure, so the valve snaps the last part shut.
* The mitral, aortic and pulmonary valves are designed shapes, not measured ones; see *The designed mitral, aortic and pulmonary
  valves (B36)* below.
* The chordae origins were moved up to 13.8 mm to the nearest right-ventricular wall, because this heart has no right-ventricular
  papillary muscles.
* The open septal leaflet touches the septum: its vertices are pushed back into the blood, but up to 0.6 mm of the open septal
  leaflet still lies outside it, against the septum.
* The Echo page uses the measured tricuspid leaflets even when the 3-D layer is off. Chordae are not imaged there.
* No clinician has reviewed the fit, the timing or the look.
* Not seen on a real GPU, in a browser or in a running app window. The valve file is 155,832 bytes in the default pack (it
  deflates to about 104 KB); the web download size was not measured.

### The designed mitral, aortic and pulmonary valves (B36)

Brief B36. The mitral, aortic and pulmonary valves in the 3-D heart and on the Echo page are designed shapes. They are generated by
rule from published dimensions on this heart's valve rings. See [the page](user/wall-motion.md#the-mitral-aortic-and-pulmonary-valve-layers).
They are heartsim's own (MIT OR Apache-2.0), not a person's anatomy, and **no clinician has reviewed them**. No lesion is implied.
Sources and every designed value are in `docs/research/designed-valve-leaflets.md`.

* The mitral overlap of the shut leaflets is less than the designed 8 mm. The ring is too short for it: the overlap is about
  5.7 mm at the leaflets' centres and less toward the commissures. The valve shuts with no gap.
* The aorta and pulmonary artery are tubes without sinuses of Valsalva, and narrower than the ring in places. The cusps'
  attachment is moved toward the axis to stay inside the blood: up to 2.75 mm for the aortic valve and up to 6.75 mm for the
  pulmonary. Where the wall is close, the open semilunar valves open less than the designed 0.75 of the ring area.
* Leaflet vertices may lie up to 0.6 mm outside the blood.
* No coronary openings. The cusps are named non-coronary, left coronary and right coronary from the neighbouring chambers, not
  from the arteries they feed.
* The open shapes and the open-orifice fraction (0.75) are designed values with no source.
* The valve rings are the dataset's flat rings.
* The pulmonary trunk is lengthened in the pack, by a designed 20 mm tube beyond the dataset's end. The dataset's stub ended about
  9 mm above the pulmonary ring, too short for the cusps. The real trunk is longer and divides. The lumen grows from 3.6 mL to
  11.8 mL. The RV outflow view on the Echo page has its stored pose moved slightly.
* Not seen on a real GPU, in a browser or in a running app window. Checked by tests and a software renderer only. No screenshots
  were made.
* Echo cost: on the build server, the echo slice and the M-mode column with the meshes measured between about +2 % and +19 %
  against the cones, from run to run on a noisy machine (the targets were 10 % for a slice and 5 % for an M-mode column). Treat
  the speed as not confirmed.

### The Echo page (probe and cross-section)

Brief B31, phase 2 of the echo simulator; the grey image of phase 3 is in the next subsection. See [the page](user/echo.md). The
cross-section is an anatomical map of the model's tissues, not an ultrasound image, and **no clinician or sonographer has reviewed it**.

* The tricuspid leaflets are measured (B35); the mitral, aortic and pulmonary leaflets are designed meshes (B36). The papillary
  muscles, the moderator band, the pericardium and the lung-edge retraction (the parasternal window) are **designed**. The lung retraction's extent, and the shares of each window hidden by bone or lung (parasternal about 0.41 at full
  inspiration and 0.16 in expiration, apical 0.09, subcostal 0.06, suprasternal 0.5; `heartsim-echo/tests/slice.rs`), were measured
  on the default pack's start poses; they are not checked against patients.
* The shader that draws the cross-section ran only on the software renderer (Mesa lavapipe). Nobody has seen it on a GPU.
* Keyboard control and dragging of the probe's handles have no automated test.
* M-mode, tissue Doppler and calipers are built (phase 5, B34); see *M-mode, tissue Doppler and calipers (B34)* below. There is no
  blood Doppler. The standard views are built; see *The standard views (B33)* below.
* RV venous lumens are absent from the heart box. Without the valve files in the pack, the valves have the older designed shapes
  (the closed tricuspid valve is flat); with them, the leaflets are the meshes (see *The tricuspid valve (B35)* and *The designed
  mitral, aortic and pulmonary valves (B36)*).
* Cost: about 4.3 ms per drawn frame for the grid update and 7.2 ms for the slice on the development machine (release build), on
  top of the 18 ms of the wall-motion evaluation, at most 30 times a second on the desktop and 20 in the browser. The first visit
  builds the anatomy in about 0.7 s on the desktop; the browser cost is not measured. The browser uses a 128 × 256 label image
  instead of 128 × 512.
* The web size budget was not run (the development machine has no `trunk`).
* The cell-in-voxel agreement is 1.07 % off, with a one-voxel tolerance.

### The grey simulated image (B32)

Brief B32, phase 3 of the echo simulator. See [the page](user/echo.md). The image is a **simulated appearance from a simple line
model**, not a recording. Nobody has compared it with a real echocardiogram, and **no clinician or sonographer has reviewed how it
looks**. Treat it as a schematic.

* **Acoustic values.** The impedances and most attenuations are read from the source the research note cites
  (`docs/research/echo-simulator.md`, section 3.2). The myocardial attenuation (the table uses 1.0 dB/cm/MHz; in vivo values by layer
  are about 0.8 to 1.9) and the aperture (about 2 cm) are **unverified**. Every backscatter level, scatterer density, specularity and
  spread is **designed**, as are the leaflet and pericardium impedance, the lung treated as air, and the reverberation parameters.
  The status of each value is in `assets/echo/acoustic.ron`.
* **Speckle.** The grain is coarser than a real image at depth (about 1 to 2.4 mm axially), and its texture is cellular.
* **Not modelled.** Side lobes, refraction, mirror artefacts, harmonic imaging, near-field clutter, contrast agents and any pathology
  look. Reverberation is a designed repeat of the pleural line; its number and decay are not checked against a patient.
* **No measurement** is offered on the image in this phase.
* **Identical output on native and web** is argued from the design (the same CPU code and the same inputs), not tested.
* **Web cost not measured.** The estimate is 25 to 30 ms per image. The web uses 128 × 256 and caps the image at 20 Hz.
* **Web size budget** not run (the development machine has no `trunk`).
* **The shader** that scan-converts the image ran only on the software renderer (Mesa lavapipe).
* **Cost** on the development machine (release build): the grey image takes 14.7 ms per frame (line model 2.9, speckle 6.4, blur and
  envelope 2.8, compensation and compression 0.5), on top of 18 ms of wall motion, 4.3 ms of grid update and 7.2 ms of slice. The
  four stages add to 12.6 ms; the rest is not attributed.

### The standard views (B33)

Brief B33, phase 4 of the echo simulator. See [the page](user/echo-views.md). The targets, the tolerances, the marker rules of
the views the guideline describes only in words, the hint wording and the screenshots are written for this model. **No
sonographer or clinician has reviewed them.**

* **Designed values.** Every tolerance of the score, the hints and the reference search is listed on the page with its default.
  The marker rules of RV inflow, RV outflow, the four short-axis levels, the subcostal IVC and the suprasternal arch are designed,
  because the guideline gives no angle for them.
* **Supine anatomy.** The patient is supine. The guideline images the parasternal and apical windows in the left lateral
  position, which is not modelled.
* **Hidden shares.** At the reference poses, bone or lung covers 0.5 of the sector for RV outflow, 0.4 for the apical five-chamber,
  0.3 for the parasternal long axis and the apical two-chamber, and 0.2 or less for the other views. These are geometry checks on
  the default pack, not tests on patients.
* **Not reachable.** The subcostal IVC (the pack's inferior vena cava is shorter than the 30 mm the view needs) and the suprasternal
  arch (its best pose scores 35 %, below the 80 % that counts as found). The page lists both with the reason.
* **Preparing the self-test.** Starting the self-test prepares the reference pose of each view one per frame, with a progress bar
  (*Preparing views n of 15*), so the page no longer freezes for the whole run. The total time and the time for one view have not
  been measured since this change; before it, the whole run blocked the page for about 12 s on the development machine. The web is
  not measured. Otherwise a reference pose is computed the first time its view is used.
* **Checklist.** A checklist line is ticked when a designed share of the image's samples shows its structure. A thin or hidden
  structure may stay unticked at a pose that the score otherwise accepts.
* **Screenshots.** The self-test screenshot is scripted: its scores and times were set by a script, not measured.
* **What would settle it.** A sonographer's review of the targets, the tolerances and the hints, and a comparison of the
  checklists and the scores with recorded views of the same windows.

### M-mode, tissue Doppler and calipers (B34)

Brief B34, phase 5 of the echo simulator. See [Measuring on the echo](user/echo-measuring.md). Every value is a measurement of the
model. **No clinician or sonographer has reviewed the measurements or the conventions** they follow (Mitchell et al. 2019, pp. 18,
31–33 and 44). Treat them as a study aid.

* **No blood Doppler**, spectral, colour or continuous wave, and no control for it. The model has no blood flow field. Tissue Doppler
  is offered only at the septal and lateral mitral annuli and the lateral tricuspid annulus.
* **The velocities are not corrected for the angle** between the beam and the motion; the angle is shown. The thin band round the
  Doppler curve is **designed** and labelled "designed band".
* **Tissue Doppler was exercised** in sinus rhythm, and for a' in atrial fibrillation, and nowhere else.
* **Low model values.** The model shortens little at the level of the LV dimensions: at 60 beats a minute its fractional shortening is
  about 20 % (LVIDd 45.0 mm, LVIDs 36.2 mm). Its tricuspid S' (8.6 cm/s at the lateral annulus) is smaller than its mitral S' (9.6 and
  14.0 cm/s). These are the model's numbers, not norms, and they have not been checked against a patient.
* **The M-mode trace cannot keep real time on the development machine.** One column costs about 34 ms against a 25 ms step. When the
  trace falls more than 16 steps (0.4 s) behind, the page computes every other column and fills the gaps from their neighbours; more
  than 32 steps behind, it skips ahead and leaves a gap. This was not measured on faster hardware or in the web build.
  *Settings → Faster M-mode trace (echo)* records a column every 50 ms from the start, which halves the cost of the trace; a time read
  from it is then good to 50 ms. The switch is off by default, and whether it is enough on the development machine was not measured in
  the running app.
* **Snap distances** are the page's own choices: 8 mm for the tissue Doppler gate, and 3 mm for the caliper snap, which is off by
  default.
* **Narrow window.** At 400 × 800 px with a trace on, the sector is about 160 px high and the controls about 100 px: cramped.
* **Send to calculator** takes only the names the simulator measures. MAPSE, a', the mitral S' and the left atrial diameter have no
  calculator input, so **Send to calculator** is disabled for them. Nothing measured is saved.
* **Italian labels** were checked for fit by a width test only.

**Decisions for the maintainer, still open.** The defaults taken for now:

* *The designed band.* Default: drawn on the tissue Doppler trace and labelled "designed band". Open: whether a plain curve is the
  more honest choice.
* *Blood Doppler.* Default: none, and the research note advises against a first version. Open: whether a schematic blood Doppler,
  labelled as schematic, is wanted later.
* *Clinician review.* Default: none yet, so the page describes the Echo section as a study aid. Open: a clinician's review of the
  measurement conventions and of the whole echo section, before it is described as more than a study aid.

### Rhythms

* Typical flutter exists in two separate forms that do not meet: a strip-level driver (scenarios `flutter_*`) and the EP lab's
  drawn, pre-seeded Tier-2 circuit, which is not induced by pacing and does not come back to Tier 1 after the procedure.
  Variable flutter only mixes 2:1 and 3:1. QT is not reported during flutter.
* Torsades does not degenerate into VF, and coarse VF does not decline to fine within a run.
* Magnesium is not in the drug table, and AV-nodal drugs do not show the acceleration they can cause in pre-excited AF.
* The analyser reads QRS about 12–18 ms short over flutter or f waves, which is why pre-excited AF is named at ≥ 110 ms rather
  than 120; fast narrow-complex AF can stay unnamed when no baseline window follows the R wave.
* PR depression in pericarditis is not drawn.
* Left out of the content so far: incomplete RBBB, manifest WPW with sustained orthodromic AVRT, antidromic AVRT, and drug
  scenarios other than adenosine.

### The EP lab

* Catheters cannot be dragged in the 3-D view: the coronary-sinus catheter has a depth slider, the ablation tip moves by clicking
  on the heart, the others sit at landmark presets.
* Cycle length and PPI are read-outs, not caliper tools. The ERP search is disabled in Tier 2; Tier-1 refractory periods are fixed
  parameters that do not shorten with the drive cycle.
* "AF with PVI" has no Tier-2 test of its own; its claims rest on the re-entry framework's tests of the same substrate with
  3 mm lesions, while the lab uses 4 mm.
* In the accessory-pathway procedure, entrainment captures 5 of 8 stimuli because the timing engine and the mesh disagree on
  refractoriness; the panel says when capture is partial. AVNRT's post-pacing interval is untested, and EP determinism across
  thread counts is untested.
* The temperature and impedance read-outs are a teaching stub, not a thermal model.

### Display and sound

* The display filters are causal, so the drawn trace lags the unfiltered one by a few milliseconds.
* The monitor beep is up to one frame late and is rate-limited to one tone per 180 ms of simulated time.
* The heart-sound trace models S1 and S2 only. Several of its numbers were seen only in extracts of the papers
  (the QS2 intercept, the 40 ms QRS-to-S1 figure, the chirp frequencies of A2 and P2), and the S1 loudness curve, the
  interventricular delays and the shapes of M1 and T1 are design choices; each is marked in `PcgParams`. The timing has not yet
  been checked against a recording with simultaneous ECG and PCG. A2 lands 20–35 ms before the drawn T end at 50–100 bpm.
  The S1 and S2 labels of a beat appear about 0.15 s after its QRS, once a second paced front can no longer change it.
* Heart-sound audio (desktop, `audio` feature): kira reports no output latency, so the views are delayed by the 40 ms queue
  plus a user offset that has to be set by ear (*Check sync*); a wrong offset leaves sound and picture apart by the
  difference. The 3-D heart is drawn in the past from its 25 Hz colour-map history, so while the sound plays the wavefront is
  interpolated between frames 40 ms apart (activation-time and APD maps step at those frames) instead of being the live
  engine state. In the EP lab and the re-entry presets the 3-D heart is not recorded in that history and stays live, about
  0.1 s ahead of the sound; the strip and the lane are delayed there as elsewhere. When those Tier-2 engines cannot keep up
  with real time, fewer samples arrive than the sound card plays and the heart sounds break up. The Axis Trainer, the Axis
  Quiz and the Electrodes page draw their leads live (not delayed). Above 1× the heart sounds are muted; while paused only
  the lane's *Play* button sounds; slices always play at 1×.
* Heart-sound audio in the browser (B26): the views are delayed by a 100 ms look-ahead, the 40 ms queue and the
  `baseLatency` and `outputLatency` the browser reports. A browser that does not report `outputLatency` (or reports 0) adds
  nothing for it, so the sound is then heard that much after the picture until the *Audio offset* is set with *Check sync*;
  the offset starts at 0 ms in the browser (60 ms on the desktop) so that the picture is never drawn after the sound. Sound
  starts only after a click or a key press on the page (autoplay policy); after a reload with the sound on, the first click
  starts it. When the tab is hidden the simulation stops (no animation frames), what is scheduled plays out and the audio
  context is suspended; on return the old stream is dropped and playback restarts from the present. A main-thread frame
  longer than about 120 ms (three times the queue) makes a gap in the sound, which then restarts with a short fade-in;
  shorter long frames are absorbed by the look-ahead. No browser has been listened to.

### Narrow windows (below 700 px)

These come from the interface pass (the one-part-at-a-time pages, the folded strip toolbar, the stacked trace). See [the tour](user/ui-tour.md#narrow-windows-and-phones).

* The tutorial and case reader is still a side panel over the page at this width.
* On the Axis Quiz and the Axis Trainer the dial needs scrolling to reach the cards below it.
* The timeline editor has not been checked at this width.
* The narrow layouts of the 12-lead, Heart Vector, Electrodes and EP Lab pages were checked by screenshots only; no automated test
  covers them.

### Browser version

* No export or file features, no threads, and slower than the desktop build; the same results, only later.

## 4. Ignored tests

`cargo test` skips three tests on purpose. They document goals the project has not reached:

| Test | Why it is ignored |
|---|---|
| The fine Niederer slab (`heartsim-cli/tests/validation_bridge.rs`) | Takes minutes of CPU in the dev profile; run it with `cargo test -p heartsim-cli --release -- --ignored`. |
| Fascicular-block axes (`heartsim-ecg/tests/forward.rs`, LAFB ≤ −45°, LPFB ≥ +90°) | In this model the axis shift of a block stays below the sinus vector while V1 keeps its rS; the test's doc comment records what was tried. Kept as the target. |
| Fascicular blocks shift the axis the right way (`forward.rs`) | On the baked anatomy a left anterior block shifts the net-area axis right (sinus ≈ +41°, LAFB ≈ +76°); see section 3. |
| R-wave progression V1 → V5 and the sinus-beat waveform test (`forward.rs`) | V4 over the apex has a QS on the baked anatomy (section 3); aVR ends in an r' of 0.5 mV and V1's r is 0.45 mV from the terminal rightward vector. V1 → V3 and V5 progress, rS in V1, qR in V5–V6, concordant T waves and the P wave all hold. |
| LBBB and RBBB morphology (`forward.rs`) | LBBB: V6's broad R has one peak, not the notch. RBBB: V1 shows r and a late R' but the s between them stays shallow. Both beats are wide with the right terminal forces. |
| RV-apex pacing (`heartsim-ecg/tests/pacing.rs`) | The paced beat is wide with a superior axis and positive I and V6, but V1's net area is slightly positive. |
| Ventricular entrainment during AVRT (`heartsim-sim/tests/ep_lab.rs`) | On the baked atria the eight stimuli from the RV apex terminate the induced AVRT instead of entraining it; the entrainment read-out is exercised in the flutter lesson. |
| Amiodarone and flecainide in AF, PVI before induction (`heartsim-sim/tests/reentry.rs`) | The AF sensing site re-arms every 135 ms whatever amiodarone does to the APD (flecainide's slower conduction does register), and the AF presets' pacing site lies on the atrial side of the vein-isolation ring, so the burst still captures the atrium after isolation. The other AF tests (persistence, irregular response, shock, PVI during AF) pass. |
| Reversed transmural gradient and pericarditis (`forward.rs`, `therapy.rs`) | Not ignored but V3 (and V4 for pericarditis) are left out of the lead lists: over the anterior RV and the apex they read the near field (section 3). |
| The `t!` example in `heartsim-ui/src/i18n.rs` | A documentation snippet marked `ignore`: it shows the macro's use and is not meant to compile on its own. |

## How to report

Open an issue at <https://github.com/inalto/heartsim/issues> with the item's name from this page, what you checked and what you
found (a reference, a measurement, a screenshot). Corrections to the numbers belong in the scenario, tutorial or source file the
table names, with the reference added to its `references` field or `docs/research/`.
