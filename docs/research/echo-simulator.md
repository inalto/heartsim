# A kinematic heart and a CPU line model can drive a teaching echo simulator

heartsim can grow a transthoracic echocardiography (TTE) simulator out of what it already has, with no shipped echo data
and no new physics engine:

* the anatomy pack's Rodero heart is a set of **closed wall shells** around a **volumetric cell cloud with transmural
  depth**;
* the KCL source torso the pack was baked from has **bones, lungs and liver** under the same CC BY 4.0 licence;
* the engines already produce **per-cell activation and action-potential times** and a **per-beat valve schedule** (M1, T1,
  A2, P2) for the heart sounds.

Three gaps stand between that and an echo image: the heart does not move, nothing in the pack labels blood, bone or lung as
a volume, and there are no valve leaflets or papillary muscles.

The recommended route has four parts:

1. **A prescribed kinematic model.** Longitudinal shortening, radial thickening and twist, with amplitudes from the
   ASE/EACVI normal ranges. Each cell's motion is timed by its own activation and APD, and the phases are gated by the
   existing valve schedule. LBBB dyssynchrony, ischaemic hypokinesis, PVC and AF beat-to-beat variation and AV dissociation
   then come out of the engine state rather than being animated by hand.
2. **A baked label volume, sampled through an inverse displacement field.** The volume is about 1 mm over the heart and
   2 mm over the thorax.
3. **A deterministic per-scan-line acoustic model in a pure crate.** It computes attenuation, interface reflection by
   incidence angle, shadowing behind bone and lung, and tissue-locked speckle from a hashed 3-D scatterer field, convolved
   with a depth-dependent PSF. Its design follows Shams et al. 2008, Mattausch & Goksel 2016 and the COLE approach of Gao
   et al. 2009.
4. **A WebGL2-safe fragment shader that only scan-converts and maps grey levels.**

The cheap fallback is the same slice drawn as a flat-coloured anatomical cross-section. Views are scored geometrically from
landmarks and the probe pose, not from the image.

The evidence supports this teaching design: simulators reliably teach spatial orientation and view recognition, and teach
probe skills that transfer to real patients less consistently. A 3-D display of the scan plane in the heart and automatic
per-view scoring are the features with a rationale behind them.

No openly licensed echo cine set exists. CAMUS, EchoNet, MITEA, M&Ms, CETUS, HMC-QU, Unity and Radiopaedia are all
non-commercial or under agreement. Validation must therefore rest on published normal values, the CC0 Sunnybrook cine MRI,
and a handful of CC BY clips on Wikimedia Commons.

Labels used below:

* **Sources state** marks what a cited page or the repository says.
* **Inference** marks reasoning in this note.
* **Designed** marks a choice the model makes where no source gives a value.
* **Unverified** marks items seen only in abstracts, search extracts or secondary copies, or not reached at all.

The source extracts, with verbatim licence quotes, are in [`echo-simulator-notes/`](echo-simulator-notes/). The research was
done on 2026-11-22. In line with the clean-room rule, no proprietary echo simulator was opened, and neither was
`epicardio/`.

## 1. What the codebase already gives

### 1.1 Anatomy pack, frame, meshes

**Frame.** The frame is in millimetres, with the origin at the centroid of the ventricular cells. +x is the patient's left,
+y cranial and +z anterior (`docs/anatomy-pack.md:9-12`, `PackManifest.frame` in `assets/anatomy/default/manifest.ron:5`).
The heart's pose is measured, not assumed:

* the LV long axis is at 36° in the frontal plane and 46° in the transverse plane (Engblom 2005; UK Biobank);
* the anterior chest wall is 84 mm from the ventricular centroid;
* V1–V6 are 54, 36, 33, 30, 41 and 57 mm from the nearest heart cell (`docs/anatomy-pack.md:16-24`).

This is exactly the geometry a parasternal window depends on.

**Heart.** The heart is the Rodero et al. 2021 statistical-shape-model *average* (Zenodo 4593739, CC BY 4.0, read today:
"Creative Commons Attribution 4.0 International"). It is end-diastolic CT-derived. The record describes "end-diastolic
CT-derived meshes" and ships a `simulation_output.csv` with EDV and EDP "for validation purposes".

The pack keeps ten render meshes (`manifest.ron`):

| mesh | kind | size |
|---|---|---|
| `lv`, `rv`, `ra`, `la` | `Myocardium` | about 30 k triangles each |
| `aorta`, `pulmonary_artery` | `Vessel` | 6 k triangles each |
| `svc`, `ivc`, `pulmonary_veins` | `Vessel` | lids only |
| `valve_planes` | `Valve` | four discs, 20 k triangles |

Each chamber mesh is "the boundary faces of its tags (closed by construction)" (`docs/anatomy-pack.md:110-115`). So
**`mesh_lv.glb` is a closed shell around the LV wall, endocardium and epicardium together, and the wall has real thickness**.
The blood pool is the cavity enclosed by the endocardial shell and the mitral and aortic discs. It is **not labelled**, but
it can be recovered by a flood fill in a voxelisation (inference).

The **cell cloud** is volumetric:

* 20 051 cells on a 2.3 mm lattice of points inside the myocardial tetrahedra;
* each cell carries a fibre, a region, a transmural depth ρ (0 endo … 1 epi) and an AHA segment;
* the atria are about one cell thick (`docs/anatomy-pack.md:88-99`, file table at `:30-41`).

**Landmarks** are cell indices, for example `lv_apex` = cell 13024 (`assets/anatomy/default/landmarks.ron:15`), `rv_apex`,
`rvot`, `laa` and `la_pv_antrum` (`docs/anatomy-pack.md` file table). The valve centres are not landmarks. They have to be
taken from the four connected components of `valve_planes` (inference).

**What is absent** from the heart data:

* valve leaflets (the valves are flat discs);
* chordae, papillary muscles, trabeculae, the moderator band;
* the pericardium;
* the LA appendage beyond its inlet, because "the dataset crops the appendage" (`docs/anatomy-pack.md:115`);
* great vessels beyond the dataset's crop.

How far the merged great-vessel work (branch `claude/objective-visvesvaraya-e828a5`, merged into master) extends the aorta
towards the arch and the descending aorta was not checked here. The suprasternal and abdominal views depend on it.

**Torso.** `torso.glb` is only the skin of KCL whole-torso CT model 1, decimated to 12 000 triangles (`docs/anatomy-pack.md:62`,
`xtask/src/bake/torso.rs:23-29` uses tags 2 skin, 16 mitral, 34 and 35 walls only). The source model is Zenodo 7253863,
read today: "Creative Commons Attribution 4.0 International". The repository's licence survey lists its tags as "inner body,
skin (2), bones, kidneys, liver, stomach, spleen, lungs (8), SVC, left PA, aorta, valves, LA/RA walls and blood pools, LV/RV
blood pools …" (`docs/research/anatomy-meshes.md:54`). So **ribs (as one bone tag), lungs and liver exist in the already
downloaded, already licensed source, in the same frame transform as the skin**. They are simply not baked.

A project memory note warns that the KCL information sheet's tag numbers are off by one around the vessels. Every tag must
be verified by volume and position before use, as the vessel bake already does.

**Human figure.** `human.glb` is a 1.55 m standing figure registered to the KCL torso. Its skin is within 3 mm of V1–V6, V3R,
V4R, RA and LA. The abdomen is fuller (RL 22 mm inside the figure) and the back flatter (`crates/heartsim-geometry/src/pack/body.rs:1-16`).
`body_region` classifies skin points as trunk or limb (`body.rs:87-105`). Inference for the probe:

* at the parasternal and apical windows the probe on the figure's skin sits within a few millimetres of the KCL skin;
* at the subcostal window the gap may be about 1–2 cm, which can be treated as subcutaneous tissue.

**Chest landmarks.** Intercostal heights and vertical chest lines exist as tables: `INTERCOSTAL_SPACES` (2nd–6th ICS at
y = 75, 45, 15, −15, −45 mm) and `VERTICAL_LINES` (sternal borders ±12°, mid-clavicular ±52° …) in
`crates/heartsim-ecg/src/placement.rs:520-547`. `ListeningSite::landmark_mm` already turns them into positions
(`crates/heartsim-ecg/src/listening.rs:97-101`). These heights are the electrode editor's, which is a designed spacing.
The real ribs are what should define the windows.

### 1.2 Rendering and picking

`heartsim-render` draws each mesh as a layer, using the scene shader, the vertex layout and the clip plane:

* **Scene shader** (`crates/heartsim-render/src/shaders/scene.wgsl:1-80`): Blinn-Phong shading, a per-vertex scalar mapped
  through a LUT texture, and an iso-contour.
* **Vertex layout**: position, normal, scalar.
* **Clip plane**: one global plane (`common.wgsl:11-14`, `is_clipped` at `:41-45`). The back faces of a clipped layer draw in a
  flat "cut" colour (`fs_cut`, `scene.wgsl:51-57`). That is already a crude anatomical cross-section.

Two things matter for animation:

* Vertex buffers are created with `BufferUsages::VERTEX` only (`crates/heartsim-render/src/layer.rs:84-88`). Only the scalar
  buffer is `COPY_DST` (`:89-93`). Moving vertices therefore needs a writable position and normal buffer, or a displacement
  attribute.
* Picking ray-casts the CPU copy of the rest mesh (`layer.rs:105`; `SceneRenderer::pick` at `scene.rs:1257`). The pick would
  lag the drawn mesh by up to the wall excursion.

The WebGL2 rules are no compute, no vertex-stage storage buffers and no `@interpolate(linear)` (`docs/web.md:84-89`). They are
enforced by `crates/heartsim-render/tests/webgl2_shaders.rs`, a no-GPU test.

**Picking on the figure.** The Electrodes tab casts `scene.pick_ray(uv, aspect)` against the body or torso, classifies the hit
with `body_region`, and snaps to chest landmarks (`crates/heartsim-app/src/electrode_tab.rs:15`, `:150-162`, `:523-534`;
`crates/heartsim-ui/src/electrode_editor.rs:288-292`). A probe can reuse this mechanism unchanged for *slide*.

**Web budget.** `cargo xtask web-budget` fails above 25 MiB (`xtask/src/web.rs:19`). A release build measures 15.1 MiB: 9.4 MiB
of wasm and a 5.7 MiB deflated asset archive (`docs/web.md:93-94`). That leaves about 10 MiB, which matches the task's
"~10 MB" figure.

### 1.3 Timing signals

* **`TimingEngine` events.** Each `Event` carries `at`, `region`, `kind`, `origin` and `chamber`
  (`crates/heartsim-sim/src/engine.rs:142-148`). `Circuit` marks flutter, AF and VF wavelets (`engine.rs:42-53`). Everything
  goes through one deterministic queue (`timing/mod.rs:26-29`).
* **Per-cell state** (`CellState`, `engine.rs:6-13`, served by `Engine::cells`, `tier1/mod.rs:1699`):
  * `vm` (normalised);
  * `activation_ms`, the time of the last activation;
  * `apd_ms`, the APD assigned at that activation.

  The heart view already maps these to vertices through hoods: 8 cells per vertex with inverse-distance weights
  (`crates/heartsim-geometry/src/hood.rs:13-21`; `map_hoods`, `crates/heartsim-render/src/scalar_map.rs:24`; usage at
  `crates/heartsim-app/src/heart_view.rs:657-747`). **This is the mechanical activation map for free:** each region's
  contraction can start at its own activation time and last for its own APD.
* **Beat schedule** (`crates/heartsim-ecg/src/pcg.rs`):
  * `BeatAssembler` turns events into `Beat` (`:323-347`). A `Beat` holds the QRS onset, the atrial activation, the AV
    interval, the `AtrialActivity` (Contracted, Dissociated, Fibrillating, Fluttering or Silent), the RR, the origin, which
    ventricle is late and by how much, and `organised` (false for a VF wavelet).
  * `schedule` places M1, T1, A2 and P2 (`:705-751`):
    * M1 is at QRS + 40 ms (`q_m1_ms`, `:230`).
    * A2 is at QRS + QS2(HR) + the LV delay, with `QS2 = 546 − 2.1·HR` (`:234-235`, `:637-639`, sourced in
      `docs/research/heart-sounds.md`).
    * A ventricular beat with RR < 300 ms does not eject (`min_ejecting_rr_ms`, `:241`, `:729-733`).
  * **The closure times of the mitral, tricuspid, aortic and pulmonary valves, and "no ejection" for early PVCs, already
    exist.** The openings follow by adding the isovolumic intervals (§2.2).
* **Respiration.** `RespiratoryClock` and the breath-hold (`timing/mod.rs:393-401`, `timing/params.rs:161-194`) can drive
  breathing displacement of the heart and lungs, IVC calibre and "hold inspiration" in the subcostal view.
* **Ischaemia.** Ischaemia is per cell, on the AHA territories, with a transmural injury `M(ρ)`
  (`crates/heartsim-sim/src/ischaemia.rs:1-25`, `Lesion::injury` at `:317`). It is a direct input for regional hypokinesis.
* **Valve lesions.** `LesionKind` includes AS, AR, MR, MS, MVP, TR, PS, VSD and ASD (`crates/heartsim-core/src/scenario.rs:151-171`).
  Leaflet appearance could follow it: restricted, thickened or prolapsing leaflets.
* **Requirements.** F-25 already asks for a "cosmetic contraction animation driven by activation (template-based vertex
  displacement); LVEF read-out", at P2 (`docs/REQUIREMENTS.md:58`). Brief B29 (Echo calculators) is independent and
  introduces the precedent of a new pure crate, `heartsim-calc` (`docs/briefs/B29-echo-calculators.md:2`).

### 1.4 What is missing for an echo image

| need | status | how to fill |
|---|---|---|
| motion | none; end-diastolic static mesh | §2 kinematic model |
| blood vs wall vs outside as a volume | only implicit in closed shells | bake a label volume (§3.4) |
| wall thickness numbers | present geometrically, never measured | measure in the bake; check against 6–10 mm |
| ribs, lungs, liver, sternum | in the KCL source (CC BY 4.0), not baked | bake into the thorax label volume |
| valve leaflets, papillary muscles, moderator band, pericardium | absent | **designed** parametric parts (§2.4) |
| descending and abdominal aorta, arch | not checked | check the great-vessel bake; else designed tube |
| probe, sector, view targets | absent | §4 |

## 2. Heart motion from beat timing alone

### 2.1 Options

1. **Volume-curve scaling (Wiggers).** Scale each chamber about its centroid to follow a template volume curve.
   * Pro: trivial, and EF is exact.
   * Con: no longitudinal or AV-plane motion, which is *the* dominant visible motion in apical views; no thickening; no
     dyssynchrony.
   * Inference: not acceptable for echo.
2. **Prescribed kinematics in cardiac coordinates.**
   * The motion is split into three components: displacement along the long axis, growing from about 0 at the apex to MAPSE
     or TAPSE at the annulus; inward radial motion, larger at the endocardium (incompressibility gives the thickening); and a
     rotation about the long axis that varies linearly from basal to apical.
   * This is the classic decomposition used to describe LV mechanics (twist: Sengupta 2008, doi:10.1016/j.jcmg.2008.02.006,
     abstract read).
   * It is the same idea as the template-based displacement in F-25.
3. **Statistical shape and motion models with open licences.** **None found.**
   * The Imperial bi-ventricular atlas is CC BY-NC-ND 4.0.
   * The UK Biobank motion atlases are under UK Biobank access terms.
   * Rodero 2021 has no motion: its 1000-mesh cohort (Zenodo 4506930, CC BY 4.0) is end-diastolic.
   * Sunnybrook cine MRI (CC0) has motion, but only as 2-D short-axis contours of the LV. It is usable for **validating**
     volume curves and wall motion, not as a 3-D motion model.
4. **Electromechanics** (active tension plus large-deformation finite elements, as in Rodero's own simulations). This is out
   of scope: REQUIREMENTS lists "true electromechanics" as a non-goal (`docs/REQUIREMENTS.md:98`), and it is not real-time on
   wasm.

**Recommendation: option 2.** Each mesh vertex is moved by a displacement that is the sum of the three components above. Each
component is weighted by a per-vertex contraction state `c(t) ∈ [0, 1]`. The state comes from that vertex's hood cells:
their `activation_ms` plus an electromechanical delay, held for a fraction of their `apd_ms`. Global volume constraints are
imposed per phase by the valve schedule. Section 2.3 gives the rules.

### 2.2 Numbers

Sources state the following, unless marked.

| quantity | value | source |
|---|---|---|
| LVEF (biplane) | men 62 ± 5 % (52–72), women 64 ± 5 % (54–74) | Lang 2015, EHJ-CVI 16:233, doi:10.1093/ehjci/jev014, p239 Table 2 |
| EDV index / ESV index | men 54 ± 10 / 21 ± 5 mL/m²; women 45 ± 8 / 16 ± 4 | same, p239 |
| LV internal diameter, diastole / systole | men 50.2 ± 4.1 / 32.4 mm; women 45.0 ± 3.6 / 28.2 mm | same, p239 |
| fractional shortening | men about 35 %, women about 37 % | **inference** from the diameters above |
| septum and posterior wall, diastole | 6–10 mm (men), 6–9 mm (women); RWT 0.22–0.42 | Lang 2015, p248 Table 6 |
| wall thickening | about 33 % from a cylindrical incompressible ring with the diameters above, more once longitudinal shortening is included | **inference**; no sourced normal value reached |
| GLS | "in the range of −20 % can be expected" | Lang 2015, p242 |
| MAPSE | normal-EF cut-off ≥ 13 mm (men), ≥ 11 mm (women); < 6 mm means severely reduced EF | Matos 2012, doi:10.1016/j.echo.2012.06.011 (abstract); 12–15 mm typical **unverified** |
| TAPSE | 24 ± 3.5 mm, abnormal < 17 | Lang 2015, p254 Table 10 |
| RV FAC; S′; RV free-wall strain | 49 ± 7 %; 14.1 ± 2.3 cm/s; −29 ± 4.5 % | same |
| LV rotation at peak | apex −11.6 ± 3.8° (MRI), base +4.8 ± 1.7°, so a net twist of about 15–16° | Helle-Valle 2005, doi:10.1161/circulationaha.104.531558 (abstract); sign as viewed from the apex (Sengupta 2008) |
| aortic annulus | men 2.6 ± 0.3 cm, women 2.3 ± 0.2 cm; sinuses 3.4 / 3.0; STJ 2.9 / 2.6 | Lang 2015, p265 |
| IVCT, LVET, IVRT | 40 ± 10 ms, 292 ± 23 ms, 96 ± 19 ms (population study, n = 1969) | Alhakak 2023, doi:10.1007/s00392-023-02269-2 (abstract; the heart-rate regressions were not seen) |
| IVRT | "≤ 70 msec in normal subjects" | Nagueh 2016, JASE 29:277, doi:10.1016/j.echo.2016.01.011, p282 |
| E/A, deceleration time | 1.4 ± 0.3; 180 ± 31 ms | Lang 2015, p254 Table 10 |
| QS2 (QRS to A2) | 546 − 2.1·HR ms (men) | already in `pcg.rs:234`; heart-sounds note |
| LVET vs HR | 413 − 1.7·HR ms (men), 418 − 1.6·HR (women) | Weissler 1968, **unverified**: AHA returned 403; read from a patent text quoting it |
| electromechanical delay, local | no healthy value found | **designed**: must fit M1 = QRS + 40 ms |

Consistency check (inference) with the code's anchors:

* Ejection would run from M1 + IVCT, about QRS + 80 ms, to A2.
* That gives LVET = QS2 − 80 = 340, 308 and 256 ms at 60, 75 and 100 bpm.
* Alhakak's 292 ± 23 ms and Weissler's 413 − 1.7·HR (311 ms at 60 bpm) both bracket this. One extra parameter (IVCT
  50–60 ms) would bring the two into closer agreement.

Diastole (inference and designed; no reached source gave the durations of the phases):

* Mitral opening = A2 + IVRT (70–90 ms).
* Rapid filling (E) runs over about 150–200 ms, with a deceleration time of about 180 ms (Lang).
* Diastasis fills the remainder.
* Atrial contraction (A) starts at the atrial activation plus an atrial electromechanical delay (designed, about 50–80 ms)
  and ends at M1.
* At high heart rates diastasis vanishes and E and A fuse. This follows from the timing; no separate rule is needed.

### 2.3 Proposed motion model (designed, every number a parameter with a source or "designed")

**Per-cell contraction state.** `c_i(t) = shape((t − t_act,i − EMD) / (k·apd_i))`.

* The shape is a smooth rise over about 80 ms and a plateau, then a decay over the last third of the APD.
* `k` (≈ 0.9, designed) is chosen so that the global LV shortening ends near A2. Two published anchors support this: the T-end
  to S2 lag is about 5 ms (Sbrollini 2017, cited in `docs/research/heart-sounds.md`), and A2 marks the end of ejection.
* Contraction duration then follows QT changes: drugs, ischaemia and long QT already move `apd_ms`.

**Gating by the valve schedule.**

* **Isovolumic contraction** (M1 to aortic opening): the LV volume is held. Regional `c_i` minus its mean drives shape change
  only, so early regions shorten and late regions are stretched. This is the mechanism of the LBBB **septal flash**, a
  "short-lived septal motion occurring during the isovolumic contraction time" (Parsai 2009, doi:10.1093/eurheartj/ehp050),
  and of **apical rocking**. Sources: Risum 2015 JACC 66:631 for the classic LBBB pattern, read only as a search extract
  (**unverified**).
* **Ejection** (aortic opening to A2): the volume falls to `EDV·(1 − EF_beat)`.
* **Isovolumic relaxation**: the volume is held.
* **Filling**: E, then diastasis, then A. The atrial volume curves are the complement of the ventricular ones, plus atrial
  contraction when `AtrialActivity::Contracted`.

**Beat-to-beat stroke volume.** `EF_beat = EF_0 · f(RR_prev, RR_prevprev)`.

* Sources state that in AF the stroke and filling volumes rise with the preceding RR only below about 600 ms and plateau
  above, and that a shorter pre-preceding RR gives a larger stroke volume (Iwase 1988, doi:10.1007/bf01797720, abstract).
* Post-extrasystolic potentiation: post-ectopic LVEF was 74 ± 6 % in a canine study (Kowlgi 2020, doi:10.1093/europace/euaa025).
  No quantified human source was reached.
* The premature beat itself ejects less, and nothing below `min_ejecting_rr_ms` (already in the heart-sound model, so the
  aortic valve then stays shut in the echo too).
* The shape of `f` is designed.

**Rhythms, read straight from engine state** (inference):

| rhythm | what the engine provides | what the echo shows |
|---|---|---|
| LBBB / RV pacing | late lateral-wall `activation_ms` | septal flash, lateral delay, paradoxical septum, apical rocking |
| RBBB | late RV | delayed RV free wall and TAPSE |
| PVC | origin Ventricular, short RR | early, weak beat; post-ectopic beat stronger |
| AF | `AtrialActivity::Fibrillating` | no A wave or atrial contraction (designed small atrial quiver); irregular EF via `f` |
| complete AV block | atrial events independent of the QRS (`Dissociated`) | atria contract on their own clock; atrial contraction against closed AV valves (the "cannon" situation; visual only, **designed**) |
| VF | `organised = false` | low-amplitude incoherent wall motion (designed), no valve motion |
| ischaemia | `M(ρ)` per cell | contraction amplitude × (1 − M), a regional wall-motion abnormality in the right AHA territory |

**Respiration.** The whole heart is translated caudally by a few millimetres in inspiration, and the lungs expand
(**designed**; no source reached).

**Output.** A displacement per cell, mapped to vertices through the existing hoods, the same path as the colour map. That
keeps the "one source of truth" rule: the ECG, the sounds, the 3-D view and the echo all read the same events and cell
state.

### 2.4 Designed parts the data lacks

* **Valve leaflets.**
  * Each leaflet is a thin curved surface hinged on its disc rim in `valve_planes`. The opening angle is a function of time
    between the opening and closing events (rapid opening, a slight mid-ejection drift, rapid closure).
  * The mitral valve also needs a partial mid-diastolic closure and an A-wave reopening, and E-point septal separation.
  * No primary source with leaflet lengths or opening durations was reached, so these values are **unsourced** and must be
    looked up before the brief.
  * Lesions modify the parameters: AS gives restricted opening and bright leaflets; MS gives doming; MVP gives a leaflet past
    the annulus plane in systole.
* **Papillary muscles.** Two ellipsoids on the LV wall at the anterolateral and posteromedial positions (AHA segments 11–12
  and 9–10 at mid level; **designed**). They are needed for the PSAX papillary level, whose quality criterion is that "the
  papillary muscles should not wobble" (Mitchell 2019, p26).
* **Moderator band.** One RV band, required for the A4C quality criterion "moderator band visible" (Mitchell p26).
* **Pericardium.** A bright 1–2 mm shell outside the epicardium.

## 3. Simulating the B-mode image in real time

### 3.1 Published approaches

The method and formula details below were read in the primary text only for Mattausch 2016 (preprint), Shams 2008 and
Alessandrini 2015 and 2018. The others are from abstracts or search metadata (unverified).

* **Ray-based, from meshes.**
  * **Bürger, Bettinghausen, Rädle & Hesser 2013**, IEEE TMI 32:609, doi:10.1109/TMI.2012.2234474 (abstract): GPU ray tracing
    through deformable triangle meshes (over 5000 rays, over 2 M triangles), simulating reflection, refraction, range
    distortion and shadowing. Speckle comes from convolution, with per-tissue scatterer parameters held in a 3-D noise
    texture. It was aimed at education.
  * **Mattausch & Goksel 2016**, Eurographics VCBM, doi:10.2312/vcbm.20161285 (preprint read):
    * Interface echo `I_r = max(cosⁿ∠(V_r, D), 0) + max(cosⁿ∠(V_i, D), 0)`, with reflected and refracted directions. It notes
      that the refractive term "was missing in previous work" (Bürger).
    * Beer–Lambert attenuation.
    * Separable Gaussian-envelope PSF `h = exp(−l²/2σ_l² − a²/2σ_a² − e²/2σ_e²)·cos(2πf_c l)`.
    * Scatterers from a tiled 256³ Gaussian-noise 3-D texture: value `µ + ξσ` where `ξ > ρ`, otherwise 0, modulated by a
      coarse 20³ texture.
    * Monte-Carlo rays model surface roughness.
  * **Mattausch, Makhinya & Goksel 2018**, CGF, doi:10.1111/cgf.13260 (metadata only): path tracing. **Freedom to operate:**
    patent US10565900B2 (ETH) was reported to cover Monte-Carlo and bidirectional path tracing for ultrasound simulation. Its
    claims were not read; the recommended design does not path-trace.
* **CT-based.**
  * **Shams, Hartley & Navab 2008**, MICCAI LNCS 5242:734 (read):
    * Reflection `α_R = ((Z₂ − Z₁)/(Z₂ + Z₁))²`.
    * Received echo ∝ `α_R·I_i·|r·n|` (Lambertian).
    * Transmission accumulated along the line.
    * Final image `(I_r + α)·I_s`, with `I_s` a precomputed speckle image.
    * It states "bone-tissue interfaces reflect 43% and air-tissue interfaces reflect 99%".
  * **Kutter, Shams & Navab 2009**, CMPB 94:250 (abstract): the GPU version.
  * **Wein et al. 2008**, MedIA 12:577 (abstract): reflection plus attenuation from CT for registration.
  * **Reichl et al. 2009**, SPIE 7261 (snippet).
  * **Salehi et al. 2015**, MICCAI LNCS 9350:510, doi:10.1007/978-3-319-24571-3_61 (abstract).
  * The TUM PDFs timed out.
* **Convolution-based.**
  * **COLE**: Gao et al. 2009, IEEE TUFFC 56:404, doi:10.1109/TUFFC.2009.1051 (abstract). A 1-D convolution per image line
    with a PSF that varies with depth, about 1200× faster than Field II with local Rayleigh statistics.
  * Open re-implementation: **OpenBCSim** (BSD, "Licensed under the BSD license", LICENSE read).
  * **Hergum et al. 2009**, "Fast ultrasound imaging simulation in k-space", TUFFC 56:1159, doi:10.1109/TUFFC.2009.1158. This
    is probably the "FUSK" the task meant; no tool of that name was found.
  * **Alessandrini et al. 2015**, TMI 34(7), doi:10.1109/TMI.2015.2396632 (read): the COLE pipeline for synthetic 3-D cardiac
    sequences, with phased array, 3.3 MHz, 76° × 76° sector and echogenicity mapping `20 log₁₀F(a) = dB·a/255`.
    Their dataset page returned 403, so its licence is unknown.
* **References only.**
  * **Field II** (field-ii.dk, read): "not permitted to utilize any part of the software in commercial products without prior
    written consent". Use it only as a cited reference, never as code.
  * **k-Wave**: LGPL-3.0.
  * **SIMUS/MUST**: PyMUST LGPL-2.1.
  * Neither is real-time.
* **Generative.**
  * GAN refinement of physics images: Tom & Sheet 2018, arXiv:1712.07881.
  * Label-conditioned diffusion trained on CAMUS: Stojanovski 2023, arXiv:2305.05424.
  * Neither fits wasm, and training data would be NC.

**Open implementations and their licences** (LICENSE files read today):

* **Permissive, reusable as reference:**
  * PlusLib's `PlusUsSimulator`: "a BSD style license".
  * UltraRay (MIT).
  * `ingle/ultrasound-simulation` (MIT-style).
  * snappysonic (BSD).
* **GPL-3.0, not usable:** sofus, scus, UltrasoundSimulator.
* **No licence, so all rights reserved:** burgercpp, MCRay-Tracing.
* **No WebGL/GLSL ultrasound simulator was found on GitHub.**

### 3.2 Physical numbers for the model

* **Impedance Z and speed** (Stanford RAD225 course notes, which cite Szabo and Duck; read): air 0.0004 MRayl / 344 m/s;
  water 1.48 / 1482; blood 1.68 / 1584; muscle 1.65 / 1580; liver 1.64 / 1578; fat 1.33 / 1430; bone 5.69 / 3360.
* **Attenuation** (same source), in dB/cm/MHz: blood 0.14, liver 0.45, soft tissue 0.5, muscle 1, bone 10, air 12.
* **Myocardium in vivo**: about 0.8–1.9 dB/cm/MHz depending on the layer (Sosnovik 2001, UMB 27:1643, snippet only,
  **unverified**).
* **IT'IS database**: its notice allows copying "only for private, scientific and noncommercial use". It must not be the
  source of shipped values.
* **Transmit frequency**: "typical range … 2.0 to 5.0 MHz" (Mitchell 2019, p6). λ = 1.54 mm / f(MHz), so about 0.6 mm at
  2.5 MHz.
* **Speckle**: Rayleigh-distributed envelope for fully developed speckle (Wagner 1983, IEEE TSU 30:156, not read). Point
  SNR ≈ 1.91.
* **Resolution and sector** (textbook values, **unverified**): axial resolution about half the spatial pulse length; lateral
  resolution about λ·F#, worsening with depth beyond the focus; sector 75–90°; footprint about 2 cm; depth 12–24 cm.
  Candidate source not read: Ng & Swanevelder 2011, doi:10.1093/bjaceaccp/mkr030.
* **Display**: log compression over about 60 dB of dynamic range (RAD225 uses −60 dB).

### 3.3 Fit to this project (inference)

* **3-D GPU ray tracing** of meshes needs compute or a BVH in storage buffers. WebGL2 has neither (`docs/web.md:86`).
* **A fragment-shader march along each scan line** is O(depth²) per pixel if every pixel re-marches. It is expensive on
  integrated GPUs under WebGL2.
* **The beam lies in the image plane**, which simplifies everything else. Shadowing and attenuation are 1-D accumulations
  along each of about 128 scan lines. On the CPU that is about 100 k samples per frame, which is cheap, deterministic and
  testable without a GPU.
* **COLE's insight applies:** in polar (line, depth) space the PSF is a separable 1-D axial plus a short lateral convolution.

### 3.4 Recommended pipeline

**Bake, offline in `xtask bake-anatomy`, deterministic.** Pack format version 3 adds a label volume.

* **Heart box.** About 140³ mm at 1 mm, labels u8: LV, RV, LA and RA myocardium; LV, RV, LA and RA blood; aortic and
  pulmonary wall and lumen; valve-plane rims. Blood comes from a flood fill inside the closed shells and lids; vessels need
  caps where the dataset crops them.
  * Stored as `HSVL` v1 (header, origin, spacing, dimensions, run-length u8), with size and CRC in the manifest.
  * Inference on size: about 2.7 MB raw. Labels run-length encode to a few hundred kB.
* **Thorax box.** About 300 × 450 × 240 mm at 2 mm (about 4 M voxels): skin and subcutaneous tissue, muscle and other soft
  tissue, bone (ribs, sternum, spine), lungs, liver, other organs, and the KCL subject's blood pools outside the Rodero heart.
  * A priority rule resolves overlaps: Rodero heart > lungs > KCL heart.
  * Inference on size: well under 1 MB deflated.
  * The project budget absorbs both boxes easily.
* **Per-label acoustic table** (RON, `format_version`): Z, attenuation, scatterer density and amplitude (Mattausch's µ, σ, ρ),
  and specularity n. Every row is sourced from §3.2 or marked designed.

**Per frame, on the CPU, in a pure crate.** These steps are deterministic functions of (engine state, probe pose, time).

1. **Displacement grid.** Evaluate the kinematic model (§2.3) on a coarse reference-space grid of about 3 mm over the heart
   box, extended smoothly into the cavities. About 50–100 k nodes.
2. **Sector geometry.** The apex is at the probe's skin point, with lines L = 96–128, a sector of 60–90°, depth D = 10–24 cm,
   and S = 512–768 samples per line.
3. **Sampling each point.** For every sample point x:
   * map it to its reference position by fixed-point iteration `X ← x − U(X)`, 2–3 iterations (inference: this converges
     because |∇U| < 1 for wall strains of about 20–40 %);
   * look up `label(X)`: the heart box, else the thorax box (thorax tissue does not move except with respiration).
   * Designed leaflets, papillary muscles and the band are evaluated analytically in reference coordinates and override the
     label.
4. **Line model.** March each line:
   * accumulate attenuation in dB (`α·f·2·depth`, two-way);
   * at label changes, take the Shams reflection coefficient, transmission `1 − α_R`, and incidence-angle weighting
     `|cos θ|ⁿ` (Mattausch);
   * the normal comes from the gradient of a smoothed one-hot field of the two labels;
   * bone sets transmission to about 0.57 and attenuates strongly, which gives a shadow;
   * air and lung reflect 99 %, followed by a designed reverberation of A-lines spaced at the pleural depth and a shadow;
   * low-attenuation blood gives posterior enhancement naturally.
5. **Speckle.**
   * Scatterer amplitude is `hash3(floor(X / 0.15 mm), seed)`: Gaussian, thresholded by the label's density ρ, then scaled by
     µ and σ.
   * Because it is sampled at the **reference** position X, speckle moves with the tissue and decorrelates with out-of-plane
     motion, as it does in reality.
   * Blood gets a very low amplitude. It is near-anechoic at these frequencies, which is the standard B-mode look (inference).
6. **PSF and image formation.**
   * Convolve in polar space, axially with a Gabor of about 2 cycles (designed, from f) and laterally over 3–7 lines with a
     width that grows with depth beyond the focus.
   * Take the envelope (the magnitude of the analytic signal, or simply the convolved magnitude of a complex PSF).
   * Add the specular interface term, apply TGC (user sliders, default compensating α·f·depth) and gain, then log-compress to
     DR = 60 dB and quantise to u8.

**GPU.** Upload a `L × S` R8 texture: 128 × 768 = 98 kB per frame. A fragment shader scan-converts (r, θ) to the screen with
bilinear sampling, masks the sector, and applies a grey map and a depth scale. It uses no storage buffers and no compute, so
it is WebGL2-safe and checked by the existing shader test. Alongside it, the 3-D view gets a translucent quad for the sector
plane, using the existing layer pipeline.

**Cheap fallback** (and phase 2's deliverable): the same per-sample labels drawn as flat colours, an "anatomical cross-section"
with no speckle and no attenuation. Its frame cost is one label lookup per sample.

**Cost estimate** (inference; measure before committing):

* 100 k samples × (3 trilinear displacement fetches + a label fetch + a trilinear hash noise) is about 10–20 M simple
  operations, plus the convolution at about 2–3 M multiply-adds.
* That is a few milliseconds natively and perhaps 5–15 ms single-threaded in wasm at 30 fps.
* If too slow, there are three escapes: halve S, run the echo at 20–25 Hz, or move speckle and PSF to two fragment passes
  over the polar texture, with the CPU supplying an RGBA16F texture of reference coordinates.

## 4. Views and probe handling

### 4.1 The standard views (Mitchell et al. 2019, JASE 32:1–64, doi:10.1016/j.echo.2018.06.004)

The full PDF was read from asecho.org; PDF page numbers equal journal page numbers. The parasternal and apical windows are
taken in left lateral decubitus, the subcostal and suprasternal ones supine (p4–5).

| view | probe position / index marker | manoeuvre from the previous view | must show / quality |
|---|---|---|---|
| PLAX | 3rd or 4th ICS, left sternal border; marker toward the right shoulder, "approximately the 9 to 10 o'clock position" | — | LV "perpendicular to the ultrasound beam", "relatively horizontal"; "the apex should not be seen" (a false apex is corrected by rotate, tilt or angle); 2 of 3 AV cusps, both MV leaflets; no papillary muscles (p23–25, 31) |
| RV outflow | same | "tilted anteriorly from the PLAX view and rotated slightly clockwise" | RVOT, 2 PV leaflets, main PA (± bifurcation) (p25) |
| RV inflow | same | "tilting the transducer inferiorly toward the patient's right hip", ± counterclockwise rotation | TV centred, RA, ± CS, Eustachian valve, IVC (p25) |
| PSAX levels | rotate "90 clockwise from the PLAX view", marker toward the left shoulder | great vessels (one interspace up helps); AV (tilt inferiorly); MV ("inferiorly and slightly leftward"); papillary ("just below the leaflet tips"); apex (tilt or slide 1–2 ICS down and lateral) | AV: all 3 cusps, PV, TV, coronary origins at about 3–5 and 11 o'clock; MV: full excursion, RV crescent; papillary: LV "should appear circular, and the papillary muscles should not wobble"; apex: RV usually absent (p25–26) |
| A4C | about the 5th ICS at the point of maximal impulse, toward the midaxillary line; marker "4 to 5 o'clock", "toward the bed" | — | about 2/3 LV and 1/3 LA along the long axis; the apex tapers (a rounded apex means foreshortened); moderator band; full MV and TV excursion; left heart on the screen's right, apex top centre (p26) |
| RV-focused A4C | same | rotate "slightly counterclockwise", keeping the LV centre and avoiding an anterior tilt | maximal RV area; TV annulus for TAPSE (p26) |
| A5C | same | tilt anteriorly | LVOT, AV, proximal ascending aorta (posterior tilt instead gives the CS view) (p26) |
| A2C | same | rotate "approximately 60 counterclockwise" from A4C | LA, MV, LV, CS in short axis, ± LAA and LUPV; no RV (p30) |
| A3C / APLAX | same | a further 60° counterclockwise | LA, MV, LV, AV, aorta (p30) |
| Subcostal 4C | subxiphoid; marker toward the patient's left at about 3 o'clock, aimed at the left shoulder; supine, knees bent, held inspiration | — | septa perpendicular to the beam (p31) |
| Subcostal SAX | same | rotate about 90° counterclockwise | a sweep from the great arteries to the apex as a PSAX substitute (p31, p59) |
| Subcostal IVC | same | keep the short-axis orientation and aim at the liver | IVC in long axis "for several centimeters" over a breath; hepatic veins by angling rightward and rocking superiorly (p24, p31) |
| SSN arch | pillow under the shoulders, head back, turned left; marker at 12 then 1 o'clock, toward "the plane that cuts through the right nipple and the tip of the left scapula" | — | ascending, transverse and descending aorta, innominate, LCC and LSA origins, RPA in cross-section (p31) |
| Abdominal aorta | from the subcostal IVC | "angle and slide slightly leftward" | aorta "more vertical than the IVC", no respiratory variation (p59) |

**Probe terms** (Mitchell p5, Figs 3–7): *tilt* (face fixed, the beam fans to other planes along the same axis), *sweep* (a
long clip through many planes), *rotate* (the marker turns about the beam axis), *slide* (move on the skin), *rock* (stay in
plane, toward or away from the marker), *angle* (same skin spot, the beam aimed at a new structure).

**Display convention.** The sector apex is at the top, with left-sided structures on the screen's right in A4C (p26). The
guideline was not found stating "marker at screen right" explicitly. That is the usual adult convention (**unverified**).

**Depth.** No numeric default is given. The guidance is to "maximize the size of the display" (p8), with about 1 cm beyond
the pericardium for PLAX after a deep scout view (p25).

### 4.2 Probe degrees of freedom (designed)

The probe state is `(s, ψ_tilt, ψ_rock, φ_rot, depth, sector)`.

* **s, slide.** A point on the figure's skin, moved by drag with the existing `pick_ray` and figure pick. The local frame is
  the skin normal n plus the head-to-foot tangent.
* **ψ_tilt (tilt or fan).** Rotation about the array axis, which moves the image plane perpendicular to itself.
* **ψ_rock (rock).** Rotation about the image-plane normal, which swings the sector within the plane.
* **φ_rot (rotate).** Rotation about the beam axis, which turns the marker. It is shown as a clock face on the chest.
* **Contact limit.** The beam stays within about 60° of −n (designed). Beyond that, contact is lost and the image is black.
* **Inputs.** Mouse drag on the figure to slide; drag handles, keys or sliders for the angles; an on-screen 3-D gizmo with the
  sector drawn in the heart, which the education evidence favours (§6). Optional: snap to an ICS from the real rib bake.

### 4.3 "View found" score, from geometry only (designed)

Each view is a target pose built from pack landmarks:

* the LV apex (`lv_apex`) and `rv_apex`;
* the centres of the four valve discs (connected components of `valve_planes`);
* the LV long axis (apex to mitral centre);
* the IVC lid and its axis, the aortic arch points (if the vessel bake has them), `laa`, the RV free wall and `rvot`;
* the papillary-muscle level (designed parts).

The target plane per view:

* **PLAX**: contains the long axis and the AV centre; the apex must be at or beyond the sector edge.
* **A4C**: contains the apex and the mitral and tricuspid centres.
* **A2C**: contains the long axis, perpendicular to the A4C plane rotated about 60°, and no RV landmark within 10 mm.
* **A3C**: contains the apex, the mitral centre and the AV centre.
* **A5C**: A4C tilted until the AV centre is in plane.
* **PSAX**: normal along the long axis, at the AV, MV-tips, papillary and apical levels.
* **Subcostal 4C**: four chambers from a subxiphoid apex.
* **IVC**: contains the IVC axis.

The score is a product of Gaussians, `S = Π exp(−(Δ_k/σ_k)²)`, over five terms:

* each key landmark's distance from the image plane (σ about 5 mm);
* each key landmark inside the sector (depth and angle), or for the PLAX apex required outside it;
* the angle between the in-plane projection of the marker and the target marker direction (σ about 15°);
* foreshortening: in apical views, the distance from the sector apex to the true LV apex direction, i.e. how well the plane
  passes through the apex;
* window quality: the fraction of rays to the key landmarks not shadowed by bone or lung, taken from the line model's
  transmission. This is the only image-derived term, and it is still geometric.

The score is computed in the pure crate from the same state, so it is testable without a GPU. All σ values are designed, and
the thresholds should be checked against a sonographer. A Nielsen 2022 style test (17 views, automatic score) is a natural
assessment format (§6).

**Acoustic windows: how far to model them** (inference):

* Ribs, sternum and lungs are essential. Finding the window *is* the psychomotor skill, and the KCL bake supplies all three.
* Model left lateral decubitus as a designed rigid shift and rotation of the heart and the left lung toward the chest wall. No
  source with numbers was reached; this is a maintainer decision. Otherwise use the supine anatomy and accept that apical
  windows are harder than in the clinic.
* Model breathing as lung-edge excursion with `RespiratoryClock`, so held inspiration helps the subcostal view.
* Skip costal-cartilage transparency and body-habitus variation in version 1.

## 5. M-mode, Doppler: feasibility

* **M-mode: yes.** It is one scan line of the same model sampled over time into a scrolling texture. PLAX M-mode at the MV tips
  or mid-LV shows wall motion and diameters, and TAPSE and MAPSE come from an apical M-mode through the annulus. It is honest
  in the sense that it shows exactly what the kinematic model does, and it gives the B29 calculators real measurements to
  feed on.
* **Tissue Doppler of the annulus: feasible.** S′, e′ and a′ are the time derivative of the prescribed annular displacement,
  so they are as honest as the motion model. TAPSE of 24 mm and S′ of 14.1 cm/s (Lang 2015) must be consistent: inference
  suggests they are with a systolic duration of about 300 ms, but this has to be checked.
* **Spectral and colour blood Doppler: leave out of version 1.** A transvalvular velocity could be derived from the chamber
  volume derivative divided by the valve area, by continuity. The E/A shape would then simply echo the designed volume curve,
  so it would teach nothing that is not hard-coded. Regurgitant and stenotic jets, proximal flow convergence and aliasing need
  a flow model. If offered later, label it schematic.
* **Strain and speckle tracking: leave out.** Strain and speckle tracking would be circular, because the strain is prescribed.

## 6. Education evidence

* **Neelankavil 2012**, Anesth Analg 115:1042, doi:10.1213/ANE.0b013e318265408f. An RCT with 61 residents on a TTE
  simulator. They acquired better images and identified anatomy better **on volunteers**: transfer.
* **Weimer 2025 (FateSim RCT)**, BMC Med Educ 25:21, doi:10.1186/s12909-024-06564-y. 128 students, simulator vs live
  models. Equal scores on the simulator, but worse on humans (59 vs 64, p < 0.01); larger knowledge gains. The authors advise
  combining the two.
* **Hempel 2020**, Ann Card Anaesth 23:293, doi:10.4103/aca.ACA_51_19. No difference between simulator and human models
  (n = 22).
* **Nielsen 2022**, Int J Med Educ 13:267, doi:10.5116/ijme.6321.8e5d. A VR TTE test across 17 views was reliable and
  discriminated experience levels; automatic scoring reduced the need for supervision. The authors note that "the evidence
  on the transfer of VR ultrasound skills to clinical performance is limited."
* **Jujo 2021**, Simul Healthc 16:341, doi:10.1097/SIH.0000000000000537. A meta-analysis of 9 TEE simulator RCTs: skill
  SMD 0.81, knowledge SMD 1.61, low certainty, no patient outcomes.
* **Zhao 2021**, BMC Med Educ 21:336, doi:10.1186/s12909-021-02753-1. TEE view interpretation; the simulator group retained
  better at 1 week and 1 month, though both groups decayed.
* **Weidenbach 2005**, JASE, doi:10.1016/j.echo.2004.10.025 (record not opened). It names orientation of the scan plane in
  the 3-D heart as the beginner's first problem and probe steering as the second. It shows the 3-D scene and the 2-D image
  side by side.
* **Salewski 2022**, BMC Med Educ, doi:10.1186/s12909-022-03242-9. 3-D printed hearts cut along the TTE planes improved
  spatial scores, most in weak visualisers.

**Implication** (inference): build for orientation and view recognition first, with a 3-D plane-in-heart display, the
landmark score, a guided tutorial and a self-test. Claim nothing about hand skills on patients. No dedicated TTE-simulation
systematic review was found.

## 7. Open data

| dataset | licence as read (quote) | verdict |
|---|---|---|
| Sunnybrook Cardiac Data (Cardiac Atlas Project) | "follows the Public Domain (CC0 1.0 Universal)" | **usable**: validate LV volume curves and short-axis wall motion |
| Rodero 2021 SSM average and extremes (Zenodo 4593739); 1000-mesh cohort (4506930) | "Creative Commons Attribution 4.0 International" | **usable**: geometry; `simulation_output.csv` for EDV checks; no motion |
| Strocchi 2020 (Zenodo 3890034) | "Creative Commons Attribution 4.0 International" | usable; end-diastolic only |
| KCL whole-torso models (Zenodo 7253863) | "Creative Commons Attribution 4.0 International" | **usable**: ribs, lungs, liver for the windows |
| TotalSegmentator dataset v2.0.1 (Zenodo 10047292) | "Creative Commons Attribution 4.0 International"; `total` task code and model "Openly available for any usage (Apache-2.0 license)" | usable reserve: individual ribs 1–12, sternum, costal cartilages, lung lobes, liver, heart (another subject, needs registration). Its `heartchambers_highres` task is **not** open ("free licenses available for non-commercial usage") |
| Wikimedia Commons clip from Ker 2009 (doi:10.1186/1476-7120-7-13), PLAX | "licensed under the Creative Commons Attribution 2.0 Generic license" | usable as a visual reference or attributed clip; other Cardiovascular Ultrasound supplements on Commons were not opened; CC BY-SA files are not on the allow-list |
| CAMUS / TED | the page states no licence; a mirror gives "CC BY-NC-SA 4.0 … strictly only non-commercial scientific research" (secondary) | **not usable** (NC) |
| EchoNet-Dynamic / LVH / Pediatric | "for personal, non-commercial research purposes only"; "must not … create derivative works" | **not usable** |
| MITEA (3-D echo) | "non-commercial license CC BY-NC-SA 4.0 for research purposes" | not usable |
| M&Ms | "for research purposes only. Its sale is prohibited." | not usable |
| CETUS | the confidentiality pledge "will not be given away or distributed" | not usable |
| ACDC; HMC-QU; Unity Imaging | no licence on the primary page; mirrors give CC BY-NC-SA 4.0, CC BY-NC-SA 3.0 IGO, CC BY-NC-ND 4.0 (secondary) | not usable |
| STRAUS / Alessandrini synthetic sequences | "public, so no login is required"; no licence | unclear, so not usable |
| Radiopaedia | licence page returned HTTP 402; snippets give a "modified … Non-commercial-Share Alike 3.0" licence | not usable |
| 4-D XCAT, UK Biobank, Imperial atlas | paid licence / restricted access / CC BY-NC-ND 4.0 | not usable |

**Caution** (inference, a decision for the maintainer): tuning a commercially distributed product against NC data may itself
be "commercial use". The safe course is to validate only against the published normal values, Sunnybrook (CC0) and the
CC BY clips.

## 8. Recommended design in phases

**Crate placement.** Put the motion, label sampling, line model and scoring in a **new pure crate, `heartsim-echo`**. It depends
on `core`, `geometry`, `sim` and `ecg`, because it needs `Beat`/`Components` from `pcg`. It compiles on wasm32 and does no I/O
and no GPU work. `CLAUDE.md`'s chain becomes `core → geometry → sim → ecg → echo → render → …`. The alternative is a `mech`
module inside `heartsim-ecg`; that is a maintainer decision.

The kinematics are a pure function of (event history, cell state, time), evaluated at the render time. Rendering interpolates
and never drives the simulation.

| phase | content | crates | main risks | acceptance tests (no GPU unless marked) |
|---|---|---|---|---|
| **E1 Beating heart** (about 4–5 days, Opus) | §2.3 model: per-cell contraction state from `activation_ms`/`apd_ms`, gating from `pcg::schedule` plus IVCT/IVRT, per-beat EF from RR, ischaemia scaling, respiration; displacement → hoods → vertices; writable vertex buffers and per-frame normals; LVEF read-out (F-25) | `heartsim-echo` (new), `heartsim-render` (`COPY_DST` positions), `heartsim-app` | self-intersection of the thin atria and RV; a non-physiological look; vertex-upload cost on web; picking on the rest mesh | sinus at 60/75/100 bpm: LV volume of the deformed closed shell gives EF 55–70 %; MAPSE 11–17 mm, TAPSE 17–28 mm, apical rotation −8 to −15°, base +3 to +7°; ejection start/end within 5 ms of the gated events; no ejection when RR < `min_ejecting_rr_ms`; LBBB: septal peak before lateral peak with a positive septal-flash index in IVC; AF: EF coefficient of variation > 0 and no atrial contraction; complete block: atrial contraction times follow P events; LAD stenosis lowers segment 7–13 excursion; determinism (same scenario and seed give bit-identical displacements); no triangle inversion; wasm32 check |
| **E2 Probe and slice** (about 4 days, Sonnet) | bake the label volumes (heart 1 mm with flood-filled blood, thorax 2 mm from KCL bones, lungs, liver), pack v3; designed leaflets, papillary muscles, moderator band, pericardium; probe pose on the figure (slide by pick; tilt, rock, rotate); sector in 3-D; flat-colour cross-section image (the fallback) | `xtask`, `heartsim-geometry` (volume type, loader), `heartsim-echo`, `heartsim-render` (scan-convert shader), `heartsim-ui`, `heartsim-app` | KCL tag numbering; overlap of KCL lungs and the Rodero heart; vessel caps for the flood fill; pack size | the label volume round-trips with its CRC; blood fraction of the LV box equals the shell's enclosed volume to ±5 %; a ray from the V2 site to the heart crosses no bone when it lies in an ICS and crosses bone over a rib (spot checks); the cross-section through the A4C target plane contains all four blood labels; web budget still passes; `webgl2_shaders.rs` passes on the new shader |
| **E3 Ultrasound appearance** (about 4–5 days, Opus) | line model: attenuation, Shams reflection plus Mattausch angle term, shadows, lung reverberation, enhancement, tissue-locked hashed speckle, polar PSF, TGC, gain, 60 dB log compression; per-label acoustic RON table with sources; depth, sector and frequency controls | `heartsim-echo`, `assets/`, `heartsim-render`, `heartsim-ui` | cost in wasm; looking "CG" rather than echo; tuning without shippable reference images | the polar image is deterministic (hash of a fixed pose and time); speckle envelope histogram in uniform myocardium fits Rayleigh (KS test) and point SNR ≈ 1.9 ± 0.2; mean blood brightness < 0.2× myocardium; shadow behind a rib (mean below −20 dB relative); brightness behind blood > behind tissue (enhancement); with TGC off, mean brightness falls ≈ 2·α·f dB/cm; speckle correlation between two frames with zero motion = 1, and falls with out-of-plane displacement; frame time < 15 ms in a release native bench (bench, not a gate); GPU smoke test only with lavapipe |
| **E4 Views, scoring, tutorial** (about 3–4 days, Sonnet) | targets for the 14 views of §4.1 from landmarks; score; guided tutorial (tutorial format in `docs/tutorials.md`) with hints ("tilt toward the right hip"); self-test quiz; 3-D plane-in-heart display; locale strings | `heartsim-echo` (targets, score), `heartsim-core` (tutorial checks), `heartsim-ui`, `heartsim-app`, `assets/tutorials`, `docs/book` | the targets' clinical validity; scores too strict or too lax; the decubitus question | for each view, the target pose scores > 0.9 and that pose ± 20° rotation scores < 0.5; PLAX with the apex in the sector scores lower than without; A2C scores < 0.5 when an RV landmark is in plane; the probe reaches each target from its window within the contact limits; the score is invariant to frame rate |
| **E5 M-mode and extras** (about 3 days, Sonnet) | M-mode lines; annular TDI (S′, e′, a′) from the kinematics; measurement calipers on B-mode and M-mode feeding the B29 calculators; optional pathology looks (AS/MS/MVP leaflets, dilated LV, pericardial effusion as a designed fluid shell) | `heartsim-echo`, `heartsim-ui`, `heartsim-app` | that "honest" holds only as far as the model does; clinician review of pathology looks | an M-mode of PLAX at the MV tips gives LVIDd/LVIDs within 2 mm of the deformed-mesh values; TAPSE measured on M-mode equals the prescribed value ± 1 mm; S′ peak equals the derivative of the annular curve; no blood Doppler UI exists |

## 9. Open questions and decisions for the maintainer

1. **New crate `heartsim-echo`** (and its place in the layering chain in `CLAUDE.md`), or a module in `heartsim-ecg`?
2. **Patient position**: model left lateral decubitus as a designed shift of the heart and left lung, or keep supine anatomy
   for every window? No numeric source was reached.
3. **Label-volume source for the thorax**: KCL torso 1 (same frame, already downloaded, one bone tag) or TotalSegmentator
   (individual ribs and cartilages, another subject, needs registration)?
4. **Designed parts** (leaflets, papillary muscles, moderator band, pericardium): accept designed geometry with literature
   dimensions? Leaflet lengths and opening and closing durations are still **unsourced**; find them before E2.
5. **NC data for tuning**: forbid it entirely (recommended), or allow viewing for qualitative look-and-feel only?
6. **Doppler**: confirm that version 1 has none. Is "schematic Doppler, labelled" wanted later?
7. **Hardware-probe input** (gamepad, phone IMU) is out of scope unless asked.
8. **Clinical review**: the view targets, the score σ values and every pathology look need a sonographer or cardiologist,
   added to `known-issues.md` as is done for the ECG.
9. **Patent**: the reported ETH patent US10565900B2 (Monte-Carlo ultrasound path tracing) was not read. The design avoids
   path tracing; get legal advice if Monte-Carlo rays are ever added.

**Not confirmed / not reached:**

* Full texts of Bürger 2013, Kutter 2009, Gao 2009 (COLE), Wein 2008, Reichl 2009, Salehi 2015, Wagner 1983, and Ng &
  Swanevelder 2011.
* Weissler 1968 (AHA 403; LVET and PEP only via secondary text).
* Rudski 2010 and Notomi 2005.
* Any source for leaflet kinematics or dimensions, E/diastasis/A durations, electromechanical delay, wall-thickening normal
  values, IVC collapsibility, and heart displacement in decubitus or with respiration.
* The licence pages of CAMUS, ACDC, HMC-QU, Unity and Radiopaedia (quoted from mirrors or snippets).
* The STRAUS page (403).
* How far the merged great-vessel bake extends the aorta (needed for the SSN and abdominal views); not checked in the code.
* The wall thickness of the Rodero average, which was never measured.
