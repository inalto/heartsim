//! Brief B35 at app level (no GPU): the "Tricuspid valve" layer of *Layers and view* exists in
//! English and Italian, is off by default, and its state survives a restart (the preferences);
//! a pack without the valve draws the heart as before; the valve the 3-D heart would draw is open
//! or shut exactly as the echo cross-section's tricuspid valve at the same instant.
#![allow(
    clippy::float_cmp,
    reason = "the valve must be exactly shut at opening 0 and open at 1"
)]

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::heart_view::{HeartView, TRICUSPID_VALVE_LAYER};
use heartsim_app::settings::{Settings, SimulatorSections};
use heartsim_core::{Auscultation, Millis};
use heartsim_ecg::Valve;
use heartsim_echo::SliceState;
use heartsim_geometry::AnatomyPack;
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::i18n;
use std::path::Path;

fn pack() -> AnatomyPack {
    AnatomyPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default"))
        .expect("default pack loads")
}

struct Panel {
    heart: HeartView,
    sections: SimulatorSections,
}

fn harness(heart: HeartView, lang: &str) -> Harness<'static, Panel> {
    i18n::set_language(lang);
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(380.0, 1400.0))
        .build_ui_state(
            |ui, p: &mut Panel| p.heart.controls(ui, &mut p.sections),
            Panel {
                heart,
                sections: SimulatorSections::default(),
            },
        )
}

/// The app's start: a heart view with the stored preferences applied (as `sync_wall_motion_in`).
fn start(settings: &Settings) -> HeartView {
    let mut heart = HeartView::from_pack(pack(), &TimingParams::default());
    heart.set_tricuspid_valve_visible(settings.tricuspid_valve);
    heart
}

#[test]
fn the_layer_toggle_is_translated_off_by_default_and_survives_a_restart() {
    let settings = Settings::default();
    assert!(!settings.tricuspid_valve, "off by default");
    let mut h = harness(start(&settings), "en");
    h.run();
    let _ = h.get_by_label("Tricuspid valve");
    assert_eq!(h.state().heart.tricuspid_valve_visible(), Some(false));
    // switched on in the panel, then stored as the app does after the UI
    h.get_by_label("Tricuspid valve").click();
    h.run();
    assert_eq!(h.state().heart.tricuspid_valve_visible(), Some(true));
    let mut stored = settings.clone();
    stored.tricuspid_valve = h.state().heart.tricuspid_valve_visible().unwrap();
    let text = stored.to_storage_string();
    drop(h);
    // a restart: the preferences read back, a new heart view
    let back: Settings = ron::from_str(&text).expect("the stored preferences load");
    assert!(back.tricuspid_valve);
    let mut h = harness(start(&back), "it");
    h.run();
    let _ = h.get_by_label("Valvola tricuspide");
    assert_eq!(h.state().heart.tricuspid_valve_visible(), Some(true));
    i18n::set_language("en");
}

#[test]
fn a_pack_without_the_valve_draws_the_heart_as_before() {
    let mut p = pack();
    let names: Vec<String> = p.meshes.iter().map(|m| m.name.clone()).collect();
    p.valves.clear();
    let heart = HeartView::from_pack(p, &TimingParams::default());
    assert_eq!(heart.tricuspid_valve_visible(), None);
    let layers: Vec<&str> = heart.panel.layers.iter().map(|l| l.name.as_str()).collect();
    let mut expected: Vec<&str> = names.iter().map(String::as_str).collect();
    expected.push("torso");
    assert_eq!(layers, expected, "the layer list of a pack without the valve");
    assert!(heart.tricuspid_valve_pose().is_none());
    // with the valve, the one extra layer sits before the torso
    let with = HeartView::from_pack(pack(), &TimingParams::default());
    let n = with.panel.layers.len();
    assert_eq!(with.panel.layers[n - 2].name, TRICUSPID_VALVE_LAYER);
}

#[test]
fn the_drawn_valve_and_the_echo_agree_on_open_and_shut() {
    let params = TimingParams::default();
    let mut timing = TimingEngine::new(params.clone());
    let mut heart = HeartView::from_pack(pack(), &params);
    heart.reset_motion(params.sinus.respiration_rate_bpm, &Auscultation::default());
    let dt = Millis(1.0);
    for _ in 0..4000 {
        timing.step(dt);
        let events = timing.drain_events();
        heart.on_step(&events, dt, timing.now());
        heart.motion_events(&events, timing.respiration());
        heart.motion_record_cells();
    }
    heart.build_echo();
    let mut st = SliceState::new(heart.echo_anatomy().expect("echo anatomy"));
    let v = heart
        .motion()
        .and_then(|m| m.valve())
        .expect("the default pack's valve")
        .data
        .clone();
    let (mut shut, mut open) = (Vec::new(), Vec::new());
    v.keyframe_positions(v.keyframe_count() - 1, &mut shut);
    v.keyframe_positions(0, &mut open);
    let (mut n_open, mut n_shut) = (0, 0);
    for (k, on) in [(0usize, true), (1, false)] {
        heart.set_motion_options(false, on, 1.0);
        let mut wall = 10.0 + 100.0 * k as f64;
        let mut t = 2800.0;
        while t < 3800.0 {
            heart.set_motion_time(t, timing.respiratory_phase(Millis(t)));
            heart.tick_motion(wall, true, false);
            wall += 1.0;
            let (o, drawn) = heart.tricuspid_valve_pose().expect("a pose");
            let m = heart.motion().unwrap();
            m.echo_update(&mut st, heart.echo_anatomy().unwrap());
            assert_eq!(
                st.openings()[Valve::Tricuspid.index()],
                o,
                "the echo and the 3-D heart at {t}"
            );
            if !on {
                // the wall at rest: the drawn valve is the posed rest shape itself
                if o == 0.0 {
                    assert_eq!(drawn, shut, "shut at {t}");
                    n_shut += 1;
                } else if o == 1.0 {
                    assert_eq!(drawn, open, "open at {t}");
                    n_open += 1;
                }
            }
            t += 20.0;
        }
    }
    assert!(n_open > 0 && n_shut > 0, "open {n_open}, shut {n_shut}");
}
