//! The medical disclaimer (requirement N-09): a dialog on first run that must be acknowledged once,
//! and an About window that shows the same text with the version at any time.
//!
//! The wording follows the guide's `docs/book/src/user/disclaimer.md` in substance (educational
//! simulation, not a medical device, simplified models, "simulated" exports, imports are only
//! displayed, no warranty). Whether it was acknowledged is the [`crate::settings::Settings`] flag
//! `disclaimer_accepted`, which eframe storage keeps on the desktop and in the browser.

use eframe::egui;
use heartsim_ui::t;

/// The version shown in About: the workspace version, as `heartsim --version` prints it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Message ids of the disclaimer paragraphs, in the order of the guide page. The first is the
/// lead sentence and is drawn in bold; a test pins that every id exists in every shipped language.
pub const PARAGRAPH_KEYS: [&str; 6] = [
    "disclaimer-lead",
    "disclaimer-models",
    "disclaimer-values",
    "disclaimer-exports",
    "disclaimer-import",
    "disclaimer-warranty",
];

/// The disclaimer text, translated, one entry per [`PARAGRAPH_KEYS`] id.
#[must_use]
pub fn paragraphs() -> Vec<String> {
    PARAGRAPH_KEYS.iter().map(|k| heartsim_ui::i18n::tr(k)).collect()
}

/// Draw the disclaimer paragraphs (lead sentence strong, the rest as a bulleted list).
pub fn disclaimer_body(ui: &mut egui::Ui) {
    let mut text = paragraphs().into_iter();
    if let Some(lead) = text.next() {
        ui.label(egui::RichText::new(lead).strong());
    }
    ui.add_space(4.0);
    for p in text {
        ui.horizontal_top(|ui| {
            ui.label("•");
            ui.add(egui::Label::new(p).wrap());
        });
    }
}

/// The first-run dialog. Modal: nothing behind it can be used until the button is pressed, and
/// Escape or a click outside does not dismiss it. Returns `true` on the frame it is acknowledged.
pub fn first_run_dialog(ctx: &egui::Context) -> bool {
    let mut accepted = false;
    egui::Modal::new(egui::Id::unique("first_run_disclaimer")).show(ctx, |ui| {
        ui.set_max_width(460.0);
        ui.heading(t!("disclaimer-title"));
        ui.add_space(6.0);
        disclaimer_body(ui);
        ui.add_space(6.0);
        ui.weak(t!("disclaimer-reminder"));
        ui.add_space(8.0);
        let button = ui.button(t!("disclaimer-accept"));
        // keyboard users land on the only action; Enter or Space acknowledges
        if ctx.memory(|m| m.focused().is_none()) {
            button.request_focus();
        }
        accepted = button.clicked();
    });
    accepted
}

/// The About window: product, version, licence and the disclaimer. `newer` is the release the update check
/// found (see [`crate::update_check`]): shown under the version with the link to its page.
pub fn about_window(ctx: &egui::Context, open: &mut bool, newer: Option<&crate::update_check::Release>) {
    egui::Window::new(t!("about-title"))
        .open(open)
        .resizable(false)
        .default_width(440.0)
        .show(ctx, |ui| {
            ui.heading("heartsim");
            ui.label(t!("about-version", version = VERSION));
            if let Some(release) = newer {
                ui.horizontal_wrapped(|ui| {
                    ui.label(t!("update-available", version = release.version.as_str()));
                    ui.hyperlink_to(t!("update-download"), &release.html_url);
                });
            }
            ui.label(t!("about-tagline"));
            ui.weak(t!("about-licence"));
            ui.separator();
            ui.strong(t!("about-disclaimer-heading"));
            disclaimer_body(ui);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_the_package_version() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
        assert!(VERSION.split('.').count() >= 3);
    }

    #[test]
    fn the_english_text_carries_the_guide_s_substance() {
        heartsim_ui::i18n::set_language("en");
        let text = paragraphs().join(" ").to_lowercase();
        for phrase in [
            "educational",
            "not a medical device",
            "diagnosis",
            "treatment",
            "simulated",
            "no patient data",
            "as is",
        ] {
            assert!(text.contains(phrase), "disclaimer lacks {phrase:?}");
        }
    }
}
