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
| Pre-excitation and re-entry | WPW (left lateral pathway), orthodromic AVRT, AVNRT |
| Ectopy | frequent PACs, ventricular bigeminy, multifocal PVCs |
| Escape and slow ventricular rhythms | junctional rhythm, idioventricular rhythm, AIVR |
| Pacing | AAI, VVI, DDD, biventricular pacing, loss of capture, undersensing, T-wave oversensing, pacemaker-mediated tachycardia |

Every scenario carries teaching notes with its literature sources and the rhythms it is most often confused with; the
[rhythm quiz](quizzes.md) uses those as wrong answers.

## Opening your own scenario files

Scenario files are plain text (RON) and can be edited in any text editor. The [export window](exports.md) saves the
running rhythm as one; [drop a file on the window](exports.md#running-a-scenario-file) to run it, or use `heartsim-cli run`
for a headless run. Teachers can give students a modified file (for example a
slower escape rhythm); [Adding a scenario](../dev/adding-a-scenario.md) describes the format.
