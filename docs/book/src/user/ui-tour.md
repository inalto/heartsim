# A tour of the window

The window has a one-line **top bar**: the program name and version, then the eleven pages as **tabs** in three groups, and on
the right a button that cycles the theme and the **hamburger menu** (three lines). The groups are *Explore* (Simulator, 12-Lead ECG,
Heart Vector, Electrodes, EP Lab), *Practise* (Axis Trainer, Axis Quiz, Rhythm Quiz, Interval Drill) and *Learn* (Tutorials, Help).
The open page is the filled tab; hover a tab to see its number-key shortcut. When the window is too narrow for eleven tabs
the bar shows three group buttons instead (*Explore*, *Practise*, *Learn*): the group of the open page carries its name, and a
click lists the pages of that group. On a phone-narrow window the bar shows only the name of the open page, and the hamburger
menu is the way to the other pages. The hamburger menu always lists the eleven pages (the open one marked, each with its
number-key shortcut), then *Export / Import*, *Settings* and *About*, and at the bottom **Theme**: system, light or dark.

Below the bar, on the pages that run the simulation, is the **rhythm bar**. Its one row holds the rhythm picker, *Restart*,
*About this rhythm* (hover it for the description, click it for the full explanation), *Run / Pause* with the simulation speed, the *Pacemaker* and *Therapy*
windows, the re-entry presets and an **Adjust** button. *Adjust* opens a second row with the sinus rate, *AV block*,
*Mobitz II* (drop every n-th beat) and the QRS axis shift and width. The row is closed when you start, and the app remembers
whether you opened it. When one of its values differs from the rhythm's own the button says so (*Adjust · 2*, in colour; hover
it for the names), so a changed value is never hidden. On the Axis Trainer the axis shift is the point of the page, so the row
is always open there. The **Export / Import** window opens from the hamburger menu; see [Exporting](exports.md).

Every slider has its label before it and its unit after the value (*SA rate*, the slider, *72 bpm*).

## Simulator

![Simulator tab with sinus rhythm](../images/simulator-sinus.png)

* **Centre: the 3-D heart.** Drag to rotate, scroll to zoom. The colour is the membrane voltage of each region (blue at
  rest, red when depolarised); the panel on the left switches to *activation time* or *APD* maps, changes the colour range
  and offers a colour-blind-safe palette. *Click on the heart* stimulates there; shift-click ablates.
* **Left: layers.** Show, hide or fade each chamber, the great vessels, the valve rings and the torso. The heart-vector
  arrow and its loop can be overlaid; *see-through* fades the walls so the arrow stays visible.
* **Right: the frontal QRS axis** measured from the strip. The hexaxial circle shows the four axis classes as soft
  sectors; the sector under the pointer brightens and shows its name and range (normal −30° to +90° in green, left deviation −30° to −90° in amber at the upper
  right, right deviation +90° to +180° in rose at the lower left, extreme or indeterminate −90° to ±180° in grey-yellow
  at the upper left), degree ticks every 30° in small pills, the six limb leads on the ring and the measured axis as a
  thick blue arrow. Beside or under it, the *Measured axis* card shows the angle on a badge in the class colour, the
  *Interpretation* card names the class with its range, the net QRS polarity of each limb lead (as a signed glyph and
  a word, so it reads without colour) and a small torso schematic with the limb leads at their angles; the legend under
  the circle is the colour key. The *Measurements* card holds the interval read-out (HR, RR, rhythm regularity, PR, QRS,
  QT, QTc).
* **Bottom: the strip.** One row of controls sits above it: the lead buttons, paper speed (12.5–200 mm/s), gain (5/10/20 mm/mV),
  scrolling paper or a monitor-style sweep bar, *Clear calipers*, the *Scrub back* slider while paused, and a **Display** button.
  *Display* opens a popup with monitor colours, grid, wave letters (P, R, T), PR/RR brackets, the screen scale in points per
  millimetre (hold a ruler to the screen and adjust it until 10 small squares measure 1 cm), the filter and the artefacts.
  Labels never overlap: a wave letter or an interval that would touch the one before it is left out, and on the 12-lead page the
  3 × 4 grid shows them on its rhythm strip only, while the 12 × 1 and 6 × 2 pages add the RR and PR read-outs only when a beat is wide
  enough on screen (slow rates, or 50 mm/s paper).

  In the 3-D views a small **orientation gizmo** in the top-right corner turns with the camera: L / R is the patient's left and right,
  S / I superior (towards the head) and inferior, A / P anterior and posterior. Hover it for the key; it does not take clicks.

**Filter and artefacts.** The *Display* popup above the strip (and the 12-lead page) changes how the trace is *drawn*. While a filter or an
artefact is on, the *Display* button is filled and names it (*Display · Monitor 0.5–40 Hz, artefacts: 2*), because it changes what you see:

* *Filter*: **Off**, **Diagnostic 0.05–150 Hz** or **Monitor 0.5–40 Hz**. The diagnostic band is what a 12-lead ECG machine
  records (the 0.05 Hz corner leaves the ST segment undistorted); the monitor band is what a bedside monitor shows (steadier
  baseline, softer QRS peaks, but ST segments and notches are distorted, which is why a monitor trace must not be used to
  judge ST changes).
* *Artefacts*: **Baseline wander**, **Respiration** (a baseline swing and a small change of the QRS height), **Mains hum**
  (50 or 60 Hz, your choice) and **Muscle** tremor, with an *Intensity* slider (1 is the reference amplitude). They are
  generated from a fixed seed, so the same settings always draw the same trace. Try the monitor filter on mains hum: it is only
  a few dB weaker, because a 40 Hz low-pass is not a notch filter.

This is a *display* stage only. The simulation, the measurements (rate, intervals, axis, wave letters), the calipers' snapping
and the exports of the clean signal are not filtered and do not change when you switch it. The filters are causal, so the trace
is a few milliseconds later than the unfiltered one. When a saved 12-lead PNG or an export shows the filtered trace, it says
which filter and artefacts were applied (see [Exporting](exports.md)).

**Heart sounds.** *Heart sounds* in the *Display* popup (off by default, remembered) adds a lane under the strip, and under
the 12-lead page, with a **phonocardiogram**: a picture of the first and second heart sounds on the same time axis as the ECG.
It scrolls, sweeps, pauses and scrubs with the strip, and calipers placed on the strip reach down into it (they can be
dragged in either lane), so you can measure from the QRS to S1 or from the end of the T wave to S2.

* **S1** is the closure of the mitral (M1) and tricuspid (T1) valves, about 40 ms after the QRS starts. Its loudness follows the
  PR interval: loudest at 80–120 ms, soft when the PR is long (first-degree block), varying from beat to beat when P waves and QRS
  complexes are dissociated (complete block, VT).
* **S2** is the closure of the aortic (A2) and pulmonic (P2) valves near the end of the T wave. P2 follows A2 by about 10 ms in
  expiration and 40 ms at the peak of inspiration (the patient breathes 12 times a minute; the split comes and goes with it).
  RBBB delays P2 (a wide split, still wider on inspiration); LBBB and right-ventricular pacing delay A2 until it falls *after* P2
  (a paradoxical split, widest in expiration). When the two are more than 20 ms apart the lane labels them **A2** and **P2**,
  joined in the order they occur (**A2·P2**, or **P2·A2** for a paradoxical split) when they are too close to label apart.
* There are no heart sounds in ventricular fibrillation or asystole, and a ventricular beat that comes too early to eject has an
  S1 but no S2.

The labels come from the model's own schedule, not from finding peaks in the trace. The trace is a **model**, built from the
timing of the simulated beats and published timing rules; it is not a recording. Murmurs, S3, S4, clicks and listening sites
are not modelled yet. An imported recording has no heart-sound lane.

**Listening to the heart sounds.** Desktop builds made with the `audio` feature can play the lane through the sound card (the
browser version is silent for now). Tick *Play heart sounds* under *Heart sounds* in the *Display* popup, or in the *Heart
sounds* section of the Settings window; it is **off by default**, and switching it on also shows the lane (hiding the lane
afterwards leaves the sound on). What you hear is the trace you see, sample for sample, so a 30 ms split of S2 is heard as a
30 ms split.

* **Headphones.** Most of S1 and S2 lies below 150 Hz, where laptop speakers reproduce very little. With speakers you may hear
  only a faint click; with headphones you hear the low "lub-dub".
* **The views run a little behind.** Every sound card plays a little late, so while the sound is on the strip, the 12-lead
  page, the heart-sound lane and the 3-D heart are drawn about 0.1 s behind the simulation (40 ms of queue plus the *audio
  offset*), so that S1 is heard when the trace shows it. The strip eases into and out of this delay over 0.3 s; it is not a
  scrub, and calipers and measurements work as usual.
* **Slow motion.** Below 1× the sounds are re-timed: each sound keeps its own pitch and length and only the gaps between
  them stretch. At 0.25× a 40 ms A2–P2 split is heard as 160 ms, which is the easiest way to hear a split. The lane keeps
  drawing the true signal.
* **Above 1×** the heart sounds are muted, because S1 and S2 would run into each other; the monitor beep still sounds. The
  *Display* popup says so.
* **Pause.** Pausing fades the sound out. While paused, the *Play* button at the top right of the heart-sound lane plays the
  trace from the scrub cursor to the newest sample (the whole strip on screen when you have not scrubbed), or the span
  between the first pair of calipers when one is placed; a line moves across the strip and the lane with the sound. *Stop*,
  or *Run*, ends it.
* **Audio offset and the sync check.** The program cannot measure how late your sound card plays, so the Settings window has
  an *Audio offset* slider (0–300 ms, default 60 ms). Press *Check sync*: a click sounds once a second while a dot lights up
  beside the button. Move the slider until the click and the light come together; Bluetooth headphones may need 150 ms or
  more. *Stop the sync check* (or closing the window) ends it.
* The sound never affects the simulation: with the sound on or off, the same rhythm gives the same trace. Without a sound
  device the program runs as usual and says "No sound device could be opened".
* The sounds are a **model** computed from the simulated beats, not a recording of a patient.

**Pause and scrub.** Press *Pause*, then drag the *Scrub back* slider or the strip itself (away from any caliper): the heart, the vector arrow and the
read-out all follow the cursor, up to 60 s back. *Run* continues from where the simulation stopped; time never jumps.

**Calipers.** Shift-click a strip to add a pair of vertical cursors: the strip shows the interval in ms and the rate it
corresponds to. Drag a line or the band between a pair to move it (it snaps to the nearby wave when released), click a line to
select it and use the arrow keys to nudge it by one sample (with shift, ten). *Clear calipers* or Esc removes them all.

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

Drag on the circle to rotate the heart's electrical axis and watch all six limb leads change: the pointer becomes a hand
over the circle, the arrow brightens and a tooltip shows its angle. The cards beside the circle name the axis class, its
range and why (which leads are positive and negative). The [Axis Quiz](quizzes.md) hides the axis and asks you to find it;
its cards appear with the answer.

The whole window follows the same look, in a light and a dark variant chosen under **Theme** in the ☰ menu: rounded cards
with soft shadows, pill-shaped labels, a calm blue accent, and green or rose result boxes with a check or an exclamation
mark in the quizzes and drills. The ECG paper and the 3-D colour maps keep their clinical colours in both themes.

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
