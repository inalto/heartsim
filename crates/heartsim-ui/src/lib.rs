//! egui widgets. Keep them stateless where possible (state lives in the app model) so they can
//! be tested with `egui_kittest` and reused in the WASM build.

pub mod anatomy_panel;
pub mod axis_dial;
pub mod colour_bar;
pub mod strip;
pub mod twelve_lead;
pub mod viewport;

pub use anatomy_panel::{AnatomyPanelState, CameraCommand, LayerToggle, anatomy_panel};
pub use axis_dial::{AxisDialStyle, axis_dial, lead_polarity};
pub use colour_bar::{colour_bar, lut_colour};
pub use strip::{StripStyle, ecg_strip};
pub use twelve_lead::{TwelveLeadLayout, lead_column, twelve_lead_page};
pub use viewport::{Viewport3d, ViewportResources};
