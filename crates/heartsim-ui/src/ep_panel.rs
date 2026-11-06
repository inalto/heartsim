//! The EP-lab panels (brief B22): the stack of electrograms with calipers at 100–200 mm/s, catheter
//! placement, programmed stimulation and the RF generator.
//!
//! Like the other panels these only edit an [`EpPanelState`] and report [`EpCommand`]s; the app
//! applies them to the engine. What the panels show is read back from the engine and the recorder,
//! so no result here is computed twice.

use crate::strip::{StripStyle, StripTrace, draw_strip};
use crate::t;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use glam::Vec3;
use heartsim_ecg::{ConductionIntervals, EgmRecorder, EgmSource, LeadRing};
use heartsim_geometry::{Catheter, CatheterKind, CatheterSite};
use heartsim_render::{GlyphInstance, glyph};
use heartsim_sim::ep::{
    AblationTarget, EntrainmentResult, ErpResult, ErpTarget, Lesion, Protocol, ProtocolOutcome,
    ProtocolStatus, RfReadout,
};

use crate::i18n::number;

/// Sweep speeds of the electrogram panel, mm/s.
pub const EGM_SPEEDS_MM_S: [f32; 2] = [100.0, 200.0];
/// Electrogram gains offered, mm/mV (near-field potentials are several mV).
pub const EGM_GAINS_MM_MV: [f32; 4] = [1.0, 2.0, 5.0, 10.0];
/// Lowest row height of the stack, points.
const MIN_ROW_PX: f32 = 26.0;
/// Radius of a catheter body, mm (≈ 7 F).
const BODY_RADIUS_MM: f32 = 1.1;
/// Radius of an electrode marker, mm.
const ELECTRODE_RADIUS_MM: f32 = 1.5;

/// The protocol the form builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolKind {
    Continuous,
    Train,
    Decremental,
    Erp,
    Entrainment,
}

impl ProtocolKind {
    const ALL: [ProtocolKind; 5] = [
        ProtocolKind::Continuous,
        ProtocolKind::Train,
        ProtocolKind::Decremental,
        ProtocolKind::Erp,
        ProtocolKind::Entrainment,
    ];

    fn label(self) -> String {
        match self {
            ProtocolKind::Continuous => t!("ep-protocol-continuous"),
            ProtocolKind::Train => t!("ep-protocol-train"),
            ProtocolKind::Decremental => t!("ep-protocol-decremental"),
            ProtocolKind::Erp => t!("ep-protocol-erp"),
            ProtocolKind::Entrainment => t!("ep-protocol-entrainment"),
        }
    }
}

fn erp_target_label(target: ErpTarget) -> String {
    match target {
        ErpTarget::Atrium => t!("ep-erp-atrium"),
        ErpTarget::Ventricle => t!("ep-erp-ventricle"),
        ErpTarget::AvNode => t!("ep-erp-av-node"),
        ErpTarget::FastPathway => t!("ep-erp-fast-pathway"),
        ErpTarget::AccessoryPathway => t!("ep-erp-accessory"),
    }
}

fn ablation_target_label(target: AblationTarget) -> String {
    match target {
        AblationTarget::AvNode => t!("ep-target-av-node"),
        AblationTarget::SlowPathway => t!("ep-target-slow-pathway"),
        AblationTarget::CtiLine => t!("ep-target-cti"),
        AblationTarget::PulmonaryVeins => t!("ep-target-pv"),
        AblationTarget::AccessoryPathway => t!("ep-target-accessory"),
    }
}

/// What the panels edit.
#[expect(clippy::struct_excessive_bools, reason = "independent check boxes of a form")]
#[derive(Debug, Clone, PartialEq)]
pub struct EpPanelState {
    /// Sweep speed, mm/s.
    pub sweep_mm_s: f32,
    /// Electrogram gain, mm/mV.
    pub gain_mm_mv: f32,
    /// Show the unipolar potential of the distal electrode of each pair instead of the bipole.
    pub unipolar: bool,
    /// Caliper lines, ms of simulation time. They stay on the beat they were put on while the paper
    /// moves.
    pub caliper: Option<(f64, f64)>,
    /// A caliper is being dragged out from its first line.
    pub dragging: bool,
    /// Index of the pacing channel in the recorder's list.
    pub pace_channel: usize,
    pub kind: ProtocolKind,
    pub cycle_ms: f64,
    pub s1_count: u32,
    pub s2_ms: f64,
    pub use_s2: bool,
    pub s3_ms: f64,
    pub use_s3: bool,
    pub ramp_end_ms: f64,
    pub ramp_step_ms: f64,
    pub erp_target: ErpTarget,
    pub entrain_offset_ms: f64,
    pub entrain_count: u32,
    /// RF power, W.
    pub power_w: f64,
    /// Heating time constant, s: real 8 s, or short so a lesion forms while watching Tier 2.
    pub rf_tau_s: f64,
}

impl Default for EpPanelState {
    fn default() -> Self {
        Self {
            sweep_mm_s: 100.0,
            gain_mm_mv: 2.0,
            unipolar: false,
            caliper: None,
            dragging: false,
            pace_channel: 0,
            kind: ProtocolKind::Train,
            cycle_ms: 600.0,
            s1_count: 8,
            s2_ms: 400.0,
            use_s2: true,
            s3_ms: 300.0,
            use_s3: false,
            ramp_end_ms: 300.0,
            ramp_step_ms: 20.0,
            erp_target: ErpTarget::AvNode,
            entrain_offset_ms: 20.0,
            entrain_count: 12,
            power_w: 30.0,
            rf_tau_s: 8.0,
        }
    }
}

impl EpPanelState {
    /// The protocol the form describes.
    #[must_use]
    pub fn protocol(&self) -> Protocol {
        match self.kind {
            ProtocolKind::Continuous => Protocol::Continuous {
                cycle_ms: self.cycle_ms,
            },
            ProtocolKind::Train => Protocol::Train {
                s1_cycle_ms: self.cycle_ms,
                s1_count: self.s1_count,
                s2_ms: self.use_s2.then_some(self.s2_ms),
                s3_ms: (self.use_s2 && self.use_s3).then_some(self.s3_ms),
            },
            ProtocolKind::Decremental => Protocol::Decremental {
                start_cycle_ms: self.cycle_ms,
                end_cycle_ms: self.ramp_end_ms.min(self.cycle_ms),
                step_ms: self.ramp_step_ms,
                beats: self.s1_count,
            },
            ProtocolKind::Erp => Protocol::ErpSearch {
                target: self.erp_target,
                s1_cycle_ms: self.cycle_ms,
                s1_count: self.s1_count,
                s2_start_ms: self.s2_ms,
                s2_step_ms: 10.0,
                s2_floor_ms: 150.0,
            },
            ProtocolKind::Entrainment => Protocol::Entrainment {
                offset_ms: self.entrain_offset_ms,
                count: self.entrain_count,
            },
        }
    }
}

/// What the user asked for this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum EpCommand {
    MoveCatheter {
        catheter: usize,
        site: CatheterSite,
    },
    StartProtocol {
        channel: usize,
        protocol: Protocol,
    },
    StopProtocol,
    PaceOnce {
        channel: usize,
    },
    RfOn,
    RfOff,
    /// Put the ablation tip on a target (the first stop of a line).
    GoToTarget(AblationTarget),
    /// Lay the whole target at once.
    AblateTarget(AblationTarget),
}

// --- the electrogram stack -----------------------------------------------------------------------

/// One row above the electrograms: a surface lead from the strip rings.
pub type SurfaceRow<'a> = (&'a str, &'a LeadRing);

fn x_of_time(rect: Rect, now_ms: f64, px_per_ms: f32, t_ms: f64) -> f32 {
    rect.right() - (now_ms - t_ms) as f32 * px_per_ms
}

/// The electrogram stack: surface leads, then every bipole of every catheter, one time base, with
/// stimulus marks and a caliper dragged across all rows. `now_ms` is the time of the newest sample.
pub fn egm_stack(
    ui: &mut Ui,
    state: &mut EpPanelState,
    surface: &[SurfaceRow<'_>],
    recorder: &EgmRecorder,
    style: &StripStyle,
    now_ms: f64,
) {
    let rows = surface.len() + recorder.channels().len();
    if rows == 0 {
        return;
    }
    let size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let row_h = (rect.height() / rows as f32).max(MIN_ROW_PX);
    let row = |k: usize| {
        Rect::from_min_size(
            rect.min + Vec2::new(0.0, k as f32 * row_h),
            Vec2::new(rect.width(), row_h),
        )
    };
    let egm_style = StripStyle {
        mm_per_s: state.sweep_mm_s,
        mm_per_mv: state.gain_mm_mv,
        show_grid: false,
        trace_width: 1.0,
        ..*style
    };
    let surface_style = StripStyle {
        mm_per_s: state.sweep_mm_s,
        show_grid: false,
        trace_width: 1.0,
        ..*style
    };
    let base_id = ui.scope_id().with("egm_stack");
    for (k, (label, ring)) in surface.iter().enumerate() {
        let trace = StripTrace {
            label,
            invert: false,
            head: ring.pushed(),
            marks: None,
            show_waves: false,
            show_intervals: false,
        };
        draw_strip(ui, base_id.with(k), row(k), ring, &surface_style, &trace);
    }
    for (c, channel) in recorder.channels().iter().enumerate() {
        let ring = if state.unipolar {
            recorder.unipolar(c)
        } else {
            recorder.bipolar(c)
        };
        let trace = StripTrace {
            label: &channel.label,
            invert: false,
            head: ring.pushed(),
            marks: None,
            show_waves: false,
            show_intervals: false,
        };
        let k = surface.len() + c;
        let rect_k = row(k);
        draw_strip(ui, base_id.with(k), rect_k, ring, &egm_style, &trace);
        ui.painter_at(rect).line_segment(
            [rect_k.left_bottom(), rect_k.right_bottom()],
            Stroke::new(0.5, catheter_colour(channel.kind).gamma_multiply(0.5)),
        );
    }
    let px_per_ms = state.sweep_mm_s * style.px_per_mm / 1000.0;
    let painter = ui.painter_at(rect);
    for t in recorder.stimuli() {
        let x = x_of_time(rect, now_ms, px_per_ms, t);
        if x >= rect.left() {
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(0.7, Color32::from_rgb(230, 120, 20)),
            );
        }
    }
    caliper_drag(state, &response, rect, now_ms, px_per_ms);
    if let Some((a, b)) = state.caliper {
        let colour = style.caliper;
        for t in [a, b] {
            let x = x_of_time(rect, now_ms, px_per_ms, t);
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0, colour),
            );
        }
        let delta = (b - a).abs();
        painter.text(
            rect.left_top() + Vec2::new(rect.width() * 0.5, 2.0),
            egui::Align2::CENTER_TOP,
            t!("ep-caliper", value = number(delta, 0)),
            egui::FontId::proportional(13.0),
            colour,
        );
    }
}

fn caliper_drag(
    state: &mut EpPanelState,
    response: &egui::Response,
    rect: Rect,
    now_ms: f64,
    px_per_ms: f32,
) {
    let time_at = |x: f32| now_ms - f64::from((rect.right() - x) / px_per_ms);
    if response.drag_started()
        && let Some(p) = response.interact_pointer_pos()
    {
        let t0 = time_at(p.x);
        state.caliper = Some((t0, t0));
        state.dragging = true;
    }
    if state.dragging
        && let (Some(p), Some((a, _))) = (response.interact_pointer_pos(), state.caliper)
    {
        state.caliper = Some((a, time_at(p.x)));
    }
    if response.drag_stopped() {
        state.dragging = false;
    }
    if response.secondary_clicked() {
        state.caliper = None;
    }
}

/// Stable colour of a catheter in the panels and on the heart.
#[must_use]
pub fn catheter_colour(kind: CatheterKind) -> Color32 {
    let [r, g, b] = kind.colour();
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

// --- catheter glyphs -----------------------------------------------------------------------------

/// `(spheres, tubes)` of the catheters on the heart: a shaft, a marker per electrode, the pacing
/// pair highlighted, and a dark mark per lesion.
#[must_use]
pub fn catheter_glyphs(
    catheters: &[Catheter],
    lesions: &[Lesion],
    pace_midpoint: Option<Vec3>,
) -> (Vec<GlyphInstance>, Vec<GlyphInstance>) {
    let (mut spheres, mut tubes) = (Vec::new(), Vec::new());
    for c in catheters {
        let [r, g, b] = c.kind.colour();
        let colour = [r, g, b, 1.0];
        let (t, j) = glyph::polyline(&c.body, BODY_RADIUS_MM, [r * 0.5, g * 0.5, b * 0.5, 1.0]);
        tubes.extend(t);
        spheres.extend(j);
        spheres.extend(
            c.electrodes
                .iter()
                .map(|e| GlyphInstance::sphere(*e, ELECTRODE_RADIUS_MM, colour)),
        );
    }
    if let Some(p) = pace_midpoint {
        spheres.push(GlyphInstance::sphere(
            p,
            ELECTRODE_RADIUS_MM * 1.6,
            [1.0, 0.55, 0.05, 1.0],
        ));
    }
    spheres.extend(
        lesions
            .iter()
            .filter(|l| l.radius_mm > 0.0)
            .map(|l| GlyphInstance::sphere(l.centre(), l.radius_mm, [0.09, 0.05, 0.05, 1.0])),
    );
    (spheres, tubes)
}

// --- the control panels --------------------------------------------------------------------------

/// Sweep, gain, trace type and the automatic calipers.
pub fn egm_controls(ui: &mut Ui, state: &mut EpPanelState, intervals: Option<ConductionIntervals>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(t!("ep-sweep"));
        for s in EGM_SPEEDS_MM_S {
            ui.selectable_value(
                &mut state.sweep_mm_s,
                s,
                t!("ep-mm-s", value = number(f64::from(s), 0)),
            );
        }
        ui.separator();
        ui.label(t!("ep-gain"));
        for g in EGM_GAINS_MM_MV {
            ui.selectable_value(
                &mut state.gain_mm_mv,
                g,
                t!("ep-mm-mv", value = number(f64::from(g), 0)),
            );
        }
        ui.separator();
        ui.checkbox(&mut state.unipolar, t!("ep-unipolar"));
        ui.separator();
        if let Some(i) = intervals {
            if ui
                .button(t!("ep-caliper-ah"))
                .on_hover_text(t!("ep-caliper-ah-hint"))
                .clicked()
            {
                state.caliper = Some((i.a_ms, i.h_ms));
            }
            if ui
                .button(t!("ep-caliper-hv"))
                .on_hover_text(t!("ep-caliper-hv-hint"))
                .clicked()
            {
                state.caliper = Some((i.h_ms, i.v_ms));
            }
        }
        if state.caliper.is_some() && ui.button(t!("ep-caliper-clear")).clicked() {
            state.caliper = None;
        }
    });
}

/// AH / HV / CL / PPI read-outs.
pub fn interval_panel(ui: &mut Ui, recorder: &EgmRecorder, outcome: Option<&ProtocolOutcome>) {
    let blank = || t!("ep-no-value");
    let ms = |x: f64| t!("ep-ms", value = number(x, 0));
    let intervals = recorder.conduction_intervals();
    let cl = |kind: CatheterKind, source: EgmSource| {
        recorder
            .channels()
            .iter()
            .position(|c| c.kind == kind)
            .and_then(|c| recorder.cycle_length_ms(c, source))
    };
    ui.horizontal_wrapped(|ui| {
        ui.strong(t!("ep-ah"));
        ui.label(intervals.map_or_else(blank, |i| ms(i.ah_ms)));
        ui.separator();
        ui.strong(t!("ep-hv"));
        ui.label(intervals.map_or_else(blank, |i| ms(i.hv_ms)));
        ui.separator();
        ui.strong(t!("ep-cl-a"));
        ui.label(cl(CatheterKind::Hra, EgmSource::Atrial).map_or_else(blank, ms));
        ui.separator();
        ui.strong(t!("ep-cl-v"));
        ui.label(cl(CatheterKind::Rva, EgmSource::Ventricular).map_or_else(blank, ms));
        if let Some(ProtocolOutcome::Entrainment(e)) = outcome {
            ui.separator();
            ui.strong(t!("ep-ppi"));
            ui.label(ms(e.ppi_ms));
        }
    });
}

/// Placing the catheters. Landmarks are the pack's; the CS catheter slides along its vessel.
pub fn catheter_panel(
    ui: &mut Ui,
    catheters: &[Catheter],
    landmarks: &[String],
    cs_length_mm: f32,
    commands: &mut Vec<EpCommand>,
) {
    ui.heading(t!("ep-catheters"));
    for (i, c) in catheters.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.colored_label(catheter_colour(c.kind), c.kind.prefix());
            if c.kind == CatheterKind::Cs {
                let mut depth = match c.site {
                    CatheterSite::CsDepth(d) => d,
                    _ => cs_length_mm,
                };
                let slider = egui::Slider::new(&mut depth, 0.0..=cs_length_mm.max(1.0))
                    .text(t!("ep-cs-depth"))
                    .fixed_decimals(0);
                if ui.add(slider).changed() {
                    commands.push(EpCommand::MoveCatheter {
                        catheter: i,
                        site: CatheterSite::CsDepth(depth),
                    });
                }
            } else {
                let shown = match &c.site {
                    CatheterSite::Landmark(name) => name.clone(),
                    CatheterSite::Point(_) => t!("ep-site-point"),
                    _ => t!("ep-site-default"),
                };
                egui::ComboBox::from_id_salt(("ep_catheter_site", i))
                    .selected_text(shown)
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(false, t!("ep-site-default")).clicked() {
                            commands.push(EpCommand::MoveCatheter {
                                catheter: i,
                                site: CatheterSite::Default,
                            });
                        }
                        for name in landmarks {
                            if ui.selectable_label(false, name).clicked() {
                                commands.push(EpCommand::MoveCatheter {
                                    catheter: i,
                                    site: CatheterSite::Landmark(name.clone()),
                                });
                            }
                        }
                    });
            }
        });
    }
    ui.small(t!("ep-catheter-hint"));
}

fn ms_slider(ui: &mut Ui, value: &mut f64, range: std::ops::RangeInclusive<f64>, label: String) {
    ui.add(egui::Slider::new(value, range).text(label).fixed_decimals(0));
}

/// Programmed stimulation: which pair, which protocol, and what came of it.
pub fn pes_panel(
    ui: &mut Ui,
    state: &mut EpPanelState,
    channels: &[String],
    status: ProtocolStatus,
    outcome: Option<&ProtocolOutcome>,
    tier_two: bool,
    commands: &mut Vec<EpCommand>,
) {
    ui.heading(t!("ep-pes"));
    let shown = channels.get(state.pace_channel).cloned().unwrap_or_default();
    egui::ComboBox::new("ep_pace_pair", t!("ep-pace-pair"))
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (i, name) in channels.iter().enumerate() {
                ui.selectable_value(&mut state.pace_channel, i, name);
            }
        });
    egui::ComboBox::new("ep_protocol", t!("ep-protocol"))
        .selected_text(state.kind.label())
        .show_ui(ui, |ui| {
            for k in ProtocolKind::ALL {
                ui.selectable_value(&mut state.kind, k, k.label());
            }
        });
    match state.kind {
        ProtocolKind::Continuous => ms_slider(ui, &mut state.cycle_ms, 150.0..=1000.0, t!("ep-cycle")),
        ProtocolKind::Train => {
            ms_slider(ui, &mut state.cycle_ms, 200.0..=1000.0, t!("ep-s1-cycle"));
            ui.add(egui::Slider::new(&mut state.s1_count, 1..=20).text(t!("ep-s1-count")));
            ui.checkbox(&mut state.use_s2, t!("ep-use-s2"));
            if state.use_s2 {
                ms_slider(ui, &mut state.s2_ms, 150.0..=600.0, t!("ep-s2"));
                ui.checkbox(&mut state.use_s3, t!("ep-use-s3"));
                if state.use_s3 {
                    ms_slider(ui, &mut state.s3_ms, 150.0..=600.0, t!("ep-s3"));
                }
            }
        }
        ProtocolKind::Decremental => {
            ms_slider(ui, &mut state.cycle_ms, 200.0..=1000.0, t!("ep-ramp-start"));
            ms_slider(ui, &mut state.ramp_end_ms, 150.0..=1000.0, t!("ep-ramp-end"));
            ms_slider(ui, &mut state.ramp_step_ms, 5.0..=50.0, t!("ep-ramp-step"));
            ui.add(egui::Slider::new(&mut state.s1_count, 1..=20).text(t!("ep-ramp-beats")));
        }
        ProtocolKind::Erp => {
            egui::ComboBox::new("ep_erp_target", t!("ep-erp-target"))
                .selected_text(erp_target_label(state.erp_target))
                .show_ui(ui, |ui| {
                    for target in [
                        ErpTarget::Atrium,
                        ErpTarget::AvNode,
                        ErpTarget::FastPathway,
                        ErpTarget::AccessoryPathway,
                        ErpTarget::Ventricle,
                    ] {
                        ui.selectable_value(&mut state.erp_target, target, erp_target_label(target));
                    }
                });
            ms_slider(ui, &mut state.cycle_ms, 300.0..=1000.0, t!("ep-s1-cycle"));
            ui.add(egui::Slider::new(&mut state.s1_count, 2..=12).text(t!("ep-s1-count")));
            ms_slider(ui, &mut state.s2_ms, 200.0..=600.0, t!("ep-s2-start"));
        }
        ProtocolKind::Entrainment => {
            ms_slider(
                ui,
                &mut state.entrain_offset_ms,
                10.0..=60.0,
                t!("ep-entrain-offset"),
            );
            ui.add(egui::Slider::new(&mut state.entrain_count, 3..=30).text(t!("ep-entrain-count")));
        }
    }
    // the ERP search reads the timing engine's capture decision, which Tier 2 leaves to the tissue
    let allowed = !(tier_two && state.kind == ProtocolKind::Erp);
    ui.horizontal(|ui| {
        let running = !matches!(status, ProtocolStatus::Finished);
        if ui
            .add_enabled(allowed && !running, egui::Button::new(t!("ep-start")))
            .clicked()
        {
            commands.push(EpCommand::StartProtocol {
                channel: state.pace_channel,
                protocol: state.protocol(),
            });
        }
        if ui
            .add_enabled(running, egui::Button::new(t!("ep-stop")))
            .clicked()
        {
            commands.push(EpCommand::StopProtocol);
        }
        if ui.button(t!("ep-pace-once")).clicked() {
            commands.push(EpCommand::PaceOnce {
                channel: state.pace_channel,
            });
        }
    });
    if !allowed {
        ui.small(t!("ep-erp-needs-tier1"));
    }
    match status {
        ProtocolStatus::Pacing {
            delivered,
            interval_ms,
        } => {
            ui.label(t!(
                "ep-status-pacing",
                count = i64::from(delivered),
                interval = number(interval_ms, 0)
            ));
        }
        ProtocolStatus::Watching => {
            ui.label(t!("ep-status-watching"));
        }
        ProtocolStatus::Finished => {}
    }
    if let Some(o) = outcome {
        outcome_label(ui, o);
    }
}

fn outcome_label(ui: &mut Ui, outcome: &ProtocolOutcome) {
    match outcome {
        ProtocolOutcome::Delivered { stimuli } => {
            ui.label(t!("ep-outcome-delivered", count = i64::from(*stimuli)));
        }
        ProtocolOutcome::NoReturn => {
            ui.label(t!("ep-outcome-no-return"));
        }
        ProtocolOutcome::Erp(e) => erp_label(ui, e),
        ProtocolOutcome::Entrainment(e) => entrainment_label(ui, e),
    }
}

fn erp_label(ui: &mut Ui, e: &ErpResult) {
    let target = erp_target_label(e.target);
    match e.erp_ms {
        Some(erp) => {
            ui.strong(t!("ep-outcome-erp", target = target, value = number(erp, 0)));
        }
        None if e.limited_by_local_erp => {
            ui.label(t!("ep-outcome-erp-limited", target = target));
        }
        None => {
            ui.label(t!("ep-outcome-erp-none", target = target));
        }
    }
    if let Some((coupling, jump)) = e.ah_jump {
        ui.label(t!(
            "ep-outcome-ah-jump",
            coupling = number(coupling, 0),
            jump = number(jump, 0)
        ));
    }
}

fn entrainment_label(ui: &mut Ui, e: &EntrainmentResult) {
    ui.label(t!(
        "ep-outcome-entrainment",
        tcl = number(e.tcl_ms, 0),
        cycle = number(e.pacing_cycle_ms, 0),
        ppi = number(e.ppi_ms, 0),
        delta = number(e.ppi_ms - e.tcl_ms, 0)
    ));
    if e.captured < e.delivered {
        ui.colored_label(
            Color32::LIGHT_RED,
            t!(
                "ep-outcome-partial-capture",
                captured = i64::from(e.captured),
                delivered = i64::from(e.delivered)
            ),
        );
    }
    ui.strong(if e.in_circuit {
        t!("ep-outcome-in-circuit")
    } else {
        t!("ep-outcome-out-of-circuit")
    });
}

/// The RF generator: targets, RF on/off, read-outs, lesion count and the goal.
pub fn ablation_panel(
    ui: &mut Ui,
    state: &mut EpPanelState,
    targets: &[AblationTarget],
    readout: Option<RfReadout>,
    lesions: usize,
    goal_text: Option<String>,
    goal_reached: bool,
    commands: &mut Vec<EpCommand>,
) {
    ui.heading(t!("ep-ablation"));
    for target in targets {
        ui.horizontal(|ui| {
            ui.label(ablation_target_label(*target));
            if ui.small_button(t!("ep-go-to-target")).clicked() {
                commands.push(EpCommand::GoToTarget(*target));
            }
            if ui.small_button(t!("ep-ablate-target")).clicked() {
                commands.push(EpCommand::AblateTarget(*target));
            }
        });
    }
    ui.add(egui::Slider::new(&mut state.power_w, 5.0..=50.0).text(t!("ep-power")));
    ui.add(egui::Slider::new(&mut state.rf_tau_s, 1.0..=15.0).text(t!("ep-rf-tau")));
    let on = readout.is_some();
    let label = if on { t!("ep-rf-off") } else { t!("ep-rf-on") };
    let button = egui::Button::new(label).fill(if on {
        Color32::from_rgb(170, 40, 30)
    } else {
        ui.visuals().widgets.inactive.weak_bg_fill
    });
    if ui.add(button).on_hover_text(t!("ep-rf-hint")).clicked() {
        commands.push(if on { EpCommand::RfOff } else { EpCommand::RfOn });
    }
    if let Some(r) = readout {
        ui.label(t!(
            "ep-rf-readout",
            elapsed = number(r.elapsed_ms / 1000.0, 1),
            temperature = number(r.temperature_c, 0),
            impedance = number(r.impedance_ohm, 0),
            radius = number(f64::from(r.lesion_radius_mm), 1)
        ));
        if r.contact <= 0.0 {
            ui.colored_label(Color32::LIGHT_RED, t!("ep-rf-no-contact"));
        }
    }
    ui.label(t!(
        "ep-lesions",
        count = i64::try_from(lesions).unwrap_or(i64::MAX)
    ));
    if let Some(goal) = goal_text {
        ui.horizontal_wrapped(|ui| {
            ui.label(goal);
            if goal_reached {
                ui.colored_label(Color32::from_rgb(60, 200, 90), t!("ep-goal-reached"));
            }
        });
    }
}
