# Candidate real heart-sound clips for the reference library (brief B28, Part C, item 9)

Status (2026-11-21): **the maintainer approved C1 and the Kudriavtsev clips S1, S2, S5, S6, S7, S9; these are shipped**
(item 10, `assets/sounds/reference/`, `ASSETS-LICENSES.md`). Everything else on the list is not shipped. Item 11 (timing
validation against SensSmartTech) is not done.

| Candidate | Status |
|---|---|
| C1 `HROgg.ogg` (CC0 1.0) | **Shipped** as `normal-heart-sounds.wav` (four cycles, 3.61 s); licence re-read on the Commons page on the day of download |
| K1, K2 (innocent Still's murmur) | **Shipped** as `innocent-stills-1.wav`, `innocent-stills-2.wav` (whole clips); CC BY 2.0 re-read on each Commons page |
| K5 (mitral valve prolapse, clicks) | **Shipped** as `mvp-clicks.wav` |
| K6 (aortic stenosis) | **Shipped** as `aortic-stenosis.wav` |
| K7 (pulmonary stenosis) | **Shipped** as `pulmonary-stenosis.wav` |
| K9 (ventricular septal defect) | **Shipped** as `vsd.wav` |
| K3 (simulated S2 split), K8 (tetralogy) | Not approved; not downloaded (K3 is synthetic, K8 has no matching finding) |
| C2, C3, C4, C5 | Not approved; not downloaded |
| J1-J4 (JUST regurgitation, CC BY 4.0) | Not approved; not downloaded; the aortic, mitral and tricuspid regurgitation findings stay model-only |
| P1, P2 (SensSmartTech) | Not approved; not downloaded; no clip with an ECG ships |

The conversion is `docs/research/heart-sound-clips-convert.py` (GStreamer decode, mono, 4 kHz, cut at quiet points, 5 ms
fades, peak at -6 dBFS); the exact arguments per file are in `assets/sounds/reference/index.ron`. The originals have
full-scale peaks, so the -6 dBFS normalisation lowers them by about 6 dB. The clips were cut and checked by waveform
(envelope and cut-point energy), not by ear. The Kudriavtsev clips are whole and some begin or end close to a sound, so the loop
joint is a 5 ms fade rather than a cut in silence.

The original candidate text follows, as read on 2026-11-21 before approval.

Every licence below was read on the primary page on **2026-11-21** (Wikimedia Commons file pages and
the Commons API's `extmetadata`, the PMC article page, the Mendeley Data record, the PhysioNet project
page), not taken from `docs/research/heart-sounds-notes/datasets_licences.md`. Where a primary page
could not be read, the entry says so. Only CC0, public-domain and CC BY material is listed (brief
non-negotiable "Legal (Part C)"); ODC-By sets (CinC 2016, CirCor, EPHNOGRAM, HeartCycle, Auricle),
share-alike, non-commercial and unlicensed sources, and the HLS-CMDS manikin set are excluded and not
listed. Nothing comes from `epicardio/`.

**Planned edit, common to every clip** (item 10, after approval): decode once on the maintainer's
machine to 16-bit mono PCM WAV at 4 kHz (anti-alias low-pass before decimation), trim to a whole
number of cycles (3 to 6 s where the source allows; the shortest Kudriavtsev clips stay whole),
5 ms fades at the cut points, peak-normalise to −6 dBFS. These are changes to be stated in
`ASSETS-LICENSES.md` ("converted to 16-bit 4 kHz mono WAV, trimmed, faded and normalised by the
heartsim authors"), with author, source URL or DOI, licence, retrieval date and SHA-256.

## 1. Wikimedia Commons, CC0 / public domain

| # | File (source page) | Author | Licence as read on the page | Content | Length | Quality (from the page) | Planned edit |
|---|---|---|---|---|---|---|---|
| C1 | [File:HROgg.ogg](https://commons.wikimedia.org/wiki/File:HROgg.ogg) | James Heilman, MD (User:Jmh649), "Own work" | "This file is made available under the Creative Commons CC0 1.0 Universal Public Domain Dedication." | "Normal heart sounds at about 70 BPM" | 19.5 s | Ogg Vorbis, 31 kbps, 73 kB; sample rate not stated | Trim to about 4 cycles of the cleanest stretch; the normal S1/S2 reference beside the "S1 and S2" finding |
| C2 | [File:Athletic bradycardia wav.wav](https://commons.wikimedia.org/wiki/File:Athletic_bradycardia_wav.wav) | "Athletearrhythmia" (user page does not exist), "Own work" | CC0, "Creative Commons Zero, Public Domain Dedication" (Commons metadata) | "Resting heart rate recorded in an elite athlete demonstrating bradycardia at 42 bpm" | 20.1 s | WAV, 1.9 MB | 3 cycles; a slow normal rhythm, long diastole. Optional: the author is anonymous, provenance cannot be checked further |
| C3 | [File:Infant Heartbeat.ogg](https://commons.wikimedia.org/wiki/File:Infant_Heartbeat.ogg) | Huckelberry A. Finne (User:Huckfinne), "Own work" | "Public domain" | "Short recording of my nine day old daughter's non-pathological heartbeat." | 9.1 s | Ogg, 235 kB; recording device not stated | Optional (paediatric rate, not a teaching finding of the trainer); likely leave out |
| C4 | [File:Human heart rate.flac](https://commons.wikimedia.org/wiki/File:Human_heart_rate.flac) | Wilfredor, "Own work" | CC0, "Creative Commons Zero, Public Domain Dedication" | "Human heart rate" (no site, rate or device given) | 71.9 s | FLAC, 2.9 MB | Not recommended: no clinical description; only if C1 proves unusable |
| C5 | [File:Heartbeat mitral valve 150 bpm.ogg](https://commons.wikimedia.org/wiki/File:Heartbeat_mitral_valve_150_bpm.ogg) | "ezwa", via PDSounds record 158, transferred by Fæ | "Public domain" | "Fast heartbeat (150 bpm) recorded after a bit of exercise. Location: mitral valve." | 11.2 s | Ogg, 87 kB | Optional: a sinus tachycardia at the apex. The primary page is pdsounds.org (record 158), which was **not** read; the licence here is the Commons statement |

## 2. Kudriavtsev, Polyshchuk & Roy 2007, CC BY 2.0 (paediatric clinical recordings)

Source article: Kudriavtsev V, Polyshchuk V, Roy D. Heart energy signature spectrogram for
cardiovascular diagnosis. *BioMedical Engineering OnLine* 2007;6:16. doi:10.1186/1475-925X-6-16.
Licence as read on the [PMC article page](https://pmc.ncbi.nlm.nih.gov/articles/PMC1899182/):
"Copyright © 2007 Kudriavtsev et al; licensee BioMed Central Ltd. This is an Open Access article
distributed under the terms of the Creative Commons Attribution License
(creativecommons.org/licenses/by/2.0), which permits unrestricted use, distribution, and
reproduction in any medium, provided the original work is properly cited." Provenance, from the
same page: recorded "between 1980–91" at the "Department of Pediatric Cardiology, IWK Center at
Dalhousie University, Canada", "recorded at 11 kHz sampling rate"; "no artificial or simulated
sounds have been used" for the clinical recordings. (The Europe PMC copy, the address in the research
note, returned HTTP 403; the PMC page above was read instead.) Each additional file is on Commons,
where the page states "This file is licensed under the Creative Commons Attribution 2.0 Generic
license", Artist "Kudriavtsev V, Polyshchuk V, Roy D", imported by the Open Access Media Importer Bot.
Attribution text: "Kudriavtsev V, Polyshchuk V, Roy D (2007), BioMed Eng OnLine 6:16, CC BY 2.0;
converted and trimmed by heartsim".

| # | Commons file (suffix of `Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-`) | Content (Commons description) | Length | Quality | Planned edit / use |
|---|---|---|---|---|---|
| K1 | [S1.ogg](https://commons.wikimedia.org/wiki/File:Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-S1.ogg) | "Innocent Still's Murmur 1 … recorded at the apex." | 2.6 s | Ogg, 10 kB; 1980s tape digitised at 11 kHz, child's heart rate | Whole clip; "Real recording" beside the innocent murmur |
| K2 | [S2.ogg](https://commons.wikimedia.org/wiki/File:Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-S2.ogg) | "Innocent Stills Murmur 2 … recorded at LSB, 2nd space." | 1.7 s | Ogg, 9 kB | Whole clip; second innocent example |
| K3 | [S3.ogg](https://commons.wikimedia.org/wiki/File:Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-S3.ogg) | "Test file for S2 split detection … realistically **simulated** two S2 heart sound splits" | 0.56 s | Ogg, 5 kB | **Do not ship as a real recording**: it is synthetic. Leave out |
| K5 | [S5.ogg](https://commons.wikimedia.org/wiki/File:Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-S5.ogg) | "Mitral Valve Prolapse with 3 clicks." | 1.2 s | Ogg, 7 kB | Whole clip; beside the mid-systolic click |
| K6 | [S6.ogg](https://commons.wikimedia.org/wiki/File:Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-S6.ogg) | "Aortic Stenosis." (paediatric) | 1.1 s | Ogg, 7 kB | Whole clip; beside aortic stenosis, labelled paediatric |
| K7 | [S7.ogg](https://commons.wikimedia.org/wiki/File:Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-S7.ogg) | "Pulmonary Stenosis." | 1.5 s | Ogg, 8 kB | Whole clip; beside pulmonary stenosis |
| K8 | [S8.ogg](https://commons.wikimedia.org/wiki/File:Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-S8.ogg) | "Tetralogy of Fallot (acyanotic)." | 1.6 s | Ogg, 8 kB | No matching finding in the trainer; leave out unless a tetralogy case is added |
| K9 | [S9.ogg](https://commons.wikimedia.org/wiki/File:Heart-energy-signature-spectrogram-for-cardiovascular-diagnosis-1475-925X-6-16-S9.ogg) | "Ventricular Septal Defect (VSD)." | 2.0 s | Ogg, 7 kB | Whole clip; beside the VSD murmur |

All are short (1–2.6 s, one to three cycles) and lossy Ogg copies of 11 kHz digitised tape; they
loop as they are, with a 5 ms fade at the joint. The paper names the original files as additional
files of the article; the Commons copies are the ones checked here.

## 3. JUST heart-sound regurgitation dataset, CC BY 4.0 (adult clinical recordings)

Fraiwan M, Ibnian A, Haider N. A Dataset of Heart Sound Regurgitation of Patients with Heart Valve
Disorders. Mendeley Data, V1, published 29 July 2025. doi:10.17632/jfbdvjw6sf.1.
Licence as read on the [Mendeley Data record](https://data.mendeley.com/datasets/jfbdvjw6sf/1):
**CC BY 4.0**. The record states a "3m Littman Electronic Stethoscope model 3200" and that this is
"the raw version" (device filtering only). The record page did not list the files, their format,
sampling rate or length (the Files section rendered empty to the reader). The companion Data in
Brief article ([PMC12504997](https://pmc.ncbi.nlm.nih.gov/articles/PMC12504997/)) gives 104
participants, **17 AR, 68 MR, 84 TR, 230 normal** recordings as `.wav` (raw and filtered) at the
aortic, mitral and tricuspid areas; that article itself is "CC BY-NC" (verbatim: "This is an open
access article under the CC BY-NC license"), so **only the audio files from the Mendeley record**,
under its CC BY 4.0, would ship, and none of the article's text or figures.

| # | Content | Length | Quality | Planned edit |
|---|---|---|---|---|
| J1 | One aortic regurgitation recording, aortic or Erb's area (pick by listening) | not stated (to be read from the file) | Littmann 3200, raw | 3–5 cycles; beside aortic regurgitation |
| J2 | One mitral regurgitation recording, mitral area | not stated | as above | 3–5 cycles; beside mitral regurgitation |
| J3 | One tricuspid regurgitation recording, tricuspid area | not stated | as above | 3–5 cycles; beside tricuspid regurgitation |
| J4 | Optional: one normal recording, mitral area | not stated | as above | a second normal adult reference |

Open point: the file listing, rates and lengths must be read when the archive is downloaded
(item 10); the choice of individual recordings is by ear, so the list names a class per slot, not a
file.

## 4. SensSmartTech (PhysioNet), CC BY 4.0: timing validation, optionally a normal clip

Lazović A, Tadić P, Đorđević N, Atanasoski V, Tiosavljevic M, Ivanovic M, Hadzievski L, Ristic A,
Vukcevic V, Petrovic J. SensSmartTech database of cardiovascular signals synchronously recorded by
an electrocardiograph, phonocardiograph, photoplethysmograph and accelerometer. PhysioNet, version
1.0.0, 19 Dec 2024, doi:10.13026/fy9p-n277 (the page names a newer version, doi:10.13026/z1de-fn10).
Licence as read on the [PhysioNet page](https://physionet.org/content/senssmarttech/): open access,
"Creative Commons Attribution 4.0 International Public License". 338 recordings of 30 s, 32 healthy
volunteers, 10 channels (4 ECG, 1 PCG at 1 kHz, 4 PPG, 1 accelerometer), 775 MB (264.9 MB zip).

| # | Use | Planned edit |
|---|---|---|
| P1 | Item 11, timing validation (QRS-to-S1, T-end-to-S2 across heart rates): a local copy only, nothing shipped | none (test-only tool, skipped when the data is absent) |
| P2 | Optional shipped clip: one rest recording, PCG with its simultaneous ECG lead II, as the only real clip with an ECG | PCG at 1 kHz is band-limited to 500 Hz: resample to 4 kHz, 4 cycles, keep the ECG excerpt for the trace |

## Recommendation for the approval

Ship, if approved: C1 (normal), K1, K2, K5, K6, K7, K9 (innocent ×2, MVP, AS, PS, VSD), J1–J3
(AR, MR, TR), and optionally P2 (normal with ECG). That covers ten trainer findings with a real
example and leaves the simulator's own model as the only source for S3, S4, the splits, MS and the
rub, as the research note expected. Leave out K3 (simulated), K8 (no matching finding), C3–C5
(weak descriptions or unread primary page). The total would be well under 1 MB at 4 kHz 16-bit,
inside the web size budget.

Decisions still with the maintainer: whether to ship real clips at all; the list above; the ODC-By
policy and the manikin set (HLS-CMDS) stay outside this brief.
