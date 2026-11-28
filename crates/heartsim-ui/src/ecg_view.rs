//! The ECG display state the app holds once: paper style, pause/scrub, calipers, wave labels and
//! the interval read-out, shared by the single-lead strip and the 12-lead page.

use crate::annotations::{EcgAnnotations, interval_readout};
use crate::calipers::{CaliperSet, Playback, draw_calipers, strip_interact};
use crate::pcg_lane::{
    HeartSoundStatus, LaneRequest, PcgLane, chest_name, draw_caliper_lines, draw_pcg_lane, draw_playhead,
    lane_height, lane_title,
};
use crate::strip::{GAINS_MM_MV, SPEEDS_MM_S, StripStyle, StripTrace, StripWindow, SweepMode, draw_strip};
use crate::t;
use crate::twelve_lead::{LeadOrder, TwelveLeadLayout, cell_is_annotated, intervals_fit_cells, page_cells};
use crate::zoom::{clamp_time_zoom, head_keeping, rebase_caliper_ms, sample_pos_at_x, strip_zoom_factor};
use egui::{Rect, Ui};
use heartsim_core::Lead;
use heartsim_ecg::artefact::INTENSITY_RANGE;
use heartsim_ecg::{
    Analysis, Analyzer, ChestPiece, DisplayConfig, DisplayStage, FilterMode, LeadRing, ListeningChoice,
    ListeningSite, MainsFrequency,
};

/// Which annotations a strip may carry (before the Display toggles are applied).
#[derive(Debug, Clone, Copy)]
struct Annotate {
    waves: bool,
    intervals: bool,
}

impl Annotate {
    const ALL: Self = Self {
        waves: true,
        intervals: true,
    };
}

/// How often (in signal time) the wave labels and intervals are re-measured, ms.
pub const ANALYSIS_PERIOD_MS: f64 = 250.0;

/// ECG display state. Rings are passed in on every call (they belong to the app's model).
#[derive(Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent display switches of the Display popup"
)]
pub struct EcgView {
    /// The paper style the learner chose: `style.mm_per_s` is the preset speed, which the exports,
    /// the printed page and the interval drill keep using. What the strips are drawn at is
    /// [`Self::effective_style`].
    pub style: StripStyle,
    /// Time zoom of the strips (1 = the chosen paper speed), a drawing matter only: the strips
    /// are drawn at `style.mm_per_s × time_zoom`. Not persisted. See [`Self::time_zoom`].
    time_zoom: f32,
    /// Whether a strip takes the plain mouse wheel as time zoom. True where the strips sit
    /// outside any scroll area (every strip of the app today: the Simulator, Echo and Heart Vector
    /// bottom strips and the 12-lead page); a host that puts a strip inside a scroll area sets it
    /// false so the wheel keeps scrolling there (pinch and Ctrl/⌘ + wheel still zoom).
    pub plain_wheel_zoom: bool,
    pub playback: Playback,
    pub order: LeadOrder,
    /// Draw P/R/T letters.
    pub show_waves: bool,
    /// Draw PR and RR brackets.
    pub show_intervals: bool,
    /// Draw the heart-sound lane (B24) under the strip and under the 12-lead page, where the caller
    /// has a phonocardiogram. Off by default.
    pub show_heart_sounds: bool,
    /// The Display popup's "Play heart sounds" switch (B25). `None` in builds without audio output
    /// (the web build): the switch is absent.
    pub play_heart_sounds: Option<bool>,
    /// What the heart-sound audio is doing, worded under the switch.
    pub heart_sound_status: HeartSoundStatus,
    /// While a slice of the PCG ring plays: the ring index heard now, drawn as a line across the
    /// strip and the lane.
    pub playhead: Option<u64>,
    /// A slice is playing: the lane's button offers Stop.
    pub slice_playing: bool,
    /// Where on the chest and with which chest piece the lane and the sound are heard (B27 part B):
    /// chosen in the Display popup, named in the lane's title. The overview by default.
    pub listening: ListeningChoice,
    /// Opacity of the torso picture behind the chest's site markers (the Settings value).
    pub chest_image_opacity: f32,
    /// What the lane says when the sound played differs from the trace drawn (B28: small-speaker
    /// mode, slow motion): an "altered sound" tag at its bottom right. `None`: the sound is the trace.
    pub sound_tag: Option<String>,
    /// The breath-hold now in force in the engine (B28), shown by the Display popup's breathing
    /// choice. The app sets it every frame from the engine.
    pub breath_hold: Option<heartsim_core::BreathPhase>,
    /// The learner picked another breathing choice this frame (`Some(None)`: breathe again).
    #[allow(
        clippy::option_option,
        reason = "outer: a request was made; inner: the breath-hold or none"
    )]
    breath_request: Option<Option<heartsim_core::BreathPhase>>,
    /// Show the breathing choice in the Display popup: set by the app where a timing engine runs (the
    /// Tier-0 simulator, not the EP lab); off for a view without one.
    pub breath_hold_available: bool,
    /// How far behind the newest sample the views are drawn while sound plays, ms (B25). Not a
    /// scrub: the scrub slider and [`Self::scrub_offset_ms`] do not include it.
    display_delay_ms: f64,
    /// The lane's Play / Stop button was clicked this frame.
    lane_request: Option<LaneRequest>,
    /// Bandwidth filter and artefacts applied to what the painters draw (F-15). The analyser, the
    /// calipers' reading of wave positions and the default exports keep using the raw rings.
    display: DisplayStage,
    analyzer: Analyzer,
    annotations: Option<EcgAnnotations>,
    strip_calipers: CaliperSet,
    page_calipers: Vec<CaliperSet>,
    /// Calipers of the 12-lead page's heart-sound lane when its last cell does not span the page.
    page_sound_calipers: CaliperSet,
    page_layout: Option<TwelveLeadLayout>,
    /// Where the 12-lead page was last drawn, screen points (snapshot size).
    page_rect: Option<Rect>,
    /// Result of the last snapshot export, for the status line.
    pub last_export: Option<String>,
    /// The Display popup is to open on the next frame.
    open_display_popup: bool,
}

impl Default for EcgView {
    fn default() -> Self {
        Self {
            style: StripStyle::default(),
            time_zoom: 1.0,
            plain_wheel_zoom: true,
            playback: Playback::default(),
            order: LeadOrder::Standard,
            show_waves: true,
            show_intervals: true,
            show_heart_sounds: false,
            play_heart_sounds: None,
            heart_sound_status: HeartSoundStatus::Off,
            playhead: None,
            slice_playing: false,
            listening: ListeningChoice::default(),
            chest_image_opacity: 0.7,
            sound_tag: None,
            breath_hold: None,
            breath_request: None,
            breath_hold_available: false,
            display_delay_ms: 0.0,
            lane_request: None,
            display: DisplayStage::default(),
            analyzer: Analyzer::default(),
            annotations: None,
            strip_calipers: CaliperSet::default(),
            page_calipers: Vec::new(),
            page_sound_calipers: CaliperSet::default(),
            page_layout: None,
            page_rect: None,
            last_export: None,
            open_display_popup: false,
        }
    }
}

fn fs_of(rings: &[LeadRing]) -> f32 {
    rings.first().map_or(500.0, |r| r.sample_rate_hz)
}

fn pushed_of(rings: &[LeadRing]) -> u64 {
    rings.first().map_or(0, LeadRing::pushed)
}

/// PCG samples per strip sample for `rings` (8 for the 500 Hz strip and the 4 kHz PCG).
fn pcg_factor(rings: &[LeadRing]) -> u64 {
    ((heartsim_ecg::PCG_RATE_HZ / fs_of(rings).max(1.0)).round() as u64).max(1)
}

impl EcgView {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the display (and so the simulation) is paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.playback.is_paused()
    }

    /// Pause, or resume from "now".
    pub fn toggle_pause(&mut self) {
        self.playback.toggle();
    }

    /// Time zoom of the strips: 1 draws them at the chosen paper speed, 2 at twice that speed.
    /// Always within the range that keeps the effective speed between 6.25 and 400 mm/s.
    #[must_use]
    pub fn time_zoom(&self) -> f32 {
        clamp_time_zoom(self.style.mm_per_s, self.time_zoom)
    }

    /// Sets the time zoom (clamped, see [`Self::time_zoom`]).
    pub fn set_time_zoom(&mut self, zoom: f32) {
        self.time_zoom = clamp_time_zoom(self.style.mm_per_s, zoom);
    }

    /// Chooses a preset paper speed. A preset is a fresh start: the time zoom goes back to 1.
    pub fn set_paper_speed(&mut self, mm_per_s: f32) {
        self.style.mm_per_s = mm_per_s;
        self.time_zoom = 1.0;
    }

    /// Back to the chosen paper speed.
    pub fn reset_time_zoom(&mut self) {
        self.time_zoom = 1.0;
    }

    /// The paper speed the strips are drawn at, mm/s: the preset times the time zoom.
    #[must_use]
    pub fn effective_mm_per_s(&self) -> f32 {
        self.style.mm_per_s * self.time_zoom()
    }

    /// [`Self::style`] at the effective paper speed: what every on-screen strip, its grid, its
    /// calipers and the heart-sound lane are drawn with. The exports use [`Self::style`] instead.
    #[must_use]
    pub fn effective_style(&self) -> StripStyle {
        StripStyle {
            mm_per_s: self.effective_mm_per_s(),
            ..self.style
        }
    }

    /// How far the scrub cursor is behind the newest sample, ms (0 when live). The 3-D view
    /// shows the state at `now − scrub_offset_ms()`.
    #[must_use]
    pub fn scrub_offset_ms(&self) -> f64 {
        self.playback.scrub_ms()
    }

    /// Draw everything `ms` behind the newest sample (the heart-sound player's display delay, B25),
    /// on top of any scrub. Calipers sit in screen coordinates and are unaffected.
    pub fn set_display_delay_ms(&mut self, ms: f64) {
        self.display_delay_ms = if ms.is_finite() { ms.max(0.0) } else { 0.0 };
    }

    /// The display delay in force, ms.
    #[must_use]
    pub fn display_delay_ms(&self) -> f64 {
        self.display_delay_ms
    }

    /// How far behind the engine the views are drawn, ms: the scrub plus the display delay. The 3-D
    /// view shows the state at `now − view_offset_ms()`.
    #[must_use]
    pub fn view_offset_ms(&self) -> f64 {
        self.playback.scrub_ms() + self.display_delay_ms
    }

    fn delay_samples(&self, fs: f32) -> u64 {
        (self.display_delay_ms * f64::from(fs) / 1000.0).round() as u64
    }

    /// Time of the newest sample on screen, ms since the first sample.
    #[must_use]
    pub fn cursor_time_ms(&self, rings: &[LeadRing]) -> f64 {
        let fs = fs_of(rings);
        self.head(rings).saturating_sub(1) as f64 * 1000.0 / f64::from(fs)
    }

    /// Absolute index one past the newest sample on screen.
    #[must_use]
    pub fn head(&self, rings: &[LeadRing]) -> u64 {
        let fs = fs_of(rings);
        self.playback
            .head(pushed_of(rings), fs)
            .saturating_sub(self.delay_samples(fs))
    }

    /// The Play / Stop request of the heart-sound lane's button this frame, if any.
    pub fn take_lane_request(&mut self) -> Option<LaneRequest> {
        self.lane_request.take()
    }

    /// Latest measurements, if the analyser has run.
    #[must_use]
    pub fn analysis(&self) -> Option<&Analysis> {
        self.annotations.as_ref().map(|a| &a.analysis)
    }

    /// Re-measure when the view head has moved by [`ANALYSIS_PERIOD_MS`] (or jumped backwards).
    /// Call once per frame before drawing.
    pub fn update(&mut self, rings: &[LeadRing]) {
        if rings.len() < heartsim_core::LEAD_COUNT {
            return;
        }
        // the display copies follow the raw rings; the analysis below reads the raw ones
        self.display.sync(rings);
        let head = self.head(rings);
        let period = (ANALYSIS_PERIOD_MS * f64::from(fs_of(rings)) / 1000.0) as u64;
        let due = self
            .annotations
            .as_ref()
            .is_none_or(|a| head < a.head || head - a.head >= period.max(1));
        if due {
            self.annotations = Some(EcgAnnotations::compute(&self.analyzer, rings, head));
        }
    }

    /// The rings now hold a different signal (a recording swapped in or out): forget what was
    /// measured on the old one. The analysis is otherwise only redone when the head moves, so a
    /// paused view would keep showing the old read-out.
    pub fn signal_replaced(&mut self) {
        self.annotations = None;
        self.display.invalidate();
        self.strip_calipers.reset();
        self.page_calipers.iter_mut().for_each(CaliperSet::reset);
        self.page_sound_calipers.reset();
    }

    /// The strip's one row of everyday controls: the lead selector (when `lead` is given), paper
    /// speed, gain, Scroll/Sweep, Calipers, the scrub slider while paused, and the Display button
    /// that opens the rest ([`Self::display_controls`]).
    pub fn controls(&mut self, ui: &mut Ui, rings: &[LeadRing], lead: Option<&mut Lead>) {
        self.controls_with(ui, rings, lead, |_, _| {});
    }

    /// [`Self::controls`] with `before` drawn at the start of the same wrapping row (the 12-lead
    /// page's layout choice and Save PNG), so the page needs one row of controls, not two. It gets
    /// the view too, for the settings it shares (the lead order).
    pub fn controls_with(
        &mut self,
        ui: &mut Ui,
        rings: &[LeadRing],
        lead: Option<&mut Lead>,
        before: impl FnOnce(&mut Ui, &mut Self),
    ) {
        if ui.ctx().content_rect().width() < crate::NARROW_PX {
            // phone width: one row, the lead as a combo box and everything else behind the Display
            // button, so the trace keeps the height of the panel
            ui.horizontal_wrapped(|ui| {
                if let Some(selected) = lead {
                    let combo = egui::ComboBox::from_id_salt("ecg_lead_narrow")
                        .selected_text(selected.label())
                        .show_ui(ui, |ui| {
                            for l in Lead::ALL {
                                ui.selectable_value(selected, l, l.label());
                            }
                        });
                    name_combo(&combo.response, &t!("ecg-lead"));
                    crate::highlight::mark(&combo.response, "lead_selector");
                }
                let mut before = Some(before);
                self.display_button_with(ui, |ecg, ui| {
                    if let Some(before) = before.take() {
                        before(ui, ecg);
                    }
                    ecg.trace_controls(ui, rings);
                    ui.separator();
                });
            });
            return;
        }
        ui.horizontal_wrapped(|ui| {
            before(ui, self);
            if let Some(selected) = lead {
                let leads = ui
                    .horizontal_wrapped(|ui| {
                        for l in Lead::ALL {
                            ui.selectable_value(selected, l, l.label());
                        }
                    })
                    .response;
                crate::highlight::mark(&leads, "lead_selector");
                ui.separator();
            }
            self.trace_controls(ui, rings);
            self.display_button(ui);
        });
    }

    /// Paper speed, gain, Scroll / Sweep, Clear calipers and, while paused, the scrub slider.
    fn trace_controls(&mut self, ui: &mut Ui, rings: &[LeadRing]) {
        let zoomed = (self.time_zoom() - 1.0).abs() > 1e-3;
        let speed_text = if zoomed {
            speed_label(self.effective_mm_per_s())
        } else {
            speed_label(self.style.mm_per_s)
        };
        let speed = egui::ComboBox::from_id_salt("ecg_speed")
            .selected_text(speed_text)
            .show_ui(ui, |ui| {
                for s in SPEEDS_MM_S {
                    let chosen = !zoomed && (self.style.mm_per_s - s).abs() < 1e-3;
                    if ui.selectable_label(chosen, speed_label(s)).clicked() {
                        self.set_paper_speed(s);
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(zoomed, egui::Button::new(t!("ecg-speed-reset")))
                    .clicked()
                {
                    self.reset_time_zoom();
                }
            });
        name_combo(&speed.response, &t!("ecg-speed"));
        speed.response.on_hover_text(t!("ecg-speed-hint"));
        let gain = egui::ComboBox::from_id_salt("ecg_gain")
            .selected_text(format!("{} mm/mV", self.style.mm_per_mv))
            .show_ui(ui, |ui| {
                for g in GAINS_MM_MV {
                    ui.selectable_value(&mut self.style.mm_per_mv, g, format!("{g} mm/mV"));
                }
            });
        name_combo(&gain.response, &t!("ecg-gain"));
        gain.response.on_hover_text(t!("ecg-gain"));
        for m in [SweepMode::Scroll, SweepMode::Sweep] {
            ui.selectable_value(&mut self.style.mode, m, m.label());
        }
        // never squeezed to fit: on a narrow window the row wraps before it instead
        if ui
            .add(egui::Button::new(t!("ecg-calipers")).wrap_mode(egui::TextWrapMode::Extend))
            .on_hover_text(t!("ecg-calipers-hint"))
            .clicked()
        {
            self.strip_calipers.reset();
            self.page_calipers.iter_mut().for_each(CaliperSet::reset);
            self.page_sound_calipers.reset();
        }
        if self.playback.is_paused() {
            let fs = fs_of(rings);
            let visible = rings
                .first()
                .map_or(0, |r| r.len() as u64)
                .min((10.0 * fs) as u64);
            let max = rings.first().map_or(0.0, |r| Playback::max_scrub_ms(r, visible));
            let mut back_s = self.playback.scrub_ms() / 1000.0;
            if crate::controls::slider(
                ui,
                &t!("ecg-scrub-back"),
                egui::Slider::new(&mut back_s, 0.0..=(max / 1000.0).max(0.0)).max_decimals(1),
                crate::controls::S,
            )
            .changed()
            {
                self.playback.set_scrub_ms(back_s * 1000.0, max);
            }
        }
    }

    /// The Display button: opens the popup with the paper, label, filter and artefact controls. It
    /// is drawn as selected, and says what is on, while a filter or an artefact changes the trace,
    /// because those change what the learner sees.
    fn display_button(&mut self, ui: &mut Ui) {
        self.display_button_with(ui, |_, _| {});
    }

    /// [`Self::display_button`] with `extras` drawn at the top of its popup (the controls the
    /// phone-width toolbar folds into it).
    fn display_button_with(&mut self, ui: &mut Ui, extras: impl FnOnce(&mut Self, &mut Ui)) {
        let cfg = *self.display.config();
        let mut parts = Vec::new();
        if cfg.filter != FilterMode::Off {
            parts.push(filter_name(cfg.filter));
        }
        let artefacts = [
            cfg.artefacts.baseline_wander,
            cfg.artefacts.respiration,
            cfg.artefacts.mains,
            cfg.artefacts.muscle,
        ]
        .into_iter()
        .filter(|on| *on)
        .count();
        if artefacts > 0 {
            parts.push(t!(
                "ecg-display-artefacts",
                count = i64::try_from(artefacts).unwrap_or_default()
            ));
        }
        let active = !parts.is_empty();
        let label = if active {
            t!("ecg-display-active", what = parts.join(", "))
        } else {
            t!("ecg-display")
        };
        let response = ui.add(egui::Button::selectable(active, label));
        let response = response.on_hover_text(t!("ecg-display-hint"));
        let mut popup = egui::Popup::from_toggle_button_response(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        if std::mem::take(&mut self.open_display_popup) {
            popup = popup.open_memory(egui::SetOpenCommand::Bool(true));
        }
        popup.show(|ui| {
            ui.set_min_width(300.0);
            extras(self, ui);
            self.display_controls(ui);
        });
    }

    /// Everything the Display popup holds: monitor colours, grid, wave letters, PR/RR brackets and
    /// the screen scale, then the display filter and the artefacts (F-15). Only what is drawn
    /// changes.
    pub fn display_controls(&mut self, ui: &mut Ui) {
        let mut dark = self.style.is_dark();
        if ui.checkbox(&mut dark, t!("ecg-monitor-colours")).changed() {
            self.style = if dark {
                self.style.monitor()
            } else {
                self.style.with_paper_colours()
            };
        }
        ui.checkbox(&mut self.style.show_grid, t!("ecg-grid"));
        ui.checkbox(&mut self.show_waves, t!("ecg-waves"));
        ui.checkbox(&mut self.show_intervals, t!("ecg-intervals"));
        ui.checkbox(&mut self.show_heart_sounds, t!("ecg-heart-sounds"))
            .on_hover_text(t!("ecg-heart-sounds-hint"));
        if let Some(play) = &mut self.play_heart_sounds {
            let status = self.heart_sound_status;
            let mut shown = false;
            ui.indent("play_heart_sounds", |ui| {
                // switching the sound on shows the lane; hiding the lane leaves the sound on
                shown = ui
                    .checkbox(play, t!("ecg-play-heart-sounds"))
                    .on_hover_text(t!("ecg-play-heart-sounds-hint"))
                    .changed()
                    && *play;
                if *play && let Some(note) = status.note() {
                    crate::helper_text(ui, note);
                }
            });
            if shown {
                self.show_heart_sounds = true;
            }
        }
        ui.indent("listening_site", |ui| self.listening_controls(ui));
        if self.breath_hold_available {
            ui.indent("breath_hold", |ui| self.breathing_controls(ui));
        }
        crate::controls::slider(
            ui,
            &t!("ecg-px-per-mm"),
            egui::Slider::new(&mut self.style.px_per_mm, 1.5..=8.0).max_decimals(2),
            crate::controls::PT_PER_MM,
        )
        .on_hover_text(t!("ecg-calibration-hint"));
        ui.separator();
        let mut cfg = *self.display.config();
        ui.strong(t!("ecg-filter"));
        for m in FilterMode::ALL {
            ui.radio_value(&mut cfg.filter, m, filter_name(m))
                .on_hover_text(t!("ecg-filter-hint"));
        }
        ui.separator();
        ui.strong(t!("ecg-artefacts"));
        let a = &mut cfg.artefacts;
        ui.checkbox(&mut a.baseline_wander, t!("ecg-art-wander"));
        ui.checkbox(&mut a.respiration, t!("ecg-art-respiration"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut a.mains, t!("ecg-art-mains"));
            if a.mains {
                for hz in MainsFrequency::ALL {
                    ui.selectable_value(&mut a.mains_hz, hz, format!("{:.0} Hz", hz.hz()));
                }
            }
        });
        ui.checkbox(&mut a.muscle, t!("ecg-art-muscle"));
        if a.any() {
            crate::controls::slider(
                ui,
                &t!("ecg-art-intensity"),
                egui::Slider::new(&mut a.intensity, INTENSITY_RANGE.0..=INTENSITY_RANGE.1).max_decimals(2),
                "",
            );
        }
        self.display.set_config(cfg);
    }

    /// The listening site (the overview and the five sites, as radio buttons beside the chest with
    /// its clickable markers) and the chest piece, which the overview does not use (B27 part B).
    pub fn listening_controls(&mut self, ui: &mut Ui) {
        ui.label(t!("ecg-listening-site"))
            .on_hover_text(t!("ecg-listening-site-hint"));
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                for site in ListeningSite::ALL {
                    ui.radio_value(
                        &mut self.listening.site,
                        site,
                        crate::auscultation_chest::site_name(site),
                    );
                }
            });
            let opacity = self.chest_image_opacity;
            crate::auscultation_chest::auscultation_chest(
                ui,
                egui::vec2(118.0, 130.0),
                &mut self.listening.site,
                opacity,
            );
        });
        ui.add_enabled_ui(!self.listening.is_overview(), |ui| {
            ui.horizontal(|ui| {
                ui.label(t!("ecg-chest-piece"));
                for chest in ChestPiece::ALL {
                    ui.radio_value(&mut self.listening.chest, chest, chest_name(chest));
                }
            })
            .response
            .on_hover_text(t!("ecg-chest-piece-hint"))
            .on_disabled_hover_text(t!("ecg-chest-piece-overview-hint"));
        });
    }

    /// The breathing choice (B28): breathe, or hold the breath at end-expiration or at peak
    /// inspiration, so that a split can be listened to beat after beat. The app turns a change into
    /// an `Action::BreathHold` for the engine ([`Self::take_breath_request`]).
    pub fn breathing_controls(&mut self, ui: &mut Ui) {
        use heartsim_core::BreathPhase;
        ui.label(t!("ecg-breathing"))
            .on_hover_text(t!("ecg-breathing-hint"));
        let mut choice = self.breath_hold;
        for (value, label) in [
            (None, t!("ecg-breathing-free")),
            (
                Some(BreathPhase::EndExpiration),
                t!("ecg-breathing-hold-expiration"),
            ),
            (
                Some(BreathPhase::PeakInspiration),
                t!("ecg-breathing-hold-inspiration"),
            ),
        ] {
            ui.radio_value(&mut choice, value, label);
        }
        if choice != self.breath_hold {
            self.breath_request = Some(choice);
            self.breath_hold = choice;
        }
    }

    /// The breathing choice the learner made this frame, if any (see [`Self::breathing_controls`]).
    #[allow(
        clippy::option_option,
        reason = "outer: a request was made; inner: the breath-hold or none"
    )]
    pub fn take_breath_request(&mut self) -> Option<Option<heartsim_core::BreathPhase>> {
        self.breath_request.take()
    }

    /// Opens the Display popup on the next frame (the documentation tour's shots).
    pub fn request_display_popup(&mut self) {
        self.open_display_popup = true;
    }

    /// The rings the painters (and a "save what is shown" export) read: `raw` filtered and with the
    /// artefacts added, or `raw` itself when the display is untouched.
    #[must_use]
    pub fn display_rings<'a>(&'a self, raw: &'a [LeadRing]) -> &'a [LeadRing] {
        self.display.rings(raw)
    }

    /// The display filter and artefacts now in effect.
    #[must_use]
    pub fn display_config(&self) -> &DisplayConfig {
        self.display.config()
    }

    /// Replace the display filter and artefacts (tests and scripted tours).
    pub fn set_display_config(&mut self, config: DisplayConfig) {
        self.display.set_config(config);
    }

    /// Moves the scrub cursor `ms` further back in time (negative = forward) while paused; the
    /// keyboard ←/→ shortcuts use it. Does nothing while the simulation runs.
    pub fn scrub_back_by(&mut self, ms: f64, rings: &[LeadRing]) {
        if !self.playback.is_paused() {
            return;
        }
        let fs = fs_of(rings);
        let visible = rings
            .first()
            .map_or(0, |r| r.len() as u64)
            .min((10.0 * fs) as u64);
        let max = rings.first().map_or(0.0, |r| Playback::max_scrub_ms(r, visible));
        let next = (self.playback.scrub_ms() + ms).clamp(0.0, max.max(0.0));
        self.playback.set_scrub_ms(next, max);
    }

    /// Interval read-out panel (HR, PR, QRS, QT, QTc, axis).
    pub fn readout(&self, ui: &mut Ui) {
        interval_readout(ui, self.analysis());
    }

    fn trace<'a>(
        &'a self,
        lead: Lead,
        label: &'a str,
        invert: bool,
        head: u64,
        annotate: Annotate,
    ) -> StripTrace<'a> {
        StripTrace {
            label,
            invert,
            head,
            marks: self.annotations.as_ref().map(|a| a.lead(lead)),
            show_waves: annotate.waves && self.show_waves,
            show_intervals: annotate.intervals && self.show_intervals,
        }
    }

    /// One interactive strip of `lead` filling `rect`. Returns its window.
    pub fn strip(&mut self, ui: &mut Ui, rect: Rect, rings: &[LeadRing], lead: Lead) -> StripWindow {
        self.strip_with_sounds(ui, rect, rings, lead, None)
    }

    /// [`Self::strip`] with the heart-sound lane under it when [`Self::show_heart_sounds`] is on and
    /// `sounds` runs in step with the rings ([`PcgLane::factor`]); otherwise exactly [`Self::strip`].
    /// The lane shares the strip's time axis, scrub and calipers: a caliper can be dragged in either.
    pub fn strip_with_sounds(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        rings: &[LeadRing],
        lead: Lead,
        sounds: Option<PcgLane<'_>>,
    ) -> StripWindow {
        let lane = sounds
            .filter(|_| self.show_heart_sounds)
            .and_then(|l| Some((l, l.factor(rings.first()?)?)));
        let region = rect;
        let (rect, lane_rect) = match lane {
            Some(_) => {
                let h = lane_height(rect.height());
                let (top, bottom) = rect.split_top_bottom_at_y(rect.bottom() - h);
                (top, Some(bottom))
            }
            None => (rect, None),
        };
        let win = self.ecg_strip(ui, rect, region, rings, lead);
        let style = self.effective_style();
        let factor = pcg_factor(rings);
        if let Some(p) = self.playhead {
            draw_playhead(ui, &win, p, factor, &style);
        }
        if let (Some((lane, factor)), Some(lane_rect)) = (lane, lane_rect) {
            let id = ui.scope_id().with("ecg_view_sounds");
            let lane_win = win.rescaled(lane_rect, factor);
            draw_pcg_lane(ui, id, &lane_win, &lane, &style, &lane_title(self.listening));
            if let Some(tag) = &self.sound_tag {
                crate::clip_view::draw_altered_tag(ui, lane_rect, tag);
            }
            strip_interact(
                ui,
                id,
                &lane_win,
                lane.ring,
                &mut self.strip_calipers,
                &mut self.playback,
            );
            draw_caliper_lines(ui, &lane_win, &self.strip_calipers, &style);
            if let Some(p) = self.playhead {
                draw_playhead(ui, &lane_win, p, 1, &style);
            }
            let calipers = self.strip_calipers.clone();
            self.lane_button(ui, &lane_win, &lane, &calipers);
        }
        win
    }

    /// The heart-sound lane's Play / Stop button (B25), at its top right while paused with the
    /// sound switched on: Play plays the PCG ring from the scrub cursor to the newest sample (the
    /// strip on screen when the cursor is at the newest sample), or the span between the first
    /// caliper pair when one is placed; Stop ends it.
    fn lane_button(
        &mut self,
        ui: &mut Ui,
        lane_win: &StripWindow,
        lane: &PcgLane<'_>,
        calipers: &CaliperSet,
    ) {
        if self.play_heart_sounds == Some(true)
            && self.heart_sound_status == HeartSoundStatus::WaitingForClick
        {
            // B26: a browser plays sound only after a click or a key press; say so where the
            // learner looks for the sound
            ui.painter().text(
                lane_win.rect.right_top() + egui::vec2(-6.0, 4.0),
                egui::Align2::RIGHT_TOP,
                t!("ecg-sound-click-notice"),
                egui::FontId::proportional(12.0),
                self.style.trace,
            );
            return;
        }
        let available = !matches!(
            self.heart_sound_status,
            HeartSoundStatus::NoDevice | HeartSoundStatus::Unsupported
        );
        if self.play_heart_sounds != Some(true) || !self.playback.is_paused() || !available {
            return;
        }
        let label = if self.slice_playing {
            t!("ecg-sound-stop")
        } else {
            t!("ecg-sound-play")
        };
        let size = egui::vec2(64.0, 18.0);
        let at = Rect::from_min_size(lane_win.rect.right_top() + egui::vec2(-size.x - 4.0, 2.0), size);
        let hint = t!("ecg-sound-play-hint");
        let clicked = ui
            .put(at, egui::Button::new(egui::RichText::new(label).small()))
            .on_hover_text(hint)
            .clicked();
        if !clicked {
            return;
        }
        if self.slice_playing {
            self.lane_request = Some(LaneRequest::Stop);
            return;
        }
        let span = calipers.pairs.first().and_then(|c| {
            let a = lane_win.sample_at_x(lane_win.x_from_left_ms(c.a_ms.min(c.b_ms)))?;
            let b = lane_win.sample_at_x(lane_win.x_from_left_ms(c.a_ms.max(c.b_ms)))?;
            Some((a, b + 1))
        });
        let (start, end) = span.unwrap_or_else(|| {
            let newest = lane.ring.pushed();
            let cursor = lane_win.head.min(newest);
            if cursor < newest {
                (cursor, newest)
            } else {
                (cursor.saturating_sub(lane_win.visible_samples), cursor)
            }
        });
        if end > start {
            self.lane_request = Some(LaneRequest::Play { start, end });
        }
    }

    /// One strip in `rect`. `region` is where the zoom gestures count: `rect` plus the heart-sound
    /// lane under it.
    fn ecg_strip(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        region: Rect,
        rings: &[LeadRing],
        lead: Lead,
    ) -> StripWindow {
        let id = ui.scope_id().with("ecg_view_strip");
        if let Some(change) = self.apply_time_zoom(ui, region, &[(rect, 0)], rings) {
            change.rebase(&mut self.strip_calipers, rect, 0);
        }
        let style = self.effective_style();
        let head = self.head(rings);
        let rings = self.display.rings(rings);
        let ring = &rings[lead as usize];
        let win = draw_strip(
            ui,
            id,
            rect,
            ring,
            &style,
            &self.trace(lead, lead.label(), false, head, Annotate::ALL),
        );
        strip_interact(ui, id, &win, ring, &mut self.strip_calipers, &mut self.playback);
        draw_calipers(ui, &win, ring, false, &self.strip_calipers, &style);
        win
    }

    /// Reads the time-zoom gestures over `region` and applies them: the zoom factor changes (within
    /// its limits) and, while paused in scroll mode, the scrub offset moves so that the sample under
    /// the pointer (or between the two fingers) stays under it. Running, the newest sample stays at
    /// the right edge and the sweep page is recomputed from the new speed, by construction.
    /// `cells` are the strip rectangles of the view with the number of columns drawn before the
    /// head (0 for a strip; see `page_cells`). Returns what changed so the calipers can follow.
    fn apply_time_zoom(
        &mut self,
        ui: &mut Ui,
        region: Rect,
        cells: &[(Rect, u64)],
        rings: &[LeadRing],
    ) -> Option<ZoomChange> {
        let (factor, anchor_x) = strip_zoom_factor(ui, region, self.plain_wheel_zoom)?;
        let old = self.effective_style();
        let before = self.time_zoom();
        self.time_zoom = clamp_time_zoom(self.style.mm_per_s, before * factor);
        if (self.time_zoom - before).abs() < 1e-6 {
            return None;
        }
        let new = self.effective_style();
        let fs = fs_of(rings);
        let old_head = self.head(rings);
        let mut change = ZoomChange {
            old,
            new,
            fs,
            old_head,
            new_head: old_head,
            rebase: false,
        };
        if !self.playback.is_paused() || new.mode != SweepMode::Scroll {
            return Some(change);
        }
        let (rect, cols) = cells
            .iter()
            .copied()
            .find(|(r, _)| anchor_x >= r.left() && anchor_x <= r.right())
            .or_else(|| cells.last().copied())?;
        let old_win = change.window(rect, cols, &old, old_head);
        let pos = sample_pos_at_x(&old_win, anchor_x);
        let probe = StripWindow::new(rect, &new, fs, 0);
        let want_head = head_keeping(&probe, pos, anchor_x) + (cols * probe.visible_samples) as f64;
        let back_ms = (old_head as f64 - want_head) * 1000.0 / f64::from(fs.max(1.0));
        let max = rings
            .first()
            .map_or(0.0, |r| Playback::max_scrub_ms(r, probe.visible_samples));
        self.playback
            .set_scrub_ms(self.playback.scrub_ms() + back_ms, max);
        change.new_head = self.head(rings);
        change.rebase = true;
        Some(change)
    }

    /// The 12-lead page in `rect`, each cell with its own calipers. Returns the cell windows.
    pub fn page(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        rings: &[LeadRing],
        layout: TwelveLeadLayout,
    ) -> Vec<StripWindow> {
        self.page_with_sounds(ui, rect, rings, layout, None)
    }

    /// [`Self::page`] with the heart-sound lane along the bottom (under the rhythm strip of the
    /// clinical layout) when [`Self::show_heart_sounds`] is on and `sounds` runs in step with the
    /// rings. Where the page's last cell spans the full width (12 × 1, 3 × 4 + rhythm) the lane
    /// shares that cell's calipers.
    pub fn page_with_sounds(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        rings: &[LeadRing],
        layout: TwelveLeadLayout,
        sounds: Option<PcgLane<'_>>,
    ) -> Vec<StripWindow> {
        let lane = sounds
            .filter(|_| self.show_heart_sounds)
            .and_then(|l| Some((l, l.factor(rings.first()?)?)));
        let Some((lane, factor)) = lane else {
            return self.page_cells(ui, rect, rect, rings, layout);
        };
        let h = lane_height(rect.height() * 0.6);
        let (top, lane_rect) = rect.split_top_bottom_at_y(rect.bottom() - h);
        let wins = self.page_cells(ui, top, rect, rings, layout);
        let style = self.effective_style();
        let head = self.head(rings);
        let probe = StripWindow::new(lane_rect, &style, fs_of(rings), head);
        let lane_win = probe.rescaled(lane_rect, factor);
        let id = ui.scope_id().with("ecg_view_page_sounds");
        draw_pcg_lane(ui, id, &lane_win, &lane, &style, &lane_title(self.listening));
        if let Some(tag) = &self.sound_tag {
            crate::clip_view::draw_altered_tag(ui, lane_rect, tag);
        }
        let spans = |w: &StripWindow| {
            (w.rect.left() - lane_rect.left()).abs() < 0.5 && (w.rect.right() - lane_rect.right()).abs() < 0.5
        };
        let calipers = match (wins.last(), self.page_calipers.last_mut()) {
            (Some(w), Some(c)) if spans(w) && w.head == head => c,
            _ => &mut self.page_sound_calipers,
        };
        strip_interact(ui, id, &lane_win, lane.ring, calipers, &mut self.playback);
        draw_caliper_lines(ui, &lane_win, calipers, &style);
        let calipers = calipers.clone();
        if let Some(p) = self.playhead {
            draw_playhead(ui, &lane_win, p, 1, &style);
            if let Some(w) = wins.last().filter(|w| spans(w)) {
                draw_playhead(ui, w, p, factor, &style);
            }
        }
        self.lane_button(ui, &lane_win, &lane, &calipers);
        wins
    }

    /// The page's cells in `rect`; `region` (`rect` plus the heart-sound lane) is where the zoom
    /// gestures count.
    fn page_cells(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        region: Rect,
        rings: &[LeadRing],
        layout: TwelveLeadLayout,
    ) -> Vec<StripWindow> {
        let cells = page_cells(rect, layout, self.order);
        if self.page_layout != Some(layout) || self.page_calipers.len() != cells.len() {
            self.page_calipers = vec![CaliperSet::default(); cells.len()];
            self.page_layout = Some(layout);
        }
        self.page_rect = Some(rect);
        let zoom_cells: Vec<(Rect, u64)> = cells
            .iter()
            .map(|c| (c.rect, c.columns_before_head as u64))
            .collect();
        if let Some(change) = self.apply_time_zoom(ui, region, &zoom_cells, rings) {
            for (set, (r, cols)) in self.page_calipers.iter_mut().zip(&zoom_cells) {
                change.rebase(set, *r, *cols);
            }
            // the lane under the page spans the page's width
            change.rebase(&mut self.page_sound_calipers, region, 0);
        }
        let style = self.effective_style();
        let head = self.head(rings);
        let rings = self.display.rings(rings);
        let base = ui.scope_id().with("ecg_view_page");
        let intervals_fit = intervals_fit_cells(
            self.annotations.as_ref().and_then(|a| a.analysis.rr_ms),
            style.mm_per_s * style.px_per_mm,
        );
        let mut wins = Vec::with_capacity(cells.len());
        for (i, cell) in cells.iter().enumerate() {
            let ring = &rings[cell.lead as usize];
            let id = base.with(i);
            let probe = StripWindow::new(cell.rect, &style, ring.sample_rate_hz, head);
            let cell_head = head.saturating_sub(cell.columns_before_head as u64 * probe.visible_samples);
            // Wave letters and interval read-outs crowd a 12-lead page: the clinical grid carries
            // them on its rhythm strip only; the other layouts draw them in every cell, and
            // `draw_strip` drops the ones that would touch their neighbour.
            let annotate = Annotate {
                waves: cell_is_annotated(layout, i, cells.len()),
                intervals: cell_is_annotated(layout, i, cells.len())
                    && (layout == TwelveLeadLayout::Clinical || intervals_fit),
            };
            let trace = self.trace(cell.lead, cell.lead.label(), cell.invert, cell_head, annotate);
            let win = draw_strip(ui, id, cell.rect, ring, &style, &trace);
            strip_interact(ui, id, &win, ring, &mut self.page_calipers[i], &mut self.playback);
            draw_calipers(ui, &win, ring, cell.invert, &self.page_calipers[i], &style);
            wins.push(win);
        }
        wins
    }

    /// PNG of the current 12-lead page (what is on screen at the scrub cursor), `size_px` pixels.
    pub fn page_png(
        &self,
        rings: &[LeadRing],
        layout: TwelveLeadLayout,
        size_px: [u32; 2],
        px_per_point: f32,
    ) -> Result<Vec<u8>, image::ImageError> {
        let head = self.head(rings);
        // the snapshot shows what the screen shows: the processed trace, and it says so
        let mut img = crate::snapshot::render_page(
            self.display.rings(rings),
            &self.style,
            layout,
            self.order,
            head,
            size_px,
            px_per_point,
        );
        if let Some(note) = self.display.config().describe() {
            let style = StripStyle {
                px_per_mm: self.style.px_per_mm * px_per_point,
                ..self.style
            };
            crate::snapshot::stamp_caption(&mut img, &style, &note);
        }
        crate::snapshot::encode_png(&img)
    }

    /// Write [`EcgView::page_png`] at the on-screen page size to `path` and record the outcome in
    /// `last_export`.
    pub fn export_page_png(
        &mut self,
        rings: &[LeadRing],
        layout: TwelveLeadLayout,
        px_per_point: f32,
        path: &std::path::Path,
    ) {
        let size = self
            .page_rect
            .map_or(egui::Vec2::new(1600.0, 1000.0), |r| r.size() * px_per_point);
        let size_px = [size.x.max(1.0) as u32, size.y.max(1.0) as u32];
        let res = self
            .page_png(rings, layout, size_px, px_per_point)
            .map_err(|e| e.to_string())
            .and_then(|png| std::fs::write(path, png).map_err(|e| e.to_string()));
        self.last_export = Some(match res {
            Ok(()) => t!("ecg-saved", path = path.display().to_string()),
            Err(e) => t!("ecg-snapshot-failed", error = e),
        });
    }

    /// Calipers of the single-lead strip.
    #[must_use]
    pub fn strip_calipers(&self) -> &CaliperSet {
        &self.strip_calipers
    }

    /// Calipers of the single-lead strip, for programmatic placement.
    pub fn strip_calipers_mut(&mut self) -> &mut CaliperSet {
        &mut self.strip_calipers
    }
}

/// What one time-zoom step changed: the strip styles before and after, and the heads (one past
/// the newest sample shown) before and after, so each caliper set can stay on its samples.
struct ZoomChange {
    old: StripStyle,
    new: StripStyle,
    fs: f32,
    old_head: u64,
    new_head: u64,
    /// Paused in scroll mode: the head moved with the zoom and calipers follow their samples.
    /// Running, the calipers keep their interval and their place from the left edge.
    rebase: bool,
}

impl ZoomChange {
    /// The window of the cell `rect` (`cols` columns before the head) drawn with `style`.
    fn window(&self, rect: Rect, cols: u64, style: &StripStyle, head: u64) -> StripWindow {
        let probe = StripWindow::new(rect, style, self.fs, head);
        StripWindow::new(
            rect,
            style,
            self.fs,
            head.saturating_sub(cols * probe.visible_samples),
        )
    }

    /// Moves the lines of `set` so each stays on the sample it was on (paused scroll mode only).
    fn rebase(&self, set: &mut CaliperSet, rect: Rect, cols: u64) {
        if !self.rebase {
            return;
        }
        let old = self.window(rect, cols, &self.old, self.old_head);
        let new = self.window(rect, cols, &self.new, self.new_head);
        for c in &mut set.pairs {
            c.a_ms = rebase_caliper_ms(c.a_ms, &old, &new);
            c.b_ms = rebase_caliper_ms(c.b_ms, &old, &new);
        }
    }
}

/// A paper speed for the combo: "25 mm/s", "37.5 mm/s", "6.25 mm/s" (at most two decimals).
fn speed_label(mm_per_s: f32) -> String {
    let text = format!("{mm_per_s:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} mm/s")
}

/// The display filter's name with its band, as in the radio list and on the Display button.
fn filter_name(m: FilterMode) -> String {
    match m {
        FilterMode::Off => t!("ecg-filter-off"),
        FilterMode::Diagnostic => t!("ecg-filter-diagnostic"),
        FilterMode::Monitor => t!("ecg-filter-monitor"),
    }
}

/// Gives a combo box its accessible name (the visible label sits beside it in the layout).
fn name_combo(response: &egui::Response, name: &str) {
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, name));
}
