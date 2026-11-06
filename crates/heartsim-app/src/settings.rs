//! User preferences (B19): language, high-contrast theme and reduced motion.
//!
//! Stored with eframe's persistence under [`STORAGE_KEY`]. A missing or unreadable value gives the
//! defaults, so a corrupt entry never blocks start-up.

use eframe::egui::{self, Color32, Stroke, Visuals};
use heartsim_ui::i18n::{self, LANGUAGES};
use heartsim_ui::t;
use serde::{Deserialize, Serialize};

/// eframe storage key of the preferences.
pub const STORAGE_KEY: &str = "settings";

/// What the user chose in the Settings window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    /// A shipped language code (`"it"`), or `None` to follow the operating system.
    pub language: Option<String>,
    /// Black-and-white panels with heavy outlines, for low vision and projector use.
    pub high_contrast: bool,
    /// Turns off the automatic rotation of the 3-D heart and the defibrillator's shock flash.
    /// Those are the only animations the simulator adds on its own; the simulation itself is unchanged.
    pub reduced_motion: bool,
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

    /// Applies the language to the translation layer and the colours to egui.
    pub fn apply(&self, ctx: &egui::Context) {
        i18n::set_language(self.resolved_language());
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

/// The Settings window. Returns `true` when a setting changed this frame, so the caller can
/// re-apply the language and the theme.
pub fn settings_window(ctx: &egui::Context, open: &mut bool, settings: &mut Settings) -> bool {
    let mut changed = false;
    egui::Window::new(t!("settings-title"))
        .open(open)
        .resizable(false)
        .show(ctx, |ui| {
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
                            .selectable_value(&mut settings.language, None, t!("settings-language-system"))
                            .changed();
                        for l in LANGUAGES {
                            changed |= ui
                                .selectable_value(&mut settings.language, Some(l.code.to_string()), l.name)
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
            ui.separator();
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
        };
        let back: Settings = ron::from_str(&s.to_storage_string()).expect("valid RON");
        assert_eq!(back, s);
    }

    #[test]
    fn corrupt_storage_gives_defaults() {
        let parsed: Option<Settings> = ron::from_str("not ron at all {{").ok();
        assert!(parsed.is_none());
        assert_eq!(Settings::default().language, None);
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
