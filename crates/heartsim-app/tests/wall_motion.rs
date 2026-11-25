//! B30 acceptance (UI and app level, no GPU): the "Wall motion" switch, the exaggeration slider
//! (capped at 1.5x) and the read-out card of the heart panel; reduced motion switches the motion
//! off; with the switch off the frame the app would upload is the rest mesh bit for bit; after a
//! few seconds of sinus rhythm the frame moves and the card reads a physiological LVEF; a new
//! scenario clears the record; the evaluation policy (rate cap, nothing when nothing changed, one
//! rest upload after the switch goes off).

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use heartsim_app::heart_motion::{self, MAX_EXAGGERATION, min_interval_s};
use heartsim_app::heart_view::HeartView;
use heartsim_core::{Auscultation, Millis};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::{Engine, EventKind, TimingEngine, TimingParams};
use heartsim_ui::i18n;
use std::path::Path;

fn pack() -> AnatomyPack {
    AnatomyPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default"))
        .expect("default pack loads")
}

/// A heart that has run `seconds` of the default (sinus) rhythm, with the times of its ventricular
/// activations.
fn beating_heart(seconds: f64) -> (HeartView, TimingEngine, Vec<f64>) {
    let params = TimingParams::default();
    let mut timing = TimingEngine::new(params.clone());
    let mut heart = HeartView::from_pack(pack(), &params);
    heart.reset_motion(params.sinus.respiration_rate_bpm, &Auscultation::default());
    let dt = Millis(1.0);
    let mut v_times: Vec<f64> = Vec::new();
    for _ in 0..(seconds * 1000.0) as usize {
        timing.step(dt);
        let events = timing.drain_events();
        for e in &events {
            if e.region.is_ventricular()
                && e.kind != EventKind::Blocked
                && v_times.last().is_none_or(|&l| e.at.0 - l > 150.0)
            {
                v_times.push(e.at.0);
            }
        }
        heart.on_step(&events, dt, timing.now());
        heart.motion_events(&events, timing.respiration());
        heart.motion_record_cells();
    }
    (heart, timing, v_times)
}

fn draw_at(heart: &mut HeartView, timing: &TimingEngine, t_ms: f64, wall_s: f64) {
    heart.set_motion_time(t_ms, timing.respiratory_phase(Millis(t_ms)));
    heart.tick_motion(wall_s, true, true);
}

fn differs_from_rest(heart: &HeartView) -> bool {
    let m = heart.motion().expect("motion model");
    m.frame().parts.iter().any(|p| {
        m.model()
            .part_rest(&p.name)
            .is_some_and(|rest| rest != p.vertices.as_slice())
    })
}

#[test]
fn after_a_few_seconds_of_sinus_rhythm_the_frame_moves_and_the_lvef_is_physiological() {
    let (mut heart, timing, v) = beating_heart(6.0);
    assert!(v.len() >= 4, "sinus beats: {v:?}");
    let tv = v[v.len() - 2];
    // mid-ejection: the wall is away from rest
    draw_at(&mut heart, &timing, tv + 200.0, 1.0);
    assert!(
        differs_from_rest(&heart),
        "the frame the app uploads moves in systole"
    );
    // the end of the beat: its EF is the card's LVEF
    draw_at(&mut heart, &timing, tv + 600.0, 2.0);
    let r = *heart.motion().unwrap().readout();
    let ef = r.lvef.expect("an ejection fraction after a beat");
    assert!((0.55..=0.70).contains(&ef), "LVEF {ef}");
    assert!(r.lv_edv_ml > r.lv_esv_ml && r.lv_esv_ml > 0.0, "{r:?}");
}

#[test]
fn with_the_switch_off_the_uploaded_frame_is_the_rest_mesh_bit_for_bit() {
    let (mut heart, timing, v) = beating_heart(4.0);
    let t = v[v.len() - 1] + 200.0;
    draw_at(&mut heart, &timing, t, 1.0);
    assert!(differs_from_rest(&heart));
    heart.set_motion_options(false, false, 1.0);
    draw_at(&mut heart, &timing, t, 2.0);
    assert!(!differs_from_rest(&heart));
    let m = heart.motion().unwrap();
    for p in &m.frame().parts {
        assert_eq!(
            m.model().part_rest(&p.name).unwrap(),
            p.vertices.as_slice(),
            "{} is the rest mesh",
            p.name
        );
    }
    // reduced motion forces it off although the switch is on
    heart.set_motion_options(false, true, 1.0);
    draw_at(&mut heart, &timing, t, 3.0);
    assert!(differs_from_rest(&heart));
    heart.set_motion_options(true, true, 1.0);
    draw_at(&mut heart, &timing, t, 4.0);
    assert!(
        !differs_from_rest(&heart),
        "reduced motion switches the motion off"
    );
    // the choice is remembered for when reduced motion goes off again
    assert_eq!(heart.motion_options(), Some((true, 1.0)));
}

#[test]
fn a_new_scenario_clears_the_record_and_the_card() {
    let (mut heart, timing, v) = beating_heart(4.0);
    let t = v[v.len() - 1] + 600.0;
    draw_at(&mut heart, &timing, t, 1.0);
    assert!(heart.motion().unwrap().readout().lvef.is_some());
    assert!(heart.motion().unwrap().history().beats().len() >= 3);
    heart.reset_motion(15.0, &Auscultation::default());
    assert_eq!(heart.motion().unwrap().history().beats().len(), 0);
    draw_at(&mut heart, &timing, t, 2.0);
    let r = *heart.motion().unwrap().readout();
    assert!(r.lvef.is_none() && r.mapse_mm.is_none(), "{r:?}");
}

#[test]
fn the_evaluation_is_rate_capped_idle_when_nothing_changed_and_uploads_rest_once() {
    use heart_motion::Tick;
    let (mut heart, timing, v) = beating_heart(4.0);
    let t0 = v[v.len() - 1] + 100.0;
    heart.set_motion_time(t0, timing.respiratory_phase(Millis(t0)));
    let mot = heart.motion_mut().unwrap();
    assert_eq!(mot.tick(10.0, true, true), Tick::Evaluated);
    assert!(
        mot.take_upload().is_some() && mot.take_upload().is_none(),
        "one upload per evaluation"
    );
    // nothing changed: nothing is done, however often it is asked
    assert_eq!(mot.tick(10.5, true, true), Tick::Unchanged);
    // the time moved but the cap says wait
    mot.set_time(t0 + 16.0, 0.0);
    match mot.tick(10.0 + min_interval_s() / 2.0, true, true) {
        Tick::Wait(s) => assert!(s > 0.0 && s <= min_interval_s()),
        other => panic!("expected the rate cap to hold it back, got {other:?}"),
    }
    assert_eq!(
        mot.tick(10.0 + min_interval_s() * 1.01, true, true),
        Tick::Evaluated
    );
    // switched off: one rest upload, then idle (no card: nothing at all)
    mot.on = false;
    assert_eq!(mot.tick(11.0, true, false), Tick::Evaluated);
    assert!(mot.take_upload().is_some());
    mot.set_time(t0 + 500.0, 0.0);
    assert_eq!(mot.tick(12.0, true, false), Tick::Unchanged);
    assert!(mot.take_upload().is_none());
    // the card alone: the read-out only, no vertex written
    assert_eq!(mot.tick(13.0, true, true), Tick::ReadOut);
    assert!(mot.take_upload().is_none());
    // the cap is the designed one
    assert!(min_interval_s() >= 1.0 / 30.0 - 1e-12);
}

struct Panel {
    heart: HeartView,
}

fn panel_harness(reduced: bool) -> Harness<'static, Panel> {
    i18n::set_language("en");
    let mut heart = HeartView::from_pack(pack(), &TimingParams::default());
    heart.set_motion_options(reduced, true, 1.0);
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(360.0, 900.0))
        .build_ui_state(|ui, p: &mut Panel| p.heart.motion_controls(ui), Panel { heart })
}

#[test]
fn the_heart_panel_has_the_switch_the_capped_slider_and_the_readout_card() {
    let mut h = panel_harness(false);
    h.run();
    let _ = h.get_by_label("Wall motion");
    let slider = h.get_by_role_and_label(Role::Slider, "Exaggeration");
    let max = slider
        .accesskit_node()
        .max_numeric_value()
        .expect("a slider has a maximum");
    assert!(
        (max - f64::from(MAX_EXAGGERATION)).abs() < 1e-6 && max <= 1.5,
        "the slider stops at {max}"
    );
    assert!(
        (slider.accesskit_node().numeric_value().unwrap() - 1.0).abs() < 1e-6,
        "1x by default"
    );
    for label in ["LVEF", "LV EDV", "LV ESV", "MAPSE", "TAPSE"] {
        let _ = h.get_by_label(label);
    }
    // each value is labelled "model"; before any beat they all read the dash
    assert_eq!(h.get_all_by_label("model").count(), 5);
    assert_eq!(h.get_all_by_label("\u{2014}").count(), 5);
    // the switch turns the motion off in the heart view
    h.get_by_label("Wall motion").click();
    h.run();
    assert_eq!(h.state().heart.motion_options().map(|o| o.0), Some(false));
}

#[test]
fn reduced_motion_disables_the_switch_and_says_why() {
    let mut h = panel_harness(true);
    h.run();
    assert!(h.get_by_label("Wall motion").accesskit_node().is_disabled());
    let _ = h.get_by_label("Off while reduced motion is on (Settings).");
    assert!(!h.state().heart.motion().unwrap().active());
    assert!(
        h.get_by_role_and_label(Role::Slider, "Exaggeration")
            .accesskit_node()
            .is_disabled()
    );
}
