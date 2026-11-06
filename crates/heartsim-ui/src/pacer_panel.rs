//! Temporary external pacemaker panel (brief B13): mode knob, rate, outputs, sensitivities,
//! AV delay, upper rate, PVARP, PAUSE / ASYNC buttons, lead sites, implanted-device presets and
//! the sense / pace / battery LEDs.
//!
//! The widget only edits a [`PacerPanelState`]; the app copies `state.device()` into the timing
//! engine's `TimingParams::pacer` when [`PacerPanelResponse::changed`] is set, and feeds the
//! engine's marker channel back with [`PacerPanelState::on_markers`]. Nothing here decides when a
//! pace happens, so the panel, the strip spikes ([`PaceMarks`]) and the heart's lead markers
//! ([`lead_glyphs`]) all show the timing engine's markers.

use crate::t;
use egui::{Color32, Painter, Pos2, Stroke, Ui, vec2};
use glam::Vec3;
use heartsim_render::GlyphInstance;
use heartsim_sim::timing::{
    LeadParams, LeadSite, MarkerKind, PacerChannel, PacerConfig, PacerMarker, PacerMode, PacerPreset,
};
use std::collections::VecDeque;

/// How long an LED stays lit after a sense or pace marker, ms of simulated time.
pub const LED_FLASH_MS: f64 = 120.0;
/// Pace marks older than this many samples are forgotten (the strip ring holds 60 s at 500 Hz).
const MARK_HISTORY: usize = 512;

/// Ranges of the temporary pacemaker's controls.
pub const RATE_RANGE_PPM: std::ops::RangeInclusive<f64> = 30.0..=180.0;
pub const OUTPUT_RANGE_MA: std::ops::RangeInclusive<f64> = 0.1..=20.0;
pub const SENSITIVITY_RANGE_MV: std::ops::RangeInclusive<f64> = 0.5..=20.0;

/// Everything the pacer panel edits, plus the LED state.
#[derive(Debug, Clone, PartialEq)]
pub struct PacerPanelState {
    /// Pacing wires connected (the device exists in `TimingParams::pacer`).
    pub connected: bool,
    pub config: PacerConfig,
    /// Simulation time of the last sense / pace marker per channel, ms.
    last_sense_ms: [f64; 3],
    last_pace_ms: [f64; 3],
    /// Simulation time of the newest markers fed in, ms (LED clock).
    now_ms: f64,
}

impl Default for PacerPanelState {
    fn default() -> Self {
        Self::new(None)
    }
}

/// What changed this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PacerPanelResponse {
    /// The device settings (or `connected`) changed: copy [`PacerPanelState::device`] into the
    /// timing engine.
    pub changed: bool,
    /// A lead moved to another site: re-place the leads on the mesh as well.
    pub leads_moved: bool,
}

impl PacerPanelState {
    /// Panel for an existing device configuration (`None` = no device, VVI 60 ready to connect).
    #[must_use]
    pub fn new(device: Option<&PacerConfig>) -> Self {
        Self {
            connected: device.is_some(),
            config: device
                .cloned()
                .unwrap_or_else(|| PacerConfig::preset(PacerPreset::Vvi60)),
            last_sense_ms: [f64::NEG_INFINITY; 3],
            last_pace_ms: [f64::NEG_INFINITY; 3],
            now_ms: 0.0,
        }
    }

    /// The device the timing engine should run (`None` = disconnected).
    #[must_use]
    pub fn device(&self) -> Option<PacerConfig> {
        self.connected.then(|| self.config.clone())
    }

    /// Feed the engine's marker channel; `now_ms` is the engine time after the step.
    pub fn on_markers(&mut self, markers: &[PacerMarker], now_ms: f64) {
        for m in markers {
            let i = m.channel as usize;
            match m.kind {
                MarkerKind::Pace { .. } => self.last_pace_ms[i] = m.at.0,
                MarkerKind::Sense => self.last_sense_ms[i] = m.at.0,
                MarkerKind::RefractorySense => {}
            }
        }
        self.now_ms = now_ms;
    }

    /// The pace LED of `channel` is lit (a stimulus within [`LED_FLASH_MS`]).
    #[must_use]
    pub fn pace_lit(&self, channel: PacerChannel) -> bool {
        self.now_ms - self.last_pace_ms[channel as usize] < LED_FLASH_MS
    }

    /// The sense LED of `channel` is lit.
    #[must_use]
    pub fn sense_lit(&self, channel: PacerChannel) -> bool {
        self.now_ms - self.last_sense_ms[channel as usize] < LED_FLASH_MS
    }
}

fn led(ui: &mut Ui, on: bool, colour: Color32, label: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
        let fill = if on { colour } else { colour.gamma_multiply(0.18) };
        ui.painter()
            .circle(rect.center(), 5.5, fill, Stroke::new(1.0, Color32::DARK_GRAY));
        ui.label(label);
    });
}

const PACE_COLOUR: Color32 = Color32::from_rgb(255, 170, 30);
const SENSE_COLOUR: Color32 = Color32::from_rgb(60, 200, 90);

fn slider(ui: &mut Ui, value: &mut f64, range: std::ops::RangeInclusive<f64>, text: &str) -> bool {
    ui.add(egui::Slider::new(value, range).text(text)).changed()
}

fn site_combo(ui: &mut Ui, label: &str, lead: &mut LeadParams, sites: &[LeadSite]) -> bool {
    let mut moved = false;
    egui::ComboBox::new(label, label)
        .selected_text(crate::labels::lead_site_name(lead.site))
        .show_ui(ui, |ui| {
            for s in sites {
                moved |= ui
                    .selectable_value(&mut lead.site, *s, crate::labels::lead_site_name(*s))
                    .changed();
            }
        });
    moved
}

/// Draw the panel.
pub fn pacer_panel(ui: &mut Ui, state: &mut PacerPanelState) -> PacerPanelResponse {
    let mut r = PacerPanelResponse::default();
    ui.horizontal(|ui| {
        ui.strong(t!("pacer-title"));
        r.changed |= ui.checkbox(&mut state.connected, t!("pacer-connected")).changed();
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(t!("pacer-preset"));
        for p in PacerPreset::ALL {
            if ui.button(p.label()).clicked() {
                state.config = PacerConfig::preset(p);
                state.connected = true;
                r.changed = true;
                r.leads_moved = true;
            }
        }
    });
    ui.add_enabled_ui(state.connected, |ui| {
        let c = &mut state.config;
        ui.horizontal_wrapped(|ui| {
            ui.label(t!("pacer-mode"));
            for m in PacerMode::ALL {
                r.changed |= ui.selectable_value(&mut c.mode, m, m.label()).changed();
            }
        });
        ui.horizontal(|ui| {
            r.changed |= ui.toggle_value(&mut c.paused, t!("pacer-pause")).changed();
            r.changed |= ui
                .toggle_value(&mut c.asynchronous, t!("pacer-async"))
                .on_hover_text(t!("pacer-async-hint"))
                .changed();
        });
        r.changed |= slider(ui, &mut c.lower_rate_bpm, RATE_RANGE_PPM, &t!("pacer-rate"));
        ui.separator();
        ui.label(t!("pacer-atrium"));
        r.changed |= slider(
            ui,
            &mut c.atrial_output_ma,
            OUTPUT_RANGE_MA,
            &t!("pacer-a-output"),
        );
        r.changed |= slider(
            ui,
            &mut c.atrial_sensitivity_mv,
            SENSITIVITY_RANGE_MV,
            &t!("pacer-a-sensitivity"),
        );
        ui.label(t!("pacer-ventricle"));
        r.changed |= slider(
            ui,
            &mut c.ventricular_output_ma,
            OUTPUT_RANGE_MA,
            &t!("pacer-v-output"),
        );
        r.changed |= slider(
            ui,
            &mut c.ventricular_sensitivity_mv,
            SENSITIVITY_RANGE_MV,
            &t!("pacer-v-sensitivity"),
        );
        ui.separator();
        r.changed |= slider(ui, &mut c.av_delay_ms, 50.0..=300.0, &t!("pacer-av-delay"));
        r.changed |= slider(ui, &mut c.upper_rate_bpm, 80.0..=180.0, &t!("pacer-upper-rate"));
        r.changed |= slider(ui, &mut c.pvarp_ms, 150.0..=500.0, &t!("pacer-pvarp"));
        ui.separator();
        ui.label(t!("pacer-leads"));
        let mut moved = false;
        moved |= site_combo(
            ui,
            &t!("pacer-a-lead"),
            &mut c.atrial_lead,
            &[LeadSite::RaAppendage],
        );
        moved |= site_combo(
            ui,
            &t!("pacer-v-lead"),
            &mut c.ventricular_lead,
            &[LeadSite::RvApex, LeadSite::RvSeptum, LeadSite::HisBundle],
        );
        let mut biv = c.lv_lead.is_some();
        if ui.checkbox(&mut biv, t!("pacer-lv-lead-biv")).changed() {
            c.lv_lead = biv.then(|| LeadParams::at(LeadSite::LvLateral));
            moved = true;
        }
        if let Some(lv) = &mut c.lv_lead {
            moved |= site_combo(
                ui,
                &t!("pacer-lv-lead"),
                lv,
                &[LeadSite::LvLateral, LeadSite::LvPosterior],
            );
            r.changed |= slider(ui, &mut c.vv_delay_ms, -80.0..=80.0, &t!("pacer-vv-offset"));
        }
        r.changed |= moved;
        r.leads_moved |= moved;
        ui.label(t!(
            "pacer-thresholds",
            atrial = crate::i18n::number(c.atrial_lead.threshold_at(state.now_ms), 1),
            ventricular = crate::i18n::number(c.ventricular_lead.threshold_at(state.now_ms), 1)
        ));
    });
    ui.separator();
    ui.horizontal(|ui| {
        for ch in [PacerChannel::Atrial, PacerChannel::Ventricular] {
            let l = ch.letter();
            led(
                ui,
                state.sense_lit(ch),
                SENSE_COLOUR,
                &t!("pacer-sense", channel = l.to_string()),
            );
            led(
                ui,
                state.pace_lit(ch),
                PACE_COLOUR,
                &t!("pacer-pace", channel = l.to_string()),
            );
        }
        led(ui, state.connected, SENSE_COLOUR, &t!("pacer-battery"));
    });
    r
}

/// One pacing spike on the strip time base.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaceMark {
    /// Absolute index of the first strip sample taken at or after the stimulus.
    pub sample: u64,
    pub channel: PacerChannel,
    pub captured: bool,
}

/// Pacing spikes in strip-sample coordinates, so they can be drawn as short vertical lines on
/// the strip whatever the paper speed (a < 2 ms stimulus is otherwise one sample wide).
#[derive(Debug, Clone, Default)]
pub struct PaceMarks {
    marks: VecDeque<PaceMark>,
}

impl PaceMarks {
    /// Record the pace markers of one engine step. `next_sample` is the absolute index the next
    /// strip sample will get (`LeadRing::pushed()` before this step's samples are pushed): the
    /// engine paces at the step's own time, so that sample is the first one at or after the
    /// stimulus, at most one sample period late.
    pub fn record(&mut self, markers: &[PacerMarker], next_sample: u64) {
        for m in markers {
            if let MarkerKind::Pace { captured } = m.kind {
                self.marks.push_back(PaceMark {
                    sample: next_sample,
                    channel: m.channel,
                    captured,
                });
            }
        }
        while self.marks.len() > MARK_HISTORY {
            self.marks.pop_front();
        }
    }

    /// Marks, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &PaceMark> {
        self.marks.iter()
    }

    /// Forget every mark.
    pub fn clear(&mut self) {
        self.marks.clear();
    }

    /// Draw the marks visible in `window` as short vertical lines above the baseline, labelled
    /// AP / VP; a dashed-looking dim line for a stimulus that did not capture.
    pub fn draw(&self, painter: &Painter, window: &crate::StripWindow) {
        let top = window.y_of_mv(1.6);
        let bottom = window.y_of_mv(0.3);
        for m in &self.marks {
            let Some(x) = window.x_of_sample(m.sample) else {
                continue;
            };
            let colour = if m.captured {
                PACE_COLOUR
            } else {
                PACE_COLOUR.gamma_multiply(0.45)
            };
            painter.line_segment(
                [Pos2::new(x, top), Pos2::new(x, bottom)],
                Stroke::new(1.5, colour),
            );
            painter.text(
                Pos2::new(x, top),
                egui::Align2::CENTER_BOTTOM,
                format!("{}P", m.channel.letter()),
                egui::FontId::proportional(9.0),
                colour,
            );
        }
    }
}

/// Lead-tip markers for the heart view: a sphere per lead at its tip position, orange while it
/// paces, green while it senses, grey otherwise. `tips[channel]` is the tip in heart mm.
#[must_use]
pub fn lead_glyphs(tips: [Option<Vec3>; 3], state: &PacerPanelState) -> Vec<GlyphInstance> {
    PacerChannel::ALL
        .iter()
        .filter_map(|&ch| {
            let p = tips[ch as usize]?;
            // linear RGB of PACE_COLOUR / SENSE_COLOUR
            let colour = if state.pace_lit(ch) {
                [1.0, 0.4, 0.012, 1.0]
            } else if state.sense_lit(ch) {
                [0.045, 0.58, 0.1, 1.0]
            } else {
                [0.75, 0.75, 0.8, 1.0]
            };
            Some(GlyphInstance::sphere(p, 3.0, colour))
        })
        .collect()
}
