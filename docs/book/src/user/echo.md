# The Echo page: probe, cross-section and simulated image

The Echo page puts a transducer on the chest of the figure. You slide it, tilt it, rock it and turn it. The 3-D view shows the
plane the beam cuts through the beating model heart. The panel on the right shows that plane in one of three ways: a grey
**simulated image** (the default), an **anatomical cross-section** in colours, or both at once. The heart moves with the same
timing as the ECG strip and the 3-D view, so what you see on the right is the beat you see on the strip.

The left column has four sections, chosen with the control **Probe | Image | Views | Measure** at its top. One section is shown at a
time: **Probe** for the windows, the probe's angles and depth, **Image** for the settings of the grey image, **Views** for
the standard views, and **Measure** for calipers, M-mode and tissue Doppler, which are [on their own page](echo-measuring.md).

![The Echo page from the apical window: the grey simulated image](../images/echo-apical.png)

> **For education only. A simulated image, not a recording.** The grey image comes from a simple line model of the model's
> tissues. It is not a picture of a real patient, and nothing on this page was compared with a real echocardiogram. No clinician or
> sonographer has reviewed how it looks. The cross-section is a map of the tissues along the plane. The valve leaflets, papillary
> muscles, moderator band and pericardium are **designed**, not taken from data. Treat both views as a schematic.

## The probe

The probe sits on the skin of the figure. Its beam starts at the skin point and fans into the body. Four buttons put the probe at
the starting pose of each window:

* **Parasternal**: beside the sternum. The lung covers the heart along this edge, so a designed lung-edge retraction (the cardiac
  notch) opens the window. The retraction narrows with inspiration: about 0.4 of the window is hidden at full inspiration and
  about 0.16 in expiration.
* **Apical**: over the apex of the heart, in the rib space whose view is the clearest. About 0.1 of the window is hidden (about
  0.2 at full inspiration); the ribs shadow the edges of the sector.
* **Subcostal**: below the ribs, under the breastbone's lower end. Little is hidden (about 0.06).
* **Suprasternal**: at the base of the neck, above the breastbone. About half is hidden.

These shares were measured once on the default anatomy pack. They are a check on the geometry, not a test of the clinical method.

![The Echo page from the parasternal window: the grey simulated image](../images/echo-parasternal.png)

Move the probe in three ways:

* **Slide** it over the skin. Drag the probe on the chest, or click the skin.
* **Tilt** it. The beam fans out of the image plane, toward or away from the side of the index marker.
* **Rock** it. The beam swings within the image plane, toward or away from the index marker.
* **Rotate** it. The probe turns about its beam. The index marker says which way the image is turned: 12 o'clock is toward the
  head, 3 o'clock toward the patient's left.

Drag the three handles on the probe, or use the sliders in the **Probe** section: **Tilt**, **Rock** and **Rotation**. **Depth**
runs from 4 to 30 cm (16 cm by default) and **Sector** from 30° to 90° (75° by default). **Reset**, on a row of its own under the
window buttons, returns the probe to the apical window, with the default depth and sector.

The drag hint over the 3-D view stays until you first move the probe. After that its text is behind **More** in the Probe section.

Keyboard: the arrows slide the probe, Shift with up and down tilts it, Shift with left and right rocks it, and `[` and `]` rotate
it. The pose is kept for the session and is not saved.

Where the beam is too far from the skin's normal (more than 60°), the probe has no contact and the page says **No contact**
over an empty sector. The page then shows no tissue and no image.

## The display: Image, Anatomy or Both

Three buttons above the right panel choose what it shows:

* **Image** (the default): the grey simulated image of the sector. Read it as described below.
* **Anatomy**: the coloured cross-section, one colour per tissue. Use it to find out what a bright or dark area is.
* **Both**: the tissue colours laid over the grey image at partial opacity, with the legend. Hatched, dimmed areas lie behind a
  rib or the lung.

The image keeps the same rectangle in all three modes. The read-out line and the legend sit under it, in the same place. In
*Image* the line says where the tissue colours and their legend are (in *Anatomy* and *Both*). The one-line description above
the image ends in **More**, which gives the full note and the disclaimer.

The display mode is kept for the session and is not saved.

## Reading the image

The image is a sector, drawn the way an echo machine draws it: the apex at the top, depth going down, each beam a line from the
apex. Each point is as bright as the echo that the model sends back from there. The model gives each tissue a reflection, an
attenuation and a grain. The brightness that is left after the sound has travelled down and back is what you see.

**What is bright and what is dark**

* **Blood** is dark. It hardly reflects sound, so the chambers and the great vessels are black or nearly so.
* **Myocardium** (the heart muscle) is mid-grey and speckled.
* **Valve leaflets** and the **pericardium** are brighter than the muscle, and they are smoother. Both are **designed** values.
* **Vessel walls** are a little brighter than the muscle.
* **Liver** and the **chest wall** are darker than the muscle.
* **Bone** (the ribs and the sternum) is bright at its surface.
* **Lung** is bright at its surface and dark beyond it.

**Speckle**

The grain of the muscle is speckle: the pattern made by many tiny reflectors in the tissue. In the model the pattern belongs to the
tissue. When the wall moves, the grain moves with it, as it does in a real image. The grain is a pattern made by the model, not a
sign of disease. It is coarser at depth than the grain of a real image.

**Shadows**

A rib or the bone of the sternum reflects most of the sound at its surface. The area below it is dark: a shadow. The shadow
falls on the edges of the sector when the probe sits in an intercostal space.

**Lung and reverberation**

The lung reflects almost all of the sound at the pleural edge. That edge is bright. Below it, the sound cannot reach the heart, so the
area is dark. In the model the sound also bounces between the probe face and the pleural edge. Each bounce gives a fainter bright
line at a multiple of the first depth. These repeat lines are **designed**: they are not measured, and they are not the same
in every real image.

**Enhancement behind blood**

Blood takes little energy from the sound. So the tissue behind a blood-filled chamber is brighter than the same tissue behind
the muscle. The model has no special rule for this. It follows from the low attenuation of blood.

**Depth and angle**

The sound loses strength as it goes down. Without compensation the image fades with depth. An interface that faces the beam
reflects more than one that is at a slant to it, so the edges of a valve or a wall look brighter when the beam meets them square.

**The controls**

The **Image** section of the left column holds the controls. Depth and sector are in the **Probe** section. The defaults are the
ones a learner starts from.

* **Frequency** (2 to 5 MHz, 2.5 by default). A higher frequency gives finer grain and better detail, but the sound fades sooner, so
  the deep part of the image goes dark.
* **Gain** (−30 to +30 dB, 0 by default). Raises or lowers the brightness of the whole image.
* **Dynamic range** (30 to 90 dB, 60 by default). The span of echo strength that fits between black and white. A smaller range
  gives more contrast and less grey.
* **Focus** (80 mm by default, shown as **F** on the depth scale). The depth at which the beam is narrowest. The image is sharpest
  sideways at that depth.
* **Time-gain compensation.** A collapsed section. The **Automatic compensation** box, on by default, raises the gain with depth by
  what the tissue absorbs on average. Off, the image fades with depth unless the eight sliders raise it. The sliders run from the skin
  (1) to the bottom of the image (8), each by up to ±20 dB.
* **Reset image settings** returns frequency, gain, dynamic range, focus, compensation, depth and sector to their defaults. The probe's
  position and the display mode stay.

The image settings are kept for the session and are not saved.

**What the image is not**

* It has no side lobes, no refraction, no mirror artefacts, no harmonic imaging and no near-field clutter. A real image may show these.
* There is no contrast agent and no blood Doppler of any kind. Tissue Doppler is offered only at the mitral and tricuspid annuli.
* The calipers, M-mode and tissue Doppler of the **Measure** section measure the model. They are described on [Measuring on the echo](echo-measuring.md).
* A bright or dark area is not a diagnosis. The image shows the model's own tissues and how they reflect sound. It shows no disease,
  clot or effusion.

## The cross-section (Anatomy)

The cross-section draws the sector as the probe would scan it: the apex at the top, the index marker on the screen's right,
and a depth scale in centimetres. Each point is coloured by the tissue there:

* the four chambers' **myocardium** and **blood**, the aortic and pulmonary walls and lumens, and the blood of the thoracic vessels;
* **valve leaflets**, **papillary muscles** and the **pericardium** (designed);
* **bone** (ribs, sternum, spine), the **lung**, the **liver** and other soft tissue.

A legend names each tissue. The colours are chosen to stay apart in both themes and for colour-blind viewers. Points behind a rib
or the lung are dimmed and hatched: those areas are hidden, and what you see there is not the image of the heart.

Hover over the cross-section to read the tissue and its depth in centimetres. Behind bone or lung, the read-out says so.

![The same plane in Anatomy: each tissue in its colour, apical window](../images/echo-anatomy-apical.png)

With **Cut the heart at the image plane** on, the half of the heart on the near side of the plane is removed, so the cut in 3-D
matches the cross-section. With **Wall motion** on (the default), the walls move as on the Simulator page. Turning Wall motion off
keeps the walls still; the leaflets still open and close with the beat.

**Reduced motion** in Settings holds the whole frame still. The walls, the leaflets, the grey image and the cross-section no longer
follow the beat. Moving the probe or changing a setting redraws the page.

![Both: the tissue colours over the grey image, apical window](../images/echo-both-apical.png)

## What is designed and what is missing

Most of the anatomy is the model's own: the chambers, the great vessels and the bones come from the open meshes and the open
torso used for the pack. The rest is designed. The page labels each designed part "(designed)". The values are listed with
their sources in `docs/anatomy-pack.md` and the code.

* **Valve leaflets**, the papillary muscles, the moderator band and the pericardium are drawn from measured dimensions where a
  source was read, and designed where none was found. They are not seen in the source data.
* **The lung-edge retraction** that opens the parasternal window is designed. The model does not show a real notch.
* **The right ventricle** has no venous lumens in the heart box, and the closed tricuspid valve has a flat shape.

**The grey image.** The acoustic values are in `assets/echo/acoustic.ron`, one row per tissue, with the status of each value.
The impedances and most of the attenuations are read from the source that the research note cites (`docs/research/echo-simulator.md`,
section 3.2). The myocardial attenuation and the aperture are marked unverified. The backscatter levels, the scatterer density and
the specularity of every tissue are **designed**, as are the leaflet and pericardium impedance, the reverberation, and the treatment
of the lung as air. None of the values was fitted to an echo recording.

The **Views** section holds the standard views, which score the probe against each one. They are described on [their own page](echo-views.md).

The Echo page does **not** show:

* blood Doppler of any kind (spectral, colour or continuous wave);
* the blood flow, the pressures or any measurement of a patient.

The limits are listed again in [Known limitations and open questions](../known-issues.md).
