//! User preferences (B19): language, high-contrast theme, reduced motion and the torso picture.
//!
//! Stored with eframe's persistence under [`STORAGE_KEY`]. A missing or unreadable value gives the
//! defaults, so a corrupt entry never blocks start-up.

use eframe::egui::{self, Color32, Stroke, Visuals};
use heartsim_ui::i18n::{self, LANGUAGES};
use heartsim_ui::{cards, t};
use serde::{Deserialize, Serialize};

/// eframe storage key of the preferences.
pub const STORAGE_KEY: &str = "settings";

/// What the user chose in the Settings window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent on/off preferences, stored by name"
)]
pub struct Settings {
    /// A shipped language code (`"it"`), or `None` to follow the operating system.
    pub language: Option<String>,
    /// Black-and-white panels with heavy outlines, for low vision and projector use.
    pub high_contrast: bool,
    /// Turns off the automatic rotation of the 3-D heart, the defibrillator's shock flash and the
    /// valve flashes of the heart sounds (B27). Those are the only animations the simulator adds on its
    /// own; the simulation itself is unchanged.
    pub reduced_motion: bool,
    /// The first-run medical disclaimer (requirement N-09) has been acknowledged on this computer.
    /// Stored so the dialog appears once; the About window shows the same text at any time.
    pub disclaimer_accepted: bool,
    /// QRS-synchronous monitor beep (desktop builds with the `audio` feature). Off by default: a simulator
    /// that suddenly makes noise in a lecture hall is a surprise.
    pub monitor_beep: bool,
    /// Linear loudness of the monitor beep, `0.0..=1.0`.
    pub beep_volume: f32,
    /// Ask GitHub once at start whether a newer release exists (desktop builds with the `update-check`
    /// feature; see [`crate::update_check`]). On by default; off means no request is made at all.
    pub check_for_updates: bool,
    /// Light, dark, or whatever the operating system says (the default). Chosen from the bottom of the
    /// hamburger menu; `high_contrast` then applies on top of the chosen one.
    pub theme: ThemeMode,
    /// Opacity, `0.0..=1.0`, of the frontal torso picture behind the limb leads in the axis panel;
    /// `0.0` draws the outline of the procedural torso instead.
    pub torso_image_opacity: f32,
    /// The second row of the rhythm bar (sinus rate, AV block, QRS axis and width) is open.
    /// Closed by default; the Adjust toggle says when a value in it is away from the rhythm's own.
    pub adjust_open: bool,
    /// The heart-sound lane under the strip (B24, the Display popup's "Heart sounds"). Off by default.
    pub heart_sounds: bool,
    /// Play the heart sounds through the sound card (B25, desktop builds with the `audio` feature).
    /// Off by default, like the beep; switching it on also shows the lane.
    pub heart_sound_audio: bool,
    /// Linear loudness of the heart sounds, `0.0..=1.0`.
    pub heart_sound_volume: f32,
    /// The sound card's own latency, ms (`0..=300`): the views are drawn this much plus the queue
    /// behind the simulation while sound plays. Set by ear with the sync check.
    pub audio_offset_ms: f32,
    /// Where on the chest the heart sounds are heard (B27 part B, the Display popup's "Listening
    /// site"): the overview, today's unweighted mix, by default.
    pub listening_site: heartsim_ecg::ListeningSite,
    /// Bell or diaphragm; not used at the overview.
    pub chest_piece: heartsim_ecg::ChestPiece,
    /// Small-speaker mode (B28): every heart-sound frequency is multiplied by
    /// [`heartsim_ecg::SMALL_SPEAKER_FACTOR`] in what is played, so S3, S4 and the rumble reach laptop
    /// and phone speakers. The lane keeps the true trace and says "altered sound". Off by default: it
    /// changes how the sounds sound and has no proven teaching effect.
    pub small_speaker: bool,
    /// Wall motion of the 3-D heart (B30), on by default; forced off while `reduced_motion` is on
    /// (the stored choice is kept for when it goes off again). Switched in the heart panel.
    pub wall_motion: bool,
    /// Multiplier on the wall displacement for teaching, `0.5..=1.5` (the model folds the thin
    /// walls at 2x); 1 is the model.
    pub wall_motion_exaggeration: f32,
    /// Which sections of the Simulator's left panel are open. The first three are open by default;
    /// the wall-motion card and the vector overlay are one click away.
    pub simulator_sections: SimulatorSections,
    /// Record the echo M-mode trace at half its rate (B34): a column every 50 ms of simulated time
    /// instead of every 25 ms, for computers that cannot keep the full trace in real time. The
    /// columns it records are the ones the full trace records; a time read from the trace is then
    /// good to 50 ms. Off by default.
    pub echo_fast_mmode: bool,
    /// Draw the measured tricuspid valve (brief B35, the "Tricuspid valve" layer of *Anatomy and
    /// camera*). Off by default. A settings file from 0.25.0 keeps its value.
    pub tricuspid_valve: bool,
    /// Draw the designed mitral, aortic and pulmonary valves (brief B36, the "Mitral valve",
    /// "Aortic valve" and "Pulmonary valve" layers). Off by default: designed shapes, not reviewed
    /// by a clinician.
    pub mitral_valve: bool,
    pub aortic_valve: bool,
    pub pulmonary_valve: bool,
}

/// The collapsible sections of the Simulator's left panel and whether each is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one independent open/closed flag per section, stored by name"
)]
pub struct SimulatorSections {
    /// "Colour map": the 3-D activation switch, mode, window and wavefront outline.
    pub colour: bool,
    /// "Click on the heart": stimulate or ablate, lesion radius, clear.
    pub tool: bool,
    /// "Layers and view": layer visibility and opacity, cut-away, camera presets.
    pub layers: bool,
    /// "Wall motion": the switch, the exaggeration and the read-out card.
    pub motion: bool,
    /// "Vector overlay": the heart vector ring in the 3-D view.
    pub vector: bool,
}

impl Default for SimulatorSections {
    fn default() -> Self {
        Self {
            colour: true,
            tool: true,
            layers: true,
            motion: false,
            vector: false,
        }
    }
}

/// The colour scheme preference, stored by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeMode {
    /// Follow the operating system's light/dark setting; dark where it is unknown.
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    /// Every mode, in menu order.
    pub const ALL: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];

    fn preference(self) -> egui::ThemePreference {
        match self {
            ThemeMode::System => egui::ThemePreference::System,
            ThemeMode::Light => egui::ThemePreference::Light,
            ThemeMode::Dark => egui::ThemePreference::Dark,
        }
    }
}

/// Default loudness of the monitor beep, linear: half scale, audible without being startling.
pub const DEFAULT_BEEP_VOLUME: f32 = 0.5;
/// Default opacity of the torso picture: present, but the lead pills and electrodes stay the subject.
pub const DEFAULT_TORSO_IMAGE_OPACITY: f32 = 0.7;

// Written by hand: `beep_volume` defaults to `DEFAULT_BEEP_VOLUME`, not 0.0, also for the fields missing in
// settings stored by an older version (`#[serde(default)]` on the struct uses this).
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            high_contrast: false,
            reduced_motion: false,
            disclaimer_accepted: false,
            monitor_beep: false,
            beep_volume: DEFAULT_BEEP_VOLUME,
            check_for_updates: true,
            theme: ThemeMode::System,
            torso_image_opacity: DEFAULT_TORSO_IMAGE_OPACITY,
            adjust_open: false,
            heart_sounds: false,
            heart_sound_audio: false,
            heart_sound_volume: crate::sound_out::DEFAULT_HEART_SOUND_VOLUME,
            audio_offset_ms: crate::sound_out::DEFAULT_AUDIO_OFFSET_MS,
            listening_site: heartsim_ecg::ListeningSite::Overview,
            chest_piece: heartsim_ecg::ChestPiece::Diaphragm,
            small_speaker: false,
            wall_motion: true,
            wall_motion_exaggeration: 1.0,
            simulator_sections: SimulatorSections::default(),
            echo_fast_mmode: false,
            tricuspid_valve: false,
            mitral_valve: false,
            aortic_valve: false,
            pulmonary_valve: false,
        }
    }
}

impl Settings {
    /// Reads the preferences from eframe storage, or the defaults.
    #[must_use]
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        storage
            .and_then(|s| s.get_string(STORAGE_KEY))
            .and_then(|raw| ron::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// The text written to eframe storage.
    #[must_use]
    pub fn to_storage_string(&self) -> String {
        ron::to_string(self).unwrap_or_default()
    }

    /// The listening site and chest piece as one choice.
    #[must_use]
    pub fn listening(&self) -> heartsim_ecg::ListeningChoice {
        heartsim_ecg::ListeningChoice {
            site: self.listening_site,
            chest: self.chest_piece,
        }
    }

    /// The language code the interface uses now: the chosen one, or the operating system's.
    #[must_use]
    pub fn resolved_language(&self) -> &'static str {
        match self.language.as_deref() {
            Some(code) => i18n::negotiate(Some(code)),
            None => i18n::negotiate(i18n::system_locale().as_deref()),
        }
    }

    /// Applies the language to the translation layer and the theme and colours to egui: the
    /// light/dark preference first, then the visuals for whichever theme that resolves to.
    pub fn apply(&self, ctx: &egui::Context) {
        i18n::set_language(self.resolved_language());
        ctx.set_theme(self.theme.preference());
        ctx.set_visuals(self.visuals(ctx.theme() == egui::Theme::Dark));
    }

    fn visuals(&self, dark: bool) -> Visuals {
        let mut v = if dark { Visuals::dark() } else { Visuals::light() };
        if self.high_contrast {
            let (fg, bg) = if dark {
                (Color32::WHITE, Color32::BLACK)
            } else {
                (Color32::BLACK, Color32::WHITE)
            };
            v.override_text_color = Some(fg);
            v.panel_fill = bg;
            v.window_fill = bg;
            v.extreme_bg_color = bg;
            v.faint_bg_color = bg;
            let outline = Stroke::new(2.0, fg);
            v.widgets.noninteractive.bg_stroke = outline;
            v.widgets.inactive.bg_stroke = outline;
            v.widgets.hovered.bg_stroke = Stroke::new(3.0, fg);
            v.widgets.active.bg_stroke = Stroke::new(3.0, fg);
            // the label of a selected tab or pill is the override colour: black reads on yellow
            // (14.5:1), white does not (1.5:1), so the dark setting selects with a deep blue (8.6:1)
            v.selection.bg_fill = if dark {
                Color32::from_rgb(0, 70, 170)
            } else {
                Color32::from_rgb(255, 210, 0)
            };
            v.selection.stroke = Stroke::new(2.0, fg);
            v.hyperlink_color = if dark {
                Color32::from_rgb(120, 220, 255)
            } else {
                Color32::from_rgb(0, 60, 180)
            };
        }
        v
    }
}

/// Reads "35" or "35 %" as the fraction 0.35 (the text box of the percent sliders).
fn parse_percent(text: &str) -> Option<f64> {
    text.trim()
        .trim_end_matches('%')
        .trim()
        .parse::<f64>()
        .ok()
        .map(|p| p / 100.0)
}

/// The Settings window. Returns `true` when a setting changed this frame, so the caller can
/// re-apply the language and the theme.
pub fn settings_window(ctx: &egui::Context, open: &mut bool, settings: &mut Settings) -> bool {
    settings_window_with_audio(ctx, open, settings, None)
}

/// What the Settings window shows about the heart-sound audio (B25).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeartSoundInfo {
    /// `None` until the device has been asked for, then whether it could be opened.
    pub available: Option<bool>,
    /// The sync check is running.
    pub calibrating: bool,
    /// The sync check's marker is lit this frame.
    pub flash: bool,
    /// What the player reports (B26: a browser waiting for a click, a browser without Web Audio).
    pub status: heartsim_ui::HeartSoundStatus,
}

impl HeartSoundInfo {
    /// The Settings card's state from the player, or `None` where there is no audio output at all
    /// (no factory, B26): the card is then absent. The Display popup's switch follows the same rule
    /// ([`crate::heart_sound_player::HeartSoundPlayer::has_output`]).
    #[must_use]
    pub fn from_player(player: &crate::heart_sound_player::HeartSoundPlayer) -> Option<Self> {
        player.has_output().then(|| Self {
            available: player.available(),
            calibrating: player.calibrating(),
            flash: player.sync_flash(),
            status: player.status(),
        })
    }
}

/// What the Settings window reports back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SettingsResponse {
    /// A setting changed this frame (re-apply the language and the theme).
    pub changed: bool,
    /// The learner started (`Some(true)`) or stopped the sync check.
    pub calibrate: Option<bool>,
}

/// The Settings window with the heart-sound section: `heart_sounds` is `None` where the app has no
/// audio output (no sink factory, [`HeartSoundInfo::from_player`]), and the section is then absent.
pub fn settings_window_full(
    ctx: &egui::Context,
    open: &mut bool,
    settings: &mut Settings,
    beep_available: Option<bool>,
    heart_sounds: Option<&HeartSoundInfo>,
) -> SettingsResponse {
    let mut calibrate = None;
    let changed = settings_window_inner(ctx, open, settings, beep_available, |ui, settings| {
        let Some(info) = heart_sounds else { return false };
        let (changed, cal) = heart_sound_section(ui, settings, *info);
        calibrate = cal;
        changed
    });
    SettingsResponse { changed, calibrate }
}

/// The Heart sounds card: the switch, the volume, the audio offset with its explanation and the
/// sync check, the "no sound device" notice and the headphone hint. Returns whether a setting
/// changed and a sync-check request.
fn heart_sound_section(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    info: HeartSoundInfo,
) -> (bool, Option<bool>) {
    let mut changed = false;
    let mut calibrate = None;
    cards::card(ui, |ui| {
        ui.strong(t!("settings-hs-heading"));
        let switch = ui.checkbox(&mut settings.heart_sound_audio, t!("settings-hs-play"));
        if switch.changed() {
            changed = true;
            if settings.heart_sound_audio {
                settings.heart_sounds = true;
            } else if info.calibrating {
                calibrate = Some(false);
            }
        }
        ui.add_enabled_ui(settings.heart_sound_audio, |ui| {
            let volume = egui::Slider::new(&mut settings.heart_sound_volume, 0.0..=1.0)
                .custom_formatter(|v, _| format!("{:.0} %", v * 100.0))
                .custom_parser(parse_percent);
            changed |= heartsim_ui::slider_row(ui, &t!("settings-hs-volume"), volume).changed();
            let offset = egui::Slider::new(
                &mut settings.audio_offset_ms,
                crate::sound_out::AUDIO_OFFSET_RANGE_MS,
            )
            .suffix(heartsim_ui::controls::MS)
            .max_decimals(0);
            changed |= heartsim_ui::slider_row(ui, &t!("settings-hs-offset"), offset).changed();
            heartsim_ui::info_disclosure(
                ui,
                "hs_offset",
                t!("settings-hs-offset-summary"),
                t!("settings-hs-offset-hint"),
            );
            ui.horizontal(|ui| {
                let label = if info.calibrating {
                    t!("settings-hs-sync-stop")
                } else {
                    t!("settings-hs-sync")
                };
                if ui.button(label).clicked() {
                    calibrate = Some(!info.calibrating);
                }
                // the marker: lit while the click should be heard
                let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                let colour = ui.visuals().text_color();
                if info.calibrating && info.flash {
                    ui.painter().circle_filled(rect.center(), 7.0, colour);
                } else {
                    ui.painter().circle_stroke(
                        rect.center(),
                        7.0,
                        Stroke::new(1.0, colour.gamma_multiply(0.5)),
                    );
                }
            });
            heartsim_ui::helper_text(ui, t!("settings-hs-sync-hint"));
            changed |= ui
                .checkbox(&mut settings.small_speaker, t!("settings-hs-small-speaker"))
                .changed();
            heartsim_ui::helper_text(ui, t!("settings-hs-small-speaker-hint"));
        });
        if settings.heart_sound_audio {
            use heartsim_ui::HeartSoundStatus;
            match info.status {
                HeartSoundStatus::Unsupported => {
                    heartsim_ui::helper_text(ui, t!("settings-hs-unsupported"));
                }
                HeartSoundStatus::WaitingForClick => {
                    heartsim_ui::helper_text(ui, t!("settings-hs-click-to-enable"));
                }
                _ if info.available == Some(false) => {
                    heartsim_ui::helper_text(ui, t!("settings-hs-unavailable"));
                }
                _ => {}
            }
        }
        heartsim_ui::helper_text(ui, t!("settings-hs-headphones"));
        heartsim_ui::helper_text(ui, t!("settings-hs-model"));
    });
    (changed, calibrate)
}

/// [`settings_window`] that also knows whether the audio output could be opened
/// ([`crate::monitor_beep::MonitorBeep::available`]): with `Some(false)` the monitor-beep section says that
/// no sound device was found. The monitor-beep controls exist only in builds with the `audio` feature.
pub fn settings_window_with_audio(
    ctx: &egui::Context,
    open: &mut bool,
    settings: &mut Settings,
    beep_available: Option<bool>,
) -> bool {
    settings_window_inner(ctx, open, settings, beep_available, |_, _| false)
}

fn settings_window_inner(
    ctx: &egui::Context,
    open: &mut bool,
    settings: &mut Settings,
    beep_available: Option<bool>,
    mut extra: impl FnMut(&mut egui::Ui, &mut Settings) -> bool,
) -> bool {
    let _ = beep_available; // read only by the `audio` section below
    let mut changed = false;
    egui::Window::new(t!("settings-title"))
        .open(open)
        .resizable(false)
        .show(ctx, |ui| {
            cards::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(t!("settings-language"));
                    let selected = match &settings.language {
                        None => t!("settings-language-system"),
                        Some(code) => LANGUAGES
                            .iter()
                            .find(|l| l.code == code)
                            .map_or_else(|| code.clone(), |l| l.name.to_string()),
                    };
                    let combo = egui::ComboBox::from_id_salt("settings_language")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            changed |= ui
                                .selectable_value(
                                    &mut settings.language,
                                    None,
                                    t!("settings-language-system"),
                                )
                                .changed();
                            for l in LANGUAGES {
                                changed |= ui
                                    .selectable_value(
                                        &mut settings.language,
                                        Some(l.code.to_string()),
                                        l.name,
                                    )
                                    .changed();
                            }
                        });
                    // the visible label sits beside the box: give it its accessible name too
                    let name = t!("settings-language");
                    combo
                        .response
                        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &name));
                });
                changed |= ui
                    .checkbox(&mut settings.high_contrast, t!("settings-high-contrast"))
                    .changed();
                changed |= ui
                    .checkbox(&mut settings.reduced_motion, t!("settings-reduced-motion"))
                    .changed();
                let slider = egui::Slider::new(&mut settings.torso_image_opacity, 0.0..=1.0)
                    .custom_formatter(|v, _| format!("{:.0} %", v * 100.0))
                    .custom_parser(parse_percent);
                changed |= heartsim_ui::slider_row(ui, &t!("settings-torso-opacity"), slider).changed();
                heartsim_ui::helper_text(ui, t!("settings-torso-opacity-hint"));
            });
            #[cfg(feature = "audio")]
            cards::card(ui, |ui| {
                changed |= ui
                    .checkbox(&mut settings.monitor_beep, t!("settings-beep"))
                    .changed();
                ui.add_enabled_ui(settings.monitor_beep, |ui| {
                    let slider = egui::Slider::new(&mut settings.beep_volume, 0.0..=1.0)
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0))
                        .custom_parser(parse_percent);
                    changed |= heartsim_ui::slider_row(ui, &t!("settings-beep-volume"), slider).changed();
                });
                heartsim_ui::helper_text(ui, t!("settings-beep-hint"));
                if settings.monitor_beep && beep_available == Some(false) {
                    heartsim_ui::helper_text(ui, t!("settings-beep-unavailable"));
                }
            });
            changed |= extra(ui, settings);
            cards::card(ui, |ui| {
                changed |= ui
                    .checkbox(&mut settings.echo_fast_mmode, t!("settings-echo-fast-mmode"))
                    .changed();
                heartsim_ui::helper_text(ui, t!("settings-echo-fast-mmode-hint"));
            });
            #[cfg(feature = "update-check")]
            cards::card(ui, |ui| {
                changed |= ui
                    .checkbox(&mut settings.check_for_updates, t!("settings-update-check"))
                    .changed();
                heartsim_ui::helper_text(ui, t!("settings-update-check-hint"));
            });
            heartsim_ui::helper_text(ui, t!("settings-hint"));
        });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_storage_text() {
        let s = Settings {
            language: Some("it".into()),
            high_contrast: true,
            reduced_motion: true,
            disclaimer_accepted: true,
            monitor_beep: true,
            beep_volume: 0.25,
            check_for_updates: false,
            theme: ThemeMode::Light,
            torso_image_opacity: 0.4,
            adjust_open: true,
            heart_sounds: true,
            heart_sound_audio: true,
            heart_sound_volume: 0.3,
            audio_offset_ms: 120.0,
            listening_site: heartsim_ecg::ListeningSite::Tricuspid,
            chest_piece: heartsim_ecg::ChestPiece::Bell,
            small_speaker: true,
            wall_motion: false,
            wall_motion_exaggeration: 1.25,
            simulator_sections: SimulatorSections {
                colour: false,
                tool: true,
                layers: false,
                motion: true,
                vector: true,
            },
            echo_fast_mmode: true,
            tricuspid_valve: true,
            mitral_valve: true,
            aortic_valve: false,
            pulmonary_valve: true,
        };
        let back: Settings = ron::from_str(&s.to_storage_string()).expect("valid RON");
        assert_eq!(back, s);
    }

    #[test]
    fn the_torso_picture_is_shown_at_its_default_opacity_also_for_older_settings() {
        let d = Settings::default();
        assert!((d.torso_image_opacity - DEFAULT_TORSO_IMAGE_OPACITY).abs() < f32::EPSILON);
        // settings saved before the picture existed load and get the default, not 0.0
        let old: Settings = ron::from_str("(language: None, theme: Dark)").expect("old settings load");
        assert!((old.torso_image_opacity - DEFAULT_TORSO_IMAGE_OPACITY).abs() < f32::EPSILON);
        let off: Settings = ron::from_str("(torso_image_opacity: 0.0)").expect("settings load");
        assert!(off.torso_image_opacity.abs() < f32::EPSILON);
    }

    /// B35: the measured valve's layer is off by default, also for settings saved before it existed.
    #[test]
    fn the_tricuspid_valve_layer_is_off_by_default_and_older_settings_load_without_it() {
        assert!(!Settings::default().tricuspid_valve);
        let old: Settings = ron::from_str("(wall_motion: true)").expect("settings load");
        assert!(!old.tricuspid_valve);
        let on: Settings = ron::from_str("(tricuspid_valve: true)").expect("settings load");
        assert!(on.tricuspid_valve);
    }

    /// B36: the designed valves' layers are off by default; a 0.25.0 settings file (with
    /// `tricuspid_valve` and without the three new fields) loads with its tricuspid value.
    #[test]
    fn the_designed_valve_layers_are_off_by_default_and_a_0_25_settings_file_loads() {
        let d = Settings::default();
        assert!(!(d.mitral_valve || d.aortic_valve || d.pulmonary_valve));
        let old: Settings =
            ron::from_str("(wall_motion: true, echo_fast_mmode: true, tricuspid_valve: true)")
                .expect("0.25.0 settings");
        assert!(old.tricuspid_valve);
        assert!(!(old.mitral_valve || old.aortic_valve || old.pulmonary_valve));
        let on: Settings = ron::from_str("(aortic_valve: true)").expect("settings load");
        assert!(on.aortic_valve && !on.mitral_valve);
    }

    /// B30: wall motion is on at 1x by default, also for settings saved before it existed.
    #[test]
    fn wall_motion_is_on_by_default_and_older_settings_load_with_it() {
        let d = Settings::default();
        assert!(d.wall_motion);
        assert!((d.wall_motion_exaggeration - 1.0).abs() < f32::EPSILON);
        let old: Settings = ron::from_str("(language: None, adjust_open: true)").expect("old settings load");
        assert!(old.wall_motion);
        assert!((old.wall_motion_exaggeration - 1.0).abs() < f32::EPSILON);
        let off: Settings = ron::from_str("(wall_motion: false)").expect("settings load");
        assert!(!off.wall_motion);
    }

    #[test]
    fn the_faster_mmode_trace_is_off_by_default_and_in_older_settings() {
        assert!(!Settings::default().echo_fast_mmode);
        let old: Settings = ron::from_str("(high_contrast: true)").expect("settings load");
        assert!(!old.echo_fast_mmode);
    }

    #[test]
    fn simulator_sections_default_to_the_first_three_open_and_older_settings_get_that() {
        let d = SimulatorSections::default();
        assert!(d.colour && d.tool && d.layers && !d.motion && !d.vector);
        let old: Settings = ron::from_str("(language: None, adjust_open: true)").expect("old settings load");
        assert_eq!(old.simulator_sections, d);
        let part: Settings = ron::from_str("(simulator_sections: (colour: false))").expect("settings load");
        assert!(!part.simulator_sections.colour && part.simulator_sections.tool);
    }

    #[test]
    fn the_adjust_row_is_closed_by_default_and_older_settings_load_closed() {
        assert!(!Settings::default().adjust_open);
        let old: Settings = ron::from_str("(language: None, theme: Dark)").expect("old settings load");
        assert!(!old.adjust_open);
    }

    #[test]
    fn the_heart_sound_lane_is_off_by_default_and_older_settings_load_without_it() {
        assert!(!Settings::default().heart_sounds);
        let old: Settings = ron::from_str("(language: None, adjust_open: true)").expect("old settings load");
        assert!(!old.heart_sounds);
        let on: Settings = ron::from_str("(heart_sounds: true)").expect("settings load");
        assert!(on.heart_sounds);
    }

    #[test]
    fn heart_sound_audio_is_off_with_designed_defaults_also_for_older_settings() {
        let d = Settings::default();
        assert!(!d.heart_sound_audio);
        assert!((d.heart_sound_volume - crate::sound_out::DEFAULT_HEART_SOUND_VOLUME).abs() < f32::EPSILON);
        assert!((d.audio_offset_ms - 60.0).abs() < f32::EPSILON);
        // a settings file from 0.16 (lane switch, no audio fields) loads with the defaults
        let old: Settings = ron::from_str("(language: None, heart_sounds: true, monitor_beep: true)")
            .expect("old settings load");
        assert!(old.heart_sounds && old.monitor_beep);
        assert!(!old.heart_sound_audio);
        assert!((old.heart_sound_volume - crate::sound_out::DEFAULT_HEART_SOUND_VOLUME).abs() < f32::EPSILON);
        assert!((old.audio_offset_ms - crate::sound_out::DEFAULT_AUDIO_OFFSET_MS).abs() < f32::EPSILON);
    }

    #[test]
    fn small_speaker_mode_is_off_by_default_also_for_older_settings_and_round_trips() {
        assert!(!Settings::default().small_speaker);
        let old: Settings =
            ron::from_str("(heart_sound_audio: true, heart_sounds: true)").expect("old settings load");
        assert!(!old.small_speaker);
        let on = Settings {
            small_speaker: true,
            ..Settings::default()
        };
        let back: Settings = ron::from_str(&on.to_storage_string()).expect("valid RON");
        assert!(back.small_speaker);
        assert_eq!(back, on);
    }

    #[test]
    fn the_theme_follows_the_system_by_default_and_is_stored_by_name() {
        assert_eq!(Settings::default().theme, ThemeMode::System);
        let old: Settings =
            ron::from_str("(language: None, high_contrast: false)").expect("old settings load");
        assert_eq!(old.theme, ThemeMode::System);
        let dark = Settings {
            theme: ThemeMode::Dark,
            ..Settings::default()
        };
        let stored = dark.to_storage_string();
        // compact RON: no space after the colon
        assert!(stored.contains("theme:Dark"), "{stored}");
        let back: Settings = ron::from_str(&stored).expect("valid RON");
        assert_eq!(back.theme, ThemeMode::Dark);
    }

    #[test]
    fn the_update_check_is_on_by_default_also_for_older_settings() {
        assert!(Settings::default().check_for_updates);
        // settings saved before the switch existed load and get the default
        let old: Settings = ron::from_str("(language: None, high_contrast: false, monitor_beep: true)")
            .expect("old settings load");
        assert!(old.check_for_updates);
        assert!(old.monitor_beep);
        let off: Settings = ron::from_str("(check_for_updates: false)").expect("settings load");
        assert!(!off.check_for_updates);
    }

    #[test]
    fn the_monitor_beep_is_off_at_half_volume_by_default() {
        let d = Settings::default();
        assert!(!d.monitor_beep);
        assert!((d.beep_volume - DEFAULT_BEEP_VOLUME).abs() < f32::EPSILON);
        // settings saved before the beep existed keep working and get the same defaults
        let old: Settings =
            ron::from_str("(language: None, high_contrast: true)").expect("old settings load");
        assert!(old.high_contrast);
        assert!(!old.monitor_beep);
        assert!((old.beep_volume - DEFAULT_BEEP_VOLUME).abs() < f32::EPSILON);
    }

    #[test]
    fn corrupt_storage_gives_defaults() {
        let parsed: Option<Settings> = ron::from_str("not ron at all {{").ok();
        assert!(parsed.is_none());
        assert_eq!(Settings::default().language, None);
    }

    #[test]
    fn the_disclaimer_is_unacknowledged_until_stored() {
        assert!(!Settings::default().disclaimer_accepted);
        // settings saved by an older version have no such field: still unacknowledged
        let old: Settings =
            ron::from_str("(language: Some(\"it\"), high_contrast: false, reduced_motion: false)")
                .expect("old settings load");
        assert!(!old.disclaimer_accepted);
        assert_eq!(old.language.as_deref(), Some("it"));
    }

    #[test]
    fn high_contrast_uses_full_black_and_white() {
        let s = Settings {
            high_contrast: true,
            ..Settings::default()
        };
        let dark = s.visuals(true);
        assert_eq!(dark.override_text_color, Some(Color32::WHITE));
        assert_eq!(dark.panel_fill, Color32::BLACK);
        let light = s.visuals(false);
        assert_eq!(light.override_text_color, Some(Color32::BLACK));
        // a selected tab's label (the override colour) reads on the selection fill
        for v in [&dark, &light] {
            let fg = v.override_text_color.expect("override");
            let r = heartsim_ui::theme::contrast_ratio(fg, v.selection.bg_fill);
            assert!(r >= 7.0, "high-contrast selected label: {r:.2}");
        }
    }
}
