//! The pages made of a figure and side panels at phone width (400 × 800, no GPU): each is one pane
//! at a time or stacked, the figure keeps a useful size, and no control extends past the right
//! edge. (The pages that need the whole app and a GPU, such as the Simulator, are covered by
//! `simulator_layout.rs`; the 12-lead, vector, electrode and EP pages use the same
//! `narrow::panes` and are checked in the screenshot tour.)

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use heartsim_app::auscultation_quiz::AuscultationQuiz;
use heartsim_app::heart_sound_player::HeartSoundPlayer;
use heartsim_app::interval_drill::IntervalDrill;
use heartsim_app::narrow::{self, Narrow, Pane, PaneSpec, Side};
use heartsim_app::rhythm_quiz::RhythmQuiz;
use heartsim_app::scenarios::ScenarioLibrary;
use heartsim_app::sound_out::FakeSink;
use heartsim_app::sound_trainer::SoundTrainer;
use heartsim_ui::{StripStyle, i18n};
use std::path::Path;

const W: f32 = 400.0;
const H: f32 = 800.0;

fn library() -> ScenarioLibrary {
    ScenarioLibrary::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios")).unwrap()
}

fn builder<S>() -> egui_kittest::HarnessBuilder<S> {
    i18n::set_language("en");
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(W, H))
}

/// No control that can be pressed or set extends past the right edge of the window.
fn assert_controls_inside<S>(h: &Harness<'_, S>, what: &str) {
    for node in h.query_all(By::new()) {
        let role = node.accesskit_node().role();
        if !matches!(
            role,
            Role::Button
                | Role::CheckBox
                | Role::RadioButton
                | Role::ComboBox
                | Role::Slider
                | Role::SpinButton
                | Role::TextInput
                | Role::Link
        ) {
            continue;
        }
        if let Some(b) = node.accesskit_node().bounding_box()
            && b.x1 > b.x0
            && b.y0 < f64::from(H)
        {
            assert!(
                b.x1 <= f64::from(W) + 0.5 && b.x0 >= -0.5,
                "{what}: {:?} ({role:?}) at {b:?}",
                node.accesskit_node().label()
            );
        }
    }
}

fn assert_figure<S>(h: &Harness<'_, S>, key: &str, min_w: f32, min_h: f32) {
    let r = narrow::figure_rect(&h.ctx, key).unwrap_or_else(|| panic!("{key}: no figure drawn"));
    assert!(
        r.width() >= min_w
            && r.height() >= min_h
            && r.left() >= -0.5
            && r.right() <= W + 0.5
            && r.bottom() <= H + 0.5,
        "{key}: figure {r:?}"
    );
}

struct Drill {
    drill: IntervalDrill,
    library: ScenarioLibrary,
}

#[test]
fn the_interval_drill_stacks_a_useful_strip_over_its_controls() {
    let mut h = builder().build_ui_state(
        |ui, m: &mut Drill| m.drill.ui(ui, &m.library, &StripStyle::default()),
        Drill {
            drill: IntervalDrill::new(11),
            library: library(),
        },
    );
    h.run();
    h.get_by_label("New question").click();
    h.run();
    assert_figure(&h, "interval_drill", 360.0, 200.0);
    let _ = h.get_by_label("Place calipers");
    assert_controls_inside(&h, "interval drill");
    // the controls are below the strip, not beside it
    let strip = narrow::figure_rect(&h.ctx, "interval_drill").unwrap();
    assert!(h.get_by_label("Place calipers").rect().top() >= strip.bottom() - 0.5);
}

struct Rhythm {
    quiz: RhythmQuiz,
    library: ScenarioLibrary,
}

#[test]
fn the_rhythm_quiz_stacks_a_useful_strip_over_its_question() {
    let mut h = builder().build_ui_state(
        |ui, m: &mut Rhythm| {
            m.quiz.ui(ui, &m.library, &StripStyle::default());
        },
        Rhythm {
            quiz: RhythmQuiz::new(5),
            library: library(),
        },
    );
    h.run();
    h.get_by_label("New question").click();
    h.run();
    assert_figure(&h, "rhythm_quiz", 360.0, 200.0);
    assert_controls_inside(&h, "rhythm quiz");
}

struct Ausc {
    quiz: AuscultationQuiz,
    player: HeartSoundPlayer,
    library: ScenarioLibrary,
}

#[test]
fn the_auscultation_quiz_shows_the_question_or_the_listening_page_at_full_width() {
    let library = library();
    let mut quiz = AuscultationQuiz::new(11);
    let mut player = HeartSoundPlayer::with_sink(Box::new(FakeSink::new()));
    quiz.next_question(&library, &mut player);
    let mut h = builder().build_ui_state(
        |ui, m: &mut Ausc| {
            let _ = m.quiz.ui(ui, &m.library, &mut m.player, &StripStyle::default());
        },
        Ausc {
            quiz,
            player,
            library,
        },
    );
    h.run();
    assert_controls_inside(&h, "auscultation question");
    let _ = h.get_by_label("Question");
    h.get_by_label("Listen").click();
    h.run();
    assert_figure(&h, "auscultation_quiz", 360.0, 200.0);
    assert_controls_inside(&h, "auscultation listen");
}

struct Trainer {
    page: SoundTrainer,
    player: HeartSoundPlayer,
    library: ScenarioLibrary,
}

#[test]
fn the_sound_trainer_has_a_list_and_a_listening_page_and_a_pick_opens_the_page() {
    let mut h = builder().build_ui_state(
        |ui, m: &mut Trainer| {
            let _ = m.page.ui(ui, &m.library, &mut m.player, &StripStyle::default());
        },
        Trainer {
            page: SoundTrainer::default(),
            player: HeartSoundPlayer::with_sink(Box::new(FakeSink::new())),
            library: library(),
        },
    );
    h.run();
    assert_figure(&h, "sound_trainer", 360.0, 200.0);
    assert_controls_inside(&h, "sound trainer page");
    h.get_by_label("Findings").click();
    h.run();
    assert_controls_inside(&h, "sound trainer list");
    h.get_by_label_contains("Aortic stenosis").click();
    h.run();
    h.run();
    assert_figure(&h, "sound_trainer", 360.0, 200.0);
    let _ = h.get_by_label("Listen");
}

/// The helper itself: wide shows the panels beside the figure, narrow either stacks or segments.
#[test]
fn the_pane_helper_stacks_or_segments_on_a_narrow_window_and_sits_beside_when_wide() {
    for (stacked, id) in [(true, "t_stacked"), (false, "t_segmented")] {
        for width in [W, 1280.0] {
            i18n::set_language("en");
            let mut h = Harness::builder()
                .with_context_setup(heartsim_ui::theme::apply_to_context)
                .with_size(egui::vec2(width, H))
                .build_ui(move |ui| {
                    let spec = PaneSpec {
                        id,
                        left: Some(Side {
                            id: "t_left",
                            default_px: 300.0,
                            cap_px: 600.0,
                            name: "Left pane",
                        }),
                        right: None,
                        centre: "Centre pane",
                        narrow: if stacked {
                            Narrow::Stacked { centre_height: 200.0 }
                        } else {
                            Narrow::Segmented { start: Pane::Centre }
                        },
                    };
                    narrow::panes(ui, &spec, |ui, pane| match pane {
                        Pane::Left => {
                            ui.label("left body");
                        }
                        Pane::Centre => {
                            let r = ui.available_rect_before_wrap();
                            ui.allocate_rect(r, egui::Sense::hover());
                        }
                        Pane::Right => {}
                    });
                });
            h.run();
            let centre = narrow::figure_rect(&h.ctx, id).expect("figure");
            if width > 700.0 {
                let left = h.get_by_label("left body").rect();
                assert!(left.right() <= centre.left() + 1.0, "beside: {left:?} {centre:?}");
            } else if stacked {
                assert!(centre.height() >= 190.0 && centre.width() >= 360.0, "{centre:?}");
                assert!(h.get_by_label("left body").rect().top() >= centre.bottom() - 0.5);
            } else {
                assert!(h.query_by_label("left body").is_none(), "one pane at a time");
                h.get_by_label("Left pane").click();
                h.run();
                let _ = h.get_by_label("left body");
            }
        }
    }
}
