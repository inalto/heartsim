//! The Electrodes tab (brief B10): the torso with its electrodes in the 3-D viewport, dragged with
//! the mouse (ray pick on the torso surface, snapping to intercostal spaces), the editor panel
//! (presets, extra leads, cable swaps, misplacement exercises with the detector's explanation)
//! and the twelve leads plus the extra leads next to it.
//!
//! The tab shares the scene with the Simulator tab. While it is shown it uses its own camera,
//! makes the torso translucent and shows the electrode glyphs; [`ElectrodeTab::deactivate`] puts
//! the other tabs' camera back and hides the glyphs (the heart view re-applies its own layer
//! settings every frame).

use eframe::egui;
use glam::Vec3;
use heartsim_core::{Electrode, LEAD_COUNT, Lead};
use heartsim_ecg::{Cable, ExtraLead, GaussianSynth, LeadRing, Limb, TorsoSurface};
use heartsim_geometry::pack::{BODY_ANKLES_MM, BODY_WRISTS_MM, BodyRegion, body_region};
use heartsim_geometry::{ElectrodeSet, TriMesh};
use heartsim_render::{GlyphInstance, GlyphSetId, OrbitCamera, SceneRenderer, srgb_color};
use heartsim_ui::t;
use heartsim_ui::{
    ElectrodeEditor, StripStyle, Viewport3d, ViewportResources, ecg_strip, electrode_editor_panel,
    lead_column,
};

/// Electrode glyph radius, mm.
const MARKER_RADIUS_MM: f32 = 7.0;
/// Electrodes are drawn at least this large on screen (radius, logical px): on the whole figure a
/// 7 mm sphere is two or three pixels.
const MARKER_MIN_PX: f32 = 4.5;
/// A press this close to an electrode on screen grabs it, logical px.
const GRAB_PX: f32 = 14.0;
/// The 3-D view keeps at least this width between the side panels, logical px.
const VIEW_MIN_WIDTH_PX: f32 = 300.0;
/// Free space around the framed figure or chest, as a fraction of its size.
const FRAME_MARGIN: f32 = 0.06;

/// What the 3-D view frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    /// Head to foot: the limb electrodes are in reach.
    WholeBody,
    /// The trunk only: the chest electrodes are far enough apart to place precisely.
    Chest,
}

/// The camera as the tab last framed it, to tell whether the user has moved it since.
#[derive(Debug, Clone, Copy)]
struct Fit {
    view: View,
    aspect: f32,
    camera: OrbitCamera,
}

/// Whether two cameras look from the same place (the user has not orbited or zoomed).
fn same_view(a: &OrbitCamera, b: &OrbitCamera) -> bool {
    a.yaw.to_bits() == b.yaw.to_bits()
        && a.pitch.to_bits() == b.pitch.to_bits()
        && a.distance.to_bits() == b.distance.to_bits()
}

/// Where each label goes so that none covers another: above its marker, or failing that the
/// first free slot further up, then below. `markers` are screen positions with the label's
/// size; returns the label rectangles in the same order (crowded chest electrodes become a
/// staircase joined to their markers by leader lines).
fn place_labels(markers: &[(egui::Pos2, egui::Vec2)]) -> Vec<egui::Rect> {
    const FIRST_PX: f32 = 12.0;
    let mut order: Vec<usize> = (0..markers.len()).collect();
    order.sort_by(|a, b| markers[*a].0.x.total_cmp(&markers[*b].0.x));
    let mut placed: Vec<(usize, egui::Rect)> = Vec::with_capacity(markers.len());
    for i in order {
        let (at, size) = markers[i];
        let step = size.y + 2.0;
        let slot = |k: i32| {
            // k = 0, 1, 2… above the marker; negative below it
            let dy = if k >= 0 {
                -(FIRST_PX + step * k as f32) - size.y * 0.5
            } else {
                FIRST_PX + step * (-k - 1) as f32 + size.y * 0.5
            };
            egui::Rect::from_center_size(at + egui::vec2(0.0, dy), size)
        };
        let rect = (0..12)
            .chain((1..12).map(|k| -k))
            .map(slot)
            .find(|r| placed.iter().all(|(_, q)| !q.expand(1.0).intersects(*r)))
            .unwrap_or_else(|| slot(0));
        placed.push((i, rect));
    }
    placed.sort_by_key(|(i, _)| *i);
    placed.into_iter().map(|(_, r)| r).collect()
}
/// Torso opacity in this tab.
const TORSO_ALPHA: f32 = 0.35;
/// Torso tint in this tab, sRGB: the translucent light blue of `torso-front-axis.png`.
const TORSO_TINT: [f32; 3] = [0.45, 0.74, 0.96];
/// Distal leg electrodes are drawn just below the torso (the ankles are out of view), mm.
const LEG_MARKER_Y_MM: f32 = -420.0;
/// Where "Save placement" writes and "Load placement" reads.
#[cfg(feature = "native")]
const SCENARIO_FILE: &str = "heartsim-electrodes.ron";

/// Cable colours of the AHA convention (IEC differs), sRGB.
fn cable_colour(c: Cable) -> [f32; 3] {
    match c {
        Cable::Standard(Electrode::RA) => [0.95, 0.95, 0.95],
        Cable::Standard(Electrode::LA) => [0.12, 0.12, 0.12],
        Cable::Standard(Electrode::LL | Electrode::V1) => [0.85, 0.12, 0.1],
        Cable::Standard(Electrode::RL | Electrode::V3) => [0.1, 0.6, 0.18],
        Cable::Standard(Electrode::V2) => [0.95, 0.85, 0.1],
        Cable::Standard(Electrode::V4) => [0.15, 0.3, 0.9],
        Cable::Standard(Electrode::V5) => [1.0, 0.55, 0.1],
        Cable::Standard(Electrode::V6) => [0.55, 0.2, 0.75],
        Cable::Extra(_) => [0.6, 0.6, 0.6],
    }
}

/// Where a distal-limb electrode (stored at a nominal [`Limb::anchor_mm`], `None` when `p` is a
/// trunk point) is drawn: where it was dropped (`spot`) or else on the human figure's wrist or
/// ankle when it is shown (`on_body`), else legs just below the torso.
fn limb_marker(p: Vec3, spot: Option<Vec3>, on_body: bool) -> Option<Vec3> {
    let limb = Limb::at(p)?;
    Some(match (on_body, spot, limb) {
        (true, Some(s), _) => s,
        (true, None, Limb::LeftArm) => BODY_WRISTS_MM[0],
        (true, None, Limb::RightArm) => BODY_WRISTS_MM[1],
        (true, None, Limb::LeftLeg) => BODY_ANKLES_MM[0],
        (true, None, Limb::RightLeg) => BODY_ANKLES_MM[1],
        (false, _, Limb::LeftLeg | Limb::RightLeg) => Vec3::new(p.x, LEG_MARKER_Y_MM, p.z),
        (false, _, _) => p,
    })
}

/// The limb a point of the human figure is on (anywhere on an arm or a leg), if any. A limb is
/// one conductor, so a drop on the hand, the upper arm or the thigh is a limb electrode.
fn limb_under(p: Vec3, on_body: bool) -> Option<Limb> {
    if !on_body {
        return None;
    }
    match body_region(p) {
        BodyRegion::Trunk => None,
        BodyRegion::LeftArm => Some(Limb::LeftArm),
        BodyRegion::RightArm => Some(Limb::RightArm),
        BodyRegion::LeftLeg => Some(Limb::LeftLeg),
        BodyRegion::RightLeg => Some(Limb::RightLeg),
    }
}

/// Skin points drawn this many entries are remembered at most.
const SKIN_CACHE_MAX: usize = 256;

/// The surface electrodes are dragged on: the human `body` when there is one, around the `torso`'s
/// axis so the landmark lines are where the standard sites were defined; else the torso. On the
/// figure, landmark lookups ([`TorsoSurface::at`]) skip the limbs, whose skin is fused with the
/// chest above the armpit; picking still hits them.
#[must_use]
pub fn electrode_surface(torso: Option<&TriMesh>, body: Option<&TriMesh>) -> Option<TorsoSurface> {
    match (torso, body) {
        (t, Some(b)) => {
            let surface = match t {
                Some(t) => TorsoSurface::with_axis(b, TorsoSurface::new(t).axis()),
                None => TorsoSurface::new(b),
            };
            Some(surface.excluding(|p| body_region(p) != BodyRegion::Trunk))
        }
        (Some(t), None) => Some(TorsoSurface::new(t)),
        (None, None) => None,
    }
}

/// State of the Electrodes tab.
#[derive(Debug)]
pub struct ElectrodeTab {
    pub editor: ElectrodeEditor,
    glyphs: Option<GlyphSetId>,
    /// This tab's camera while another tab is shown.
    camera: Option<OrbitCamera>,
    /// The other tabs' camera while this one is shown.
    saved_camera: Option<OrbitCamera>,
    dragging: Option<Cable>,
    /// Electrodes are on the human figure (limb electrodes drawn on it, trunk electrodes on its
    /// skin).
    on_body: bool,
    /// The limb under the cursor while an electrode is dragged.
    drag_limb: Option<Limb>,
    /// What the 3-D view frames.
    view: View,
    /// Bounds of the pack torso, which the chest view frames (`None` without one).
    chest: Option<(Vec3, Vec3)>,
    /// The last framing the tab chose itself.
    fit: Option<Fit>,
    /// Nearest skin point of recently drawn trunk positions: `(stored, drawn)`, compared bitwise.
    skin: Vec<(Vec3, Vec3)>,
    #[cfg(feature = "native")]
    file_status: Option<String>,
}

impl ElectrodeTab {
    /// Electrode sites and torso of the default anatomy pack (procedural sites and no torso when
    /// it is missing), for strips sampled at `sample_rate_hz` holding `capacity` samples.
    #[must_use]
    pub fn load_default(sample_rate_hz: f32, capacity: usize) -> Self {
        #[cfg(feature = "native")]
        {
            let dir = crate::heart_view::default_pack_dir();
            let sites = dir
                .as_ref()
                .and_then(|d| std::fs::read_to_string(d.join("electrodes.ron")).ok())
                .and_then(|t| ElectrodeSet::from_ron(&t).ok())
                .unwrap_or_else(heartsim_geometry::procedural::procedural_electrodes);
            let read = |file: &str| dir.as_ref().and_then(|d| std::fs::read(d.join(file)).ok());
            let torso: Option<TriMesh> =
                read("torso.glb").and_then(|b| heartsim_geometry::pack::read_glb(&b).ok());
            let body: Option<TriMesh> =
                read("human.glb").and_then(|b| heartsim_geometry::pack::read_human_body(&b).ok());
            Self::new(sites, torso.as_ref(), body.as_ref(), sample_rate_hz, capacity)
        }
        #[cfg(not(feature = "native"))]
        {
            Self::new(
                heartsim_geometry::procedural::procedural_electrodes(),
                None,
                None,
                sample_rate_hz,
                capacity,
            )
        }
    }

    /// The tab for electrode `sites` on `torso`, dragged on the human `body` when there is one
    /// (e.g. all taken from an anatomy pack already in memory). Without either the electrodes can
    /// only be edited from the panel.
    #[must_use]
    pub fn new(
        sites: ElectrodeSet,
        torso: Option<&TriMesh>,
        body: Option<&TriMesh>,
        sample_rate_hz: f32,
        capacity: usize,
    ) -> Self {
        let surface = electrode_surface(torso, body);
        if surface.is_none() {
            tracing::warn!("no torso mesh: electrodes can be edited from the panel only");
        }
        Self {
            editor: ElectrodeEditor::on_surface(sites, surface, sample_rate_hz, capacity)
                .with_exercise_surface(body.and(torso).map(TorsoSurface::new)),
            on_body: body.is_some(),
            drag_limb: None,
            view: View::WholeBody,
            chest: body.and(torso).map(TriMesh::bounds),
            fit: None,
            skin: Vec::new(),
            glyphs: None,
            camera: None,
            saved_camera: None,
            dragging: None,
            #[cfg(feature = "native")]
            file_status: None,
        }
    }

    /// Feed the editor this frame's new strip samples (after the simulation step).
    pub fn update(&mut self, rings: &[LeadRing], synth: &mut GaussianSynth) {
        self.editor.update(rings, synth);
    }

    /// Give the scene back to the other tabs (call every frame this tab is not shown).
    pub fn deactivate(&mut self, rs: Option<&egui_wgpu::RenderState>) {
        let Some(saved) = self.saved_camera.take() else {
            return;
        };
        let Some(rs) = rs else { return };
        let mut renderer = rs.renderer.write();
        if let Some(res) = renderer.callback_resources.get_mut::<ViewportResources>() {
            self.camera = Some(res.scene.camera);
            res.scene.camera = saved;
            if let Some(g) = self.glyphs.and_then(|id| res.scene.glyph_set_mut(id)) {
                g.visible = false;
            }
        }
    }

    /// Take over the scene for this tab: own camera, translucent torso, electrode glyphs. The
    /// camera frames the chosen view for a viewport of this `aspect` (width / height), and is
    /// framed again when the view or the viewport's shape changes, unless the user has moved it.
    fn activate(&mut self, scene: &mut SceneRenderer, aspect: f32) {
        if self.saved_camera.is_none() {
            self.saved_camera = Some(scene.camera);
            if let Some(c) = self.camera {
                scene.camera = c;
            }
        }
        let refit = match &self.fit {
            None => true,
            Some(f) => {
                f.view != self.view
                    || ((f.aspect - aspect).abs() > 0.02 && same_view(&f.camera, &scene.camera))
            }
        };
        if refit {
            let bounds = match (self.view, self.chest) {
                (View::Chest, Some(b)) => Some(b),
                _ => scene
                    .layers()
                    .iter()
                    .find(|l| l.name == "Torso")
                    .map(|l| l.mesh.bounds()),
            };
            if let Some((lo, hi)) = bounds {
                let mut c = scene.camera;
                c.frame_to_fit(lo, hi, aspect, FRAME_MARGIN);
                c.yaw = 0.0;
                c.pitch = 0.1;
                scene.camera = c;
            }
            self.fit = Some(Fit {
                view: self.view,
                aspect,
                camera: scene.camera,
            });
        }
        let id = *self
            .glyphs
            .get_or_insert_with(|| scene.add_glyph_set("electrodes"));
        if let Some(g) = scene.glyph_set_mut(id) {
            g.visible = true;
            g.clip = false;
        }
        for l in scene.layers_mut() {
            if l.name == "Torso" {
                l.visible = true;
                l.alpha = TORSO_ALPHA;
                let [r, g, b, _] = srgb_color([TORSO_TINT[0], TORSO_TINT[1], TORSO_TINT[2], 1.0]);
                l.base_color = [r, g, b];
            }
        }
        scene.clip.enabled = false;
    }

    /// The skin point nearest to the stored trunk position `p`: the standard sites RL, LL and
    /// V7–V9 are off the figure's skin (the stored positions and the ECG stay as they are; only
    /// the marker moves). Remembered per position, so a frame costs no triangle tests unless an
    /// electrode moved.
    fn skin_point(&mut self, p: Vec3) -> Vec3 {
        let bits = |v: Vec3| v.to_array().map(f32::to_bits);
        if let Some((_, q)) = self.skin.iter().find(|(k, _)| bits(*k) == bits(p)) {
            return *q;
        }
        let q = self.editor.surface().and_then(|s| s.nearest(p)).unwrap_or(p);
        if self.skin.len() >= SKIN_CACHE_MAX {
            self.skin.clear();
        }
        self.skin.push((p, q));
        q
    }

    /// Where the electrode of `c` stored at `p` is drawn.
    fn marker_at(&mut self, c: Cable, p: Vec3) -> Vec3 {
        let spot = self.editor.limb_spot(c);
        match limb_marker(p, spot, self.on_body) {
            Some(q) => q,
            None if self.on_body => self.skin_point(p),
            None => p,
        }
    }

    /// Electrodes shown in the viewport (none during an unrevealed exercise: the ECG is the clue).
    fn markers(&mut self) -> Vec<(Cable, Vec3)> {
        if !self.editor.revealed() {
            return Vec::new();
        }
        let placed: Vec<(Cable, Vec3)> = {
            let p = self.editor.placement();
            Electrode::ALL
                .into_iter()
                .map(|e| (Cable::Standard(e), p.position(e)))
                .chain(p.extras.iter().map(|(x, q)| (Cable::Extra(*x), *q)))
                .collect()
        };
        placed
            .into_iter()
            .map(|(c, p)| (c, self.marker_at(c, p)))
            .collect()
    }

    /// The whole tab.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rs: Option<&egui_wgpu::RenderState>,
        rings: &[LeadRing],
        style: &StripStyle,
    ) {
        // the side panels leave the 3-D view its minimum width, so the figure is never a sliver
        let side_cap = ((ui.ctx().content_rect().width() - VIEW_MIN_WIDTH_PX) * 0.5).max(120.0);
        egui::Panel::left("electrode_editor")
            .max_size(crate::responsive::max_side(ui, side_cap))
            .default_size(340.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    electrode_editor_panel(ui, &mut self.editor);
                    ui.separator();
                    #[cfg(feature = "native")]
                    self.file_buttons(ui);
                });
            });
        egui::Panel::right("electrode_leads")
            .max_size(crate::responsive::max_side(ui, side_cap))
            .default_size(420.0)
            .show(ui, |ui| self.leads(ui, rings, style));
        egui::CentralPanel::default().show(ui, |ui| self.viewport(ui, rs));
    }

    #[cfg(feature = "native")]
    fn file_buttons(&mut self, ui: &mut egui::Ui) {
        use heartsim_core::Scenario;
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(t!("el-save"))
                .on_hover_text(t!("el-save-hint", file = SCENARIO_FILE))
                .clicked()
            {
                let mut s = Scenario::sinus_normal();
                self.editor.save_scenario(&mut s);
                self.file_status = Some(match s.to_ron().map(|t| std::fs::write(SCENARIO_FILE, t)) {
                    Ok(Ok(())) => t!("el-saved", file = SCENARIO_FILE),
                    Ok(Err(e)) => t!("el-save-failed", error = e.to_string()),
                    Err(e) => t!("el-save-failed", error = e.to_string()),
                });
            }
            if ui.button(t!("el-load")).clicked() {
                self.file_status = Some(
                    match std::fs::read_to_string(SCENARIO_FILE)
                        .map_err(|e| e.to_string())
                        .and_then(|t| Scenario::from_ron(&t).map_err(|e| e.to_string()))
                    {
                        Ok(s) => {
                            self.editor.load_scenario(&s);
                            t!("el-loaded", file = SCENARIO_FILE)
                        }
                        Err(e) => t!("el-load-failed", error = e.as_str()),
                    },
                );
            }
        });
        if let Some(s) = &self.file_status {
            ui.weak(s);
        }
    }

    /// The twelve leads and, below them, the extra leads that are on.
    fn leads(&self, ui: &mut egui::Ui, rings: &[LeadRing], style: &StripStyle) {
        if rings.len() < LEAD_COUNT {
            return;
        }
        let extras: Vec<(ExtraLead, &LeadRing)> = self.editor.extra_rings().collect();
        let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
        let rows = (LEAD_COUNT + extras.len()) as f32;
        let row_h = rect.height() / rows;
        let top = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), row_h * LEAD_COUNT as f32));
        lead_column(ui, top, rings, style, &Lead::ALL);
        for (k, (x, ring)) in extras.iter().enumerate() {
            let min = top.left_bottom() + egui::vec2(0.0, k as f32 * row_h);
            let r = egui::Rect::from_min_size(min, egui::vec2(rect.width(), row_h)).shrink(1.0);
            ecg_strip(ui, r, ring, style, x.label());
        }
        ui.allocate_rect(rect, egui::Sense::hover());
    }

    /// The torso viewport with draggable electrodes.
    fn viewport(&mut self, ui: &mut egui::Ui, rs: Option<&egui_wgpu::RenderState>) {
        let Some(rs) = rs else {
            ui.label(t!("el-no-3d"));
            return;
        };
        let size = ui.available_size();
        let rect = egui::Rect::from_min_size(ui.cursor().min, size);
        let aspect = rect.width() / rect.height().max(1.0);
        let markers = self.markers();
        let (pressed, down, origin, pos) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.press_origin(),
                i.pointer.latest_pos(),
            )
        });
        let mut screen = Vec::with_capacity(markers.len());
        let camera_basis;
        {
            let mut renderer = rs.renderer.write();
            let Some(res) = renderer.callback_resources.get_mut::<ViewportResources>() else {
                return;
            };
            self.activate(&mut res.scene, aspect);
            camera_basis = res.scene.camera.basis();
            for (c, p) in &markers {
                if let Some((u, v, _)) = res.scene.camera.project(*p, aspect) {
                    screen.push((*c, rect.min + egui::vec2(u * rect.width(), v * rect.height())));
                }
            }
        }
        if pressed && let Some(o) = origin.filter(|o| rect.contains(*o)) {
            self.dragging = screen
                .iter()
                .map(|(c, s)| (*c, s.distance(o)))
                .filter(|(_, d)| *d <= GRAB_PX)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(c, _)| c);
            if self.dragging.is_some() {
                self.editor.selected = self.dragging;
            }
        }
        if !down {
            self.dragging = None;
        }
        let response = Viewport3d::ui_with_orbit(ui, size, self.dragging.is_none());

        let mut renderer = rs.renderer.write();
        let Some(res) = renderer.callback_resources.get_mut::<ViewportResources>() else {
            return;
        };
        let scene = &mut res.scene;
        if let (Some(c), Some(p)) = (self.dragging, pos.filter(|p| rect.contains(*p))) {
            let uv = (
                (p.x - rect.left()) / rect.width(),
                (p.y - rect.top()) / rect.height(),
            );
            let (o, dir) = scene.pick_ray(uv, aspect);
            if let Some(hit) = self.editor.surface().and_then(|s| s.ray_hit(o, dir)) {
                // a drop anywhere on an arm or a leg puts the electrode on that limb
                self.drag_limb = limb_under(hit, self.on_body);
                match self.drag_limb {
                    Some(l) => self.editor.place_on_limb(c, l, hit),
                    None => self.editor.drag_to(c, hit),
                }
            }
        } else {
            self.drag_limb = None;
        }
        let markers = self.markers();
        // millimetres per logical pixel at the camera's target: markers keep a minimum size
        let mm_per_px =
            2.0 * scene.camera.distance * (scene.camera.fov_y * 0.5).tan() / rect.height().max(1.0);
        let base_mm = MARKER_RADIUS_MM.max(MARKER_MIN_PX * mm_per_px);
        let instances: Vec<GlyphInstance> = markers
            .iter()
            .map(|(c, p)| {
                let [r, g, b] = cable_colour(*c);
                let radius = if self.editor.selected == Some(*c) {
                    base_mm * 1.4
                } else {
                    base_mm
                };
                GlyphInstance::sphere(*p, radius, srgb_color([r, g, b, 1.0]))
            })
            .collect();
        if let Some(id) = self.glyphs {
            scene.set_spheres(&rs.device, &rs.queue, id, &instances);
        }
        let painter = ui.painter_at(rect);
        // labels on theme-coloured plates, moved apart where the electrodes crowd together
        let (text, plate) = (
            ui.visuals().strong_text_color(),
            ui.visuals().extreme_bg_color.gamma_multiply(0.8),
        );
        let labelled: Vec<(egui::Pos2, std::sync::Arc<egui::Galley>)> = markers
            .iter()
            .filter_map(|(c, p)| {
                let (u, v, _) = scene.camera.project(*p, aspect)?;
                let at = rect.min + egui::vec2(u * rect.width(), v * rect.height());
                let galley =
                    painter.layout_no_wrap(c.label().to_string(), egui::FontId::proportional(13.0), text);
                rect.contains(at).then_some((at, galley))
            })
            .collect();
        let sizes: Vec<(egui::Pos2, egui::Vec2)> = labelled
            .iter()
            .map(|(at, g)| (*at, g.size() + egui::vec2(6.0, 2.0)))
            .collect();
        for ((at, galley), slot) in labelled.into_iter().zip(place_labels(&sizes)) {
            let near = if slot.center().y < at.y {
                slot.bottom()
            } else {
                slot.top()
            };
            painter.line_segment(
                [at, egui::pos2(at.x, near)],
                egui::Stroke::new(1.0, text.gamma_multiply(0.6)),
            );
            painter.rect_filled(slot, 3.0, plate);
            painter.galley(slot.center() - galley.size() * 0.5, galley, text);
        }
        drop(renderer);
        if self.chest.is_some() {
            // view switch, top left of the 3-D view
            let bar = egui::Rect::from_min_size(
                rect.min + egui::vec2(6.0, 6.0),
                egui::vec2(rect.width() - 12.0, 24.0),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.view, View::WholeBody, t!("el-view-body"));
                    ui.selectable_value(&mut self.view, View::Chest, t!("el-view-chest"));
                });
            });
        }
        let hint = match (self.dragging, self.editor.landmark()) {
            _ if !self.editor.revealed() => {
                t!("el-hidden")
            }
            (Some(c), _) if self.drag_limb.is_some() => format!(
                "{}: {}",
                c.label(),
                self.drag_limb
                    .map(heartsim_ui::labels::limb_name)
                    .unwrap_or_default()
            ),
            (Some(c), Some(l)) => format!("{}: {l}", c.label()),
            (Some(c), None) => c.label().to_string(),
            (None, _) => t!("el-drag-hint"),
        };
        painter_hint(ui, rect, &hint);
        heartsim_ui::orientation_gizmo(ui, rect, camera_basis, heartsim_ui::GizmoCorner::TopRight);
        response.on_hover_cursor(if self.dragging.is_some() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::Default
        });
    }
}

/// The hint line along the bottom of the 3-D view: wrapped to its width, in the theme's colours.
fn painter_hint(ui: &egui::Ui, rect: egui::Rect, text: &str) {
    let painter = ui.painter_at(rect);
    let galley = painter.layout(
        text.to_string(),
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
        (rect.width() - 16.0).max(40.0),
    );
    let plate = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 4.0, rect.bottom() - galley.size().y - 8.0),
        galley.size() + egui::vec2(8.0, 4.0),
    );
    painter.rect_filled(plate, 3.0, ui.visuals().extreme_bg_color.gamma_multiply(0.8));
    painter.galley(
        plate.min + egui::vec2(4.0, 2.0),
        galley,
        ui.visuals().text_color(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Distal-limb electrodes are drawn on the figure's wrists and ankles (or where they were
    /// dropped), and a drop anywhere on a limb puts an electrode on that limb; the chest is none.
    #[test]
    fn limb_electrodes_round_trip_through_the_figure() {
        for l in Limb::ALL {
            let drawn = limb_marker(l.anchor_mm(), None, true).expect("limb electrode");
            assert_eq!(limb_under(drawn, true), Some(l), "{l:?} drawn at {drawn}");
            let spot = Vec3::new(drawn.x, drawn.y + 50.0, drawn.z);
            assert_eq!(limb_marker(l.anchor_mm(), Some(spot), true), Some(spot));
        }
        // upper arm, thigh and shoulder
        assert_eq!(
            limb_under(Vec3::new(250.0, 100.0, 0.0), true),
            Some(Limb::LeftArm)
        );
        assert_eq!(
            limb_under(Vec3::new(-350.0, -100.0, 0.0), true),
            Some(Limb::RightArm)
        );
        assert_eq!(
            limb_under(Vec3::new(80.0, -700.0, 0.0), true),
            Some(Limb::LeftLeg)
        );
        assert_eq!(
            limb_under(Vec3::new(-190.0, -600.0, 0.0), true),
            Some(Limb::RightLeg)
        );
        assert_eq!(limb_under(Vec3::new(0.0, 0.0, 80.0), true), None, "chest");
        assert_eq!(limb_under(Vec3::new(-50.0, 300.0, 0.0), true), None, "head");
        assert_eq!(limb_under(BODY_WRISTS_MM[0], false), None, "no figure, no limbs");
        assert_eq!(
            limb_marker(Vec3::new(0.0, 0.0, 80.0), None, true),
            None,
            "trunk point"
        );
    }

    /// Six chest electrodes a few pixels apart get six labels that do not cover one another;
    /// a lone electrode keeps its label straight above it.
    #[test]
    fn crowded_labels_are_moved_apart() {
        let size = egui::vec2(24.0, 16.0);
        let crowd: Vec<(egui::Pos2, egui::Vec2)> = (0..6)
            .map(|k| (egui::pos2(100.0 + 4.0 * k as f32, 200.0 - k as f32), size))
            .chain([(egui::pos2(300.0, 400.0), size)])
            .collect();
        let rects = place_labels(&crowd);
        assert_eq!(rects.len(), crowd.len());
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                assert!(!a.intersects(*b), "{a:?} covers {b:?}");
            }
            assert!(
                (a.center().x - crowd[i].0.x).abs() < 1e-3,
                "labels stay over their marker"
            );
        }
        assert!(
            (rects[6].bottom() - (400.0 - 12.0)).abs() < 1e-3,
            "{:?}",
            rects[6]
        );
    }
}
