# Patient cases: what the engine can show and which cases to write (brief B44, part A)

Research for the new cases of `assets/cases/` (part B of brief B44). Nothing here is copied from a paper, a book or a web
site: facts are restated and cited, the patients of the future cases are invented. No clinician has reviewed this document.

**How the sources were read.** Pages were opened with a fetch tool that converts a page to text and answers a question
about it; for the ESC guidelines the recommendation tables (class and level) are images that the tool could not read, so
**no class or level of recommendation is confirmed here**. Where a guideline is cited for a statement, the statement comes
from its running text and the part B author must re-check the wording and the class against the document before it is
taught. Status words: **full text** (the whole article came through), **partial** (the text came through but was cut off
or the tables were images), **not read** (403, login or paywall; nothing taken from it), **from memory, not confirmed**
(a reference known to exist, not opened).

## 1. What the engine can show

Read from `assets/scenarios/*.ron` (ids and titles), `assets/scenarios/ep/`, `assets/scenarios/reentry/`,
`assets/drugs.ron`, `crates/heartsim-core/src/tutorial.rs` and `scenario.rs` (`Step`, `UiAction`, `Action`),
`crates/heartsim-sim/src/timing/mod.rs` (`apply`) and the three cases in `assets/cases/`.

### 1.1 The vocabulary of a stage

A case stage runs `Step`s (`case.rs` uses the tutorial vocabulary):

* `Ui(LoadScenario(id))` loads a rhythm and restarts the clock at zero; `Ui(StartReentry("af_persistent"))` starts the
  reaction-diffusion AF preset (slower than real time).
* `Ui(FocusPanel(Simulator | TwelveLead | HeartVector | AxisTrainer | Electrodes | SoundTrainer | AuscultationQuiz | Echo ...))`,
  `SelectLead`, `CameraPreset`, `SetSpeed`, `Pause`/`Resume`, `WaitForEvent(ElapsedMs | Qrs)`, `Highlight`.
* Sound and echo: `HeartSoundAudio`, `HeartSoundLane`, `Listen { site, chest }`, `ProbeAtWindow`, `ProbeAtView`,
  `SetEchoView`, `EchoMeasure("lvid" | "tapse" | "e-prime")`.
* `Sim(Action)` on the running simulation. The timing engine (every `tier: Template` and `Eikonal` scenario) accepts:
  `SetGlobalSinusRate { bpm }`, `Block { region, blocked }` (only regions that are an edge of the timing graph:
  AV node compact, His, bundle branches, fascicles, accessory pathway, ...), `Stimulate`, `PacingTrain`,
  `AccessoryPathway { enabled, antegrade, retrograde }`, `Drug { name, dose }`, `Shock { joules, synchronised }`,
  `Stenosis { branch, severity }` (branches `"LAD"`, `"LCx"`, `"RCA"`, `"all"`, and the stage tags `"LAD infarct"`,
  `"LAD evolving"` used by `mi_evolution_stages.ron`), `BreathHold`.
* **Not available** on the timing engine: `SetRegion`, `Ablate`, `MoveElectrode` (all return `Unsupported`). The
  reaction-diffusion engine (re-entry presets) accepts only stimuli, pacing trains and blocks, no drug, shock or
  stenosis. Cases must therefore stay on Template/Eikonal scenarios.
* Pacemaker settings (mode, sensitivity, output, PVARP) are part of a scenario's `pacer` block; there is **no action** to
  reprogram a device during a stage. A device stage is a `LoadScenario` of one of the `paced_*` ids.
* Shocks: success is drawn from a seeded curve (`defib.rs`), so a shock is deterministic per scenario but can fail;
  a synchronised shock never fires in VF, polymorphic VT or torsades (no reliable R wave); asystole is not shockable.
* Drugs (`assets/drugs.ron`): adenosine, verapamil, diltiazem, digoxin, flecainide, amiodarone, atropine, adrenaline,
  lidocaine, plus two electrolyte "drugs": `hyperkalaemia` (doses 0.5, 1.0, 1.5, about K 6, 7, 8+; stop with dose 0)
  and `hypokalaemia` (0.5, 1.0; dose 0 stops). Onset is 1 s (adenosine) to 15 min of simulated time (digoxin); use
  `SetSpeed` or `WaitForEvent(ElapsedMs)` for the slow ones. **There is no magnesium, calcium, insulin, beta-blocker,
  procainamide, ibutilide, isoprenaline, anticoagulant, thrombolytic or oxygen, and no CPR.**

### 1.2 Clinical event to engine action

| Clinical event | Action that shows it (ids exist today) | Limits |
|---|---|---|
| Normal sinus rhythm baseline | `LoadScenario("sinus_normal")` | |
| Sinus bradycardia, tachycardia, respiratory arrhythmia | `sinus_brady_45`, `sinus_tachy_130`, `sinus_arrhythmia`, or `Sim(SetGlobalSinusRate)` | |
| Sick sinus syndrome, SA exit block, arrest with escape | `sick_sinus_tachy_brady`, `sa_exit_block`, `sinus_arrest_junctional_escape` | alternation is scripted, not drug-driven |
| Junctional or ventricular escape, AIVR | `junctional_rhythm`, `accelerated_junctional`, `idioventricular`, `aivr` | |
| First-degree, Mobitz I, Mobitz II, high-grade, complete AV block | `avb_1st`, `avb_2nd_mobitz1`, `avb_2nd_mobitz2_4to3`, `avb_2nd_mobitz2_3to1`, `avb_3rd_junctional`, `avb_3rd_ventricular`; or on a running rhythm `Block(AvNodeCompact / HisBundle, true)` | |
| Bundle-branch and fascicular block, bifascicular | `rbbb`, `lbbb`, `lafb`, `lpfb`, `bifascicular`; `Block(RightBundleBranch / LeftBundleBranch / LeftAnteriorFascicle ...)`; rate-related aberrancy `rate_dependent_rbbb` | |
| Atropine response of nodal block, vagal slowing | `Drug("atropine", 1.0)` | no infranodal/nodal discrimination except by the scenario chosen |
| Acute STEMI by territory | `stemi_anterior`, `stemi_inferior`, `stemi_lateral`, `stemi_posterior`; or `Stenosis("LAD" / "RCA" / "LCx", 0.9..1.0)` on a running rhythm; evolving: `stemi_evolving`, `mi_evolution_stages` | no troponin, no culprit-by-angiography, no cardiogenic shock |
| NSTEMI / subendocardial ischaemia | `nstemi_subendocardial` (ST depression, ST elevation in aVR) | |
| Reperfusion | `Stenosis("all", 0.0)` or `Stenosis(branch, 0.0)`; evolved infarcts `mi_anterior_evolved`, `mi_inferior_evolved`, `mi_lateral_evolved`, `mi_old_anterior` | |
| Pericarditis | `pericarditis` (diffuse ST elevation, ST depression in aVR, rub in the sound lane, `ecg_diagnostic`) | PR depression is not stated in the scenario description; check on the strip before a question depends on it |
| Premature beats, R-on-T, bigeminy, couplets, NSVT, parasystole | `pvc_*`, `pac_*`, `nsvt_runs` | |
| Monomorphic VT with AV dissociation | `vt_monomorphic` (150 bpm, P waves dissociated) | no fusion/capture-beat annotation unless the strip happens to show them |
| Polymorphic VT, torsades, VF | `polymorphic_vt`, `torsades` (long QT, pause-dependent start), `vf_coarse`, `vf_fine` | VF can only be entered by loading it; a PVC does not degrade into VF on the Template tier |
| Defibrillation / cardioversion | `Shock(joules, synchronised)`; tutorials use 100 J sync for VT, 150 J unsync for torsades, 200 J unsync for VF | outcome probabilistic and seeded |
| Atrial fibrillation (controlled), persistent AF 3-D | `atrial_fibrillation` (about 100 bpm), `StartReentry("af_persistent")` | no AF with a rate of 150 as a ready scenario; the fast case is pre-excited AF or flutter |
| Rate control of AF/flutter | `Drug("diltiazem" / "verapamil" / "digoxin", 1.0)` on `atrial_fibrillation`, `flutter_*`; tests assert a 20 % fall with verapamil | beta-blocker absent |
| Atrial flutter 2:1, 4:1, variable | `flutter_2to1`, `flutter_4to1`, `flutter_variable`; adenosine unmasks flutter waves (`drugs.ron` note) | CTI ablation exists only in the `ep/` scenarios, whose `Ablate` is not a Template action |
| AVNRT, orthodromic AVRT, termination by adenosine | `avnrt_150`, `avrt_orthodromic`, then `Drug("adenosine", 1.0)` (used by the WPW case) | |
| Atrial tachycardia (adenosine does not stop it) | `atrial_tachycardia_150` | |
| WPW pattern, pre-excited AF | `wpw_left_lateral`, `wpw_right_type_b`, `af_preexcited`; pathway block `Block(AccessoryPathway, true)` | verapamil in `af_preexcited` is meant to fail to slow the pathway (`drugs.ron` note); confirm on a run |
| Long QT by drug or electrolyte | `Drug("hypokalaemia", 1.0)` (low flat T, U wave, long QT(U)); `Drug("amiodarone", 1.0)` (APD 1.15x); `torsades` | no magnesium, no QT-drug list, no genotype |
| Hyperkalaemia | `Drug("hyperkalaemia", 0.5 / 1.0 / 1.5)` on `sinus_normal` or another rhythm: peaked T, short QT, flat wide P, long PR, wide QRS; `Drug("hyperkalaemia", 0.0)` stops it | calcium, insulin, dialysis not modelled; the case text says what the simulator did |
| Digoxin effect and toxicity | `Drug("digoxin", 1.0)` (scooped ST, short QT, more ectopy and AV block) | |
| Pacemaker normal function | `paced_aai_sinus_brady`, `paced_vvi`, `paced_ddd`, `paced_biv` | |
| Pacemaker malfunction | `paced_loss_of_capture` (threshold rises, spikes without QRS, 30 bpm escape), `paced_undersensing`, `paced_t_wave_oversensing`, `paced_pmt` (endless-loop tachycardia stopped by a longer PVARP) | no in-stage reprogramming; no chest X-ray or interrogation |
| Heart sounds and murmurs | scenario `auscultation` section: `aortic_stenosis_mild/severe`, `aortic_regurgitation`, `mitral_stenosis_sinus/af`, `mitral_regurgitation`, `mitral_valve_prolapse`, `tricuspid_regurgitation`, `pulmonary_stenosis`, `ventricular_septal_defect`, `atrial_septal_defect`, `innocent_murmur`, `heart_failure_s3`, `lvh_s4`, `summation_gallop`; `Listen`, `BreathHold` | audio needs sound output; `run-tutorial` and validate-cases ignore it, so a question must also be answerable from the text and the trace |
| Echo views and three measurements | `FocusPanel(Echo)`, `ProbeAtView`, `EchoMeasure(...)` | no aortic valve area or gradient measurement |
| Axis | `SetQrsAxis`, `axis_*`, `lafb`, `lpfb` | |

### 1.3 What the engine cannot show (so a key finding that needs it rules the case out)

Troponin and other blood tests; blood pressure, shock or heart failure as a state; syncope as an event (the story says it, the
trace cannot); CPR; magnesium, calcium, insulin, beta-blockers, procainamide, ibutilide, isoprenaline, anticoagulants;
thyroid disease; Brugada pattern, congenital long-QT genotypes, ARVC, Takotsubo (no scenario); reprogramming a pacemaker
or ICD during a stage; ablation in a Template-tier case; VF arising from a PVC on a Template scenario; Valsalva and
other loading manoeuvres other than a breath-hold; pulse oximetry, exercise testing.

## 2. Candidate cases, ranked

Ranking rule: first how well the engine shows the decisive finding without a workaround, then how little the case overlaps
the three shipped cases (inferior STEMI with AV block, WPW with orthodromic AVRT, syncope with heart block), then how good
the open sources are. Each patient below is a placeholder: part B invents the person.

### C1. Regular wide-complex tachycardia: ventricular tachycardia until proved otherwise
* **Presenting problem.** An older adult with an old infarct history and sudden palpitations and breathlessness; a wide,
  regular tachycardia at about 150.
* **ECG sequence.** (1) `vt_monomorphic`: wide bizarre QRS, P waves marching through at their own rate (AV dissociation).
  (2) Contrast stage: `sinus_normal` then `rate_dependent_rbbb` (a wide QRS appears when the rate rises, with a normal
  recovery when it falls: aberrancy); the learner compares the two. (3) Synchronised cardioversion (VT with pulse and
  hypotension) with the unstable branch, or the stable branch (amiodarone `Drug` then shock if it persists).
* **Decision points.** What is the default diagnosis of a wide regular tachycardia when the mechanism is unclear; which
  findings favour VT (AV dissociation, a history of structural disease); why verapamil is a trap (the case that found
  a verapamil-sensitive-looking tachycardia that turned out to be AVNRT with aberrancy is a reason for caution, not
  a licence); unstable versus stable branch; why adenosine is not a test in an irregular or pre-excited rhythm.
* **Engine.** `Ui(LoadScenario("vt_monomorphic"))`, `Ui(WaitForEvent(ElapsedMs(7000)))`,
  `Sim(Shock(joules: 100, synchronised: true))` (the same sequence the 09_ventricular tutorial uses), then
  `LoadScenario("sinus_normal")`. Optional: `Sim(Drug("amiodarone", 1.0))` for the stable branch.
* **Sources.** ESC 2019 SVT guideline (Brugada J et al., Eur Heart J 2020;41:655-720, doi 10.1093/eurheartj/ehz467):
  **partial** text; default diagnosis is VT, unstable patients are cardioverted, a stable wide tachycardia of unclear
  mechanism is treated as VT, procainamide or amiodarone in hospital; the tables with class and level were images and
  are **not confirmed**. Copyright ESC; no open licence stated on the page. Case: Prasitlumkum N et al., "An Endless
  Riddle of Wide Complex Tachycardia: VT or SVT?", Cureus 2026, doi 10.7759/cureus.109414, PMC13283302: **full text**,
  CC BY 4.0; used only for the pattern (surface algorithms can disagree, compare with the baseline ECG, lead reversal
  can fake a change in axis, empirical verapamil is dangerous when VT is possible). 2022 ESC ventricular arrhythmia
  guideline (Zeppenfeld K et al., Eur Heart J 2022;43:3997-4126, doi 10.1093/eurheartj/ehac262): **partial** (contents and
  figure titles only; the recommendation tables are images). Brugada P et al., Circulation 1991;83:1649 and Vereckei A
  et al., Heart Rhythm 2008;5:89: **from memory, not confirmed**; a case may name the criteria in general terms only.

### C2. Anterior STEMI, a malignant ventricular arrhythmia and defibrillation
* **Presenting problem.** Crushing chest pain, a proximal LAD pattern on the first ECG; then collapse in the department.
* **ECG sequence.** `stemi_anterior` (ST elevation V1 to V6, I, aVL, reciprocal inferior depression); a stage with
  ventricular ectopy on the T wave (`pvc_r_on_t` shows the timing: coupling 320 ms on the T peak); `vf_coarse`;
  unsynchronised shock; sinus rhythm; after reperfusion `Stenosis("all", 0)` and finally `mi_anterior_evolved`.
* **Decision points.** Which territory and which artery; reciprocal change; the early arrhythmia of an acute infarct
  is a reason for monitoring, not for delay; VF is shocked unsynchronised (sync mode refuses in VF; the tutorial teaches
  this); what an evolved infarct looks like (Q waves, T inversion) after reperfusion; why the ECG is read before the
  troponin returns.
* **Engine.** `LoadScenario("stemi_anterior")`; `LoadScenario("pvc_r_on_t")` is a different scenario (loses the ST
  elevation), so the R-on-T stage is **told** and the VF stage is a `LoadScenario("vf_coarse")` followed after about
  6 s by `Shock(200 J, unsynchronised)`; recovery `LoadScenario("stemi_anterior")` then `Stenosis("LAD", 0)`.
  The simulator cannot degrade the infarct rhythm into VF by itself; the text must say "(the simulator now shows the
  arrest rhythm)".
* **Sources.** Savaş G et al., "R-on-T-triggered ventricular fibrillation during acute anterior ST-elevation myocardial
  infarction", Eur Heart J Case Rep 2026;10(5), doi 10.1093/ehjcr/ytag342, PMC13193675: **full text**, CC BY 4.0;
  pattern only (anterior STEMI, a PVC on the T wave starts VF, unsynchronised shock, proximal LAD occlusion). The
  existing case's references (2023 ESC ACS guideline, 2017 ESC STEMI guideline, Fourth Universal Definition) apply and
  were **not read** by this research; the ESC ventricular arrhythmia guideline above is **partial**. The 2014
  EHRA/ACCA/EAPCI position paper on arrhythmias in ACS is cited by the case report only; **from memory, not confirmed**.

### C3. The pre-excited AF trap
* **Presenting problem.** A young adult with a known short-PR pattern, palpitations, light-headedness; fast, broad,
  irregular tachycardia.
* **ECG sequence.** `wpw_left_lateral` (delta wave, short PR) then `af_preexcited` (fast, broad, irregular, up to over
  250 bpm); an AV-nodal blocker is offered as a wrong answer (`Drug("verapamil", 1.0)` shows the nodal drug does
  not slow the pathway: **to be confirmed by running it**); synchronised cardioversion (150 J, as tutorial 08 does).
* **Decision points.** Why "irregular and wide" in a patient with pre-excitation is not VT and is not rate-controlled
  with nodal drugs; which drugs to avoid (adenosine, verapamil, diltiazem, digoxin, beta-blockers in guideline text);
  electrical cardioversion if unstable; the long-term answer is ablation.
* **Overlap.** The shipped WPW case already covers the pathway and orthodromic AVRT. This case would reuse
  `wpw_left_lateral`; the new content is the AF trap only. Hence rank 3, not higher.
* **Sources.** Schiavone M et al., "Pre-Excited Atrial Fibrillation in WPW Syndrome: A Case Report and a Review of the
  Literature", Rev Cardiovasc Med 2024;25(4):125, doi 10.31083/j.rcm2504125, PMC11263991: **full text**, CC BY 4.0
  (pattern only: a patient with a mildly wide irregular tachycardia, AV-nodal blockers avoided, cardioversion, ablation
  later; the report involves thyroid storm, which the engine cannot show and which part B must **not** carry over).
  ESC 2019 SVT guideline: **partial** (electrical cardioversion is the acute treatment of choice for irregular
  pre-excited tachycardia; adenosine must be avoided if pre-excitation suggests pre-excited tachycardia).
  `af_preexcited.ron` already cites Klein 1979 (NEJM 301:1080), **not read** here.

### C4. Acquired long QT and torsades de pointes (hypokalaemia, a slow rate, a pause)
* **Presenting problem.** An older adult on a diuretic, with vomiting, palpitations and a fainting episode.
* **ECG sequence.** `sinus_brady_45` plus `Drug("hypokalaemia", 1.0)` (low flat T, a U wave, long QT(U), more ectopy);
  `torsades` (long QT, bigeminy, a short-long-short sequence, polymorphic VT that twists); unsynchronised shock
  (150 J) and the observation that the long QT is still there so it recurs; correction `Drug("hypokalaemia", 0.0)`
  and `sinus_normal` to show the QT back.
* **Decision points.** Read the QT and name the cause; why the sequence is pause-dependent; shock polymorphic VT
  unsynchronised; the standing measures (stop QT-prolonging drugs, correct potassium and magnesium, raise the rate)
  as the cited guideline words them; why amiodarone is not the answer for torsades (it lengthens QT; the engine
  models it so: APD 1.15x).
* **Engine.** All of the above exist. Magnesium and pacing as treatment are not available and are **told only**.
  Tutorial 09 already uses the torsades scenario and says "magnesium is not in the simulator's drug table yet".
* **Sources.** Drew BJ et al., AHA/ACCF scientific statement on prevention of torsade de pointes in hospital settings,
  Circulation 2010;121:1047-1060 (cited by `torsades.ron`): **not read** here. Dzebu AS et al., "A Scary Tale of a
  Twisted Storm: Torsades de Pointes in Acquired Long QT Syndrome", Cureus 2025, doi 10.7759/cureus.86577,
  PMC12284911: **full text**, CC BY 4.0 (pattern: bradycardia from high-grade AV block, QTc 544 ms, a long-short
  sequence, magnesium; permanent pacing is generally needed for symptomatic block without a reversible cause).
  ESC 2022 ventricular arrhythmia guideline: **partial** (section 7.2.2 and figure 13 on QT-prolonging drugs exist;
  wording not read).

### C5. Atrial flutter with 2:1 conduction behind a "sinus tachycardia"
* **Presenting problem.** Breathlessness and palpitations; a regular narrow tachycardia at exactly 150 that did not
  change with rest.
* **ECG sequence.** `flutter_2to1` (regular 150, one flutter wave hidden in the ST-T); adenosine `Drug` shows flutter
  waves (the AV block unmasks them); rate control (`Drug("diltiazem", 1.0)`; the ratio changes, see
  `flutter_variable` and `flutter_4to1`); synchronised cardioversion 50 to 100 J range as `defib.rs` documents for
  flutter (use 100 J in the case, as tutorial 06 does). Contrast `atrial_tachycardia_150` where adenosine does not stop it.
* **Decision points.** A fixed rate near 150 suggests flutter 2:1; adenosine as a diagnostic aid and why it does not
  terminate flutter; rate control versus rhythm control; the stroke-prevention question (told, not shown).
* **Engine.** All present, except ablation of the cavotricuspid isthmus, which is only in `ep/ep_flutter.ron` (not a
  Template action); the case says ablation in text.
* **Sources.** ESC 2019 SVT guideline: **partial** (adenosine "does not interrupt macro-re-entrant atrial tachycardias",
  and may reveal flutter waves); ESC 2020 AF guideline (Hindricks G et al., Eur Heart J 2021;42:373-498,
  doi 10.1093/eurheartj/ehaa612) and its 2024 update (doi 10.1093/eurheartj/ehae176, from the page's own note): **partial**,
  the page gave only the table of contents (section 17 covers flutter). 2023 ACC/AHA/ACCP/HRS AF guideline
  (doi 10.1161/CIR.0000000000001193): **not read** (HTTP 403; the PMC copy PMC11095842 was blocked by a browser check).
  `flutter_2to1.ron` carries its own references. No case report read for flutter; a candidate list entry (Miyake D,
  Eur Heart J Case Rep 2026, doi 10.1093/ehjcr/ytag491) was **seen in a search listing only, not read**.

### C6. A pacemaker problem: loss of capture and its relatives
* **Presenting problem.** A pacemaker-dependent patient, a few days or weeks after a device procedure, with new
  dizziness or a faint.
* **ECG sequence.** `paced_vvi` (normal paced rhythm at 60); `paced_loss_of_capture` (spikes without QRS, a slow
  ventricular escape at about 30); then a short comparison with `paced_undersensing` (spikes landing in the sinus
  rhythm, competition) and `paced_t_wave_oversensing` (rate falls to about 46) as the other two ways a device can go wrong.
* **Decision points.** Spikes without a QRS after them mean failure to capture; a rising threshold, a lead problem, a
  recent procedure as causes; what is the danger in a pacemaker-dependent patient; undersensing versus oversensing from
  the position of the spikes; a first-line action the cited sources describe (device interrogation, a temporary bridge).
* **Engine.** Three `LoadScenario` stages; no re-programming action, so "the device is reprogrammed" is told, and the
  stage after the fix is `paced_vvi` or `paced_ddd`.
* **Sources.** Cacciapuoti F et al., "When Pacing Fails After Generator Replacement: A Stepwise Diagnostic Approach to a
  Reversible Lead-Header Interface Problem", Reports (MDPI) 2026;9(2):137, doi 10.3390/reports9020137, PMC13214654:
  **full text**, CC BY 4.0 (pattern: syncope after a generator change, complete heart block with a slow escape, loss
  of bipolar capture with a high impedance, a temporary isoprenaline bridge, fluoroscopy, a seating problem; the text
  does not describe the pacing spikes, so the ECG appearance comes from the simulator scenario descriptions and the
  NBG timing reference in `paced_*.ron`). ESC 2021 pacing guideline (Glikson M et al., Eur Heart J 2021;42:3427-3520,
  doi 10.1093/eurheartj/ehab364): **partial** (the text on mode and sinus node disease came through; nothing on device
  malfunction in the part read). Bernstein AD et al., NBG code, PACE 2002 (cited by `paced_pmt.ron`): **not read** here.

### C7. Pericarditis or STEMI: the diffuse ST elevation question
* **Presenting problem.** A young adult with sharp pleuritic chest pain that eases on leaning forward, after a viral illness.
* **ECG sequence.** `pericarditis` (ST elevation in most leads, ST depression in aVR, in no coronary territory, with a
  friction rub in the sound lane); compare `stemi_anterior` (a territory, reciprocal change); optionally a final stage
  with a rub (`Listen`/`HeartSoundLane`).
* **Decision points.** Why diffuse ST elevation with no reciprocal change is not a single-vessel STEMI; the criteria
  for the diagnosis (two of four: chest pain, rub, ECG, effusion); which test finds an effusion (echo, `FocusPanel(Echo)`);
  the first-line treatment the guideline names (an anti-inflammatory drug with colchicine); what must be excluded.
* **Engine.** Two `LoadScenario` calls and a panel focus; no treatment is shown.
* **Sources.** ESC 2015 guidelines on pericardial diseases (Adler Y et al., Eur Heart J 2015;36:2921-2964,
  doi 10.1093/eurheartj/ehv318): **partial** (the diagnostic criteria, the ECG description, aspirin or NSAID plus
  low-dose colchicine came through; class and level were images and are **not confirmed**). `pericarditis.ron` has
  its own references (Burns/Buttner LITFL page). Whether PR depression is shown is unconfirmed (see the table).

### C8. Hyperkalaemia in a patient on dialysis
* **Presenting problem.** A missed dialysis session, weakness, palpitations.
* **ECG sequence.** `sinus_normal` + `Drug("hyperkalaemia", 0.5)` (tall narrow peaked T); dose 1.0 (flat wide P, longer
  PR, wider QRS); dose 1.5 (towards the sine wave); `Drug("hyperkalaemia", 0.0)` as the treated state.
* **Decision points.** Name the ECG sequence and match it to the level; the ECG may look worse or better than the
  potassium suggests; which finding makes it an emergency; (calcium, insulin and dialysis are told, not shown).
* **Why it ranks low.** The engine shows the ECG well, but the management of the case is entirely outside the engine,
  so the questions would be about text; the ECG alone makes a good tutorial page rather than a full case. The one case
  report on BRASH read here (Shetty D et al., Cureus 2026, doi 10.7759/cureus.110308, PMC13333146, **full text**, CC BY 4.0)
  had no peaked T or wide QRS at K 6.9 (junctional rhythm 46 bpm, LAFB), so the classic sequence does not always appear;
  a BRASH case (bradycardia, renal failure, nodal blocker, shock, hyperkalaemia) is **rejected** because the engine
  has no beta-blocker, no renal failure and no shock to combine.
* **Sources.** Life in the Fast Lane, "Hyperkalaemia" ECG library page, litfl.com/hyperkalaemia-ecg-library/: **full
  text** (readable), site licence CC BY-NC-SA 4.0, cited only for the order of the changes; UK Kidney Association and
  Resuscitation Council UK hyperkalaemia guidance: **from memory, not confirmed**, not opened.

### C9. Mitral stenosis that turns into atrial fibrillation
* **Presenting problem.** A middle-aged woman born abroad, breathless on exertion, palpitations that became constant.
* **ECG sequence / sounds.** `mitral_stenosis_sinus` (loud S1, opening snap, rumble with presystolic accent) then
  `mitral_stenosis_af` (the same sounds, no presystolic accent, irregularly irregular); rate control with
  `Drug("diltiazem" / "digoxin")` on `mitral_stenosis_af` (to be confirmed: the scenario is a murmur case with `ecg_diagnostic`
  not set); echo `FocusPanel(Echo)`.
* **Decision points.** Why the presystolic accent disappears in AF; why a longer diastole helps filling (told);
  anticoagulation in AF with clinically significant mitral stenosis.
* **Sources.** ESC/EACTS 2021 valvular heart disease guideline (Vahanian A et al., Eur Heart J 2022;43:561-632,
  doi 10.1093/eurheartj/ehab395): **partial** (MVA <= 1.5 cm2 defines clinically significant stenosis; NOACs are not
  recommended in AF with clinically significant mitral stenosis; the VKA text and rate-control text were not in the
  part read).

### C10. Severe aortic stenosis and exertional syncope
* **Presenting problem.** An older adult who blacked out climbing stairs.
* **ECG / sounds.** `aortic_stenosis_mild` then `aortic_stenosis_severe` (late-peaking murmur, single soft S2, S4);
  `lvh_s4` for the ECG of LVH; `Listen { Aortic }`; echo view `plax`.
* **Decision points.** What the murmur shape tells about severity; why a symptomatic patient is a different case from
  an asymptomatic one; the thresholds for severe stenosis; why exercise testing is discouraged in symptomatic patients.
* **Why it ranks last.** The engine shows the sounds, not the valve gradient or the area, and the key decision (valve
  replacement) cannot be shown; the case would be mostly text. Sound needs audio.
* **Sources.** ESC/EACTS 2021 valvular guideline: **partial** (high-gradient AS is defined as a mean gradient
  >= 40 mmHg, a peak velocity >= 4.0 m/s, an area <= 1.0 cm2; intervention is strongly recommended in symptomatic severe
  AS regardless of LVEF; exercise testing is discouraged in symptomatic patients). A listing result, "Torsades de
  Pointes in Severe Aortic Stenosis" (Kumar P, J Innov Card Rhythm Manag 2026, doi 10.19102/icrm.2026.17102), was
  **seen in a search listing only, not read**.

### Considered and not ranked
* **Sick sinus syndrome with syncope, then pacing** (`sick_sinus_tachy_brady`, `paced_aai_sinus_brady` or `paced_ddd`):
  the engine shows it, but the shipped syncope case already ends in `paced_ddd` and teaches the pacing indication.
  Text from the ESC 2021 pacing guideline (**partial**): pacing only for symptomatic sinus node disease, with a
  symptom-rhythm correlation; no prognostic benefit shown for asymptomatic patients. A reserve case.
* **AVNRT and adenosine** (`avnrt_150`): shipped WPW case already uses adenosine on AVRT.
* **Pacemaker-mediated tachycardia** (`paced_pmt`): folded into C6 as one more malfunction if a fourth stage is wanted.

### Rejected because the engine cannot show the key finding
* **Brugada pattern, congenital long-QT genotype, ARVC, Takotsubo**: no scenario and no action for any of them.
* **Thyroid storm with AF** (the trigger in the Schiavone report): thyroid state is not modelled.
* **BRASH syndrome**: needs a beta-blocker, renal failure and shock together; only the hyperkalaemia and AV-block
  parts exist (see C8).
* **Cardiogenic shock after infarction, cardiac arrest with CPR**: no haemodynamics, no CPR.
* **Aortic stenosis decision (valve area, gradient)**: the echo panel has only three measurements, none of them valve area.
* **A "slow-then-fast" AF with a rate of 150 as a scenario**: no ready scenario; pre-excited AF or flutter 2:1 are the
  fast, ready rhythms.

## 3. Recommendation: six cases to write

1. **C1 Wide-complex tachycardia (`vt_monomorphic`, `rate_dependent_rbbb`, `sinus_normal`).** It is the commonest
   dangerous decision in emergency electrocardiography, the engine has both the VT and the aberrancy scenario, and
   the shock sequence already runs in a tutorial. It adds a decision (assume VT) that the three shipped cases do not touch.
2. **C2 Anterior STEMI and defibrillation (`stemi_anterior`, `vf_coarse`, `mi_anterior_evolved`, `Stenosis`, `Shock`).**
   It completes the ischaemia strand (the shipped case is inferior with AV block) with the other territory, the
   malignant arrhythmia and the evolved infarct, and uses `Stenosis` before and after reperfusion. The arrest rhythm
   is a rhythm swap, which the text must say.
3. **C4 Acquired long QT and torsades (`sinus_brady_45` + `hypokalaemia`, `torsades`, `sinus_normal`, unsynchronised shock).**
   It is the only case that teaches repolarisation, and the engine has an electrolyte drug that makes the QT long and
   a pause-dependent torsades scenario. The standing measures (magnesium, pacing) are told, not shown.
4. **C5 Atrial flutter with 2:1 conduction (`flutter_2to1`, `flutter_variable`, adenosine, diltiazem, synchronised shock,
   `atrial_tachycardia_150` as contrast).** It covers the supraventricular side the shipped cases skip (they have AVRT
   only), and the adenosine "unmasking" is a visible effect. One of the few cases whose first picture misleads on purpose.
5. **C6 Pacemaker malfunction (`paced_vvi`, `paced_loss_of_capture`, `paced_undersensing`, `paced_t_wave_oversensing`).**
   Device troubleshooting is a recurring exam topic and the engine has four ready scenarios no tutorial case uses; it
   follows the shipped syncope case that ends with a pacemaker. No in-stage reprogramming, so the fix is told.
6. **C7 Pericarditis against STEMI (`pericarditis`, `stemi_anterior`, the rub, echo).** A clean pattern-recognition case
   that adds the heart-sound lane and the Echo panel to a case, with a non-ischaemic diagnosis; low risk of an
   engine surprise. (If a sixth arrhythmia is preferred, C3 replaces it; C3 overlaps the shipped WPW case.)

Reserves, in order: C3 pre-excited AF, C9 mitral stenosis to AF, C8 hyperkalaemia, sick sinus syndrome, C10.

## 4. What to carry into part B, and what is not confirmed

* Class and level of every guideline recommendation: **not confirmed**; the tables were images. Part B cites the
  guideline for the statement and puts the statement in `known-issues.md` for a clinician to check.
* Doses: this document gives no dose except the shock energies the repository already uses (100 J synchronised for VT
  and flutter, 150 J synchronised for pre-excited AF, 150 J unsynchronised for torsades, 200 J unsynchronised for VF)
  and the drug-table doses (`standard_dose` in `drugs.ron`); the guideline doses were not read.
* Things to run before writing (the research could not build or run anything): `af_preexcited` with `verapamil`
  (does the rate stay high), `flutter_2to1` with `adenosine` (are flutter waves visible), `pericarditis` PR segment,
  `hyperkalaemia` doses 0.5 to 1.5 on `sinus_normal`, `stemi_anterior` then `vf_coarse` rhythm swap in the UI,
  and `Shock` success on the chosen seeds (a failure is part of the engine).
* All three shipped cases put the right answer at index 0 (24 of 24 questions: `grep -c "answer: 0"` equals the
  `answer:` count in each file), so the brief's rule that the right answer is not in the same place in more than half
  the questions is new, and the existing cases would fail it.

## 5. Source table

| Source | Status | DOI / URL | Licence |
|---|---|---|---|
| Brugada J et al., ESC 2019 SVT guideline, Eur Heart J 2020;41:655 | partial | 10.1093/eurheartj/ehz467 | © ESC, no open licence on the page |
| Zeppenfeld K et al., ESC 2022 ventricular arrhythmias guideline, Eur Heart J 2022;43:3997 | partial (contents, figure titles) | 10.1093/eurheartj/ehac262 | © ESC, no commercial use |
| Hindricks G et al., ESC 2020 AF guideline (and 2024 update, ehae176) | partial (contents only) | 10.1093/eurheartj/ehaa612 | © ESC |
| Glikson M et al., ESC 2021 pacing guideline | partial | 10.1093/eurheartj/ehab364 | © ESC |
| Adler Y et al., ESC 2015 pericardial diseases | partial | 10.1093/eurheartj/ehv318 | © ESC |
| Vahanian A et al., ESC/EACTS 2021 valvular heart disease | partial | 10.1093/eurheartj/ehab395 | © ESC |
| Joglar JA et al., 2023 ACC/AHA/ACCP/HRS AF guideline, Circulation 2024;149:e1 | not read (403; PMC browser check) | 10.1161/CIR.0000000000001193 | not determined |
| Kusumoto FM et al., 2018 ACC/AHA/HRS bradycardia guideline, Circulation 2019;140:e382 | not read (403) | 10.1161/CIR.0000000000000628 | not determined |
| Drew BJ et al., AHA/ACCF torsade scientific statement, Circulation 2010;121:1047 | not read | from memory (cited by `torsades.ron`), not confirmed | |
| Byrne RA et al., 2023 ESC ACS guideline | not read | cited by the shipped case | |
| Schiavone M et al., Rev Cardiovasc Med 2024;25:125 | full text | 10.31083/j.rcm2504125, PMC11263991 | CC BY 4.0 |
| Dzebu AS et al., Cureus 2025 | full text | 10.7759/cureus.86577, PMC12284911 | CC BY 4.0 |
| Prasitlumkum N et al., Cureus 2026 | full text | 10.7759/cureus.109414, PMC13283302 | CC BY 4.0 |
| Cacciapuoti F et al., Reports (MDPI) 2026;9:137 | full text | 10.3390/reports9020137, PMC13214654 | CC BY 4.0 |
| Shetty DC et al., Cureus 2026 (BRASH) | full text | 10.7759/cureus.110308, PMC13333146 | CC BY 4.0 |
| Savaş G et al., Eur Heart J Case Rep 2026;10(5) | full text | 10.1093/ehjcr/ytag342, PMC13193675 | CC BY 4.0 |
| LITFL hyperkalaemia ECG library page | full text (page) | litfl.com/hyperkalaemia-ecg-library/ | CC BY-NC-SA 4.0 (cited only) |
| Brugada 1991 (Circulation 83:1649), Vereckei 2008 (Heart Rhythm 5:89), Klein 1979 (NEJM 301:1080), Bernstein 2002 (NBG, PACE) | not read | from memory, not confirmed | |
| Miyake D (EHJ-CR 2026, ytag491), Kumar P (JICRM 2026, icrm.2026.17102) and other titles seen in Europe PMC listings | not read | | |

The Europe PMC and article pages were reached through public pages only: no account, login or paywall workaround was used,
no publisher PDF was saved, and no query contained anything about the project or its maintainer.
