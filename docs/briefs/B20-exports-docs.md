# B20 — Exports and user documentation
**Crates** `heartsim-app`, `heartsim-cli`, `docs/book` · **Model** Sonnet · **Milestone** M5 · **Depends on** B7.

## Scope
1. Exports: 12-lead PDF on A4/Letter paper layout with grid, patient-free header (scenario, speed, gain, date) via `printpdf`;
   PNG snapshot of the strip/page/3-D view; CSV and WFDB (`.hea` + 16-bit `.dat`, 500 Hz, gain 1000/mV) of the last N seconds;
   scenario RON; "share bundle" zip.
2. CLI parity: `heartsim-cli run <scenario> --seconds 10 --out ...` producing the same files headlessly.
3. mdBook user guide (install, UI tour, scenarios, tutorials, quizzes, exports, FAQ, medical disclaimer) and developer guide
   (architecture summary, adding a scenario, adding a drug, running benchmarks); published by a `docs.yml` workflow to GitHub Pages.

## Acceptance tests
PDF opens and has 12 traces (parse with `pdf` crate or check object count); WFDB readable by `wfdb` Python in a CI step
(optional); CLI and app exports byte-identical for the same scenario/seed; mdBook builds without warnings.
