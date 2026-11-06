# Adding a scenario

A rhythm is one RON file in `assets/scenarios/`; adding one needs no code. The app lists every file it can parse, and CI
runs each one and checks it against its own `expected` block.

## 1. Start from a similar file

Copy the closest existing scenario, or write a fresh one with `heartsim-cli default-scenario --out new.ron`. The file name
must equal the `id`.

```ron
// Tier-0 rhythm preset. Ranges in `expected` are textbook criteria.
Scenario(
    format_version: 1,
    id: "sinus_brady_45",
    title: "Sinus bradycardia, 45 bpm",
    description: "Sinus rhythm at 45 bpm: normal P-QRS-T sequence with long, regular RR intervals.",
    tier: Template,
    anatomy: "default",
    seed: 42,
    notes: Some("Why it looks like this, and what it is confused with."),
    confused_with: ["sinus_arrest_junctional_escape", "sa_exit_block", "junctional_rhythm"],
    references: ["Author et al. Title. Journal Year;Vol:Pages.", "Burns E, Buttner R. Life in the Fast Lane ECG Library: topic, litfl.com/ecg-library."],
    expected: Some(ExpectedFindings(
        hr_bpm: (40.0, 59.0),
        pr_ms: Some((120.0, 200.0)),
        qrs_ms: Some((70.0, 110.0)),
        axis_deg: Some((-30.0, 90.0)),
    )),
    timing: TimingParams(
        sinus: SinusParams(rate_bpm: 45.0),
        escape: EscapeParams(junctional_bpm: 38.0, ventricular_bpm: 25.0),
    ),
)
```

## 2. The sections

* **Common fields** (`heartsim_core::Scenario`): `id`, `title`, `description` (one sentence shown in the picker),
  `notes` (the teaching explanation: required), `references` (at least one open literature source the clinical statements were checked against: required; the first 33 presets keep theirs in a `Sources:` block at the end of `notes`, which is read too), `confused_with` (at least two other scenario ids, used as quiz
  distractors), `expected` (the textbook ranges the run must satisfy), `timeline` (timed `Action`s such as stimuli or
  parameter changes), `electrodes_mm` (placement overrides), `seed`.
* **`timing`** (`heartsim_sim::TimingParams`): sinus rate and respiration, AV conduction and blocks, His–Purkinje and bundle
  branches, escape rates, accessory pathways, ectopic foci, pacemaker. Anything you leave out keeps its healthy default; the
  field documentation in `crates/heartsim-sim/src/timing/params.rs` gives units and sources.
* **`morphology`** (`heartsim_ecg::MorphologyOverrides`): QRS width scale, axis rotation, QT rate adaptation, and per-wave
  overrides of the Tier-0 templates (amplitude, width, timing of P, Q, R, S, T for a template).

Every parameter value should be traceable to the literature in `docs/research/`; cite it in `notes`. Never copy values from
another simulator.

## 3. Check it

```bash
cargo run -p heartsim-cli -- validate-scenarios assets/scenarios
```

```bash
cargo run -p heartsim-cli -- run assets/scenarios/sinus_brady_45.ron --seconds 20 --out /tmp/check
```

The first prints HR, PR, QRS and axis of every scenario and fails on any violated `expected` range; the second writes the
PDF and PNGs so you can look at the result. `cargo test -p heartsim-sim --test scenario_library` runs the same checks in the
test suite. If the scenario changes a golden snapshot on purpose, update the snapshot in the same commit.

## Sustained arrhythmias and infarct stages

Sections that are optional in every file, so older files load unchanged and `format_version` stays 1:

* `timing.atrial_driver`: `AtrialDriver(kind: Flutter, cycle_ms: 200.0, cycle_jitter: 0.0)` or `kind: Fibrillation`. The AV ratio is
  not stated: it follows from `timing.av`.
* `timing.ventricular_driver`: `VentricularDriver(kind: Fibrillation, cycle_ms: 180.0, cycle_jitter: 0.25, start: AtMs(2000.0))`, or
  `kind: Torsades` / `Polymorphic`. `start: PauseDependentRonT(not_before_ms: 5000.0)` waits for a short-long-short sequence ending
  in an ectopic beat on the T wave. `duration_ms` and `recur_after_ms` make an episode end by itself and return.
* `Stenosis(branch: "LAD evolving", severity: 1.0)` draws Q waves and inverted T waves in the territory's leads; `"LAD infarct"` draws
  the Q waves of a healed infarct; both are a silent core on the 3-D heart.
* In `expected`, `rhythm: Some(AtrialFlutter)` requires the analyser to name the rhythm, and `hr_bpm` may be left out for a rhythm with
  no countable beats.

The wave shapes can be overridden like any other: `WaveOverride(template: VfWave, wave: "VF", amplitude_mv: Some(0.08))`.
