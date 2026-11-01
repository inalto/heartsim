# B14 — Drugs, ischaemia by coronary branch, defibrillation
**Crates** `heartsim-sim`, `heartsim-ecg`, `heartsim-ui` · **Model** Sonnet · **Milestone** M3 · **Depends on** B2 (territories), B3, B5.

## Goal
Pharmacology and ischaemia as parameter-modifier sets with onset/offset dynamics, and a defibrillator with sync, energy and outcome
probability — all driven through `Action`s so they work in timelines and tutorials.

## Read first
`docs/research/electrophysiology-and-ecg-models.md` §3.5 (ST elevation/depression recipe, electrolyte/QT table), §2.5.1 (AV-node
effects), the private reference-product analysis notes (not published) §4 items 12–14, `crates/heartsim-core/src/scenario.rs`.

## Scope
1. `Drug` table (data file `assets/drugs.ron`): adenosine (AV ERP ↑↑ for 10 s), verapamil/diltiazem (AV CV ↓, ERP ↑), digoxin
   (AV ERP ↑, ST "scoop" template, automaticity ↑), flecainide (CV ↓ all myocardium, QRS widening), amiodarone (APD ↑, CV ↓
   slight), atropine (SA rate ↑, AV ERP ↓), adrenaline (SA rate ↑↑, automaticity ↑), lidocaine (ventricular ERP ↑), potassium
   hyper/hypo (AP template swaps + APD), with onset/half-life curves.
2. Ischaemia: `Action::Stenosis{branch, severity}` → cells whose territory weight × severity exceeds a threshold get `v_rest_offset`
   (injury current), APD ↓, CV ↓; infarction → scar (non-conducting core, border zone). Territories from B2 (LAD/LCx/RCA/PDA).
   Presets: anterior/inferior/lateral/posterior STEMI, subendocardial ischaemia (ST depression), pericarditis (diffuse).
3. Defibrillation: charge time, energy, sync-on-R (uses B4 QRS detection), shock resets all cells (Tier 1) or sets `u=v=rest`
   (Tier 2) with success probability per rhythm class; animation hook + sound event; "shock delivered during T" in unsynced mode
   can induce VF when Tier 2 is active (B17).
4. UI: drug tray with dose buttons and timers; ischaemia panel with coronary tree picker (highlight branch on the heart, B6);
   defibrillator panel.

## Acceptance tests
* Adenosine on AVNRT scenario terminates the loop within 10 s; verapamil slows AF ventricular rate by ≥ 20 %.
* Anterior stenosis 0.9 → ST elevation ≥ 0.2 mV in V2–V4 with reciprocal depression in III/aVF in the forward model; inferior →
  II/III/aVF elevation.
* Synced shock never lands within ±60 ms of a T-wave peak (test with B4 detection); shock during VT (Tier 1 stub) → sinus resumes.
