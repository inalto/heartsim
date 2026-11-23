# B33 — Standard views: targets from landmarks, a "view found" score, a tutorial and a self-test (echo simulator, phase 4)
**Crates** `heartsim-echo`, `heartsim-core` (tutorial actions), `heartsim-ui`, `heartsim-app`, assets, docs · **Model** Sonnet
(Haiku for the docs) · **Milestone** M11 (echo simulator, phase 4 of 5) · **Depends on** B31 (probe, slice), B32 (image),
B15 (tutorial engine).

## Goal
The learner is asked for a view ("parasternal long axis"), moves the probe, and the page tells them how close they are and
what to change ("rotate clockwise", "tilt toward the right hip", "you are over a rib: slide down one space"). Each standard
view has a target computed from the heart's own landmarks, a score from geometry alone, a list of what the view must show,
and a "show me" that animates the probe to the target. A guided tutorial walks through the views in the order they are
taught, and a self-test asks for them without hints and reports a score per view. The claim is orientation and view
recognition in this model, not hand skill on patients.

## Read first
`CLAUDE.md`; `docs/research/echo-simulator.md` §4 (all), §6, §9 (items 2 and 8); the briefs B31 and B32 and what they built
(`ProbePose`, window starting poses, `slice`, occlusion, the Echo page); `docs/tutorials.md`,
`crates/heartsim-app/src/tutorial.rs`, `assets/tutorials/11_heart_sounds` (a tutorial that drives a page);
`crates/heartsim-app/src/auscultation_quiz.rs` (a self-test page with per-item scoring and saved progress).

## Non-negotiables for this brief
* **Sourced.** What each view must show, the probe position, the index-marker direction and the manoeuvre from the previous
  view come from the ASE guideline for the transthoracic examination (Mitchell et al. 2019, doi:10.1016/j.echo.2018.06.004),
  re-read on the primary document for this work and cited with page per view. The research note's table is a guide, not the
  source of record. All text is written fresh, en and it; nothing is copied from any teaching product or from `epicardio/`.
* **Targets are computed, not typed.** Each target plane and pose derives from pack landmarks, the valve-disc centres, the
  LV long axis, the designed papillary level and the baked ribs, so a different pack gives different, still valid, targets.
* **Designed, said so.** Every tolerance of the score is designed, lives in one struct, and is listed in the docs for a
  sonographer to review.
* **Pure and deterministic.** Targets, score and hints are pure functions in `heartsim-echo` of (pack, pose, time-independent
  anatomy at rest, occlusion); independent of frame rate; no GPU.
* The patient stays supine unless the maintainer decided otherwise in B31; if a view cannot be reached from its window
  within the contact limit in this anatomy, it is reported and shipped as "not reachable in this model", not faked.
* Engines and goldens untouched.

## Scope
1. **Views** (ids stable): parasternal long axis; RV inflow; RV outflow; parasternal short axis at the aortic valve, mitral
   valve, papillary and apical levels; apical four-, five-, two- and three-chamber; RV-focused apical four-chamber;
   subcostal four-chamber; subcostal IVC; suprasternal arch (only if the pack's aorta reaches the arch; check and report).
2. **Targets.** For each view: the window, the target plane and index-marker direction, the landmarks that must lie in the
   plane and inside the sector, landmarks that must be absent (the apex in the parasternal long axis, any RV landmark in the
   two-chamber), and a reference pose found by a small deterministic optimisation over the probe's degrees of freedom from
   the window's starting pose.
3. **Score** in [0, 1], a product of terms: distance of each key landmark from the plane; each inside the sector; the
   marker direction; foreshortening in apical views (how well the plane passes through the true apex); window quality (the
   fraction of lines to the key landmarks not hidden by bone or lung). "Found" above a threshold held for one second.
4. **Hints.** From the pose error decomposed into the six named movements (slide, rotate, tilt, rock, angle, depth), the
   largest correctable term gives one hint in words with its direction relative to the patient (toward the right shoulder,
   toward the left hip), plus specific hints for a rib, lung, lost contact, foreshortening and a wrong marker side.
5. **Page.** A view picker on the Echo page with: the checklist of what the view shows (ticked live from the label slice),
   a score bar, the hint line, *Show me* (animates the probe to the reference pose; instant with reduced motion), and a
   ghost of the target plane in the 3-D heart that can be switched off. Structure labels on the image for the current view
   (on, off). en + it.
6. **Tutorial.** `assets/tutorials/13_echo_views`: windows and probe movements, then the views in guideline order, each page
   setting the window, asking for the view and checking it by score (a new tutorial check kind "view found", plus the
   actions to set a probe pose and a target view); multiple-choice questions on what each view shows.
7. **Self-test.** A mode with no hints and no ghost: a sequence of views, a time and a best score per view, a summary table,
   progress saved like the other quizzes.
8. **CLI.** `heartsim-cli echo views` lists each view with its reference pose and its score (a regression aid).
9. **Docs.** User page *Standard views* (one short section per view: where the probe goes, what you see, the common
   mistake); known-issues (targets and tolerances not reviewed by a sonographer; supine anatomy; views not reachable);
   `CHANGELOG.md`; `docs/ROADMAP.md`; screenshots of each view; tour steps.

## Out of scope
M-mode, tissue Doppler, calipers (phase 5); Doppler; image-quality grading beyond the geometric score; contrast; stress
echo; transoesophageal views; certification-style exams.

## Acceptance tests (no GPU unless marked)
* For every shipped view: the reference pose has contact, lies within the probe limits from its window, scores above 0.9;
  the same pose rotated ±20° about the beam scores below 0.5; tilted 15° out of plane scores below 0.5.
* Parasternal long axis: a pose with the apex inside the sector scores lower than the reference. Two-chamber: below 0.5
  when an RV landmark is in the plane. Apical four-chamber: a plane missing the true apex by 15 mm scores lower
  (foreshortening) and gives the foreshortening hint; the label slice at the reference contains all four blood labels and
  both atrioventricular valves. Short axis at the papillary level: LV blood enclosed by myocardium with both papillary
  bodies. Each view's checklist is fully ticked at its reference pose.
* Hints: from the reference pose perturbed along one movement, the hint names that movement and the correcting direction;
  over a rib the hint is the rib hint; following hints greedily from the window's starting pose reaches "found" for each
  view within a bounded number of steps.
* The score is a pure function of the pose (equal for two frame rates, two times in the beat).
* Tutorial: the file validates (`heartsim-cli` validation), runs headless to the end, and each "view found" check passes
  at the reference pose and fails at the starting pose.
* Self-test and page (egui_kittest): the picker, the score bar, *Show me*, the ghost switch, labels on and off, the summary
  table, saved progress round-trip, en and it, reduced motion.
* CLI: `echo views` output snapshot.
* `cargo xtask ci`, both wasm32 checks and `cargo check -p heartsim-app --no-default-features --lib` pass; goldens unchanged.

## Decisions for the maintainer
* The list of views for a first version (the brief ships every view the anatomy can reach).
* The "found" threshold and whether the self-test shows a pass mark.
* A sonographer's review of targets, tolerances and hint wording.
