# B39 — Interval drill: caliper handles, pointer shapes and a zoom
**Crates** `heartsim-ui`, `heartsim-app`, docs · **Model** Sonnet · **Milestone** M13 · **Depends on** B38 (strip zoom
gesture), B40 (the record window that starts at the left). Read both briefs and the code they left.

## Goal
In the interval quizzes (the Interval drill page) the calipers are easy to grab and say so: each line has a visible
handle, the pointer changes over it, and the strip can be zoomed and panned to place a line precisely. The keyboard
path keeps working.

## What is there today (read first)
`crates/heartsim-ui/src/calipers.rs`: `CaliperSet` (pairs in milliseconds from the left edge, `hit`, `selected`,
`drag`), `strip_interact` (drag a line or the band, shift-click adds a pair, ←/→ nudge, Delete/Esc clear, drag
elsewhere scrubs when paused), `draw_calipers` (two plain lines, a band, a read-out). There is no handle and no cursor
change. `crates/heartsim-app/src/interval_drill.rs`: `IntervalDrill::strip` (fixed `DRILL_SPEED_MM_S` = 50 mm/s,
lead II), `caliper_ms`, the measurement box. The same calipers are used on the Simulator strip through `EcgView`: the
handles and cursors are for every strip, the zoom controls of this brief are the drill's.

## Work
**Handles (all strips).**
* Draw a handle on each caliper line: a filled grip at the top and at the bottom of the line (a rounded tab about
  12 × 18 points with the line running through it; larger when selected), in `style.caliper` with a contrasting
  outline so it shows on the pink paper and on the dark monitor. The band between a pair gets a small grip on the
  read-out bar for "move both".
* Hit targets: a line is grabbed within ±6 points of it along its whole height, and within the handle's rect
  enlarged to at least 24 × 24 points (44 × 44 when the last input was a touch: `ctx.input(|i| i.any_touches())` or
  the pointer's kind). When two lines are closer than the target, the nearer one wins; the band wins only outside
  both line targets. Keep `CaliperSet::hit` the single place that decides, and give it the y position.
* Pointer shapes, set on hover and held during the drag: `CursorIcon::ResizeHorizontal` over a line or its handle,
  `Grab` over the band grip and `Grabbing` while moving the pair; while paused and over free paper keep what the
  strip shows today.
* The handles must not hide the trace under them more than they need to: keep them at the top and bottom edges.

**Zoom and pan (the drill).**
* The drill's strip gets a time zoom from 25 to 400 mm/s equivalent, starting at 50 mm/s as today, with the gesture
  function of B38 (pinch, Ctrl/⌘ + wheel, and the plain wheel if B38's rule allows it here: the drill's strip is not
  inside a scroll area, check) and three buttons next to the strip: zoom out, zoom in, "Fit" (back to 50 mm/s from
  the start of the record). Give the buttons text or hover text and Fluent keys in all five languages.
* The strip is a finished record (B40). Zooming keeps the sample under the pointer in place; zoomed in, the strip
  pans with a horizontal drag on free paper (the scrub drag of `strip_interact`, mapped to the record window's first
  sample), with a horizontal wheel or two-finger swipe, and with a thin scroll bar or position marker under the
  strip that shows which part of the record is in view. The pan is clamped to the record.
* The calipers stay on their samples through zoom and pan (store them so that this holds: for the record window,
  milliseconds from the start of the record rather than from the left edge, or convert on every change; say which in
  the doc comment). The read-out and `caliper_ms` do not change with the zoom.
* The grid stays truthful at every zoom (B38's rule); the page states the scale in words next to the strip
  ("1 mm = 20 ms" at 50 mm/s, recomputed).
* The tolerance text and the grading are untouched. A new question resets the zoom and the pan.

**Keyboard.** ←/→ nudge the selected line by one sample (Shift: ten) as today; add Tab to move the selection to the
next line while the strip has focus if that does not fight egui's focus traversal (if it does, use `[` and `]` and
document it); `+`/`-` zoom, `0` fits. Keep Delete/Esc.

## Acceptance tests
* `calipers.rs` (pure): `hit` returns the line within ±6 points at mid-height and within the handle rect at the top
  and bottom; with a touch-sized target the rect is at least 44 points; between two lines 8 points apart the nearer
  wins; the band is hit only outside the line targets.
* `egui_kittest` (follow the existing GUI tests): hovering a handle sets `ResizeHorizontal`
  (`ctx.output(|o| o.cursor_icon)`), hovering the band grip sets `Grab`; dragging a handle 50 points moves that end by
  `50 / px_per_ms` ms and not the other; the same drag at 2× zoom moves it by half as many milliseconds.
* Drill: zoom in/out/fit change the effective speed within 25–400; a caliper pair placed on two samples stays on
  them (within one sample) after zoom in, pan and fit; `caliper_ms` is unchanged by zoom; `next_question` resets zoom
  and pan; the keyboard nudge still moves one sample at any zoom.
* The existing drill tests pass unchanged. `nice -n 10 cargo xtask ci`.

## Docs
`docs/book/src/user/quizzes.md` (the drill: handles, zoom, pan, keys), `ui-tour.md` where the calipers are described,
`known-issues.md` (cursor shapes and touch targets **not seen on a real screen**), `CHANGELOG.md` `## [Unreleased]`.

## Rules
One cargo build at a time, always `nice -n 10`, `export PATH="$HOME/.cargo/bin:$PATH"`. Do not commit or stage, do not
edit `CLAUDE.md`. Report what you did not verify.
