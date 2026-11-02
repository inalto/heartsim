use eframe::egui;
use heartsim_core::{AxisClass, FrontalAxis, LEAD_COUNT, Lead, Millis, SimClock};
use heartsim_ecg::{GaussianSynth, LeadRing, estimate_frontal_axis};
use heartsim_geometry::primitives::icosphere;
use heartsim_render::SceneRenderer;
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::{
    AxisDialStyle, StripStyle, TwelveLeadLayout, Viewport3d, ViewportResources, axis_dial, ecg_strip,
    lead_column, lead_polarity, twelve_lead_page,
};

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
    strip_style: StripStyle,
    paused: bool,
    last_v_activation: Millis,
    vertex_count: usize,
    tab: Tab,
    layout: TwelveLeadLayout,
    quiz: Quiz,
    /// The anatomy-pack heart (B9); `None` falls back to the placeholder sphere.
    heart: Option<crate::heart_view::HeartView>,
}

const LIMB: [Lead; 6] = [Lead::I, Lead::II, Lead::III, Lead::AVR, Lead::AVL, Lead::AVF];

/// The top-level pages, one tab each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Simulator,
    TwelveLead,
    AxisTrainer,
    Quiz,
    Help,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Simulator,
        Tab::TwelveLead,
        Tab::AxisTrainer,
        Tab::Quiz,
        Tab::Help,
    ];

    fn title(self) -> &'static str {
        match self {
            Tab::Simulator => "Simulator",
            Tab::TwelveLead => "12-Lead ECG",
            Tab::AxisTrainer => "Axis Trainer",
            Tab::Quiz => "Axis Quiz",
            Tab::Help => "Help",
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
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let rs = cc
            .wgpu_render_state
            .as_ref()
            .expect("heartsim requires the wgpu backend");
        let mut scene = SceneRenderer::new(&rs.device, &rs.queue, rs.target_format);
        let mut sphere = icosphere(4);
        sphere.recompute_normals();
        let vertex_count = sphere.vertex_count();
        let heart = crate::heart_view::HeartView::load_default(&TimingParams::default())
            .inspect_err(|e| tracing::warn!("no anatomy pack, showing a sphere: {e:#}"))
            .ok()
            .map(|mut h| {
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

        Self {
            clock: SimClock::new(Millis(1.0)),
            engine: TimingEngine::new(TimingParams::default()),
            synth: GaussianSynth::textbook(),
            rings: (0..LEAD_COUNT)
                .map(|_| LeadRing::new((SAMPLE_RATE_HZ * 30.0) as usize, SAMPLE_RATE_HZ))
                .collect(),
            sample_accumulator: 0.0,
            selected_lead: Lead::II,
            strip_style: StripStyle::default(),
            paused: false,
            last_v_activation: Millis(-1000.0),
            vertex_count,
            tab: Tab::Simulator,
            layout: TwelveLeadLayout::Stacked,
            quiz: Quiz {
                rng: 0x9E37_79B9_7F4A_7C15,
                ..Quiz::default()
            },
            heart,
        }
    }

    fn step_simulation(&mut self, frame_dt: Millis) {
        let steps = self.clock.advance(frame_dt);
        let sample_period_ms = 1000.0 / f64::from(SAMPLE_RATE_HZ);
        for _ in 0..steps {
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
            if ui.button(if self.paused { "Run" } else { "Pause" }).clicked() {
                self.paused = !self.paused;
            }
            let mut bpm = self.engine.params.sinus.rate_bpm;
            if ui
                .add(egui::Slider::new(&mut bpm, 30.0..=200.0).text("SA rate bpm"))
                .changed()
            {
                self.engine.params.sinus.rate_bpm = bpm;
            }
            ui.checkbox(&mut self.engine.params.av.complete_block, "AV block");
            let mut ratio = self.engine.params.av.mobitz2_ratio;
            if ui
                .add(egui::Slider::new(&mut ratio, 0..=6).text("Mobitz II n"))
                .changed()
            {
                self.engine.params.av.mobitz2_ratio = ratio;
            }
            ui.add(egui::Slider::new(&mut self.synth.axis_shift_deg, -180.0..=180.0).text("axis shift °"));
            ui.add(egui::Slider::new(&mut self.synth.qrs_width_scale, 0.7..=2.0).text("QRS width"));
            ui.add(
                egui::Slider::new(&mut self.clock.speed, 0.05..=2.0)
                    .logarithmic(true)
                    .text("speed"),
            );
        });
    }

    fn paper_controls(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Slider::new(&mut self.strip_style.mm_per_s, 12.5..=50.0).text("mm/s"));
        ui.add(egui::Slider::new(&mut self.strip_style.mm_per_mv, 2.5..=20.0).text("mm/mV"));
    }

    fn live_axis_panel(&mut self, ui: &mut egui::Ui) {
        let axis = self.measured_axis();
        ui.vertical_centered(|ui| {
            ui.strong("Frontal QRS axis");
            let side = ui.available_width().min(330.0);
            axis_dial(ui, side, axis, None, false, &AxisDialStyle::default());
        });
        if let Some(a) = axis {
            ui.separator();
            ui.label("Net QRS polarity:");
            egui::Grid::new("polarity").show(ui, |ui| {
                for l in LIMB {
                    ui.label(l.label());
                    ui.label(lead_polarity(a, l));
                    ui.end_row();
                }
            });
        }
    }

    fn simulator_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("strip")
            .resizable(true)
            .default_size(200.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for l in Lead::ALL {
                        ui.selectable_value(&mut self.selected_lead, l, l.label());
                    }
                    ui.separator();
                    self.paper_controls(ui);
                });
                let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
                ecg_strip(
                    ui,
                    rect,
                    &self.rings[self.selected_lead as usize],
                    &self.strip_style,
                    self.selected_lead.label(),
                );
                ui.allocate_rect(rect, egui::Sense::hover());
            });
        egui::Panel::right("axis")
            .default_size(280.0)
            .show(ui, |ui| self.live_axis_panel(ui));
        if let Some(h) = &mut self.heart {
            egui::Panel::left("heart").default_size(280.0).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| h.controls(ui))
            });
        }
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
                ui.label("Layout:");
                for l in [
                    TwelveLeadLayout::Stacked,
                    TwelveLeadLayout::TwoColumn,
                    TwelveLeadLayout::Clinical,
                ] {
                    ui.selectable_value(&mut self.layout, l, l.label());
                }
                ui.separator();
                self.paper_controls(ui);
            });
        });
        egui::Panel::right("axis")
            .default_size(280.0)
            .show(ui, |ui| self.live_axis_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
            twelve_lead_page(ui, rect, &self.rings, &self.strip_style, self.layout);
        });
    }

    fn axis_trainer_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("dial").default_size(430.0).show(ui, |ui| {
            ui.label("Click or drag on the circle to rotate the heart's electrical axis.");
            let axis = self.measured_axis();
            let side = ui.available_width().min(ui.available_height() - 40.0).max(200.0);
            if let Some(target) = axis_dial(ui, side, axis, None, true, &AxisDialStyle::default()) {
                self.set_qrs_axis(target);
            }
            if ui.button("Reset to normal axis").clicked() {
                self.synth.axis_shift_deg = 0.0;
            }
        });
        egui::Panel::right("polarity").default_size(200.0).show(ui, |ui| {
            ui.strong("Net QRS in each lead");
            if let Some(a) = self.measured_axis() {
                for l in LIMB {
                    ui.label(format!("{:<4} {}", l.label(), lead_polarity(a, l)));
                }
                ui.separator();
                ui.label(match (FrontalAxis { degrees: a }).classify() {
                    AxisClass::Normal => "Lead I and aVF both positive (or II positive): normal.",
                    AxisClass::LeftDeviation => "Lead I positive, aVF negative: left axis deviation.",
                    AxisClass::RightDeviation => "Lead I negative, aVF positive: right axis deviation.",
                    AxisClass::ExtremeDeviation => "Both I and aVF negative: extreme (north-west) axis.",
                });
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
            lead_column(ui, rect, &self.rings, &self.strip_style, &LIMB);
        });
    }

    fn quiz_tab(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("quizdial").default_size(430.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("New question").clicked() {
                    let target = FrontalAxis::normalise(self.quiz.next_random() * 360.0 - 180.0);
                    self.set_qrs_axis(target);
                    self.quiz.target_deg = Some(target);
                    self.quiz.guess_deg = None;
                    self.quiz.revealed = false;
                }
                let can_check =
                    self.quiz.target_deg.is_some() && self.quiz.guess_deg.is_some() && !self.quiz.revealed;
                if ui
                    .add_enabled(can_check, egui::Button::new("Check answer"))
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
                ui.label(format!("Score {}/{}", self.quiz.correct, self.quiz.asked));
            });
            match (self.quiz.target_deg, self.quiz.guess_deg, self.quiz.revealed) {
                (None, ..) => {
                    ui.label(
                        "Press New question, read the six limb leads, then click the axis on the circle.",
                    );
                }
                (Some(_), None, _) => {
                    ui.label("Click the circle where you think the QRS axis points.");
                }
                (Some(t), Some(g), true) => {
                    let err = FrontalAxis::normalise(g - t);
                    ui.colored_label(
                        if err.abs() <= Quiz::TOLERANCE_DEG {
                            egui::Color32::LIGHT_GREEN
                        } else {
                            egui::Color32::LIGHT_RED
                        },
                        format!("Answer {t:+.0}°, you chose {g:+.0}° (off by {:.0}°)", err.abs()),
                    );
                }
                (Some(_), Some(g), false) => {
                    ui.label(format!("Your answer: {g:+.0}°. Press Check answer."));
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
            lead_column(ui, rect, &self.rings, &self.strip_style, &LIMB);
        });
    }

    fn help_tab(ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("heartsim");
                ui.label("An open educational heart electrophysiology and ECG simulator.");
                ui.separator();
                ui.strong("Tabs");
                ui.label("Simulator: the heart model with one selectable lead and the live axis.");
                ui.label("12-Lead ECG: all twelve leads at once (12 × 1, 6 × 2 or the clinical 3 × 4 with rhythm strip) next to the axis circle.");
                ui.label("Axis Trainer: drag on the circle to rotate the axis and watch every limb lead change.");
                ui.label("Axis Quiz: a hidden axis is dialled in; read the leads and click your answer.");
                ui.separator();
                ui.strong("Finding the axis");
                ui.label("Normal axis is −30° to +90°. Lead I and aVF both positive: normal. I positive, aVF negative: left deviation (beyond −30° when II is also negative). I negative, aVF positive: right deviation. Both negative: extreme axis. The equiphasic limb lead is perpendicular to the axis.");
                ui.separator();
                ui.label("Educational tool, not for clinical use.");
            });
        });
    }
}

impl eframe::App for HeartSimApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        if !self.paused {
            self.step_simulation(Millis::from_seconds(f64::from(dt)));
        }
        if let Some(rs) = frame.wgpu_render_state.as_ref().filter(|_| self.heart.is_none()) {
            let scalars = self.activation_scalars();
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
            });
        });
        if matches!(self.tab, Tab::Simulator | Tab::TwelveLead | Tab::AxisTrainer) {
            egui::Panel::top("rhythm").show(ui, |ui| self.rhythm_controls(ui));
        }

        match self.tab {
            Tab::Simulator => self.simulator_tab(ui),
            Tab::TwelveLead => self.twelve_lead_tab(ui),
            Tab::AxisTrainer => self.axis_trainer_tab(ui),
            Tab::Quiz => self.quiz_tab(ui),
            Tab::Help => Self::help_tab(ui),
        }

        ctx.request_repaint();
    }
}
