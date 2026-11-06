# Writing tutorials

A tutorial is `assets/tutorials/<id>/tutorial.ron`: an ordered list of pages, each with Markdown text, the actions that set the
simulator up when the page is entered, and an optional multiple-choice check. The app lists every tutorial in the **Tutorials** tab
(sorted by directory name) and shows the open one in a reader panel beside whichever tab is in front. Format and runner:
`crates/heartsim-app/src/tutorial.rs`; examples: `assets/tutorials/01_basics`, `assets/tutorials/03_cardiac_axis`.

```ron
Tutorial(
    format_version: 1,
    id: "01_basics",                    // = directory name; keys the saved progress, and its sort order
    title: "Basics: from one heartbeat to the ECG",
    summary: "One or two sentences for the list.",
    pages: [
        Page(
            title: "Freeze a moment",       // shown in the page index
            markdown: r#"CommonMark text …"#,
            actions: [                      // run in order on entering the page
                Ui(LoadScenario("sinus_normal")),
                Sim(SetGlobalSinusRate(bpm: 100.0)),
                Ui(WaitForEvent(Qrs)),
                Ui(WaitForEvent(ElapsedMs(40.0))),
                Ui(Pause),
            ],
            check: Some(Question(           // optional
                prompt: "…?",
                choices: ["…", "…"],
                answer: 1,                  // index into choices
                explanation: "Markdown shown once answered correctly.",
            )),
        ),
    ],
)
```

## Actions

* `Sim(<Action>)`: any scenario-timeline action (`crates/heartsim-core/src/scenario.rs`), in the scenario-file syntax. It is
  applied to the timing engine at the current instant, exactly as a timeline entry due now would be, so new interventions work in
  tutorials as soon as the engines support them.
* `Ui(…)`: application commands.
  * `LoadScenario("<scenario id>")`: a rhythm from `assets/scenarios/` (restarts the clock).
  * `FocusPanel(Simulator | TwelveLead | HeartVector | AxisTrainer | AxisQuiz | Electrodes | RhythmQuiz | Tutorials)`.
  * `Highlight("<tool id>")`: outline a control until the next page. Ids: `rhythm_picker`, `pause`, `speed`, `pacemaker`,
    `lead_selector`, `camera_views`, `timeline_editor` (`heartsim_ui::highlight::TOOLS`).
  * `CameraPreset(Ap | Pa | Lao | Rao | Left | Cranial | Caudal | Frame)`.
  * `SetSpeed(0.2)`, `Pause`, `Resume`, `SetQrsAxis(-60.0)`, `SelectLead(II)`.
  * `WaitForEvent(ElapsedMs(ms) | Qrs)`: hold the remaining actions until that much simulated time has passed, or until the next
    ventricular activation. Waits measure simulated time, so do not `Pause` before a wait.

## Modes and progress

*Self-study* (default) moves page by page and requires each check to be answered correctly before **Next**; *Tutor* mode allows free
navigation for presenting to a group. Progress (current page, furthest page, passed checks, completed) is saved per tutorial in
`tutorial_progress.ron` in the user data directory, next to the app's own settings.

## Scenario timeline editor

The Tutorials tab also holds the **scenario timeline editor**: the loaded rhythm's timed actions as a table (time, kind, action in
scenario syntax) with add / edit / delete, a scrub bar with a tick per action, and markers on the Simulator strip. *Apply and
restart* runs the edited timeline; *Save scenario* writes a complete scenario file (engine sections included) to the user data
directory or any path.

Text in shipped tutorials must be original (see `ASSETS-LICENSES.md`).

## Rules for shipped tutorials (brief B18)

* One idea per page; **every page has at least one action that changes the simulation** (a `Sim` action, `LoadScenario`,
  `StartReentry` or `SetQrsAxis`; moving the camera or a panel does not count). Start each page with the `LoadScenario` it needs,
  so that **Replay** puts the heart back in the page's starting state.
* A tutorial lists the open literature the clinical statements were checked against in `references` (guideline, textbook
  chapter, LITFL page). Checks need three or more distinct choices and an explanation.
* Write only what the simulator shows. Tier-0 renders a coronary lesion as an ST vector (no Q waves or T inversion), the drugs of
  `assets/drugs.ron` take tens of seconds to minutes of simulated time to act (adenosine takes seconds), and only the re-entry
  presets in `assets/scenarios/reentry/` fibrillate.
* Tutorial ids are numbered (`01_basics`, `02_normal_rhythm`, …) because the list is sorted by directory name.
* `heartsim-cli run-tutorial` (all installed tutorials by default, or a file, directory or id) checks these rules and executes every
  page's actions headlessly on the timing engine and synthesiser; it fails on an action the engine refuses, an unknown rhythm or
  highlight id, or a wait that never ends. `cargo test -p heartsim-cli --test content` runs it over every tutorial and case.

## Patient cases

A **case** is a short scripted story in `assets/cases/<id>.ron` (format: `crates/heartsim-core/src/case.rs`, `format_version: 1`).
It has a `patient` line, `references`, and `stages`. A stage has a Markdown vignette, `actions` (the same vocabulary as tutorial
pages) and `questions`, each a multiple-choice `Question` with an explanation:

```ron
Case(
    format_version: 1,
    id: "case_syncope_heart_block",
    title: "Collapse in the supermarket",
    summary: "…",
    patient: "74-year-old woman, …",
    references: ["…"],
    stages: [
        Stage(
            title: "The first ECG",
            markdown: r#"…"#,
            actions: [Ui(LoadScenario("bifascicular")), Ui(FocusPanel(TwelveLead))],
            questions: [Question(prompt: "…", choices: ["…", "…", "…"], answer: 0, explanation: "…")],
        ),
    ],
)
```

The Tutorials tab lists the cases below the tutorials. A case is played in the reader panel: the vignette, then each question once
(the first answer counts for the score; the explanation is shown either way), then the final score. Rules checked by
`heartsim-cli validate-cases assets/cases`: at least five questions per case, three to five distinct choices each, an explanation
for every question, `references`, and a first stage that loads a rhythm.
