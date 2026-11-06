# heartsim-app: schede, barra degli strumenti, addestratore dell'asse, quiz, guida.
# Il nome del programma e le abbreviazioni (ECG, AV, QRS) restano invariati.

## Schede
tab-simulator = Simulatore
tab-12-lead = ECG a 12 derivazioni
tab-heart-vector = Vettore cardiaco
tab-axis-trainer = Addestratore dell'asse
tab-axis-quiz = Quiz sull'asse
tab-electrodes = Elettrodi
tab-rhythm-quiz = Quiz sul ritmo
tab-tutorials = Tutorial
tab-help = Guida

## Barra degli strumenti
app-av-block = Blocco AV
app-sa-rate = Frequenza SA bpm
app-mobitz = Mobitz II n
app-axis-shift = spostamento asse °
app-qrs-width = Ampiezza QRS
app-pacemaker = Pacemaker
app-therapy = Terapia
app-frontal-axis = Asse QRS frontale
app-net-polarity = Polarità netta QRS:
app-layout = Disposizione:
app-save-png = Salva PNG

## Addestratore dell'asse
app-axis-trainer-hint = Clicca o trascina sul cerchio per ruotare l'asse elettrico del cuore.
app-reset-axis = Ripristina asse normale
app-net-qrs-each = QRS netto in ciascuna derivazione
app-axis-normal = Derivazioni I e aVF entrambe positive (o II positiva): normale.
app-axis-left = Derivazione I positiva, aVF negativa: deviazione sinistra dell'asse.
app-axis-right = Derivazione I negativa, aVF positiva: deviazione destra dell'asse.
app-axis-extreme = I e aVF entrambe negative: asse estremo (nord-ovest).

## Quiz sull'asse
app-quiz-new = Nuova domanda
app-quiz-check = Verifica risposta
app-quiz-score = Punteggio { $correct }/{ $asked }
app-quiz-intro = Premi Nuova domanda, leggi le sei derivazioni degli arti, poi clicca sul cerchio l'asse.
app-quiz-click = Clicca sul cerchio il punto in cui pensi punti l'asse QRS.
app-quiz-answer = Risposta { $answer }°, hai scelto { $chosen }° (errore di { $off }°)
app-quiz-your-answer = La tua risposta: { $chosen }°. Premi Verifica risposta.

## Guida
help-intro = Un simulatore didattico aperto di elettrofisiologia cardiaca e di ECG.
help-tabs-heading = Schede
help-tab-simulator = Simulatore: il modello del cuore con una derivazione selezionabile e l'asse in tempo reale.
help-tab-12-lead = ECG a 12 derivazioni: tutte le dodici derivazioni insieme (12 × 1, 6 × 2 oppure la disposizione clinica 3 × 4 con striscia del ritmo) accanto al cerchio dell'asse.
help-tab-axis-trainer = Addestratore dell'asse: trascina sul cerchio per ruotare l'asse e osserva come cambia ogni derivazione degli arti.
help-tab-heart-vector = Vettore cardiaco: il dipolo cardiaco nel cursore della striscia e il suo loop sull'ultimo battito, sui piani frontale e orizzontale.
help-tab-axis-quiz = Quiz sull'asse: un asse nascosto viene impostato; leggi le derivazioni e clicca la tua risposta.
help-tab-tutorials = Tutorial: lezioni guidate che pilotano il simulatore pagina per pagina, e un editor per la linea temporale degli interventi di un ritmo.
help-tab-rhythm-quiz = Quiz sul ritmo: riconosci il ritmo di una striscia della libreria di scenari; tra le risposte ci sono i ritmi con cui viene confuso più spesso.
help-rhythms-heading = Ritmi e scorrimento
help-rhythms-text = Scegli un ritmo dall'elenco Ritmo sopra la striscia: ECG e cuore 3D seguono entrambi quel ritmo. Metti in pausa e trascina il cursore 's indietro': cuore, freccia del vettore e striscia mostrano lo stesso istante.
help-axis-heading = Come trovare l'asse
help-axis-text = L'asse normale va da −30° a +90°. Derivazioni I e aVF entrambe positive: normale. I positiva, aVF negativa: deviazione sinistra (oltre −30° se anche II è negativa). I negativa, aVF positiva: deviazione destra. Entrambe negative: asse estremo. La derivazione degli arti isoelettrica è perpendicolare all'asse.
help-keys-heading = Scorciatoie da tastiera
help-keys-space = Spazio: mette in pausa o riprende la simulazione.
help-keys-arrows = ← / →: in pausa, sposta il cursore di scorrimento indietro o avanti. Se è attivo un cursore di misura, le frecce lo spostano.
help-keys-speed = + / −: aumenta o diminuisce la velocità della simulazione e della carta.
help-keys-tabs = 1 – 9: apre una scheda, nell'ordine mostrato nella barra in alto.
help-keys-note = Le scorciatoie non funzionano mentre una casella di testo ha il focus della tastiera.
help-disclaimer = Strumento didattico, non destinato all'uso clinico.

## Finestra Impostazioni (settings.rs)
settings-button = Impostazioni
settings-title = Impostazioni
settings-language = Lingua
settings-language-system = Predefinita del sistema
settings-high-contrast = Tema ad alto contrasto
settings-reduced-motion = Movimento ridotto (niente rotazione automatica né lampeggio della scarica)
settings-hint = Le modifiche hanno effetto subito e vengono ricordate su questo computer.

## Finestra Esporta / Importa (app/exports.rs)
export-title = Esporta / Importa
export-reviewing = Revisione di { $label }
export-heading = Esporta
export-length = Durata
export-seconds = s
export-pdf-paper = Formato carta PDF
export-formats = Formati
export-scenario = Scenario
export-bundle = Pacchetto condiviso (.zip)
export-folder = Cartella
export-settings-summary = { $speed } mm/s, { $gain } mm/mV e ordine delle derivazioni { $order } della striscia; termina sul cursore.
export-button = Esporta
export-live-hint = Torna al vivo per esportare: una registrazione in revisione non è una simulazione.
export-screenshot = Immagine della finestra
export-screenshot-hint = PNG dell'intera finestra, vista 3D inclusa.
import-heading = Importa per la revisione
import-hint = percorso .csv, .hea, .zip o .ron
import-button = Importa
import-drop-hint = Oppure trascina un file sulla finestra. Striscia e pagina a 12 derivazioni lo mostrano; i cursori funzionano come sempre.
export-review-length = Revisione di { $label } ({ $seconds } s)
export-back-live = Torna al vivo
export-failed = Esportazione non riuscita: { $error }
export-saved = Salvato { $path }
export-screenshot-failed = Immagine non riuscita: { $error }
import-failed = Importazione non riuscita: { $error }
import-failed-path = Importazione non riuscita: { $path }: { $error }
export-running = In esecuzione: { $title }
export-closed = Chiuso { $label }
import-reviewing = Revisione di { $label }: { $seconds } s a { $rate } Hz

## Vettore cardiaco, terapia e linea temporale (app/vector_tab.rs, app/therapy.rs, app/tutorials.rs)
vector-heading = Vettore cardiaco
vector-intro = La freccia è il dipolo cardiaco nel cursore della striscia, ricostruito da tutte le dodici derivazioni. Il loop è il percorso della sua punta nell'ultimo battito completo (dal blu al rosa).
vector-x = x { $value }  ({ $side })
vector-y = y { $value }  ({ $side })
vector-z = z { $value }  ({ $side })
vector-left = sinistra
vector-cranial = craniale
vector-anterior = anteriore
vector-scrub-hint = Metti in pausa e scorri la striscia per far avanzare la freccia tra i loop P, QRS e T.
therapy-drugs = Farmaci
therapy-ischaemia = Ischemia
therapy-defib = Defibrillatore
timeline-editor-mode = Editor della linea temporale dello scenario
timeline-load-first = Carica un ritmo (scheda Simulatore, elenco Ritmo) per modificarne la linea temporale.
timeline-row-error = Una riga non è valida (passa il mouse sopra per l'errore).
timeline-apply = Applica e riavvia
timeline-apply-hint = Usa questa linea temporale per lo scenario in corso (solo per questa sessione).
timeline-revert = Ripristina
timeline-file = File:
timeline-save = Salva scenario
timeline-not-saved = Non salvato: { $error }
timeline-save-desktop-only = Il salvataggio dei file di scenario è disponibile solo nell’app desktop.

## Vista del cuore (heart_view.rs)
colour-voltage = Voltaggio
colour-activation = Tempo di attivazione
colour-apd = APD
part-lv = Ventricolo sinistro
part-rv = Ventricolo destro
part-la = Atrio sinistro
part-ra = Atrio destro
part-aorta = Aorta
part-pulmonary-artery = Arteria polmonare
part-pulmonary-veins = Vene polmonari
part-svc = Vena cava superiore
part-ivc = Vena cava inferiore
part-valve-rings = Anelli valvolari
part-torso = Torace
heart-ablated = Ablazione di { $radius } mm attorno a { $region }
heart-stimulus = Stimolo: { $region } a { $time } ms
heart-ablation-cleared = Lesioni da ablazione rimosse
heart-tier1 = Attivazione 3D (Livello 1)
heart-tier1-hint = Disattivata: solo striscia ECG (Livello 0); il cuore è disegnato senza mappa di colori.
heart-reset-window = Ripristina finestra
heart-colour-blind = Adatta al daltonismo
heart-wavefront = Contorno del fronte d'onda
heart-click = Clicca sul cuore
heart-stimulate = Stimola
heart-ablate = Ablazione
heart-lesion-radius = raggio della lesione mm
heart-shift-click = Maiusc+clic per ablare.
heart-clear-ablation = Cancella ablazioni
heart-territory = Territorio: { $territory }
heart-activated = Attivato a { $time } ms in questo battito
heart-not-activated = Non ancora attivato in questo battito
heart-apd = APD { $value } ms

## Riscontro comune
feedback-correct = Corretto.

## Quiz sul ritmo (rhythm_quiz.rs)
rq-streak = serie { $count }
rq-twelve-lead = Mostra tutte le 12 derivazioni
rq-no-library = Libreria di scenari non trovata (assets/scenarios).
rq-intro = Premi Nuova domanda, leggi la striscia, poi scegli il ritmo.
rq-correct-choice = { $title }  (corretto)
rq-your-choice = { $title }  (la tua risposta)
rq-wrong = Non esatto: questo è { $title }.
rq-why = Perché
rq-open = Apri nel simulatore

## Lettore dei tutorial (tutorial_ui.rs)
tut-mode = Modalità:
tut-self-study = Studio autonomo
tut-self-study-hint = Pagine in ordine; rispondi a ogni verifica prima di proseguire.
tut-tutor = Docente
tut-tutor-hint = Navigazione libera per presentare a un gruppo; le verifiche sono facoltative.
tut-none = Nessun tutorial installato (assets/tutorials).
tut-completed = completato
tut-pages = { $count } pagine
tut-start = Inizia { $title }
tut-resume = Riprendi da pagina { $page }
tut-close = Chiudi tutorial
tut-page-of = Pagina { $page } di { $total }
tut-contents = Indice
tut-passed = (superata)
tut-check = Verifica la tua comprensione
tut-try-again = Non esatto. Riprova.
tut-previous = Indietro
tut-next = Avanti
tut-finish = Fine
tut-answer-first = Prima rispondi alla domanda.
tut-replay = Ripeti
tut-replay-hint = Esegui di nuovo le azioni del simulatore di questa pagina.
tut-running = in corso…

## Casi clinici (case_ui.rs)
case-heading = Casi clinici
case-intro = Brevi storie guidate: leggi la scheda, osserva l'ECG che mostra il simulatore, rispondi alle domande.
case-none = Nessun caso installato (assets/cases).
case-best = migliore { $correct } / { $total }
case-size = { $stages } fasi · { $questions } domande
case-start = Inizia { $title }
case-close = Chiudi caso
case-score = { $correct } corrette su { $answered } risposte (su { $total })
case-finished = Caso concluso
case-final-score = Hai risposto correttamente al primo tentativo a { $correct } domande su { $total }.
case-first-try-hint = Conta solo la prima risposta a ogni domanda. Ricomincia il caso per migliorare il punteggio.
case-stage-of = Fase { $stage } di { $total }: { $title }
case-question-of = Domanda { $question } di { $total }
case-wrong = Non esatto. La risposta è: { $answer }
case-finish = Concludi il caso
case-continue = Continua
case-answer-first = Prima rispondi alla domanda.

## Overlay del vettore cardiaco (vector_overlay.rs)
vec-arrow = Freccia
vec-loop = Loop (ultimo battito)
vec-see-through = Cuore trasparente
vec-see-through-hint = Rende il cuore traslucido, così si vede il vettore al suo interno.
vec-scale = mm per mV

## Scheda elettrodi (electrode_tab.rs)
el-save = Salva posizione
el-saved = Salvato { $file }
el-save-failed = Salvataggio non riuscito: { $error }
el-load = Carica posizione
el-loaded = Caricato { $file }
el-load-failed = Caricamento non riuscito: { $error }
el-no-3d = Vista 3D non disponibile: modifica gli elettrodi dal pannello.
el-hidden = Elettrodi nascosti durante l'esercizio: leggi l'ECG.
el-drag-hint = Trascina un elettrodo sul torace; trascina altrove per ruotare.

## Pannello di esportazione e selettore di scenari (export_panel.rs, scenarios.rs)
export-wrote = Scritto { $names } in { $dir }
export-nothing = Nessun file scritto
scenario-custom = Personalizzato (cursori)
scenario-rhythm = Ritmo:
scenario-restart = Riavvia
scenario-restart-hint = Ricarica lo scenario da t = 0.
scenario-about = Info
rq-next = Domanda successiva
rq-question = Qual è il ritmo?
rq-answer = Risposta:
el-save-hint = Scrive uno scenario con questi elettrodi in { $file }

## Preset di rientro (app/reentry.rs)
reentry-picker = Preset di rientro…
reentry-tier1 = Livello 1
reentry-tier2 = Livello 2 · { $count } singolarità di fase · quiete { $quiet } s
reentry-speed = { $tier } · { $speed }× tempo reale
reentry-speed-hint = Il livello 2 non funziona in tempo reale su tutte le macchine; la simulazione procede alla massima velocità possibile.
reentry-stop = Ferma
