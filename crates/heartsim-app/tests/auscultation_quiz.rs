//! Acceptance tests of brief B28 for the auscultation quiz: deterministic questions from a seed;
//! only auscultation cases asked or offered, four distinct choices with the answer; the trace and
//! the name hidden until both steps are answered when sound is available, the trace shown when it
//! is not; feature scoring follows `ExpectedFindings`; the score round-trips; the Rhythm Quiz pool is
//! unchanged.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::auscultation_quiz::{
    AuscultationQuiz, FeatureItem, Features, Step, expected_features, make_question, score_features,
};
use heartsim_app::heart_sound_player::HeartSoundPlayer;
use heartsim_app::rhythm_quiz::OPTIONS;
use heartsim_app::scenarios::ScenarioLibrary;
use heartsim_app::sound_out::FakeSink;
use heartsim_ui::{StripStyle, i18n};
use std::path::Path;

fn library() -> ScenarioLibrary {
    ScenarioLibrary::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios")).unwrap()
}

#[test]
fn questions_are_deterministic_auscultation_cases_with_four_distinct_choices() {
    let lib = library();
    let pool = lib.auscultation_pool();
    assert!(pool.len() >= 12, "{} cases", pool.len());
    let (mut a, mut b) = (5, 5);
    for _ in 0..30 {
        assert_eq!(
            make_question(&lib, &mut a, None),
            make_question(&lib, &mut b, None)
        );
    }
    let mut rng = 17;
    let mut prev = None;
    let mut asked = std::collections::BTreeSet::new();
    for _ in 0..2000 {
        let q = make_question(&lib, &mut rng, prev).unwrap();
        assert_eq!(q.options.len(), OPTIONS);
        assert!(q.options.contains(&q.answer));
        let mut o = q.options.clone();
        o.sort_unstable();
        o.dedup();
        assert_eq!(o.len(), OPTIONS, "duplicate choice");
        for &i in &q.options {
            assert!(
                lib.entries[i].scenario.is_auscultation_case(),
                "{}",
                lib.entries[i].scenario.id
            );
        }
        assert_ne!(Some(q.answer), prev);
        prev = Some(q.answer);
        asked.insert(q.answer);
    }
    assert_eq!(asked.len(), pool.len(), "every case gets asked");
}

#[test]
fn the_rhythm_quiz_pool_is_unchanged() {
    let lib = library();
    let ecg = lib.ecg_quiz_pool();
    let ausc = lib.auscultation_pool();
    // the two pools partition the library
    assert_eq!(ecg.len() + ausc.len(), lib.len());
    assert!(ecg.iter().all(|i| !ausc.contains(i)));
    assert_eq!(
        ecg,
        (0..lib.len())
            .filter(|&i| !lib.entries[i].scenario.is_auscultation_case())
            .collect::<Vec<_>>()
    );
    // the ECG-diagnostic scenarios with sounds stay in the rhythm quiz
    for id in ["pericarditis", "stemi_anterior"] {
        assert!(ecg.contains(&lib.index_of(id).unwrap()), "{id}");
    }
}

#[test]
fn every_case_has_scorable_features_and_a_differential_among_the_cases() {
    let lib = library();
    let pool = lib.auscultation_pool();
    for &i in &pool {
        let s = &lib.entries[i].scenario;
        let e = s.expected.as_ref().unwrap_or_else(|| panic!("{}", s.id));
        assert!(e.extra_sounds.is_some() && e.murmurs.is_some(), "{}", s.id);
        let x = expected_features(e);
        // ticking exactly what is expected scores full marks
        let (_, right, total) = score_features(&x, &x.features);
        assert_eq!(right, total, "{}", s.id);
        assert!(total >= 9);
        let in_pool = s
            .confused_with
            .iter()
            .filter_map(|id| lib.index_of(id))
            .filter(|j| pool.contains(j))
            .count();
        assert!(in_pool >= 2, "{}: {in_pool} auscultation differentials", s.id);
    }
}

#[test]
fn feature_scoring_follows_the_expected_findings() {
    let lib = library();
    let get = |id: &str| {
        expected_features(
            lib.entries[lib.index_of(id).unwrap()]
                .scenario
                .expected
                .as_ref()
                .unwrap(),
        )
    };
    let ms = get("mitral_stenosis_sinus");
    assert!(ms.features.get(FeatureItem::AfterS2), "opening snap");
    assert!(ms.features.get(FeatureItem::DiastolicMurmur));
    assert!(ms.features.get(FeatureItem::Mid) && ms.features.get(FeatureItem::Late));
    assert!(!ms.features.get(FeatureItem::SystolicMurmur));
    let as_severe = get("aortic_stenosis_severe");
    assert!(as_severe.features.get(FeatureItem::BeforeS1), "S4");
    assert!(as_severe.features.get(FeatureItem::SystolicMurmur) && as_severe.features.get(FeatureItem::Mid));
    let mr = get("mitral_regurgitation");
    assert!(mr.features.get(FeatureItem::Throughout));
    let asd = get("atrial_septal_defect");
    assert!(asd.features.get(FeatureItem::SplitS2), "fixed split");
    // a physiological split is not scored either way
    let normal_split = get("heart_failure_s3");
    let mut ticked = normal_split.features;
    ticked.set(FeatureItem::SplitS2, true);
    let (marks, right, total) = score_features(&normal_split, &ticked);
    assert_eq!(right, total);
    assert!(marks.iter().all(|(i, _)| *i != FeatureItem::SplitS2));
    // nothing ticked on MR: the systolic murmur and its timing are missed, the rest right
    let (marks, right, total) = score_features(&mr, &Features::default());
    let wrong: Vec<FeatureItem> = marks.iter().filter(|(_, ok)| !ok).map(|(i, _)| *i).collect();
    assert_eq!(wrong, [FeatureItem::SystolicMurmur, FeatureItem::Throughout]);
    assert_eq!(right + 2, total);
}

#[test]
fn the_two_steps_score_separately_and_the_score_round_trips() {
    let lib = library();
    let mut player = HeartSoundPlayer::without_output();
    let mut q = AuscultationQuiz::new(3);
    q.next_question(&lib, &mut player);
    assert_eq!(q.step, Step::Features);
    q.answer(0);
    assert_eq!(q.asked, 0, "no diagnosis before the features");
    let expected = q.expected(&lib).unwrap();
    q.given = expected.features;
    q.submit_features(&lib);
    assert_eq!(q.step, Step::Diagnosis);
    assert_eq!(q.features_right, q.features_scored);
    let answer = q.question.as_ref().unwrap().answer;
    q.answer(answer);
    q.answer(answer);
    assert_eq!((q.correct, q.asked, q.step), (1, 1, Step::Feedback));
    let saved = q.score_string();
    let mut back = AuscultationQuiz::default();
    back.restore_score(&saved);
    assert_eq!(back.score_string(), saved);
    for bad in ["", "1/2", "3/2;1/1", "a/b;c/d"] {
        back.restore_score(bad);
        assert_eq!(back.score_string(), saved, "{bad:?}");
    }
}

struct Page {
    quiz: AuscultationQuiz,
    player: HeartSoundPlayer,
    library: ScenarioLibrary,
}

fn page(player: HeartSoundPlayer) -> Harness<'static, Page> {
    i18n::set_language("en");
    let library = library();
    let mut quiz = AuscultationQuiz::new(11);
    let mut player = player;
    quiz.next_question(&library, &mut player);
    Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .build_ui_state(
            |ui, p: &mut Page| {
                let style = StripStyle::default();
                let _ = p.quiz.ui(ui, &p.library, &mut p.player, &style);
            },
            Page {
                quiz,
                player,
                library,
            },
        )
}

#[test]
fn with_sound_the_trace_and_the_name_stay_hidden_until_both_steps_are_answered() {
    let mut h = page(HeartSoundPlayer::with_sink(Box::new(FakeSink::new())));
    h.run();
    let answer = h.state().quiz.question.as_ref().unwrap().answer;
    let title = h.state().library.entries[answer].scenario.title.clone();
    assert!(!h.state().quiz.trace_visible(true));
    let _ = h.get_by_label("The trace is hidden until you have answered both steps.");
    assert!(h.query_by_label(&title).is_none(), "the name is hidden in step 1");
    h.get_by_label("Done: choose the diagnosis").click();
    h.run();
    assert_eq!(h.state().quiz.step, Step::Diagnosis);
    // the four choices are offered; the trace is still hidden
    let _ = h.get_by_label("The trace is hidden until you have answered both steps.");
    let other = *h
        .state()
        .quiz
        .question
        .as_ref()
        .unwrap()
        .options
        .iter()
        .find(|&&i| i != answer)
        .unwrap();
    let other_title = h.state().library.entries[other].scenario.title.clone();
    h.get_by_label(&other_title).click();
    h.run();
    assert_eq!(h.state().quiz.step, Step::Feedback);
    assert!(h.state().quiz.trace_visible(true));
    assert!(
        h.query_by_label("The trace is hidden until you have answered both steps.")
            .is_none()
    );
    assert!(
        h.query_all_by_label_contains(&title).count() >= 1,
        "the name is shown now"
    );
    let _ = h.get_by_label("Open in simulator");
}

#[test]
fn without_sound_the_trace_is_shown_from_the_start_and_the_name_is_not() {
    let mut h = page(HeartSoundPlayer::without_output());
    h.run();
    let answer = h.state().quiz.question.as_ref().unwrap().answer;
    let title = h.state().library.entries[answer].scenario.title.clone();
    assert!(h.state().quiz.trace_visible(false));
    assert!(
        h.query_by_label("The trace is hidden until you have answered both steps.")
            .is_none()
    );
    let _ = h.get_by_label_contains("No sound output here, so the trace is shown from the start");
    assert!(h.query_by_label(&title).is_none(), "the name stays hidden");
    assert!(!h.state().quiz.name_visible());
}
