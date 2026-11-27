# B38 — Pinch-to-zoom in the 3-D views and time zoom on the ECG strip
**Crates** `heartsim-ui`, `heartsim-render` (camera only), `heartsim-app`, docs · **Model** Sonnet · **Milestone** M13 ·
**Depends on** B6, B7.

## Goal
1. Every 3-D view zooms with a two-finger pinch: on a touch screen (the browser build on a tablet or phone) and on a
   laptop trackpad. The mouse wheel keeps zooming as today.
2. The time axis of the ECG strip zooms with the mouse wheel and with a pinch. Zoom is a drawing matter only: it changes
   the paper speed the strip is drawn at and nothing in `heartsim-sim` or `heartsim-ecg` (non-negotiable 2).

## What is there today (read first)
* `crates/heartsim-ui/src/viewport.rs`: the one 3-D widget, `Viewport3d::ui_with_orbit`. It reads
  `smooth_scroll_delta.y` and passes `scroll / 40` to `Camera::zoom` inside the wgpu paint callback. All four 3-D views
  go through it (`heart_view.rs`, `electrode_tab.rs`, `echo_page.rs`, the placeholder in `app.rs`), so the fix is in
  one place. `crates/heartsim-render/src/camera.rs`: `Camera::zoom(scroll)` multiplies the distance by
  `1 - 0.1·scroll` and clamps it.
* `crates/heartsim-ui/src/strip.rs`: `StripStyle.mm_per_s` (one of `SPEEDS_MM_S`), `StripWindow` (the single mapping
  from samples to x), `draw_strip`. `crates/heartsim-ui/src/ecg_view.rs`: `EcgView` owns the style, the speed combo
  (`controls_with`, about line 320), pause and scrub (`calipers::Playback`), `strip_with_sounds` and `page_with_sounds`.
  `crates/heartsim-ui/src/calipers.rs`: `strip_interact` (drag, keys); caliper positions are milliseconds from the
  strip's left edge.
* egui 0.37 input: `InputState::zoom_delta()` is a multiplicative factor that already combines a touch pinch
  (`multi_touch()`), a trackpad magnify gesture and Ctrl + wheel (⌘ on macOS); while Ctrl is held egui reports no
  scroll. Verify these statements in the egui 0.37 source in `~/.cargo/registry` before relying on them.

## Design decisions (made; do not reopen without evidence)
**3-D views.** Zoom = wheel (as today) × pinch factor. Add `Camera::zoom_by(factor)` (distance divided by the factor,
same clamp) and apply `zoom_delta()` when the pointer is over the view or a touch is on it. While two fingers are down
(`multi_touch().is_some()`), a drag must not orbit the camera, and it must not start an object drag in the views that
have one (`orbit == false` callers): pass that through. Put the reading of the input into a pure function
(`fn zoom_input(input: &egui::InputState, hovered: bool) -> ZoomInput { wheel, factor, pinching }`) so it is tested
without a GPU.

**ECG strip.** One zoom factor per `EcgView`, `time_zoom` (1.0 = the selected paper speed), clamped so that the
effective speed `mm_per_s × time_zoom` stays within 6.25–400 mm/s. The grid stays truthful: the strip is drawn at the
effective speed exactly as if that speed had been chosen (1 mm squares keep their size, the time per square changes).
* The speed combo shows the effective speed when it is not a preset (for example "37 mm/s") and gains a "Reset zoom"
  entry; choosing a preset sets `time_zoom = 1`. Exports, the printed page and the interval drill's fixed speed keep
  using the preset (`style.mm_per_s`): add `EcgView::effective_style()` for drawing and leave `style` as the stored
  choice. Check every user of `self.style.mm_per_s` in `ecg_view.rs` and decide which of the two it needs.
* Gestures, when the pointer is over a strip (or a touch is on it):
  - pinch, trackpad magnify, Ctrl/⌘ + wheel: `time_zoom *= zoom_delta()`, everywhere a strip is drawn;
  - plain vertical wheel: zooms too (one notch about 10 %), **only** where the strip does not sit inside a scroll
    area that the wheel would otherwise scroll. Find out which strips those are (the Simulator strip and the 12-lead
    page are expected to qualify) and list them in the code comment and the book; where a strip is inside a scroll
    area the plain wheel keeps scrolling. When the strip takes the wheel, consume it (`ui.input_mut`, zero the scroll
    deltas) so nothing behind scrolls;
  - double-click on the strip does not reset the zoom (double-click is not used today; leave it free).
* What stays still while zooming: running, the newest sample stays at the right edge (scroll mode) and the sweep page
  recomputes (sweep mode). Paused, the sample under the pointer (or the pinch centre) stays under it: adjust the scrub
  offset, clamped as `Playback::set_scrub_ms` clamps. Paused, the calipers stay on the samples they were on (convert
  their milliseconds-from-left for the new window); running, they keep their interval.
* The heart-sound lane and the timeline marks use the same `StripWindow`, so they follow without separate code; check.
* `time_zoom` is not persisted (a fresh start is at the chosen paper speed).

Put the gesture reading in one function in `heartsim-ui` (`strip.rs` or a new `zoom.rs`) that returns the factor for a
strip rect, so B39 can reuse it for the interval drill:
`pub fn strip_zoom_factor(ui: &mut Ui, rect: Rect, plain_wheel: bool) -> Option<(f32, f32)>` (factor and the x it
is anchored at), or a shape you find better; document it.

## Acceptance tests
* `camera.rs`: `zoom_by(2.0)` halves the distance, respects the clamp; `zoom_by(1.0)` is a no-op.
* `viewport.rs` (pure): with `egui::Event::Zoom(1.25)` (and with two `Event::Touch` points moving apart, if egui's
  test input makes that practical) `zoom_input` returns a factor above 1; with a plain wheel event it returns the
  wheel and factor 1; not hovered and no touch gives nothing; `pinching` is set only with two touches.
* `strip`/`ecg_view`: (a) the factor is clamped at both ends of 6.25–400 mm/s; (b) `StripWindow` built from the
  effective style has `px_per_ms` scaled by the factor and `visible_samples` divided by it; (c) paused zoom about a
  pointer x keeps `sample_at_x(x)` within one sample; (d) paused zoom keeps a caliper line on its sample within one
  sample; (e) choosing a preset resets the factor; (f) the export path still reports the preset speed.
* An `egui_kittest` test in `crates/heartsim-ui/tests/` or `crates/heartsim-app/tests/` (follow the existing ones,
  for example `axis_panel`): a strip is drawn, a Ctrl + wheel event over it changes the effective speed, the same
  event away from it does not, and a plain wheel event over a strip inside a `ScrollArea` scrolls and does not zoom.
* Pure crates first, then `nice -n 10 cargo test --workspace --test <name>` for a GUI test, then
  `nice -n 10 cargo xtask ci`.

## Docs
`docs/book/src/user/ui-tour.md` (the strip and the 3-D view: the gestures, with the modifier rule in one table),
`docs/book/src/user/faq.md` if it mentions zooming, `docs/web.md` (touch), `known-issues.md` (**not seen on a real
touch screen or trackpad**: the build machine has no display), `CHANGELOG.md` under `## [Unreleased]`. Strings go
through Fluent (`assets/locales/*/app.ftl`, all five languages; keep the keys in the same order).

## Rules
One cargo build at a time, always `nice -n 10`, `export PATH="$HOME/.cargo/bin:$PATH"`. Do not commit or stage, do not
edit `CLAUDE.md`. Report what you did not verify.
