# Extra heart sounds and murmurs

Fifteen scenarios add the sounds of the ventricles and the valves to an ordinary ECG: the third and fourth heart sounds, the
ejection and prolapse clicks, the opening snap of mitral stenosis, a pericardial rub and the classic murmurs. They are listed
under **Heart sounds and murmurs** at the bottom of the **Rhythm** list. Switch on *Heart sounds* in the *Display* popup to see
the phonocardiogram lane under the strip, and *Play heart sounds* to hear it (see [A tour of the window](ui-tour.md)); choose
where on the chest to listen, and bell or diaphragm, as described [below](#where-you-listen-sites-bell-and-diaphragm).

Two rhythm scenarios also carry sounds: **acute pericarditis** plays a friction rub and the **anterior STEMI** an S4. Their ECG
is the lesson, so they stay with the rhythms.

**What you hear is a model, not a recording.** The timing of every sound comes from the cited clinical texts and is tied to the
same engine events as the ECG, beat by beat. Pitch, loudness and the "texture" of each murmur are designed: published
frequency bands for murmurs disagree, and nobody has yet listened to these sounds with a clinician's ears. Use them to learn
*when* a sound occurs and *how it behaves with the rhythm*; real patients sound different.

## The extra sounds

| Sound | When in the cycle | On the lane | What the rhythm does to it |
|---|---|---|---|
| **S3** | early diastole, 120–200 ms after A2 (the model uses 125 ms) | a small slow wave labelled `S3` | follows every beat that ejected; present in atrial fibrillation too |
| **S4** | late diastole, about 70 ms after the P wave, just before S1 | a small slow wave labelled `S4` | needs an atrial contraction: never in atrial fibrillation, and in complete heart block it follows the P waves, not the QRS; no S4 from a P wave that falls in ventricular systole |
| **Summation gallop** | S3 and S4 together in a short diastole | `S3` and `S4` joined into one label over one low wave | appears at fast rates when the P wave moves into early diastole; slow the heart and the two separate |
| **Ejection click** | 120–140 ms (aortic) or 90–110 ms (pulmonary) after the QRS onset | a sharp spike `EC` soon after S1 | every beat; a severe, rigid valve has none |
| **Mid-systolic click** | mid-systole (mitral valve prolapse) | a sharp spike `C` | every beat, followed by the late systolic murmur |
| **Opening snap** | 40–120 ms after A2 (mitral stenosis); under 70 ms suggests severe stenosis | a sharp spike `OS` | earlier the tighter the valve |
| **Friction rub** | up to three scratchy bursts: atrial systole, ventricular systole, early diastole | three small noise bursts, no label | the atrial burst disappears without an atrial contraction |

## The murmurs

A murmur is drawn as its **envelope**: a band of noise between the heart sounds, with no label. Its shape is the lesson.

| Scenario | Murmur | Shape on the lane | With the rhythm and breathing |
|---|---|---|---|
| Aortic stenosis, mild and severe | ejection (mid-systolic) | a diamond that starts after a silent gap following S1 and ends before A2; peaks later when severe, with a soft single S2 | louder on the beat after a long pause (after an extrasystole, long cycles in atrial fibrillation) |
| Pulmonary stenosis | ejection | a diamond after the pulmonary click; P2 soft and late, S2 widely split | louder after a long pause and on inspiration |
| Mitral regurgitation | holosystolic | a flat plateau from S1 through A2 | hardly changes after a long pause: the classic contrast with aortic stenosis |
| Tricuspid regurgitation | holosystolic | a plateau from S1 to P2 | louder on inspiration (the same breathing clock that widens the S2 split) |
| Ventricular septal defect | holosystolic | a dense, high plateau from S1 to S2 | steady |
| Mitral valve prolapse | late systolic | from the click to A2 | steady (manoeuvres that move the click are not modelled) |
| Aortic regurgitation | early diastolic | a thin band that starts at A2 and tapers through diastole | ends early when the next beat comes early |
| Mitral stenosis, sinus rhythm | mid-diastolic rumble with presystolic accentuation | low noise after the opening snap that swells again just before S1 | the swelling needs the atrial contraction |
| Mitral stenosis, atrial fibrillation | mid-diastolic rumble | low noise after the snap that runs on to the next S1, without a final swell | longer in the long cycles |
| Atrial septal defect | soft pulmonary flow murmur and a **fixed split** of S2 | a small diamond; A2 and P2 the same distance apart in inspiration and expiration | the split changes by less than 20 ms with breathing |
| Innocent (Still's) murmur | early to mid systolic, low and musical | a short narrow band well before S2 | S2 splits normally |

In **slow motion** each murmur keeps its pitch while its envelope stretches with the interval it fills, just as the gaps
between the heart sounds stretch.

## Where you listen: sites, bell and diaphragm

The *Display* popup has a **Listening site** control under *Play heart sounds*: the **overview** and the five classic
auscultation areas, as a list and as markers on a small chest drawing (click a marker to listen there).

![The five auscultation sites](../images/auscultation-sites.svg)

| Site | Where | What stands out there (in the model) |
|---|---|---|
| **Overview** (the default) | not a place on the chest | every sound at its modelled level: the lane as it has always been, without a chest piece |
| **Aortic** | 2nd intercostal space, right sternal border | A2 dominates S2; the murmur of aortic stenosis and the aortic ejection click |
| **Pulmonic** | 2nd intercostal space, left sternal border | P2 is heard best, so the split of S2 is easiest to follow; pulmonary stenosis and the flow murmur of an atrial septal defect |
| **Erb's point** | 3rd intercostal space, left sternal border | the high decrescendo of aortic regurgitation; the pericardial rub |
| **Tricuspid** | lower left sternal border | T1; tricuspid regurgitation, the ventricular septal defect and the innocent (Still's) murmur |
| **Mitral** | the apex, 5th intercostal space, mid-clavicular line | S1 is loudest; S3, S4, the opening snap; mitral regurgitation, prolapse and the rumble of mitral stenosis |

The positions on the drawing come from the same intercostal landmarks the electrode editor uses. The lane and the sound
always follow the selected site, and the lane's title names it, for example *Heart sounds (model) · Apex · Bell*. A change
of site is heard from the next sample; what is already on the lane stays as it was drawn.

**Chest piece.** At a site, choose the **diaphragm** (the default) or the **bell**. The bell brings out low sounds and
softens high ones: S3, S4 and the mid-diastolic rumble of mitral stenosis are much easier to hear with the bell at the apex,
which is the point of the control. The diaphragm does the opposite: S2, the clicks and the high regurgitant murmurs stand
out, and S3 and S4 almost disappear. Each sound keeps its timing; only its loudness changes. The overview is heard without a
chest piece, so the bell/diaphragm switch is greyed out there.

**These levels are designed.** No source gives the loudness of a heart sound at each area, nor the response of a
stethoscope that a model could copy. The gains (in `assets/auscultation_sites.ron`) only turn the textbook statements into
numbers: S1 loudest at the apex, A2 dominant at the aortic area, P2 heard at the pulmonic area, each murmur loudest over its
valve, aortic regurgitation at Erb's point. The bell and the diaphragm are two weightings by frequency (a low and a high
emphasis), not the measured behaviour of a real chest piece. Radiation (to the neck, the axilla, the back) is not modelled.

## Valve highlights

With the **Valve planes** layer visible in the 3-D view, each valve's disc flashes when its closure sound occurs: the mitral
valve with M1, the tricuspid with T1, the aortic with A2 and the pulmonic with P2. With a valve's own layer shown (**Tricuspid valve**,
**Mitral valve**, **Aortic valve** or **Pulmonary valve**), that valve's flash is on its leaflets instead of the flat disc. In the valve scenarios the stenotic or
regurgitant valve also glows for as long as its murmur sounds (the aortic valve through the murmur of aortic stenosis, the
mitral valve through the rumble of mitral stenosis, and so on; the septal defects, the innocent murmur and the rub belong to
no valve and light nothing). The highlights come from the same schedule as the sound and are drawn with the same delay as the
strip, so a flash, the label on the lane and the sound happen together. *Reduced motion* in Settings turns them off.

## What is not modelled

* The ECG of the valve scenarios is ordinary sinus rhythm (atrial fibrillation in one mitral stenosis preset): no hypertrophy
  voltage, no left atrial enlargement, no right axis of an atrial septal defect. These scenarios are therefore never asked in
  the Rhythm Quiz or the Interval Drill.
* The listening sites are a designed weighting of the same sounds, not a model of sound travelling through the chest.
  Radiation (to the neck, the axilla) is not modelled.
* No manoeuvres (Valsalva, squatting, handgrip, standing), no pulse, no venous waves, no carotid or apex signs.
* Critical aortic stenosis with a failing ventricle (whose murmur becomes softer), acute regurgitation, HOCM, patent ductus and
  prosthetic valves are not included.

The sources of every rule and the designed values are listed in `docs/research/heart-sounds.md`, in the parameter
documentation of `crates/heartsim-ecg/src/auscultation.rs` and, for the sites and chest pieces, at the top of
`assets/auscultation_sites.ron`.
