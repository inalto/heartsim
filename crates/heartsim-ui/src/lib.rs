//! egui widgets. Keep them stateless where possible (state lives in the app model) so they can
//! be tested with `egui_kittest` and reused in the WASM build.

pub mod strip;
pub mod viewport;

pub use strip::{StripStyle, ecg_strip};
pub use viewport::{Viewport3d, ViewportResources};
