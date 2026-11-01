# B17 — Re-entry presets and tier switching
**Crates** `heartsim-sim`, `heartsim-app`, `assets/scenarios` · **Model** Sonnet · **Milestone** M4 · **Depends on** B16, B14.

## Goal
Scenarios for atrial fibrillation, typical/atypical flutter, scar-related monomorphic VT, polymorphic VT/torsades, VF, pre-excited
AF; automatic Tier 1 ↔ Tier 2 switching; defibrillation/ablation/drug effects on them.

## Read first
B16 engine API, `docs/research/electrophysiology-and-ecg-models.md` §3.5 (fibrillatory baseline, flutter waves), §4.3,
the private pathology notes (not published) (which knobs the reference touches for AF/AFL/VT/VF — concepts only).

## Scope
1. Scenario fields: `tier: ReactionDiffusion`, initiation protocol (S1–S2 site/timing, or pre-seeded spiral state file), tissue
   patches (short-APD pulmonary-vein sleeves for AF, CTI obstacle for flutter, scar + border zone for VT, long-APD + EAD-like
   parameters for torsades).
2. AV node filtering of AF: the timing engine receives atrial activations sensed at the AV-node region from Tier 2 and applies its
   recovery curve → irregular ventricular response (compare RR irregularity statistics to clinical ranges).
3. Switching: when a scenario or action requires re-entry, snapshot Tier 1 → Tier 2; when the PS count is 0 for 2 s, switch back.
4. Interventions: PVI ablation lines isolate the sleeves (AF terminates or cannot be re-induced), CTI line stops flutter, synced
   cardioversion vs unsynced shock-on-T induces VF, amiodarone/flecainide effects on AF persistence.
5. ECG checks: AF → no P, irregular RR, f-waves 0.05–0.2 mV; flutter → saw-tooth at 250–350/min with 2:1/4:1 response; VT →
   wide complex 150–220 bpm; VF → chaotic 4–8 Hz.

## Acceptance tests
Integration tests (reduced grid 32³ for speed) asserting the ECG checks above through B4 analysis; PVI test; switching hysteresis
test; determinism.
