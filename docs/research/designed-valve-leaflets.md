# Designed valve leaflets: dimensions and orientation rules (brief B36, scope item 1)

The mitral, aortic and pulmonary valves of the 3-D heart get leaflets that are **designed**: a generator builds them on this
heart's own valve discs from the numbers below. This note is the only source of those numbers. It is written for the
implementer of the generator (`heartsim-geometry`), so every row is a value in mm, a fraction or an angle.

Nothing here comes from `epicardio/`, `epicardio-extracted/`, an atlas image, another simulator or the meshes of the
repositories the brief lists as not used. `github.com/alexkaiser/heart_valves` (MATLAB code) was **not read or downloaded**;
its paper was read instead. Downloaded files (the arXiv PDF, Europe PMC XML, PubMed abstracts) lie outside the repository,
under the session scratchpad, and are not part of the product.

## How to read the tables

* **Full text**: the number was read in the full text of the paper, by the author of this note, on 2026-11-26.
* **Abstract**: only the abstract was reachable (or the abstract states the number); the row says so.
* **Via DesignedParams**: the number is already in `crates/heartsim-echo/src/designed.rs` with the source and locator B31
  recorded. It was **not re-read** here. Those rows repeat what `DesignedParams` says and do not add a claim.
* **Secondary**: a review that quotes another paper. The original was not read. Such a row is a cross-check, never a
  parameter.
* **Designed**: no source was read; the reason is given. A Designed value is a choice, not a measurement.
* "Location not verified" means the number was seen but its table, figure or page was not confirmed.
* Page numbers of the Kaiser paper are the printed numbers of the arXiv v2 PDF (1902.00018v2).

Cohorts differ (porcine against human, CT against autopsy against surgery, patients against donors). The tables keep the
cohort in the row. Do not mix numbers across cohorts without saying so in the code comment.

## Sources read

| Short name | Reference | What was read |
|---|---|---|
| Kaiser 2019 | Kaiser AD, McQueen DM, Peskin CS. *Modeling the mitral valve*. arXiv:1902.00018v2 (2019); the arXiv page lists the published version as doi:10.1002/cnm.3240 (journal name not verified). | Full text (PDF), appendix A in detail. Porcine dissections of the authors plus a design-based model. |
| Ishida 2026 | Ishida K, Igarashi T, Yokoyama H, Imasaka K. Determinants of effective height in morphologically normal tricuspid aortic valves: a computed tomography-based anatomical study. *Interdiscip Cardiovasc Thorac Surg* 2026;41(11):ivag233. doi:10.1093/icvts/ivag233 (CC BY). | Full text via Europe PMC (PMC13730563). ECG-gated CT, n = 105 (mean age 70.7 y), normal tricuspid valves, diastole. |
| Viva 2026 | Viva T, Fiore C, Dulgheru R, Andreini D, Glauber M, Lancellotti P. Advanced 3D echocardiographic assessment of the aortic valve and aortic root complex: a repair-oriented perspective. *Eur Heart J Imaging Methods Pract* 2026;4(3):qyag136. doi:10.1093/ehjimp/qyag136 (CC BY). | Full text via Europe PMC (PMC13489153). Review; definitions are taken from it, its normal values are secondary. |
| Rankin 2008 | Rankin JS, Dalley AF, Crooke PS, Anderson RH. A "hemispherical" model of aortic valvar geometry. *J Heart Valve Dis* 2008;17(2):179-186. PMID 18512488 (no DOI in the PubMed record). | **Abstract only.** 8 human cadaver hearts. |
| De Kerchove 2018 | De Kerchove L, Momeni M, Aphram G, Watremez C, Bollen X, Jashari R, Boodhwani M, Astarci P, Noirhomme P, El Khoury G. Free margin length and coaptation surface area in normal tricuspid aortic valve: an anatomical study. *Eur J Cardiothorac Surg* 2018;53(5):1040-1048. doi:10.1093/ejcts/ezx456 | **Full text read 2026-11-26** (PDF supplied by the maintainer at `docs/research/aortic/ezx456.pdf`, git-ignored: © the authors, published by OUP, all rights reserved). 25 homograft roots, donors 52 ± 6 years; six measures on fresh tissue **under tension**, coaptation after formalin fixation in the diastolic position. See the section "De Kerchove 2018 in full text" below. The rows that say "abstract only" for this paper are confirmed by Table 1. |
| Haj-Ali 2012 | Haj-Ali R, Marom G, Ben Zekry S, Rosenfeld M, Raanani E. A general three-dimensional parametric geometry of the native aortic valve and root for biomechanical modeling. *J Biomech* 2012;45(14):2392-2397. doi:10.1016/j.jbiomech.2012.07.017 | **Full text read 2026-11-26** (PDF supplied by the maintainer at `docs/research/aortic/j.jbiomech.2012.07.017.pdf`, git-ignored: © Elsevier, all rights reserved). A parametric model of the shut valve, not a measurement; one 3D-TEE patient as a check (mean error 0.78 ± 0.63 mm, p. 2395). See the section "Haj-Ali 2012 in full text" below. |
| Garbi 2015 | Garbi M, Monaghan MJ. Quantitative mitral valve anatomy and pathology. *Echo Res Pract* 2015;2(3):R63-R72. doi:10.1530/erp-15-0008 (CC BY-NC). | Full text via Europe PMC (PMC4676476). Review: every number is **secondary**. |
| Mehari Abraha 2026 | Mehari Abraha H, Goddard C, Bryan R, Hyde-Linaker G, Conway C. Morphological variation in the human mitral valve using statistical shape modelling. *Ann Biomed Eng* 2026;54(10):3465-3476. doi:10.1007/s10439-026-04060-6 (CC BY). | Full text via Europe PMC (PMC13649912). 72 contrast CT scans; ranges only. |
| Cureus 2026 | Kumar LK, Chopra J, Rai A, Rani A, Verma A, Singh H. Morphometric study of the mitral and tricuspid annuli in fresh cadaveric human hearts from Uttar Pradesh, India. *Cureus* 2026;18(10):e117971. doi:10.7759/cureus.117971 (CC BY). The author initials are as Europe PMC lists them ("L K K"). | Full text via Europe PMC (PMC13769421). 100 fresh hearts, vernier caliper. |
| Lis 2023 | Lis M, Krawczyk-Ożóg A, Hołda J, Tyrak K, Dudkiewicz D, Yakovliev A, Strona M, Bolechała F, Jakiel R, Jakiel M, Hołda MK. Pulmonary valve morphometry revisited: clinical implications for valvular and supravalvular interventions. *Clin Anat* 2023;36(2):234-241. doi:10.1002/ca.23959 | **Full text read 2026-11-26** (PDF supplied by the maintainer at `docs/research/pulmonary/ca.23959.pdf`, git-ignored: © the publisher; page numbers below are those of the early-view PDF, 1 to 8). 182 autopsied hearts, fixed with the valve shut, calipers. See the section "Lis 2023 in full text" below; the rows P1 to P9 written from the abstract stand unless that section corrects them. |
| Kunzelman 1994 | Kunzelman KS, Cochran RP, Verrier ED, Eberhart RC. Anatomic basis for mitral valve modelling. *J Heart Valve Dis* 1994;3(5):491-496. PMID 8000582. | **Abstract only.** It lists what was measured but the abstract (truncated) gives no dimension. Not used for numbers. |
| Ranganathan 1970 | Ranganathan N, Lam JHC, Wigle ED, Silver MD. Morphology of the human mitral valve. II. The valve leaflets. *Circulation* 1970;41(3):459-467. doi:10.1161/01.cir.41.3.459 | **Full text read 2026-11-26** (PDF supplied by the maintainer at `docs/research/mitral/01.CIR.41.3.459.pdf`, git-ignored: © the publisher). 50 normal adult mitral valves at autopsy (26 male, 24 female), caliper and rule on the opened valve. See the section "Ranganathan 1970 in full text" below. |

Not reached at all: Swanson and Clark 1974, Thubrikar's work, Schäfers' effective-height papers (no PubMed hit for the
queries tried, no open full text), Carpentier's papers, the ASE/EACVI guideline text. None is cited below.

## Mitral valve

Frame: the generator works on this heart's mitral disc (centre, downstream normal = atrium to ventricle, rim). Angles
below are measured **around the disc normal**, from the direction of the anterior leaflet's centre (rule M1, section
"Orientation rules"), positive toward the anterolateral side.

| # | Parameter | Value | Source and place | Note |
|---|---|---|---|---|
| M1 | Anterior leaflet length (annulus to free edge, mid-leaflet) | **25.1 mm** (SD 3.7) | Via DesignedParams: Tarkiainen 2016, *J Cardiovasc Magn Reson* 18:33, doi:10.1186/s12968-016-0250-5, Table 1. Not re-read. | Already the default. Cross-check, full text: the CT shape model of Mehari Abraha 2026 gives a range 23.5 to 36.8 mm (Results, paragraph on the five linear measurements; no table or figure number verified). Consistent. |
| M2 | Posterior leaflet length | **14.4 mm** (SD 3.6) | Via DesignedParams, same table. | Cross-check, full text: range 13.2 to 25.3 mm in Mehari Abraha 2026, same paragraph. Consistent. |
| M3 | Share of the annulus taken by the anterior leaflet | **150 degrees of 360 = 0.417** | Kaiser 2019, full text, Appendix A.2.2, p. 32: `a_ant = 5 pi / 6`, `a_post = 7 pi / 6`, anterior leaflet fixed for `theta` in (-a_ant/2, a_ant/2). These angles are a **design choice** of that model, which the paper bases on McCarthy 2010 (secondary) saying the anterior leaflet takes less than half the ring. | Cross-check (secondary, review text): annular circumferential length of about 3 cm anterior and about 5 cm posterior, so 0.375 anterior, i.e. 135 degrees (Garbi 2015, section on leaflet dimensions, quoting Dal-Bianco and Levine 2013; the original not read). Use 150 degrees; 135 degrees is the lower bound. |
| M4 | Share taken by the posterior leaflet | **210 degrees = 0.583** | Kaiser 2019, same place. | Complement of M3. |
| M5 | Commissure positions | Anterolateral commissure at **+75 degrees**, posteromedial at **-75 degrees** from the anterior leaflet's centre, i.e. at the two ends of the anterior leaflet's attachment | Kaiser 2019, A.2.2 p. 32 (ends of the anterior arc). The paper says the leaflet separation at the commissures is incomplete: a band of leaflet runs unbroken below the ring (Section 2, and A.1, p. 28). | The commissures lie where the anterior arc ends; nothing more precise was read. |
| M6 | Annulus shape, anterior-posterior to commissure-to-commissure ratio | Kaiser model **5:8 = 0.625** (design, with a dip `rho_dip = 0.75` on the anterior side); a manufacturer's 3:4 is quoted there. Fresh cadaver hearts: AP **27 +/- 4 mm**, intercommissural **29 +/- 3 mm** (ratio 0.93). | Kaiser 2019, A.2.2 p. 33. Cureus 2026, Table 1 (full text; AP 2.7 +/- 0.4 cm, transverse 2.9 +/- 0.3 cm, n = 100, orifice area 6.19 +/- 1.34 cm2). | **Not a generator input.** The ring comes from the pack's disc. Use only as a sanity check on the pack's rim. The two sources disagree because the Kaiser ring is a design value for a closed, systolic ring. |
| M7 | Posterior scallops: count | **3** (P1 lateral, P2 middle, P3 medial) | Garbi 2015 uses P2 and "scallops" throughout, full text. Carpentier's nomenclature itself was not read. | Convention. |
| M8 | Scallop widths as fractions of the posterior leaflet's attachment | P1 : P2 : P3 = **0.30 : 0.40 : 0.30** | **Designed.** Only "P2 is larger than the other two scallops in normal hearts" was read (Garbi 2015, section on leaflet dimensions, secondary, quoting its ref. 19, not read). No width in mm was found in a source that could be opened. | The 0.40 is a choice. |
| M9 | Indentation (cleft) depth between scallops, as a fraction of the local leaflet height | **0.20** | **Designed.** The one number read is a definition: a "cleft" reaches more than 50 % of the leaflet height, an "indentation" less than 50 % (Garbi 2015, section on 3D quantification, secondary). 0.20 keeps a normal valve well under that. | Not a pathological cleft. Never above 0.50. |
| M10 | Commissural leaflet height (at M5) | **7 mm** (about half of M2) | **Designed.** Kunzelman 1994 measured commissural heights, but the abstract gives no number. Kaiser 2019 only says the commissures are not completely separated. | Joins M1 and M2 smoothly. |
| M11 | Anterior leaflet height profile along its attachment | `h(a) = M10 + (M1 - M10) * cos^2(pi * a / (2 * 75 deg))`, `a` = angle from the leaflet centre, `|a| <= 75 deg` | **Designed**: smooth, deepest at the centre, equal to M10 at the commissures. Kaiser's anterior leaflet is also "deeper" at the centre (Section 2). | |
| M12 | Posterior leaflet height profile | Height M2 at the middle of each scallop, reduced to `(1 - M9) * M2` (not below M10) at the two scallop boundaries, falling to M10 at the commissures; smooth blends | **Designed.** | |
| M13 | Coaptation (tip overlap), shut | **4.0 mm** | Already `DesignedParams::av_coaptation_mm`, marked Designed. | **See conflict C1.** A review quotes about 10 mm for the coaptation length (Garbi 2015, section on leaflet dimensions, secondary, citing Silbiger 2011, not read). The two may not measure the same thing (overlap of the tips against the length of the apposed zone). Kept at 4.0 mm. |
| M14 | Coaptation depth (shut: coaptation line below the annulus plane) | Not set by a number. | **Designed.** Only a surgical cut-off was read: depth at or under 10 mm suggests a repairable valve (Garbi 2015, section on intervention planning, secondary, patients with remodelling). | The coaptation depth follows from M1, M2 and M13 and the disc. Report it; do not tune it. |
| M15 | Billow of the shut leaflets toward the atrium | At most **1 mm** above the annulus plane; the brief says never above | Garbi 2015, section on 3D quantification, secondary: billowing height above 1.0 mm separates degenerative disease from normal valves. | The brief's "never above" is stricter and stays. |
| M16 | Total leaflet area against the annulus area | About **2** (leaflets "almost twice" the valve area); anterior and posterior leaflets of nearly equal area | Garbi 2015, section on leaflet dimensions, secondary (ref. 9, Dal-Bianco and Levine 2013, not read; ref. 19 not read). | An acceptance cross-check on the generator's reported areas, not a parameter. |
| M17 | Leaflet thickness | **1.6 mm** | Via DesignedParams: Sahasakul 1988, *Am J Cardiol* 62(7):424-430, doi:10.1016/0002-9149(88)90971-X (abstract only). | Kaiser 2019 thickens its model to about 0.7 mm (A.3.3) for numerical reasons and calls that about the thickness of a leaflet. That is a different number; DesignedParams stays. |
| M18 | Open leaflet angle to the valve axis | **10 degrees**, leaning toward the axis | Via DesignedParams, marked Designed. | |
| M19 | Chordae drawn: count | Per papillary muscle: **2** to the anterior leaflet, **2** to the central posterior leaflet, **2** to the commissural region of the posterior leaflet on its side; **12** in all | **Designed**, inside the ranges Kaiser 2019 reports from its own porcine dissections (A.2.3, p. 33): one to two chordae per muscle supporting the anterior leaflet, one to two supporting the central posterior leaflet, two to four supporting each side of the posterior leaflet near the commissures; "many specimens have more". | Porcine, the authors' own observation. Human counts were not found in a source that could be opened. |
| M20 | Where chordae leave the muscle | Spread over about three quarters of a circle around the muscle tip, the open quarter pointing at the other muscle | Kaiser 2019, A.1, Figure A.4 and its text, p. 28. | For fan-out only; the muscle tips come from `papillary_muscles`. |
| M21 | Where chordae insert | Free edge and rough zone: along the free edge and in a band up to **0.25 x the local leaflet height** from it; clear zone toward the annulus has none | **Designed**. A clear zone near the base, devoid of chordal attachments, is described in Viva 2025 (full text, section on the ventricular view; not used for a number). The 0.25 is a choice. | Viva T, Fiore C, Lancellotti P. *Eur Heart J Imaging Methods Pract* 2025;3(4):qyaf128, doi:10.1093/ehjimp/qyaf128 (CC BY-NC), full text via PMC12596252. |
| M22 | Which muscle serves which half | The anterolateral muscle serves the anterolateral half of each leaflet (the side of the anterolateral commissure), the posteromedial muscle the other half; the two halves meet at the leaflet's centre line. Each muscle serves **both** leaflets. | **Designed** (the usual arrangement). Kaiser 2019 A.2.3 p. 33 supports "each muscle serves both leaflets" (one anterior tree and one posterior tree per muscle); the half-by-half split was not read as a rule. | |
| M23 | Muscle tip to annulus distance | **20.7 mm** (anterolateral), **22.75 mm** (posteromedial) | Via DesignedParams: Candir et al., *Folia Morphol* 2025;84(1):83-96, Table 8. | Kaiser 2019 A.3.2 p. 43 quotes Sanfilippo et al. (secondary) that the annulus-to-tip distance does not change significantly through systole; the generator keeps the tips fixed. |

### Ranganathan 1970 in full text (added 2026-11-26)

Values are male / female means in mm (the paper prints cm), measured on the opened, flattened valve: heights from free
edge to base at the leaflet's centre, widths along the base.

| # | What | Value | Place | Use |
|---|---|---|---|---|
| R1 | Anterior leaflet | height 24 / 22, width at the base 36 / 29 | Table 2 (p. 464) | Agrees with M1 (25.1 mm). Free margin "with few or no indentations", shape semicircular or triangular (p. 461). |
| R2 | Posterior leaflet, middle scallop | height 14 / 12, width 23 / 18 | Table 2 | Agrees with M2 (14.4 mm), which is therefore the **middle scallop's** height. |
| R3 | Anterolateral commissural scallop (P1) | height 11 / 10, width 16 / 14 | Table 2 | |
| R4 | Posteromedial commissural scallop (P3) | height 10 / 8, width 15 / 11 | Table 2 | |
| R5 | Scallop widths as fractions of the posterior leaflet, **derived here** from R2 to R4 | male 0.30 : 0.43 : 0.28, female 0.33 : 0.42 : 0.26; use **0.31 : 0.42 : 0.27** (P1 : P2 : P3) | Table 2 | **Replaces M8** (Designed 0.30 : 0.40 : 0.30), now sourced. |
| R6 | Scallop heights against the middle scallop, **derived here** | P1 0.81, P3 0.69 (means of male and female ratios) | Table 2 | **New for M12**: the two commissural scallops are shorter than the middle one, the posteromedial one the shortest. With M2 = 14.4 mm: P1 11.7 mm, P3 9.9 mm. |
| R7 | Number of scallops | three in 46 of 50 hearts, a large middle scallop in 42 of those; two scallops in 2, five in 2 | Table 4 (p. 465) | Sources M7 directly. |
| R8 | Commissural areas | height at the centre 8 / 7 (anterolateral) and 8 / 8 (posteromedial); spread of the commissural chorda along the free edge 12 / 9 and 18 / 15 | Table 1 (p. 462) | **Replaces M10** (Designed 7 mm): use **7.5 mm**. The valve is a continuous veil; the commissures are indentations, not gaps (p. 459). |
| R9 | Share of the annulus, **derived here** from the base widths | anterior 36 / (36 + 54) = 0.40 male, 29 / (29 + 43) = 0.40 female, the commissural areas left out | Table 2; "the posterior leaflet has a wider attachment to the atrioventricular annulus than the anterior leaflet" (p. 462, 466) | Supports M3 (0.417). Keep M3 and M5. |
| R10 | Rough and clear zones | anterior leaflet: rough 9 / 8, clear 13 / 13, ratio 0.6; middle scallop: rough 7 / 7, clear 5 / 5, ratio 1.4; the posterior scallops also have a basal zone next to the annulus, and their clear zone "extends to within 2 mm of the valve annulus" | Table 3 (p. 464); p. 464 text | **Replaces the 0.25 of M21**: chordae insert in the rough zone, which is the distal **0.37** of the anterior leaflet's height (8.5 of 23) and the distal **0.55** of a posterior scallop's (7 of about 13). The basal chordae of the posterior leaflet are not drawn (Designed). |
| R11 | Line of closure | A ridge on the atrial face of the anterior leaflet, **8 to 10 mm from its free margin**, "defines the line of leaflet closure"; the rough zone beyond it "comes into apposition with its counterpart on the posterior leaflet" | p. 461 | Bears on **C1**: the apposed band is about 7 to 9 mm of each leaflet, not the 4.0 mm of `av_coaptation_mm` (Designed). `DesignedParams` is not changed by this note; see C1. |
| R12 | Chordae to the scallop boundaries | "Two cleft chordae were obvious in all tri-scalloped" leaflets, fanning into the clefts; one fan-shaped commissural chorda per commissure, from the papillary muscle tip, in 49 of 50 hearts | p. 460, 463; Figures 1 and 5 legends | For M19: of the chordae drawn, one per side should go to the cleft between scallops and one to the commissure. Counts stay Designed. |
| R13 | Anterior leaflet and the aortic valve | "a common attachment to the cardiac skeleton with the left coronary cusp and half of the noncoronary cusp of the aortic valve" | p. 462 | Sources the mitral orientation rule and helps the aortic one: the two aortic cusps nearest the anterior mitral leaflet are the left coronary and the non-coronary; the third is the right coronary. |

**C1, updated.** R11 is a primary source for an apposed band of 7 to 9 mm on each leaflet, against the designed 4.0 mm.
**Decided by the maintainer on 2026-11-26: the mitral coaptation is 8.0 mm** (row M13 is superseded). The tricuspid
value stays 4.0 mm; the echo targets that depend on the mitral value are re-derived.

## Aortic valve

Frame: the aortic disc (centre, downstream normal = ventricle to artery). The three cusps are at 0, 120 and 240 degrees
around the normal; commissures at 60, 180, 300 degrees from the first cusp's centre (rule A1 assigns the names). Heights
are measured from the plane of the three cusp nadirs (the "virtual basal ring", VBR), which is the disc plane.

| # | Parameter | Value | Source and place | Note |
|---|---|---|---|---|
| A1 | Annulus (VBR) diameter, for scale | 24.2 +/- 1.9 mm | Ishida 2026, full text, Table 2. | Sanity check on the pack's disc only. |
| A2 | Geometric height (nadir of the attachment to the midpoint of the free margin) | Mean **14.9 +/- 1.4 mm**; right 14.4, left 14.4, non-coronary 15.9 mm | Ishida 2026, Table 2 (CT, diastole, n = 105). | Matches `DesignedParams` (Izawa 2021, 14.7 +/- 1.3 mm, Table 2, not re-read). Use 14.7 as the project does. The non-coronary cusp is the longest cusp (Ishida 2026, Results). |
| A3 | Effective height (VBR plane to the free-margin midpoint, shut) | **8.5 +/- 1.3 mm**; the three cusps do not differ (P = .232) | Ishida 2026, Table 2. Definition: Viva 2026, Table 1 and the section on effective height (full text). | Matches `DesignedParams` (Izawa 2021, 8.6 +/- 1.4 mm). Use 8.6. |
| A4 | Link between A2, A3 and the annulus radius | A shut cusp is, to 0.5 mm, a straight line from the nadir at the wall to the centre: `sqrt(R^2 + eH^2)`. With R = 12.1 mm and eH = 8.5 mm this is 14.8 mm against the measured 14.9 mm. | **Derived here** from A1, A2, A3 (same cohort, Ishida 2026). | The generator builds the cusp length from the disc radius and the effective height; this is why the acceptance test can ask for both heights within 0.5 mm. |
| A5 | Surgical geometric height | About 19 to 20 mm; surgical thresholds 16 mm | Viva 2026, Table 1 (review, **secondary**; its references 14 and 19 not read). Also De Kerchove 2018 (abstract): free margin length divided by geometric height = 1.81. | Taken on a cusp under tension, so larger than the CT value (A2). **Do not mix** A5 with A2/A3 in one cusp. The CT pair is self-consistent (A4). |
| A6 | Free-edge length, per cusp | Design value **27 mm** = 1.81 x 14.9 mm. Bracketed by 24.5 +/- 2.1 mm (unloaded, 8 cadaver hearts, **abstract only**) and 34.3 +/- 3.1 mm (under tension, 25 homografts, **abstract only**). | Rankin 2008 abstract (L = 24.5 mm; D' = 24.7 mm; 120 degrees per cusp). De Kerchove 2018 abstract (FML 34.3 mm; FML/gH 1.81; FML / sinotubular junction diameter 1.29). The 27 mm is **derived here**, mixing two cohorts, as a midpoint. | The free edge of a shut cusp is the two straight runs from the commissures to the centre; its length follows from the disc radius and the commissure height. Report it and compare with the bracket; do not force it. |
| A7 | Commissure height (VBR plane to the top of the commissure) | **18.6 +/- 1.8 mm** (right-left 18.7, left-non 17.7, non-right 19.6 mm) | Ishida 2026, Table 2; the commissure top is marked in its Figure 1C legend. The lower end is the VBR plane by the figure's legend; the definition in the Methods text was not verified. | 18.6 / 14.9 = 1.25 x A2. Rankin 2008 (abstract) measures a different thing, the height of the zone of apposition between two cusps: 11.9 +/- 1.0 mm, about equal to the radius (12.0 +/- 1.6 mm). |
| A8 | Cusp attachment line (crown) | `z(phi) = A7 * (1 - cos(3 phi)) / 2` in the disc frame, `phi` = angle from a cusp's centre: 0 at the nadir (`phi` = 0), A7 at the commissures (`phi` = +/- 60 degrees) | **Designed**: smooth crown through the two measured points. Haj-Ali 2012 gives no closed formula for the attachment line: it is the intersection of the cusp surface with the sinus, found numerically (full text, p. 2394 and 2396). Rankin 2008 abstract says the attachments are the intersection of three hemispheres with a cylinder, each cusp spanning 120 degrees; its equations were not read. | If a later reader obtains Haj-Ali or Rankin in full text, replace this row. |
| A9 | Valsalva height, sinus diameter, sinotubular junction | Valsalva height 22.5 +/- 2.4 mm; sinus diameter 34.8 +/- 2.9 mm; commissure diameter 32.3 +/- 2.9 mm; sinotubular junction 30.1 +/- 2.7 mm; STJ/VBR about 1.2 | Ishida 2026, Table 2; STJ/VBR ratio of about 1.2 also in Viva 2026 Table 1 (secondary). | **Not used.** The pack's vessels are tubes without sinuses (brief B36, out of scope). Listed so the implementer knows how much room the open cusps would have in real sinuses. |
| A10 | Coaptation length (apposed zone), shut | Central **3.3 +/- 0.8 mm**, lateral **5.9 +/- 0.6 mm**; coaptation surface 122 +/- 21 mm2 per cusp, 41 % of the cusp surface | De Kerchove 2018, abstract only. A review quotes "about 5 mm" for the coaptation height (Viva 2026 Table 1, secondary). | Gives the overlap of neighbouring cusps. Use central 3.3 mm. |
| A11 | Nodule of Arantius | **Designed**: no separate nodule; the three free-edge midpoints meet at one point on the axis at eH | Thickness 0.87 mm at ages 20 to 59 (Sahasakul 1988, abstract only, via DesignedParams) is below the 1 mm voxel floor. Viva 2026 describes the nodule only as the free-margin midpoint. | |
| A12 | Cusp thickness drawn | **1.0 mm** | Via DesignedParams (one voxel). | |
| A13 | Opening in systole: shape | A rounded triangle: each cusp's free edge swings toward the vessel wall; the three free edges stay straight-ish and meet the commissures | **Designed.** The brief asks for it; no source for the shape was read. | |
| A14 | Opening in systole: orifice area as a fraction of the annulus area | **0.75** | **Designed.** No source was read. Nothing in the papers opened here gives a systolic orifice area against the annulus area. | The acceptance test asks for a stated fraction with its source: the source is "Designed". See the list of what could not be sourced. |
| A15 | Opening motion: angle of the open cusp to the axis | `open_angle_from_axis_deg` = **10 degrees** | Via DesignedParams, Designed. | Containment in the tube decides the final opening, as the brief says. |
| A16 | Timing | `valve_openings` | Via DesignedParams (Zhong 2021, Leyh 1999, abstracts only). | No new clock. |

### Haj-Ali 2012 in full text (added 2026-11-26)

Read after the tables above were written; rows A2, A3, A7 and A8 keep their values, and this section says how the paper
compares. Only numbers and relations are taken, no figure and no mesh.

| # | What | Value | Place | Use |
|---|---|---|---|---|
| H1 | Healthy parametric case, all lengths as multiples of the annulus radius `r_r` = 12 mm | sinus radius 1.4, aortic radius 1.1, root height `h1 + h2` = 0.6 + 0.8 = 1.4, free-edge height at the centre `h_f` = 0.35, at the commissure `h_c` = 0.5, inner radii `r_fo` = 0.27 and `r_f` = 0.07, exponents `n` = 1.5, `N` = 1.2, `m` = 1.25 | Table 1, first column (p. 2395) | Heights are measured from the plane `z = 0`, "the lower edge of the coaptation region" (p. 2393), which is `h1` above the cusp base. Reading the 0.6 as `h1` is this note's reading of "(0.6 + 0.8)"; the paper does not label the two terms. |
| H2 | Geometric height of that case | 16.2 mm = 1.36 `r_r` = 0.68 of the annulus diameter; literature 0.66 to 0.76, mean 0.7 (Swanson and Clark 1974, as quoted) | p. 2395; Table 1 | Against A2: 14.7 mm on a 24.2 mm annulus is 0.61, below this range. Excised-valve and model values run higher than CT ones (see C2). Keep A2; on a pack disc of another size, scale the height with the disc (ratio 0.61), and report the result against 0.66 to 0.76. |
| H3 | Cusp profile in the symmetry plane (belly) | `z = -h1 + h1 * ((r_v - x) / (r_v - r_fo))^m`, from the wall (`x = r_v`, base) to `x = r_fo` at `z = 0`; above it the coapting part is a straight line | Eq. 6 and the text under it (p. 2394) | A sourced alternative to the straight line of A4: with `m` = 1.25 the belly is slightly concave toward the ventricle. May replace the shut cusp's radial profile; mark which is used. |
| H4 | Free edge of the shut cusp | In plan: `x = r_f + (r_c cos θ1 - r_f) * (y / (r_c sin θ1))^N`. Its height rises linearly with the plan distance from the centre end, from `h_f` to `h_c` at the commissure | Eqs. 7 and 8 (p. 2394) | So the commissure end of the free edge is 0.15 `r_r` (1.8 mm at 12 mm) higher than its centre. Use for the shut keyframe's free-edge height. |
| H5 | Angular widths | Each sinus with its cusp spans 116 degrees and each commissure 4 degrees; `θ1` = 58 degrees | Section 2.2 (p. 2395) | Replaces the 120 degrees per cusp of A8's frame if commissure posts are drawn; with one-voxel cusps 120 degrees is within tolerance. |
| H6 | Derived relations | commissure radius `r_co = (r_STJ + r_s) / 2`; `r_c = 2/3 (r_co - r_STJ) + r_STJ`; annulus radius = LV outflow radius = cusp base radius | Eqs. 13 and 14 (p. 2395) | Root and sinus shape: not used (tube vessels, out of scope). |

The paper gives no open (systolic) shape, no orifice area and no thickness: A13 and A14 stay Designed.

### De Kerchove 2018 in full text (added 2026-11-26)

| # | What | Value | Place | Use |
|---|---|---|---|---|
| K1 | Confirmed from the abstract | free margin length 34.3 ± 3.1 mm; central coaptation length 3.3 ± 0.8 mm, lateral 5.9 ± 0.6 mm; coaptation surface 122 ± 21 mm² per cusp, 41 % of the cusp surface (range 31 to 49 %) | Table 1 (p. 1042); p. 1046 | A6 and A10 stand, now from the full text. |
| K2 | The same valves, other measures (fresh, under tension) | ventriculo-aortic junction 24.2 ± 1.8 mm; sinotubular junction 26.6 ± 2.4 mm; geometric height 18.9 ± 1.5 mm; effective height 8.5 ± 1.3 mm; commissure height 20.7 ± 1.5 mm; commissure distance 24.6 ± 2.7 mm; cusp insertion length 51.0 ± 4.9 mm | Table 1 | The **effective height equals the CT value** (A3, 8.5 mm) although the geometric height is 4 mm longer than on CT (A2): the extra length is the stretch and the belly, as C2 says. Keep A2 and A3. The commissure height here is measured from the line joining two cusp nadirs to the commissure tip, under tension (p. 1041); A7 (18.6 mm, CT) stays. |
| K3 | Ratios to the geometric height | effective height 0.45, commissure height 1.1, commissure distance 1.3, free margin 1.81, ventriculo-aortic junction 1.28, sinotubular junction 1.41, cusp insertion length 2.69 | Figure 1 (p. 1041), numbers only | Scale-free checks for the generated cusp. With the CT pair of this note the effective height is 8.6 / 14.7 = 0.59 of the geometric height; with this paper's stretched cusp 0.45. |
| K4 | Cusp surface, fixed and flattened | 297 ± 41 mm² per cusp (range 224 to 444); non-coronary 303, left 290 mm² (the right coronary row prints "399 ± 47", which does not fit the mean and the range of the same table: read as a misprint, not used) | Table 1 | Acceptance cross-check: report each generated cusp's area against 297 ± 41 mm², scaled by (disc diameter / 24.2)². |
| K5 | Shape of the coaptation band | Two semilunar areas joined on the cusp's midline: the band is **narrowest at the centre (3.3 mm) and widest on each side of it (5.9 mm)** | p. 1043 (definitions), p. 1046, Figure 4 legend | For the shut keyframe: neighbouring cusps overlap by 3.3 mm at the centre, rising to about 5.9 mm midway to the commissure. Replaces "use central 3.3 mm" of A10 where the generator can vary the overlap along the free edge. |
| K6 | Differences between the three cusps | none significant for geometric height, free margin, effective height, coaptation; the right/non commissure is higher than the left/right one (21.8 against 19.7 mm, P = 0.005) | Table 1; p. 1045 | Supports three equal cusps (Designed symmetric valve). Ishida 2026 found the non-coronary cusp longest on CT (A2); the difference is 1.5 mm and is not modelled. |
| K7 | What it does not give | the open valve; the orifice; thickness | | A13 and A14 stay Designed. |

## Pulmonary valve

No full text with pulmonary cusp heights could be opened, so the cusps are the aortic cusps scaled to this heart's
pulmonary disc, as `DesignedParams` already does for the effective height. Each row names the scaling.

| # | Parameter | Value | Source and place | Note |
|---|---|---|---|---|
| P1 | Annulus (basal ring), for scale | Area **433.9 +/- 133.6 mm2** (equivalent diameter 23.5 mm, derived here) | Lis 2023, abstract only. | Sanity check on the pack's disc. About the aortic annulus (A1). |
| P2 | Effective height | **8.6 mm** | Via DesignedParams: Designed, equal to the aortic value. | |
| P3 | Geometric height | `sqrt(R^2 + eH^2)` with R the disc radius | **Designed**: the straight-cusp relation of A4, which was checked on aortic data only. | |
| P4 | Commissure height | **1.25 x P3** | **Designed**: the aortic ratio (A7 / A2), applied to the pulmonary cusps. | |
| P5 | Free-edge length, coaptation, nodule, crown, opening shape and area | As A6, A10, A11, A8, A13, A14 | **Designed** (aortic values). | |
| P6 | Cusp thickness | **1.0 mm** | Via DesignedParams (voxel floor). The pulmonary cusps are thinner than the aortic ones in reality; no number was read. | |
| P7 | Root areas, for sanity checks | Coaptation-centre plane 626.7 +/- 191.7 mm2; basal ring 433.9 +/- 133.6 mm2; tubular plane 290.0 +/- 110.1 mm2 | Lis 2023, abstract only. | The open cusps in a tube vessel (tubular plane smaller than the basal ring) will not fit as in the real sinuses. Report the containment as the brief asks. |
| P8 | Sinus depth and height | Depth: left anterior 15.59 +/- 2.91, posterior 16.04 +/- 2.82, right anterior 16.21 +/- 2.81 mm. Height: 15.24 +/- 3.10, 19.12 +/- 3.79, 18.59 +/- 4.03 mm. | Lis 2023, abstract only. | **Not used** (no sinuses). |
| P9 | Asymmetry | Only 31.3 % of valves have the coaptation centre near the valve's geometric centre; sinuses of similar size in 35.7 %; fenestrations in 12.5 % | Lis 2023, abstract only. | The generator is symmetric on purpose; this is a note for the docs ("a designed symmetric valve"). |

### Lis 2023 in full text (added 2026-11-26)

| # | What | Value | Place | Use |
|---|---|---|---|---|
| L1 | Geometric height of each pulmonary leaflet (nadir to the central part of the free margin) | left anterior **15.24 ± 3.10**, posterior **15.69 ± 3.38**, right anterior **15.49 ± 2.79 mm**; no difference between leaflets (p = 0.375); mean of the three 15.5 mm | Table 1 (p. 4); definition in section 2.2 (p. 3) | **Replaces the scaled aortic value** for the pulmonary geometric height. Cadaver calipers, so compare with the caution of C2; it is close to the aortic CT value (14.7 mm) all the same. |
| L2 | Basal ring | perimeter 73.6 ± 10.9 mm (equivalent diameter 23.4 mm, derived here), area 433.9 ± 133.6 mm² | Section 3.1 (p. 4) | Confirms P1. Geometric height / ring diameter = 15.5 / 23.4 = 0.66 (derived here): the ratio to use when the pack's pulmonary disc has another size. |
| L3 | Sinus height (nadir to the tubular plane) | left anterior 19.20 ± 3.59, posterior 19.12 ± 3.79, right anterior 18.59 ± 4.03 mm | Table 1 (p. 4) | **Corrects P8**: the abstract prints 15.24 mm as the left anterior sinus height; in Table 1 that number is the left anterior geometric height (L1) and the sinus height is 19.20 mm. Not used by the generator (no sinuses). |
| L4 | Intercommissural angle at the coaptation centre | left anterior 124.5 ± 36.2, posterior 110.5 ± 28.5, right anterior 125.1 ± 32.6 degrees (p < 0.001) | Table 1 (p. 4) | The posterior leaflet is the narrowest on average. The generator may keep 120 degrees each (Designed, symmetric valve) or use 124.5 / 110.5 / 125.0; say which. |
| L5 | Intercommissural distance | 17.36 ± 4.25, 17.62 ± 3.61, 17.21 ± 4.37 mm (same order) | Table 1 (p. 4) | Check on the generated shut valve: chord between neighbouring commissures. |
| L6 | Nodules of Arantius present | 85.2 %, 72.5 %, 85.2 % of leaflets (same order) | Table 1 (p. 4) | A nodule is the usual finding. It is still not drawn (below the drawn thickness); say so in known issues. |
| L7 | Leaflet names | "the right anterior, the left anterior, and the posterior leaflet", called a nomenclature "referring to the anatomical position" | Section 2.2 (p. 2) | See Pu-R1 below. |
| L8 | The valve was fixed and measured shut; "we were unable to evaluate the behavior and dimensional changes of PV components in the cardiac cycle" | | Sections 2.2 and 4 (p. 2, 7) | No open shape and no orifice area from this paper: the open keyframe stays Designed. |

**Pu-R1, revised.** The paper names the leaflets by where they lie in the body and does not define them by the aorta or by
other landmarks. So the rule that follows the source is positional, in the heart frame (+x left, +z anterior): the cusp
whose centre is most posterior is the **posterior** cusp; of the other two, the one further to the patient's left is the
**left anterior** and the other the **right anterior**. This replaces "anterior, left, right" of the brief and the rule
"anterior = pointing away from the aortic disc". How these names map to the other schemes in use (anterior / left / right;
non-facing / left-facing / right-facing) was not verified and is not claimed.

## Orientation rules

What the pack gives (from `heartsim-echo`): the four valve discs (centre, downstream normal, rim); the label volumes (LV,
RV, LA, RA blood and myocardium, aorta and pulmonary artery lumen); the two papillary muscles from `papillary_muscles`
(anterolateral first, posteromedial second); the moderator band. **The pack has no coronary ostium, no left atrial
appendage and no atrial-septum label.** Every rule below therefore uses the discs, the label volumes and the muscles. All
rules are **Designed** in the sense that they are stated here, not measured; the anatomical convention behind each is named.
Directions are projected onto the plane of the valve in question, about the valve's centre. Heart axes: +x patient's
left, +y cranial, +z anterior.

**Mitral**
* M-R1. The anterior leaflet's centre direction `u_A` is the projection of (aortic disc centre minus mitral disc centre).
  The anterior leaflet faces the aortic valve (Kaiser 2019, A.2.2 p. 32: the anterior leaflet's centre is where the ring
  lies next to the aorta). The anterior leaflet is therefore the leaflet whose centroid is nearest the aortic disc, as the
  brief says.
* M-R2. Handedness: let `s` = (mitral disc normal) x `u_A`. The anterolateral side is the side on which the anterolateral
  papillary muscle's base lies (the sign of `s` dotted with (muscle base minus mitral centre)); the posteromedial muscle
  must lie on the other side, else report the pack as unusual and fall back to +x = lateral.
* M-R3. Commissures at +75 and -75 degrees from `u_A` (M5), the first on the anterolateral side. Scallops P1, P2, P3 run
  from the anterolateral commissure to the posteromedial one along the posterior arc, widths M8.
* M-R4. Chordae: M22. The anterolateral muscle serves the half toward the anterolateral commissure.

**Aortic**
* A-R1. Cusp directions are three directions 120 degrees apart in the aortic disc plane. Choose the rotation so that
  commissure directions are symmetric about the mean of the two directions below.
* A-R2. The **non-coronary cusp** is the cusp whose centre direction is closest to the direction from the aortic centre
  to the **midpoint between the LA blood centroid and the RA blood centroid** (the atrial septum lies between them; the
  non-coronary cusp sits above it). If the LA and RA centroids give a degenerate direction, use the LA centroid alone.
  The commissure between the non-coronary and the left coronary cusp then points toward the mitral disc, which agrees with
  the aortic valve sitting above the anterior mitral leaflet (Kaiser 2019, A.2.2). Check this as a test: that commissure's
  direction is within 45 degrees of the direction to the mitral centre.
* A-R3. The other two cusps face the pulmonary valve and the right ventricular outflow. The **right coronary cusp** is
  the one nearer the patient's right (smaller x, after projection) and anterior; the **left coronary cusp** the other.
  Equivalently, the right coronary cusp's direction is the one closest to the direction toward the RV blood centroid.
* A-R4. Report which landmarks were used; there is no coronary ostium in the pack, so the names are an assignment by
  neighbours, not a check against the ostia. Cusp names follow Ishida 2026 (right, left, non-coronary); the non-coronary
  cusp has the longest geometric height there (15.9 mm), but the generator gives all three the same height (A2).

**Pulmonary**
* Pu-R1. The brief names the cusps anterior, left and right. Anatomical naming differs between sources: Lis 2023 (abstract)
  calls the sinuses left anterior, posterior and right anterior; other usage speaks of left-facing, right-facing and
  non-facing sinuses. **The correspondence was not verified.** The rule that does not depend on the names:
  the **anterior cusp** is the cusp whose centre direction points **away from the aortic disc** (largest angle to the
  direction toward the aortic centre); the other two are the **left** and **right** cusps, split by the sign of x after
  projection (patient's left = +x). Report the maintainer's naming question (see below).

## What could not be sourced

1. **Mitral scallop widths in mm and their indentation depths** (M8, M9): no open full text with numbers was found.
   Kunzelman 1994 and Ranganathan 1970 (the standard sources) have no numbers in their PubMed records.
2. **Mitral commissural leaflet height and the leaflet height profile** (M10 to M12): Designed.
3. **Human chordae counts and insertion bands** (M19, M21, M22): only the authors' own porcine dissection counts
   (Kaiser 2019) and a qualitative description were read.
4. **Mitral coaptation length**: about 10 mm (secondary) against 4.0 mm in `DesignedParams`; see C1.
5. **Systolic orifice area of the aortic and pulmonary valves, and of the open mitral valve**, as a fraction of the annulus
   (A14, and the mitral equivalent): no source read. The brief's acceptance test needs "a stated fraction with its source";
   the honest source is "Designed". The ASE/EACVI guideline's normal valve area ranges were not reached.
6. **The crown-shaped attachment line equations**: Haj-Ali 2012 (read later, see its section: no closed formula) and Rankin 2008 (abstract only)
   would give them; A8 is a smooth stand-in.
7. **Any pulmonary cusp dimension of its own** (P2 to P6): only root areas and sinus sizes (abstract only).
8. **Swanson and Clark 1974, Thubrikar, Schafers, Carpentier, the ASE/EACVI guidelines**: no open full text was reached.
9. **No coronary ostium, left atrial appendage or atrial-septum landmark in the pack**: aortic naming is by neighbours.
10. **Normal values from the Viva 2026 review** (gH about 19 to 20 mm, eH at least 9 to 10 mm, coaptation about 5 mm) are
    secondary and come from surgical measurements under tension; the CT cohort (Ishida 2026) is used instead.

## Conflicts and open points for the maintainer

* **C1. Mitral coaptation length.** `DesignedParams::av_coaptation_mm` = 4.0 mm (Designed); a review (secondary) quotes about
  10 mm. Not changed. If the echo's M-mode shows leaflets meeting too shallowly, this is the first number to revisit.
* **C2. Geometric height: CT or surgery.** `DesignedParams` uses CT (14.7 mm, Izawa 2021; Ishida 2026 gives 14.9 mm).
  Surgical values are 19 to 20 mm for a cusp under tension. No conflict with `DesignedParams`; the generator must not take
  its geometric height from the surgical number (see A4, A5).
* **C3. Pulmonary cusp naming** in the brief (anterior, left, right) against the naming in Lis 2023 (left anterior,
  posterior, right anterior). Needs a ruling before the labels are shown to a student (Pu-R1).
* **C4. Mitral leaflet thickness.** `av_leaflet_thickness_mm` = 1.6 mm (Sahasakul 1988, abstract only) against Kaiser's
  roughly 0.7 mm numerical thickness. Not a conflict; 1.6 mm stays.
* No row contradicts `mitral_anterior_length_mm`, `mitral_posterior_length_mm`, `aortic_effective_height_mm`,
  `pulmonary_effective_height_mm`, `cusp_thickness_mm`, `open_angle_from_axis_deg` or `papillary_tip_to_annulus_mm`.

## Parameters for the generator, in one place

| Name | Value |
|---|---|
| mitral anterior length, posterior length | 25.1 mm, 14.4 mm (DesignedParams) |
| anterior attachment angle, posterior | 150 degrees, 210 degrees; commissures at +75 and -75 degrees |
| scallop fractions P1 : P2 : P3 | 0.30 : 0.40 : 0.30 |
| indentation depth | 0.20 of the local height (never above 0.50) |
| commissural leaflet height | 7 mm |
| coaptation overlap, AV leaflets | 4.0 mm (DesignedParams) |
| chordae | 12 (per muscle: 2 anterior, 2 central posterior, 2 commissural); insertion band 0.25 of the local height |
| aortic and pulmonary effective height | 8.6 mm (DesignedParams) |
| cusp length (geometric height) | `sqrt(R^2 + eH^2)` |
| commissure height | 1.25 x geometric height |
| crown | `z(phi) = H_c * (1 - cos(3 phi)) / 2` |
| central coaptation length, semilunar | 3.3 mm |
| open orifice fraction of the disc area, semilunar | 0.75 (Designed) |
| thickness: AV leaflets, cusps | 1.6 mm, 1.0 mm (DesignedParams) |

## Values the generator added (B36 implementation), all Designed

The tables above leave some choices open. The generator (`heartsim_geometry::valve_design`) fixes them as below.
Each one is **Designed**. Nothing here was read in a source.

| Name | Value | Reason |
|---|---|---|
| Apposition: distance between the midsurfaces of two apposed leaflets or cusps in the coaptation zone | 0.8 mm | Half the drawn AV thickness (1.6 mm): the drawn leaflets overlap a little, and the free edges are under the brief's 1 mm gap. Used for the cusps too. |
| Smallest mitral overlap before a gap is reported | 0.5 mm | Where the two columns of one place on the coaptation line are too short for M13 (4.0 mm), the overlap is lowered to what they allow. The default pack uses down to 2.6 mm (posterior scallop dips against the long anterior chord). |
| Mitral chordae (after Ranganathan 1970, R10, R12) | per muscle: two to the anterior leaflet at 20° (free edge) and 45° (middle of the rough zone, the distal 0.37 of the height); one commissural chord to its commissure's free edge; one cleft chord to the free edge at the boundary between P2 and its side's scallop; two to P2 at 15° (free edge) and 30° (middle of the rough zone, distal 0.55) from the posterior leaflet's centre toward its side | Counts Designed; R12 asks for one cleft and one commissural chord per side, both are drawn. |
| Mitral free edges level when shut | the longer column of a pair puts its spare length into its billow (or a belly toward the ventricle) | So the free edges of a pair end at the same depth and stay 0.8 mm apart. The billow toward the atrium stays at most M15's 1 mm from the straight hinge-to-meeting line, and never above the hinge. |
| Mitral shut shape of a column | arc from the hinge to the meeting point (tangent angle linear in length), then the coaptation zone straight downstream | The simplest inextensible curve with a bounded billow; the echo's designed cone meets at one point the same way. |
| Mitral open shape | the straight column at M18 (10°) to the valve axis | As the designed cone. |
| Mitral motion | each column keeps its length; its tangent angle is blended from open to shut | No vertex leaves its column plane; a leaflet tip moves on an arc. |
| Where a chord leaves its muscle | 2.4 mm from the echo's designed muscle tip, toward its insertion | About the tip radius of the echo's designed muscle (8 mm base × 0.6 / 2). |
| Drawn chord radius | 0.4 mm | No chordal diameter was read. |
| Mitral open orifice | at least 0.5 of the disc area (0.75 on a circular test disc) | No source for the open mitral orifice against the annulus was read. |
| Semilunar apposed zone | 3.3 mm at the cusp centre rising to 5.9 mm at 30° from it (`3.3 + 2.6 sin(π |φ| / 60°)`, K5), tapering to nothing over the last 15° before each commissure | The profile follows K5 (narrowest at the centre, widest on each side); its shape between the two values and the taper are Designed. Scaled with the cusp heights where a height fit applies. |
| Semilunar columns | stop 1.5° short of each commissure | So no column has zero length. |
| Semilunar mesh | 31 columns × 8 rows per cusp, 1,440 triangles per valve | The echo's cost gate (brief B36 item 5): at 41 × 10 the slice through the pulmonary valve was 14 % slower than with the designed cone; at 31 × 8 it is 6 % slower. |
| Haj-Ali 2012 rows H3 and H4 | **not used** by the generator | Read after the generator was built. H4's free edge rises only 0.15 r from centre to commissure because its commissure top is at 0.5 r; with A7's commissure height (18.6 mm, kept) the shut free edge here runs straight from the effective height up to the commissure top. Using H4 would need a commissure post below the crown's top; using H3 (Eq. 6) would replace the straight belly of A4. Both are left for a later change. On the default pack the generated aortic geometric height (nadir to free-edge midpoint, straight) is 16.0 mm on a 27.3 mm disc, 0.59 of the diameter (H2's range 0.66 to 0.76; the CT value A2 gives 0.61). |
| Semilunar open shape | the free edge at the azimuth of its column, at a radius `R (1 − (1 − k) cos²(1.5 φ))` (`k` solved so the orifice is A14's 0.75 of the disc), as high as the column's shut length allows, never above the commissure height | A rounded-triangle orifice with the cusps toward the wall (A13). |
| Semilunar motion | each vertex on a straight line from open to shut | Each cusp then stays inside its own 120° wedge about the axis at every opening, so the cusps never cross. |
| Keyframes | 8 shapes; per column, the keyframes where its vertices have covered equal shares of their summed open-to-shut path | The brief's 25 % rule: on the default pack no vertex moves more than about a fifth of its path in one step. |
| Containment fit, opening | per column, the largest opening of 1, 0.95, … whose open shape is within 0.6 mm of the valve's blood; then the smallest over ±3 columns, averaged over ±3 columns | The brief: shorten the opening, do not edit the vessel. |
| Containment fit, semilunar attachment | each attachment point outside the blood moves toward the axis in 0.25 mm steps until inside (at most 8 mm) | The pack's vessels are tubes without sinuses, narrower than the disc in places (the aortic root by up to 2.75 mm). |
| Containment fit, semilunar heights | when the shut cusps still leave the blood, every height (effective, commissure, apposed zone) is lowered in 5 % steps, to at most 0.3 of the design | See the next paragraph. |

**The pulmonary trunk of the default pack was lengthened for this valve (maintainer's decision, 2026-11-26).** In the
pack as it was, the pulmonary artery lumen ended about 9 mm above the pulmonary disc (the Rodero trunk is cropped there),
and the designed pulmonary heights (geometric height 0.66 of the disc diameter, L1/L2: 17.7 mm on this 26.8 mm disc;
effective height by A4 11.6 mm; commissures 22.1 mm) did not fit: the generator had lowered them to 0.35 of the design.
The bake now continues the trunk 20 mm beyond its crop, in the label volume and in the drawn mesh
(`docs/anatomy-pack.md`, *Label volumes*; a designed straight tube). With it the pulmonary valve has its full designed
heights (effective 11.6 mm, commissures 22.1 mm). The trunk's lumen is narrower than the disc (mean inner radius
11.1 mm against 13.4 mm), so the cusps' attachment moves toward the axis by up to 6.75 mm on the way up. The aortic
valve kept its full heights throughout (its lumen is a tube of radius about 14 mm for 40 mm); its file changed slightly
with this bake, because the plug that used to close the pulmonary stub no longer cuts a sliver out of the aortic lumen
(aortic lumen 26.2 → 26.4 mL).

**The echo draws all four meshes.** The slice through the pulmonary valve ran 6 to 19 % slower than with the cones in
four criterion runs on the build server (gate: 10 %). Keeping the pulmonary cone in the echo (B35's option (b)) left that
slice about as slow (+10 %), so the extra cost is not the pulmonary mesh's and the cone was not kept;
`heartsim_echo::ECHO_CONE_VALVES` is the switch should a valve need it. The final run measured +2 % (see the B36 report).
