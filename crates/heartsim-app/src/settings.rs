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
    /// Turns off the automatic rotation of the 3-D heart and the defibrillator's shock flash.
    /// Those are the only animations the simulator adds on its own; the simulation itself is unchanged.
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
            v.selection.bg_fill = Color32::from_rgb(255, 210, 0);
            v.selection.stroke = Stroke::new(2.0, Color32::BLACK);
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

/// [`settings_window`] that also knows whether the audio output could be opened
/// ([`crate::monitor_beep::MonitorBeep::available`]): with `Some(false)` the monitor-beep section says that
/// no sound device was found. The monitor-beep controls exist only in builds with the `audio` feature.
pub fn settings_window_with_audio(
    ctx: &egui::Context,
    open: &mut bool,
    settings: &mut Settings,
    beep_available: Option<bool>,
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
                    egui::ComboBox::from_id_salt("settings_language")
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
                });
                changed |= ui
                    .checkbox(&mut settings.high_contrast, t!("settings-high-contrast"))
                    .changed();
                changed |= ui
                    .checkbox(&mut settings.reduced_motion, t!("settings-reduced-motion"))
                    .changed();
                let slider = egui::Slider::new(&mut settings.torso_image_opacity, 0.0..=1.0)
                    .text(t!("settings-torso-opacity"))
                    .custom_formatter(|v, _| format!("{:.0} %", v * 100.0))
                    .custom_parser(parse_percent);
                changed |= ui.add(slider).changed();
                ui.weak(t!("settings-torso-opacity-hint"));
            });
            #[cfg(feature = "audio")]
            cards::card(ui, |ui| {
                changed |= ui
                    .checkbox(&mut settings.monitor_beep, t!("settings-beep"))
                    .changed();
                ui.add_enabled_ui(settings.monitor_beep, |ui| {
                    let slider = egui::Slider::new(&mut settings.beep_volume, 0.0..=1.0)
                        .text(t!("settings-beep-volume"))
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0))
                        .custom_parser(parse_percent);
                    changed |= ui.add(slider).changed();
                });
                ui.weak(t!("settings-beep-hint"));
                if settings.monitor_beep && beep_available == Some(false) {
                    ui.weak(t!("settings-beep-unavailable"));
                }
            });
            #[cfg(feature = "update-check")]
            cards::card(ui, |ui| {
                changed |= ui
                    .checkbox(&mut settings.check_for_updates, t!("settings-update-check"))
                    .changed();
                ui.weak(t!("settings-update-check-hint"));
            });
            ui.weak(t!("settings-hint"));
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
    }
}
