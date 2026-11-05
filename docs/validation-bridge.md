# Validation bridge to scientific solvers

`heartsim-cli` can write a scenario as an openCARP experiment, read a solver's results back onto the cell cloud, and report how they
compare with heartsim's own run (brief B23, requirement F-52). heartsim only writes and reads files. It does not contain or launch a
solver.

**Licences.** openCARP's academic public licence applies to *running* openCARP, and commercial use needs a separate agreement. MonoAlg3D is
MIT. Neither is a dependency of heartsim.

## Status: what has and has not been checked against a real solver

**No solver has been run on these files.** openCARP, `meshtool` and MonoAlg3D were not installed where this was developed. The exports are
tested against the formats in the openCARP user's manual, and the readers against files written independently from the VTK documentation.
Until the manual steps below have been done, treat the exports as unproven with the real programs. Three things in particular could only
be taken from documentation:

* Element node order. The manual's figure could not be read, so elements use positive signed volume, the VTK convention.
* The `carputils` calls in `run.py`, which were written from the manual and the carputils source but never executed.
* The AP `im_param` syntax `epsilon*r,mu1*r`, inferred from the manual's `GKr*1.6` example and the model file's parameter names.

## Commands

```bash
heartsim-cli export-opencarp sinus_normal --out export --ionic ap
```

```bash
heartsim-cli import-vtk results --mesh export/heart --out imported
```

```bash
heartsim-cli validation-report sinus_normal --imported imported --out report
```

```bash
heartsim-cli niederer --out niederer --dx-mm 0.5,0.2
```

`import-vtk` and `validation-report` must use the same `--pack` or `--spacing-mm` as the export.

### `export-opencarp`

| file | contents |
|---|---|
| `heart.pts` | nodes, in **micrometres** as openCARP requires (heartsim is in mm) |
| `heart.elem` | tetrahedra with region tags |
| `heart.lon` | one fibre per element |
| `heart_preview.vtu` | the mesh for ParaView, in mm |
| `stim_NNNN.vtx`, `stimuli.csv` | vertex electrodes from the timing engine's events, binned to 1 ms |
| `parameters.par`, `run.py` | openCARP parameters and a carputils script |
| `tags.csv` | each tag's region, target CV and APD, cell model |
| `mesh_summary.json` | node, element and cell counts |
| `README.md` | what was written, with the manual check |

**Mesh.** Each tissue cell gets a cube of six Kuhn tetrahedra, oriented along the cloud's own lattice. Tetrahedra between cell centres would lose walls one cell thick, such as the
atria. Atrial and ventricular nodes are never shared, so the fibrous ring insulates as it does in Tier 1 and Tier 2. Conduction-system cells
get no elements: the timing engine owns their delays and delivers them as the electrodes. The fast endocardial layer has its own tags.

**Parameters are a first guess.** Conductivities follow from each tag's target velocity by CV ∝ √σ, but the real velocity depends on the
mesh, model and time step, so calibrate on the mesh before comparing velocities. `--ionic ap` runs the same Aliev-Panfilov equations as Tier 2,
with its APD dial carried over. `--ionic tt06` runs TT06 in the ventricles (`ENDO` or `EPI` by depth) and Courtemanche in the atria; there the target
APDs are recorded in `tags.csv` but **not applied**, because heartsim has no validated mapping from APD to channel scalings. A 2.3 mm mesh cannot
resolve a TT06 front (research suggests 0.1-0.2 mm), and the exporter warns about it.

`niederer-slab` as the scenario writes the benchmark itself, with its published conductivities, stimulus and TT06 epicardial cells.

### `import-vtk`

Reads `.vtu` (ASCII, inline, appended raw or base64, optionally zlib), legacy `.vtk`, `.pvd` series, openCARP `.igb` and activation `.dat`. Values
per node and per cell both work. Each cell takes the value of the nearest solver sample, and cells with nothing within the limit stay empty and are
counted. Where no activation array exists, activation is the first upward crossing of a threshold in V_m. The coordinate unit is told from the size of
the mesh unless `--length-unit` says otherwise. Not supported: `.pvtu`, structured datasets, LZ4 or LZMA compression, and unevenly spaced V_m frames.

### `validation-report`

Writes `report.md`, `metrics.json`, `activation_scatter.png` and `ecg_overlay.png`: activation RMS, bias and correlation; per-lead ECG RMSE and QRS and T
correlation; forward-model weight statistics per electrode. Both V_m fields go through the same `ForwardModel`, so the ECG numbers measure differences in
activation and repolarisation, not in forward models. Windows come from heartsim's first beat, so export one beat at a time.

## Manual validation with openCARP's tools

Not run by heartsim or its tests.

```bash
meshtool query quality -msh=export/heart
```

```bash
meshtool query tags -msh=export/heart
```

```bash
meshtool convert -imsh=export/heart -ifmt=carp_txt -omsh=export/heart_check -ofmt=vtk
```

Expect the node, element and tag counts in `README.md` and `tags.csv`, and no inverted elements. If `meshtool` reports inverted elements, the node-order
assumption above was wrong: please file it.

```bash
cd export && python3 run.py --np 4
```

## The Niederer slab

`heartsim-cli niederer` runs the slab of Niederer et al. 2011 (Phil. Trans. R. Soc. A 369:4331) on the Tier-2 solver: 20 × 7 × 3 mm, fibres along x, a
1.5 mm corner stimulus for 2 ms.

**This is not a verification of the monodomain equations with TT06**, which is what the benchmark is for. Tier 2 runs phenomenological Aliev-Panfilov. With the
benchmark's diffusivities that model conducts far too slowly (the report shows by how much), so the speeds are calibrated:

1. `CV_l` and `CV_t` come from the reference's P1, P2 and P5.
2. They become diffusivities on a cable of the solver's own step (`calibrate::solve_d`).
3. The excitation dial is the one of 2, 4, 8 and 16 that best fits the `fit` points (P1, P2, P3, P5).

The `held_out` points (P4, P6, P7, P8 and the centre) take no part, and they are the number to look at. A small held-out error says that anisotropic front
geometry, the corner stimulus and the no-flux boundaries behave, and nothing about TT06.

**The reference is not the paper's supplement.** The brief asked for the published consensus, but the electronic supplementary material could not be
retrieved (the publisher's site refuses automated access and Europe PMC does not hold it as open access). `assets/validation/niederer_slab_reference.csv` instead stores
the h = 0.1 mm FEM row of Table 3 of arXiv:2111.06975: whole milliseconds, one solver, somewhat different diffusivities, with point labels read from that
paper's Figure 5. Its P8 of 43 ms agrees with the 42-43 ms that arXiv:2308.01651 gives as the benchmark's converged value. Replace the file when you have the supplement;
`--reference` takes another copy without rebuilding.

Other deviations are listed in each generated report.

### Measured results

From `heartsim-cli niederer --dx-mm 0.5,0.2`, against the stored table above, in ms:

| dx mm | dt ms | RMS, 9 points | RMS, fit | RMS, held out | largest error (P8) | benchmark D, no calibration: RMS | same: P8 |
|---|---|---|---|---|---|---|---|
| 0.5 | 0.05 | 2.01 | 0.60 | 2.64 | 4.12 | 128.8 | 228.6 |
| 0.2 | 0.005 | 1.70 | 0.63 | 2.21 | 3.28 | 124.7 | 222.6 |

The brief's bound of 3 ms at 0.5 mm is met, and the held-out points meet it too, but only after calibration. Tier 2 runs early at every held-out point, and by
about 3-4 ms at the far corner, whose reference is 43 ms. Without calibration the same cell model reaches that corner about 180 ms late, which is why this result
says nothing about TT06.

To compare a real solver on the same slab:

```bash
heartsim-cli export-opencarp niederer-slab --out slab --dx-mm 0.2
```

```bash
heartsim-cli niederer --out report --external results --mesh slab/heart
```
