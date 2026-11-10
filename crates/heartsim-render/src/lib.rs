//! wgpu rendering. The scene is drawn into an **offscreen colour + depth texture** (so we own
//! depth testing, MSAA and clear colour independently of the GUI pass) and then blitted into the
//! GUI through a textured quad. `heartsim-ui` wraps this in an `egui_wgpu::CallbackTrait`.
//!
//! The scene is a list of [`Layer`]s (one mesh each, with visibility, opacity, colour map, scalar
//! window, clip flag) plus instanced [`GlyphSet`]s (spheres and tubes). Per-vertex scalar state
//! (V_m, activation time, APD) is a separate `VERTEX` buffer per layer updated each frame with
//! `queue.write_buffer`; the fragment shader maps it through a colour LUT row. Geometry buffers are
//! uploaded once. Colours are linear RGB throughout; the target is sRGB.

pub mod camera;
mod capture;
pub mod glyph;
pub mod layer;
pub mod lut;
pub mod pick;
pub mod scalar_map;
pub mod scene;

pub use camera::{CameraPreset, OrbitCamera, oblique_yaw_deg};
pub use glyph::{GlyphInstance, GlyphSet, GlyphSetId};
pub use layer::{Contour, Layer, LayerDesc, LayerId, MeshGpu, ScalarMode, srgb_color, srgb_to_linear};
pub use lut::{LutId, LutStyle};
pub use pick::Hit;
pub use scalar_map::{VertexSmoother, map_hoods};
pub use scene::{
    ClipPlane, SceneOptions, SceneRenderer, SceneSettings, SceneUniforms, ScreenLabel, best_msaa_samples,
};
