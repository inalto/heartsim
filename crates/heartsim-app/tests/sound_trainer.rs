//! Acceptance tests of brief B28 for the Sound Trainer, with a fake sink (no device): the listen
//! counter rises by one per completed loop and not on a stopped one, persists and restores; a
//! contrast-pair switch keeps the point of the cycle; half speed is labelled as altered; with no
//! sound output the trace cannot be hidden and the page says why. And small-speaker mode: the lane
//! keeps its samples, the sound does not, the tag says so.

use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use heartsim_app::heart_sound_player::{HeartSoundPlayer, PlayerInput};
use heartsim_app::heart_sounds::{HeartSounds, altered_sound_tag};
use heartsim_app::scenarios::ScenarioLibrary;
use heartsim_app::sound_out::FakeSink;
use heartsim_app::sound_trainer::{FINDINGS, PAIRS, Selection, SoundTrainer};
use heartsim_core::Millis;
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::{HeartSoundStatus, StripStyle, i18n};
use std::path::Path;

fn library() -> ScenarioLibrary {
    ScenarioLibrary::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios")).unwrap()
}

fn input() -> PlayerInput {
    PlayerInput {
        enabled: false,
        volume: 1.0,
        offset_ms: 0.0,
        paused: false,
        speed: 1.0,
        sim_now_ms: 0.0,
        dt_s: 1.0 / 60.0,
        muted: true,
    }
}

/// Run `frames` frames of 1/60 s: the player pushes, the fake device drains, the trainer counts.
fn run(
    player: &mut HeartSoundPlayer,
    sink: &FakeSink,
    trainer: &mut SoundTrainer,
    sounds: &mut HeartSounds,
    frames: usize,
) {
    for _ in 0..frames {
        player.frame(sounds, &input());
        sink.drain_ms(1000.0 / 60.0);
        trainer.update(player);
    }
}

#[test]
fn the_counter_counts_completed_loops_only_and_persists() {
    let lib = library();
    let sink = FakeSink::new();
    let mut player = HeartSoundPlayer::with_sink(Box::new(sink.clone()));
    let mut sounds = HeartSounds::new(10.0);
    let mut t = SoundTrainer::default();
    assert_eq!(t.selection, Selection::Finding(0));
    let len_s = t.entry(&lib).unwrap().clip.duration_s();
    let frames_per_loop = (len_s * 60.0).ceil() as usize;
    t.play(&lib, &mut player);
    // just short of one loop: nothing yet
    run(&mut player, &sink, &mut t, &mut sounds, frames_per_loop - 3);
    assert_eq!(t.count("s1_s2"), 0);
    run(&mut player, &sink, &mut t, &mut sounds, 6);
    assert_eq!(t.count("s1_s2"), 1, "one loop heard to its end");
    // half way through the second loop, stop: not counted
    run(&mut player, &sink, &mut t, &mut sounds, frames_per_loop / 2);
    t.stop(&mut player);
    run(&mut player, &sink, &mut t, &mut sounds, frames_per_loop);
    assert_eq!(t.count("s1_s2"), 1, "a stopped loop does not count");
    // two more full loops
    t.play(&lib, &mut player);
    run(&mut player, &sink, &mut t, &mut sounds, 2 * frames_per_loop + 3);
    assert_eq!(t.count("s1_s2"), 3);
    assert_eq!(t.total(), 3);
    // persists and restores
    let saved = t.counts_string();
    let mut back = SoundTrainer::default();
    back.restore_counts(&saved);
    assert_eq!(back.count("s1_s2"), 3);
    assert_eq!(back.total(), 3);
    back.restore_counts("garbage,x=,=4,murmur_as=2");
    assert_eq!(back.count("murmur_as"), 2);
    assert_eq!(back.total(), 5);
}

#[test]
fn every_finding_and_pair_has_a_clip_and_its_texts() {
    let lib = library();
    i18n::set_language("en");
    let mut player = HeartSoundPlayer::without_output();
    let mut t = SoundTrainer::default();
    for (i, f) in FINDINGS.iter().enumerate() {
        t.select(Selection::Finding(i), &mut player);
        let e = t.entry(&lib).unwrap_or_else(|e| panic!("{}: {e}", f.id));
        assert!(e.clip.duration_s() > 1.0, "{}", f.id);
        for key in [format!("st-{}", f.id), format!("st-{}-listen", f.id)] {
            assert_ne!(i18n::tr(&key), key, "missing text {key}");
        }
    }
    for (i, p) in PAIRS.iter().enumerate() {
        t.select(Selection::Pair(i), &mut player);
        for key in [format!("st-{}", p.id), format!("st-{}-listen", p.id)] {
            assert_ne!(i18n::tr(&key), key, "missing text {key}");
        }
        for (m, (id, key)) in p.members.iter().enumerate() {
            assert!(lib.index_of(id).is_some(), "{id}");
            assert_ne!(i18n::tr(key), *key, "missing text {key}");
            t.member = m;
            assert!(t.entry(&lib).is_ok());
        }
    }
}

#[test]
fn a_contrast_pair_switch_keeps_the_point_of_the_cycle() {
    let lib = library();
    let sink = FakeSink::new();
    let mut player = HeartSoundPlayer::with_sink(Box::new(sink.clone()));
    let mut sounds = HeartSounds::new(10.0);
    let mut t = SoundTrainer::default();
    t.select(Selection::Pair(0), &mut player);
    t.play(&lib, &mut player);
    run(&mut player, &sink, &mut t, &mut sounds, 50);
    let a = t.entry(&lib).unwrap();
    let at = player.clip_playhead().unwrap();
    let (cycle, f) = a.clip.cycle_phase(at).unwrap();
    t.switch_member(&lib, &mut player);
    assert_eq!(t.member, 1);
    let b = t.entry(&lib).unwrap();
    assert_ne!(a.clip.scenario_id, b.clip.scenario_id);
    // the new clip starts where the old one was in the cycle
    let start = player.clip_playhead().unwrap();
    let (c2, f2) = b.clip.cycle_phase(start).unwrap();
    assert_eq!(c2, cycle % b.clip.qrs.len());
    assert!((f2 - f).abs() < 0.01, "{f} → {f2}");
    // and the counter belongs to the pair
    assert_eq!(t.selection_id(), PAIRS[0].id);
}

struct Page {
    trainer: SoundTrainer,
    player: HeartSoundPlayer,
    library: ScenarioLibrary,
}

fn page(player: HeartSoundPlayer) -> Harness<'static, Page> {
    i18n::set_language("en");
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(1200.0, 800.0))
        .build_ui_state(
            |ui, p: &mut Page| {
                let style = StripStyle::default();
                p.trainer.ui(ui, &p.library, &mut p.player, &style);
            },
            Page {
                trainer: SoundTrainer::default(),
                player,
                library: library(),
            },
        )
}

#[test]
fn half_speed_is_labelled_as_altered_sound() {
    let mut h = page(HeartSoundPlayer::with_sink(Box::new(FakeSink::new())));
    h.run();
    let _ = h.get_by_label("Half speed (altered sound)");
    h.state_mut().trainer.half_speed = true;
    assert_eq!(
        h.state().trainer.altered_tag().as_deref(),
        Some("Altered sound: slowed")
    );
    h.run();
}

#[test]
fn without_sound_output_the_trace_cannot_be_hidden_and_the_page_says_why() {
    let mut h = page(HeartSoundPlayer::without_output());
    h.state_mut().trainer.show_trace = false;
    h.run();
    assert!(h.state().trainer.show_trace, "the trace is forced on");
    let _ = h.get_by_label(
        "No sound output here, so the trace stays on: learn the timing from the labelled phonocardiogram.",
    );
    // with a sink it can be hidden
    let mut h = page(HeartSoundPlayer::with_sink(Box::new(FakeSink::new())));
    h.state_mut().trainer.show_trace = false;
    h.run();
    assert!(!h.state().trainer.show_trace);
    let _ = h.get_by_label("The trace is hidden: listen first, then show it to check what you heard.");
}

#[test]
fn small_speaker_mode_alters_the_sound_and_not_the_lane() {
    let run = |small: bool| {
        let mut engine = TimingEngine::new(TimingParams::default());
        let mut sounds = HeartSounds::new(10.0);
        sounds.set_small_speaker(small);
        for k in 0..6000 {
            engine.step(Millis(1.0));
            sounds.on_events(&engine.drain_events());
            if k % 2 == 1 {
                sounds.push_for_ecg_sample(engine.now());
            }
        }
        let lane: Vec<f32> = sounds.ring().iter().collect();
        let heard: Vec<f32> = sounds.audio_ring().iter().collect();
        (lane, heard, sounds.audio_ring().pushed())
    };
    let (lane_off, heard_off, n_off) = run(false);
    let (lane_on, heard_on, n_on) = run(true);
    assert!(
        lane_off
            .iter()
            .zip(&lane_on)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "the lane's samples are unchanged"
    );
    assert_eq!(heard_off, lane_off, "off: the sound is the lane");
    assert_eq!(n_on, n_off, "the same indices");
    assert_ne!(heard_on, lane_on, "on: the sound is altered");
    i18n::set_language("en");
    assert_eq!(
        altered_sound_tag(HeartSoundStatus::Playing, true).as_deref(),
        Some("Altered sound: small-speaker mode")
    );
    assert_eq!(altered_sound_tag(HeartSoundStatus::Playing, false), None);
    assert!(altered_sound_tag(HeartSoundStatus::Paused, true).is_some());
    assert_eq!(altered_sound_tag(HeartSoundStatus::Off, true), None);
    assert!(altered_sound_tag(HeartSoundStatus::SlowMotion, false).is_some());
}

/// The primary actions on the page (by label, in tree order): the rule of use allows one per panel.
fn primary_names<S>(h: &Harness<'_, S>) -> Vec<String> {
    h.query_all(By::new())
        .filter(|n| n.accesskit_node().class_name() == Some(heartsim_ui::PRIMARY_CLASS))
        .map(|n| n.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// Play is the one primary action (Stop is plain), and the counts beside the items are explained:
/// a legend says what the bracketed number is and each item says it on hover.
#[test]
fn the_trainer_has_one_primary_action_and_explains_its_counts() {
    let mut h = page(HeartSoundPlayer::with_sink(Box::new(FakeSink::new())));
    h.run();
    assert_eq!(primary_names(&h), ["Play"]);
    let _ = h.get_by_label("The number in brackets is how many times you have listened to that item.");
    h.get_by_label("Play").click();
    h.run();
    assert!(primary_names(&h).is_empty(), "Stop is a plain button");
    let _ = h.get_by_label("Stop");
    for lang in ["en", "it"] {
        i18n::set_language(lang);
        for key in ["st-count-legend", "st-count-hint"] {
            assert_ne!(i18n::tr(key), key, "{lang}: {key}");
        }
    }
    i18n::set_language("en");
}
