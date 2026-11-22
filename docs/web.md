# heartsim in the browser (brief B21)

`crates/heartsim-web` builds the same `HeartSimApp` as the desktop program for `wasm32-unknown-unknown`. eframe's web runner
mounts it on a canvas and wgpu draws it with **WebGPU**, or with **WebGL2** when the browser has no `navigator.gpu`.

## Build and run

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --locked          # or: cargo binstall trunk
cd crates/heartsim-web
trunk serve                           # http://127.0.0.1:8080, rebuilds on change
trunk build --release                 # static site in crates/heartsim-web/dist/
cargo xtask web-budget                # wasm + assets must be < 25 MiB
```

`trunk` runs `cargo xtask web-assets` first (a `pre_build` hook in `Trunk.toml`). That writes
`heartsim-assets.zip`: the default anatomy pack, the rhythm library, the tutorials, the patient cases and the real heart-sound recordings (`sounds/reference`), laid out like `assets/`. The wasm itself
carries no anatomy. The page downloads the archive while the loading screen is up, unpacks it with the `zip` crate and hands it
to `HeartSimApp::with_assets` as an in-memory `AssetBundle`. If the download fails the app still starts, with the placeholder
sphere and no rhythm library.

The site needs **no special headers**: there are no threads, so no COOP/COEP. All URLs are relative (`public_url = "./"`), so it
works from a sub-path such as `https://<user>.github.io/heartsim/` and from any static file server.

`wasm-bindgen` is pinned with `=` in the workspace manifest. Trunk downloads the `wasm-bindgen` command-line tool that matches
`Cargo.lock`; bump the crate and the lock file together.

## What the web build drops

| desktop feature | in the browser | why |
|---|---|---|
| Export / Import window (PDF, PNG, CSV, WFDB, share bundle, scenario `.ron`, drag-and-drop import) | gone (`export` feature off, so `heartsim-io` and `printpdf` are not linked) | needs files; the PDF fonts also cost size |
| "Save PNG" of the 12-lead page, "Save/Load placement", "Save scenario" in the timeline editor | hidden | plain files in the working directory |
| Asset discovery on disk (`$HEARTSIM_ASSETS`, next to the executable, per-user data directory) | replaced by the downloaded archive | `native` feature off |
| Tutorial progress file | `localStorage` through eframe's storage | `native` feature off |
| rayon (Tier 1 batches, Tier 2 slabs, forward-matrix gathers, scalar mapping) | single thread (`parallel` off) | no threads without COOP/COEP and `wasm-bindgen-rayon`; results are identical, only slower |
| Screenshot tour (`HEARTSIM_SCREENSHOT_DIR`) | inert | no environment |
| `--version`, logging to the terminal | none; panics and `log` output go to the browser console | |

## Audio (brief B26)

The desktop's `audio` feature (`kira`, `rtrb`, the monitor beep) is not compiled for the web. The **heart sounds** are: the
same `HeartSoundPlayer` as on the desktop (`heartsim-app/src/heart_sound_player.rs`) writes to a Web Audio sink
(`heartsim-web/src/audio.rs`, `wasm32` only) that `heartsim-web` registers when it creates the app
(`HeartSimApp::set_heart_sound_output`). The *Play heart sounds* switch and the Settings card appear wherever a sink factory
exists; volume, audio offset, the sync check, slow motion, muting above 1×, pause and *Play* from the cursor work as on the
desktop. The monitor beep stays desktop-only.

* **Scheduling.** There is no audio thread: each chunk of the phonocardiogram is resampled from 4 kHz to the context's
  `sampleRate` (an `AudioBuffer` below 8 kHz is not guaranteed), given gain and the soft limiter in the samples, wrapped in an
  `AudioBuffer` and started with `AudioBufferSourceNode.start(when)` through one `GainNode` that only fades out on a clear. The
  start times are decided by `heartsim-app/src/web_schedule.rs` (pure, tested natively with a fake clock): contiguous to the
  sample; a new stream no earlier than `LOOKAHEAD_MS` (100 ms) after `currentTime`, placed so that its newest sample is heard
  100 + 40 ms after the push; continued through a long frame as long as the scheduled audio has not run out (a frame up to
  about 120 ms is absorbed), otherwise re-anchored with a 10 ms fade-in;
  pushes under 20 ms merged; the resampling ratio nudged by at most 0.5 % to absorb the drift between the audio clock and the
  frame clock. The views are drawn the look-ahead, the 40 ms queue, `baseLatency` and `outputLatency` (where reported) plus the
  user's offset behind the engine; the offset defaults to 0 ms in the browser. Nothing from `currentTime` reaches the
  simulation.
* **Gesture gate.** States *off*, *waiting*, *running*, *unavailable*. The factory creates no `AudioContext`; the context is
  created, or `resume()`d, from `pointerdown`, `pointerup`, `mousedown` and `touchend` on the canvas and `keydown` on the
  window (capture), whenever the setting is on and the context is not `running` (`suspended` and iOS's `interrupted` alike).
  eframe runs the app logic inside its own canvas listeners, installed before ours, so a click that ticks the switch starts the
  sound on that same click. While waiting, the Display popup, the Settings card and the lane say *Click anywhere to enable
  sound*. A browser without `AudioContext` shows *Sound is not available in this browser*.
* **Life cycle.** On `visibilitychange` to hidden, what is scheduled plays out and the context is suspended; on visible the old
  stream is dropped, the context resumed and the player's next push starts a new stream.
* **Static site.** No threads, no `SharedArrayBuffer`, no AudioWorklet, no headers.
* **Status and URL.** `window.heartsimStatus.audio` = `{ state, sampleRate, chunks, queuedMs, latencyMs, late }`; `?sound=1`
  switches the sound on at start (the smoke test uses it).

Persistence: eframe's `persistence` feature stores the Rhythm Quiz score, the tutorial progress and the Settings window's choices
(language, high contrast, reduced motion) in `localStorage`.

Language: the Fluent catalogues are compiled into the wasm; the language follows `navigator.language` (`sys-locale` with its
`js` feature) until the user picks one in Settings.

Re-entry presets (Tier 2, brief B17): the preset list and the anatomy pack come from the downloaded archive
(`scenarios/reentry/`, `anatomy/default/`). They run on one thread with a `Date.now()` frame budget (`std::time::Instant` panics
in a browser), so the achieved speed shown in the controls will be below the desktop's. This path is covered by native tests
(bundle presets equal disk presets, the pack loads from the kept subset) but has not been run in a browser.

## WebGL2 fallback

wgpu's GLES backend (WebGL2) has no compute shaders, no storage buffers in the vertex stage and no `@interpolate(linear)`.
The renderer uses none of them: per-vertex scalars are vertex attributes, colours come from a 2-D LUT texture sampled in the
fragment shader, and the simulation runs on the CPU. `crates/heartsim-render/tests/webgl2_shaders.rs` fails if a shader
starts using one of these constructs.

## Size budget

`cargo xtask web-budget` sums the `.wasm` files and the asset archive in `dist/` and fails above 25 MiB. The archive is about
5.7 MiB of deflate (the zip format uses the same algorithm as gzip) for 9.6 MB of pack files; the wasm is about 9.4 MiB after `wasm-opt -Oz`, so a release build measures 15.1 MiB.
Servers that gzip or brotli `.wasm` on the fly make the transfer smaller still.

## Tests

* Native, no GPU: `cargo test -p heartsim-web -p heartsim-app -p heartsim-render -p xtask` (archive unpacking, bundle loaders
  against the disk loaders, pack checksum, WebGL2 shader constraints, archive contents).
* `cargo check --target wasm32-unknown-unknown -p heartsim-web` with `RUSTFLAGS=-D warnings` (CI job *Web*).
* Browser smoke test (`crates/heartsim-web/tests/browser`, Playwright): serves `dist/`, opens it in headless Chromium on
  software WebGL (set `HEARTSIM_BROWSER_PATH` to use an installed Chromium) and checks `window.heartsimStatus` — the pack loaded, frames are drawn, the strip's sample count and the
  simulation clock advance — plus a non-blank screenshot and no panics. With `?sound=1` and Chromium's
  `--autoplay-policy=user-gesture-required` it also checks the heart sounds: *waiting* before input, *running* after one click
  on the canvas, chunks scheduled over two seconds with 50–300 ms queued, *running* again after the page is hidden and shown,
  nothing scheduled in the past, no audio errors in the console. CI job *Web / smoke*.

## Manual test matrix

Run `trunk serve` (or open a deployed build) and go through: loading screen disappears; heart visible in the Simulator tab and
rotating with the mouse; strip scrolls and the beat on the heart matches the QRS; pick a rhythm; 12-lead tab; Electrodes tab
(drag an electrode); a tutorial page and its check; Rhythm Quiz; reload the page and see the quiz score again; resize the
window (canvas follows); background the tab and return; Tools → Echo calculators: load a worked example, change a value,
reload and see the favourite kept and the typed values gone.

Sound (B26), with headphones: switch on *Play heart sounds* and hear S1/S2 (on the click, or the next one); run *Check sync* in
Settings and note the offset at which click and dot coincide; slow motion (0.25×) stretches an RBBB's S2 split; above 1× it is
muted; pause, then *Play* in the lane with the moving playhead; switch to another tab and return (no burst, restarts from the
present); reload with sound on (*Click anywhere to enable sound*, then the first click starts it).

| browser | GPU API expected | status |
|---|---|---|
| Chrome / Edge 113+ (Windows, macOS, ChromeOS) | WebGPU | to be filled in by the tester |
| Chrome on Linux | WebGL2 (WebGPU behind flags on most GPUs) | |
| Firefox 141+ (Windows), 145+ (Apple Silicon macOS) | WebGPU | |
| Firefox on Linux / Intel macOS | WebGL2 | |
| Safari 26 (macOS Tahoe, iOS 26) | WebGPU | |
| Safari 17-18 | WebGL2 | |
| Sound: Chromium, Firefox, Safari macOS, Safari iOS | — | not done (no browser with a sound device was available) |

Record browser version, GPU API (`navigator.gpu` present?), whether the check list passed, and the frame rate of the Simulator
tab. Safari and Firefox are listed because the brief asks for them; they have not been tested for this change.

## Publishing

GitHub Pages serves the guide at <https://inalto.github.io/heartsim/> and the app at
<https://inalto.github.io/heartsim/app/>. A Pages site has one source, so one workflow deploys both:
`.github/workflows/docs.yml` builds the mdBook, and on `master` also runs `trunk build --release` and the size budget, copies
`dist/` into the book's output as `app/`, and deploys the result. It runs on every push to `master` that touches the docs, the
crates, the assets or the lock file. The app's URLs are relative, so it works from that sub-path unchanged.

`.github/workflows/web.yml` is the check for pull requests: it builds, checks the budget and runs the smoke test, uploads
`dist/` as an artifact and publishes nothing. The deployed copy is therefore built by `docs.yml`, not the one the smoke test
ran against; both come from the same commit. Any static host works for `dist/`.
