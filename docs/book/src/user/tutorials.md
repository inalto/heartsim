# Tutorials

Tutorials are guided lessons: a sequence of short pages, each with text and figures, that can drive the simulator for you
(load a rhythm, set a parameter, pause at the moment that matters, point the camera at a region, highlight a tool) and can end
with a knowledge check.

* **Next / Previous** move between pages; the actions of a page run when you enter it, so going back re-creates the
  situation the text describes.
* **Self-study mode** asks you to answer each page's question before moving on; **tutor mode** lets a teacher jump freely
  while projecting the screen.
* Progress is saved per tutorial on your computer, so you can stop and continue later. Each card shows how far you got ("3 of 6 pages" and a progress bar, or *Completed* with a tick); its button says **Start**, **Continue** or **Start again**.

Tutorials are files under `assets/tutorials/<id>/`: Markdown text plus a list of actions, the same actions a scenario
timeline uses. A teacher can write one without programming; the format is described in the tutorial files themselves.

Good first tutorials:

1. **Reading the axis** — the hexaxial reference system, then the [Axis Trainer](ui-tour.md#axis-trainer-and-axis-quiz).
2. **AV blocks** — first degree to complete block, with the PR intervals measured by calipers.
3. **Bundle branch blocks** — why LBBB and RBBB widen the QRS differently, watched on the 3-D heart.

For the heart sounds there are two:

* **Heart sounds and the rhythm** (`11_heart_sounds`) opens with headphone advice and a **sound check**: a low tone
  (200 Hz) and a higher one (800 Hz). If you cannot hear the low one, your speakers are too small for heart sounds; the page
  says so and points you to headphones and to the heart-sound lane, where every sound is labelled in time with the ECG.
  Then: S1 with the QRS and S2 at the end of the T wave, the split and breathing (with a breath-hold), slow motion, a long PR
  softening S1, complete block, the reversed split of LBBB, atrial fibrillation, S3 and S4.
* **Murmurs and where to listen** (`12_murmurs`): systolic against diastolic, the ejection murmur of aortic stenosis, the
  innocent murmur, aortic regurgitation, mitral stenosis with its snap and rumble, and the five listening sites.

Each page sets up the rhythm, switches the sound and the lane on, chooses the site and chest piece, and ends with a question you
can answer by ear or by eye. Without a sound output the sound check is text only and everything else works from the lane. The
Sound Trainer and the Auscultation Quiz ([Learning auscultation](auscultation.md)) are where the listening is practised.
