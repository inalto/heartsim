# B6 — Multi-mesh scene rendering
**Crate** `heartsim-render` · **Model** Sonnet · **Milestone** M2 · **Depends on** B2 (pack meshes; icosphere/procedural meanwhile).

## Goal
Grow `SceneRenderer` from one mesh to a scene: chambers, conduction system, vessels, torso, electrodes, markers, heart-vector arrow,
with per-layer visibility/transparency, a clip plane, outline of the wavefront, and offscreen PNG capture.

## Read first
`crates/heartsim-render/src/{scene.rs,camera.rs,lut.rs,shaders/*.wgsl}`, `crates/heartsim-ui/src/viewport.rs`,
`docs/research/rust-stack.md` §1.3 (per-vertex colour, transparency, picking, labels, gotchas) and §1.6.

## Scope
1. `Layer { id, mesh: MeshGpu, visible, alpha, base_color, scalar_mode: None|Lut(id), clip: bool }`; draw opaque layers first
   (depth write), then transparent sorted back-to-front by centroid (no depth write).
2. Clip plane uniform (point + normal) with fragment `discard`; back-face rendering with a flat "cut" colour for cut-away views.
3. LUT set: V_m, isochrone, APD, plus a "conduction system highlight" solid colour; scalar window (min/max) uniforms per layer.
4. Instanced spheres (electrodes, markers, catheter electrodes) and polyline tubes (heart-vector arrow, vector loop, catheter
   bodies, pacing wires) with a small shader; colours per instance.
5. Lighting: two-light Blinn-Phong with rim; sRGB-correct; background gradient; optional MSAA 4× on the offscreen target when
   supported.
6. Picking support: `pick(uv) -> Option<Hit { layer, triangle, position, normal }>` by CPU ray–triangle over visible layers
   (BVH optional; brute force acceptable < 100 k triangles).
7. Screenshot: `capture_png(device, queue) -> Vec<u8>` via buffer mapping (padded rows).
8. Labels: expose per-layer label anchors (world → screen projection helper) for egui to draw text.

## Acceptance tests
* Headless test with `wgpu` on `lavapipe`/`VK_ICD` if available (skip otherwise): render the icosphere with V_m LUT at 256×256 and
  compare to a stored PNG (mean abs diff < 2 %); clip plane halves the visible pixel count ± 10 %.
* `pick` on the sphere centre returns a hit whose position has |p| ≈ 1; off-sphere returns None.
* Transparency: torso at alpha 0.3 still shows the heart (pixel test).
* No validation errors with `WGPU_VALIDATION=1` over 100 frames; frame time for 200 k triangles < 2 ms on the dev box's software
  renderer is *not* required — just no CPU work above O(layers) per frame apart from scalar uploads.
