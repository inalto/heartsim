# Tricuspid valve source (brief B35)

The 3-D heart's tricuspid leaflets and chordae come from one open finite-element model of a healthy human valve. This note records
where the data came from, the licence check, the files used, the facts checked on the source, what was rejected and why, and the
numbers of the fit. The code is in `crates/heartsim-geometry/src/valve.rs` and `xtask/src/bake/valve.rs`; the pack format is in
[anatomy-pack.md](../anatomy-pack.md).

## The source

* **Model:** Texas TriValve 1.1, Soft Tissue Biomechanics Laboratory.
* **Repository:** https://github.com/SoftTissueBiomechanicsLab/Texas_TriValve_1.1
* **Commit:** `cfbdad5ab8f2e02934f317dde1c2566963c98f4b`. The source is fetched at this commit only.
* **Folder used:** `Healthy_QuasiStatic/` (`Healthy_QuasiStatic.inp` and the 21 frames `Healthy_QuasiStatic_0.vtu` to
  `Healthy_QuasiStatic_20.vtu`).
* **Citation:** Mathur M, Meador WD, Malinowski M, Jazwiec T, Timek TA, Rausch MK. Texas TriValve 1.0: a reverse-engineered, open
  model of the human tricuspid valve. *Engineering with Computers* 2022. doi:10.1007/s00366-022-01659-w. Version 1.1 is a second
  donor valve built by the same method.

## Licence check

* The repository's `LICENSE-CC-BY.txt` is the Creative Commons Attribution 4.0 licence. GitHub reports the licence as CC-BY-4.0.
* CC BY 4.0 allows the valve file to ship in the pack with attribution. The README and the About window credit the source.

## Files used

* `Healthy_QuasiStatic.inp`: the Abaqus input, read for the node sets, the leaflet and chordae elements, the annulus and the free
  edge. Read by `xtask` only.
* `Healthy_QuasiStatic_0.vtu` to `_20.vtu`: the 21 result frames. Read by `xtask` only, with an LZMA reader that lives in `xtask`
  (`lzma-rust2` 0.23, Apache-2.0). No shipped crate depends on it.

The source files are downloaded by `cargo xtask bake-valve` (or `cargo xtask bake-anatomy`) at the pinned commit and checked by
size and MD5.

## Verified source facts

These were checked on all 21 frames.

* The `.inp` holds the leaflet shells (anterior, septal and posterior), the chordae truss elements (4,166 in total, with
  `Anterior_Chordae`, `Septal_Chordae` and `Posterior_Chordae`), the annular and free-edge node sets.
* Each frame has 11,046 points and 8,283 quad cells.
* The frames hold the leaflet shells only. The chordae truss elements are only in the `.inp`; the frames move the chordae nodes
  but do not carry the chordae connectivity.
* Frame 0 equals the `.inp` nodes within 9.5e-7 mm.
* The arrays are stored as xz (LZMA) blocks.
* The source shell thickness is 0.65 mm.
* The frames are one load step from end-diastole to end-systole: a pressure of 0.00306 MPa (about 23 mmHg) with prescribed annulus
  motion. The frames are ordered by load, not by time. They give shapes between open and shut; they are not a timed recording.

## Used in the pack

* Keyframes: source frames 0, 1, 2, 3, 5, 6 and 20 (seven shapes). The rest shape is frame 0 (open).
* Output (`valve_tricuspid.bin`, `HSAV` version 1): 155,832 bytes; 2,012 vertices (1,768 on the leaflets); 3,000 leaflet triangles;
  988 chordae segments. It deflates to about 104 KB; the web download size was not measured.
* Decimation: to a budget of about 3,000 leaflet triangles. Quads are split into triangles.

## Rejected

* **Texas TriValve 1.0** (repository `SoftTissueBiomechanicsLab/Texas_TriValve`). Its licence is CC BY-NC-SA 4.0, which does not allow
  it to ship in the product. It was not downloaded, and nothing was copied from it, including its `.glb` files.
* **Laurence, Lee, Johnson, Hsu, Data in Brief 39 (2021) 107664** (the tricuspid valve benchmark; PDF in `docs/research/tricuspid/`).
  It was examined and not used. It is an idealised oval cylinder with porcine dimensions, has one leaflet skirt with no leaflet
  identity, and its frames show only open and shut.
* **NIH 3-D valve entries** seen during the search. Their licence is CC BY-NC, so they cannot ship.

Texas TriValve 1.2 (CC BY 4.0, a third valve) was named in the brief as an alternative. It was not used.

## Fit to the pack heart

The fit registers the donor's annulus to the pack heart's tricuspid disc rim (the `ValveDisc` for the tricuspid valve).

* **Scale:** a uniform scale of 1.237. This heart's tricuspid disc rim has a mean radius of 18.50 mm; the donor annulus has a mean
  radius of 14.95 mm.
* **Orientation:** the donor's annulus normal is turned onto the disc normal; the septal leaflet is placed toward the septum.
* **Warp:** the annulus is warped onto the rim by up to 4.74 mm, and the warp fades to zero toward the free edge.
* **Chordae origins:** this heart has no right-ventricular papillary muscles, so each chordae origin was moved to the nearest
  right-ventricular wall. The moves were 1.3 to 13.8 mm (median 3.9 mm); 13 of the 29 origins moved 13.8 mm.
* **Leaflet vertices:** pushed back into the right heart's blood. The worst is 0.60 mm outside, for the open septal leaflet against
  the septum.
* **Leaflet areas:** the fitted area divided by the source area times the scale squared: anterior 1.01, septal 1.10,
  posterior 0.91.

## Timing

* The valve's opening is the designed valve schedule (`valve_openings`) that the echo leaflets and the heart sounds use. Opening 1
  shows the first keyframe; opening 0 shows the last.
* The last keyframe segment holds about 6 mm of motion in the final 1.3 % of closure, so the valve snaps the last part of the way
  shut. This is a limit of the source, not a designed effect.

## Not verified

* Nothing was seen on a real GPU, in a browser or in a running app window.
* No clinician has reviewed the fit, the timing or the look.
* The web download size was not measured.
