# B28 — Teaching auscultation: tutorial, quiz, repetition trainer, reference clips (heart sounds, phase 5)
**Crates** `heartsim-core`, `heartsim-ecg`, `heartsim-app`, `heartsim-ui`, `heartsim-io`, `heartsim-cli`, assets · **Model** Opus
(Part C: Sonnet) · **Milestone** M9 (audio, phase 5 of 5) · **Depends on** B18 (tutorials), B24, B25, B27 (B26 for the web).

Three parts. **Part A** (trainer, quiz, tutorial) is the teaching layer. **Part B** (breath-hold, small-speaker mode, WAV
export) is three small tools. **Part C** (real reference recordings) ships third-party audio and therefore has its own rules
and a stop for the maintainer's approval; it can be done last or not at all without affecting A and B.

## Goal
Turn the sounds into a course. The evidence the research found is specific: baseline auscultation skill is poor at every level
(residents recognised 20 % of recorded events); what works is **repetition** (500 listens per murmur took students from about
20 % to 85 %, unsupervised as well as supervised), a **synchronised trace with a moving cursor** (a modest, real gain), and
**self-testing with features before diagnosis**. So: a trainer that loops single findings and counts listens, a quiz that
plays a clip with the trace hidden and asks what was heard before asking what it is, and a tutorial that leads into both.
What heartsim adds that recordings cannot is the link to the rhythm: hear S1 fade as the PR lengthens, the split reverse when
LBBB appears.

## Read first
`docs/research/heart-sounds.md` ("Repetition, a synchronised trace and self-testing…", "Only a thin set of recordings clears
the licence policy", "Recommended design": teaching layer, recordings) and `docs/research/heart-sounds-notes/
{conditions_teaching,datasets_licences}.md`; the briefs B24, B25, B27; `crates/heartsim-app/src/rhythm_quiz.rs` (the pattern to
copy: `Question`, `make_question`, `explanation`, score persistence, `confused_with`), `interval_drill.rs`, `tutorial_ui.rs`,
`nav.rs`; the tutorial format and its actions and checks in `heartsim-core`, `assets/tutorials/*/tutorial.ron`, and
`heartsim-cli run-tutorial` (`crates/heartsim-cli/src/content.rs`); `crates/heartsim-app/src/heart_sound_player.rs` (slice
playback) and `sound_out.rs`; `crates/heartsim-io/src/` (the export modules); `ASSETS-LICENSES.md`; `docs/web.md` (size budget,
what the web build leaves out).

## Non-negotiables for this brief
* **Works without sound.** Every page here must be usable with audio unavailable or switched off (no device, CI, a silent
  classroom, a hearing-impaired learner): the phonocardiogram is then shown instead of hidden, and the page says so. Nothing
  panics or blocks waiting for a device.
* **Honest labels.** Synthesised sounds are called models. Anything that alters the sound (slow motion, small-speaker mode) is
  labelled as altered where it is switched on. A real recording is labelled as such with its source.
* **Determinism.** Questions, clip choice and distractors come from a seeded generator, as in the Rhythm Quiz; a clip is an
  offline `record()` of a scenario, so the same seed gives the same quiz. ECG and PCG goldens are unchanged.
* **Legal (Part C).** Only CC0, public-domain and CC BY material ships, each file listed in `ASSETS-LICENSES.md` with author,
  source URL or DOI, licence, date retrieved, checksum and the changes made. The licence is read on the primary page at
  download time, not taken from the research note. ODC-By sets (CinC 2016, CirCor, EPHNOGRAM and the like), share-alike,
  non-commercial and unlicensed sources, and the manikin set (HLS-CMDS) do **not** ship under this brief. Nothing comes from
  `epicardio/`.
* All text is original, in en and it; tutorial text states its sources where it makes a clinical claim.

## Part A — the teaching layer
1. **Clips.** A `SoundClip` is an offline recording of a scenario (ECG + PCG from `Tier0Scenario::record`, B24) at a chosen
   listening site and chest piece (B27), a whole number of cardiac cycles long so that it loops without a click (cut at a quiet
   point in diastole; for irregular rhythms a fixed 8–10 s with a short fade). The player loops it through the existing sink
   with a playhead; the clip is the single source for the sound, the trace and the labels.
2. **Repetition trainer** ("Sound Trainer", Practise group). A list of single findings, each one clip: S1 and S2; physiological
   split; wide, fixed and paradoxical split; soft S1 (long PR); variable S1 (complete block); S3; S4; summation gallop; ejection
   click; mid-systolic click; opening snap; each murmur of B27; rub. For each: Play/Stop (looping), half speed (the wall-time
   synthesiser; labelled), trace shown or hidden, site and chest piece, a short description of what to listen for, and a
   **listen counter** (completed loops, persisted per finding, with the learner's total). **Contrast pairs** switch between two
   clips at the same point of the cycle with one key: innocent murmur against aortic stenosis; ejection against regurgitant
   systolic murmur; split S2 against S2 + opening snap against S2 + S3; S4 + S1 against split S1.
3. **Auscultation quiz** (Practise group; copy the structure of `RhythmQuiz`). A question plays a looping clip with the trace
   and the scenario name hidden. Step 1, features: the learner ticks what they hear (extra sound before S1 / after S2, split
   S2, click, systolic murmur, diastolic murmur, timing early / mid / late / throughout) and may move the stethoscope between
   sites. Step 2, diagnosis: four choices from the scenario's `confused_with` and the other auscultation scenarios. Feedback
   shows the trace with its labels, the ECG above it, which features were right, and the teaching text; "Open in simulator"
   as in the Rhythm Quiz. Score (features and diagnosis separately) persists. Uses the `ExpectedFindings` auscultation fields
   added in B27; add `confused_with` entries to the B27 scenarios where missing.
4. **Tutorial** (`assets/tutorials/11_heart_sounds/`, and `12_murmurs/` if one grows too long). Opens with headphone advice and
   a **sound check**: a low tone (200 Hz, as in the Watsjold module) and a higher one; if the learner cannot hear the low one
   they are told the speakers are too small and pointed to headphones and to the trace. Then: where S1 and S2 sit against the
   QRS and the T wave; the split and breathing; what the rhythm does to S1 and S2 (PR, bundle-branch block, complete block, AF);
   gallops; systolic against diastolic murmurs; where to listen. Each page drives the simulator through tutorial actions
   (new ones as needed: switch the sound on, choose site and chest piece, set the speed, show or hide the lane) and ends with a
   check the learner can pass by ear or by eye. `heartsim-cli run-tutorial` must run it headless: audio actions are no-ops
   there and the checks do not depend on a device.
5. **Navigation and docs.** The two new pages join the Practise group in `nav.rs` (tabs, group menu, shortcuts; the narrow
   layout still fits). User book: a page for the trainer and the quiz, the tutorial listed; `CHANGELOG.md`; phase 5 ticked in
   `docs/ROADMAP.md`; the README's feature list.

## Part B — three small tools
6. **Breath-hold.** A control (and a tutorial action) that holds the respiratory phase at end-expiration or peak inspiration,
   so a split can be studied without waiting for the breath. It is a scenario-timeline action in the engine, not a display
   trick: the sinus-arrhythmia modulation freezes with it, runs are reproducible, and scenarios that do not use it are
   bit-identical to today. Other manoeuvres (Valsalva, squatting, handgrip) need haemodynamics the simulator does not have and
   stay out of scope; the user text says so.
7. **Small-speaker mode.** Off by default; multiplies the frequency of every component by a designed factor so that low sounds
   (S3, S4, rumble) become audible on laptop and phone speakers. Applies to the sound only: the lane keeps the true signal and
   shows an "altered sound" tag. The setting's text says that this changes how the sounds sound and has no proven teaching
   effect.
8. **WAV export** (`heartsim-io`, with the existing `export` feature; absent on the web like the other exports). Export the
   heart-sound signal of the current recording or of a clip as 16-bit mono WAV at 44.1 kHz (the B25 resampler), with the
   scenario name, site and "synthesised by heartsim" in the file's metadata chunk; listed in the Export window and in
   `docs/book/src/user/exports.md`.

## Part C — real reference recordings (stop for approval before committing audio)
9. **Candidate list first.** Produce `docs/research/heart-sound-clips.md`: for each candidate clip the source page, author,
   licence as read on the primary page today, what it contains, length, quality, and the planned edit. Candidates named by the
   research: Wikimedia Commons CC0 / public-domain normal heart sounds; the paediatric clips of Kudriavtsev et al. 2007
   (CC BY 2.0: Still's murmur, prolapse clicks, aortic and pulmonary stenosis, VSD); the JUST regurgitation dataset (Mendeley,
   CC BY 4.0: aortic, mitral, tricuspid regurgitation); SensSmartTech (PhysioNet, CC BY 4.0: normal, with simultaneous ECG).
   **Stop here and hand the list to the maintainer.** Do not add audio files to the repository until the list is approved.
10. **After approval.** Convert each approved clip to 16-bit mono PCM WAV at 4 kHz, trimmed to a few cycles (a decoder written
    in the app for that one format; no new codec dependency), under `assets/sounds/reference/` with a RON index (title,
    condition, source, licence, attribution text). Add every file to `ASSETS-LICENSES.md`. The trainer shows a "Real recording"
    button beside the synthesised finding where a clip exists, with the attribution visible; the clip has no ECG and no labels
    unless the source provides them. The web size budget (`cargo xtask web-budget`) still passes.
11. **Timing validation (no shipping needed).** A test-only or xtask tool that, given a local copy of SensSmartTech, reports
    QRS-to-S1 and T-end-to-S2 against the model across heart rates; its result goes into `docs/research/`. Skipped when the
    data is not present.

## Out of scope
Scoring against a class or a teacher's dashboard; spaced-repetition scheduling; manoeuvres other than the breath-hold; recording
the learner's own stethoscope; any clip whose licence is not on the list above.

## Acceptance tests
* Clips: a clip of a regular rhythm loops with no discontinuity larger than the neighbouring sample steps; same scenario,
  site and seed give the same clip; irregular rhythms get the fixed length with fades.
* Trainer (egui_kittest and logic tests with the fake sink): the counter rises by one per completed loop and not on a stopped
  one, persists and restores; contrast-pair switching keeps the cycle position; half speed is labelled; with no sink the trace
  cannot be hidden and the page says why.
* Quiz: deterministic questions from a seed; only auscultation scenarios are drawn; the trace and the scenario name are hidden
  until both steps are answered when sound is available, and shown when it is not; feature scoring matches
  `ExpectedFindings`; four distinct choices including the answer; score round-trips; the Rhythm Quiz pool is unchanged.
* Tutorial: `heartsim-cli run-tutorial` passes for the new tutorial(s) headless; the tutorials test lists them; every page's
  action is understood by the app; the sound check works with the fake sink and degrades to text with none.
* Breath-hold: with the hold at end-expiration the normal split stays under 30 ms beat after beat and at peak inspiration it
  stays 30–60 ms; the RR interval stops varying with breathing while held; a scenario without the action gives bit-identical
  ECG and PCG to today (all goldens unchanged).
* Small-speaker mode: the played signal's spectral centroid for S3 rises by the designed factor; the lane's samples are
  unchanged; the tag is shown; the setting defaults to off and round-trips.
* WAV export: the file parses back to the same samples within 16-bit rounding, has the right rate, length and metadata; absent
  from the web build.
* Navigation: the nav tests cover thirteen pages; the narrow layout shows the group menus without overflow.
* Part C: the index and `ASSETS-LICENSES.md` list exactly the files in `assets/sounds/reference/` (a test compares them); every
  entry has author, source, licence, retrieval date and checksum, and the checksum matches; `cargo xtask web-budget` passes.
* The wasm32 check of the pure crates (by hand), `cargo check -p heartsim-app --no-default-features --lib` and `cargo xtask ci`
  pass.

## Decisions for the maintainer
* Part C: whether to ship real clips at all, and the approved list. The earlier open questions stay open and outside this
  brief: the ODC-By policy for the PhysioNet sets, and the legal check on the manikin set.
* The small-speaker factor and whether that mode should exist, given that it has no evidence of a learning effect.
* Whether the quiz should allow slow motion while a question is open (it helps, and it changes what is being tested).
