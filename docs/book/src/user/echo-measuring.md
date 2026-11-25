# Measuring on the echo

The **Measure** section of the Echo page's left column puts calipers on the simulated echo and reads what they cover. It has three
modes, chosen with the control at its top: **2-D**, **M-mode** and **Tissue Doppler**. Every number is a measurement of the model.
Beside each named measurement, the page shows the model's own value for the same place and instant, with the difference, so the
method can be learned against an answer that is known. Nothing is graded here and no normal range is given: the
[echo calculators](echo-calculators.md) hold the criteria, each with its source.

> **For education only. Measurements of a model.** No clinician or sonographer has reviewed the measurements or the conventions
> they follow. The M-mode and Doppler traces come from the same motion and the same grey image as the sector. Nothing is drawn from
> a recording or from a template, except the thin band round the Doppler curve, which is designed and labelled as such. This page
> teaches the method on the model. It does not teach hand skill on a patient.

![M-mode, frozen: the cursor on the sector, two depth calipers on the trace and the ECG lead under it](../images/echo-mmode-lvid.png)

The left column has four sections, **Probe | Image | Views | Measure**. One is shown at a time.

## Freezing, sweep speed and scrub

* **Freeze** holds the sector and the trace still, so that calipers can be put on them. **Live** lets them run again. Clicking a
  trace freezes it too. While the sector is frozen, moving the probe changes nothing.
* **Sweep speed** is 25, 50 or 100 mm/s. The label says it is nominal, on a 96 dpi screen. 100 mm/s is the usual speed for measuring
  times, and the guideline gives rapid sweep speeds of 100 to 200 mm/s for time measurements (Mitchell et al. 2019, p. 18).
* **Scrub** moves back through the trace that has been recorded, up to 12 s. The readout gives the time back, for example
  "0.0 s back" when the trace is at its newest.

## M-mode: where to put the cursor

M-mode records one scan line of the sector over time. Depth runs down the trace, time runs to the right, and the ECG lead under it
is on the same clock. Drag on the sector to move the yellow **cursor line**. The readout gives its angle from the central beam (for
example "Cursor -14.0° from the central beam" in the picture above). The trace runs to its right edge as it fills.

Three buttons aim the cursor for you:

* **Mitral tips**: just beyond the tips of the mitral leaflets, where the guideline puts the cursor for LV dimensions in the
  parasternal long axis (p. 31).
* **Tricuspid annulus**: along the lateral tricuspid annulus, for TAPSE (p. 33). The tutorial uses it from the apical four-chamber
  view.
* **Mitral annulus**: along the mitral annulus, the line for MAPSE.

The cursor should be as parallel as possible to the motion you measure. A cursor at an angle to the motion sees only part of it:
the excursion you read is the true excursion times the cosine of the angle. The TAPSE page of tutorial 13 shows this with a question.

![M-mode, frozen, cursor at -27.5° from the central beam, aimed for the tricuspid annulus](../images/echo-mmode-tapse.png)

## Timing against the ECG

A new column of the M-mode trace is taken every 25 ms of simulated time. The time step does not depend on the speed of the screen:
the column for a given instant is the same however often the page redraws. A time read from the trace is good to about one column,
25 ms. The trace is sampled from the same beat as the ECG strip, so a line at the same instant on both is the same beat. If the page
cannot keep up, it fills the missing columns from their neighbours or leaves a gap; see [the known limitations](../known-issues.md).

On a slow computer, switch on **Faster M-mode trace (echo)** in Settings. The trace then takes a column every 50 ms, at half the
cost. The columns it takes are the same as before, each drawn twice as wide, and a time read from the trace is good to 50 ms. The
M-mode panel says when the switch is on. It does not change the tissue Doppler trace.

## Calipers

The calipers come in three groups. Choose the tool with the buttons under **Calipers**, then click on the sector or the trace. The
reading appears under the tools, or "Place the calipers to read a value" until you have placed them.

* **On the frozen sector.** **Distance** (mm): click two points. **Area** (cm²): click round an outline; the area closes on itself,
  and **Remove last point** takes back the last point.
* **On the M-mode trace.** **Depth** (mm): click two points of the trace, and the depth between them is read. **Time**: the time
  between two points. **Slope** (cm/s): how fast the depth changes between two points.
* **On the Tissue Doppler trace.** **Velocity** (cm/s): click the peak of a wave. **Time**: the time between two points.

**Snap to tissue boundaries** is off by default, so you place each point yourself. When it is on, a placed point moves to the
nearest boundary between tissues on its beam, within 3 mm.

**Clear calipers** removes the calipers on the page.

### Naming a measurement and the list

Once a caliper reads a value, pick a name from the list and press **Add to the list**. The names are:

LVIDd, LVIDs, IVSd, LVPWd, LA diameter, aortic root, TAPSE, MAPSE, e', a' and S'.

The list holds four measurements. Remove one with **Remove** to add another. Measurements are not saved: they are gone when the
session ends.

## Reference feedback

After you add a named measurement, the page gives the model's value for the same place and instant and your difference, in the form
"The model has X mm here. Your difference: Y mm." The difference is your value minus the model's value:

* For a distance, the model's value is read from the label line along your own line. It is the extent of the tissue that the middle
  of your caliper lies in. So the measure and the reference describe the same structure.
* For an annular velocity, it is the motion of the annulus along the same beam.
* With a cursor on an annulus, the page also gives the model's own reading for the beat along the annulus's own axis, and the angle
  between the cursor and that motion.

Where the model has no value for that place and instant, the page says "The model has no value for this place and instant." and
offers no reference.

## Tissue Doppler

Tissue Doppler records the velocity of an annulus, the ring where a valve joins the muscle. It does not record blood. Choose
**Tissue Doppler**, then choose one of three sites:

* the septal mitral annulus,
* the lateral mitral annulus,
* the lateral tricuspid annulus.

Click an annulus in the sector, or use a button, to place the **gate**. The gate snaps to the annulus only when it is within 8 mm of
it in the image plane. Elsewhere in the sector there is no annulus to gate, and the page says so: "No annulus there: tissue Doppler is
offered only at the mitral and tricuspid annuli." With the gate on an annulus, the panel names it, for example "Gate on the lateral mitral annulus".

![Tissue Doppler, frozen, on the lateral mitral annulus: S' e' and a' marked, the angle to the beam, and the designed band](../images/echo-tissue-doppler.png)

The trace is the velocity of that annulus point along the beam, in cm/s. It is **positive toward the probe**. For an apical probe the
annulus moves toward the apex in systole, so S' is positive, and e' and a' are negative, when the annulus moves back. The wave names
are on the trace:

* **S'** is the systolic wave, toward the probe.
* **e'** is the early diastolic wave, away from it.
* **a'** is the wave after the atrial contraction, also away from it. It is absent when there is no atrial contraction, as in atrial
  fibrillation.

**The angle is not corrected.** The page shows the angle between the beam and the motion at the S' peak, in the form "Angle between
the beam and the motion at the S' peak: 11°. The velocity is not corrected for it." The trace is labelled "angle 11°, not corrected".
The velocity you read is the true velocity along the motion times the cosine of this angle, so at 60 degrees the peak is half the true
one. The guideline asks for the beam to be as parallel as possible to the motion, and the page reports the angle so that its effect
can be seen.

### The designed band

A thin band is drawn round the curve. It is for the look only, because a real spectrum has a width. It is drawn from the curve itself
and labelled **designed band** on the trace. It carries no measurement.

### The model's values

The model's own velocities, in cm/s, in sinus rhythm at 60 beats a minute:

| Annulus | S' | e' | a' |
|---|---|---|---|
| Septal mitral | 14.0 | -8.6 | -3.3 |
| Lateral mitral | 9.6 | -8.8 | -3.3 |
| Lateral tricuspid | 8.6 | -12.8 | -7.5 |

The picture above runs at about 72 beats a minute (RR 832 to 834 ms on its strip), so its values differ from the table. These are
the model's numbers, not norms.

## The model's LV and annular values

At 60 beats a minute in sinus rhythm, the model gives an LV internal diameter of 45.0 mm in diastole and 36.2 mm in systole, a
fractional shortening of about 20 %, a TAPSE of about 23 mm and a MAPSE of about 14 mm. The model shortens little at this level, so its
fractional shortening is low for a clinical reader. Read these as the model's values to check your method against, not as a reference
for a patient.

## Send to calculator

Some measurements have a calculator input. For these, **Send to calculator** opens the [echo calculators](echo-calculators.md) with
the matching field filled with the value you measured. The field is marked **from the echo simulator** until you edit it. You type
the rest of the inputs. Only what the simulator measured is sent, under the name you gave it.

![The fractional shortening calculator, opened from the echo simulator: the end-diastolic diameter filled and marked](../images/echo-send-to-calculator.png)

| Measurement | Calculator inputs it fills |
|---|---|
| LVIDd | Fractional shortening; indexed LV diameters; LV mass (linear method) |
| LVIDs | Fractional shortening; indexed LV diameters |
| IVSd | LV mass (linear method) |
| LVPWd | LV mass (linear method) |
| Aortic root | Indexed aortic diameter, at the sinuses |
| TAPSE | RV systolic function; TAPSE/PASP coupling |
| S' (lateral tricuspid annulus only) | RV systolic function |
| e' (septal or lateral mitral annulus) | E/e' ratio and septal-to-lateral e' ratio, using the septal or the lateral e' input to match the annulus |
| LA diameter, MAPSE, a', mitral S' | None. **Send to calculator** is disabled. |

The simulator converts its millimetres to the centimetres the diameter inputs take. Velocities go in cm/s and TAPSE in mm.

## What is designed, and what is missing

**Designed:**

* the band round the Doppler curve, labelled as above;
* the snap distances, 8 mm for the gate and 3 mm for the calipers. They are the page's own choices; the guideline's sample volume is
  5 to 10 mm (p. 44);
* the grey image and the cross-section, as described on [the Echo page](echo.md).

**Missing, on purpose:**

* **No blood Doppler**, spectral or colour, and no continuous-wave Doppler. The model has no blood flow field, so a blood velocity
  could only be drawn from a template, and a control for it would suggest a measurement the model cannot make. Tissue Doppler is
  therefore offered only at the annuli, where the velocity is the time derivative of the model's own annular motion.
* **No angle correction** of the velocities.
* **No strain and no speckle tracking.** They would return the motion the model prescribes.
* **No report or export** of the measurements.

## The conventions

The page follows the conventions of the American Society of Echocardiography guideline below. Where it gives a page, the page is
cited in the text above.

> Mitchell C, Rahko PS, Blauwet LA, et al. Guidelines for performing a comprehensive transthoracic echocardiographic examination in
> adults: recommendations from the American Society of Echocardiography. *J Am Soc Echocardiogr* 2019;32:1–64.
> doi:10.1016/j.echo.2018.06.004

* **p. 18**: sweep speeds of 100 to 200 mm/s for measuring times.
* **p. 31**: LV dimensions in the parasternal long axis, just below the mitral leaflet tips, at end-diastole (the first frame after the
  mitral valve closes) and end-systole (the smallest cavity); the septum and the posterior wall; the left atrium, leading edge to
  leading edge.
* **p. 32**: the aortic root, leading edge to leading edge, at end-diastole.
* **p. 33**: TAPSE, the distance the leading edge of the lateral tricuspid annulus moves from end-diastole toward the apex; the cursor
  as parallel as possible to the annulus. The guideline also says that M-mode is not recommended for routine linear dimensions.
* **p. 44**: tissue Doppler, with s', e' and a' named; a sample volume of 5 to 10 mm; the beam as parallel as possible to the motion;
  a velocity scale of 25 cm/s or lower and a sweep of 100 mm/s.

The measurements follow these conventions as the page reads them. No clinician or sonographer has checked that reading.

For the other practice on the Echo page, see the [standard views](echo-views.md). The last three pages of tutorial 13, *Echo: the
standard views*, walk through LV diameters on an M-mode, TAPSE and tissue Doppler.
