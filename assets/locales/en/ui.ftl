# heartsim-ui widgets: strip, 12-lead page, calipers, parameter panels.
# Keys are kebab-case and prefixed by the widget; variables use Fluent's { $name } syntax.
# Units (mm/s, mV, bpm, ms) are not translated.

## ECG paper controls (ecg_view.rs)
ecg-monitor-colours = Monitor colours
ecg-grid = Grid
ecg-waves = Waves
ecg-intervals = PR/RR
ecg-px-per-mm = pt/mm
ecg-calibration-hint = Screen calibration: hold a ruler to the 5 mm grid squares and adjust until they measure 5 mm.
ecg-calipers = Calipers ✕
ecg-calipers-hint = Shift-click a strip to add a caliper pair; drag lines; ←/→ nudge; Esc clears.
ecg-scrub-back = s back
ecg-saved = Saved { $path }
ecg-snapshot-failed = Snapshot failed: { $error }

## Strip and 12-lead page (strip.rs, twelve_lead.rs, vector_view.rs)
sweep-scroll = Scroll
sweep-sweep = Sweep
layout-12x1 = 12 × 1
layout-6x2 = 6 × 2
layout-3x4 = 3 × 4 + rhythm
plane-frontal = Frontal plane
plane-horizontal = Horizontal plane
vector-ring-scale = ring = { $value } mV

## Interval read-out (annotations.rs). HR, RR, PR, QRS, QT and QTc are standard abbreviations and stay as they are.
readout-measuring = Measuring…
readout-no-signal = No signal
readout-no-qrs = No QRS complexes detected
readout-rhythm = Rhythm
readout-regular = regular
readout-irregular = irregular
readout-no-p = no P waves
readout-qrs-axis = QRS axis
readout-t-axis = T axis
axis-normal = normal
axis-left = left deviation
axis-right = right deviation
axis-extreme = extreme

## Defibrillator (defib_panel.rs)
defib-energy = Energy
defib-sync-button = SYNC
defib-sync-hint = Synchronised cardioversion: the shock is delivered on the next R wave (VT with a pulse, SVT, AF, flutter). Off for VF.
defib-charge = CHARGE
defib-disarm = Disarm
defib-idle = idle
defib-charging = charging { $percent } %
defib-charged = charged to { $joules } J
defib-sync-waiting = SYNC: waiting for an R wave…
defib-sync = synchronised
defib-unsync = unsynchronised
defib-terminated = terminated
defib-no-effect = no effect
defib-shock-line = { $joules } J { $mode } at { $at } s on { $rhythm } (p ≈ { $probability } %): { $outcome }
defib-vulnerable = delivered in the T wave (vulnerable period)

## Drug tray (drug_tray.rs)
drug-dose-one = Dose 1 = { $dose }
drug-stop = Stop
drug-dose-given = Dose given { $dose } × standard, { $ago } ago; effect { $effect } % of one standard dose at full effect.

## Anatomy panel (anatomy_panel.rs)
anatomy-layers = Layers
anatomy-opacity = opacity
anatomy-labels = Labels
anatomy-cut-away = Cut-away
anatomy-depth = depth
anatomy-view = View
anatomy-fit = Fit
anatomy-oblique = oblique
anatomy-auto-rotate = Auto-rotate

## Pacemaker panel (pacer_panel.rs)
pacer-title = Pacemaker
pacer-connected = Connected
pacer-preset = Preset:
pacer-mode = Mode:
pacer-pause = PAUSE
pacer-async = ASYNC
pacer-async-hint = Asynchronous (magnet) mode: sensing off, fixed-rate pacing.
pacer-rate = rate ppm
pacer-atrium = Atrium
pacer-a-output = A output mA
pacer-a-sensitivity = A sensitivity mV
pacer-ventricle = Ventricle
pacer-v-output = V output mA
pacer-v-sensitivity = V sensitivity mV
pacer-av-delay = AV delay ms
pacer-upper-rate = upper rate ppm
pacer-pvarp = PVARP ms
pacer-leads = Leads
pacer-a-lead = A lead
pacer-v-lead = V lead
pacer-lv-lead-biv = LV lead (BiV)
pacer-lv-lead = LV lead
pacer-vv-offset = VV offset ms (− = LV first)
pacer-thresholds = Thresholds now: A { $atrial } mA, V { $ventricular } mA
pacer-sense = { $channel } sense
pacer-pace = { $channel } pace
pacer-battery = battery

## Domain names (labels.rs)
branch-left-main = Left main
branch-lad = Left anterior descending
branch-lcx = Left circumflex
branch-obtuse-marginal = Obtuse marginal
branch-posterolateral = Posterolateral
branch-rca = Right coronary
branch-pda = Posterior descending
branch-global = Global subendocardial
branch-pericardium = Pericardium (diffuse)
preset-anterior-stemi = Anterior STEMI (LAD)
preset-inferior-stemi = Inferior STEMI (RCA)
preset-lateral-stemi = Lateral STEMI (OM)
preset-posterior-stemi = Posterior STEMI (PL)
preset-subendocardial = Subendocardial ischaemia
rhythm-vf = VF
rhythm-vt = VT
rhythm-svt = SVT
rhythm-af-flutter = AF / flutter
rhythm-organised = organised rhythm
rhythm-asystole = asystole
limb-right-arm = right wrist
limb-left-arm = left wrist
limb-left-leg = left ankle
limb-right-leg = right ankle
site-ra-appendage = RA appendage
site-rv-apex = RV apex
site-rv-septum = RV septum
site-his-bundle = His bundle
site-lv-lateral = LV lateral (CS)
site-lv-posterior = LV posterior (CS)
region-sinoatrial-node = SA node
region-right-atrium = Right atrium
region-left-atrium = Left atrium
region-crista-terminalis = Crista terminalis
region-bachmann = Bachmann's bundle
region-atrial-septum = Atrial septum
region-cavotricuspid-isthmus = Cavotricuspid isthmus
region-la-appendage = LA appendage
region-coronary-sinus = Coronary sinus
region-pulmonary-veins = Pulmonary vein sleeves
region-av-fast = AV node – fast pathway
region-av-slow = AV node – slow pathway
region-av-compact = AV node – compact
region-his-bundle = Bundle of His
region-right-bundle-branch = Right bundle branch
region-left-bundle-branch = Left bundle branch
region-left-anterior-fascicle = Left anterior fascicle
region-left-posterior-fascicle = Left posterior fascicle
region-purkinje-right = Purkinje (right)
region-purkinje-left = Purkinje (left)
region-right-ventricle = Right ventricle
region-left-ventricle = Left ventricle
region-ventricular-septum = Ventricular septum
region-scar = Scar
region-border-zone = Border zone
region-ischaemic = Ischaemic tissue
region-accessory-pathway = Accessory pathway

## Electrode editor (electrode_editor.rs)
electrode-off = off
electrode-on-limb = on the { $limb }
electrode-standard = { $place } (standard)
electrode-presets = Presets
electrode-fit = Fit leads
electrode-fit-hint = Put every electrode back on its standard site.
electrode-extra-leads = Extra leads
electrode-exercises = Exercises
electrode-new-exercise = New exercise
electrode-choose = Choose
electrode-explain = Explain
electrode-exercise-prompt = Something is wrong with the electrodes. Read the 12-lead, decide what, then press Explain.
electrode-exercise-name = Exercise: { $name }.
electrode-detector = Detector
electrode-recording = Recording with the new placement…
electrode-matches-standard = The leads match the standard placement.
electrode-electrodes = Electrodes
electrode-snap = Snap to intercostal spaces and chest lines
electrode-last-snap = Last snap: { $landmark }
electrode-back-on-limb = Back on the { $limb }
electrode-back-standard = Back to the standard site
electrode-drag-hint = Drag it on the torso to move it.
electrode-swap-and = and
electrode-swap-cables = cables
electrode-swap = Swap

## Timeline editor (timeline_editor.rs)
timeline-time = t (ms)
timeline-kind = Kind
timeline-action = Action (scenario syntax)
timeline-delete = Delete
timeline-new-action = New action:
timeline-at = at
timeline-ms = ms
timeline-add = Add
timeline-hint = Scenario timeline: one tick per action, red = now. Click a tick to select its row.

## Transport controls (transport.rs)
transport-run = Run
transport-pause = Pause
transport-speed = speed

## Colour bar and defibrillator button
colour-min = min
colour-max = max
defib-shock = SHOCK
ecg-speed = Paper speed
ecg-gain = Gain
preset-pericarditis = Pericarditis
preset-old-anterior = Old anterior infarct
preset-old-inferior = Old inferior infarct

## Axis dial polarity (axis_dial.rs)
polarity-positive = positive
polarity-negative = negative
polarity-equiphasic = equiphasic

## Ischaemia panel (ischaemia_panel.rs). ischaemia and infarct are the standard clinical terms.
ischaemia-coronary-tree = Coronary tree
ischaemia-selected = Selected: { $branch } ({ $id })
ischaemia-severity = Severity
ischaemia-severity-hint = 0 is an open artery, 1 is fully blocked.
ischaemia-infarct = Infarct
ischaemia-apply = Apply
ischaemia-reperfuse = Reperfuse
ischaemia-clear-all = Clear all
ischaemia-presets = Presets
ischaemia-in-effect = In effect
ischaemia-acute = acute ischaemia
ischaemia-infarct-kind = infarct
ischaemia-lesion-line = { $branch }: { $kind }, severity { $severity }
