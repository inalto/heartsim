# B24 — Silent phonocardiogram (heart sounds, phase 1)
**Crates** `heartsim-ecg`, `heartsim-sim`, `heartsim-ui`, `heartsim-app` · **Model** Opus · **Milestone** M9 (audio, phase 1 of 5) ·
**Depends on** B1, B4, B7, B11.

## Goal
A deterministic phonocardiogram (PCG) trace, synthesised from the same `TimingEngine` events as the ECG and drawn under the strip on
the same time axis: S1 (M1 + T1) and S2 (A2 + P2) for every existing scenario, with the rhythm-dependent behaviour a student should
learn (S1 loudness follows the PR interval, S2 splitting follows the order of ventricular activation). **No audio device code in
this brief**: the trace is the single source the later audio phases will play, so the physiology is settled in golden tests first.

## Read first
`docs/research/heart-sounds.md` (sections "Timing against the ECG", "Loudness and splitting", "Recommended design", "What could not
be verified"); `crates/heartsim-sim/src/engine.rs` (`Event`, `EventKind`, `Circuit`); `crates/heartsim-ecg/src/synth.rs`
(`GaussianSynth::on_events`, `sample`, `event_noise`: the pattern to copy); `crates/heartsim-ecg/src/ring.rs` (`LeadRing`);
`crates/heartsim-ecg/src/scenario.rs` (`Tier0Scenario::record`, `Recording`); `crates/heartsim-app/src/app.rs`
(`step_simulation`); `crates/heartsim-ui/src/ecg_view.rs`, `strip.rs`.

## Non-negotiables for this brief
* Pure and deterministic: the `pcg` module has no I/O, no audio dependency, no wall clock; it compiles on `wasm32`. Randomness is a
  pure function of scenario seed and event time (as `event_noise`).
* One source of truth: the PCG is derived from the engine's events only. It never feeds back into the engine or the ECG.
* Every numeric parameter lives in one documented table (`PcgParams`, with a `Default`) and its doc comment says either the source
  (author, year, the value) or the word **designed**. Values the research could only see as search extracts are marked
  "unverified" until someone re-reads the paper (see the list at the end).
* No recordings are used or shipped. Nothing comes from `epicardio/`.

## Scope
1. **Beat assembler** (`heartsim_ecg::pcg::BeatAssembler`). Consumes `&[Event]` in engine order and emits one `Beat` per ventricular
   activation: QRS onset time; time of the last conducted atrial activation and the resulting atrium-to-ventricle interval (`None`
   when the atria are fibrillating, fluttering or silent, or the beat is not preceded by an atrial activation within 400 ms);
   preceding RR; origin (sinus/atrial/junctional/ventricular/paced, from `Event::origin`); which ventricle is activated first and
   the interventricular delay in ms (from the bundle-branch `Conducted`/`Blocked` events, the ectopic or paced site); whether an
   organised ventricular beat exists at all (`Circuit::VentricularFibrillation` → none).
   The decision "which branch is blocked / which ventricle leads" is today private to `GaussianSynth`. Move it into one small shared
   function used by both, so the strip's QRS morphology and the PCG's split direction can never disagree; the ECG output must stay
   bit-identical (all golden ECG snapshots unchanged).
2. **Electromechanical model** (`pcg::schedule(&Beat, &PcgParams, resp_phase) -> Components`). Rules, with their sources in the
   research note:
   * M1 at QRS onset + 40 ms (LV-first or normal activation); when the LV is activated late (LBBB, RV pacing, RV ectopy) M1 is
     delayed by a fraction of the interventricular delay (designed; Felner: LBBB does not split S1).
   * T1 = M1 + 25 ms normally; up to + 60 ms when the RV is activated late (RBBB, LV pacing, LV ectopy).
   * A2 at QRS onset + QS2, QS2 = 546 − 2.1 × HR ms with HR from the **preceding** RR (clamped to 40–180 bpm); delayed with late LV
     activation.
   * P2 = A2 + base split + respiratory term + (RV delay − LV delay). Base split 10 ms in expiration rising to 40 ms at peak
     inspiration. A negative result is a paradoxical split: P2 before A2, widest in expiration.
   * S1 gain from the atrium-to-ventricle interval: maximal at 80–120 ms, falling to soft beyond 200 ms, a designed mid value when
     there is no atrial contraction (AF) scaled by the preceding RR (short cycle → louder; designed).
   * No components for wavelets of ventricular fibrillation or during asystole; a premature ventricular beat coupled closer than
     a designed threshold produces S1 only (the ventricle ejects nothing).
3. **Respiratory phase.** `SinusParams` already modulates the sinus rate with `respiration_depth` / `respiration_rate_bpm`. Expose
   the phase (0–1, inspiration/expiration) from `TimingEngine` as a pure function of simulation time so that inspiratory splitting
   and sinus arrhythmia share one clock. With `respiration_depth = 0` the phase still runs (people breathe); document that.
4. **Synthesis** (`pcg::PcgSynth`, API parallel to `GaussianSynth`: `on_events(&[Event])`, `sample(t: Millis) -> f32`). Fixed
   internal rate **4000 Hz**. Each component is a closed-form atom: valve closures as short falling-frequency chirps under a smooth
   envelope (A2 about 250 → 50 Hz and P2 about 200 → 50 Hz over 60 ms, after the Xu–Durand–Pibarot trajectory as restated by
   Djebbari & Bereksi-Reguig 2013; M1/T1 designed in the same family, lower and longer so that S1 ≈ 120 ms and S2 ≈ 95 ms overall).
   Small seeded beat-to-beat jitter of amplitude (designed, a few per cent). Output in arbitrary units, normal S1 peak = 1.0.
5. **Recording and ring.** `Recording` gains `pcg: Vec<f32>` and its sample rate; `Tier0Scenario::record` fills it. The app keeps a
   PCG `LeadRing` (4000 Hz, same seconds as the ECG rings), fed in `step_simulation` next to the ECG sampling, and also in the EP-lab
   and re-entry step loops if they produce ventricular events (if they cannot yet, leave the ring empty there and say so).
6. **Display.** A "Heart sounds" toggle in the strip's Display popup (off by default, persisted, i18n en + it) adds a PCG lane under
   the ECG strip on the Simulator and under the rhythm strip on the 12-lead page: same time axis, scrub, sweep/scroll and pause
   behaviour as the strip, drawn as an envelope-filled trace with small `S1` / `S2` labels (and `A2` `P2` when the split exceeds
   20 ms) from the component schedule, not from peak-picking. Calipers work across both lanes. The lane is omitted, not blank, where
   there is no PCG.
7. **Docs.** `docs/book/src/user/` page section on the heart-sound trace (what S1/S2 are, what the trace shows, that it is a model
   and not a recording); `CHANGELOG.md` under Unreleased; `docs/web.md` no longer says the app has no sound code path (the trace is
   silent, the desktop beep exists).

## Out of scope (later phases)
Audio output on desktop (phase 2) and web (phase 3); S3, S4, clicks, snaps, murmurs, the `auscultation:` scenario section,
listening sites and valve highlights (phase 4); tutorial, quiz, repetition trainer, reference recordings (phase 5).

## Acceptance tests
* Golden PCG snapshots (`crates/heartsim-ecg/tests/`, decimated envelope, same style as the golden ECG tests) for `sinus_normal`,
  first-degree AV block, complete AV block, RBBB, LBBB, atrial fibrillation, a ventricular-paced scenario and `vf_coarse`.
* Determinism: the same scenario and seed give bit-identical PCG samples on two runs, and sampling in one call or in frame-sized
  chunks gives the same samples. The module compiles for `wasm32-unknown-unknown` (covered by `cargo xtask ci`'s wasm check).
* Timing: in sinus scenarios at 50, 75 and 100 bpm, A2 onset is within ± 40 ms of the T-wave end found by the B4 analysis, and
  QRS-onset-to-M1 is 40 ± 5 ms.
* S1 gain is non-increasing in the atrium-to-ventricle interval from 120 ms to 300 ms; in complete AV block the S1 peak varies by
  at least a factor of 2 across 20 beats; in sinus rhythm it varies by less than 10 %.
* Splitting: RBBB — P2 after A2 in both respiratory phases, wider in inspiration; LBBB and RV pacing — P2 before A2 in expiration
  and the split narrows in inspiration; normal — split under 30 ms in expiration, 30–60 ms at peak inspiration.
* VF and asystole: the PCG is zero (below 1 % of a normal S1) after the last organised beat's sounds have ended.
* Every golden ECG snapshot is unchanged.
* UI (egui_kittest): the toggle adds the lane; with it off the strip is pixel-identical to before; the lane's `S1` label sits
  between the QRS and the T wave of the same beat.
* `cargo xtask ci` passes.

## To verify before the parameters are called sourced
Re-read in the papers themselves (the research saw only extracts): the QS2 regression intercepts (Weissler 1968 / Boudoulas 1979),
the 40 ms Q–S1 figure and the PR-to-loudness relation (Leech 1980; Burggraf & Craige 1974), and the chirp constants of Xu, Durand &
Pibarot (IEEE TBME 2001). Timing can then be validated against the SensSmartTech dataset (PhysioNet, CC BY 4.0, simultaneous ECG
and PCG) without shipping any of it.
