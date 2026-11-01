# B10 — Electrode editor on the torso
**Crates** `heartsim-ui`, `heartsim-app`, `heartsim-ecg` · **Model** Sonnet · **Milestone** M3 · **Depends on** B5, B6, B9.

## Goal
Let the learner drag the ten electrodes over the torso surface and immediately see the 12-lead change; provide presets and
misplacement exercises with automatic detection (limb reversals, V1/V2 too high, V5/V6 posterior), plus V3R/V4R/V7–V9 extras.

## Read first
`docs/research/electrophysiology-and-ecg-models.md` §3.6 (electrode reversal table), `docs/research/reference-apps.md` §6 P1-1,
`crates/heartsim-ecg/src/forward.rs` (B5 `move_electrode`), B6 picking.

## Scope
1. Drag electrodes constrained to the torso mesh (ray pick → surface point); snapping to anatomical landmarks (intercostal spaces).
2. Presets: standard, Mason–Likar, posterior leads, right-sided leads, paediatric; "fit leads" (reset).
3. Exercises: "RA/LA reversed", "V1/V2 one space too high", "limb leads on torso" with a detector that compares the derived leads
   against the standard placement (rules: lead I inverted + aVR positive → RA/LA swap, etc.) and explains.
4. Lead set editor: enable extra electrodes; the forward model adds rows; strip shows the extra leads.

## Acceptance tests
* Swapping RA/LA in the editor inverts lead I in the ring buffers (integration test with the Tier-0/Tier-1 engine).
* Detector unit tests for each reversal pattern in the table; false-positive rate 0 on 25 normal presets.
* Electrode positions persist in the scenario (`electrodes_mm`) and reload.
