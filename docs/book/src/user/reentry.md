# Re-entry presets: AF, flutter, VT, torsades, VF

Most rhythms in heartsim are *timed*: the conduction system decides when each structure fires and the heart follows. Atrial
fibrillation, flutter, scar-related ventricular tachycardia, torsades de pointes and ventricular fibrillation are not
like that. They are **functional or anatomical re-entry**: a wave front that circles, breaks up and keeps itself going in the
tissue, with nobody telling it when to fire. These presets therefore run on the **Tier 2** engine, a reaction–diffusion model
of the heart muscle, and the ECG you see is computed from the same simulated tissue the 3-D heart shows.

Pick a preset from **Re-entry presets…** next to the rhythm list in the Simulator tab. The status line shows which tier is
running, how many *phase singularities* (the tips of rotating waves) the tissue holds, and how fast the simulation runs
compared with real time.

## What is on the list

| Preset | What the tissue is like | What you should see |
|---|---|---|
| Atrial fibrillation (persistent) | Remodelled atria: action potential 0.4 × and conduction 0.3 × normal, the pulmonary-vein sleeves slightly more so; a burst of rapid pacing from the veins starts it | No P waves, a fibrillatory baseline of about 0.15 mV in V1, an irregularly irregular ventricular rate of about 100–130/min (the AV node filters the atrial activity) |

Typical flutter is a procedure of the [EP lab](ep-lab.md), on a circuit drawn into the atrium, not a preset here.
Scar-related VT, torsades de pointes and VF presets are **not in this release**: the Tier-2 engine runs them
as tissue models, but none of the induction protocols tried (programmed stimulation, burst pacing, cross-field stimulation,
a pre-seeded rotating wave) produced a rotor that survives in the heart-shaped model, so they would not do what their names say.
The tools for building them — tissue patches, cross-field and pre-seeded-wave initiation, ablation lines — are in place.
The ordinary [rhythm library](scenarios.md) draws these rhythms, flutter among them, at the timing level, with no tissue model behind them.

## What you can do to it

* **Shock it.** The Therapy window's defibrillator (synchronised or not) cardioverts the AF; two seconds after the last
  sign of re-entry the simulation goes back to the fast Tier 1 engine and the strip shows sinus rhythm with P waves again.
  An *unsynchronised* shock at any time moves the simulation to Tier 2 for a few seconds (it could land on the T wave of a
  normal beat, where a shock can start fibrillation).
* **Ablate it.** Switch the click tool to *Ablate* (or shift-click) and draw a ring round the pulmonary-vein openings
  (pulmonary-vein isolation). Done before the burst, it makes AF impossible to induce; done during established persistent
  AF it does not stop it, because the remodelled atrial body keeps it going (as in patients).
* **Give drugs.** Amiodarone lengthens the refractory period and so the wavelength (the fibrillatory rate falls);
  flecainide slows conduction and slows the atrial rate a lot.

## How switching between tiers works

Tier 1 (the activation-time engine) is fast but cannot sustain a re-entrant circuit; Tier 2 can but is slow. heartsim runs
Tier 1 until something needs re-entry — the scenario's initiation time, a rapid pacing burst, a pacing train with an early
extrastimulus, or an unsynchronised shock — then continues the *same* simulation in Tier 2, with the same beat timing and the
same ECG. When there has been no phase singularity and no unexplained tissue activation for **two seconds** it returns to
Tier 1. The two-second wait (plus a minimum stay in Tier 2) is a deliberate hysteresis: a rotor that drifts out of sight for
a moment must not make the simulation flip back and forth.

## Speed

Tier 2 simulates tens of thousands of voxels per millisecond of heart time. The AF preset needs the 2 mm grid (about
35 000 voxels): on a 4 mm grid (32³ box) the short-APD atrial front does not conduct through the narrow atrial bridges.
That runs at roughly a third of real time on a laptop-class CPU, so the status line shows the achieved speed (for
example *0.4× real time*) and the strip simply moves more slowly. Nothing in a preset depends on the simulation keeping
up with the clock.

## What is simplified

* The cell model is a phenomenological one (Aliev–Panfilov): it has no ion channels, so drugs act through the
  action-potential duration and the conduction velocity only, and early afterdepolarisations do not arise by themselves.
* The model atria are small, so the preset uses markedly shortened refractory periods and slow conduction to make the
  wavelength fit. The fibrillatory rate comes out at about 165/min (2.7 Hz), slower than clinical AF (240–600/min);
  the numbers are teaching values, not patient data.
* A phase singularity count is measured on the voxel grid and is noisy on curved tissue, so the hand-back to Tier 1
  needs singularities that last 0.6 s, or an atrial/ventricular activation at the AV node or lead tip that the timing
  engine did not cause.
