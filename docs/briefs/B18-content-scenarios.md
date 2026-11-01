# B18 — Content: scenarios, tutorials, cases
**Paths** `assets/scenarios`, `assets/tutorials`, `assets/cases` · **Model** Haiku (data entry) + Sonnet (narratives) ·
**Milestone** M5 · **Depends on** B11, B14, B15, B17.

## Goal
Grow the library to 40+ scenarios with explanations and "confused with" lists, 10 tutorials (Basics, Normal rhythm, Cardiac axis,
Conduction abnormalities, AV blocks & bradycardias, Atrial arrhythmias, SVT, WPW, Ventricular arrhythmias, Ischaemia & infarction),
and 3 scripted patient cases with MCQs.

## Rules
Write original text; cite textbook sources in a `references` field; no text from any commercial product. Clinical facts must be
checked against at least one standard reference (ESC/AHA guidelines, Goldberger, LITFL). Keep reading level for students; one idea
per page; every tutorial page has an action that changes the simulation.

## Acceptance tests
Scenario validator passes; every tutorial loads and all its actions execute without error in a headless run (`heartsim-cli
run-tutorial`); spell-check (`typos`) clean; each case has ≥ 5 MCQs with explanations.
