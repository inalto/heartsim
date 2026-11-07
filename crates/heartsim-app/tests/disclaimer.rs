//! N-09 acceptance: the medical disclaimer appears on first run until it is acknowledged, is
//! remembered through the settings text eframe stores, and is shown again, with the version, in
//! About. The wording matches the user guide's disclaimer page. No GPU.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::disclaimer::{self, PARAGRAPH_KEYS};
use heartsim_app::settings::Settings;
use heartsim_ui::i18n;
use std::path::Path;

struct Model {
    settings: Settings,
    about_open: bool,
}

/// What `HeartSimApp::ui` does with the dialog and the About window, on a bare context.
fn harness(settings: Settings) -> Harness<'static, Model> {
    Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                let ctx = ui.ctx().clone();
                if m.about_open {
                    disclaimer::about_window(&ctx, &mut m.about_open);
                }
                if !m.settings.disclaimer_accepted && disclaimer::first_run_dialog(&ctx) {
                    m.settings.disclaimer_accepted = true;
                }
            },
            Model {
                settings,
                about_open: false,
            },
        )
}

fn repo(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn first_run_shows_the_dialog_until_it_is_acknowledged() {
    i18n::set_language("en");
    let mut h = harness(Settings::default());
    h.run();
    let _ = h.get_by_label("Before you start: educational use only");
    let _ = h.get_by_label_contains("not a medical device");
    let _ = h.get_by_label_contains("diagnosis, treatment, monitoring");
    assert!(!h.state().settings.disclaimer_accepted);

    h.get_by_label("I understand: educational use only").click();
    h.run();
    assert!(h.state().settings.disclaimer_accepted);
    assert!(
        h.query_by_label("Before you start: educational use only")
            .is_none(),
        "the dialog stays after being acknowledged"
    );
}

#[test]
fn the_acknowledgement_is_remembered_in_the_stored_settings() {
    i18n::set_language("en");
    let mut h = harness(Settings::default());
    h.run();
    h.get_by_label("I understand: educational use only").click();
    h.run();
    let stored = h.state().settings.to_storage_string();

    // the next start reads what eframe stored
    let restored: Settings = ron::from_str(&stored).expect("stored settings parse");
    assert!(restored.disclaimer_accepted);
    let mut again = harness(restored);
    again.run();
    assert!(
        again
            .query_by_label("Before you start: educational use only")
            .is_none(),
        "asked again on the second start"
    );
}

#[test]
fn about_shows_the_version_and_the_disclaimer_at_any_time() {
    i18n::set_language("en");
    let mut h = harness(Settings {
        disclaimer_accepted: true,
        ..Settings::default()
    });
    h.run();
    assert!(h.query_by_label("About heartsim").is_none());
    h.state_mut().about_open = true;
    h.run();
    let _ = h.get_by_label("About heartsim");
    let _ = h.get_by_label(&format!("Version {}", env!("CARGO_PKG_VERSION")));
    let _ = h.get_by_label("Medical disclaimer");
    let _ = h.get_by_label_contains("educational simulation");
    let _ = h.get_by_label_contains("without warranty");
}

#[test]
fn the_dialog_is_available_in_italian() {
    i18n::set_language("it");
    let mut h = harness(Settings::default());
    h.run();
    let _ = h.get_by_label("Prima di iniziare: solo uso didattico");
    let _ = h.get_by_label_contains("Non è un dispositivo medico");
    h.get_by_label("Ho capito: solo uso didattico").click();
    h.run();
    assert!(h.state().settings.disclaimer_accepted);
    i18n::set_language("en");
}

#[test]
fn every_paragraph_exists_in_both_languages_and_matches_the_guide() {
    for lang in ["en", "it"] {
        let ftl = repo(&format!("assets/locales/{lang}/app.ftl"));
        for key in PARAGRAPH_KEYS {
            assert!(
                ftl.lines().any(|l| l.starts_with(&format!("{key} = "))),
                "{lang}/app.ftl lacks {key}"
            );
        }
    }
    // the guide page and the dialog make the same statements
    // the page wraps its lines: compare on single-spaced text
    let guide = repo("docs/book/src/user/disclaimer.md")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    i18n::set_language("en");
    let text = disclaimer::paragraphs().join(" ");
    for phrase in [
        "educational simulation",
        "not a medical device",
        "diagnosis, treatment, monitoring",
        "simplified models",
        "not validated against clinical data",
        "no patient data",
        "without warranty of any kind",
    ] {
        assert!(guide.contains(phrase), "guide lacks {phrase:?}");
        assert!(text.contains(phrase), "dialog lacks {phrase:?}");
    }
}
