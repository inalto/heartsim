# A tour of the window

The window has a row of tabs at the top. Below it, on the pages that run the simulation, is the **rhythm bar**: the rhythm
picker, *Restart*, *Run / Pause*, and sliders for the sinus rate, AV block, Mobitz II ratio, QRS axis shift, QRS width and
simulation speed, plus the *Pacemaker* window. The **Export / Import** button at the right of the tab row opens the
[export window](exports.md).

## Simulator

![Simulator tab with sinus rhythm](../images/simulator-sinus.png)

* **Centre: the 3-D heart.** Drag to rotate, scroll to zoom. The colour is the membrane voltage of each region (blue at
  rest, red when depolarised); the panel on the left switches to *activation time* or *APD* maps, changes the colour range
  and offers a colour-blind-safe palette. *Click on the heart* stimulates there; shift-click ablates.
* **Left: layers.** Show, hide or fade each chamber, the great vessels, the valve rings and the torso. The heart-vector
  arrow and its loop can be overlaid; *see-through* fades the walls so the arrow stays visible.
* **Right: the frontal QRS axis** measured from the strip, the net QRS polarity in each limb lead, and the interval
  read-out (HR, RR, rhythm regularity, PR, QRS, QT, QTc).
* **Bottom: the strip.** Pick the lead with the buttons above it. Paper speed (12.5–200 mm/s), gain (5/10/20 mm/mV),
  scrolling paper or a monitor-style sweep bar, monitor colours, grid, wave letters (P, R, T) and PR/RR brackets, and the
  screen calibration in points per millimetre (hold a ruler to the screen and adjust it until 10 small squares measure 1 cm).

**Pause and scrub.** Press *Pause*, then drag the *s back* slider or the strip itself (away from any caliper): the heart, the vector arrow and the
read-out all follow the cursor, up to 60 s back. *Run* continues from where the simulation stopped; time never jumps.

**Calipers.** Shift-click a strip to add a pair of vertical cursors: the strip shows the interval in ms and the rate it
corresponds to. Drag a line or the band between a pair to move it (it snaps to the nearby wave when released), click a line to
select it and use the arrow keys to nudge it by one sample (with shift, ten). *Calipers ✕* or Esc removes them all.

![LBBB with the heart-vector arrow, scrubbed to the middle of the QRS](../images/simulator-lbbb-vector.png)

## 12-Lead ECG

![The clinical 3 × 4 layout with a lead II rhythm strip](../images/app-12lead-clinical.png)

All twelve leads on one shared time base, in three layouts: 12 × 1 (one row per lead, best for comparing morphology),
6 × 2 (limb leads left, chest leads right) and the clinical 3 × 4 printout with a lead II rhythm strip. *Cabrera* reorders
the limb leads by their angle (aVL, I, −aVR, II, aVF, III). Calipers work in every cell. *Save PNG* writes the page as it is
on screen; the export window writes a printable PDF.

![The 12 × 1 layout](../images/app-12lead-12x1.png)

## Heart Vector

![The vectorcardiogram of LBBB](../images/heart-vector-lbbb.png)

The cardiac dipole at the strip cursor and its loop over the last beat, in the frontal and horizontal planes.

## Axis Trainer and Axis Quiz

![Axis trainer](../images/app-axis-trainer.png)

Drag on the circle to rotate the heart's electrical axis and watch all six limb leads change; the right-hand panel names the
axis class and why. The [Axis Quiz](quizzes.md) hides the axis and asks you to find it.

## Electrodes

Move the ten electrodes on the torso, try the preset misplacements (limb-cable reversals, chest leads one space too high) and
see which leads change; the detector explains what was misplaced.

## Help

A one-page summary of the tabs and the axis rules, inside the app.
