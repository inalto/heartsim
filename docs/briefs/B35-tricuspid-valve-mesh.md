# B35 — A measured tricuspid valve in the 3-D heart, opening and closing on the engine's timing
**Crates** `xtask` (bake), `heartsim-geometry`, `heartsim-echo`, `heartsim-render`, `heartsim-app`, docs · **Model** Opus
(Haiku for the docs) · **Milestone** M11 follow-up (echo simulator) · **Depends on** B2 (anatomy bake), B30 (wall
motion), B31 (designed leaflets, label volume).

## Goal
The 3-D heart shows a tricuspid valve with three real leaflets and their chordae, taken from an openly licensed
finite-element model of a human valve, fitted to this heart's tricuspid ring. It opens and closes with the valve schedule
the engine already produces, so the valve, the heart sounds, the ECG and the echo page agree. This brief is the first valve;
it must leave a path that the mitral, aortic and pulmonary valves can follow later without a second design.

## The source (checked on GitHub on 2026-11-25; re-check before use)
**Texas TriValve 1.1**, `github.com/SoftTissueBiomechanicsLab/Texas_TriValve_1.1`, licence **CC BY 4.0**
(`LICENSE-CC-BY.txt`). Cite Mathur, Meador, Malinowski, Jazwiec, Timek and Rausch, *Engineering with Computers* 2022,
doi:10.1007/s00366-022-01659-w, and the repository.

* `Healthy_QuasiStatic/Healthy_QuasiStatic.inp` (Abaqus, text, millimetres): node sets `Leaflet_Nodes` and
  `Chordae_Nodes`; S4R shell elements in `Anterior_Leaflet_Elements`, `Septal_Leaflet_Elements` and
  `Posterior_Leaflet_Elements` (the valve's midsurface); T3D2 truss elements in `Chordae_Elements`, with
  `Anterior_Chordae`, `Septal_Chordae` and `Posterior_Chordae`; node sets `Annular_Nodes` and `Free_Edge_Nodes`.
* `Healthy_QuasiStatic/Healthy_QuasiStatic_0.vtu` … `_20.vtu`: 21 result frames "from end-diastole to end-systole"
  (README). Each has 11,046 points and 8,283 cells with a cell array `element_sets` (1–3). The cell count equals the
  leaflet shells, and the point count equals leaflet plus chordae nodes, so the chordae move in the frames but their
  connectivity is only in the `.inp`. **Verify both statements before relying on them.** Arrays are appended and
  LZMA-compressed (`vtkLZMADataCompressor`, `header_type="UInt64"`).
* The frames are one load step (pressure to about 23 mmHg with prescribed annulus motion). They are ordered by load, not
  by time: they give shapes between open and shut, not a timing.

**Not usable:** `Texas_TriValve` (1.0) is CC BY-NC-SA 4.0, including its `.glb` files: do not download it into the
repository, do not copy from it. `Texas_TriValve_1.2` (CC BY 4.0, a third valve) is an alternative source if 1.1 fits
badly; decide with evidence, not both.

**Examined and not chosen for the display: the idealised benchmark** in `docs/research/tricuspid/` (Laurence, Lee,
Johnson and Hsu, *Data in Brief* 39 (2021) 107664, doi:10.1016/j.dib.2021.107664; the article is CC BY 4.0 and says
"All data is available with the article"; the data folder itself carries no licence file). Read on 2026-11-25:

* It is an oval cylinder, 30 × 20 mm at the annulus and 15 mm high, with six chordae to three papillary points 10 mm
  below the free edge; the article says the dimensions represent a typical **porcine** valve. The leaflets are one skirt
  (`PROP_TV`, 18,960 S4 elements, 19,279 nodes): there are no separate anterior, septal and posterior leaflets.
* The annulus and the papillary points are pinned. The pressure reaches 25 mmHg in 5 ms and is held to 0.4 s, and the five
  frames are 0.1 s apart: frame 0 is the open cylinder and frames 1–4 are all the closed valve (they differ by 0.23 mm
  on average, 1.6 mm at most). It gives two shapes, open and shut, and no intermediate ones.
* `Idealized_TV.obj` holds NURBS curves and surfaces, no faces; the mesh is in `01_Model_node.inp` and
  `02_Model_elem_leaflet.inp`, and the frames are ASCII `.vtk`.

Use it only as a **test fixture for the bake's Abaqus and VTK readers** (plain text, small enough to subset) and, if the
maintainer wants it, as the shape of a generic valve for the other three positions later. Do not ship it as the tricuspid
valve: it is not human and has no leaflet identity. The folder is 272 MB (a 77 MB `.odb`, 185 MB of text frames) and
must not be committed; only the article PDF may be.

## Read first
`CLAUDE.md` (non-negotiables 1–4); `xtask/src/bake/` (`fetch.rs`, `heart.rs`, `surface.rs`, `volumes.rs`: how sources are
fetched by checksum and baked into `assets/anatomy/default/`); `assets/anatomy/default/manifest.ron` and `LICENSE`;
`crates/heartsim-echo/src/valves.rs` (the four discs of `valve_planes`), `designed.rs` (leaflet dimensions with their
sources, valve timing, `valve_openings`), `anatomy.rs` (the designed leaflets as cones, `pose`, `contains`), `slice.rs`
(`pose_leaflets`); `crates/heartsim-echo/src/model.rs` (`valve_discs`, `annulus_position`, how the valve discs follow
the wall); `crates/heartsim-app/src/heart_view.rs` and `anatomy_panel.rs` (layers, the valve highlight);
`docs/book/src/user/echo.md` and `known-issues.md` (what is said about the designed leaflets today).

## Non-negotiables for this brief
* **Licence.** Only the CC BY 4.0 files above. The source is fetched by the bake with a pinned commit and checksum, never
  committed raw. `assets/anatomy/default/LICENSE`, the About window's credits and the user guide name the source, the
  authors, the licence and "modified: decimated, fitted to another heart's annulus, re-timed". Nothing from `epicardio/`,
  `epicardio-extracted/` or any other simulator.
* **One source of truth.** The valve's opening at any instant is the value `valve_openings` already returns for the
  tricuspid valve. No second clock, no timing read from the frames, no new engine state. Engines and ECG goldens untouched.
* **Deterministic and pure.** The pose of the valve is a pure function of (rest data, opening, the deformed annulus). No
  I/O or GPU in `heartsim-geometry` or `heartsim-echo`; both still compile on `wasm32` without wgpu.
* **Honest.** It is one donor's valve fitted to another heart, moved by a load sequence replayed against a designed
  timing. The page and the docs say so. No pathology is implied: the regurgitant series in the repository is out of scope.
* **The 3-D heart and the echo must not disagree** about whether the tricuspid valve is open. See scope item 5.

## Scope
1. **Bake** (`xtask bake`): fetch the `.inp` and the 21 `.vtu` at a pinned commit; read the Abaqus node and element
   blocks; read the frames (an LZMA reader in `xtask` only, never in a shipped crate); check that frame 0 equals the
   `.inp` nodes; quads to triangles; decimate to a budget (target about 3,000 leaflet triangles, chordae as polylines);
   keep per-vertex leaflet identity (anterior, septal, posterior), the annulus ring and the free edge. Give the midsurface
   the designed leaflet thickness only at render time (two-sided shading or an offset shell), not in the data.
2. **Fit**: register the donor annulus to this heart's tricuspid disc rim (`ValveDisc` for the tricuspid valve): a
   similarity transform for position, orientation and size, then a smooth warp that puts each annulus node on the rim and
   fades to zero toward the free edge. Orient it anatomically: septal leaflet on the septum, anterior leaflet toward the
   outflow tract; state how the orientation was established and test it. Chordae origins are attached to the right
   ventricular wall or the designed papillary muscles nearest to them; report how far each origin had to move.
3. **Pack format**: a new mesh kind in `heartsim-geometry` for an animated valve: rest positions, triangles, leaflet ids,
   chordae segments, and K keyframes of positions (K ≤ 21; quantised; report the size). `format_version` bumped with a
   loader that still reads the old pack. `manifest.ron` lists the file with its checksum.
4. **Pose** (`heartsim-echo` or `heartsim-geometry`, pure): `opening` in 0..=1 selects and interpolates keyframes (frame 0
   open, last frame shut; monotone mapping, documented); the result is then carried by the deformed annulus of the current
   instant (the same disc motion the wall model already gives the valve plane), so the valve rides the beating heart.
   No leaflet may pass through the opposite leaflet by more than the drawn thickness at any opening: test it.
5. **Echo consistency**: choose one and say which in the report.
   (a) Preferred: the echo's tricuspid leaflets come from the same posed mesh (a `contains` test against the posed
   triangles with the designed thickness, inside the existing bounding-box early-out), replacing the designed cone for
   this valve only; or
   (b) the designed cone stays in the echo, its opening and closure instants and its tip positions at full opening and at
   closure are tested to agree with the mesh within a stated tolerance, and the docs say the echo draws a simplified
   leaflet.
   If (a) costs more than 10 % of the slice time on the bench, fall back to (b) and report the numbers.
6. **Render and UI**: the valve as a part of the 3-D heart with a layer toggle "Tricuspid valve" (translated, en and it)
   in *Anatomy and camera*; visible when the right ventricle or atrium is cut or made translucent; the existing valve
   highlight still flashes. Reduced motion or *Wall motion* off: the valve holds its posed shape for the instant, as the
   echo leaflets do today. WebGL2-safe; no new shader feature beyond what the heart mesh uses.
7. **Docs** (Haiku): a section in the user page *The beating heart* and in *The Echo page*; known-issues (one donor
   valve, fitted; load sequence, not a timed recording; the other three valves have no leaflets in the 3-D heart; no
   clinician review); `CHANGELOG.md`; `docs/ROADMAP.md`; `README.md` credits; one screenshot and one tour step; a short
   note in `docs/research/` recording the source, the commit, the licence check and what was rejected and why.

## Out of scope
The mitral, aortic and pulmonary valves (find sources first, in a separate brief); tricuspid regurgitation or any other
lesion; blood flow; leaflet mechanics computed at run time; running Abaqus or any finite-element solver; chordae rupture;
using the valve to change the engine, the sounds or the ECG.

## Acceptance tests (no GPU unless marked)
* Bake: reproducible (same bytes twice); frame 0 matches the `.inp` nodes within float rounding; every triangle keeps a
  leaflet id; the three leaflets' areas after fitting are within a stated factor of the source's.
* Fit: every annulus vertex lies within 0.5 mm of the tricuspid disc rim at rest; the septal leaflet's centroid is the one
  nearest the septum; no leaflet vertex lies outside the right heart's blood pool by more than 1 mm at any keyframe
  (report the worst case and where).
* Pose: `opening = 1` and `0` reproduce the first and last keyframes; the map is continuous and monotone in the free
  edge's distance to the coaptation point; with the motion on, the annulus vertices follow the deformed disc within
  0.5 mm over a sinus beat; two evaluations of the same instant are bit-identical.
* Timing: over a sinus beat, in complete AV block and in atrial fibrillation, the mesh valve is shut exactly when the
  tricuspid opening is 0 and open when it is 1; it closes with the first heart sound's tricuspid component (the event the
  sound uses), within one evaluation step.
* Echo consistency: under (a), a slice through the tricuspid valve labels a point as leaflet if and only if it lies within
  the designed thickness of the posed mesh, at 20 poses and 5 openings; under (b), the tolerance test above.
* Old packs still load; a pack without the valve draws the heart as before.
* App (egui_kittest): the layer toggle exists in en and it, is off or on as the default says, and survives a restart.
* Render (GPU, lavapipe, skipped without it): the valve draws without validation errors at open and shut.
* Bench (not a gate): added cost per frame of the pose and, under (a), of the slice; added bytes in the pack and in the web
  build.
* `cargo xtask ci`, both wasm32 checks and `cargo check -p heartsim-app --no-default-features --lib` pass; ECG and sim
  goldens unchanged.

## Report back
Which echo option was taken and its measured cost; the fit residuals; how far the chordae origins moved; pack and web
size added; screenshots of the valve open and shut from the atrial side and in a four-chamber cut; everything not
verified (in particular: on a real GPU, in a browser, by a clinician).

## Decisions for the maintainer
* Whether a single detailed valve beside three valves without leaflets is acceptable in a release, or whether this stays
  behind the layer toggle (off by default) until the other valves exist.
* Whether Texas TriValve 1.2 should replace 1.1 if it fits this heart better.
* Whether the regurgitant series (also CC BY 4.0) should later become a teaching scenario; it would need a clinician's
  input and its own brief.
* Clinician review of the valve's look and motion before it is described as more than an illustration.
