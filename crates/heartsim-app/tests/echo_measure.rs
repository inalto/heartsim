//! B34 acceptance (UI and app level, no GPU): the Measure section of the Echo page. The page is
//! drawn by egui_kittest without a render state; the pose, the M-mode scan, the Doppler scan, the
//! calipers, the list and the references are the real ones.
//!
//! * the cursor, the sweep, freeze and scrub; each caliper kind; the list of at most four; the
//!   difference shown is measured minus the model's value; no reference where the model has none;
//! * no blood Doppler of any kind among the page's controls, in both languages;
//! * the trace does not move the sector's rectangle; the section fits at 1280 × 800 and at 400 × 800;
//! * reduced motion holds the traces; nothing measured is in the saved settings.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use heartsim_app::echo_page::{Demo, EchoPage, EchoSection, MeasureMode, Tool};
use heartsim_app::heart_view::HeartView;
use heartsim_core::{Auscultation, Millis};
use heartsim_echo::{AnnulusSite, Named, SweepSpeed, WaveKind};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::{Engine, EventKind, TimingEngine, TimingParams};
use heartsim_ui::i18n;
use std::path::Path;
use std::sync::OnceLock;

fn pack() -> AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| {
        AnatomyPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default"))
            .expect("default pack loads")
    })
    .clone()
}

struct St {
    page: EchoPage,
    heart: HeartView,
    timing: TimingEngine,
    t_ms: f64,
}

fn beating(seconds: f64) -> (HeartView, TimingEngine, Vec<f64>) {
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

fn sized(lang: &str, size: (f32, f32), seconds: f64) -> Harness<'static, St> {
    let (heart, timing, v) = beating(seconds);
    i18n::set_language(lang);
    let st = St {
        page: EchoPage::default(),
        heart,
        timing,
        t_ms: v[v.len() - 4] + 5.0,
    };
    let mut h = Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(size.0, size.1))
        .with_step_dt(0.1)
        .build_ui_state(
            |ui, st: &mut St| {
                let phase = st.timing.respiratory_phase(Millis(st.t_ms));
                st.heart.set_motion_time(st.t_ms, phase);
                st.page.ui(ui, Some(&mut st.heart), None, false);
            },
            st,
        );
    h.run_steps(8);
    h
}

/// Advance the drawn time by `ms` in frames of `frame_ms`.
fn play(h: &mut Harness<'static, St>, ms: f64, frame_ms: f64) {
    let n = (ms / frame_ms).round() as usize;
    for _ in 0..n {
        h.state_mut().t_ms += frame_ms;
        h.run_steps(1);
    }
}

const WIDE: (f32, f32) = (1280.0, 590.0);

#[test]
fn the_cursor_records_an_m_mode_trace_that_freezes_and_scrubs() {
    let mut h = sized("en", WIDE, 7.0);
    h.state_mut().page.set_measure_mode(MeasureMode::MMode);
    h.state_mut().page.set_sweep_speed(SweepSpeed::Mm50);
    h.state_mut().page.set_cursor_deg(5.0);
    assert_eq!(h.state().page.section(), EchoSection::Measure);
    assert!((h.state().page.cursor_deg() - 5.0).abs() < 1e-6);
    play(&mut h, 600.0, 40.0);
    let n0 = h.state().page.mmode_columns();
    assert!(n0 >= 10, "the trace records ({n0} columns)");
    play(&mut h, 400.0, 40.0);
    let n1 = h.state().page.mmode_columns();
    assert!(n1 > n0, "and goes on ({n0} -> {n1})");
    // freeze: nothing more is recorded while the clock runs on
    h.state_mut().page.set_frozen(true);
    play(&mut h, 400.0, 40.0);
    assert_eq!(h.state().page.mmode_columns(), n1, "frozen");
    assert!(h.state().page.frozen());
    // scrub: the window's end moves back
    let (newest, _) = h.state().page.trace_window().expect("a window");
    h.state_mut().page.set_frozen(true);
    // the slider moves the scrub: drag it with the keyboard
    let scrub_name = heartsim_ui::t!("echo-measure-scrub");
    let slider = h.get_by_role_and_label(egui::accesskit::Role::Slider, &scrub_name);
    slider.focus();
    h.key_press(egui::Key::ArrowRight);
    h.run_steps(2);
    let (scrubbed, span) = h.state().page.trace_window().expect("a window");
    assert!(span > 500.0, "the window has a length ({span} ms)");
    assert!(scrubbed <= newest + 1e-6);
    // releasing the freeze resumes it
    h.state_mut().page.set_frozen(false);
    play(&mut h, 200.0, 40.0);
    assert!(h.state().page.mmode_columns() > n1);
}

#[test]
fn the_faster_trace_records_every_other_column_and_says_so() {
    let mut full = sized("en", WIDE, 7.0);
    full.state_mut().page.set_measure_mode(MeasureMode::MMode);
    play(&mut full, 1000.0, 40.0);
    let n_full = full.state().page.mmode_columns();
    assert!(full.query_by_label_contains("Faster M-mode trace").is_none());

    let mut fast = sized("en", WIDE, 7.0);
    assert!(!fast.state().page.fast_mmode(), "off by default");
    fast.state_mut().page.set_fast_mmode(true);
    fast.state_mut().page.set_measure_mode(MeasureMode::MMode);
    play(&mut fast, 1000.0, 40.0);
    let n_fast = fast.state().page.mmode_columns();
    assert!(
        n_fast >= 15 && n_fast * 2 <= n_full + 2,
        "half the columns ({n_fast} against {n_full})"
    );
    // the panel says why the trace is coarser
    fast.get_by_label_contains("Faster M-mode trace");
}

#[test]
fn the_sector_keeps_its_rectangle_when_a_trace_appears() {
    let mut h = sized("en", WIDE, 6.0);
    h.state_mut().page.set_section(EchoSection::Measure);
    h.state_mut().page.set_measure_mode(MeasureMode::MMode);
    h.run_steps(3);
    let waiting = h.state().page.section_rect().expect("drawn");
    play(&mut h, 800.0, 40.0);
    assert_eq!(
        h.state().page.section_rect(),
        Some(waiting),
        "M-mode: recording does not move it"
    );
    h.state_mut().page.set_measure_mode(MeasureMode::Doppler);
    h.run_steps(3);
    let d = h.state().page.section_rect().expect("drawn");
    h.state_mut().page.request_gate(AnnulusSite::MitralLateral);
    play(&mut h, 600.0, 40.0);
    assert_eq!(
        h.state().page.section_rect(),
        Some(d),
        "Doppler: the gate and the trace do not move it"
    );
    assert_eq!(d, waiting, "the two trace modes reserve the same space");
}

#[test]
fn every_caliper_kind_reads_what_the_arithmetic_says() {
    let mut h = sized("en", WIDE, 6.0);
    let p = &mut h.state_mut().page;
    // the sector: a distance and an area
    p.set_measure_mode(MeasureMode::Plane);
    p.set_tool(Tool::Distance);
    p.place_caliper_point((0.0, 50.0));
    assert!(p.frozen(), "a click freezes");
    assert!(p.reading().is_none(), "one point is not a distance");
    p.place_caliper_point((30.0, 90.0));
    let r = p.reading().expect("two points");
    assert!((r.value - 50.0).abs() < 1e-4 && r.unit == "mm");
    p.set_tool(Tool::Area);
    for k in 0..36 {
        let a = f64::from(k) / 36.0 * std::f64::consts::TAU;
        p.place_caliper_point((20.0 * a.cos(), 80.0 + 20.0 * a.sin() as f32));
    }
    let r = p.reading().expect("an outline");
    let want = std::f32::consts::PI * 4.0;
    assert!((r.value - want).abs() / want < 0.02 && r.unit == "cm²", "{r:?}");
    // the M-mode trace: depth, time, slope
    p.set_measure_mode(MeasureMode::MMode);
    p.set_tool(Tool::Depth);
    p.place_caliper_point((1000.0, 40.0));
    p.place_caliper_point((1400.0, 66.0));
    assert!((p.reading().unwrap().value - 26.0).abs() < 1e-4);
    p.set_tool(Tool::Interval);
    p.place_caliper_point((1000.0, 40.0));
    p.place_caliper_point((1400.0, 66.0));
    assert!((p.reading().unwrap().value - 400.0).abs() < 1e-3);
    p.set_tool(Tool::Slope);
    p.place_caliper_point((1000.0, 40.0));
    p.place_caliper_point((1400.0, 66.0));
    // 26 mm in 0.4 s = 6.5 cm/s
    assert!((p.reading().unwrap().value - 6.5).abs() < 1e-3);
    // the Doppler trace: velocity and time
    p.set_measure_mode(MeasureMode::Doppler);
    p.set_tool(Tool::Velocity);
    p.place_caliper_point((1200.0, -8.4));
    let r = p.reading().unwrap();
    assert!((r.value + 8.4).abs() < 1e-4 && r.unit == "cm/s");
    p.set_tool(Tool::Interval);
    p.place_caliper_point((1200.0, 0.0));
    p.place_caliper_point((1500.0, 0.0));
    assert!((p.reading().unwrap().value - 300.0).abs() < 1e-3);
}

#[test]
fn the_list_holds_four_and_shows_measured_minus_the_models_value() {
    let mut h = sized("en", WIDE, 7.0);
    h.state_mut().page.request_demo(Demo::Lvid);
    h.run_steps(4);
    play(&mut h, 2200.0, 40.0);
    h.state_mut().page.finish_demo();
    h.run_steps(3);
    let list = h.state().page.measurements().to_vec();
    assert_eq!(list.len(), 2, "the demo measured LVIDd and LVIDs");
    assert_eq!(list[0].name, Named::Lvidd);
    assert_eq!(list[1].name, Named::Lvids);
    for e in &list {
        let r = e.feedback.reference.expect("the model has this one");
        let d = e.feedback.difference.expect("so there is a difference");
        assert!((d - (e.value - r)).abs() < 1e-4, "{e:?}");
        assert!(d.abs() < 4.0, "the hand was a few mm off: {d}");
    }
    assert!(list[0].value > list[1].value + 4.0, "LVIDs under LVIDd: {list:?}");
    assert!(h.query_all_by_label_contains("LVIDd").count() >= 1);
    assert_eq!(
        h.query_all_by_label_contains("The model has").count(),
        2,
        "one line per measurement"
    );
    // four at most
    let st = h.state_mut();
    for _ in 0..3 {
        st.page.set_tool(Tool::Depth);
        st.page.place_caliper_point((1000.0, 40.0));
        st.page.place_caliper_point((1000.0, 70.0));
        st.page.add_measurement(&mut st.heart, Named::Ivsd);
    }
    assert_eq!(st.page.measurements().len(), 4, "full");
    st.page.set_tool(Tool::Depth);
    st.page.place_caliper_point((1000.0, 40.0));
    st.page.place_caliper_point((1000.0, 70.0));
    assert!(
        !st.page.add_measurement(&mut st.heart, Named::Ivsd),
        "the fifth is refused"
    );
}

#[test]
fn a_measurement_the_model_has_no_value_for_gets_no_reference() {
    let mut h = sized("en", WIDE, 7.0);
    // the sector at the apical start: a caliper in the LV named as the aortic root: the model has
    // nothing to compare with
    let st = h.state_mut();
    st.page.set_measure_mode(MeasureMode::Plane);
    st.page.set_tool(Tool::Distance);
    st.page.place_caliper_point((0.0, 60.0));
    st.page.place_caliper_point((0.0, 70.0));
    assert!(st.page.add_measurement(&mut st.heart, Named::AorticRoot));
    let e = &st.page.measurements()[0];
    assert_eq!(e.feedback.reference, None);
    assert_eq!(e.feedback.difference, None);
    // and the wrong kind of name is refused: a velocity name on a distance
    st.page.place_caliper_point((0.0, 60.0));
    st.page.place_caliper_point((0.0, 70.0));
    assert!(!st.page.add_measurement(&mut st.heart, Named::EPrime));
}

#[test]
fn tissue_doppler_records_at_an_annulus_and_marks_the_waves() {
    let mut h = sized("en", WIDE, 8.0);
    h.state_mut().page.request_demo(Demo::EPrime);
    h.run_steps(4);
    assert_eq!(
        h.state().page.gate_site(),
        Some(AnnulusSite::MitralLateral),
        "snapped to the annulus"
    );
    play(&mut h, 2400.0, 40.0);
    assert!(h.state().page.doppler_samples() > 100);
    let angle = h
        .state()
        .page
        .doppler_angle_deg()
        .expect("the angle at the S' peak");
    assert!(
        (0.0..40.0).contains(&angle),
        "the beam is nearly along the annulus's motion: {angle}"
    );
    assert!(
        h.query_all_by_label_contains("not corrected").count() >= 1,
        "the angle is shown, uncorrected"
    );
    let marks = h.state().page.doppler_markers().to_vec();
    assert!(
        marks.iter().any(|m| m.kind == WaveKind::S && m.cm_s > 3.0),
        "{marks:?}"
    );
    assert!(
        marks.iter().any(|m| m.kind == WaveKind::E && m.cm_s < -3.0),
        "{marks:?}"
    );
    h.state_mut().page.finish_demo();
    h.run_steps(3);
    let list = h.state().page.measurements();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, Named::EPrime);
    assert_eq!(list[0].site, Some(AnnulusSite::MitralLateral));
    let d = list[0].feedback.difference.expect("e′ has a model value");
    assert!(d.abs() < 2.0, "{d}");
}

#[test]
fn reduced_motion_holds_the_traces() {
    let mut h = sized("en", WIDE, 6.0);
    h.state_mut().page.set_measure_mode(MeasureMode::MMode);
    play(&mut h, 600.0, 40.0);
    let n = h.state().page.mmode_columns();
    assert!(n > 0);
    h.state_mut().heart.set_motion_options(true, true, 1.0);
    play(&mut h, 600.0, 40.0);
    assert_eq!(
        h.state().page.mmode_columns(),
        n,
        "reduced motion: the trace holds still"
    );
    h.state_mut().heart.set_motion_options(false, true, 1.0);
    play(&mut h, 400.0, 40.0);
    assert!(h.state().page.mmode_columns() > n);
}

/// The controls of the page (buttons, sliders, combo boxes, switches), by their names.
fn control_names(h: &Harness<'static, St>) -> Vec<String> {
    use egui::accesskit::Role;
    [
        Role::Button,
        Role::Slider,
        Role::ComboBox,
        Role::CheckBox,
        Role::RadioButton,
        Role::Switch,
    ]
    .into_iter()
    .flat_map(|role| h.query_all_by_role(role).collect::<Vec<_>>())
    .filter_map(|n| n.accesskit_node().label())
    .collect()
}

#[test]
fn there_is_no_blood_doppler_control_anywhere_on_the_page() {
    for (lang, banned) in [
        (
            "en",
            vec![
                "blood",
                "spectral",
                "colour",
                "color",
                "flow",
                "regurgit",
                "jet",
                "pulsed",
                "continuous",
            ],
        ),
        (
            "it",
            vec![
                "sangue", "spettral", "colore", "flusso", "rigurgit", "getto", "pulsat", "continuo",
            ],
        ),
    ] {
        let mut h = sized(lang, WIDE, 6.0);
        for mode in MeasureMode::ALL {
            h.state_mut().page.set_measure_mode(mode);
            h.run_steps(3);
            let names = control_names(&h);
            assert!(names.len() > 5, "{lang} {mode:?}: {names:?}");
            for n in &names {
                for b in &banned {
                    assert!(
                        !n.to_lowercase().contains(b),
                        "{lang} {mode:?}: control {n:?} suggests blood Doppler"
                    );
                }
            }
        }
        // the three annulus gates are the only places a Doppler can be put
        h.state_mut().page.set_measure_mode(MeasureMode::Doppler);
        h.run_steps(3);
        for s in AnnulusSite::ALL {
            let _ = h.get_by_label(&heartsim_app::echo_page::site_name(s));
        }
    }
    i18n::set_language("en");
}

#[test]
fn the_measure_section_fits_wide_and_phone_narrow_in_both_languages() {
    for lang in ["en", "it"] {
        for size in [WIDE, (400.0, 800.0)] {
            let mut h = sized(lang, size, 5.0);
            h.state_mut().page.set_measure_mode(MeasureMode::MMode);
            h.state_mut().page.set_section(EchoSection::Measure);
            play(&mut h, 400.0, 40.0);
            for mode in MeasureMode::ALL {
                h.state_mut().page.set_measure_mode(mode);
                play(&mut h, 200.0, 40.0);
                if mode.is_trace() {
                    let r = h.state().page.trace_rect().expect("the trace is drawn");
                    assert!(
                        r.right() <= size.0 + 0.5 && r.left() >= 0.0,
                        "{lang} {size:?} {mode:?}: trace {r:?} outside the window"
                    );
                    let s = h.state().page.section_rect().expect("the sector is drawn");
                    assert!(
                        s.right() <= size.0 + 0.5,
                        "{lang} {size:?} {mode:?}: sector {s:?}"
                    );
                }
                for role in [
                    egui::accesskit::Role::Button,
                    egui::accesskit::Role::Slider,
                    egui::accesskit::Role::ComboBox,
                ] {
                    for n in h.query_all_by_role(role) {
                        assert!(
                            n.rect().right() <= size.0 + 1.0,
                            "{lang} {size:?} {mode:?}: {:?} past the right edge ({:?})",
                            n.accesskit_node().label(),
                            n.rect()
                        );
                    }
                }
            }
        }
    }
    i18n::set_language("en");
}

#[test]
fn nothing_measured_is_in_the_saved_settings() {
    let mut h = sized("en", WIDE, 6.0);
    h.state_mut().page.request_demo(Demo::Lvid);
    h.run_steps(4);
    play(&mut h, 2200.0, 40.0);
    h.state_mut().page.finish_demo();
    h.run_steps(3);
    assert_eq!(h.state().page.measurements().len(), 2);
    let saved = heartsim_app::settings::Settings::default().to_storage_string();
    for key in ["lvid", "measure", "caliper", "cursor", "gate", "sweep", "tapse"] {
        assert!(!saved.to_lowercase().contains(key), "the settings mention {key}");
    }
    // what the page itself stores (the quiz record) holds no measurement either
    let quiz = h.state().page.quiz_string().to_lowercase();
    for key in ["lvid", "measure", "caliper"] {
        assert!(!quiz.contains(key), "the stored quiz record mentions {key}");
    }
}

#[test]
fn snapping_is_off_by_default_and_moves_a_point_to_a_tissue_boundary_within_3_mm() {
    use heartsim_app::echo_page::{SNAP_MM, snap_depth, snap_plane};
    use heartsim_echo::{LabelImage, MModeColumn, Plane2};
    use heartsim_geometry::Label;
    let h = sized("en", WIDE, 5.0);
    assert!(!h.state().page.snap(), "off by default");
    // an M-mode column: blood to 50 mm, then wall
    let col = MModeColumn {
        step: 0,
        t_ms: 0.0,
        contact: true,
        depth_mm: 100.0,
        grey: vec![0; 200],
        labels: (0..200)
            .map(|i| {
                if i < 100 {
                    Label::LvBlood as u8
                } else {
                    Label::LvMyocardium as u8
                }
            })
            .collect(),
        hidden_from: 200,
    };
    assert!((snap_depth(&col, 48.5) - 50.0).abs() < 0.3);
    assert!((snap_depth(&col, 52.0) - 50.0).abs() < 0.3);
    assert!(
        (snap_depth(&col, 40.0) - 40.0).abs() < 1e-6,
        "farther than {SNAP_MM} mm: unchanged"
    );
    // a sector: the same boundary on every beam
    let mut img = LabelImage::new(64, 200);
    img.contact = true;
    img.depth_mm = 100.0;
    img.sector_deg = 90.0;
    for line in 0..64 {
        for s in 0..200 {
            img.labels[line * 200 + s] = if s < 100 {
                Label::LvBlood as u8
            } else {
                Label::LvMyocardium as u8
            };
        }
    }
    let p = snap_plane(&img, Plane2::new(10.0, 48.0));
    let r = p.x_mm.hypot(p.z_mm);
    assert!((r - 50.0).abs() < 0.8, "snapped to the boundary at 50 mm: {r}");
    let q = Plane2::new(10.0, 30.0);
    assert_eq!(snap_plane(&img, q), q, "no boundary within reach");
}

/// The phone layout (400 x 800): the image is always in view, at a useful size, with the trace under
/// it while M-mode is on; the 3-D chest is one switch away and nothing is lost by going there.
#[test]
fn the_phone_layout_keeps_the_image_and_the_trace_in_view_and_the_3d_view_one_switch_away() {
    let top = |h: &Harness<'static, St>, name: &str| {
        h.query_all_by_label(name)
            .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
            .map(|n| n.rect())
    };
    // 800 px is the window; 440 px is what the app leaves the page of a 400 x 800 window under its
    // rhythm bar and strip
    for (lang, size) in [
        ("en", (400.0_f32, 800.0_f32)),
        ("it", (400.0, 800.0)),
        ("en", (400.0, 440.0)),
        ("it", (400.0, 440.0)),
    ] {
        let tall = size.1 > 600.0;
        let mut h = sized(lang, size, 5.0);
        let inside =
            |r: egui::Rect| r.left() >= 0.0 && r.right() <= size.0 + 0.5 && r.bottom() <= size.1 + 0.5;
        let img = h.state().page.section_rect().expect("the image is drawn");
        assert!(
            inside(img) && img.width() >= 360.0 && img.height() >= 200.0,
            "{lang} {size:?}: {img:?}"
        );
        // the four sections fit the control
        for s in EchoSection::ALL {
            let r = top(&h, &s.name()).unwrap_or_else(|| panic!("{lang}: section {s:?} missing"));
            assert!(
                r.right() <= size.0 + 0.5 && r.width() >= 40.0,
                "{lang}: {s:?} {r:?}"
            );
        }
        h.state_mut().page.set_measure_mode(MeasureMode::MMode);
        h.state_mut().page.set_cursor_deg(7.0);
        play(&mut h, 600.0, 40.0);
        let img = h.state().page.section_rect().expect("drawn");
        let tr = h.state().page.trace_rect().expect("the trace is drawn");
        let min_h = if tall { 220.0 } else { 160.0 };
        assert!(
            inside(img) && img.width() >= 360.0 && img.height() >= min_h,
            "{lang} {size:?} with the trace: {img:?}"
        );
        assert!(
            inside(tr) && tr.height() >= 100.0 && tr.top() >= img.bottom(),
            "{lang} {size:?}: trace {tr:?} under {img:?}"
        );
        // the section control stays reachable under them
        let tab = top(&h, &EchoSection::Measure.name()).expect("the Measure section");
        assert!(inside(tab), "{lang} {size:?}: the section control {tab:?}");
        let before = h.state().page.mmode_columns();
        assert!(before > 0);
        // to the chest and back
        let chest = heartsim_ui::t!("echo-narrow-3d");
        h.get_by_label(&chest).click();
        h.run_steps(3);
        assert_eq!(h.state().page.measure_mode(), MeasureMode::MMode);
        assert!((h.state().page.cursor_deg() - 7.0).abs() < 1e-6);
        let back = top(&h, &heartsim_ui::t!("echo-narrow-image")).expect("the image switch");
        h.get_all_by_label(&heartsim_ui::t!("echo-narrow-image"))
            .find(|n| n.rect() == back)
            .expect("switch")
            .click();
        play(&mut h, 200.0, 40.0);
        assert!(h.state().page.trace_rect().is_some());
        assert!(
            h.state().page.mmode_columns() >= before,
            "the trace kept its columns"
        );
        assert_eq!(h.state().page.measure_mode(), MeasureMode::MMode);
        for role in [
            egui::accesskit::Role::Button,
            egui::accesskit::Role::Slider,
            egui::accesskit::Role::ComboBox,
        ] {
            for n in h.query_all_by_role(role) {
                assert!(
                    n.rect().right() <= size.0 + 1.0,
                    "{lang}: {:?} past the edge",
                    n.accesskit_node().label()
                );
            }
        }
    }
    i18n::set_language("en");
}
