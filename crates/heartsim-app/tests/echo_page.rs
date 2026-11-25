//! B31 part B acceptance (UI and app level, no GPU): the Echo page. The page is drawn by
//! egui_kittest without a render state, so the 3-D view is replaced by a note and the
//! cross-section by egui's coarse fallback; the pose, the slice, the controls, the legend, the
//! read-out and the evaluation policy are the real ones.
//!
//! * the window buttons and the sliders change the pose; a pose beyond the contact limit shows
//!   "no contact" and an empty image;
//! * the legend lists every label present in the image, in both languages;
//! * the slice does no work when neither the evaluation, the pose nor (motion off) the time changed,
//!   and the page never evaluates the wall motion a second time;
//! * reduced motion freezes the wall (the leaflets still follow the beat), the wall moves with it on;
//! * the pose is not part of the saved settings;
//! * the grey image (brief B32): the display switch (image, anatomy, both), every image control
//!   changes the settings and costs exactly one recompute, reset, nothing is recomputed while
//!   nothing changed or while the grey image is not shown, reduced motion holds the frame, the
//!   settings are not stored, en and it, the phone layout;
//! * the colour tables keep every pair of labels apart, also for red-green colour-vision deficiency;
//! * the narrow (phone) layout stays inside the window.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use heartsim_app::echo_colours;
use heartsim_app::echo_page::{DisplayMode, EchoPage, EchoSection};
use heartsim_app::heart_view::HeartView;
use heartsim_core::{Auscultation, Millis};
use heartsim_echo::{EchoWindow, ImageSettings, ProbePose};
use heartsim_geometry::{AnatomyPack, Label};
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

/// What a test page runs on: the app's heart after some seconds of sinus rhythm, the page, and the
/// drawn time the harness feeds the heart each frame.
struct St {
    page: EchoPage,
    heart: HeartView,
    timing: TimingEngine,
    t_ms: f64,
    colour_blind: bool,
}

/// The ventricular activation times of the run, ms.
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

fn harness(
    lang: &str,
    width: f32,
    heart: HeartView,
    timing: TimingEngine,
    t_ms: f64,
) -> Harness<'static, St> {
    i18n::set_language(lang);
    let st = St {
        page: EchoPage::default(),
        heart,
        timing,
        t_ms,
        colour_blind: false,
    };
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(width, 900.0))
        .with_step_dt(0.1)
        .build_ui_state(
            |ui, st: &mut St| {
                let phase = st.timing.respiratory_phase(Millis(st.t_ms));
                st.heart.set_motion_time(st.t_ms, phase);
                st.page.ui(ui, Some(&mut st.heart), None, st.colour_blind);
            },
            st,
        )
}

/// A page that has built its anatomy and drawn the apical slice at `t_ms`.
fn ready(lang: &str, seconds: f64, at: impl Fn(&[f64]) -> f64) -> (Harness<'static, St>, Vec<f64>) {
    let (heart, timing, v) = beating(seconds);
    let t = at(&v);
    let mut h = harness(lang, 1400.0, heart, timing, t);
    h.run_steps(8);
    (h, v)
}

fn settle(h: &mut Harness<'static, St>) {
    h.run_steps(4);
}

fn anatomy_pose(h: &Harness<'static, St>, w: EchoWindow) -> ProbePose {
    h.state().heart.echo_anatomy().expect("built").window_pose(w)
}

/// Mid-diastole of the second to last beat (the ventricles filled), where the four chambers show.
fn diastole(v: &[f64]) -> f64 {
    v[v.len() - 2] + 5.0
}

#[test]
fn the_page_builds_its_anatomy_and_draws_the_apical_cross_section() {
    let (h, _) = ready("en", 6.0, diastole);
    let page = &h.state().page;
    assert!(page.pose().is_some(), "the probe has a pose");
    assert_eq!(page.window(), Some(EchoWindow::Apical));
    assert!(page.slice_count() >= 1, "a slice was made");
    let image = page.image();
    assert!(image.contact, "the apical start has contact");
    let present = image.labels_present();
    for l in [Label::LvBlood, Label::RvBlood, Label::LaBlood, Label::RaBlood] {
        assert!(
            present.contains(&l),
            "{l:?} in the apical four-chamber plane at diastole"
        );
    }
    let _ = h.get_by_label("Apical");
    let _ = h.get_by_label("Parasternal");
    let _ = h.get_by_label("Subcostal");
    let _ = h.get_by_label("Suprasternal");
    let _ = h.get_by_label("Reset");
}

#[test]
fn the_window_buttons_start_from_the_windows_poses_and_keep_depth_and_sector() {
    let (mut h, _) = ready("en", 5.0, diastole);
    let want = [
        ("Parasternal", EchoWindow::Parasternal),
        ("Subcostal", EchoWindow::Subcostal),
        ("Suprasternal", EchoWindow::Suprasternal),
        ("Apical", EchoWindow::Apical),
    ];
    // a non-default depth and sector are the user's imaging settings, not part of a window
    let mut p = h.state().page.pose().unwrap();
    p.depth_mm = 120.0;
    p.sector_deg = 60.0;
    h.state_mut().page.set_pose(p);
    for (label, w) in want {
        let target = anatomy_pose(&h, w);
        h.get_by_label(label).click();
        settle(&mut h);
        let pose = h.state().page.pose().unwrap();
        assert!(
            (pose.skin_mm - target.skin_mm).length() < 1e-3,
            "{label}: skin point"
        );
        assert!(
            (pose.rotation_deg - target.rotation_deg).abs() < 1e-3,
            "{label}: rotation"
        );
        assert!((pose.depth_mm - 120.0).abs() < 1e-3 && (pose.sector_deg - 60.0).abs() < 1e-3);
        assert_eq!(h.state().page.window(), Some(w));
        assert!(h.state().page.image().contact, "{label} has contact");
    }
    h.get_by_label("Reset").click();
    settle(&mut h);
    let reset = h.state().page.pose().unwrap();
    let apical = anatomy_pose(&h, EchoWindow::Apical);
    assert!(
        (reset.depth_mm - apical.depth_mm).abs() < 1e-3,
        "reset restores the default depth"
    );
}

#[test]
fn the_sliders_change_the_pose_and_clear_the_window() {
    let (mut h, _) = ready("en", 5.0, diastole);
    let before = h.state().page.pose().unwrap();
    let slices = h.state().page.slice_count();
    slider(&mut h, "Tilt", 12.0);
    slider(&mut h, "Rock", -7.0);
    slider(&mut h, "Rotation", 40.0);
    slider(&mut h, "Depth", 130.0);
    slider(&mut h, "Sector", 55.0);
    let pose = h.state().page.pose().unwrap();
    assert!((pose.tilt_deg - 12.0).abs() < 0.01, "{pose:?}");
    assert!((pose.rock_deg + 7.0).abs() < 0.01);
    assert!((pose.rotation_deg - 40.0).abs() < 0.01);
    assert!((pose.depth_mm - 130.0).abs() < 0.01);
    assert!((pose.sector_deg - 55.0).abs() < 0.01);
    assert_eq!(
        pose.skin_mm, before.skin_mm,
        "the sliders do not move the contact point"
    );
    assert_eq!(h.state().page.window(), None, "moved away from the window");
    assert!(h.state().page.slice_count() > slices, "the cross-section follows");
    assert!((h.state().page.image().depth_mm - 130.0).abs() < 0.01);
    assert!((h.state().page.image().sector_deg - 55.0).abs() < 0.01);
    // the clock position of the marker is shown
    let _ = h.get_by_label_contains("Index marker at");
}

#[test]
fn beyond_the_contact_limit_the_page_says_no_contact_and_the_image_is_empty() {
    let (mut h, _) = ready("en", 5.0, diastole);
    assert!(h.query_by_label_contains("No contact").is_none());
    let mut p: ProbePose = h.state().page.pose().unwrap();
    p.tilt_deg = 75.0;
    h.state_mut().page.set_pose(p);
    settle(&mut h);
    assert!(!h.state().page.image().contact);
    assert_eq!(h.state().page.image().labels_present(), vec![Label::Outside]);
    let _ = h.get_by_label_contains("No contact");
    // back inside the limit
    p.tilt_deg = 10.0;
    h.state_mut().page.set_pose(p);
    settle(&mut h);
    assert!(h.state().page.image().contact);
    assert!(h.query_by_label_contains("No contact").is_none());
}

#[test]
fn the_legend_names_every_label_present_in_both_languages() {
    for lang in ["en", "it"] {
        let (mut h, _) = ready(lang, 5.0, diastole);
        // the legend belongs to the colours: the image alone (the default) has none
        h.state_mut().page.set_display(DisplayMode::Anatomy);
        for w in EchoWindow::ALL {
            let a = h.state().heart.echo_anatomy().unwrap().window_pose(w);
            h.state_mut().page.set_pose(a);
            settle(&mut h);
            let present = h.state().page.image().labels_present();
            assert_ne!(present.len(), 0);
            for l in present {
                let name = echo_colours::name(l);
                assert!(
                    h.query_all_by_label(&name).next().is_some(),
                    "{lang}, {w:?}: the legend has no entry for {l:?} ({name})"
                );
            }
        }
    }
    i18n::set_language("en");
}

#[test]
fn the_hover_read_out_names_the_tissue_and_its_depth() {
    let (mut h, _) = ready("en", 6.0, diastole);
    settle(&mut h);
    let page = &h.state().page;
    let layout = page.layout().expect("the cross-section was drawn");
    let img = page.image();
    // the middle of the sector at 60 mm: a point whose label the image holds
    let p = layout.point_of(img.lines as f32 * 0.5, img.lines, 60.0);
    let r = page.read_out_at(p).expect("inside the sector");
    assert!((r.depth_mm - 60.0).abs() < 1.0, "{r:?}");
    let (line, sample, _) = layout.locate(p, img.lines, img.samples).unwrap();
    assert_eq!(r.label, img.label(line, sample));
    assert_eq!(r.hidden, img.is_hidden(line, sample));
    // outside the sector and the apex's far side: nothing
    assert!(
        page.read_out_at(glam::Vec2::new(1.0, layout.size_px.y - 1.0))
            .is_none()
    );
    // an end-to-end read-out label under the pointer
    let page_rect = h.get_by_label_contains("Point at the image").rect();
    assert!(
        page_rect.width() > 0.0,
        "the hint is shown while nothing is under the pointer"
    );
}

#[test]
fn nothing_is_computed_when_nothing_changed_and_the_wall_is_evaluated_once_per_time() {
    let (mut h, v) = ready("en", 5.0, diastole);
    let slices = h.state().page.slice_count();
    let serial = h.state().heart.motion().unwrap().echo_key();
    // many frames at the same time with the same pose: no slice, no evaluation
    h.run_steps(12);
    assert_eq!(h.state().page.slice_count(), slices, "no work without a change");
    assert_eq!(
        h.state().heart.motion().unwrap().echo_key(),
        serial,
        "no second evaluation"
    );
    // a pose change slices once and still evaluates nothing
    let mut p = h.state().page.pose().unwrap();
    p.tilt_deg += 4.0;
    h.state_mut().page.set_pose(p);
    h.run_steps(6);
    assert_eq!(h.state().page.slice_count(), slices + 1);
    assert_eq!(
        h.state().heart.motion().unwrap().echo_key(),
        serial,
        "the pose does not evaluate the wall"
    );
    // a new drawn time is one evaluation and one slice (the clock moved, one frame at a time)
    let t = v[v.len() - 2] + 200.0;
    h.state_mut().t_ms = t;
    h.run_steps(6);
    let after = h.state().heart.motion().unwrap().echo_key();
    assert_eq!(after.0, serial.0 + 1, "one evaluation for the new time");
    assert_eq!(h.state().page.slice_count(), slices + 2);
    // and the slice came from the evaluation the heart view holds: its LV blood is the model's
    // systolic cavity, smaller than at diastole
}

#[test]
fn the_lv_blood_area_is_smaller_at_end_systole_with_wall_motion_and_frozen_without() {
    let lv_area = |h: &Harness<'static, St>| h.state().page.image().count(Label::LvBlood);
    // wall motion on
    let (mut h, v) = ready("en", 6.0, diastole);
    let ed = lv_area(&h);
    h.state_mut().t_ms = v[v.len() - 2] + 330.0;
    h.run_steps(8);
    let es = lv_area(&h);
    assert!(
        ed > 0 && es > 0 && (es as f64) < ed as f64 * 0.92,
        "ED {ed} ES {es} samples"
    );
    // reduced motion: the wall is at rest, the cavity does not change with the beat (the leaflets
    // still move, so allow the few samples of a closing valve)
    let (heart, timing, v) = beating(6.0);
    let mut h = harness("en", 1400.0, heart, timing, diastole(&v));
    h.state_mut().heart.set_motion_options(true, true, 1.0);
    h.run_steps(8);
    let ed = lv_area(&h);
    h.state_mut().t_ms = v[v.len() - 2] + 330.0;
    h.run_steps(8);
    let es = lv_area(&h);
    let (a, b) = (ed as f64, es as f64);
    assert!(
        a > 0.0 && (a - b).abs() / a < 0.04,
        "reduced motion: ED {ed} ES {es}"
    );
    assert!(!h.state().heart.motion().unwrap().active());
    let _ = h.get_by_label_contains("Reduced motion is on");
}

#[test]
fn the_wall_motion_switch_of_the_page_is_the_hearts() {
    let (mut h, _) = ready("en", 5.0, diastole);
    assert_eq!(h.state().heart.motion_options().map(|o| o.0), Some(true));
    h.get_by_label("Wall motion").click();
    settle(&mut h);
    assert_eq!(h.state().heart.motion_options().map(|o| o.0), Some(false));
    let _ = h.get_by_label_contains("The wall is at rest");
}

#[test]
fn the_pose_is_not_in_the_saved_settings() {
    // the pose lives in the page; the settings that are stored have no field for it, and moving
    // the probe changes nothing in them
    let before = heartsim_app::settings::Settings::default().to_storage_string();
    let (mut h, _) = ready("en", 5.0, diastole);
    let mut p = h.state().page.pose().unwrap();
    p.tilt_deg = 33.0;
    p.rotation_deg = 77.0;
    h.state_mut().page.set_pose(p);
    settle(&mut h);
    let after = heartsim_app::settings::Settings::default().to_storage_string();
    assert_eq!(before, after);
    // ("echo" itself is there: the one echo preference, `echo_fast_mmode`, is not a pose)
    for word in ["tilt", "rock", "rotation", "probe", "pose", "window", "skin"] {
        assert!(
            !after.to_lowercase().contains(word),
            "settings mention {word}: {after}"
        );
    }
}

#[test]
fn the_phone_layout_keeps_every_control_inside_the_window_in_both_languages() {
    for lang in ["en", "it"] {
        let (heart, timing, v) = beating(5.0);
        let mut h = harness(lang, 400.0, heart, timing, diastole(&v));
        h.run_steps(8);
        let tilt = heartsim_ui::t!("echo-tilt");
        let a = h.get_by_role_and_label(Role::Slider, &tilt);
        assert!(
            a.rect().right() <= 400.5,
            "{lang}: the tilt slider reaches {}",
            a.rect().right()
        );
        for w in EchoWindow::ALL {
            let name = heartsim_app::echo_page::window_name(w);
            let b = h.get_by_label(&name);
            assert!(
                b.rect().right() <= 400.5,
                "{lang}: {w:?} reaches {}",
                b.rect().right()
            );
        }
    }
    i18n::set_language("en");
}

fn srgb_to_linear(c: f64) -> f64 {
    let c = c / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn lab(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb.map(srgb_to_linear);
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| {
        if t > 0.008_856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// Colour-vision deficiency as Machado, Oliveira and Fernandes (2009) model it, severity 1.
fn simulate(rgb: [u8; 3], m: [[f64; 3]; 3]) -> [f64; 3] {
    let l = rgb.map(|c| srgb_to_linear(f64::from(c)));
    let out = m.map(|row| (row[0] * l[0] + row[1] * l[1] + row[2] * l[2]).clamp(0.0, 1.0));
    out.map(|x| {
        255.0
            * if x <= 0.003_130_8 {
                12.92 * x
            } else {
                1.055 * x.powf(1.0 / 2.4) - 0.055
            }
    })
}

#[test]
fn every_pair_of_labels_is_told_apart_in_both_colour_tables() {
    const DEUTERANOPIA: [[f64; 3]; 3] = [
        [0.367_322, 0.860_646, -0.227_968],
        [0.280_085, 0.672_501, 0.047_413],
        [-0.011_820, 0.042_940, 0.968_881],
    ];
    const PROTANOPIA: [[f64; 3]; 3] = [
        [0.152_286, 1.052_583, -0.204_868],
        [0.114_503, 0.786_281, 0.099_216],
        [-0.003_882, -0.048_116, 1.051_998],
    ];
    let dist = |a: [f64; 3], b: [f64; 3]| {
        let (la, lb) = (lab(a), lab(b));
        ((la[0] - lb[0]).powi(2) + (la[1] - lb[1]).powi(2) + (la[2] - lb[2]).powi(2)).sqrt()
    };
    let labels: Vec<Label> = Label::ALL.into_iter().filter(|l| *l != Label::Outside).collect();
    for (cvd, floor_normal, floor_simulated) in [(false, 30.0, None), (true, 15.0, Some(15.0))] {
        let mut worst = f64::INFINITY;
        for (i, a) in labels.iter().enumerate() {
            for b in &labels[i + 1..] {
                let (ca, cb) = (echo_colours::rgb(*a, cvd), echo_colours::rgb(*b, cvd));
                let plain = dist(ca.map(f64::from), cb.map(f64::from));
                assert!(
                    plain >= floor_normal,
                    "cvd={cvd}: {a:?} and {b:?} are only {plain:.1} apart"
                );
                worst = worst.min(plain);
                if let Some(floor) = floor_simulated {
                    for m in [DEUTERANOPIA, PROTANOPIA] {
                        let d = dist(simulate(ca, m), simulate(cb, m));
                        assert!(
                            d >= floor,
                            "{a:?} and {b:?} look {d:.1} apart with a colour-vision deficiency"
                        );
                    }
                }
            }
        }
        assert!(worst.is_finite());
    }
    // every label has a name in both languages (a key missing from the catalogue would show as itself)
    for lang in ["en", "it"] {
        i18n::set_language(lang);
        for l in Label::ALL {
            let n = echo_colours::name(l);
            assert!(!n.starts_with("echo-label-"), "{lang}: {l:?} has no name");
        }
    }
    i18n::set_language("en");
    let table = echo_colours::palette(false);
    assert_eq!(table.len(), heartsim_render::PALETTE_SLOTS);
}

struct P {
    page: EchoPage,
    heart: HeartView,
}

#[test]
fn a_pack_without_label_volumes_says_the_page_is_unavailable() {
    i18n::set_language("en");
    let procedural = heartsim_geometry::AnatomyPack::procedural(heartsim_geometry::Resolution::COARSE);
    let heart = HeartView::from_pack(procedural, &TimingParams::default());
    assert!(!heart.echo_possible());
    let mut h = Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(900.0, 600.0))
        .build_ui_state(
            |ui, p: &mut P| p.page.ui(ui, Some(&mut p.heart), None, false),
            P {
                page: EchoPage::default(),
                heart,
            },
        );
    h.run_steps(3);
    let _ = h.get_by_label_contains("The Echo page needs the default anatomy pack");
    assert!(h.state().page.pose().is_none());
}

// ---- the grey image (brief B32, item 6) -------------------------------------------------------

fn slider(h: &mut Harness<'static, St>, label: &str, value: f64) {
    show_slider(h, label);
    h.get_by_role_and_label(Role::Slider, label)
        .set_value_accesskit(value);
    settle(h);
}

/// Show the section of the left column that holds the slider named `label` (one section at a
/// time is shown).
fn show_slider(h: &mut Harness<'static, St>, label: &str) {
    for section in EchoSection::ALL {
        if h.query_by_role_and_label(Role::Slider, label).is_some() {
            return;
        }
        h.state_mut().page.set_section(section);
        settle(h);
    }
    assert!(
        h.query_by_role_and_label(Role::Slider, label).is_some(),
        "no slider {label} in any section"
    );
}

/// The display-mode button named `name` (the right pane; the left column's section control has an
/// "Image" too, further left).
fn display_button<'a>(h: &'a Harness<'static, St>, name: &'a str) -> egui_kittest::Node<'a> {
    h.get_all_by_label(name)
        .max_by(|a, b| a.rect().left().total_cmp(&b.rect().left()))
        .expect("the display button")
}

/// Open the "More" of the cross-section's note (the rightmost one).
fn open_section_note(h: &mut Harness<'static, St>) {
    h.get_all_by_label("More")
        .max_by(|a, b| a.rect().left().total_cmp(&b.rect().left()))
        .expect("the note's More")
        .click();
    settle(h);
}

/// Open the time-gain compensation section (collapsed by default) of the Image section.
fn open_tgc(h: &mut Harness<'static, St>, lang: &str) {
    i18n::set_language(lang);
    h.state_mut().page.set_section(EchoSection::Image);
    settle(h);
    let name = heartsim_ui::t!("echo-tgc");
    h.get_by_label(&name).click();
    settle(h);
}

fn grey_mean(h: &Harness<'static, St>, label: Label) -> f64 {
    let page = &h.state().page;
    let (labels, grey) = (page.image(), page.grey());
    assert_eq!((labels.lines, labels.samples), (grey.lines, grey.samples));
    let (mut sum, mut n) = (0.0, 0.0);
    for l in 0..labels.lines {
        for s in 0..labels.samples {
            if labels.label(l, s) == label && !labels.is_hidden(l, s) {
                sum += f64::from(grey.pixel(l, s));
                n += 1.0;
            }
        }
    }
    assert!(n > 50.0, "{label:?} has only {n} visible samples");
    sum / n
}

#[test]
fn the_grey_image_is_the_default_and_comes_from_the_same_frame_as_the_slice() {
    let (h, _) = ready("en", 6.0, diastole);
    let page = &h.state().page;
    assert_eq!(
        page.display(),
        DisplayMode::Image,
        "the grey image is the default"
    );
    assert!(page.grey_count() >= 1 && page.grey().contact);
    assert!(
        page.grey().pixels.iter().any(|&p| p > 100),
        "the myocardium is bright"
    );
    assert_eq!(
        (page.grey().lines, page.grey().samples),
        (page.image().lines, page.image().samples)
    );
    // one source of truth: the grey image has the slice's depth and sector, the pose's
    let pose = page.pose().unwrap();
    assert!((page.grey().depth_mm - pose.depth_mm).abs() < 1e-3);
    assert!((page.grey().sector_deg - pose.sector_deg).abs() < 1e-3);
    assert!((page.settings().depth_mm - pose.depth_mm).abs() < 1e-3);
    // blood is dark, muscle is bright, in the same frame
    let blood = grey_mean(&h, Label::LvBlood);
    let muscle = grey_mean(&h, Label::LvMyocardium);
    assert!(
        blood < 0.5 * muscle,
        "LV blood {blood:.0} against myocardium {muscle:.0}"
    );
    // the simulated-image line, no legend in the image mode; the disclaimer behind "More"
    let _ = h.get_by_label_contains("simulated image from a simple line model, not a recording");
    assert!(h.query_by_label(&echo_colours::name(Label::LvBlood)).is_none());
    let _ = display_button(&h, "Image");
    let _ = h.get_by_label("Anatomy");
    let _ = h.get_by_label("Both");
    let mut h = h;
    assert!(h.query_by_label_contains("Educational use only").is_none());
    open_section_note(&mut h);
    let _ = h.get_by_label_contains("Educational use only. Not a medical device.");
}

#[test]
fn the_three_display_modes_show_the_legend_where_the_colours_are_and_compute_the_image_where_it_is_shown() {
    let (mut h, _) = ready("en", 6.0, diastole);
    let lv = echo_colours::name(Label::LvBlood);
    let greys = h.state().page.grey_count();
    let slices = h.state().page.slice_count();
    // anatomy: the legend, no new grey image, and a pose change slices without computing one
    h.get_by_label("Anatomy").click();
    settle(&mut h);
    assert_eq!(h.state().page.display(), DisplayMode::Anatomy);
    assert!(h.query_by_label(&lv).is_some(), "legend in the anatomy mode");
    let _ = h.get_by_label_contains("anatomical cross-section of the model, not an ultrasound image");
    assert_eq!(
        h.state().page.grey_count(),
        greys,
        "switching to anatomy computes nothing"
    );
    let mut p = h.state().page.pose().unwrap();
    p.tilt_deg += 5.0;
    h.state_mut().page.set_pose(p);
    settle(&mut h);
    assert_eq!(h.state().page.slice_count(), slices + 1);
    assert_eq!(
        h.state().page.grey_count(),
        greys,
        "no grey image is computed while it is not shown"
    );
    // both: the legend and the image, computed once for the slice made meanwhile
    h.get_by_label("Both").click();
    settle(&mut h);
    assert_eq!(h.state().page.display(), DisplayMode::Both);
    assert!(h.query_by_label(&lv).is_some(), "legend in the both mode");
    let _ = h.get_by_label_contains("simulated image from a simple line model, not a recording");
    assert_eq!(h.state().page.grey_count(), greys + 1);
    // image: no legend, nothing recomputed by the switch
    display_button(&h, "Image").click();
    settle(&mut h);
    assert_eq!(h.state().page.display(), DisplayMode::Image);
    assert!(h.query_by_label(&lv).is_none(), "no legend in the image mode");
    assert_eq!(h.state().page.grey_count(), greys + 1);
    assert_eq!(h.state().page.slice_count(), slices + 1);
    // the page can be told the mode as the tour does
    h.state_mut().page.set_display(DisplayMode::Both);
    settle(&mut h);
    assert!(h.query_by_label(&lv).is_some());
}

#[test]
fn every_image_control_changes_the_settings_and_costs_one_recompute() {
    type Get = fn(&ImageSettings) -> f32;
    let (mut h, _) = ready("en", 6.0, diastole);
    open_tgc(&mut h, "en");
    let base = h.state().page.settings().clone();
    let controls: [(&str, f64, Get); 12] = [
        ("Frequency", 3.5, |s| s.frequency_mhz),
        ("Gain", 6.0, |s| s.gain_db),
        ("Dynamic range", 45.0, |s| s.dynamic_range_db),
        ("Focus", 60.0, |s| s.focus_mm),
        ("TGC 1", -4.0, |s| s.tgc_db[0]),
        ("TGC 2", 3.0, |s| s.tgc_db[1]),
        ("TGC 3", 5.0, |s| s.tgc_db[2]),
        ("TGC 4", -6.0, |s| s.tgc_db[3]),
        ("TGC 5", 7.0, |s| s.tgc_db[4]),
        ("TGC 6", -8.0, |s| s.tgc_db[5]),
        ("TGC 7", 9.0, |s| s.tgc_db[6]),
        ("TGC 8", 10.0, |s| s.tgc_db[7]),
    ];
    for (label, value, get) in controls {
        let (greys, slices) = (h.state().page.grey_count(), h.state().page.slice_count());
        slider(&mut h, label, value);
        let now = get(h.state().page.settings());
        assert!((f64::from(now) - value).abs() < 0.06, "{label}: {now}");
        assert_eq!(h.state().page.grey_count(), greys + 1, "{label}: one recompute");
        assert_eq!(
            h.state().page.slice_count(),
            slices,
            "{label}: the label slice is not redone for an image setting"
        );
    }
    // the automatic compensation switch
    let greys = h.state().page.grey_count();
    assert!(base.tgc_auto);
    h.get_by_label("Automatic compensation").click();
    settle(&mut h);
    assert!(!h.state().page.settings().tgc_auto);
    assert_eq!(h.state().page.grey_count(), greys + 1);
    // depth and sector are the probe's: one source of truth, and the image follows with its slice
    for (label, value) in [("Depth", 120.0), ("Sector", 60.0)] {
        let (greys, slices) = (h.state().page.grey_count(), h.state().page.slice_count());
        slider(&mut h, label, value);
        assert_eq!(h.state().page.slice_count(), slices + 1, "{label}: one slice");
        assert_eq!(h.state().page.grey_count(), greys + 1, "{label}: one image");
    }
    let (page, grey) = (&h.state().page, h.state().page.grey());
    assert!((page.pose().unwrap().depth_mm - 120.0).abs() < 0.01);
    assert!((page.settings().depth_mm - 120.0).abs() < 0.01);
    assert!((grey.depth_mm - 120.0).abs() < 0.01 && (grey.sector_deg - 60.0).abs() < 0.01);
    assert!(page.settings().focus_mm <= 120.0);
    // the image really changed with the controls: brighter with more gain
    let mean = |h: &Harness<'static, St>| {
        let px = &h.state().page.grey().pixels;
        px.iter().map(|&p| f64::from(p)).sum::<f64>() / px.len() as f64
    };
    let before = mean(&h);
    slider(&mut h, "Gain", 20.0);
    assert!(mean(&h) > before + 1.0, "{before} -> {}", mean(&h));
}

#[test]
fn reset_returns_the_image_controls_and_the_depth_and_sector_to_their_defaults() {
    let (mut h, _) = ready("en", 6.0, diastole);
    open_tgc(&mut h, "en");
    slider(&mut h, "Gain", -12.0);
    slider(&mut h, "Frequency", 4.5);
    slider(&mut h, "TGC 5", 12.0);
    slider(&mut h, "Depth", 110.0);
    slider(&mut h, "Sector", 50.0);
    h.get_by_label("Both").click();
    settle(&mut h);
    let tilt = h.state().page.pose().unwrap().tilt_deg;
    let (greys, slices) = (h.state().page.grey_count(), h.state().page.slice_count());
    h.state_mut().page.set_section(EchoSection::Image);
    settle(&mut h);
    h.get_by_label("Reset image settings").click();
    settle(&mut h);
    let page = &h.state().page;
    let want = ImageSettings::default();
    let s = page.settings();
    assert_eq!(
        (
            s.frequency_mhz,
            s.gain_db,
            s.dynamic_range_db,
            s.focus_mm,
            s.tgc_auto,
            s.tgc_db
        ),
        (
            want.frequency_mhz,
            want.gain_db,
            want.dynamic_range_db,
            want.focus_mm,
            true,
            want.tgc_db
        )
    );
    let pose = page.pose().unwrap();
    assert!((pose.depth_mm - want.depth_mm).abs() < 1e-3 && (pose.sector_deg - want.sector_deg).abs() < 1e-3);
    assert!((pose.tilt_deg - tilt).abs() < 1e-6, "the probe stays where it is");
    assert_eq!(page.display(), DisplayMode::Both, "the display choice is kept");
    assert_eq!(page.slice_count(), slices + 1, "the depth changed: one slice");
    assert_eq!(page.grey_count(), greys + 1, "and one image");
    // the probe's own Reset button is another button
    h.state_mut().page.set_section(EchoSection::Probe);
    settle(&mut h);
    let _ = h.get_by_label("Reset");
}

#[test]
fn nothing_is_recomputed_when_nothing_changed() {
    let (mut h, v) = ready("en", 6.0, diastole);
    let (greys, slices) = (h.state().page.grey_count(), h.state().page.slice_count());
    h.run_steps(15);
    assert_eq!(h.state().page.grey_count(), greys);
    assert_eq!(h.state().page.slice_count(), slices);
    // a new drawn time: one evaluation, one slice, one image (the wall moved)
    h.state_mut().t_ms = v[v.len() - 2] + 250.0;
    h.run_steps(6);
    assert_eq!(h.state().page.slice_count(), slices + 1);
    assert_eq!(h.state().page.grey_count(), greys + 1);
    // the same setting set again is no change
    let s = h.state().page.settings().clone();
    h.state_mut().page.set_settings(&s);
    h.run_steps(6);
    assert_eq!(h.state().page.grey_count(), greys + 1);
    // the same pose set again is no change
    let p = h.state().page.pose().unwrap();
    h.state_mut().page.set_pose(p);
    h.run_steps(6);
    assert_eq!(h.state().page.slice_count(), slices + 1);
    assert_eq!(h.state().page.grey_count(), greys + 1);
    // out-of-range settings are clamped the way a slider would
    let wild = ImageSettings {
        gain_db: 1e9,
        frequency_mhz: f32::NAN,
        ..ImageSettings::default()
    };
    h.state_mut().page.set_settings(&wild);
    h.run_steps(4);
    let s = h.state().page.settings();
    assert!(s.gain_db <= ImageSettings::GAIN_RANGE_DB.1 && s.frequency_mhz.is_finite());
    assert_eq!(h.state().page.grey_count(), greys + 2);
}

#[test]
fn reduced_motion_holds_the_frame_until_the_probe_or_a_setting_changes() {
    let (heart, timing, v) = beating(6.0);
    let mut h = harness("en", 1400.0, heart, timing, diastole(&v));
    h.state_mut().heart.set_motion_options(true, true, 1.0);
    h.run_steps(8);
    let (greys, slices) = (h.state().page.grey_count(), h.state().page.slice_count());
    assert!(greys >= 1 && slices >= 1);
    let frozen = h.state().page.grey().pixels.clone();
    // the beat plays: the frame stays
    for dt in [100.0, 250.0, 330.0] {
        h.state_mut().t_ms = v[v.len() - 2] + dt;
        h.run_steps(6);
    }
    assert_eq!(
        h.state().page.slice_count(),
        slices,
        "no new slice under reduced motion"
    );
    assert_eq!(
        h.state().page.grey_count(),
        greys,
        "no new image under reduced motion"
    );
    assert_eq!(h.state().page.grey().pixels, frozen);
    let _ = h.get_by_label_contains("Reduced motion is on");
    // a setting or the probe still redraws it
    slider(&mut h, "Gain", 9.0);
    assert_eq!(h.state().page.grey_count(), greys + 1);
    assert_ne!(h.state().page.grey().pixels, frozen);
    slider(&mut h, "Tilt", 8.0);
    assert_eq!(h.state().page.slice_count(), slices + 1);
    assert_eq!(h.state().page.grey_count(), greys + 2);
}

#[test]
fn without_reduced_motion_the_beat_redraws_the_image() {
    let (mut h, v) = ready("en", 6.0, diastole);
    let before = h.state().page.grey().pixels.clone();
    let greys = h.state().page.grey_count();
    h.state_mut().t_ms = v[v.len() - 2] + 330.0;
    h.run_steps(8);
    assert_eq!(h.state().page.grey_count(), greys + 1);
    assert_ne!(
        h.state().page.grey().pixels,
        before,
        "the image moved with the wall"
    );
}

#[test]
fn the_image_settings_are_not_in_the_saved_settings() {
    let before = heartsim_app::settings::Settings::default().to_storage_string();
    let (mut h, _) = ready("en", 5.0, diastole);
    open_tgc(&mut h, "en");
    slider(&mut h, "Gain", 12.0);
    slider(&mut h, "TGC 4", 9.0);
    h.get_by_label("Both").click();
    settle(&mut h);
    let after = heartsim_app::settings::Settings::default().to_storage_string();
    assert_eq!(before, after);
    for word in [
        "gain",
        "tgc",
        "dynamic",
        "focus",
        "frequency",
        "grey",
        "display_mode",
        "echo_image",
    ] {
        assert!(
            !after.to_lowercase().contains(word),
            "settings mention {word}: {after}"
        );
    }
}

#[test]
fn the_grey_image_controls_and_notes_are_in_english_and_italian() {
    // "Image" twice: the left column's section and the display mode
    let en = ["Image", "Anatomy", "Both", "Probe", "Reset image settings"];
    let it = [
        "Immagine",
        "Anatomia",
        "Entrambe",
        "Sonda",
        "Ripristina impostazioni immagine",
    ];
    for (lang, labels, sliders, note) in [
        (
            "en",
            en,
            ["Frequency", "Gain", "Dynamic range", "Focus"],
            "simulated image from a simple line model, not a recording",
        ),
        (
            "it",
            it,
            ["Frequenza", "Guadagno", "Gamma dinamica", "Fuoco"],
            "modello a linee di scansione, non una registrazione",
        ),
    ] {
        let (mut h, _) = ready(lang, 5.0, diastole);
        h.state_mut().page.set_section(EchoSection::Image);
        settle(&mut h);
        for l in labels {
            assert!(h.query_all_by_label(l).next().is_some(), "{lang}: {l}");
        }
        for l in sliders {
            let _ = h.get_by_role_and_label(Role::Slider, l);
        }
        let _ = h.get_by_label_contains(note);
        open_tgc(&mut h, lang);
        let band = heartsim_ui::t!("echo-tgc-band", n = "8");
        let _ = h.get_by_role_and_label(Role::Slider, &band);
        let auto = heartsim_ui::t!("echo-tgc-auto");
        let _ = h.get_by_label(&auto);
        // no key shows as itself
        for key in [
            "echo-display-image",
            "echo-heading-image",
            "echo-frequency",
            "echo-gain",
            "echo-dynamic-range",
            "echo-focus",
            "echo-tgc",
            "echo-tgc-note",
            "echo-image-note",
            "echo-both-note",
            "echo-reset-image",
            "echo-focus-hint",
            "echo-unit-mhz",
        ] {
            let text = i18n::tr(key);
            assert_ne!(text, key, "{lang}: {key} is missing");
        }
        // the three modes draw without a GPU (the fallback) in each language
        for m in DisplayMode::ALL {
            h.state_mut().page.set_display(m);
            settle(&mut h);
            assert_eq!(h.state().page.display(), m);
        }
    }
    i18n::set_language("en");
}

#[test]
fn the_phone_layout_keeps_the_display_switch_and_the_image_controls_inside_the_window() {
    for lang in ["en", "it"] {
        let (heart, timing, v) = beating(5.0);
        let mut h = harness(lang, 400.0, heart, timing, diastole(&v));
        h.run_steps(8);
        for m in DisplayMode::ALL {
            let name = m.name();
            for b in h.get_all_by_label(&name) {
                assert!(
                    b.rect().right() <= 400.5,
                    "{lang}: the {m:?} button reaches {}",
                    b.rect().right()
                );
            }
        }
        for key in [
            "echo-frequency",
            "echo-gain",
            "echo-dynamic-range",
            "echo-focus",
            "echo-depth",
        ] {
            let name = i18n::tr(key);
            show_slider(&mut h, &name);
            let a = h.get_by_role_and_label(Role::Slider, &name);
            assert!(
                a.rect().right() <= 400.5,
                "{lang}: the {key} slider reaches {}",
                a.rect().right()
            );
        }
        h.state_mut().page.set_section(EchoSection::Image);
        settle(&mut h);
        let reset = heartsim_ui::t!("echo-reset-image");
        assert!(h.get_by_label(&reset).rect().right() <= 400.5, "{lang}: reset");
    }
    i18n::set_language("en");
}

// ---- the interface pass: sections, a still image, Reset, the hint, the wording ------------------

/// The page as the app shows it at 1280 × 800: the top bars and the strip take about 210 px, so the
/// page itself gets about 1280 × 590.
const APP_PAGE: (f32, f32) = (1280.0, 590.0);

fn sized(lang: &str, size: (f32, f32)) -> Harness<'static, St> {
    let (heart, timing, v) = beating(5.0);
    i18n::set_language(lang);
    let st = St {
        page: EchoPage::default(),
        heart,
        timing,
        t_ms: diastole(&v),
        colour_blind: false,
    };
    let mut h = Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(size.0, size.1))
        .with_step_dt(0.1)
        .build_ui_state(
            |ui, st: &mut St| {
                let phase = st.timing.respiratory_phase(Millis(st.t_ms));
                st.heart.set_motion_time(st.t_ms, phase);
                st.page.ui(ui, Some(&mut st.heart), None, st.colour_blind);
            },
            st,
        );
    h.run_steps(8);
    h
}

/// The harness has the app's theme (A5): the button padding and the selection colours.
#[test]
fn the_test_harness_draws_with_the_apps_theme() {
    let h = sized("en", APP_PAGE);
    let style = h.ctx.style_of(h.ctx.theme());
    assert_eq!(style.spacing.button_padding, heartsim_ui::theme::BUTTON_PADDING);
    let p = heartsim_ui::Palette::for_theme(h.ctx.theme());
    assert_eq!(style.visuals.selection.stroke.color, p.selected_text());
}

/// B1: one section at a time, chosen with the control (Probe first); the Probe and Image sections
/// fit the column at 1280 × 800; asking for a view or starting the self-test shows Views.
#[test]
fn the_left_column_shows_one_section_at_a_time_and_fits_at_1280_by_800() {
    for lang in ["en", "it"] {
        let mut h = sized(lang, APP_PAGE);
        assert_eq!(h.state().page.section(), EchoSection::Probe);
        let tilt = heartsim_ui::t!("echo-tilt");
        let gain = heartsim_ui::t!("echo-gain");
        assert!(h.query_by_role_and_label(Role::Slider, &tilt).is_some());
        assert!(h.query_by_role_and_label(Role::Slider, &gain).is_none(), "{lang}");
        // the last control of the Probe section is inside the page
        let motion_name = heartsim_ui::t!("echo-wall-motion");
        let motion = h.get_by_label(&motion_name);
        assert!(
            motion.rect().bottom() <= APP_PAGE.1,
            "{lang}: probe section overflows: {:?}; tilt at {:?}",
            motion.rect(),
            h.get_by_role_and_label(Role::Slider, &tilt).rect()
        );
        // the control: Image
        let image = EchoSection::Image.name();
        h.get_all_by_label(&image)
            .min_by(|a, b| a.rect().left().total_cmp(&b.rect().left()))
            .expect("the section control")
            .click();
        settle(&mut h);
        assert_eq!(h.state().page.section(), EchoSection::Image);
        assert!(h.query_by_role_and_label(Role::Slider, &tilt).is_none());
        let reset_name = heartsim_ui::t!("echo-reset-image");
        let reset = h.get_by_label(&reset_name);
        assert!(
            reset.rect().bottom() <= APP_PAGE.1,
            "{lang}: image section overflows"
        );
        // the control: Views
        h.get_by_label(&EchoSection::Views.name()).click();
        settle(&mut h);
        assert_eq!(h.state().page.section(), EchoSection::Views);
        let _ = h.get_by_label(&heartsim_ui::t!("echo-mode-explore"));
        // a tutorial's view request and the self-test choose Views
        h.state_mut().page.set_section(EchoSection::Probe);
        h.state_mut()
            .page
            .request_view(heartsim_echo::ViewId::ApicalFourChamber);
        settle(&mut h);
        assert_eq!(h.state().page.section(), EchoSection::Views);
        h.state_mut().page.set_section(EchoSection::Image);
        h.state_mut().page.start_selftest();
        assert_eq!(h.state().page.section(), EchoSection::Views);
    }
    i18n::set_language("en");
}

/// B1: the section control is reachable with the keyboard (Tab) and every segment has a name.
#[test]
fn the_section_control_is_reachable_with_the_keyboard() {
    let mut h = sized("en", APP_PAGE);
    let mut seen = Vec::new();
    for _ in 0..6 {
        h.key_press(egui::Key::Tab);
        h.run_steps(2);
        if let Some(n) = h.query_by(|n: &egui_kittest::kittest::AccessKitNode<'_>| n.is_focused()) {
            seen.push(n.accesskit_node().label().unwrap_or_default());
        }
    }
    for name in ["Probe", "Image", "Views"] {
        assert!(
            seen.iter().any(|s| s == name),
            "{name} not focused by Tab: {seen:?}"
        );
    }
}

/// B2: the cross-section keeps its rectangle in the three display modes, wide and phone-narrow, in
/// both languages (the legend is below it).
#[test]
fn the_image_keeps_its_rectangle_in_the_three_modes() {
    for lang in ["en", "it"] {
        for size in [APP_PAGE, (400.0, 900.0)] {
            let mut h = sized(lang, size);
            let mut rects = Vec::new();
            for m in DisplayMode::ALL {
                h.state_mut().page.set_display(m);
                settle(&mut h);
                rects.push(h.state().page.section_rect().expect("drawn"));
            }
            assert!(rects.iter().all(|r| *r == rects[0]), "{lang} {size:?}: {rects:?}");
            // the legend is under the image
            h.state_mut().page.set_display(DisplayMode::Both);
            settle(&mut h);
            let lv = echo_colours::name(Label::LvBlood);
            let chip = h.get_by_label(&lv);
            assert!(
                chip.rect().top() >= rects[0].bottom(),
                "{lang} {size:?}: legend above the image"
            );
        }
    }
    i18n::set_language("en");
}

/// B3: Reset is on its own row under the window buttons, at the right.
#[test]
fn reset_is_not_in_the_row_of_the_windows() {
    let h = sized("en", APP_PAGE);
    let reset = h.get_by_label("Reset").rect();
    let windows: Vec<egui::Rect> = EchoWindow::ALL
        .into_iter()
        .map(|w| h.get_by_label(&heartsim_app::echo_page::window_name(w)).rect())
        .collect();
    let lowest = windows.iter().map(egui::Rect::bottom).fold(f32::MIN, f32::max);
    assert!(reset.top() >= lowest, "Reset at {reset:?}, windows to {lowest}");
    let rightmost = windows.iter().map(egui::Rect::right).fold(f32::MIN, f32::max);
    assert!(reset.right() >= rightmost - 1.0, "Reset is right-aligned");
}

/// B4: the drag hint is shown until the probe is first moved (here with the keys), then not again;
/// its text stays in the Probe section's "More".
#[test]
fn the_drag_hint_goes_after_the_first_probe_interaction() {
    let mut h = sized("en", APP_PAGE);
    assert!(h.state().page.drag_hint_shown());
    h.key_press(egui::Key::ArrowRight);
    settle(&mut h);
    assert!(!h.state().page.drag_hint_shown());
    h.get_all_by_label("More")
        .min_by(|a, b| a.rect().left().total_cmp(&b.rect().left()))
        .expect("the probe's More")
        .click();
    settle(&mut h);
    let _ = h.get_by_label_contains("Drag the probe on the chest");
}

/// B5: the pointer prompt says "image" in the image mode and "cross-section" in the anatomy mode.
#[test]
fn the_pointer_prompt_names_what_is_shown() {
    for (lang, image, section) in [
        ("en", "Point at the image", "Point at the cross-section"),
        ("it", "Punta l'immagine", "Punta la sezione"),
    ] {
        let mut h = sized(lang, APP_PAGE);
        let _ = h.get_by_label_contains(image);
        assert!(h.query_by_label_contains(section).is_none());
        h.state_mut().page.set_display(DisplayMode::Anatomy);
        settle(&mut h);
        let _ = h.get_by_label_contains(section);
    }
    i18n::set_language("en");
}
