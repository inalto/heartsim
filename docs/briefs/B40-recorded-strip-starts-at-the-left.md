# B40 — A recorded strip starts at the left edge (rhythm quiz bug)
**Crates** `heartsim-ui`, `heartsim-app`, docs · **Model** Sonnet · **Milestone** M13 · **Depends on** B7, B8.
Do this before B39 (B39 builds on the window it introduces).

## The report
"Sometimes the trace doesn't start from the left in the rhythm quizzes."

## Suspected cause (from reading the code on 2026-11-27; confirm it with a failing test before changing anything)
`rhythm_quiz::record_strip` returns a fixed recording: `RECORD_S - SETTLE_S` = 10 s at 500 Hz. The quiz draws it with
`heartsim_ui::twelve_lead::lead_column` → `strip::ecg_strip` → `draw_strip` in `SweepMode::Scroll`, whose
`StripWindow` puts the newest sample at the **right** edge. The quiz takes the paper speed of the main ECG view
(`self.ecg.style`). At 25 mm/s and 3 points per millimetre 10 s are 750 points: on a strip wider than that (most
desktop windows; any window at 12.5 mm/s) the recording is right-aligned and the paper to its left is empty, so the
trace "does not start from the left". On a narrower strip, or at 50 mm/s and above, only the last part is shown and the
strip looks full. That is the "sometimes". The interval drill (`interval_drill.rs`, fixed 50 mm/s, 1500 points for
10 s) has the same fault on very wide windows.

If the failing test shows a different or a second cause (state carried from one question to the next, the sweep mode
leaking in, a warm-up offset), report it and fix that; do not paper over it.

## Work
1. Reproduce: a test that builds the quiz strip's `StripWindow` for a 10 s ring on a 1200-point-wide rect at 25 mm/s
   and asserts that the first sample is drawn at the left edge. It fails today.
2. Give a finished recording its own alignment. In `strip.rs`, a window for a **static record**: the first sample of
   the visible part at the left edge. When the record is shorter than the strip, the trace starts at the left and the
   paper to the right of its end is empty; when it is longer, the window shows `visible_samples` starting at a
   `first` sample (0 by default; B39 pans it). Do it in `StripWindow` itself (for example a third alignment beside
   scroll and sweep, or a constructor `StripWindow::record(rect, style, fs, first, len)`), so `segments`,
   `x_of_sample`, `sample_at_x`, the wave labels and the calipers keep sharing one mapping. Do not shift the rect and
   do not pad the ring with zeros.
3. Use it for the rhythm quiz (both the two-lead column and the 12-lead layout: check `twelve_lead_page`, whose cells
   are narrow, and decide per cell with evidence) and for the interval drill. The live strips (Simulator, 12-lead
   page, EP lab) keep scroll and sweep unchanged.
4. Decide, with the numbers, whether 10 s is enough: at 12.5 mm/s a 1900-point strip shows 50 s. Either record what
   the widest reasonable strip needs (cost: `Tier0Scenario::record` time, measure it) or keep 10 s and leave the rest
   of the paper empty. Say which and why in the module doc. The interval drill's fairness tests
   (`every_question_has_a_fair_reference`) must not change their outcome.
5. Check the other pages that draw a recording rather than the live rings (grep `record_strip` and `ecg_strip`): the
   auscultation quiz, the sound trainer, tutorials. Fix those that share the fault, list those that do not.

## Acceptance tests
* The reproducing test, now passing; the same at 12.5, 25 and 50 mm/s and for widths 400, 750, 1200 and 2400 points:
  the first drawn sample is at `rect.left()` (within one sample) and no sample is drawn outside the rect.
* `sample_at_x`/`x_of_sample` round-trip in the record window; calipers placed in milliseconds from the left read
  the same interval as before.
* Two questions in a row (`next_question` twice, different lengths if the recording length now varies): the second
  strip also starts at the left (guards against carried state).
* Existing strip, quiz and drill tests still pass; `nice -n 10 cargo xtask ci`.

## Docs
`CHANGELOG.md` under `## [Unreleased]` (`### Fixed`), with the cause in one sentence; `docs/book/src/user/quizzes.md`
if it describes the strip.

## Rules
One cargo build at a time, always `nice -n 10`, `export PATH="$HOME/.cargo/bin:$PATH"`. Do not commit or stage, do not
edit `CLAUDE.md`. Report the root cause you found and what you did not verify (nothing is seen on a real screen).
