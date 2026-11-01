# B7 — ECG strip and 12-lead pages
**Crates** `heartsim-ui` (`src/strip.rs`, `src/twelve_lead.rs`, `src/calipers.rs`), `heartsim-app` · **Model** Sonnet ·
**Milestone** M1 · **Depends on** B4 (annotations, intervals).

## Goal
Make the strip clinical-grade: formats, sweep speeds and gains, monitor sweep-bar mode, pause + scrub synchronised with the
simulation clock, calipers, wave labels, interval read-outs, HiDPI correctness, ≥ 60 fps for 12 leads.

## Read first
`crates/heartsim-ui/src/strip.rs` (painter + decimation), `crates/heartsim-app/src/app.rs`, `docs/research/rust-stack.md` §3 (strip
design notes), `docs/research/reference-apps.md` §2.8 and §6 P0-4/9/10 (what Epicardio/cardiacrhythmsim offer),
`docs/REQUIREMENTS.md` F-12, F-13.

## Scope
1. `StripStyle` gains 5/10/20 mm/mV, speeds 12.5/25/50/100/200, `px_per_mm` calibration slider in settings, paper on/off, dark theme.
2. Modes: scrolling (exists) and **sweep bar** (fixed trace, erase bar moving left→right).
3. 12-lead page layouts 3×4 (+ rhythm strip II), 6×2, 12×1 with lead labels and shared time base; Cabrera order option.
4. **History and scrub**: rings of ≥ 60 s; pausing freezes the ring and exposes a cursor time; dragging the cursor sets
   `app.scrub_time` which the viewport and vector panels read (the engine stores V_m snapshots at 10 Hz for the last 60 s —
   implement a `SnapshotRing` in `app`; B9 consumes it).
5. Calipers: two draggable vertical cursors per strip; Δms and bpm; shift-click adds a pair; keyboard nudge; reset.
6. Wave labels from B4 annotations (toggle); PR/QRS/QT/QTc/HR/axis read-out panel.
7. Snapshot: render the current page to an egui texture/PNG (uses `plotters` or the painter shapes → `image`).
8. Performance: decimation only for the visible window; cache grid shapes per rect; measure with `puffin` optional.

## Acceptance tests
* `egui_kittest` harness: strip renders with 12 rings × 30 s at 500 Hz in < 4 ms/frame (measure with `Instant` in a test, loose
  threshold 20 ms in CI).
* Caliper on a synthetic 60 bpm signal reads 1000 ± 10 ms / 60 bpm.
* Changing sweep from 25 to 50 mm/s halves the seconds visible; gain 20 doubles amplitude in pixels.
* Pause → scrub back 5 s → the cursor time reported equals `now − 5000 ms ± 1 sample`; resume continues from `now` (no time jump).
* Snapshot PNG produced with the expected size.
