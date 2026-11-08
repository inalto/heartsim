# Export file formats

All export and import code is in the `heartsim-io` crate, shared by the app and `heartsim-cli`. Both cut the excerpt into an
`EcgRecord` (twelve leads at 500 Hz) and call one function, `heartsim_io::export_files`, so the files are identical byte for
byte; `crates/heartsim-cli/tests/run_parity.rs` checks this. Every writer is a pure function of the record, the header
information and the date: no clock, no randomness.

## Sample timing

The timing engine steps every 1 ms; the strip and every export take one sample every 2 ms, at simulation times 2, 4, 6 … ms
after the scenario starts. An excerpt of *N* seconds is the last *N* × 500 samples before the strip cursor.

## CSV

```text
t_ms,I,II,III,aVR,aVL,aVF,V1,V2,V3,V4,V5,V6
0,0.012,0.020,0.008,-0.016,0.002,0.014,-0.004,0.003,0.006,0.009,0.011,0.012
2,…
```

Time from the start of the excerpt in ms; leads in mV with three decimals (1 µV); negative zero is written as `0.000`.

## WFDB

A PhysioNet record named `heartsim_<scenario id>`: the header `<name>.hea` and one data file `<name>.dat` with the twelve
signals interleaved as little-endian 16-bit integers (format 16), 1000 ADC units per mV, baseline 0, 500 Hz. The header has
the usual initial-value and checksum fields, so `wfdb` validates the file on reading, and comment lines with the generator,
scenario, start time and date:

```text
heartsim_sinus_normal 12 500 5000
heartsim_sinus_normal.dat 16 1000/mV 16 0 0 -1234 0 I
…
heartsim_sinus_normal.dat 16 1000/mV 16 0 3 812 0 V6
# heartsim 0.8.0 - simulated ECG, not a patient recording
# scenario: sinus_normal - Sinus rhythm, normal axis
# start_ms: 2002
# date: 2026-11-03
```

Values beyond ±32.767 mV are clamped (−32768 is WFDB's "no sample" value).

## PDF

One landscape page (A4 297 × 210 mm or Letter 279.4 × 215.9 mm) with a 250 × 160 mm grid of 1 mm and 5 mm lines: three rows
of four leads (columns are consecutive quarters of the page time, as on an electrocardiograph printout) and a full-width
lead II rhythm strip, a 1 mV × 200 ms calibration pulse per row, the header (scenario, speed, gain, sampling rate, date) and a
footer with the simulation time and the disclaimer. Text uses the PDF builtin Helvetica, so nothing is embedded; metadata dates
are the export date and the document ID is a hash of the content.

## Scenario RON

The running scenario in the format of `assets/scenarios/`: the common fields plus the `timing` and `morphology` sections.
`Tier0Scenario::from_ron` reads it back to the same values (a unit test round-trips the whole library).

## Share bundle

A zip of all the files above plus `README.txt`, with fixed entry order, timestamps (1980-01-01, the zip epoch) and
permissions, so the bundle is reproducible too.

## Import

`heartsim_io::import_path` reads CSV (lead columns by name, any order and case; `t_ms` or `t_s` first), WFDB format-16
records (signals matched to leads by description; units mV, uV or V; baselines honoured; other formats refused with a
message), and share bundles. The app puts the record into the strip rings in place of the live simulation and pauses; resuming
restores the live rings. Exporting is off during a review, since an export's header describes the running scenario. `.ron` files
are not recordings: the app loads those as scenarios (`app::exports`).

Ring indices are only monotonic within one set of rings, so everything indexed by them has to follow the swap: pacing marks are
swapped too, the electrode editor is not updated during a review, and `EcgView::signal_replaced` drops the analysis and calipers.
