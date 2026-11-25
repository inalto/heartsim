//! The 3-D heart in the Simulator tab (brief B9): anatomy pack layers coloured from the Tier-1
//! eikonal engine, the colour-map legend, the anatomy panel and click-to-stimulate / ablate.
//!
//! **One source of truth.** The eikonal engine never decides when a beat happens: the app feeds it
//! the timing engine's events every step ([`HeartView::on_step`]), exactly as
//! `heartsim_sim::Tier1Engine` does, so the strip (Tier-0 synthesis of the same events) and the
//! colour map show the same beats. A click stimulates both engines: the mesh at the clicked cell,
//! the timing engine in that cell's chamber, so the strip shows the ectopic beat the heart shows.
//!
//! Everything except [`HeartView::viewport`] runs without a GPU, which keeps the mapping testable.

use crate::heart_motion::{self, HeartMotion, Tick};
use crate::settings::SimulatorSections;
#[cfg(feature = "native")]
use anyhow::Context as _;
use eframe::egui;
use glam::Vec3;
use heartsim_core::{Action, Millis, Parameters, Region, StimSite};
use heartsim_echo::{DesignedParams, EchoAnatomy, ViewTargets};
use heartsim_geometry::{AnatomyPack, Hood, MeshKind, Territory};
use heartsim_render::{
    ClipPlane, Contour, GlyphInstance, GlyphSetId, LayerDesc, LayerId, LutId, LutStyle, ScalarMode,
    SceneRenderer, VertexSmoother, map_hoods, oblique_yaw_deg,
};
use heartsim_sim::timing::{PacerConfig, RespiratoryClock};
use heartsim_sim::{EikonalEngine, EikonalParams, Engine, Event, EventKind, TimingEngine, TimingParams};
use heartsim_ui::t;
use heartsim_ui::{
    AnatomyPanelState, CameraCommand, Viewport3d, ViewportResources, anatomy_panel, colour_bar,
};
#[cfg(feature = "native")]
use std::path::PathBuf;
use std::sync::Arc;

/// Resting and peak transmembrane potential used to show the engine's normalised V_m in mV
/// (ventricular myocyte: rest ≈ −85 mV, overshoot ≈ +25 mV; Katz, *Physiology of the Heart*).
const REST_MV: f32 = -85.0;
const PEAK_MV: f32 = 25.0;
/// Activation threshold drawn as the wavefront outline, mV.
const WAVEFRONT_MV: f32 = -30.0;
/// Passes of edge smoothing on each layer's vertex field before it is drawn. The hoods take the
/// eight cells nearest a vertex, so across a wavefront neighbouring vertices mix depolarised
/// and resting cells in jumping proportions: blotches in the colour ramp and a ragged, speckled
/// wavefront line. Four half-weight passes spread each value over about 2 mm of mesh, well
/// under the 2.3 mm cell spacing, and leave one clean front.
const DISPLAY_SMOOTHING_PASSES: usize = 4;
/// Auto-rotate speed, radians per second of wall-clock time (one turn in ~20 s).
const AUTO_ROTATE_RAD_S: f32 = 0.3;
/// Colour (linear RGB) a valve ring takes at full highlight (B27 part B): a bright cyan that no
/// tissue, vessel or colour map of the scene uses near the valve plane. Designed.
const VALVE_HIGHLIGHT: [f32; 3] = [0.1, 0.75, 1.0];

/// Layer name of the measured tricuspid valve (brief B35) in the anatomy panel.
pub const TRICUSPID_VALVE_LAYER: &str = "tricuspid_valve";

/// The measured tricuspid valve as drawn: its two-sided shell and chordae, the scene layer, and
/// what its last upload was made from.
struct ValveLayer {
    render: heartsim_geometry::ValveRenderMesh,
    id: Option<LayerId>,
    key: Option<(u64, bool, u32)>,
    scratch: Vec<Vec3>,
    posed: Vec<Vec3>,
    vertices: Vec<heartsim_geometry::Vertex>,
}

/// The four valve discs of a `Valve` mesh, each with its valve: the mesh's connected components
/// (four, or `None`), told apart by their centres in the heart frame — the pulmonic the most
/// anterior and cranial (largest `y + z`), then the tricuspid the furthest to the patient's right
/// (smallest `x`), then the mitral the most posterior (smallest `z`), the aortic the one left.
#[must_use]
pub fn split_valves(
    mesh: &heartsim_geometry::TriMesh,
) -> Option<Vec<(heartsim_ecg::Valve, heartsim_geometry::TriMesh)>> {
    use heartsim_ecg::Valve;
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let n = mesh.vertices.len();
    let mut parent: Vec<usize> = (0..n).collect();
    for tri in mesh.indices.as_chunks::<3>().0 {
        let a = find(&mut parent, tri[0] as usize);
        for &v in &tri[1..] {
            let b = find(&mut parent, v as usize);
            if a != b {
                parent[b] = a;
            }
        }
    }
    let mut roots: Vec<usize> = Vec::new();
    let mut component = vec![0usize; n];
    for (v, slot) in component.iter_mut().enumerate() {
        let r = find(&mut parent, v);
        let k = roots.iter().position(|&x| x == r).unwrap_or_else(|| {
            roots.push(r);
            roots.len() - 1
        });
        if roots.len() > 4 {
            return None;
        }
        *slot = k;
    }
    if roots.len() != 4 {
        return None;
    }
    let mut parts: Vec<heartsim_geometry::TriMesh> = vec![heartsim_geometry::TriMesh::default(); 4];
    let mut remap = vec![u32::MAX; n];
    for (v, vert) in mesh.vertices.iter().enumerate() {
        let part = &mut parts[component[v]];
        remap[v] = part.vertices.len() as u32;
        part.vertices.push(*vert);
    }
    for tri in mesh.indices.as_chunks::<3>().0 {
        let part = &mut parts[component[tri[0] as usize]];
        part.indices.extend(tri.iter().map(|&v| remap[v as usize]));
    }
    let centre = |m: &heartsim_geometry::TriMesh| {
        let (lo, hi) = m.bounds();
        (lo + hi) * 0.5
    };
    let mut left: Vec<(Vec3, heartsim_geometry::TriMesh)> =
        parts.into_iter().map(|m| (centre(&m), m)).collect();
    let mut take = |key: fn(Vec3) -> f32| {
        let i = (0..left.len())
            .min_by(|&a, &b| key(left[a].0).total_cmp(&key(left[b].0)))
            .unwrap_or(0);
        left.remove(i).1
    };
    let pulmonic = take(|c| -(c.y + c.z));
    let tricuspid = take(|c| c.x);
    let mitral = take(|c| c.z);
    let aortic = take(|c| c.x);
    Some(vec![
        (Valve::Mitral, mitral),
        (Valve::Tricuspid, tricuspid),
        (Valve::Aortic, aortic),
        (Valve::Pulmonic, pulmonic),
    ])
}

/// A valve ring's colour at highlight `level` (`0..=1`): its flat colour moved towards
/// [`VALVE_HIGHLIGHT`].
#[must_use]
pub fn valve_colour(base: [f32; 3], level: f32) -> [f32; 3] {
    let k = level.clamp(0.0, 1.0);
    std::array::from_fn(|i| base[i] + (VALVE_HIGHLIGHT[i] - base[i]) * k)
}

/// What the heart surface shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColourMode {
    /// Transmembrane potential now, mV: the moving depolarisation / repolarisation wave.
    Voltage,
    /// Activation time of each cell in the current beat, ms after the beat started
    /// (isochrones). Cells not yet reached this beat show the latest colour.
    Isochrone,
    /// Action-potential duration assigned at the last activation, ms (dispersion of
    /// repolarisation; static between beats).
    Apd,
}

impl ColourMode {
    pub const ALL: [ColourMode; 3] = [ColourMode::Voltage, ColourMode::Isochrone, ColourMode::Apd];

    #[must_use]
    pub fn label(self) -> String {
        match self {
            ColourMode::Voltage => t!("colour-voltage"),
            ColourMode::Isochrone => t!("colour-activation"),
            ColourMode::Apd => t!("colour-apd"),
        }
    }

    #[must_use]
    pub fn lut(self) -> LutId {
        match self {
            ColourMode::Voltage => LutId::Voltage,
            ColourMode::Isochrone => LutId::Isochrone,
            ColourMode::Apd => LutId::Apd,
        }
    }

    #[must_use]
    pub fn unit(self) -> &'static str {
        match self {
            ColourMode::Voltage => "mV",
            ColourMode::Isochrone | ColourMode::Apd => "ms",
        }
    }

    /// Range the legend handles can cover.
    #[must_use]
    pub fn range(self) -> (f32, f32) {
        match self {
            ColourMode::Voltage => (-100.0, 60.0),
            ColourMode::Isochrone => (0.0, 500.0),
            ColourMode::Apd => (100.0, 500.0),
        }
    }

    /// Default window: full AP amplitude; P wave to end of QRS (≈ 100 ms atria + 160 ms PR
    /// segment and QRS at 70 bpm); the physiological ventricular APD spread.
    #[must_use]
    pub fn default_window(self) -> (f32, f32) {
        match self {
            ColourMode::Voltage => (REST_MV, PEAK_MV),
            ColourMode::Isochrone => (0.0, 260.0),
            ColourMode::Apd => (180.0, 320.0),
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Whether clicks report their position instead of acting on the heart (placing the ablation tip).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Pick {
    Off,
    Waiting,
    Hit(Vec3),
}

/// What a plain click on the heart does (shift-click always ablates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickTool {
    /// Programmed stimulus at the clicked cell (an ectopic beat).
    Stimulate,
    /// Make all cells within the radius inexcitable (an ablation lesion).
    Ablate,
}

/// Read-out of the cell under the cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct CellInfo {
    pub cell: u32,
    pub region: Region,
    pub territory: Territory,
    pub vm_mv: f32,
    /// Activation time in the current beat, ms after its start (`None` = not yet this beat).
    pub activation_ms: Option<f32>,
    pub apd_ms: f32,
    /// Longitudinal conduction velocity of the region (before modifiers), m/s.
    pub cv_m_s: f64,
}

/// One anatomy-pack mesh in the scene.
#[derive(Debug)]
struct HeartLayer {
    name: String,
    kind: Option<MeshKind>,
    /// Vertex → cell interpolation; `None` for flat-coloured meshes (vessels, valves, torso).
    hoods: Option<Vec<Hood>>,
    /// Smooths the hood-interpolated vertex field along the mesh edges (hooded layers only).
    smoother: Option<VertexSmoother>,
    /// Per-vertex scalars of the current colour mode.
    scalars: Vec<f32>,
    id: Option<LayerId>,
    /// A valve mesh split into its four discs (B27 part B), each drawn as its own scene layer so it
    /// can flash with its closure sound; empty for every other mesh (and a valve mesh that does
    /// not split into four).
    valves: Vec<(heartsim_ecg::Valve, heartsim_geometry::TriMesh, Option<LayerId>)>,
}

/// Anatomy, engine and display state of the 3-D heart.
pub struct HeartView {
    engine: EikonalEngine,
    params: Parameters,
    pack_meshes: Vec<heartsim_geometry::NamedMesh>,
    /// Mesh of the "torso" layer: the human figure when the pack has one, else the pack torso.
    torso: heartsim_geometry::TriMesh,
    aha_segments: Vec<u8>,
    layers: Vec<HeartLayer>,
    per_cell: Vec<f32>,
    /// Scratch for [`VertexSmoother::smooth`].
    scratch: Vec<f32>,
    /// Start of the current beat (last pacemaker discharge or stimulus), ms.
    beat_start_ms: f64,
    now_ms: f64,
    /// Show the eikonal map (Tier 1); off = strip only (Tier 0), the heart is drawn flat.
    pub tier1: bool,
    pub mode: ColourMode,
    windows: [(f32, f32); 3],
    pub lut_style: LutStyle,
    luts_dirty: bool,
    pub wavefront: bool,
    pub panel: AnatomyPanelState,
    pub tool: ClickTool,
    pub ablate_radius_mm: f32,
    camera_command: Option<CameraCommand>,
    hover: Option<CellInfo>,
    status: Option<String>,
    /// Half the diagonal of the heart's bounding box, mm (cut-plane travel).
    radius_mm: f32,
    render_state: Option<egui_wgpu::RenderState>,
    /// Pacemaker lead markers (B13): which channels have a lead, the glyph set and its spheres.
    pacer_channels: [bool; 3],
    lead_set: Option<GlyphSetId>,
    lead_glyphs: Vec<GlyphInstance>,
    /// Extra markers drawn with the lead glyphs (coronary territory highlight, B14).
    overlay_glyphs: Vec<GlyphInstance>,
    /// EP catheters and lesions (B22): glyph set, and spheres and tubes waiting to be uploaded.
    catheter_set: Option<GlyphSetId>,
    catheter_glyphs: Option<(Vec<GlyphInstance>, Vec<GlyphInstance>)>,
    /// What a click does in the EP lab.
    pick: Pick,
    /// Per-cell values to show instead of the live engine state (scrubbing a paused strip).
    scrub_cells: Option<Vec<f32>>,
    /// Cell state of the Tier-2 engine while a re-entry scenario runs (B17): shown instead of
    /// the eikonal engine's, which is idle then.
    external: Option<heartsim_sim::CellState>,
    /// Clicks made while `external` is set, as actions for the engine that owns the heart.
    pending_actions: Vec<Action>,
    /// Highlight of each valve ring this frame, `heartsim_ecg::Valve::ALL` order (B27 part B).
    valve_glow: [f32; 4],
    /// Wall motion (B30); `None` when the pack lacks what the model needs (the heart is then drawn
    /// at rest and the controls are absent).
    motion: Option<HeartMotion>,
    /// Frame numbers (`Context::cumulative_frame_nr`) in which the 3-D view and the read-out card
    /// were last drawn: the card alone triggers a read-out evaluation, the view an upload.
    view_frame: Option<u64>,
    card_frame: Option<u64>,
    /// Scratch for the vertices of one valve disc.
    disc_scratch: Vec<heartsim_geometry::Vertex>,
    /// The measured tricuspid valve (B35), when the pack has one and the motion model is built.
    tricuspid: Option<ValveLayer>,
    /// The echo cross-section's anatomy (B31), built on first use.
    echo: EchoState,
}

/// Where the echo anatomy is: waiting to be built from a reduced copy of the pack (the cells, the
/// figure and the label volumes: what [`EchoAnatomy::new`] reads, about 20 MB), built, or not
/// possible (a pack without label volumes, or without the wall-motion model).
#[derive(Debug)]
enum EchoState {
    Source(Box<AnatomyPack>),
    Built(Box<(EchoAnatomy, Option<ViewTargets>)>),
    Unavailable,
}

/// The parts of `pack` that [`EchoAnatomy::new`] reads, copied before the engine consumes the cells.
fn echo_source(pack: &AnatomyPack) -> Box<AnatomyPack> {
    Box::new(AnatomyPack {
        manifest: pack.manifest.clone(),
        // only the IVC (the standard views' vena cava check) is read from the meshes
        meshes: pack.meshes.iter().filter(|m| m.name == "ivc").cloned().collect(),
        cells: pack.cells.clone(),
        hoods: std::collections::BTreeMap::new(),
        landmarks: pack.landmarks.clone(),
        electrodes: pack.electrodes.clone(),
        torso: if pack.body.is_some() {
            heartsim_geometry::TriMesh::default()
        } else {
            pack.torso.clone()
        },
        body: pack.body.clone(),
        aha_segments: pack.aha_segments.clone(),
        licence: String::new(),
        volumes: pack.volumes.clone(),
        valves: pack.valves.clone(),
    })
}

impl std::fmt::Debug for HeartView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeartView")
            .field("cells", &self.per_cell.len())
            .field("layers", &self.layers.len())
            .field("mode", &self.mode)
            .field("tier1", &self.tier1)
            .finish_non_exhaustive()
    }
}

/// Where the default anatomy pack is looked for: the first assets root (see
/// [`crate::asset_dirs::asset_roots`]) that holds `anatomy/default/manifest.ron`.
#[cfg(feature = "native")]
#[must_use]
pub fn default_pack_dir() -> Option<PathBuf> {
    crate::asset_dirs::resolved_assets_dir().map(|r| r.join("anatomy").join("default"))
}

/// Display name of a pack mesh, for the layer list and the labels.
fn pretty_name(name: &str) -> String {
    match name {
        "lv" => t!("part-lv"),
        "rv" => t!("part-rv"),
        "la" => t!("part-la"),
        "ra" => t!("part-ra"),
        "aorta" => t!("part-aorta"),
        "pulmonary_artery" => t!("part-pulmonary-artery"),
        "pulmonary_veins" => t!("part-pulmonary-veins"),
        "svc" => t!("part-svc"),
        "ivc" => t!("part-ivc"),
        "valve_rings" => t!("part-valve-rings"),
        "valve_planes" => t!("part-valve-planes"),
        TRICUSPID_VALVE_LAYER => t!("part-tricuspid-valve"),
        "torso" => t!("part-torso"),
        other => other.to_string(),
    }
}

/// Flat colour (linear RGB) of a mesh: oxygenated vessels red, deoxygenated blue (textbook); the
/// torso the light blue of the frontal torso illustration in the axis panel, so the two agree.
fn base_colour(name: &str, kind: Option<MeshKind>) -> [f32; 3] {
    match (name, kind) {
        ("aorta" | "pulmonary_veins", _) => [0.55, 0.05, 0.05],
        ("pulmonary_artery" | "svc" | "ivc", _) => [0.12, 0.16, 0.5],
        (_, Some(MeshKind::Valve)) => [0.8, 0.75, 0.55],
        (_, None) => [0.3, 0.55, 0.85],
        _ => [0.5, 0.16, 0.14],
    }
}

/// Timing-engine site (landmark prefix it understands) for a stimulus in `region`, so a click
/// on the mesh also schedules the beat on the strip. `None` for conduction tissue.
fn timing_site(region: Region) -> Option<StimSite> {
    let name = match region {
        Region::LeftVentricle | Region::VentricularSeptum => "lv",
        Region::RightVentricle => "rv",
        Region::LeftAtrium
        | Region::LeftAtrialAppendage
        | Region::PulmonaryVeinSleeves
        | Region::BachmannBundle => "la",
        r if r.is_atrial() => "ra",
        _ => return None,
    };
    Some(StimSite::Landmark(name.to_string()))
}

impl HeartView {
    /// Load the default anatomy pack and build the engine for `timing`.
    ///
    /// # Errors
    /// When no pack is found or it fails to load.
    #[cfg(feature = "native")]
    pub fn load_default(timing: &TimingParams) -> anyhow::Result<Self> {
        let dir = default_pack_dir().context("anatomy pack `assets/anatomy/default` not found")?;
        let pack = AnatomyPack::load(&dir).with_context(|| format!("loading {}", dir.display()))?;
        Ok(Self::from_pack(pack, timing))
    }

    /// Build the eikonal engine on `pack` (inheriting His–Purkinje delays and accessory-pathway
    /// sides from `timing`, as `Tier1Engine::new` does) and the display layers.
    #[must_use]
    pub fn from_pack(pack: AnatomyPack, timing: &TimingParams) -> Self {
        // the motion model reads the whole pack (meshes, hoods, cells); the engine below consumes
        // it, so the model is built first
        let motion = HeartMotion::new(&pack, pack.cells.len())
            .inspect_err(|e| tracing::warn!("no wall motion: {e}"))
            .ok();
        let echo = if motion.is_some() && pack.volumes.is_some() {
            EchoState::Source(echo_source(&pack))
        } else {
            EchoState::Unavailable
        };
        let params = Parameters::default();
        let mut engine = EikonalEngine::with_seed_sets(
            Arc::new(pack.cells),
            &params,
            EikonalParams::default(),
            &pack.landmarks.sets,
        );
        engine.set_his_purkinje(timing.his_purkinje.clone());
        engine.set_accessory(
            timing
                .accessory
                .as_ref()
                .map(|ap| (ap.ventricular_side, ap.atrial_side)),
        );
        // coronary territories for `Action::Stenosis` (B14)
        if pack.aha_segments.len() == engine.cloud().len() {
            engine.set_aha_segments(&pack.aha_segments);
        }
        let mut hoods = pack.hoods;
        let mut layers: Vec<HeartLayer> = pack
            .meshes
            .iter()
            .map(|m| {
                let h = if m.conducting() {
                    hoods.remove(&m.name)
                } else {
                    None
                };
                let valves = if m.kind == MeshKind::Valve {
                    split_valves(&m.mesh)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(v, mesh)| (v, mesh, None))
                        .collect()
                } else {
                    Vec::new()
                };
                HeartLayer {
                    name: m.name.clone(),
                    kind: Some(m.kind),
                    scalars: vec![0.0; if h.is_some() { m.mesh.vertex_count() } else { 0 }],
                    smoother: h.is_some().then(|| VertexSmoother::new(&m.mesh)),
                    hoods: h,
                    id: None,
                    valves,
                }
            })
            .collect();
        // the measured tricuspid valve (B35): a layer of its own, off until asked for (the
        // other three valves have no leaflets in the 3-D heart)
        let tricuspid = motion.as_ref().and_then(|m| m.valve()).map(|v| ValveLayer {
            render: heartsim_geometry::ValveRenderMesh::new(
                &v.data,
                DesignedParams::default().av_leaflet_thickness_mm,
            ),
            id: None,
            key: None,
            scratch: Vec::new(),
            posed: Vec::new(),
            vertices: Vec::new(),
        });
        if tricuspid.is_some() {
            layers.push(HeartLayer {
                name: TRICUSPID_VALVE_LAYER.to_string(),
                kind: Some(MeshKind::Valve),
                hoods: None,
                smoother: None,
                scalars: Vec::new(),
                id: None,
                valves: Vec::new(),
            });
        }
        layers.push(HeartLayer {
            name: "torso".to_string(),
            kind: None,
            hoods: None,
            smoother: None,
            scalars: Vec::new(),
            id: None,
            valves: Vec::new(),
        });
        let mut panel = AnatomyPanelState::new(layers.iter().map(|l| (l.name.clone(), 1.0)));
        // the torso: off until asked for, and translucent like the illustration when on
        if let Some(t) = panel.layers.last_mut() {
            t.visible = false;
            t.alpha = 0.2;
        }
        if let Some(t) = panel.layers.iter_mut().find(|l| l.name == TRICUSPID_VALVE_LAYER) {
            t.visible = false;
        }
        let n = engine.cloud().len();
        Self {
            engine,
            params,
            pack_meshes: pack.meshes,
            torso: pack.body.unwrap_or(pack.torso),
            aha_segments: pack.aha_segments,
            layers,
            per_cell: vec![0.0; n],
            scratch: Vec::new(),
            beat_start_ms: 0.0,
            now_ms: 0.0,
            tier1: true,
            mode: ColourMode::Voltage,
            windows: ColourMode::ALL.map(ColourMode::default_window),
            lut_style: LutStyle::Classic,
            luts_dirty: true,
            wavefront: false,
            panel,
            tool: ClickTool::Stimulate,
            ablate_radius_mm: 6.0,
            camera_command: None,
            hover: None,
            status: None,
            radius_mm: 1.0,
            render_state: None,
            pacer_channels: [false; 3],
            lead_set: None,
            lead_glyphs: Vec::new(),
            overlay_glyphs: Vec::new(),
            catheter_set: None,
            catheter_glyphs: None,
            pick: Pick::Off,
            scrub_cells: None,
            external: None,
            pending_actions: Vec::new(),
            valve_glow: [0.0; 4],
            motion,
            view_frame: None,
            card_frame: None,
            disc_scratch: Vec::new(),
            tricuspid,
            echo,
        }
    }

    /// Whether the measured tricuspid valve is drawn (`None` when the pack has none). Stored with
    /// the preferences by the app.
    #[must_use]
    pub fn tricuspid_valve_visible(&self) -> Option<bool> {
        self.tricuspid.as_ref()?;
        self.panel.layer(TRICUSPID_VALVE_LAYER).map(|l| l.visible)
    }

    /// Show or hide the measured tricuspid valve (no effect without one).
    pub fn set_tricuspid_valve_visible(&mut self, on: bool) {
        if let Some(l) = self
            .panel
            .layers
            .iter_mut()
            .find(|l| l.name == TRICUSPID_VALVE_LAYER)
        {
            l.visible = on;
        }
    }

    /// The measured tricuspid valve as it would be drawn now (heart frame, mm; the midsurface and
    /// chordae vertices, before the thickness), with its opening; `None` without one (tests).
    #[must_use]
    pub fn tricuspid_valve_pose(&self) -> Option<(f32, Vec<Vec3>)> {
        let m = self.motion.as_ref()?;
        let (mut a, mut b) = (Vec::new(), Vec::new());
        let o = m.pose_valve(&mut a, &mut b)?;
        Some((o, b))
    }

    /// How bright each valve ring is drawn this frame (`heartsim_ecg::Valve::ALL` order, `0..=1`):
    /// the app passes the highlights of the heart sounds at the time the views are drawn, or zeros
    /// with reduced motion. Only the `valve_planes` layer shows it.
    pub fn set_valve_glow(&mut self, glow: [f32; 4]) {
        self.valve_glow = glow;
    }

    // ---- wall motion (B30) -----------------------------------------------------------------

    /// The wall-motion model and its state, if the pack supports it.
    #[must_use]
    pub fn motion(&self) -> Option<&HeartMotion> {
        self.motion.as_ref()
    }

    /// The wall-motion state, for the switch, the exaggeration and the evaluation policy.
    pub fn motion_mut(&mut self) -> Option<&mut HeartMotion> {
        self.motion.as_mut()
    }

    /// The user's choices from the settings: reduced motion (forces the motion off), the "Wall
    /// motion" switch and the exaggeration (clamped to what the slider offers).
    pub fn set_motion_options(&mut self, reduced_motion: bool, on: bool, exaggeration: f32) {
        if let Some(m) = &mut self.motion {
            m.reduced_motion = reduced_motion;
            m.on = on;
            m.exaggeration =
                exaggeration.clamp(heart_motion::MIN_EXAGGERATION, heart_motion::MAX_EXAGGERATION);
        }
    }

    /// The switch and the exaggeration as the user left them (what the settings store).
    #[must_use]
    pub fn motion_options(&self) -> Option<(bool, f32)> {
        self.motion.as_ref().map(|m| (m.on, m.exaggeration))
    }

    /// Feed one step's timing events and the engine's breathing clock to the motion record
    /// (every 1 ms step, where [`Self::on_step`] runs). Never read back by an engine.
    pub fn motion_events(&mut self, events: &[Event], respiration: RespiratoryClock) {
        if let Some(m) = &mut self.motion {
            m.on_events(events, respiration);
        }
    }

    /// Note the activations of the cell state on display (after [`Self::on_step`], or once per
    /// frame while an external engine drives the heart).
    pub fn motion_record_cells(&mut self) {
        let cells = self.external.as_ref().unwrap_or_else(|| self.engine.cells());
        if let Some(m) = &mut self.motion {
            m.record_cells(cells);
        }
    }

    /// A new scenario: forget the motion record and the lesions (called where the heart sounds
    /// are reset, with the same breathing rate and `auscultation`).
    pub fn reset_motion(&mut self, respiration_rate_bpm: f64, auscultation: &heartsim_core::Auscultation) {
        if let Some(m) = &mut self.motion {
            m.reset(respiration_rate_bpm, auscultation);
        }
    }

    /// The coronary lesions changed (an `Action::Stenosis` reached the 3-D engine).
    pub fn refresh_motion_injury(&mut self) {
        if let Some(m) = &mut self.motion {
            m.refresh_injury(&self.engine, &self.aha_segments);
        }
    }

    /// The time the view is drawn at (engine time minus display delay and scrub, ms) and the
    /// respiratory phase there.
    pub fn set_motion_time(&mut self, t_ms: f64, resp_phase: f64) {
        if let Some(m) = &mut self.motion {
            m.set_time(t_ms, resp_phase);
        }
    }

    /// Evaluate the wall motion if the policy in [`crate::heart_motion`] says so. Returns the
    /// seconds to wait before the next evaluation is due, if the rate cap held one back.
    pub fn tick_motion(&mut self, wall_s: f64, want_vertices: bool, want_readout: bool) -> Option<f64> {
        match self.motion.as_mut()?.tick(wall_s, want_vertices, want_readout) {
            Tick::Wait(s) => Some(s),
            _ => None,
        }
    }

    // ---- the echo cross-section (B31) -----------------------------------------------------

    /// Whether the pack can show the echo page (it has the label volumes and the motion model).
    #[must_use]
    pub fn echo_possible(&self) -> bool {
        !matches!(self.echo, EchoState::Unavailable)
    }

    /// Build the echo anatomy now if it is still waiting (about 0.7 s natively, once; the
    /// reduced pack copy is dropped afterwards). A failure is logged and leaves the page
    /// unavailable.
    pub fn build_echo(&mut self) {
        if let EchoState::Source(_) = &self.echo
            && let EchoState::Source(pack) = std::mem::replace(&mut self.echo, EchoState::Unavailable)
        {
            let built = self.motion.as_ref().map(|m| {
                EchoAnatomy::new(&pack, m.model(), DesignedParams::default()).map(|a| {
                    let views = ViewTargets::new(&pack, m.model(), &a);
                    (a, views)
                })
            });
            self.echo = match built {
                Some(Ok(a)) => EchoState::Built(Box::new(a)),
                Some(Err(e)) => {
                    tracing::warn!("no echo cross-section: {e}");
                    EchoState::Unavailable
                }
                None => EchoState::Unavailable,
            };
        }
    }

    /// The echo anatomy, once [`Self::build_echo`] has built it.
    #[must_use]
    pub fn echo_anatomy(&self) -> Option<&EchoAnatomy> {
        match &self.echo {
            EchoState::Built(a) => Some(&a.0),
            _ => None,
        }
    }

    /// The echo anatomy and the motion parts a scan needs, borrowed together (B34): the scans
    /// evaluate the model at their own times.
    pub fn echo_scan_parts(&mut self) -> Option<(&EchoAnatomy, heart_motion::ScanParts<'_>)> {
        let anatomy = match &self.echo {
            EchoState::Built(a) => &a.0,
            _ => return None,
        };
        Some((anatomy, self.motion.as_mut()?.scan_parts()))
    }

    /// The standard views' targets (B33), built with the echo anatomy.
    #[must_use]
    pub fn echo_views(&self) -> Option<&ViewTargets> {
        match &self.echo {
            EchoState::Built(a) => a.1.as_ref(),
            _ => None,
        }
    }

    /// The scene layer of the figure's skin ("torso"), once the view is installed.
    #[must_use]
    pub fn torso_layer(&self) -> Option<LayerId> {
        self.layers.iter().find(|l| l.name == "torso").and_then(|l| l.id)
    }

    /// The 3-D view's per-frame work for a page that arranges the scene itself (the Echo page):
    /// the colour map, the wall-motion policy (**the one evaluation per drawn frame**, capped in
    /// rate, shared with the cross-section) and the upload of the moving vertices, then `f` with
    /// the scene. Layer visibility and opacity come from the anatomy panel as usual; the camera
    /// and the clip plane are left to `f`. No picking, no click on the heart. `None` before the
    /// view is installed in a GPU scene.
    pub fn with_scene<R>(
        &mut self,
        ui: &egui::Ui,
        f: impl FnOnce(&mut SceneRenderer, &egui_wgpu::RenderState) -> R,
    ) -> Option<R> {
        let rs = self.render_state.clone()?;
        if self.tier1 {
            self.update_scalars();
        }
        self.view_frame = Some(ui.ctx().cumulative_frame_nr());
        if let Some(wait_s) = self.tick_motion(ui.input(|i| i.time), true, false) {
            ui.ctx().request_repaint_after_secs(wait_s as f32);
        }
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let mut renderer = rs.renderer.write();
        let res = renderer.callback_resources.get_mut::<ViewportResources>()?;
        self.upload_motion(&mut res.scene, &rs.queue);
        self.sync_scene(&mut res.scene, &rs.queue, dt, true);
        Some(f(&mut res.scene, &rs))
    }

    /// Write the evaluated vertices to the GPU layers, once per evaluation. Picking and the
    /// markers keep the rest positions (see [`Self::viewport`]).
    fn upload_motion(&mut self, scene: &mut SceneRenderer, queue: &wgpu::Queue) {
        self.upload_frame(scene, queue);
        self.upload_valve(scene, queue);
    }

    /// The measured tricuspid valve, posed at the drawn instant and riding the drawn disc,
    /// re-uploaded only when its pose changed (the frame or the opening) and only while shown.
    fn upload_valve(&mut self, scene: &mut SceneRenderer, queue: &wgpu::Queue) {
        let (Some(motion), Some(v)) = (&self.motion, &mut self.tricuspid) else {
            return;
        };
        let Some(id) = v.id else { return };
        if !self.panel.layer(TRICUSPID_VALVE_LAYER).is_some_and(|l| l.visible) {
            return;
        }
        let key = motion.valve_key();
        if v.key == Some(key) {
            return;
        }
        if motion.pose_valve(&mut v.scratch, &mut v.posed).is_some() {
            v.render.fill(&v.posed, &mut v.vertices);
            scene.set_layer_vertices(queue, id, &v.vertices);
            v.key = Some(key);
        }
    }

    fn upload_frame(&mut self, scene: &mut SceneRenderer, queue: &wgpu::Queue) {
        let Some(motion) = &mut self.motion else { return };
        let discs: Vec<(heartsim_ecg::Valve, Vec<u32>)> = motion
            .model()
            .valve_discs()
            .iter()
            .map(|d| (d.valve, d.vertices.clone()))
            .collect();
        let Some(frame) = motion.take_upload() else { return };
        for l in &self.layers {
            let Some(part) = frame.part(&l.name) else { continue };
            if let Some(id) = l.id {
                scene.set_layer_vertices(queue, id, &part.vertices);
            }
            for (valve, _, id) in &l.valves {
                let (Some(id), Some((_, idx))) = (*id, discs.iter().find(|(v, _)| v == valve)) else {
                    continue;
                };
                self.disc_scratch.clear();
                self.disc_scratch
                    .extend(idx.iter().map(|&i| part.vertices[i as usize]));
                scene.set_layer_vertices(queue, id, &self.disc_scratch);
            }
        }
    }

    /// The valve discs found in the anatomy, with the centre of each (tests, the tour).
    #[must_use]
    pub fn valve_parts(&self) -> Vec<(heartsim_ecg::Valve, Vec3)> {
        self.layers
            .iter()
            .flat_map(|l| l.valves.iter())
            .map(|(v, m, _)| {
                let (lo, hi) = m.bounds();
                (*v, (lo + hi) * 0.5)
            })
            .collect()
    }

    /// The Tier-1 engine (read-only).
    #[must_use]
    pub fn engine(&self) -> &EikonalEngine {
        &self.engine
    }

    /// The Tier-1 engine, for actions and modifiers (drug effects, shocks, lesions — B14).
    pub fn engine_mut(&mut self) -> &mut EikonalEngine {
        &mut self.engine
    }

    /// Territory weight of a coronary branch per cell (0 … 1), for highlighting it.
    #[must_use]
    pub fn territory_weight(&self, branch: heartsim_sim::ischaemia::CoronaryBranch) -> Vec<f32> {
        heartsim_sim::ischaemia::territory_weight(
            self.engine.cloud(),
            self.engine.graph(),
            self.engine.kinds(),
            &self.aha_segments,
            branch,
        )
    }

    /// Extra markers drawn with the lead glyphs (replaced every call).
    pub fn set_overlay_glyphs(&mut self, glyphs: Vec<GlyphInstance>) {
        self.overlay_glyphs = glyphs;
    }

    /// Show EP catheters and lesions on the heart (spheres, tubes).
    pub fn set_catheter_glyphs(&mut self, spheres: Vec<GlyphInstance>, tubes: Vec<GlyphInstance>) {
        self.catheter_glyphs = Some((spheres, tubes));
    }

    /// Let clicks report their position instead of acting on the heart (placing the ablation tip).
    pub fn set_pick_mode(&mut self, on: bool) {
        self.pick = if on { Pick::Waiting } else { Pick::Off };
    }

    /// The last click in pick mode, once.
    pub fn take_pick(&mut self) -> Option<Vec3> {
        if let Pick::Hit(p) = self.pick {
            self.pick = Pick::Waiting;
            Some(p)
        } else {
            None
        }
    }

    /// Put the pacemaker leads at the sites of `cfg` (B13); only the channels its mode uses are
    /// marked on the heart.
    pub fn set_pacer_leads(&mut self, cfg: &PacerConfig) {
        self.engine.set_pacer_leads(cfg);
        let m = cfg.mode;
        let v = m.paces_ventricle() || m.senses_ventricle();
        self.pacer_channels = [
            m.paces_atrium() || m.senses_atrium(),
            v,
            v && cfg.lv_lead.is_some(),
        ];
    }

    /// Lead-tip cells per pacer channel (index = `PacerChannel`).
    #[must_use]
    pub fn lead_tip_cells(&self) -> [Vec<u32>; 3] {
        let s = self.engine.seeds();
        [
            s.atrial_lead.clone(),
            s.ventricular_lead.clone(),
            s.lv_lead.clone(),
        ]
    }

    /// Lead-tip positions (heart mm) of the channels in use.
    #[must_use]
    pub fn lead_tips(&self) -> [Option<Vec3>; 3] {
        let cells = self.lead_tip_cells();
        std::array::from_fn(|i| {
            let c = *cells[i].first()?;
            self.pacer_channels[i].then(|| self.engine.cloud().positions[c as usize])
        })
    }

    /// Lead markers to draw this frame.
    pub fn set_lead_glyphs(&mut self, glyphs: Vec<GlyphInstance>) {
        self.lead_glyphs = glyphs;
    }

    /// Current colour window `(min, max)` of `mode`, in its unit.
    #[must_use]
    pub fn window(&self, mode: ColourMode) -> (f32, f32) {
        self.windows[mode.index()]
    }

    /// Advance with the timing events of one fixed step of length `dt` ending at `now` — the
    /// same ingest-then-step order as `Tier1Engine`.
    pub fn on_step(&mut self, events: &[Event], dt: Millis, now: Millis) {
        for e in events {
            if matches!(e.kind, EventKind::Automatic | EventKind::Stimulus) {
                self.beat_start_ms = e.at.0;
            }
        }
        self.now_ms = now.0;
        if self.tier1 && self.external.is_none() {
            self.engine.ingest(events);
            self.engine.step(dt);
        }
    }

    /// Turn the 3-D engine on or off. Re-enabling restarts it at the current time (the
    /// heart is at rest until the next beat), keeping clicks and ablations deterministic.
    pub fn set_tier1(&mut self, on: bool) {
        if on && !self.tier1 {
            self.engine.reset();
            self.engine.step(Millis(self.now_ms));
        }
        self.tier1 = on;
    }

    /// Show `cells` (the Tier-2 engine's state of a re-entry scenario, B17) instead of the
    /// eikonal engine's; `None` returns to it. Clicks are then collected as actions
    /// ([`HeartView::take_pending_actions`]) for the engine that owns the heart.
    pub fn set_external_cells(&mut self, cells: Option<&heartsim_sim::CellState>) {
        match (cells, &mut self.external) {
            (Some(c), Some(buf)) => buf.clone_from(c),
            (Some(c), slot) => *slot = Some(c.clone()),
            (None, slot) => *slot = None,
        }
    }

    /// Actions the user made on the heart while it is driven externally (stimulus, ablation).
    pub fn take_pending_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.pending_actions)
    }

    /// The cell state on display: the external engine's, else the eikonal engine's.
    fn shown_cells(&self) -> &heartsim_sim::CellState {
        self.external.as_ref().unwrap_or_else(|| self.engine.cells())
    }

    /// Per-cell value of the current colour mode, in its display unit.
    fn fill_per_cell(&mut self) {
        let cells = self.external.as_ref().unwrap_or_else(|| self.engine.cells());
        let beat = self.beat_start_ms as f32;
        let late = self.window(ColourMode::Isochrone).1;
        match self.mode {
            ColourMode::Voltage => {
                for (o, v) in self.per_cell.iter_mut().zip(&cells.vm) {
                    *o = REST_MV + (PEAK_MV - REST_MV) * v;
                }
            }
            ColourMode::Isochrone => {
                for (o, t) in self.per_cell.iter_mut().zip(&cells.activation_ms) {
                    *o = if *t >= beat - 0.5 { *t - beat } else { late };
                }
            }
            ColourMode::Apd => self.per_cell.copy_from_slice(&cells.apd_ms),
        }
    }

    /// Per-cell values of the current colour mode from live engine state (what the history
    /// records for scrubbing).
    pub fn cell_values(&mut self) -> &[f32] {
        self.fill_per_cell();
        &self.per_cell
    }

    /// Show `cells` (per-cell values of the current mode, e.g. a history snapshot) instead of
    /// the live state; `None` returns to live.
    pub fn set_scrub_cells(&mut self, cells: Option<&[f32]>) {
        match (cells, &mut self.scrub_cells) {
            (Some(c), Some(buf)) => {
                buf.clear();
                buf.extend_from_slice(c);
            }
            (Some(c), slot) => *slot = Some(c.to_vec()),
            (None, slot) => *slot = None,
        }
    }

    /// Re-time the 3-D engine for a newly loaded scenario: His–Purkinje delays and accessory
    /// pathway from `timing`, tissue restored, clock back to zero.
    pub fn retime(&mut self, timing: &TimingParams) {
        self.engine.set_his_purkinje(timing.his_purkinje.clone());
        self.engine.set_accessory(
            timing
                .accessory
                .as_ref()
                .map(|ap| (ap.ventricular_side, ap.atrial_side)),
        );
        self.engine.reset();
        self.beat_start_ms = 0.0;
        self.now_ms = 0.0;
        self.scrub_cells = None;
        self.external = None;
        self.pending_actions.clear();
        self.status = None;
    }

    /// Recompute every hooded layer's vertex scalars from engine state (or the scrub snapshot).
    pub fn update_scalars(&mut self) {
        match &self.scrub_cells {
            Some(c) if c.len() == self.per_cell.len() => self.per_cell.copy_from_slice(c),
            _ => self.fill_per_cell(),
        }
        for l in &mut self.layers {
            if let Some(h) = &l.hoods {
                map_hoods(h, &self.per_cell, &mut l.scalars);
                if let Some(sm) = &l.smoother {
                    sm.smooth(&mut l.scalars, DISPLAY_SMOOTHING_PASSES, &mut self.scratch);
                }
            }
        }
    }

    /// Vertex scalars of the mesh called `name` (empty for flat layers).
    #[must_use]
    pub fn layer_scalars(&self, name: &str) -> &[f32] {
        self.layers
            .iter()
            .find(|l| l.name == name)
            .map_or(&[], |l| l.scalars.as_slice())
    }

    /// What the cursor would read at `cell`.
    #[must_use]
    pub fn cell_info(&self, cell: u32) -> CellInfo {
        let i = cell as usize;
        let cells = self.shown_cells();
        let region = self.engine.cloud().regions[i];
        let t = cells.activation_ms[i];
        let beat = self.beat_start_ms as f32;
        CellInfo {
            cell,
            region,
            territory: Territory::for_cell(region, self.aha_segments.get(i).copied().unwrap_or(0)),
            vm_mv: REST_MV + (PEAK_MV - REST_MV) * cells.vm[i],
            activation_ms: (t >= beat - 0.5).then_some(t - beat),
            apd_ms: cells.apd_ms[i],
            cv_m_s: self.params.region(region).cv_long_m_s,
        }
    }

    /// Stimulate (or, with `ablate`, ablate around) the cell nearest to `point` (heart mm),
    /// in both engines where applicable. `timing` receives the chamber-level stimulus so the
    /// strip shows the same beat.
    pub fn click(&mut self, point: Vec3, ablate: bool, timing: &mut TimingEngine) {
        let Some(cell) = self.engine.cloud().nearest(point) else {
            return;
        };
        let now = timing.now();
        let region = self.engine.cloud().regions[cell as usize];
        let site = StimSite::Cell(cell);
        if self.external.is_some() {
            // re-entry scenario: the engine that owns the heart takes the action
            let (action, text) = if ablate {
                (
                    Action::Ablate {
                        site,
                        radius_mm: f64::from(self.ablate_radius_mm),
                    },
                    format!(
                        "Ablated {:.0} mm around {}",
                        self.ablate_radius_mm,
                        region.label()
                    ),
                )
            } else {
                (
                    Action::Stimulate {
                        site,
                        strength: 1.0,
                        duration_ms: 2.0,
                    },
                    format!("Stimulus: {} at {:.0} ms", region.label(), now.0),
                )
            };
            self.pending_actions.push(action);
            self.status = Some(text);
            return;
        }
        let result = if ablate {
            let action = Action::Ablate {
                site,
                radius_mm: f64::from(self.ablate_radius_mm),
            };
            self.engine.apply(&action, now).map(|()| {
                t!(
                    "heart-ablated",
                    radius = format!("{:.0}", self.ablate_radius_mm),
                    region = heartsim_ui::labels::region_name(region)
                )
            })
        } else {
            let mesh = Action::Stimulate {
                site,
                strength: 1.0,
                duration_ms: 2.0,
            };
            let r = self.engine.apply(&mesh, now);
            if let Some(site) = timing_site(region) {
                // the mesh already accepted it; a timing refusal only means no strip beat
                let _ = timing.apply(
                    &Action::Stimulate {
                        site,
                        strength: 1.0,
                        duration_ms: 2.0,
                    },
                    now,
                );
            }
            r.map(|()| {
                t!(
                    "heart-stimulus",
                    region = heartsim_ui::labels::region_name(region),
                    time = format!("{:.0}", now.0)
                )
            })
        };
        self.status = Some(result.unwrap_or_else(|e| e.to_string()));
    }

    /// Restore all ablated tissue (restarts the 3-D engine at the current time).
    pub fn clear_ablation(&mut self) {
        self.engine.reset();
        self.engine.step(Millis(self.now_ms));
        self.status = Some(t!("heart-ablation-cleared"));
    }

    // ---- UI ------------------------------------------------------------------------------

    /// The whole left panel of the Simulator: [`Self::controls`] and the vector overlay's section,
    /// in a vertical scroll area whose bar stays visible while the content overflows (on a short
    /// window the sections run past the bottom; the bar says so).
    pub fn controls_panel(
        &mut self,
        ui: &mut egui::Ui,
        sections: &mut SimulatorSections,
        vector: &mut crate::vector_overlay::VectorOverlay,
    ) {
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.controls(ui, sections);
                let mut alphas: Vec<&mut f32> = self.panel.layers.iter_mut().map(|l| &mut l.alpha).collect();
                heartsim_ui::collapsible_section(ui, &mut sections.vector, &t!("vector-heading"), |ui| {
                    vector.controls(ui, &mut alphas);
                });
            });
    }

    /// Colour-map, tool, anatomy and wall-motion controls (left panel of the Simulator tab), as
    /// collapsible sections whose open state is `sections` (the caller stores it in the settings).
    /// The caller puts this in a vertical scroll area.
    pub fn controls(&mut self, ui: &mut egui::Ui, sections: &mut SimulatorSections) {
        // the layer list names follow the language; an unknown part keeps its pack id
        for l in &mut self.panel.layers {
            l.display = pretty_name(&l.name);
        }
        heartsim_ui::collapsible_section(ui, &mut sections.colour, &t!("heart-section-colour"), |ui| {
            self.colour_section(ui);
        });
        heartsim_ui::collapsible_section(ui, &mut sections.tool, &t!("heart-click"), |ui| {
            self.tool_section(ui);
        });
        heartsim_ui::collapsible_section(ui, &mut sections.layers, &t!("heart-section-layers"), |ui| {
            let r = ui.scope(|ui| anatomy_panel(ui, &mut self.panel));
            heartsim_ui::highlight::mark(&r.response, "camera_views");
            if let Some(c) = r.inner {
                self.camera_command = Some(c);
            }
        });
        if self.motion.is_some() {
            heartsim_ui::collapsible_section(ui, &mut sections.motion, &t!("heart-section-motion"), |ui| {
                self.motion_controls(ui);
            });
        }
    }

    /// The 3-D activation switch, the colour mode, its window and the wavefront outline.
    fn colour_section(&mut self, ui: &mut egui::Ui) {
        let mut tier1 = self.tier1;
        if ui
            .checkbox(&mut tier1, t!("heart-tier1"))
            .on_hover_text(t!("heart-tier1-hint"))
            .changed()
        {
            self.set_tier1(tier1);
        }
        ui.add_enabled_ui(self.tier1, |ui| {
            ui.horizontal(|ui| {
                for m in ColourMode::ALL {
                    ui.selectable_value(&mut self.mode, m, m.label());
                }
            });
            let table = self.mode.lut().table(self.lut_style);
            let (lo, hi) = self.mode.range();
            let w = &mut self.windows[self.mode.index()];
            let width = ui.available_width().min(260.0);
            colour_bar(ui, width, &table, &mut w.0, &mut w.1, lo..=hi, self.mode.unit());
            ui.horizontal(|ui| {
                if ui.button(t!("heart-reset-window")).clicked() {
                    *w = self.mode.default_window();
                }
                if ui
                    .checkbox(
                        &mut (self.lut_style == LutStyle::ColourBlindSafe),
                        t!("heart-colour-blind"),
                    )
                    .changed()
                {
                    self.lut_style = match self.lut_style {
                        LutStyle::Classic => LutStyle::ColourBlindSafe,
                        LutStyle::ColourBlindSafe => LutStyle::Classic,
                    };
                    self.luts_dirty = true;
                }
            });
            ui.add_enabled(
                self.mode == ColourMode::Voltage,
                egui::Checkbox::new(&mut self.wavefront, t!("heart-wavefront")),
            );
        });
    }

    /// Stimulate or ablate on a click, the lesion radius, and clearing the ablation.
    fn tool_section(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.tier1, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.tool, ClickTool::Stimulate, t!("heart-stimulate"));
                ui.radio_value(&mut self.tool, ClickTool::Ablate, t!("heart-ablate"))
                    .on_hover_text(t!("heart-shift-click"));
            });
            heartsim_ui::controls::slider(
                ui,
                &t!("heart-lesion-radius"),
                egui::Slider::new(&mut self.ablate_radius_mm, 2.0..=15.0),
                heartsim_ui::controls::MM,
            );
            heartsim_ui::helper_text(ui, t!("heart-shift-click"));
            if ui.button(t!("heart-clear-ablation")).clicked() {
                self.clear_ablation();
            }
            if let Some(s) = &self.status {
                ui.label(s);
            }
        });
    }

    /// The "Wall motion" switch, the exaggeration slider and the read-out card (B30). Absent when
    /// the pack gave no motion model.
    pub fn motion_controls(&mut self, ui: &mut egui::Ui) {
        let frame_nr = ui.ctx().cumulative_frame_nr();
        self.card_frame = Some(frame_nr);
        // no 3-D view drawn last frame or this one (the card without the view): read-out only
        if self.view_frame.is_none_or(|f| f + 1 < frame_nr)
            && let Some(wait_s) = self.tick_motion(ui.input(|i| i.time), false, true)
        {
            ui.ctx().request_repaint_after_secs(wait_s as f32);
        }
        let Some(m) = &mut self.motion else { return };
        let reduced = m.reduced_motion;
        let mut on = m.on && !reduced;
        let switch = ui
            .add_enabled(!reduced, egui::Checkbox::new(&mut on, t!("heart-motion")))
            .on_hover_text(if reduced {
                t!("heart-motion-reduced")
            } else {
                t!("heart-motion-hint")
            });
        if switch.changed() {
            m.on = on;
        }
        if reduced {
            heartsim_ui::helper_text(ui, t!("heart-motion-reduced"));
        }
        ui.add_enabled_ui(on, |ui| {
            heartsim_ui::controls::slider(
                ui,
                &t!("heart-motion-exaggeration"),
                egui::Slider::new(
                    &mut m.exaggeration,
                    heart_motion::MIN_EXAGGERATION..=heart_motion::MAX_EXAGGERATION,
                )
                .max_decimals(2),
                heartsim_ui::controls::TIMES,
            );
        });
        let r = *m.readout();
        let dash = || "\u{2014}".to_string();
        let model = t!("heart-motion-model");
        heartsim_ui::cards::card(ui, |ui| {
            heartsim_ui::cards::heading(ui, t!("heart-motion-card"));
            egui::Grid::new("heart_motion_readout")
                .num_columns(3)
                .show(ui, |ui| {
                    let rows = [
                        (
                            t!("heart-motion-lvef"),
                            r.lvef.map_or_else(dash, |v| format!("{:.0} %", v * 100.0)),
                        ),
                        (
                            t!("heart-motion-edv"),
                            r.lvef.map_or_else(dash, |_| format!("{:.0} mL", r.lv_edv_ml)),
                        ),
                        (
                            t!("heart-motion-esv"),
                            r.lvef.map_or_else(dash, |_| format!("{:.0} mL", r.lv_esv_ml)),
                        ),
                        (
                            t!("heart-motion-mapse"),
                            r.mapse_mm.map_or_else(dash, |v| format!("{v:.1} mm")),
                        ),
                        (
                            t!("heart-motion-tapse"),
                            r.tapse_mm.map_or_else(dash, |v| format!("{v:.1} mm")),
                        ),
                    ];
                    for (label, value) in rows {
                        ui.label(label);
                        ui.label(value);
                        heartsim_ui::helper_text(ui, model.clone());
                        ui.end_row();
                    }
                });
            heartsim_ui::helper_text(ui, t!("heart-motion-note"));
        });
    }

    /// Move the camera on the next frame (tutorial pages, the screenshot tour).
    pub fn command_camera(&mut self, command: CameraCommand) {
        self.camera_command = Some(command);
    }

    /// The 3-D viewport with picking, hover read-out and labels. Uploads this frame's scalars
    /// and display state into the renderer first.
    pub fn viewport(&mut self, ui: &mut egui::Ui, timing: &mut TimingEngine) {
        let Some(rs) = self.render_state.clone() else {
            return;
        };
        if self.tier1 {
            self.update_scalars();
        }
        // wall motion: at most one evaluation per drawn frame, capped in rate, none when nothing
        // changed (see `crate::heart_motion`); the simulation never waits for it
        let frame_nr = ui.ctx().cumulative_frame_nr();
        self.view_frame = Some(frame_nr);
        let card = self.card_frame.is_some_and(|f| f + 1 >= frame_nr);
        if let Some(wait_s) = self.tick_motion(ui.input(|i| i.time), true, card) {
            ui.ctx().request_repaint_after_secs(wait_s as f32);
        }
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let size = ui.available_size();
        let response = Viewport3d::ui(ui, size);
        let rect = response.rect;
        let mut renderer = rs.renderer.write();
        let Some(res) = renderer.callback_resources.get_mut::<ViewportResources>() else {
            return;
        };
        let scene = &mut res.scene;
        let camera_basis = scene.camera.basis();
        self.upload_motion(scene, &rs.queue);
        self.sync_scene(scene, &rs.queue, dt, false);
        if let Some(id) = self.lead_set {
            if self.overlay_glyphs.is_empty() {
                scene.set_spheres(&rs.device, &rs.queue, id, &self.lead_glyphs);
            } else {
                let all: Vec<GlyphInstance> = self
                    .lead_glyphs
                    .iter()
                    .chain(&self.overlay_glyphs)
                    .copied()
                    .collect();
                scene.set_spheres(&rs.device, &rs.queue, id, &all);
            }
        }

        if let (Some(id), Some((spheres, tubes))) = (self.catheter_set, self.catheter_glyphs.take()) {
            scene.set_spheres(&rs.device, &rs.queue, id, &spheres);
            scene.set_tubes(&rs.device, &rs.queue, id, &tubes);
        }

        // Picking (stimulate, ablate, the hover read-out, the EP lab's catheter placement) ray-casts
        // the REST mesh, not the moving one: the target of a click is the anatomical site, and the
        // hoods that map a vertex to its cells index the rest mesh. The drawn wall is at most about
        // a centimetre from there (the apex stays, the base moves), so a click lands on the cell
        // under the resting surface point. The lead, catheter and territory markers are glyphs at
        // the cells' rest positions and do not move either; the valve discs and their highlights
        // are layers and do move with the annuli.
        let uv = |p: egui::Pos2| {
            (
                (p.x - rect.left()) / rect.width(),
                (p.y - rect.top()) / rect.height(),
            )
        };
        self.hover = response
            .hover_pos()
            .filter(|_| !response.dragged())
            .and_then(|p| scene.pick(uv(p)))
            .and_then(|hit| self.engine.cloud().nearest(hit.position))
            .map(|c| self.cell_info(c));
        if response.clicked()
            && let Some(hit) = response.interact_pointer_pos().and_then(|p| scene.pick(uv(p)))
        {
            if self.pick == Pick::Off {
                let ablate = self.tool == ClickTool::Ablate || ui.input(|i| i.modifiers.shift);
                self.click(hit.position, ablate, timing);
            } else {
                self.pick = Pick::Hit(hit.position);
            }
        }
        if self.panel.labels {
            let painter = ui.painter_at(rect);
            for l in scene.label_anchors() {
                painter.text(
                    rect.min + egui::vec2(l.u * rect.width(), l.v * rect.height()),
                    egui::Align2::CENTER_CENTER,
                    &l.text,
                    egui::FontId::proportional(13.0),
                    egui::Color32::WHITE,
                );
            }
        }
        drop(renderer);
        heartsim_ui::orientation_gizmo(ui, rect, camera_basis, heartsim_ui::GizmoCorner::TopRight);
        if let Some(info) = self.hover.clone() {
            response.on_hover_ui_at_pointer(|ui| {
                ui.strong(heartsim_ui::labels::region_name(info.region));
                if info.territory != Territory::None {
                    ui.label(t!("heart-territory", territory = info.territory.label()));
                }
                ui.label(format!("V_m {:.0} mV", info.vm_mv));
                ui.label(match info.activation_ms {
                    Some(t) => t!("heart-activated", time = format!("{t:.0}")),
                    None => t!("heart-not-activated"),
                });
                ui.label(t!("heart-apd", value = format!("{:.0}", info.apd_ms)));
                ui.label(format!("CV {:.2} m/s", info.cv_m_s));
            });
        }
    }

    /// Add the pack's meshes to `scene` and keep the render state for picking.
    pub fn install(&mut self, rs: &egui_wgpu::RenderState, scene: &mut SceneRenderer) {
        for (l, toggle) in self.layers.iter_mut().zip(&self.panel.layers) {
            if !l.valves.is_empty() {
                // one scene layer per valve disc, labelled once, so each can flash on its own
                for (i, (_, mesh, slot)) in l.valves.iter_mut().enumerate() {
                    let mut desc = LayerDesc::new(pretty_name(&l.name), mesh);
                    desc.base_color = base_colour(&l.name, l.kind);
                    desc.alpha = toggle.alpha;
                    let id = scene.add_layer(&rs.device, &desc);
                    if let Some(layer) = scene.layer_mut(id) {
                        layer.label = (i == 0).then(|| pretty_name(&l.name));
                        layer.pickable = true;
                        layer.clip = true;
                        layer.visible = toggle.visible;
                    }
                    *slot = Some(id);
                }
                continue;
            }
            let valve_mesh = (l.name == TRICUSPID_VALVE_LAYER)
                .then(|| self.tricuspid.as_ref().map(|v| &v.render.mesh))
                .flatten();
            let mesh = valve_mesh.unwrap_or_else(|| {
                self.pack_meshes
                    .iter()
                    .find(|m| m.name == l.name)
                    .map_or(&self.torso, |m| &m.mesh)
            });
            let mut desc = LayerDesc::new(pretty_name(&l.name), mesh);
            desc.base_color = base_colour(&l.name, l.kind);
            desc.alpha = toggle.alpha;
            let id = scene.add_layer(&rs.device, &desc);
            if let Some(layer) = scene.layer_mut(id) {
                layer.label = Some(pretty_name(&l.name));
                // the valve is not myocardium: a click goes through to the heart behind it
                layer.pickable = l.kind.is_some() && valve_mesh.is_none();
                layer.clip = true;
                layer.visible = toggle.visible;
            }
            l.id = Some(id);
            if valve_mesh.is_some()
                && let Some(v) = &mut self.tricuspid
            {
                v.id = Some(id);
                v.key = None;
            }
        }
        self.lead_set = Some(scene.add_glyph_set("Pacing leads"));
        self.catheter_set = Some(scene.add_glyph_set("EP catheters"));
        scene.frame_scene();
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for m in &self.pack_meshes {
            let (a, b) = m.mesh.bounds();
            lo = lo.min(a);
            hi = hi.max(b);
        }
        self.radius_mm = ((hi - lo).length() * 0.5).max(1.0);
        self.render_state = Some(rs.clone());
    }

    /// Copy display state into the renderer and upload this frame's scalars.
    fn sync_scene(&mut self, scene: &mut SceneRenderer, queue: &wgpu::Queue, dt: f32, own_camera: bool) {
        if self.luts_dirty {
            for id in [LutId::Voltage, LutId::Isochrone, LutId::Apd] {
                scene.set_lut(queue, id, &id.table(self.lut_style));
            }
            self.luts_dirty = false;
        }
        let (lo, hi) = self.window(self.mode);
        // the measured tricuspid valve, when shown, takes the place of its flat disc (and its
        // highlight)
        let leaflets_shown =
            self.tricuspid.is_some() && self.panel.layer(TRICUSPID_VALVE_LAYER).is_some_and(|l| l.visible);
        for (l, toggle) in self.layers.iter().zip(&self.panel.layers) {
            for (valve, _, id) in &l.valves {
                let Some(layer) = (*id).and_then(|id| scene.layer_mut(id)) else {
                    continue;
                };
                layer.visible =
                    toggle.visible && !(leaflets_shown && *valve == heartsim_ecg::Valve::Tricuspid);
                layer.alpha = toggle.alpha;
                layer.scalar_mode = ScalarMode::None;
                layer.base_color = valve_colour(base_colour(&l.name, l.kind), self.valve_glow[valve.index()]);
            }
            let Some(id) = l.id else { continue };
            let mapped = self.tier1 && l.hoods.is_some();
            if mapped {
                scene.set_layer_scalars(queue, id, &l.scalars);
            }
            if let Some(layer) = scene.layer_mut(id) {
                layer.visible = toggle.visible;
                layer.alpha = toggle.alpha;
                layer.scalar_mode = if mapped {
                    ScalarMode::Lut(self.mode.lut())
                } else {
                    ScalarMode::None
                };
                layer.scalar_min = lo;
                layer.scalar_max = hi;
                if l.name == TRICUSPID_VALVE_LAYER {
                    layer.base_color = valve_colour(
                        base_colour(&l.name, l.kind),
                        self.valve_glow[heartsim_ecg::Valve::Tricuspid.index()],
                    );
                }
                layer.contour =
                    (mapped && self.wavefront && self.mode == ColourMode::Voltage).then_some(Contour {
                        level: WAVEFRONT_MV,
                        ..Contour::default()
                    });
            }
        }
        if own_camera {
            // the page that called this keeps its own camera and clip plane
            return;
        }
        match self.camera_command.take() {
            Some(CameraCommand::Preset(p)) => scene.camera.apply_preset(p),
            Some(CameraCommand::Oblique(deg)) => {
                scene.camera.yaw = oblique_yaw_deg(deg).to_radians();
                scene.camera.pitch = 0.0;
            }
            Some(CameraCommand::Frame) => scene.frame_scene(),
            None => {}
        }
        if self.panel.auto_rotate && !self.panel.reduced_motion {
            scene.camera.yaw += AUTO_ROTATE_RAD_S * dt;
        }
        let back = (scene.camera.eye() - scene.camera.target).normalize_or_zero();
        scene.clip = ClipPlane {
            enabled: self.panel.clip_enabled,
            point: scene.camera.target - back * self.panel.clip_depth * self.radius_mm,
            normal: back,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_geometry::K_NEIGHBOURS;
    use std::path::Path;
    use std::sync::OnceLock;

    fn pack() -> AnatomyPack {
        static PACK: OnceLock<AnatomyPack> = OnceLock::new();
        PACK.get_or_init(|| {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
            AnatomyPack::load(&dir).expect("default pack loads")
        })
        .clone()
    }

    /// B9 acceptance: hood mapping of a known V_m field reproduces vertex values within 1e-5
    /// for a delta-function cell.
    #[test]
    fn hood_mapping_reproduces_delta_cell() {
        let pack = pack();
        let hoods = &pack.hoods["lv"];
        // a vertex that coincides with a cell has all its weight on it; any vertex will do for
        // the general case
        let exact = hoods
            .iter()
            .position(|h| (h.weights[0] - 1.0).abs() < 1e-6)
            .unwrap_or(0);
        let cell = hoods[exact].cells[0];
        // the engine's normalised V_m (0 rest, 1 peak), which is what the hoods interpolate
        let mut vm = vec![0.0; pack.cells.len()];
        vm[cell as usize] = 1.0;
        let mut out = vec![0.0; hoods.len()];
        map_hoods(hoods, &vm, &mut out);
        for (h, s) in hoods.iter().zip(&out) {
            let w: f32 = (0..K_NEIGHBOURS)
                .filter(|&k| h.cells[k] == cell)
                .map(|k| h.weights[k])
                .sum();
            assert!((s - w).abs() < 1e-5, "vertex {s} vs {w}");
        }
        let w0: f32 = (0..K_NEIGHBOURS)
            .filter(|&k| hoods[exact].cells[k] == cell)
            .map(|k| hoods[exact].weights[k])
            .sum();
        assert!(
            (out[exact] - w0).abs() < 1e-5 && w0 > 0.0,
            "the vertex nearest the cell carries it"
        );
    }

    /// A sinus beat driven by timing events colours the atria before the ventricles, and the
    /// voltage scalars are the hood-weighted engine V_m.
    #[test]
    fn sinus_beat_reaches_atria_then_ventricles() {
        let params = TimingParams::default();
        let mut timing = TimingEngine::new(params.clone());
        let mut heart = HeartView::from_pack(pack(), &params);
        let dt = Millis(1.0);
        let mean = |h: &HeartView, name: &str| {
            let s = h.layer_scalars(name);
            s.iter().sum::<f32>() / s.len() as f32
        };
        let (mut atria_first, mut ventricles_first) = (None, None);
        for step in 0..600 {
            timing.step(dt);
            let events = timing.drain_events();
            heart.on_step(&events, dt, timing.now());
            heart.update_scalars();
            let t = f64::from(step);
            if atria_first.is_none() && mean(&heart, "ra") > REST_MV + 20.0 {
                atria_first = Some(t);
            }
            if ventricles_first.is_none() && mean(&heart, "lv") > REST_MV + 20.0 {
                ventricles_first = Some(t);
            }
        }
        let (a, v) = (
            atria_first.expect("atria activate"),
            ventricles_first.expect("ventricles activate"),
        );
        assert!(
            a + 80.0 < v,
            "atria at {a} ms, ventricles at {v} ms: the PR interval separates them"
        );
        // determinism of the mapping: identical engine state → identical scalars
        let before = heart.layer_scalars("lv").to_vec();
        heart.update_scalars();
        assert_eq!(before, heart.layer_scalars("lv"));
    }

    #[test]
    fn click_on_rv_apex_starts_a_ventricular_beat_in_both_engines() {
        let params = TimingParams::default();
        let mut timing = TimingEngine::new(params.clone());
        let p = pack();
        let apex = p.cells.positions[p.landmarks.points["rv_apex"] as usize];
        let mut heart = HeartView::from_pack(p, &params);
        let dt = Millis(1.0);
        for _ in 0..50 {
            timing.step(dt);
            let ev = timing.drain_events();
            heart.on_step(&ev, dt, timing.now());
        }
        heart.click(apex, false, &mut timing);
        let mut ventricular_stimulus = false;
        for _ in 0..40 {
            timing.step(dt);
            let ev = timing.drain_events();
            ventricular_stimulus |= ev
                .iter()
                .any(|e| e.kind == EventKind::Stimulus && e.region.is_ventricular());
            heart.on_step(&ev, dt, timing.now());
        }
        assert!(ventricular_stimulus, "the strip's timing engine saw the PVC");
        let cell = heart.engine().cloud().nearest(apex).unwrap();
        let info = heart.cell_info(cell);
        assert_eq!(info.region, Region::RightVentricle);
        let t = info.activation_ms.expect("apex activated by the click");
        assert!(
            t <= 2.0,
            "the clicked cell fires first ({t} ms after the stimulus)"
        );
    }

    /// B27 part B: the default pack's valve planes split into the four valves, each where the
    /// anatomy puts it.
    #[test]
    fn the_valve_planes_split_into_four_valves_in_their_places() {
        use heartsim_ecg::Valve;
        let heart = HeartView::from_pack(pack(), &TimingParams::default());
        let parts = heart.valve_parts();
        assert_eq!(parts.len(), 4, "{parts:?}");
        let at = |v: Valve| parts.iter().find(|(x, _)| *x == v).map(|(_, c)| *c).unwrap();
        let (mi, tr, ao, pu) = (
            at(Valve::Mitral),
            at(Valve::Tricuspid),
            at(Valve::Aortic),
            at(Valve::Pulmonic),
        );
        assert!(tr.x < mi.x && tr.x < ao.x, "tricuspid on the right");
        assert!(mi.z < ao.z && mi.z < tr.z, "mitral posterior");
        assert!(pu.y > ao.y && pu.y > mi.y, "pulmonic highest");
        assert!(pu.z > ao.z, "pulmonic anterior of the aortic");
        // colour: base at rest, the highlight at full level
        let base = base_colour("valve_planes", Some(MeshKind::Valve));
        assert_eq!(valve_colour(base, 0.0), base);
        let full = valve_colour(base, 1.0);
        assert!(
            full.iter()
                .zip(VALVE_HIGHLIGHT)
                .all(|(a, b)| (a - b).abs() < 1e-6),
            "{full:?}"
        );
    }

    #[test]
    fn isochrone_and_apd_modes_use_their_own_windows() {
        let params = TimingParams::default();
        let mut heart = HeartView::from_pack(pack(), &params);
        heart.mode = ColourMode::Isochrone;
        heart.update_scalars();
        let late = heart.window(ColourMode::Isochrone).1;
        assert!(
            heart.layer_scalars("lv").iter().all(|s| (s - late).abs() < 1e-3),
            "before any beat every cell shows the 'not yet' colour"
        );
        assert_ne!(heart.window(ColourMode::Apd), heart.window(ColourMode::Voltage));
    }
}
