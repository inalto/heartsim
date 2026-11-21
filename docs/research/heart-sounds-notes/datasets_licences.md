# Openly available heart-sound (PCG / auscultation) recordings and their licences (status: 20 Nov 2026)

Verdict legend, against the project policy "only CC-BY, CC0, MIT, BSD or Apache assets may ship":

* **SHIP** = licence is CC-BY / CC0 / public domain, stated on a primary page I read.
* **STUDY** = usable for listening, analysis and parameter fitting only (NC, ND, SA, credentialed, proprietary, or no licence).
* **POLICY?** = ODC-By 1.0: attribution-only and permissive, but (a) not literally on the allow-list and (b) by its own text it licenses the *database*, not rights in the individual contents. Needs an explicit project decision.
* **UNCLEAR** = licence not stated, or only stated by a third-party mirror, or I could not reach the primary page.

"Verified" below means I read the licence statement on the linked page (or via its API) during this session; "search snippet" means only a search-engine summary was seen.

---

## 1. PhysioNet: CinC 2016, CirCor DigiScope, EPHNOGRAM and newer PCG data — exact licences and versions

### Takeaway
All three classic PhysioNet PCG sets are open access under the **Open Data Commons Attribution License (ODC-By) v1.0** — not CC-BY, not credentialed. The only PhysioNet PCG set under plain **CC BY 4.0** is **SensSmartTech (2024)**, which also happens to carry simultaneous 4-lead ECG + PCG; two more ODC-By sets with ECG+PCG appeared in 2025–2026 (HeartCycle, Auricle).

### Cited Findings

| Dataset (version, date) | Contents | Fs / duration | Sites / device | Simultaneous ECG | Licence (exact) | Verdict |
|---|---|---|---|---|---|---|
| **Classification of Heart Sound Recordings: PhysioNet/CinC Challenge 2016** v1.0.0, 4 Mar 2016 | 3,126 training recordings (page) / 3,125 (news post) in sub-databases A–E; labels normal/abnormal + signal quality; hand-corrected S1/S2 state annotations | Resampled to 2,000 Hz, .wav; ~5 s to just over 120 s | "Nine locations", four typical (aortic, pulmonic, tricuspid, mitral); clinical and non-clinical; devices not listed on the landing page | Landing page: "Each recording contains only one PCG lead"; training-a lists only `.wav`, `.dat`, `.hea` per record (first 100k chars of the listing read) | "Open Data Commons Attribution License v1.0" | POLICY? |
| **CirCor DigiScope Phonocardiogram Dataset** v1.0.3, 10 May 2022 (v1.0.0 11 Jan 2022) | 5,272 recordings, 1,568 subjects aged 0–21 y (mean 6.1 ± 4.3); murmur present/absent/unknown, clinical outcome, S1/S2 segmentation; systolic and diastolic murmur **timing** (early/mid/late/holo), **shape** (crescendo, decrescendo, diamond, plateau), **pitch** (low/medium/high), **grading** (Levine I–III/VI systolic; I–III/IV diastolic), **quality** (blowing/harsh/musical) | 4,000 Hz appears in the page example only; 4.8–80.4 s (mean 22.9 ± 7.4 s), > 33.5 h total | AV, PV, TV, MV, Phc (other); "electronic auscultation", device not named on page | No ECG released | "Open Data Commons Attribution License v1.0" | POLICY? |
| **EPHNOGRAM: A Simultaneous Electrocardiogram and Phonocardiogram Database** v1.0.0, 11 Jun 2021 | 69 records from 24 healthy men aged 23–29; rest, walking, running, cycling (stress test). 8 × 30 s + 61 × 30 min (text also says 62) | 8 kHz, 12-bit (10.5 effective bits); WFDB 16-bit + MATLAB | Stethoscope between tricuspid and mitral landmarks, held by elastic brace; two PCG channels + two auxiliary ambient-noise channels for some subjects; custom hardware v2.1 | **Yes** — three-lead ECG acquired synchronously (lead configuration not named on page) | "Open Data Commons Attribution License v1.0" | POLICY? |
| **SensSmartTech database of cardiovascular signals synchronously recorded by an electrocardiograph, phonocardiograph, photoplethysmograph and accelerometer** v1.0.0, 19 Dec 2024 | 338 thirty-second recordings, 32 healthy adults (18 F, 14 M, 34.0 ± 8.6 y), standing, at rest and immediately after exercise; benign arrhythmias included; HR range cited 58–173 bpm | ECG 500 Hz, PCG 1 kHz, PPG 100 Hz; WFDB version resampled to 1 kHz on a common time axis; 775 MB | PCG from stethoscope-mounted microphone (abstract: apex; Methods: sternum, right of V3 — page is inconsistent) | **Yes** — ECG leads I, II, V3, V4 + carotid/brachial PPG + z-axis accelerometer | "Creative Commons Attribution 4.0 International Public License" | **SHIP** |
| **HeartCycle: synchronized impedance cardiography and echocardiography** v1.0.0, 2 Nov 2025 | 17 healthy volunteers (14 M, 3 F, 19–40 y), 208 HDF5 records | PCG 44,100 Hz; ECG 200 Hz / 500 Hz / 44,100 Hz (stethoscope); ICG 200 Hz; echo 136 Hz | Stethoscope PCG | **Yes** — ECG + ICG + echo (+ PPG in one experiment) | "Open Data Commons Attribution License v1.0" | POLICY? |
| **Auricle: A Multimodal Wearable Sensing Dataset for Cardiovascular and Hemodynamic Assessment** v1.0.0, 13 Oct 2026 | 42 healthy adults (19–55 y, 37 with second session) + **28 heart-failure patients** (21–79 y); breath-hold, light exercise, Valsalva, cold pressor (healthy only) | Recorded 10 kHz, released at 1 kHz; WFDB | Digital-stethoscope PCG + chest SCG + binaural in-ear cardiac audio | **Yes** — 3-lead ECG, continuous non-invasive arterial pressure, finger/earlobe PPG, respiration | "Open Data Commons Attribution License v1.0" | POLICY? |
| CardioWave-Portable v1.0.0, 30 Sep 2026 | ECG + PPG only, **no PCG** (listed here only to rule it out) | 500 Hz | — | — | "PhysioNet Credentialed Health Data License 1.5.0" | not relevant |

- CinC 2016 licence, version, date, counts, 2,000 Hz, durations, sites — [PhysioNet challenge-2016 v1.0.0](https://physionet.org/content/challenge-2016/1.0.0/); licence text — [view-license](https://physionet.org/content/challenge-2016/view-license/1.0.0/)
- CinC 2016 training-a file types (`.wav`, `.dat`, `.hea`; no separate ECG file seen in the portion read) — [training-a listing](https://physionet.org/content/challenge-2016/1.0.0/training-a/)
- CirCor details and licence — [PhysioNet circor-heart-sound v1.0.3](https://physionet.org/content/circor-heart-sound/1.0.3/)
- EPHNOGRAM details and licence — [PhysioNet ephnogram v1.0.0](https://physionet.org/content/ephnogram/1.0.0/)
- SensSmartTech details and CC BY 4.0 licence — [PhysioNet senssmarttech](https://www.physionet.org/content/senssmarttech/)
- HeartCycle details and licence — [PhysioNet heartcycle](https://www.physionet.org/content/heartcycle/)
- Auricle details and licence — [PhysioNet auricle v1.0.0](https://physionet.org/content/auricle/1.0.0/)
- CardioWave-Portable is credentialed and has no PCG — [PhysioNet cardiowave-portable](https://physionet.org/content/cardiowave-portable/1.0.0/)
- ODC-By v1.0 §4.2: when publicly conveying the database you must include "a copy of this License or its Uniform Resource Identifier (URI) with the Database" and keep notices intact; §4.3 example notice for produced works: "Contains information from DATABASE NAME which is made available under the ODC Attribution License."; §2.4: the licence does not cover "any rights (other than Database Rights or in contract) in individual Contents" — [ODC-By text on PhysioNet](https://physionet.org/content/challenge-2016/view-license/1.0.0/)
- Requested citations: Liu et al., Physiol. Meas. 2016;37(12):2181-2213, doi:10.1088/0967-3334/37/12/2181 and Clifford et al. (CinC 2016); Oliveira et al., IEEE JBHI, doi:10.1109/JBHI.2021.3137048 and dataset doi:10.13026/tshs-mw03; Kazemnejad, Gordany & Sameni 2021, doi:10.13026/tjtq-5911 plus bioRxiv doi:10.1101/2021.05.17.444563; all pages also ask for the standard PhysioNet citation, now given as Pollard et al., 2026, Nature Health, doi:10.1038/s44360-026-00096-z — [CinC 2016](https://physionet.org/content/challenge-2016/1.0.0/), [CirCor](https://physionet.org/content/circor-heart-sound/1.0.3/), [EPHNOGRAM](https://physionet.org/content/ephnogram/1.0.0/)
- SensSmartTech citation: Lazović A. et al. (2024), version 1.0.0, PhysioNet, doi:10.13026/fy9p-n277 — [PhysioNet senssmarttech](https://www.physionet.org/content/senssmarttech/)

### Inferences
- None of the PhysioNet PCG sets is CC-BY except SensSmartTech. ODC-By is attribution-only (functionally close to CC-BY) but is a *database* licence that explicitly leaves rights in the individual recordings unaddressed; under a literal reading of the heartsim policy it is not on the allow-list. If the project decides to accept ODC-By, CirCor is by far the richest source of labelled real murmurs (paediatric), and CinC 2016 of adult normal/abnormal sounds.
- All of these sets, whatever the shipping decision, can be used for study and for fitting synthesiser parameters.
- CirCor labels murmurs by character, not by diagnosis; it cannot directly supply "this is aortic stenosis" clips.

### Gaps
- I did not confirm from the CinC 2016 file listing whether any sub-database carries an ECG channel inside the `.dat` files; the landing page says one PCG lead per recording, and I read only the first part of the training-a listing. Liu et al. 2016 should be checked directly if ECG in training-a matters.
- CirCor's dataset-wide sampling rate and device are not stated on the landing page read (4,000 Hz appears only in an example).
- I did not run an exhaustive PhysioNet site search; the 2024–2026 entries above came from web-search results, so further PCG-containing sets may exist.

---

## 2. Other datasets and sound libraries (PASCAL, Yaseen, Michigan, UW, BMD-HS, HLS-CMDS, ZCHSound, HeartWave, Mendeley, vendor libraries)

### Takeaway
Outside PhysioNet, the only clearly shippable research datasets found are **HLS-CMDS** (CC BY 4.0, but manikin-generated sounds) and the **JUST heart-sound regurgitation dataset** (CC BY 4.0 on Mendeley Data; real AR/MR/TR/normal patients). Everything else is NC/ND, share-alike, proprietary, unlicensed or of unverifiable provenance.

### Cited Findings

| Dataset / library | Contents | Fs / duration / device | ECG | Licence as stated (where) | Verdict |
|---|---|---|---|---|---|
| **HLS-CMDS** (Torabi, Shirani, Reilly; Mendeley Data v3, doi:10.17632/8972jxbpmp.3; also UCI, Zenodo, Figshare) | 535 recordings from a **clinical manikin**: 50 heart, 50 lung, 145 mixed with matching source heart and source lung tracks. Heart types: Normal, Late Diastolic Murmur, Mid Systolic Murmur, Late Systolic Murmur, Early Systolic Murmur, Atrial Fibrillation, Fourth Heart Sound, Third Heart Sound, Tachycardia, Atrioventricular Block. Sites: apex, right/left upper sternal border, lower left sternal border, right/left costal margin | 15 s each; 3M Littmann CORE Digital Stethoscope (bell / diaphragm / midrange); sampling rate not on page | No | "CC BY 4.0" (Mendeley); UCI: "Creative Commons Attribution 4.0 International (CC BY 4.0)" | **SHIP** (see caveat in Inferences) |
| **A Dataset of Heart Sound Regurgitation of Patients with Heart Valve Disorders** (Fraiwan, Ibnian, Haider; Jordan Univ. of Science and Technology; Mendeley Data v1, 29 Jul 2025, doi:10.17632/jfbdvjw6sf.1) | 399 raw `.wav` from 104 participants: **17 AR, 68 MR, 84 TR, 230 normal**; three chest positions (aortic 2nd ICS RSB, mitral apex, tricuspid LLSB) | 3M Littmann Electronic Stethoscope 3200, extended mode, StethAssist; Fs and duration not stated | No | Mendeley record: "CC BY 4.0". The Data in Brief article itself is CC BY-NC 4.0 | **SHIP** (dataset record governs the audio) |
| **BUET Multi-disease Heart Sound (BMD-HS)** | "over 800" (872 `.wav` listed in Train; paper says 864) recordings, 59 patients × 8 positions; AS, AR, MR, MS, multi-disease, normal; NICVD Dhaka | 4 kHz, 20 s | No | LICENSE file: "Creative Commons Attribution-NonCommercial-NoDerivatives 4.0 International License (CC BY-NC-ND 4.0)", "(c) 2024 mHealth Lab" | STUDY |
| **Yaseen et al. 2018** (GitHub yaseen21khan) | 1,000 `.wav`, 200 each: AS, MR, MS, MVP, Normal | not on README | No | **No LICENSE file; GitHub API `license: null`.** README does not state provenance | STUDY (unlicensed; provenance third-party, see below) |
| **PASCAL Classifying Heart Sounds Challenge** (Bentley et al. 2011) | A: 176 WAV (normal, murmur, extra heart sound, artifact) from the public via iStethoscope Pro iPhone app. B: 656 WAV (normal, murmur, extrasystole) from hospital trial with DigiScope | — | No | Original site unreachable. TU Berlin mldata mirror (uploaded by user "Yiqi", 2012) labels it "CC0" | UNCLEAR |
| **Michigan Heart Sound and Murmur Library** (Univ. of Michigan, Deep Blue, April 2015 zip of mp3) | Teaching library of heart sounds and murmurs | mp3 | No | Search snippet of the Deep Blue record: rights URI Creative Commons Attribution-ShareAlike 4.0; Lecturio credits "The Regents of the University of Michigan" under CC BY-SA 3.0 | STUDY (share-alike; licence seen only via search snippet) |
| **Univ. of Washington "Advanced Physical Diagnosis" heart sounds** | Demonstration heart and lung sounds | — | No | "© 1997, Christopher Cable and the UCLA School of Medicine"; "Permission for educational use only of specific sounds granted by the author, Dr. Hans Pasterkamp."; "© Copyright 1999 Medi-Wave Inc." | STUDY (third-party copyrighted, permission-based) |
| **Wellcome Collection "Heart sounds and murmurs" reels (Georgetown, W. Proctor Harvey, 1949)** | Nine reels; reel 5 = murmurs, 31.56 min | analogue transfer | No | "Creative Commons Attribution-Non-Commercial 2.0 UK: England & Wales"; copyright note "Georgetown University Medical Centre 1949" | STUDY (NC) |
| **Easy Auscultation** | Commercial teaching site | — | — | About page: content "mainly consists of proprietary information created by our authors and is subject to copyright protections" (search snippet) | STUDY |
| **ZCHSound** (ZJU paediatric CHD; Jia et al., IEEE TBME 2024, doi:10.1109/tbme.2023.3348800) | 1,259 children, split into a clean high-quality set and a noisy low-quality set; collected 2020–2022 | electronic stethoscopes | No | Paper says public download at zchsound.ncrcch.org.cn; **no licence found**; the host presented a mismatched TLS certificate when fetched | UNCLEAR |
| **HeartWave** (Alrabie & Barnawi, IEEE Access 2023, doi:10.1109/access.2023.3325749) | 1,353 recordings, nine classes | — | No | No dataset licence or download page reached; one search summary called it "CC BY-NC-ND 4.0" without showing the source | UNCLEAR (likely STUDY) |
| Zenodo 17489962 "Cardio-mechanical Signals from Patients with Valvular Heart Diseases" (31 Oct 2025) | ECG + SCG + GCG in AS/MS patients — **not PCG** | — | ECG | CC BY 4.0 (search snippet) | not heart sounds |

- HLS-CMDS contents, device, licence — [Mendeley Data 8972jxbpmp v3](https://data.mendeley.com/datasets/8972jxbpmp/3); CC BY 4.0 wording and reuse statement — [UCI ML Repository #1202](https://archive.ics.uci.edu/dataset/1202); requested citation: Torabi, Shirani, Reilly, IEEE Data Descriptions, doi:10.1109/IEEEDATA.2025.3566012 — [Mendeley Data](https://data.mendeley.com/datasets/8972jxbpmp/3)
- JUST regurgitation dataset licence "CC BY 4.0", device, institution, date — [Mendeley Data jfbdvjw6sf v1](https://data.mendeley.com/datasets/jfbdvjw6sf/1); class counts (17/68/84/230; 104 participants), sites, and the article's own CC BY-NC 4.0 licence — [Data in Brief article, PMC12504997](https://pmc.ncbi.nlm.nih.gov/articles/PMC12504997/)
- BMD-HS README and CC BY-NC-ND 4.0 — [github.com/sani002/BMD-HS-Dataset](https://github.com/sani002/BMD-HS-Dataset); LICENSE text — [raw LICENSE](https://raw.githubusercontent.com/sani002/BMD-HS-Dataset/main/LICENSE). The URL given in the arXiv paper (github.com/mHealthBuet/BMD-HS-Dataset) returned HTTP 404 — [arXiv 2409.00724](https://arxiv.org/abs/2409.00724). One search summary called the licence "CC BY-NC-SA 4.0"; the LICENSE file read directly says NC-ND.
- Yaseen repository: 5 classes × 200, no licence, no provenance in README — [GitHub repo](https://github.com/yaseen21khan/Classification-of-Heart-Sound-Signal-Using-Multiple-Features-). Third-party descriptions say the recordings were "collected from various sources like books and websites" — [CardioXNet, arXiv 2010.01392](https://arxiv.org/pdf/2010.01392) and gathered "from many online sources, including medical textbooks and websites" — [arXiv 2309.07466](https://arxiv.org/pdf/2309.07466)
- PASCAL A/B description and the mirror's "CC0" label — [TU Berlin mldata mirror](https://mldata-static.ml.tu-berlin.de/repository/data/viewslug/record-of-heart-sound/2.html); recording counts 176 / 656 also in [PMC7199391](https://pmc.ncbi.nlm.nih.gov/articles/PMC7199391)
- Michigan library record — [Deep Blue item](https://deepblue.lib.umich.edu/items/d9d03331-12f5-414e-9c19-3c5985bedb49) (HTTP 403 to my fetch; licence from search snippet only)
- UW credits — [depts.washington.edu/physdx/credits.html](https://depts.washington.edu/physdx/credits.html)
- Wellcome reel 5 licence — [wellcomecollection.org/works/yc4rcgqv](https://wellcomecollection.org/works/yc4rcgqv)
- Easy Auscultation — [easyauscultation.com/about](https://www.easyauscultation.com/about)
- ZCHSound — [paper record](https://oa.mg/work/10.1109/tbme.2023.3348800)
- HeartWave — [DOAJ record](https://doaj.org/article/541a1c86210644a3918bdc2818c57708)

### Inferences
- **Yaseen dataset must not ship and is risky even as a "source"**: it has no licence, and secondary papers say the clips were harvested from textbooks/websites, i.e. probably third-party teaching CDs. Fine for study only.
- **PASCAL**: the "CC0" tag was set by a third-party uploader on a mirror, not by the challenge organisers; treat as unclear until the original terms are found. Dataset B is DigiScope hospital data, the same lineage as CirCor (which is ODC-By).
- **HLS-CMDS caveat**: the authors license their *recordings* CC BY 4.0, but the sounds were produced by a commercial manikin's built-in sound library; whether the manikin vendor holds rights in the underlying sounds is not addressed on the pages read. It is the only CC-BY source found for S3, S4, AF and AV-block auscultation, so this is worth a short legal sanity check. Labels are generic (e.g. "Mid Systolic Murmur"), not diagnoses.
- **JUST regurgitation dataset** is the best real-patient, plainly CC-BY source found for AR, MR and TR.

### Gaps
- **HSCT-11**, **Thinklabs**, **3M Littmann sound library**, **Texas Heart Institute**: no licence statement found in this session (searches returned nothing relevant). Vendor/institution libraries should be assumed proprietary until a licence is located; that is an assumption, not a finding.
- **Kaggle** collections (e.g. kinguistics/heartbeat-sounds, a PASCAL re-upload): the Kaggle page did not render to my fetcher, so its licence field is unverified. Kaggle licence tags are set by uploaders and do not establish rights in re-uploaded data.
- Original PASCAL terms: `istethoscope.peterjbentley.com` did not resolve.
- Michigan library: could not read the Deep Blue record directly (403); the CC BY-SA 4.0 vs 3.0 discrepancy is unresolved. Either way it is share-alike, hence not shippable under the policy.
- ZCHSound and HeartWave licences not found.
- Sampling rates for HLS-CMDS and the JUST dataset are not on the landing pages.
- An "Open Heart Sounds" dataset by that name was not found.

---

## 3. Wikimedia Commons and Freesound: individual files

### Takeaway
Commons holds a small but valuable set of shippable files: a **CC0 normal heart sound** (James Heilman) and a **CC BY 2.0 series of real paediatric clinical recordings** (Kudriavtsev, Polyshchuk & Roy 2007: Still's murmur, MVP clicks, aortic stenosis, pulmonary stenosis, tetralogy of Fallot, VSD, plus a simulated narrow S2 split). Most other Commons heart audio is CC BY-SA and therefore study-only.

### Cited Findings

Licence, author and description below are the Commons `extmetadata` values returned by the Commons API on 20 Nov 2026; file pages are at `https://commons.wikimedia.org/wiki/<File:name>`.

**Shippable (CC0 / public domain / CC BY)**

| File | Content | Author | Licence |
|---|---|---|---|
| [File:HROgg.ogg](https://commons.wikimedia.org/wiki/File:HROgg.ogg) | "Normal heart sounds at about 70 BPM" | James Heilman, MD | CC0 |
| [File:Athletic bradycardia wav.wav](https://commons.wikimedia.org/wiki/File:Athletic_bradycardia_wav.wav) | Elite athlete, resting bradycardia 42 bpm | Athletearrhythmia | CC0 |
| [File:10 min max heart rate wav.wav](https://commons.wikimedia.org/wiki/File:10_min_max_heart_rate_wav.wav) | Elite athlete, > 180 bpm for 10 min | Athletearrhythmia | CC0 |
| [File:Human heart rate.flac](https://commons.wikimedia.org/wiki/File:Human_heart_rate.flac) | "Human heart rate" | Wilfredor | CC0 |
| [File:Infant Heartbeat.ogg](https://commons.wikimedia.org/wiki/File:Infant_Heartbeat.ogg) | Nine-day-old infant, non-pathological | Huckelberry A. Finne | Public domain |
| [File:Heartbeat mitral valve 150 bpm.ogg](https://commons.wikimedia.org/wiki/File:Heartbeat_mitral_valve_150_bpm.ogg) | Mitral area, 150 bpm, from pdsounds.org | ezwa | Public domain |
| [File:Human heart beating at 61 bpm (Cc-by-3.0).ogg](https://commons.wikimedia.org/wiki/File:Human_heart_beating_at_61_bpm_(Cc-by-3.0).ogg) | Processed heartbeat, from Freesound 62912 | "Benboncan" at Freesound.org | CC BY 3.0 |
| [File:Heartbeat.ogg](https://commons.wikimedia.org/wiki/File:Heartbeat.ogg) | Enhanced real heartbeat (Foley), Freesound 32731 | HerbertBoland | CC BY 3.0 |
| [File:Heart beats sounds - Glaneur de sons.ogg](https://commons.wikimedia.org/wiki/File:Heart_beats_sounds_-_Glaneur_de_sons.ogg) | Odd heartbeat, provenance unknown to the uploader | Glaneur de sons (Freesound) | CC BY 3.0 |
| File:Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-**S1**.ogg | Innocent Still's murmur 1, apex | Kudriavtsev V, Polyshchuk V, Roy D | CC BY 2.0 |
| …-**S2**.ogg | Innocent Still's murmur 2, LSB 2nd space | same | CC BY 2.0 |
| …-**S3**.ogg | *Simulated* test file: two narrow S2 splits (20 ms and 41 ms) | same | CC BY 2.0 |
| …-**S5**.ogg | Mitral valve prolapse with 3 clicks | same | CC BY 2.0 |
| …-**S6**.ogg | Aortic stenosis | same | CC BY 2.0 |
| …-**S7**.ogg | Pulmonary stenosis | same | CC BY 2.0 |
| …-**S8**.ogg | Tetralogy of Fallot (acyanotic) | same | CC BY 2.0 |
| …-**S9**.ogg | Ventricular septal defect | same | CC BY 2.0 |

- The Kudriavtsev files are the additional files of an open-access paper: "Copyright © 2007 Kudriavtsev et al; licensee BioMed Central Ltd. … This is an Open Access article distributed under the terms of the Creative Commons Attribution License (https://creativecommons.org/licenses/by/2.0/), which permits unrestricted use, distribution, and reproduction in any medium, provided the original work is properly cited." — [Europe PMC full text, PMC1899182](https://europepmc.org/article/PMC/PMC1899182)
- Provenance in the paper: sounds "recorded between 1980–91 by one of the authors at the Department of Pediatric Cardiology, IWK Center at Dalhousie University", four major auscultation positions, "binary wave sound track (recorded at 11 kHz sampling rate)"; "no artificial or simulated sounds have been used" for additional files 1, 2, 5–9; additional file 3 is a "realistically simulated" S2-split test signal; most of the data were also included in the teaching system "EarsOn" — [PMC1899182](https://europepmc.org/article/PMC/PMC1899182); DOI [10.1186/1475-925X-6-16](https://doi.org/10.1186/1475-925X-6-16)
- Commons files exist for S1, S2, S3, S5, S6, S7, S8, S9 (sizes 5–10 kB each, i.e. very short clips) — [Commons API listing, e.g. S6](https://commons.wikimedia.org/wiki/File:Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-S6.ogg)

**Study-only on Commons (share-alike)**

| File | Content | Author | Licence |
|---|---|---|---|
| File:AfibO.ogg | Rapid atrial fibrillation | James Heilman, MD | CC BY-SA 3.0 |
| File:Heartsounds.ogg | Own heart, Littmann 3200 | James Heilman, MD | CC BY-SA 3.0 |
| File:HROgg Louder.ogg | Amplified derivative of HROgg | Heilman / Natural Philo | CC BY-SA 3.0 |
| File:MurmurO.ogg | Carotid bruit, 70 % stenosis (not a heart murmur) | James Heilman, MD | CC BY-SA 3.0 |
| File:Stills Murmur in a Toddler.ogg | Innocent Still's murmur | Lars Erickson MD | CC BY-SA 3.0 |
| File:Ash-SVT.ogg | Paroxysmal SVT, 20-year-old male | Ashquacks | CC BY-SA 4.0 |
| File:Mitral Valve Prolapse.wav; File:Liz-s-heart-at-tricuspid.wav; …-holding-breath.wav; …-after-exercise-at-tricuspid.wav | 16-year-old with MVP + mitral regurgitation, systolic murmur | EmilyHopeS | CC BY-SA 4.0 |
| File:Emily's heartbeat.wav; Emily's racing heartbeat.wav; Emma's irregular heartbeat.wav; Faith's heart at tricuspid.wav; Laura's heartbeat after exercise (1–3).wav | Healthy teenagers, digital stethoscope, tricuspid area, rest / post-exercise | EmilyHopeS | CC BY-SA 4.0 |
| File:Ventricular Septal Defect.wav | Heart with a VSD, recorded at the mitral area | Emmadon3 | CC BY-SA 4.0 |
| File:V’s heart at tricuspid.wav; V’s heart at tricuspid 2.wav | Healthy 17-year-old, tricuspid area | EmilyHopeS | CC BY-SA 4.0 |
| File:FunctionalHeartMurmurFemale1.wav | Innocent murmur after exercise | Kreglas413 | CC BY-SA 4.0 |
| File:Premature ventricular contractions, recorded on cardiac event monitor.ogg | PVCs (event-monitor audio) | (not returned) | CC BY-SA 3.0 |
| File:Heartbeat after breathholds.wav; Changing heart rate from 120 bpm down to 60 bpm…wav; Heartbeat under pressure at mitral point.wav; Heartbeat after pressure at pulmonary valve.wav | Apex recordings, Tascam DR-40 | Alexander Rahm | CC BY-SA 4.0 |
| TECHNO-INSPECTOR series (e.g. File:Heart sound 82bpm.ogg) | iPhone stethoscope-app recordings | TECHNO-INSPECTOR | CC BY-SA 4.0 |
| Beeld en Geluid series (70 / 75 / 80 / 110 hartslagen per minuut) | Heartbeats at set rates, incl. irregular | Beeld en Geluid | CC BY-SA 3.0 |

- All rows — Commons API `imageinfo/extmetadata` for the named files, e.g. [File:AfibO.ogg](https://commons.wikimedia.org/wiki/File:AfibO.ogg), [File:Stills Murmur in a Toddler.ogg](https://commons.wikimedia.org/wiki/File:Stills_Murmur_in_a_Toddler.ogg), [File:Mitral Valve Prolapse.wav](https://commons.wikimedia.org/wiki/File:Mitral_Valve_Prolapse.wav), [File:Ash-SVT.ogg](https://commons.wikimedia.org/wiki/File:Ash-SVT.ogg)
- Commons categories that actually exist and hold the audio: "Audios of heart murmurs", "Audios of functional murmur", "Audios of human heart", "Audios of diseases and disorders of the heart", "Audios of auscultation"; `Category:Heart sounds` is empty/nonexistent — [Category:Heart murmurs](https://commons.wikimedia.org/wiki/Category:Heart_murmurs)

**Freesound**
- Freesound heartbeat recordings exist under mixed licences; e.g. Benboncan "Heartbeat Mono" (sound 62912) is mirrored on Commons as CC BY 3.0 — [Commons copy](https://commons.wikimedia.org/wiki/File:Human_heart_beating_at_61_bpm_(Cc-by-3.0).ogg); original at [freesound.org/people/Benboncan/sounds/62912](https://freesound.org/people/Benboncan/sounds/62912)

### Inferences
- The Kudriavtsev clips are single short excerpts (a few kB each, 11 kHz originals) of paediatric patients — enough as authentic reference examples and for fitting murmur envelopes/spectra, but too short to serve alone as looping auscultation tracks.
- The original WAV additional files on the publisher site (rather than the Commons OGG transcodes) are the better source for shipping; the licence is the same CC BY 2.0.
- CC BY-SA files (AF, SVT, MVP+MR, Still's murmur) are legitimate *listening references* for tuning a synthesiser but cannot be bundled.

### Gaps
- My Commons enumeration (category walk from the categories named above plus about twenty keyword searches restricted to audio) was rate-limited (HTTP 429) on a few requests, so some categories ("Audio files of heartbeats", "Heartbeat") and one or two keyword searches may not have been fully covered; the tables are a best-effort list, not a guaranteed complete one.
- Freesound pages require login to my fetcher; no Freesound licence was verified at source, and no Freesound *murmur* recording was found.
- No Commons audio was found for S3/S4 gallops, AR, MS/opening snap, PDA, ASD, pericardial rub, HOCM, prosthetic valves or complete heart block (within what was enumerated).

---

## 4. Which conditions have shippable (CC-BY / CC0 / PD) recordings?

### Takeaway
Clean CC-BY/CC0 real recordings exist for normal sounds, innocent murmur, AS, PS, VSD, ToF, MVP clicks, AR, MR and TR; S3, S4, AF and AV block exist only as CC-BY **manikin** recordings; nothing shippable was found for MS/opening snap, PDA, ASD, pericardial rub, HOCM or prosthetic valves — these must be synthesised.

### Cited Findings

| Condition | Shippable source found | Notes |
|---|---|---|
| Normal S1/S2 | HROgg.ogg (CC0); Athletic bradycardia (CC0); Infant Heartbeat (PD); SensSmartTech (CC BY 4.0, with ECG); JUST dataset class N, 230 recordings (CC BY 4.0); HLS-CMDS "Normal Heart" (CC BY 4.0, manikin) | [HROgg](https://commons.wikimedia.org/wiki/File:HROgg.ogg), [SensSmartTech](https://www.physionet.org/content/senssmarttech/), [JUST](https://data.mendeley.com/datasets/jfbdvjw6sf/1), [HLS-CMDS](https://data.mendeley.com/datasets/8972jxbpmp/3) |
| Physiological / narrow split S2 | Only a *simulated* test signal: Kudriavtsev additional file 3 (CC BY 2.0) | [PMC1899182](https://europepmc.org/article/PMC/PMC1899182) |
| S3 | HLS-CMDS "Third Heart Sound" (CC BY 4.0, manikin) | [HLS-CMDS](https://data.mendeley.com/datasets/8972jxbpmp/3) |
| S4 | HLS-CMDS "Fourth Heart Sound" (CC BY 4.0, manikin) | same |
| Innocent (Still's) murmur | Kudriavtsev S1, S2 (CC BY 2.0, real) | [PMC1899182](https://europepmc.org/article/PMC/PMC1899182) |
| Aortic stenosis | Kudriavtsev S6 (CC BY 2.0, real, paediatric) | same |
| Aortic regurgitation | JUST dataset, 17 recordings (CC BY 4.0) | [PMC12504997](https://pmc.ncbi.nlm.nih.gov/articles/PMC12504997/) |
| Mitral regurgitation | JUST dataset, 68 recordings (CC BY 4.0) | same |
| Tricuspid regurgitation | JUST dataset, 84 recordings (CC BY 4.0) | same |
| Mitral stenosis / opening snap | **None.** HLS-CMDS has a generic "Late Diastolic Murmur" (manikin) | [HLS-CMDS](https://data.mendeley.com/datasets/8972jxbpmp/3) |
| MVP click | Kudriavtsev S5 "Mitral Valve Prolapse with 3 clicks" (CC BY 2.0) | [PMC1899182](https://europepmc.org/article/PMC/PMC1899182) |
| VSD | Kudriavtsev S9 (CC BY 2.0) | same |
| Pulmonary stenosis; tetralogy of Fallot | Kudriavtsev S7; S8 (CC BY 2.0) | same |
| PDA; ASD fixed split; pericardial rub; HOCM; prosthetic valve clicks | **None found** | — |
| Atrial fibrillation | HLS-CMDS "Atrial Fibrillation" (CC BY 4.0, manikin). Real recording AfibO.ogg is CC BY-SA 3.0 → study-only | [HLS-CMDS](https://data.mendeley.com/datasets/8972jxbpmp/3), [AfibO](https://commons.wikimedia.org/wiki/File:AfibO.ogg) |
| Complete heart block | Only HLS-CMDS "Atrioventricular Block" (degree not stated on page; manikin) | [HLS-CMDS](https://data.mendeley.com/datasets/8972jxbpmp/3) |
| Tachycardia / bradycardia | HLS-CMDS "Tachycardia"; Commons CC0 athlete files | as above |

Study-only sources per condition (for parameter fitting): AS/AR/MR/MS/multi-valve adults — BMD-HS (CC BY-NC-ND 4.0) — [BMD-HS](https://github.com/sani002/BMD-HS-Dataset); AS/MR/MS/MVP — Yaseen (unlicensed) — [GitHub](https://github.com/yaseen21khan/Classification-of-Heart-Sound-Signal-Using-Multiple-Features-); murmur timing/shape/pitch/grade statistics — CirCor (ODC-By) — [CirCor](https://physionet.org/content/circor-heart-sound/1.0.3/); classic adult murmurs with expert narration — Wellcome/Georgetown reels (CC BY-NC) — [Wellcome](https://wellcomecollection.org/works/yc4rcgqv); MVP+MR, SVT, AF, Still's — Commons CC BY-SA files (section 3).

### Inferences
- Coverage of shippable *real* recordings is too thin and too heterogeneous (paediatric 1980s tape excerpts, Littmann 3200 clips, manikin output) to build the simulator's sound set from samples. A parametric synthesiser, fitted against the study-only sets and validated by ear against the shippable examples, is the route that satisfies the licence policy for every condition on the list.
- If ODC-By is accepted by policy, CirCor and CinC 2016 would add many real murmur and normal recordings but still not diagnosis-labelled adult MS/PDA/ASD/rub/HOCM/prosthetic examples.

### Gaps
- No shippable recording located for MS with opening snap, PDA, ASD, pericardial rub, HOCM, prosthetic valves; I did not search open-access case reports with audio supplements (PMC), which is the most likely place to find CC-BY single examples.
- Whether the HLS-CMDS "Atrioventricular Block" track represents complete heart block is not stated on the landing page.

---

## 5. Attribution requirements for the shippable items

### Takeaway
CC0/PD files need no credit (credit is still good practice); CC BY items need author, title/source, licence name + link, and an indication of changes; the datasets additionally ask for a specific paper/DOI citation.

### Cited Findings
- **SensSmartTech (CC BY 4.0)**: cite "Lazović, A., Tadić, P., Đorđević, N., Atanasoski, V., Tiosavljevic, M., Ivanovic, M., Hadzievski, L., Ristic, A., Vukcevic, V., & Petrovic, J. (2024). SensSmartTech database of cardiovascular signals synchronously recorded by an electrocardiograph, phonocardiograph, photoplethysmograph and accelerometer (version 1.0.0). PhysioNet. https://doi.org/10.13026/fy9p-n277" plus the standard PhysioNet citation — [PhysioNet](https://www.physionet.org/content/senssmarttech/)
- **HLS-CMDS (CC BY 4.0)**: cite Y. Torabi, S. Shirani, J. P. Reilly, "Descriptor: Heart and Lung Sounds Dataset Recorded from a Clinical Manikin using Digital Stethoscope (HLS-CMDS)," IEEE Data Descriptions, doi:10.1109/IEEEDATA.2025.3566012; dataset doi:10.17632/8972jxbpmp.3 — [Mendeley Data](https://data.mendeley.com/datasets/8972jxbpmp/3)
- **JUST regurgitation dataset (CC BY 4.0)**: contributors Mohammad Fraiwan, Ali Ibnian, Nishi Haider; doi:10.17632/jfbdvjw6sf.1 (Version 1, 29 Jul 2025) — [Mendeley Data](https://data.mendeley.com/datasets/jfbdvjw6sf/1)
- **Kudriavtsev clips (CC BY 2.0)**: licence condition is that "the original work is properly cited": Kudriavtsev V, Polyshchuk V, Roy DL. "Heart energy signature spectrogram for cardiovascular diagnosis." BioMedical Engineering OnLine 2007;6:16, doi:10.1186/1475-925X-6-16, © 2007 Kudriavtsev et al; licensee BioMed Central Ltd., CC BY 2.0 — [PMC1899182](https://europepmc.org/article/PMC/PMC1899182)
- **Commons CC0/PD files** (HROgg.ogg — James Heilman, MD; athlete files — Athletearrhythmia; Human heart rate.flac — Wilfredor; Infant Heartbeat.ogg — Huckelberry A. Finne; mitral 150 bpm — ezwa / pdsounds.org): no attribution condition under CC0/PD — [HROgg.ogg](https://commons.wikimedia.org/wiki/File:HROgg.ogg)
- **Commons CC BY 3.0 Freesound-derived files**: credit the Freesound author as named on the file page (e.g. "Benboncan" at Freesound.org, with link to the original sound) and the licence — [file page](https://commons.wikimedia.org/wiki/File:Human_heart_beating_at_61_bpm_(Cc-by-3.0).ogg)
- **If ODC-By sets are admitted**: ship the licence URI with the data, keep notices, and add the produced-work notice "Contains information from [DATABASE NAME] which is made available under the ODC Attribution License."; cite the dataset papers listed in section 1 — [ODC-By v1.0](https://physionet.org/content/challenge-2016/view-license/1.0.0/)

### Inferences
- A single `assets/…/ATTRIBUTION` table with columns file · author · source URL/DOI · licence · changes made (resampled, trimmed, loudness-normalised) satisfies every item above; converting OGG/WAV, resampling or trimming counts as a modification that CC BY requires to be indicated.

### Gaps
- The JUST dataset page does not give a formatted "how to cite"; the Data in Brief article should be cited alongside the DOI (full reference not captured here beyond authors/journal/year: Fraiwan et al., Data in Brief, 2025, PMC12504997).

---

## 6. Simultaneous ECG + PCG datasets for validating R-peak→S1 and T-end→S2 timing

### Takeaway
Four open sets pair ECG with PCG: **SensSmartTech** (CC BY 4.0; leads I, II, V3, V4; rest and post-exercise, HR 58–173 bpm) is both shippable and the best fit for rate-dependent timing; **EPHNOGRAM**, **HeartCycle** and **Auricle** (all ODC-By 1.0) add long stress-test records, 44.1 kHz PCG with echo/ICG, and heart-failure patients respectively.

### Cited Findings
- SensSmartTech: 338 × 30 s, 32 healthy adults, ECG I/II/V3/V4 at 500 Hz + PCG at 1 kHz, WFDB copy on a common 1 kHz time axis, before/after exercise with HR recorded in Demographics.csv; CC BY 4.0 — [PhysioNet senssmarttech](https://www.physionet.org/content/senssmarttech/)
- EPHNOGRAM: 24 healthy men, 3-lead ECG + PCG at 8 kHz, 61–62 thirty-minute stress-test records (rest, walking, running, cycling) + 8 thirty-second rest records; ODC-By v1.0 — [PhysioNet ephnogram](https://physionet.org/content/ephnogram/1.0.0/)
- HeartCycle: 17 healthy volunteers, PCG 44.1 kHz with ECG, impedance cardiography and echocardiography (valve-event ground truth); ODC-By v1.0 — [PhysioNet heartcycle](https://www.physionet.org/content/heartcycle/)
- Auricle: 42 healthy + 28 heart-failure adults, 3-lead ECG + stethoscope PCG + SCG + continuous arterial pressure at 1 kHz, with breath-hold / exercise / Valsalva / cold-pressor; ODC-By v1.0; published 13 Oct 2026 — [PhysioNet auricle](https://physionet.org/content/auricle/1.0.0/)
- A 2026 arXiv dataset with 15 synchronized ECG+PCG recordings and blood pressure exists but its hosting/licence was not established — [arXiv 2605.23516](https://arxiv.org/pdf/2605.23516)
- CirCor releases no ECG — [CirCor](https://physionet.org/content/circor-heart-sound/1.0.3/)

### Inferences
- For validating a synthesiser's electromechanical delays (R→S1) and QT-linked S2 timing (T-end→S2) across heart rates, SensSmartTech + EPHNOGRAM cover healthy subjects from rest to ~170 bpm; using them to *derive timing parameters* does not require redistributing them, so the ODC-By question does not arise for that use.
- HeartCycle's echo/ICG channels allow checking S1/S2 against actual valve events, not only ECG fiducials; Auricle is the only one with a diseased (heart-failure) cohort, where S3 and prolonged electromechanical delay might be observable.
- All four are healthy or heart-failure cohorts; none provides simultaneous ECG+PCG in valvular disease, AF or heart block.

### Gaps
- PCG 1 kHz in SensSmartTech and Auricle limits bandwidth to 500 Hz — adequate for S1/S2 timing, marginal for high-pitched murmurs.
- Lead configuration of EPHNOGRAM's "three-lead ECG" is not named on the landing page; the PCG site description in SensSmartTech is internally inconsistent (apex vs sternum).
- Whether any CinC 2016 sub-database retains its original simultaneous ECG was not confirmed (see section 1 gaps).
