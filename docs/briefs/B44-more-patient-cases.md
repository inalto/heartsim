# B44 — More patient cases from the literature
**Crates** `assets` (cases), `heartsim-cli` (checks only), docs · **Model** Sonnet (part A, research, with web access;
part B, writing) · **Milestone** M13 · **Depends on** B18; part B's packaging check depends on B41.

## Goal
The Cases page offers more than three cases. Each new case is a teaching scenario the engine can actually show,
chosen from the published literature and written in our own words.

## Part A — research (no build; may run beside an implementation brief)
Write `docs/research/patient-cases.md`.
1. **What the engine can show.** Read `assets/scenarios/*.ron` (ids and titles), `assets/scenarios/reentry/`,
   `assets/drugs.ron`, `crates/heartsim-core/src/tutorial.rs` (`Step`, `UiAction`, `SimAction` or its equivalent:
   what a stage can do: load a rhythm, block a region, give a drug, pace, shock, occlude a coronary branch, …) and
   the three cases in `assets/cases/`. Make a table: clinical event → the action that shows it. A candidate case
   whose key finding the engine cannot show is rejected (say why: for example hyperkalaemia, if there is no
   potassium model; check, do not assume).
2. **Candidates.** Search the open literature for teaching cases and for the guideline statements each case rests
   on: current ESC and ACC/AHA/HRS guidelines (bradycardia and conduction, supraventricular tachycardia,
   ventricular arrhythmias, atrial fibrillation, acute coronary syndromes, pacing, valvular disease, syncope),
   open-access case reports and case series (for example *European Heart Journal – Case Reports*, *JACC: Case
   Reports*, *BMJ Case Reports* open-access items, *Journal of Medical Case Reports*, *Cureus*), and open
   educational resources. For each candidate record: the presenting problem, the sequence of ECG findings, the
   decision points worth a question, the scenario ids and actions that would show it, and the sources with DOI or
   URL and their licence.
3. Aim for a spread the three existing cases do not cover (they are: inferior STEMI with AV block, WPW with
   palpitations, syncope with heart block). Likely areas, to be confirmed against step 1: a broad-complex
   tachycardia (VT against SVT with aberrancy), atrial fibrillation with fast ventricular response and the
   pre-excited AF trap, atrial flutter, AVNRT and adenosine, long QT with torsades, a pacemaker problem (loss of
   capture, undersensing, pacemaker-mediated tachycardia), sick sinus syndrome, an anterior STEMI going into VF and
   defibrillation, pericarditis against STEMI, a murmur case (aortic stenosis with syncope, mitral stenosis with AF)
   that uses the heart sounds. Propose **eight to ten**, ranked, and recommend **six** to write.

**Rules for the research.** Facts and criteria are re-derived from the cited sources and cited; no case text, table,
figure or ECG image is copied from a paper, a book, a web site or a commercial product, whatever its licence (a
CC BY source is cited as a source; we still write our own words, so nothing needs an entry in
`ASSETS-LICENSES.md`). Nothing from `epicardio/`, `epicardio-extracted/` or echosuite.app. Do not create accounts
and do not work around a paywall or a login: a source you could not read is listed as "not read" and nothing is
taken from its abstract beyond what the abstract says. Publisher PDFs are not saved into the repository. A
published case report describes a real person: use it only for the clinical pattern; the patient in our case is
invented (age, sex, setting, history, name all different) and no detail that could identify the reported patient is
carried over. Never send the maintainer's name, e-mail or project details to a search or any other service.
State for each source whether you read the full text, the abstract only, or could not open it.

## Part B — write the cases (after the maintainer or the planner has accepted the list)
* One file per case in `assets/cases/case_<slug>.ron`, in the style and header comment of the existing three
  (original text by the heartsim authors, MIT OR Apache-2.0), `format_version: 1`, at least five questions, three
  to five choices each with one right answer that is **not always the first** (vary the position), an explanation
  for every question, references to the guideline or paper each clinical statement was checked against.
* Stages use only rhythms and actions that exist. Where a stage changes the rhythm, prefer an action on the running
  simulation (a block, a drug, a shock) to loading another rhythm when the engine supports it, as
  `case_chest_pain_inferior_stemi.ron` does, and say in the text what the simulator did.
* Plain clinical English, invented patients, no real names of people or hospitals, no drug brand names. Doses only
  where a guideline gives them and with the citation; add no treatment advice beyond what the cited guideline says.
  The book's disclaimer applies: say nothing that reads as advice for a real patient.
* Every statement that a clinician should check before it is taught goes into `docs/book/src/known-issues.md`
  under the existing "needs a clinician's check" list, by case and stage.
* `ASSETS-LICENSES.md`: the new files under the entry that covers `assets/cases/`.
* Translations: the existing cases are English only (check); keep that.

## Acceptance tests
* `nice -n 10 cargo run -p heartsim-cli -- validate-cases assets/cases` passes for all cases (editorial rules and
  every stage's actions run). The existing content tests that count or list cases are updated.
* For each new case: the position of the right answer is not the same in more than half of its questions (add this
  as a check in `validate-cases` or as a test over `assets/cases/`, applying to the new cases; list the old ones as
  exempt if they fail it, do not rewrite them in this brief).
* If B41 is in: `heartsim-cli case pack` then `case check` succeeds for each new case; the web archive test in
  `xtask/src/web.rs` still passes.
* `nice -n 10 cargo xtask ci`.

## Docs
`docs/research/patient-cases.md` (part A; link it from `docs/README.md`), `docs/book/src/user/cases.md` (the list of
cases with one line each, no spoilers), `known-issues.md`, `CHANGELOG.md` `## [Unreleased]`.

## Rules
One cargo build at a time, always `nice -n 10`, `export PATH="$HOME/.cargo/bin:$PATH"`. Do not commit or stage, do not
edit `CLAUDE.md`. Report what you did not verify, and in particular that no clinician has reviewed the cases.
