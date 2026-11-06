# Widget heartsim-ui: striscia, pagina a 12 derivazioni, cursori, pannelli dei parametri.
# Traduzione italiana: da verificare con un clinico madrelingua prima della distribuzione.

## Controlli della carta ECG (ecg_view.rs)
ecg-monitor-colours = Colori monitor
ecg-grid = Griglia
ecg-waves = Onde
ecg-intervals = PR/RR
ecg-px-per-mm = pt/mm
ecg-calibration-hint = Calibrazione dello schermo: appoggia un righello alle caselle da 5 mm della griglia e regola finché misurano 5 mm.
ecg-calipers = Cursori ✕
ecg-calipers-hint = Maiusc+clic su una striscia per aggiungere una coppia di cursori; trascina le linee; ←/→ per spostarle; Esc cancella.
ecg-scrub-back = s indietro
ecg-saved = Salvato { $path }
ecg-snapshot-failed = Salvataggio dell'immagine non riuscito: { $error }

## Striscia e pagina a 12 derivazioni (strip.rs, twelve_lead.rs, vector_view.rs)
sweep-scroll = Scorrimento
sweep-sweep = Spazzata
layout-12x1 = 12 × 1
layout-6x2 = 6 × 2
layout-3x4 = 3 × 4 + ritmo
plane-frontal = Piano frontale
plane-horizontal = Piano orizzontale
vector-ring-scale = anello = { $value } mV

## Lettura degli intervalli (annotations.rs). HR, RR, PR, QRS, QT e QTc sono abbreviazioni standard e restano invariate.
readout-measuring = Misura in corso…
readout-no-signal = Nessun segnale
readout-no-qrs = Nessun complesso QRS rilevato
readout-rhythm = Ritmo
readout-regular = regolare
readout-irregular = irregolare
readout-no-p = onde P assenti
readout-qrs-axis = Asse QRS
readout-t-axis = Asse T
axis-normal = normale
axis-left = deviazione sinistra
axis-right = deviazione destra
axis-extreme = estremo

## Defibrillatore (defib_panel.rs)
defib-energy = Energia
defib-sync-button = SYNC
defib-sync-hint = Cardioversione sincronizzata: la scarica viene erogata sulla successiva onda R (TV con polso, TSV, FA, flutter). Disattivata per la FV.
defib-charge = CARICA
defib-disarm = Disarma
defib-idle = inattivo
defib-charging = carica in corso { $percent } %
defib-charged = carica a { $joules } J
defib-sync-waiting = SYNC: in attesa di un'onda R…
defib-sync = sincronizzata
defib-unsync = non sincronizzata
defib-terminated = aritmia interrotta
defib-no-effect = nessun effetto
defib-shock-line = Scarica da { $joules } J { $mode } a { $at } s su { $rhythm } (p ≈ { $probability } %): { $outcome }
defib-vulnerable = erogata nell'onda T (periodo vulnerabile)

## Pompa farmaci (drug_tray.rs)
drug-dose-one = Dose 1 = { $dose }
drug-stop = Interrompi
drug-dose-given = Dose somministrata { $dose } × standard, { $ago } fa; effetto { $effect } % di una dose standard a effetto pieno.

## Pannello anatomico (anatomy_panel.rs)
anatomy-layers = Strati
anatomy-opacity = opacità
anatomy-labels = Etichette
anatomy-cut-away = Sezione
anatomy-depth = profondità
anatomy-view = Vista
anatomy-fit = Adatta
anatomy-oblique = obliquità
anatomy-auto-rotate = Rotazione automatica

## Pacemaker (pacer_panel.rs)
pacer-title = Pacemaker
pacer-connected = Collegato
pacer-preset = Preset:
pacer-mode = Modalità:
pacer-pause = PAUSA
pacer-async = ASINC
pacer-async-hint = Modalità asincrona (magnete): sensing disattivato, stimolazione a frequenza fissa.
pacer-rate = frequenza ppm
pacer-atrium = Atrio
pacer-a-output = uscita A mA
pacer-a-sensitivity = sensibilità A mV
pacer-ventricle = Ventricolo
pacer-v-output = uscita V mA
pacer-v-sensitivity = sensibilità V mV
pacer-av-delay = ritardo AV ms
pacer-upper-rate = frequenza massima ppm
pacer-pvarp = PVARP ms
pacer-leads = Elettrocateteri
pacer-a-lead = elettrocatetere A
pacer-v-lead = elettrocatetere V
pacer-lv-lead-biv = elettrocatetere VS (BiV)
pacer-lv-lead = elettrocatetere VS
pacer-vv-offset = offset VV ms (− = prima VS)
pacer-thresholds = Soglie attuali: A { $atrial } mA, V { $ventricular } mA
pacer-sense = sensing { $channel }
pacer-pace = stimolo { $channel }
pacer-battery = batteria

## Nomi di dominio (labels.rs)
branch-left-main = Tronco comune sinistro
branch-lad = Discendente anteriore sinistra (IVA)
branch-lcx = Circonflessa (CX)
branch-obtuse-marginal = Marginale ottusa
branch-posterolateral = Posterolaterale
branch-rca = Coronaria destra (CD)
branch-pda = Discendente posteriore (DP)
branch-global = Subendocardio globale
branch-pericardium = Pericardio (diffuso)
preset-anterior-stemi = STEMI anteriore (IVA)
preset-inferior-stemi = STEMI inferiore (CD)
preset-lateral-stemi = STEMI laterale (MO)
preset-posterior-stemi = STEMI posteriore (PL)
preset-subendocardial = Ischemia subendocardica
rhythm-vf = FV
rhythm-vt = TV
rhythm-polymorphic-vt = TV polimorfa
rhythm-flutter = flutter
rhythm-svt = TSV
rhythm-af-flutter = FA / flutter
rhythm-organised = ritmo organizzato
rhythm-asystole = asistolia
limb-right-arm = polso destro
limb-left-arm = polso sinistro
limb-left-leg = caviglia sinistra
limb-right-leg = caviglia destra
site-ra-appendage = appendice atriale destra
site-rv-apex = apice del VD
site-rv-septum = setto del VD
site-his-bundle = fascio di His
site-lv-lateral = VS laterale (seno coronarico)
site-lv-posterior = VS posteriore (seno coronarico)
region-sinoatrial-node = Nodo del seno (SA)
region-right-atrium = Atrio destro
region-left-atrium = Atrio sinistro
region-crista-terminalis = Cresta terminale
region-bachmann = Fascio di Bachmann
region-atrial-septum = Setto interatriale
region-cavotricuspid-isthmus = Istmo cavotricuspidale
region-la-appendage = Auricola sinistra
region-coronary-sinus = Seno coronarico
region-pulmonary-veins = Manicotti delle vene polmonari
region-av-fast = Nodo AV, via veloce
region-av-slow = Nodo AV, via lenta
region-av-compact = Nodo AV, compatto
region-his-bundle = Fascio di His
region-right-bundle-branch = Branca destra
region-left-bundle-branch = Branca sinistra
region-left-anterior-fascicle = Fascicolo anteriore sinistro
region-left-posterior-fascicle = Fascicolo posteriore sinistro
region-purkinje-right = Purkinje (destra)
region-purkinje-left = Purkinje (sinistra)
region-right-ventricle = Ventricolo destro
region-left-ventricle = Ventricolo sinistro
region-ventricular-septum = Setto ventricolare
region-scar = Cicatrice
region-border-zone = Zona di confine
region-ischaemic = Tessuto ischemico
region-accessory-pathway = Via accessoria

## Editor degli elettrodi (electrode_editor.rs)
electrode-off = scollegato
electrode-on-limb = sul { $limb }
electrode-standard = { $place } (standard)
electrode-presets = Preimpostazioni
electrode-fit = Ripristina
electrode-fit-hint = Riporta ogni elettrodo nella sua sede standard.
electrode-extra-leads = Derivazioni extra
electrode-exercises = Esercizi
electrode-new-exercise = Nuovo esercizio
electrode-choose = Scegli
electrode-explain = Spiega
electrode-exercise-prompt = Qualcosa non va negli elettrodi. Leggi il 12 derivazioni, individua il problema e premi Spiega.
electrode-exercise-name = Esercizio: { $name }.
electrode-detector = Rilevatore
electrode-recording = Registrazione con la nuova posizione…
electrode-matches-standard = Le derivazioni corrispondono alla posizione standard.
electrode-electrodes = Elettrodi
electrode-snap = Aggancia agli spazi intercostali e alle linee toraciche
electrode-last-snap = Ultimo aggancio: { $landmark }
electrode-back-on-limb = Riporta sul { $limb }
electrode-back-standard = Riporta alla sede standard
electrode-drag-hint = Trascinalo sul torace per spostarlo.
electrode-swap-and = e
electrode-swap-cables = cavi
electrode-swap = Scambia

## Editor della linea temporale (timeline_editor.rs)
timeline-time = t (ms)
timeline-kind = Tipo
timeline-action = Azione (sintassi dello scenario)
timeline-delete = Elimina
timeline-new-action = Nuova azione:
timeline-at = a
timeline-ms = ms
timeline-add = Aggiungi
timeline-hint = Linea temporale dello scenario: un segno per azione, rosso = ora. Clicca un segno per selezionare la sua riga.

## Controlli di trasporto (transport.rs)
transport-run = Avvia
transport-pause = Pausa
transport-speed = velocità

## Barra dei colori e pulsante di scarica
colour-min = min
colour-max = max
defib-shock = SCARICA
ecg-speed = Velocità carta
ecg-gain = Guadagno
preset-pericarditis = Pericardite
preset-old-anterior = Vecchio infarto anteriore
preset-old-inferior = Vecchio infarto inferiore

## Polarità sulla rosa degli assi (axis_dial.rs)
polarity-positive = positiva
polarity-negative = negativa
polarity-equiphasic = isodifasica

## Pannello ischemia (ischaemia_panel.rs). Termini clinici standard: ischemia, infarto.
ischaemia-coronary-tree = Albero coronarico
ischaemia-selected = Selezionato: { $branch } ({ $id })
ischaemia-severity = Gravità
ischaemia-severity-hint = 0 è un'arteria aperta, 1 è completamente occlusa.
ischaemia-infarct = Infarto
ischaemia-apply = Applica
ischaemia-reperfuse = Ripristina il flusso
ischaemia-clear-all = Azzera tutto
ischaemia-presets = Preimpostazioni
ischaemia-in-effect = In atto
ischaemia-acute = ischemia acuta
ischaemia-infarct-kind = infarto
ischaemia-evolving-kind = infarto in evoluzione
ischaemia-lesion-line = { $branch }: { $kind }, gravità { $severity }
