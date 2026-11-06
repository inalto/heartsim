# Rhythm scenarios

A scenario is one rhythm or condition: the settings of the conduction system, the shape of the waves, and optionally a
timeline of events (an ectopic beat at 3.2 s, a pacing threshold change at 10 s). Pick one from the **Rhythm** list in the
rhythm bar; *Restart* reloads it from time zero, and *About* shows its description. Moving a slider afterwards changes the
running rhythm; the list then still shows the scenario you started from.

![Second-degree AV block, Mobitz I (Wenckebach), on the 12-lead page](../images/twelve-lead-mobitz1.png)

The library that ships with heartsim covers:

| Group | Scenarios |
|---|---|
| Sinus | normal sinus rhythm, sinus bradycardia, sinus tachycardia, sinus arrhythmia, sinus arrest with junctional escape, SA exit block |
| AV conduction | first-degree block, Mobitz I, Mobitz II 3:1, third-degree block with junctional or ventricular escape |
| Bundle branches and fascicles | LBBB, RBBB, LAFB, LPFB, bifascicular block |
| Pre-excitation and re-entry | WPW (left lateral and right-sided pathways), orthodromic AVRT, AVNRT, pre-excited atrial fibrillation |
| Atrial flutter and fibrillation | flutter with 2:1, 4:1 and variable block, atrial fibrillation |
| Ectopy | frequent PACs, ventricular bigeminy, multifocal PVCs, R-on-T and others |
| Ventricular tachycardia and fibrillation | monomorphic VT and its runs, polymorphic VT, torsades de pointes, coarse and fine ventricular fibrillation |
| Ischaemia and infarction | STEMI by territory, subendocardial ischaemia, pericarditis, evolved anterior, inferior and lateral infarcts with Q waves and T inversion, an old infarct, the stages in compressed time |
| Escape and slow ventricular rhythms | junctional rhythm, idioventricular rhythm, AIVR |
| Pacing | AAI, VVI, DDD, biventricular pacing, loss of capture, undersensing, T-wave oversensing, pacemaker-mediated tachycardia |

Every scenario carries teaching notes with its literature sources and the rhythms it is most often confused with; the
[rhythm quiz](quizzes.md) uses those as wrong answers.

## What the 3-D heart shows during the sustained arrhythmias

The strip and the heart come from the same engine events, so they show the same rhythm, but the heart is a schematic of these
rhythms and not a simulation of their mechanism:

| Rhythm | Strip | 3-D heart |
|---|---|---|
| Atrial flutter | Sawtooth at about 300/min, the AV ratio from the node | A front from the right atrium every cycle. The anatomical circuit around the tricuspid valve is not drawn |
| Atrial fibrillation | f waves, irregular QRS | Random wavelets in both atria each cycle, no common beat. For a tissue model see [Re-entry presets](reentry.md) |
| Pre-excited AF | Fast, broad, irregular QRS | As atrial fibrillation, with the ventricles entered at the pathway's insertion |
| Ventricular fibrillation | Chaotic deflections, no QRS, no P waves | Random wavelets running into each other and into refractory tissue. There are no rotors behind them |
| Torsades de pointes, polymorphic VT | Large wide complexes that twist, or change every beat | A front from either ventricle each cycle. The twist itself is not drawn |
| Infarct with Q waves | Q waves, T inversion | A silent necrotic core with a slow border zone: the same lesion |

A successful shock stops the rhythm on the strip and on the heart at the same moment. In sync mode the defibrillator waits for an
R wave, so it does not fire during ventricular fibrillation, polymorphic VT or torsades.

Infarct stages in `mi_evolution_stages` are compressed from hours and weeks to seconds, for teaching only.

Magnesium, the first-line treatment of torsades, is not in the drug table yet, and AV-nodal drugs do not show the
acceleration they can cause in pre-excited AF.

## Opening your own scenario files

Scenario files are plain text (RON) and can be edited in any text editor. The [export window](exports.md) saves the
running rhythm as one; [drop a file on the window](exports.md#running-a-scenario-file) to run it, or use `heartsim-cli run`
for a headless run. Teachers can give students a modified file (for example a
slower escape rhythm); [Adding a scenario](../dev/adding-a-scenario.md) describes the format.
