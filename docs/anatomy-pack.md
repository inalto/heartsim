# Anatomy packs

An **anatomy pack** is the directory the engines and the renderer load at start-up: render meshes, the
simulation cell cloud with its neighbour graph and fibres, vertex→cell hoods, conduction-system landmarks,
Purkinje seed sets, the torso and the electrode sites. `heartsim_geometry::AnatomyPack` is the API;
`cargo xtask bake-anatomy` builds packs; `heartsim-cli anatomy-info <dir>` inspects them. Brief:
[`briefs/B02-anatomy-pipeline.md`](briefs/B02-anatomy-pipeline.md).

## Coordinate frame

Millimetres. Origin = centroid of the ventricular cells. **+x = patient's left, +y = cranial,
+z = anterior.** Electrodes are stored in the same frame, so the forward model needs no further transform.

Sources of the heart's position in the default pack (`procedural/heart.rs`, `procedural/torso.rs`):

| quantity | value | source |
|---|---|---|
| LV long axis, frontal plane (0° = patient's left, + towards the feet) | 36° | CMR, 94 healthy adults: +38° ± 10°, range 16–66° (Engblom et al., *Am Heart J* 2005;150:507, doi:10.1016/j.ahj.2004.10.041); UK Biobank 39° ± 12° (Kayyali et al., *PLoS Comput Biol* 2025;21:e1013161, mitral-valve definition) |
| LV long axis, transverse plane (0° = patient's left, + towards the front) | 46° | same: +46° ± 7°, range 29–70°; UK Biobank 49° ± 9° |
| septal normal in the horizontal plane | right and anterior, perpendicular to the long axis | septum on edge in LAO 53° ± 9° on CT (Narumi et al., *Open Heart* 2022;9:e002009) |
| cranial tilt of the septal normal | 15° | **no direct measurement found**; chosen from the forward model's response, inside the range implied by the axial angiographic views (Bargeron et al., *Circulation* 1977;56:1075; the view angles are quoted from memory) |
| ventricular centroid from the midline | 53 mm left | measured: the KCL CT torso 1 subject's own ventricular centroid, after the torso is scaled ×0.869 to the baked heart's LV length (the procedural pack chose 65 mm) |
| anterior chest wall from the centroid | 84 mm | measured on the same subject (the procedural pack chose ≈ 90 mm); the chest is 302 mm wide and 237 mm deep at heart level, and V1…V6 are 54 / 36 / 33 / 30 / 41 / 57 mm from the nearest heart cell |
| atria and pulmonary veins | LA centre 11 mm above the ventricular centroid, veins leaving backwards and outwards near the horizontal, SA node at the SVC–RA junction 38 mm above the centroid | **chosen, not measured**, to the qualitative anatomy of an axial chest CT: all four chambers share a slice through the mid ventricles and the veins enter the LA at or above it. The atria stay centred 1–3 cm further left than a patient's because the chambers are spheres on the ventricular base; P axis +33°. The AV node and His bundle stay at the septal crest, about 13 mm from ventricular muscle at the His catheter tip |
| chest width × depth | 290 × 240 mm | **chosen, not measured**; an adult width-to-depth ratio |
| LAF and LPF terminals | anterior paraseptal (95°, y = −5 mm); posterior paraseptal, a third of the way from apex to base (245°, y = −25 mm) | breakthrough sites of Durrer et al., *Circulation* 1970;41:899 |

| LAD's zone of the RV free wall | 37 % of its cells: anterior half, apical half | the LAD perfuses almost 30 % of the anterior RV wall, about 15 % of its posterior wall, and much of its apex (Novo et al., *Curr Probl Cardiol* 2022;47:101277, doi:10.1016/j.cpcardiol.2022.101277, summarising anatomic and post-mortem studies); the zone's limits were **chosen** so that LAD and RCA occlusions give their textbook leads |

The nearest heart cell is 68, 42, 32, 38, 34 and 43 mm from V1…V6. These distances are what R-wave progression needs in the
infinite-medium forward model; they are not measured values. See the doc comments of `r_wave_amplitude_progression` and
`fascicular_block_axes` in `crates/heartsim-ecg/tests/forward.rs` for what the geometry does and does not achieve.

## Files

| file | content | encoding |
|---|---|---|
| `manifest.ron` | `PackManifest`: `format_version`, name, provenance (`source`), licence, frame, cell spacing, cell count, mesh table, and for every other file its size and CRC-32 | RON |
| `LICENSE` | licence and attribution of the geometry in this pack | text |
| `mesh_<name>.glb` | render mesh of one structure (`lv`, `rv`, `ra`, `la`, `aorta`, `pulmonary_artery`, `svc`, `ivc`, `pulmonary_veins`, `valve_planes`; the procedural pack has `valve_rings` instead); kind `Myocardium` / `Vessel` / `Valve` in the manifest | glTF 2.0 binary, one primitive, `POSITION` + `NORMAL` f32, `u32` indices |
| `torso.glb` | body surface | glTF 2.0 binary |
| `cells.bin` | `HSCL` v1: `n`, positions `n×3 f32`, fibres `n×3 f32` (unit), region codes `n×u8` (index into `Region::ALL`), transmural depth `n×f32` (0 endo → 1 epi; atria 0.5; septum measured from the LV side) | little-endian |
| `neighbours.bin` | `HSNB` v1: `n`, `k = 8`, then per cell `k × (index u32, distance_mm f32, fibre_alignment f32)` nearest first | little-endian |
| `segments.bin` | `HSSG` v1: `n`, AHA 17-segment index `u8` per cell (0 = not in the LV model; 18 on the RV free wall = the LAD's zone); coronary territory = `Territory::for_cell(region, segment)` | little-endian |
| `hoods/<name>.bin` | `HSHD` v1: vertex count, `k = 8`, per vertex `k × u32` cell indices then `k × f32` inverse-distance weights (sum 1) — one file per `Myocardium` mesh | little-endian |
| `landmarks.ron` | `Landmarks { points: name → cell, sets: name → [cell] }`; points include `sa_node, av_node, his, rbb, lbb, laf, lpf, rbb_terminal, rv_apex, rvot, rv_free_wall, lv_apex, lv_lateral, lv_posterior, lv_anterior, lv_septal, cs_os, cs_distal, bachmann, ra_lateral, la_roof, laa, la_pv_antrum`; sets `rbb`, `laf`, `lpf` are the PMJ seed cells per fascicle; optional pacing-lead points `raa` (RA appendage, else `ra_lateral`) and `rv_septum` (else the RV cell nearest `lv_septal`), see `LeadSite::landmarks` | RON |
| `electrodes.ron` | `ElectrodeSet`: `RA LA LL RL V1…V6` plus `V3R V4R V7 V8 V9`, heart frame | RON |

Loading verifies the format version, each file's size and CRC-32, and the cross-file counts (cells ↔ neighbours
↔ segments, hoods ↔ mesh vertices, landmark indices in range). A pack that loads is internally consistent.
`PACK_FORMAT_VERSION` is bumped when a file is added or changes meaning; the per-file binary version covers the
record layout.

## The default pack (`assets/anatomy/default/`)

Baked from two open, CC BY 4.0 meshes (sources, licences and the rejected alternatives:
[`research/anatomy-meshes.md`](research/anatomy-meshes.md)):

* **Heart** — Rodero et al. 2021, the *average* four-chamber mesh of the statistical shape model built from 19 healthy
  adults' CT (Zenodo [4593739](https://zenodo.org/records/4593739), `average.tar.gz`, 58 MB; 379 158 nodes, 1 766 006
  tetrahedra, 24 tags, rule-based ventricular fibres 80° endo → −60° epi, universal ventricular coordinates ρ φ Z V).
* **Torso** — Qian, Bishop et al. 2022, KCL whole-torso CT model 1 of five healthy patients (Zenodo
  [7253863](https://zenodo.org/records/7253863), `KCL_torso1.zip`, 168 MB; 1 689 018 nodes, 9 686 213 tetrahedra, skin,
  bones, lungs, organs, the subject's own heart chambers and great vessels).

```bash
cargo xtask bake-anatomy            # downloads into assets/anatomy-src/ (git-ignored), verifies MD5s, bakes, re-loads
cargo xtask bake-anatomy --offline  # never downloads
cargo xtask bake-anatomy --procedural --out /tmp/proc   # the idealised heart, still the fast test fixture
```

What the bake does (`xtask/src/bake/`): `fetch` (curl, MD5 from the Zenodo record, tar.gz / zip) → `tet` (legacy VTK and
openCARP readers) → `heart` → `torso` → pack assembly → `AnatomyPack::save` → re-load and verify. Everything is deterministic:
the same sources give the same bytes.

### The heart (`bake/heart.rs`)

1. **Tags** are the Strocchi/Rodero table, verified by adjacency before anything else (the mitral plane must touch LV and LA,
   the aortic plane LV and aorta, every inlet its border tag and its atrium) and by position (SVC above IVC, appendage anterior
   to the left veins, right veins on the right): 1 LV, 2 RV, 3 LA, 4 RA, 5 aorta, 6 pulmonary artery, 7–10 mitral / tricuspid /
   aortic / pulmonary planes, 11 LAA inlet, 12–15 LSPV / LIPV / RIPV / RSPV inlets, 16 SVC, 17 IVC, 18–24 the atrial-wall
   border of each inlet.
2. **Frame.** The file is in DICOM LPS. The LV long axis is measured from the UVC (centroid of the apical nodes Z < 0.03 to
   the basal nodes Z > 0.97) and the septal direction from the nodes shared by the LV and RV tags; those two directions are
   rotated onto the *canonical* frame of `procedural::heart` (long axis −y, septum at azimuth 160°), every construction below
   uses the procedural conventions, and `heart_rotation()` tilts the result into the thorax — so the pose is the measured one
   (36° / 46°, septum right-anterior) regardless of the dataset's frame. The dataset's own long axis is reported by the bake.
3. **Cells** are the points of a regular 2.3 mm lattice that fall inside a tetrahedron of the four myocardial tags (a
   point-in-tetrahedron test over a spatial hash; the atria, about as thin as the lattice, also take the points within
   0.5 mm of one of their tetrahedra so that the wall stays connected): each cell inherits its tag, the element fibre, ρ as
   transmural depth (septum from the LV side, RV side = 1) and Z. The lattice keeps the neighbour graph as even as the
   procedural pack's; claiming every voxel a wall touches would have doubled the thin right ventricle's mass, and a second
   atrial cell layer made the P wave 0.6 mV. The pack has 19 596 myocardial cells (LV 7284, septum 3871, RV 4288, LA 2117,
   RA 2036) plus 455 conduction-system, sinus and appendage cells. Septal cells are LV-tag cells whose φ lies in the range of
   the LV/RV shared nodes and that lie within 14 mm of RV tissue. Atrial fibres are circumferential around the chamber's
   valve-to-roof axis; the atrial working myocardium conducts isotropically (`Parameters`), because on a one-cell wall the
   eikonal engine and the monodomain grid disagree about anisotropic conduction.
4. **Regions and landmarks** are built on the real surfaces with the procedural rules: the SA node on the RA wall lateral to
   the SVC inlet, the crista terminalis from there to the IVC along the lateral wall, the cavotricuspid isthmus between the IVC
   inlet and the tricuspid plane, the AV node on the RA wall just above the septal tricuspid rim with its fast and slow
   inputs, the His bundle to the crest of the septum, the left bundle down the septal endocardium, the fascicles to Durrer's
   paraseptal sites at the same fractional heights as before, the right bundle down the RV side of the septum to the anterior
   free wall, the coronary sinus just outside the posterior mitral annulus from its ostium to the lateral wall, Bachmann's
   bundle across the anterior interatrial groove, the pulmonary-vein sleeves around each inlet, the atrial septum where the two
   atria share a wall, and a thin-walled appendage lobe on the LAA inlet (the dataset crops the appendage). PMJ seed sets are
   12 mm patches at the fascicle terminals and at the right bundle's end a quarter of the way from the RV apex to the
   pulmonary valve (the procedural pack's 25 mm patches lit most of the real LV endocardium at once).
5. **AHA segments** from the canonical coordinates (apex–base between the epicardial apex and the mitral plane, angle in the
   short-axis plane), the RV's LAD zone by the same rule as before.
6. **Render meshes**: only the dataset's own parts, nothing designed. Each is the boundary faces of its tags (closed by
   construction), Taubin-smoothed and quadric-decimated to 30 000 triangles per chamber and 6 000 per other part: `la` and
   `ra` are the atrial tags plus their inlet border rings (tags 18–24) and `la` also the appendage inlet lid; `aorta` and
   `pulmonary_artery` are the wall tags, cropped where the dataset crops them; `svc`, `ivc` and `pulmonary_veins` are the
   inlet lids (tags 16, 17, 12–15) that close each vessel mouth; `valve_planes` are the four valve-plane discs (tags 7–10).
   The appendage lobe of the cell cloud has no render mesh: the dataset crops the appendage at its inlet.

### What the engines do differently on it

Documented next to each value: the fast endocardial layer takes the RV free wall to depth 0.5 (its wall is 1.5 cells thick)
and leaves out only the infundibulum, 15 mm around `rvot`, instead of an apex–base limit (`EikonalParams`); the atrial
CV multiplier is 1.25 (`atrial_cv_scale`, P ≈ 105 ms); the coronary-sinus sleeve conducts at 1.2 m/s along its fibres; the
Tier-2 grid stamps atrial cells with a 4 mm radius (`Tier2Params::atrial_reach_h`) so the one-cell wall becomes a slab
that conducts at the calibrated velocity, and a lesion kills the whole voxel column of every cell it reaches; the forward
gain is 4.5 mV. The EP flutter lesson draws its bands on the real right atrium (centre, axis and polar angles printed by
the bake) and makes the left atrium, Bachmann's bundle, the coronary sinus and the septal strip above the barrier inert.
The persistent-AF presets give the AV node a beat-to-beat scatter of its concealed penetration (`AvParams::concealment_spread`
0.25 around 0.75, drawn from the scenario seed: ≈ 100 bpm with an RR coefficient of variation ≈ 0.2): the organised atrial wave on this grid arrives at a constant cycle, and a
deterministic node fed a constant cycle settles into a fixed conduction ratio and a regular ventricular response, which is not
AF. The scatter stands in for the changing penetration of fibrillatory wavefronts (Cohen, Berger & Dushane 1983; Lian, Müssig
& Lang 2006) and is 0 everywhere else, so flutter keeps its fixed ratios.

### The torso (`bake/torso.rs`)

The outer surface of the skin tag (its boundary faces not shared with inner tissue), closed at the CT's cut planes, smoothed,
decimated to 12 000 triangles, and translated so the **subject's own ventricular centroid** (LV + RV wall tags) is the origin
and rotated from LPS into the heart frame. So the heart sits where this subject's heart was: see the table in *Coordinate
frame* for the measured offsets. Electrodes are placed as before (`procedural::torso::ELECTRODE_SITES`): a ray from the
torso axis at the site's height and azimuth, intersected with the skin; the limb sites are clamped inside the CT's vertical
extent (shoulders to pelvis).

### The procedural pack (`--procedural`)

`heartsim_geometry::procedural::idealised_heart` — two truncated ellipsoidal ventricles (LV wall 10 mm, RV wall 4 mm, the LV
wall inside the RV ellipsoid is the septum), two spherical atria whose overlap is the interatrial septum, an LA appendage,
great-vessel stubs and valve rings, on a 2.3 mm lattice, with the same landmarks, regions and pose. It was the default pack
until 0.7 and remains the fast fixture of the geometry tests (`AnatomyPack::procedural`). Everything in it is heartsim's own
work (MIT OR Apache-2.0).

## Legal

Nothing in any pack may come from `epicardio/` or `epicardio-extracted/` (proprietary, git-ignored, study only).
Only CC BY / CC0 / MIT / BSD / Apache material ships; every pack carries its `LICENSE` with the attribution its sources
require (creators, record DOI, licence link, changes made) and a row in `ASSETS-LICENSES.md`. The downloaded sources stay in
the git-ignored `assets/anatomy-src/`; only the baked pack is committed.
