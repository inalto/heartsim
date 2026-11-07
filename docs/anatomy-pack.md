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
| ventricular centroid from the midline | 65 mm left | **chosen, not measured**: puts the apex ≈ 95 mm from the midline under V4 (bedside rule: within 10 cm); larger than in a patient because the procedural heart is small |
| anterior chest wall from the centroid | ≈ 90 mm | **chosen, not measured** |
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
| `mesh_<name>.glb` | render mesh of one structure (`lv`, `rv`, `ra`, `la`, `aorta`, `pulmonary_artery`, `svc`, `ivc`, `pulmonary_veins`, `valve_rings`); kind `Myocardium` / `Vessel` / `Valve` in the manifest | glTF 2.0 binary, one primitive, `POSITION` + `NORMAL` f32, `u32` indices |
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

Procedural, generated by `heartsim_geometry::procedural::idealised_heart` — two truncated ellipsoidal ventricles
(LV wall 10 mm, RV wall 4 mm, the LV wall inside the RV ellipsoid is the septum), two spherical atria whose
overlap is the interatrial septum, an LA appendage, great-vessel stubs and valve rings. Cells on a 2.3 mm lattice
(≈ 17.5 k), helix fibres +60° (endo) → −60° (epi), conduction system and coronary sinus as explicit cell
polylines, atrial sub-regions by position, AHA segments by apex–base/azimuth coordinates, and the whole heart
rotated into the thorax as tabulated above. The torso is a superellipsoid with the AHA/ACC standard precordial
positions and Mason–Likar limb sites. Everything is
heartsim's own work (MIT OR Apache-2.0); see the pack's `LICENSE` and [`../ASSETS-LICENSES.md`](../ASSETS-LICENSES.md).

Regenerate after changing the generator (the test `default_pack_loads_fast_and_matches_the_generator` fails
when the committed pack is stale):

```bash
cargo xtask bake-anatomy
```

Options: `--out <dir>`, `--name <name>`, `--voxel-mm` (render mesh voxel, default 2.5), `--spacing-mm` (cell
lattice, default 2.3). The command re-loads what it wrote and reports the load time (budget: < 500 ms).

## The real-mesh path (`--source <dir>`)

Target dataset: the **Strocchi et al. 2020 four-chamber cohort** (CC BY 4.0) or the **Rodero et al. 2021 healthy
cohort** built from it (CC BY 4.0) — both have chamber/vessel tags, ventricular fibres and UVCs. Do **not** vendor
the raw data (22.5 GB; 0.7–1.3 GB per heart): download one case locally, bake, commit only the pack.

1. Pick a case. Rodero 2021 "average" healthy heart (case 01 of the 20) is the intended default; the Strocchi
   cohort are heart-failure patients and are the fallback.
2. Download from Zenodo into an empty directory (replace the file names with the ones listed on the record):
   ```bash
   mkdir -p ~/anatomy-src/rodero01 && cd ~/anatomy-src/rodero01
   curl -L -O "https://zenodo.org/records/4590294/files/<case-01-archive>?download=1"
   ```
   Records: Rodero 2021 — <https://zenodo.org/records/4590294> and <https://zenodo.org/records/4593739>
   (CC BY 4.0; CARP `.pts/.elem/.lon` + tags). Strocchi 2020 — <https://zenodo.org/records/3890034>
   (CC BY 4.0; EnSight `.case` coarse, VTK fine).
3. Bake: `cargo xtask bake-anatomy --source ~/anatomy-src/rodero01 --out assets/anatomy/rodero01 --name rodero01`.
   The command detects the format (CARP / VTK / EnSight / glTF) and prints the stage plan; stages marked
   `todo` are not implemented yet and the command exits non-zero so a half-baked pack is never written.
4. Attribution (CC BY 4.0 requires it in the shipped pack's `LICENSE` and in `ASSETS-LICENSES.md`):
   > Rodero C, Strocchi M, Marciniak M, et al. Linking statistical shape models and simulated function in the
   > healthy adult human heart. *PLoS Comput Biol* 2021;17(4):e1008851. doi:10.1371/journal.pcbi.1008851.
   > Data: Zenodo, doi:10.5281/zenodo.4590294, CC BY 4.0. Modified: re-centred, decimated, resampled to cells.

   > Strocchi M, Augustin CM, Gsell MAF, et al. A publicly available virtual cohort of four-chamber heart meshes
   > for cardiac electro-mechanics simulations. *PLoS ONE* 2020;15(6):e0235145. doi:10.1371/journal.pone.0235145.
   > Data: Zenodo, doi:10.5281/zenodo.3890034, CC BY 4.0.

Pipeline stages (`xtask/src/bake.rs`, `STAGES`): read → frame → surfaces (decimate to ~15 k vertices per
chamber) → cells (2.5 mm sampling; UVC apex–base and transmural coordinates from Laplace solves with `sprs` CG)
→ fibres (dataset fibres, else Bayer LDRB α 40°/−50°, β −65°/25°) → regions (tags → `Region`, AHA segments,
territories) → conduction (landmarks; Costabal fractal tree → PMJ seeds) → hoods → write. `hoods` and `write`
are shared with the procedural path and done; `read` is done for glTF surfaces only.

Torso: the procedural superellipsoid is the default; BodyParts3D (CC BY-SA 2.1 JP, share-alike applies to the
derived mesh) is the candidate for a realistic body surface — see
[`research/electrophysiology-and-ecg-models.md`](research/electrophysiology-and-ecg-models.md) §5.1.

## Legal

Nothing in any pack may come from `epicardio/` or `epicardio-extracted/` (proprietary, git-ignored, study only).
Only CC BY / CC BY-SA (noted) / CC0 / MIT / BSD / Apache material ships; every pack carries its `LICENSE` and a
row in `ASSETS-LICENSES.md`.
