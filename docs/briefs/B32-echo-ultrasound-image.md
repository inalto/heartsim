# B32 — The ultrasound image: a line model that turns the label slice into B-mode (echo simulator, phase 3)
**Crates** `heartsim-echo`, assets, `heartsim-render`, `heartsim-ui`, `heartsim-app`, docs · **Model** Opus (image model),
Sonnet (controls), Haiku (docs) · **Milestone** M11 (echo simulator, phase 3 of 5) · **Depends on** B31 (label volumes, probe,
`slice`, the Echo page).

## Goal
The Echo page's right pane shows a grey-scale sector that reads as an echocardiogram: bright myocardium with speckle that
moves with the tissue, dark blood, bright valve leaflets and pericardium, shadows behind ribs, the bright line and
reverberation of lung, brighter tissue behind blood, an image that fades with depth unless the gain compensation is raised.
The learner can still switch to B31's coloured cross-section, alone or blended over the grey image, to see what each
structure is. It is a **simulated appearance from a simple line model**, said so on the page, with no real recordings.

## Read first
`CLAUDE.md`; `docs/research/echo-simulator.md` §3 (all; §3.4 steps 4–6 are the model, §3.2 the numbers and their status),
§9 (items 5 and 9); `docs/briefs/B31-echo-probe-and-slice.md` and what it built (`heartsim-echo` crate docs: `slice`,
`LabelImage`, the rest-position mapping, the designed parts); the Echo page in `heartsim-app`; `docs/web.md`;
`crates/heartsim-render/tests/webgl2_shaders.rs`.

## Non-negotiables for this brief
* **Clean room.** The model is written from the published formulas (Shams 2008 reflection and transmission; Mattausch 2016
  incidence term, Beer–Lambert attenuation, thresholded Gaussian scatterers; a separable polar point-spread function). No
  code is copied from any simulator, whatever its licence; Field II and GPL projects are not opened. **No Monte-Carlo or path
  tracing** (a patent on it was reported and not read). No echo dataset or clip is used, shipped or tuned against.
* **Sourced or designed.** Every acoustic number sits in one RON table under `assets/` with `format_version`, one row per
  label: impedance, attenuation, scatterer density, mean and spread, specularity. Each value carries its source as read in
  the research note, or "designed". Values the note marks unverified stay marked so. Nothing from the IT'IS database.
* **Pure, deterministic, on the CPU.** The polar grey image is a pure function of (label slice inputs, acoustic table,
  image settings, seed): byte-identical for the same inputs, on native and wasm32. Speckle is a hash of the **rest**
  position, so it rides with the tissue. The GPU only scan-converts.
* **One source of truth**; engines and goldens untouched; WebGL2-safe shader; web budget passes.
* **Honest labels.** The page says "simulated image"; no measurement is offered on it in this phase.

## Scope
1. **Acoustic table** (`assets/echo/acoustic.ron`) and its loader with validation.
2. **Line model** (`heartsim-echo`), per scan line over the samples B31 already provides (label, rest position, the two
   boxes): two-way attenuation in dB from the label's coefficient and the transmit frequency; at label changes the reflection
   coefficient from the impedances, the transmitted fraction carried down the line, and the incidence weighting from the
   angle between the beam and the interface normal (normal from the gradient of the smoothed label field, in the image plane
   and across it); bone reflects strongly and shadows; lung reflects almost everything and is followed by a designed
   reverberation (repeats of the pleural line at multiples of its depth, decaying) and shadow; low attenuation in blood gives
   enhancement behind it without a special case.
3. **Speckle.** Scatterer amplitude from a 3-D hash of the rest position on a lattice of about 0.15 mm, thresholded by the
   label's density and scaled by its mean and spread; blood nearly anechoic; designed parts (leaflets, pericardium) brighter
   and more specular.
4. **Image formation.** Convolution in polar space, axial (a short pulse of about two cycles at the transmit frequency) and
   lateral (a few lines, widening with depth beyond the focus); envelope; add the interface term; time-gain compensation
   (default cancels the mean attenuation; per-depth sliders); overall gain; log compression over a dynamic range (60 dB
   default); quantise to `u8`. Settings struct: frequency (2–5 MHz), depth, sector, focus depth, gain, TGC curve, dynamic
   range, lines and samples.
5. **Cost.** Target 15 ms or less natively for the grey image on top of the label slice (bench, not a gate). Reuse the
   label slice of the same frame; no second motion evaluation. If the web is too slow: halve the samples, cap at 20 Hz, and
   say so. Report the split per stage.
6. **Page.** Grey image by default with a display switch: *Image*, *Anatomy* (B31's colours), *Both* (colours blended over
   grey, with the legend). Controls: depth, sector width, frequency, gain, dynamic range, focus, TGC sliders, a reset. A
   depth scale, the focus marker and the index-marker dot. "No contact" as before. en + it. Reduced motion freezes the frame.
7. **Docs.** User page section *Reading the image* (what makes each structure bright or dark, shadow, reverberation,
   enhancement, what the controls do, what is designed); known-issues (nobody compared it with a real echo; no clinician or
   sonographer has reviewed it; acoustic values and their status); `CHANGELOG.md`; `docs/ROADMAP.md`; a screenshot and tour
   steps; a developer note on the pipeline and its cost.

## Out of scope
Views, scores and tutorials (phase 4); M-mode, tissue Doppler, calipers (phase 5); any blood Doppler; harmonic imaging,
side lobes, refraction, mirror artefacts, near-field clutter; contrast; pathology looks; recorded clips.

## Acceptance tests (no GPU unless marked)
* Determinism: the polar image for a fixed scenario, seed, pose, time and settings has a fixed hash, equal across two
  sampling cadences; the acoustic table round-trips and rejects a missing label or a non-positive impedance.
* Speckle statistics in a uniform block of myocardium with compensation exact: the envelope before log compression fits a
  Rayleigh distribution (Kolmogorov–Smirnov at 1 %) and its mean over standard deviation is 1.9 ± 0.2.
* Contrast: mean brightness of LV blood below 0.2 of the adjacent myocardium's; leaflets and pericardium brighter than
  myocardium.
* Shadow: behind a rib the mean level is at least 20 dB below the same depth one interspace away; behind lung likewise, and
  the reverberation lines appear at multiples of the pleural depth.
* Enhancement: at equal depth, tissue behind a blood-filled chamber is brighter than tissue behind myocardium.
* Attenuation: with TGC off in uniform tissue the mean level falls by 2·α·f dB per cm within 10 %; with default TGC it is
  flat within 3 dB.
* Tissue-locked speckle: two frames with motion off correlate at 1; a wall region tracked through a sinus beat keeps its
  pattern (correlation of the patch at its moved position above 0.7 between frames 40 ms apart) while the same fixed image
  window does not; correlation falls as the probe tilts out of plane.
* Angle dependence: an interface perpendicular to the beam is brighter than the same interface at 60°.
* Settings: gain shifts the mean monotonically; a smaller dynamic range raises contrast; frequency changes speckle size and
  penetration in the expected direction; all outputs finite for every setting in range.
* Page (egui_kittest): the three display modes, each control, reset, en and it, reduced motion.
* Shader: `webgl2_shaders.rs` covers any new shader; GPU smoke test only with lavapipe.
* Bench reported with the per-stage split and a web estimate; `cargo xtask web-budget` passes.
* `cargo xtask ci`, both wasm32 checks and `cargo check -p heartsim-app --no-default-features --lib` pass; goldens unchanged.

## Decisions for the maintainer
* Whether the grey image or the coloured anatomy is the default.
* Whether unverified acoustic values (myocardial attenuation, resolution figures) may ship marked as such.
* Who reviews the look against real echocardiography; until then the docs call it a schematic.
