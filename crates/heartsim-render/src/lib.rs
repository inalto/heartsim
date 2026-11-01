//! wgpu rendering. The scene is drawn into an **offscreen colour + depth texture** (so we own
//! depth testing, MSAA and clear colour independently of the GUI pass) and then blitted into the
//! GUI through a textured quad. `heartsim-ui` wraps this in an `egui_wgpu::CallbackTrait`.
//!
//! Per-vertex scalar state (V_m, activation time, APD) is a separate `STORAGE`/`VERTEX` buffer
//! updated each frame with `queue.write_buffer`; the fragment shader maps it through a colour
//! LUT. Geometry buffers are uploaded once.

pub mod camera;
pub mod lut;
pub mod scene;

pub use camera::OrbitCamera;
pub use scene::{SceneRenderer, SceneUniforms};
