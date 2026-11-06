# The EP lab: catheters, electrograms, stimulation, ablation

The **EP Lab** tab is a cardiac electrophysiology laboratory. Choose a procedure from the list at the top and the five standard
catheters go in; the heart in the middle shows them and the activation wave, and the stack on the right shows three surface leads
and every intracardiac electrogram at 100 or 200 mm/s.

Everything on the screen comes from one simulation: the wave on the heart, the surface ECG and the electrograms are the same cells
seen three ways, and a stimulus either captured in all of them or in none.

## Procedures

| Procedure | What to do | What you should find |
|---|---|---|
| EP study: baseline | Measure AH and HV; run an automatic ERP search from the HRA and the RVA | AH 50–120 ms, HV 35–55 ms; CS activation proximal to distal; atrial, AV-nodal and ventricular ERPs in their normal ranges |
| AVNRT and slow-pathway modification | Search the fast-pathway ERP from the HRA, then give S2 at that coupling | An A2–H2 jump of ≥ 50 ms, then a narrow-complex tachycardia with A and V almost together; after ablating the slow pathway it can no longer be induced and the PR interval is unchanged |
| Left-lateral accessory pathway | Give S2 from the HRA, read the CS during the tachycardia, ablate the pathway | Orthodromic AVRT with **distal-first** (eccentric) CS activation; after ablation it can no longer be induced |
| Typical flutter and CTI ablation | Entrain from the isthmus and from elsewhere, then ablate the line | A regular macro-re-entry; PPI − TCL ≤ 30 ms in the circuit and longer outside it; the line stops the wave and sinus rhythm returns |
| AF and pulmonary-vein isolation | Isolate the veins before the burst, or after AF has started | PVI prevents the induction but does not stop AF that is already established; a shock does |

The flutter and AF procedures run on the slower reaction-diffusion engine; the status line shows which engine is running and how far
behind real time it is. The other three run in real time.

Measured in this model, for comparison with your own calipers: AH 77 ms and HV 41 ms in the baseline study; CS atrial activation from about 45 ms
(`CS 9-10`) to 91 ms (`CS 1-2`) after the sinus discharge; in left-lateral AVRT from about 9 ms (`CS 1-2`) to 24 ms (`CS 9-10`) after the
retrograde atrial activation; flutter cycle 235 ms, with PPI − TCL about −10 ms at the isthmus and about +120 ms at the left atrial roof.

## Catheters

| Catheter | Electrodes | Usual site | What it shows |
|---|---|---|---|
| HRA | 4 (2-5-2 mm) | High right atrium, by the sinus node | Atrial activation |
| His | 6 (2-5-2-5-2 mm) | Across the tricuspid valve at the His bundle | A, H and V on one trace |
| CS | 10 (2-6-2 mm) | Coronary sinus | Left atrial and left ventricular activation along the mitral annulus |
| RVA | 4 (2-5-2 mm) | Right ventricular apex | Ventricular activation |
| ABL | 4 | Wherever you put it | Local activation at the tip; delivers RF |

Electrode 1 is the most distal, as in the lab, so `CS 1-2` is deep in the vessel and `CS 9-10` at the ostium.

Move a catheter by picking a landmark from its list, slide the CS with its depth slider, and click on the heart to put the ablation tip
there. A tip cannot leave its chamber: it stops at the wall. The real CS decapolar is usually longer (2-8-2 mm); the model's vessel is
short, so its electrodes are closer together.

## Electrograms and measurements

Each electrode records the local potential from the tissue within 15 mm of it. *Bipolar* (the default) is the difference between the
two electrodes of a pair, which cancels what they both see and leaves local activity. Tick *Unipolar* to see the distal electrode alone,
with its far field. The orange lines are stimuli.

The **AH**, **HV**, **A–A** and **V–V** read-outs are measured from the electrograms of the last beat. AH runs from the A to the H on the
His catheter; HV runs from the H to the earliest V on any catheter. The *AH caliper* and *HV caliper* buttons put a caliper on that beat so
you can see what was measured. Drag across the stack for a caliper of your own; a right-click clears it. Pause first if the trace moves too fast.

## Programmed stimulation

Choose any bipole as the **pacing pair**, then a protocol:

* **Continuous pacing** at a fixed cycle, until you stop it.
* **S1 train with S2 / S3**: S1 × n, then one or two premature stimuli.
* **Decremental pacing**: trains at a cycle that shortens step by step. Watch for the Wenckebach point.
* **Automatic ERP search**: S1 × n and S2, shorter by 10 ms each time, until the chosen target fails. The result is the longest S1–S2 that
  failed. If the tissue under the pair goes refractory first, the panel says so instead of giving a number, as often happens with the AV
  node when pacing the atrium. The search needs the fast engine, so it is not offered during flutter or AF.
* **Entrainment**: measures the tachycardia cycle at the pair, paces 20 ms faster and times the first return beat. PPI − TCL ≤ 30 ms puts the
  pair in the circuit. When pacing the RV in a supraventricular tachycardia, more than 115 ms favours AVNRT over AVRT. The panel says so if some
  stimuli did not capture, or if the pacing stopped the tachycardia instead.

A pair more than 6 mm from any tissue cannot capture, and the panel says so.

## Ablation

*RF on* heats the tissue at the ablation tip. The read-out shows time, temperature, impedance and lesion radius. **Temperature and impedance
are a teaching stub, not a thermal model**: they rise and fall with one time constant and with how close the tip is to the wall. The lesion
grows only above 50 °C, up to 4 mm radius at full contact, so a catheter floating in the blood pool or an application stopped early leaves no
lesion. The default time constant is 8 s; shorten it when the slow engine is running.

*Go* puts the tip on a target of the procedure, and *Ablate all* lays the whole target at once, which is the practical way to make a line.

A lesion kills the tissue under it, and whatever conduction structure it touches: the compact AV node or the His bundle (complete block with
a junctional escape), a nodal pathway, a bundle branch, or an accessory-pathway insertion. Lesions are permanent for the rest of the procedure;
*Restart* or choosing a procedure from the list starts over with a healthy heart.

## What is simplified

* Nodal, His and accessory-pathway conduction come from the timing model, not from tissue: the delay through the node does not depend on its
  anatomy, and a lesion either destroys such a structure or does not touch it. There is no partial slow-pathway injury.
* The accessory pathway's insertions are fixed points on the left or right annulus. The procedure teaches reading the CS sequence, not fine
  mapping along the annulus.
* The atria of the default heart are closed spheres with no holes. The flutter circuit's annulus and outer barrier are drawn as inert tissue
  (only while the slow engine runs), with a shortened action potential to make the wavelength fit, and the wave is pre-seeded rather than
  induced, so it is not started by pacing as in a patient. The barrier's one gap makes the rest of the atria a dead end, so the wave depends on the
  ring around the annulus and a complete line across the ring stops it.
* The fast engine has no functional re-entry, and refractoriness there is one number per chamber. So the atrial and ventricular ERPs you
  measure are those numbers, independent of the drive cycle.
* The CS sleeve joins the right atrium by a link added for the EP lab, since the model's ostium stops short of the atrial wall.
