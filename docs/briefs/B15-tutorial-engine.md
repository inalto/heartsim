# B15 — Tutorial engine and scenario timeline editor
**Crates** `heartsim-app`, `heartsim-ui` · **Model** Sonnet · **Milestone** M3 · **Depends on** B7, B9, B11.

## Goal
Markdown-based guided tutorials whose pages can drive the simulator (load scenario, set parameter, pause at time, focus camera,
highlight a tool, ask a question), with progress tracking; and a timeline editor to author scenario `TimedAction`s visually.

## Read first
`crates/heartsim-core/src/scenario.rs` (`Action`, `TimedAction`), the private reference-product analysis notes (not published) §1 "Education"
(index lines, knowledge checks, tutor vs self-study), `docs/research/reference-apps.md` §6 P1-7/8.

## Scope
1. Tutorial format: `assets/tutorials/<id>/tutorial.ron` = ordered pages { markdown, actions: Vec<Action|UiAction>, check:
   Option<Question> }; `UiAction` = FocusPanel, Highlight(tool id), CameraPreset, SetSpeed, Pause, WaitForEvent(kind).
2. Renderer: `egui_commonmark` for text, next/prev, progress, "run actions" automatically on page enter; tutor mode (free
   navigation) vs self-study (must complete checks).
3. Timeline editor: table of timed actions with add/edit/delete, a scrub bar showing action markers on the strip, save to scenario.
4. Progress persistence per tutorial in settings dir.

## Acceptance tests
* Tutorial file roundtrip; executing a page's actions mutates the model exactly as applying the actions directly (unit test with a
  fake model); `egui_kittest` navigation smoke test; timeline save/load equality.
