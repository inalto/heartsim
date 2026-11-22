# B30 — The beating heart: wall motion from the engine's own timing (echo simulator, phase 1)
**Crates** new `heartsim-echo` (pure), `heartsim-render`, `heartsim-app`, `heartsim-ui`, docs · **Model** Opus ·
**Milestone** M11 (echo simulator, phase 1 of 5) · **Depends on** B3 (Tier 1 cell state), B24 (beat assembler and valve
schedule). Requirement F-25.

## Goal
The 3-D heart moves. Each region contracts when it was activated and for as long as its action potential lasts; the valves'
closure times that already place the heart sounds also bound ejection and filling; so the mechanics, the ECG, the colour map and
the sounds can never disagree. A normal beat shows the base descending toward the apex, the walls thickening and the left
ventricle twisting; left bundle-branch block shows the septal flash; an early PVC does not eject; atrial fibrillation loses the
atrial kick and varies beat to beat; an ischaemic territory moves less. An LVEF read-out is computed from the deformed mesh.
This phase is useful on its own and is what every later echo phase (slice, ultrasound image, views, M-mode) samples.

## Read first
`docs/research/echo-simulator.md` §1.3, §2 (all of it; §2.3 is the model to build, §2.2 the numbers with their sources) and
§8–9; `docs/research/echo-simulator-notes/`; `docs/REQUIREMENTS.md` F-25 and the non-goals (no electromechanics);
`crates/heartsim-sim/src/engine.rs` (`CellState`, `Event`), `crates/heartsim-ecg/src/pcg.rs` (`BeatAssembler`, `Beat`,
`schedule`, `PcgParams`: `q_m1_ms`, QS2, `min_ejecting_rr_ms`), `crates/heartsim-sim/src/ischaemia.rs` (`Lesion::injury`),
`crates/heartsim-geometry/src/hood.rs`, `crates/heartsim-render/src/scalar_map.rs` (`map_hoods`),
`crates/heartsim-app/src/heart_view.rs` (how the colour map reaches the vertices, the 25 Hz history used for the sound's display
delay), `docs/anatomy-pack.md` (parts, tags, landmarks, the frame: mm, +x patient's left, +y cranial, +z anterior).

## Non-negotiables for this brief
* **A kinematic model, said so.** Prescribed motion in cardiac coordinates, not electromechanics. The page and the docs call it
  a model of wall motion; no pressure, no flow.
* **One source of truth.** Motion is a pure function of (event history, cell state, valve schedule, respiration, time). It
  never feeds back into the engines; rendering evaluates it at the time it draws (the same delayed time as the strip and the
  colour map while sound plays). Same scenario and seed give bit-identical displacements.
* **Sourced or designed.** Every parameter sits in one `MotionParams` with its source (the research note gives them) or the
  word "designed". The unsourced ones (electromechanical delay, phase durations of filling, respiratory shift) are designed and
  listed in the docs.
* **Engines and goldens untouched.** No change to `heartsim-sim` behaviour; every existing `.snap` stays byte-identical.
* Pure logic in the new pure crate (no wgpu/egui, no I/O, wasm32-clean); the GPU side only uploads positions and normals.

## Scope
1. **Crate `heartsim-echo`** (depends on `core`, `geometry`, `sim`, `ecg`; sits between `ecg` and `render` in the layering;
   add it to the workspace, to the CI wasm32 pure-crate check and to the lists of pure crates in the docs).
2. **Cardiac coordinates**, computed once from the pack's landmarks and tags: for the LV a long axis (apex to mitral-plane
   centre), a normalised height, a radial direction and a transmural depth; the same for the RV about its own axis; for each
   atrium a centre and the AV-plane normal. Stored per cell; tested for coverage and orthogonality.
3. **Per-cell contraction state** `c(t)` from `activation_ms`, an electromechanical delay and a fraction of `apd_ms` (rise,
   plateau, decay; §2.3), scaled by `1 − injury` in ischaemic cells.
4. **Phase gating** from the beat schedule: isovolumic contraction (M1 to aortic opening = M1 + IVCT), ejection (to A2),
   isovolumic relaxation (A2 + IVRT), early filling, diastasis, atrial contraction (only with organised atrial activity; on
   the atria's own clock in AV dissociation). During the isovolumic phases the cavity volume is held and regional differences
   in `c` only change the shape (early regions shorten, late regions stretch).
5. **Displacement field**: long-axis shortening growing from the apex to the annulus (to MAPSE / TAPSE), inward radial motion
   larger at the endocardium (thickening), rotation about the long axis varying from base to apex, each weighted by `c` and
   scaled so that the cavity volume follows the phase targets with the beat's ejection fraction. Atria: the complement of the
   ventricular filling plus their own contraction. Great vessels follow the annuli rigidly near the root and fade to rest.
   Respiration: a small caudal shift in inspiration from the engine's respiratory clock (designed).
6. **Beat-to-beat ejection fraction** from the preceding and pre-preceding RR (AF variation, the weak premature beat, the
   stronger post-ectopic beat; shape designed, anchored on the sources of §2.3); no ejection below `min_ejecting_rr_ms`;
   VF and asystole: no organised motion (a designed low-amplitude quiver in VF).
7. **Mapping to the meshes** through the existing hoods, for every part of the heart; normals recomputed; no triangle may
   invert. The valve-plane discs move with their annuli (leaflets are phase 2).
8. **Rendering.** Writable position and normal buffers in `heartsim-render`; the app evaluates the field once per drawn frame
   at the drawn time. Picking (stimulate, ablate) keeps working: either pick on the rest mesh with the inverse of the
   displacement or freeze motion while the pointer is down; say which.
9. **UI.** A "Wall motion" switch in the heart panel (on by default, off with reduced motion; persisted), an exaggeration
   slider (1× default) for teaching, and a read-out card: LVEF of the current beat, LV end-diastolic and end-systolic volume of
   the deformed mesh, MAPSE and TAPSE of the model, labelled "model". en + it.
10. **Docs.** User book: a short page on what moves and why, what is designed, and what the model cannot show; known-issues
    entry (nobody has compared it with a real echo; clinician review needed); `CHANGELOG.md`; `docs/ROADMAP.md`; the layering
    line in `docs/ARCHITECTURE.md`. `CLAUDE.md`'s layering line is the maintainer's to change: propose the edit in the report.

## Out of scope
Valve leaflets, papillary muscles, the probe, any ultrasound image (phases 2–3); pressure, flow, Doppler; the re-entry (Tier 2)
and EP-lab views unless they come for free; pathological chamber shapes (dilatation, hypertrophy).

## Acceptance tests (no GPU unless marked)
* Sinus rhythm at 60, 75 and 100 bpm: LVEF of the deformed closed LV shell 55–70 %; MAPSE 11–17 mm; TAPSE 17–28 mm; apical
  rotation −8° to −15° and basal +3° to +7° (sign as seen from the apex); wall thickening positive in every LV segment.
* Timing: the LV volume starts to fall within 5 ms of M1 + IVCT and stops falling within 5 ms of A2; it is constant (±1 %)
  in both isovolumic phases; filling shows E, diastasis and A at 60 bpm and fused E/A at 120 bpm.
* A ventricular beat with RR below `min_ejecting_rr_ms` does not change the LV volume by more than 2 %; the beat after a
  compensatory pause has a higher EF than the sinus beats.
* LBBB: the septum's peak inward motion precedes the lateral wall's, and a septal-flash index (inward then outward septal
  motion inside isovolumic contraction) is positive; normal conduction gives none. RBBB: RV free wall later than the septum.
* AF: no atrial contraction; the coefficient of variation of EF over 30 beats is above zero and EF rises with the preceding
  RR below 600 ms. Complete AV block: atrial contractions follow the P events, not the QRS.
* Ischaemia: an LAD lesion lowers the excursion of its AHA segments against the same segments without it, and leaves a remote
  segment within 10 %.
* VF and asystole: no cavity-volume change above 2 %.
* Determinism: same scenario and seed give bit-identical displacement at a fixed time; evaluation is independent of frame rate
  (two different sampling cadences agree at common times).
* Mesh safety: no inverted triangle and no non-finite vertex in any of the above; with the switch off the vertices equal the
  rest mesh bit for bit.
* UI (egui_kittest): the switch, the slider and the read-out; reduced motion switches the motion off; settings round-trip.
* A bench (not a gate) reports the per-frame cost natively; the report gives the number and an estimate for the web.
* `cargo xtask ci`, the wasm32 check of the pure crates including `heartsim-echo`, the wasm32 check of `heartsim-web`, and
  `cargo check -p heartsim-app --no-default-features --lib` pass; a GPU smoke test only where lavapipe is present.

## Decisions for the maintainer
* The new crate and the changed layering line in `CLAUDE.md`.
* Whether wall motion is on by default.
* Who reviews the look against real echocardiography before the docs call it more than a schematic.
