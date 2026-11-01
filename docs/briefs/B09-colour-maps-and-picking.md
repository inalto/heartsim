# B9 — Colour maps and picking in the app
**Crates** `heartsim-app`, `heartsim-ui`, `heartsim-render` · **Model** Sonnet · **Milestone** M2 · **Depends on** B3, B5, B6, B7.

## Goal
Wire the Tier-1 engine into the app: V_m / isochrone / APD maps on the heart through the hoods, adjustable voltage window, wavefront
outline, click-to-stimulate, synchronised scrubbing with the strip, anatomy layer panel, camera presets and auto-rotate.

## Read first
`crates/heartsim-app/src/app.rs`, `crates/heartsim-ui/src/viewport.rs`, B3/B5/B6/B7 outputs, `docs/ARCHITECTURE.md` §3 and §5,
`docs/reverse-engineering/EPICARDIO-ANALYSIS.md` §6 (hood colouring, iso/voltage maps).

## Scope
1. Hood mapping: `scalars[v] = Σ_k w_vk · vm[cell_vk]` per render mesh (rayon), uploaded each frame; isochrone mode uses activation
   times of the current beat normalised to the QRS window; APD mode static.
2. Legend/colour bar widget with draggable min/max (mV window), colour-blind-safe alternative LUT.
3. Click on the heart → `Action::Stimulate{Point}`; shift-click → `Block`/`Ablate` radius tool; hover tooltip shows region, APD,
   CV, activation time of the cell under the cursor.
4. Scrub: when paused, show V_m from the `SnapshotRing` at the strip cursor time; heart-vector arrow from B4.
5. Anatomy panel: layer toggles, transparency sliders, clip plane slider, labels toggle, presets AP/PA/LAO/RAO/cranial/caudal/left,
   LAO−90…RAO+90 slider, auto-rotate.
6. Engine selection: Tier 0 (strip only) vs Tier 1 (3-D) switch in the toolbar; scenario loading from `assets/scenarios`.

## Acceptance tests
* App unit test (no GPU): hood mapping of a known V_m field reproduces vertex values within 1e-5 for a delta-function cell.
* `egui_kittest`: anatomy panel toggles exist and flip layer visibility flags in the model.
* Manual (documented with screenshots in the PR): sinus beat shows atrial → ventricular colour wave, strip and heart agree during
  scrub, click on RV apex produces a PVC-shaped beat with the expected 3-D pattern.
