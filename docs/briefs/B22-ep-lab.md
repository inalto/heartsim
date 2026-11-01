# B22 — Cardiac EP lab (P2)
**Crates** `heartsim-sim`, `heartsim-ecg`, `heartsim-render`, `heartsim-ui` · **Model** Opus · **Milestone** M7 · **Depends on** B3, B5, B6, B13, B17.

## Goal
Catheters (HRA, HIS, CS decapolar, RVA, ablation) placed on landmarks or dragged along vessels, bipolar electrograms, programmed
stimulation from any electrode pair, AH/HV/CL/PPI measurements, entrainment, ERP determination, and RF ablation lesions (AVN, PVI,
CTI line, accessory pathway) with persistent effects.

## Read first
`docs/reverse-engineering/EPICARDIO-ANALYSIS.md` §1 "Cardiac EP mode", §4 items 9–11, §5 EGMs; `docs/REQUIREMENTS.md` F-16, F-33;
`docs/research/electrophysiology-and-ecg-models.md` §3.1 (local potentials).

## Scope
1. Catheter models: polyline bodies with electrode positions, constrained to chamber interiors (B6 lines + picking), presets.
2. EGMs: unipolar local potential at an electrode = forward-model row restricted to nearby cells (or −dV_m/dt of the nearest cells);
   bipolar = difference; display in a dedicated panel with sweep 100–200 mm/s.
3. PES: S1×n / S2 / S3 / continuous from a chosen pair; decremental protocols; automatic ERP search; AH/HV/CL/PPI calipers.
4. Ablation: lesions as spherical masks setting cells non-conducting with a time constant; lesion persistence; "ablate" button with
   impedance/temperature stub.
5. Scenarios: typical flutter with CTI ablation, AVNRT slow-pathway modification, AP localisation by CS activation sequence, AF PVI.

## Acceptance tests
Activation sequences on CS during sinus (proximal → distal) and during left-lateral AP AVRT (distal first); AH/HV in normal ranges
for sinus; CTI line terminates flutter in the Tier-2 test; lesion persistence across engine reset.
