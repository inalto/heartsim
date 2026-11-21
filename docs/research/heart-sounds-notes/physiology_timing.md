# Physiology and timing of heart sounds relative to the ECG and valve events

Scope: adult human, for a simulator that knows per-beat electrical timing (P onset, PR, QRS onset, QRS duration, BBB, pacing site, RR) but has no mechanical model. All times in ms unless stated.

Access notes (affect confidence):
- **Read in full text**: Clinical Methods 3rd ed. chapters 22-28 (via Internet Archive copies of the NCBI Bookshelf pages), Springer et al. 2016 PDF and its PhysioNet code, Sbrollini et al. 2017 (CinC), Orter et al. 2022, Luo et al. 2021 (PMC7785055), Nemani & Pechetty 2020 (Part 2), MSD Manual "Cardiac auscultation", Univ. of Washington physical-diagnosis page, Healio S2 page.
- **Seen only as search-engine extracts (not opened; publisher returned 403 or CAPTCHA)**: Weissler 1968, Lewis 1977, Boudoulas 1979, Leech 1980, Burggraf & Craige 1974, the JAMA "Genesis of heart sounds II" review, Nemani Part 1 (S3/S4), McKusick 1957 symposium, and the frequency-content papers. These are marked "(extract)" below; numbers should be re-checked against the PDF before being hard-coded as "cited from the paper".

---

## 1. Electromechanical delays: QRS onset to S1 (M1, T1) and to S2 (A2, P2); heart-rate, contractility and age dependence

### Takeaway
S1 (M1) begins about 40-45 ms after QRS onset in a normal adult (earlier and softer with long PR, later and louder with short PR), with T1 20-30 ms after M1. A2 falls at QRS onset + QS2, where QS2 is about 546 - 2.1·HR ms (men) or 549 - 2.0·HR ms (women); this is essentially the end of the T wave (S2 onset about 5 ms after T-offset in normal subjects).

### Cited Findings

**Q-S1 (electromechanical activation time)**
- With PR 180-190 ms, mitral closure occurs roughly 40 ms after QRS onset; the shorter the PR, the later and louder the closure sound; with a very long PR, mitral closure occurs soon after QRS onset and is silent (extract; Leech, Brooks, Green-Wilkinson, Leatham, Br Heart J 1980;43:138-142, doi:10.1136/hrt.43.2.138) — [Leech 1980, PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC482253)
- A 2025 synchronized ECG/PCG study measured mean Q-S1 = 40.45 ms against a literature reference of about 45 ms (extract) — [PubMed 40648475](https://pubmed.ncbi.nlm.nih.gov/40648475/)
- Q-S1 (QRS onset to S1 onset) is also called electromechanical activation time (EMAT) and is used for AV-delay optimisation in CRT (extract) — [Heart Rhythm 2022](https://www.heartrhythmjournal.com/article/S1547-5271(22)02795-3/fulltext)
- In first-degree block (PQ > 200 ms) the AV valves have already begun to close, "S1 occurs earlier" and is softer — [Felner, Clinical Methods ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- The signal-processing convention treats the *start of S1 as coincident with the ECG R-peak* and labels R-peak → R-peak + mean S1 duration as S1; segmentation is scored correct if S1 onset is within 100 ms of the R-peak — [Springer, Tarassenko, Clifford, IEEE TBME 2016, doi:10.1109/TBME.2015.2475278](https://moody-challenge.physionet.org/2022/refs/LR_HSMM_Segmentation_IEEE_Springer_Tarassenko_Clifford.pdf). This is a labelling approximation, coarser than the 40 ms figure above.

**M1-T1**
- S1 has two audible components, M1 then T1; M1-T1 normally 20-30 ms; M1 is associated with the abrupt rise of LV pressure at the start of isovolumic contraction — [Felner, Clinical Methods ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- The ear resolves two sounds separated by as little as 20 ms — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- T1 is much softer than M1 because of the lower right-sided pressures (extract) — [Healio, S1 topic review](https://cm.healio.com/cardiology/learn-the-heart/cardiology-review/topic-reviews/s1-heart-sound)

**QS2 (QRS onset to A2), LVET, PEP: Weissler regressions**
- QS2 = 546 - 2.1·HR ms (men); QS2 = 549 - 2.0·HR ms (women), HR in beats/min (extract, quoted in Boudoulas et al., Circulation 1979;60:164) — [Boudoulas 1979](https://www.ahajournals.org/doi/pdf/10.1161/01.cir.60.1.164); male equation also quoted as "QS2 = -2.1 HR + 546 msec; 546 is the normal QS2 index" (extract) — [Circulation 1972;46:856](https://www.ahajournals.org/doi/pdf/10.1161/01.CIR.46.5.856)
- Heart-rate correction indices (read directly, Table 1): LVETI = LVET + 1.7·HR (men), + 1.6·HR (women); PEPI = PEP + 0.4·HR (both sexes); QS2I = QS2 + 2.1·HR (men), + 2.0·HR (women) [paper gives them in seconds: 0.0017, 0.0016, 0.0004, 0.0021, 0.0020] — [Orter et al., Exp Physiol 2022, doi:10.1113/EP090103](https://elib.dlr.de/190261/1/EPH-107-213.pdf)
- All three intervals vary inversely with HR; slope largest for QS2 and LVET, smallest for PEP; Weissler's equations are from the largest normal population (211 normals) and are generally adopted (extract) — [Lewis, Rittgers, Forester, Boudoulas, Circulation 1977;56:146, doi:10.1161/01.cir.56.2.146](https://www.ahajournals.org/doi/pdf/10.1161/01.cir.56.2.146)
- Disagreement: a study using atropine to vary HR found significant linear HR regressions for LVET and QS2 but *no significant PEP-HR relationship* — [Br J Clin Pharmacol, PMC1401761](https://pmc.ncbi.nlm.nih.gov/articles/PMC1401761/); another normal group gave QS2 = 548 - 2.33·HR (extract, via patent text) — [US 7174203](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/7174203)
- Weissler's original paper: Weissler AM, Harris WS, Schoenfeld CD. Systolic time intervals in heart failure in man. Circulation 1968;37:149-159 — [doi:10.1161/01.cir.37.2.149](https://doi.org/10.1161/01.cir.37.2.149) (not opened)

**S2 relative to the T wave**
- In 99 normal PhysioNet/CinC-2016 recordings, S2 onset followed T-wave offset by a median of 5 ms (P = 0.007); regression of S2-onset on T-offset had slope 0.6 and intercept 200 ms, i.e. they are not interchangeable beat-by-beat — [Sbrollini et al., Computing in Cardiology 2017](https://cinc.org/archives/2017/pdf/085-076.pdf)
- "S2 occurs at approximately the end-T-wave"; S2 centre is found as the envelope maximum within end-T ± (mean S2 duration + 1 SD); tolerance 100 ms — [Springer 2016](https://moody-challenge.physionet.org/2022/refs/LR_HSMM_Segmentation_IEEE_Springer_Tarassenko_Clifford.pdf)
- In normal subjects QT is consistently slightly shorter than QS2 and parallels it across resting HR; QT > QS2 is abnormal ("QT > QS2 syndrome", Boudoulas 1982) and adrenergic stimulation raises QT/QS2 by shortening QS2 (extracts) — [Boudoulas, Eur J Heart Fail 2021](https://onlinelibrary.wiley.com/doi/full/10.1002/ejhf.2165); [van der Linde, Br J Pharmacol 2010](https://bpspubs.onlinelibrary.wiley.com/doi/full/10.1111/j.1476-5381.2010.00934.x)
- A2 and P2 coincide with the incisurae of the aortic and pulmonary pressure curves; the "hangout" between ventricular and arterial pressure is ≤ 5 ms on the left and 30-120 ms on the right, which is why P2 follows A2 — [Felner, Clinical Methods ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)

**Contractility, disease, age**
- Heart failure: prolonged PEP, shortened LVET, raised PEP/LVET (title finding of Weissler 1968; described as the "Weissler index" PEP/LVET) — [Orter 2022](https://elib.dlr.de/190261/1/EPH-107-213.pdf); [Weissler 1968](https://doi.org/10.1161/01.cir.37.2.149)
- 60 days head-down bed rest (deconditioning): LVETI fell, PEPI and PEP/LVET rose (P < 0.005) — [Orter 2022](https://elib.dlr.de/190261/1/EPH-107-213.pdf)
- Hyperadrenergic states (exercise, anxiety, fever, anaemia, pregnancy, thyrotoxicosis) give a faster ventricular pressure rise and a louder S1; myocardial depression (cardiomyopathy, MI, shock, myxoedema) gives a softer S1 — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- Children: Q-to-first-sound interval is roughly constant across age/HR; QS2 and LVET correlate strongly with HR (extract) — [Circulation 1974;49:1107](https://www.ahajournals.org/doi/10.1161/01.cir.49.6.1107)

### Inferences
- Scheduling rule (my synthesis, not a published model): `t_M1 = t_QRS_onset + ~40 ms` (range roughly 20-60 ms, modulated by PR, see section 5); `t_T1 = t_M1 + 20-30 ms`; `t_A2 = t_QRS_onset + QS2(HR)` with Weissler's equation; `t_P2 = t_A2 + split` (section 3).
- Worked values with the male equation: HR 60 → QS2 420 ms; HR 75 → 388 ms; HR 100 → 336 ms; HR 150 → 231 ms. S1-S2 interval is then QS2 minus ~40 ms.
- The HR to use is ambiguous for irregular rhythms. Weissler's regressions were fitted on steady-state HR; for AF/ectopy a simulator should probably use the preceding RR (or a short average) and should prefer anchoring A2 to the simulated T-wave end (+0 to +20 ms) when a repolarisation model exists, since QT and QS2 track each other and S2 onset ≈ T-offset + 5 ms.
- A contractility knob can be modelled as a PEP change (and matching Q-S1/S1 loudness change): positive inotropy shortens PEP and QS2 at a given HR; failure lengthens PEP and shortens LVET with QS2 nearly unchanged.

### Gaps
- Weissler's LVET and PEP *intercepts* and SDs could not be verified from an opened source. From memory (unverified, do not cite): LVET = 413 - 1.7·HR (M), 418 - 1.6·HR (F); PEP = 131 - 0.4·HR (M), 133 - 0.4·HR (F); QS2I SD about 14 ms, LVETI SD about 10 ms, normal PEP/LVET about 0.35 ± 0.04. The slopes are confirmed (Orter 2022). Check Circulation 37:149 Table 2 or Lewis 1977.
- A teaching page surfaced in search stating "S1 occurs approximately 0.02-0.04 s after QRS onset"; I could not identify which page, so it is unattributed.
- No opened source gave Q-S1 as a function of QRS duration, LBBB or pacing site, nor a normative range/SD for Q-S1.
- No quantitative age regression for QS2/LVET in adults was retrieved (Lewis 1977 is the place to look).
- R-peak-referenced (rather than Q-onset-referenced) S1 delays with mean ± SD were not found; Springer and the CinC-2016 paper (Liu et al., Physiol Meas 2016, PMC7199391, not retrievable) only use R-peak as an anchor with a 100 ms tolerance.

---

## 2. Durations, frequency content, relative intensities and best listening sites of S1-S4

### Takeaway
S1 is about 120 ms long and S2 about 90 ms, with most energy below 150 Hz and a spectral peak near 50 Hz (S2 slightly higher-pitched and shorter than S1); S3 and S4 are short, faint, very low-frequency sounds (about 20-50 Hz) at the apex. Published frequency bands disagree widely because of differing sensors and definitions.

### Cited Findings

**Durations**
- Mean S1 duration 122 ms (SD 22 ms); mean S2 duration 94 ms (SD 22 ms) (values attributed by Springer to Schmidt et al. 2010, read from the reference implementation: 0.122 s and 0.094 s); systole-interval SD 25 ms; diastole SD = 0.07·mean + 6 ms; shortest expected fundamental heart sound 50 ms — [PhysioNet hss 1.0, get_duration_distributions.m](https://physionet.org/files/hss/1.0/get_duration_distributions.m); [Springer 2016](https://moody-challenge.physionet.org/2022/refs/LR_HSMM_Segmentation_IEEE_Springer_Tarassenko_Clifford.pdf)
- S1 70-150 ms, S2 60-120 ms (extract) — [Heart sound biometric system, PMC3649396](https://pmc.ncbi.nlm.nih.gov/articles/PMC3649396/)
- S1 and S2 last 25-150 ms (extract) — [MDPI Technologies 2025;13:147](https://www.mdpi.com/2227-7080/13/4/147)
- A2 and P2 are each modelled as transient chirps of ≤ 60 ms in the widely used Xu et al. 2000 S2 model — [Luo et al., Physiol Rep 2021, doi:10.14814/phy2.14687](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7785055/)

**Frequency content**
- Majority of S1/S2 energy is below 150 Hz with a peak at 50 Hz; optimal analysis wavelets had centre frequencies 65-85 Hz; a 40-60 Hz PSD band is a good S1/S2 detector — [Springer 2016](https://moody-challenge.physionet.org/2022/refs/LR_HSMM_Segmentation_IEEE_Springer_Tarassenko_Clifford.pdf)
- Disagreeing bands (all extracts from secondary papers): S1 25-45 Hz and S2 about 50 Hz, with marginal-spectrum peaks S1 20-40 Hz, S2 80-120 Hz — [PMC3649396](https://pmc.ncbi.nlm.nih.gov/articles/PMC3649396/); S1 50-150 Hz, S2 50-200 Hz (also S1 40-60, S2 60-100 Hz) — [PMC8749711](https://pmc.ncbi.nlm.nih.gov/articles/PMC8749711/); S1/S2 content mainly 24-144 Hz — [MDPI 2025](https://www.mdpi.com/2227-7080/13/4/147); murmurs 200-600 Hz — [PMC3649396](https://pmc.ncbi.nlm.nih.gov/articles/PMC3649396/)
- S1 is "lower-pitched, duller, and longer" than S2 and has a booming quality — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- A 100 Hz and a 200 Hz mother wavelet are used to resolve A2/P2 — [Luo 2021](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7785055/)
- S4: low-frequency 20-30 Hz vibrations generated within the ventricle — [Williams, Clinical Methods ch. 25](https://www.ncbi.nlm.nih.gov/books/NBK344/)
- S3: low-pitched, 25-50 Hz (extract) — [Nemani, Additional Heart Sounds Part 1, 2020](https://d-nb.info/1220535346/34); S3 is a "low-frequency, brief vibration", lower in frequency than S2 — [Silverman, ch. 24](https://www.ncbi.nlm.nih.gov/books/NBK342/); [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)

**Relative intensity and location**
- Listening areas: aortic = 2nd right intercostal space (ICS); pulmonic = 2nd left ICS; tricuspid = lower left sternal border (4th ICS); mitral = apex — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/); "secondary aortic area" 3rd left ICS (Erb's point region) is named for AS/AR murmurs — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/)
- S1 is usually louder than S2 at the apex; at the base both components of S2 are normally louder than S1; T1 and S1 splitting are best heard at the lower left sternal border — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- A2 is much louder than P2, loudest at the right upper sternal border and radiates to all areas; P2 is normally heard only at the left upper sternal border, so S2 splitting is heard at the pulmonic area — [Healio, S2](https://www.healio.com/cardiology/learn-the-heart/cardiology-review/topic-reviews/s2-heart-sound)
- P2 is abnormal in an adult > 20 y if louder than A2 at the 2nd left ICS or audible at the apex; P2 is relatively loud in the young; A2 is reduced in aortic stenosis, P2 in COPD and pulmonic stenosis — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)
- S3: best at the apex (LV), not usually heard at the base; LV S3 best in expiration in left lateral decubitus, RV S3 best in inspiration supine — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- S4: soft, dull/thudding, most prominent at the apex with the bell lightly applied, patient in left lateral position; suppressed by firm pressure; RV S4 increases with inspiration — [Williams, ch. 25](https://www.ncbi.nlm.nih.gov/books/NBK344/)
- S1 is louder in young people and thin chests, attenuated by emphysema — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)

### Inferences
- A defensible synthesis default: S1 = 100-140 ms burst centred about 40-60 Hz; S2 = 70-100 ms burst centred about 60-90 Hz, built from two ≤ 60 ms chirps (A2, P2); S3 = 40-60 ms at 25-50 Hz; S4 = 40-60 ms at 20-30 Hz. S3/S4 durations are my estimate (see Gaps).
- Per-site mix (qualitative only, from the statements above): apex S1 > S2, S3/S4 present, P2 absent; aortic area A2 dominant, S1 soft; pulmonic area A2 + P2 both present (split audible), S1 soft; tricuspid area T1 audible (S1 split), RV S3/S4.
- Laptop/phone speakers reproduce 20-50 Hz poorly, so S3/S4 will need pitch-shifting or harmonics to be audible in an education product (engineering inference).

### Gaps
- No opened primary source gave S3/S4 durations in ms or relative amplitudes in dB versus S1/S2, nor a quantified per-site amplitude ratio (S1/S2 at apex vs base).
- The frequency numbers are from heterogeneous secondary sources; classical phonocardiography texts (Luisada; Tavel, *Clinical Phonocardiography*; Leatham) were not accessible. Treat any single Hz band as indicative.
- Erb's point specifics (3rd left ICS, where S2 components and AR murmur are well heard) are only indirectly supported by the Clinical Methods "secondary aortic area" wording.

---

## 3. Splitting of S2 (physiological, wide, fixed, paradoxical) and of S1

### Takeaway
Normal A2-P2 is < 30 ms in expiration (heard single) and 20-80 ms (mean 30-40; 40-50 in the young) in inspiration; expiratory splitting > 40 ms is abnormal. Delayed RV activation (RBBB, LV pacing/ectopy) gives a persistent wide split that still widens on inspiration; ASD gives a wide split that changes < 20 ms; delayed LV activation or ejection (LBBB, RV pacing, severe AS, HOCM) reverses the order so the split is maximal in expiration.

### Cited Findings

**Physiological**
- Inspiration: A2 precedes P2 by 20-80 ms (mean 30-40 ms) in quiet respiration; younger subjects average 40-50 ms. Expiration: superimposed, rarely as much as 40 ms; > 40 ms in expiration is usually abnormal; audible expiratory splitting implies ≥ 30-40 ms — [Felner, Clinical Methods ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)
- Expiration < 30 ms, heard as single; maximal inspiratory split 40-50 ms in the young, decreasing with age; at ≥ 40 y a single S2 in both phases may be normal — [Univ. of Washington, Advanced Physical Diagnosis](https://depts.washington.edu/physdx/heart/path.html)
- Mechanism: inspiration delays P2 (major effect, longer RV ejection and hangout) and makes A2 slightly earlier (minor effect) — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/); [Healio, S2](https://www.healio.com/cardiology/learn-the-heart/cardiology-review/topic-reviews/s2-heart-sound)
- Many normal people over 50 have a single S2 or a narrow inspiratory split (P2 earlier as pulmonary impedance rises with age); persistent expiratory splitting can be normal when *recumbent* in children and young adults — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)
- Audibility thresholds: splitting is detectable by ear at ≥ 40 ms; 20-40 ms only with low noise and an experienced listener — [Luo 2021](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7785055/); below 20 ms S2 is heard single — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)
- S2 is physiologically split in about 90% of people — [Healio, S2](https://www.healio.com/cardiology/learn-the-heart/cardiology-review/topic-reviews/s2-heart-sound)

**Persistent / wide (P2 late or A2 early), respiratory variation preserved**
- RBBB is the commonest cause of persistent expiratory splitting on standing; P2 is delayed by delayed electrical activation of the RV: RBBB, *LV ectopic or LV paced beats*, WPW (left-sided pre-excitation); early A2 with mitral regurgitation, VSD, constrictive pericarditis — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/); [Healio, S2](https://www.healio.com/cardiology/learn-the-heart/cardiology-review/topic-reviews/s2-heart-sound)
- Animal pacing data (pigs with AV block, biventricular pacing): S2 split fell from mean 47 ms with LV pre-excitation to 23 ms with RV pre-excitation as the interventricular relaxation delay went from +8 ms to -18 ms; most of the change occurred between A-LV delays of 100 and 200 ms (A-RV fixed at 150 ms) — [Luo 2021](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7785055/)
- Narrow (< 30 ms) but physiologic split with loud P2 in pulmonary hypertension — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/); [UW](https://depts.washington.edu/physdx/heart/path.html)

**Fixed**
- ASD: wide split with no significant respiratory variation, because the two ventricles share a common venous reservoir and A2 and P2 are delayed about equally on inspiration; after Valsalva release the interval increases by < 20 ms; severe right heart failure can give a relatively fixed split — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)
- Fixed split is described as pathognomonic of ASD; VSD does not cause fixed splitting — [Healio, S2](https://www.healio.com/cardiology/learn-the-heart/cardiology-review/topic-reviews/s2-heart-sound); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)

**Paradoxical / reversed (P2 before A2; maximal in expiration, narrows or disappears in inspiration)**
- Commonest cause is LBBB; also LV outflow obstruction (more often HOCM than AS), first days after acute MI, severe LV dysfunction; becomes a single S2 when the interval falls below 20 ms — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)
- RV pacing, complete LBBB, AS, LVOT obstruction, LV failure, high afterload and (right-sided) pre-excitation delay A2 relative to P2 (extract) — [Seepana & Vala, "Second Heart Sound", 2020](https://d-nb.info/1212596692/34); WPW listed as cause of early P2 — [UW](https://depts.washington.edu/physdx/heart/path.html)
- Single case with numbers: RV pacing (LBBB-pattern QRS) produced paradoxical split of 100 ms in expiration and 70 ms in inspiration, which narrowed after upgrade to biventricular pacing — [Eur Heart J Case Rep 2023;7:ytad233](https://academic.oup.com/ehjcr/article/7/5/ytad233/7150401)
- Severe AS: A2 soft or inaudible, giving a single S2 — [Healio, S2](https://www.healio.com/cardiology/learn-the-heart/cardiology-review/topic-reviews/s2-heart-sound); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)

**S1 splitting**
- M1-T1 normally 20-30 ms; may increase to 60 ms in RBBB, Ebstein's anomaly, ventricular ectopic beats, ventricular tachycardia, AV block with idioventricular rhythm and LV pacing. S1 splitting is *not* characteristic of LBBB "because there is no significant delay in onset of left ventricular contraction" — [Felner, Clinical Methods ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)

### Inferences
- A mechanistic rule consistent with all of the above: schedule M1 and A2 from *LV activation onset* and T1 and P2 from *RV activation onset*. Then `split_S2 = base(resp) + (t_RV_act - t_LV_act)` with base ≈ 10-25 ms in expiration and 40-50 ms in inspiration (sinusoidal with the respiratory phase). RBBB/LV pacing add a positive interventricular delay (wide, still respiratory), LBBB/RV pacing a negative one large enough to flip the sign (P2 first; |split| then shrinks on inspiration because P2 still moves later).
- Magnitudes to target (inference from the single case and the general statements): RBBB expiratory split about 40-70 ms widening by a further 20-30 ms on inspiration; LBBB/RV-pacing reversed split about 40-100 ms in expiration, 0-70 ms in inspiration; ASD about 40-60 ms with < 20 ms variation. Only the RV-pacing case (100/70 ms) and "RBBB S1 split up to 60 ms" are directly sourced.
- Felner's statement that LBBB does not split S1 conflicts with a pure "M1 follows LV activation" rule; a pragmatic treatment is to delay M1 by less than the full electrical delay in LBBB (so M1 and T1 merge into a single, softer S1) while delaying A2 by the full amount.
- Render a split as audible only when ≥ 20-30 ms; below that merge the components into one sound.

### Gaps
- No primary phonocardiographic series with mean ± SD A2-P2 intervals for RBBB, LBBB or ASD was retrieved (classical sources: Leatham 1954 Lancet 267:607; Shaver; Adolph & Fowler 1970 — listed in Felner's references but not opened).
- A "A2-P2 expiratory split of 40-50 ms corresponds to RV systolic pressure 40-50 mmHg" claim appeared only in a slide deck; not used.
- No quantitative relation between QRS duration and split width was found.

---

## 4. S3 and S4: timing, relation to P wave / PR, conditions, rhythm dependence

### Takeaway
S3 follows A2 by about 120-180 ms (sources span 100-240 ms) at the end of rapid filling and is independent of atrial activity; S4 is produced by atrial contraction, sits in presystole just before S1, requires an effective atrial systole (absent in AF) and follows each P wave even when AV conduction fails.

### Cited Findings

**S3**
- Occurs 120-160 ms after A2, lower in frequency than S2, best at the apex — [Felner, Clinical Methods ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)
- 130-180 ms after A2, coinciding with the end of rapid LV filling (extract) — [JAMA, "Current Concepts of the Genesis of Heart Sounds II"](https://jamanetwork.com/journals/jama/article-abstract/1722214); 120-200 ms after A2 (extract) — [Nemani Part 1](https://d-nb.info/1220535346/34); 100-200 ms and "about 160-240 ms after A2" appear in the same article in different places — [Nemani & Pechetty Part 2, doi:10.1055/s-0040-1722385](https://ijcdw.org/additional-heart-sounds-part-2-clicks-opening-snap-and-more/)
- Neighbouring early-diastolic sounds for comparison: opening snap 40-120 ms after S2 (30-140 ms in Clinical Methods); pericardial knock about 100-120 ms (higher-pitched and earlier than S3); tumour plop 80-150 ms — [Nemani Part 2](https://ijcdw.org/additional-heart-sounds-part-2-clicks-opening-snap-and-more/); [Crawley, ch. 27](https://www.ncbi.nlm.nih.gov/books/NBK346/); [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)
- Physiological S3 in children and adults up to 35-40 y (and pregnancy), identical in timing and frequency to the pathological one; after 40 it usually means ventricular dysfunction or volume overload: heart failure, valvular regurgitation, high-output states, left-to-right shunts, *complete AV block*, renal failure. Myocardial ischaemia without dysfunction does not cause S3 — [Silverman, Clinical Methods ch. 24](https://www.ncbi.nlm.nih.gov/books/NBK342/); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- Constrictive pericarditis: a loud early S3 ("knock") without S4 — [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)

**S4**
- Low-pitched sound coincident with late diastolic filling due to atrial contraction; requires effective atrial contraction, hence absent in atrial fibrillation — [Williams, Clinical Methods ch. 25](https://www.ncbi.nlm.nih.gov/books/NBK344/); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- S4 precedes S1 by a longer interval than the M1-T1 split (i.e. > 20-30 ms) (extract) — [Nemani Part 1](https://d-nb.info/1220535346/34); a device-patent detection window places S4 from 100-140 ms before S1 to 10-20 ms before S1 (weak source: a US patent claim seen only as a search extract; which of several returned patents it came from was not confirmed) — [one candidate, US 11653844](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/11653844)
- Conditions: reduced ventricular compliance (the chapter discusses LVH, hypertension, ischaemia; common in otherwise healthy people ≥ 50 y, significance debated; a *palpable* S4 is clearly pathological) — [Williams, ch. 25](https://www.ncbi.nlm.nih.gov/books/NBK344/); MSD: "S4 is always abnormal" (disagreement) — [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- Summation gallop: with tachycardia S3 and S4 coincide into one loud sound; slowing the rate (carotid massage) separates them — [Williams, ch. 25](https://www.ncbi.nlm.nih.gov/books/NBK344/); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- With severe pulmonic stenosis, vigorous atrial contraction can open the pulmonary valve and produce a presystolic click — [Jacobs, Clinical Methods ch. 28](https://www.ncbi.nlm.nih.gov/books/NBK347/)

### Inferences
- S3 schedule: `t_S3 = t_A2 + 140 ms` (range 120-180), gated by a "ventricular dysfunction / volume overload / young" flag; independent of rhythm, so it persists in AF and after PVCs.
- S4 schedule: tie it to *atrial* activation, not to the QRS: `t_S4 ≈ t_P_onset + (atrial electromechanical delay + active filling)`. With a normal PR of 160 ms and S1 at QRS + 40 ms, an S4 that is 60-100 ms before S1 lands roughly 100-140 ms after P onset. Consequences that follow from the atrial origin: first-degree block → S4 well separated from S1 (more audible); complete AV block → S4s march with the P waves, independent of S1/S2 (audible in long diastoles); AF/atrial standstill → no S4; atrial flutter → none (no effective contraction; inference).
- Summation: emit one louder sound when |t_S3 - t_S4| is below about 30 ms (typically HR above roughly 100-120 with PR normal).
- Drop the S4 if it would fall after S1 onset or before S3 of the previous beat's rapid filling in very short diastoles (inference).

### Gaps
- No opened source gave a P-onset-to-S4 interval or an S4-S1 interval with mean ± SD; the 100-140 ms after P onset figure above is derived, and my unverified recollection of textbook values is "S4 about 70-ish to 100+ ms before S1 and after the P-wave peak". Needs Tavel / Luisada / Shah & Jackson (AHA monograph 1975) to confirm.
- "Independent S4s in complete AV block" is supported only indirectly (S4 requires atrial contraction; complete block listed among S3 causes). A direct phonocardiographic citation was not found.
- Whether S3 timing shortens with tachycardia or high filling pressure (earlier S3) was not quantified in opened sources.

---

## 5. S1 intensity versus PR interval; beat-to-beat variation; arrhythmias, ectopy, pacing

### Takeaway
S1 loudness is governed mainly by how far apart the AV leaflets are when the ventricle contracts: maximal at PR 80-120 ms, progressively softer as PR lengthens, soft/absent beyond about 200 ms; any rhythm that varies the P-to-QRS relationship (Wenckebach, complete block, VT, junctional rhythm) or filling/contractility beat to beat (AF, alternans) makes S1 vary beat to beat.

### Cited Findings
- Maximal S1 intensity at PQ 80-120 ms; intensity falls as PQ lengthens; at PQ > 200 ms (first-degree block) the valves have begun to close with atrial relaxation, S1 is earlier and softer — [Felner, Clinical Methods ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- PR < 180 ms: mitral closure is a sharp audible sound, later and louder the shorter the PR; PR 180-190 ms: closure about 40 ms after QRS onset; very long PR: closure soon after QRS onset with *no* accompanying sound (extract) — [Leech et al., Br Heart J 1980, doi:10.1136/hrt.43.2.138](https://pmc.ncbi.nlm.nih.gov/articles/PMC482253)
- First-degree AV block is the commonest cause of a soft S1; S1 "soft or absent" — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- WPW exception: the short PQ of WPW does *not* reliably give a loud S1, because ventricular contraction starts later than the delta wave suggests — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/). (This contradicts the common teaching "short PR/WPW → loud S1"; short PR without pre-excitation, e.g. Lown-Ganong-Levine or tachycardia, does give a loud S1.)
- Beat-to-beat S1 variation: Wenckebach second-degree block (varying PQ), AV dissociation (complete heart block, junctional or ventricular tachycardia), variable rate of pressure development (atrial fibrillation, pulsus alternans), electrical alternans — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- Complete block: the first sound is loudest ("bruit de canon", cannon sound) when atrial contraction falls about 100-120 ms before ventricular contraction (extract; secondary teaching source) — [Semiologia Clinica](https://semiologiaclinica.com/index.php/en-us/articlecontainereng/369-abnormal-findings-on-examination-of-the-cardiocirculatory-system); primary study: Burggraf GW, Craige E. The first heart sound in complete heart block. Circulation 1974;50:17 — [doi link](https://www.ahajournals.org/doi/pdf/10.1161/01.CIR.50.1.17) (not opened)
- Atrial flutter with varying block: marked S1 intensity variation (extract) — [Dis Chest abstract](https://journal.chestnet.org/article/S0096-0217(16)31113-X/abstract)
- Ventricular tachycardia: changing S1 intensity, slightly irregular rate, and "a succession of (usually low-frequency) sounds" attributed to wide splitting of S1 and S2 plus gallops (extract; 1957 source) — [McKusick symposium, Circulation 1957;16:414](https://www.ahajournals.org/doi/pdf/10.1161/01.CIR.16.3.414); S1 split widens to about 60 ms in ventricular ectopics, VT, idioventricular rhythm and LV pacing — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- Other S1 modifiers: loud in mobile mitral stenosis, left atrial myxoma, hyperadrenergic states; soft in MR, VSD, acute severe AR (mitral pre-closure), myocardial depression, emphysema — [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- Post-extrasystolic beat: the pause increases filling and contractility; ejection murmurs (AS, HOCM, PS) get louder on the post-PVC beat, MR murmur does not change comparably — [Alpert, Clinical Methods ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- LV ectopic or LV-paced beats delay P2 (wide S2 split); RV pacing gives paradoxical split — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/); [Seepana & Vala 2020](https://d-nb.info/1212596692/34)
- Complete heart block increases diastolic mitral flow enough to cause a mid-diastolic flow rumble, and is listed among S3 causes — [Crawley, ch. 27](https://www.ncbi.nlm.nih.gov/books/NBK346/); [Silverman, ch. 24](https://www.ncbi.nlm.nih.gov/books/NBK342/)
- Pacemaker artefact: an extracardiac "pacemaker sound" from intercostal/diaphragm twitch can mimic a rub (no timing given) — [Nemani Part 2](https://ijcdw.org/additional-heart-sounds-part-2-clicks-opening-snap-and-more/)

### Inferences
- S1 gain as a function of the *effective* atrial-to-ventricular interval (time from the last conducted or non-conducted atrial activation to QRS onset): peak (≈ +6 dB, "cannon") at 80-120 ms; normal (0 dB) around 140-180 ms; declining to soft (≈ -10 dB or more) by 200-240 ms; near-inaudible beyond about 300 ms; and for P waves falling > ~500 ms before or within/after the QRS, revert to a medium level set by passive leaflet position. The 80-120 ms peak and the 200 ms knee are sourced; the dB values and the tail behaviour are my choices.
- The same function evaluated per beat reproduces: constant soft S1 in first-degree block; progressively softening S1 across a Wenckebach cycle; random loud/soft S1 with occasional cannon sounds in complete block and VT with AV dissociation.
- AF: no atrial term at all; make S1 gain depend on the preceding RR (short preceding cycle → valve wide open → louder S1; long cycle → softer) plus no S4. Felner attributes AF variation to "variable rates of intraventricular pressure development"; the direction with RR is my inference from the leaflet-position mechanism.
- WPW: apply the PR rule to the interval from P to the *mechanically effective* ventricular activation, not to the delta-wave onset, so S1 is not artificially loud.
- VF and asystole: no organised ventricular contraction, hence no valve closure sounds; emit silence (pulseless). Pulseless VT / very fast VT: sounds faint or absent. This is physiological inference; no auscultation citation was found.
- Early PVCs: if the coupling interval is so short that LV pressure does not exceed aortic diastolic pressure, the aortic valve does not open; render S1 only (often loud, split) with no S2 and no pulse. A workable rule is to drop A2/P2 when the preceding diastolic filling time is below about 200-250 ms or the coupling interval is below about 50-55% of the sinus RR (threshold is my estimate). The post-extrasystolic beat gets a louder S1 and louder ejection murmur.
- Paced rhythms: RV apical pacing → LBBB-like sounds (reversed S2 split, single or narrow S1); LV pacing → RBBB-like (wide S1 and S2 splits); biventricular → near-normal; VVI with AV dissociation → variable S1 and cannon sounds as in complete block; DDD/atrial-tracking with a short programmed AV delay (≈ 100-120 ms) → consistently loud S1.

### Gaps
- No citable source was found for "absent S2 on early premature beats", for auscultation in VF/asystole, or for S1 intensity vs preceding RR in AF; these are standard clinical teaching but remain unsourced here.
- Burggraf & Craige 1974 (the primary quantitative study of S1 vs PR in complete block) could not be opened; the 100-120 ms "loudest" figure comes from a secondary teaching site and agrees with Felner's 80-120 ms.
- No dB-scale relation between PR and S1 amplitude was retrieved; Leech 1980 would contain the graphs.
- Heart sounds specific to modern pacing modes (His-bundle / left-bundle-area pacing) were not researched.

---

## 6. Murmur and added-sound envelopes within the cycle

### Takeaway
Each murmur can be drawn as an envelope anchored to S1, A2/P2 and (for presystolic components) the P wave: ejection murmurs are diamonds that start after S1 and end before S2, peaking later as stenosis worsens; regurgitant systolic murmurs are plateaus from S1 to or through A2; AR/PR murmurs are decrescendos starting at A2/P2; mitral stenosis is an opening snap 30-140 ms after A2 (shorter = more severe; < 70 ms severe) followed by a low rumble with presystolic crescendo that requires atrial contraction.

### Cited Findings

**Systolic ejection (mid-systolic, crescendo-decrescendo)**
- Mid-systolic murmurs begin after a murmur-free interval following S1 and end before S2; early-systolic begin with S1; holosystolic begin with S1 and extend to or through S2; late-systolic begin in the last half of systole — [Alpert, Clinical Methods ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/)
- Innocent/flow murmurs: begin in early systole, peak in early-to-mid systole, end well before S2, medium-pitched, non-radiating; Still's murmur is short, buzzing, pure-toned at the upper/left sternal border — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/)
- Aortic stenosis: crescendo-decrescendo, low-to-medium pitch, best at primary and secondary aortic areas, radiates to carotids, usually grade 2-4; peaks in mid or late systole and "progressively later as stenosis becomes more severe"; louder after a post-extrasystolic pause; an ejection click implies a mobile valve and argues against severe stenosis; A2 diminishes with severity — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/); [AAFP 2001](https://www.aafp.org/pubs/afp/issues/2001/0601/p2201.html); the more severe the obstruction "the longer the crescendo and the shorter the diminuendo" — [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- HOCM: medium-pitched diamond at the left sternal border, radiates to the base of the neck but not the carotids, louder with Valsalva strain, standing, post-PVC — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/)
- Pulmonic stenosis: diamond at 2nd left ICS, peaks later with severity, grade ≥ 3, with a pulmonic ejection click that softens on inspiration (absent when very severe) — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/); [Jacobs, ch. 28](https://www.ncbi.nlm.nih.gov/books/NBK347/)
- ASD: nondescript pulmonic flow murmur; the diagnosis rests on the fixed split S2 — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/)

**Holosystolic / regurgitant**
- Chronic (rheumatic) MR: high-pitched, blowing, apex → axilla, holosystolic from S1 to and sometimes through A2, plateau, occasionally late-systolic accentuation; soft S1 — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/); [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- Acute MR: decrescendo, grade ≥ 3; papillary-muscle dysfunction: mid-systolic diamond — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/)
- TR: high-pitched, blowing, holosystolic plateau at the lower left sternal border, louder in inspiration (Carvallo's sign); acute TR may be decrescendo and end before S2 — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/)
- VSD (small, Roger type): low-to-medium pitch holosystolic with mid-systolic accentuation; disappears or becomes a short early-peaking murmur with Eisenmenger physiology — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/)
- Holosystolic murmurs are louder with high-velocity low-volume flow, softer with high-volume flow — [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)

**Clicks**
- Ejection click timing: aortic valvular click 120-140 ms after ECG Q-wave onset; pulmonary valvular click 90-110 ms after Q onset; both about coincident with the carotid upstroke — [Nemani & Pechetty Part 2](https://ijcdw.org/additional-heart-sounds-part-2-clicks-opening-snap-and-more/); [Jacobs, ch. 28](https://www.ncbi.nlm.nih.gov/books/NBK347/)
- Aortic click: high-pitched, 2nd right ICS or apex (valvular clicks loudest at apex), unaffected by respiration, immediately precedes the AS murmur; with a non-stenotic bicuspid valve it is loud and widely separated from S1. Pulmonary click: 2nd left ICS, softens or merges with S1 on inspiration — [Jacobs, ch. 28](https://www.ncbi.nlm.nih.gov/books/NBK347/)
- MVP: mid-to-late systolic click(s), often multiple and variable, followed by a medium-pitched late-systolic (often crescendo to A2) murmur at the apex; click moves earlier (toward S1) with reduced LV volume (standing, Valsalva strain, amyl nitrite) and later with squatting, leg raising, handgrip, post-PVC pause; murmur may become holosystolic — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/); [Jacobs, ch. 28](https://www.ncbi.nlm.nih.gov/books/NBK347/); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- Prosthetic reference timings: aortic opening click about 70 ms after S1 (ball valve), 40 ms (tilting disc), 30-80 ms (bioprosthesis); mitral opening click 70-110 ms after A2 (ball), 50-90 ms (disc), about 100 ms (bioprosthesis) — [Nemani Part 2](https://ijcdw.org/additional-heart-sounds-part-2-clicks-opening-snap-and-more/)

**Diastolic**
- AR: begins with A2, decrescendo for a variable part of diastole, heard at the left lower sternal border / 2nd right ICS with the patient leaning forward in expiration; a murmur lasting throughout diastole plus an apical Austin Flint rumble implies significant AR; in acute severe AR S1 is soft (mitral pre-closure) — [Crawley, Clinical Methods ch. 27](https://www.ncbi.nlm.nih.gov/books/NBK346/); [Felner, ch. 22](https://www.ncbi.nlm.nih.gov/books/NBK333/)
- PR with pulmonary hypertension (Graham Steell): early diastolic decrescendo from P2 at the upper left sternal border; PR without pulmonary hypertension: lower-pitched, mid-diastolic, crescendo-decrescendo — [Crawley, ch. 27](https://www.ncbi.nlm.nih.gov/books/NBK346/)
- Mitral stenosis: loud S1; opening snap (high-frequency) 30-140 ms after S2 introducing a low-frequency mid-diastolic rumble at the apex with presystolic (late diastolic) crescendo; the snap moves closer to S2 as left atrial pressure rises; calcified immobile leaflets lose both the snap and the loud S1; mild stenosis gives only a mid-diastolic or presystolic murmur — [Crawley, ch. 27](https://www.ncbi.nlm.nih.gov/books/NBK346/)
- A2-OS interval: usually 40-120 ms; < 70 ms suggests severe MS, > 100 ms mild; ≥ 80 ms ↔ mean LA pressure 9-12 mmHg, < 60 ms ↔ 25-36 mmHg, 60-80 ms overlapping (12-25 mmHg) — [Nemani & Pechetty Part 2](https://ijcdw.org/additional-heart-sounds-part-2-clicks-opening-snap-and-more/); alternative grading "mild > 110 ms, moderate 70-110 ms" (search extract; originating page not confirmed, wikidoc is the likeliest of the returned pages) — [wikidoc, MS physical examination](https://www.wikidoc.org/index.php/Mitral_stenosis_physical_examination)
- Duration, not loudness, of the MS rumble tracks severity — [AAFP 2001](https://www.aafp.org/pubs/afp/issues/2001/0601/p2201.html); A2-OS widens on standing whereas A2-P2 does not — [Felner, ch. 23](https://www.ncbi.nlm.nih.gov/books/NBK341/)
- Presystolic component: "late diastolic (presystolic) murmur may occur *in sinus rhythm* with rheumatic mitral stenosis" — [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation); tricuspid stenosis presystolic component is crescendo-decrescendo, louder on inspiration — [Crawley, ch. 27](https://www.ncbi.nlm.nih.gov/books/NBK346/)
- Flow rumbles without stenosis: MR, VSD, PDA, complete heart block (mitral); ASD, TR (tricuspid) — [Crawley, ch. 27](https://www.ncbi.nlm.nih.gov/books/NBK346/)

**Continuous and extracardiac**
- PDA: continuous murmur when pulmonary resistance is normal, loudest at the 2nd left ICS below the medial clavicle; the diastolic part fades and disappears as pulmonary resistance rises — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- Venous hum: continuous, right supraclavicular fossa — [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- Pericardial friction rub: high-pitched, scratchy; up to three components (atrial systole, ventricular systole, rapid early-diastolic filling); all three heard in < 50% of cases; best leaning forward in held expiration; nearly continuous with tachycardia — [Nemani Part 2](https://ijcdw.org/additional-heart-sounds-part-2-clicks-opening-snap-and-more/); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)
- Levine grading 1-6 (thrill from grade 4) for systolic, 1-4 for diastolic murmurs — [Alpert, ch. 26](https://www.ncbi.nlm.nih.gov/books/NBK345/); [MSD Manual](https://www.msdmanuals.com/professional/cardiovascular-disorders/approach-to-the-cardiac-patient/cardiac-auscultation)

### Inferences
- Envelope parameterisation relative to per-beat anchors (S1 = M1 onset, A2, P2, next P, next S1), with `sys = t_A2 - t_S1`:
  - AS/PS diamond: start = S1 + (PEP - QS1) ≈ S1 + 40-70 ms (or at the ejection click, Q + 120-140 ms aortic / Q + 90-110 ms pulmonic), end ≈ A2 - 20-40 ms; peak at about 30-40% of the murmur for mild, 50% moderate, 60-75% severe; severe AS also softens/removes A2 and can reverse the split. The peak fractions are my quantification of "progressively later".
  - MR/TR/VSD: plateau from S1 onset to A2 (MR may run through A2 to P2), with S1 attenuated/masked.
  - MVP: click at about 40-60% of systole, then crescendo to A2; "standing" moves the click to about 25-35% and lengthens the murmur.
  - AR: starts at A2, exponential decay; time constant longer (to full diastole) with chronic severe AR, short in mild AR; acute severe AR short and soft with soft S1.
  - MS: OS at A2 + 100-120 ms (mild), 70-100 (moderate), 40-70 (severe); rumble from OS decaying through mid-diastole (longer with severity; merges into presystole at higher HR), then a crescendo from roughly P onset + ~80-100 ms up to the loud S1.
  - PDA: continuous, peaking around S2, waning in late diastole.
  - Rub: three short scratchy bursts at atrial systole (after P), mid-systole, and early diastole (around S3 time).
- Rhythm coupling: the presystolic crescendo of MS/TS and the atrial rub component must be driven by atrial activation, so they vanish in AF (MS in AF: OS + mid-diastolic rumble only, audible mainly in long cycles, S1 varying) and dissociate from S1 in complete block. Felner/MSD support the "requires sinus rhythm / atrial contraction" premise; "loss in AF" itself is an inference here (see Gaps).
- Ejection murmur intensity should scale with the preceding RR (louder after long pauses, post-PVC), regurgitant MR murmur should not; this is a classic differentiator and worth simulating in AF and with ectopy.

### Gaps
- No opened source gave ms values for the onset delay and end of ejection murmurs relative to S1/A2, the fraction of systole at which the AS murmur peaks per severity grade, or the mid-systolic click position in MVP; these are parameter choices.
- "Presystolic accentuation is lost in AF" was not found stated verbatim in an opened source (MSD only says the presystolic murmur occurs in sinus rhythm). Note that the older literature (Criley & Hermer, NEJM 1971) reports a presystolic crescendo can persist in AF during short cycles; not retrieved, so unverified.
- Timing (ms) of the three rub components and of the Austin Flint rumble was not found.
- Paediatric differences beyond those noted (physiological S3, louder P2, wider inspiratory split, innocent Still's murmur, constant Q-S1) were not researched.
- Murmur frequency bands per lesion (beyond "murmurs 200-600 Hz" and qualitative low/medium/high pitch) were not quantified from primary sources.
