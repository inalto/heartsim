//! B33 acceptance (UI and app level, no GPU): the standard views on the Echo page, drawn by
//! egui_kittest without a render state: the picker, the score bar, the hint, the checklist, *Show me*,
//! the ghost and the labels switches, the self-test with its summary and saved progress, en and it,
//! reduced motion, the phone layout.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use heartsim_app::echo_page::EchoPage;
use heartsim_app::echo_views::{self, Phase};
use heartsim_app::heart_view::HeartView;
use heartsim_core::{Auscultation, Millis};
use heartsim_echo::{HintKind, ProbePose, ViewId};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::{Engine, EventKind, TimingEngine, TimingParams};
use heartsim_ui::{i18n, t};
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

fn ready(lang: &str, width: f32) -> Harness<'static, St> {
    let (heart, timing, v) = beating(5.0);
    i18n::set_language(lang);
    let st = St {
        page: EchoPage::default(),
        heart,
        timing,
        t_ms: v[v.len() - 2] + 5.0,
    };
    let mut h = Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(width, 1400.0))
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
    // the left column shows the Probe section first
    h.get_by_label(&t!("echo-section-views")).click();
    h.run_steps(3);
    h
}

fn reference(h: &Harness<'static, St>, id: ViewId) -> ProbePose {
    let s = h.state();
    s.heart
        .echo_views()
        .expect("views")
        .reference(s.heart.echo_anatomy().expect("anatomy"), id)
        .pose
}

fn click(h: &mut Harness<'static, St>, label: &str) {
    h.get_by_label(label).click();
    h.run_steps(3);
}

#[test]
fn the_picker_lists_the_views_and_choosing_one_scores_and_hints() {
    let mut h = ready("en", 1600.0);
    for id in [
        ViewId::ParasternalLongAxis,
        ViewId::ApicalFourChamber,
        ViewId::SubcostalFourChamber,
    ] {
        let _ = h.get_by_label(&echo_views::view_name(id));
    }
    assert!(h.state().page.target().is_none());
    click(&mut h, "Apical four-chamber");
    assert_eq!(h.state().page.target(), Some(ViewId::ApicalFourChamber));
    let score = h.state().page.view_score().expect("a score").total;
    assert!((0.0..=1.0).contains(&score));
    // the score bar is there, with its percentage, the hint and the checklist
    let _ = h.get_by_role(Role::ProgressIndicator);
    let _ = h.get_by_label("What to change");
    let _ = h.get_by_label("This view must show");
    assert!(h.state().page.view_hint().is_some());
    assert_ne!(h.state().page.view_checklist().len(), 0);
    // the page cites the guideline page
    let _ = h.get_by_label("Mitchell et al. 2019, page 26");
}

#[test]
fn show_me_plays_the_movement_and_the_view_is_found_after_a_second() {
    let mut h = ready("en", 1600.0);
    click(&mut h, "Apical four-chamber");
    assert!(!h.state().page.view_found(ViewId::ApicalFourChamber));
    click(&mut h, "Show me");
    assert!(h.state().page.show_me_active(), "the movement is playing");
    h.run_steps(25);
    assert!(!h.state().page.show_me_active(), "it ends");
    let want = reference(&h, ViewId::ApicalFourChamber);
    let got = h.state().page.pose().expect("pose");
    assert!(
        got.skin_mm.distance(want.skin_mm) < 0.5,
        "{:?} vs {:?}",
        got.skin_mm,
        want.skin_mm
    );
    let s = h.state().page.view_score().expect("score").total;
    assert!(s > 0.9, "{s}");
    h.run_steps(14);
    assert!(h.state().page.view_found(ViewId::ApicalFourChamber));
    let _ = h.get_by_label_contains("held for a second");
}

#[test]
fn show_me_is_instant_with_reduced_motion() {
    let mut h = ready("en", 1600.0);
    h.state_mut().heart.set_motion_options(true, true, 1.0);
    h.run_steps(4);
    click(&mut h, "Parasternal long axis");
    click(&mut h, "Show me");
    assert!(!h.state().page.show_me_active());
    let want = reference(&h, ViewId::ParasternalLongAxis);
    assert_eq!(h.state().page.pose().map(|p| p.clamped()), Some(want));
}

#[test]
fn the_ghost_and_the_labels_have_switches() {
    let mut h = ready("en", 1600.0);
    click(&mut h, "Apical four-chamber");
    assert!(h.state().page.ghost_on() && h.state().page.labels_on());
    click(&mut h, "Show the target plane in the 3-D heart");
    assert!(!h.state().page.ghost_on());
    click(&mut h, "Name structures on the image");
    assert!(!h.state().page.labels_on());
    click(&mut h, "Show the target plane in the 3-D heart");
    assert!(h.state().page.ghost_on());
}

#[test]
fn a_view_the_model_cannot_show_says_so() {
    let mut h = ready("en", 1600.0);
    click(
        &mut h,
        "Subcostal inferior vena cava (not reachable in this model)",
    );
    let _ = h.get_by_label_contains("Not reachable in this model");
    assert!(
        h.query_by_label("Show me").is_none(),
        "no controls for a view that cannot be shown"
    );
}

#[test]
fn the_self_test_asks_without_hints_shows_a_summary_and_saves_progress() {
    let mut h = ready("en", 1600.0);
    click(&mut h, "Self-test");
    let _ = h.get_by_label_contains("no pass mark");
    click(&mut h, "Start the self-test");
    // the views are prepared one per frame, with a progress bar
    let (done, total) = h.state().page.selftest_preparing().expect("preparing");
    assert!(done < total && total == ViewId::ALL.len(), "{done} of {total}");
    let _ = h.get_by_label_contains(&format!("Preparing views {} of 15", done + 1));
    let mut frames = 0;
    while h.state().page.selftest_preparing().is_some() {
        h.run_steps(1);
        frames += 1;
        assert!(frames < 60, "the preparation ends");
    }
    assert!(
        frames + done >= ViewId::ALL.len(),
        "one view per frame: {frames} frames"
    );
    h.run_steps(4);
    assert_eq!(h.state().page.quiz().phase(), Phase::Running);
    let first = h.state().page.quiz().current().expect("a view");
    let _ = h.get_by_label(&t!("echo-test-find", view = echo_views::view_name(first)));
    // no hints, no checklist, no score bar, no ghost
    assert!(h.query_by_label("What to change").is_none());
    assert!(h.query_by_label("This view must show").is_none());
    assert!(h.query_by_role(Role::ProgressIndicator).is_none());
    // find it at its reference pose
    let r = reference(&h, first);
    h.state_mut().page.set_pose(r);
    h.run_steps(16);
    assert_eq!(h.state().page.quiz().phase(), Phase::Between);
    let _ = h.get_by_label_contains("Found in");
    // give up on the second, find nothing in the rest: skip through to the summary
    click(&mut h, "Next view");
    assert_eq!(h.state().page.quiz().phase(), Phase::Running);
    loop {
        match h.state().page.quiz().phase() {
            Phase::Running => click(&mut h, "Give up on this view"),
            Phase::Between => click(&mut h, "Next view"),
            _ => break,
        }
    }
    assert_eq!(h.state().page.quiz().phase(), Phase::Summary);
    let _ = h.get_by_label("Summary");
    let _ = h.get_by_label("Best");
    // the table: a dash for a view not found, the numbers right-aligned in one column, and the
    // record of all sessions is not repeated under it
    let dashes: Vec<_> = h.get_all_by_label("not found").collect();
    assert!(!dashes.is_empty(), "skipped views show a dash named 'not found'");
    let rights: Vec<f32> = h
        .get_all_by_label_contains(" %")
        .filter(|n| {
            let node = n.accesskit_node();
            let l = node.label().or_else(|| node.value()).unwrap_or_default();
            l.strip_suffix(" %")
                .is_some_and(|d| d.chars().all(|c| c.is_ascii_digit()))
        })
        .map(|n| n.rect().right())
        .collect();
    assert!(rights.len() >= 8);
    assert!(
        rights.iter().all(|r| (r - rights[0]).abs() < 0.5),
        "scores right-aligned: {rights:?}"
    );
    assert!(h.query_by_label("Your record").is_none(), "no duplicate list");
    let _ = h.get_by_label_contains("Mean of the best scores");
    let n = h.state().page.quiz().results().len();
    assert!(n >= 8, "{n} views asked");
    // the progress round-trips through the stored string
    let saved = h.state().page.quiz_string();
    let mut fresh = EchoPage::default();
    fresh.restore_quiz(&saved);
    assert_eq!(fresh.quiz().progress, h.state().page.quiz().progress);
    assert!(fresh.quiz().progress.best.contains_key(first.id()));
    assert!(fresh.quiz().progress.best[first.id()].time_s.is_some());
}

#[test]
fn the_views_are_in_english_and_italian() {
    for lang in ["en", "it"] {
        let mut h = ready(lang, 1600.0);
        for id in ViewId::ALL {
            let name = echo_views::view_name(id);
            assert!(!name.starts_with("echo-view"), "{lang}: {id:?} has no name");
        }
        let a4c = echo_views::view_name(ViewId::ApicalFourChamber);
        click(&mut h, &a4c);
        let _ = h.get_by_label(&t!("echo-checklist"));
        let _ = h.get_by_label(&t!("echo-show-me"));
        let hint = h.state().page.view_hint().expect("hint");
        if hint.kind != HintKind::Found {
            let text = echo_views::hint_text(&hint);
            assert!(!text.contains("echo-hint"), "{lang}: {text}");
        }
        for item in &h
            .state()
            .heart
            .echo_views()
            .expect("views")
            .target(ViewId::ApicalFourChamber)
            .checklist
        {
            assert!(
                !echo_views::checklist_text(item.key).starts_with("echo-chk"),
                "{lang} {}",
                item.key
            );
        }
    }
    i18n::set_language("en");
}

#[test]
fn the_phone_layout_keeps_the_picker_inside_the_window() {
    for lang in ["en", "it"] {
        let h = ready(lang, 400.0);
        let name = echo_views::view_name(ViewId::ParasternalLongAxis);
        let b = h.get_by_label(&name);
        assert!(
            b.rect().right() <= 400.5,
            "{lang}: {} reaches {}",
            name,
            b.rect().right()
        );
    }
    i18n::set_language("en");
}

/// B6: the checklist is read-only status marks (a tick or a ring, "Met"/"Not met" for a screen
/// reader), not checkboxes.
#[test]
fn the_checklist_is_status_marks_not_checkboxes() {
    let mut h = ready("en", 1600.0);
    click(&mut h, "Apical four-chamber");
    let n = h.state().page.view_checklist().len();
    assert_ne!(n, 0);
    let marks = h.get_all_by_label_contains("Met: ").count();
    assert_eq!(marks, n, "one status mark per checklist line");
    let lv = h.state().page.view_checklist()[0];
    let want = format!("{}: Left ventricle", if lv { "Met" } else { "Not met" });
    assert!(h.query_all_by_label(&want).next().is_some(), "{want}");
    assert!(h.query_all_by_role(Role::CheckBox).all(|c| {
        !c.accesskit_node()
            .label()
            .unwrap_or_default()
            .contains("ventricle")
    }));
}

/// B8: the self-test's views prepared one per frame give exactly what the blocking search gives
/// (each on its own, fresh view targets: nothing cached is shared).
#[test]
fn preparing_the_views_incrementally_equals_the_blocking_search() {
    use heartsim_echo::{DesignedParams, EchoAnatomy, MotionModel, MotionParams, ViewTargets};
    let pack = pack();
    let model = MotionModel::new(&pack, MotionParams::default()).expect("model");
    let anatomy = EchoAnatomy::new(&pack, &model, DesignedParams::default()).expect("anatomy");
    let blocking_targets = ViewTargets::new(&pack, &model, &anatomy).expect("views");
    let blocking = echo_views::shipped_views(&anatomy, &blocking_targets);
    let stepped_targets = ViewTargets::new(&pack, &model, &anatomy).expect("views");
    let mut prep = echo_views::ViewPreparation::default();
    let mut steps = 0;
    let stepped = loop {
        if let Some(v) = prep.step(&anatomy, &stepped_targets) {
            break v;
        }
        steps += 1;
        assert_eq!(prep.progress(), (steps, ViewId::ALL.len()));
    };
    assert_eq!(steps, ViewId::ALL.len(), "one view per step");
    assert_eq!(stepped, blocking);
    for id in ViewId::ALL {
        let (a, b) = (
            blocking_targets.reference(&anatomy, id),
            stepped_targets.reference(&anatomy, id),
        );
        assert_eq!(a.pose, b.pose, "{id:?}");
        assert_eq!(a.reachable, b.reachable, "{id:?}");
        assert_eq!(a.score.total.to_bits(), b.score.total.to_bits(), "{id:?}");
    }
}

/// B9 (b), (c): the arch view's checklist line names what it tests (the blood of the thoracic
/// vessels), and the arch view says why it is not reachable: the arch is found, the best pose scores
/// too low (not "no arch").
#[test]
fn the_arch_view_says_the_real_reason_and_its_checklist_names_its_test() {
    let label = heartsim_app::echo_colours::name(heartsim_geometry::Label::ThoracicBlood);
    assert!(
        echo_views::checklist_text("blood").starts_with(&label),
        "{}",
        echo_views::checklist_text("blood")
    );
    let mut h = ready("en", 1600.0);
    let arch = echo_views::view_name(ViewId::SuprasternalArch);
    click(&mut h, &arch);
    h.run_steps(2);
    let _ = h.get_by_label_contains("Not reachable in this model");
    let _ = h.get_by_label_contains("below the 80 % that counts as found");
    assert!(h.query_by_label_contains("no aortic arch").is_none());
}
