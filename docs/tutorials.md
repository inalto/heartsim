# Writing tutorials

A tutorial is `assets/tutorials/<id>/tutorial.ron`: an ordered list of pages, each with Markdown text, the actions that set the
simulator up when the page is entered, and an optional multiple-choice check. The app lists every tutorial in the **Tutorials** tab
(sorted by directory name) and shows the open one in a reader panel beside whichever tab is in front. Format and runner:
`crates/heartsim-app/src/tutorial.rs`; examples: `assets/tutorials/basics`, `assets/tutorials/cardiac_axis`.

```ron
Tutorial(
    format_version: 1,
    id: "basics",                       // = directory name; keys the saved progress
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
