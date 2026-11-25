//! The Echo page (brief B31, echo simulator phase 2): a transducer on the figure's chest, the
//! plane it cuts drawn as a sector in the 3-D heart, and the cross-section of the beating model
//! heart in that plane.
//!
//! **Two ways to look at the same plane** ([`DisplayMode`]). *Image* (the default, brief B32) is a
//! **simulated B-mode appearance from a simple line model** (`heartsim_echo::bmode`), not a
//! recording: speckle that rides with the tissue, shadows, enhancement, depth fading and the
//! controls of an echo machine (frequency, gain, time-gain sliders, dynamic range, focus).
//! *Anatomy* (B31) is an anatomical cross-section: which tissue of the model lies at each point
//! of the plane (flat colours, one per label; ribs and lung hide what lies behind them
//! geometrically, hatched). *Both* blends the colours over the grey image, with the legend.
//!
//! **One source of truth, one evaluation.** The wall motion is evaluated once per drawn frame by
//! [`HeartView::with_scene`] (the same policy and rate cap as the Simulator page); the slice reads
//! that evaluated frame ([`crate::heart_motion::HeartMotion::echo_update`]) and never evaluates
//! motion itself. The slice is refreshed when the evaluation, the pose or (motion off) the drawn
//! time changed, never otherwise, and only while this page is shown. The grey image is computed
//! from that slice (the same frame: it reads the label image and the slice state, never the
//! motion) only in *Image* or *Both*, only while the page is shown, and only when the slice, the
//! settings or the display changed. With **reduced motion** on, the frame is held still: the
//! slice and the image are redrawn when the probe or the settings change, not as the beat plays.
//!
//! **Settings of the image** (frequency, gain, dynamic range, focus, time-gain sliders, the
//! display mode) live in this struct for the session and are not stored, like the probe pose;
//! depth and sector width are the probe pose's (one source of truth: the settings take them from
//! the pose every frame).
//!
//! **The probe.** Dragging the glyph on the chest slides it over the figure's trunk (the skin of
//! the pack's human figure, as on the Electrodes page); three handles on the glyph tilt, rock and
//! rotate it; sliders give the same in degrees; four buttons start from the windows' poses.
//! Keys: arrows slide, Shift+↑↓ tilt, Shift+←→ rock, `[` and `]` rotate. The pose lives in this
//! struct for the session and is not stored.

use std::sync::Arc;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2 as EVec2};
use glam::{Vec2, Vec3, Vec4};
use heartsim_echo::probe::{ANGLE_LIMIT_DEG, DEPTH_RANGE_MM, SECTOR_RANGE_DEG};
use heartsim_echo::{
    AcousticTable, EchoAnatomy, EchoImager, EchoWindow, GreyImage, ImageSettings, LabelImage, ProbeFrame,
    ProbePose, SliceState, TGC_BANDS,
};
use heartsim_geometry::pack::{BodyRegion, body_region};
use heartsim_geometry::{Label, TriMesh, Vertex};
use heartsim_render::{
    GlyphInstance, GlyphSetId, LayerDesc, LayerId, LutStyle, OrbitCamera, PALETTE_SLOTS, SceneRenderer,
    SectionLayout, SectionMode, SectionRenderer, srgb_color,
};
use heartsim_ui::{Viewport3d, ViewportResources, cards, t};

mod measure;
mod views;

use crate::echo_colours;
use crate::heart_motion;
use crate::heart_view::HeartView;

pub use measure::{
    Demo, EcgTrace, Entry, MeasureMode, Reading, SNAP_MM, SendRequest, Tool, name_of, site_name, snap_depth,
    snap_plane, trace_scale,
};

/// Lines and samples of the label image **and of the grey image** (they share the layout): the
/// brief's default natively; on the web the samples are halved (128 × 256: the browser is
/// single-threaded and several times slower; the slice and the line model cost in proportion to
/// the samples) and the whole chain (evaluation, slice, image) is capped at 20 Hz by
/// [`heart_motion::min_interval_s`]. Designed, not measured on the web: see the reports of
/// briefs B31 part B and B32.
#[must_use]
pub const fn image_size() -> (usize, usize) {
    if cfg!(target_arch = "wasm32") {
        (128, 256)
    } else {
        (
            heartsim_echo::slice::DEFAULT_LINES,
            heartsim_echo::slice::DEFAULT_SAMPLES,
        )
    }
}

/// Distance on screen at which a press grabs the probe or one of its handles, logical px.
const GRAB_PX: f32 = 13.0;
/// Degrees of tilt, rock or rotation per pixel of handle drag.
const DEG_PER_PX: f32 = 0.5;
/// Distance of the tilt and rock handles from the probe on screen, logical px (at least; at most
/// the longer one).
const HANDLE_NEAR_PX: f32 = 48.0;
const HANDLE_FAR_PX: f32 = 84.0;
/// Radius of the rotation ring around the probe on screen, logical px.
const RING_PX: f32 = 26.0;
/// Slide per arrow press, mm.
const SLIDE_STEP_MM: f32 = 4.0;
/// Tilt and rock per modified-arrow press, rotation per bracket press, degrees.
const ANGLE_STEP_DEG: f32 = 3.0;
const ROTATE_STEP_DEG: f32 = 5.0;
/// The 3-D view keeps this much width, the cross-section this much, logical px.
const VIEW_MIN_PX: f32 = 220.0;
/// Free space around the framed chest, fraction of its size.
const FRAME_MARGIN: f32 = 0.05;
/// Half-extent of the box the camera frames around the heart box's centre, mm.
const FRAME_HALF_MM: Vec3 = Vec3::new(170.0, 170.0, 130.0);
/// Torso opacity and tint (sRGB) while the probe is placed on it.
const TORSO_ALPHA: f32 = 0.22;
const TORSO_TINT: [f32; 3] = [0.45, 0.74, 0.96];
/// Opacity of the sector quad in the 3-D view, and its colours (sRGB): teal with contact, grey
/// without.
const SECTOR_ALPHA: f32 = 0.38;
const SECTOR_TEAL: [f32; 3] = [0.1, 0.78, 0.8];
const SECTOR_GREY: [f32; 3] = [0.55, 0.55, 0.58];
/// The ghost of the target view's plane (brief B33): magenta, fainter than the live sector.
const GHOST_ALPHA: f32 = 0.22;
const GHOST_MAGENTA: [f32; 3] = [0.85, 0.2, 0.75];
/// The index marker's colour (sRGB): the edge of the sector on the marker side and the bump on
/// the probe.
const MARKER_ORANGE: [f32; 3] = [1.0, 0.55, 0.1];
/// Segments of the sector's arc.
const ARC_SEGMENTS: usize = 24;
/// Brightness of hidden samples in the cross-section, and the period of their hatch, logical px.
const HIDDEN_BRIGHTNESS: f32 = 0.5;
const HATCH_PX: f32 = 6.0;
/// Opacity of the tissue colours over the grey image in [`DisplayMode::Both`].
const OVERLAY_OPACITY: f32 = heartsim_render::DEFAULT_OVERLAY;
/// The focus marker's colour on the depth scale.
const FOCUS_TEAL: Color32 = Color32::from_rgb(30, 190, 200);

/// What the right pane shows of the plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    /// The simulated grey image alone.
    #[default]
    Image,
    /// The anatomical cross-section (flat colours, one per tissue).
    Anatomy,
    /// The colours blended over the grey image, with the legend.
    Both,
}

impl DisplayMode {
    /// Every mode, in the order of the switch.
    pub const ALL: [Self; 3] = [Self::Image, Self::Anatomy, Self::Both];

    /// Whether the grey image is part of what is shown (and so has to be computed).
    #[must_use]
    pub fn shows_image(self) -> bool {
        self != Self::Anatomy
    }

    /// Whether the tissue colours are part of what is shown.
    #[must_use]
    pub fn shows_colours(self) -> bool {
        self != Self::Image
    }

    fn section(self) -> SectionMode {
        match self {
            Self::Image => SectionMode::Image,
            Self::Anatomy => SectionMode::Anatomy,
            Self::Both => SectionMode::Both,
        }
    }

    /// The name on the switch, in the interface language.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Image => t!("echo-display-image"),
            Self::Anatomy => t!("echo-display-anatomy"),
            Self::Both => t!("echo-display-both"),
        }
    }
}

/// What the left column shows: one section at a time, chosen with the segmented control at its
/// top. Remembered for the session (not stored).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum EchoSection {
    /// The windows, the probe's angles, depth and sector, and the view switches (cut, wall motion).
    #[default]
    Probe,
    /// The controls of the simulated image (frequency, gain, dynamic range, focus, time-gain).
    Image,
    /// The standard views: practise and self-test.
    Views,
    /// M-mode, annular tissue Doppler, calipers and the model's own values to compare with (B34).
    Measure,
}

impl EchoSection {
    /// Every section, in the order of the control.
    pub const ALL: [Self; 4] = [Self::Probe, Self::Image, Self::Views, Self::Measure];

    /// The name on the control, in the interface language.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Probe => t!("echo-section-probe"),
            Self::Image => t!("echo-section-image"),
            Self::Views => t!("echo-section-views"),
            Self::Measure => t!("echo-section-measure"),
        }
    }
}

/// The phone layout: the height of the Image / 3-D switch and of the display switch with its note, px.
const SWITCH_PX: f32 = 40.0;
const DISPLAY_PX: f32 = 62.0;
/// The phone layout keeps at least this much of the page for the section control and its section.
const CONTROLS_MIN_PX: f32 = 150.0;

/// Height kept under the cross-section for the read-out line and the legend, logical px. The same
/// in every display mode, so the image keeps its rectangle when the mode changes.
const LEGEND_PX: f32 = 92.0;

/// What a drag started on.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    /// The probe itself: slide over the skin.
    Slide,
    /// The tilt handle (pixels along the screen direction of the elevation axis).
    Tilt,
    /// The rock handle (along the marker axis).
    Rock,
    /// The rotation ring: the angle of the pointer about the probe, radians, at the last frame.
    Rotate(f32),
}

/// What the pointer reads over the cross-section.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReadOut {
    pub label: Label,
    /// Behind the first bone or lung sample of its line.
    pub hidden: bool,
    /// Depth below the skin, mm.
    pub depth_mm: f32,
}

/// The slice as last made: what it was made from.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drawn {
    key: (u64, u64),
    pose: ProbePose,
}

/// What the grey image was made from: the slice (its generation) and the settings, with the depth
/// and sector of that slice.
#[derive(Debug, Clone, PartialEq)]
struct GreyKey {
    generation: u64,
    settings: ImageSettings,
}

/// What the page keeps of the 3-D scene while it is shown.
#[derive(Debug, Default)]
struct SceneObjects {
    /// This page's camera while another page is shown, and the other pages' while this one is.
    own_camera: Option<OrbitCamera>,
    saved_camera: Option<OrbitCamera>,
    /// The camera as the page last framed it, and the viewport aspect it was framed for.
    fit: Option<(f32, OrbitCamera)>,
    glyphs: Option<GlyphSetId>,
    sector: Option<LayerId>,
    /// The ghost of the target view's plane (B33).
    ghost: Option<LayerId>,
    torso: Option<LayerId>,
    /// The torso layer's clip flag before the page turned it off.
    torso_clip: bool,
    /// The camera of the last frame, for the handles and the picking.
    camera_now: Option<OrbitCamera>,
}

/// State of the Echo page.
#[derive(Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one user switch and three independent frame states"
)]
pub struct EchoPage {
    pose: Option<ProbePose>,
    window: Option<EchoWindow>,
    /// Clip the heart at the image plane (the existing clip plane), so the cut matches the slice.
    pub clip: bool,
    state: Option<SliceState>,
    image: LabelImage,
    texels: Arc<Vec<u8>>,
    /// Bumped with every slice; the GPU copy is current when `uploaded == generation`.
    generation: u64,
    uploaded: u64,
    drawn: Option<Drawn>,
    last_slice_s: f64,
    /// The anatomy is being built (one frame of "preparing" is shown first).
    warming: bool,
    /// What the right pane shows. Not stored.
    mode: DisplayMode,
    /// The image controls (depth and sector are overwritten from the pose every frame). Not stored.
    settings: ImageSettings,
    imager: Option<EchoImager>,
    grey: GreyImage,
    grey_texels: Arc<Vec<u8>>,
    /// Bumped with every grey image; the GPU copy is current when `grey_uploaded` equals it.
    grey_generation: u64,
    grey_uploaded: u64,
    grey_drawn: Option<GreyKey>,
    last_grey_s: f64,
    /// Grey images computed (tests, the cost report).
    greys: u64,
    scene: SceneObjects,
    active: bool,
    drag: Option<Drag>,
    /// The layout of the cross-section as last drawn, for the pointer read-out.
    layout: Option<SectionLayout>,
    hover: Option<ReadOut>,
    /// Frames in which the slice was recomputed (tests, the cost report).
    slices: u64,
    /// Aspect of the 3-D view as last laid out (the next frame's camera framing uses it).
    view_aspect: f32,
    /// A window to start from once the anatomy is built (the documentation tour, tests).
    pending_window: Option<EchoWindow>,
    /// The press that is going on was a grab of the probe or a handle: its release is no click.
    press_grabbed: bool,
    /// The standard views (B33): target, score, hint, checklist, self-test.
    views: views::ViewsState,
    /// The section the left column shows.
    section: EchoSection,
    /// The probe was dragged, clicked on the skin or moved with the keys this session: the drag
    /// hint over the 3-D view is no longer shown (it stays in the Probe section's "More").
    probe_touched: bool,
    /// Where the cross-section was last drawn, in screen points.
    section_rect: Option<Rect>,
    /// The phone layout shows the 3-D chest view instead of the image (the switch above it).
    narrow_3d: bool,
    /// M-mode, tissue Doppler, calipers and the list of measurements (B34).
    measure: measure::MeasureState,
}

impl Default for EchoPage {
    fn default() -> Self {
        let (lines, samples) = image_size();
        Self {
            pose: None,
            window: None,
            clip: false,
            state: None,
            image: LabelImage::new(lines, samples),
            texels: Arc::new(Vec::new()),
            generation: 0,
            uploaded: u64::MAX,
            drawn: None,
            last_slice_s: f64::NEG_INFINITY,
            warming: false,
            mode: DisplayMode::default(),
            settings: default_settings(),
            imager: None,
            grey: GreyImage::new(lines, samples),
            grey_texels: Arc::new(Vec::new()),
            grey_generation: 0,
            grey_uploaded: u64::MAX,
            grey_drawn: None,
            last_grey_s: f64::NEG_INFINITY,
            greys: 0,
            scene: SceneObjects::default(),
            active: false,
            drag: None,
            layout: None,
            hover: None,
            slices: 0,
            view_aspect: 1.4,
            pending_window: None,
            press_grabbed: false,
            views: views::ViewsState::default(),
            section: EchoSection::default(),
            probe_touched: false,
            section_rect: None,
            narrow_3d: false,
            measure: measure::MeasureState::default(),
        }
    }
}

/// The image settings the page starts from and returns to: the model's defaults at the page's
/// image size.
fn default_settings() -> ImageSettings {
    let (lines, samples) = image_size();
    ImageSettings {
        lines,
        samples,
        ..ImageSettings::default()
    }
}

// ---- pure helpers ------------------------------------------------------------------------

/// The ray `(origin, unit direction)` through the widget-relative point `uv` of a viewport of
/// `aspect` seen by `camera`: what [`SceneRenderer::pick_ray`] does, for the camera of the last
/// frame.
fn pick_ray(camera: &OrbitCamera, uv: (f32, f32), aspect: f32) -> (Vec3, Vec3) {
    let inv = camera.view_projection(aspect).inverse();
    let ndc = Vec4::new(uv.0 * 2.0 - 1.0, 1.0 - uv.1 * 2.0, 1.0, 1.0);
    let far = inv * ndc;
    let far = far.truncate() / far.w;
    let eye = camera.eye();
    (eye, (far - eye).normalize_or(Vec3::NEG_Z))
}

/// The screen point of world point `p` in `rect`, if it is in front of the camera.
fn to_screen(camera: &OrbitCamera, aspect: f32, rect: Rect, p: Vec3) -> Option<Pos2> {
    let (u, v, _) = camera.project(p, aspect)?;
    Some(rect.min + EVec2::new(u * rect.width(), v * rect.height()))
}

fn v2(p: Pos2) -> Vec2 {
    Vec2::new(p.x, p.y)
}

/// A screen direction of at least [`HANDLE_NEAR_PX`] and at most [`HANDLE_FAR_PX`], from `from`
/// toward `to`; `fallback` when the two coincide on screen (the axis points at the camera).
fn handle_vector(from: Pos2, to: Option<Pos2>, fallback: Vec2) -> Vec2 {
    let d = to.map_or(Vec2::ZERO, |t| v2(t) - v2(from));
    if d.length() < 4.0 {
        return fallback.normalize_or(Vec2::X) * HANDLE_NEAR_PX;
    }
    d.normalize() * d.length().clamp(HANDLE_NEAR_PX, HANDLE_FAR_PX)
}

/// The tangent plane axes at a skin point with outward `normal`: `(cranial, patient's left)`.
fn skin_axes(normal: Vec3) -> (Vec3, Vec3) {
    let up = Vec3::Y - normal * normal.dot(Vec3::Y);
    let up = up.normalize_or(Vec3::Z);
    // looking at the skin from outside with the head up, the patient's left is on the right
    (up, up.cross(normal).normalize_or(Vec3::X))
}

/// The pose after `deg` more tilt (clamped). Positive tilt fans the beam toward `inward × marker`,
/// the side of the image plane opposite the frame's `elevation` axis.
fn with_tilt(p: &ProbePose, deg: f32) -> ProbePose {
    ProbePose {
        tilt_deg: p.tilt_deg + deg,
        ..*p
    }
    .clamped()
}

/// The pose after `deg` more rock (clamped). Positive rock swings the beam toward the marker.
fn with_rock(p: &ProbePose, deg: f32) -> ProbePose {
    ProbePose {
        rock_deg: p.rock_deg + deg,
        ..*p
    }
    .clamped()
}

/// The pose after `deg` more rotation about the beam (wrapped).
fn with_rotation(p: &ProbePose, deg: f32) -> ProbePose {
    ProbePose {
        rotation_deg: p.rotation_deg + deg,
        ..*p
    }
    .clamped()
}

/// The fan of the sector in the image plane as a mesh: the apex, then `ARC_SEGMENTS + 1` points
/// of the arc; both windings, so it shows from either side.
fn sector_mesh(frame: &ProbeFrame, depth_mm: f32, sector_deg: f32) -> TriMesh {
    let mut mesh = TriMesh::default();
    let normal = frame.elevation.to_array();
    mesh.vertices.push(Vertex {
        position: frame.apex_mm.to_array(),
        normal,
    });
    for i in 0..=ARC_SEGMENTS {
        let f = i as f32 / ARC_SEGMENTS as f32 - 0.5;
        let (s, c) = (f * sector_deg).to_radians().sin_cos();
        let p = frame.apex_mm + (frame.beam * c + frame.marker * s) * depth_mm;
        mesh.vertices.push(Vertex {
            position: p.to_array(),
            normal,
        });
    }
    for i in 0..ARC_SEGMENTS as u32 {
        mesh.indices.extend_from_slice(&[0, 1 + i, 2 + i]);
        mesh.indices.extend_from_slice(&[0, 2 + i, 1 + i]);
    }
    mesh
}

/// The vertices of [`sector_mesh`] only (what is rewritten every time the pose changes).
fn sector_vertices(frame: &ProbeFrame, depth_mm: f32, sector_deg: f32) -> Vec<Vertex> {
    sector_mesh(frame, depth_mm, sector_deg).vertices
}

// ---- the paint callback of the cross-section ---------------------------------------------

struct SectionCallback {
    upload: Option<Arc<Vec<u8>>>,
    upload_grey: Option<Arc<Vec<u8>>>,
    lines: u32,
    samples: u32,
    mode: DisplayMode,
    layout: SectionLayout,
    palette: Vec<[u8; 3]>,
}

impl egui_wgpu::CallbackTrait for SectionCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(r) = resources.get_mut::<SectionRenderer>() {
            if let Some(texels) = &self.upload {
                r.set_image(device, queue, self.lines, self.samples, texels);
            }
            if let Some(pixels) = &self.upload_grey {
                r.set_grey(device, queue, self.lines, self.samples, pixels);
            }
            r.set_palette(&self.palette);
            r.set_mode(self.mode.section(), OVERLAY_OPACITY);
            r.set_layout(queue, &self.layout, HIDDEN_BRIGHTNESS, HATCH_PX);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(r) = resources.get::<SectionRenderer>() {
            r.draw(pass);
        }
    }
}

impl EchoPage {
    // ---- state ---------------------------------------------------------------------------

    /// The probe pose (`None` until the anatomy has been built).
    #[must_use]
    pub fn pose(&self) -> Option<ProbePose> {
        self.pose
    }

    /// Put the probe at `pose` (clamped), as a drag, a slider or a window button does.
    pub fn set_pose(&mut self, pose: ProbePose) {
        self.pose = Some(pose.clamped());
    }

    /// The window whose button was last pressed and whose pose has not been moved since.
    #[must_use]
    pub fn window(&self) -> Option<EchoWindow> {
        self.window
    }

    /// The label image of the last slice.
    #[must_use]
    pub fn image(&self) -> &LabelImage {
        &self.image
    }

    /// What the right pane shows.
    #[must_use]
    pub fn display(&self) -> DisplayMode {
        self.mode
    }

    /// Choose what the right pane shows (the switch, the documentation tour).
    pub fn set_display(&mut self, mode: DisplayMode) {
        self.mode = mode;
    }

    /// The image controls as they are (depth and sector as of the last frame).
    #[must_use]
    pub fn settings(&self) -> &ImageSettings {
        &self.settings
    }

    /// Replace the image controls (clamped to their ranges, as a slider would).
    pub fn set_settings(&mut self, settings: &ImageSettings) {
        let (lines, samples) = image_size();
        self.settings = ImageSettings {
            lines,
            samples,
            ..settings.clamped()
        };
    }

    /// The grey image as last computed (all black before the first).
    #[must_use]
    pub fn grey(&self) -> &GreyImage {
        &self.grey
    }

    /// How many times the grey image was computed (zero until it is shown).
    #[must_use]
    pub fn grey_count(&self) -> u64 {
        self.greys
    }

    /// Back to the model's default image controls and the default depth and sector width (the
    /// display mode and the probe's position are kept).
    pub fn reset_image(&mut self) {
        self.settings = default_settings();
        if let Some(p) = self.pose.as_mut() {
            p.depth_mm = self.settings.depth_mm;
            p.sector_deg = self.settings.sector_deg;
        }
    }

    /// How many times the slice was recomputed (zero before the first drawn frame).
    #[must_use]
    pub fn slice_count(&self) -> u64 {
        self.slices
    }

    /// Start from a window: the window's pose with the imaging depth and sector width kept.
    pub fn go_to_window(&mut self, anatomy: &EchoAnatomy, w: EchoWindow) {
        let cur = self.pose.unwrap_or_default();
        self.pose = Some(
            ProbePose {
                depth_mm: cur.depth_mm,
                sector_deg: cur.sector_deg,
                ..anatomy.window_pose(w)
            }
            .clamped(),
        );
        self.window = Some(w);
    }

    /// Start from `w` as soon as the anatomy is built (now, if it is).
    pub fn request_window(&mut self, w: EchoWindow) {
        self.pending_window = Some(w);
    }

    /// Back to the apical start with the default depth and sector.
    pub fn reset(&mut self, anatomy: &EchoAnatomy) {
        self.pose = Some(anatomy.window_pose(EchoWindow::Apical));
        self.window = Some(EchoWindow::Apical);
    }

    /// The section the left column shows.
    #[must_use]
    pub fn section(&self) -> EchoSection {
        self.section
    }

    /// Show `section` in the left column (the control, the tour; asking for a view and starting
    /// the self-test choose [`EchoSection::Views`]).
    pub fn set_section(&mut self, section: EchoSection) {
        self.section = section;
    }

    /// Whether the drag hint is drawn over the 3-D view: until the probe is first dragged, clicked
    /// on or moved with the keys in this session.
    #[must_use]
    pub fn drag_hint_shown(&self) -> bool {
        !self.probe_touched
    }

    /// Where the cross-section (the image itself, without the controls, read-out and legend) was
    /// last drawn, in screen points. The same in the three display modes.
    #[must_use]
    pub fn section_rect(&self) -> Option<Rect> {
        self.section_rect
    }

    /// Where the sector sits in the cross-section's widget, as last drawn.
    #[must_use]
    pub fn layout(&self) -> Option<SectionLayout> {
        self.layout
    }

    /// Whether the page has taken the 3-D scene (it is the page being shown).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The tissue under the widget point `p_px` of the cross-section as last drawn.
    #[must_use]
    pub fn read_out_at(&self, p_px: Vec2) -> Option<ReadOut> {
        let layout = self.layout?;
        if !self.image.contact {
            return None;
        }
        let (line, sample, depth_mm) = layout.locate(p_px, self.image.lines, self.image.samples)?;
        Some(ReadOut {
            label: self.image.label(line, sample),
            hidden: self.image.is_hidden(line, sample),
            depth_mm,
        })
    }

    // ---- per-frame work -------------------------------------------------------------------

    /// Slide the probe to the skin point nearest `p`, when `p` is on the trunk.
    fn slide_to(&mut self, anatomy: &EchoAnatomy, p: Vec3) {
        let Some(pose) = self.pose else { return };
        if body_region(p) != BodyRegion::Trunk {
            return;
        }
        let moved = anatomy.slide_to(&pose, p);
        if body_region(moved.skin_mm) == BodyRegion::Trunk {
            self.pose = Some(moved);
            self.window = None;
        }
    }

    /// The keys of the page (only while nothing has the keyboard focus).
    fn keys(&mut self, ctx: &egui::Context, anatomy: &EchoAnatomy) {
        use egui::Key;
        if ctx.egui_wants_keyboard_input() || ctx.memory(egui::Memory::focused).is_some() {
            return;
        }
        let (left, right, up, down, open, close, shift) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::OpenBracket),
                i.key_pressed(Key::CloseBracket),
                i.modifiers.shift,
            )
        });
        let Some(pose) = self.pose else { return };
        let mut next = pose;
        let sign = |neg: bool, pos: bool| f32::from(u8::from(pos)) - f32::from(u8::from(neg));
        let (dx, dy) = (sign(left, right), sign(down, up));
        if shift {
            next = with_tilt(&next, dy * ANGLE_STEP_DEG);
            next = with_rock(&next, dx * ANGLE_STEP_DEG);
        } else if dx != 0.0 || dy != 0.0 {
            let (cranial, patient_left) = skin_axes(pose.normal);
            let target = pose.skin_mm + (patient_left * dx + cranial * dy) * SLIDE_STEP_MM;
            self.slide_to(anatomy, target);
            next = self.pose.unwrap_or(pose);
        }
        next = with_rotation(&next, sign(open, close) * ROTATE_STEP_DEG);
        if next != pose {
            self.probe_touched = true;
            if (next.skin_mm - pose.skin_mm).length_squared() < 1e-9 {
                self.window = None;
            }
            self.pose = Some(next);
        }
    }

    /// Refresh the slice if the evaluation, the pose or (motion off) the drawn time changed and
    /// the rate cap allows; otherwise nothing is computed.
    fn refresh_slice(&mut self, ui: &egui::Ui, heart: &HeartView) {
        let (Some(anatomy), Some(motion), Some(pose)) = (heart.echo_anatomy(), heart.motion(), self.pose)
        else {
            return;
        };
        // a frozen sector holds its image: calipers are put on a still picture (B34)
        if self.measure.frozen && self.drawn.is_some() {
            return;
        }
        let pose = pose.clamped();
        let mut key = motion.echo_key();
        let mut new_time = self.drawn.as_ref().is_none_or(|d| d.key != key);
        let new_pose = self.drawn.as_ref().is_none_or(|d| d.pose != pose);
        // Reduced motion holds the frame: the beat does not redraw it (the wall is at rest, and
        // the leaflets and the image with it); a new probe pose or setting does. A new evaluation
        // serial (another scenario, the motion switched on) thaws it.
        if motion.reduced_motion
            && new_time
            && let Some(d) = &self.drawn
            && d.key.0 == key.0
        {
            new_time = false;
            key = d.key;
        }
        if !new_time && !new_pose {
            return;
        }
        // An evaluation was already rate-capped by the motion policy; a pose change, or the
        // leaflets of the rest heart following the clock, are capped here by the same interval.
        let capped = key.1 != 0 || (new_pose && !new_time);
        let now = ui.input(|i| i.time);
        let since = now - self.last_slice_s;
        if capped && self.drawn.is_some() && since < heart_motion::min_interval_s() {
            ui.ctx()
                .request_repaint_after_secs((heart_motion::min_interval_s() - since) as f32);
            return;
        }
        self.last_slice_s = now;
        let state = self.state.get_or_insert_with(|| SliceState::new(anatomy));
        if new_time {
            motion.echo_update(state, anatomy);
        }
        state.slice(anatomy, &pose, &mut self.image);
        self.texels = Arc::new(self.image.texels());
        self.generation += 1;
        self.slices += 1;
        self.drawn = Some(Drawn { key, pose });
    }

    /// Compute the grey image of the slice just made, if it is shown and anything it depends on
    /// changed: the slice (its generation), the image settings or the depth and sector of the
    /// slice. It reads the label image and the slice state of the same frame and never evaluates
    /// the wall motion. A change of the settings alone (a slider drag) is capped by the same
    /// interval as the slice, so a drag costs at most that rate.
    fn refresh_grey(&mut self, ui: &egui::Ui, heart: &HeartView) {
        if !self.mode.shows_image() {
            return;
        }
        let (Some(anatomy), Some(drawn), Some(state)) =
            (heart.echo_anatomy(), self.drawn, self.state.as_ref())
        else {
            return;
        };
        if self.measure.frozen && self.grey_drawn.is_some() {
            return;
        }
        let mut s = self.settings.clone();
        // the slice's own depth and sector (the pose it was cut with), and the image's size
        s.depth_mm = drawn.pose.depth_mm;
        s.sector_deg = drawn.pose.sector_deg;
        s.lines = self.image.lines;
        s.samples = self.image.samples;
        let key = GreyKey {
            generation: self.generation,
            settings: s.clamped(),
        };
        if self.grey_drawn.as_ref() == Some(&key) {
            return;
        }
        let now = ui.input(|i| i.time);
        let since = now - self.last_grey_s;
        let settings_only = self
            .grey_drawn
            .as_ref()
            .is_some_and(|d| d.generation == key.generation);
        if settings_only && since < heart_motion::min_interval_s() {
            ui.ctx()
                .request_repaint_after_secs((heart_motion::min_interval_s() - since) as f32);
            return;
        }
        self.last_grey_s = now;
        let imager = self
            .imager
            .get_or_insert_with(|| EchoImager::new(AcousticTable::builtin()));
        imager.image(
            anatomy,
            state,
            &drawn.pose,
            &self.image,
            &key.settings,
            &mut self.grey,
        );
        self.grey_texels = Arc::new(self.grey.pixels.clone());
        self.grey_generation += 1;
        self.greys += 1;
        self.grey_drawn = Some(key);
    }

    // ---- the 3-D scene ----------------------------------------------------------------------

    /// Take over the scene for this page: own camera, translucent torso, the probe and the sector.
    fn prepare_scene(
        &mut self,
        scene: &mut SceneRenderer,
        rs: &egui_wgpu::RenderState,
        aspect: f32,
        chest: (Vec3, Vec3),
    ) {
        let s = &mut self.scene;
        if s.saved_camera.is_none() {
            s.saved_camera = Some(scene.camera);
            if let Some(c) = s.own_camera {
                scene.camera = c;
            }
        }
        let same = |a: &OrbitCamera, b: &OrbitCamera| {
            a.yaw.to_bits() == b.yaw.to_bits()
                && a.pitch.to_bits() == b.pitch.to_bits()
                && a.distance.to_bits() == b.distance.to_bits()
        };
        let refit = match &s.fit {
            None => true,
            Some((a, cam)) => (a - aspect).abs() > 0.02 && same(cam, &scene.camera),
        };
        if refit {
            let mut c = scene.camera;
            c.frame_to_fit(chest.0, chest.1, aspect, FRAME_MARGIN);
            c.yaw = 0.45;
            c.pitch = 0.4;
            scene.camera = c;
            s.fit = Some((aspect, c));
        }
        let glyphs = *s.glyphs.get_or_insert_with(|| scene.add_glyph_set("echo probe"));
        if let Some(g) = scene.glyph_set_mut(glyphs) {
            g.visible = true;
            g.clip = false;
        }
        if s.sector.is_none() {
            let mesh = sector_mesh(&ProbePose::default().frame(60.0), 100.0, 60.0);
            let mut desc = LayerDesc::new("Echo sector", &mesh);
            desc.alpha = SECTOR_ALPHA;
            let id = scene.add_layer(&rs.device, &desc);
            if let Some(l) = scene.layer_mut(id) {
                l.clip = false;
                l.pickable = false;
                l.label = None;
            }
            s.sector = Some(id);
        }
        if let Some(l) = s.sector.and_then(|id| scene.layer_mut(id)) {
            l.visible = true;
        }
        if s.ghost.is_none() {
            let mesh = sector_mesh(&ProbePose::default().frame(60.0), 100.0, 60.0);
            let mut desc = LayerDesc::new("Echo target plane", &mesh);
            desc.alpha = GHOST_ALPHA;
            let id = scene.add_layer(&rs.device, &desc);
            if let Some(l) = scene.layer_mut(id) {
                l.clip = false;
                l.pickable = false;
                l.label = None;
                l.visible = false;
                let [r, g, b, _] = srgb_color([GHOST_MAGENTA[0], GHOST_MAGENTA[1], GHOST_MAGENTA[2], 1.0]);
                l.base_color = [r, g, b];
            }
            s.ghost = Some(id);
        }
        if let Some(l) = s.torso.and_then(|id| scene.layer_mut(id)) {
            l.visible = true;
            l.alpha = TORSO_ALPHA;
            let [r, g, b, _] = srgb_color([TORSO_TINT[0], TORSO_TINT[1], TORSO_TINT[2], 1.0]);
            l.base_color = [r, g, b];
            l.clip = false;
        }
        scene.clip.enabled = false;
        s.camera_now = Some(scene.camera);
    }

    /// Put the probe glyph, the sector and the clip plane where the pose says.
    fn update_overlay(
        &mut self,
        scene: &mut SceneRenderer,
        rs: &egui_wgpu::RenderState,
        frame: &ProbeFrame,
        pose: &ProbePose,
        rect: Rect,
        ghost: Option<(ProbeFrame, f32, f32)>,
    ) {
        let s = &self.scene;
        let aspect = rect.width() / rect.height().max(1.0);
        let mm_per_px =
            2.0 * scene.camera.distance * (scene.camera.fov_y * 0.5).tan() / rect.height().max(1.0);
        // the glyph keeps a minimum size on screen
        let k = (30.0 * mm_per_px / 55.0).max(1.0);
        let out = -frame.beam;
        let a = frame.apex_mm;
        let rgba = |c: [f32; 3], alpha: f32| srgb_color([c[0], c[1], c[2], alpha]);
        let body = rgba([0.2, 0.22, 0.28], 1.0);
        let face = rgba([0.85, 0.87, 0.9], 1.0);
        let marker = rgba(MARKER_ORANGE, 1.0);
        let mut spheres = vec![GlyphInstance::sphere(
            a + frame.marker * 14.0 * k + out * 8.0 * k,
            4.0 * k,
            marker,
        )];
        let mut tubes = vec![
            // the array face along the marker axis, the body and the cable along the beam axis
            GlyphInstance::tube(
                a - frame.marker * 12.0 * k + out * 3.0 * k,
                a + frame.marker * 12.0 * k + out * 3.0 * k,
                5.0 * k,
                face,
            ),
            GlyphInstance::cone(a + out * 6.0 * k, a + out * 55.0 * k, 9.0 * k, 12.0 * k, body),
            GlyphInstance::tube(a + out * 55.0 * k, a + out * 100.0 * k, 3.0 * k, body),
        ];
        // the sector outline: the arc and the two edges, the marker side in the marker colour
        let outline = rgba(SECTOR_TEAL, 1.0);
        let half = pose.sector_deg * 0.5;
        let point = |deg: f32| {
            let (sn, cs) = deg.to_radians().sin_cos();
            a + (frame.beam * cs + frame.marker * sn) * pose.depth_mm
        };
        let arc: Vec<Vec3> = (0..=ARC_SEGMENTS)
            .map(|i| point(-half + pose.sector_deg * i as f32 / ARC_SEGMENTS as f32))
            .collect();
        let (arc_spheres, arc_tubes) = heartsim_render::glyph::polyline(
            &arc,
            0.8 * k,
            if frame.contact {
                outline
            } else {
                rgba(SECTOR_GREY, 1.0)
            },
        );
        spheres.extend(arc_spheres);
        tubes.extend(arc_tubes);
        tubes.push(GlyphInstance::tube(
            a,
            point(-half),
            0.7 * k,
            rgba(SECTOR_TEAL, 1.0),
        ));
        tubes.push(GlyphInstance::tube(a, point(half), 1.6 * k, marker));
        spheres.push(GlyphInstance::sphere(point(half), 2.4 * k, marker));
        if let Some(id) = s.glyphs {
            scene.set_spheres(&rs.device, &rs.queue, id, &spheres);
            scene.set_tubes(&rs.device, &rs.queue, id, &tubes);
        }
        if let Some(id) = s.sector {
            let vertices = sector_vertices(frame, pose.depth_mm, pose.sector_deg);
            scene.set_layer_vertices(&rs.queue, id, &vertices);
            if let Some(l) = scene.layer_mut(id) {
                let c = if frame.contact { SECTOR_TEAL } else { SECTOR_GREY };
                let [r, g, b, _] = srgb_color([c[0], c[1], c[2], 1.0]);
                l.base_color = [r, g, b];
            }
        }
        if let Some(id) = s.ghost {
            if let Some((gf, depth, sector)) = &ghost {
                scene.set_layer_vertices(&rs.queue, id, &sector_vertices(gf, *depth, *sector));
            }
            if let Some(l) = scene.layer_mut(id) {
                l.visible = ghost.is_some();
            }
        }
        // the cut: remove the half of the heart on the camera's side of the image plane
        let toward_camera = (scene.camera.eye() - a).dot(frame.elevation);
        let normal = if toward_camera >= 0.0 {
            frame.elevation
        } else {
            -frame.elevation
        };
        scene.clip = heartsim_render::ClipPlane {
            enabled: self.clip && frame.contact,
            point: a,
            normal,
        };
        let _ = aspect;
    }

    /// Give the scene back to the other pages (call every frame this page is not shown).
    pub fn deactivate(&mut self, rs: Option<&egui_wgpu::RenderState>) {
        if !self.active && self.scene.saved_camera.is_none() {
            return;
        }
        self.active = false;
        self.drag = None;
        let Some(rs) = rs else { return };
        let Some(saved) = self.scene.saved_camera.take() else {
            return;
        };
        let mut renderer = rs.renderer.write();
        if let Some(res) = renderer.callback_resources.get_mut::<ViewportResources>() {
            self.scene.own_camera = Some(res.scene.camera);
            res.scene.camera = saved;
            let s = &self.scene;
            if let Some(g) = s.glyphs.and_then(|id| res.scene.glyph_set_mut(id)) {
                g.visible = false;
            }
            if let Some(l) = s.sector.and_then(|id| res.scene.layer_mut(id)) {
                l.visible = false;
            }
            if let Some(l) = s.ghost.and_then(|id| res.scene.layer_mut(id)) {
                l.visible = false;
            }
            if let Some(l) = s.torso.and_then(|id| res.scene.layer_mut(id)) {
                l.clip = s.torso_clip;
            }
            res.scene.clip.enabled = false;
        }
    }

    // ---- the page --------------------------------------------------------------------------

    /// The whole page: controls on the left, the figure and heart in the middle, the
    /// cross-section on the right (stacked on a phone-narrow window). `heart` is the app's 3-D
    /// heart (`None`: the placeholder sphere, no echo); `rs` the GPU state (`None`: the 3-D view
    /// is replaced by a note and the cross-section is drawn by egui itself, coarsely).
    /// `colour_blind` selects the colour-blind-safe palette.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        heart: Option<&mut HeartView>,
        rs: Option<&egui_wgpu::RenderState>,
        colour_blind: bool,
    ) {
        let Some(heart) = heart.filter(|h| h.echo_possible()) else {
            message(ui, t!("echo-unavailable"));
            return;
        };
        if heart.echo_anatomy().is_none() {
            if !self.warming {
                // one frame of "preparing" is drawn before the anatomy is built (about 0.7 s)
                self.warming = true;
                message(ui, t!("echo-preparing"));
                ui.ctx().request_repaint();
                return;
            }
            heart.build_echo();
            if heart.echo_anatomy().is_none() {
                message(ui, t!("echo-unavailable"));
                return;
            }
        }
        self.active = true;
        if self.scene.torso.is_none() {
            self.scene.torso = heart.torso_layer();
            self.scene.torso_clip = true;
        }
        let ctx = ui.ctx().clone();
        let chest = heart.echo_anatomy().map_or((Vec3::ZERO, Vec3::ONE), |a| {
            let vol = a.heart_volume();
            let c = (vol.origin_mm + vol.max_mm()) * 0.5;
            (c - FRAME_HALF_MM, c + FRAME_HALF_MM)
        });
        if self.pose.is_none()
            && let Some(a) = heart.echo_anatomy()
        {
            self.pose = Some(a.window_pose(EchoWindow::Apical));
            self.window = Some(EchoWindow::Apical);
        }
        if let Some(w) = self.pending_window.take()
            && let Some(a) = heart.echo_anatomy()
        {
            self.go_to_window(a, w);
        }
        // the motion policy and the scene first, so the slice below reads this frame's wall
        if let Some(rs) = rs {
            let aspect = self.view_aspect;
            heart.with_scene(ui, |scene, rs| self.prepare_scene(scene, rs, aspect, chest));
            self.ensure_section_renderer(rs);
        } else if let Some(wait_s) = heart.tick_motion(ui.input(|i| i.time), true, false) {
            ctx.request_repaint_after_secs(wait_s as f32);
        }
        if let Some(a) = heart.echo_anatomy() {
            self.keys(&ctx, a);
        }
        // depth and sector are the probe's: the settings follow the pose
        if let Some(p) = self.pose {
            self.settings.depth_mm = p.depth_mm;
            self.settings.sector_deg = p.sector_deg;
        }
        self.refresh_slice(ui, heart);
        self.refresh_grey(ui, heart);
        self.update_views(ui, heart);
        self.update_measure(ui, heart);
        self.measure.compact = crate::responsive::is_narrow(&ctx);
        if crate::responsive::is_narrow(&ctx) {
            self.narrow_page(ui, heart, rs, colour_blind);
            return;
        }
        let total = ui.available_width();
        egui::Panel::left("echo_controls")
            .resizable(true)
            .default_size(300.0)
            .max_size(crate::responsive::max_side(ui, 360.0))
            .show(ui, |ui| self.controls(ui, heart, true));
        egui::Panel::right("echo_section")
            .resizable(true)
            .default_size(((total - 280.0) * 0.5).max(300.0))
            .max_size((total - 280.0 - VIEW_MIN_PX).max(300.0))
            .show(ui, |ui| self.section_pane(ui, rs, colour_blind, None));
        egui::CentralPanel::default().show(ui, |ui| self.view_3d(ui, heart, rs, None));
    }

    /// The phone layout: the image (the sector, with the trace under it while M-mode or Doppler is
    /// on) at the top at a useful height, always in view; the 3-D chest view is the other side of a
    /// two-way switch rather than stacked below; the section control and its section scroll under
    /// them.
    fn narrow_page(
        &mut self,
        ui: &mut egui::Ui,
        heart: &mut HeartView,
        rs: Option<&egui_wgpu::RenderState>,
        colour_blind: bool,
    ) {
        egui::CentralPanel::default().show(ui, |ui| {
            let width = ui.available_width();
            let avail = ui.available_height();
            // the sector gets what is left of its share after the display switch, the one-line
            // note, the read-out and the trace, never less than 220 px
            // under a trace the display switch, the note and the read-out give way to the trace;
            // the controls keep at least CONTROLS_MIN_PX of the page
            let trace = self.measure_mode().is_trace();
            let rest = if trace {
                self.trace_reserve_px()
            } else {
                DISPLAY_PX + 44.0
            };
            let floor = if trace { 160.0 } else { 220.0 };
            let sector_h = (avail - SWITCH_PX - rest - CONTROLS_MIN_PX).clamp(floor, 340.0);
            let top_h = sector_h + rest;
            let options = [(false, t!("echo-narrow-image")), (true, t!("echo-narrow-3d"))];
            heartsim_ui::segmented(ui, &mut self.narrow_3d, &options);
            ui.allocate_ui(egui::vec2(width, top_h), |ui| {
                if self.narrow_3d {
                    self.view_3d(ui, heart, rs, Some(top_h));
                } else {
                    self.section_pane(ui, rs, colour_blind, Some(sector_h));
                }
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("echo_narrow_controls")
                .auto_shrink([false, false])
                .show(ui, |ui| self.controls(ui, heart, false));
        });
    }

    fn ensure_section_renderer(&mut self, rs: &egui_wgpu::RenderState) {
        let mut renderer = rs.renderer.write();
        if renderer.callback_resources.get::<SectionRenderer>().is_none() {
            renderer
                .callback_resources
                .insert(SectionRenderer::new(&rs.device, rs.target_format));
            self.uploaded = u64::MAX;
            self.grey_uploaded = u64::MAX;
        }
    }

    // ---- controls ---------------------------------------------------------------------------

    /// The left column: the section control, then the chosen section, in a scroll area with a
    /// solid (always drawn) scroll bar when `scroll` (the phone layout scrolls the whole page).
    fn controls(&mut self, ui: &mut egui::Ui, heart: &mut HeartView, scroll: bool) {
        if self.pose.is_none() {
            return;
        }
        let options = EchoSection::ALL.map(|s| (s, s.name()));
        // four sections: a little less padding in the buttons keeps every name whole
        ui.scope(|ui| {
            ui.spacing_mut().button_padding.x = 3.0;
            heartsim_ui::segmented(ui, &mut self.section, &options);
        });
        ui.add_space(4.0);
        if scroll {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            egui::ScrollArea::vertical()
                .id_salt("echo_controls_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| self.section_ui(ui, heart));
        } else {
            self.section_ui(ui, heart);
        }
    }

    fn section_ui(&mut self, ui: &mut egui::Ui, heart: &mut HeartView) {
        match self.section {
            EchoSection::Probe => self.probe_controls(ui, heart),
            EchoSection::Image => self.image_controls(ui),
            EchoSection::Views => self.views_card(ui, heart),
            EchoSection::Measure => self.measure_controls(ui, heart),
        }
    }

    /// The Probe section: the windows, Reset, the angle, depth and sector sliders, where the marker
    /// points and whether the probe has contact; then the view switches.
    fn probe_controls(&mut self, ui: &mut egui::Ui, heart: &mut HeartView) {
        let Some(pose) = self.pose else { return };
        cards::card(ui, |ui| {
            heartsim_ui::info_disclosure(
                ui,
                "echo_probe_info",
                t!("echo-intro-line"),
                format!(
                    "{}\n\n{}\n\n{}",
                    t!("echo-intro"),
                    t!("echo-drag-hint"),
                    t!("echo-keys")
                ),
            );
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for w in EchoWindow::ALL {
                    let button = egui::Button::selectable(self.window == Some(w), window_name(w));
                    if ui.add(button).clicked()
                        && let Some(a) = heart.echo_anatomy()
                    {
                        self.go_to_window(a, w);
                    }
                }
            });
            // Reset is not a fifth window: its own row, at the right, a plain (secondary) button
            right_aligned_row(ui, |ui| {
                if ui
                    .button(t!("echo-reset"))
                    .on_hover_text(t!("echo-reset-hint"))
                    .clicked()
                    && let Some(a) = heart.echo_anatomy()
                {
                    self.reset(a);
                }
            });
            ui.add_space(2.0);
            let mut p = pose;
            let mut moved = false;
            moved |= heartsim_ui::slider_row(
                ui,
                &t!("echo-tilt"),
                deg_slider(&mut p.tilt_deg, -ANGLE_LIMIT_DEG, ANGLE_LIMIT_DEG),
            )
            .on_hover_text(t!("echo-tilt-hint"))
            .changed();
            moved |= heartsim_ui::slider_row(
                ui,
                &t!("echo-rock"),
                deg_slider(&mut p.rock_deg, -ANGLE_LIMIT_DEG, ANGLE_LIMIT_DEG),
            )
            .on_hover_text(t!("echo-rock-hint"))
            .changed();
            moved |= heartsim_ui::slider_row(
                ui,
                &t!("echo-rotation"),
                deg_slider(&mut p.rotation_deg, -180.0, 179.0),
            )
            .on_hover_text(t!("echo-rotation-hint"))
            .changed();
            let imaging = heartsim_ui::slider_row(
                ui,
                &t!("echo-depth"),
                egui::Slider::new(&mut p.depth_mm, DEPTH_RANGE_MM.0..=DEPTH_RANGE_MM.1)
                    .suffix(format!(" {}", t!("echo-unit-mm"))),
            )
            .changed()
                | heartsim_ui::slider_row(
                    ui,
                    &t!("echo-sector"),
                    egui::Slider::new(&mut p.sector_deg, SECTOR_RANGE_DEG.0..=SECTOR_RANGE_DEG.1).suffix("°"),
                )
                .changed();
            if moved || imaging {
                self.pose = Some(p.clamped());
                if moved {
                    self.window = None;
                }
            }
            let contact_limit = heart
                .echo_anatomy()
                .map_or(60.0, |a| a.designed.contact_limit_deg);
            let frame = pose.frame(contact_limit);
            ui.add_space(2.0);
            ui.label(t!("echo-clock", hour = clock_text(pose.clock_hours())));
            if frame.contact {
                heartsim_ui::helper_text(
                    ui,
                    t!(
                        "echo-contact",
                        angle = format!("{:.0}", frame.beam_from_normal_deg),
                        limit = format!("{contact_limit:.0}")
                    ),
                );
            } else {
                ui.colored_label(ui.visuals().warn_fg_color, t!("echo-no-contact"));
            }
        });
        cards::card(ui, |ui| {
            cards::heading(ui, t!("echo-heading-view"));
            ui.checkbox(&mut self.clip, t!("echo-clip"))
                .on_hover_text(t!("echo-clip-hint"));
            if let Some(m) = heart.motion_mut() {
                let reduced = m.reduced_motion;
                ui.add_enabled(!reduced, egui::Checkbox::new(&mut m.on, t!("echo-wall-motion")));
                if reduced {
                    heartsim_ui::helper_text(ui, t!("echo-reduced-motion"));
                } else if !m.on {
                    heartsim_ui::helper_text(ui, t!("echo-motion-off"));
                }
            }
        });
    }

    /// The controls of the simulated image: what an echo machine's knobs do in the line model.
    /// Depth and sector width are in the Probe section (one source of truth: the pose).
    fn image_controls(&mut self, ui: &mut egui::Ui) {
        let depth = self.pose.map_or(self.settings.depth_mm, |p| p.depth_mm);
        cards::card(ui, |ui| {
            let mut s = self.settings.clone();
            let mut changed = false;
            let (f_lo, f_hi) = ImageSettings::FREQUENCY_RANGE_MHZ;
            changed |= heartsim_ui::slider_row(
                ui,
                &t!("echo-frequency"),
                egui::Slider::new(&mut s.frequency_mhz, f_lo..=f_hi)
                    .suffix(format!(" {}", t!("echo-unit-mhz")))
                    .custom_formatter(|v, _| format!("{v:.1}")),
            )
            .on_hover_text(t!("echo-frequency-hint"))
            .changed();
            let (g_lo, g_hi) = ImageSettings::GAIN_RANGE_DB;
            changed |= heartsim_ui::slider_row(
                ui,
                &t!("echo-gain"),
                egui::Slider::new(&mut s.gain_db, g_lo..=g_hi)
                    .suffix(format!(" {}", t!("echo-unit-db")))
                    .custom_formatter(|v, _| format!("{v:.0}")),
            )
            .on_hover_text(t!("echo-gain-hint"))
            .changed();
            let (r_lo, r_hi) = ImageSettings::DYNAMIC_RANGE_DB;
            changed |= heartsim_ui::slider_row(
                ui,
                &t!("echo-dynamic-range"),
                egui::Slider::new(&mut s.dynamic_range_db, r_lo..=r_hi)
                    .suffix(format!(" {}", t!("echo-unit-db")))
                    .custom_formatter(|v, _| format!("{v:.0}")),
            )
            .on_hover_text(t!("echo-dynamic-range-hint"))
            .changed();
            let (z_lo, z_hi) = ImageSettings::FOCUS_RANGE_MM;
            s.focus_mm = s.focus_mm.min(depth).clamp(z_lo, z_hi);
            changed |= heartsim_ui::slider_row(
                ui,
                &t!("echo-focus"),
                egui::Slider::new(&mut s.focus_mm, z_lo..=z_hi.min(depth).max(z_lo))
                    .suffix(format!(" {}", t!("echo-unit-mm")))
                    .custom_formatter(|v, _| format!("{v:.0}")),
            )
            .on_hover_text(t!("echo-focus-hint"))
            .changed();
            egui::CollapsingHeader::new(t!("echo-tgc"))
                .default_open(false)
                .show(ui, |ui| {
                    heartsim_ui::helper_text(ui, t!("echo-tgc-note"));
                    changed |= ui
                        .checkbox(&mut s.tgc_auto, t!("echo-tgc-auto"))
                        .on_hover_text(t!("echo-tgc-auto-hint"))
                        .changed();
                    let (t_lo, t_hi) = ImageSettings::TGC_RANGE_DB;
                    for i in 0..TGC_BANDS {
                        changed |= heartsim_ui::slider_row(
                            ui,
                            &t!("echo-tgc-band", n = format!("{}", i + 1)),
                            egui::Slider::new(&mut s.tgc_db[i], t_lo..=t_hi)
                                .suffix(format!(" {}", t!("echo-unit-db")))
                                .custom_formatter(|v, _| format!("{v:.0}")),
                        )
                        .on_hover_text(t!("echo-tgc-band-hint", n = format!("{}", i + 1)))
                        .changed();
                    }
                });
            if changed {
                self.settings = s;
            }
            right_aligned_row(ui, |ui| {
                if ui
                    .button(t!("echo-reset-image"))
                    .on_hover_text(t!("echo-reset-image-hint"))
                    .clicked()
                {
                    self.reset_image();
                }
            });
        });
    }

    // ---- the 3-D view -----------------------------------------------------------------------

    /// The handles of the probe on screen.
    fn handles(camera: &OrbitCamera, aspect: f32, rect: Rect, frame: &ProbeFrame) -> Option<Handles> {
        let apex = to_screen(camera, aspect, rect, frame.apex_mm)?;
        let ahead = |dir: Vec3| to_screen(camera, aspect, rect, frame.apex_mm + dir * 60.0);
        let tilt = handle_vector(apex, ahead(frame.elevation), Vec2::new(0.0, -1.0));
        let rock = handle_vector(apex, ahead(frame.marker), Vec2::new(1.0, 0.0));
        // the rotation dot on the diagonal away from both other handles, so no label covers another
        let ring_dir =
            (-(rock.normalize_or(Vec2::X) + tilt.normalize_or(Vec2::Y))).normalize_or(Vec2::new(-1.0, 1.0));
        Some(Handles {
            apex,
            tilt: apex + EVec2::new(tilt.x, tilt.y),
            tilt_dir: tilt.normalize_or(Vec2::Y),
            rock: apex + EVec2::new(rock.x, rock.y),
            rock_dir: rock.normalize_or(Vec2::X),
            ring: apex + EVec2::new(ring_dir.x, ring_dir.y) * RING_PX,
        })
    }

    fn view_3d(
        &mut self,
        ui: &mut egui::Ui,
        heart: &mut HeartView,
        rs: Option<&egui_wgpu::RenderState>,
        height: Option<f32>,
    ) {
        let Some(rs) = rs else {
            ui.label(t!("echo-no-3d"));
            return;
        };
        let (Some(pose), Some(anatomy)) = (self.pose, heart.echo_anatomy()) else {
            return;
        };
        let pose = pose.clamped();
        let frame = pose.frame(anatomy.designed.contact_limit_deg);
        let mut size = ui.available_size();
        if let Some(h) = height {
            size.y = h;
        }
        size.y = size.y.max(120.0);
        let rect = Rect::from_min_size(ui.cursor().min, size);
        let aspect = rect.width() / rect.height().max(1.0);
        self.view_aspect = aspect;
        let handles = self
            .scene
            .camera_now
            .and_then(|c| Self::handles(&c, aspect, rect, &frame));
        let (pressed, down, origin, pos, delta) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.press_origin(),
                i.pointer.latest_pos(),
                i.pointer.delta(),
            )
        });
        if pressed {
            self.press_grabbed = false;
            if let (Some(h), Some(o)) = (&handles, origin.filter(|o| rect.contains(*o))) {
                let candidates = [
                    (Drag::Rotate(0.0), h.ring, GRAB_PX),
                    (Drag::Tilt, h.tilt, GRAB_PX),
                    (Drag::Rock, h.rock, GRAB_PX),
                    (Drag::Slide, h.apex, GRAB_PX * 1.6),
                ];
                self.drag = candidates
                    .into_iter()
                    .filter(|(_, at, r)| at.distance(o) <= *r)
                    .min_by(|a, b| a.1.distance(o).total_cmp(&b.1.distance(o)))
                    .map(|(d, _, _)| match d {
                        Drag::Rotate(_) => Drag::Rotate((o.y - h.apex.y).atan2(o.x - h.apex.x)),
                        other => other,
                    });
                self.press_grabbed = self.drag.is_some();
                self.probe_touched |= self.press_grabbed;
            }
        }
        if !down {
            self.drag = None;
        }
        if let (Some(drag), Some(h), Some(p), Some(camera)) =
            (self.drag, &handles, pos, self.scene.camera_now)
        {
            let d = Vec2::new(delta.x, delta.y);
            match drag {
                // the handle sits on the elevation axis; positive tilt fans the beam the other way
                Drag::Tilt => self.pose = Some(with_tilt(&pose, -d.dot(h.tilt_dir) * DEG_PER_PX)),
                Drag::Rock => self.pose = Some(with_rock(&pose, d.dot(h.rock_dir) * DEG_PER_PX)),
                Drag::Rotate(last) => {
                    let now = (p.y - h.apex.y).atan2(p.x - h.apex.x);
                    let step = (now - last + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI;
                    // seen from outside, clockwise on screen is the positive rotation
                    let outside = (camera.eye() - camera.target).dot(pose.normal) > 0.0;
                    let sign = if outside { 1.0 } else { -1.0 };
                    self.pose = Some(with_rotation(&pose, sign * step.to_degrees()));
                    self.drag = Some(Drag::Rotate(now));
                }
                Drag::Slide => {
                    if rect.contains(p) {
                        let uv = (
                            (p.x - rect.left()) / rect.width(),
                            (p.y - rect.top()) / rect.height(),
                        );
                        let (o, dir) = pick_ray(&camera, uv, aspect);
                        if let Some((hit, _)) = anatomy.skin().ray_first(o, dir) {
                            self.slide_to(anatomy, hit);
                        }
                    }
                }
            }
            if !matches!(drag, Drag::Slide) {
                self.window = None;
            }
        }
        let response = Viewport3d::ui_with_orbit(ui, size, self.drag.is_none());
        if response.clicked()
            && !self.press_grabbed
            && let (Some(p), Some(camera)) = (response.interact_pointer_pos(), self.scene.camera_now)
        {
            let uv = (
                (p.x - rect.left()) / rect.width(),
                (p.y - rect.top()) / rect.height(),
            );
            let (o, dir) = pick_ray(&camera, uv, aspect);
            if let Some((hit, _)) = anatomy.skin().ray_first(o, dir) {
                self.slide_to(anatomy, hit);
                self.probe_touched = true;
            }
        }
        let pose = self.pose.map_or(pose, |p| p.clamped());
        let frame = pose.frame(anatomy.designed.contact_limit_deg);
        let ghost = self.ghost_frame(heart);
        {
            let mut renderer = rs.renderer.write();
            if let Some(res) = renderer.callback_resources.get_mut::<ViewportResources>() {
                self.update_overlay(&mut res.scene, rs, &frame, &pose, rect, ghost);
            }
        }
        // the handles, over the 3-D view
        let painter = ui.painter_at(rect);
        let handles = self
            .scene
            .camera_now
            .and_then(|c| Self::handles(&c, aspect, rect, &frame));
        if let Some(h) = handles {
            let teal = Color32::from_rgb(30, 190, 200);
            let orange = Color32::from_rgb(255, 140, 26);
            let text = ui.visuals().strong_text_color();
            let plate = ui.visuals().extreme_bg_color.gamma_multiply(0.8);
            painter.circle_stroke(h.apex, RING_PX, Stroke::new(1.5, orange.gamma_multiply(0.7)));
            for (at, colour, label) in [
                (h.ring, orange, t!("echo-handle-rotate")),
                (h.tilt, teal, t!("echo-handle-tilt")),
                (h.rock, teal, t!("echo-handle-rock")),
            ] {
                if at != h.ring {
                    painter.line_segment([h.apex, at], Stroke::new(1.0, colour.gamma_multiply(0.7)));
                }
                painter.circle_filled(at, 6.0, colour);
                painter.circle_stroke(at, 6.0, Stroke::new(1.0, Color32::WHITE));
                let galley = painter.layout_no_wrap(label, egui::FontId::proportional(12.0), text);
                let place = at + EVec2::new(8.0, -galley.size().y - 2.0);
                painter.rect_filled(Rect::from_min_size(place, galley.size()).expand(2.0), 3.0, plate);
                painter.galley(place, galley, text);
            }
        }
        // the drag hint, until the probe is first moved (then it is in the Probe section's "More")
        if !self.probe_touched {
            let galley = painter.layout(
                t!("echo-drag-hint"),
                egui::FontId::proportional(13.0),
                ui.visuals().text_color(),
                (rect.width() - 16.0).max(40.0),
            );
            let plate = Rect::from_min_size(
                Pos2::new(rect.left() + 4.0, rect.bottom() - galley.size().y - 8.0),
                galley.size() + EVec2::new(8.0, 4.0),
            );
            painter.rect_filled(plate, 3.0, ui.visuals().extreme_bg_color.gamma_multiply(0.8));
            painter.galley(
                plate.min + EVec2::new(4.0, 2.0),
                galley,
                ui.visuals().text_color(),
            );
        }
        if let Some(camera) = self.scene.camera_now {
            let basis = camera.basis();
            heartsim_ui::orientation_gizmo(ui, rect, basis, heartsim_ui::GizmoCorner::TopRight);
        }
        response.on_hover_cursor(if self.drag.is_some() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::Default
        });
    }

    // ---- the cross-section ------------------------------------------------------------------

    fn section_pane(
        &mut self,
        ui: &mut egui::Ui,
        rs: Option<&egui_wgpu::RenderState>,
        colour_blind: bool,
        height: Option<f32>,
    ) {
        let palette = echo_colours::palette(colour_blind);
        // the phone layout (a given height) has no room for the heading
        if height.is_none() {
            cards::heading(
                ui,
                if self.mode == DisplayMode::Anatomy {
                    t!("echo-heading-section")
                } else {
                    t!("echo-heading-simulated")
                },
            );
        }
        // phone layout under a trace: only the image and the trace
        let lean = height.is_some() && self.measure_mode().is_trace();
        if !lean {
            let options = DisplayMode::ALL.map(|m| (m, m.name()));
            heartsim_ui::segmented(ui, &mut self.mode, &options);
            // one line of the same height in every mode (what the image is, and is not); the full note
            // and the educational disclaimer behind "More"
            let (line, note) = match self.mode {
                DisplayMode::Anatomy => (t!("echo-section-line"), t!("echo-section-note")),
                DisplayMode::Image => (t!("echo-image-line"), t!("echo-image-note")),
                DisplayMode::Both => (
                    t!("echo-image-line"),
                    format!("{} {}", t!("echo-image-note"), t!("echo-both-note")),
                ),
            };
            heartsim_ui::info_disclosure(
                ui,
                "echo_section_info",
                line,
                format!("{note}\n\n{}", t!("echo-disclaimer")),
            );
        }
        ui.add_space(2.0);
        // the image's rectangle does not depend on the mode: the read-out and the legend are below
        // it, in a height kept for them in every mode
        let legend_px = if lean {
            0.0
        } else if height.is_some() {
            44.0
        } else {
            self.legend_px()
        };
        let mut size = ui.available_size();
        match height {
            Some(h) => size.y = h,
            None => size.y -= legend_px + self.trace_reserve_px(),
        }
        size = size.max(EVec2::splat(120.0));
        // the Measure section puts the cursor, the gate and the calipers on the sector
        let sense = if self.section == EchoSection::Measure || self.measure.mode.is_trace() {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(size, sense);
        let pose = self.pose.unwrap_or_default().clamped();
        // the image's own depth and sector (the pose it was sliced with, which can lag the
        // sliders by a frame) set the layout; before the first slice, the pose's
        let (depth_mm, sector_deg) = if self.image.depth_mm > 0.0 {
            (self.image.depth_mm, self.image.sector_deg)
        } else {
            (pose.depth_mm, pose.sector_deg)
        };
        let layout = SectionLayout::fit(
            Vec2::new(rect.width(), rect.height()),
            depth_mm,
            sector_deg,
            [34.0, 10.0, 10.0, 12.0],
        );
        self.layout = Some(layout);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
        if self.generation > 0 {
            if rs.is_some() {
                // a texture is uploaded only while the mode shows it, and then once per change
                let upload = (self.mode.shows_colours() && self.uploaded != self.generation)
                    .then(|| Arc::clone(&self.texels));
                if upload.is_some() {
                    self.uploaded = self.generation;
                }
                let upload_grey = (self.mode.shows_image()
                    && self.grey_generation > 0
                    && self.grey_uploaded != self.grey_generation)
                    .then(|| Arc::clone(&self.grey_texels));
                if upload_grey.is_some() {
                    self.grey_uploaded = self.grey_generation;
                }
                painter.add(egui_wgpu::Callback::new_paint_callback(
                    rect,
                    SectionCallback {
                        upload,
                        upload_grey,
                        lines: self.image.lines as u32,
                        samples: self.image.samples as u32,
                        mode: self.mode,
                        layout,
                        palette: palette.to_vec(),
                    },
                ));
            } else {
                let grey = (self.mode.shows_image() && self.grey_generation > 0).then_some(&self.grey);
                paint_fallback(&painter, rect, &layout, &self.image, grey, self.mode, &palette);
            }
        }
        self.paint_section_overlay(ui, &painter, rect, &layout);
        self.paint_measure_overlay(&painter, rect, &layout);
        self.section_rect = Some(rect);
        if self.section == EchoSection::Measure {
            self.measure_sector_input(&response, rect, &layout);
        }
        self.hover = response
            .hover_pos()
            .and_then(|p| self.read_out_at(Vec2::new(p.x - rect.left(), p.y - rect.top())));
        // the read-out and the legend keep a height of their own (smaller under a trace), the
        // same in every display mode
        let legend_size = EVec2::new(ui.available_width(), legend_px);
        if !lean {
            ui.allocate_ui(legend_size, |ui| self.readout_and_legend(ui, &palette));
        }
        self.trace_pane(ui);
    }

    /// Under the cross-section: the pointer read-out (one line) and the legend of the tissue
    /// colours, scrolled within [`LEGEND_PX`] when it is longer; in the image mode, which has no
    /// colours, a line saying where the colours are.
    fn readout_and_legend(&mut self, ui: &mut egui::Ui, palette: &[[u8; 3]; PALETTE_SLOTS]) {
        let line = match self.hover {
            Some(r) => {
                let tissue = echo_colours::name(r.label);
                let depth = format!("{:.1}", r.depth_mm / 10.0);
                if r.hidden {
                    t!("echo-readout-hidden", tissue = tissue, depth = depth)
                } else {
                    t!("echo-readout", tissue = tissue, depth = depth)
                }
            }
            None if self.mode == DisplayMode::Anatomy => t!("echo-readout-none"),
            None => t!("echo-readout-none-image"),
        };
        ui.add(egui::Label::new(line).truncate());
        if !self.mode.shows_colours() {
            heartsim_ui::helper_text(ui, t!("echo-legend-hint"));
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("echo_legend")
            .max_height(ui.available_height().max(40.0))
            .show(ui, |ui| self.legend(ui, palette));
    }

    /// The legend in rows that never split a chip (swatch and name stay together).
    fn legend(&self, ui: &mut egui::Ui, palette: &[[u8; 3]; PALETTE_SLOTS]) {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let gap = ui.spacing().item_spacing.x;
        let room = ui.available_width();
        let mut rows: Vec<Vec<(Label, String)>> = vec![Vec::new()];
        let mut used = 0.0;
        for label in self.image.labels_present() {
            let name = echo_colours::name(label);
            let text_w = ui
                .painter()
                .layout_no_wrap(name.clone(), font.clone(), Color32::WHITE)
                .size()
                .x;
            let w = 11.0 + gap + text_w + gap * 2.0;
            if used + w > room && rows.last().is_some_and(|r| !r.is_empty()) {
                rows.push(Vec::new());
                used = 0.0;
            }
            used += w;
            if let Some(row) = rows.last_mut() {
                row.push((label, name));
            }
        }
        for row in rows {
            ui.horizontal(|ui| {
                for (label, name) in row {
                    let [r, g, b] = palette[usize::from(label.to_u8())];
                    let (swatch, _) = ui.allocate_exact_size(EVec2::splat(11.0), Sense::hover());
                    ui.painter().rect_filled(swatch, 2.0, Color32::from_rgb(r, g, b));
                    ui.painter().rect_stroke(
                        swatch,
                        2.0,
                        Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color),
                        egui::StrokeKind::Inside,
                    );
                    ui.label(name);
                    ui.add_space(gap);
                }
            });
        }
    }

    fn paint_section_overlay(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        layout: &SectionLayout,
    ) {
        let ink = ui.visuals().text_color();
        let stroke = Stroke::new(1.2, ink.gamma_multiply(0.8));
        let at = |v: Vec2| rect.min + EVec2::new(v.x, v.y);
        let lines = self.image.lines;
        let depth = layout.depth_mm;
        // the outline of the sector
        let arc: Vec<Pos2> = (0..=48)
            .map(|i| at(layout.point_of(lines as f32 * i as f32 / 48.0, lines, depth)))
            .collect();
        painter.add(egui::Shape::line(arc, stroke));
        let apex = at(layout.apex_px);
        painter.line_segment([apex, at(layout.point_of(0.0, lines, depth))], stroke);
        painter.line_segment(
            [apex, at(layout.point_of(lines as f32, lines, depth))],
            Stroke::new(2.0, Color32::from_rgb(255, 140, 26)),
        );
        // the marker side
        let tag = at(layout.point_of(lines as f32 * 0.985, lines, depth * 0.1));
        painter.circle_filled(tag, 4.5, Color32::from_rgb(255, 140, 26));
        // the depth scale along the left edge, in cm
        let axis_x = rect.left() + 26.0;
        let cm_px = layout.px_per_mm * 10.0;
        let every = if cm_px >= 16.0 {
            1
        } else if cm_px >= 8.0 {
            2
        } else {
            5
        };
        let top = apex.y;
        painter.line_segment(
            [
                Pos2::new(axis_x, top),
                Pos2::new(axis_x, top + depth * layout.px_per_mm),
            ],
            stroke,
        );
        let mut cm = 0;
        while (cm as f32) * 10.0 <= depth + 0.01 {
            let y = top + cm as f32 * cm_px;
            painter.line_segment([Pos2::new(axis_x - 4.0, y), Pos2::new(axis_x, y)], stroke);
            if cm % every == 0 {
                painter.text(
                    Pos2::new(axis_x - 6.0, y),
                    egui::Align2::RIGHT_CENTER,
                    format!("{cm}"),
                    egui::FontId::proportional(11.0),
                    ink,
                );
            }
            cm += 1;
        }
        // the transmit focus, on the depth scale: a triangle pointing at the scale and an F
        if self.mode.shows_image() && self.image.contact {
            let focus = self.settings.clamped().focus_mm.min(depth);
            let y = top + focus * layout.px_per_mm;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(axis_x + 1.0, y),
                    Pos2::new(axis_x + 9.0, y - 5.0),
                    Pos2::new(axis_x + 9.0, y + 5.0),
                ],
                FOCUS_TEAL,
                Stroke::new(1.0, ink.gamma_multiply(0.6)),
            ));
            painter.text(
                Pos2::new(axis_x + 11.0, y),
                egui::Align2::LEFT_CENTER,
                t!("echo-focus-marker"),
                egui::FontId::proportional(11.0),
                FOCUS_TEAL,
            );
        }
        self.paint_marks(ui, painter, rect, layout);
        painter.text(
            Pos2::new(rect.left() + 4.0, rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            t!("echo-unit-cm"),
            egui::FontId::proportional(11.0),
            ink,
        );
        if !self.image.contact && self.generation > 0 {
            let galley = painter.layout(
                t!("echo-no-contact"),
                egui::FontId::proportional(16.0),
                ui.visuals().warn_fg_color,
                (rect.width() - 40.0).max(60.0),
            );
            let pos = rect.center() - galley.size() * 0.5;
            painter.rect_filled(
                Rect::from_min_size(pos, galley.size()).expand(6.0),
                6.0,
                ui.visuals().extreme_bg_color.gamma_multiply(0.85),
            );
            painter.galley(pos, galley, ui.visuals().warn_fg_color);
        }
    }
}

/// Where the handles of the probe are on screen, and the screen directions their drags follow.
#[derive(Debug, Clone, Copy)]
struct Handles {
    apex: Pos2,
    tilt: Pos2,
    tilt_dir: Vec2,
    rock: Pos2,
    rock_dir: Vec2,
    ring: Pos2,
}

/// The cross-section drawn by egui itself, coarsely (no GPU scene: the tests, a failed device):
/// blocks of 4 lines × 8 samples in the label of their centre.
fn paint_fallback(
    painter: &egui::Painter,
    rect: Rect,
    layout: &SectionLayout,
    image: &LabelImage,
    grey: Option<&GreyImage>,
    mode: DisplayMode,
    palette: &[[u8; 3]; PALETTE_SLOTS],
) {
    const LINE_STEP: usize = 4;
    const SAMPLE_STEP: usize = 8;
    let at = |v: Vec2| rect.min + EVec2::new(v.x, v.y);
    let depth_of = |s: usize| s as f32 / image.samples as f32 * layout.depth_mm;
    for l0 in (0..image.lines).step_by(LINE_STEP) {
        let l1 = (l0 + LINE_STEP).min(image.lines);
        for s0 in (0..image.samples).step_by(SAMPLE_STEP) {
            let s1 = (s0 + SAMPLE_STEP).min(image.samples);
            let (lc, sc) = (usize::midpoint(l0, l1), usize::midpoint(s0, s1));
            let lc = lc.min(image.lines - 1);
            let sc = sc.min(image.samples - 1);
            let label = image.label(lc, sc);
            let [r, g, b] = palette[usize::from(label.to_u8())];
            let k = if image.is_hidden(lc, sc) {
                HIDDEN_BRIGHTNESS
            } else {
                1.0
            };
            let tissue = [f32::from(r) * k, f32::from(g) * k, f32::from(b) * k];
            // the grey image's mean over the block, 0..255
            let level = grey.map(|gi| {
                let (mut sum, mut n) = (0.0_f32, 0.0_f32);
                for l in l0..l1 {
                    for s in s0..s1 {
                        if l < gi.lines && s < gi.samples {
                            sum += f32::from(gi.pixel(l, s));
                            n += 1.0;
                        }
                    }
                }
                sum / n.max(1.0)
            });
            let [r, g, b] = match (mode, level) {
                (DisplayMode::Both, Some(v)) if label != Label::Outside => {
                    let a = OVERLAY_OPACITY;
                    tissue.map(|c| c * a + v * (1.0 - a))
                }
                // the image alone, or no tissue under the colours: the grey
                (DisplayMode::Image | DisplayMode::Both, Some(v)) => [v, v, v],
                _ => tissue,
            };
            let colour = Color32::from_rgb(r as u8, g as u8, b as u8);
            let corners = [
                at(layout.point_of(l0 as f32, image.lines, depth_of(s0))),
                at(layout.point_of(l1 as f32, image.lines, depth_of(s0))),
                at(layout.point_of(l1 as f32, image.lines, depth_of(s1))),
                at(layout.point_of(l0 as f32, image.lines, depth_of(s1))),
            ];
            painter.add(egui::Shape::convex_polygon(
                corners.to_vec(),
                colour,
                Stroke::NONE,
            ));
        }
    }
}

/// A row of one button's height with its contents at the right (a secondary action on its own row).
fn right_aligned_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let size = EVec2::new(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), add);
}

/// A slider of an angle in degrees with one decimal.
fn deg_slider(v: &mut f32, lo: f32, hi: f32) -> egui::Slider<'_> {
    egui::Slider::new(v, lo..=hi)
        .suffix("°")
        .custom_formatter(|v, _| format!("{v:.1}"))
}

/// A page-wide note in the middle panel (the anatomy is missing, or being built).
fn message(ui: &mut egui::Ui, text: String) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.add_space(8.0);
        ui.label(text);
    });
}

/// The name of a window on its button.
#[must_use]
pub fn window_name(w: EchoWindow) -> String {
    match w {
        EchoWindow::Parasternal => t!("echo-window-parasternal"),
        EchoWindow::Apical => t!("echo-window-apical"),
        EchoWindow::Subcostal => t!("echo-window-subcostal"),
        EchoWindow::Suprasternal => t!("echo-window-suprasternal"),
    }
}

/// A clock position as text: the nearest half hour, 12 for zero ("3", "4:30").
#[must_use]
pub fn clock_text(hours: f32) -> String {
    let halves = ((hours * 2.0).round() as i32).rem_euclid(24);
    let (h, half) = (halves / 2, halves % 2 == 1);
    let h = if h == 0 { 12 } else { h };
    if half { format!("{h}:30") } else { format!("{h}") }
}

/// Whether the colour-blind-safe colour maps are chosen (the palette follows them).
#[must_use]
pub fn colour_blind(style: LutStyle) -> bool {
    style == LutStyle::ColourBlindSafe
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_of(p: &ProbePose) -> ProbeFrame {
        p.frame(60.0)
    }

    /// The drag directions agree with the pose: positive tilt fans the beam away from the frame's
    /// `elevation` axis (toward `inward × marker`; the tilt handle sits on `elevation` and is
    /// dragged with a negated sign), positive rock swings it toward the marker.
    #[test]
    fn tilt_and_rock_signs_match_the_axes_the_handles_are_drawn_on() {
        let base = ProbePose {
            skin_mm: Vec3::new(0.0, 0.0, 100.0),
            normal: Vec3::Z,
            ..ProbePose::default()
        };
        let f0 = frame_of(&base);
        let tilted = frame_of(&with_tilt(&base, 10.0));
        assert!(
            tilted.beam.dot(f0.elevation) < -0.1,
            "positive tilt moves the beam away from +elevation"
        );
        let rocked = frame_of(&with_rock(&base, 10.0));
        assert!(
            rocked.beam.dot(f0.marker) > 0.1,
            "rock swings the beam toward the marker"
        );
        // the handles lie on the frame's own axes
        assert!((f0.marker.dot(f0.elevation)).abs() < 1e-5);
    }

    #[test]
    fn rotation_turns_the_marker_clockwise_seen_from_outside() {
        let base = ProbePose {
            skin_mm: Vec3::new(0.0, 0.0, 100.0),
            normal: Vec3::Z,
            ..ProbePose::default()
        };
        let m0 = frame_of(&base).marker;
        let m1 = frame_of(&with_rotation(&base, 30.0)).marker;
        // seen from the front (from +z, x to the right, y up), clockwise means the marker goes
        // from toward the head (+y) toward the patient's left (+x)
        assert!(m0.y > 0.9, "rotation 0 is toward the head: {m0:?}");
        assert!(m1.x > 0.3, "positive rotation goes toward +x: {m1:?}");
    }

    #[test]
    fn clock_text_rounds_to_half_hours() {
        assert_eq!(clock_text(0.0), "12");
        assert_eq!(clock_text(3.0), "3");
        assert_eq!(clock_text(4.4), "4:30");
        assert_eq!(clock_text(11.9), "12");
    }

    #[test]
    fn the_sector_mesh_has_the_apex_and_a_closed_fan() {
        let f = frame_of(&ProbePose::default());
        let m = sector_mesh(&f, 100.0, 60.0);
        assert_eq!(m.vertices.len(), ARC_SEGMENTS + 2);
        assert_eq!(m.indices.len(), ARC_SEGMENTS * 6);
        assert_eq!(sector_vertices(&f, 100.0, 60.0).len(), m.vertices.len());
        for v in &m.vertices[1..] {
            let d = Vec3::from(v.position) - f.apex_mm;
            assert!((d.length() - 100.0).abs() < 1e-3, "on the arc");
            assert!(d.dot(f.elevation).abs() < 1e-3, "in the image plane");
        }
    }
}
