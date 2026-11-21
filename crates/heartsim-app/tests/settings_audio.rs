//! B25 acceptance tests of the Settings window's Heart sounds section: present when the build has
//! audio output (the app passes the section's state only then) and absent otherwise, the switch,
//! volume and offset, the "no sound device" notice, the sync check, and the settings round trip
//! including a settings text stored by an older version. Headless, no GPU, no sound card.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::heart_sound_player::AUDIO_BUILD;
use heartsim_app::settings::{HeartSoundInfo, Settings, SettingsResponse, settings_window_full};
use heartsim_app::sound_out::{DEFAULT_AUDIO_OFFSET_MS, DEFAULT_HEART_SOUND_VOLUME};
use heartsim_ui::i18n;

struct Model {
    settings: Settings,
    open: bool,
    /// `None` stands for a build without audio output.
    info: Option<HeartSoundInfo>,
    last: SettingsResponse,
}

fn harness(info: Option<HeartSoundInfo>) -> Harness<'static, Model> {
    Harness::builder()
        .with_size(egui::vec2(700.0, 900.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                let ctx = ui.ctx().clone();
                let r = settings_window_full(&ctx, &mut m.open, &mut m.settings, None, m.info.as_ref());
                if r.calibrate.is_some() || r.changed {
                    m.last = r;
                }
            },
            Model {
                settings: Settings::default(),
                open: true,
                info,
                last: SettingsResponse::default(),
            },
        )
}

#[test]
fn the_section_exists_only_in_builds_with_audio() {
    i18n::set_language("en");
    let mut without = harness(None);
    without.run();
    let _ = without.get_by_label("Settings");
    assert!(
        without
            .query_by_label("Play heart sounds (off by default)")
            .is_none()
    );
    assert!(without.query_by_label("Check sync").is_none());
    let mut with = harness(Some(HeartSoundInfo::default()));
    with.run();
    let _ = with.get_by_label("Play heart sounds (off by default)");
    let _ = with.get_by_label("Check sync");
    let _ = with.get_by_label_contains("laptop speakers reproduce little below 150 Hz");
    // and the app hands the section over exactly when it was built with the `audio` feature
    assert_eq!(AUDIO_BUILD, cfg!(feature = "audio"));
}

#[test]
fn switching_the_sound_on_also_shows_the_lane_and_a_missing_device_is_said() {
    i18n::set_language("en");
    let mut h = harness(Some(HeartSoundInfo {
        available: Some(false),
        ..HeartSoundInfo::default()
    }));
    h.run();
    assert!(
        h.query_by_label_contains("heart sounds stay silent").is_none(),
        "only once switched on"
    );
    h.get_by_label("Play heart sounds (off by default)").click();
    h.run();
    assert!(h.state().settings.heart_sound_audio);
    assert!(h.state().settings.heart_sounds, "the lane comes with the sound");
    assert!(h.state().last.changed);
    let _ = h.get_by_label_contains("No sound device could be opened, so the heart sounds stay silent.");
    i18n::set_language("it");
    h.run();
    let _ = h.get_by_label("Ascolta i toni cardiaci (disattivato di default)");
    i18n::set_language("en");
}

#[test]
fn the_sync_check_is_asked_for_and_stopped() {
    i18n::set_language("en");
    let mut h = harness(Some(HeartSoundInfo::default()));
    h.state_mut().settings.heart_sound_audio = true;
    h.run();
    h.get_by_label("Check sync").click();
    h.run();
    assert_eq!(h.state().last.calibrate, Some(true));
    h.state_mut().info = Some(HeartSoundInfo {
        available: Some(true),
        calibrating: true,
        flash: true,
    });
    h.run();
    h.get_by_label("Stop the sync check").click();
    h.run();
    assert_eq!(h.state().last.calibrate, Some(false));
}

#[test]
fn heart_sound_settings_round_trip_and_old_files_get_the_defaults() {
    let s = Settings {
        heart_sound_audio: true,
        heart_sound_volume: 0.4,
        audio_offset_ms: 150.0,
        ..Settings::default()
    };
    let back: Settings = ron::from_str(&s.to_storage_string()).expect("valid RON");
    assert_eq!(back, s);
    let old: Settings = ron::from_str("(language: Some(\"it\"), heart_sounds: true)").expect("0.16 settings");
    assert!(!old.heart_sound_audio);
    assert!((old.heart_sound_volume - DEFAULT_HEART_SOUND_VOLUME).abs() < f32::EPSILON);
    assert!((old.audio_offset_ms - DEFAULT_AUDIO_OFFSET_MS).abs() < f32::EPSILON);
    assert!(
        (DEFAULT_AUDIO_OFFSET_MS - 60.0).abs() < f32::EPSILON,
        "the maintainer's default"
    );
}
