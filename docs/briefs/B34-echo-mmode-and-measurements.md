# B34 — M-mode, annular tissue Doppler and calipers that feed the calculators (echo simulator, phase 5)
**Crates** `heartsim-echo`, `heartsim-ui`, `heartsim-app`, `heartsim-calc` (ids only), docs · **Model** Sonnet (Haiku for the
docs) · **Milestone** M11 (echo simulator, phase 5 of 5) · **Depends on** B30, B31, B32, B33, B29 (calculators).

## Goal
The learner measures on the simulated echo. An M-mode cursor on the sector gives a scrolling depth-against-time trace of
one scan line, with the ECG under it; calipers on the frozen 2-D image and on the M-mode give distances, times and slopes;
a tissue-Doppler sample at the mitral or tricuspid annulus gives a velocity trace with S′, e′ and a′. A measurement can be
sent to the matching echo calculator (LV diameters to fractional shortening and LV mass, TAPSE to the RV tools, e′ to
E/e′ where the other inputs are typed). Everything shown is exactly what the motion model does, and the page says so:
these are measurements of a model, for learning the method.

## Read first
`CLAUDE.md`; `docs/research/echo-simulator.md` §5, §8 (row E5), §9 (item 6); briefs B30–B33 and what they built
(`MotionModel`, `MotionReadout`, `slice`, the grey image, views); `crates/heartsim-calc` (registry, input ids, units) and
`crates/heartsim-app/src/echo_calc.rs` (how a value is offered to a calculator: the "from the running rhythm" link);
the strip calipers in `heartsim-ui` (interaction to reuse).

## Non-negotiables for this brief
* **No blood Doppler**, spectral or colour, and no control that suggests it. Tissue Doppler is offered only at the two
  annuli, where the velocity is the time derivative of the model's own annular motion along the beam.
* **Honest.** Every trace and number is derived from the same motion evaluation and image as the 2-D sector; nothing is
  drawn from a template. Measurements are labelled *model*. No strain or speckle tracking (it would only return the
  prescribed motion).
* **Sourced where graded.** No normal range or grading is added here; interpretation stays in the B29 calculators, which
  cite their sources. Conventions for where and when to measure (end-diastole at the QRS onset, leading-edge or
  inner-edge) are read on the primary guideline (Lang et al. 2015; Mitchell et al. 2019) and cited with page.
* **Pure and deterministic** for the traces and the automatic reference measurements (in `heartsim-echo`); engines and
  goldens untouched; WebGL2-safe; nothing typed or measured is persisted.

## Scope
1. **M-mode** (`heartsim-echo`): one line of the polar image (label and grey) sampled over time into a column buffer at a
   fixed time step independent of frame rate; sweep speeds 25, 50 and 100 mm/s. On the page: a cursor line on the sector
   that follows rock within the plane, the scrolling trace beside or under the sector, the ECG lead aligned in time, freeze
   and scrub.
2. **Annular tissue Doppler**: a sample gate placed on the septal or lateral mitral annulus or the lateral tricuspid
   annulus (snapped to the model's annular points when within a few millimetres); the trace is the velocity of that tissue
   point along the beam, with the angle between beam and motion shown and **not** corrected; S′, e′ and a′ marked (a′
   absent without atrial contraction). A designed thin spectral band around the curve for the look, labelled designed.
3. **Calipers**: on the frozen sector (distance in mm; area by trace in cm²), on M-mode (depth distance, time interval,
   slope in cm/s) and on the Doppler trace (velocity, time). Snap off by default. Up to four measurements listed with
   names the user picks from a short list (LVIDd, LVIDs, IVSd, LVPWd, LA diameter, aortic root, TAPSE, MAPSE, e′, a′, S′).
4. **Reference values for feedback**: for the named measurements, the model's own value on the same line and frame
   (cavity and wall boundaries from the label line; annular excursion from the motion), shown after the user commits a
   measurement, with the difference. This is the teaching loop: measure, then compare with the truth of the model.
5. **Send to calculator**: a named measurement can be sent to the calculators that take it (by stable input id); the Echo
   calculators page opens with that field filled and marked "from the echo simulator"; the user types the rest. Never
   fills a value the simulator does not really have.
6. **Docs**: user page *Measuring on the echo* (M-mode, where to put the cursor, timing against the ECG, tissue Doppler and
   its angle dependence, what is designed, what is absent and why there is no blood Doppler); known-issues; a tutorial
   page or two appended to `13_echo_views` or a short new tutorial; `CHANGELOG.md`; `docs/ROADMAP.md` (M11 complete);
   screenshots; tour steps.

## Out of scope
Spectral and colour blood Doppler, continuous-wave Doppler, strain, 3-D echo, contrast, stress protocols, reports or PDF
export of measurements, pathology looks (lesion-specific leaflets, dilated chambers, effusion): each needs its own brief
and a clinician's input.

## Acceptance tests (no GPU unless marked)
* M-mode: the column buffer is identical for two frame rates; at the parasternal long-axis reference pose with the cursor
  at the mitral leaflet tips' level, LVIDd and LVIDs read from the label line agree with the deformed mesh's diameters on
  that line within 2 mm, and LVIDs is smaller than LVIDd in sinus rhythm and nearly equal in VF.
* TAPSE measured on an apical M-mode through the lateral tricuspid annulus equals the model's TAPSE read-out times the
  cosine of the beam angle within 1 mm; MAPSE likewise.
* Tissue Doppler: the trace equals the finite-difference derivative of the annular displacement along the beam (maximum
  error under 0.5 cm/s); S′ positive toward an apical probe, e′ and a′ of opposite sign; no a′ in atrial fibrillation; two
  frame rates give the same trace; at 60° to the motion the peak is half.
* Calipers: distance, time and slope arithmetic against hand-computed cases at each sweep speed and depth; area of a traced
  circle within 2 %.
* Reference feedback: the difference shown equals measured minus reference; no reference is offered where the model has
  none.
* Send to calculator: each named measurement maps to existing calculator input ids (a test fails if an id disappears); the
  calculator shows the value with its origin; nothing is written to the saved settings.
* No Doppler control for blood exists (a UI test lists the page's controls).
* Page (egui_kittest): cursor, freeze, scrub, each caliper kind, the measurement list, en and it, reduced motion.
* Bench (not a gate): added cost of M-mode and Doppler per frame.
* `cargo xtask ci`, both wasm32 checks and `cargo check -p heartsim-app --no-default-features --lib` pass; goldens unchanged.

## Decisions for the maintainer
* Whether the designed spectral band on the tissue-Doppler trace is wanted or a plain curve is more honest.
* Whether "schematic blood Doppler, labelled" is wanted later (the research note advises against it for a first version).
* Clinician review of the measurement conventions and of the whole echo section before it is described as more than a
  study aid.
