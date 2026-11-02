# Tier-0 rhythm library: diagnostic criteria and parameter derivation

Brief B11. Every preset in `assets/scenarios/*.ron` carries an `expected` block (`hr_bpm`, `pr_ms`, `qrs_ms`, `axis_deg`)
that the B4 analyser must reproduce on the last 10 s of a 20 s run (`crates/heartsim-ecg/tests/golden.rs`,
`heartsim-cli validate-scenarios`). The ranges are the textbook diagnostic criteria of the rhythm, narrowed only where the
rhythm itself fixes a value (e.g. a programmed pacing rate). `None` means "not a criterion of this rhythm" — PR in AV
dissociation, QRS duration or axis when normal and ectopic beats alternate.

The engine parameters are re-derived from the same sources plus the timing-model references already used by B1
(`electrophysiology-and-ecg-models.md` §1 and §2.5); nothing is taken from any other product.

## Criteria used

| quantity | value | source |
|---|---|---|
| Normal sinus rate | 60–100 bpm (bradycardia < 60, guideline < 50; tachycardia > 100) | Goldberger 2017 ch. 13; Kusumoto 2018 §2 |
| Normal PR | 120–200 ms; 1st-degree AV block > 200 ms | Mason 2007; Kusumoto 2018 |
| Normal QRS | < 110 ms (adult, Surawicz uses ≥ 110 ms for incomplete block) | Surawicz 2009 |
| Normal frontal axis | −30° to +90° | Mason 2007 |
| RBBB / LBBB | QRS ≥ 120 ms; RBBB rsR′ V1, wide S in I/V6; LBBB broad notched R I/aVL/V5–V6, no septal q, QS/rS V1 | Surawicz 2009 |
| LAFB | axis −45° to −90°, qR aVL, rS II/III/aVF, QRS < 120 ms | Surawicz 2009 |
| LPFB | axis +90° to +180°, rS I/aVL, qR III/aVF, QRS < 120 ms | Surawicz 2009 |
| Pre-excitation | PR < 120 ms, delta wave, QRS > 110–120 ms | Surawicz 2009; Page 2015 |
| Junctional escape | 40–60 bpm (accelerated 60–100) | Goldberger 2017; LITFL |
| Ventricular escape / idioventricular | 20–40 bpm, QRS ≥ 120 ms | Goldberger 2017; LITFL |
| AIVR | 50–110 bpm, QRS ≥ 120 ms | LITFL |
| Sinus pause "significant" | ≥ 3 s | Kusumoto 2018 |
| Typical AVNRT | 120–220 bpm, RP < 70 ms (P in / just after QRS) | Page 2015 |
| Orthodromic AVRT | 150–250 bpm, RP > 70 ms | Page 2015 |
| Pacing modes | NBG code (VVI, DDD), timing cycles | Bernstein 2002 |
| QT rate adaptation | QT ∝ RR^⅓ (Fridericia) — synthesiser T timing in scenario runs | Fridericia 1920 |

## How each rhythm is produced

The timing engine (B1) decides *which* structures fire and when; the Tier-0 synthesiser picks the beat morphology from
the engine's events (`heartsim_ecg::synth::Template`):

* atrial events — sinus `Automatic` → sinus P; focus `Automatic` → ectopic P; `Conducted` (retrograde over node or
  accessory pathway) → retrograde P (superior axis); `Stimulus` → spike + P;
* ventricular events — His–Purkinje `Conducted` → normal QRS–T, or the block pattern given by the bundle-branch
  `Blocked` events of the same beat (RBB → RBBB, LBB or LAF+LPF → LBBB, LAF/LPF → QRS rotated by −120°/+60°, which puts
  the net axis at −60°/+120°); accessory-pathway `Conducted` → delta wave (left/right by ventricular insertion);
  `Automatic` (focus, ventricular escape) → LV or RV ectopic complex; `Stimulus` → spike + ectopic complex.

Bundle-branch morphologies follow the activation sequences of Durrer 1970: RBBB keeps the normal initial forces and adds
a slow terminal right-anterior force (R′ in V1); LBBB loses the septal q (septum activated right-to-left) and has slow
leftward-posterior forces with discordant T. Ectopic complexes point away from their origin (LV focus → RBBB-like,
RV apex → LBBB-like with superior axis), as described in `electrophysiology-and-ecg-models.md` §2.5.3.

Rhythm-specific parameter choices (all in the RON files, with the reasoning in each file's `notes`):

* **Escape rates are lowered** where a subsidiary pacemaker would otherwise interrupt the teaching point (sinus brady
  45 with junction at 38; SA exit block pause of 2 × 800 ms with junction at 35).
* **Mobitz I** uses the B1 Wenckebach parameter set (fast pathway min 110 ms, A = 200 ms, τ = 150 ms, ERP 300 ms at an
  atrial rate of 110 bpm), Shrier 1987.
* **3:1 Mobitz II** uses the new `AvParams::mobitz2_conducted` (one of three beats conducted below the node) with RBBB,
  since infranodal block usually coexists with bundle-branch disease (Kusumoto 2018); junctional escape is off because
  the block is below the junction.
* **PVC bigeminy** couples the PVC late enough (600 ms at 72 bpm) that its retrograde concealed conduction leaves the
  AV node refractory for the next sinus P — the full compensatory pause.
* **AVNRT 150**: rate = retrograde fast-pathway time (50 ms) + atrial turnaround (50 ms) + slow-pathway AVCT
  (min 218 ms + 100 ms · e^−0.2 ≈ 300 ms) ≈ 400 ms. Started by one PAC 300 ms after the 4th sinus P.
* **Orthodromic AVRT** uses a concealed pathway (retrograde only) with a 70 ms VA time (RP > 70 ms) and ERP 250 ms,
  started by a PAC 280 ms after the 4th sinus P; the rate (≈ 200 bpm) follows from the circuit.
* **Paced rhythms** run in complete AV block so every ventricular beat is paced (VVI: P dissociated; DDD: P-tracked
  with a 150 ms AV delay).

## References

* Bernstein AD, Daubert J-C, Fletcher RD et al. The revised NASPE/BPEG generic code for antibradycardia, adaptive-rate,
  and multisite pacing. *PACE* 2002;25:260–264. https://doi.org/10.1046/j.1460-9592.2002.00260.x
* Burns E, Buttner R. *Life in the Fast Lane ECG Library.* https://litfl.com/ecg-library/
* Durrer D, van Dam RT, Freud GE et al. Total excitation of the isolated human heart. *Circulation* 1970;41:899–912.
* Fridericia LS. Die Systolendauer im Elektrokardiogramm bei normalen Menschen und bei Herzkranken. *Acta Med Scand*
  1920;53:469–486.
* Goldberger AL, Goldberger ZD, Shvilkin A. *Goldberger's Clinical Electrocardiography: A Simplified Approach*, 9th ed.
  Elsevier 2017.
* Kusumoto FM, Schoenfeld MH, Barrett C et al. 2018 ACC/AHA/HRS guideline on the evaluation and management of patients
  with bradycardia and cardiac conduction delay. *Circulation* 2019;140:e382–e482.
* Mason JW, Hancock EW, Gettes LS. Recommendations for the standardization and interpretation of the
  electrocardiogram, part II: electrocardiography diagnostic statement list. *Circulation* 2007;115:1325–1332.
* Page RL, Joglar JA, Caldwell MA et al. 2015 ACC/AHA/HRS guideline for the management of adult patients with
  supraventricular tachycardia. *Circulation* 2016;133:e506–e574.
* Shrier A, Dubarsky H, Rosengarten M et al. Prediction of complex atrioventricular conduction rhythms in humans with use
  of the atrioventricular nodal recovery curve. *Circulation* 1987;76:1196–1205.
* Surawicz B, Childers R, Deal BJ, Gettes LS. AHA/ACCF/HRS recommendations for the standardization and interpretation of
  the electrocardiogram, part III: intraventricular conduction disturbances. *Circulation* 2009;119:e235–e240.
