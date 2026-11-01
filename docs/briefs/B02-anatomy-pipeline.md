# B2 — Anatomy pipeline: open mesh → anatomy pack
**Crates** `heartsim-geometry`, `xtask` (`bake-anatomy`) · **Model** Opus · **Milestone** M2 · **Depends on** nothing.

## Goal
Produce `assets/anatomy/default/` — an openly licensed four-chamber heart (+ torso with electrode sites) in the formats
`docs/ARCHITECTURE.md` §7 defines, and the loader/API in `heartsim-geometry` that the engines and renderer consume. Until the
real mesh is processed, ship a **procedural idealised heart** (two ellipsoidal ventricles with a septum, two atria, great-vessel
stubs) so every other brief can proceed.

## Read first
`docs/research/electrophysiology-and-ecg-models.md` §5.1 (meshes + licences: Strocchi/Rodero cohort CC BY 4.0, Fritz/Gerach,
BodyParts3D CC BY-SA), §5.3 (Bayer LDRB fibre rule, default angles α 40°/−50°), §5.4 (Costabal fractal Purkinje tree);
`docs/reverse-engineering/EPICARDIO-ANALYSIS.md` §3 (what a working cell cloud looks like: ~4 k atrial + ~10 k ventricular
cells, 8-NN with distance and fibre cosine, transmural depth, vertex→cell hoods, landmarks, electrode frame); `crates/heartsim-geometry`.

## Scope
1. **Coordinate frame**: mm, +x patient left, +y cranial, +z anterior, origin at the ventricular centroid. Document in the pack.
2. **Procedural heart** (`geometry::procedural::idealised_heart(resolution)`): parametric surfaces for LV/RV (truncated ellipsoids,
   wall thickness 10/4 mm), septum, RA/LA (spheres with appendage), valve rings, aorta/PA stubs; labelled regions; render meshes
   (`TriMesh` per structure) and a volumetric cell cloud (points inside the walls at ~2.5 mm spacing, depth = normalised
   transmural coordinate, fibres by a rule: helix angle linear in depth from +60° endo to −60° epi around the long axis).
3. **Real mesh path** (`cargo xtask bake-anatomy --source <dir> --out assets/anatomy/default`): read VTK/VTU or glTF from the chosen
   open dataset (document download steps and licence; do not vendor the raw 20 GB cohort — pick one case), decimate render meshes to
   ~15 k vertices per chamber (meshopt or a quadric simplifier), sample cells, compute LDRB fibres if the source has none (needs a
   Laplace solve on the volume mesh: use `sprs`/`faer` CG), UVC-like apex–base and transmural coordinates, label regions by the
   dataset's tags, generate conduction-system **landmarks** (SA node, Bachmann, AV node, His, RBB/LBB origins, LAF/LPF terminals,
   PMJ seed sets per fascicle via a fractal tree on the endocardium — Costabal parameters), coronary territories (AHA 17-segment
   approximation by UVC) for ischaemia.
4. **Torso**: generic adult torso surface (procedural superellipsoid is acceptable first; BodyParts3D later), electrode sites
   (RA, LA, LL, RL, V1–V6 standard positions, plus V3R/V4R/V7–V9), stored in `electrodes.ron` in the heart frame.
5. **Hoods**: for each render mesh vertex, the 8 nearest cells with inverse-distance weights (`hoods/<mesh>.bin`).
6. **Binary formats + loader**: `AnatomyPack::load(dir)` returning meshes, `CellCloud`, hoods, landmarks, electrodes, territories;
   `AnatomyPack::procedural()` for tests; version field; checksums.
7. **Validation**: CLI `heartsim-cli anatomy-info <pack>` printing counts, bounds, per-region cell counts, kNN stats.

## Acceptance tests
* `idealised_heart` cloud: 15–40 k cells; every ventricular cell has depth ∈ [0,1]; fibre vectors unit; mean nearest-neighbour
  distance within 20 % of the requested spacing; region counts > 0 for all ventricular/atrial regions in `Region::ALL` that the
  procedural model defines; landmarks present for `sa_node, av_node, his, rbb, lbb, laf, lpf, rv_apex, lv_lateral, lv_posterior`.
* Hoods: every vertex's 8 cells are within 3× spacing; weights sum to 1.
* Electrodes: 10 standard sites outside the heart bounds, V1/V2 at +z, LL below the heart.
* Pack roundtrip: bake → load → equality of counts and checksums; load time < 500 ms for the default pack.
* Licence file present in the pack directory and `ASSETS-LICENSES.md` updated.
