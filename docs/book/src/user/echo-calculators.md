# Echo calculators

The **Tools** group of the top bar has one page, **Echo calculators**: 74 calculators for the quantitative methods of adult
transthoracic echocardiography and for the ultrasound physics a trainee is examined on, 16 of them guideline decision tools
(see below). Each one shows its inputs with units
and plausible ranges, the result as you type, the formula written out with your numbers in it, what the guideline that
defines the method says about the result (where it gives a grading), the reference with the table and page, and a worked
example of our own.

![Echo calculators: the aortic valve area filled from its worked example](../images/echo-calculators.png)

> **For education only.** This is a study aid, not a medical device. A finding says which published criterion a value meets
> ("in the range the source grades *severe*"), never what a patient has. Nothing you type is stored or sent: favourites and the
> last calculator you opened are remembered, the values are not. The gradings have not been reviewed by a clinician for this
> program; check them against the source before relying on them.

## Using the page

* **Search** (left, at the top) matches the name, other names and abbreviations, and the measurements a calculator asks for,
  in the interface language: "pressure half-time", "EROA", "TAPSE", "tempo di dimezzamento" all work. The list below it is
  grouped by category; favourites come first. Calculators in *italics* are formula only (see below).
* **Inputs** are in the order you acquire them, with the unit and the accepted range beside each. **Tab** moves between fields
  and **Enter** moves to the next number. A comma or a point both work as the decimal separator. Optional inputs are marked; a
  value outside the range, or a combination that cannot be (an end-systolic volume larger than the end-diastolic one), is
  explained beside its field and the result disappears until it is corrected: the page never shows a result that is out of date.
* **Load the worked example** fills physiologically plausible values; **Clear** empties the fields; **Favourite** keeps the
  calculator at the top of the list.
* **Heart rate from the running rhythm** appears only where the simulator really has the input: the cardiac output and shunt
  flow calculators take the ventricular rate of the rhythm running in the Simulator, measured from its last beats. The
  simulator has no echo images, so no echo measurement is ever taken from it.
* At phone width the list comes first; choose a calculator to open it, and **Back to the list** to return.

## Verified and formula only

Every calculator carries a badge:

* **Verified against the source**: the formula and every threshold the page shows were read in the primary publication (the
  guideline or the original paper), at the table and page given under *References*.
* **Formula only (no grading)**: the arithmetic is standard and was checked against a primary source or first principles, but
  no interpretation is shown, because its criteria were not read in a primary source (for example, the aortic diameter: the
  guideline defines dilatation on a nomogram whose limits are not printed as numbers).

Some gradings depend on the **sex** (ejection fraction, LV diameters, volumes and mass, by the 2015 chamber quantification
tables): choose it in the *Sex* field. Without it the page says that the grading needs it; with it, the same value can be
normal for a man and mildly abnormal for a woman (an ejection fraction of 53 %, for example).

## Decision tools

Sixteen calculators follow a published algorithm step by step instead of computing one formula: diastolic function and left
atrial pressure in sinus rhythm and in each special population of the 2025 ASE recommendation (atrial fibrillation, mitral
annular calcification, mitral stenosis, mitral regurgitation, hypertrophic and restrictive cardiomyopathy, sinus tachycardia,
pulmonary hypertension, heart transplant, LV assist device, diastolic exercise echocardiography), the stepwise grading of aortic
stenosis with the low-flow pathway, dobutamine and calcium score (2017), and the screening of aortic and mitral prostheses
and the comparison of an aortic bioprosthesis with its post-implant baseline (2024).

Their result is shown as **Path through the algorithm**: each step is a question of the figure or table, with the answer your
measurements give, the values behind it and the figure and page; the last line is the result.

![The atrial fibrillation algorithm filled from its worked example: two of the four first criteria, then two of the three second ones, elevated LAP](../images/echo-calculators-decision.png)

* **Any field may be left empty.** The tool checks every value an empty field could have. If none of them could change the
  result, the result is given and the page says that the empty fields did not matter. If they could, the result is
  **indeterminate** and lists the results that are still possible: measure what the path stopped at.
* **Indeterminate** also appears where the source itself says so (for example the "indeterminate LAP" of the atrial
  fibrillation and pulmonary hypertension algorithms, or a TR jet that is absent in a transplant recipient), and where the
  measurements put the case on a branch the figure does not draw.
* The lists of Table 7 (mitral stenosis and regurgitation, HCM, restrictive cardiomyopathy, tachycardia, LVAD) give indicators of
  *elevated* pressure only: these tools say whether at least one is met; meeting none is not a criterion for normal pressure.
* Where a figure and the text of the same guideline disagree on a limit, the figure is followed; the differences are listed in
  `docs/research/echo-calculators.md`.

## The catalogue

| Category | Calculators (✓ verified, ◦ formula only) |
| --- | --- |
| Stenosis | ✓ aortic valve area by continuity, with indexed area, velocity ratio, peak and mean gradient; ✓ aortic stenosis stepwise grading and low-flow pathway (decision tool); ✓ mitral valve area by pressure half-time; ✓ mitral valve area by continuity; ◦ mitral valve area by PISA with the funnel-angle correction; ✓ pulmonic peak gradient; ✓ tricuspid stenosis criteria |
| Regurgitation | ✓ mitral, aortic and tricuspid regurgitation by PISA (EROA, volume, fraction; tricuspid vena contracta); ✓ mitral, aortic and pulmonic regurgitation by the volumetric method |
| Filling pressures | ✓ E/e' (average, septal, lateral, wedge-pressure regression); ◦ left atrial pressure from the MR jet; ◦ LV systolic and end-diastolic pressure from the MR and AR jets; ✓ left atrial volume index; decision tools (all ✓): diastolic function in sinus rhythm, atrial fibrillation, mitral annular calcification, pulmonary hypertension, heart transplant, diastolic exercise echo, mitral stenosis, mitral regurgitation, HCM, restrictive cardiomyopathy, sinus tachycardia, LVAD |
| Left ventricle and aorta | ◦ fractional shortening; ✓ indexed LV diameters (by sex); ✓ LV mass (linear method) with index, relative wall thickness and geometry (by sex); ✓ indexed LV volumes (by sex); ✓ ejection fraction (by sex); ✓ relative change of GLS; ✓ LVOT stroke volume, cardiac output and indices; ◦ indexed aortic diameter; ◦ wall-motion score index (16 or 17 segments) |
| Right heart and pulmonary circulation | ✓ right atrial pressure from the IVC; ✓ RV systolic and pulmonary systolic pressure; ✓ pulmonary diastolic and mean pressure from the PR jet; ✓ mean pressure from the RVOT acceleration time; ✓ pulmonary vascular resistance (Doppler); ✓ RV systolic function (TAPSE, S', FAC, 3D EF, free-wall strain); ✓ TAPSE/PASP coupling; ✓ RV myocardial performance index; ✓ right atrial size; ✓ right ventricular size; ◦ body surface area (Mosteller, Du Bois, Haycock) |
| Pericardium | ✓ effusion size; ✓ mitral and ✓ tricuspid inflow respiratory variation; ◦ LVOT respiratory variation; ✓ hepatic-vein expiratory reversal ratio; ◦ septal-to-lateral e' ratio |
| Shunts and congenital | ◦ Qp/Qs; ◦ left-to-right shunt flow; ◦ RV pressure from a VSD jet; ◦ PA pressure from a PDA jet; ◦ coarctation gradient with the proximal velocity; ◦ mean gradient from a traced velocity |
| Prosthetic valves | ◦ aortic and ◦ mitral prosthesis EOA, Doppler velocity index and indexed EOA; decision tools (all ✓): aortic prosthesis stenosis and mismatch screening, aortic bioprosthesis change from baseline, mitral prosthesis stenosis, MR and mismatch screening |
| Ultrasound physics | ◦ period, wavelength and pulse duration; ◦ pulse repetition, Nyquist limit and duty factor; ◦ spatial pulse length, axial resolution and near field; ◦ the Doppler equation both ways, with the angle error; ◦ acoustic impedance; ◦ intensity reflection coefficient; ◦ acoustic intensity; ◦ Poiseuille flow and resistance |

The publication, table and page behind each calculator are listed on its page and in `docs/research/echo-calculators.md`.

## On the command line

The same calculators run headless (see [Command line](cli.md)):

```bash
heartsim-cli calc list                       # every calculator with its id and status
heartsim-cli calc aortic-valve-area lvot_d_cm=2.0 lvot_vti_cm=20 av_vti_cm=80
heartsim-cli calc rvsp-pasp --example --json  # the worked example, as JSON
heartsim-cli calc la-volume-index la_volume_ml=68 bsa_m2=2 --lang it
heartsim-cli calc diastolic-mac e_cm_s=110 a_cm_s=90 ivrt_ms=68   # a decision tool: path, then result
```

The output is the same as the page's: values, the worked formula, the findings with criterion and source. A missing or
invalid input exits with an error that names the field.
