# B29 — Echo calculators: a Tools section for echocardiography and haemodynamics
**Crates** new `heartsim-calc` (pure), `heartsim-ui`, `heartsim-app`, `heartsim-cli`, assets, docs · **Model** Opus ·
**Milestone** M10 (clinical calculators) · **Depends on** B19 (i18n), the navigation of 0.16.0. Independent of the simulation.

Two parts for two implementers one after the other. **Part A** is the framework and every calculator that is a closed formula
(about 50). **Part B** is the guideline decision tools (diastolic function, prosthetic-valve screening, graded size tables),
which need the primary documents open and are shipped only where the criteria were read there.

## Goal
A fourth group in the top bar, **Tools**, with one page, **Echo calculators**: a searchable catalogue of the quantitative
methods of adult transthoracic echocardiography (valve areas, regurgitant volumes, filling pressures, ventricular size and
function, right-heart pressures, shunts, prosthetic valves, pericardial respiratory variation) and of the ultrasound physics a
trainee is examined on. Each calculator shows its inputs with units and plausible ranges, the result with the formula written
out and the numbers substituted, an interpretation where a guideline gives one, the reference with its locator, and a worked
example. It is a teaching tool: the page says so, and it never stores or sends what is typed.

The scope list comes from surveying what such tools offer (the public catalogue of EchoSuite, echosuite.app, was the survey).
**That site is proprietary** (software and teaching text © its author). The same rule applies to it as to `epicardio/`:

## Non-negotiables for this brief
* **Clean room.** The survey extract (kept outside the repository, in the session scratchpad under `echosuite/site/`, and never
  committed) may be read for *which* calculators exist, their inputs and units, and *which publications* they cite. Nothing else
  is taken: no purpose, help or guide text, no worked-example values, no figures, no field help wording, no code, no option
  labels beyond plain medical terms, no layout. Formulas, thresholds and tables are re-derived from the primary publications
  (ASE / EACVI / ESC guidelines and the original papers) and each calculator's source file cites the publication, the table or
  equation and the page. All text is written fresh, in en and it.
* **Verified or labelled.** Each calculator carries a `review` status: `Verified` (formula and every threshold read on the
  primary source during this work, locator recorded), `FormulaOnly` (the arithmetic is standard and verified, but no grading is
  shown because the criteria were not read on the primary source), or omitted. No threshold from memory, from a secondary
  website or from the survey ships as an interpretation. The status is visible on the page.
* **Not a medical device.** The page carries the educational disclaimer; results are never worded as a diagnosis ("meets the
  guideline's criterion for …", not "the patient has …"); nothing typed leaves the device or is persisted beyond the session
  unless the user asks to keep a draft (then in the app's own storage only).
* **Pure and deterministic.** `heartsim-calc` depends only on `heartsim-core` (or on nothing), has no I/O, no egui, compiles for
  `wasm32`, and every calculator is a pure function from typed inputs to a typed result. Units in names (`_cm`, `_m_s`,
  `_mmhg`, `_ml`, `_cm2`, `_ms`, `_m2`, `_khz` …).
* The simulation crates and every existing golden are untouched.

## Read first
`CLAUDE.md`; `crates/heartsim-app/src/nav.rs` and its tests (groups, tabs, shortcuts, narrow layout); `interval_drill.rs` and
`sound_trainer.rs` (a page with a list at the left and a detail at the right); `crates/heartsim-ui/src/controls.rs` (labelled
numeric inputs with units); `assets/locales/{en,it}/`; `crates/heartsim-cli/src/` (sub-commands); `docs/book/src/user/`;
`docs/web.md` (the page must work in the web build). The survey extract for scope only.

## Part A — framework and formula calculators
1. **Model** (`heartsim-calc`). A `Calculator` description: stable id, category, inputs (id, quantity, unit, plausible range,
   optional or not, enumerated choices), a pure `compute` to a `Outcome` (named values with units, the substituted formula as
   structured steps, zero or more `Finding`s with a severity class and the criterion that produced it), references (authors,
   title, journal, year, DOI, locator) and the review status. Input validation returns typed errors (missing, out of range,
   inconsistent, e.g. ESV > EDV); no panics, no NaN in results. A registry lists every calculator; ids are stable and unique.
   Shared building blocks are written once: Bernoulli (simplified and with proximal velocity), circular area from a diameter,
   stroke volume from diameter and VTI, continuity, PISA flow and orifice (with the angle correction), body surface area,
   indexing, percentage change.
2. **Calculators** (names are ours; each is one module with its tests):
   * *Stenosis*: aortic valve area by continuity with indexed area and velocity ratio; mitral valve area by pressure half-time,
     by continuity and by PISA with angle correction; pulmonic peak gradient; tricuspid stenosis measurements.
   * *Regurgitation*: PISA orifice area and regurgitant volume for aortic, mitral and tricuspid regurgitation; volumetric
     regurgitant volume and fraction for aortic, mitral and pulmonic regurgitation.
   * *Filling pressures (formulas)*: average E/e′; left atrial pressure from the MR jet and systolic pressure; LV systolic and
     end-diastolic pressure from MR and AR jets; left atrial volume index.
   * *Ventricular function*: fractional shortening; indexed LV diameters; LV mass (cube formula with the ASE correction), mass
     index and relative wall thickness; LV volumes indexed; ejection fraction; relative change of global longitudinal strain;
     LVOT stroke volume, cardiac output and indices; indexed aortic diameters; wall-motion score index (16 or 17 segments).
   * *Right heart and pulmonary circulation*: right atrial pressure from the IVC; RVSP and PASP; pulmonary end-diastolic and
     mean pressure from the PR jet; mean PA pressure from the RVOT acceleration time; pulmonary vascular resistance (Abbas);
     RV fractional area change, TAPSE/PASP coupling; RV myocardial performance index (pulsed and tissue Doppler); RA and RV
     indexed measurements; body surface area (Mosteller, and Du Bois as an option).
   * *Pericardium*: effusion size by separation; respiratory variation of mitral, tricuspid and LVOT flow; hepatic-vein
     expiratory reversal ratio; medial/lateral e′ ratio.
   * *Shunts and congenital*: Qp/Qs; ASD flow ratio; RV pressure from a VSD jet; PA pressure from a PDA jet; coarctation
     gradient with the proximal-velocity correction; mean gradient from a traced velocity.
   * *Prosthetic valves (formulas)*: effective orifice area, Doppler velocity index and indexed area for aortic and mitral
     prostheses.
   * *Ultrasound physics*: period, wavelength and pulse duration; pulse repetition period and frequency, Nyquist limit, duty
     factor; spatial pulse length, axial resolution, near-field length; the Doppler equation both ways with the angle error;
     acoustic impedance, intensity reflection coefficient, intensity; Poiseuille flow and resistance.
   Where the guideline that defines a method also grades its result in a table that was read during this work, the grading is
   included and the calculator is `Verified`; otherwise it is `FormulaOnly`.
3. **Page** (`heartsim-app` + `heartsim-ui`). Group **Tools** in `nav.rs` with the page *Echo calculators* (tab, group menu,
   hamburger list; the narrow layout still fits; no number-key shortcut is taken from an existing page). Left: search box
   (matches name, synonyms, quantity names in the current language) and the categories; right: the selected calculator —
   inputs in the order of acquisition with unit and range hint, live result, the formula with the numbers substituted, findings
   with their criterion, "Load a worked example" (our own values, physiologically plausible), "Clear", the references with DOI
   links, and the review status. Invalid input is explained beside its field; the result area never shows a stale value.
   Keyboard: Tab through fields, Enter moves on. Favourites and the last-opened calculator are remembered; typed values are not.
   Works in both themes, in the web build, at phone width.
4. **Link to the simulator, where it is honest.** A small "from the running rhythm" button only on calculators whose input the
   simulator really has (heart rate for cardiac output). No invented echo measurements.
5. **CLI.** `heartsim-cli calc list` and `heartsim-cli calc <id> key=value …` print the same outcome as text or `--json`.
6. **Docs.** A user-book page (the catalogue by category, what "Verified" and "Formula only" mean, the disclaimer), a developer
   page "Adding a calculator"; `CHANGELOG.md`; milestone M10 in `docs/ROADMAP.md`; the brief index; README feature list; a
   screenshot in `docs/images/`.

## Part B — guideline decision tools
7. **Diastolic function and filling pressure** by the current ASE/EACVI recommendations: the general algorithm in sinus rhythm
   and the special populations the recommendation covers (atrial fibrillation, mitral annular calcification, mitral stenosis
   and regurgitation, hypertrophic and restrictive cardiomyopathy, sinus tachycardia, pulmonary hypertension, heart
   transplant, LVAD, exercise). Each is a decision tree encoded as data with every node citing figure or table and page; the
   page shows the path taken through the tree, including "indeterminate".
8. **Prosthetic valves**: screening of aortic and mitral prostheses for obstruction or mismatch, and the serial comparison
   with a postimplant baseline, by the current ASE recommendation.
9. **Graded tables**: aortic stenosis severity with the low-flow and discordant-grading pathway and the stress comparison;
   regurgitation grades by orifice, volume and fraction; chamber sizes and function by sex-specific normal ranges (LV, LA, RA,
   RV, aorta); tricuspid stenosis findings; RV function thresholds.
10. A tool ships only if its primary document was read during the work. A document that cannot be obtained means the tool is
    left out and listed in `docs/research/echo-calculators.md` with what is missing.

## Out of scope
Echo image simulation, probe-position training and cine clips; patient records, reports or PDF export of results; paediatric
z-scores; transoesophageal-specific methods; anything not in an openly citable publication.

## Acceptance tests
* Registry: ids unique and stable (snapshot of the id list); every calculator has at least one reference with a locator, a
  review status, a worked example that validates and computes, and names in en and it.
* Each calculator: a hand-computed case from the reference or from first principles (the test states the arithmetic); the
  range and consistency errors; no NaN or infinity for any in-range input (property test over the ranges); units of every
  output; `FormulaOnly` calculators return no findings.
* Building blocks: Bernoulli, areas, continuity and PISA against closed-form values; BSA against published examples.
* Grading (where present): every boundary value falls on the side the guideline states, tested at the boundary and one step
  either side.
* Part B: every decision tree is total (each combination of present, absent and missing inputs reaches a leaf), each leaf and
  node carries a locator, and the worked cases in the recommendation (where it gives any) reach the published answer.
* Page (egui_kittest): search finds a calculator by name and by quantity in both languages; typing valid inputs shows the
  result and the substituted formula; an out-of-range input shows its message and no result; the worked example fills and
  computes; favourites round-trip; typed values are not in the saved settings; the status badge is shown.
* Navigation: the nav tests cover fourteen pages and four groups; the narrow layout shows four group menus without overflow.
* CLI: `calc list` lists the registry; `calc <id>` equals the library outcome; bad input exits non-zero with the message.
* Clean room: a test or xtask check that no file under the repository contains the survey's distinctive strings is not
  practical; instead the implementer's report lists, per calculator, the primary source read and its locator.
* `cargo xtask ci`, the wasm32 check of the pure crates including `heartsim-calc`, the wasm32 check of `heartsim-web`, and
  `cargo check -p heartsim-app --no-default-features --lib` pass; every existing golden is unchanged.

## Decisions for the maintainer
* Whether `FormulaOnly` calculators should ship at all or wait until graded.
* Whether to mention the survey source in the documentation (the brief names it; the product text need not).
* A clinician's review of the gradings before the section is described as more than a study aid.
