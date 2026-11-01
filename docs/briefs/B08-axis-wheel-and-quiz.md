# B8 — Cabrera axis wheel, heart-vector view, quizzes
**Crates** `heartsim-ui` (`src/axis_wheel.rs`, `src/vector_view.rs`, `src/quiz.rs`), `heartsim-app` · **Model** Sonnet ·
**Milestone** M1 · **Depends on** B4.

## Goal
Reproduce and surpass the open-source axis trainer: a hexaxial wheel showing lead axes, the live QRS axis arrow and classification,
projection guide lines onto the selected lead, and training modes (axis drill, rhythm recognition) with scoring.

## Read first
`docs/research/reference-apps.md` §1 and §7 (verified maths: lead angles, projection `cos(axis − θ)`, classification schemes,
random-axis guard bands, strip synthesis from axis), `crates/heartsim-core/src/axis.rs`, `docs/REQUIREMENTS.md` F-14, F-40.

## Scope
1. Wheel widget: six limb-lead axes (+ negative poles), colour sectors for Normal/LAD/RAD/Extreme, optional German 7-class scheme,
   live arrow for QRS axis (and T axis, P axis toggles), hover shows projection value on each lead.
2. Vector view (2-D frontal/horizontal planes, 3-D loop in the viewport via B6 lines): per-sample heart vector from B4.
3. Axis drill: random axis with guard bands (exclude 170–280°, ±7° around boundaries); render the six limb leads from
   `FrontalAxis::projection_onto` using the Tier-0 synth with `axis_shift`; learner answers class or degrees (±15°); score, streak,
   explanation ("I positive, aVF negative → …").
4. Rhythm quiz: pick from the scenario library (B11), show a strip (and optionally the 3-D view), multiple choice with distractors
   from the scenario metadata, feedback text, score persistence (settings dir).
5. Interval drill: measure PR/QRS/QT with calipers; tolerance grading.

## Acceptance tests
* Wheel math: projection values shown equal `cos(axis − θ_lead)` for all six leads (unit test on the pure functions).
* Drill generator never produces an axis inside guard bands (proptest over 10 k draws); classification of generated axis matches
  the expected class.
* `egui_kittest`: quiz flow (start → answer → feedback → next) reachable through AccessKit nodes.
