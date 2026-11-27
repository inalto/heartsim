# Tutorials

Tutorials are guided lessons: a sequence of short pages, each with text and figures, that can drive the simulator for you
(load a rhythm, set a parameter, pause at the moment that matters, point the camera at a region, highlight a tool) and can end
with a knowledge check.

* **Next / Previous** move between pages; the actions of a page run when you enter it, so going back re-creates the
  situation the text describes.
* **Self-study mode** asks you to answer each page's question before moving on; **tutor mode** lets a teacher jump freely
  while projecting the screen.
* Progress is saved per tutorial on your computer, so you can stop and continue later. Each card shows how far you got ("3 of 6 pages" and a progress bar, or *Completed* with a tick); its button says **Start**, **Continue** or **Start again**. The list has one accent-coloured button: **Continue** on the tutorial
you opened last if it is not finished, otherwise **Start** on the first one not completed. The other buttons are plain.

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

## Patient cases

A patient case is a short story told in stages. Each stage gives the history and the observations, sets the simulator up (a
rhythm, a drug, a shock), and asks multiple-choice questions with an explanation; a score closes the case. The cases are for
education only and **no clinician has reviewed them** (see the [disclaimer](disclaimer.md) and the
[known issues](../known-issues.md)). Where the simulator cannot show something (a drug it does not have, a pacemaker that cannot
be reprogrammed, an arrest that starts from an infarct), the text says so and tells it instead. A simulated shock is drawn from a
seeded random generator and can fail; every case that shocks loads the rhythm that follows on the next page, so the story holds.

The nine cases, in the order of the list, with no spoilers:

* **A fast, wide rhythm in an older man** — an older man with an old heart attack and sudden palpitations has a regular rhythm with
  wide complexes.
* **Crushing pain, then a collapse** — a man in his sixties arrives with severe chest pain; read the first ECG and follow the
  heart to the next day.
* **A pulse stuck at 150** — a woman short of breath has a regular narrow tachycardia that does not change with rest.
* **Chest pain on the golf course** — a middle-aged man with crushing chest pain: read the ST segments and see what
  reperfusion changes.
* **Dizzy weeks after a pacemaker** — a woman who depends on her pacemaker becomes dizzy.
* **A racing heart in a student** — a fit young man has sudden fast palpitations that stop abruptly.
* **Sharp chest pain after a cold** — a young woman with chest pain has ST elevation on her ECG.
* **A faint after three days of vomiting** — an older woman with a stomach bug and a slow pulse faints on the ward.
* **Collapse in the supermarket** — an older woman faints twice in a week; follow her ECG from a quiet baseline to the rhythm that explains it.

The files are `assets/cases/case_*.ron`; `heartsim-cli validate-cases assets/cases` checks them (see [the command line](cli.md)).
