# B11 — Tier-0 scenario library (25 rhythms) and golden tests
**Paths** `assets/scenarios/*.ron`, `crates/heartsim-ecg/tests/golden.rs`, `crates/heartsim-sim/tests/` · **Model** Haiku ·
**Milestone** M1 · **Depends on** B1 (timing parameters), B4 (interval checks).

## Goal
Encode the first 25 rhythms as scenario files driving the timing engine + Tier-0 synthesiser, each with title, description,
teaching notes, "confused with", and expected interval ranges; add golden CSV snapshots and interval assertions.

## Read first
`crates/heartsim-core/src/scenario.rs`, `crates/heartsim-sim/src/timing.rs` (+ B1 params), `crates/heartsim-ecg/src/synth.rs`,
`docs/REQUIREMENTS.md` F-07, `docs/reverse-engineering/pathology-recipes.md` (**for the list of conditions and which knobs each
touches only — do not copy numbers**), standard textbook ranges (cite: e.g. Goldberger's Clinical Electrocardiography, Life in the
Fast Lane ECG library) for the expected intervals.

## Scenarios
sinus_normal, sinus_brady_45, sinus_tachy_130, sinus_arrhythmia, sinus_arrest_junctional_escape, sa_exit_block, avb_1st,
avb_2nd_mobitz1, avb_2nd_mobitz2_3to1, avb_3rd_junctional, avb_3rd_ventricular, lbbb, rbbb, lafb, lpfb, bifascicular, pac_frequent,
pvc_bigeminy, pvc_multifocal, junctional_rhythm, idioventricular, aivr, wpw_left_lateral (pre-excitation only), avnrt_150 (via B1 loop),
avrt_orthodromic, paced_vvi, paced_ddd. (Tier-0 morphology knobs: `qrs_width_scale`, `axis_shift_deg`, per-wave amplitude/offset
overrides — extend `GaussianSynth` with a serialisable `MorphologyOverrides` struct.)

## Acceptance tests
* Every `.ron` loads (`Scenario::from_ron`), has non-empty description and ≥ 2 "confused_with" entries, unique id.
* For each scenario: run 20 s, B4 analysis within the scenario's `expected: { hr: (lo,hi), pr: …, qrs: …, axis: … }` block.
* `insta` CSV snapshot (100 Hz, 3 decimals, 5 s) per scenario; snapshots reviewed and committed.
* `heartsim-cli validate-scenarios assets/scenarios` passes.
