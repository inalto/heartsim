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

## Case files

`heartsim-cli case` makes, takes apart and checks patient case files (`.hs`, see [Patient cases](cases.md) and the
[format](https://github.com/inalto/heartsim/blob/master/docs/case-file.md)), with the same reader and writer as the application:

```bash
heartsim-cli case pack my_case.ron --scenario my_rhythm.ron -o my_case.hs --author "A. Teacher" --licence CC-BY-4.0
heartsim-cli case check my_case.hs
heartsim-cli case unpack my_case.hs -o my_case/
```

* `case pack <case.ron> [--scenario <file>]… -o <name.hs>` bundles the case with the rhythms it needs that are not built in.
  `--author` (repeatable), `--licence`, `--language` (default `en`) and `--created` (default today) fill the manifest. It fails
  when the case needs a rhythm that is neither bundled nor built in, or bundles one with a built-in id. The same inputs and
  `--created` give the same bytes; the SHA-256 is printed.
* `case check <name.hs>` prints the verdict of the application's reader and fails when the application would refuse the file;
  then the editorial rules of the shipped cases and a headless run of every stage, as warnings.
* `case unpack <name.hs> -o <dir>` writes `heartsim-case.ron`, `case.ron` and `scenarios/<id>.ron` into `<dir>`, and nothing
  else.
* `validate-cases assets/cases` still checks a folder of case RON files.

`--assets <dir>` (for `pack` and `check`) names the `assets/` folder whose rhythms count as built in.

## Other commands

* `heartsim-cli validate-scenarios assets/scenarios` checks every scenario in a folder against its expected findings.
* `heartsim-cli synth --bpm 70 --seconds 10 --out ecg.csv` synthesises a simple sinus or complete-block ECG.
* `heartsim-cli default-scenario --out my.ron` writes a starting point for a new scenario.
* `heartsim-cli anatomy-info assets/anatomy/default` describes an anatomy pack.
* `heartsim-cli calc list` lists the echo calculators; `heartsim-cli calc <id> input=value …` (add `--example` to start from
  the worked example, `--json` for JSON, `--lang it` for Italian) prints the values, the worked formula and the findings with
  their source, as the [Echo calculators](echo-calculators.md) page does. Invalid input exits non-zero, naming the field.

`heartsim-cli --help` and `heartsim-cli <command> --help` list all options.
