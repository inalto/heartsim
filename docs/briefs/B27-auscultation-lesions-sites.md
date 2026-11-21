# B27 — Gallops, murmurs and listening sites (heart sounds, phase 4)
**Crates** `heartsim-core`, `heartsim-ecg`, `heartsim-ui`, `heartsim-app`, assets · **Model** Opus · **Milestone** M9 (audio, phase 4
of 5) · **Depends on** B24, B25 (B26 for the web; not a build dependency).

This brief has two parts that can be given to two implementers one after the other: **Part A** (sounds, scenario section, valve
scenarios) is complete and testable on its own; **Part B** (listening sites, bell and diaphragm, valve highlights) builds on it.

## Goal
Beyond S1 and S2: third and fourth heart sounds, clicks, the opening snap, a pericardial rub and the classic murmurs, each
scheduled from the same engine events as the ECG so that they react to the rhythm the way the textbooks say (no S4 in atrial
fibrillation, an ejection murmur louder after a pause, a mitral-stenosis rumble that loses its presystolic accentuation in AF).
A scenario declares what is wrong with the valves or ventricles in an optional `auscultation` section; the rhythm scenarios that
exist today keep sounding exactly as they do. The learner chooses where on the chest to listen, and bell or diaphragm.

## Read first
`docs/research/heart-sounds.md` (the timing table, "Murmurs are envelopes between the same anchors", the conditions table,
"Recommended design", "What could not be verified") and `docs/research/heart-sounds-notes/{physiology_timing,synthesis_models,
conditions_teaching}.md`; `crates/heartsim-ecg/src/pcg.rs` (`Beat`, `schedule`, `Components`, `PcgParams`, the atoms,
`scheduled_atoms`, `PcgSynth`, `WallTimeSynth`, `SoundLabel`); `crates/heartsim-core/src/scenario.rs` (`Scenario`,
`ExpectedFindings`); `crates/heartsim-ecg/src/scenario.rs` (`Tier0Scenario::pcg_synth`, `record`); `crates/heartsim-ui/src/
pcg_lane.rs`; `crates/heartsim-app/src/{heart_sounds,heart_sound_player,rhythm_quiz,scenarios}.rs`;
`crates/heartsim-ui/src/torso_schematic.rs` and the intercostal landmarks used by the electrode editor
(`crates/heartsim-ecg/src/placement.rs`); the `valve_planes` layer in `crates/heartsim-app/src/heart_view.rs`.

## Non-negotiables for this brief
* **Existing output is frozen.** A scenario without an `auscultation` section, heard at the default site, produces bit-identical
  PCG samples to today (all B24 PCG goldens unchanged), and every ECG golden is unchanged. The new section never changes the ECG.
* **Pure, deterministic, random-access.** Everything stays in `heartsim_ecg::pcg`: no I/O, no audio dependency, `wasm32`. A sample
  at time *t* is a function of the events up to *t*, the scenario seed and *t* only, so sampling in one call or in chunks, and
  the wall-time synthesiser, agree exactly. Murmur "noise" is therefore a seeded sum of sinusoids (random phases and small
  detunings per beat from the seed and the beat time), not a filtered random stream with hidden state.
* **Causal.** A sound may depend on the past and on the current beat's record only. A diastolic murmur runs until the next
  ventricular event arrives and then fades; nothing already sampled is revised (beyond the existing `replaces_previous` window).
* **Every number is documented**: sourced (author, year, value) or the word **designed**. Published frequency bands for
  murmurs conflict; treat all spectral values as designed and say so in the user text: the sounds are a model, not recordings.
* No recordings are used or shipped in this phase. Nothing comes from `epicardio/`: scenario names, descriptions and teaching
  text are written fresh from the cited sources.
* Valve scenarios have an ordinary ECG (sinus rhythm unless the scenario says otherwise). They must not appear in the Rhythm
  Quiz, Axis Quiz, Axis Trainer or Interval Drill pools, where a strip that looks like normal sinus rhythm would be a wrong
  question.

## Part A — sounds and scenarios
1. **Scenario section** (`heartsim-core`). Optional `auscultation: ( ... )` in the scenario RON, `#[serde(default)]`, no
   `format_version` bump: `s3: bool`, `s4: bool`, `rub: bool`, `fixed_split: bool`, `lesions: [(kind, severity)]` with severity
   mild / moderate / severe. Lesion kinds in this brief: aortic stenosis, aortic regurgitation, mitral regurgitation, mitral
   stenosis, mitral valve prolapse, tricuspid regurgitation, pulmonary stenosis, ventricular septal defect, atrial septal defect,
   innocent (Still's) murmur. `ExpectedFindings` gains optional auscultation fields (extra sounds present, murmur timing class,
   split mode) used by the tests below and by the phase-5 quiz.
2. **Extra sounds** (new `Components` fields and atoms; rules with their sources in the research note):
   * **S3**: A2 + 120–180 ms, low (25–50 Hz), only after a beat that ejected; duration designed.
   * **S4**: a designed interval after an organised atrial activation that falls in ventricular diastole, low (20–30 Hz); never
     in atrial fibrillation or with silent atria; in complete block it follows the P waves, not the QRS.
   * **Summation gallop**: no special case. When S3 and S4 coincide at fast rates they add; a test shows it.
   * **Ejection click**: QRS onset + 120–140 ms (aortic) or 90–110 ms (pulmonary), high and brief.
   * **Mid-systolic click** of prolapse: designed position in mid-systole, followed by the late systolic murmur.
   * **Opening snap**: A2 + 30–140 ms, earlier with severity (under 70 ms when severe).
   * **Pericardial rub**: up to three scratchy components (atrial systole, ventricular systole, early diastole); the atrial one
     is absent without atrial contraction.
3. **Murmurs** as envelopes between the same anchors, with a per-lesion band (designed) and gain by severity:
   * Aortic and pulmonary stenosis: crescendo–decrescendo after a silent gap following S1, ending before the semilunar closure
     of its side, peaking later as severity rises; A2 softer in severe aortic stenosis (single S2), P2 soft and late in
     pulmonary stenosis; louder after a long preceding RR (the beat after a PVC, long cycles in AF).
   * Mitral and tricuspid regurgitation, ventricular septal defect: plateau from S1 to or through the closure sound of that
     side; mitral regurgitation does **not** grow comparably after a long cycle; tricuspid regurgitation is louder in
     inspiration (the respiratory phase from B24).
   * Prolapse: click, then a late systolic murmur to A2.
   * Aortic regurgitation: high-pitched decrescendo from A2.
   * Mitral stenosis: loud S1, opening snap, low mid-diastolic rumble with presystolic accentuation after the atrial event; the
     accentuation is absent in atrial fibrillation; the rumble lengthens with severity.
   * Atrial septal defect: `fixed_split` (wide, respiratory change under 20 ms) and a soft pulmonary ejection murmur.
   * Innocent murmur: short, early-to-mid systolic, low and "musical" (a narrow band, designed); S2 normal.
4. **Synthesis and labels.** New atoms in the existing family; the sum-of-sinusoids murmur generator; `PcgSynth` and
   `WallTimeSynth` both render them (in slow motion a murmur's band stays, its envelope stretches with the interval it fills).
   New `SoundLabel`s for S3, S4, the clicks, the snap; the lane labels discrete sounds only, the murmur is visible as its
   envelope. Headroom: with the loudest lesion the limiter of B25 is not driven beyond its knee on a normal beat (adjust the
   murmur gains, not `HEADROOM_GAIN`).
5. **Scenarios** (`assets/scenarios/`, RON, each with fresh description, teaching text and `ExpectedFindings`): heart failure
   with S3; left-ventricular hypertrophy with S4; S3 + S4 at a fast rate (summation); aortic stenosis mild and severe; aortic
   regurgitation; mitral regurgitation; mitral stenosis in sinus rhythm and in atrial fibrillation; mitral valve prolapse;
   tricuspid regurgitation; pulmonary stenosis; ventricular septal defect; atrial septal defect; innocent murmur; pericarditis
   with a rub (add the section to the existing pericarditis scenario if there is one). Add `s4: true` to one acute-infarction
   scenario. Group them in the scenario picker under their own heading, and keep them out of the quiz pools (see above).
6. **Docs.** A user-book page on the extra sounds and murmurs: what each is, when it occurs in the cycle, what to look for in
   the lane, what the rhythm does to it, and a plain statement of what is modelled and what is not (no carotid pulse, no
   manoeuvres, no radiation). `CHANGELOG.md`; scenario count in the README if it states one.

## Part B — where and how the learner listens
7. **Sites.** Five sites: aortic (2nd right intercostal space), pulmonic (2nd left), Erb's point (3rd left), tricuspid (lower left
   sternal border), mitral (apex). A gain table per site and per component or murmur class in a RON file under `assets/`
   (designed; backed only by qualitative statements: S1 louder at the apex, A2 dominant at the aortic area, P2 heard at the
   pulmonic area, each murmur loudest over its valve, aortic regurgitation at Erb's point). The **default** is an "overview"
   mix identical to today's signal, so goldens and recordings do not change; choosing a site applies its gains.
8. **Bell and diaphragm.** Two presets applied as per-partial weights by frequency (a designed low-emphasis and a designed
   high-emphasis curve), not as a time-domain filter, so the synthesiser stays closed-form. S3, S4 and the mitral rumble are
   clearly easier with the bell; that is the point of the control.
9. **UI.** A "Listening site" control next to "Play heart sounds" in the Display popup: the six choices and bell / diaphragm,
   persisted, en + it. The same sites as clickable markers on the chest drawing (reuse `torso_schematic`, positions from the
   intercostal landmarks). The lane and the sound always show the selected site; the lane title names it.
10. **Valve highlights.** With the `valve_planes` layer visible, each valve's ring flashes briefly when its component occurs
    (mitral M1, tricuspid T1, aortic A2, pulmonic P2; the stenotic or regurgitant valve glows for the length of its murmur),
    driven by the same schedule and the same display delay as the sound. Off when the app's reduced-motion or equivalent setting
    asks for it; if there is none, add a switch next to the layer.
11. **Docs.** The listening page gains the sites and the bell/diaphragm section with a chest figure; `CHANGELOG.md`; phase 4
    ticked in `docs/ROADMAP.md`.

## Out of scope
Manoeuvres (Valsalva, squatting, handgrip), radiation to the neck or axilla, HOCM, PDA, prosthetic valves, pulse and venous
signs. The auscultation tutorial, quiz, repetition trainer, small-speaker mode, reference recordings and WAV export (phase 5).

## Acceptance tests
* Frozen output: all existing ECG and PCG goldens pass unchanged; a scenario with an empty `auscultation` section equals one
  without it sample for sample.
* Golden PCG snapshots, one per new scenario (overview mix).
* Determinism and random access: every new scenario gives identical samples in one call and in frame-sized chunks; the
  wall-time synthesiser at 1× equals `PcgSynth` on three of them (one gallop, one systolic, one diastolic murmur).
* S3 onset is 120–180 ms after A2; S4 lies between the atrial activation and M1; with S3 and S4 at 140 bpm their envelopes
  overlap and the summed peak exceeds either alone.
* No S4 and no presystolic accentuation in the atrial-fibrillation variant of mitral stenosis; in the sinus variant the rumble
  rises before S1. In complete AV block with `s4: true`, S4 follows the P waves.
* After a PVC with a compensatory pause (build the case in the test), the aortic-stenosis murmur of the next beat is at least
  20 % louder (RMS) than in the beats before; the mitral-regurgitation murmur changes by less than 10 %.
* Aortic stenosis: the murmur's peak is later in severe than in mild, there is a silent gap after S1, and A2 is softer in severe.
* Aortic regurgitation starts at A2 and its envelope is non-increasing; mitral regurgitation spans M1 to A2 with a plateau
  (variation under a stated bound).
* Tricuspid regurgitation is louder at peak inspiration than in expiration; the atrial-septal-defect split changes by less than
  20 ms over the respiratory cycle while the normal split changes by 20 ms or more.
* Opening snap follows A2 by 30–140 ms and is earlier when severe.
* Quiz pools: no scenario with a non-empty `auscultation` section is ever drawn by the Rhythm Quiz, Axis Quiz, Axis Trainer
  or Interval Drill.
* Headroom: the loudest new scenario at full volume stays within ±1.0 after the B25 limiter and below the knee on normal beats.
* Part B: the overview mix equals today's signal; at the aortic site A2 is louder than P2 and at the pulmonic site P2 is
  relatively louder than at the aortic site; S1 is loudest at the apex; the bell raises S3 relative to A2 and the diaphragm
  lowers it; the site and chest-piece settings round-trip; UI tests (egui_kittest) for the control, the chest markers and the
  lane title; valve-highlight schedule test (which valve, when, for how long) without a GPU.
* Scenario files load, carry `format_version`, and pass the existing scenario validation tests; the wasm32 check of the pure
  crates passes (run by hand); `cargo xtask ci` passes.

## Review this brief cannot do
A clinician's ears. The literature offers no benchmark for how these sounds should sound, so before they are presented as what a
stethoscope delivers, someone who auscultates should listen to each scenario at each site and note what is wrong. Until then
the user text calls them models. The MSD-Manual details in the research note were read through a summariser: spot-check each
rule against the source before hard-coding it, and list any that could not be confirmed.

## Decisions for the maintainer
* Whether valve scenarios with S4 should also show left-ventricular-hypertrophy voltage on the ECG (needs its own sourced
  morphology; this brief leaves the ECG as plain sinus rhythm).
* Whether the default should stay the "overview" mix or become Erb's point once sites exist.
