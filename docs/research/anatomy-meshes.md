# Open 3-D meshes for the anatomy pack — sources, licences, recommendation

*Phase 1 of the real-anatomy bake (November 2026). Written before anything was downloaded. Every licence
below was read from the record page on the day of writing; the Zenodo file names, sizes and MD5s come from
the records' JSON API. Nothing in `epicardio/` or `epicardio-extracted/` was consulted.*

What the pack needs (`docs/anatomy-pack.md`, `procedural/heart.rs`): closed render surfaces for LV, RV, RA,
LA, aorta, pulmonary artery, SVC, IVC, the four pulmonary veins and the valve rings; a myocardial cell cloud
with region ids, transmural depth and fibres; AHA segments; conduction-system, coronary-sinus, flutter/CTI,
`la_pv_antrum` and catheter landmarks; a torso surface with the ten standard electrodes (plus V3R/V4R/V7–V9)
in the heart frame. The procedural pack is 9.5 MB; the web build has a 25 MiB budget for everything.

## 1. Heart meshes

| source | licence | what it contains | format / size | verdict |
|---|---|---|---|---|
| **Rodero et al. 2021, "Virtual cohort of extreme and average four-chamber heart meshes from statistical shape model"**, Zenodo 4593739 (v1.0.0, 2021-03-15) | **CC BY 4.0** (`cc-by-4.0` on the record) | 39 SSM-generated four-chamber hearts; `average.tar.gz` is the mean shape of 19 healthy adults (CT, end-diastole). 24 element tags: LV/RV/LA/RA myocardium, aorta and pulmonary-artery walls, mitral/tricuspid/aortic/pulmonary valve planes, LAA inlet+border, four pulmonary-vein inlets+borders, SVC and IVC inlets+borders. Rule-based ventricular fibres (80° endo → −60° epi) as `VECTORS`; UVC (ρ, φ, Z, V) as `POINT_DATA` on the ventricles (−10 elsewhere). ~1 mm tets, coordinates in mm | ASCII VTK `UNSTRUCTURED_GRID`; `average.tar.gz` 58 167 648 bytes, md5 `992f31e20c1aa73c10c5d9a6b6ac903a` | **recommended default** |
| Rodero et al. 2021, "Virtual cohort of adult healthy four-chamber heart meshes from CT images", Zenodo 4590294 | CC BY 4.0 | the 20 real CT cases the SSM was built from, same tags/fibres/UVC; case 20 has an unalignable LA | ASCII VTK; `01.tar.gz` 91 057 486 bytes, md5 `12df3f7f99fa8d2784d87f32ea98de71` | fallback if a real (not averaged) heart is preferred |
| Rodero et al. 2021, 1000 synthetic hearts, Zenodo 4506930 | CC BY 4.0 | same tags, **no fibres, no UVC** | 40 zips, 27.5 GB | not needed |
| Strocchi et al. 2020 four-chamber cohort, Zenodo 3890034 (v1, 2020-06-11) | **CC BY 4.0** | 24 heart-failure patients (CT); 24 tags as above plus an RV CRT electrode node label; fibres + sheets; UVC | EnSight `.case` coarse (1.1 mm), VTK fine (0.39 mm); 0.7–1.3 GB per case, 22.5 GB total | fallback only: HF hearts are dilated, archives are 15× larger |
| KCL "Whole Torso Computational Models", Zenodo 7253863 — the hearts inside the torsos | CC BY 4.0 | LV/RV walls with fibres, LA/RA walls, blood pools, SVC, aorta, left PA, four valves; no pulmonary veins, no IVC, no UVC | CARP `.pts/.elem/.lon` | use for the heart's **position** in the body (§3), not for the heart |
| Cardiac Atlas Project / Bai 2015 biventricular atlas; KIT biventricular SSM (Zenodo 4506463) | CC BY-NC-ND 4.0 / inherits NC | — | — | **rejected**: non-commercial |
| MedalCare-XL (Zenodo 7293655 / 8068944) | CC BY 4.0 | simulated ECGs only; the 13 anatomies are not released | — | not applicable |
| BodyParts3D heart parts (FMA 13884 wall of ventricle, 9457/9531 atrial walls, 49908–49916 pulmonary veins, 4720/10951 venae cavae, 4706 coronary sinus, 3736 ascending aorta, 8612 pulmonary trunk) | CC BY 4.0 (see §2) | surfaces of a single MRI male, 99 % polygon-reduced, no fibres/UVC, walls not separated from cavities in a way the cell sampler can use | OBJ | **not for the heart** (too coarse, no myocardial volume); its coronary sinus surface is a candidate for the CS *landmark path* only |
| Zygote, commercial anatomy packs | proprietary | — | — | rejected |

**Rodero "average" vs "case 01".** The average mesh is the textbook heart (the mean of 19 healthy adults, so
no individual's quirks), carries fibres and UVC, and is the smallest download. A real case is the same
format, so the bake must accept either; I propose baking the average and keeping case 01 as the comparison.

**What the Rodero mesh does not have, and what the bake adds (all heartsim's own work):**

* the veins and arteries are *rings* ("inlet + border" tags) on the atria, not tubes → extrude short tubes
  (≈ 25 mm) along each ring's normal: SVC and IVC from the RA rings, the four pulmonary veins from the LA rings,
  the aorta and pulmonary artery already have wall tags (short stubs) and are extended the same way;
* no LAA body (the paper excluded it because it left the CT field of view in some cases; only an inlet ring)
  → a short procedural appendage lobe on the LAA ring, the only procedural chamber part left;
* no coronary sinus → a cell polyline in the left AV groove from the CS ostium (RA, between the IVC ring and
  the tricuspid plane) along the posterior mitral annulus to the lateral LV, as today;
* no conduction system → landmarks from the UVC and the tags (SA node at the SVC ring – RA junction, AV node
  and His at the septal crest = RA cells nearest the base of the ventricular septum, bundle branches down the
  septal endocardium by UVC φ, LAF/LPF terminals at Durrer's paraseptal sites by UVC (φ, Z), PMJ patches by
  radius on the endocardium as today), CTI between the IVC ring and the tricuspid plane, the flutter circuit
  around the tricuspid plane, Bachmann's bundle on the anterior interatrial wall, `la_pv_antrum` on the
  posterior LA wall between the four PV rings;
* AHA segments and coronary territories from UVC (Z, φ) and the existing `territory.rs` rules;
* transmural depth = UVC ρ; the atria keep 0.5; atrial fibres stay rule-based (circumferential around each
  chamber's ring axis), as today.

## 2. Torso surfaces

| source | licence | what it contains | format / size | verdict |
|---|---|---|---|---|
| **KCL "Whole Torso Computational Models"** (Qian, Monaci, Mendonca-Costa, Campos, Gemmell, Zaidi, Rajani, Whitaker, Rinaldi, Bishop), Zenodo 7253863 (v1.0, 2022-10-26) | **CC BY 4.0** (`cc-by-4.0`) | five healthy patients' whole-torso tetrahedral models from high-resolution CT: tags for inner body, **skin (2)**, bones, kidneys, liver, stomach, spleen, lungs (8), SVC, left PA, aorta, valves, LA/RA walls and blood pools, LV/RV blood pools, **LV wall (35), RV wall (34)** with fibres, plus ICD leads/cans (tags 26–33, ignored) | CARP `.pts/.elem/.lon` in a zip; `KCL_torso1.zip` 167 823 369 bytes, md5 `23082fcd5649f39671e3d831121948cb` (others 128–191 MB) | **recommended**: real skin surface *and* the same subject's heart in the same frame |
| **BodyParts3D 4.0** (DBCLS), FMA 7163 "skin" | **CC BY 4.0** since the licence page of 2025-02-27 ("BodyParts3D, © The Database Center for Life Science licensed under CC Attribution 4.0 International"); earlier releases and third-party copies still say CC BY-SA 2.1 JP | whole-body skin of one adult male (1 mm MRI), 99 % polygon-reduced; heart parts in the same frame | OBJ; `isa_BP3D_4.0_obj_99.zip` 136 MB (all ~2 000 parts) | **second choice**: a whole body with arms and head reads well in the 3-D view, but the skin is coarse and the download page still links the old BY-SA summary — the licence page is authoritative but must be quoted in the pack `LICENSE` with its date |
| Buttmann & Lehrmann 2026, "ECGI via Posterior Sampling under Operator Discrepancy: Datasets", Zenodo 23449575 | CC BY 4.0 (torsos derived from the TotalSegmentator v2.0.1 CT dataset, Zenodo 10047292, CC BY 4.0; hearts from Rodero 4506930) | 139 CT torso surfaces with lungs, electrode coordinates and placements of Rodero-family hearts, LPS mm | custom NumPy/JSON packages, loaders in a Python repo (MIT); `native-ecgi-v1-geometry.tar` 1.0 GB | **viable but heavier**: a second-generation derivative with an undocumented container format; keep as reserve |
| Bender, Stoks, Barrios Espinosa, Becker, Cluitmans, Loewe 2026, WCT dataset, Zenodo 20086105 | CC BY 4.0 on the record, **but** the torsos are instances of the MPII Human Shape body SSM, whose page says "freely available for non-commercial and educational purposes only" (alongside a "Simplified BSD" link) | 25 torso surfaces (VTK) with RA/LA/LL/RL/V1–V6 coordinates, 1.9 MB | VTK + CSV | **rejected**: upstream non-commercial statement; the record's CC BY cannot override it |
| Moss, Wuelfers, Seemann (Freiburg/KIT) "Tetrahedral meshes of male human torso and heart", Zenodo 8285403 | **EUPL 1.2** | torso + heart volume meshes, 6.5 GB | VTU | **rejected**: EUPL is not in the allowed list (copyleft) |
| Grychtol 2024 "Torso Organ Meshes", Zenodo 11047831 | **CC BY-SA 2.1 JP** | BodyParts3D-derived lungs, heart, vessels (no skin) | MSH/OBJ/STL | **rejected**: share-alike |
| Höhne et al. 2025 "Segmented Internal Organs of the Visible Human Male", Zenodo 15882019 | CC BY 4.0 | 1 mm voxel label volume, > 200 objects, 774 slices | TIFF stacks, 395 MB | not needed (voxel data; the KCL skin is already a mesh); reserve |
| ECGSIM thorax/heart data | "Creative Commons GNU general public license" on ecgsim.org, data terms not stated | thorax, lungs, ventricles, 64 electrodes | binary | **rejected**: unclear / GPL |
| SCI Utah torso (Johnson, MacLeod, Ershler 1992), EDGAR | no licence stated on the archived dataset pages | MRI male torso surfaces, tank geometries | TriSurf | **rejected**: no terms |
| Dalhousie torso (Horáček) | distributed inside other packages, no standalone terms | — | — | **rejected**: unclear |
| openCARP experiments torso example | repository blocked by an access wall on the day of writing; openCARP core is the Academic Public License | — | — | **not verified**; unnecessary given the KCL set |

**Recommendation for the torso: KCL torso 1** (or whichever of the five has the most typical build; the
bake prints each subject's chest width/depth and the heart's long-axis angles so the choice is documented).
BodyParts3D's skin is the alternative if a full body with arms and head is wanted for the 3-D view; its
licence is CC BY 4.0 today, but because older copies carry BY-SA text the pack `LICENSE` must cite the
licence page and its date.

**No shippable torso at all?** Not the case any more: two CC BY sources exist. The procedural superellipsoid
stays in the code as the fallback behind the existing `AnatomyPack::procedural` path.

## 3. Posing the heart in the torso

* **Heart frame.** The Rodero mesh is in mm in the CT/SSM reference frame (the cases were rigidly aligned
  before the shape model; the paper does not state the reference frame). The bake does not trust it: it
  measures the LV long axis from the UVC (centroid of the apical cells, Z ≈ 0, to the centroid of the basal
  LV cells, Z ≈ 1) and the septal direction from the UVC V coordinate (LV vs RV side, φ near the septum), and
  rotates them onto `APEX_DIRECTION` (frontal 36°, transverse 46°) and `SEPTAL_DIRECTION` of
  `procedural/heart.rs`, then translates the ventricular centroid to the origin. So the measured pose is kept
  exactly, independent of the dataset's frame.
* **Position in the chest.** Today's "65 mm left of the midline, 90 mm behind the anterior wall" are chosen
  numbers. The KCL torso has the subject's own LV and RV walls in the skin's frame, so the bake computes that
  subject's ventricular centroid, long-axis angles (reported against Engblom's 38° ± 10° / 46° ± 7°) and the
  distances to the skin, and places the Rodero heart's centroid where the KCL heart's is. The KCL heart's
  own pose is reported but not used (the measured population angles stay the pose; one subject is one sample).
  The Rodero average heart and a KCL subject are different people, so the fit is checked for
  intersections with the lungs (tag 8) and skin, and the heart is scaled only if it intersects (documented
  if so).
* **Electrodes.** As today: each site is a ray from the torso axis at a height and azimuth, intersected with
  the skin surface, so every electrode lies exactly on the mesh. Heights stay relative to the ventricular
  centroid (V1/V2 15 mm above, V4–V6 15 mm below, Mason–Likar limb sites on the shoulders and hips). The
  KCL torso covers shoulders to pelvis, which is enough for the Mason–Likar sites; V7–V9 and V3R/V4R follow.
  The resulting nearest-cell distances V1…V6 are printed by the bake and compared with the 68/42/32/38/34/43 mm
  the procedural pack needed (`forward.rs`): the real chest is closer under V1, which is expected to
  change the R-wave progression thresholds — the tests are re-tuned and the change documented next to them.
* **Scale.** Rodero hearts are adult size (LV epicardial long axis ≈ 90 mm), larger than the procedural heart
  (80 mm cavity axis) that the doc calls "small for its chest"; with a real skin from a real adult the two
  now match without the 65 mm fudge.

## 4. Attribution text required by the licences

CC BY 4.0 requires: creator names, a copyright/licence notice, the licence link, a link to the material, and
an indication of changes. The pack `LICENSE` and `ASSETS-LICENSES.md` will carry:

> Heart: Rodero C, Strocchi M, Marciniak M, Longobardi S, Whitaker J, O'Neill MD, Gillette K, Augustin C,
> Plank G, Vigmond EJ, Lamata P, Niederer SA. *Virtual cohort of extreme and average four-chamber heart
> meshes from statistical shape model* (v1.0.0), Zenodo, 2021. doi:10.5281/zenodo.4593739. Licence CC BY 4.0
> (https://creativecommons.org/licenses/by/4.0/). Paper: Rodero C et al. Linking statistical shape models and
> simulated function in the healthy adult human heart. *PLoS Comput Biol* 2021;17(4):e1008851,
> doi:10.1371/journal.pcbi.1008851. Modified by heartsim: the `average` mesh was re-oriented to the measured
> long-axis pose, re-centred, its tagged boundary surfaces extracted and decimated, its myocardium resampled
> to a 2.3 mm cell cloud, and vessel tubes, an appendage lobe, a coronary sinus path, conduction landmarks and
> AHA segments were added.

> Torso: Qian S, Monaci S, Mendonca-Costa C, Campos F, Gemmell P, Zaidi S, Rajani R, Whitaker J, Rinaldi CA,
> Bishop MJ. *Whole Torso Computational Models* (v1.0), King's College London, Zenodo, 2022.
> doi:10.5281/zenodo.7253863. Licence CC BY 4.0. Modified by heartsim: the skin surface of torso 1 was
> extracted from the tetrahedral model, decimated and translated into the heart frame; all other tags were
> discarded.

(If BodyParts3D is chosen instead: "BodyParts3D, © The Database Center for Life Science licensed under CC
Attribution 4.0 International", https://dbarchive.biosciencedbc.jp/en/bodyparts3d/lic.html (2025-02-27),
Mitsuhashi N et al. *Nucleic Acids Res* 2009;37:D782, FMA 7163 "skin", release 4.0, decimated and re-posed.)

## 5. Plan for Phase 2 (after the sources are agreed)

1. `cargo xtask bake-anatomy --source assets/anatomy-src/<name>` gains a `fetch` step: download
   `average.tar.gz` (58 MB) and `KCL_torso1.zip` (168 MB) into the git-ignored `assets/anatomy-src/`, verify
   the MD5s above, unpack. ≈ 230 MB total, nothing vendored.
2. Readers: legacy ASCII VTK unstructured grid (the existing `bridge::vtk` handles XML VTU only), CARP
   `.pts/.elem/.lon`.
3. Stages as in `xtask/src/bake.rs` `STAGES`: frame (UVC-based pose, §3) → surfaces (tagged boundary faces
   per chamber, closed with the ring/valve planes, quadric decimation to ≈ 15 k vertices, vessel tubes
   extruded from the rings) → cells (2.3 mm sampling inside the myocardial tets, depth = ρ, fibres from the
   mesh) → regions (tags + UVC → `Region`, atrial sub-regions by position relative to the rings) → segments
   (UVC → AHA) → conduction landmarks → torso (KCL skin extraction, electrodes) → hoods → write, with the
   attribution above in `LICENSE`. Every stage deterministic; the pack is committed, the sources are not.
4. `PACK_FORMAT_VERSION` stays 2 unless a file changes meaning (the LAA lobe and the vessel tubes fit the
   existing mesh table).
5. Re-tune `forward.rs`, `tier1.rs`, `anatomy_pack.rs`, the EP-lab tests; golden snapshots untouched.
6. Screenshots, `cargo xtask ci`, wasm check, web budget, docs, CHANGELOG.

## 6. Open questions for the user

1. Heart: Rodero **average** (recommended) or real **case 01**?
2. Torso: **KCL torso 1** (recommended; chest only, CT, same-subject heart position) or **BodyParts3D skin**
   (whole body, coarser, licence page newer than the files)?
3. The parent session is releasing 0.7.0 on `master` with uncommitted changes in this checkout; Phase 2 must
   go on its own branch, so it should run in a worktree or wait until the release is committed.
