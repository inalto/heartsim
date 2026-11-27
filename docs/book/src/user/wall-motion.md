# The beating heart (wall motion)

In the Simulator the 3-D heart moves. The walls contract and relax, the base of the heart moves toward the apex and back, the
left ventricle twists, and the atria fill and empty. The motion follows the same timing as the ECG strip, the colours on the
heart and the heart sounds, so a bundle-branch block that delays the left ventricle's contraction shows on the strip, in the
colours and in the movement at the same moment.

> **For education only, and a model.** The motion is a **kinematic model of wall motion**: a prescribed movement, timed by the
> simulation, with the numbers the published sources give where there are any. It is not electromechanics: there is no
> pressure, no blood flow and no Doppler. Nobody has compared it with a real echocardiogram, and no clinician has reviewed it.
> The read-outs are labelled *model* and are teaching values, not measurements of a patient.

## What moves, and why

Each region of the muscle starts to shorten a little after its electrical activation arrives, and keeps shortening for about
as long as its action potential lasts (about 90 % of it). The regions therefore contract in the order the impulse spreads,
and each one relaxes at its own time. The heart then behaves the way a normal beat does:

* **The valves set the phases.** The same closing and opening times that place the heart sounds bound the motion. The left
  ventricle holds its volume while the muscle tightens against the closed valves (isovolumic contraction, about 40 ms after the
  QRS starts, and lasts about 40 ms), empties through the aortic valve (ejection), holds again while it relaxes with both valves
  shut (isovolumic relaxation), and then fills: first fast, then slowly, then with the atrial push at the end.
* **The base descends toward the apex** as the ventricle shortens. In a normal beat the mitral annulus moves about 14 mm toward
  the apex in the model, and the tricuspid annulus about 24 mm.
* **The walls thicken** as they shorten: the inner surface moves in further than the outer one, so the wall gets thicker.
* **The left ventricle twists.** Seen from the apex, the apex turns one way and the base the other (the apex about −12°, the
  base about +5° at end-systole in the model), which is why the two ends of the ventricle do not move in step.
* **The atria** fill and empty into the ventricles, and contract at the end of diastole when their activity is organised (see the
  rhythms below).
* **The great vessels** move with the valve rings near their roots and are still further away.
* **A small respiratory shift.** At peak inspiration the whole heart moves a few millimetres downward, following the breathing
  rate of the simulated patient. This is a designed touch, not a measured effect.

In a sinus beat the read-outs of the model are about **LVEF 62 %**, a left-ventricular end-diastolic volume of about 130 mL and
an end-systolic volume of about 49 mL, **MAPSE about 14 mm** (the base of the left ventricle, measured at the mitral annulus)
and **TAPSE about 24 mm** (the same for the right ventricle, at the tricuspid annulus).

## Using it

The controls sit in the **Wall motion** section of the Simulator's left panel. That section is closed at first; open it by its heading.

* **Wall motion** (a switch, on by default). Off, the heart keeps its resting shape and the colours and the ECG work as before.
  The switch is greyed out while **Reduced motion** is on in Settings, because the heart then does not move at all. Your choice
  is remembered.
* **Exaggeration** (a slider, 0.5× to 1.5×, 1× by default). It scales the movement for teaching: at 1.5× the contraction is
  easier to see, at 0.5× it is subtle. The read-outs are always computed at 1×, so the numbers do not change when you move the
  slider. It stops at 1.5×: at 2× the thin walls fold on themselves at end-systole.
* **Wall-motion read-out** (a card). It shows the **LVEF** of the current beat, the left ventricle's **end-diastolic** (EDV) and
  **end-systolic** (ESV) volumes in millilitres, and **MAPSE** and **TAPSE** in millimetres. Each value is tagged *model*. The
  card is shown with the switch off too, so you can read the numbers of a rhythm without the movement. The labels are in English
  and Italian.

Clicking the heart still stimulates or ablates the site on the heart's resting shape, so the spot you click is where the
anatomy says it is. The pacing-lead and catheter markers stay at their resting positions while the wall moves around them.

Scrubbing back with the *Scrub back* control shows the motion for up to about 6 s. Further back, the heart still moves but
without the regional detail of each segment.

## The tricuspid valve layer

The 3-D heart can show a tricuspid valve with three leaflets and their cords. Open *Anatomy and camera* in the left panel and
tick **Tricuspid valve** in the layers list. It is on when the program starts, and your choice is saved with your preferences.
A settings file saved by an earlier version has its valve layers switched on once; after that your own choice is kept.

The leaflets are one donor's valve fitted to this heart. The donor is Texas TriValve 1.1, a finite-element model of a healthy
human tricuspid valve (Mathur et al. 2022, CC BY 4.0). Its leaflets were scaled and placed on this heart's tricuspid ring. Its
cords start on the right ventricle's wall, because this heart has no papillary muscles on that side. It is a measured shape, not a
measurement of a patient's valve, and it has not been reviewed by a clinician.

* When it is shown, it replaces the flat tricuspid disc of the *Valve planes* layer, and the closure flash of the tricuspid
  valve is drawn on the leaflets.
* It is easiest to see when the right ventricle or the right atrium is cut or made translucent. A click passes through it.
* It follows the wall motion. With *Wall motion* off, or with Reduced motion on, it stays on the resting ring and still opens
  and shuts.
* It opens and shuts on the same timing as the heart sounds and the Echo page's leaflets. The source is a load sequence of seven
  shapes, not a timed recording, so the valve snaps the last small part of the way shut.
* The mitral, aortic and pulmonary valves have their own layers. See *The mitral, aortic and pulmonary valve layers* below.

## The mitral, aortic and pulmonary valve layers

The 3-D heart can also show a mitral, an aortic and a pulmonary valve. Open *Anatomy and camera* in the left panel and tick
**Mitral valve**, **Aortic valve** or **Pulmonary valve** in the layers list. Each is on when the program starts, and your choice is
saved with your preferences. A settings file from 0.25.0 or 0.26.0 still loads; its valve layers are switched on once.

These three valves are **designed shapes**, not measured ones. Heartsim generated each one by rule, from published dimensions, on
this heart's own valve rings. They are not a person's anatomy. They were not taken from any model, mesh or image, and **no
clinician has reviewed them**. No lesion is implied. The tricuspid valve stays the one measured valve.

* **Mitral valve.** An anterior leaflet and a posterior leaflet. The posterior leaflet has three scallops (P1, P2 and P3), and the
  leaflets meet at two commissures. Twelve chordae run to the two papillary muscles, six to each. The leaflet lengths are 25.1 mm
  (anterior) and 14.4 mm (posterior) (Tarkiainen 2016). The leaflets are designed to overlap by 8 mm when shut, but on this heart's
  ring they are too short for that: the overlap is about 5.7 mm at the centre of the leaflets and less toward the commissures. The
  valve still shuts with no gap.
* **Aortic valve.** Three cusps, named non-coronary, left coronary and right coronary. The pack has no coronary openings, so the
  names come from the neighbouring chambers. The cusp heights follow Izawa 2021, Ishida 2026 and De Kerchove 2018.
* **Pulmonary valve.** Three cusps, named by position: posterior, left anterior and right anterior (Lis 2023). Its height is set
  from Lis 2023, and the other values are taken from the aortic valve.
* **Timing.** Each valve has eight shapes, from open to shut. It opens and shuts on the same valve timing as the heart sounds and
  the Echo page.
* **Shape.** When a valve's layer is shown, it replaces that valve's flat disc of the *Valve planes* layer.
* **Motion.** It follows the wall motion. With *Wall motion* off, or with Reduced motion on, it stays on the resting ring and still
  opens and shuts.

The full list of sources and designed values is in the research note `docs/research/designed-valve-leaflets.md`.

### The lengthened pulmonary trunk

The dataset's pulmonary artery stopped about 9 mm above the pulmonary ring, too short for the pulmonary cusps, whose commissures sit
about 22 mm high. The default pack therefore continues the pulmonary trunk 20 mm beyond that end, as a straight tube. The tube
carries the stub's own end cross-section and is drawn in to keep 2 mm clear of the aorta. It is the only designed part of the
heart's drawn walls. The real trunk is longer and divides; the model stops at the designed end. The continuation is in the 3-D
heart and in the echo's label volume alike, and the lumen of the pulmonary artery grows from 3.6 mL to 11.8 mL.

## What each rhythm shows

* **Sinus rhythm.** The normal pattern described above.
* **Left bundle-branch block.** The septum is activated late, so it contracts early, before the lateral wall. The septum moves
  inward first and then pulls back as the lateral wall squeezes it: a small **septal flash**, the movement that echo teachers
  look for. The lateral wall contracts late.
* **Right bundle-branch block.** The free wall of the right ventricle contracts late, after the septum.
* **An early ventricular beat (a premature beat).** Too early to open the aortic valve, it does not eject: the left ventricle
  keeps its volume and the card shows a low EF. The beat after the pause is stronger, with a higher EF than the sinus beats.
* **Atrial fibrillation.** The atria do not contract, so the atrial push is lost. The ejection fraction changes from beat to
  beat with the length of the preceding interval: short intervals eject less, and the ejection fraction is different on each
  beat.
* **Complete AV block.** The atria contract after each P wave, whatever the ventricles do, so the atria and ventricles beat on
  their own rhythms. The ventricle's contraction still follows its own QRS.
* **Ischaemia (for example an anterior STEMI, the LAD territory).** The affected segments move much less, and the LVEF read-out
  falls: about 41 % in the anterior STEMI scenario. The rest of the wall moves as before, so the difference is regional.
* **Ventricular fibrillation and asystole.** There is no organised motion. In VF the walls show a faint, designed quiver, and the
  card shows "—" instead of the read-outs.

## What is designed

Some of the numbers are published, and the code says which. Many are not: they are choices made for this model, inside what
the sources describe in words. A clinician reviewing the model should look at these first. They are all in one parameter file.

* **Timing of the contraction:** the electromechanical delays (about 20 ms ventricular and 60 ms atrial), how the contraction
  rises and falls, and the share of the action potential it lasts (0.9).
* **The filling phases:** the speed of rapid filling, the slow diastasis filling and its share, the atrial share of the filling
  and the length of the atrial push, and the right-sided isovolumic times. The left isovolumic relaxation time was chosen inside
  the published range.
* **The ejection fraction from beat to beat:** how it falls with a short preceding interval, the boost after a premature beat,
  and the ceiling. The right-ventricular ejection fraction is designed too.
* **The shape of the movement:** how much the epicardium moves compared with the endocardium, the right ventricle's inward
  movement, how the motion tapers toward the base and fades at the apex, the heights at which the rotation starts and stops,
  how far the atria and the vessels move, and how the contraction is averaged between neighbouring parts of the muscle.
* **The quivers:** the small atrial movement in atrial fibrillation and the ventricular movement in VF.
* **The respiratory shift** of about 4 mm.

Published, and cited in the code and the research note: the ventricular isovolumic contraction time (about 40 ms), the LVEF
of a steady sinus beat (62 %), the TAPSE target (24 mm, from Lang 2015), the
apical and basal rotation angles (Helle-Valle 2005), the way the stroke volume depends on the preceding interval in
atrial fibrillation (Iwase 1988), and the rapid-filling deceleration time (Lang 2015). The MAPSE of 14 mm is a typical value
whose source the implementers could not fully confirm.

## What the model cannot show

* **Valves.** The valves are drawn as flat rings, except a valve whose layer is shown (see *The tricuspid valve layer* and *The
  mitral, aortic and pulmonary valve layers* above). The mitral, aortic and pulmonary shapes are designed, not measured. The
  mitral chordae end at the two papillary muscle sites, but the muscles themselves are not drawn. The pericardium is not shown.
* **Pressure, flow and Doppler.** Nothing is measured in blood. There is no pressure curve, no flow, no regurgitation jet and no
  Doppler velocity.
* **The right ventricle's volume.** The right ventricle has no closed cavity in the anatomy, so the model gives no RV volume and
  no RV ejection fraction. Only its longitudinal movement (TAPSE) is read out.
* **Enlarged or thickened chambers.** The shapes are the normal ones. Dilatation and hypertrophy are not modelled.
* **Ischaemia of the right ventricle.** The model reduces the inward movement of the left ventricle's ischaemic territory only.
* **Fine detail.** One small fold of the right-ventricular surface near the tricuspid annulus can appear briefly in LBBB. The thin
  parts (the atrial walls and the root of the vessels) can slip a few millimetres against their neighbours.
* **Real echo images.** This page is about the 3-D movement only. Ultrasound images, the probe and the standard views come in
  later phases of the echo simulator.
* **Real measurements.** Nothing here was checked against a real echo or a patient. Use the read-outs to learn what the
  numbers mean and how they change with the rhythm, not as values to quote.

The limits are listed again in [Known limitations and open questions](../known-issues.md).
