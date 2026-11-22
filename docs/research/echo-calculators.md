# Echo calculators: sources read (brief B29, parts A and B)

Every calculator of `heartsim-calc` and the publication actually opened for it during this work, with the place read, the
review status shown on the page, and what could not be confirmed. *Verified* means the formula and every threshold the page
shows were read in the primary source at that place; *Formula only* means the arithmetic is standard and was checked against a
primary source or first principles, and no grading is shown.

The scope (which calculators, which inputs, which publications to open) came from a survey of a public calculator catalogue
(EchoSuite, echosuite.app, proprietary). Only a stripped list of calculator ids, input names and ranges and the citations was
used; no text, help, worked example, code or layout was taken, and every formula and threshold was re-read in the publications
below. Calculator ids, names, descriptions and examples are our own.

## Publications opened

| Key | Publication | Obtained from |
| --- | --- | --- |
| AS 2017 | Baumgartner H et al. EACVI/ASE focused update on aortic stenosis. J Am Soc Echocardiogr 2017;30:372–392. doi:10.1016/j.echo.2017.02.009 | asecho.org PDF |
| VS 2009 | Baumgartner H et al. EAE/ASE recommendations on valve stenosis. J Am Soc Echocardiogr 2009;22:1–23. doi:10.1016/j.echo.2008.11.029 | asecho.org PDF |
| RHD 2023 | Pandian NG et al. Echocardiography in rheumatic heart disease. J Am Soc Echocardiogr 2023;36:3–28. doi:10.1016/j.echo.2022.10.009 | asecho.org PDF |
| VR 2017 | Zoghbi WA et al. Native valvular regurgitation. J Am Soc Echocardiogr 2017;30:303–371. doi:10.1016/j.echo.2017.01.007 | asecho.org PDF |
| DD 2025 | Nagueh SF et al. LV diastolic function and HFpEF. J Am Soc Echocardiogr 2025;38:537–569. doi:10.1016/j.echo.2025.03.011 | asecho.org PDF |
| CQ 2015 | Lang RM et al. Chamber quantification. J Am Soc Echocardiogr 2015;28:1–39.e14. doi:10.1016/j.echo.2014.10.003 | asecho.org PDF |
| Strain 2025 | Thomas JD et al. Clinical applications of strain echocardiography. J Am Soc Echocardiogr 2025;38:985–1020. doi:10.1016/j.echo.2025.07.007 | asecho.org PDF |
| TTE 2019 | Mitchell C et al. Comprehensive TTE examination. J Am Soc Echocardiogr 2019;32:1–64. doi:10.1016/j.echo.2018.06.004 | asecho.org PDF |
| RH 2025 | Mukherjee M et al. Right heart and pulmonary hypertension. J Am Soc Echocardiogr 2025;38:141–186. doi:10.1016/j.echo.2025.01.006 | asecho.org PDF |
| Peri 2013 | Klein AL et al. Pericardial disease. J Am Soc Echocardiogr 2013;26:965–1012. doi:10.1016/j.echo.2013.06.023 | asecho.org PDF |
| ASD 2015 | Silvestry FE et al. ASD and PFO. J Am Soc Echocardiogr 2015;28:910–958. doi:10.1016/j.echo.2015.05.015 | asecho.org PDF |
| PV 2024 | Zoghbi WA et al. Prosthetic valve function. J Am Soc Echocardiogr 2024;37:2–63. doi:10.1016/j.echo.2023.10.004 | asecho.org PDF |
| Gorcsan 1991 | Am Heart J 1991;121:858–863 (abstract only) | PubMed 2000753 |
| Nishimura 1988 | J Am Coll Cardiol 1988;11:317–321 (abstract only) | PubMed 3339169 |
| Haycock 1978 | J Pediatr 1978;93:62–66 (abstract only) | PubMed 650346 |
| Du Bois 1916 | Proc Soc Exp Biol Med 1916;13:77–78 (the short note) | Zenodo record 2326550 |
| OpenStax | College Physics 2e §17.7; University Physics 1 §14.7 | openstax.org |

Not obtained: Mosteller 1987 (NEJM letter, publisher refused access); Du Bois & Du Bois 1916, Arch Intern Med 17:863 (the
constant 71.84); 2025 ESC myocarditis and pericarditis guideline (publisher refused automated access); the EACVI 2021 shunt
lesion paper, the ASE 2024 paediatric and neonatal papers and the 2025 ACC/AHA adult congenital guideline (not attempted:
their thresholds are not used).

## Calculators

Counts after part B: 74 calculators, 49 verified, 25 formula only (part A: 58, 30 and 28; none left out of the part A list).
The part B tools and the part A rows that part B changed are in the section *Part B* below.

| Id | Name | Status | Source read and locator | Notes, and what is not confirmed |
| --- | --- | --- | --- | --- |
| `aortic-valve-area` | Aortic valve area (continuity) | Verified | Baumgartner H J Am Soc Echocardiogr 2017;30:372–392 — Continuity equation p. 379; velocity ratio p. 382; Table 3 p. 383 | Table 3 bands copied as printed; values in the printed gaps (2.5–2.6 m/s) go to the higher band; peak velocity 4.0 and mean gradient 40 are severe (text: "≥4.0 m/s", "≥40 mmHg"). Table 3 puts VR < 0.25 in severe while the text says "0.25 or less": the table is followed (0.25 is moderate). |
| `mitral-area-pht` | Mitral valve area (pressure half-time) | Verified | Pandian NG J Am Soc Echocardiogr 2023;36:3–28 — PHT equation and Table 1, p. 8 | Table 1 (progressive mild/moderate, severe) for area, PHT and the mean gradient (footnote: at 60–80 bpm). |
| `mitral-area-continuity` | Mitral valve area (continuity) | Verified | Pandian NG J Am Soc Echocardiogr 2023;36:3–28 — Continuity equation and Table 1, p. 8 | Same Table 1 area bands. Exclusions (significant AR or MR) in the summary. |
| `mitral-area-pisa` | Mitral valve area (PISA) | Formula only | Baumgartner H J Am Soc Echocardiogr 2009;22:1–23 — §B.1.5 PISA method, p. 14 | The 2009 text prints "MVA = π(r²)(Valiasing)/Peak Vmitral · α/180°"; the calculator uses the hemisphere 2πr² (first principles, and the 2πr² of the 2017 regurgitation PISA equations). The 2023 RHD guideline describes the method and the angle correction (p. 9) without printing the formula. Because the printed formula differs, no grading is shown. |
| `pulmonic-stenosis-gradient` | Pulmonic stenosis peak gradient | Verified | Baumgartner H J Am Soc Echocardiogr 2009;22:1–23 — §V.B.1.1 and Table 11, pp. 19–20 | Table 11 is an image in the PDF; the bands were read in the text that quotes it (p. 20: severe > 4 m/s, moderate 3–4, mild < 3). |
| `tricuspid-stenosis-criteria` | Tricuspid stenosis criteria | Verified | Pandian NG J Am Soc Echocardiogr 2023;36:3–28 — Rheumatic tricuspid stenosis, p. 16 | Criteria for significant TS (mean ≥ 5 mmHg, VTI ≥ 60 cm, PHT ≥ 190 ms, TVA ≤ 1 cm²) and the area grades (mild > 1.5, moderate 1–1.5). The area from 190 ÷ PHT is shown, flagged as less reliable as the text says. |
| `mitral-regurgitation-pisa` | Mitral regurgitation (PISA) | Verified | Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — PISA equations, p. 312; Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — Table 8, p. 332 | Hand test against the worked example in the Figure 11 caption (p. 321). Table 8 four grades (mild, mild–moderate, moderate–severe, severe). |
| `aortic-regurgitation-pisa` | Aortic regurgitation (PISA) | Verified | Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — PISA equations, p. 312; Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — Table 11, p. 340 | Table 11 four grades. |
| `tricuspid-regurgitation-pisa` | Tricuspid regurgitation (PISA) | Verified | Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — PISA equations, p. 312; Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — Table 14, p. 350 | Table 14 three grades for EROA, RVol (2D PISA) and VC width. |
| `mitral-regurgitation-volumetric` | Mitral regurgitation (volumetric) | Verified | Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — Volumetric method and Figure 4, p. 311; Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — Table 8, p. 332 | Hand test against Figure 4 (p. 311): SV 183 and 58 mL, RVol 125 mL, RF 68 %. |
| `aortic-regurgitation-volumetric` | Aortic regurgitation (volumetric) | Verified | Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — Volumetric method and Figure 4, p. 311; Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — Table 11, p. 340 |  |
| `pulmonic-regurgitation-volumetric` | Pulmonic regurgitation (volumetric) | Verified | Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — Volumetric method and Figure 4, p. 311; Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — Table 16, p. 358 | Table 16 RF bands (< 20, 20–40, > 40 %); the table notes RF cut-offs come mainly from CMR. PR PHT < 100 ms shown as an Info note (the table says steep deceleration is not specific). |
| `mitral-e-to-e-prime` | E/e' ratio | Verified | Nagueh SF J Am Soc Echocardiogr 2025;38:537–569 — Table 3 (average E/e'), p. 541; Table 4, p. 548; Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — Table 4 (PCWP = 1.24 × E/e' + 1.9), p. 163 | E/e' < 8 usually normal, > 14 high specificity, 8–14 indeterminate (Table 4 row, p. 548). The PCWP regression is given in the right-heart guideline with "mitral E/e'" (no site); the average is used. |
| `lap-from-mr-jet` | Left atrial pressure from the MR jet | Formula only | Gorcsan J 3rd Am Heart J 1991;121:858–863 — Abstract: LAP = SBP − MR gradient, Am Heart J 121:858 | Formula from the abstract (full text not read); no threshold. |
| `lv-pressure-from-jets` | LV pressures from regurgitant jets | Formula only | Nishimura RA J Am Coll Cardiol 1988;11:317–321 — Abstract: LV–LA and aorta–LV gradients, J Am Coll Cardiol 11:317 | Abstract read (gradients LV–LA from MR, aorta–LV end-diastolic from AR); DBP stands in for aortic end-diastolic pressure (first principles). No threshold. |
| `la-volume-index` | Left atrial volume index | Verified | Lang RM J Am Soc Echocardiogr 2015;28:1–39.e14 — LA volume §9.3, p. 25; Table 4, p. 10 | Table 4 partitions (same for men and women): 16–34, 35–41, 42–48, > 48 mL/m². |
| `lv-fractional-shortening` | Fractional shortening | Formula only | Lang RM J Am Soc Echocardiogr 2015;28:1–39.e14 — §2.1 Fractional shortening, p. 6 | Definition. Part B: no fractional-shortening range in Table 2 or Supplemental Table 3, so it stays formula only. |
| `lv-diameter-index` | Indexed LV diameters | Verified (part B) | Lang RM J Am Soc Echocardiogr 2015;28:1–39.e14 — §1.1 linear measurements; Supplemental Table 3, p. 39.e8 | Part B: graded by sex (optional input) with the Supplemental Table 3 rows LV diastolic and systolic diameter, absolute and per BSA. Without the sex the page asks for it. |
| `lv-mass-linear` | LV mass (linear method) | Verified | Lang RM J Am Soc Echocardiogr 2015;28:1–39.e14 — Table 5 (cube formula), p. 14; RWT, p. 16; Supplemental Table 3, p. 39.e8; Table 6 and Figure 6, p. 17 | RWT cut-off (> 0.42 concentric, ≤ 0.42 eccentric, p. 16). Part B: with the sex, the linear-method rows of Supplemental Table 3 (mass, mass/BSA, septal and posterior wall) and the geometry of Figure 6 (mass index above 115 g/m² in men, 95 in women, p. 16, against RWT 0.42). |
| `lv-volumes-indexed` | Indexed LV volumes | Verified (part B) | Lang RM J Am Soc Echocardiogr 2015;28:1–39.e14 — §1.2 volumes; Supplemental Table 3, p. 39.e8 | Part B: the 2D (biplane) rows of Supplemental Table 3 by sex, absolute and per BSA. 3D volumes (Table 3, p. 10: per-study ranges) not applied. |
| `ejection-fraction` | Ejection fraction | Verified (part B) | Lang RM J Am Soc Echocardiogr 2015;28:1–39.e14 — §2.2 ejection fraction, pp. 6–7; Table 4, p. 10 | Part B: Table 4 by sex (men 52–72 normal, 41–51 mild, 30–40 moderate, < 30 severe; women 54–74, 41–53, 30–40, < 30). Above the normal range is reported as such, not graded. |
| `gls-relative-change` | Relative change of GLS | Verified | Thomas JD J Am Soc Echocardiogr 2025;38:985–1020 — Key points p. 995; cardio-oncology p. 999 | ≥ 15 % relative fall: preclinical dysfunction during chemotherapy (p. 999; ESC/IC-OS mild CTRCD); 10–15 % likely significant (key points, p. 995). |
| `lvot-stroke-volume` | Stroke volume and cardiac output (LVOT) | Verified | Zoghbi WA J Am Soc Echocardiogr 2017;30:303–371 — Stroke volume at a site, p. 311; Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — Table 4 (CO = πr² × VTI × HR), p. 162; Baumgartner H J Am Soc Echocardiogr 2017;30:372–392 — Low flow, SVi < 35 mL/m², p. 383 | SVi < 35 mL/m² shown as the AS update's example of low flow (p. 383). Heart rate may come from the running rhythm (engine events). |
| `aortic-diameter-index` | Indexed aortic diameter | Formula only | Lang RM J Am Soc Echocardiogr 2015;28:1–39.e14 — §10 aorta and Table 14, p. 32 | Part B: Table 14 gives mean ± SD only; dilatation is defined against the nomograms of Figure 13 (95 % CI not printed as numbers), so it stays formula only. |
| `wall-motion-score-index` | Wall-motion score index | Formula only | Lang RM J Am Soc Echocardiogr 2015;28:1–39.e14 — Regional function and scoring, p. 11; Figures 3–4 | Scoring 1–4 and the mean of segments seen (p. 11); no threshold shown. |
| `rap-from-ivc` | Right atrial pressure from the IVC | Verified | Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — RAP section p. 160; Table 4, p. 161 | Table 4 categories (3, 8, 15, 20 mmHg) and the text's down/upgrading of the indeterminate 8 with secondary signs (p. 160). Cut-offs inclusive as printed (≤ 2.1 cm, ≥ 50 %). |
| `rvsp-pasp` | RV systolic and pulmonary systolic pressure | Verified | Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — RVSP p. 160; Table 4, p. 161; Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — Table 1, p. 145 | Table 1 rows TRV and RVSP; values in the printed gaps (2.8–3.1 / 3.2–3.5; ≤ 34 / ≥ 35) go to the higher band. PASP = RVSP − PS gradient also in VS 2009 p. 20. |
| `pa-pressures-from-pr` | Pulmonary diastolic and mean pressure (PR jet) | Verified | Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — Table 4, p. 162; PAEDP and mPAP, p. 166 | End-diastolic PR velocity ≥ 2.2 m/s "generally considered abnormal" (p. 166). |
| `mpap-from-acceleration-time` | Mean PA pressure from the RVOT acceleration time | Verified | Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — mPAP formulas, p. 166; AccT, p. 158; Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — Table 1, p. 145 | Table 1 AccT row (> 105 normal …). mPAP itself is not graded (the > 20 mmHg definition is invasive). |
| `pvr-doppler` | Pulmonary vascular resistance (Doppler) | Verified | Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — PVR, p. 167; Table 4, p. 162 | < 1.5 WU normal, > 2.0 WU abnormal, unreliable > 8 WU (p. 167). |
| `rv-systolic-function` | RV systolic function | Verified | Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — FAC formula p. 157; Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — Table 1, p. 145 | Table 1 rows TAPSE (cm, entered in mm), S', FAC, 3D RVEF, free-wall strain. |
| `tapse-pasp-coupling` | TAPSE/PASP coupling | Verified | Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — RV–PA coupling and key points, p. 152 | Text values: 0.5–0.7 in healthy people, 0.3–0.4 associated with uncoupling in PH; shown as ≤ 0.4 (uncoupling range), 0.5–0.7 (healthy range), otherwise a note. |
| `rv-mpi` | RV myocardial performance index | Verified | Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — RV MPI formulas, p. 158; Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — Table 1, p. 145 | Table 1 rows for the pulsed and tissue Doppler methods. |
| `right-atrial-size` | Right atrial size | Verified | Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — Table 1, p. 145 | Table 1 RA rows (sex-independent in this guideline). |
| `right-ventricular-size` | Right ventricular size | Verified | Mukherjee M J Am Soc Echocardiogr 2025;38:141–186 — Table 1, p. 145 | Table 1 RV rows (sex-independent in this guideline). |
| `body-surface-area` | Body surface area | Formula only | Mosteller RD. N Engl J Med 1987;317:1098 — letter, p. 1098; Du Bois D Proc Soc Exp Biol Med 1916;13:77–78 — exponents 1/2.35 and 1/1.38, p. 77; Haycock GB J Pediatr 1978;93:62–66 — abstract, p. 62 | NOT fully confirmed on primaries: the Mosteller letter (NEJM, paywalled) could not be read; the Du Bois 1916 note gives the exponents (1/2.35, 1/1.38) but defers the constant (71.84) to the Archives paper, which could not be obtained; the Haycock formula was read in the paper's abstract. Shipped as formula only because indexing in the guidelines (the 2015 chamber reference values were indexed with Mosteller, CQ 2015 Appendix p. 39.e1) needs it; the three formulas agree within 2 % for an adult (tested). |
| `pericardial-effusion-size` | Pericardial effusion size | Verified | Klein AL J Am Soc Echocardiogr 2013;26:965–1012 — 2D echocardiography of effusions, p. 981 | ESC 2025 (cited by the survey) could not be fetched (the publisher refuses automated access); the ASE 2013 grades were used: small < 10, moderate 10–20, large > 20, very large > 25 mm. |
| `mitral-respiratory-variation` | Mitral inflow respiratory variation | Verified | Klein AL J Am Soc Echocardiogr 2013;26:965–1012 — Tamponade p. 991; constriction p. 994; Figures A5–A6, p. 1012.e4 | (expiration − inspiration) ÷ expiration; > 25 % constriction (p. 994), > 30 % tamponade (p. 991, "usually exceeds"). Worked example Figure A5. |
| `tricuspid-respiratory-variation` | Tricuspid inflow respiratory variation | Verified | Klein AL J Am Soc Echocardiogr 2013;26:965–1012 — Tamponade p. 991; constriction p. 994; Figures A5–A6, p. 1012.e4 | Same convention (negative when inspiration is higher); > 40 % constriction, > 60 % tamponade. Figure A6: (34 − 64) ÷ 34 = −88 % (the caption prints 87 %). |
| `lvot-respiratory-variation` | LVOT respiratory variation | Formula only | Klein AL J Am Soc Echocardiogr 2013;26:965–1012 — Figure 38 and Doppler in tamponade, pp. 988–990 | Described in tamponade (Figure 38) with no cut-off. |
| `hepatic-vein-reversal-ratio` | Hepatic vein expiratory reversal ratio | Verified | Nagueh SF J Am Soc Echocardiogr 2025;38:537–569 — Pericardial constriction and key points, p. 561 | ≥ 0.8 among the constriction findings (p. 561). |
| `septal-lateral-e-prime-ratio` | Septal-to-lateral e' ratio | Formula only | Nagueh SF J Am Soc Echocardiogr 2025;38:537–569 — Pericardial constriction, p. 561 | "Annulus reversus" is described without a numeric cut-off, so none is shown. |
| `pulmonary-systemic-flow-ratio` | Pulmonary-to-systemic flow ratio (Qp/Qs) | Formula only | Silvestry FE J Am Soc Echocardiogr 2015;28:910–958 — Pulsed Doppler Qp/Qs, p. 929 | Table 8 (p. 946) uses Qp/Qs ≥ 1.5 as a closure indication: a treatment criterion, not shown. |
| `shunt-flow` | Left-to-right shunt flow | Formula only | Silvestry FE J Am Soc Echocardiogr 2015;28:910–958 — Pulsed Doppler Qp/Qs, p. 929 | Qp − Qs and its fraction (first principles from the Qp/Qs method). |
| `vsd-rv-pressure` | RV systolic pressure from a VSD jet | Formula only | Baumgartner H J Am Soc Echocardiogr 2017;30:372–392 — Simplified Bernoulli, p. 378; Gorcsan J 3rd Am Heart J 1991;121:858–863 — SBP as LV systolic pressure (abstract) | Bernoulli; SBP as LV systolic pressure without LVOT obstruction. The EACVI 2021 shunt paper cited by the survey was not read. |
| `pda-pa-pressure` | PA systolic pressure from a PDA jet | Formula only | Baumgartner H J Am Soc Echocardiogr 2017;30:372–392 — Simplified Bernoulli, p. 378 | Bernoulli; the paediatric ASE papers cited by the survey were not read. |
| `coarctation-gradient` | Coarctation peak gradient | Formula only | Baumgartner H J Am Soc Echocardiogr 2017;30:372–392 — Bernoulli with proximal velocity, p. 378 | Bernoulli with the proximal velocity (AS 2017 p. 378). The 2025 ACC/AHA ACHD guideline thresholds were not read. |
| `mean-gradient-from-trace` | Mean gradient from a traced velocity | Formula only | Baumgartner H J Am Soc Echocardiogr 2017;30:372–392 — Mean gradient from instantaneous gradients, p. 378 | "The mean gradient requires averaging of instantaneous gradients and cannot be calculated from the mean velocity" (p. 378). |
| `aortic-prosthesis-eoa` | Aortic prosthesis EOA and DVI | Formula only | Zoghbi WA J Am Soc Echocardiogr 2024;37:2–63 — EOA p. 7; DVI p. 9 | Formula only by decision; the screening and mismatch criteria are in `aortic-prosthesis-screening` (part B). |
| `mitral-prosthesis-eoa` | Mitral prosthesis EOA and DVI | Formula only | Zoghbi WA J Am Soc Echocardiogr 2024;37:2–63 — DVI p. 9; mitral EOA and DVI p. 24 | Formula only by decision; the screening criteria are in `mitral-prosthesis-screening` (part B). |
| `wave-period-wavelength` | Period, wavelength and pulse duration | Formula only | Ling SJ OpenStax, Rice University — §16.1, v = fλ | Definitions (T = 1/f, λ = c/f); §16.1 not opened, the relation is definitional. |
| `prf-nyquist` | Pulse repetition, Nyquist limit and duty factor | Formula only | Urone PP OpenStax, Rice University — §17.7 echo timing, Example 17.8; Mitchell C J Am Soc Echocardiogr 2019;32:1–64 — Spectral Doppler, p. 42 | Round-trip time 2d/c, PRF ≤ c/2d, Nyquist = PRF/2 from first principles (sampling); 1540 m/s from OpenStax Example 17.8. |
| `spatial-pulse-length` | Spatial pulse length, axial resolution and near field | Formula only | Ling SJ OpenStax, Rice University — §16.1, v = fλ; Urone PP OpenStax, Rice University — §17.7 resolution and wavelength | SPL = nλ, axial resolution ≈ SPL/2 and the near field D²/4λ of a disc (Fresnel distance a²/λ, a = D/2) from first principles; no source opened for the near-field expression. |
| `doppler-shift-velocity` | Doppler equation | Formula only | Mitchell C J Am Soc Echocardiogr 2019;32:1–64 — Doppler angle, p. 42 | Doppler equation from first principles; the guideline's "20° → 6 %" is a test (1 − cos 20° = 6.0 %). |
| `acoustic-impedance` | Acoustic impedance | Formula only | Urone PP OpenStax, Rice University — §17.7 Eq. 17.38 and Table 17.5 | Eq. 17.38 and Table 17.5 read; blood 1060 × 1570 = 1.66 MRayl tested. |
| `reflection-coefficient` | Intensity reflection coefficient | Formula only | Urone PP OpenStax, Rice University — §17.7 Eq. 17.39 | Eq. 17.39 read. |
| `acoustic-intensity` | Acoustic intensity | Formula only | Urone PP OpenStax, Rice University — §17.3 intensity (I = P ÷ A) | I = P/A, definitional (§17.3 not opened). |
| `poiseuille-flow` | Poiseuille flow and resistance | Formula only | Ling SJ OpenStax, Rice University — §14.7 Eqs. 14.18–14.19 | Eqs. 14.18–14.19 read. |

## Boundary conventions

Each graded table is copied row by row into a `Scale` (`crates/heartsim-calc/src/grade.rs`): every limit is inclusive or not
as printed. Where a table leaves a gap between two printed limits (for example "2.6–2.9" and "3.0–4.0" m/s, or "≤ 34" and
"≥ 35" mmHg), a value inside the gap goes to the higher band. A computed value within 10⁻⁹ (relative) of a limit counts as
equal to it. Each boundary is tested at the limit and one step either side.

## For the maintainer

* A clinician's review of the gradings is still needed before the page is described as more than a study aid.
* RA and RV size and RV function, whose Table 1 in RH 2025 is the same for men and women, were graded in part A and are not
  repeated in part B; nor is tricuspid stenosis.

## Part B — guideline decision tools

Part B re-opened the four guidelines below from the PDFs fetched from asecho.org in part A (the same files, not a secondary
copy) and read every figure and table used here on the page; the algorithm figures were rendered as images and read, because
their arrows are not in the text layer.

| Key | What was read for part B |
| --- | --- |
| DD 2025 | Figure 2 (p. 553), Table 6 (p. 553), Figure 3 and its legend (p. 554), text pp. 554–555 (additional and supplemental parameters), section 13 diastolic exercise echocardiography and its key points (pp. 555–556), Table 7 (p. 557), valvular disease and Figure 4 (pp. 557–558), heart transplant and Figure 5 (p. 559), pulmonary hypertension and Figure 6 (p. 560), restrictive cardiomyopathy (p. 561), HCM (p. 562), atrial fibrillation and Figure 8 (p. 563) |
| PV 2024 | Table 5 and Figure 13 (p. 19), Figure 11 caption and image (p. 17), Tables 6 and 7 (p. 20), mitral prosthesis text (pp. 24–25), Tables 10 and 11 (p. 27), Table 12 (p. 28), mismatch text (p. 10) |
| AS 2017 | Table 3 (p. 383), low-flow low-gradient sections and Table 4 (p. 384), Table 5 and the dobutamine findings (p. 385), new classification (p. 386), integrated approach (pp. 386–388), Figure 8 (p. 388) |
| CQ 2015 | Table 2 (p. 7), Table 4 (p. 10), LV mass text (p. 16), Table 6 and Figure 6 (p. 17), Table 14 and Figure 13 (p. 32), Supplemental Table 3 (p. 39.e8) |

### How a decision tool works

A tool is a tree of `crates/heartsim-calc/src/tree.rs`: static nodes (each a question with its figure or table and page) and
leaves (each a result with its rule and page), and a pure walk from the answers to a leaf. The answers are computed from the
measurements; every field may be left empty. The tool walks the tree for every value the empty fields could take: if all walks
end at the same leaf, that is the result (and the page says the empty fields could not change it); otherwise the result is
**indeterminate** and lists the results still possible. The source's own "indeterminate" ends (Figures 5, 6 and 8, the exercise
criteria) are leaves of their own. Where one of several alternative measurements suffices ("PV S/D or LARS or LAVi, or
alternatively IVRT" in Figure 3; e' at either site), the measured ones decide and an unmeasured one does not make the result
open. A test enumerates every combination of met, not met and not measured for every tree (`tests/trees.rs`) and checks that
each reaches a leaf and that every leaf is reachable; a property test checks the same through the inputs.

### Tools

| Id | Name | Status | Source and locator | Notes and choices |
| --- | --- | --- | --- | --- |
| `diastolic-sinus-rhythm` | Diastolic function and LAP (sinus rhythm) | Verified | DD 2025 — Figure 3 and text, pp. 554–555; Table 6, p. 553 | Figure 3 as drawn: e' septal ≤ 6, lateral ≤ 7, average ≤ 6.5 cm/s (or Table 6 age-specific `<` limits, optional); E/e' septal ≥ 15, lateral ≥ 13, average ≥ 14; PASP ≥ 35 mmHg when given (RAP estimated), else TR ≥ 2.8 m/s; additional variables PV S/D ≤ 0.67, LARS ≤ 18 %, LAVi > 34 mL/m², IVRT ≤ 70 ms; supplemental methods of the legend (PR end-diastolic ≥ 2 m/s, PA diastolic ≥ 16 mmHg, L wave ≥ 50 cm/s, Ar − A > 30 ms, E/A fall ≥ 50 % with Valsalva); grade by E/A ≤ 0.8, < 2, ≥ 2. |
| `diastolic-atrial-fibrillation` | Filling pressure in atrial fibrillation | Verified | DD 2025 — Figure 8, p. 563 | E ≥ 100 cm/s, septal E/e' > 11, TR > 2.8 m/s or PASP > 35 mmHg, DT ≤ 160 ms; then LARS < 18 %, PV S/D < 1, BMI > 30 kg/m². |
| `diastolic-mac` | Filling pressure with mitral annular calcification | Verified | DD 2025 — Figure 4 and text, p. 558; Table 7 row 8, p. 557 | E/A < 0.8 normal, > 1.8 elevated, 0.8–1.8 (both included, as Figure 4) by IVRT ≥ 80 / < 80 ms. |
| `diastolic-pulmonary-hypertension` | Filling pressure in pulmonary hypertension | Verified | DD 2025 — Figure 6 and text, p. 560 | Figure 6: E/A ≤ 0.8 with E ≤ 50 cm/s normal; E/A ≥ 2 with reduced e' elevated; otherwise LARS > 18 % / ≤ 18 %, or if LARS is not available lateral E/e' < 8 / > 13, 8–13 indeterminate (8 and 13 included). "e' reduced" is a yes/no input. E/A ≥ 2 without reduced e' has no branch in the figure: reported as such. |
| `diastolic-heart-transplant` | Filling pressure after heart transplant | Verified | DD 2025 — Figure 5 and text, p. 559 | Average E/e' < 7, 7–14, > 14; E/SRIVR ≤ 200 / > 200 cm; TR ≤ 2.8 / > 2.8 m/s; an empty SRIVR is "cannot be measured" and an empty TR "absent or incomplete", the figure's own branches. |
| `diastolic-exercise` | Diastolic exercise echocardiography | Verified | DD 2025 — section 13C and key points, pp. 555–556 | Positive: average E/e' ≥ 14 (or septal ≥ 15) and TR ≥ 3.2 m/s; normal: E/e' < 10 and TR < 2.8; "HFpEF likely": E/e' ≥ 14 with TR 2.8 to < 3.2; E/e' < 14 with TR ≥ 3.2 does not support HFpEF; other combinations indeterminate. |
| `diastolic-mitral-stenosis` | Filling pressure with mitral stenosis | Verified | DD 2025 — Table 7 row 6, p. 557; text pp. 557–558 | Indicators IVRT < 60 ms, A > 1.5 m/s, IVRT/TE-e' < 4.2. |
| `diastolic-mitral-regurgitation` | Filling pressure with mitral regurgitation | Verified | DD 2025 — Table 7 row 7, p. 557; text and key points, p. 558 | IVRT < 60 ms, Ar − A ≥ 30 ms, IVRT/TE-e' < 5.6, average E/e' > 14 only with reduced EF. |
| `diastolic-hcm` | Filling pressure in HCM | Verified | DD 2025 — Table 7 row 3, p. 557; p. 562 | Average E/e' > 14, Ar − A ≥ 30 ms, TR > 2.8 m/s, LAVi > 34 mL/m². |
| `diastolic-restrictive` | Filling pressure in restrictive cardiomyopathy | Verified | DD 2025 — Table 7 row 4, p. 557; p. 561 | Average E/e' > 14, DT < 140 ms, E/A > 2.5, IVRT < 50 ms. |
| `diastolic-sinus-tachycardia` | Filling pressure in sinus tachycardia | Verified | DD 2025 — Table 7 row 2, p. 557 | Predominant early filling with reduced EF (yes/no), IVRT ≤ 70 ms, PV systolic filling fraction ≤ 40 %, average E/e' > 14. Row 2.5 (the beat after a premature beat) is in the summary. |
| `diastolic-lvad` | Filling pressure with an LVAD | Verified | DD 2025 — Table 7 row 9 and footnote, p. 557 | E/A > 2, RAP > 10, PASP > 40 mmHg, average E/e' > 14 or septal ≥ 15, LAVi > 33 mL/m²; LAP = RAP, RAP + 5 or RAP − 5 by the septal position (footnote). |
| `aortic-stenosis-pathway` | Aortic stenosis: stepwise grading and low-flow pathway | Verified | AS 2017 — Figure 8, p. 388; pp. 384–388; Tables 4 and 5, pp. 384–385 | High gradient Vmax ≥ 4 m/s or mean ≥ 40 mmHg (p. 387); high flow SVi > 58 mL/m² (p. 387); Figure 8's AVA ≤ 1.0 / > 1.0 cm² and SVi ≤ 35 / > 35 mL/m²; LVEF < 50 %; flow reserve SV rise ≥ 20 % (p. 387); dobutamine: area > 1.0 cm² pseudosevere, Vmax ≥ 4 or mean ≥ 40 with area ≤ 1.0 true severe (p. 385); calcium score (Table 5): unlikely men < 1600, women < 800; likely ≥ 2000 / ≥ 1200; very likely ≥ 3000 / ≥ 1600 AU; between: grey zone. |
| `aortic-prosthesis-screening` | Aortic prosthesis: stenosis and mismatch screening | Verified | PV 2024 — Figure 13 and Table 5, p. 19; Table 7, p. 20 | Figure 13 from Vmax > 3 m/s: early peak (AT < 100 ms, AT/ET < 0.37) or late, then DVI ≥ 0.30, 0.25–0.29, < 0.25; Table 5 column of each parameter (AT, AT/ET, Vmax for every valve; mean gradient, DVI, EOA against the reference for surgical valves); Table 7 mismatch by iEOA with the BMI ≥ 30 rows. Worked case: Figure 11 (p. 17), stenotic valve (Vmax 4.1, AT 127, AT/ET 0.4, DVI 0.24) reaches "stenosis"; the normal valve (1.8 m/s) does not enter the figure. |
| `aortic-prosthesis-serial` | Aortic bioprosthesis: change from baseline | Verified | PV 2024 — Table 5 (TAVI rows), p. 19; Table 6, p. 20 | Table 5 TAVI columns for the change in mean gradient, DVI and EOA; Table 6 possible / significant structural valve deterioration (gradient rise with the gradient reached, with the EOA and/or DVI fall, or the AR row as a choice); "stable LV function and blood pressure" is an input because the table assumes it. |
| `mitral-prosthesis-screening` | Mitral prosthesis: stenosis, MR and mismatch screening | Verified | PV 2024 — Table 11, p. 27, main criteria p. 24; Table 12, p. 28; Table 7, p. 20 | Table 11 column of each parameter; Table 12 pattern (mechanical valve: Vmax ≥ 1.9 m/s and VTI ratio ≥ 2.5 with PHT < 130 ms); then the column holding at least three of the five parameters (our reading of the footnote "accuracy is best if most of the parameters are normal or abnormal"); Table 7 mismatch. Figure 19 (p. 28, mean gradient 29 mmHg) is in the significant column (tested). |

Worked cases: DD 2025 prints no numeric worked cases for its algorithms (checked page by page); AS 2017 has none for Figure 8;
PV 2024 Figure 11 and Figure 19 are tested as above.

### Where the source disagrees with itself, and what was chosen

* DD 2025: Figure 3 prints e' "≤ 6, ≤ 7, ≤ 6.5" where Key Point 2 (p. 553) has "< 6, < 7, < 6.5": the figure is followed (Table 6's
  age limits are "<" and used as printed). Figure 2 has average E/e' "> 14", Figure 3 "≥ 14": Figure 3 is followed in the
  algorithm. Figure 4 has E/A "0.8–1.8" where Table 7 has "> 0.8 but < 1.8": the figure is followed. Figure 6 has LARS ≤ 18 %
  where the PH text and key point 4 (p. 560) have LARS < 16 % / ≤ 16 % and key point 3 < 18 %: the figure is followed. Table 7
  gives DT < 140 ms for restrictive cardiomyopathy, the text (p. 561) < 150 ms: the table is followed. Table 7 gives Ar − A
  ≥ 30 ms for MR and HCM, key point 2 (p. 558) > 30 ms: the table is followed. Section 13C gives a positive exercise test with TR
  "> 3.2 m/s", key point 3 (p. 556) "≥ 3.2 m/s": the key point is followed; "HFpEF likely" (text: E/e' "> 14") starts at the
  positive cut-off of 14. Table 7 row 1 lists septal E/e' ≥ 11 and DT < 160 ms (with depressed EF) for AF; Figure 8 has
  > 11 and ≤ 160: the figure is followed.
* AS 2017: Figure 8 has AVA "≤ 1.0" and "> 1.0 → moderate" and SVi "≤ 35"/"> 35", the text (pp. 383–387) "< 1.0" / "≥ 1.0" and
  "< 35" / "≥ 35": the figure is followed. Flow reserve: "> 20 %" (pp. 384–385) and "by 20 % or more" (p. 387): ≥ 20 %. For the
  dobutamine result the text gives "a mean gradient > 30–40 mmHg"; only Vmax ≥ 4 m/s or a mean ≥ 40 mmHg is taken as meeting it,
  a mean in 30–40 mmHg is reported as left open and the path goes on to the calcium score.
* PV 2024: Table 7 has moderate mitral mismatch "1.2–0.91" (so 1.2 is moderate), the text (p. 10) "< 1.2": the table is
  followed. Table 11 has VTI ratio "> 2.5" for significant stenosis, Table 12 "≥ 2.5" for the MR pattern: each table as printed.
  Figure 13 uses DVI "≥ 0.30 / 0.29–0.25 / < 0.25" and Table 5 "> 0.35 / 0.25–0.35 / < 0.25" (surgical valves): each where it
  belongs. Figure 13 draws no arrow from an early peak to DVI < 0.25 nor from a late peak to DVI ≥ 0.30: such cases are reported
  as not drawn. AT exactly 100 ms and AT/ET exactly 0.37 count as late (the gap convention). Table 5 and Table 6 give each
  change as an absolute "or" relative amount: the more abnormal of the two columns is taken.
* Gaps between printed limits go to the more abnormal band, as in part A (Table 11 mean gradient 5–6 mmHg is "possible";
  Supplemental Table 3 LVIDd 5.8–5.9 cm in men is "mildly abnormal"; EF 51–52 % in men is "mildly abnormal").
* Table 7 of DD 2025 lists indicators of elevated pressure for MS, MR, HCM, restrictive cardiomyopathy, tachycardia and LVAD
  without a rule that combines them or establishes normal pressure: those tools report "at least one indicator met" or "none
  met", and say that the table gives no criterion for normal pressure.

### Left out, and why

* `aortic-diameter-index` stays **formula only**: Table 14 (p. 32) gives mean ± SD by sex without abnormality partitions, and
  the guideline defines dilatation as above the 95 % confidence interval of the BSA nomograms of Figure 13, whose upper limit is
  not printed as a number (only the regression lines and their SEE are).
* `lv-fractional-shortening` stays **formula only**: neither Table 2 nor Supplemental Table 3 gives a fractional-shortening range.
* LA volume index: Table 4 (p. 10) is the same for men and women and was graded in part A (`la-volume-index`); RA, RV and
  RV function are graded from RH 2025 Table 1 in part A; tricuspid stenosis is in part A.
* LV mass by the 2D method and 3D volumes: their rows exist (Supplemental Table 3, Table 3) but no calculator takes those
  measurements.
* Regurgitation grades by EROA, volume and fraction: part A already grades every native regurgitation calculator it has
  (Tables 8, 11, 14, 16 of VR 2017); nothing was left to add.
* Diastolic function, not in the brief's list and not implemented: the constrictive-restrictive algorithm (Figure 7, p. 562, an
  image reprinted from Klein et al.), the HFpEF diagnosis algorithm (Figure 9), the qualitative signs in severe AR (p. 558), the
  LAP across an iatrogenic atrial septal defect after TEER (LAP = 4V² + RAP, p. 558), and conduction disease and pacing (no
  algorithm in the source).
* Prosthetic valves, not in the brief's list: AR after AVR (Table 8, Figure 16), prosthetic MR severity (Table 13, Figure 21),
  pulmonary and tricuspid prostheses (Tables 15, 16, 19, 21). The heart rate at which a mitral gradient is measured is not an
  input (Table 11 assumes a normal rate).
* AS: the projected valve area at a normal flow rate (250 mL/s) is mentioned (p. 385) without its formula, so the ejection times
  of the survey's stress inputs are not used; of Table 5 only the calcium score is encoded (its clinical and qualitative rows
  are shown as the "integrated approach" step, not scored).
