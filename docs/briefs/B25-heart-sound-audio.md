# B25 — Audible heart sounds on desktop (heart sounds, phase 2)
**Crates** `heartsim-ecg`, `heartsim-app`, `heartsim-ui` · **Model** Opus · **Milestone** M9 (audio, phase 2 of 5) ·
**Depends on** B24, B7, B19 and the monitor beep (`crates/heartsim-app/src/monitor_beep.rs`, `audio_out.rs`).

## Goal
The learner hears the phonocardiogram that B24 draws. On desktop builds with the `audio` feature, the samples in the PCG ring are
played through the sound card a small constant delay behind the simulation, and the strip, the heart-sound lane and the 3-D map
are drawn the same delay behind, so that S1 is heard when the trace shows it. Spacing inside a beat (the A2–P2 split) is
sample-exact because what is played is the drawn signal, not a sound triggered per frame. Slow motion keeps the natural pitch of
each sound and stretches only the intervals between them. The web build stays silent in this brief (phase 3).

## Read first
`docs/research/heart-sounds.md` (sections "Why synchronisation needs a fixed delay" and "Recommended design": device sinks, speed,
pause, scrub); `docs/research/heart-sounds-notes/codebase_audio_sync.md`; `crates/heartsim-ecg/src/pcg.rs` (`PcgSynth`,
`schedule`, `Components`, the atoms, `replaces_previous`); `crates/heartsim-app/src/heart_sounds.rs` (the live ring);
`crates/heartsim-app/src/monitor_beep.rs` and `audio_out.rs` (`BeepSink`, lazy device creation, the fake-sink test pattern to
copy); `crates/heartsim-app/src/app.rs` (`step_simulation`, `sync_scrub`, the colour-map history used while scrubbing);
`crates/heartsim-ui/src/ecg_view.rs` (Display popup, `scrub_offset_ms`); `crates/heartsim-ui/src/transport.rs` (`SPEED_RANGE`);
kira 0.12.6 `Sound` / `SoundData` documentation on docs.rs (custom streaming sounds). Look the kira API up; do not write it from
memory.

## Non-negotiables for this brief
* **Write-only audio.** Nothing flows from an audio clock, queue depth or device callback into `SimClock`, the engines or the
  rings. With sound on or off, the same scenario and seed give the same engine events, ECG and PCG samples. Drift between the
  simulation and the sound card is absorbed on the audio side.
* **Never early.** A sound may be heard a constant few tens of milliseconds after its drawn position, never before it.
* **Real-time safety.** The code that runs on the audio thread (the kira `Sound::process`) takes no lock, allocates nothing, logs
  nothing and never blocks; it pops from a lock-free single-producer single-consumer queue and writes silence on underrun.
* **Lazy device.** No audio device is opened until the learner switches the beep or the heart sounds on (as today for the beep).
  A machine without a device (CI, headless) logs one warning, shows "no sound device" in the UI and carries on silently.
* All new logic except the kira sink is pure, always compiled (with and without `audio`, on `wasm32`) and tested with a fake sink.
  Only `audio_out.rs` touches kira. `heartsim-ecg` gains no audio dependency.
* Every new numeric constant (queue target, fades, default offset, limiter knee, filter corner) is named, documented and marked
  **designed** or given its source. Nothing comes from `epicardio/`; no recordings are used or shipped.

## Scope
1. **Sink trait** (`heartsim-app`, new `sound_out.rs`, always compiled). `PcmSink`: mono `f32` at the PCG rate in, write-only:
   `push(&mut self, samples: &[f32]) -> usize` (how many were accepted), `queued_ms() -> f32`, `clear(&mut self)` (drop what is
   queued; the sink fades to silence itself), `set_gain(f32)`. A `FakeSink` for tests records pushes and lets a test drain it at a
   chosen rate.
2. **Shared device** (`audio_out.rs`, feature `audio`). One lazily created `AudioManager` owned by an `AudioOutput` that hands out
   both the beep sink and the heart-sound sink; the beep keeps its present behaviour and tests. The heart-sound sink is a custom
   kira `Sound` fed by an `rtrb` queue (add `rtrb` to `[workspace.dependencies]`, optional under `audio`; it is already in
   `Cargo.lock` through kira). On the audio thread: pop, resample 4000 Hz to the device rate, apply gain, soft-limit, write both
   channels. The resampler is a pure function in the always-compiled module so it can be tested: band-limited (windowed-sinc or
   polyphase; plain linear interpolation leaves images above 2 kHz that sound like hiss), with a 10 ms fade-in after an underrun and
   a 10 ms fade-out on `clear`.
3. **Player** (`heartsim-app`, new `heart_sound_player.rs`, always compiled, pure). Owns a `Box<dyn PcmSink>` and decides what to
   push each frame from the PCG ring's absolute sample index. States and rules:
   * *Live at 1×*: push every new ring sample exactly once, in order. Keep the queue near `QUEUE_TARGET_MS` (designed, about
     40 ms). Underrun: the sink plays silence and the player re-anchors at the newest sample (never replays old ones). Backlog
     above about three times the target (a stalled frame, a debugger): drop the oldest queued audio through `clear` and re-anchor.
   * *Slow motion (speed below 1×)*: push from the wall-time synthesiser of item 4 instead of the ring.
   * *Fast (speed above 1×)*: heart sounds are muted; the monitor beep still works. The Display popup says why.
   * *Paused*: fade out and clear; nothing is pushed.
   * *Play from cursor* while paused: a "Play" button in the heart-sound lane plays the ring from the scrub cursor to the head
     once; when a caliper pair is placed it plays the span between the two lines. A playhead line moves across the strip and the
     lane with the sound (position = samples pushed minus queue depth minus audio offset). "Stop" or resuming the simulation ends
     it.
   * Scenario load, restart, EP-lab or re-entry start, or the engine clock going backwards: clear and re-anchor.
   * Output level: PCG units have a normal S1 peak at 1.0 and loud S1 above it. Apply `HEADROOM_GAIN` (designed) times the
     user's volume, then a soft limiter in the sink so that no beat clips.
4. **Wall-time synthesiser** (`heartsim_ecg::pcg`, pure). For slow motion: fed the same events as `PcgSynth`, it places each
   component (from the same `schedule`) at its simulation onset divided by the speed and renders the atom at its natural
   frequency and duration in wall time. Speed changes take effect for components not yet started. It follows beat revisions
   (`replaces_previous`) the way `PcgSynth` does. The lane keeps showing the true signal; only the sound is re-timed.
5. **Display delay.** While heart sounds are playing live, the strip, the 12-lead page, the heart-sound lane and the 3-D colour
   map are drawn `QUEUE_TARGET_MS + audio offset` behind the engine, using the mechanism that scrubbing already uses (a hidden
   scrub offset for the rings; the colour-map and viewport history for the 3-D view). The delay is applied only while sound is
   actually playing, changes without a visible jump (ramp over about 0.3 s) and is not shown as "scrubbed back". Calipers,
   labels and measurements are unaffected. If a view cannot be drawn in the past (say which and why), leave it live and document
   it in known issues.
6. **Settings and controls.** `Settings` gains `heart_sound_audio: bool` (default off), `heart_sound_volume: f32` and
   `audio_offset_ms: f32` (default designed, about 60 ms; range 0–300 ms), all persisted with defaults for old settings files.
   In builds with `audio`: a "Play heart sounds" switch under "Heart sounds" in the Display popup (switching it on also shows the
   lane; switching the lane off does not stop the sound), and a "Heart sounds" section in the Settings window with the switch, the
   volume slider, the offset slider with a short explanation, the "no sound device" notice, and a headphone hint (laptop speakers
   reproduce little below 150 Hz, where most of S1 and S2 lies). In builds without `audio` (the web build) the switch is absent.
   Strings in en and it.
7. **Offset calibration.** A small "Check sync" helper in the Settings section: it plays a short click once a second through the
   heart-sound sink while a marker flashes on the drawn schedule; the learner moves the offset slider until click and flash
   coincide. This is also the manual latency check for the three desktop platforms.
8. **Docs.** User book: a section on listening (how to switch it on, headphones, what slow motion does to the sounds, why sound
   stops above 1×, the offset slider and the sync check, that the sounds are a model and not a recording); `known-issues.md`;
   `CHANGELOG.md` under Unreleased; `docs/web.md` (still silent on the web; phase 3); add milestone M9 and its five phases to
   `docs/ROADMAP.md`.

## Out of scope (later phases)
Web Audio output (phase 3). S3, S4, clicks, snaps, murmurs, the `auscultation:` scenario section, listening sites, bell and
diaphragm, valve highlights (phase 4). Tutorial, quiz, looping repetition trainer, small-speaker mode, reference recordings, WAV
export (phase 5). Making the monitor beep sample-accurate (it may be mixed into the same stream later; not required here).

## Acceptance tests
* Player with `FakeSink` (no device, runs on CI):
  * live at 1×, a 10 s scenario stepped in uneven frame sizes: the concatenated pushes equal the PCG ring samples exactly, each
    once, in order;
  * underrun (the fake drains faster than the player pushes): no sample is pushed twice, playback re-anchors at the newest sample;
  * backlog (a 500 ms stalled frame): the queue returns to the target within one frame and nothing older than the target is played;
  * pause clears the queue and pushes nothing; resume re-anchors; scenario reload and a clock that goes backwards re-anchor;
  * speed above 1× pushes nothing; back at 1× playback resumes without replaying;
  * play from cursor pushes exactly the ring slice from the cursor (or between the calipers) and reports a playhead that moves
    monotonically and ends at the slice end;
  * with the switch off, no sink is ever created (the factory is not called).
* Wall-time synthesiser: at speed 1.0 its output equals `PcgSynth` sample for sample on sinus, RBBB and a biventricular-paced
  scenario; at 0.25× the A2–P2 onset interval of an RBBB beat is four times the 1× interval within one sample, while each atom's
  zero-crossing rate over its first 20 ms is unchanged.
* Resampler: a 200 Hz sine at 4 kHz resampled to 44.1 kHz and to 48 kHz matches the ideal sine within 1 %; image energy above
  2.5 kHz is at least 40 dB below the tone; an underrun followed by data has no step larger than the fade allows.
* Limiter: the loudest S1 in the complete-AV-block golden scenario at full volume stays within ±1.0 after gain and limiting.
* Determinism: ECG and PCG golden snapshots unchanged; a run with heart-sound audio on (fake sink) produces the same rings as a
  run with it off.
* Display delay (unit test on the offset logic): with sound on the drawn head is `QUEUE_TARGET_MS + offset` behind, with sound
  off or paused it is zero, and the transition is a ramp.
* UI (egui_kittest): the switch appears in the Display popup and Settings in a build with `audio` and not without; "no sound
  device" is shown when the factory fails; settings round-trip, and an old settings file loads with the defaults.
* `cargo check -p heartsim-app --no-default-features --lib` and the wasm32 check of the pure crates pass (run the wasm check by
  hand: `cargo xtask ci` has no wasm step); `cargo xtask ci` passes.
* Manual, reported honestly as not done if no device is available: on Linux, Windows and macOS, switch the sounds on, run the
  sync check, and note the offset at which click and flash coincide.

## Decisions for the maintainer
* Default audio offset (proposed 60 ms) and whether the drawn views may lag by that much while sound plays.
* Whether "Play heart sounds" should also be reachable from the top bar (a speaker button) or only from the Display popup and
  Settings.
