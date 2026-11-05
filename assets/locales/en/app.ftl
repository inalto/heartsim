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

## Toolbar
app-av-block = AV block
app-sa-rate = SA rate bpm
app-mobitz = Mobitz II n
app-axis-shift = axis shift °
app-qrs-width = QRS width
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
app-quiz-score = Score { $correct }/{ $asked }
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
help-keys-tabs = 1 – 9: open a tab, in the order shown in the top bar.
help-keys-note = Shortcuts are off while a text box has the keyboard.
help-disclaimer = Educational tool, not for clinical use.

## Settings window (settings.rs)
settings-button = Settings
settings-title = Settings
settings-language = Language
settings-language-system = System default
settings-high-contrast = High-contrast theme
settings-reduced-motion = Reduced motion (no automatic rotation or shock flash)
settings-hint = Changes apply at once and are remembered on this computer.

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
part-torso = Torso
heart-ablated = Ablated { $radius } mm around { $region }
heart-stimulus = Stimulus: { $region } at { $time } ms
heart-ablation-cleared = Ablation lesions cleared
heart-tier1 = 3-D activation (Tier 1)
heart-tier1-hint = Off: ECG strip only (Tier 0); the heart is drawn without a colour map.
heart-reset-window = Reset window
heart-colour-blind = Colour-blind safe
heart-wavefront = Wavefront outline
heart-click = Click on the heart
heart-stimulate = Stimulate
heart-ablate = Ablate
heart-lesion-radius = lesion radius mm
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
tut-completed = completed
tut-pages = { $count } pages
tut-start = Start { $title }
tut-resume = Resume at page { $page }
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

## Heart vector overlay (vector_overlay.rs)
vec-arrow = Arrow
vec-loop = Loop (last beat)
vec-see-through = See-through heart
vec-see-through-hint = Make the heart translucent so the vector inside it shows.
vec-scale = mm per mV

## Electrode tab (electrode_tab.rs)
el-save = Save placement
el-saved = Saved { $file }
el-save-failed = Save failed: { $error }
el-load = Load placement
el-loaded = Loaded { $file }
el-load-failed = Load failed: { $error }
el-no-3d = 3-D view unavailable: edit the electrodes from the panel.
el-hidden = Electrodes hidden during the exercise: read the ECG.
el-drag-hint = Drag an electrode over the torso; drag elsewhere to rotate.

## Export panel and scenario picker (export_panel.rs, scenarios.rs)
export-wrote = Wrote { $names } to { $dir }
export-nothing = Nothing written
scenario-custom = Custom (sliders)
scenario-rhythm = Rhythm:
scenario-restart = Restart
scenario-restart-hint = Reload the scenario from t = 0.
scenario-about = About
rq-next = Next question
rq-question = What is the rhythm?
rq-answer = Answer:
el-save-hint = Write a scenario with these electrodes to { $file }
