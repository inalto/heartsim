# Heart-sound audio in heartsim: what the codebase offers and which audio technology fits

Scope: read-only study of `/home/alain/heartsim` (state of `master` at 5a1e6da plus uncommitted work) and of the audio stack.
No build or test was run. Paths are relative to the repository root unless absolute. "Registry copy" means the crate source
that cargo already downloaded to `~/.cargo/registry/src/*/` (kira-0.12.6, cpal-0.18.2); it is the exact code the app links, so
it was preferred over web pages for kira/cpal facts. Canonical online locations: https://docs.rs/kira/0.12.6 and
https://docs.rs/cpal/0.18.2.

Corrections to the assignment's premises, found while reading:
- The UI speed range is **0.05x-2x**, not 0.1x-10x (`crates/heartsim-ui/src/transport.rs:10`, `SPEED_RANGE = 0.05..=2.0`;
  tutorials: `UiAction::SetSpeed` "0.05-2", `crates/heartsim-core/src/tutorial.rs`).
- The ECG rings are at **500 Hz** (confirmed, `crates/heartsim-app/src/app.rs:38`).
- `docs/web.md:41-42` still says "the app has no sound yet"; it is stale (the beep exists on desktop, CHANGELOG.md:253-255).

---

## 1. What does the TimingEngine emit per beat, at what resolution, and what is missing for a heart-sound scheduler?

### Takeaway
The engine emits a flat, chronological stream of `Event { at, region, kind, origin, chamber }` with exact `f64` millisecond
timestamps (delivered in 1 ms steps), from which atrial activation, His activation, each bundle branch (conducted or blocked) and
ventricular activation can be read; there is **no per-beat record** (PR, RR, QRS width, AV relation) and **no mechanical or
haemodynamic quantity at all**, so a heart-sound scheduler needs a small "beat assembler" plus an electromechanical model on top.

### Cited Findings
- Event type: `pub struct Event { at: Millis, region: Region, kind: EventKind, origin: Origin, chamber: Chamber }`; doc comment
  says it is "the audit trail the UI uses for beeps, markers and ladder diagrams" — `crates/heartsim-sim/src/engine.rs:138-148`.
- `EventKind = Automatic | Conducted | Stimulus | Blocked` — `crates/heartsim-sim/src/engine.rs:26-36`.
- `Origin = Sinus | Focus(u8) | AccessoryPathway | Reentry | Escape | Pacer(u8: 0 atrial, 1 RV, 2 LV) | External |
  Circuit(Circuit)`; `Circuit = AtrialFlutter | AtrialFibrillation | VentricularFibrillation | Torsades | PolymorphicVt` —
  `crates/heartsim-sim/src/engine.rs:38-112`. Conducted events carry the origin of the impulse they propagate (same file, :90-91).
- `Chamber = Atrium | AvJunction | Ventricle`, derived from the region — `crates/heartsim-sim/src/engine.rs:114-136`.
- `Region` has 27 values incl. `SinoatrialNode, RightAtrium, LeftAtrium, AvNodeCompact, HisBundle, RightBundleBranch,
  LeftBundleBranch, LeftAnteriorFascicle, LeftPosteriorFascicle, RightVentricle, LeftVentricle, VentricularSeptum,
  AccessoryPathway` — `crates/heartsim-core/src/region.rs:11-44`.
- Resolution: time is `Millis(f64)` (`crates/heartsim-core/src/time.rs:5-9`). All happenings go through one time-sorted queue
  (`Scheduled { at: f64, what: Pending }`), and `step(dt)` dispatches every item with `at <= now` using the item's own `at`, so
  event timestamps are sub-step exact even though they are delivered once per fixed step —
  `crates/heartsim-sim/src/timing/mod.rs:27-30, 180-184, 884-895`. The app clock is `SimClock::new(Millis(1.0))`
  (`crates/heartsim-app/src/app.rs:280`); Tier 2 uses 0.1-0.5 ms (`crates/heartsim-core/src/time.rs:60`).
- What is emitted along one conducted beat (all via `TimingEngine::emit`, `crates/heartsim-sim/src/timing/mod.rs:491`):
  sinus block (`SinoatrialNode`, Blocked, conduction.rs:128); atrial activation or atrial refractory block (conduction.rs:180,
  187); AV-node block (`AvNodeCompact`, Blocked, conduction.rs:256, 284); AV pathway used (`AvNodeFastPathway`/`SlowPathway`,
  Conducted, conduction.rs:294); His (`HisBundle`, Conducted, conduction.rs:327) or Mobitz-II drop (`HisBundle`, Blocked,
  conduction.rs:50); each bundle branch / fascicle as Conducted or Blocked (conduction.rs:51-61); accessory-pathway block
  (conduction.rs:245, 524); ventricular activation or refractory block (conduction.rs:448-479); escape beats
  (`Origin::Escape`, conduction.rs:666, 838); stimuli (`EventKind::Stimulus`, conduction.rs:727).
- A ventricular activation is the "QRS onset": `WaitEvent::Qrs` = "The next ventricular activation (QRS onset)" —
  `crates/heartsim-core/src/tutorial.rs` (enum `WaitEvent`).
- Sustained arrhythmias emit one event per circuit revolution / fibrillatory wavelet with `Origin::Circuit`; in VF "the
  ventricles have no organised beat" — `crates/heartsim-sim/src/engine.rs:38-53`, `timing/mod.rs:16-21`.
- Pacing: besides `Origin::Pacer(ch)` events there is a marker channel `drain_pacer_markers() -> Vec<PacerMarker>` (every
  stimulus with capture result, every sense) — `crates/heartsim-sim/src/timing/mod.rs:388-393`.
- Ladder samples are the closest thing to per-beat intervals: `LadderSample { chamber, enter_ms, exit_ms, direction, blocked,
  origin }`, one per row traversal, drained with `drain_ladder()` — `crates/heartsim-sim/src/timing/ladder.rs:21-36`,
  `timing/mod.rs:383-386`. The ventricular row is pushed with a **fixed** width `fusion_window_ms` (default 120 ms), not the
  real QRS duration — `crates/heartsim-sim/src/timing/conduction.rs:476-478`, `timing/params.rs:78`.
- State the engine has but does not publish per beat: `last_v_rr_ms`, `last_a_cycle_ms`, `rr_history: [f64; 3]`,
  `last_vent_via: Option<VentVia>` (HisPurkinje / Accessory / Focus / Escape / Pacer / PacerLv / External / Tissue / Circuit),
  fusion detection — all private or `pub(crate)` — `crates/heartsim-sim/src/timing/mod.rs:67-82, 285-325`,
  `conduction.rs:455-475`. A coarse `rhythm_class(t) -> RhythmClass` (incl. `Asystole`) is public (`timing/mod.rs:595`,
  `crates/heartsim-sim/src/defib.rs:59`).
- QRS width and bundle-branch-block morphology are decided in the **ECG synthesiser**, not the engine: `GaussianSynth` collects
  `Blocked` branch events into a `branch_blocks` bit set until the next His event and picks a `Template`; it tracks its own
  `last_beat_ms` / `last_rr_ms` for QT adaptation — `crates/heartsim-ecg/src/synth.rs:5-9, 509-544, 651-687, 709-713`.
- Respiration exists only as a sinus-rate modulation parameter (`SinusParams.respiration_depth`, `respiration_rate_bpm`), with
  no published respiratory phase — `crates/heartsim-sim/src/timing/params.rs:98-105`.
- No mechanics anywhere: a repository-wide search for contraction, valve motion, systole/diastole, pressure, haemodynamics,
  murmur, auscultation, phonocardiogram in `crates/`, `docs/ARCHITECTURE.md`, `docs/REQUIREMENTS.md`, `docs/ROADMAP.md`,
  `docs/briefs/`, scenarios, tutorials and English locales found only electrophysiology uses ("diastolic interval", AP
  upstroke) and render-only valve meshes. The only sound requirement is F-32 "DC cardioversion/defibrillation ... animation and
  sound" — `docs/REQUIREMENTS.md:65`.

### Inferences
- A consumer can already derive per beat, from the event stream alone and without touching `heartsim-sim`: P time (atrial
  event), origin of the beat (`Origin`), PR of this beat (atrial event to ventricular event of the same impulse; or the
  ladder samples), preceding RR (difference of ventricular events, as `GaussianSynth` does), which branch blocked (Blocked
  events between His and ventricle), paced chamber (`Origin::Pacer(ch)`), pre-excitation (`Origin::AccessoryPathway`),
  absence of organised beats (`Origin::Circuit(VentricularFibrillation)`), absence of atrial contraction (atrial
  `Origin::Circuit(AtrialFibrillation | AtrialFlutter)`).
- Missing for heart sounds, to be added as a pure layer above the engine (not inside it):
  1. A **beat record** (`Beat { p_ms: Option, qrs_onset_ms, pr_ms: Option, rr_prev_ms, origin, branch_blocks, paced,
     preexcited, fusion, atrial_kick: bool }`). AV dissociation is not flagged; it must be inferred (ventricular beat whose
     origin is Escape/Focus/Pacer while atrial events continue, or a varying P-to-QRS interval).
  2. **QRS duration / ventricular activation sequence** (LV-first vs RV-first), needed for split S1/S2 and paradoxical
     splitting in LBBB/RV pacing: only obtainable today by re-implementing the synth's template choice, or from Tier 1
     activation times in `HeartView`'s `EikonalEngine` (not available in Tier 0 runs). Cleanest: move the "branch_blocks ->
     conduction pattern" classification into a small shared function used by both the ECG synth and the sound synth.
  3. **Electromechanical model**: electromechanical delay, isovolumic contraction, ejection time (rate-dependent, like the
     existing QT adaptation with `qt_rate_exponent`), filling time = RR - systole (S3 presence, diastolic murmur length),
     PR-dependent S1 intensity, respiratory phase for physiological S2 splitting. None exists.
  4. A **respiratory phase** output if S2 splitting is to follow breathing (today only the rate modulation exists).
- Because event times are exact floats, the scheduler can place sounds with sub-millisecond precision in simulation time; the
  limiting factor is the frame-quantised delivery to the app and the audio output path, not the engine.

### Gaps
- Physiological values for the electromechanical delays (QRS-to-S1, QT-to-S2, etc.) were out of scope here and are not sourced
  in these notes.
- Whether `PacerMarker`/`LadderSample` timestamps always precede the corresponding `Event` in the same step was not traced line
  by line.

---

## 2. How are scenarios described, is there any structural/valvular/haemodynamic notion, and how would a valve lesion be added?

### Takeaway
Scenarios are single RON files (`Scenario(format_version: 1, ...)`) with engine sections (`timing`, `morphology`) riding along as
extra fields that each engine crate deserialises separately; there is **no valve disease, haemodynamics or auscultation
concept**, and the established pattern for adding one is a new optional engine section (e.g. `auscultation: (...)`) plus,
if it must change over time, a new `Action` variant.

### Cited Findings
- `SCENARIO_FORMAT_VERSION = 1`; `Scenario` fields: `format_version, id, title, description, tier, anatomy, parameters,
  ablated_cells, timeline: Vec<TimedAction>, electrodes_mm, extra_electrodes_mm, seed, notes, confused_with, references,
  expected: Option<ExpectedFindings>`; most are `#[serde(default)]` — `crates/heartsim-core/src/scenario.rs:12, 181-224`.
- "Engine-specific sections (`timing` ... `morphology` ...) live in the same file; this crate cannot name their types
  (layering) and ignores them, `heartsim_sim` / `heartsim_ecg` read them from the same text" —
  `crates/heartsim-core/src/scenario.rs:178-180`; test `minimal_file_uses_defaults_and_ignores_engine_sections` (:321-344).
  Implementation: `TimingParams::from_scenario_ron(text)` = `ron::from_str::<TimingSection>(text)?.timing`
  (`crates/heartsim-sim/src/timing/params.rs:63-68`); `Tier0Scenario::from_ron` parses `Scenario`, `timing`, and
  `MorphologySection` from the same text (`crates/heartsim-ecg/src/scenario.rs:62-91`). No `deny_unknown_fields` exists in
  any crate (grep).
- `Action` vocabulary: `SetGlobalSinusRate, SetRegion, Block, Stimulate, PacingTrain, Ablate, AccessoryPathway, Stenosis
  { branch, severity }` (a **coronary** branch stenosis, 0-1), `Shock, Drug { name, dose }, MoveElectrode` —
  `crates/heartsim-core/src/scenario.rs:26-87`. The timing engine rejects actions it cannot honour with
  `ActionError::Unsupported(&'static str)` (`crates/heartsim-sim/src/timing/mod.rs:212-226`).
- `ExpectedFindings { hr_bpm, pr_ms, qrs_ms, axis_deg, rhythm: Option<RhythmKind> }` drive golden tests and quiz answers —
  `crates/heartsim-core/src/scenario.rs:127-149`.
- Scenario library (70 files in `assets/scenarios/`, plus `ep/` with 5 and `reentry/` with 1): accelerated_junctional,
  af_preexcited, aivr, atrial_fibrillation, atrial_tachycardia_150, avb_1st, avb_2nd_mobitz1, avb_2nd_mobitz2_3to1,
  avb_2nd_mobitz2_4to3, avb_3rd_junctional, avb_3rd_ventricular, avnrt_150, avrt_orthodromic, axis_left_deviation,
  axis_right_deviation, bifascicular, flutter_2to1, flutter_4to1, flutter_variable, idioventricular, junctional_rhythm, lafb,
  lbbb, lpfb, mi_anterior_evolved, mi_evolution_stages, mi_inferior_evolved, mi_lateral_evolved, mi_old_anterior,
  nstemi_subendocardial, nsvt_runs, pac_bigeminy, pac_blocked, pac_frequent, paced_aai_sinus_brady, paced_biv, paced_ddd,
  paced_loss_of_capture, paced_pmt, paced_t_wave_oversensing, paced_undersensing, paced_vvi, pericarditis, polymorphic_vt,
  pvc_bigeminy, pvc_couplets, pvc_multifocal, pvc_parasystole, pvc_r_on_t, rate_dependent_rbbb, rbbb, sa_exit_block,
  sick_sinus_tachy_brady, sinus_arrest_junctional_escape, sinus_arrhythmia, sinus_brady_45, sinus_normal, sinus_tachy_130,
  stemi_anterior, stemi_evolving, stemi_inferior, stemi_lateral, stemi_posterior, torsades, vf_coarse, vf_fine,
  vt_monomorphic, wpw_left_lateral, wpw_right_type_b — directory listing of `assets/scenarios/`.
- Example file shape: `assets/scenarios/sinus_normal.ron` (id, title, description, tier, anatomy, seed, notes with a
  `Sources:` block, confused_with, expected, `timing: TimingParams(sinus: SinusParams(rate_bpm: 72.0))`);
  `assets/scenarios/avb_3rd_junctional.ron` (`av: AvParams(complete_block: true)`, AV dissociation described in the text).
- Existing diseases are electrical or ischaemic/pharmacological only: ischaemia (`crates/heartsim-sim/src/ischaemia.rs`),
  drugs (`pharma.rs`), defibrillation (`defib.rs`), pacing (`timing/pacer.rs`), pericarditis and MI as ECG morphologies.

### Inferences
- In keeping with the format, a valve lesion / auscultation finding would be a third engine section, read by the crate that owns
  the sound synthesiser, e.g.
  `auscultation: Auscultation(lesions: [AorticStenosis(severity: 0.6)], s3: true, s4: false, split_s2: Physiological)`,
  defaulting to "normal S1/S2" when absent. Because unknown fields are ignored by the other readers and the section is
  optional, `format_version` can stay 1 (older apps would silently ignore the section; bump only if silent ignoring is
  judged unsafe).
- Purely electrical consequences need no authoring: the sound synth can derive from the existing events that e.g. `lbbb`
  gives paradoxical S2 splitting, `avb_3rd_*` gives varying S1 intensity / cannon sounds, `atrial_fibrillation` has no S4
  and beat-to-beat varying S1, `vf_*` is silent, `pericarditis` could carry a rub. So all ~70 existing scenarios get plausible
  auscultation for free, and new "valve" scenarios (aortic stenosis, mitral regurgitation, ...) are `sinus_normal` plus an
  `auscultation` section.
- For findings that change on the timeline (a murmur appearing with papillary-muscle ischaemia, a manoeuvre) a new
  `Action::SetAuscultation { .. }` (or `Action::Manoeuvre`) variant in `heartsim-core` fits the "actions are data" rule and
  automatically works in tutorials (`Step::Sim(Action)`); `TimingEngine::apply` must then ignore it or return
  `Unsupported`, and the app must route it to the sound layer before logging a warning.
- `ExpectedFindings` could gain optional auscultation fields (e.g. `murmur: Option<MurmurKind>`), giving quiz answers and
  golden tests the same way `rhythm: Option<RhythmKind>` does.
- Legal constraint from CLAUDE.md non-negotiable 1 applies: no sound sample may come from `epicardio/`; fully procedural
  synthesis (as the beep already does: "no sound file, nothing taken from any product",
  `crates/heartsim-app/src/monitor_beep.rs:4-5`) avoids any asset-licence question.

### Gaps
- Whether the EP-lab (`assets/scenarios/ep/`) and re-entry scenario formats share the same section mechanism was not checked.

---

## 3. How does the existing QRS beep work (synthesis, trigger, flush, latency, speed, pause, scrub, web)?

### Takeaway
The beep is a code-synthesised 1 kHz / 100 ms tone played as a kira `StaticSoundData` "now" once per frame when a ventricular
event arrived; it is deliberately **not** sample-accurate (latency = up to one frame + audio buffer), is gated in simulation
time, has no speed/pitch handling, and is not compiled into the web build. Its structure (pure logic always compiled, a sink
trait, device code behind feature `audio`) is the template to follow for heart sounds.

### Cited Findings
- Synthesis: `synth_beep(sample_rate_hz) -> Vec<f32>`, 1000 Hz sine, 100 ms, 5 ms raised-cosine attack, 20 ms release, peak
  1.0, a pure function of the rate — `crates/heartsim-app/src/monitor_beep.rs:18-25, 42-69`.
- Output: `KiraBeepSink { manager: AudioManager<DefaultBackend>, sound: StaticSoundData }`, beep rendered once at 44 100 Hz
  ("kira does not report the device's rate; a static sound is resampled to the output rate on playback"), played with
  `self.manager.play(self.sound.volume(db))` and `AudioManagerSettings::default()` — `crates/heartsim-app/src/audio_out.rs:11-69`.
- Trigger: in the fixed-step loop every non-blocked ventricular event calls `self.beep.on_ventricular_event(e.at.0)` —
  `crates/heartsim-app/src/app.rs:375-384`; also in the Tier-2 re-entry loop (`crates/heartsim-app/src/app/reentry.rs:291-295`).
  The EP-lab loop records `last_v_activation` but does **not** feed the beep (`crates/heartsim-app/src/app/ep_lab.rs:233-238`).
- Flush: once per frame after stepping, `self.beep.flush(self.engine.now().0)` — `crates/heartsim-app/src/app.rs:919-923`.
  `BeepGate::take`: several events between flushes coalesce to the latest; a request older than `STALE_AFTER_MS = 250`
  (simulation ms) is dropped; one closer than `MIN_SPACING_MS = 180` (simulation ms) to the previous beep is dropped —
  `crates/heartsim-app/src/monitor_beep.rs:27-35, 71-124`.
- Stated latency: "The beep is requested in the frame in which the ventricle activates. Its latency is therefore up to one
  frame plus the audio buffer; it is not sample-accurate with the strip or the 3-D map" —
  `crates/heartsim-app/src/monitor_beep.rs:12-13`. Frame time comes from `ctx.input(|i| i.stable_dt).min(0.1)`
  (`app.rs:919`), and `SimClock::advance` caps at 200 steps per frame and then drops the backlog
  (`crates/heartsim-core/src/time.rs:84-97`).
- Speed != 1x: the gate is clocked in simulation time ("at 10x speed beeps can still follow each other ten times faster in
  wall-clock time", `monitor_beep.rs:27-31`); the tone itself is never stretched or re-pitched. `SimClock.speed` multiplies
  frame time (`time.rs:64, 86`).
- Pause: `if !self.ecg.is_paused() { self.step_simulation(..) }` then flush (`app.rs:920-923`): no steps, no events, no beeps;
  an already started beep plays to its end (nothing stops it).
- Scrub: scrubbing exists only while paused (`EcgView::scrub_offset_ms`, "How far the scrub cursor is behind the newest
  sample ... The 3-D view shows the state at `now - scrub_offset_ms()`", `crates/heartsim-ui/src/ecg_view.rs:89-93`); the
  beep has no scrub behaviour. Imported recordings under review mute the beep (`app.rs:894-902`).
- Device handling: off by default; the sink is created lazily the first time the beep is enabled; on failure one warning and
  silence for the rest of the run — `monitor_beep.rs:157-162, 219-250`. Tests use a fake `BeepSink` (`monitor_beep.rs:126-132`).
- Feature gating: `audio = ["native", "dep:kira"]`, not in `default` ("the release workflow switches it on. The web build
  never enables it") — `crates/heartsim-app/Cargo.toml:16-31, 61`; `#[cfg(feature = "audio")] pub mod audio_out;`
  (`crates/heartsim-app/src/lib.rs:8-9`) while `monitor_beep` is always compiled (`lib.rs:19`). Workspace dependency:
  `kira = { version = "0.12.6", default-features = false, features = ["cpal"] }` (`Cargo.toml:55`); Cargo.lock has kira
  0.12.6, cpal 0.18.2, web-sys 0.3.107, wasm-bindgen 0.2.130.
- Settings: `Settings { monitor_beep: bool (default false), beep_volume: f32 (default DEFAULT_BEEP_VOLUME = half scale) }`,
  UI section only under `#[cfg(feature = "audio")]`, with a "no sound device" hint when `beep_available == Some(false)` —
  `crates/heartsim-app/src/settings.rs:32-36, 71-86, 176-246`.
- Web: "QRS-synchronous monitor beep ... desktop only ... never in the browser build" (CHANGELOG.md:253-255); brief B21:
  "no `kira` audio (use web audio via `web-sys` beep)" (`docs/briefs/B21-wasm.md:9`); `heartsim-web` enables only the
  web-sys features `Window, Document, Element, HtmlElement, HtmlCanvasElement, Response, console`
  (`crates/heartsim-web/Cargo.toml:28-31`) — no Web Audio types.
- The stack research had recommended kira precisely for clock scheduling ("`StartTime::ClockTime` lets you schedule the beep
  for the exact simulated R-peak time a frame ahead", `docs/research/rust-stack.md:435-437`); the shipped beep does not use it.

### Inferences
- Worst-case beep latency at 60 fps is about 17 ms (frame) + kira command hand-over + the device buffer; with jitter of one
  frame. That is acceptable for a monitor beep but too loose and too jittery for S1/S2 pairs whose spacing carries the
  teaching content (a 17 ms jitter between S1 and S2 would be audible as rhythm irregularity, and split S2 components 30-60
  ms apart could not be rendered by per-frame triggering at all).
- The module split to reuse: (a) pure, always-compiled, unit-tested logic keyed on simulation time; (b) a small sink trait;
  (c) device implementation behind `audio`; (d) lazy device opening and silent failure; (e) "the simulation never depends on
  audio" (`monitor_beep.rs:7-8`).
- The beep and heart sounds should share one `AudioManager` (one device stream); `KiraBeepSink` currently owns its own.

### Gaps
- Actual measured output latency of the beep on the three desktop platforms is unknown (no measurement in the repo; kira
  exposes no latency figure, see section 7).

---

## 4. Could a phonocardiogram be synthesised like the ECG into a ring buffer so that drawing and audio share the samples?

### Takeaway
Yes: the ECG path (events -> `GaussianSynth::on_events` -> `sample(t)` -> `LeadRing::push`) is a direct model, and `LeadRing`
is a generic `f32` ring with its own `sample_rate_hz` and absolute sample indices; the only real difference is rate — the
ECG rings run at 500 Hz, whereas a PCG that is also the audio source needs roughly 4-8 kHz and must be resampled to the
device's 44.1/48 kHz.

### Cited Findings
- Pipeline per fixed step: `engine.step` -> `drain_events` -> `synth.on_events(&events)` -> heart view `on_step` -> every
  2 ms `synth.sample(engine.now())` -> `ring.push(v)` for 12 rings — `crates/heartsim-app/src/app.rs:364-401`.
- `SAMPLE_RATE_HZ: f32 = 500.0`; rings hold 60 s (`LeadRing::new((SAMPLE_RATE_HZ * 60.0) as usize, SAMPLE_RATE_HZ)`) —
  `crates/heartsim-app/src/app.rs:38, 203-204, 283-284`.
- `GaussianSynth` is event-driven: "each timing event spawns a set of Gaussian waves ... Because the waves are triggered by
  the shared timing engine, the strip stays consistent with any 3-D view"; randomness "is a pure function of the event's
  time (`event_noise`)", so "a scenario and its seed fix the strip" — `crates/heartsim-ecg/src/synth.rs:1-24`.
  `sample(t)` sums active waves `amplitude * exp(-dt^2 / 2 width^2)` and drops finished ones (`synth.rs:1053-1083`).
- `LeadRing { buf: Vec<f32>, head, len, total: u64, sample_rate_hz: f32 }` with `push`, `pushed()`, `oldest_index()`,
  `at(abs)`, `range(start, end, out)`, `latest(n, out)`, and `minmax_decimate(samples, columns)` "so that narrow QRS spikes
  are never lost when thousands of samples map to a few hundred pixels" — `crates/heartsim-ecg/src/ring.rs:1-141`.
- Offline rendering exists: `Tier0Scenario::record(seconds) -> Recording { leads, events }`, "a pure function of the file:
  fixed 1 ms step, no randomness, no I/O" (`crates/heartsim-ecg/src/scenario.rs:5-8, 68-131`); the rhythm quiz and interval
  drill build their strips from it (`crates/heartsim-app/src/rhythm_quiz.rs:88-110`).
- Architecture rule: "the ECG strip and the 3-D colour map are both derived from engine state so they can never disagree"
  (CLAUDE.md non-negotiable 3); `heartsim-app` is the documented home of "audio (kira)" (`docs/ARCHITECTURE.md:33`);
  `core`, `geometry`, `sim`, `ecg` must compile without wgpu/egui and on wasm32 (CLAUDE.md non-negotiable 4).
- Heart-sound energy bands reported by search results (not verified against the primary table): S1/S2 about 50-500 Hz, S3/S4
  about 20-200 Hz — search summary of [PMC6313612](https://pmc.ncbi.nlm.nih.gov/articles/PMC6313612) /
  [PMC7305673](https://pmc.ncbi.nlm.nih.gov/articles/PMC7305673) (which of the two carries the table was not confirmed).

### Inferences
- Recommended shape: a pure `PcgSynth` (new module in `heartsim-ecg`, or a new pure crate between `sim` and `render`) with
  `on_events(&[Event])` and `render(until_ms, out: &mut Vec<f32>)` at a fixed internal rate, deterministic (noise for
  murmurs from a seeded PRNG keyed on event time, as `event_noise` does). It needs no audio dependency and runs on wasm32,
  satisfying layering and determinism; golden PCG snapshot tests can sit next to the golden ECG tests. `Tier0Scenario`-style
  offline `record()` then yields a PCG for quizzes and exports (WAV export would belong in `heartsim-io`).
- Rate choice: murmurs need content to roughly 1 kHz or more, so an internal rate of 4 kHz (Nyquist 2 kHz) is the minimum and
  8 kHz comfortable. Memory for a 60 s ring: 4 kHz = 240 000 f32 = 0.96 MB; 8 kHz = 1.9 MB; 48 kHz = 11.5 MB. CPU: 4-8
  samples per 1 ms engine step, trivial next to Tier 1. Rendering directly at 48 kHz is also affordable for a handful of
  oscillators/filters but multiplies golden-file size and ties determinism to one rate; a fixed internal rate plus
  resampling at the device boundary keeps the simulation side device-independent.
- 48 kHz is an integer multiple of 4 and 8 kHz (x12, x6); 44.1 kHz is not, so the device side needs a fractional
  resampler (kira resamples `StaticSoundData` itself; a custom stream must do it, e.g. linear or cubic interpolation, which
  is adequate because the signal is band-limited far below Nyquist of the device rate).
- "One source of truth" then holds at 1x speed in the strict sense (the drawn PCG and the loudspeaker signal are the same
  samples, the latter merely interpolated). At other speeds see section 8: either the same samples are played at a
  different rate (pitch changes) or audio is re-synthesised with natural pitch (then the drawn trace and the audio agree in
  event timing but not sample-for-sample).
- The strip widget already handles pause/scrub/calipers on `LeadRing`s (`EcgView`), so a PCG trace under the ECG, time-aligned
  by absolute sample index (`LeadRing::starting_at`, ring.rs:29-37), is mostly a drawing task; min/max decimation is exactly
  what a PCG envelope needs at strip scale.

### Gaps
- How `strip.rs` / `ecg_view.rs` lay out multiple traces with different sample rates on one time axis was not read in detail.
- No measurement of murmur bandwidth requirements was done here; the 4-8 kHz figure rests on the assignment's ">= 4 kHz" and
  the unverified band table above.

---

## 5. Is there mechanical animation in the 3-D heart view, and where would valve / "which valve is sounding" highlights attach?

### Takeaway
The 3-D view is purely electrical (per-cell colour maps from the Tier-1 eikonal engine); there is no contraction or valve
motion, but valve geometry exists as flat-coloured, render-only meshes (`MeshKind::Valve`: `valve_rings` in the procedural
pack, `valve_planes` = four discs in the default pack) that could be tinted or pulsed, and glyph overlays offer a second hook.

### Cited Findings
- `HeartView` doc: "anatomy pack layers coloured from the Tier-1 eikonal engine, the colour-map legend, the anatomy panel and
  click-to-stimulate / ablate"; "The eikonal engine never decides when a beat happens: the app feeds it the timing engine's
  events every step (`HeartView::on_step`)" — `crates/heartsim-app/src/heart_view.rs:1-10, 476-490`.
- `enum MeshKind { Myocardium (coloured from cell state through hoods), Vessel (flat material), Valve ("Valve annuli") }` —
  `crates/heartsim-geometry/src/procedural/heart.rs:77-84`; valve rings are "tori (render-only)" (same file :6-8, :349,
  :1083-1085).
- Default pack: "`valve_planes` are the four valve-plane discs (tags 7-10)" and mesh files `mesh_<name>.glb` incl.
  `valve_planes` — `docs/anatomy-pack.md:40, 114`.
- Valve layers are flat-coloured and named: `"valve_rings" => t!("part-valve-rings")`, `"valve_planes" =>
  t!("part-valve-planes")`, colour `[0.8, 0.75, 0.55]` for `MeshKind::Valve`; vertex-to-cell interpolation is `None` for
  "flat-coloured meshes (vessels, valves, torso)" — `crates/heartsim-app/src/heart_view.rs:151, 242-255`.
- Overlay hooks already used by other features: `set_overlay_glyphs(Vec<GlyphInstance>)`, `set_catheter_glyphs`,
  `set_lead_glyphs` — `crates/heartsim-app/src/heart_view.rs:408-467`; `crates/heartsim-render/src/glyph.rs` exists.
- Scrub support for the heart: `set_scrub_cells(Option<&[f32]>)` and a `HeartHistory` "colour map over the last minute, shown
  at the strip cursor while scrubbing" — `heart_view.rs:552`, `crates/heartsim-app/src/app.rs:76-77`.
- A human-figure torso is now in the 3-D scene (commits 83c5012 "human figure as the 3-D torso", f47bcea "electrodes anywhere
  on the human figure, markers on its skin").
- Reduced-motion setting is forwarded to the heart panel (`app.rs:915-918`).

### Inferences
- "Which valve is sounding" is cheapest as (a) a per-valve tint/emissive pulse of the valve mesh — but `valve_planes` is one
  mesh for four discs, so per-valve highlighting needs either four meshes/sub-ranges or a per-vertex tag attribute (tags 7-10
  exist in the bake) — or (b) glyphs (rings/spheres) at four valve landmarks via `set_overlay_glyphs`, which needs no
  mesh change. Option (b) is the low-risk first step.
- The same mechanical-phase state that schedules the sounds (valve closed/open intervals per beat) should drive the highlight,
  so highlight and sound cannot disagree (rule 3), and it must honour `reduced_motion`.
- The electrode-marker-on-skin machinery of the human figure is a natural base for **auscultation sites** (aortic, pulmonic,
  Erb, tricuspid, mitral): a clickable "stethoscope" marker on the chest that selects the listening site and therefore the
  mix/gain of each sound. That makes the 3-D view part of the auscultation UI without any mechanical animation.
- True contraction animation would be a new render feature (vertex displacement), not required for heart sounds.

### Gaps
- Whether `valve_planes.glb` keeps per-disc tags as a vertex attribute or separate primitives was not inspected (binary
  asset). Whether the anatomy pack has named valve landmarks was not checked.

---

## 6. How could an auscultation quiz reuse the tutorial engine and the quizzes?

### Takeaway
Both mechanisms are data-driven and reusable almost as is: tutorials are RON pages with `Step::Sim(Action)` / `Step::Ui(UiAction)`
and an optional multiple-choice `Question`; the rhythm quiz draws a hidden scenario from the library, records it offline and
uses the scenario's `confused_with` list for distractors — an auscultation quiz is the same loop with a recorded PCG/audio
clip instead of (or next to) the strip.

### Cited Findings
- Tutorial model in `heartsim-core` (so desktop, web and `heartsim-cli run-tutorial` share it): `Tutorial`, `Page { title,
  markdown, actions: Vec<Step>, check: Option<Question> }`, `Step = Sim(Action) | Ui(UiAction)`, `Question { prompt, choices,
  answer, explanation }` — `crates/heartsim-core/src/tutorial.rs:1-16` and the type definitions following line 40.
- `UiAction = LoadScenario(String) | StartReentry(String) | FocusPanel(Panel) | Highlight(String) | CameraPreset(CameraView)
  | SetSpeed(f64) | Pause | Resume | WaitForEvent(WaitEvent) | SetQrsAxis(f64) | SelectLead(Lead)`; `Panel = Simulator |
  TwelveLead | HeartVector | AxisTrainer | AxisQuiz | Electrodes | RhythmQuiz | Tutorials`; `WaitEvent = ElapsedMs(f64) |
  Qrs` — `crates/heartsim-core/src/tutorial.rs` (enums `UiAction`, `Panel`, `WaitEvent`).
- Host interface: `trait TutorialHost { apply_action(&Action); apply_ui(&UiAction); sim_now_ms(); last_qrs_ms() }`
  (`crates/heartsim-core/src/tutorial.rs`, before `PageRun`); the app implements it
  (`crates/heartsim-app/src/app/tutorials.rs`, e.g. :102).
- "Actions are data. A page's `Step::Sim` carries a scenario `Action` verbatim ... so every intervention the engines learn
  ... works in tutorials without a change here" — `crates/heartsim-core/src/tutorial.rs:8-11`.
- Highlights: "a tutorial page names a control by a stable tool id and the control draws a pulsing outline" —
  `crates/heartsim-ui/src/highlight.rs:1-3`.
- Rhythm quiz: "a hidden rhythm from the scenario library is recorded and shown as a strip; the learner picks the diagnosis
  from four choices. Distractors come first from the scenario's own `confused_with` list ... then at random. The strip is a
  Tier-0 recording of the scenario (`Tier0Scenario::record`), deterministic and independent of the simulator, so answering
  never disturbs the running simulation" — `crates/heartsim-app/src/rhythm_quiz.rs:1-7`; API `make_question`,
  `record_strip`, `explanation`, `RhythmQuiz { next_question, answer, rings, score_string, restore_score, ui }`
  (`rhythm_quiz.rs:27-250`). The score persists through eframe storage (docs/web.md "Persistence").
- 10 tutorials exist (`assets/tutorials/01_basics` ... `10_ischaemia`); patient cases (B18) play in the same reader
  (`crates/heartsim-app/src/app.rs:86-88`). Other drills: `axis_quiz.rs`, `interval_drill.rs`.

### Inferences
- Tutorial extension is small: new `UiAction` variants such as `SetAuscultationSite(Site)`, `SetSound(bool)` / `SetVolume`,
  a `Panel::Auscultation`, and `WaitEvent::S1` (or reuse `Qrs`); highlight ids for the new controls. Valve findings arrive
  through `LoadScenario` (scenario with an `auscultation` section) or a new `Action` (section 2). Existing `Question`
  already suffices for "what do you hear?" checks.
- An "Auscultation quiz" can copy `RhythmQuiz`: pick a scenario that has an auscultation finding, call an offline
  `record()` that also returns PCG samples, show the PCG (optionally hidden at first: listen-only, then reveal PCG + ECG),
  loop the clip through the audio sink, four choices from `confused_with`. Since the clip is a finite pre-rendered buffer,
  playback is a plain kira `StaticSoundData` (desktop) or one `AudioBuffer` (web) — no streaming or sync problem at all.
- The quiz must cope with "no audio device / audio feature off / browser context suspended": fall back to PCG-only questions
  and show the "no sound device" hint the settings window already has.
- Headphone advice (section 9) belongs on the first page of an auscultation tutorial.

### Gaps
- `tutorial_ui.rs` and `case_ui.rs` were not read; how a quiz widget is embedded in the reader panel is not documented here.

---

## 7. kira 0.12 and alternatives: streaming a procedural sound, scheduling, latency, licences, wasm

### Takeaway
kira 0.12.6 can host a procedurally generated continuous stream through a custom `SoundData`/`Sound` implementation whose
`process()` is called on the audio thread, and can schedule one-shots by `StartTime::Delayed` or `StartTime::ClockTime`,
but start times are quantised to its internal buffer (default 128 frames, about 3 ms), it exposes no output-latency
figure, and on wasm it runs its mixer on the main thread through cpal's Web Audio host with 2048-frame buffers — so kira is a
good desktop fit and a poor browser fit.

### Cited Findings
- kira 0.12.6: licence "MIT OR Apache-2.0", edition 2024, description "Expressive audio library for games" — registry copy
  `kira-0.12.6/Cargo.toml` ([docs.rs/kira/0.12.6](https://docs.rs/kira/0.12.6)).
- Custom sounds: `trait SoundData { type Error; type Handle; fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle),
  Self::Error>; }` and `trait Sound: Send { fn on_start_processing(&mut self) {} fn process(&mut self, out: &mut [Frame],
  dt: f64, info: &Info); fn finished(&self) -> bool; }` with the note "the methods of this trait should not allocate or
  deallocate memory" — `kira-0.12.6/src/sound.rs:37-90` ([docs.rs](https://docs.rs/kira/0.12.6/kira/sound/trait.Sound.html)).
  `AudioManager::play<D: SoundData>` accepts any implementation (`src/manager.rs:109`).
- kira already depends on `rtrb` 0.4.0 (a lock-free SPSC ring buffer) and `triple_buffer` 9.0.0 — `kira-0.12.6/Cargo.toml`.
- Streaming sounds: `StreamingSoundData::from_decoder(decoder: impl Decoder)`, `trait Decoder: Send { sample_rate();
  num_frames(); decode() -> Result<Vec<Frame>, _>; seek(index) }` — `kira-0.12.6/src/sound/streaming/decoder.rs:8-31`,
  `streaming/data.rs:42`. It is file-oriented (needs `num_frames`, `seek`) and thread-based.
- Scheduling: `enum StartTime { Immediate, Delayed(Duration), ClockTime(ClockTime) }`; `ClockTime { clock: ClockId, ticks:
  u64, fraction: f64 }`; `ClockSpeed = SecondsPerTick | TicksPerSecond | TicksPerMinute`; clocks are created with
  `AudioManager::add_clock`, and `ClockHandle` offers `time()`, `start()`, `pause()`, `stop()` —
  `kira-0.12.6/src/start_time.rs`, `src/clock/time.rs:18-28`, `src/clock/clock_speed.rs:6-13`, `src/clock/handle.rs:12-58`,
  `src/manager.rs:182`.
- Granularity of a start: `StaticSound::process` calls `self.start_time.update(dt * out.len() as f64, info)` once per
  processed chunk and outputs silence for the whole chunk while the start time has not arrived
  (`kira-0.12.6/src/sound/static_sound/sound.rs:196-216`); the renderer processes in chunks of `internal_buffer_size`
  (`src/backend/renderer.rs:71-75`); `AudioManagerSettings::internal_buffer_size` defaults to 128: "At the default size of
  128 samples, at a sample rate of 44100hz, modulators and clocks will update about every 3 milliseconds. Decreasing this
  value increases the precision of clocks and modulators at the expense of higher CPU usage" —
  `src/manager/settings.rs:38-61`.
- Latency reporting: a search of kira's `src/` for "latency" returns nothing; the cpal backend's data callback ignores the
  callback info (`move |data: &mut [f32], _| { renderer.on_start_processing(); renderer.process(data, channels); }`,
  `src/backend/cpal/wasm.rs:63-69`). The manager exposes `backend_mut()` (`src/manager.rs:327`), and the desktop backend
  offers `pop_cpu_usage()` / `pop_error()` (`src/backend/cpal/desktop.rs:51-63`) but no timing.
- Device configuration: the cpal backend uses `device.default_output_config()` unless `CpalBackendSettings { device, config }`
  is given; the buffer size comes from that config — `kira-0.12.6/src/backend/cpal/desktop.rs:102-130`.
- kira on wasm, README: "Kira can also be used in wasm environments with the following limitations: Static sounds cannot be
  loaded from files; Streaming sounds are not supported because they make heavy use of threads" —
  `kira-0.12.6/README.md:111-114` ([repository](https://github.com/tesselode/kira)). The wasm backend wraps the stream in
  `send_wrapper::SendWrapper` and builds a normal cpal output stream (`src/backend/cpal/wasm.rs`); kira enables cpal's
  `wasm-bindgen` feature on wasm32 (`Cargo.toml`, target-specific dependency).
- cpal 0.18.2: licence "Apache-2.0" (only) — registry copy `cpal-0.18.2/Cargo.toml:34`. Hosts present: aaudio, alsa, asio,
  audioworklet, coreaudio, jack, pipewire, pulseaudio, wasapi, webaudio, custom, null (`cpal-0.18.2/src/host/`).
- cpal gives per-callback timing: `OutputStreamTimestamp { callback: StreamInstant, playback: StreamInstant }` where
  `playback` is "the predicted instant that data written will be delivered to the device for playback" —
  `cpal-0.18.2/src/timestamp.rs:54-63`; streams also have `now()` on the same time base and `buffer_size()`
  (`src/traits.rs:487-504`).
- cpal wasm: feature `wasm-bindgen` = "Web Audio API backend for browser-based audio; required for any WebAssembly audio
  support"; feature `audioworklet` = "Audio Worklet backend for lower-latency web audio ... running audio on a dedicated
  thread. Requires atomics support"; README table: `WASM (audioworklet)` needs nightly and "`-Zbuild-std` with atomics
  support enabled" — `cpal-0.18.2/README.md:55-73`.
- cpal's Web Audio host mechanics: `DEFAULT_BUFFER_SIZE: usize = 2048` frames; the data callback is run from closures on the
  main thread ("Starting with two workers, e.g. a front and back buffer"), each fills an `AudioBuffer` and schedules an
  `AudioBufferSourceNode` with `source.start_with_when(time_at_start_of_buffer)`; the first buffer is scheduled at
  `now + base_latency_secs + buffer_time_step_secs`; `playback` timestamps add `baseLatency + outputLatency` (read each
  callback because "outputLatency can change at runtime"); `ctx.resume()` is called on play —
  `cpal-0.18.2/src/host/webaudio/mod.rs:110, 371-390, 441-479, 552-621, 641-661`.
- rodio 0.23.0: "MIT OR Apache-2.0" (registry copy `rodio-0.23.0/Cargo.toml:36`); the project's own stack research rates it
  "Simplest ... Scheduling is 'play now', jitter ~ one audio buffer (5-20 ms)" — `docs/research/rust-stack.md:431-433`.
- fundsp: latest 0.23.0 (released 2026-01-07), "Licensed under either of Apache License, Version 2.0 or MIT license at your
  option", `no_std` supported by disabling the default `std` feature (needs `alloc`), per-sample `tick` and block `process`,
  filters, oscillators, noise; the crate page does not mention WebAssembly — [docs.rs/crate/fundsp](https://docs.rs/crate/fundsp/latest).
- Project stack rules: wgpu 30 / eframe 0.37 / winit 0.30 pinned together; "Audio: `kira`"; dependencies declared once in
  `[workspace.dependencies]` (CLAUDE.md "Toolchain").
- The project's earlier stack note claims "All three [cpal, rodio, kira] work on WASM (web audio via cpal's ...)"
  (`docs/research/rust-stack.md:439`); kira's README qualifies this (no streaming sounds), and the registry sources show the
  main-thread, 2048-frame design above.

### Inferences
- Desktop, continuous stream: implement `HeartSoundStream: Sound` holding the consumer end of an `rtrb` ring (already in the
  dependency tree; add it as a direct workspace dependency) fed from the UI thread with PCG samples; `process()` pops,
  resamples to the device rate (`dt` gives the output sample period) and writes frames, emitting silence on underrun. No
  allocation, no locks, `Send` only. This bypasses kira's "streaming sound" (decoder/thread) machinery entirely and keeps
  one device stream shared with the beep. Sample placement inside the stream is exact (it is the stream), so kira's 128-frame
  start quantisation does not matter.
- Desktop, one-shots: `StaticSoundData` per sound with `StaticSoundSettings.start_time = StartTime::Delayed(d)` or a
  `ClockTime` on a clock running at e.g. 1000 ticks/s. Start precision = one internal chunk: 128 frames = 2.9 ms at 44.1 kHz,
  2.7 ms at 48 kHz; setting `internal_buffer_size` to 32 would give about 0.7 ms at higher CPU cost. Two one-shots scheduled
  independently (S1, S2, or the A2/P2 components of a split S2) therefore jitter against each other by up to one chunk;
  rendering a whole beat's sounds into one buffer removes intra-beat jitter.
- Absolute desktop latency with kira = hand-over of the play command (next audio callback) + device buffer + OS mixer/driver;
  kira reports none of it. Options: (1) a user "audio offset (ms)" calibration slider (click-and-flash test), (2) bypass kira
  for the stream and use cpal directly, reading `OutputStreamTimestamp.playback - callback` per callback, (3) contribute/patch
  kira's backend to surface the timestamp. cpal is already a transitive dependency (cpal 0.18.2 in Cargo.lock), licence
  Apache-2.0 (allowed by CLAUDE.md's licence list), so direct use adds no new licence but duplicates device ownership if
  kira stays for the beep (two streams, or move the beep into the same custom mixer).
- kira on wasm32-unknown-unknown without threads: compiles by design (dedicated `wasm.rs` backend) and a custom `Sound`
  needs no thread, so it should run; but (a) the whole kira mixer then executes on the browser main thread inside
  cpal's callbacks, competing with the egui frame and the single-threaded Tier 1/2 engines (docs/web.md: rayon off, "only
  slower"), (b) buffers are 2048 frames = 42.7 ms at 48 kHz (46.4 ms at 44.1 kHz) and the first is scheduled one buffer
  plus `baseLatency` ahead, with a second callback pre-filling the next, so the stream is fed roughly 43-90 ms ahead of
  the playhead, and (c) any main-thread stall longer than the queued audio produces a dropout. Direct Web Audio scheduling
  (section 8) avoids (a)-(c) for one-shots and gives exact `start(when)` times.
- fundsp is licence-compatible and could supply band-pass filters/noise for murmurs, but heart sounds need only a few damped
  sinusoids, band-limited noise and envelopes; hand-written DSP in the pure crate avoids a dependency whose determinism
  across targets (SIMD paths) and wasm behaviour were not verified here.
- rodio offers nothing kira lacks here and has weaker scheduling.

### Gaps
- Not verified by running: that kira 0.12.6 with a custom `Sound` actually works on wasm32 in current browsers; real latency
  numbers on WASAPI / CoreAudio / ALSA-PipeWire for the default cpal config.
- Whether `ClockHandle` has a speed setter (to slave a kira clock to simulation speed) was not confirmed in the registry copy;
  only `time/start/pause/stop` were seen. Kira GitHub issues about wasm were not searched.
- fundsp's wasm support and bit-exact reproducibility are unverified (the crate page is silent on wasm).

---

## 8. Browser audio from Rust/wasm: Web Audio scheduling, AudioWorklet without SharedArrayBuffer, autoplay, latency reporting

### Takeaway
With no threads and no COOP/COEP, the robust browser path is plain Web Audio through `web-sys`: synthesise on the main
thread, hand finished `AudioBuffer`s to the audio thread with `AudioBufferSourceNode.start(when)` on the `AudioContext`
clock using a look-ahead of about 100 ms, start/resume the context from the user's click on the sound switch, and compensate
visuals with `baseLatency + outputLatency`; cpal's AudioWorklet backend is unavailable (needs atomics/nightly), and a
hand-made worklet fed by `postMessage` is possible but brings little for pre-scheduled buffers.

### Cited Findings
- The site "needs no special headers: there are no threads, so no COOP/COEP"; rayon is off in the web build —
  `docs/web.md:23-24, 38`. `wasm-bindgen` is pinned `=0.2.130`, web-sys 0.3.107 (`Cargo.toml:58-62`).
- Scheduling model ("A Tale of Two Clocks"): `AudioContext.currentTime` is the audio hardware clock, "designed to be able to
  specify alignment at an individual sound sample level"; main-thread timers "can easily be skewed by tens of milliseconds
  or more", so it "is a bad idea to use JavaScript's setTimeout to directly start playing audio events"; recommended: a timer
  that schedules ahead, "A good place to start is probably 100ms of 'lookahead' time, with intervals set to 25ms"; the
  audio-block scheduling time "varies across operating systems and hardware, from low single digits of milliseconds to
  around 50ms"; visuals are synchronised by comparing a queue of scheduled events against `audioContext.currentTime` in
  `requestAnimationFrame` — [web.dev, Audio scheduling](https://web.dev/articles/audio-scheduling).
- `AudioContext.outputLatency`: "an estimation of the output latency of the current audio context ... the time, in seconds,
  between the browser passing an audio buffer out of an audio graph over to the host system's audio subsystem to play, and
  the time at which the first sample in the buffer is actually processed by the audio output device"; Baseline "Newly
  available" since March 2025 — [MDN outputLatency](https://developer.mozilla.org/en-US/docs/Web/API/AudioContext/outputLatency).
- `AudioContext.baseLatency`: "the number of seconds of processing latency incurred by the AudioContext passing an audio
  buffer from the AudioDestinationNode ... into the host system's audio subsystem"; `latencyHint` "interactive" (default)
  vs "playback" (example value 0.15 s), "the browser may ignore the option"; Baseline widely available since April 2021 —
  [MDN baseLatency](https://developer.mozilla.org/en-US/docs/Web/API/AudioContext/baseLatency).
- cpal's own Web Audio host computes playback time as buffer start + `baseLatency + outputLatency` and treats a missing or
  changing `outputLatency` defensively — `cpal-0.18.2/src/host/webaudio/mod.rs:371-376, 461-479`.
- Autoplay: "In the Web Audio API, a website or app can start playing audio using the start() method on a source node linked
  to the AudioContext. Doing so outside the context of handling a user input event is subject to autoplay rules"; playback
  is generally allowed once "The user has interacted with the site (by clicking, tapping, pressing keys, etc.)"; Firefox
  pref `media.autoplay.block-webaudio` default true: "audio contexts are only able to play on pages once there has been
  Sticky activation"; `navigator.getAutoplayPolicy("audiocontext")` returns allowed/disallowed —
  [MDN Autoplay guide](https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Autoplay).
- Context state: a suspended context "would be unpaused by running the AudioContext.resume() method"; "In iOS Safari, when a
  user leaves the page (e.g., switches tabs, minimizes the browser, or turns off the screen) the audio context's state
  changes to 'interrupted' and needs to be resumed" —
  [MDN BaseAudioContext.state](https://developer.mozilla.org/en-US/docs/Web/API/BaseAudioContext/state).
- AudioWorklet from Rust: the official wasm-bindgen AudioWorklet example relies on threads (SharedArrayBuffer) —
  [wasm-bindgen guide](https://rustwasm.github.io/wasm-bindgen/examples/wasm-audio-worklet.html); without threads, write-ups
  post the compiled module or bytes to the processor with `node.port.postMessage` and adapt the JS glue to the worklet scope
  — [dev.to, wasm-pack in an AudioWorkletProcessor](https://dev.to/speratus/how-i-used-wasm-pack-to-build-a-webassembly-module-for-an-audioworkletprocessor-4aa7),
  [dev.to follow-up](https://dev.to/speratus/i-built-this-despite-a-flaw-in-rusts-webassembly-toolchain-38p2). The Web Audio
  working group notes the tension of using `postMessage` for both small low-latency messages and large arrays on the audio
  thread — [web-audio-api issue 2269](https://github.com/WebAudio/web-audio-api/issues/2269).
- cpal's `audioworklet` host is compiled only with `target_feature = "atomics"` (`cpal-0.18.2/src/host/mod.rs:30-36`,
  `src/platform/mod.rs:878-884`), i.e. it needs shared memory and therefore COOP/COEP headers.

### Inferences
- Minimal web sink (new file in `heartsim-web` or a `cfg(target_arch = "wasm32")` module of `heartsim-app`): web-sys features
  `AudioContext`, `AudioContextOptions`, `AudioContextState`, `AudioBuffer`, `AudioBufferSourceNode`, `AudioDestinationNode`,
  `GainNode`, `AudioParam`. Per chunk or per sound: `create_buffer(1, n, rate)`, `copy_to_channel`, `create_buffer_source`,
  `connect(gain)`, `start_with_when(t)`. An `AudioContext` may be created at the PCG's own rate (e.g. `sampleRate: 8000` is
  inside the 3 000-768 000 Hz range cpal cites from the spec, `webaudio/mod.rs:103-105`) or buffers can be created at the PCG
  rate inside a 48 kHz context and the browser resamples them — no Rust resampler needed on the web.
- One-shot-per-sound (or per beat) scheduling suits the browser best: each sound becomes an `AudioBuffer` started at
  `t_ctx = ctx_time_at_sim_anchor + (sim_at - sim_anchor) / speed + lookahead`. Once handed over, the audio thread plays it on
  time even if the main thread stalls (web.dev article), unlike a main-thread-fed stream (kira/cpal), which drops out.
- A continuous stream in the browser = chained `AudioBufferSourceNode` chunks (what cpal does) with at least ~100 ms queued; it
  works but its latency is the queue depth and it glitches on long frames. A custom JS `AudioWorkletProcessor` that receives
  `Float32Array` chunks by `postMessage` (transferable, no SharedArrayBuffer, no wasm inside the worklet) gives the same queue
  with smoother consumption, at the cost of a JS asset file and message jitter; it does not reduce the queue depth needed to
  ride out main-thread stalls, so it is not worth the complexity for the first version.
- Autoplay: create (or `resume()`) the `AudioContext` when the learner switches heart sounds on. Risk to test: eframe on the
  web delivers input in DOM event listeners but runs `App::update` in the animation frame, so a `resume()` issued from
  egui's click handling may execute outside the gesture's call stack. Browsers with sticky activation accept that; stricter
  engines (historically Safari/iOS) may not — a small DOM-level `pointerdown`/`keydown` listener that resumes the context
  is the conservative design. Also re-resume on iOS "interrupted", and show a "click to enable sound" state when
  `ctx.state != "running"`.
- Latency compensation on the web: `delay = lookahead + baseLatency + outputLatency` is known in seconds; visuals that should
  coincide with a sound can be drawn at `currentTime - (baseLatency + outputLatency)` mapped back to simulation time. Where
  `outputLatency` is missing (older browsers) fall back to `baseLatency` plus a user offset. Bluetooth headphones add
  100-300 ms that only `outputLatency` (when the platform reports it) or a manual offset can absorb.

### Gaps
- Per-browser version numbers for `outputLatency` (notably Safari) were not retrieved; MDN only states Baseline March 2025.
- No source was found describing a `postMessage`-fed ring-buffer worklet in Rust; the design above is standard practice
  but uncited. One anecdotal report of Chromium/Linux failing to receive a wasm module by `postMessage` exists
  ([itch.io post](https://itch.io/post/10032478)); not relevant if the worklet is plain JS.
- Whether eframe 0.37's web runner executes app code inside the original user-gesture task was not verified.
- MDN's `AudioBufferSourceNode.start(when)` page was not fetched; its semantics are taken from the web.dev article and cpal's
  use of `start_with_when`.

---

## 9. Synchronisation strategies (60 fps, 0.05x-2x, pause, scrub), perception thresholds, determinism

### Takeaway
A fixed-delay design is needed either way: per-frame triggering cannot meet a 20-30 ms target, whereas (a) look-ahead-scheduled
one-shots or (b) a continuous stream fed from the simulation with a known, compensated delay can; the broadcast standard
ITU-R BT.1359-1 puts detectability at +45 ms (audio early) / -125 ms (audio late), so the practical rule is "never early,
at most a few tens of ms late", which a 30-60 ms audio delay plus an equal display delay (or the natural
electromechanical delay) satisfies. Determinism is kept by making sound synthesis a pure function of engine events in a pure
crate and treating every audio device as a write-only sink.

### Cited Findings
- ITU-R BT.1359-1 (1998): "the thresholds of detectability are about +45 ms to -125 ms and thresholds of acceptability are
  about +90 ms to -185 ms on the average", positive = sound advanced with respect to vision —
  [ITU-R BT.1359-1 PDF](https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.1359-1-199811-I!!PDF-E.pdf) (text as quoted in
  search results; an NTIA report restates the same numbers with the opposite sign convention —
  [NTIA 11-474](https://its.ntia.gov/publications/download/11-474.pdf)). A secondary article notes the Recommendation is still
  in force — [forasoft.com overview](https://www.forasoft.com/learn/audio-for-video/articles-audio/lip-sync-itu-r-bt-1359-tolerance-windows).
- Audiovisual pairs "tend to be judged more synchronous when the visual stimulus leads", and sensitivity to asynchrony is
  typically higher for audio-leading pairs; training narrows the temporal binding window — search summary of Vroomen &
  Keetels, "Perception of intersensory synchrony: A tutorial review", Atten Percept Psychophys 2010;72:871-884
  ([NCBI Bookshelf chapter](https://www.ncbi.nlm.nih.gov/books/NBK92837/), full text not retrievable here).
- Frame pacing facts: frame dt is `stable_dt` capped at 100 ms; `SimClock::advance(frame * speed)` yields whole 1 ms steps,
  caps at 200 steps per frame and then drops the backlog — `crates/heartsim-app/src/app.rs:919-922`,
  `crates/heartsim-core/src/time.rs:84-97`. Speed range 0.05-2 (`crates/heartsim-ui/src/transport.rs:10`).
- Pause stops stepping; scrub is a paused-only cursor into the 60 s rings and heart history (`app.rs:53-54, 76-77, 920`;
  `crates/heartsim-ui/src/ecg_view.rs:89-93, 341-354`).
- Rule: "Rendering interpolates; it never drives the simulation"; engines "deterministic for a given scenario + seed,
  fixed-step, free of I/O and GPU calls" (CLAUDE.md non-negotiables 2-3); the beep follows it: "The simulation never depends
  on audio" (`crates/heartsim-app/src/monitor_beep.rs:7-8`).
- Web look-ahead guidance: 100 ms look-ahead, 25 ms scheduler interval — [web.dev](https://web.dev/articles/audio-scheduling).
- kira start quantisation 128 frames (about 3 ms) and absence of latency reporting; cpal Web Audio 2048-frame buffers — section 7.

### Inferences

**Why per-frame triggering fails.** At 60 fps the simulation advances in bursts of about 17 ms; an event is seen up to one
frame late with one frame of jitter, before any device latency. That alone uses the whole 20-30 ms budget and makes S1-S2
intervals wobble. Any accurate scheme must (1) timestamp in simulation time (available: `Event.at` is exact), (2) map
simulation time to the audio clock through an anchor, and (3) play at `anchor + offset + D` with a fixed delay `D` larger
than the worst frame time.

**Where the delay can come from without touching the visuals.** Heart sounds are mechanical and follow their electrical
trigger (S1 after QRS onset, S2 near the end of T, S4 after the P wave, S3 after S2). If the sound model guarantees that no
sound starts sooner than `L_min` after the event that determines it, the synthesiser can always render audio up to
`now + L_min` using only past events: a free look-ahead equal to the smallest electromechanical delay (a few tens of ms;
values not sourced here). It covers one 60 fps frame but not frame + device latency, so some extra delay or display
compensation is still required for strict alignment. Diastolic content is the hard case: a diastolic murmur or the silence
before S1 depends on the *next* beat, which the engine does not announce in advance; a murmur must therefore be an open-ended
stream that is ended (short fade) when the next ventricular event arrives, not a fixed-length clip.

**Option (a): event-triggered one-shots with look-ahead scheduling.**
- How: a pure `SoundScheduler` turns events into `ScheduledSound { at_sim_ms, kind, params }`; the sink maps to device time
  and calls kira `play` with `StartTime::Delayed`/`ClockTime` (desktop) or `start(when)` (web).
- Sync: desktop precision about 3 ms (kira chunk) + unknown device latency (calibration offset); web sample-accurate against
  `currentTime`, latency known (`baseLatency + outputLatency`).
- Pros: tiny; resilient to main-thread stalls on the web; reuses kira as is; pause = stop/cancel pending handles.
- Cons: open-ended sounds (diastolic/continuous murmurs, rubs) need stop commands, whose timing has the same frame jitter;
  the drawn PCG is a parallel rendition, not literally the played samples; S1/S2 pairs scheduled by separate calls jitter by
  up to one kira chunk unless a whole beat is rendered into one buffer; many small buffers per second in tachycardia/AF.
- Speed changes: clip pitch is naturally preserved (clips are not stretched), onset spacing scales with 1/speed; already
  scheduled clips after a speed change or pause must be cancelled and rescheduled.

**Option (b): continuous synthesis into a stream with a fixed, compensated delay.**
- How: `PcgSynth` (pure) renders samples for each simulated interval as the fixed-step loop runs (exactly like the ECG ring),
  pushes them to the PCG `LeadRing` (for drawing) and to an SPSC queue; a custom kira `Sound` (desktop) or chunked
  `AudioBufferSourceNode`s (web) consume it. The audio runs `D` behind the simulation, where `D` = target queue depth: about
  2-3 frames (35-50 ms) on desktop, about 100 ms on the web.
- Sync: audio is late by `D + device latency`. To bring it inside 20-30 ms, draw the strip/PCG cursor and the 3-D map at
  `now - D_total` (the app already renders the past from rings and `HeartHistory` when scrubbing; the live sweep would use the
  same path with a small constant offset), or accept "audio late by 40-60 ms", which is still inside the BT.1359-1
  detectability bound for late audio (-125 ms) though outside the 20-30 ms goal. Because the stream is continuous,
  intra-beat timing (S1-S2, split S2, murmur shape) is sample-exact regardless of frame jitter.
- Pros: one source of truth in the strict sense at 1x (drawn PCG = played samples); murmurs, rubs and any future continuous
  sound are natural; a single code path for all sounds; the same samples serve export (WAV) and quizzes.
- Cons: needs clock-drift control between the simulation (frame clock) and the audio device clock (they are different
  oscillators): monitor queue depth and nudge the resampling ratio by a fraction of a percent, or drop/insert a few samples
  in silence; underruns on long frames (fill with silence, re-anchor); on the web the stream lives on the main thread
  (dropouts during long frames, e.g. scenario loads or Tier 2).
- Speed != 1x: playing the simulation-time samples at `speed` changes pitch by the same factor — at 0.5x a 50 Hz S1 becomes
  25 Hz and inaudible, at 0.05x nothing is audible; at 2x everything moves up an octave. Options: (1) audio only at 1x
  (mute otherwise, show the PCG); (2) since the sounds are parametric, re-synthesise for wall time: keep each sound's
  internal frequency and duration, stretch only the intervals between sounds and the envelopes of interval-filling murmurs —
  pitch-preserving by construction, no phase vocoder needed, but then the audio at speed != 1 is no longer the drawn samples
  (timing agrees, waveform does not); (3) above about 1.5x-2x, where S1/S2 (each on the order of 100 ms) start to crowd the
  shortened systole, mute or fall back to the monitor beep. A reasonable policy: exact stream at 1x; natural-pitch
  re-synthesis in slow motion (the main teaching use: "hear the split at half speed"); mute above 1x or keep to 2x with
  re-synthesis.

**Option (c): pre-render one beat (or one cycle) and loop/trigger it.**
- How: at scenario load or whenever the beat pattern changes, render one cardiac cycle to a buffer; trigger it on each
  ventricular event (or loop it at the RR interval).
- Pros: simplest; sample-exact inside the beat; ideal for the **quiz** and for a stand-alone "auscultation player" where a
  finite clip of several seconds is recorded offline (`record()`), played as one static buffer and drawn from the same
  samples — no live sync problem at all.
- Cons: wrong for exactly the rhythms the simulator teaches: AF (every RR differs), AV block (PR varies beat to beat so S1
  intensity varies), ectopy, pacing with loss of capture, live interventions; diastole length varies, so a fixed clip either
  truncates or overlaps. Usable live only as "per-beat render at the QRS event" = option (a) with one buffer per beat, which
  is a good compromise: S1 + systolic murmur + S2 (+S3) rendered together at the ventricular event (QT, hence S2 time, is
  known then because the ECG synth derives it from the preceding RR), S4 and diastolic murmurs handled separately.

**Comparison against the 20-30 ms goal.**

| | intra-beat accuracy | audio vs visuals | open-ended murmurs | web main-thread stalls | drawn PCG = audio | effort |
|---|---|---|---|---|---|---|
| per-frame "play now" (today's beep) | +/- 1 frame (17 ms) | 17 ms + device, jittery | no | n/a | no | exists |
| (a) one-shots, look-ahead | desktop about 3 ms per start; web sample-exact | D chosen (30-100 ms), compensable; desktop device latency unknown | awkward | robust | no (parallel rendition) | low-medium |
| (b) stream, fixed delay | sample-exact | D (35-50 ms desktop, about 100 ms web) + device, compensable by display delay | natural | fragile | yes at 1x | medium-high |
| (c) per-beat pre-render | sample-exact within a beat | as (a) | no | robust | yes for offline clips | low |

**Suggested combination.** One pure `PcgSynth` + beat assembler (deterministic, wasm-clean, golden-tested) feeding: the PCG
ring for drawing (always, also with audio off); on desktop a kira custom `Sound` stream (option b) with a queue of about 40 ms
and a user-adjustable "audio offset"; on the web per-beat buffers through `start(when)` (option a/c hybrid) with a 100 ms
look-ahead and `baseLatency + outputLatency` compensation; offline clips (option c) for quizzes and export on both. The sink
trait hides the difference, as `BeepSink` does today.

**Pause, scrub, reload.** Pause: fade the stream out over about 5-10 ms and clear the queue (or cancel scheduled nodes); on
resume re-anchor simulation time to the audio clock. Scrub (paused): nothing plays live; offer "play from cursor" = slice
of the PCG ring (60 s history exists) played as a static buffer while a playhead moves over strip and PCG — cheap with option
(b)'s ring. Scenario reload restarts simulation time at zero (the beep gate already special-cases this,
`monitor_beep.rs:108-114`): clear queues and re-anchor. Backlog drop (`max_steps_per_frame`) = a discontinuity: re-anchor.

**Keeping the simulation deterministic and audio-free.** (1) `PcgSynth` lives below `render` (in `heartsim-ecg` or a new pure
crate), depends only on `heartsim-core`/`heartsim-sim` types, takes events and simulation time, never wall time or device
rate; murmur noise from a seeded generator keyed on event time (pattern: `event_noise`, synth.rs:22-24). (2) All device
code stays in `heartsim-app` behind `audio` (desktop) / in the web crate, write-only: nothing flows back from the audio clock
into `SimClock`. Drift correction happens on the audio side (resampling ratio / queue trimming), never by changing simulation
stepping. (3) Display-delay compensation only changes which already-computed sample is drawn. (4) Golden tests compare PCG
sample arrays from `record()`; device sinks are tested with fakes like `BeepSink`.

### Gaps
- EBU R37 could not be verified: no search result gave its figures (from memory it recommends tighter end-to-end limits than
  BT.1359, on the order of 40 ms audio-early / 60 ms audio-late; treat as unverified).
- No citable figure was retrieved for audiovisual asynchrony thresholds with simple transient stimuli (flash/click), which are
  closer to "S1 against a moving trace" than lip-sync is; the commonly quoted figure of roughly 20 ms temporal-order
  resolution for simple stimuli is from memory (Hirsh & Sherrick 1961) and unverified here. The 20-30 ms target is therefore a
  conservative design goal, not a sourced threshold.
- Clock drift magnitude between display-frame time and audio device clocks, and typical desktop output latencies per backend,
  were not sourced or measured.
- No published precedent of an ECG/heart simulator's audio-sync design was searched for.

---

## 10. Low-frequency reproduction: S3/S4 on laptop and phone speakers

### Takeaway
S3 and S4 sit at the bottom of the audio band where small speakers radiate almost nothing, so the product should recommend
headphones, always show the phonocardiogram, and optionally offer an "enhanced for small speakers" mode (upward pitch shift
or added harmonics), clearly labelled as non-natural; pitch shifting of heart sounds for phone playback has published
support, harmonic "virtual bass" is a known psychoacoustic technique with quality trade-offs.

### Cited Findings
- Frequency content reported in search results: S3 and S4 roughly 20-200 Hz, S1 and S2 roughly 50-500 Hz; S3 "appears at the
  lowest frequency of the heart sounds" — search summary of [PMC6313612](https://pmc.ncbi.nlm.nih.gov/articles/PMC6313612) /
  [PMC7305673](https://pmc.ncbi.nlm.nih.gov/articles/PMC7305673) (primary tables not opened; the assignment's own figure is
  20-70 Hz).
- "Enhancement of cardiac and respiratory sounds for cellphone reproduction by means of digital sound processing methods",
  Personal and Ubiquitous Computing 2024;28(5):845-856, DOI 10.1007/s00779-024-01833-5: smartphone speakers have limited
  accuracy for low-frequency sounds such as heart sounds; the authors pitch-shift the sounds upward; in a 40-student survey
  80% judged heart-sound clinical information preserved, and 71% of 14 clinicians/students in an in-house evaluation;
  validation was done over headphones "as a first step towards delivering them through loudspeakers"; "further research is
  essential to understand smartphones' playback capabilities in a clinical setting" — abstract as indexed at
  [investigadores.anid.cl](https://investigadores.anid.cl//en/public_search/work?id=866963) (full text not read).
- Virtual bass: systems exploit the missing-fundamental effect, tricking "the human auditory system into perceiving
  low-frequency content that is not physically reproduced" by adding harmonics that small transducers can reproduce;
  implemented in the time domain with non-linear processing or in the frequency domain with a phase vocoder; drawbacks
  include transient smearing and quality loss with higher-order harmonics — [NTU virtual bass system overview](https://www3.ntu.edu.sg/home/ewsgan/VBS.html),
  [Google Patents US9794688](https://patents.google.com/patent/US9794688),
  [CORE: Development and evaluation of virtual bass systems for small loudspeakers](https://core.ac.uk/works/108090091).
- Listener factor: hearing loss "can impact the ability to detect heart sounds across critical frequency ranges" (search
  summary, source among the PMC results above; not individually verified).
- Recorded heart-sound audio files are used as teaching aids for medical students (search summary of
  [iStethoscope, PubMed 25626547](https://pubmed.ncbi.nlm.nih.gov/25626547/); abstract not opened).

### Inferences
- Because heartsim's sounds would be synthesised parametrically, both enhancements are nearly free and exact: a "small
  speaker" mode can (1) multiply all component frequencies by a factor (e.g. 2-3x) while keeping timing and envelopes —
  a pitch shift without any vocoder artefact — or (2) add 2nd-4th harmonics to the low components (missing-fundamental cue)
  and high-pass the result. Either is a pure parameter of `PcgSynth`, deterministic and testable. It must be labelled (the
  timbre is no longer what a stethoscope delivers) and off by default.
- The drawn PCG should show the true (unenhanced) signal; S3/S4 remain visible there even when inaudible, which also serves
  hearing-impaired learners and silent classrooms. A spectrogram or band-split view is a possible later addition.
- UI text: a one-line headphone recommendation near the sound switch and in the auscultation tutorial; a short "sound check"
  (a 40 Hz and a 200 Hz burst: "if you hear only the second, use headphones or switch on enhancement").
- On-device equalisation and Bluetooth latency are outside the app's control; the manual audio-offset setting (section 7/9)
  covers the latter.

### Gaps
- No measured low-frequency cut-offs of laptop/phone speakers were retrieved (typical roll-off figures are not cited here).
- No source was found on frequency-shifting modes of commercial electronic stethoscopes; that claim in the assignment is
  unverified by this research.
- JMIR Cardio 2024;8:e54746, which a search summary first associated with pitch shifting, is in fact a comparison of two
  digital stethoscopes (Eko DUO vs Littmann CORE) and is not evidence for pitch shifting.
