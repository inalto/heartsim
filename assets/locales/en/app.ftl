# heartsim-app: tabs, toolbar, axis trainer, quiz, help. Brand names (heartsim) and abbreviations stay as they are.

## Tabs
tab-simulator = Simulator
tab-12-lead = 12-Lead ECG
tab-heart-vector = Heart Vector
tab-axis-trainer = Axis Trainer
tab-axis-quiz = Axis Quiz
tab-electrodes = Electrodes
tab-rhythm-quiz = Rhythm Quiz
tab-tutorials = Tutorials
tab-help = Help

## Page groups of the top bar (nav.rs)
nav-group-explore = Explore
nav-group-practise = Practise
nav-group-learn = Learn
nav-shortcut = Shortcut: key { $key }

## Toolbar
app-av-block = AV block
app-sa-rate = SA rate
app-mobitz = Mobitz II: drop every n-th beat
app-axis-shift = Axis shift
app-qrs-width = QRS width
app-mobitz-short = Mobitz II
app-mobitz-off = off
app-adjust = Adjust
app-adjust-count = Adjust · { $count }
app-adjust-hint = Sinus rate, AV block, QRS axis and width. Shown when a value differs from the rhythm's own.
app-adjust-changed = Changed from this rhythm: { $what }
app-pacemaker = Pacemaker
app-therapy = Therapy
app-frontal-axis = Frontal QRS axis
app-net-polarity = Net QRS polarity:
app-layout = Layout:
app-save-png = Save PNG

## Axis trainer
app-axis-trainer-hint = Click or drag on the circle to rotate the heart's electrical axis.
app-reset-axis = Reset to normal axis
app-net-qrs-each = Net QRS in each lead
app-axis-normal = Lead I and aVF both positive (or II positive): normal.
app-axis-left = Lead I positive, aVF negative: left axis deviation.
app-axis-right = Lead I negative, aVF positive: right axis deviation.
app-axis-extreme = Both I and aVF negative: extreme (north-west) axis.

## Axis quiz
app-quiz-new = New question
app-quiz-check = Check answer
app-quiz-score = Score: { $correct } of { $asked } correct
app-quiz-intro = Press New question, read the six limb leads, then click the axis on the circle.
app-quiz-click = Click the circle where you think the QRS axis points.
app-quiz-answer = Answer { $answer }°, you chose { $chosen }° (off by { $off }°)
app-quiz-your-answer = Your answer: { $chosen }°. Press Check answer.

## Help
help-intro = An open educational heart electrophysiology and ECG simulator.
help-tabs-heading = Tabs
help-tab-simulator = Simulator: the heart model with one selectable lead and the live axis.
help-tab-12-lead = 12-Lead ECG: all twelve leads at once (12 × 1, 6 × 2 or the clinical 3 × 4 with rhythm strip) next to the axis circle.
help-tab-axis-trainer = Axis Trainer: drag on the circle to rotate the axis and watch every limb lead change.
help-tab-heart-vector = Heart Vector: the cardiac dipole at the strip cursor and its loop over the last beat, on the frontal and horizontal planes.
help-tab-axis-quiz = Axis Quiz: a hidden axis is dialled in; read the leads and click your answer.
help-tab-tutorials = Tutorials: guided lessons that drive the simulator page by page, and an editor for a rhythm's timeline of interventions.
help-tab-rhythm-quiz = Rhythm Quiz: name the rhythm on a strip from the scenario library; the choices include the rhythms it is most often mistaken for.
help-rhythms-heading = Rhythms and scrubbing
help-rhythms-text = Pick a rhythm from the Rhythm list above the strip: the ECG and the 3-D heart both run from it. Pause and drag the 's back' slider: the heart, the heart-vector arrow and the strip all show the same instant.
help-axis-heading = Finding the axis
help-axis-text = Normal axis is −30° to +90°. Lead I and aVF both positive: normal. I positive, aVF negative: left deviation (beyond −30° when II is also negative). I negative, aVF positive: right deviation. Both negative: extreme axis. The equiphasic limb lead is perpendicular to the axis.
help-keys-heading = Keyboard shortcuts
help-keys-space = Space: pause or resume the simulation.
help-keys-arrows = ← / →: while paused, move the scrub cursor back or forward. When a caliper has focus, the arrows nudge the caliper instead.
help-keys-speed = + / −: simulation and paper speed faster or slower.
help-keys-tabs = 1 – 9, 0: open a page, in the order of the Menu (the number is shown next to each page).
menu-button = Menu
menu-theme = Theme
settings-theme-system = System
settings-theme-light = Light
settings-theme-dark = Dark
assets-missing = The program's assets (anatomy pack, rhythms, tutorials, cases) were not found: the heart is a placeholder sphere and the rhythm list is empty.
assets-missing-hint = Reinstall heartsim, or set HEARTSIM_ASSETS to the folder that contains anatomy/default/manifest.ron. "heartsim --assets-dir" prints where it looked.
help-keys-note = Shortcuts are off while a text box has the keyboard.
help-disclaimer = Educational tool, not for clinical use.

## Settings window (settings.rs)
settings-button = Settings
settings-title = Settings
settings-language = Language
settings-language-system = System default
settings-high-contrast = High-contrast theme
settings-reduced-motion = Reduced motion (no automatic rotation, shock flash or valve flashes)
settings-torso-opacity = Torso picture in the axis panel
settings-torso-opacity-hint = Opacity of the frontal torso behind the limb leads; 0 % shows the outline drawing instead.
settings-beep = Monitor beep (QRS-synchronous, off by default)
settings-beep-volume = Volume
settings-beep-hint = A short tone at each ventricular activation, at most one per 180 ms of simulated time; not sample-accurate with the strip.
settings-beep-unavailable = No sound device could be opened, so the beep stays silent.
settings-hs-heading = Heart sounds
settings-hs-play = Play heart sounds (off by default)
settings-hs-volume = Volume
settings-hs-offset = Audio offset
settings-hs-offset-hint = Your sound card plays a little late, and the program cannot measure by how much. The strip and the 3-D heart are drawn this much (plus 40 ms of queue) behind the simulation so that what you hear matches what you see. Use the sync check to set it.
settings-hs-headphones = Use headphones: laptop speakers reproduce little below 150 Hz, where most of S1 and S2 lies.
settings-hs-model = The sounds are a model computed from the same beats as the ECG, not a recording.
settings-hs-unavailable = No sound device could be opened, so the heart sounds stay silent.
settings-hs-unsupported = Sound is not available in this browser, so the heart sounds stay silent.
settings-hs-click-to-enable = Click anywhere to enable sound: the browser plays sound only after a click or a key press.
settings-hs-sync = Check sync
settings-hs-sync-stop = Stop the sync check
settings-hs-sync-hint = A click every second while the dot lights up. Move the offset until the click and the light come together.
settings-hint = Changes apply at once and are remembered on this computer.
settings-update-check = Check for a newer release at start
settings-update-check-hint = One request to api.github.com that sends only the app version. When a newer release exists a notice appears; the update is downloaded only if you choose Update now.

## Update notice (update_check.rs)
update-available = heartsim { $version } is available.
update-download = Download from the release page
update-dismiss = Dismiss
update-install = Update now…
update-modal-title = Update to heartsim { $version }
update-modal-file = Installer: { $file } ({ $size } MB)
update-modal-note = The installer is downloaded from GitHub, checked against the SHA-256 published with the release, then started. heartsim closes so the update can replace it.
update-modal-start = Download and install
update-modal-cancel = Cancel
update-modal-close = Close
update-modal-retry = Retry
update-modal-progress = { $done } of { $total } MB
update-modal-installing = Starting the installer… (on Linux the system may ask for your password)
update-modal-closing = The update is starting; heartsim is closing.
update-modal-no-installer = The release has no installer for this system. Download it from the release page.
update-modal-failed = The update did not complete: { $error }

## Export / Import window (app/exports.rs)
export-title = Export / Import
export-reviewing = Reviewing { $label }
export-heading = Export
export-length = Length
export-seconds = s
export-pdf-paper = PDF paper
export-formats = Formats
export-scenario = Scenario
export-bundle = Share bundle (.zip)
export-folder = Folder
export-settings-summary = { $speed } mm/s, { $gain } mm/mV and the { $order } lead order from the strip; ends at the strip cursor.
export-button = Export
export-live-hint = Go back to live to export: a recording under review is not a simulation.
export-screenshot = Window screenshot
export-screenshot-hint = PNG of the whole window, including the 3-D view.
import-heading = Import for review
import-hint = .csv, .hea, .zip or .ron path
import-button = Import
import-drop-hint = Or drop a file on the window. The strip and the 12-lead page show it; calipers work as usual.
export-review-length = Reviewing { $label } ({ $seconds } s)
export-back-live = Back to live
export-failed = Export failed: { $error }
export-saved = Saved { $path }
export-screenshot-failed = Screenshot failed: { $error }
import-failed = Import failed: { $error }
import-failed-path = Import failed: { $path }: { $error }
export-running = Running { $title }
export-closed = Closed { $label }
import-reviewing = Reviewing { $label }: { $seconds } s at { $rate } Hz

## Heart vector, therapy and timeline (app/vector_tab.rs, app/therapy.rs, app/tutorials.rs)
vector-heading = Heart vector
vector-intro = The arrow is the cardiac dipole at the strip cursor, recovered from all twelve leads. The loop is the path of its tip through the last complete beat (blue to pink).
vector-x = x { $value }  ({ $side })
vector-y = y { $value }  ({ $side })
vector-z = z { $value }  ({ $side })
vector-left = left
vector-cranial = cranial
vector-anterior = anterior
vector-scrub-hint = Pause and scrub the strip to step the arrow through the P, QRS and T loops.
therapy-drugs = Drugs
therapy-ischaemia = Ischaemia
therapy-defib = Defibrillator
timeline-editor-mode = Scenario timeline editor
timeline-load-first = Load a rhythm (Simulator tab, Rhythm list) to edit its timeline.
timeline-row-error = A row does not parse (hover it for the error).
timeline-apply = Apply and restart
timeline-apply-hint = Use this timeline for the running scenario (this session only).
timeline-revert = Revert
timeline-file = File:
timeline-save = Save scenario
timeline-not-saved = Not saved: { $error }
timeline-save-desktop-only = Saving scenario files is a desktop feature.

## Heart view (heart_view.rs)
colour-voltage = Voltage
colour-activation = Activation time
colour-apd = APD
part-lv = Left ventricle
part-rv = Right ventricle
part-la = Left atrium
part-ra = Right atrium
part-aorta = Aorta
part-pulmonary-artery = Pulmonary artery
part-pulmonary-veins = Pulmonary veins
part-svc = Superior vena cava
part-ivc = Inferior vena cava
part-valve-rings = Valve rings
part-valve-planes = Valve planes
part-torso = Torso
heart-ablated = Ablated { $radius } mm around { $region }
heart-stimulus = Stimulus: { $region } at { $time } ms
heart-ablation-cleared = Ablation lesions cleared
heart-tier1 = 3-D activation
heart-tier1-hint = Off: ECG strip only; the heart is drawn without a colour map.
heart-reset-window = Reset window
heart-colour-blind = Colour-blind safe
heart-wavefront = Wavefront outline
heart-click = Click on the heart
heart-stimulate = Stimulate
heart-ablate = Ablate
heart-lesion-radius = Lesion radius
heart-shift-click = Shift-click ablates.
heart-clear-ablation = Clear ablation
heart-territory = Territory: { $territory }
heart-activated = Activated at { $time } ms this beat
heart-not-activated = Not yet activated this beat
heart-apd = APD { $value } ms

## Shared feedback
feedback-correct = Correct.

## Rhythm quiz (rhythm_quiz.rs)
rq-streak = streak { $count }
rq-twelve-lead = Show all 12 leads
rq-no-library = No scenario library found (assets/scenarios).
rq-intro = Press New question, read the strip, then choose the rhythm.
rq-correct-choice = { $title }  (correct)
rq-your-choice = { $title }  (your answer)
rq-wrong = Not quite: this is { $title }.
rq-why = Why
rq-open = Open in simulator

## Tutorial reader (tutorial_ui.rs)
tut-mode = Mode:
tut-self-study = Self-study
tut-self-study-hint = Pages in order; answer each check before moving on.
tut-tutor = Tutor
tut-tutor-hint = Free navigation for presenting to a group; checks are optional.
tut-none = No tutorials installed (assets/tutorials).
tut-completed = Completed
tut-pages = { $count } pages
tut-start = Start { $title }
tut-resume = Continue { $title } at page { $page }
tut-close = Close tutorial
tut-page-of = Page { $page } of { $total }
tut-contents = Contents
tut-passed = (passed)
tut-check = Check your understanding
tut-try-again = Not quite. Try again.
tut-previous = Previous
tut-next = Next
tut-finish = Finish
tut-answer-first = Answer the question first.
tut-replay = Replay
tut-replay-hint = Run this page's simulator actions again.
tut-running = running…

## Patient cases (case_ui.rs)
case-heading = Patient cases
case-intro = Short scripted stories: read the vignette, look at the ECG the simulator shows, answer the questions.
case-none = No cases installed (assets/cases).
case-best = best { $correct } / { $total }
case-size = { $stages } stages · { $questions } questions
case-start = Start { $title }
case-close = Close case
case-score = { $correct } correct of { $answered } answered (of { $total })
case-finished = Case finished
case-final-score = You answered { $correct } of { $total } questions correctly at the first try.
case-first-try-hint = Only the first answer to each question counts. Start the case again to improve your score.
case-stage-of = Stage { $stage } of { $total }: { $title }
case-question-of = Question { $question } of { $total }
case-wrong = Not quite. The answer is: { $answer }
case-finish = Finish case
case-continue = Continue
case-answer-first = Answer the question first.

## Heart vector overlay (vector_overlay.rs)
vec-arrow = Arrow
vec-loop = Loop (last beat)
vec-see-through = See-through heart
vec-see-through-hint = Make the heart translucent so the vector inside it shows.
vec-scale = Scale

## Electrode tab (electrode_tab.rs)
el-save = Save placement
el-saved = Saved { $file }
el-save-failed = Save failed: { $error }
el-load = Load placement
el-loaded = Loaded { $file }
el-load-failed = Load failed: { $error }
el-no-3d = 3-D view unavailable: edit the electrodes from the panel.
el-hidden = Electrodes hidden during the exercise: read the ECG.
el-drag-hint = Drag an electrode anywhere on the body; drag elsewhere to rotate.
el-view-body = Whole body
el-view-chest = Chest

## Export panel and scenario picker (export_panel.rs, scenarios.rs)
export-wrote = Wrote { $names } to { $dir }
export-nothing = Nothing written
scenario-custom = Custom (sliders)
scenario-rhythm = Rhythm:
scenario-restart = Restart
scenario-restart-hint = Reload the scenario from t = 0.
scenario-about = About this rhythm
scenario-group-sounds = Heart sounds and murmurs
rq-next = Next question
rq-question = What is the rhythm?
rq-answer = Answer:
el-save-hint = Write a scenario with these electrodes to { $file }

## Re-entry presets (app/reentry.rs)
reentry-picker = Re-entry presets…
reentry-tier1 = Fast model
reentry-tier2 = Tissue model · { $count } phase singularities · quiet { $quiet } s
reentry-speed = { $tier } · { $speed }× real time
reentry-speed-hint = The tissue model is not real time on every machine; the simulation runs as fast as it can.
reentry-stop = Stop

## First-run disclaimer and About (requirement N-09; wording follows docs/book/src/user/disclaimer.md)
disclaimer-title = Before you start: educational use only
disclaimer-lead = heartsim is an educational simulation. It is not a medical device and must not be used for diagnosis, treatment, monitoring or any other clinical decision.
disclaimer-models = The heart, the torso and the conduction system are simplified models. Every rhythm is a teaching example built from textbook criteria, not a recording of a patient.
disclaimer-values = Intervals, amplitudes and morphologies are realistic enough to practise recognition and measurement, but they are not validated against clinical data for any individual. Real ECGs are noisier and more varied.
disclaimer-exports = Exported files (PDF, PNG, CSV, WFDB) say "simulated" in their header. They contain no patient data, and they must not be presented as patient recordings.
disclaimer-import = Importing a real recording for review only displays it. heartsim does not interpret it, and its measurements are no substitute for a qualified reader or a certified analysis program.
disclaimer-warranty = The software is provided "as is", without warranty of any kind; see the MIT and Apache-2.0 licences that come with it.
disclaimer-reminder = You can read this again at any time under About.
disclaimer-accept = I understand: educational use only
about-button = About
about-title = About heartsim
about-version = Version { $version }
about-tagline = 3-D heart electrophysiology and 12-lead ECG simulator for medical education.
about-licence = Free software under the MIT or Apache-2.0 licence, at your choice. Anatomy and content are derived from the published literature; the user guide lists the sources.
about-disclaimer-heading = Medical disclaimer

## Export of the displayed trace (requirement F-15)
export-trace = Trace
export-as-displayed = As displayed (with filter and artefacts)
export-as-displayed-hint = Off: the clean simulated signal is exported. On: the trace as the strip shows it, and every file states the filter and artefacts applied.
export-as-displayed-active = The display filter or artefacts are on: the files will say so.

## Quiz explanations (rhythm, axis and interval drills; brief B8)
rq-yours-was = You chose: { $title }
rq-confused = Often confused with: { $list }
axis-why-normal = Lead I is { $i } and lead II is { $ii }: the net QRS vector points to the left and down, between −30° and +90°. That is a normal axis.
axis-why-left = Lead I is { $i } and lead II is { $ii }: the net QRS vector points to the left and up, beyond −30°. That is left axis deviation (−30° to −90°).
axis-why-right = Lead I is { $i } and aVF is { $avf }: the net QRS vector points to the right and down, beyond +90°. That is right axis deviation (+90° to +180°).
axis-why-extreme = Lead I is { $i } and aVF is { $avf }: the net QRS vector points up and to the right, into "no man's land". That is extreme axis deviation (−90° to ±180°).

## Interval drill
tab-interval-drill = Interval Drill
help-tab-interval-drill = Interval Drill: measure the PR interval, the QRS duration or the QT interval of a strip with the calipers; answers within a tolerance count.
interval-intro = Press New question, then measure the interval it names on the strip with the calipers.
interval-task = Measure the { $interval } in lead II at { $speed } mm/s. Place the calipers on the strip, enter the result and press Check answer.
interval-name-pr = PR interval
interval-name-qrs = QRS duration
interval-name-qt = QT interval
interval-def-pr = PR interval: from the start of the P wave to the start of the QRS complex. Normal: 120–200 ms.
interval-def-qrs = QRS duration: from the first deflection of the QRS complex to its end. Normal: under 120 ms.
interval-def-qt = QT interval: from the start of the QRS complex to the end of the T wave. It shortens as the heart rate rises.
interval-calipers-place = Place calipers
interval-calipers-clear = Clear calipers
interval-calipers-use = Use caliper reading
interval-calipers-hint = Drag the two lines to the start and the end of the interval; shift-click on the strip adds another pair.
interval-calipers-reading = Calipers: { $ms } ms
interval-your-value = Your measurement
interval-correct = Correct: you measured { $entry } ms, the reference is { $truth } ms (tolerance ±{ $tolerance } ms).
interval-wrong = Not quite: you measured { $entry } ms, the reference is { $truth } ms (off by { $off } ms, tolerance ±{ $tolerance } ms).
interval-rhythm = Rhythm: { $title }.
interval-reference-note = The reference is the simulator's own measurement of this strip; the labelled waves show where it measured. Real readers differ a little, which is why a tolerance applies.
interval-no-strip = No suitable strip was found. Press New question to try again.

## Frontal QRS-axis panel: the cards beside the circle, the sector names and the legend
axis-measured-heading = Measured axis
axis-no-axis = No QRS complexes yet
axis-interpretation-heading = Interpretation
axis-sector-normal = Normal axis
axis-sector-left = Left axis deviation
axis-sector-right = Right axis deviation
axis-sector-extreme = Extreme axis (indeterminate)
axis-range = { $from } to { $to }
axis-interp-normal = The QRS axis lies between { $from } and { $to }: a normal axis.
axis-interp-left = The QRS axis lies beyond { $from }, up to { $to }: left axis deviation.
axis-interp-right = The QRS axis lies beyond { $from }, up to { $to }: right axis deviation.
axis-interp-extreme = The QRS axis lies between { $from } and { $to }, the "north-west" quadrant: extreme axis deviation (indeterminate axis).
axis-legend-heading = Axis classes (frontal plane)
axis-polarity-heading = Net QRS polarity
axis-torso-heading = Limb leads on the torso
axis-arrow-tooltip = QRS axis { $deg } — drag to turn it
axis-guess-tooltip = Your answer { $deg } — drag to change it
readout-heading = Measurements
interval-calipers-use-disabled = Place the calipers first
interval-check-disabled = Enter a measurement first
interval-value-hint = Click to type a value, or drag to adjust it
tut-progress = { $done } of { $total } pages
tut-start-short = Start
tut-continue = Continue
tut-start-again = Start again
tut-start-again-hint = Start { $title } again from page 1

## Heart-sound teaching (B28): Sound Trainer, auscultation quiz, sound check
tab-sound-trainer = Sound Trainer
tab-auscultation-quiz = Auscultation Quiz
help-tab-sound-trainer = Sound Trainer: single heart-sound findings as loops to listen to again and again, with a count of your listens, and contrast pairs to switch between at the same point of the cycle.
help-tab-auscultation-quiz = Auscultation Quiz: listen to a hidden case, tick what you hear, then choose the diagnosis; the trace and the answer follow.
settings-hs-small-speaker = Small-speaker mode (alters the sound)
settings-hs-small-speaker-hint = Raises every heart sound by about one and a half octaves, so that S3, S4 and the mitral rumble reach laptop and phone speakers. It changes how the sounds sound and has no proven teaching effect. The lane keeps the true trace and is tagged "altered sound". Headphones are better.
export-wav = Heart sounds (WAV)
export-wav-hint = The heart-sound signal of the same excerpt, as heard at the listening site: 16-bit mono WAV at 44.1 kHz, marked in the file as synthesised by heartsim.
st-findings = Findings
st-pairs = Contrast pairs
st-play = Play
st-stop = Stop
st-listens = Listens: { $count } · all findings: { $total }
st-half-speed = Half speed (altered sound)
st-half-speed-hint = The gaps between the sounds double while each sound keeps its pitch, so a split or a gallop is easier to pick out. An aid, not the real timing; no study shows that it helps learning.
st-show-trace = Show the trace
st-trace-no-sound = No sound output here, so the trace stays on: learn the timing from the labelled phonocardiogram.
st-no-sound = No sound output here.
st-switch = Switch (S)
st-switch-hint = Go to the next sound of the pair at the same point of the cardiac cycle. The S key does the same.
st-save-wav = Save WAV
st-save-wav-hint = One loop of this clip, as it is played, as a WAV file in the export folder.
st-model-note = Synthesised heart sounds: a model computed from the simulated beats, not a recording.
st-trace-hidden = The trace is hidden: listen first, then show it to check what you heard.
st-s1_s2 = S1 and S2
st-s1_s2-listen = Normal sinus rhythm at the apex. S1 (mitral and tricuspid closure) opens systole just after the QRS; S2 (aortic and pulmonary closure) closes it near the end of the T wave. At normal rates the pause after S1 is shorter than the pause after S2: that is how to tell them apart.
st-split_physiological = Physiological split
st-split_physiological-listen = At the pulmonic area S2 is single in expiration and splits into A2 then P2 in inspiration, when the right ventricle fills more and empties later. Hold the breath at peak inspiration, then at end-expiration, to compare the two.
st-split_wide = Wide split (RBBB)
st-split_wide-listen = Right bundle-branch block delays the right ventricle: P2 comes late in both phases of the breath, and the split widens further on inspiration.
st-split_fixed = Fixed split (ASD)
st-split_fixed-listen = With an atrial septal defect the split is wide and hardly changes with breathing; a soft flow murmur at the pulmonic area comes with it.
st-split_paradoxical = Paradoxical split (LBBB)
st-split_paradoxical-listen = Left bundle-branch block delays the left ventricle, so A2 follows P2. The split is heard in expiration and narrows on inspiration: the reverse of normal.
st-s1_soft = Soft S1 (long PR)
st-s1_soft-listen = First-degree AV block: with a long PR interval the mitral leaflets drift almost shut before the ventricle contracts, so S1 is quieter than S2 at the apex.
st-s1_variable = Variable S1 (complete block)
st-s1_variable-listen = In complete heart block the atria and the ventricles beat independently: the PR changes from beat to beat and so does the loudness of S1, with an occasional very loud one.
st-s3 = Third heart sound (S3)
st-s3-listen = Heart failure: a low, dull sound early in diastole, about 0.12–0.15 s after S2, during rapid filling. Bell, lightly applied, at the apex.
st-s4 = Fourth heart sound (S4)
st-s4-listen = A stiff, hypertrophied ventricle: a low sound just before S1, from the atrial contraction. Bell at the apex. It needs an atrial contraction, so it is never heard in atrial fibrillation.
st-summation_gallop = Summation gallop
st-summation_gallop-listen = At a fast rate the short diastole brings S3 and S4 together into one loud sound in mid-diastole.
st-ejection_click = Ejection click
st-ejection_click-listen = Mild aortic stenosis: a short, high click just after S1 as the valve opens, then the ejection murmur.
st-midsystolic_click = Mid-systolic click (prolapse)
st-midsystolic_click-listen = Mitral valve prolapse: a click in mid-systole as the leaflet snaps back, followed by a late systolic murmur up to S2.
st-opening_snap = Opening snap
st-opening_snap-listen = Mitral stenosis: a sharp, high sound shortly after S2 as the stiff valve opens; the closer it is to S2, the tighter the stenosis. Diaphragm.
st-murmur_as = Aortic stenosis
st-murmur_as-listen = Severe aortic stenosis: a harsh crescendo–decrescendo murmur in systole that peaks late, with a soft, single S2. Aortic area.
st-murmur_ar = Aortic regurgitation
st-murmur_ar-listen = A high-pitched, blowing decrescendo murmur that starts right at S2. Erb's point, diaphragm.
st-murmur_mr = Mitral regurgitation
st-murmur_mr-listen = A blowing, level murmur from S1 to S2 at the apex, much the same from beat to beat.
st-murmur_ms = Mitral stenosis rumble
st-murmur_ms-listen = A low rumble in mid-diastole after the opening snap, louder again just before S1 in sinus rhythm. Bell, at the apex.
st-murmur_tr = Tricuspid regurgitation
st-murmur_tr-listen = A holosystolic murmur at the lower left sternal border that grows louder on inspiration.
st-murmur_ps = Pulmonary stenosis
st-murmur_ps-listen = An ejection murmur at the pulmonic area, with a click and a late, soft P2: the split is wide.
st-murmur_vsd = Ventricular septal defect
st-murmur_vsd-listen = A harsh, high-pitched holosystolic murmur at the lower left sternal border.
st-murmur_innocent = Innocent murmur
st-murmur_innocent-listen = A short, soft, musical murmur in early to mid systole with a normal S2: no click, no extra sound, nothing in diastole.
st-rub = Pericardial friction rub
st-rub-listen = Pericarditis: a scratchy sound with up to three parts in each cycle (atrial systole, ventricular systole, early diastole). Erb's point, diaphragm.
st-pair_innocent_as = Innocent murmur or aortic stenosis
st-pair_innocent_as-listen = Both are systolic. The innocent murmur is short, soft and musical and ends well before S2; mild aortic stenosis has an ejection click and a harsher murmur that lasts longer.
st-pair_ejection_regurgitant = Ejection or regurgitant murmur
st-pair_ejection_regurgitant-listen = An ejection murmur rises and falls between S1 and S2 and leaves a gap after S1; a regurgitant murmur starts with S1 and stays level up to S2.
st-pair_split_snap_s3 = Split S2, opening snap or S3
st-pair_split_snap_s3-listen = Three ways to hear two sounds at the end of systole: a split S2 (two close, high sounds), S2 and an opening snap (a little later, sharp), S2 and S3 (later still, low and dull).
st-pair_s4_split_s1 = S4 or split S1
st-pair_s4_split_s1-listen = A sound just before S1 can be an S4 (low, dull, best with the bell) or the first half of a split S1 (high, the two halves close together, as in RBBB).
st-member-innocent = Innocent murmur
st-member-as = Aortic stenosis
st-member-ejection = Ejection (aortic stenosis)
st-member-regurgitant = Regurgitant (mitral regurgitation)
st-member-split-s2 = Split S2
st-member-snap = Opening snap
st-member-s3 = S3
st-member-s4 = S4
st-member-split-s1 = Split S1
aq-intro = Listen to a hidden case, tick what you hear, then choose the diagnosis. The trace and the answer come afterwards. Headphones help.
aq-intro-listen = Press New question to hear a case.
aq-score = Features { $features }/{ $scored } · diagnoses { $correct }/{ $asked }
aq-step-features = Step 1: what do you hear?
aq-step-features-hint = Play the clip and move the stethoscope if you like. Tick everything you hear and leave the rest.
aq-timing = The murmur lies…
aq-f-before-s1 = An extra sound just before S1
aq-f-after-s2 = An extra sound after S2
aq-f-split = A split S2 in both phases of the breath
aq-f-click = A click in systole
aq-f-systolic = A systolic murmur
aq-f-diastolic = A diastolic murmur
aq-f-early = early in its phase
aq-f-mid = in the middle of its phase
aq-f-late = late in its phase
aq-f-throughout = throughout its phase
aq-features-done = Done: choose the diagnosis
aq-step-diagnosis = Step 2: what is it?
aq-features-result = Features right: { $right } of { $total }
aq-present = present
aq-absent = absent
aq-mark-right = Right ·
aq-mark-wrong = Missed ·
aq-split-not-scored = The split was not scored: a physiological split is heard only in inspiration, so either answer is fair.
aq-no-slow-motion = Half speed is offered once both steps are answered: slowing the sound changes what is being tested.
aq-no-sound = No sound output here, so the trace is shown from the start: answer from the labelled phonocardiogram. The name stays hidden until you answer.
aq-trace-hidden = The trace is hidden until you have answered both steps.
tut-sound-check = Sound check
tut-sound-check-none = There is no sound output here (no sound device, or no sound in this build). Follow the tutorial with the heart-sound lane under the strip: every sound is labelled on it, in time with the ECG.
tut-sound-check-low = Play the low tone (200 Hz)
tut-sound-check-high = Play the higher tone (800 Hz)
tut-sound-check-heard = I can hear the low tone
tut-sound-check-not-heard = I cannot hear the low tone
tut-sound-check-ok = Good: your output reproduces the range where most heart sounds lie.
tut-sound-check-small = Your speakers are too small for heart sounds.
tut-sound-check-small-hint = Most of S1, S2, S3, S4 and the rumbles lie below 200 Hz. Use headphones, and read the heart-sound lane under the strip as you listen. Small-speaker mode in Settings raises the pitch so that something can be heard, but it alters the sounds.
