//! egui widgets. Keep them stateless where possible (state lives in the app model) so they can
//! be tested with `egui_kittest` and reused in the WASM build.

pub mod anatomy_panel;
pub mod annotations;
pub mod axis_dial;
pub mod calipers;
pub mod colour_bar;
pub mod ecg_view;
pub mod electrode_editor;
pub mod pacer_panel;
pub mod snapshot;
pub mod strip;
pub mod twelve_lead;
pub mod vector_view;
pub mod viewport;

pub use anatomy_panel::{AnatomyPanelState, CameraCommand, LayerToggle, anatomy_panel};
pub use annotations::{EcgAnnotations, interval_readout};
pub use axis_dial::{AxisDialStyle, axis_dial, lead_polarity};
pub use calipers::{Caliper, CaliperReading, CaliperSet, Playback};
pub use colour_bar::{colour_bar, lut_colour};
pub use ecg_view::EcgView;
pub use electrode_editor::{EditorCommand, ElectrodeEditor, electrode_editor_panel};
pub use pacer_panel::{PaceMark, PaceMarks, PacerPanelResponse, PacerPanelState, lead_glyphs, pacer_panel};
pub use strip::{StripStyle, StripWindow, SweepMode, draw_strip, ecg_strip};
pub use twelve_lead::{LeadOrder, TwelveLeadLayout, lead_column, page_cells, twelve_lead_page};
pub use vector_view::{VectorPlane, VectorViewStyle, full_scale_mv, vector_plane};
pub use viewport::{Viewport3d, ViewportResources};
