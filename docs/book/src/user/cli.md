# Command line

`heartsim-cli` runs scenarios without a window, for batch exports, worksheets and checks.

## Exporting a scenario run

```bash
heartsim-cli run assets/scenarios/avb_2nd_mobitz1.ron --seconds 20 --out exports/
```

This runs the scenario for 20 simulated seconds and writes the last 10 s (`--window`) as the same files the
[export window](exports.md) writes: PDF, page and strip PNGs, CSV, WFDB, scenario and share bundle. Instead of a path, a
scenario id from the installed library works too (`heartsim-cli run lbbb --out exports/`).

| Option | Default | Meaning |
|---|---|---|
| `--seconds S` | 10 | Simulated time to run. |
| `--window S` | 10 | Length of the exported excerpt (the end of the run). |
| `--speed MM_S` | 25 | Paper speed of the PDF and PNGs. |
| `--gain MM_MV` | 10 | Gain of the PDF and PNGs. |
| `--paper a4\|letter` | a4 | PDF paper size. |
| `--date YYYY-MM-DD` | today | Date printed in the headers. |
| `--cabrera` | off | Cabrera limb-lead order on the PDF and page PNG. |
| `--strip-lead L` | II | Lead of the strip PNG. |
| `--formats LIST` | all | Comma-separated subset of `pdf,png,csv,wfdb,ron,bundle`. |

For the same scenario, `--seconds`, `--window`, speed, gain, paper and date, the CLI writes exactly the bytes the app writes
when it exports at that moment of the same scenario, so classes can be prepared in scripts.

## Other commands

* `heartsim-cli validate-scenarios assets/scenarios` checks every scenario in a folder against its expected findings.
* `heartsim-cli synth --bpm 70 --seconds 10 --out ecg.csv` synthesises a simple sinus or complete-block ECG.
* `heartsim-cli default-scenario --out my.ron` writes a starting point for a new scenario.
* `heartsim-cli anatomy-info assets/anatomy/default` describes an anatomy pack.

`heartsim-cli --help` and `heartsim-cli <command> --help` list all options.
