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
| Windows and macOS | The installers are built and the test suite passes on GitHub's Windows and macOS runners, which have no display either. No person has opened the program on either system. | Same as above, on each system. The installers are **unsigned**: Windows SmartScreen and macOS Gatekeeper warn before they run. |
| The browser version | It compiles, the size budget holds and an automated smoke test loads the page in headless Chromium. Nobody has used it in a real browser with WebGPU or WebGL2; the browser table in `docs/web.md` is empty. | Open [the web app](app/) in Chrome, Edge, Firefox and Safari; fill in the table in `docs/web.md` (browser, GPU API, frame rate). |
| The monitor beep | The audio path is compiled on the CI runners and in the release builds, but no sound has been heard anywhere: the build server has no sound device or ALSA headers, so the beep was only type-checked there. Whether the AppImage carries `libasound` is unverified. | Turn the beep on in Settings on each system and listen. |
| Printed and exported ECGs | PDF, PNG, CSV and WFDB exports are checked by tests that read them back, not by a person looking at a printout or opening the WFDB record in another program. | Print a PDF at 25 mm/s and measure the grid; open the WFDB record in WFDB tools or PhysioNet's LightWAVE. |
| The validation bridge | **No solver has been run** on the openCARP exports. The element node order, the `carputils` calls in `run.py` and the `im_param` syntax were taken from documentation. | Run the exported case in openCARP and compare with `heartsim-cli validation-report` (see `docs/validation-bridge.md`). |
| The re-tuned anatomy in the 3-D view | The 0.6.0 pack (heart tilted as measured, atria and veins at anatomical levels, narrower torso) was checked by the automated tests and the screenshot tour only; nobody has looked at the chambers, vessels and torso layers from every angle. In the tour's pictures the default camera looks almost along the new long axis, so the left ventricle fills the view and the atria sit behind it; whether the default view should be re-framed is open. | Open the Simulator, turn each layer on and rotate the heart; report anything that looks misplaced. |
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
| Atrial and venous positions (`RA_CENTRE`, `LA_CENTRE`, caval and pulmonary-vein points, `la_pv_antrum`) | The atria, the veins and the AF presets | **Chosen, not measured**: set so the levels and relations are right; the chambers are spheres and sit about 1–3 cm further left than in a patient |
| `ForwardOptions::gain_mv` 4.4 (was 5.0) | Scales the forward-model ECG | Recalibrated so V1–V6 and the ST segments stay in range on the new pack; not a physical constant |
| AV concealment 0.85 in the AF presets | How irregular the ventricular response is | Above the 0.3–0.7 range the documentation cites; needed for an irregular response (about 113 bpm) on the new atria |
| Sinus axis +24°, P axis +33°, P(II) < 0.25 mV, AH 75 ms, HV 40 ms | The forward model's normal sinus beat | Measured on the pack; in range, but not compared with a recorded ECG |
| RBBB frontal axis | The axis read-out during RBBB | The net-area axis over the QRS reads about 170° while the peak amplitudes suggest about +25°; the wide terminal S is the likely cause and was not confirmed |

### Statements in the tutorials and cases

The content author flagged these for a clinician to confirm before they are taught as facts: the pacing advice for Mobitz II
and for AV block in inferior infarction, the 48-hour rule and the 120–200 J energy for AF cardioversion, the warning against
nitrates in right-ventricular infarction, the ST-elevation thresholds for STEMI, and the sentence on reciprocal changes in
III and aVF.

### Translation

Tutorial pages, scenario texts, electrode presets and exercise texts, ischaemia preset descriptions, the detector's findings,
pacemaker preset codes, territory names and export preset descriptions are English only; the Italian locale covers the
interface.

## 3. Behaviour that is approximate or missing

### The heart model

* The anatomy is a **procedural** heart (spheres and tubes placed on literature dimensions), not a mesh of a real heart. The
  bake from the open Strocchi/Rodero meshes described in `docs/anatomy-pack.md` has not been done. The generator still places a
  few cells on top of each other (the slow pathway against the compact AV node, the two left fascicles), which is why one
  validation-bridge correlation bound had to be loosened from 0.99999 to 0.9999.
* Fascicular blocks move the frontal axis the right way but not far enough: sinus ≈ +24°, LAFB ≈ +17°, LPFB ≈ +35° against the
  textbook ≤ −45° and ≥ +90°. Several hundred variants of the fascicle geometry were tried; every one that reached the target
  axis lost the rS pattern in V1. The test stays ignored with the target values (section 4).
* After flecainide ends an induced AF, the model stays on the Tier-2 grid with sinus sensed instead of returning to Tier 1.
* Tier 2 (the reaction–diffusion engine used by the re-entry presets and the EP lab's flutter and AF) runs on the CPU and is
  **not real time**: about 46–62 ms per frame at 64³ against an 8 ms budget on the development machine, and Tier 1 takes 60–85 ms
  per beat against 15 ms. There is no GPU path yet.
* Fibrillating tissue on the 3-D heart conducts at 0.3× normal speed so that the ventricles stop sweeping together; this is a
  display choice, not a measured value.
* No rotors, no anatomical flutter circuit and no drawn twist on the 3-D heart for torsades: those rhythms are drawn by the
  timing engine for the strip, and the heart shows a schematic version.

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

### Browser version

* No export or file features, no threads, and slower than the desktop build; the same results, only later.

## 4. Ignored tests

`cargo test` skips three tests on purpose. They document goals the project has not reached:

| Test | Why it is ignored |
|---|---|
| The fine Niederer slab (`heartsim-cli/tests/validation_bridge.rs`) | Takes minutes of CPU in the dev profile; run it with `cargo test -p heartsim-cli --release -- --ignored`. |
| Fascicular-block axes (`heartsim-ecg/tests/forward.rs`, LAFB ≤ −45°, LPFB ≥ +90°) | In this model the axis shift of a block stays below the sinus vector while V1 keeps its rS; the test's doc comment records what was tried. Kept as the target. The R-wave progression test next to it, ignored until 0.5.0, passes on the 0.6.0 pack. |
| The `t!` example in `heartsim-ui/src/i18n.rs` | A documentation snippet marked `ignore`: it shows the macro's use and is not meant to compile on its own. |

## How to report

Open an issue at <https://github.com/inalto/heartsim/issues> with the item's name from this page, what you checked and what you
found (a reference, a measurement, a screenshot). Corrections to the numbers belong in the scenario, tutorial or source file the
table names, with the reference added to its `references` field or `docs/research/`.
