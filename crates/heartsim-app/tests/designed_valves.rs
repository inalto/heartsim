//! Brief B36 at app level (no GPU): the "Mitral valve", "Aortic valve" and "Pulmonary valve"
//! layers of *Anatomy and camera* exist beside "Tricuspid valve" in English and Italian, are on
//! at start-up (B37) and survive a restart (the preferences); a 0.25.0 settings file gets them on
//! once; the hint under the toggles says which valve is measured and which designed;
//! and each valve the 3-D heart would draw has the echo's opening at the same instant.
#![allow(
    clippy::float_cmp,
    reason = "the drawn and the echo opening must be the same value"
)]

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::heart_view::HeartView;
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
        .with_size(egui::vec2(380.0, 1600.0))
        .build_ui_state(
            |ui, p: &mut Panel| p.heart.controls(ui, &mut p.sections),
            Panel {
                heart,
                sections: SimulatorSections {
                    layers: true,
                    ..SimulatorSections::default()
                },
            },
        )
}

fn switches(s: &Settings) -> [(Valve, bool); 4] {
    [
        (Valve::Mitral, s.mitral_valve),
        (Valve::Tricuspid, s.tricuspid_valve),
        (Valve::Aortic, s.aortic_valve),
        (Valve::Pulmonic, s.pulmonary_valve),
    ]
}

/// The app's start: a heart view with the stored preferences applied (as `sync_wall_motion_in`).
fn start(settings: &Settings) -> HeartView {
    let mut heart = HeartView::from_pack(pack(), &TimingParams::default());
    for (v, on) in switches(settings) {
        heart.set_valve_visible(v, on);
    }
    heart
}

const EN: [(Valve, &str); 4] = [
    (Valve::Mitral, "Mitral valve"),
    (Valve::Tricuspid, "Tricuspid valve"),
    (Valve::Aortic, "Aortic valve"),
    (Valve::Pulmonic, "Pulmonary valve"),
];
const IT: [(Valve, &str); 4] = [
    (Valve::Mitral, "Valvola mitrale"),
    (Valve::Tricuspid, "Valvola tricuspide"),
    (Valve::Aortic, "Valvola aortica"),
    (Valve::Pulmonic, "Valvola polmonare"),
];

#[test]
fn the_four_toggles_are_translated_on_at_start_up_and_survive_a_restart() {
    let settings = Settings::default();
    let mut h = harness(start(&settings), "en");
    h.run();
    for (v, label) in EN {
        let _ = h.get_by_label(label);
        assert_eq!(
            h.state().heart.valve_visible(v),
            Some(true),
            "{v:?} on at start-up"
        );
    }
    // the hint under the toggles
    let _ = h.get_by_label_contains("one donor's measured valve");
    let _ = h.get_by_label_contains("designed shapes");
    // the mitral and the pulmonary switched off, then stored as the app does after the UI
    h.get_by_label("Mitral valve").click();
    h.get_by_label("Pulmonary valve").click();
    h.run();
    let mut stored = settings.clone();
    stored.mitral_valve = h.state().heart.valve_visible(Valve::Mitral).unwrap();
    stored.aortic_valve = h.state().heart.valve_visible(Valve::Aortic).unwrap();
    stored.pulmonary_valve = h.state().heart.valve_visible(Valve::Pulmonic).unwrap();
    stored.tricuspid_valve = h.state().heart.valve_visible(Valve::Tricuspid).unwrap();
    let text = stored.to_storage_string();
    drop(h);
    // a restart: the preferences read back, a new heart view, in Italian
    let back: Settings = ron::from_str(&text).expect("the stored preferences load");
    assert!(!back.mitral_valve && !back.pulmonary_valve && back.aortic_valve && back.tricuspid_valve);
    let mut h = harness(start(&back), "it");
    h.run();
    for (v, label) in IT {
        let _ = h.get_by_label(label);
        let want = matches!(v, Valve::Aortic | Valve::Tricuspid);
        assert_eq!(
            h.state().heart.valve_visible(v),
            Some(want),
            "{v:?} after the restart"
        );
    }
    let _ = h.get_by_label_contains("forme progettate");
    i18n::set_language("en");
}

#[test]
fn a_settings_file_from_0_25_0_gets_the_valves_on_once() {
    // a 0.25.0 file stores the tricuspid switch (off) and none of the designed valves; the
    // migration `Settings::load` runs switches all four on
    let old =
        ron::from_str::<Settings>("(wall_motion: true, echo_fast_mmode: false, tricuspid_valve: false)")
            .expect("a 0.25.0 settings file loads")
            .migrate_layer_defaults();
    let heart = start(&old);
    for v in [Valve::Tricuspid, Valve::Mitral, Valve::Aortic, Valve::Pulmonic] {
        assert_eq!(heart.valve_visible(v), Some(true), "{v:?}");
    }
}

#[test]
fn each_drawn_valve_has_the_echos_opening_at_the_same_instant() {
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
    let mut seen = [[false; 2]; 4];
    for (k, on) in [(0usize, true), (1, false)] {
        heart.set_motion_options(false, on, 1.0);
        let mut wall = 10.0 + 100.0 * k as f64;
        let mut t = 2800.0;
        while t < 3800.0 {
            heart.set_motion_time(t, timing.respiratory_phase(Millis(t)));
            heart.tick_motion(wall, true, false);
            wall += 1.0;
            let m = heart.motion().unwrap();
            m.echo_update(&mut st, heart.echo_anatomy().unwrap());
            for v in Valve::ALL {
                let (o, drawn) = heart.valve_pose(v).expect("every valve has leaflets");
                assert_eq!(
                    st.openings()[v.index()],
                    o,
                    "{v:?}: the echo and the 3-D heart at {t}"
                );
                let data = &m.valve(v).unwrap().data;
                if !on {
                    // the wall at rest: the drawn valve is the posed rest shape itself
                    let mut want = Vec::new();
                    data.pose_rest(o, &mut want);
                    assert_eq!(drawn, want, "{v:?} at {t}");
                }
                seen[v.index()][usize::from(o > 0.0)] = true;
            }
            t += 20.0;
        }
    }
    for v in Valve::ALL {
        assert!(
            seen[v.index()][0] && seen[v.index()][1],
            "{v:?} seen shut and open: {seen:?}"
        );
    }
}
