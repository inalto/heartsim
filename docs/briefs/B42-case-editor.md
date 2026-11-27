# B42 — Case editor: write a patient case in the application
**Crates** `heartsim-app`, `heartsim-core` (helpers only), docs · **Model** Sonnet · **Milestone** M13 ·
**Depends on** B41 (Cases page, `heartsim-casefile`, open and save). Read B41 and the code it left first.

## Goal
A teacher creates a new patient case, or edits a copy of an existing one, on the Cases page, tries it, and saves it
as a `.hs` file. No text editor and no knowledge of RON is needed for the story and the questions; the simulator
actions use the same text syntax as the scenario timeline editor.

## What is there today (read first)
`crates/heartsim-core/src/case.rs` (`Case`, `Stage`, `content_problems`), `tutorial.rs` (`Step`, `UiAction`,
`Question`); `crates/heartsim-app/src/case_ui.rs` and the Cases page of B41; the scenario timeline editor
(`crates/heartsim-ui/src/…timeline_editor…`, used from `crates/heartsim-app/src/app/tutorials.rs`) for how actions
are edited as RON text row by row with a "kind" list as a starting point; `crates/heartsim-casefile`.

## Work
* **New case** and **Edit a copy** (on every card) open the editor on the Cases page in place of the list. It edits
  a draft `CasePackage`; the installed case is never changed.
* Form, top to bottom: file details (title, id derived from the title and editable, `[a-z0-9_]`, summary, patient
  line, language, authors, licence from a short list of SPDX ids plus free text, references one per line); then the
  stages as a list on the left (add, duplicate, delete, move up and down) and the selected stage on the right:
  title, the vignette as multi-line markdown with a live preview beside or under it (the same viewer the reader
  uses), the actions, the questions.
* Actions: one row per `Step`, written in RON as the case files have them (`Ui(LoadScenario("stemi_inferior"))`,
  `Sim(Block(region: AvNodeCompact, blocked: true))`), each row parsed on edit and marked when it does not parse.
  Above the rows, helpers that insert a correct row: "Load rhythm" with the rhythm picker, "Show panel", "Select
  lead", "Wait", and a list of the other kinds with an example each (generate the examples from real values and
  test that every example parses). A row with `LoadScenario` of an unknown id is marked.
* Questions: prompt, three to five choices with a radio button for the right one, explanation; add, delete, move.
* A problems panel always visible: `Case::content_problems` of the draft plus the unparsed rows, each line jumping
  to the stage it names. **Try it** plays the draft from the selected stage in the normal reader without leaving the
  editor's draft (closing the reader returns to the editor). **Save as…** is enabled when `Case::problems` is empty
  and there is no unparsed row (the editorial `content_problems` are warnings for a user's own case, shown before
  saving); it writes the `.hs` through B41's save and adds the case to My cases.
* Rhythms that are not built in: when the draft loads a rhythm that came from another case file or from the
  Simulator's "add to library" (custom scenarios of the session), the editor offers to bundle it; bundled rhythms
  are listed in the file details with a remove button.
* The draft survives leaving the page (kept in memory for the session); closing the application with an unsaved
  draft asks once on the desktop if eframe makes that practical, otherwise the page shows "not saved" next to the
  title: say which.
* Everything works on the narrow (phone) layout: stack the stage list above the stage (see `narrow.rs`).
* Strings through Fluent in the five languages.

## Acceptance tests
* Pure: the id derived from a title is valid and stable; every action example parses to the `Step` it names; a
  draft built by the editor's own operations (add stage, add question, set answer, insert "Load rhythm") passes
  `Case::problems`; moving and deleting stages and questions keeps the right answer attached to its question.
* `egui_kittest`: New case → fill the fields → the problems panel empties → Save produces bytes that
  `CasePackage::read` accepts and that equal the draft; Edit a copy of a built-in case leaves the built-in unchanged;
  an unparsable action row disables Save and is named in the panel; Try it opens the reader on the selected stage.
* `nice -n 10 cargo xtask ci`.

## Docs
`docs/book/src/user/cases.md` (a section "Writing a case": a worked example from an empty form to a saved file, the
action syntax with the helper list, the editorial advice of `content_problems`, the rule that cases are synthetic and
that sources are cited, not copied), `docs/case-file.md` (a pointer), `known-issues.md` (not seen on a real screen),
`CHANGELOG.md` `## [Unreleased]`.

## Rules
One cargo build at a time, always `nice -n 10`, `export PATH="$HOME/.cargo/bin:$PATH"`. Do not commit or stage, do not
edit `CLAUDE.md`. Report what you did not verify.
