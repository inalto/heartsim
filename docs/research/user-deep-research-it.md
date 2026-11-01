# Ricerca approfondita: simulatore 3D fisiologicamente accurato del cuore con ECG e propagazione elettrica

## Sintesi esecutiva

Un simulatore cardiaco 3D “completo” non è un singolo modello, ma una catena multiscala: anatomia e fibre → elettrofisiologia cellulare → propagazione nel tessuto → sistema di conduzione → sorgenti extracardiache → forward model torso/ECG → visualizzazione e interazione. I framework di riferimento confermano questa separazione: openCARP copre modelli cellulari, monodominio/bidominio, eikonal, ECG/lead fields e strumenti di pre/post-processing; Chaste offre elettrofisiologia continua, bidominio/monodominio ed elettromeccanica; life^x-ep, sviluppato nell'ecosistema iHEART del Politecnico di Milano, implementa monodominio FEM, fibre e simulazioni HPC; MonoAlg3D è orientato a monodominio FVM e accelerazione GPU. [1]

La scelta più equilibrata per un progetto nuovo è non riscrivere subito il solutore elettrofisiologico. Conviene usare openCARP come riferimento/solver scientifico iniziale, affiancare un formato dati standard VTK/HDF5, ParaView per la validazione visiva e costruire separatamente un frontend interattivo. Per simulazioni ad alte prestazioni o studi parametrici, MonoAlg3D è oggi particolarmente interessante perché implementa monodominio FVM su GPU e MPI; uno studio del 2026 riporta un'accelerazione di circa 10,9× rispetto alla soluzione CPU considerata e simulazioni biventricolari da ~0,86 a ~6,85 milioni di volumi, con tempi da meno di 24 a circa 303 minuti per un battito con un modello cellulare umano a 43 variabili. [2]

Per la fisiologia, suggerisco una gerarchia a tre livelli. Il livello “scientifico” dovrebbe usare monodominio + modello ionico umano dettagliato — per esempio O'Hara–Rudy per ventricoli, Courtemanche per atri — più fibre anisotrope e sistema His–Purkinje. Il livello “rapido” dovrebbe usare reaction-eikonal/eikonal oppure un modello fenomenologico come Aliev–Panfilov/Bueno-Orovio. Il bidominio va riservato a problemi in cui il campo extracellulare modifica significativamente l'eccitazione, alla stimolazione extracellulare o a forward model ECG ad alta fedeltà, perché richiede la soluzione aggiuntiva di un problema ellittico ed è sensibilmente più costoso. openCARP descrive il bidominio come riferimento più completo per il campo extracellulare e indica che può costare circa un ordine di grandezza più del monodominio in scenari tipici; il pseudo-bidominio/lead-field offre un compromesso molto più economico. [3]

Un punto importante riguarda la richiesta di “visualizzare il movimento delle cariche”. Nei modelli di tessuto cardiaco standard non si simulano particelle di carica che viaggiano macroscopicamente nel miocardio: si calcolano soprattutto potenziale transmembrana , potenziale extracellulare , correnti ioniche, densità di corrente e tempi di attivazione. Una visualizzazione fisicamente corretta dovrebbe quindi mostrare fronti di depolarizzazione, mappe di , isocrone, vettori di corrente e — se desiderato — correnti ioniche specifiche, invece di animare “elettroni che scorrono” lungo il cuore. Questa rappresentazione corrisponde alla formulazione mono/bidominio usata nei simulatori cardiaci correnti. [4]

Per arrivare a una piattaforma di ricerca credibile, stimerei 12–18 mesi con 3–5 persone per una prima versione completa e validata basata su solutori esistenti, e 24–36+ mesi se si vuole sviluppare anche il solver PDE/HPC proprietario, elettromeccanica, personalizzazione da immagini cliniche e real-time avanzato. Sono stime progettuali mie, non benchmark pubblicati. L'hardware minimo sensato è una workstation con 64–128 GB RAM e GPU NVIDIA/AMD moderna; per modelli 3D sub-millimetrici, ensemble o bidominio è molto utile un nodo da 256–512 GB RAM e una o più GPU da 24–80 GB, oppure un cluster HPC.

## Componenti fisiologici e modelli da implementare

### Anatomia tridimensionale e microstruttura

Il dominio anatomico dovrebbe distinguere almeno atrio destro/sinistro, ventricolo destro/sinistro, setti, anelli valvolari, grandi vasi prossimali e, se si vuole un ECG realistico, un torso con polmoni e possibilmente sangue, scheletro e altri compartimenti conduttivi. Per simulazioni di sola propagazione ventricolare si può iniziare da un modello biventricolare; per P-wave, conduzione AV e ciclo completo P–QRS–T serve un modello quattro camere. Esistono già geometrie pubbliche quattro camere pronte per elettrofisiologia ed elettromeccanica: il modello Fritz/Gerach disponibile via Zenodo include una mesh EP di circa 50 milioni di tetraedri con passo medio ~0,4 mm e fibre rule-based; esiste inoltre una coorte pubblica di 24 cuori quattro camere con mesh, etichette e fibre, e dal 2025 una coorte di 50 modelli patient-specific quattro camere derivati da CT. [5]

La geometria da imaging segue normalmente il percorso DICOM/NIfTI → segmentazione → superficie triangolare → correzione topologica → mesh volumetrica tetraedrica/esagonale → tagging anatomico → fibre. La microstruttura è critica perché la propagazione è fortemente anisotropa; life^x-fiber e le geometrie pubbliche citate usano metodi rule-based Laplace–Dirichlet per generare orientazioni fibre/sheet coerenti con la parete. [6]

Per un modello PDE accurato, letteratura e benchmark suggeriscono generalmente una risoluzione spaziale nell'ordine di 0,1–0,2 mm per risolvere adeguatamente il fronte d'onda con elementi lineari, anche se modelli anatomici completi spesso usano mesh più grosse per ragioni computazionali. Il benchmark Niederer e studi successivi hanno considerato 0,5, 0,2 e 0,1 mm e passi temporali nell'ordine di 0,05–0,005 ms; una review numerica segnala tipicamente 0,1–0,2 mm e 0,01–0,02 ms per simulazioni tissutali dettagliate. [7]

### Elettrofisiologia cellulare e canali ionici

Il modello cellulare fornisce la corrente ionica totale , dove  comprende gating variables, concentrazioni ioniche e stati intracellulari. I parametri richiesti comprendono conduttanze massime dei canali, potenziali di equilibrio o concentrazioni intra/extra-cellulari, cinetiche di gating, pompe/scambiatori, capacità di membrana, volumi cellulari e parametri del calcio. Modelli dettagliati permettono di simulare farmaci, channelopatie e rimodellamento; quelli fenomenologici sono molto più economici ma non permettono inferenze affidabili sui singoli canali. [8]

Per inizializzare correttamente un organo non basta porre tutte le cellule a riposo: è preferibile pre-pacing fino al limit cycle, eventualmente separato per regioni endocardiche, epicardiche, atriali e Purkinje. openCARP offre workflow espliciti per single-cell limit cycle, trasferimento cell-to-tissue e protocolli di calibrazione/restition. [15]

### Propagazione tissutale, fibre e sistema di conduzione

Il monodominio risolve una PDE parabolica di reaction-diffusion per  e assume, in sostanza, un rapporto di anisotropia compatibile fra domini intra- ed extracellulare; è il candidato principale per simulazione organ-level. Il bidominio tratta separatamente domini intra- ed extracellulare e aggiunge un problema ellittico; è più completo quando la distribuzione del campo extracellulare o la stimolazione elettrica sono centrali. openCARP implementa entrambi, oltre a eikonal, reaction-eikonal, EMI e modelli di rete. [16]

Per ciascun elemento della mesh serve un tensore di conducibilità costruito dalle direzioni fibra–sheet–normal e dalle conducibilità longitudinali/trasversali. I parametri vengono normalmente calibrati contro velocità di conduzione target piuttosto che copiati indiscriminatamente dalla letteratura, perché la velocità risultante dipende anche da mesh, modello ionico, discretizzazione e passo temporale; openCARP include un tutorial specifico per la calibrazione della conduction velocity. [17]

Il sistema His–Purkinje può essere rappresentato come una rete 1D accoppiata al miocardio 3D tramite Purkinje–muscle junctions. Sono disponibili algoritmi pubblici per generare reti Purkinje su superfici ventricolari, e studi recenti mostrano che MonoAlg3D integra rete Purkinje, calibrazione PMJ e monodominio GPU. Le simulazioni senza rete di conduzione possono produrre sequenze di attivazione e QRS meno fisiologiche. [18]

Per prototipi quasi real-time, l'alternativa migliore è eikonal/reaction-eikonal: si calcola il tempo di arrivo del fronte in funzione di una velocità anisotropa locale, evitando di integrare tutte le ODE e la PDE reaction-diffusion a ogni piccolo timestep. openCARP include eikonal e reaction-eikonal; la documentazione descrive l'equazione con un tensore di velocità ortotropo. [19]

### Forward model ECG

Un ECG 12 derivazioni richiede di trasformare l'attività cardiaca in potenziali nel torso e quindi campionarla nelle posizioni degli elettrodi. Le opzioni principali sono: bidominio heart+torso, pseudo-bidominio, boundary-element/volume-conductor, lead-field e formule di recovery in mezzo infinito. openCARP documenta esplicitamente questi approcci e segnala che il lead-field/BEM è più efficiente quando si vogliono molte osservazioni sulla superficie corporea, mentre la recovery puntuale è conveniente per pochi elettrodi. [20]

La pipeline consigliata è:

Il torso deve avere geometria e conducibilità dei compartimenti rilevanti e condizioni al contorno appropriate; in una prima versione si può usare un torso omogeneo e lead field precomputato, poi passare a compartimenti sangue/polmone/tessuto. ECGSIM è utile come riferimento indipendente: è un simulatore interattivo del rapporto fra sorgenti cardiache, ECG e body-surface potential maps e usa un equivalent surface source model. [21]

## Numerica, costi e strategie real-time

Una formulazione FEM standard usa elementi tetraedrici lineari e operator splitting fra reazione cellulare e diffusione. La parte ODE può essere integrata con Rush–Larsen per gating quasi lineari, Euler/Runge–Kutta, backward Euler o CVODE; la parte PDE usa sistemi lineari sparsi risolti con CG/GMRES e precondizionamento multigrid/AMG. Chaste utilizza FEM e PETSc, mentre MonoAlg3D utilizza FVM e GPU; life^x-ep usa FEM basato su deal.II/Trilinos. [22]

Un timestep fisso semplice può richiedere ~0,005–0,02 ms per i tratti rapidi dell'upstroke; adaptive/multirate stepping permette passi più grandi durante plateau e diastole. Studi di benchmarking hanno usato, per esempio, PDE step di 0,01 ms e ODE step più piccoli durante l'upstroke, e Chaste supporta CVODE come alternativa per velocità/accuratezza. [23]

In termini qualitativi di costo:

Le cifre qui sopra sono ordini di grandezza progettuali, non benchmark universali: il costo dipende fortemente da mesh, solver, modello ionico, I/O e hardware. Il solo confronto monodominio/bidominio è supportato dalla documentazione openCARP, che riporta un incremento tipico di circa un ordine di grandezza per il bidominio completo. [20]

Per GPU, le opzioni più concrete sono MonoAlg3D/CUDA, openCARP con backend Ginkgo/MLIR per hardware NVIDIA e AMD, oppure un proprio kernel CUDA/HIP/OpenCL. La versione corrente di openCARP dichiara accelerazione GPU via Ginkgo e MLIR; MonoAlg3D usa CUDA, FVM e MPI ed è rilasciato con licenza MIT. [24]

Una strategia real-time robusta consiste nel separare simulation truth da interactive surrogate. Il server scientifico genera campi ad alta fedeltà offline; il client usa eikonal, modelli ridotti, interpolazione temporale o surrogate neural/operator-learning per variazioni immediate di pacing/parametri. Il gruppo MOX/Politecnico di Milano ha già studiato operator learning per tempi di attivazione e ripolarizzazione come alternativa alle PDE complete, evidenziando il potenziale di forti risparmi computazionali. [25]

## Dati, software open source e visualizzazione

Le immagini sopra mostrano esempi utili per il design dell'interfaccia: mappe di , isocrone di attivazione, rete Purkinje e relazione fra propagazione 3D ed ECG. Una UI scientificamente corretta dovrebbe offrire almeno vista superficiale e cut-plane, volume/surface coloring di , isosurface del fronte, isocrone, fibre, Purkinje, elettrodi, ECG sincronizzato e selezione temporale. life^x, CARPentry/openCARP e SimCardio mostrano esempi pubblici di questi paradigmi visivi. [26]

### Dataset e mesh consigliati

Il Cardiac Atlas Project ospita dati CMR, contorni e modelli cardiaci; il Sunnybrook Cardiac Dataset è indicato come public domain e include 45 studi con contorni e modelli FEM 3D. CAP ospita anche dataset atriali, congeniti e mesh biventricolari. [27]

Per partire immediatamente con simulazioni 3D suggerirei, in ordine: il modello quattro camere Fritz/Gerach per un singolo cuore ad alta risoluzione; la coorte Rodero/Strocchi da 24 cuori per test di variabilità; la coorte quattro camere da 50 modelli pubblicata nel 2025 per studi più ampi; e il dataset InSilicoHeartGen 2026 se l'obiettivo è MonoAlg3D, perché fornisce direttamente mesh, fibre, coordinate e file di configurazione per quel solver. [28]

### Framework a confronto

CARPentry va considerato soprattutto come materiale storico/tutoriale: il progetto stesso raccomanda ai nuovi utenti di passare a openCARP, mantenendo compatibilità con molti workflow precedenti. [39]

### Formati dati

Per imaging clinico: DICOM per CT/MRI raw e NIfTI per segmentazioni volumetriche. Per geometrie e risultati: VTK/VTU/VTP, Ensight e i formati openCARP; le coorti pubbliche citate forniscono già VTK/VTU/Ensight in vari casi. Per serie temporali molto grandi è opportuno aggiungere HDF5/XDMF, evitando migliaia di file VTK separati. Il dataset Cardiac Atlas biv-me, ad esempio, distribuisce DICOM e mesh .vtk, mentre le geometrie openCARP usano formati propri e possono essere visualizzate con meshalyzer/ParaView. [40]

Per il frontend, esporterei una mesh decimata in glTF/GLB più stream binari di campi scalari; il calcolo rimane sulla mesh scientifica ad alta risoluzione e la visualizzazione usa un'identica parametrizzazione o una mappa nodo→render-vertex. Questo evita di caricare decine di milioni di tetraedri in Unity/WebGL.

## Architettura implementativa

La separazione fra motore scientifico, storage e renderer permette di cambiare solver senza riscrivere l'applicazione, e riflette il modo in cui openCARP usa un core C++ più carputils Python e strumenti separati di visualizzazione. [41]

Nel backend, suggerisco una API astratta come Geometry → TissueModel → CellularModel → Stimulus → Solver → ForwardECG → Results. La configurazione deve essere serializzabile in YAML/JSON e contenere versione solver, hash della mesh, modello cellulare, parametri, timestep, solver lineare, tolleranze, hardware e seed. Questo è fondamentale per riproducibilità e confronti fra versioni; openCARP è esplicitamente progettato attorno a esperimenti riproducibili e bundle carputils. [42]

Le condizioni al contorno standard per un cuore isolato monodominio sono tipicamente no-flux/Neumann omogenee sulla superficie del miocardio; la formulazione openCARP riporta esplicitamente questa condizione per il monodominio. Per il bidominio heart–torso si devono invece imporre continuità del potenziale/corrente alle interfacce e una condizione di riferimento/gauge per eliminare l'indeterminazione additiva del potenziale. [43]

Per la meccanica, non la implementerei nella prima release se l'obiettivo principale è ECG/EP. Il movimento modifica geometria, orientamento e quindi forward ECG; un lavoro del Politecnico di Milano ha mostrato un modello heart–torso elettromeccanico che tiene conto della deformazione. openCARP, allo stato attuale, dichiara elettromeccanica accoppiata a livello single-cell ma non organ-level completa, mentre Chaste e Continuity dispongono di percorsi elettromeccanici più estesi. [44]

Per una futura elettromeccanica completa, aggiungerei:

modello attivo tensione/Ca;

materiale passivo anisotropo;

equilibrio quasi-statico FEM;

pericardio e vincoli basali;

accoppiamento 0D circolatorio;

eventualmente mechano-electric feedback.

Il progetto iHEART del Politecnico di Milano costituisce un riferimento particolarmente pertinente in italiano/europeo per questo livello di integrazione. [45]

## Validazione, testing e requisiti hardware

La validazione va distinta in verification — il codice risolve correttamente le equazioni — e validation — le equazioni e i parametri riproducono la fisiologia osservata. Il benchmark Niederer del 2011 confrontò undici simulatori e produsse una soluzione consenso per la verifica dei solver cardiaci; Pathmanathan e Gray introdussero successivamente problemi con soluzione analitica per monodominio, bidominio e bidominio con bath. La FDA distribuisce oggi questi problemi come Regulatory Science Tool per la verifica di software EP. [46]

Un protocollo minimo dovrebbe quindi includere: test ODE single-cell contro tracce di riferimento; convergenza in spazio/tempo; benchmark Niederer; test analitici FDA; conduction velocity longitudinal/transverse; APD90 e restitution; attivazione/repolarizzazione ventricolare; QRS duration/morphology; confronto ECG sintetico contro ECGSIM o dati sperimentali; test di conservazione/condizioni al contorno; regression test bitwise o entro tolleranza per ogni release. openCARP pubblica anche test report e benchmark report continui, che possono fungere da esempio di engineering. [47]

Per un modello umano, la validazione fisiologica deve poi includere range sperimentali di AP, conduction velocity, activation sequence, ECG 12 derivazioni, risposta a pacing rate e, se si modellano farmaci, curve dose–response dei canali. Per sistemi destinati ad applicazioni cliniche o dispositivi, il livello di V&V richiesto è molto più elevato; la FDA richiama esplicitamente il framework di credibility e ASME V&V 40 per modelli usati in contesti regolatori. [48]

### Hardware indicativo

Per sviluppo e test 1D/2D o mesh 3D coarse: 16–32 core CPU, 64 GB RAM e una GPU da 12–24 GB sono sufficienti. Per monodominio umano dettagliato a ~0,2–0,4 mm, suggerisco 32–64 core, 128–256 GB RAM e GPU da 24–48 GB. Per bidominio/torso, mesh molto fini o ensemble: 256–512+ GB RAM, più GPU o cluster MPI. Queste sono stime di pianificazione; i requisiti effettivi dipendono drasticamente dal numero di nodi, variabili cellulari e frequenza di output. Come riferimento concreto di scala, il lavoro MonoAlg3D del 2026 ha usato milioni di volumi e GPU cluster per simulazioni dettagliate e batch paralleli. [49]

Per ridurre l'I/O, conviene salvare  a 0,5–2 ms anziché a ogni timestep di 0,01 ms, calcolare online activation time/APD e scrivere solo biomarcatori quando non serve il film completo. La frequenza di simulazione interna e quella di rendering devono essere indipendenti.

## Roadmap prioritaria e timeline

La roadmap seguente massimizza la probabilità di arrivare presto a un risultato scientificamente credibile senza bloccare il progetto sullo sviluppo HPC prematuro.

Priorità immediata: nei primi 2–3 mesi costruirei un ventricolo open-source con fibre, TTP06 o ORd, monodominio openCARP, stimolo apicale/Purkinje semplificato e export VTK. Il criterio di uscita è riprodurre benchmark Niederer/FDA e ottenere un'animazione di  corretta in ParaView. openCARP fornisce già esempi di monodominio, tuning della conduction velocity, reentry, ECG e visualizzazione. [50]

Seconda priorità: mesi 3–6, aggiungere Purkinje 1D, fibre realistiche, heterogeneity endo/mid/epi e torso/lead field. In questa fase il risultato minimo dovrebbe essere un QRS sintetico coerente con la sequenza d'attivazione e un confronto indipendente con ECGSIM o un altro solver. Gli algoritmi pubblici per la generazione di reti Purkinje e i workflow MonoAlg3D/openCARP riducono molto il rischio di implementazione. [51]

Terza priorità: mesi 6–10, aggiungere atri, modello Courtemanche, nodo SA/AV semplificato, ciclo P–QRS–T e frontend interattivo. La visualizzazione dovrebbe sincronizzare cuore 3D ed ECG, permettere cut planes, selezione di regioni, mappe di APD/activation time e confronto fra condizioni.

Quarta priorità: mesi 9–14, accelerazione GPU e modalità quasi real-time. Prima introdurrei reaction-eikonal; solo dopo misurerei se serve un solver monodominio GPU proprietario. MonoAlg3D è una possibile integrazione diretta ed è MIT, mentre openCARP rimane utile come oracle scientifico per cross-validation. [52]

Quinta priorità: oltre 12–18 mesi, elettromeccanica, torso deformabile, circolazione 0D e patient-specific personalization. Questa parte aumenta drasticamente sia il numero di parametri sia l'onere di validazione; i lavori iHEART/life^x del Politecnico di Milano sono fra i riferimenti più pertinenti per un'implementazione italiana. [53]

Sul piano delle licenze, una combinazione Chaste/MonoAlg3D/VTK/ParaView/WebGL è la più semplice per eventuale commercializzazione perché usa licenze permissive o standard aperti; openCARP richiede invece attenzione perché la sua licenza gratuita è per uso accademico e l'uso commerciale necessita di accordo separato. life^x-ep viene distribuito sotto CC BY-NC-ND 4.0, Continuity è gratuito per ricerca non-profit ma richiede contatto UCSD per uso commerciale, ECGSIM è GPLv3, e Unity/Unreal hanno termini commerciali propri che vanno verificati alla data di distribuzione. [54]

Come set di riferimenti primari da leggere per primo, sceglierei: il paper openCARP 2021 e i tutorial ufficiali; Niederer et al. 2011 e Pathmanathan & Gray 2014 per verification; ten Tusscher–Panfilov 2006, Courtemanche 1998 e O'Hara–Rudy 2011 per la fisiologia cellulare; la documentazione ECG di openCARP e ECGSIM per il forward model; i lavori e il software life^x/iHEART del Politecnico di Milano come risorsa prioritaria in italiano/europeo; e il lavoro MonoAlg3D 2026 per l'architettura GPU moderna. [55]

[1] [19] [24] [42] [54] Getting started | openCARP

https://opencarp.org/getting-started?utm_source=chatgpt.com

[2] [49] Toward cardiac electrophysiology digital twins with an efficient open source scalable solver on GPU clusters | Scientific Reports

https://www.nature.com/articles/s41598-025-33709-w?utm_source=chatgpt.com

[3] [20] Extracellular potentials and ECGs | openCARP

https://opencarp.org/documentation/examples/02_ep_tissue/07_extracellular?utm_source=chatgpt.com

[4] [43] openCARP - User’s Manual - Anton Prassl c, Aurel N

https://git.opencarp.org/openCARP/openCARP/-/package_files/441/download?utm_source=chatgpt.com

[5] List of provided meshes from third party — caridac-geometriesx

https://computationalphysiology.github.io/cardiac-geometriesx/docs/published_meshes.html?utm_source=chatgpt.com

[6] lifex-fiber

https://lifex.gitlab.io/fibers.html?utm_source=chatgpt.com

[7] Frontiers | GEMS: A Fully Integrated PETSc-Based Solver for Coupled Cardiac Electromechanics and Bidomain Simulations

https://www.frontiersin.org/journals/physiology/articles/10.3389/fphys.2018.01431/full?utm_source=chatgpt.com

[8] [13] Cell model for efficient simulation of wave propagation in human ventricular tissue under normal and pathological conditions - PubMed

https://pubmed.ncbi.nlm.nih.gov/17110776/?utm_source=chatgpt.com

[9] Impulses and Physiological States in Theoretical Models of Nerve Membrane.

https://pubmed.ncbi.nlm.nih.gov/19431309/?utm_source=chatgpt.com

[10] Impulses and Physiological States in Theoretical Models of Nerve Membrane — Physiome Model Repository

https://models.cellml.org/exposure/cf32346a9e5c4b2cdb559b11da5f1ae1/fitzhugh_1961.cellml/view?utm_source=chatgpt.com

[11] A model of the ventricular cardiac action potential. Depolarization, repolarization, and their interaction - PubMed

https://pubmed.ncbi.nlm.nih.gov/1709839/?utm_source=chatgpt.com

[12] Ionic mechanisms underlying human atrial action potential properties: insights from a mathematical model - PubMed

https://pubmed.ncbi.nlm.nih.gov/9688927/?utm_source=chatgpt.com

[14] Simulation of the undiseased human cardiac ventricular action potential: model formulation and experimental validation.

https://pubmed.ncbi.nlm.nih.gov/21637795/?utm_source=chatgpt.com

[15] [16] [17] [50] Examples | openCARP

https://opencarp.org/documentation/examples?utm_source=chatgpt.com

[18] [51] Generating Purkinje networks in the human heart - PMC

https://pmc.ncbi.nlm.nih.gov/articles/PMC4917481/?utm_source=chatgpt.com

[21] ECGSIM: The interactive ECG tool for research and education

https://ecgsim.org/?utm_source=chatgpt.com

[22] FEM Implementations Chaste

https://chaste.github.io/docs/user-guides/finite-element-implementations/?utm_source=chatgpt.com

[23] Toward GPGPU accelerated human electromechanical cardiac simulations - Vigueras - 2014 - International Journal for Numerical Methods in Biomedical Engineering - Wiley Online Library

https://onlinelibrary.wiley.com/doi/full/10.1002/cnm.2593?utm_source=chatgpt.com

[25] Publication Results Detail

https://mox.polimi.it/reports-and-books/publication-results/?id=1347&utm_source=chatgpt.com

[26] lifex-cfd

https://lifex.gitlab.io/ep.html?utm_source=chatgpt.com

[27] Cardiac Atlas Project - sharing a large-scale database of cardiac images, models and associated clinical datasets

https://www.cardiacatlas.org/?utm_source=chatgpt.com

[28] Four-Chamber Human Heart Model for the Simulation of Cardiac Electrophysiology and Cardiac Mechanics | Zenodo

https://zenodo.org/records/10526554?utm_source=chatgpt.com

[29] [41] Software Management Plan | openCARP

https://opencarp.org/about/software-management-plan?utm_source=chatgpt.com

[30] Chaste

https://chaste.github.io/?utm_source=chatgpt.com

[31] [52] GitHub - rsachetto/MonoAlg3D_C: Extensible Monodomain simulator · GitHub

https://github.com/rsachetto/MonoAlg3D_C?utm_source=chatgpt.com

[32] lifex-ep: a robust and efficient software for cardiac electrophysiology simulations | BMC Bioinformatics | Springer Nature Link

https://link.springer.com/article/10.1186/s12859-023-05513-8?utm_source=chatgpt.com

[33] Getting Started With Continuity – Continuity

https://continuity.ucsd.edu/continuity-documentation/getting-started-with-continuity/?utm_source=chatgpt.com

[34] ECGSIM Downloads: The Program (ECGSIM)

https://www.ecgsim.org/downloads/?utm_source=chatgpt.com

[35] ParaView License

https://www.paraview.org/license/?utm_source=chatgpt.com

[36] Unity Editor Software Terms

https://web-prd.hexagon.unity.com/legal/editor-terms-of-service/software?utm_source=chatgpt.com

[37] Unreal Engine (UE5) licensing options - Unreal Engine

https://www.unrealengine.com/license?utm_source=chatgpt.com

[38] Getting Started - WebGL Public Wiki

https://wikis.khronos.org/webgl/Getting_Started?utm_source=chatgpt.com

[39] CARPentry Modeling Environment Documentation — CARPentry documentation

https://carpentry.medunigraz.at/?utm_source=chatgpt.com

[40] biv-me rescans - Cardiac Atlas Project

https://www.cardiacatlas.org/biv-me-rescans/?utm_source=chatgpt.com

[44] [53] An integrated heart–torso electromechanical model for the simulation of electrophysiological outputs accounting for myocardial deformation

https://re.public.polimi.it/handle/11311/1275002?utm_source=chatgpt.com

[45] Models for the Heart

https://mox.polimi.it/research-areas/computational-medicine-for-the-cardiocirculatory-system/models-for-the-heart/?utm_source=chatgpt.com

[46] Verification of cardiac tissue electrophysiology simulators using an N-version benchmark - PubMed

https://pubmed.ncbi.nlm.nih.gov/21969679/?utm_source=chatgpt.com

[47] Documentation | openCARP

https://opencarp.org/documentation?utm_source=chatgpt.com

[48] Verification Test Problems for Cardiac Electrophysiology Modeling Software | FDA

https://www.fda.gov/medical-devices/regulatory-science-tools-catalog/verification-test-problems-cardiac-electrophysiology-modeling-software?utm_source=chatgpt.com

[55] Cardiac Electrophysiology Simulator | openCARP

https://opencarp.org/?utm_source=chatgpt.com


| Modello | Uso consigliato | Complessità | Parametri principali | Trade-off |

|---|---|---|---|---|

| FitzHugh–Nagumo | demo, didattica, algoritmi real-time | molto bassa | eccitabilità, recupero, diffusività | riproduce eccitazione/refrattarietà qualitativa, non canali cardiaci specifici. [9] |

| Aliev–Panfilov / Rogers–McCulloch | propagazione rapida, aritmie qualitative | bassa | soglie, recovery, diffusività | ottimo per prototipi; fisiologia cellulare ridotta. [10] |

| Luo–Rudy I/II | ventricolo, riferimento storico | media | Na, Ca, K, pompe/scambiatori | ben studiato ma basato soprattutto su dati non umani e meno moderno. [11] |

| Courtemanche–Ramirez–Nattel | atrio umano | medio-alta | correnti Na/K/Ca, exchanger, pump | riferimento classico per atri umani e AF. [12] |

| ten Tusscher–Panfilov 2006 | ventricolo umano | alta | correnti rapide/lente, Ca handling, endo/M/epi | buon compromesso costo/fedeltà per organo intero. [13] |

| O'Hara–Rudy 2011 | ventricolo umano dettagliato | molto alta | dataset esteso di canali, Ca cycling, pompe/scambiatori | maggiore fedeltà fisiologica ma più ODE per ogni nodo; codice originale disponibile dal Rudy Lab. [14] |


| Configurazione | Accuratezza | Costo relativo | Target |

|---|---|---|---|

| Eikonal | attivazione solamente | 1× | UI interattiva, fitting |

| Reaction-eikonal + waveform template | buona attivazione + pseudo-AP | ~1–10× | ECG rapido |

| Monodominio + modello fenomenologico | buona propagazione | ~10–100× | training/visualizzazione |

| Monodominio + TTP06/ORd | elevata | ~100–1000× | ricerca EP |

| Pseudo-bidominio + lead-field | elevata per ECG in molti casi | modesto overhead sul mono | ECG organ-level |

| Bidominio + torso | massima fra i continuum standard | più alto | stimolazione/campo extracellulare |


| Framework | Funzioni cardiache | Linguaggi | Licenza | GPU/HPC | Maturità / uso consigliato |

|---|---|---|---|---|---|

| openCARP | mono/bidominio, eikonal, EMI, ionic models, ECG, Purkinje/network, post-processing | C++ + Python/carputils | Academic Public License; commerciale separata | MPI, Ginkgo/MLIR GPU | riferimento generale per EP; documentazione e tutorial molto ampi. [29] |

| Chaste | mono/bidominio, bath, cellule, elettromeccanica | C++ | BSD | MPI/PETSc CPU/HPC | molto maturo e fortemente testato; ottimo per sviluppo metodologico. [30] |

| MonoAlg3D | monodominio, Purkinje, adaptive mesh, ionic models | C/CUDA | MIT | CUDA + MPI | eccellente per grandi campagne GPU. [31] |

| life^x-ep | monodominio, modelli ionici, fibre, eterogeneità | C++17/deal.II | distribuzione EP CC BY-NC-ND 4.0 | MPI CPU; forte scalabilità | particolarmente interessante in ambito italiano/iHEART. [32] |

| Continuity | EP, biomeccanica, trasporto, multiphysics | principalmente Python legacy + C/Fortran | gratuito per ricerca non-profit; commerciale da concordare | vecchio supporto GPU/HPC | storico e molto completo, ma stack legacy basato anche su Python 2.x: non lo sceglierei come base di un nuovo prodotto. [33] |

| ECGSIM | ECG/BSPM e source-surface model | applicazione dedicata | GPLv3 | non orientato a HPC | ottimo riferimento indipendente per forward ECG. [34] |

| VTK / ParaView | mesh, campi scalari/vettoriali, volume/surface rendering | C++/Python | BSD | rendering parallelo/GPU | prima scelta per analisi scientifica e debugging. [35] |

| Unity | UI 3D desktop/VR | C# | commerciale/proprietaria | GPU real-time | ottimo frontend, ma verificare sempre i termini correnti; i termini Unity sono stati aggiornati nel 2026. [36] |

| Unreal Engine | visualizzazione 3D high-end/VR | C++/Blueprint | EULA Epic | GPU real-time | eccellente grafica; royalty/seat dipendono dal tipo e fatturato del prodotto. [37] |

| WebGL / Three.js | browser 3D | JS/TS/GLSL | WebGL royalty-free; Three.js MIT | GPU browser | soluzione migliore per distribuzione web leggera. [38] |


|  |

|---|


|  |

|---|