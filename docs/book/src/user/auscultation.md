# Learning auscultation

Two pages of the **Practise** group teach listening to the heart: the **Sound Trainer** and the **Auscultation Quiz**. They
follow what the teaching studies found to work: listening to the same finding many times (repetition took students from about
20 % to 85 % in one study, unsupervised as well as supervised), a trace with a moving cursor beside the sound, and testing
yourself on what you hear before naming the diagnosis. The tutorials *Heart sounds and the rhythm* and *Murmurs and where to
listen* lead into both ([Tutorials](tutorials.md)).

Every sound is a **model** computed from the simulated beats, not a recording. Use headphones: laptop and phone speakers
reproduce little below 150–200 Hz, where most heart sounds lie.

## Sound Trainer

![Sound Trainer](../images/sound-trainer.png)

On the left, the **findings**: S1 and S2; the physiological, wide, fixed and paradoxical splits; a soft S1 (long PR) and a
variable S1 (complete block); S3, S4 and the summation gallop; the ejection click, the mid-systolic click of prolapse and the
opening snap; the murmurs of aortic stenosis and regurgitation, mitral regurgitation and stenosis, tricuspid regurgitation,
pulmonary stenosis, ventricular septal defect and the innocent murmur; the pericardial rub. Each one is a short **clip** of a
scenario, heard at the site where that finding is best heard: a whole number of cardiac cycles, cut at a quiet point of
diastole, so it loops without a click (an irregular rhythm loops 9 s with a short fade instead).

* **Play / Stop** loops the clip. The **listen counter** adds one each time a loop has been heard to its end (a loop you stop
  half way does not count), per finding and in total. The counts are kept between sessions.
* **Half speed** plays a slowed version: the gaps between the sounds double, each sound keeps its pitch. It is labelled
  *altered sound*, on the button and on the lane, because it is an aid, not the real timing.
* **Show the trace** draws the clip: lead II above the labelled phonocardiogram, with a playhead. Hide it to listen first and
  check afterwards. Without a sound output the trace cannot be hidden, and the page says why.
* The **site and chest piece** and the **breathing** (breathe normally, hold at end-expiration, hold at peak inspiration) can
  be changed; the clip is recorded again and continues at the same point of the cycle.
* **Save WAV** (desktop) writes one loop as a WAV file ([Exporting](exports.md#heart-sounds-as-wav)).

The **contrast pairs** are the confusions the teaching literature reports: an innocent murmur against aortic stenosis, an
ejection against a regurgitant systolic murmur, a split S2 against S2 with an opening snap against S2 with an S3, and an S4
against a split S1. **Switch** (or the **S** key) goes to the other sound **at the same point of the cardiac cycle**, so the
difference is heard where it is.

## Real recordings

Beside six of the findings (S1 and S2, the innocent murmur, the mid-systolic click, aortic stenosis, pulmonary stenosis and
the ventricular septal defect) the trainer offers a **Real recording** button: a real patient's sound, played in a loop. It is
a recording and not the heartsim model, and the page says so. The sources give no ECG and no labels, so none is drawn; the
site, chest piece, breathing, half speed and the trace do not apply to it. Plays of a recording are **not** counted as
listens, because the counter measures repetition of the model's finding. A recording can only be heard: without a sound
output the button is off and says why. The credit and licence stay on the page while a recording is selected.

Credits (all clips converted to 4 kHz mono, and trimmed where noted, by the heartsim authors; details in `ASSETS-LICENSES.md`):

* Normal heart sounds: James Heilman, MD, Wikimedia Commons (`HROgg.ogg`), CC0 1.0.
* Innocent Still's murmur (two recordings), mitral valve prolapse with clicks, aortic stenosis, pulmonary stenosis and
  ventricular septal defect, all from children: Kudriavtsev V, Polyshchuk V, Roy D (2007), "Heart energy signature
  spectrogram for cardiovascular diagnosis", *BioMedical Engineering OnLine* 6:16, doi:10.1186/1475-925X-6-16, licensed
  [CC BY 2.0](https://creativecommons.org/licenses/by/2.0/). Recorded at the Department of Pediatric Cardiology, IWK Center,
  Dalhousie University. Paediatric hearts beat faster than an adult's, so the timing differs from the model.

## Auscultation Quiz

![Auscultation quiz](../images/auscultation-quiz.png)

Press **New question**: a hidden case from the auscultation scenarios (valve lesions, gallops, the fixed split) plays as a loop.
The trace and the name stay hidden.

1. **What do you hear?** Tick the features: an extra sound before S1 or after S2, a split S2 in both phases of the breath, a
   click, a systolic or a diastolic murmur, and where the murmur lies (early, mid, late, throughout). You may move the
   stethoscope between the sites. Press **Done: choose the diagnosis**.
2. **What is it?** Four diagnoses: the case and the cases it is most often confused with.
3. **Feedback**: the trace with its labels and the ECG above it, each feature marked right or missed against the case's
   expected findings (a physiological split is not scored, since it is heard only in inspiration), the teaching text, and
   **Open in simulator**, which loads the case at the site you were listening at.

Features and diagnoses are scored separately ("Features 31/40 · diagnoses 3/4") and kept between sessions. Half speed is
offered only in the feedback step: slowing the sound down changes what is being tested. Without a sound output the trace is
shown from the start (the name stays hidden) and you answer from the labelled phonocardiogram.

## Breath-hold and small speakers

The **Breathing** choice of the Display popup (and of the Sound Trainer) holds the breath at end-expiration or at peak
inspiration. The split of S2 then stays the same beat after beat, and so does a sinus arrhythmia's RR interval, until you
choose *Breathing normally* again. It is a real change of the simulation, written as a `BreathHold` action in the scenario
timeline language, so a tutorial or a scenario can use it too. Other manoeuvres (Valsalva, squatting, handgrip) change venous
return and afterload, which this simulator does not model, so they are not offered.

**Small-speaker mode** (Settings → Heart sounds, off by default) multiplies the frequency of every heart sound by 3 in what you
hear, so S3, S4 and the mitral rumble come within reach of laptop and phone speakers. It changes how the sounds sound and has no
proven teaching effect; the lane keeps drawing the true signal and shows *Altered sound: small-speaker mode*. Prefer
headphones.
