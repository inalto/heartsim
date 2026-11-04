# Adding a drug

Drugs, like rhythms, are data. Each drug is an entry in the drug table, `assets/drugs.ron`: a set of parameter modifiers
(for example AV-node refractory period × 2, sinus rate + 30 %, myocardial conduction velocity × 0.8, a different
action-potential template) with an onset and a half-life. Giving a drug is an `Action`, so it works from the drug tray, from
a scenario timeline and from a tutorial page alike.

## Steps

1. **Find the effect in the literature.** Write down which electrophysiological parameters change, by how much, and how
   quickly (onset, peak, half-life), with the source. `docs/research/electrophysiology-and-ecg-models.md` lists the
   parameters the engines expose and what each one does to the ECG.
2. **Add the entry** to `assets/drugs.ron`, copying the structure of an existing drug with a similar mechanism. Modifiers
   multiply or offset the scenario's parameters while the drug is active and fade with its half-life; keep doses in the
   units clinicians use and cite the source in the entry's notes.
3. **Write a test** next to the existing drug tests in `crates/heartsim-sim/tests/`: run a scenario where the drug has a
   textbook effect (adenosine terminating AVNRT, verapamil slowing the ventricular rate in AF) and assert the effect with
   the B4 analysis, within the time course you specified.
4. **Try it in the app**: load the scenario, give the drug from the tray, and export a share bundle before and after for the
   pull request.

The field names and units of a drug entry are documented on the drug types in `heartsim-sim`; the data file is checked when
it is loaded, so a misspelt parameter fails the test suite rather than being ignored.
