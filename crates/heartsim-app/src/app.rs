use crate::settings::settings_window;
use eframe::egui;
use heartsim_core::{AxisClass, FrontalAxis, LEAD_COUNT, Lead, Millis, SimClock};
use heartsim_ecg::{GaussianSynth, LeadRing, estimate_frontal_axis};
use heartsim_geometry::primitives::icosphere;
use heartsim_render::SceneRenderer;
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::{
    AxisDialStyle, EcgView, LeadOrder, TwelveLeadLayout, Viewport3d, ViewportResources, axis_dial, highlight,
    lead_column, lead_polarity, t, transport_controls,
};

use crate::heart_history::HeartHistory;
use crate::rhythm_quiz::RhythmQuiz;
use crate::scenarios::{ScenarioLibrary, Timeline, scenario_picker};
use crate::snapshot_ring::SnapshotRing;
use crate::vector_overlay::VectorOverlay;

#[cfg(feature = "export")]
mod exports;
mod keys;
mod scenario_control;
mod tour;
mod tutorials;
mod vector_tab;

/// eframe storage key of the rhythm-quiz score.
const RHYTHM_QUIZ_SCORE_KEY: &str = "rhythm_quiz_score";
/// eframe storage key of the tutorial progress where there is no settings directory (the web build).
const TUTORIAL_PROGRESS_KEY: &str = "tutorial_progress";

mod ep_lab;
mod pacing;
mod reentry;
mod therapy;

const SAMPLE_RATE_HZ: f32 = 500.0;

/// The vertical slice: timing engine → Tier-0 synth → ring buffers → strip, plus a placeholder
/// sphere in the 3-D viewport whose colour pulses with the ventricular activation (stands in for
/// the heart until anatomy packs and Tier 1 land).
#[derive(Debug)]
pub struct HeartSimApp {
    clock: SimClock,
    engine: TimingEngine,
    synth: GaussianSynth,
    rings: Vec<LeadRing>,
    sample_accumulator: f64,
    selected_lead: Lead,
    /// Strip/12-lead display state: paper, pause + scrub, calipers, wave labels (B7).
    ecg: EcgView,
    /// Viewport scalars at 10 Hz for the last 60 s, shown at the scrub cursor while paused.
    history: SnapshotRing<Vec<f32>>,
    last_v_activation: Millis,
    vertex_count: usize,
    tab: Tab,
    layout: TwelveLeadLayout,
    quiz: Quiz,
    /// The anatomy-pack heart (B9); `None` falls back to the placeholder sphere.
    heart: Option<crate::heart_view::HeartView>,
    /// Pacemaker window, strip spike marks and lead markers (B13).
    pacing: pacing::Pacing,
    /// Drugs, ischaemia and defibrillator window (B14).
    therapy: therapy::Therapy,
    /// Electrode placement editor (B10); its placement sets the synthesiser's lead vectors.
    electrodes: crate::electrode_tab::ElectrodeTab,
    /// Rhythm presets (B11), the loaded one (`None` = slider settings) and its timeline.
    library: ScenarioLibrary,
    scenario: Option<usize>,
    timeline: Timeline,
    /// The heart's colour map over the last minute, shown at the strip cursor while scrubbing.
    heart_history: HeartHistory,
    /// Heart-vector arrow and loop in the 3-D view.
    vector: VectorOverlay,
    rhythm_quiz: RhythmQuiz,
    /// Documentation screenshot run (`HEARTSIM_SCREENSHOT_DIR`).
    tour: Option<tour::Tour>,
    /// Guided tutorials (B15) and the scenario timeline editor.
    tutorials: crate::tutorial_ui::Tutorials,
    timeline_tab: tutorials::TimelineTab,
    /// Export / Import window and the recording under review (B20).
    #[cfg(feature = "export")]
    exports: crate::export_panel::ExportPanel,
    /// Re-entry presets and the running one (B17).
    reentry: reentry::Reentry,
    /// Cardiac EP lab: procedures, catheters, electrograms (B22).
    ep: ep_lab::EpLabTab,
    /// Preferences (language, high contrast, reduced motion) and the Settings window (B19).
    settings: crate::settings::Settings,
    settings_open: bool,
}

/// A snapshot of the model for hosts that cannot look inside it (the web build publishes it to the
/// page so a browser test can tell that the heart is alive and the strip is moving).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppStatus {
    /// Simulation clock, ms.
    pub sim_time_ms: f64,
    /// ECG samples pushed to the strip since start; grows while the simulation runs.
    pub strip_samples: u64,
    /// The anatomy pack is shown (not the placeholder sphere).
    pub anatomy_loaded: bool,
}

const LIMB: [Lead; 6] = [Lead::I, Lead::II, Lead::III, Lead::AVR, Lead::AVL, Lead::AVF];

/// The top-level pages, one tab each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Simulator,
    TwelveLead,
    HeartVector,
    AxisTrainer,
    Quiz,
    Electrodes,
    RhythmQuiz,
    Tutorials,
    EpLab,
    Help,
}

impl Tab {
    const ALL: [Tab; 10] = [
        Tab::Simulator,
        Tab::TwelveLead,
        Tab::HeartVector,
        Tab::AxisTrainer,
        Tab::Quiz,
        Tab::Electrodes,
        Tab::RhythmQuiz,
        Tab::Tutorials,
        Tab::EpLab,
        Tab::Help,
    ];

    fn title(self) -> String {
        match self {
            Tab::Simulator => t!("tab-simulator"),
            Tab::TwelveLead => t!("tab-12-lead"),
            Tab::HeartVector => t!("tab-heart-vector"),
            Tab::AxisTrainer => t!("tab-axis-trainer"),
            Tab::Quiz => t!("tab-axis-quiz"),
            Tab::Electrodes => t!("tab-electrodes"),
            Tab::RhythmQuiz => t!("tab-rhythm-quiz"),
            Tab::Tutorials => t!("tab-tutorials"),
            Tab::EpLab => t!("tab-ep-lab"),
            Tab::Help => t!("tab-help"),
        }
    }
}

/// Axis quiz state: a hidden target axis is dialled into the synthesiser; the student reads the
/// limb leads and clicks where they think the axis points.
#[derive(Debug, Default)]
struct Quiz {
    target_deg: Option<f64>,
    guess_deg: Option<f64>,
    revealed: bool,
    asked: u32,
    correct: u32,
    rng: u64,
}

impl Quiz {
    /// Tolerance for a correct answer, degrees (one hexaxial sector is 30°).
    const TOLERANCE_DEG: f64 = 15.0;

    fn next_random(&mut self) -> f64 {
        // xorshift64*: deterministic per session, no extra dependency
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let r = self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (r >> 11) as f64 / (1u64 << 53) as f64
    }
}

impl HeartSimApp {
    /// The desktop app: anatomy pack, rhythms and tutorials are looked for on disk
    /// (`$HEARTSIM_ASSETS`, next to the executable, the source tree). Without the `native`
    /// feature nothing is found and the app shows the placeholder sphere; use
    /// [`Self::with_assets`] there.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::build(cc, None)
    }

    /// The app on assets that are already in memory (the web build downloads them).
    pub fn with_assets(cc: &eframe::CreationContext<'_>, assets: &crate::assets::AssetBundle) -> Self {
        Self::build(cc, Some(assets))
    }

    /// The anatomy-pack heart and the electrode tab, from the bundle or from disk. The pack is
    /// read once: the electrode tab takes its sites and torso before the heart consumes it.
    fn load_heart(
        assets: Option<&crate::assets::AssetBundle>,
    ) -> (
        Option<crate::heart_view::HeartView>,
        crate::electrode_tab::ElectrodeTab,
    ) {
        let rate = SAMPLE_RATE_HZ;
        let capacity = (SAMPLE_RATE_HZ * 60.0) as usize;
        let timing = TimingParams::default();
        if let Some(assets) = assets {
            return match assets.load_pack() {
                Ok(pack) => {
                    let tab = crate::electrode_tab::ElectrodeTab::new(
                        pack.electrodes.clone(),
                        Some(&pack.torso),
                        rate,
                        capacity,
                    );
                    (Some(crate::heart_view::HeartView::from_pack(pack, &timing)), tab)
                }
                Err(e) => {
                    tracing::warn!("no anatomy pack, showing a sphere: {e}");
                    (
                        None,
                        crate::electrode_tab::ElectrodeTab::load_default(rate, capacity),
                    )
                }
            };
        }
        #[cfg(feature = "native")]
        let heart = crate::heart_view::HeartView::load_default(&timing)
            .inspect_err(|e| tracing::warn!("no anatomy pack, showing a sphere: {e:#}"))
            .ok();
        #[cfg(not(feature = "native"))]
        let heart = None;
        (
            heart,
            crate::electrode_tab::ElectrodeTab::load_default(rate, capacity),
        )
    }

    fn build(cc: &eframe::CreationContext<'_>, assets: Option<&crate::assets::AssetBundle>) -> Self {
        let rs = cc
            .wgpu_render_state
            .as_ref()
            .expect("heartsim requires the wgpu backend");
        let mut scene = SceneRenderer::new(&rs.device, &rs.queue, rs.target_format);
        let mut sphere = icosphere(4);
        sphere.recompute_normals();
        let vertex_count = sphere.vertex_count();
        let (heart, electrodes) = Self::load_heart(assets);
        let heart = heart.map(|mut h| {
            h.install(rs, &mut scene);
            h
        });
        if heart.is_none() {
            scene.set_mesh(&rs.device, &sphere);
        }
        rs.renderer.write().callback_resources.insert(ViewportResources {
            scene,
            pending_scalars: None,
        });
        let mut vector = VectorOverlay::new();
        vector.install(rs);
        let settings = crate::settings::Settings::load(cc.storage);
        settings.apply(&cc.egui_ctx);
        let mut rhythm_quiz = RhythmQuiz::default();
        if let Some(score) = cc.storage.and_then(|s| s.get_string(RHYTHM_QUIZ_SCORE_KEY)) {
            rhythm_quiz.restore_score(&score);
        }

        Self {
            clock: SimClock::new(Millis(1.0)),
            engine: TimingEngine::new(TimingParams::default()),
            synth: GaussianSynth::textbook(),
            rings: (0..LEAD_COUNT)
                .map(|_| LeadRing::new((SAMPLE_RATE_HZ * 60.0) as usize, SAMPLE_RATE_HZ))
                .collect(),
            sample_accumulator: 0.0,
            selected_lead: Lead::II,
            ecg: EcgView::new(),
            history: SnapshotRing::standard(),
            last_v_activation: Millis(-1000.0),
            vertex_count,
            tab: Tab::Simulator,
            layout: TwelveLeadLayout::Stacked,
            quiz: Quiz {
                rng: 0x9E37_79B9_7F4A_7C15,
                ..Quiz::default()
            },
            heart,
            pacing: pacing::Pacing::default(),
            therapy: therapy::Therapy::default(),
            electrodes,
            library: assets.map_or_else(ScenarioLibrary::load_default, ScenarioLibrary::from_bundle),
            scenario: None,
            timeline: Timeline::default(),
            heart_history: HeartHistory::new(),
            vector,
            rhythm_quiz,
            tour: tour::Tour::from_env(),
            tutorials: Self::load_tutorials(cc, assets),
            timeline_tab: tutorials::TimelineTab::default(),
            #[cfg(feature = "export")]
            exports: crate::export_panel::ExportPanel::default(),
            reentry: assets.map_or_else(reentry::Reentry::load_default, reentry::Reentry::from_bundle),
            ep: assets.map_or_else(ep_lab::EpLabTab::load_default, ep_lab::EpLabTab::from_bundle),
            settings,
            settings_open: false,
        }
    }

    fn load_tutorials(
        cc: &eframe::CreationContext<'_>,
        assets: Option<&crate::assets::AssetBundle>,
    ) -> crate::tutorial_ui::Tutorials {
        let Some(assets) = assets else {
            return crate::tutorial_ui::Tutorials::load_default();
        };
        let saved = cc.storage.and_then(|s| s.get_string(TUTORIAL_PROGRESS_KEY));
        crate::tutorial_ui::Tutorials::from_bundle(assets, saved.as_deref())
    }

    /// See [`AppStatus`].
    #[must_use]
    pub fn status(&self) -> AppStatus {
        AppStatus {
            sim_time_ms: self.engine.now().0,
            strip_samples: self.rings[0].pushed(),
            anatomy_loaded: self.heart.is_some(),
        }
    }

    fn step_simulation(&mut self, frame_dt: Millis) {
        let steps = self.clock.advance(frame_dt);
        if self.ep_active() {
            self.step_ep(steps);
            return;
        }
        if self.reentry_active() {
            self.step_reentry(steps);
            return;
        }
        let sample_period_ms = 1000.0 / f64::from(SAMPLE_RATE_HZ);
        for _ in 0..steps {
            self.apply_timeline();
            self.engine.step(self.clock.dt);
            let events = self.engine.drain_events();
            for e in &events {
                if e.region.is_ventricular() && e.kind != heartsim_sim::EventKind::Blocked {
                    self.last_v_activation = e.at;
                }
            }
            self.synth.on_events(&events);
            if let Some(h) = &mut self.heart {
                h.on_step(&events, self.clock.dt, self.engine.now());
            }
            self.pacing_step();
            self.therapy_step();
            self.record_heart_history();
            self.sample_accumulator += self.clock.dt.0;
            while self.sample_accumulator >= sample_period_ms {
                self.sample_accumulator -= sample_period_ms;
                let s = self.synth.sample(self.engine.now());
                for (ring, v) in self.rings.iter_mut().zip(s.0) {
                    ring.push(v as f32);
                }
            }
        }
    }

    fn activation_scalars(&self) -> Vec<f32> {
        // crude stand-in: whole sphere follows a ventricular AP template after each QRS
        let elapsed = (self.engine.now().0 - self.last_v_activation.0) as f32;
        let tpl = heartsim_sim::ap::ApTemplate::ventricular(64);
        let v = tpl.value(elapsed, 300.0);
        vec![v; self.vertex_count]
    }

    /// Viewport scalars: live (recorded into the history), or the snapshot at the scrub cursor.
    fn view_scalars(&mut self) -> Vec<f32> {
        let now = self.engine.now();
        if self.ecg.scrub_offset_ms() > 0.0
            && let Some(s) = self.history.at(Millis(now.0 - self.ecg.scrub_offset_ms()))
        {
            return s.clone();
        }
        let s = self.activation_scalars();
        self.history.record(now, || s.clone());
        s
    }

    /// QRS axis measured from the last few seconds of leads I and aVF, if there are complexes.
    fn measured_axis(&self) -> Option<f64> {
        let n = (SAMPLE_RATE_HZ * 3.0) as usize;
        let (mut i, mut avf) = (Vec::new(), Vec::new());
        self.rings[Lead::I as usize].latest(n, &mut i);
        self.rings[Lead::AVF as usize].latest(n, &mut avf);
        estimate_frontal_axis(&i, &avf, SAMPLE_RATE_HZ).map(|a| a.degrees)
    }

    /// Rotate the ventricular dipole so the QRS axis becomes `target_deg`.
    fn set_qrs_axis(&mut self, target_deg: f64) {
        let mut base = self.synth.clone();
        base.axis_shift_deg = 0.0;
        self.synth.axis_shift_deg = FrontalAxis::normalise(target_deg - base.qrs_axis_deg());
    }

    fn rhythm_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if let Some(i) = scenario_picker(ui, &self.library, self.scenario) {
                self.load_scenario(i);
            }
            ui.separator();
            let transport = transport_controls(ui, self.ecg.is_paused(), &mut self.clock.speed);
            highlight::mark(&transport.pause, "pause");
            if transport.pause.clicked() {
                self.ecg.toggle_pause();
            }
            let mut bpm = self.engine.params.sinus.rate_bpm;
            if ui
                .add(egui::Slider::new(&mut bpm, 30.0..=200.0).text(t!("app-sa-rate")))
                .changed()
            {
                self.engine.params.sinus.rate_bpm = bpm;
            }
            ui.checkbox(&mut self.engine.params.av.complete_block, t!("app-av-block"));
            let mut ratio = self.engine.params.av.mobitz2_ratio;
            if ui
                .add(egui::Slider::new(&mut ratio, 0..=6).text(t!("app-mobitz")))
                .changed()
            {
                self.engine.params.av.mobitz2_ratio = ratio;
            }
            ui.add(
                egui::Slider::new(&mut self.synth.axis_shift_deg, -180.0..=180.0).text(t!("app-axis-shift")),
            );
            ui.add(egui::Slider::new(&mut self.synth.qrs_width_scale, 0.7..=2.0).text(t!("app-qrs-width")));
            highlight::mark(&transport.speed, "speed");
            highlight::mark(
                &ui.toggle_value(&mut self.pacing.open, t!("app-pacemaker")),
                "pacemaker",
            );
            highlight::mark(
                &ui.toggle_value(&mut self.therapy.open, t!("app-therapy")),
                "therapy",
            );
            self.reentry_controls(ui);
        });
    }

    fn paper_controls(&mut self, ui: &mut egui::Ui) {
        self.ecg.controls(ui, &self.rings);
    }

    fn live_axis_panel(&mut self, ui: &mut egui::Ui) {
        let axis = self.measured_axis();
        ui.vertical_centered(|ui| {
            ui.strong(t!("app-frontal-axis"));
            let side = ui.available_width().min(330.0);
            axis_dial(ui, side, axis, None, false, &AxisDialStyle::default());
        });
        if let Some(a) = axis {
            ui.separator();
            ui.label(t!("app-net-polarity"));
            egui::Grid::new("polarity").show(ui, |ui| {
                for l in LIMB {
                    ui.label(l.label());
                    ui.label(lead_polarity(a, l));
                    ui.end_row();
                }
            });
        }
        ui.separator();
        self.ecg.readout(ui);
    }

    fn simulator_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("strip")
            .resizable(true)
            .default_size(200.0)
            .show(ui, |ui| {
                let leads = ui.horizontal_wrapped(|ui| {
                    for l in Lead::ALL {
                        ui.selectable_value(&mut self.selected_lead, l, l.label());
                    }
                });
                highlight::mark(&leads.response, "lead_selector");
                self.paper_controls(ui);
                let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
                let window = self.ecg.strip(ui, rect, &self.rings, self.selected_lead);
                self.pacing.marks.draw(ui.painter(), &window);
                self.draw_timeline_marks(ui.painter(), &window);
                ui.allocate_rect(rect, egui::Sense::hover());
            });
        egui::Panel::right("axis")
            .default_size(280.0)
            .show(ui, |ui| self.live_axis_panel(ui));
        if let Some(h) = &mut self.heart {
            egui::Panel::left("heart").default_size(280.0).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    h.controls(ui);
                    ui.separator();
                    let mut alphas: Vec<&mut f32> = h.panel.layers.iter_mut().map(|l| &mut l.alpha).collect();
                    self.vector.controls(ui, &mut alphas);
                });
            });
        }
        self.sync_vector_overlay();
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(h) = &mut self.heart {
                h.viewport(ui, &mut self.engine);
            } else {
                let size = ui.available_size();
                Viewport3d::ui(ui, size);
            }
        });
    }

    fn twelve_lead_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("layout").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(t!("app-layout"));
                for l in [
                    TwelveLeadLayout::Stacked,
                    TwelveLeadLayout::TwoColumn,
                    TwelveLeadLayout::Clinical,
                ] {
                    ui.selectable_value(&mut self.layout, l, l.label());
                }
                ui.separator();
                for o in [LeadOrder::Standard, LeadOrder::Cabrera] {
                    ui.selectable_value(&mut self.ecg.order, o, o.label());
                }
                ui.separator();
                #[cfg(feature = "native")]
                if ui.button(t!("app-save-png")).clicked() {
                    let path =
                        std::path::PathBuf::from(format!("heartsim-12lead-{:.0}.png", self.engine.now().0));
                    let ppp = ui.ctx().pixels_per_point();
                    self.ecg.export_page_png(&self.rings, self.layout, ppp, &path);
                }
                if let Some(msg) = &self.ecg.last_export {
                    ui.weak(msg);
                }
            });
            self.paper_controls(ui);
        });
        egui::Panel::right("axis")
            .default_size(280.0)
            .show(ui, |ui| self.live_axis_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
            self.ecg.page(ui, rect, &self.rings, self.layout);
            ui.allocate_rect(rect, egui::Sense::hover());
        });
    }

    fn axis_trainer_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("dial").default_size(430.0).show(ui, |ui| {
            ui.label(t!("app-axis-trainer-hint"));
            let axis = self.measured_axis();
            let side = ui.available_width().min(ui.available_height() - 40.0).max(200.0);
            if let Some(target) = axis_dial(ui, side, axis, None, true, &AxisDialStyle::default()) {
                self.set_qrs_axis(target);
            }
            if ui.button(t!("app-reset-axis")).clicked() {
                self.synth.axis_shift_deg = 0.0;
            }
        });
        egui::Panel::right("polarity").default_size(200.0).show(ui, |ui| {
            ui.strong(t!("app-net-qrs-each"));
            if let Some(a) = self.measured_axis() {
                for l in LIMB {
                    ui.label(format!("{:<4} {}", l.label(), lead_polarity(a, l)));
                }
                ui.separator();
                ui.label(match (FrontalAxis { degrees: a }).classify() {
                    AxisClass::Normal => t!("app-axis-normal"),
                    AxisClass::LeftDeviation => t!("app-axis-left"),
                    AxisClass::RightDeviation => t!("app-axis-right"),
                    AxisClass::ExtremeDeviation => t!("app-axis-extreme"),
                });
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
            lead_column(ui, rect, &self.rings, &self.ecg.style, &LIMB);
        });
    }

    fn quiz_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("quizdial").default_size(430.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(t!("app-quiz-new")).clicked() {
                    let target = FrontalAxis::normalise(self.quiz.next_random() * 360.0 - 180.0);
                    self.set_qrs_axis(target);
                    self.quiz.target_deg = Some(target);
                    self.quiz.guess_deg = None;
                    self.quiz.revealed = false;
                }
                let can_check =
                    self.quiz.target_deg.is_some() && self.quiz.guess_deg.is_some() && !self.quiz.revealed;
                if ui
                    .add_enabled(can_check, egui::Button::new(t!("app-quiz-check")))
                    .clicked()
                {
                    self.quiz.revealed = true;
                    self.quiz.asked += 1;
                    if let (Some(t), Some(g)) = (self.quiz.target_deg, self.quiz.guess_deg)
                        && FrontalAxis::normalise(g - t).abs() <= Quiz::TOLERANCE_DEG
                    {
                        self.quiz.correct += 1;
                    }
                }
                ui.label(t!(
                    "app-quiz-score",
                    correct = self.quiz.correct.to_string(),
                    asked = self.quiz.asked.to_string()
                ));
            });
            match (self.quiz.target_deg, self.quiz.guess_deg, self.quiz.revealed) {
                (None, ..) => {
                    ui.label(t!("app-quiz-intro"));
                }
                (Some(_), None, _) => {
                    ui.label(t!("app-quiz-click"));
                }
                (Some(t), Some(g), true) => {
                    let err = FrontalAxis::normalise(g - t);
                    ui.colored_label(
                        if err.abs() <= Quiz::TOLERANCE_DEG {
                            egui::Color32::LIGHT_GREEN
                        } else {
                            egui::Color32::LIGHT_RED
                        },
                        t!(
                            "app-quiz-answer",
                            answer = format!("{t:+.0}"),
                            chosen = format!("{g:+.0}"),
                            off = format!("{:.0}", err.abs())
                        ),
                    );
                }
                (Some(_), Some(g), false) => {
                    ui.label(t!("app-quiz-your-answer", chosen = format!("{g:+.0}")));
                }
            }
            let side = ui.available_width().min(ui.available_height() - 40.0).max(200.0);
            let shown = if self.quiz.revealed {
                self.quiz.target_deg
            } else {
                None
            };
            if let Some(g) = axis_dial(
                ui,
                side,
                shown,
                self.quiz.guess_deg,
                !self.quiz.revealed && self.quiz.target_deg.is_some(),
                &AxisDialStyle::default(),
            ) {
                self.quiz.guess_deg = Some(g);
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
            lead_column(ui, rect, &self.rings, &self.ecg.style, &LIMB);
        });
    }

    fn help_tab(ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("heartsim");
                ui.label(t!("help-intro"));
                ui.separator();
                ui.strong(t!("help-tabs-heading"));
                ui.label(t!("help-tab-simulator"));
                ui.label(t!("help-tab-12-lead"));
                ui.label(t!("help-tab-axis-trainer"));
                ui.label(t!("help-tab-heart-vector"));
                ui.label(t!("help-tab-axis-quiz"));
                ui.label(t!("help-tab-tutorials"));
                ui.label(t!("help-tab-ep-lab"));
                ui.label(t!("help-tab-rhythm-quiz"));
                ui.separator();
                ui.strong(t!("help-rhythms-heading"));
                ui.label(t!("help-rhythms-text"));
                ui.separator();
                ui.strong(t!("help-axis-heading"));
                ui.label(t!("help-axis-text"));
                ui.separator();
                ui.strong(t!("help-keys-heading"));
                ui.label(t!("help-keys-space"));
                ui.label(t!("help-keys-arrows"));
                ui.label(t!("help-keys-speed"));
                ui.label(t!("help-keys-tabs"));
                ui.weak(t!("help-keys-note"));
                ui.separator();
                ui.label(t!("help-disclaimer"));
            });
        });
    }
}

impl eframe::App for HeartSimApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(feature = "export")]
        self.exports_frame(&ctx);
        self.keyboard_shortcuts(&ctx);
        if self.settings_open && settings_window(&ctx, &mut self.settings_open, &mut self.settings) {
            self.settings.apply(&ctx);
        }
        self.therapy.set_reduced_motion(self.settings.reduced_motion);
        if let Some(h) = &mut self.heart {
            h.panel.reduced_motion = self.settings.reduced_motion;
        }
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        if !self.ecg.is_paused() {
            self.step_simulation(Millis::from_seconds(f64::from(dt)));
        }
        self.ecg.update(&self.rings);
        // the editor counts live samples; a recording under review must not reach it
        #[cfg(feature = "export")]
        let reviewing = self.exports.review.is_some();
        #[cfg(not(feature = "export"))]
        let reviewing = false;
        if !reviewing {
            self.electrodes.update(&self.rings, &mut self.synth);
        }
        if self.tab != Tab::Electrodes {
            self.electrodes.deactivate(frame.wgpu_render_state.as_ref());
        }
        self.sync_scrub();
        self.drive_tour(&ctx);
        if let Some(rs) = frame.wgpu_render_state.as_ref().filter(|_| self.heart.is_none()) {
            let scalars = self.view_scalars();
            if let Some(res) = rs
                .renderer
                .write()
                .callback_resources
                .get_mut::<ViewportResources>()
            {
                res.pending_scalars = Some(scalars);
            }
        }

        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("heartsim");
                ui.separator();
                for t in Tab::ALL {
                    ui.selectable_value(&mut self.tab, t, t.title());
                }
                #[cfg(feature = "export")]
                self.export_button(ui);
                ui.toggle_value(&mut self.settings_open, t!("settings-button"));
            });
        });
        if matches!(
            self.tab,
            Tab::Simulator | Tab::TwelveLead | Tab::HeartVector | Tab::AxisTrainer | Tab::Electrodes
        ) {
            egui::Panel::top("rhythm").show(ui, |ui| self.rhythm_controls(ui));
        }

        self.pacing_window(&ctx);
        self.therapy_window(&ctx);
        self.tutorial_frame(ui);
        match self.tab {
            Tab::Simulator => self.simulator_tab(ui),
            Tab::TwelveLead => self.twelve_lead_tab(ui),
            Tab::HeartVector => self.vector_tab(ui),
            Tab::AxisTrainer => self.axis_trainer_tab(ui),
            Tab::Quiz => self.quiz_tab(ui),
            Tab::Electrodes => {
                self.electrodes
                    .ui(ui, frame.wgpu_render_state.as_ref(), &self.rings, &self.ecg.style);
            }
            Tab::RhythmQuiz => {
                if let Some(i) = self.rhythm_quiz.ui(ui, &self.library, &self.ecg.style) {
                    self.load_scenario(i);
                    self.tab = Tab::Simulator;
                }
            }
            Tab::Tutorials => self.tutorials_tab(ui),
            Tab::EpLab => self.ep_lab_tab(ui),
            Tab::Help => Self::help_tab(ui),
        }

        ctx.request_repaint();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(RHYTHM_QUIZ_SCORE_KEY, self.rhythm_quiz.score_string());
        #[cfg(not(feature = "native"))]
        storage.set_string(TUTORIAL_PROGRESS_KEY, self.tutorials.progress.to_text());
        storage.set_string(crate::settings::STORAGE_KEY, self.settings.to_storage_string());
    }
}
