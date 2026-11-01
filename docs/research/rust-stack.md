# HeartSim: Rust technology-stack research

*Date of research: 2026-10-31. Every version number, release date, star count, issue count,
licence and MSRV below was read from crates.io (`/api/v1/crates/<name>` and
`/<version>/dependencies`), the GitHub REST API, or the project's own changelog/README on that
day. Where a statement comes from general knowledge rather than a fetched source it is marked
"(not re-verified)". Star/issue counts are GitHub `stargazers_count` / `open_issues_count`
(the latter includes open PRs).*

*Toolchain context: latest stable Rust is 1.99.1 (released 2026-10-16).*

---

## 0. Executive summary

HeartSim is a scientific desktop app: one or two big meshes, a per-vertex scalar field that
changes every frame, a dense 2D strip-chart, a lot of conventional GUI (panels, sliders,
dialogs, i18n), audio cues, file export, installers. It is *not* a game: no scene graph of
thousands of entities, no animation blending, no physics. That shapes the recommendation.

**Recommended stack: egui/eframe on top of wgpu, with a hand-written 3D viewport drawn through
an `egui_wgpu` paint callback.** Bevy is the strong second choice and is recommended only if
you want its glTF/PBR/picking/pan-orbit batteries more than you want egui-at-latest, small
binaries, short compile times and full control over GPU buffers.

| Layer | Crate @ version (verified 2026-10-31) | Why |
|---|---|---|
| GPU API | `wgpu` 30.0.1 (2026-08-21) | The only serious cross-platform Rust GPU abstraction (Vulkan/Metal/DX12/GL/WebGPU/WebGL2); used by egui, Bevy, Rerun; compute shaders in WGSL |
| Windowing / app shell | `eframe` 0.37.0 (winit 0.30.13 inside) | Does window, wgpu device/surface, egui integration, screenshots, persistence, WASM (`trunk`) for you; thin enough to replace with raw `winit`+`egui-winit` later |
| GUI | `egui` 0.37.0 + `egui_extras` 0.37.0 | Immediate mode is ideal for a sim whose state changes every frame; AccessKit built in; huge ecosystem; same crate on desktop and web |
| Docking | `egui_tiles` 0.18.0 | Tracks egui 0.37 the day it ships (maintained by Rerun); `egui_dock` 0.21.1 is still on egui 0.36 |
| Plots (analysis views) | `egui_plot` 0.38.0 | Zoom/pan/hover for offline ECG inspection; *not* for the live 25 mm/s strip |
| Live ECG strip | custom `egui::Painter` line strips + min/max decimation | 12 leads x ~1000 px columns is well inside epaint's tessellation budget at 60 fps; upgrade path is a wgpu instanced-line callback |
| 3D viewport | hand-written: `wgpu` + `glam` 0.34.1 + `bytemuck` 1.25.2 + `gltf` 1.4.1 | A heart + torso + electrodes is one or two draw calls; per-vertex colour is a separate `STORAGE`/`VERTEX` buffer updated with `queue.write_buffer` each frame |
| Picking | `parry3d` 0.31.1 (`TriMesh` + QBVH ray cast) | Exact triangle hit on CPU in microseconds; no GPU readback |
| Spatial queries | `kiddo` 6.3.0 (k-d tree) / `rstar` 0.13.0 | Nearest-vertex for electrode snapping, local stimulus |
| Simulation (CPU) | `rayon` 1.12.0, `wide` 1.7.1, `sprs` 0.11.6 (or `faer` 0.24.4) | Monodomain reaction-diffusion with a sparse cotangent/FEM Laplacian scales to 500k vertices on CPU with SoA f32 + rayon; eikonal via fast marching on CPU |
| Simulation (GPU, phase 2) | `wgpu` compute (WGSL), `encase` 0.13.0 | Ping-pong storage buffers that the renderer reads directly; no readback for colouring |
| Audio | `kira` 0.12.6 | Sample-accurate clock-scheduled beeps/alarms; `rodio` 0.23.0 if you want simplest-possible |
| Serialisation | `serde` 1.0.229, `ron` 0.12.3, `toml` 1.1.8, `serde_json` 1.0.154 | RON for scenarios (enums, comments), TOML for user config, JSON for interchange |
| Logging / profiling | `tracing` 0.1.44, `tracing-subscriber` 0.3.23, `puffin` via egui (optional) | Structured logs; `profiling` crate shims Tracy if needed |
| Errors | `anyhow` 1.0.104 (app), `thiserror` 2.0.21 (libs) | Standard |
| Paths / config | `directories` 6.0.0 | XDG / `~/Library/Application Support` / `%APPDATA%` |
| Export | `image` 0.25.10 (PNG), `csv` 1.4.0, `printpdf` 0.12.8 (PDF), `plotters` 0.3.7 (off-screen charts) | All pure Rust, all WASM-capable except printpdf's font loading needs care |
| i18n | `fluent` 0.17.0 (+ `fluent-bundle`, `unic-langid`) ; alt `rust-i18n` 4.2.4 | Mozilla Fluent handles plurals/gender; `sys-locale` 0.3.2 for detection |
| File dialogs | `rfd` 0.17.2 | Native dialogs on 3 OSes + web |
| Packaging | `cargo-packager` 0.11.8 (+ `apple-codesign`/`rcodesign` 0.29.0) | Produces .app/.dmg, .msi/.exe (WiX/NSIS), .deb/.AppImage from one config; rcodesign signs + notarises from Linux CI |
| Web | `trunk` 0.22.1, `wasm-bindgen` 0.2.130, wgpu `webgpu`+`webgl` features | WebGPU on Chrome/Edge/Safari 26/Firefox-Windows; WebGL2 fallback (no compute) |
| Testing | `proptest` 1.12.0, `insta` 1.49.0, `criterion` 0.8.2`/`divan` 0.1.21, `cargo-nextest` 0.9.150 | Golden-ECG snapshot tests with insta's CSV/JSON snapshots |
| Hygiene | `cargo-deny` 0.20.2, `cargo-about` 0.9.3, `cargo-release` 1.1.7 | Licence/advisory audit, THIRD-PARTY notices, versioned releases |

Second-choice stack (if Bevy): `bevy` 0.20.1 + `bevy_egui` 0.43.1 + `egui` **0.36** (not 0.37) +
`egui_plot` **0.37** + `egui_tiles` **0.17** + `egui-notify` **0.23** + `bevy_kira_audio` 0.27.1 +
`bevy-inspector-egui` 0.38.0. Bevy's own `PanOrbitCamera` and `MeshPickingPlugin` (both in-tree
in 0.20) replace `bevy_panorbit_camera`/`bevy_mod_picking`. MSRV jumps to 1.97.1.

---

## 1. Rendering / engine layer

### 1.1 Candidates

| Crate | Version | Last release | GitHub stars | Open issues+PRs | Licence | MSRV | Backend | Status |
|---|---|---|---|---|---|---|---|---|
| `wgpu` | 30.0.1 | 2026-08-21 | 18,434 | 1,313 | MIT OR Apache-2.0 | 1.87 | Vulkan/Metal/DX12/GLES/WebGPU/WebGL2 | Very active (pushed 2026-10-31) |
| `bevy` | 0.20.1 | 2026-10-28 (0.20.0: 2026-10-08) | 49,419 | 3,523 | MIT OR Apache-2.0 | **1.97.1** | wgpu ^30 | Very active; 0.x release every ~3-4 months, each with a migration guide |
| `three-d` | 0.19.0 | 2026-04-17 | 1,685 | 28 | MIT | none declared | **OpenGL via `glow` 0.17** (not wgpu); `winit` ^0.28, `egui` ^0.34 | Maintained but slow; lags egui by three majors and winit by two |
| `rend3` | 0.3.0 | **2022-02-12** | 1,156 | 50 | MIT OR Apache-2.0 OR Zlib | 1.57 | wgpu (ancient) | **Archived on GitHub**, README says "MAINTENANCE MODE". Do not use. |
| `kiss3d` | 0.47.0 | 2026-10-04 | 1,791 | 89 | BSD-3-Clause | none declared | OpenGL/WebGL (`glow`-ish) | Alive again under dimforge; toy-level API, no custom-shader story, no egui |
| `fyrox` | 1.0.1 | 2026-03-28 | 9,640 | 60 | MIT | 1.87 | own OpenGL renderer | Full game engine with editor; scene-oriented; 1.0 in 2026; no wgpu, no egui integration; WASM via WebGL |
| hand-rolled `wgpu`+`winit`+`egui` | wgpu 30.0.1 / winit 0.30.13 / egui 0.37.0 | 2026-08..10 | n/a | n/a | MIT/Apache | 1.95 (egui) | wgpu | You own the renderer (~1-2k lines for this app) |

Winit note: `winit` 0.30.13 is the stable line; 0.31.0-beta.4 (2026-10-24, Apache-2.0, MSRV
1.86) rewrites the `ApplicationHandler` surface again (`can_create_surfaces`, `Box<dyn Window>`).
Neither eframe 0.37 nor Bevy 0.20 has moved to it. Stay on 0.30 and let eframe/Bevy do the
migration for you.

### 1.2 Version coupling you must respect

```
wgpu 30.0.1 ──┬── egui-wgpu 0.37.0  (wgpu ^30, winit ^0.30.13, egui ^0.37)
              ├── eframe 0.37.0     (wgpu ^30, winit ^0.30.13, glow ^0.17 fallback)
              ├── bevy_render 0.20  (wgpu ^30, encase ^0.12, bytemuck ^1.5)
              └── bevy_egui 0.43.1  (wgpu-types ^30.0.1, winit ^0.30, **egui ^0.36**)

egui 0.37.0 ── egui_plot 0.38.0, egui_tiles 0.18.0, egui-notify 0.24.0, egui_extras 0.37.0
egui 0.36.0 ── egui_plot 0.37.0, egui_tiles 0.17.0, egui-notify 0.23.0, egui_dock 0.21.1, bevy_egui 0.43.x
```

Consequence: **with Bevy you are pinned to egui 0.36 until bevy_egui bumps**; with eframe you are
on egui 0.37 today. egui releases roughly every 2-3 months (0.36 on 2026-08-05, 0.37 on
2026-10-15), and the Bevy pairing typically lags by one egui release. This has been the pattern
for years (bevy_egui's compatibility table shows one bevy_egui major per Bevy release).

### 1.3 How each option handles the hard requirements

#### Per-vertex colour updated every frame (50k-500k vertices)

* **Hand-rolled wgpu**: keep positions/normals/indices in immutable `VERTEX`/`INDEX` buffers
  uploaded once. Keep the activation scalar (or an RGBA8 colour) in a separate buffer with
  `BufferUsages::VERTEX | COPY_DST` (or `STORAGE | COPY_DST` and index it by
  `@builtin(vertex_index)` in WGSL). Each frame: `queue.write_buffer(&color_buf, 0,
  bytemuck::cast_slice(&values))`. 500k x f32 = 2 MB/frame = 120 MB/s at 60 fps; trivial on any
  PCIe/UMA GPU. wgpu's `write_buffer` goes through a staging belt internally; for even less
  overhead use `StagingBelt` or a double-buffered mapped buffer. Better still: upload the scalar,
  not a colour, and apply the colour map in the fragment shader with a 1D LUT texture, so
  changing the colour map or the iso-contour settings costs nothing on the CPU.
* **Bevy**: two routes. (a) Mutate the `Mesh` asset's `ATTRIBUTE_COLOR` every frame with
  `RenderAssetUsages::MAIN_WORLD | RENDER_WORLD`. Bevy then re-extracts the *whole* mesh and
  re-creates/re-uploads the interleaved vertex buffer (positions, normals, UVs and colour), i.e.
  ~10x more bytes than needed and an allocation per frame; it works for 50k vertices, it is
  wasteful at 500k. (b) The idiomatic way: a custom `Material` (or `ExtendedMaterial` over
  `StandardMaterial`) with a `#[storage(n)] Handle<ShaderStorageBuffer>` field; mutate that
  asset each frame (a flat `Vec<f32>`) and read `activation[vertex_index]` in a custom vertex
  shader. Bevy re-uploads just that buffer. This requires learning Bevy's material/shader-def
  system and, in 0.20, writing the shader in **WESL** (`.wesl`) if you use any of Bevy's
  `#import`s; plain WGSL still works.
* **three-d**: `Mesh::update_colors`/`CpuMesh` round-trips; OpenGL only; no compute. Not
  competitive for this workload.
* **kiss3d / Fyrox**: both expose "set vertex colours on a mesh", both re-upload the whole mesh;
  Fyrox can do custom shaders but in its own GLSL-ish material system. Neither gives you wgpu
  compute.

#### Custom shaders and compute

* wgpu: WGSL directly, `ComputePipeline`, `dispatch_workgroups` (renamed in 30.0 from
  `dispatch`). Simulation state in two `STORAGE` buffers (ping-pong); the render pass binds the
  current one as a read-only storage buffer. No readback needed for colouring. For the ECG you
  need per-lead scalars per timestep: either compute them in a second small compute pass
  (lead-field dot product, workgroup reduction) and read back ~12 floats/step via a small
  mapped buffer (wgpu 30: `Buffer::get_mapped_range` now returns `Result`; `Device::poll` with
  `PollType::Wait` returns `CantWait` on WebGPU, so on web you must await the map callback), or
  keep the source-vector readback asynchronous one frame behind. Latency of one frame is
  irrelevant for an ECG strip.
* Bevy: `bevy_render` exposes the same wgpu objects through its render graph; you write a
  `Node` plus `ExtractSchedule`/`Render` systems. Third-party helpers (`bevy_app_compute`
  0.16.0, last released 2025-04-28; `bevy_easy_compute` 0.15.0, 2024-12-08) are **stale
  relative to Bevy 0.20** and should not be relied on. Bevy 0.20 adopted WESL (crate `wesl`
  0.6.0) as its shader language; mesh shaders landed but are "not supported on web".

#### glTF loading

* `gltf` 1.4.1 (2024-05-10; MIT OR Apache-2.0; MSRV 1.61; 645 stars, 90 open; last push
  2026-05-11). Mature, slow-moving, perfectly adequate for reading positions/normals/indices/
  named nodes and mesh "regions" (one primitive per region, or a per-vertex `_REGION`
  custom attribute written by Blender). Supports `.glb` and Draco-less `.gltf`; KHR_mesh_quantization
  needs manual handling (not re-verified).
* Bevy's `bevy_gltf` loads the whole scene into entities with `StandardMaterial`s and names;
  you then query by `Name` to find "LV", "RV", "torso". Convenient, but you still need to copy
  the mesh data out to build the Laplacian for the simulation.

#### Picking

* Hand-rolled: `parry3d::shape::TriMesh` built once from the mesh; `cast_ray_and_get_normal`
  with the camera ray. Parry's QBVH makes this ~10-50 us for 500k triangles (not re-verified
  benchmark, but it is the same BVH rapier uses). Gives triangle id -> region / barycentric ->
  nearest vertex for electrode placement. Also works on WASM.
* Bevy 0.20: `MeshPickingPlugin` (in-tree since 0.15; `bevy_mod_picking` 0.20.1 from 2024 and
  `bevy_mod_raycast` 0.18.0 from 2024 are the pre-merge crates, both dead). Observer-based
  `Pointer<Click>` events. Mesh picking in Bevy also walks a BVH-less triangle list per mesh by
  default (not re-verified for 0.20), so for 500k triangles check cost; `parry3d` can still be
  used alongside.

#### Transparency / cut-away

* Hand-rolled: one mesh, so "sorting" is: draw opaque geometry (torso) first, then the heart
  with alpha blending and `depth_write_enabled: false`, back faces first then front faces
  (two draws with opposite `cull_mode`) for a cheap but convincing order-independent look.
  Cut-away = `discard` in the fragment shader against a clip plane uniform (and draw the
  back faces with a flat "interior" colour). Metal (macOS) and WebGL2 are fine with this.
  Full OIT (weighted-blended) is ~200 lines if ever needed.
* Bevy: `AlphaMode::Blend` on the material, `Transmission`, `OrderIndependentTransparencyPlugin`
  (since 0.15, not re-verified for 0.20). Clip planes need a custom material.

#### Labels / text in 3D

* Hand-rolled: project 3D anchor to screen each frame and let egui draw the label (`painter.text`
  on a `LayerId::background` or a foreground layer); zero extra text-rendering code, i18n for
  free. Leader lines via `painter.line_segment`.
* Bevy: `Text2d`/UI nodes with a `Transform` projected via `Camera::world_to_viewport`; or
  bevy_egui the same way as above.

#### Integrating a 2D plotting panel

* Hand-rolled with eframe: the ECG strip *is* egui; the 3D view is an
  `egui_wgpu::Callback` inside a `CentralPanel`/docked tile. One frame, one swapchain, one
  present. Rerun (31k-star egui-based app) is the production proof of this architecture.
* Bevy: `bevy_egui` draws egui into Bevy's render graph; the plot is a normal egui widget. Works,
  but bevy_egui's own colour test "does not fully pass egui.rs rendering test" (issue #291 in
  their README), i.e. gamma/sRGB subtleties exist.

### 1.4 Compile times, binary size, WASM

* **eframe + wgpu** clean debug build: ~1-2 min on a laptop; incremental: a few seconds. Release
  binary 15-25 MB with wgpu's Vulkan+DX12+Metal+GL backends (strip + LTO gets ~10 MB). WASM
  ~6-10 MB uncompressed, ~2-3 MB brotli (figures from experience with egui apps; not
  re-verified for 0.37).
* **Bevy 0.20** clean debug build: 3-8 min; needs the "fast compiles" setup (`dynamic_linking`
  feature, `lld`/`mold`, cranelift for dev) to keep iteration bearable; release binary 40-80 MB
  with default features (trim with `default-features = false` and a curated list). WASM
  ~20-40 MB before compression. Bevy explicitly documents `webgl2` vs `webgpu` features and
  `wayland`/`x11` features (`docs/cargo_features.md`).
* Both support WASM via wgpu; Bevy's `webgl2` feature "enables limitations so Bevy can use
  WebGL2"; its mesh shaders and compute are WebGPU-only.

### 1.5 Recommendation

**Primary: eframe 0.37 (egui 0.37 + egui-wgpu 0.37 + winit 0.30.13) with a hand-written wgpu 30
renderer for the 3D viewport.** Reasons, in order of weight:

1. The hot path (per-vertex scalar -> GPU -> colour-mapped mesh) is a 2 MB `write_buffer` and
   one draw call; you want direct control of that buffer and of the compute pipelines that will
   eventually write it. Bevy makes you go through assets/extract/render-graph abstractions
   designed for thousands of entities you do not have.
2. GUI-heavy app: 80 % of the screen is egui. eframe gives you egui at the newest version
   (0.37 today) with `egui_plot`, `egui_tiles`, `egui-notify`, AccessKit, screenshots,
   persistence, native file dialogs and a working `trunk` web build with zero glue code.
3. Compile time and binary size are 3-5x better, which matters for a two-to-four-person
   medical-education project more than for a studio.
4. Fewer breaking migrations: wgpu majors (every ~3 months) touch ~50 lines of your renderer;
   Bevy majors touch everything.

**Second option: Bevy 0.20.1 + bevy_egui 0.43.1.** Choose it if (a) you expect to grow into
many animated entities (catheters, valves, blood particles, multiple hearts), (b) you want the
in-tree glTF scene loader, PBR, shadows, `PanOrbitCamera` (new in 0.20, CAD-style) and
`MeshPickingPlugin` on day one, or (c) your team already knows ECS. Accept egui lagging one
version, egui 0.36-era plugins, and the WESL shader dialect.

Do not use rend3 (archived), kiss3d (no shader/egui story), or three-d (OpenGL, winit 0.28,
egui 0.34). Fyrox is a capable engine but it is its own world (own renderer, own UI, own editor,
GLSL) and gives you nothing for the ECG/GUI side.

### 1.6 Gotchas (rendering layer)

* **wgpu 30 breaking changes** (from the changelog): `VertexState::buffers` is now
  `&[Option<VertexBufferLayout>]`; integer inter-stage I/O must be `@interpolate(flat)`
  explicitly; `dispatch` -> `dispatch_workgroups`; `SurfaceConfiguration` gained `color_space`;
  `SurfaceTexture::present()` became `Queue::present(surface_texture)`;
  `Buffer::get_mapped_range` returns `Result`. Tutorials written for wgpu 22-26 will not
  compile unchanged.
* **wgpu 30.0.0 had a Vulkan validation error every frame** (un-waited fence passed to
  `vkAcquireNextImageKHR`), fixed in 30.0.1. Pin `>=30.0.1`.
* **GLES/WebGL2 reject `@interpolate(linear)`** (no `LINEAR_INTERPOLATION` downlevel flag).
  Use `perspective` (default) or `flat`.
* **macOS/Metal**: the swapchain is premultiplied-alpha; if you blend egui over a 3D
  callback make sure the 3D pass does not write alpha < 1 into the swapchain, or you get a
  translucent window on macOS Tahoe with transparent-window support enabled. Render the 3D
  view to an offscreen `Rgba8UnormSrgb`/`Rgba16Float` texture and composite as an egui image
  if you hit this (also fixes MSAA mismatches between egui's and your pipeline).
* **MSAA**: egui-wgpu's render pass has its own sample count; a `Callback` draws into egui's
  pass, so your pipeline must match `msaa_samples` from `eframe::NativeOptions`. If you want
  8x MSAA on the heart but not on the UI, render to texture.
* **Linux**: wgpu picks Vulkan first. On Wayland, `winit` 0.30 needs the `wayland` feature
  (eframe enables both `x11` and `wayland` by default); older Intel/Mesa stacks and NVIDIA
  535-series drivers under Wayland have had surface-resize/present-mode issues; expose an env
  var (`WGPU_BACKEND=gl`) escape hatch and a `--backend` CLI flag. `MESA_VK_DEVICE_SELECT`
  helps on hybrid laptops. Debian-based CI needs `libxcb-render0-dev libxcb-shape0-dev
  libxcb-xfixes0-dev` (bevy_egui README) and `libwayland-dev libxkbcommon-dev` (not
  re-verified for eframe).
* **Windows**: wgpu defaults to Vulkan if a Vulkan ICD exists, otherwise DX12. Some Intel iGPU
  Vulkan drivers are worse than their DX12 drivers; many wgpu apps prefer `Backends::DX12 |
  VULKAN` with DX12 first on Windows. Build with MSVC, not GNU (DX12 via `windows` crate; DXC
  shader compiler needs the `dxc` runtime DLLs if you enable `Dx12Compiler::Dxc`).
* **Intel / older AMD**: `Limits::downlevel_defaults()` on GL; storage buffers in vertex
  shaders are not available on WebGL2/GLES3.0 (`max_storage_buffers_per_shader_stage == 0`), so
  for the WebGL2 fallback the per-vertex scalar must be a **vertex attribute**, not a storage
  buffer. Design the colour-upload path as a `VERTEX` buffer from day one and you avoid two
  code paths.
* **Bevy only**: `bevy_egui` requires scheduling UI systems in `EguiPrimaryContextPass`
  (multi-pass egui); `bevy_panorbit_camera` 0.36.0 (2026-10-20, bevy ^0.20) still exists if
  you dislike the in-tree `PanOrbitCamera`; `bevy-inspector-egui` 0.38.0 pairs with bevy 0.20
  + egui 0.36.

---

## 2. GUI layer

| Crate | Version | Last release | Stars | Open | Licence | MSRV | Paradigm | 3D integration |
|---|---|---|---|---|---|---|---|---|
| `egui` / `eframe` | 0.37.0 | 2026-10-15 | 31,895 | 1,106 | MIT OR Apache-2.0 | 1.95 | Immediate | `egui_wgpu::Callback` or render-to-texture; first-class |
| `iced` | 0.14.0 | 2025-12-07 | 31,878 | 516 | MIT | 1.88 | Retained/Elm | `iced_wgpu` custom `shader::Program` widget exists (not re-verified for 0.14); fewer plot/dock crates |
| `slint` | 1.18.1 | 2026-09-21 | 24,341 | 703 | **GPL-3.0 OR Slint-Royalty-free-2.0 OR Slint-Software-3.0** | 1.92 | Declarative DSL (.slint), retained | wgpu integration via `slint::wgpu_30` texture import (Slint 1.13+ added a wgpu-30 compatible API, not re-verified); 3D goes in a texture |
| `dioxus` (desktop) | 0.7.10 stable / 0.8.0-alpha.3 | 2026-10-16 | 39,561 | 899 | MIT OR Apache-2.0 | 1.85 | React-like, **webview** (wry) | 3D must be WebGPU/WebGL *inside* the webview via JS, or a separate native window; awkward |
| `tauri` | 2.12.5 | 2026-10-21 | n/a | n/a | Apache-2.0 OR MIT | 1.90 | Webview shell | Same as Dioxus; great bundler, wrong fit for a wgpu app |
| `xilem` / `masonry` | 0.4.0 | 2025-10-29 | 5,584 | 159 | Apache-2.0 | 1.88 | Retained, Vello/wgpu | Vello renders via wgpu so sharing a device is possible, but widgets/docking/plots are immature; 12k downloads total; pre-1.0 research project |

AccessKit (`accesskit` 0.25.2, 2026-10-25) is integrated in egui (0.37 switched to
`accesskit::Role`, better popup/menu/modal trees), iced, Slint, Xilem and Bevy's `bevy_a11y`
(accesskit ^0.24). Webview frameworks get the browser's accessibility tree.

### Evaluation

* **Immediate vs retained for this app**: the UI mirrors simulation state that changes 60 times
  a second (heart rate, phase, lead values, alarm state). Immediate mode means no observable/
  signal plumbing; you just read the sim struct while building the frame. Retained frameworks
  (iced, Slint) require messages/properties for every changing value. For editor-like tools
  with many panels this is where egui wins decisively; it is also why Rerun chose it.
* **Look & feel**: egui looks like egui (flat, slightly "tool-ish"); `egui` 0.37 added an
  experimental `StyleProvider` theme system, `RoundedRect`, colour emoji and system-font
  fallback for non-Latin scripts (important for i18n). Slint and iced look more "native/
  designed". For medical education software an egui look is acceptable (Rerun, Zed-adjacent
  tools); dark/light themes are built in.
* **i18n**: none of the Rust GUI toolkits has a built-in translation system except Slint
  (gettext-based `@tr()`). With egui use Fluent (`fluent` 0.17.0, 2025-05-23, Apache/MIT) or
  `rust-i18n` 4.2.4 (2026-09-29, MIT, YAML/TOML string tables, `t!("key")` macro). Fluent is the
  better choice for proper plural rules in medical text ("1 beat" / "2 beats", French/German
  genders). egui 0.37's system-glyph fallback means CJK/Arabic labels render without bundling
  fonts, but you still need a bundled font for deterministic PDF output.
* **Docking**: `egui_tiles` 0.18.0 (Rerun, egui 0.37) vs `egui_dock` 0.21.1 (2026-08-06, egui
  0.36, 645 stars). Both are fine; egui_tiles tracks egui releases same-day and is what Rerun
  ships. Pick egui_tiles.
* **Notifications**: `egui-notify` 0.24.0 (2026-10-30, MIT, egui 0.37).
* **Icons**: `egui-phosphor` 0.14.0 (2026-09-10) or `egui_material_icons` 0.9.0 (2026-10-19).
  Both are icon fonts; check their egui req matches 0.37 when you add them (not fetched).
* **Tables/images/syntax**: `egui_extras` 0.37.0 (tables, image loaders, date picker).
* **Markdown help panels**: `egui_commonmark` 0.26.0 (2026-10-15).
* **Dialogs**: `rfd` 0.17.2 (2026-10-22, MIT) native open/save on all three OSes + web
  (download/upload). egui modals: `egui::Modal` is built in since 0.31.

### Recommendation

**egui 0.37 via eframe.** Second choice **iced 0.14** only if a designer insists on a
retained, themeable, native-looking UI and is willing to write the 3D view as an iced
`shader` widget and the ECG strip as a `canvas`. Avoid Slint for this project unless you are
comfortable with GPL-3 or the royalty-free licence terms (which restrict distribution
contexts) and with the texture-based 3D embedding. Avoid webview frameworks: they put the
GPU work behind a JS boundary and double your language surface.

---

## 3. ECG strip rendering (12 leads x ~2500 samples, 25 mm/s, grid paper, 60 fps)

Sizing the problem: a 10-second strip at 25 mm/s is 250 mm. On a 96-dpi display that is ~945
px; on a 2x HiDPI display ~1890 physical px. A lead sampled at 250 Hz has 2500 samples in those
~1000 logical px, i.e. ~2.5 samples per pixel column. 12 leads = 30,000 samples per frame if
drawn naively.

| Approach | 60 fps budget | Anti-aliasing | Line width | Verdict |
|---|---|---|---|---|
| `egui_plot` 0.38 `Line` per lead | Tessellates ~30k segments/frame through epaint. Rerun devs state epaint tessellation is "one of the performance bottlenecks of egui_plot"; the egui issue tracker's standing advice is to down-sample first. ~30k points is borderline-OK on desktop, bad on WASM. Also egui_plot's auto-bounds, hover, legend and grid cost per frame. | epaint feathering (1 px soft edge) | any `f32` width, screen-space | Use for *paused/analysis* views (zoom, pan, hover values, calipers). Not for the live strip. |
| **Custom `egui::Painter`** (`Shape::line` / `PathShape` per lead + `painter.line_segment` for the 1 mm/5 mm grid) | After min/max decimation to one (min,max) pair per pixel column you draw ~2 x 1000 points per lead = 24k vertices total, no plot bookkeeping. epaint tessellation for 24k vertices is ~1-2 ms on a laptop (estimate; benchmark with `egui_plot`'s `benches/performance.rs` method). Draw the grid once into a cached texture or as a `Mesh` shape. | same feathering; looks like paper | 1.0-1.5 px; set `stroke.width` in points and remember HiDPI scaling | **Recommended first implementation.** ~300 lines; works on WASM. |
| **wgpu instanced line segments** via `egui_wgpu::Callback` (ring buffer of samples in a `STORAGE` or `VERTEX` buffer, one quad per segment, `write_buffer` of only the new samples each frame) | <0.2 ms; scales to 1M points (cf. the `egui_wgpu_plot` experiment and Rerun's custom line shader) | you implement AA yourself (SDF in the fragment shader or MSAA); mismatched MSAA with egui pass is the classic bug | arbitrary, with proper joins if you do SDF | Phase-2 upgrade if you add 1 kHz sampling, 20-second strips, or multiple simultaneous strips. |

Design notes for the strip:

* Keep each lead in a fixed-size ring buffer (`VecDeque<f32>` or a `Box<[f32; N]>` with a
  write cursor). Decimate per *pixel column* each frame: for each column find min and max of the
  samples that fall into it, emit two points (max then min alternating, or a thin filled
  polygon). This is the standard oscilloscope technique and guarantees QRS spikes are never
  dropped, which naive stride decimation would do.
* Sweep ("erase bar") mode vs scrolling mode: both are a translation of x; the sweep mode
  avoids redrawing the grid.
* Grid: draw 1 mm minor (light pink/grey) and 5 mm major lines; at 25 mm/s and 10 mm/mV the
  paper is defined in *mm*, so define a `px_per_mm = ui.ctx().pixels_per_point() * dpi /
  25.4` and accept that most monitors lie about DPI; expose a calibration slider. Painter
  lines snap to whole pixels only if you round coordinates yourself; a half-pixel offset keeps
  1 px lines crisp.
* Anti-aliasing: epaint feathers edges by `tessellation_options.feathering_size_in_pixels`
  (default 1). For a crisp trace set `feathering = false` for the strip's shapes or keep it
  but use 1.5 px width. On WASM/WebGL2 without MSAA, feathering is what makes diagonal lines
  look acceptable.
* Export: the same decimated polyline can be fed to `plotters` for PNG and to `printpdf`
  for PDF, so on-screen and exported strips match.

---

## 4. Simulation, numerics, mesh, audio, serialisation, utilities

### 4.1 CPU parallelism and math

| Crate | Version | Last release | Licence | MSRV | Role |
|---|---|---|---|---|---|
| `rayon` | 1.12.0 | 2026-04-14 | MIT OR Apache-2.0 | 1.80 | Parallel iterators over vertices/elements; `par_chunks_mut` for SoA updates |
| `glam` | 0.34.1 | 2026-10-06 | MIT OR Apache-2.0 | 1.85 | SIMD `Vec3`/`Mat4` for camera, mesh math; matches wgpu/bytemuck idioms; Bevy uses it |
| `nalgebra` | 0.35.0 | 2026-05-24 | Apache-2.0 | 1.89 | Needed anyway by `parry3d`; generic dense linear algebra, small matrices for FEM element assembly |
| `ndarray` | 0.17.2 | 2026-01-10 | MIT OR Apache-2.0 | 1.64 | N-d arrays if you store fields as 2D/3D arrays (e.g. voxel torso); optional |
| `wide` | 1.7.1 | 2026-09-14 | Zlib OR Apache-2.0 OR MIT | 1.89 | Stable-Rust SIMD (`f32x8`) for the ionic-model ODE step (the hot loop); `std::simd` is still nightly-only (not re-verified for 1.99) |
| `sprs` | 0.11.6 | 2026-10-13 | MIT OR Apache-2.0 | 1.88 | CSR sparse matrices; SpMV for the diffusion term; simple, pure Rust |
| `faer` | 0.24.4 | 2026-06-24 | MIT | 1.84 | High-performance dense + sparse (Cholesky/LU) factorisations; use if you go implicit (backward Euler for diffusion needs a sparse solve each step, or factor once) |
| `nalgebra-sparse` | 0.12.0 | 2026-05-24 | Apache-2.0 | n/a | COO/CSR/CSC conversions; less complete than sprs/faer for solves |

Guidance: the monodomain model on a 50k-500k vertex surface or 1-5M-element volume mesh is
dominated by (a) the ionic ODE update (embarrassingly parallel, SIMD-friendly: rayon +
`wide`), and (b) one SpMV per step for the Laplacian (sprs CSR, `rayon` over rows). With
explicit forward Euler at dt = 0.02-0.05 ms and a 10-20x coarser render rate, a 100k-vertex
surface monodomain at real-time is achievable on 8 cores. For the eikonal (activation-time)
formulation, a fast-marching / fast-iterative method on the triangle mesh is a few hundred
lines and runs in <100 ms for 100k vertices on one core; it is the right default for an
*education* simulator (deterministic, instant restart, trivially parameterised by regional
conduction velocities), with reaction-diffusion as the "research mode".

### 4.2 GPU compute

* `wgpu` 30 compute with WGSL. Structure: `state_a`, `state_b` storage buffers (f32 Vm plus
  gating variables as SoA arrays), a CSR Laplacian uploaded once (`row_ptr`, `col_idx`,
  `vals`), a uniform with dt/params, N compute dispatches per rendered frame, then the render
  pass binds the live `Vm` buffer. **Zero readback for colouring.** For ECG leads run a
  reduction compute pass over the lead-field matrix (12 x N) and read back 12 floats via a
  `MAP_READ` staging buffer asynchronously (one-frame latency). `encase` 0.13.0 (2026-09-19,
  MIT-0) handles WGSL std430/uniform layout for your param structs; `bytemuck` 1.25.2 for the
  plain arrays.
* Limits: default `max_storage_buffer_binding_size` is 128 MiB; `max_compute_workgroups_per_dimension`
  65535 (so index with 2D dispatch or `workgroup_size(256)` x ceil(N/256)); WebGL2 has **no
  compute**, so the GPU path must be optional (`cfg`/runtime `Features` check) with the CPU
  path as fallback. f16 storage needs `Features::SHADER_F16`.
* `cubecl` 0.11.3 (2026-10-28; Rust-DSL -> WGSL/CUDA/ROCm) is an interesting alternative that
  lets you write the kernel once in Rust and run it on CPU or GPU, but it brings a large
  dependency tree and its wgpu runtime version coupling must be checked; not recommended for
  v1.
* Bevy: same wgpu underneath; helper crates are stale (see 1.3).

### 4.3 Mesh and geometry crates

| Crate | Version | Last release | Licence | Notes |
|---|---|---|---|---|
| `gltf` | 1.4.1 | 2024-05-10 | MIT OR Apache-2.0 | glTF 2.0 reader/writer; stable, slow cadence; pushed 2026-05 |
| `tobj` | 4.0.5 | 2026-08-02 | MIT | OBJ loader, active; handy for research meshes |
| `obj` | 0.10.2 | 2020-11-30 | Apache-2.0 | Stale; prefer tobj |
| `ply-rs` | 0.1.3 | 2020-08-27 | MIT | Stale; write your own ASCII/binary PLY reader (~150 lines) if you need PLY |
| `meshopt` | 0.7.0 | 2026-10-17 | MIT OR Apache-2.0 | Bindings to meshoptimizer: vertex-cache/overdraw optimisation, simplification (LOD for 500k -> 50k), works offline in an asset pipeline |
| `parry3d` | 0.31.1 | 2026-09-18 | Apache-2.0 | `TriMesh`, QBVH, ray casting, point projection -> picking and electrode snapping |
| `rapier3d` | 0.36.1 | 2026-10-10 | Apache-2.0 | Full physics; **not needed** (parry alone suffices) |
| `kiddo` | 6.3.0 | 2026-09-06 | MIT OR Apache-2.0 | Fast k-d tree, SIMD; nearest-vertex queries |
| `rstar` | 0.13.0 | 2026-05-24 | MIT OR Apache-2.0 | R*-tree; good for 2D/3D bulk loads and AABB queries |
| `kdtree` | 0.8.1 | 2026-07-23 | MIT OR Apache-2.0 | Older, simpler k-d tree; kiddo is faster |

### 4.4 Audio

| Crate | Version | Last release | Licence | MSRV | Verdict |
|---|---|---|---|---|---|
| `cpal` | 0.18.2 | 2026-08-16 | Apache-2.0 | 1.85 | Raw device I/O; everything else is built on it; use directly only if you write your own mixer |
| `rodio` | 0.23.0 | 2026-10-11 | MIT OR Apache-2.0 | 1.87 | Simplest: `Sink::append(SineWave...)`. Scheduling is "play now", jitter ~ one audio buffer (5-20 ms) |
| `kira` | 0.12.6 | 2026-10-28 | MIT OR Apache-2.0 | n/a | Game-audio engine: clocks, precise start times, tweens, effects; `bevy_kira_audio` 0.27.1 wraps it for Bevy |

Recommendation: **kira**. A QRS beep that drifts 15 ms against the on-screen R-wave is
noticeable to clinicians; kira's clock-based scheduling (`StartTime::ClockTime`) lets you
schedule the beep for the exact simulated R-peak time a frame ahead. Alarms are looping
sounds with tweened volume. Generate tones procedurally (no asset files) or ship small OGG
files (`symphonia` decoders via kira features). All three work on WASM (web audio via cpal's
`wasm-bindgen` backend; user-gesture requirement applies).

### 4.5 Serialisation, config, logging, errors, units

| Crate | Version | Last release | Licence | Use |
|---|---|---|---|---|
| `serde` | 1.0.229 | 2026-07-18 | MIT OR Apache-2.0 | Everything |
| `ron` | 0.12.3 | 2026-10-28 | MIT OR Apache-2.0 | Scenario files: Rust-like enums/structs, comments, trailing commas; readable by educators |
| `toml` | 1.1.8+spec-1.1.0 | 2026-10-09 | MIT OR Apache-2.0 | User settings; TOML 1.1 |
| `serde_json` | 1.0.154 | 2026-10-11 | MIT OR Apache-2.0 | Interchange/export, web |
| `tracing` / `tracing-subscriber` | 0.1.44 / 0.3.23 | 2025-12-18 / 2026-03-13 | MIT | Structured logging with spans; `tracing-wasm`/`tracing-web` on web; `EnvFilter` |
| `directories` | 6.0.0 | 2025-01-12 | MIT OR Apache-2.0 | `ProjectDirs::from("org","Inalto","HeartSim")` -> config/data/cache dirs |
| `anyhow` | 1.0.104 | 2026-07-18 | MIT OR Apache-2.0 | App-level errors |
| `thiserror` | 2.0.21 | 2026-09-23 | MIT OR Apache-2.0 | Library error enums |
| `uom` | 0.38.0 | 2026-02-14 | Apache-2.0 OR MIT | Type-safe SI units. **Not recommended** in the hot loop or GPU-facing structs (zero-cost in theory, but `f32`-generic code becomes verbose and it fights `bytemuck`). Use it, if at all, at the scenario-file boundary; otherwise newtype `Millivolts(f32)`, `Milliseconds(f32)` |
| `clap` | 4.6.7 | 2026-09-14 | MIT OR Apache-2.0 | CLI crate (headless sim, batch ECG export) |
| `rfd` | 0.17.2 | 2026-10-22 | MIT | File dialogs |

Fixed-timestep loop: do not use the render frame as the sim clock. Run the simulation on a
dedicated thread (`std::thread` + `crossbeam-channel`/`std::sync::mpsc`) with an accumulator
(`sim_time += dt` while `sim_time < wall_time * speed_factor`), publish snapshots (Vm array
+ 12 lead samples per step) through a triple buffer (`triple_buffer` crate, not fetched) or
an `Arc<Mutex<Vec<f32>>>` swapped per frame; the UI thread calls
`ctx.request_repaint_after(16ms)` or `request_repaint()` on new data. On WASM there is no
thread by default: run N sim steps inside the frame callback (keep per-frame work under
~8 ms) or use `wasm_thread`/web workers with SharedArrayBuffer (requires COOP/COEP headers).

### 4.6 Export

| Crate | Version | Last release | Licence | Notes |
|---|---|---|---|---|
| `image` | 0.25.10 | 2026-03-10 | MIT OR Apache-2.0 | PNG encode of the swapchain/offscreen readback (`COPY_SRC` texture -> buffer, 256-byte row padding) and of plotters bitmaps |
| `csv` | 1.4.0 | 2025-10-17 | Unlicense/MIT | 12-lead samples, activation times |
| `printpdf` | 0.12.8 | 2026-09-05 | MIT | 0.12 is a rewrite (HTML-ish layout, SVG, fonts); vector ECG paper at 25 mm/s with exact mm units is a natural fit; embed a bundled TTF |
| `genpdf` | 0.2.0 | **2021-06-17** | Apache-2.0 OR MIT | Unmaintained; skip |
| `typst` (as a library) | 0.15.1 | 2026-07-17 | Apache-2.0 | Alternative for report-style PDFs (text + embedded SVG of the strip); heavy dependency; good if you want "patient report" documents with typesetting |
| `plotters` | 0.3.7 | 2024-09-08 | MIT | Off-screen PNG/SVG charts; stable but slow-moving; there is also `plotters-backend` for custom (e.g. printpdf) backends |

Screenshots: eframe 0.37 exposes `ViewportCommand::Screenshot` (now callback-based) for the
whole window; for a clean 3D-only PNG at arbitrary resolution render to an offscreen texture
with your own pipeline and read it back (works headless in the CLI crate too, with
`wgpu::Instance` + `request_adapter` with `force_fallback_adapter` for CI).

---

## 5. Cross-platform packaging, distribution and CI

### 5.1 Tools

| Tool | Version | Last release | Stars | Open | Licence | Produces | Notes |
|---|---|---|---|---|---|---|---|
| `cargo-dist` ("dist") | 0.32.0 | 2026-05-22 | 2,144 | 354 | MIT OR Apache-2.0 | tarballs/zips, `curl \| sh` + PowerShell installers, npm, Homebrew, **MSI (WiX)**; generates the whole GitHub Actions release workflow | **No .app/.dmg, no AppImage/.deb/Flatpak** (listed as "requested future installers"). Astral's fork was merged back in 0.29 (2025-07) and Axo Releases hosting was removed; repo still pushed 2026-10-30. Great for CLIs, not for a GUI app's installers |
| `cargo-packager` (CrabNebula) | 0.11.8 | 2025-11-27 | 500 | 43 | Apache-2.0 OR MIT | macOS `.app` + `.dmg`; Windows **NSIS `.exe`** + **WiX `.msi`**; Linux `.deb`, **AppImage**, pacman | Tauri's bundler extracted for any binary (lists egui, slint, wry, deno); companion `cargo-packager-updater`; repo pushed 2026-10-28 although the crate release is 11 months old; config in `Cargo.toml` metadata or `Packager.toml`; signing hooks for Windows (signtool/osslsigncode) and macOS (codesign/notarytool) handled via config keys (not re-verified in detail) |
| `cargo-bundle` | 0.12.0 | 2026-09-20 | 1,378 | 74 | MIT OR Apache-2.0 | `.app`, `.deb`, (MSI experimental) | Older, simpler; no dmg, no AppImage, no NSIS; fine for a quick `.app` |
| `tauri-bundler` | 2.10.4 | 2026-10-21 | (tauri repo) | | Apache-2.0 OR MIT | same formats as cargo-packager plus RPM | Usable without Tauri in principle but expects Tauri's config shape; prefer cargo-packager |
| `apple-codesign` / `rcodesign` | 0.29.0 | **2024-11-29** (crates.io and GitHub release) | 875 | 120 | MPL-2.0 | Signs Mach-O/.app/.dmg/.pkg and **notarises** via App Store Connect API **from Linux or Windows** | Repo pushed 2026-10-17 but no release in 23 months; still works with current Apple notarisation API as of community reports (not re-verified). Keeps the macOS runner out of the signing path |
| WiX Toolset | v4/v5 (external) | | | | MS-RL | `.msi` | `cargo-packager` drives WiX 3.x binaries it downloads (not re-verified for v4); NSIS gives a friendlier per-user installer without admin |
| `cargo-about` | 0.9.3 | 2026-10-26 | | | MIT OR Apache-2.0 | THIRD-PARTY-LICENSES.html | Required for shipping MIT/Apache attributions in the installers |

### 5.2 Platform specifics

* **Windows**: build `x86_64-pc-windows-msvc` (DX12 requires MSVC-compatible `windows` crate
  bindings; GNU toolchain works but has worse debug info and no DXC). Also ship
  `aarch64-pc-windows-msvc` if you care about Snapdragon laptops (`windows-11-arm` runner
  exists). Sign with `signtool` using an Azure Trusted Signing or EV cert; unsigned MSI/NSIS
  triggers SmartScreen. Set a per-monitor-v2 DPI manifest (eframe/winit does this; verify in
  the final exe with `mt.exe` if you embed an icon via `winresource`/`embed-resource`).
* **macOS**: build **both** `aarch64-apple-darwin` and `x86_64-apple-darwin` (cross-compiling
  between the two on one macOS runner is just a target add; both link against the same SDK),
  then `lipo -create -output HeartSim HeartSim-arm64 HeartSim-x86_64` and bundle one universal
  `.app`. Minimum macOS: wgpu/Metal needs 10.13+ in principle; eframe/winit 0.30 practically
  11+; set `MACOSX_DEPLOYMENT_TARGET=11.0`. Sign with hardened runtime + `com.apple.security.cs.allow-jit`
  not needed (no JIT); entitlements for audio are not needed for output-only. Notarise
  (`rcodesign notary-submit --api-key-path ... --staple`), then build the `.dmg`
  (`cargo-packager` or `create-dmg`), sign and notarise the dmg too. Store the Developer ID
  `.p12` and the App Store Connect API key as GitHub secrets. Gatekeeper on macOS 15/26 refuses
  unnotarised apps with no override UI, so this is mandatory for distribution outside the App
  Store.
* **Linux**: build on **`ubuntu-22.04`** (glibc 2.35) so the binary runs on Debian 12, Ubuntu
  22.04+, RHEL 9 (glibc 2.34 -> you need `ubuntu-22.04` *or* the `cargo-zigbuild`
  `-target x86_64-unknown-linux-gnu.2.34` trick; RHEL 9 is your own dev box's family, so test
  it). Produce `.deb` + `.AppImage` from cargo-packager; add `.rpm` via `cargo-generate-rpm`
  (not fetched) if RHEL/Fedora users matter; Flatpak via a `flathub` manifest using the
  `org.freedesktop.Platform` 25.08 runtime and `cargo-sources.json` generated by
  `flatpak-cargo-generator` (not re-verified). Runtime deps are tiny (libxkbcommon, wayland/
  x11 libs, alsa or pipewire-alsa for cpal, libgl/vulkan loader). Wayland vs X11: winit picks
  Wayland when `WAYLAND_DISPLAY` is set; let users force X11 with `WINIT_UNIX_BACKEND=x11`.
  NVIDIA + Wayland + Vulkan has historically been the flakiest combination (explicit sync
  landed in 2024 drivers); document `WGPU_BACKEND=gl` as the fallback.
* **GPU backend notes**: wgpu order of preference per OS: Windows DX12/Vulkan, macOS Metal,
  Linux Vulkan then GL. Ship with `Backends::PRIMARY | GL` and `PowerPreference::HighPerformance`
  (dGPU on laptops), and surface the chosen adapter name in an About/Diagnostics panel.
  Test on Intel iGPU (Vulkan driver quality varies; `Limits` smaller), on an NVIDIA proprietary
  driver, and on Mesa RADV.

### 5.3 GitHub Actions runner images available today (from `actions/runner-images` README)

`ubuntu-22.04`, `ubuntu-24.04` (= `ubuntu-latest`), `ubuntu-26.04`, all with `-arm` variants;
`windows-2022`, `windows-2025` (= `windows-latest`), `windows-11-arm`; `macos-15`,
`macos-26` (= `macos-latest`, arm64) and `macos-15-intel`/`macos-26-intel` (x64); macOS 14 is
deprecated. Native runners beat cross-compilation for a wgpu/winit app because you also want to
*run* smoke tests (`cargo nextest run` with `force_fallback_adapter` or `WGPU_BACKEND=gl` +
`xvfb` on Linux; software-rendered Vulkan via `lavapipe`/`mesa-vulkan-drivers` for shader
tests).

### 5.4 Recommended concrete pipeline

```
.github/workflows/
  ci.yml       push/PR: fmt, clippy -D warnings, cargo-deny, nextest (3 OS matrix),
               wasm32 check (cargo check --target wasm32-unknown-unknown -p heartsim-web),
               doc build, criterion smoke (no comparisons)
  release.yml  on tag v*: 
    - linux  (ubuntu-22.04): cargo build --release; cargo packager (deb, appimage); upload
    - windows (windows-2025): build; signtool sign exe; cargo packager (nsis, msi); sign installers; upload
    - macos  (macos-15): build arm64 + x86_64; lipo; cargo packager (app); rcodesign sign;
             rcodesign notary-submit --staple; packager dmg; sign+notarise dmg; upload
    - web    (ubuntu-24.04): trunk build --release; upload to GitHub Pages / S3
    - publish: gh release create with CHANGELOG section (git-cliff or cargo-release notes)
```

Use `dtolnay/rust-toolchain@stable` pinned to the MSRV for one job and stable for the rest,
`Swatinem/rust-cache@v2`, `taiki-e/install-action` for nextest/cargo-deny/cargo-packager,
`sccache` optional. Expected release build time per OS: 5-10 min for the eframe stack,
15-25 min for Bevy.

### 5.5 WASM pipeline

* Toolchain: `rustup target add wasm32-unknown-unknown`; `trunk` 0.22.1 (2026-10-26, MSRV
  1.96.1) builds `index.html` + runs `wasm-bindgen` 0.2.130 (2026-10-30) + `wasm-opt`;
  `wasm-bindgen-futures` 0.4.80; `console_error_panic_hook` 0.1.7 (2021, tiny, fine).
  eframe's web backend (`eframe::WebRunner`) is documented via the `eframe_template` repo.
* wgpu features: enable `webgpu` and `webgl` (the GLES backend compiled for WebGL2). wgpu
  tries WebGPU first and falls back to WebGL2 when `navigator.gpu` is absent. WebGL2 limits:
  no compute, no storage buffers in vertex stage, no `@interpolate(linear)`, 4k-16k uniform
  bytes, `Rgba16Float` render targets need extensions. So the web fallback = CPU simulation
  + vertex-attribute colours. Also `wasm64-unknown-unknown` is now supported by wgpu's web
  backend (unreleased changelog) if you ever exceed 4 GB (you will not).
* Browser status (web.dev, Nov 2025): WebGPU in Chrome/Edge 113+ (Windows/macOS/ChromeOS;
  Linux behind flags/limited GPUs), Safari in macOS Tahoe 26 / iOS 26, Firefox 141+ on Windows
  and 145+ on Apple Silicon macOS; Firefox Linux and Intel Macs still "in progress" as of the
  sources found. Plan for WebGL2 to remain the common denominator for Linux browser users
  through 2027.
* egui on web: text input/IME works; file save = download via `rfd`'s web backend; audio needs a
  user gesture; `localStorage` persistence via eframe's `persistence` feature. Threads: none
  without COOP/COEP + `wasm_thread`; keep the sim step small. Binary size: enable `opt-level
  = "s"` or `"z"` for the wasm profile, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`,
  and `wasm-opt -Oz`.

---

## 6. Project structure and engineering practice

### 6.1 Cargo workspace

```
heartsim/
  Cargo.toml                 # [workspace] resolver = "3", workspace.package, workspace.dependencies, lints
  rust-toolchain.toml        # channel = "1.99.1" (dev pin); rust-version in crates = MSRV
  deny.toml  clippy.toml  rustfmt.toml  .cargo/config.toml  about.toml
  crates/
    heartsim-core/           # units, time, scenario model (serde), ids; no_std-friendly, no GPU deps
    heartsim-geometry/       # mesh loading (gltf/tobj), half-edge/CSR adjacency, Laplacian, regions, parry TriMesh
    heartsim-sim/            # ionic models, monodomain CPU stepper (rayon+wide), eikonal/FMM, lead fields; feature "gpu" -> wgpu compute
    heartsim-ecg/            # ECGSYN-style synthesis for "textbook" mode, filters, QRS detection, strip ring buffers, decimation
    heartsim-render/         # wgpu pipelines: heart/torso/electrode passes, colour LUTs, clip planes, offscreen capture
    heartsim-ui/             # egui panels, docking layout, i18n (fluent), themes, ECG strip painter, 3D viewport widget
    heartsim-app/            # eframe binary: wiring, settings, audio (kira), persistence, exports; [[bin]] name = "heartsim"
    heartsim-cli/            # clap: run scenario headless, export CSV/PNG/PDF, benchmark
    heartsim-web/            # trunk entry (index.html, wasm-bindgen start); depends on heartsim-app with default-features = false
  assets/  (meshes .glb, fonts, locales/*.ftl, scenarios/*.ron)
  docs/    (mdBook: user guide + developer guide; research/)
  xtask/   (optional: asset pipeline (meshopt simplify, region bake), icon generation, release checklist)
```

Rules: `core`, `geometry`, `sim`, `ecg` must compile without wgpu/egui (unit-testable, usable
from the CLI, embeddable in other tools). `render` depends on wgpu only; `ui` depends on egui +
render; `app` is thin. Put all third-party versions in `[workspace.dependencies]` and use
`dep.workspace = true` in crates so a wgpu/egui bump is a one-line change.

### 6.2 Feature flags (suggested)

* `heartsim-sim`: `gpu` (wgpu compute), `f64` (double-precision CPU path for validation
  tests), `parallel` (rayon; off for wasm without threads).
* `heartsim-app`: `audio` (kira; off in CI headless), `pdf` (printpdf; large), `profiling`
  (puffin/tracy), `persistence`.
* `heartsim-web`: forces `wgpu/webgl`, `wgpu/webgpu`, no `audio` files, no `pdf` if the font
  embed blows up the wasm size.

### 6.3 MSRV policy

Toolchain reality today: stable 1.99.1; egui/eframe/egui-wgpu/egui_plot/egui_tiles MSRV 1.95;
wgpu 1.87; trunk 1.96.1; cargo-release 1.97; **Bevy 1.97.1**; cargo-nextest 1.91. Set
`rust-version = "1.95"` for the egui stack (or `"1.97"` with Bevy) and adopt a policy of "MSRV =
latest stable minus 2 at the time of each minor release, bumped only in minor releases", which
is what wgpu documents ("stable minus three"). CI runs clippy/tests on the MSRV and on stable.
Build *releases* with the pinned stable in `rust-toolchain.toml`.

### 6.4 Lints and hygiene

* `[workspace.lints]`: `rust.unsafe_op_in_unsafe_fn = "deny"`, `rust.missing_debug_implementations = "warn"`,
  `clippy.all = "warn"`, `clippy.pedantic = "warn"` with a curated `allow` list
  (`module_name_repetitions`, `cast_precision_loss`, `too_many_lines`), `clippy.unwrap_used =
  "warn"` in library crates. `rustfmt.toml`: `edition = "2024"`, `imports_granularity = "Module"`
  (nightly-only, optional).
* `deny.toml` (cargo-deny 0.20.2): `[licenses] allow = ["MIT","Apache-2.0","Apache-2.0 WITH
  LLVM-exception","BSD-2-Clause","BSD-3-Clause","ISC","Zlib","MIT-0","Unicode-3.0","Unlicense",
  "CC0-1.0","MPL-2.0","OFL-1.1"]`; `[advisories]` deny unmaintained/yanked; `[bans]`
  `multiple-versions = "warn"` (wgpu/egui stacks always drag two `windows-sys`). Note: MPL-2.0
  appears via `apple-codesign` only as a *tool*, not a dependency; `wide`/`bytemuck` are
  Zlib-or-MIT; `encase` is MIT-0; `csv` is Unlicense/MIT; fonts you bundle are OFL.
* `cargo-about` generates `THIRD_PARTY_LICENSES.html` shipped in the installer and shown in
  About.

### 6.5 Testing strategy

* **Unit**: ionic model single-cell APD/CV against published values; Laplacian row sums = 0;
  lead-field reciprocity.
* **Property tests** (`proptest` 1.12.0): decimation never drops a global min/max; ring-buffer
  invariants; scenario (de)serialisation round-trips; FMM activation times are monotone along
  paths.
* **Golden/snapshot** (`insta` 1.49.0): store CSV (`assert_csv_snapshot!` via `serde`) of
  12-lead ECGs for canonical scenarios (sinus 60 bpm, LBBB, RBBB, STEMI anterior, AF) at
  reduced sample rate; compare with a tolerance by snapshotting rounded values (insta compares
  text; round to 3 decimals and keep `f32` determinism by avoiding rayon reductions in
  non-associative order, or sum in fixed order). Also snapshot the activation-time field as a
  hashed histogram, not a 500k-float file.
* **Image regression**: offscreen render -> PNG -> compare with `image` + a perceptual
  threshold (`dssim`-like crate or a simple mean abs diff) on the `lavapipe` software Vulkan
  driver in CI; keep thresholds loose across drivers.
* **GUI tests**: `egui_kittest` (egui's own harness; 0.37 added video recording) with
  AccessKit queries for the panels.
* **Benchmarks**: `criterion` 0.8.2 (2026-02-04) for the sim step and decimation; `divan`
  0.1.21 (2025-04-10; simpler but slower-moving) is fine for micro-benchmarks. Run on PRs with
  `--noplot` and no history comparison; keep a weekly scheduled job with history.
* **Runner**: `cargo-nextest` 0.9.150 (per-test process isolation; important because wgpu/cpal
  tests touch global device state).

### 6.6 Docs, licensing, releases

* `mdBook` for user guide + dev guide; `cargo doc --workspace --no-deps` published with it.
* Project licence: **MIT OR Apache-2.0** (the ecosystem default; maximises reuse by
  universities) or **Apache-2.0 only** (explicit patent grant; several medical-device contexts
  prefer it). GPL would be legitimate for an education tool but would block embedding in
  commercial simulators and conflicts with nothing in the dependency set *except* that you
  could not then use the Slint royalty-free licence or openCARP-derived code anyway. If you
  port **ECGSYN** (McSharry et al., PhysioNet) note its reference implementations are **GPL**;
  re-implement from the paper's equations (3 coupled ODEs, documented parameters) rather than
  translating the C/Matlab source if you want a permissive licence.
* Asset licences: heart meshes from e.g. the Zygote/BodyParts3D (CC-BY-SA) or the
  Cardiac Atlas Project have their own terms; keep an `ASSETS-LICENSES.md`.
* Versioning: SemVer for crates; app version = `heartsim-app` version; `cargo-release` 1.1.7
  (2026-10-21) with a workspace `release.toml` (`shared-version = true`, `tag-name = "v{{version}}"`,
  `pre-release-replacements` for CHANGELOG); Conventional Commits enforced by `cocogitto` or
  `commitlint`, changelog by `git-cliff` (not fetched). Tag push triggers `release.yml`.
* Pre-commit: `pre-commit` framework or `lefthook` with `cargo fmt --check`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo deny check`, `typos`, `taplo fmt` for TOML.

---

## 7. Existing Rust work worth reusing or studying

### 7.1 crates.io (searched "ecg", "cardiac", "electrophysiology", "monodomain", "heart")

| Crate | Version | Updated | What it is | Verdict |
|---|---|---|---|---|
| `tpt-sci-electrophys` | 0.1.0 | 2026-08-22 | "Cardiac electrophysiology (Hodgkin-Huxley, bidomain) models" | Only hit for cardiac PDE models; 0.1.0, single author, inspect before depending; likely study-only |
| `organpool` | 0.2.0 | 2026-02-18 | Organ simulation with cardiac/respiratory modelling and vital-sign diagnostics (lumped-parameter) | Possible source for haemodynamic coupling (BP, SpO2) in the monitor view |
| `qrs_detector` | 0.2.0 | 2020-07-03 | Real-time QRS detector (Pan-Tompkins-like) | Small; copy the algorithm (MIT) |
| `cardio-rs` | 0.2.1 | 2025-03-28 | HRV metrics from ECG/PPG | Reuse for HRV display |
| `ecgtoolkit` | 0.1.0 | 2026-03-17 | Port of the C# ECGToolkit: read/write/convert ECG files (SCP-ECG, DICOM, HL7 aECG) | Useful for *exporting* real-format ECG files; check licence ("Other") |
| `electrocardiogram-synthetic-data-generator` | 0.1.0 | 2026-03-17 | Synthetic ECG generator, marked WIP/not for production | Study only |
| `ge-healthcare-muse` | 1.0.0 | 2026-07-25 | Parses GE MUSE XML | Import real 12-leads for comparison mode |
| `vyna-dcm-*` / `dicom` 0.10.1 | 2026-10 | | DICOM libraries (`dicom` is the established pure-Rust DICOM implementation) | If you ever export DICOM Waveform (12-lead ECG IOD) |
| `exg` | (GitHub eugenehp/exg) | 2026-06 | EEG/ECG/EMG preprocessing with MNE-Python parity, **GPL-3.0** | Licence-incompatible with a permissive app; study only |
| `liveplot` | 2.5.3 | 2026-10-27 | Real-time plotting on egui/eframe with gRPC/Parquet export (Apache-2.0) | Worth reading for its multi-trace realtime design (`max_points` per trace) |
| `dspk` | 0.0.4 | 2026-10-29 | Multi-channel signal processing on GPU/CPU via CubeCL, min/max envelope pyramids for viewing | Directly relevant to the decimation design in section 3; pre-release |
| `seiscode-eikonal` (GitHub PageotD) | | 2026-10-16 | Pure-Rust fast marching eikonal solver (MIT) | Grid-based (seismic) but the FMM core is reusable; a mesh-based FMM is a different algorithm |

No crate implements a monodomain/bidomain solver on unstructured meshes in a reusable way; the
GitHub search for `cardiac electrophysiology language:rust` returned **0 repositories**,
`monodomain OR bidomain OR eikonal language:rust` returned 2 (both grid-based eikonal). The
simulation core will be original work. `hector-crean/bild-sim` (Apache-2.0, 2026-08) is a
"GPU physics simulation framework on Bevy/wgpu with WGSL solvers and volume tools" and is a
useful reference for structuring wgpu compute inside Bevy if you go that route.

### 7.2 Rust medical visualisation to study

* `rerun-io/rerun` (not fetched; well known): egui + wgpu, custom line/point renderers, time
  series at kHz rates, docking via `egui_tiles`. The single best architectural reference for
  "scientific viewport + egui panels".
* `timcogan/perspecta` (MIT, 2026-10-28), `NoahFeldt/ChromaCT`, `carlvillads/illuminator`,
  `irixyoctane/xray`: small Rust DICOM/CT viewers; useful for volume/slice rendering idioms if
  you add a CT-derived torso later.
* `evroon/bevy-volumetric-clouds` (58 stars, MIT) is the most-starred "volume rendering in
  Bevy/wgpu" example; ray-marching a 3D texture, directly transferable to a voxel torso.

### 7.3 C/C++ libraries via FFI (only if truly needed)

| Library | Licence | Notes |
|---|---|---|
| **openCARP** | Academic Public License 1.1 (non-commercial) for the simulator core; Apache-2.0 for most tooling; commercial licence available | Reference-grade monodomain/bidomain with many ionic models. The APL makes any binary that links it non-open-source and non-commercial. Study its models and CellML exports; do not link. |
| **Chaste** | BSD-3-Clause (current `LICENSE` in `Chaste/Chaste` develop) | Large C++ (PETSc, MPI, Boost); FFI would mean embedding PETSc. Use as a validation oracle (generate reference activation maps offline), not as a runtime dependency. |
| **CellML models** (`cellml.org`, e.g. ten Tusscher 2006, O'Hara-Rudy 2011, Courtemanche) | Mostly CC-BY / permissive per model | Export to C via `cellmlmanip`/`chaste_codegen` or write a small Python script that emits Rust from CellML; this is the practical way to get validated ionic models into Rust. |
| **ECGSYN** | GPL (C/Matlab/Java reference code) | Re-implement from the paper (3 ODEs, Gaussian PQRST). |
| **meshoptimizer** | MIT | Already wrapped by `meshopt` 0.7.0. |
| **Embree / OptiX** | Apache-2.0 / proprietary | Unnecessary; parry's QBVH suffices for picking. |

---

## 8. Recommended `Cargo.toml` dependency list (versions verified on crates.io, 2026-10-31)

```toml
[workspace]
resolver = "3"
members = ["crates/*", "xtask"]

[workspace.package]
edition = "2024"
rust-version = "1.95"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
# --- GPU / windowing / GUI ------------------------------------------------
wgpu            = { version = "30.0.1", default-features = false, features = ["wgsl", "vulkan", "metal", "dx12", "gles", "webgpu", "webgl"] }
eframe          = { version = "0.37.0", default-features = false, features = ["wgpu", "default_fonts", "persistence", "x11", "wayland", "accesskit"] }
egui            = "0.37.0"
egui-wgpu       = "0.37.0"
egui_extras     = { version = "0.37.0", features = ["image", "svg"] }
egui_plot       = "0.38.0"
egui_tiles      = "0.18.0"
egui-notify     = "0.24.0"
egui_commonmark = "0.26.0"        # optional help panels
rfd             = "0.17.2"
accesskit       = "0.25.2"        # only if you query the a11y tree directly

# --- math / geometry / simulation ----------------------------------------
glam            = { version = "0.34.1", features = ["bytemuck", "serde"] }
bytemuck        = { version = "1.25.2", features = ["derive"] }
encase          = { version = "0.13.0", features = ["glam"] }
nalgebra        = "0.35.0"
parry3d         = "0.31.1"
kiddo           = "6.3.0"
rstar           = "0.13.0"        # alternative spatial index
rayon           = "1.12.0"
wide            = "1.7.1"
sprs            = "0.11.6"
faer            = "0.24.4"        # optional, implicit solvers
ndarray         = "0.17.2"        # optional
gltf            = { version = "1.4.1", features = ["names", "utils"] }
tobj            = "4.0.5"
meshopt         = "0.7.0"         # asset pipeline / xtask only

# --- audio ---------------------------------------------------------------
kira            = "0.12.6"
# rodio         = "0.23.0"        # simpler alternative
# cpal          = "0.18.2"        # raw alternative

# --- data / io -----------------------------------------------------------
serde           = { version = "1.0.229", features = ["derive"] }
serde_json      = "1.0.154"
ron             = "0.12.3"
toml            = "1.1.8"
csv             = "1.4.0"
image           = { version = "0.25.10", default-features = false, features = ["png"] }
printpdf        = "0.12.8"
plotters        = { version = "0.3.7", default-features = false, features = ["bitmap_backend", "svg_backend", "line_series"] }
directories     = "6.0.0"

# --- app plumbing --------------------------------------------------------
anyhow          = "1.0.104"
thiserror       = "2.0.21"
tracing         = "0.1.44"
tracing-subscriber = { version = "0.3.23", features = ["env-filter", "fmt"] }
clap            = { version = "4.6.7", features = ["derive"] }
fluent          = "0.17.0"
fluent-bundle   = "0.16"          # pairs with fluent 0.17 (not fetched; check)
unic-langid     = "0.9"           # (not fetched; check)
sys-locale      = "0.3.2"
pollster        = "1.0.1"         # block_on for wgpu adapter/device on native
# uom           = "0.38.0"        # only at the scenario boundary, if at all

# --- web -----------------------------------------------------------------
wasm-bindgen          = "0.2.130"
wasm-bindgen-futures  = "0.4.80"
console_error_panic_hook = "0.1.7"
web-sys               = "0.3"     # eframe 0.37 pins ^0.3.103

# --- dev / test ----------------------------------------------------------
proptest        = "1.12.0"
insta           = { version = "1.49.0", features = ["csv", "json", "ron"] }
criterion       = { version = "0.8.2", features = ["html_reports"] }
divan           = "0.1.21"
egui_kittest    = "0.37.0"        # egui test harness (same version as egui)
```

Developer tools (install with `cargo install --locked` or `taiki-e/install-action`):
`cargo-nextest 0.9.150`, `cargo-deny 0.20.2`, `cargo-about 0.9.3`, `cargo-release 1.1.7`,
`cargo-packager 0.11.8`, `apple-codesign 0.29.0` (`rcodesign`), `trunk 0.22.1`,
`cargo-dist 0.32.0` (only if you also ship the CLI as a `curl | sh` install), `wasm-bindgen-cli`
pinned to **exactly** the `wasm-bindgen` crate version in `Cargo.lock` (0.2.130).

If you pick the Bevy route instead, replace the GUI block with:

```toml
bevy                = { version = "0.20.1", default-features = false, features = ["bevy_winit", "bevy_render", "bevy_pbr", "bevy_gltf", "bevy_picking", "mesh_picking", "bevy_camera_controller", "bevy_text", "bevy_ui", "x11", "wayland", "tonemapping_luts", "multi_threaded", "webgl2"] }
bevy_egui           = "0.43.1"     # egui 0.36
egui                = "0.36.0"
egui_plot           = "0.37.0"
egui_tiles          = "0.17.0"
egui-notify         = "0.23.0"
bevy_kira_audio     = "0.27.1"     # kira 0.12
bevy-inspector-egui = "0.38.0"     # dev only
# bevy_panorbit_camera = "0.36.0"  # or in-tree PanOrbitCamera
```
and set `rust-version = "1.97"` (feature names for `bevy_camera_controller` not re-verified;
consult `docs/cargo_features.md` for 0.20).

---

## 9. Decision summary and open risks

1. **Renderer**: eframe + hand-written wgpu viewport (Bevy second). Risk: you own ~2k lines of
   renderer; mitigated by the small feature set (one lit mesh, LUT colour, clip plane, lines,
   picking on CPU).
2. **GUI**: egui 0.37 + egui_tiles. Risk: egui's 0.x cadence (migration every ~2-3 months);
   mitigated by workspace-level version pins and upgrading only at milestones.
3. **ECG strip**: custom painter with min/max decimation now; wgpu lines later. Risk: low.
4. **Sim**: CPU rayon/wide + sprs first, eikonal as default education mode; wgpu compute as a
   feature. Risk: performance of 500k-vertex monodomain on CPU in real time is not guaranteed;
   the design keeps the GPU path as a drop-in (same CSR layout, same buffers).
5. **Packaging**: cargo-packager + rcodesign. Risk: cargo-packager's last crate release is
   2025-11 (repo active) and rcodesign's is 2024-11 (repo active); both are "works but slow".
   Fallback: `tauri-bundler` 2.10.4 or hand-written `create-dmg`/`makensis` scripts; `xcrun
   notarytool` on the macOS runner replaces rcodesign if Apple changes the API.
6. **Web**: WebGPU where available, WebGL2 fallback without compute. Risk: Linux browsers and
   Intel Macs; acceptable for a stretch goal.
7. **MSRV**: 1.95 (egui) today; expect each egui release to raise it by 2-3 minors.
8. **Licensing**: all recommended runtime dependencies are MIT/Apache/BSD/Zlib/MIT-0/Unlicense;
   nothing copyleft. Slint, openCARP, ECGSYN sources and `exg` are the only copyleft/
   non-commercial items encountered and are excluded.

---

## 10. References consulted (2026-10-31)

crates.io API (`https://crates.io/api/v1/crates/<name>` and `/<version>/dependencies`) for:
bevy, bevy_render, bevy_winit, bevy_egui, bevy_panorbit_camera, bevy-inspector-egui,
bevy_kira_audio, bevy_mod_picking, bevy_mod_raycast, bevy_mod_outline, bevy_app_compute,
bevy_easy_compute, wgpu, winit, egui, eframe, egui-wgpu, egui_extras, egui_plot, egui_dock,
egui_tiles, egui-notify, egui_commonmark, egui_material_icons, egui-phosphor, three-d, rend3,
kiss3d, fyrox, iced, slint, dioxus, tauri, tauri-bundler, xilem, masonry, gltf, tobj, obj,
ply-rs, meshopt, parry3d, rapier3d, rstar, kiddo, kdtree, rayon, nalgebra, nalgebra-sparse,
glam, ndarray, wide, sprs, faer, rodio, cpal, kira, hound, serde, serde_json, ron, toml,
tracing, tracing-subscriber, directories, anyhow, thiserror, uom, clap, printpdf, genpdf,
typst, image, csv, plotters, cargo-dist, cargo-bundle, cargo-packager, apple-codesign,
cargo-about, cargo-deny, cargo-release, cargo-nextest, proptest, insta, criterion, divan,
wasm-bindgen, wasm-bindgen-futures, console_error_panic_hook, trunk, bytemuck, encase,
pollster, rfd, fluent, rust-i18n, sys-locale, accesskit, wesl, cubecl, liveplot; and crate
searches `?q=ecg`, `?q=cardiac`, `?q=electrophysiology`, `?q=monodomain`, `?q=heart`, `?q=dicom`.

GitHub REST API (`/repos/<owner>/<repo>`) for: bevyengine/bevy, gfx-rs/wgpu, emilk/egui,
asny/three-d, BVE-Reborn/rend3, dimforge/kiss3d, FyroxEngine/Fyrox, iced-rs/iced,
slint-ui/slint, DioxusLabs/dioxus, linebender/xilem, vladbat00/bevy_egui, emilk/egui_plot,
anhosh/egui_dock, axodotdev/cargo-dist, crabnebula-dev/cargo-packager,
indygreg/apple-platform-rs (+ `/releases/latest`), burtonageo/cargo-bundle, gltf-rs/gltf,
rust-lang/rust (`/releases/latest`); and `/search/repositories` for "cardiac electrophysiology
language:rust", "ecg language:rust", "heart simulation language:rust", "monodomain OR
bidomain OR eikonal language:rust", "volume rendering wgpu", "medical bevy OR dicom viewer
language:rust", "reaction diffusion wgpu compute", "rerun egui plot".

Documents:
- https://bevy.org/news/bevy-0-20/ (release 2026-10-08: PanOrbitCamera upstreamed, WESL adoption, Feathers widgets, mesh shaders not on web)
- https://raw.githubusercontent.com/bevyengine/bevy/main/docs/cargo_features.md (webgl2/webgpu/dynamic_linking/wayland/x11/mesh_picking features)
- https://raw.githubusercontent.com/vladbat00/bevy_egui/main/README.md (compatibility table; Linux xcb packages; colour test caveat)
- https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/CHANGELOG.md (v30.0.0 2026-07-01 breaking changes; v30.0.1 2026-08-21 Vulkan fence fix; WebGL/GLES notes; MSRV policy)
- https://raw.githubusercontent.com/emilk/egui/main/CHANGELOG.md (0.36 2026-08-05 MSRV 1.95; 0.37 colour emoji/system fonts, accesskit::Role, StyleProvider, default_fonts no longer default)
- https://raw.githubusercontent.com/axodotdev/cargo-dist/main/README.md and https://axodotdev.github.io/cargo-dist/book/installers/index.html (installers: shell, powershell, npm, homebrew, msi; no .app/.dmg/AppImage)
- https://axodotdev.github.io/cargo-dist/changelog/v0.29.0 (via search: Astral fork merged, Axo Releases removed)
- https://raw.githubusercontent.com/crabnebula-dev/cargo-packager/main/README.md (formats per OS; updater; framework-agnostic)
- https://raw.githubusercontent.com/actions/runner-images/main/README.md (runner labels, macOS 14 deprecated, macOS 26 = macos-latest arm64)
- https://raw.githubusercontent.com/Chaste/Chaste/develop/LICENSE (BSD-3-Clause)
- https://opencarp.org/download/license (APL-1.1 non-commercial; Apache-2.0 tooling)
- https://web.dev/blog/webgpu-supported-major-browsers (Firefox 141 Windows / 145 Apple Silicon; Safari 26; Linux in progress)
- https://github.com/emilk/egui/issues/1485 and r/rust "Rerun 0.13 realtime kHz time series" thread (egui_plot tessellation bottleneck; downsample advice)
- https://awesome.ecosyste.ms/projects/github.com%2Fkylc%2Fegui_wgpu_plot (GPU line plotting experiment)
- https://github.com/rust-windowing/winit/releases/tag/v0.31.0-beta.2 and docs.rs winit 0.31.0-beta docs (ApplicationHandler changes)
- https://www.physionet.org/content/ecgsyn/1.0.0/ (ECGSYN, GPL reference code)
- https://www.ncbi.nlm.nih.gov/pmc/articles/PMC3597547/ (Chaste overview)
