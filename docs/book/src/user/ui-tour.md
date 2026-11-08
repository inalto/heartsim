# A tour of the window

The window has a one-line **menu bar** at the top: the program name and version with the name of the open page on the left,
and on the right the **☰ menu**, which lists the eleven pages (the open one is marked and each shows its number-key
shortcut), then *Export / Import*, *Settings* and *About*, and at the bottom **Theme**: system, light or dark. Below it, on the pages that run the simulation, is the **rhythm bar**: the rhythm
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

**Filter and artefacts.** The second row of controls above the strip (and the 12-lead page) changes how the trace is *drawn*:

* *Filter*: **Off**, **Diagnostic 0.05–150 Hz** or **Monitor 0.5–40 Hz**. The diagnostic band is what a 12-lead ECG machine
  records (the 0.05 Hz corner leaves the ST segment undistorted); the monitor band is what a bedside monitor shows (steadier
  baseline, softer QRS peaks, but ST segments and notches are distorted, which is why a monitor trace must not be used to
  judge ST changes).
* *Artefacts*: **Baseline wander**, **Respiration** (a baseline swing and a small change of the QRS height), **Mains hum**
  (50 or 60 Hz, your choice) and **Muscle** tremor, with an *intensity* slider (1 is the reference amplitude). They are
  generated from a fixed seed, so the same settings always draw the same trace. Try the monitor filter on mains hum: it is only
  a few dB weaker, because a 40 Hz low-pass is not a notch filter.

This is a *display* stage only. The simulation, the measurements (rate, intervals, axis, wave letters), the calipers' snapping
and the exports of the clean signal are not filtered and do not change when you switch it. The filters are causal, so the trace
is a few milliseconds later than the unfiltered one. When a saved 12-lead PNG or an export shows the filtered trace, it says
which filter and artefacts were applied (see [Exporting](exports.md)).

**Pause and scrub.** Press *Pause*, then drag the *s back* slider or the strip itself (away from any caliper): the heart, the vector arrow and the
read-out all follow the cursor, up to 60 s back. *Run* continues from where the simulation stopped; time never jumps.

**Calipers.** Shift-click a strip to add a pair of vertical cursors: the strip shows the interval in ms and the rate it
corresponds to. Drag a line or the band between a pair to move it (it snaps to the nearby wave when released), click a line to
select it and use the arrow keys to nudge it by one sample (with shift, ten). *Calipers ✕* or Esc removes them all.

![LBBB with the heart-vector arrow, scrubbed to the middle of the QRS](../images/simulator-lbbb-vector.png)

The same views with a fibrillating heart: in [coarse VF](../images/simulator-vf.png) every region recycles on its own
schedule and no QRS is found, and in [flutter with 2:1 conduction](../images/simulator-flutter.png) the strip shows the
sawtooth while the read-out gives the atrial rate and the conduction ratio.

## 12-Lead ECG

![The clinical 3 × 4 layout with a lead II rhythm strip](../images/twelve-lead-clinical.png)

All twelve leads on one shared time base, in three layouts: 12 × 1 (one row per lead, best for comparing morphology),
6 × 2 (limb leads left, chest leads right) and the clinical 3 × 4 printout with a lead II rhythm strip. *Cabrera* reorders
the limb leads by their angle (aVL, I, −aVR, II, aVF, III). Calipers work in every cell. *Save PNG* writes the page as it is
on screen; the export window writes a printable PDF.

![The 6 × 2 layout showing an anterior STEMI](../images/twelve-lead-stemi.png)

## Heart Vector

![The vectorcardiogram of LBBB](../images/heart-vector-lbbb.png)

The cardiac dipole at the strip cursor and its loop over the last beat, in the frontal and horizontal planes.

## Axis Trainer and Axis Quiz

![Axis trainer, dialled to a left axis deviation](../images/axis-trainer.png)

Drag on the circle to rotate the heart's electrical axis and watch all six limb leads change; the right-hand panel names the
axis class and why. The [Axis Quiz](quizzes.md) hides the axis and asks you to find it.

## Electrodes

![The Electrodes tab](../images/electrodes.png)

Move the ten electrodes on the torso, try the preset misplacements (limb-cable reversals, chest leads one space too high) and
see which leads change; the detector explains what was misplaced.

## Rhythm Quiz, Interval Drill, Tutorials and EP Lab

The [Rhythm Quiz](quizzes.md#rhythm-quiz) shows a strip and asks which rhythm it is; the [Interval Drill](quizzes.md#interval-drill)
asks you to measure PR, QRS and QT with the calipers and grades the answer. The [Tutorials](tutorials.md) tab holds the ten
guided lessons and the three patient cases. The [EP Lab](ep-lab.md) is the catheter laboratory: electrograms, programmed
stimulation and ablation.

![The Tutorials tab](../images/tutorials.png)

## Settings and About

*Settings* in the Menu opens the preferences: language, high-contrast theme and reduced motion. Changes apply at once
and are remembered on this computer.

The **About** button next to it shows the version and the [medical disclaimer](disclaimer.md), which is also shown once, as a
dialog that must be acknowledged, the first time the program runs.

### Monitor beep

Desktop builds made with the `audio` feature add a **Monitor beep** switch and a volume slider to the Settings window. It is
**off by default**; the browser version and builds without the feature have no sound and do not show the switch.

- It plays a short 1 kHz tone, synthesised in the program, in the frame in which the ventricle activates. It is a teaching
  imitation of a bedside monitor's QRS tone, not a reproduction of any product. The latency is up to one frame plus the audio
  buffer, so it is not sample-accurate with the strip or the 3-D map.
- To keep fast rhythms and fast-forward from turning into a buzz, at most one beep is sounded per 180 ms of *simulated* time.
- The sound never affects the simulation. Without an audio device the program runs as usual and the beep stays silent (the
  Settings window says so).

## Help

A one-page summary of the tabs and the axis rules, inside the app.
