# B31 — Probe and slice: a label volume, designed valve parts, a probe on the figure, the anatomical cross-section (echo simulator, phase 2)
**Crates** `xtask` (bake), `heartsim-geometry`, `heartsim-echo`, `heartsim-render`, `heartsim-ui`, `heartsim-app`, assets, docs ·
**Model** Opus for part A, Sonnet for part B, Haiku for the docs · **Milestone** M11 (echo simulator, phase 2 of 5) ·
**Depends on** B30 (the motion model in `heartsim-echo`), B2 (anatomy bake), B10 (picking on the figure).

Two parts for two implementers one after the other. **Part A** is the data and the pure logic: the baked label volumes, the
designed parts, the probe geometry and the function that turns (engine state, probe pose, time) into a label image. **Part B**
is the page: the probe on the figure, the sector in the 3-D view and the cross-section on screen.

## Goal
A new page, **Echo**, where the learner puts a transducer on the chest of the figure, slides, tilts, rocks and rotates it,
and sees the plane it cuts through the beating heart: on the left the figure and the 3-D heart with the sector drawn in it,
on the right the cross-section inside the sector, in flat colours by tissue (myocardium of each chamber, blood of each
chamber, vessel wall and lumen, valve leaflets, papillary muscles, bone, lung, liver, other soft tissue). The cross-section
moves with the beat because it samples the B30 motion model; the leaflets open and close on the valve schedule. Ribs and
lung hide what lies behind them, so finding a window already matters. There is **no ultrasound appearance yet** (no speckle,
no attenuation, no grey image): that is phase 3, and it will reuse this phase's sampling unchanged. The page says it is an
anatomical cross-section.

## Read first
`CLAUDE.md`; `docs/research/echo-simulator.md` §1.1, §1.2, §1.4, §2.4, §3.3, §3.4 (the bake and steps 1–3 of the per-frame
pipeline; "Cheap fallback" is this phase's image), §4.1–4.2, §8 (row E2), §9; `docs/research/echo-simulator-notes/`;
`docs/briefs/B30-beating-heart.md` and `crates/heartsim-echo/src/lib.rs` (API, the Cost section);
`docs/anatomy-pack.md`; `docs/research/anatomy-meshes.md` (the KCL tag list and its warning); `xtask/src/bake/` (`torso.rs`,
`heart.rs`, `fetch.rs`: how sources are fetched, verified and baked deterministically); `crates/heartsim-geometry/src/pack/`
(`body.rs`, the loader, the manifest with sizes and CRCs); `crates/heartsim-app/src/electrode_tab.rs` and
`crates/heartsim-ui/src/electrode_editor.rs` (picking and dragging on the figure); `crates/heartsim-render/src/scene.rs`,
`shaders/`, `tests/webgl2_shaders.rs`; `crates/heartsim-app/src/nav.rs` and its tests; `docs/web.md` (WebGL2 rules, the
25 MiB budget); `docs/book/src/known-issues.md`.

## Non-negotiables for this brief
* **Legal.** Only the sources already cleared ship: the Rodero 2021 average heart and the KCL whole-torso model 1 (both
  CC BY 4.0). Nothing from `epicardio/`, `epicardio-extracted/` or any echo teaching product: no images, view pictures, texts
  or parameter tables. No echo dataset is used, not even for tuning (the research note's §7 lists why each is excluded).
* **KCL tags are verified, not trusted.** The information sheet's tag numbers are off by one around the vessels. Every tag
  used (bone, lungs, liver, blood pools) is checked in the bake by volume and position against stated expectations, and the
  bake fails loudly when a check fails, as the vessel bake does.
* **Designed means designed.** Leaflets, papillary muscles, the moderator band and the pericardium do not exist in the data.
  Each dimension and each timing sits in one parameter struct with its primary source and locator, or the word "designed".
  The research note found **no** source for leaflet lengths or opening and closing durations: look for one (the ASE
  chamber-quantification and valve guidelines, anatomical series) and record what was read; what stays unsourced is listed in
  the docs. Nothing from memory or a secondary website is labelled as sourced.
* **Pure and deterministic.** Volume type, sampling, designed parts, probe geometry and the label image live in pure crates
  (no wgpu/egui, no I/O, wasm32-clean). The label image is a pure function of (motion history, cell state, probe pose, time,
  respiration): same scenario, seed and pose give a byte-identical image. Rendering only uploads and scan-converts it.
* **One source of truth.** The slice samples the same motion evaluation as the 3-D heart at the same drawn time; it never
  feeds back into the engines. Engines and every existing golden stay byte-identical.
* **Bake is reproducible.** `cargo xtask bake-anatomy` regenerates the new files bit for bit from the fetched sources; the
  manifest carries size and CRC-32 for each; the pack `format_version` goes from 2 to 3 and a version-2 pack is refused with
  a clear message (or loaded without the echo page, say which).
* **Web.** The new shader uses no compute and no storage buffers and passes `webgl2_shaders.rs`; `cargo xtask web-budget`
  still passes.

## Part A — data and pure logic (Opus)
1. **Label-volume type** (`heartsim-geometry`). A run-length-encoded `u8` volume (`HSVL` v1: magic, version, origin, spacing,
   dimensions, runs) with a loader, a CRC check, nearest lookup and bounds handling. A label enum with stable numeric values,
   documented in `docs/anatomy-pack.md`.
2. **Heart box** (bake). About 140 mm cubed at 1 mm in the pack frame: myocardium of LV, RV, LA, RA; blood of LV, RV, LA, RA;
   wall and lumen of the aorta and the pulmonary artery; the venous lumens where the pack has them. Blood is a flood fill
   inside the closed shells and the valve discs; vessels are capped where the dataset crops them. The bake measures and
   prints the LV wall thickness by AHA level and fails outside 5–13 mm.
3. **Thorax box** (bake). About 300 × 450 × 240 mm at 2 mm from the KCL model in the same frame transform as the skin: bone
   (ribs, sternum, spine as one label), lungs, liver, other soft tissue, outside. Overlaps resolve by a stated priority
   (pack heart > lungs > KCL heart; the KCL subject's own heart region that the pack heart does not cover becomes soft tissue
   or blood by a stated rule). Report the worst gap between the two hearts' outlines.
4. **Designed parts** (`heartsim-echo`), evaluated analytically in rest coordinates and overriding the label:
   * **Leaflets**: mitral (anterior, posterior), tricuspid, aortic and pulmonary cusps as thin surfaces hinged on the rims of
     the B30 valve discs, with a thickness of at least one heart voxel so they are visible. Their opening follows the phase
     gating B30 already has (rapid opening, held open, rapid closure; the mitral valve also half-closes in diastasis and
     reopens with the atrial contraction when there is one). No ejection means the aortic valve stays shut.
   * **Papillary muscles**: two bodies on the LV wall at the anterolateral and posteromedial positions at mid level (from the
     AHA segments), moving with the wall. No chordae.
   * **Moderator band**: one band in the RV apex third.
   * **Pericardium**: a shell of 1–2 mm outside the epicardium.
   Valve lesions (`LesionKind`) do **not** change the leaflets in this phase.
5. **Displacement for arbitrary points.** B30 moves mesh vertices; a slice needs the motion at any point, cavities included.
   Evaluate the model on a coarse rest-space grid over the heart box (about 3 mm), extended smoothly into the cavities and
   fading to zero in the thorax, and map a sample point x to its rest position by fixed-point iteration `X ← x − U(X)`
   (2–3 steps). **Cost is the main risk**: one B30 evaluation is about 18 ms on the development VM. The grid must be filled
   from the evaluation the 3-D view already made for that drawn time (one evaluation per drawn frame for both, not two), or
   from the model's cell-level state directly; say which and measure it.
6. **Probe geometry.** `ProbePose { skin point, tilt, rock, rotation, depth_mm, sector_deg }` on the figure's skin: local
   frame from the skin normal and the head-to-foot tangent; tilt about the array axis, rock in the image plane, rotation about
   the beam axis (the index marker); contact is lost when the beam leaves about 60° of the inward normal (designed), and the
   image is then empty. Ranges are clamped, never NaN. Named starting poses for the four windows (parasternal, apical,
   subcostal, suprasternal) are computed from the pack landmarks and the baked ribs, not typed in by hand. The patient is
   **supine** in this phase; left lateral decubitus is a later decision.
7. **Label image.** `slice(&model, &volumes, &MotionInput, &ProbePose, &mut LabelImage)`: a polar image (lines × samples; default
   128 × 512, both parameters), each sample the label at the rest position of its point (heart box, else thorax box moved only
   by respiration, else outside), with the designed parts on top. A per-line **occlusion** flag from the first bone or lung
   sample on the line: samples behind it are marked hidden. This is geometric, not acoustic, and the page says so.
8. **Bench** (not a gate): the slice alone and slice plus motion, natively; an estimate for the web.

## Part B — the page (Sonnet)
9. **Navigation.** A page **Echo** (it: *Eco*) in the Explore group, after Heart Vector; fifteen pages; the tab bar, the
   group menus and the phone layout still fit in both languages; no existing shortcut changes.
10. **Probe on the figure.** Left pane: the figure and the heart, torso translucent, with a transducer glyph on the skin.
    Drag on the skin slides it (the Electrodes page's pick, `body_region` trunk only); tilt, rock and rotation by drag
    handles on a gizmo and by sliders with degrees; the index marker shown on the glyph and as a clock position; depth and
    sector width as sliders; four buttons for the windows' starting poses; a reset. Keyboard: arrows slide, modified arrows
    tilt and rock, `[` and `]` rotate. The pose is remembered for the session, not persisted.
11. **Sector in 3-D.** The sector drawn as a translucent quad in the heart with its marker edge, using the existing layer
    pipeline; a switch to clip the heart at the image plane (the existing clip plane) so the cut matches the slice.
12. **Cross-section.** Right pane: the label image scan-converted by a fragment shader (apex at the top, marker side at the
    screen's right, depth scale in cm, sector mask), colours from a small table that works in both themes and in the
    colour-blind-safe palette, hidden samples dimmed with a hatch, a legend that names each tissue, and a hover read-out
    (tissue name, depth). "No contact" is shown as text over an empty sector. The page carries the line "anatomical
    cross-section, not an ultrasound image" and the educational disclaimer.
13. **Timing and controls.** The page shares the rhythm bar; the slice is drawn at the same delayed time as the strip and the
    3-D heart; pause and scrub work; Wall motion off (or reduced motion) gives the rest anatomy with leaflets closed or open
    by phase only. One strip lead is shown under the panes so the beat can be related to the ECG.
14. **Performance policy.** The slice follows the B30 rate cap (30 Hz native, 20 Hz web) and does no work when neither pose nor
    time changed. If the measured cost on the web is too high, halve the samples there and say so.
15. **Docs** (Haiku, after part B). User-book page *Echo: probe and cross-section* (the probe movements with their names, the
    windows, what the colours mean, what is designed, what is missing); `known-issues.md`; `docs/anatomy-pack.md` (format 3,
    the two volumes, the label table); `CHANGELOG.md`; `docs/ROADMAP.md`; a requirement line in `docs/REQUIREMENTS.md`;
    README feature list; a screenshot in `docs/images/` and five tour steps.

## Out of scope
Any ultrasound appearance (speckle, attenuation, shadows as acoustics, gain, TGC, grey maps: phase 3). The named standard
views, targets, the "view found" score, tutorial and self-test (phase 4). M-mode, tissue Doppler, calipers (phase 5). Blood
Doppler of any kind. Lesion-specific leaflets, dilated or hypertrophied chambers, effusion. Left lateral decubitus. Chordae,
trabeculae, coronary arteries. Hardware probe input. A transoesophageal probe.

## Acceptance tests (no GPU unless marked)
* Volume: encode/decode round-trip; CRC mismatch is refused; lookup outside the box returns "outside"; the baked files match
  the manifest's sizes and CRCs; a second bake is byte-identical (xtask test, skipped when the sources are not on disk, as the
  existing bake tests are).
* Heart box: the LV blood volume equals the closed shell's enclosed volume (`lv_cavity_volume_ml` at rest) within 5 %; every
  cell of the cell cloud lies in a myocardium voxel of its own chamber (allow 2 % at boundaries); no blood voxel touches
  "outside" (the fill did not leak).
* Thorax box: a ray from each of V1–V6's skin points straight toward the heart centroid crosses bone for at least one of them
  and none for at least one of them (so ribs and spaces both exist); a ray from the subxiphoid point crosses liver before
  heart; the sternum is on the midline anterior to the heart; lung volume left and right each between 0.8 and 3.5 L.
* Designed parts: each leaflet lies within its valve's disc radius plus its length; closed leaflets separate the two blood
  labels on a line through the valve centre, open leaflets do not; the mitral valve is open from the end of isovolumic
  relaxation to M1 and closed through ejection; the aortic valve is open only during ejection and never in a beat that does
  not eject; in AF the mitral valve has no atrial reopening; both papillary muscles are inside the LV cavity touching the
  wall at every time step of a sinus beat.
* Displacement inverse: for random points on the moving LV wall, `X` recovered by the iteration, pushed forward again, lands
  within 0.3 mm; with motion off, the slice equals the rest volume sampled directly.
* Probe: the pose frame is orthonormal for any angles; rotation by 360° returns the same plane; contact loss beyond the limit;
  each window's starting pose has contact and its central line reaches a heart label within the default depth.
* Slice: from the apical starting pose, some rotation within ±90° gives an image containing all four blood labels (found by
  search in the test); a short-axis plane through the papillary level shows LV blood enclosed by LV myocardium with both
  papillary bodies; the LV blood area in a fixed plane is smaller at end-systole than at end-diastole; a pose over a rib has
  more hidden samples than the pose one interspace away.
* Determinism: the label image is byte-identical for the same scenario, seed, pose and time, and independent of the sampling
  cadence.
* Page (egui_kittest): the page is in the nav (fifteen pages, four groups, narrow layout without overflow, en and it); the
  sliders and window buttons change the pose; "no contact" appears beyond the limit; the legend lists every label present;
  the pose is not in the saved settings; reduced motion freezes the wall.
* Shader: `webgl2_shaders.rs` covers the scan-conversion shader; a GPU smoke test (lavapipe only) renders the sector and
  checks that the apex pixel column is inside the mask and a corner pixel is outside.
* Budget: `cargo xtask web-budget` passes; the report gives the two volumes' sizes on disk and deflated.
* Bench (not a gate): slice cost and slice-plus-motion cost natively, with the web estimate.
* `cargo xtask ci`, the wasm32 check of the pure crates, the wasm32 check of `heartsim-web`, and
  `cargo check -p heartsim-app --no-default-features --lib` pass; every existing golden is unchanged.

## Report
Per part: what was built, the measured numbers (volumes' sizes, wall thickness, blood-volume agreement, heart-outline gap,
bench), every designed value with what was searched and not found, the KCL tags used with their verified volumes, screenshots
of the four windows at end-diastole and end-systole, and what was not verified.

## Decisions for the maintainer
* **A new page or a mode of the Simulator.** The brief assumes a new page *Echo* in Explore (fifteen tabs; the bar is already
  wide in Italian).
* **Thorax source.** The brief assumes KCL torso 1 (same frame, already licensed, one bone label). TotalSegmentator would
  give separate ribs and cartilages but is another subject and needs registration.
* **Patient position.** Supine here. Whether to add a designed left-lateral shift of the heart later (no numeric source was
  found).
* **Designed valve parts.** Whether unsourced leaflet and papillary dimensions may ship labelled "designed", as the motion
  parameters do.
* **Occlusion in a non-acoustic image.** Whether ribs and lung should already hide what is behind them in this phase, or wait
  for phase 3.
* **Clinician or sonographer review** of the cross-sections before the docs call them more than a schematic.
