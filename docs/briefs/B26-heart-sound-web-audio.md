# B26 — Heart sounds in the browser (heart sounds, phase 3)
**Crates** `heartsim-web`, `heartsim-app` · **Model** Opus · **Milestone** M9 (audio, phase 3 of 5) ·
**Depends on** B21, B24, B25.

## Goal
The web build plays the heart sounds that the desktop build plays since B25: the same player, the same samples, the same
controls (switch, volume, offset, sync check, slow motion, play from cursor), through the browser's Web Audio API instead of
kira. Sound starts only after a click of the learner, as browsers require, and the app says so when it is waiting for one. The
site keeps working as a plain static site: no threads, no `SharedArrayBuffer`, no COOP/COEP headers.

## Read first
`docs/web.md` (the whole file: what the web build leaves out and why, the smoke test, the manual matrix, the size budget);
`docs/research/heart-sounds.md` ("Why synchronisation needs a fixed delay", "Device sinks, write-only");
`docs/briefs/B25-heart-sound-audio.md`; `crates/heartsim-app/src/sound_out.rs` (`PcmSink`, `FakeSink`, `Resampler`, `soft_limit`,
the fades), `heart_sound_player.rs` (`DEVICE_FACTORY`, `AUDIO_BUILD`, `with_factory_opt`, `DisplayDelay`, `QUEUE_TARGET_MS`),
`app/listening.rs`, `settings.rs` (`settings_window_full`); `crates/heartsim-web/src/` (`web.rs`, `lib.rs`, how
`window.heartsimStatus` is filled) and `crates/heartsim-web/tests/browser`. For the browser API read MDN, do not write it from
memory: `AudioContext` (`state`, `resume`, `currentTime`, `sampleRate`, `baseLatency`, `outputLatency`), `AudioBuffer`,
`AudioBufferSourceNode.start(when)`, `GainNode`, the autoplay policy pages of Chrome, Firefox and WebKit, and the
"A tale of two clocks" scheduling article (web.dev).

## Non-negotiables for this brief
* **Write-only audio**, as in B25: nothing flows from `AudioContext.currentTime` into `SimClock`, the engines or the rings. ECG and
  PCG goldens stay unchanged; rings are identical with sound on or off.
* **Never early**: a sound may be heard a constant delay after its drawn position, never before.
* **Static site**: no threads, no `SharedArrayBuffer`, no AudioWorklet that needs special headers, no new server requirement.
  `cargo xtask web-budget` still passes.
* **No sound without a gesture, no error without one either**: a suspended context is a normal state shown in the UI, not a
  failure. A browser without Web Audio shows "sound is not available in this browser" and stays silent.
* The desktop build is untouched in behaviour: the kira path, its tests and the `audio` feature keep working, and the web code
  is compiled only for `wasm32`.
* All timing decisions are in pure code tested natively with a fake clock; only a thin layer calls `web-sys`.
* New constants are named, documented and marked **designed** or sourced. Nothing comes from `epicardio/`.

## Scope
1. **Sink selection at run time.** Today `DEVICE_FACTORY` / `AUDIO_BUILD` are compile-time constants tied to the `audio` feature.
   Let the embedding crate provide the factory: `heartsim-web` registers its Web Audio factory when it creates the app, the
   desktop keeps the kira factory, and "is sound available in this build" becomes "is there a factory". The Display-popup switch
   and the Settings card then appear on the web without a `cfg` in `heartsim-ui`.
2. **Latency owned by the sink.** `PcmSink` gains a way to report its own constant delay (queue target plus device latency, ms)
   so that `DisplayDelay` uses the sink's figure instead of the fixed `QUEUE_TARGET_MS`: 40 ms for kira as today (no behaviour
   change on desktop), look-ahead plus reported latency on the web. The user's audio offset still adds to it.
3. **Scheduler** (pure, `heartsim-app/src/sound_out.rs` or a new `web_schedule.rs`, always compiled, tested natively). Given the
   context clock read at each push, decide the start time of each chunk: contiguous with the previous chunk; never before
   `now + LOOKAHEAD_MS` (designed, about 100 ms, enough for one long main-thread frame); after a gap (underrun, hidden tab,
   suspended context) re-anchor at `now + LOOKAHEAD_MS` with the 10 ms fade-in; `queued_ms` = scheduled end − now; `clear` cancels
   what has not started. Chunks shorter than about 20 ms are merged before scheduling so a 144 Hz display does not create
   hundreds of nodes per second.
4. **Web Audio sink** (`heartsim-web/src/audio.rs`, `wasm32` only). Implements `PcmSink`: resample each chunk from 4000 Hz to
   `AudioContext.sampleRate` with the existing `Resampler` (an `AudioBuffer` below 8000 Hz is not guaranteed by the standard),
   apply gain and `soft_limit` in the samples, wrap in an `AudioBuffer`, and `start(when)` an `AudioBufferSourceNode` at the
   scheduler's time through one `GainNode` used for the clear fade. Keep handles of nodes not yet finished so `clear` can stop
   them. Latency = look-ahead + `baseLatency` + `outputLatency` where the browser reports them (treat a missing or zero
   `outputLatency` as zero and say so in known issues). Add the needed `web-sys` features only to `heartsim-web`.
5. **Gesture gate.** States: *off*, *waiting for a click*, *running*, *unavailable*. The context is created, or `resume()`d, from
   a real DOM event: install `pointerdown` and `keydown` listeners on the page that resume the context whenever the sound
   setting is on and the context is not running (egui handles clicks in its frame, which is too late to count as a gesture in
   Safari). Switching the setting on by a click therefore starts the sound on that same click or the next one. After a page
   reload with the setting on, the app shows "Click anywhere to enable sound" in the Display popup's status note and as a small
   notice near the heart-sound lane, and starts on the first click. Handle `suspended` and, where it exists, `interrupted`
   (phone call, iOS) the same way.
6. **Tab and page life cycle.** When the tab is hidden the simulation stops being stepped; what is already scheduled plays out
   and then there is silence. On return the player re-anchors (no burst of late audio, no fast-forward). Suspend the context on
   `visibilitychange` to hidden and resume on visible when the setting is on.
7. **Everything else comes from B25 unchanged** and must work on the web through the same player: volume, audio offset, the sync
   check, muting above 1×, slow-motion re-synthesis, pause, play from cursor with the moving playhead, re-anchoring on scenario
   load. Settings persist in the web build's existing storage.
8. **Status for tests.** `window.heartsimStatus` gains an `audio` object: state, context sample rate, chunks scheduled, queued
   ms, latency ms. A URL parameter (for example `?sound=1`) switches the setting on at start so the smoke test does not have to
   find an egui widget.
9. **Docs.** `docs/web.md` (audio section rewritten; manual matrix gains the sound checks), user book (the listening section
   says it also works in the browser, and what "click to enable sound" means), `known-issues.md`, `CHANGELOG.md` under
   Unreleased, phase 3 ticked in `docs/ROADMAP.md`.

## Out of scope
The monitor beep on the web. AudioWorklet or any threaded audio. S3, S4, murmurs, listening sites (phase 4). Tutorial, quiz,
recordings, WAV export (phase 5). Changing the desktop latency model.

## Acceptance tests
* Scheduler, native, fake clock:
  * steady pushes at 60 Hz and at 144 Hz frames: chunk start times are contiguous to the sample, and none is earlier than
    `now + LOOKAHEAD_MS` at the moment it is scheduled, except chunks that continue a running stream;
  * a 400 ms gap between pushes re-anchors at `now + LOOKAHEAD_MS`, with one fade-in and no overlap;
  * `clear` reports zero queued and the next push re-anchors;
  * a 2 s hidden-tab gap followed by pushes schedules nothing in the past;
  * chunk merging: no scheduled chunk is shorter than the minimum except the last before a clear.
* Player with a fake sink that reports a web-like latency: the display delay equals the sink's latency plus the offset; with the
  kira-like figure it equals today's value (existing B25 tests unchanged).
* Factory selection: with no factory the switch and Settings card are absent; with a registered factory they are present; a
  factory that fails shows the "not available" notice; the factory is not called while the setting is off.
* Determinism: ECG and PCG goldens unchanged; rings identical with sound on or off.
* Builds: `cargo check --target wasm32-unknown-unknown -p heartsim-web` (and clippy for that target if CI runs it),
  `cargo check -p heartsim-app --no-default-features --lib`, `trunk build --release`, `cargo xtask web-budget`, `cargo xtask ci`.
* Browser smoke test (Playwright, headless Chromium): open with `?sound=1`; `heartsimStatus.audio.state` is "waiting" before any
  input; after one click on the canvas it becomes "running", the scheduled-chunk count rises over two seconds and queued ms
  stays between 50 and 300; after hiding and showing the page it is "running" again and nothing was scheduled in the past; no
  panic and no console error.
* Manual, reported honestly as not done where it was not: Chromium, Firefox and Safari (macOS and, if available, iOS): switch on,
  hear S1/S2, run the sync check and note the offset, slow motion, pause and play from cursor, switch tab and return, reload
  with sound on.

## Decisions for the maintainer
* Look-ahead (proposed 100 ms): a longer one survives slower frames but makes the drawn views lag more while sound plays.
* Whether the "click to enable sound" notice near the lane is wanted, or the status note in the Display popup is enough.
