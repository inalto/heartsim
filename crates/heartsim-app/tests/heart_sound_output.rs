//! Acceptance tests of brief B26 that run natively: the heart-sound output chosen at run time (no
//! factory: no switch, no Settings card; a registered factory: both; a failing one: the "not
//! available" notice; never called while the setting is off), the display delay from the sink's
//! own latency, and a sink that waits for the learner's click. Headless, no GPU, no sound card.

use std::cell::Cell;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::heart_sound_player::{HeartSoundPlayer, HeartSoundStatus, PlayerInput};
use heartsim_app::heart_sounds::HeartSounds;
use heartsim_app::settings::{HeartSoundInfo, Settings, settings_window_full};
use heartsim_app::sound_out::{FakeSink, PcmSink, QUEUE_TARGET_MS, SinkReadiness};
use heartsim_app::web_schedule::{LOOKAHEAD_MS, WebScheduler};
use heartsim_core::Millis;
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::{EcgView, i18n};

struct Sim {
    engine: TimingEngine,
    sounds: HeartSounds,
}

impl Sim {
    fn new() -> Self {
        let params = TimingParams::default();
        let mut sounds = HeartSounds::new(60.0);
        sounds.reset(
            7,
            params.sinus.respiration_rate_bpm,
            &heartsim_core::Auscultation::default(),
        );
        Self {
            engine: TimingEngine::new(params),
            sounds,
        }
    }

    fn step(&mut self, ms: u32) {
        for k in 0..ms {
            self.engine.step(Millis(1.0));
            let ev = self.engine.drain_events();
            self.sounds.on_events(&ev);
            if k % 2 == 1 {
                self.sounds.push_for_ecg_sample(self.engine.now());
            }
        }
    }

    fn input(&self, enabled: bool) -> PlayerInput {
        PlayerInput {
            enabled,
            volume: 1.0,
            offset_ms: 0.0,
            paused: false,
            speed: 1.0,
            sim_now_ms: self.engine.now().0,
            dt_s: 0.016,
            muted: false,
        }
    }
}

/// What the app shows for a player: the Display popup's switch and the Settings card.
fn controls(player: &HeartSoundPlayer, settings: &Settings) -> (Option<bool>, Option<HeartSoundInfo>) {
    (
        player.has_output().then_some(settings.heart_sound_audio),
        HeartSoundInfo::from_player(player),
    )
}

struct Model {
    settings: Settings,
    view: EcgView,
    info: Option<HeartSoundInfo>,
    open: bool,
}

fn harness(play: Option<bool>, info: Option<HeartSoundInfo>, settings: Settings) -> Harness<'static, Model> {
    let mut view = EcgView::new();
    view.play_heart_sounds = play;
    if let Some(i) = info {
        view.heart_sound_status = i.status;
    }
    Harness::builder()
        .with_size(egui::vec2(800.0, 1000.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                m.view.display_controls(ui);
                let ctx = ui.ctx().clone();
                let _ = settings_window_full(&ctx, &mut m.open, &mut m.settings, None, m.info.as_ref());
            },
            Model {
                settings,
                view,
                info,
                open: true,
            },
        )
}

#[test]
fn without_a_factory_the_switch_and_the_card_are_absent() {
    i18n::set_language("en");
    let player = HeartSoundPlayer::without_output();
    assert!(!player.has_output());
    let (play, info) = controls(&player, &Settings::default());
    assert_eq!(play, None);
    assert_eq!(info, None);
    let mut h = harness(play, info, Settings::default());
    h.run();
    assert!(h.query_by_label("Play heart sounds").is_none());
    assert!(h.query_by_label("Play heart sounds (off by default)").is_none());
}

#[test]
fn a_registered_factory_brings_the_switch_and_the_card_and_is_not_called_while_off() {
    i18n::set_language("en");
    let calls = Rc::new(Cell::new(0));
    let mut player = HeartSoundPlayer::new();
    let c = calls.clone();
    player.set_factory(
        move || {
            c.set(c.get() + 1);
            Ok(Box::new(FakeSink::new()) as Box<dyn PcmSink>)
        },
        HeartSoundStatus::Unsupported,
    );
    assert!(player.has_output());
    let mut sim = Sim::new();
    for _ in 0..100 {
        sim.step(16);
        let i = sim.input(false);
        player.frame(&mut sim.sounds, &i);
    }
    assert_eq!(
        calls.get(),
        0,
        "the factory is not called while the setting is off"
    );
    let (play, info) = controls(&player, &Settings::default());
    assert_eq!(play, Some(false));
    let mut h = harness(play, info, Settings::default());
    h.run();
    let _ = h.get_by_label("Play heart sounds");
    let _ = h.get_by_label("Play heart sounds (off by default)");
    // switched on: called once
    sim.step(16);
    let i = sim.input(true);
    player.frame(&mut sim.sounds, &i);
    sim.step(16);
    let i = sim.input(true);
    player.frame(&mut sim.sounds, &i);
    assert_eq!(calls.get(), 1);
    assert_eq!(player.status(), HeartSoundStatus::Playing);
}

#[test]
fn a_failing_factory_shows_the_not_available_notice() {
    i18n::set_language("en");
    let mut player = HeartSoundPlayer::new();
    player.set_factory(
        || Err("no AudioContext".to_string()),
        HeartSoundStatus::Unsupported,
    );
    let mut sim = Sim::new();
    sim.step(16);
    let i = sim.input(true);
    player.frame(&mut sim.sounds, &i);
    assert_eq!(player.available(), Some(false));
    assert_eq!(player.status(), HeartSoundStatus::Unsupported);
    assert!(player.display_delay_ms().abs() < 1e-9);
    let settings = Settings {
        heart_sound_audio: true,
        ..Settings::default()
    };
    let (play, info) = controls(&player, &settings);
    assert_eq!(
        play,
        Some(true),
        "the switch stays, so the learner can turn it off"
    );
    let mut h = harness(play, info, settings);
    h.run();
    let shown = h
        .query_all_by_label("Sound is not available in this browser, so the heart sounds stay silent.")
        .count();
    assert_eq!(shown, 2, "under the popup's switch and in the Settings card");
    i18n::set_language("it");
    h.run();
    assert_eq!(
        h.query_all_by_label_contains("L'audio non è disponibile in questo browser")
            .count(),
        2
    );
    i18n::set_language("en");
}

#[test]
fn the_display_delay_is_the_sinks_latency_plus_the_offset() {
    let web_latency = WebScheduler::latency_ms(0.005, 0.02);
    assert!((f64::from(web_latency) - (LOOKAHEAD_MS + f64::from(QUEUE_TARGET_MS) + 25.0)).abs() < 1e-3);
    for (latency, want) in [
        (None, f64::from(QUEUE_TARGET_MS)),
        (Some(web_latency), f64::from(web_latency)),
    ] {
        let mut sim = Sim::new();
        let mut fake = FakeSink::priming();
        if let Some(l) = latency {
            fake = fake.with_latency_ms(l);
        }
        let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
        for _ in 0..60 {
            sim.step(16);
            let mut i = sim.input(true);
            i.offset_ms = 30.0;
            player.frame(&mut sim.sounds, &i);
            fake.drain_ms(16.0);
        }
        assert!(
            (player.display_delay_ms() - (want + 30.0)).abs() < 1e-6,
            "{latency:?}: {}",
            player.display_delay_ms()
        );
    }
}

#[test]
fn a_sink_waiting_for_a_click_gets_nothing_and_delays_nothing() {
    i18n::set_language("en");
    let mut sim = Sim::new();
    let fake = FakeSink::new().with_latency_ms(160.0);
    fake.set_readiness(SinkReadiness::WaitingForGesture);
    let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
    for _ in 0..30 {
        sim.step(16);
        let i = sim.input(true);
        player.frame(&mut sim.sounds, &i);
    }
    assert_eq!(player.status(), HeartSoundStatus::WaitingForClick);
    assert_eq!(fake.pushed().len(), 0, "nothing pushed before the click");
    assert!(player.display_delay_ms().abs() < 1e-9);
    // the click: the stream starts from the newest samples, nothing old is replayed
    fake.set_readiness(SinkReadiness::Running);
    sim.step(16);
    let i = sim.input(true);
    player.frame(&mut sim.sounds, &i);
    assert_eq!(player.status(), HeartSoundStatus::Playing);
    sim.step(16);
    let i = sim.input(true);
    player.frame(&mut sim.sounds, &i);
    let pushed = fake.pushed().len();
    assert!((60..=70).contains(&pushed), "one frame's samples, {pushed}");
    // the notice in the popup and the card
    let settings = Settings {
        heart_sound_audio: true,
        ..Settings::default()
    };
    fake.set_readiness(SinkReadiness::WaitingForGesture);
    let i = sim.input(true);
    player.frame(&mut sim.sounds, &i);
    let (play, _) = controls(&player, &settings);
    let info = HeartSoundInfo {
        status: player.status(),
        ..HeartSoundInfo::default()
    };
    let mut h = harness(play, Some(info), settings);
    h.run();
    let n = h
        .query_all_by_label_contains("Click anywhere to enable sound")
        .count();
    assert_eq!(n, 2, "under the popup's switch and in the Settings card");
}

/// A Web Audio sink in miniature: the real scheduler on a fake audio clock.
#[derive(Clone)]
struct ClockSink {
    s: Rc<std::cell::RefCell<WebScheduler>>,
    now_s: Rc<Cell<f64>>,
    chunks: Rc<std::cell::RefCell<Vec<heartsim_app::web_schedule::ScheduledChunk>>>,
}

impl PcmSink for ClockSink {
    fn push(&mut self, samples: &[f32]) -> usize {
        if let Some(c) = self.s.borrow_mut().push(self.now_s.get(), samples) {
            self.chunks.borrow_mut().push(c);
        }
        samples.len()
    }
    fn queued_ms(&self) -> f32 {
        self.s.borrow().sink_queued_ms(self.now_s.get())
    }
    fn clear(&mut self) {
        let _ = self.s.borrow_mut().clear(self.now_s.get());
    }
    fn set_gain(&mut self, gain: f32) {
        self.s.borrow_mut().set_gain(gain);
    }
    fn latency_ms(&self) -> f32 {
        WebScheduler::latency_ms(0.0, 0.0)
    }
}

#[test]
fn player_and_scheduler_keep_the_reported_delay_at_60_hz_and_at_8_fps() {
    let rate = 48_000.0;
    for frame_ms in [16u32, 110] {
        let sink = ClockSink {
            s: Rc::new(std::cell::RefCell::new(WebScheduler::new(rate))),
            now_s: Rc::new(Cell::new(5.0)),
            chunks: Rc::default(),
        };
        let mut player = HeartSoundPlayer::with_sink(Box::new(sink.clone()));
        let mut sim = Sim::new();
        for _ in 0..(8000 / frame_ms) {
            sim.step(frame_ms);
            sink.now_s.set(sink.now_s.get() + f64::from(frame_ms) / 1000.0);
            let mut i = sim.input(true);
            i.dt_s = f64::from(frame_ms) / 1000.0;
            player.frame(&mut sim.sounds, &i);
            let now = sink.now_s.get();
            let tail = sink.s.borrow_mut().tick(now);
            if let Some(c) = tail {
                sink.chunks.borrow_mut().push(c);
            }
        }
        let chunks = sink.chunks.borrow();
        assert_eq!(
            sink.s.borrow().stats().anchors,
            1,
            "{frame_ms} ms frames: one stream"
        );
        let want_ms = f64::from(WebScheduler::latency_ms(0.0, 0.0));
        for (k, c) in chunks.iter().enumerate() {
            if k > 0 {
                assert_eq!(c.start_frame, chunks[k - 1].end_frame(), "contiguous");
            }
            // the newest sample of each chunk is heard the reported delay after it was handed over
            let delay_ms = (c.end_frame() as f64 / rate - c.scheduled_at_s) * 1000.0;
            assert!(
                (delay_ms - want_ms).abs() < 3.0,
                "{frame_ms} ms frames, chunk {k}: {delay_ms:.1} ms vs {want_ms}"
            );
        }
        assert!((player.display_delay_ms() - want_ms).abs() < 1e-6);
    }
}
