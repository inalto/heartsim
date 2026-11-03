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

use anyhow::Context as _;
use eframe::egui;
use glam::Vec3;
use heartsim_core::{Action, Millis, Parameters, Region, StimSite};
use heartsim_geometry::{AnatomyPack, Hood, MeshKind, Territory};
use heartsim_render::{
    ClipPlane, Contour, GlyphInstance, GlyphSetId, LayerDesc, LayerId, LutId, LutStyle, ScalarMode,
    SceneRenderer, map_hoods, oblique_yaw_deg,
};
use heartsim_sim::timing::PacerConfig;
use heartsim_sim::{EikonalEngine, EikonalParams, Engine, Event, EventKind, TimingEngine, TimingParams};
use heartsim_ui::{
    AnatomyPanelState, CameraCommand, Viewport3d, ViewportResources, anatomy_panel, colour_bar,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Resting and peak transmembrane potential used to show the engine's normalised V_m in mV
/// (ventricular myocyte: rest ≈ −85 mV, overshoot ≈ +25 mV; Katz, *Physiology of the Heart*).
const REST_MV: f32 = -85.0;
const PEAK_MV: f32 = 25.0;
/// Activation threshold drawn as the wavefront outline, mV.
const WAVEFRONT_MV: f32 = -30.0;
/// Auto-rotate speed, radians per second of wall-clock time (one turn in ~20 s).
const AUTO_ROTATE_RAD_S: f32 = 0.3;

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
    pub fn label(self) -> &'static str {
        match self {
            ColourMode::Voltage => "Voltage",
            ColourMode::Isochrone => "Activation time",
            ColourMode::Apd => "APD",
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
    /// Per-vertex scalars of the current colour mode.
    scalars: Vec<f32>,
    id: Option<LayerId>,
}

/// Anatomy, engine and display state of the 3-D heart.
pub struct HeartView {
    engine: EikonalEngine,
    params: Parameters,
    pack_meshes: Vec<heartsim_geometry::NamedMesh>,
    torso: heartsim_geometry::TriMesh,
    aha_segments: Vec<u8>,
    layers: Vec<HeartLayer>,
    per_cell: Vec<f32>,
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
    /// Per-cell values to show instead of the live engine state (scrubbing a paused strip).
    scrub_cells: Option<Vec<f32>>,
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

/// Where the default anatomy pack is looked for, in order: `$HEARTSIM_ASSETS`, next to the
/// executable (installed layout), macOS bundle resources, the working directory, the source tree.
#[must_use]
pub fn default_pack_dir() -> Option<PathBuf> {
    let rel = Path::new("anatomy").join("default");
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("HEARTSIM_ASSETS") {
        roots.push(dir.into());
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        roots.push(exe_dir.join("assets"));
        roots.push(exe_dir.join("../Resources/assets"));
    }
    roots.push(PathBuf::from("assets"));
    roots.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
    roots
        .into_iter()
        .map(|r| r.join(&rel))
        .find(|d| d.join("manifest.ron").is_file())
}

/// Display name of a pack mesh, for the layer list and the labels.
fn pretty_name(name: &str) -> &str {
    match name {
        "lv" => "Left ventricle",
        "rv" => "Right ventricle",
        "la" => "Left atrium",
        "ra" => "Right atrium",
        "aorta" => "Aorta",
        "pulmonary_artery" => "Pulmonary artery",
        "pulmonary_veins" => "Pulmonary veins",
        "svc" => "Superior vena cava",
        "ivc" => "Inferior vena cava",
        "valve_rings" => "Valve rings",
        "torso" => "Torso",
        other => other,
    }
}

/// Flat colour (linear RGB) of a mesh: oxygenated vessels red, deoxygenated blue (textbook).
fn base_colour(name: &str, kind: Option<MeshKind>) -> [f32; 3] {
    match (name, kind) {
        ("aorta" | "pulmonary_veins", _) => [0.55, 0.05, 0.05],
        ("pulmonary_artery" | "svc" | "ivc", _) => [0.12, 0.16, 0.5],
        (_, Some(MeshKind::Valve)) => [0.8, 0.75, 0.55],
        (_, None) => [0.75, 0.6, 0.5],
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
    pub fn load_default(timing: &TimingParams) -> anyhow::Result<Self> {
        let dir = default_pack_dir().context("anatomy pack `assets/anatomy/default` not found")?;
        let pack = AnatomyPack::load(&dir).with_context(|| format!("loading {}", dir.display()))?;
        Ok(Self::from_pack(pack, timing))
    }

    /// Build the eikonal engine on `pack` (inheriting His–Purkinje delays and accessory-pathway
    /// sides from `timing`, as `Tier1Engine::new` does) and the display layers.
    #[must_use]
    pub fn from_pack(pack: AnatomyPack, timing: &TimingParams) -> Self {
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
                HeartLayer {
                    name: m.name.clone(),
                    kind: Some(m.kind),
                    scalars: vec![0.0; if h.is_some() { m.mesh.vertex_count() } else { 0 }],
                    hoods: h,
                    id: None,
                }
            })
            .collect();
        layers.push(HeartLayer {
            name: "torso".to_string(),
            kind: None,
            hoods: None,
            scalars: Vec::new(),
            id: None,
        });
        let mut panel = AnatomyPanelState::new(layers.iter().map(|l| (l.name.clone(), 1.0)));
        if let Some(t) = panel.layers.last_mut() {
            t.visible = false;
            t.alpha = 0.15;
        }
        let n = engine.cloud().len();
        Self {
            engine,
            params,
            pack_meshes: pack.meshes,
            torso: pack.torso,
            aha_segments: pack.aha_segments,
            layers,
            per_cell: vec![0.0; n],
            beat_start_ms: 0.0,
            now_ms: 0.0,
            tier1: true,
            mode: ColourMode::Voltage,
            windows: ColourMode::ALL.map(ColourMode::default_window),
            lut_style: LutStyle::Classic,
            luts_dirty: true,
            wavefront: true,
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
            scrub_cells: None,
        }
    }

    /// The Tier-1 engine (read-only).
    #[must_use]
    pub fn engine(&self) -> &EikonalEngine {
        &self.engine
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
        if self.tier1 {
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

    /// Per-cell value of the current colour mode, in its display unit.
    fn fill_per_cell(&mut self) {
        let cells = self.engine.cells();
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
        let cells = self.engine.cells();
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
        let result = if ablate {
            let action = Action::Ablate {
                site,
                radius_mm: f64::from(self.ablate_radius_mm),
            };
            self.engine.apply(&action, now).map(|()| {
                format!(
                    "Ablated {:.0} mm around {}",
                    self.ablate_radius_mm,
                    region.label()
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
            r.map(|()| format!("Stimulus: {} at {:.0} ms", region.label(), now.0))
        };
        self.status = Some(result.unwrap_or_else(|e| e.to_string()));
    }

    /// Restore all ablated tissue (restarts the 3-D engine at the current time).
    pub fn clear_ablation(&mut self) {
        self.engine.reset();
        self.engine.step(Millis(self.now_ms));
        self.status = Some("Ablation lesions cleared".to_string());
    }

    // ---- UI ------------------------------------------------------------------------------

    /// Colour-map, tool and anatomy controls (left panel of the Simulator tab).
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        let mut tier1 = self.tier1;
        if ui
            .checkbox(&mut tier1, "3-D activation (Tier 1)")
            .on_hover_text("Off: ECG strip only (Tier 0); the heart is drawn without a colour map.")
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
                if ui.button("Reset window").clicked() {
                    *w = self.mode.default_window();
                }
                if ui
                    .checkbox(
                        &mut (self.lut_style == LutStyle::ColourBlindSafe),
                        "Colour-blind safe",
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
                egui::Checkbox::new(&mut self.wavefront, "Wavefront outline"),
            );
            ui.separator();
            ui.strong("Click on the heart");
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.tool, ClickTool::Stimulate, "Stimulate");
                ui.radio_value(&mut self.tool, ClickTool::Ablate, "Ablate");
            });
            ui.add(egui::Slider::new(&mut self.ablate_radius_mm, 2.0..=15.0).text("lesion radius mm"));
            ui.horizontal(|ui| {
                ui.label("Shift-click ablates.");
                if ui.button("Clear ablation").clicked() {
                    self.clear_ablation();
                }
            });
            if let Some(s) = &self.status {
                ui.label(s);
            }
        });
        ui.separator();
        let r = ui.scope(|ui| anatomy_panel(ui, &mut self.panel));
        heartsim_ui::highlight::mark(&r.response, "camera_views");
        if let Some(c) = r.inner {
            self.camera_command = Some(c);
        }
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
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let size = ui.available_size();
        let response = Viewport3d::ui(ui, size);
        let rect = response.rect;
        let mut renderer = rs.renderer.write();
        let Some(res) = renderer.callback_resources.get_mut::<ViewportResources>() else {
            return;
        };
        let scene = &mut res.scene;
        self.sync_scene(scene, &rs.queue, dt);
        if let Some(id) = self.lead_set {
            scene.set_spheres(&rs.device, &rs.queue, id, &self.lead_glyphs);
        }

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
            let ablate = self.tool == ClickTool::Ablate || ui.input(|i| i.modifiers.shift);
            self.click(hit.position, ablate, timing);
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
        if let Some(info) = self.hover.clone() {
            response.on_hover_ui_at_pointer(|ui| {
                ui.strong(info.region.label());
                if info.territory != Territory::None {
                    ui.label(format!("Territory: {}", info.territory.label()));
                }
                ui.label(format!("V_m {:.0} mV", info.vm_mv));
                ui.label(match info.activation_ms {
                    Some(t) => format!("Activated at {t:.0} ms this beat"),
                    None => "Not yet activated this beat".to_string(),
                });
                ui.label(format!("APD {:.0} ms", info.apd_ms));
                ui.label(format!("CV {:.2} m/s", info.cv_m_s));
            });
        }
    }

    /// Add the pack's meshes to `scene` and keep the render state for picking.
    pub fn install(&mut self, rs: &egui_wgpu::RenderState, scene: &mut SceneRenderer) {
        for (l, toggle) in self.layers.iter_mut().zip(&self.panel.layers) {
            let mesh = self
                .pack_meshes
                .iter()
                .find(|m| m.name == l.name)
                .map_or(&self.torso, |m| &m.mesh);
            let mut desc = LayerDesc::new(pretty_name(&l.name), mesh);
            desc.base_color = base_colour(&l.name, l.kind);
            desc.alpha = toggle.alpha;
            let id = scene.add_layer(&rs.device, &desc);
            if let Some(layer) = scene.layer_mut(id) {
                layer.label = Some(pretty_name(&l.name).to_string());
                layer.pickable = l.kind.is_some();
                layer.clip = true;
                layer.visible = toggle.visible;
            }
            l.id = Some(id);
        }
        self.lead_set = Some(scene.add_glyph_set("Pacing leads"));
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
    fn sync_scene(&mut self, scene: &mut SceneRenderer, queue: &wgpu::Queue, dt: f32) {
        if self.luts_dirty {
            for id in [LutId::Voltage, LutId::Isochrone, LutId::Apd] {
                scene.set_lut(queue, id, &id.table(self.lut_style));
            }
            self.luts_dirty = false;
        }
        let (lo, hi) = self.window(self.mode);
        for (l, toggle) in self.layers.iter().zip(&self.panel.layers) {
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
                layer.contour =
                    (mapped && self.wavefront && self.mode == ColourMode::Voltage).then_some(Contour {
                        level: WAVEFRONT_MV,
                        ..Contour::default()
                    });
            }
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
        if self.panel.auto_rotate {
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
