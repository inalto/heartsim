use eframe::egui;
use heartsim_core::{LEAD_COUNT, Lead, Millis, SimClock};
use heartsim_ecg::{GaussianSynth, LeadRing};
use heartsim_geometry::primitives::icosphere;
use heartsim_render::SceneRenderer;
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::{StripStyle, Viewport3d, ViewportResources, ecg_strip};

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
    show_twelve: bool,
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
        scene.set_mesh(&rs.device, &sphere);
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
            show_twelve: false,
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
}

impl eframe::App for HeartSimApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        if !self.paused {
            self.step_simulation(Millis::from_seconds(f64::from(dt)));
        }
        if let Some(rs) = frame.wgpu_render_state.as_ref() {
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

        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("heartsim");
                ui.separator();
                if ui.button(if self.paused { "Run" } else { "Pause" }).clicked() {
                    self.paused = !self.paused;
                }
                ui.separator();
                let mut bpm = self.engine.params.sinus_rate_bpm;
                if ui
                    .add(egui::Slider::new(&mut bpm, 30.0..=200.0).text("SA rate bpm"))
                    .changed()
                {
                    self.engine.params.sinus_rate_bpm = bpm;
                }
                ui.checkbox(&mut self.engine.params.av_block, "AV block");
                let mut ratio = self.engine.params.mobitz2_ratio;
                if ui
                    .add(egui::Slider::new(&mut ratio, 0..=6).text("Mobitz II n"))
                    .changed()
                {
                    self.engine.params.mobitz2_ratio = ratio;
                }
                ui.add(
                    egui::Slider::new(&mut self.synth.axis_shift_deg, -120.0..=120.0).text("axis shift °"),
                );
                ui.add(egui::Slider::new(&mut self.synth.qrs_width_scale, 0.7..=2.0).text("QRS width"));
                ui.separator();
                ui.add(
                    egui::Slider::new(&mut self.clock.speed, 0.05..=2.0)
                        .logarithmic(true)
                        .text("speed"),
                );
                ui.checkbox(&mut self.show_twelve, "12-lead");
            });
        });

        egui::Panel::bottom("strip")
            .resizable(true)
            .default_size(200.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for l in Lead::ALL {
                        ui.selectable_value(&mut self.selected_lead, l, l.label());
                    }
                    ui.separator();
                    ui.add(egui::Slider::new(&mut self.strip_style.mm_per_s, 12.5..=50.0).text("mm/s"));
                    ui.add(egui::Slider::new(&mut self.strip_style.mm_per_mv, 5.0..=20.0).text("mm/mV"));
                });
                let avail = ui.available_size();
                if self.show_twelve {
                    let rows = 4;
                    let cols = 3;
                    let cell = egui::vec2(avail.x / cols as f32, avail.y / rows as f32);
                    let origin = ui.cursor().min;
                    for (i, l) in Lead::ALL.iter().enumerate() {
                        let r = i % rows;
                        let c = i / rows;
                        let rect = egui::Rect::from_min_size(
                            origin + egui::vec2(c as f32 * cell.x, r as f32 * cell.y),
                            cell,
                        );
                        ecg_strip(ui, rect, &self.rings[*l as usize], &self.strip_style, l.label());
                    }
                    ui.allocate_rect(egui::Rect::from_min_size(origin, avail), egui::Sense::hover());
                } else {
                    let rect = egui::Rect::from_min_size(ui.cursor().min, avail);
                    ecg_strip(
                        ui,
                        rect,
                        &self.rings[self.selected_lead as usize],
                        &self.strip_style,
                        self.selected_lead.label(),
                    );
                    ui.allocate_rect(rect, egui::Sense::hover());
                }
            });

        egui::CentralPanel::default().show(ui, |ui| {
            let size = ui.available_size();
            Viewport3d::ui(ui, size);
        });

        ctx.request_repaint();
    }
}
