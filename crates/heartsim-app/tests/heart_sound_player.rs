//! Acceptance tests of brief B25 for the heart-sound player, with a fake sink (no device): live
//! playback pushes the ring exactly once, underrun and backlog re-anchor without replaying, pause,
//! reload, a clock going back and speeds above 1× behave, a slice plays with a moving playhead, the
//! device is never opened while the switch is off, the display delay follows the sound, the limiter
//! holds the loudest beat, and the rings do not depend on whether sound is on.

use std::sync::atomic::{AtomicUsize, Ordering};

use heartsim_app::heart_sound_player::{HeartSoundPlayer, PlayerInput};
use heartsim_app::heart_sounds::HeartSounds;
use heartsim_app::sound_out::{FakeSink, HEADROOM_GAIN, LIMITER_KNEE, PcmSink, QUEUE_TARGET_MS, soft_limit};
use heartsim_core::Millis;
use heartsim_ecg::Tier0Scenario;
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::HeartSoundStatus;

const TARGET: u64 = 160; // QUEUE_TARGET_MS at 4 kHz

/// The app's live loop in miniature: timing engine, PCG ring in step with a 500 Hz strip.
struct Sim {
    engine: TimingEngine,
    sounds: HeartSounds,
    acc_ms: f64,
}

impl Sim {
    fn new() -> Self {
        Self::with(TimingParams::default())
    }

    fn with(params: TimingParams) -> Self {
        let mut sounds = HeartSounds::new(60.0);
        sounds.reset(
            7,
            params.sinus.respiration_rate_bpm,
            &heartsim_core::Auscultation::default(),
        );
        Self {
            engine: TimingEngine::new(params),
            sounds,
            acc_ms: 0.0,
        }
    }

    fn step(&mut self, ms: u32) {
        for _ in 0..ms {
            self.engine.step(Millis(1.0));
            let ev = self.engine.drain_events();
            self.sounds.on_events(&ev);
            self.acc_ms += 1.0;
            while self.acc_ms >= 2.0 {
                self.acc_ms -= 2.0;
                self.sounds.push_for_ecg_sample(self.engine.now());
            }
        }
    }

    fn now(&self) -> f64 {
        self.engine.now().0
    }
}

fn input(sim: &Sim, dt_ms: f64) -> PlayerInput {
    PlayerInput {
        enabled: true,
        volume: 1.0,
        offset_ms: 60.0,
        paused: false,
        speed: 1.0,
        sim_now_ms: sim.now(),
        dt_s: dt_ms / 1000.0,
        muted: false,
    }
}

/// Uneven frame times, ms (from 300 fps to 30 fps).
const FRAMES: [u32; 8] = [16, 7, 33, 3, 21, 12, 17, 9];

fn ring_slice(sounds: &HeartSounds, from: u64, to: u64) -> Vec<f32> {
    let mut out = Vec::new();
    let first = sounds.ring().range(from, to, &mut out);
    assert_eq!(first, from, "still in the ring");
    out
}

#[test]
fn live_playback_pushes_every_ring_sample_once_in_order() {
    let mut sim = Sim::new();
    let fake = FakeSink::priming();
    let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
    player.record_ranges(true);
    let mut elapsed = 0;
    let mut k = 0;
    while elapsed < 10_000 {
        let dt = FRAMES[k % FRAMES.len()];
        k += 1;
        sim.step(dt);
        elapsed += dt;
        let inp = input(&sim, f64::from(dt));
        player.frame(&mut sim.sounds, &inp);
        fake.drain_ms(f64::from(dt));
    }
    let ranges = player.take_ranges();
    assert!(
        ranges.windows(2).all(|w| w[0].1 == w[1].0),
        "contiguous, nothing skipped or repeated"
    );
    let (first, last) = (ranges[0].0, ranges.last().unwrap().1);
    assert_eq!(last, sim.sounds.ring().pushed(), "up to the newest sample");
    assert_eq!(
        fake.pushed(),
        ring_slice(&sim.sounds, first, last),
        "the ring, exactly"
    );
    assert!(
        last - first > 39_000,
        "about 10 s pushed ({} samples)",
        last - first
    );
    assert!(
        fake.pushed().iter().any(|v| v.abs() > 0.5),
        "with the heart sounds in it"
    );
    assert_eq!(fake.log().underruns, 0);
    assert_eq!(player.status(), HeartSoundStatus::Playing);
    assert!((fake.log().gain - HEADROOM_GAIN).abs() < 1e-6, "full volume");
}

#[test]
fn an_underrun_re_anchors_at_the_newest_sample_without_replaying() {
    let mut sim = Sim::new();
    // the device drains faster than the simulation produces
    let fake = FakeSink::new();
    let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
    player.record_ranges(true);
    for k in 0..300 {
        let dt = FRAMES[k % FRAMES.len()];
        sim.step(dt);
        let inp = input(&sim, f64::from(dt));
        player.frame(&mut sim.sounds, &inp);
        fake.drain_ms(1.5 * f64::from(dt));
    }
    // a long frame (100 ms of simulation) on an empty queue
    sim.step(100);
    let newest = sim.sounds.ring().pushed();
    let inp = input(&sim, 100.0);
    player.frame(&mut sim.sounds, &inp);
    let ranges = player.take_ranges();
    assert!(fake.log().underruns > 100, "the fake ran dry");
    assert!(
        ranges.windows(2).all(|w| w[0].1 <= w[1].0),
        "no sample pushed twice"
    );
    let last = *ranges.last().unwrap();
    assert_eq!(
        last,
        (newest - TARGET, newest),
        "re-anchored at the newest target's worth"
    );
}

#[test]
fn a_backlog_is_dropped_and_the_queue_returns_to_the_target_in_one_frame() {
    let mut sim = Sim::new();
    let fake = FakeSink::priming();
    let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
    player.record_ranges(true);
    for k in 0..200 {
        let dt = FRAMES[k % FRAMES.len()];
        sim.step(dt);
        let inp = input(&sim, f64::from(dt));
        player.frame(&mut sim.sounds, &inp);
        fake.drain_ms(f64::from(dt));
    }
    let _ = player.take_ranges();
    let clears = fake.log().clears;
    // a 500 ms stalled frame: the simulation ran on, the device took nothing
    sim.step(500);
    let newest = sim.sounds.ring().pushed();
    let inp = input(&sim, 500.0);
    player.frame(&mut sim.sounds, &inp);
    assert_eq!(fake.log().clears, clears + 1, "the old audio is dropped");
    assert_eq!(
        player.take_ranges(),
        vec![(newest - TARGET, newest)],
        "only the newest target"
    );
    assert!(
        (fake.queued_ms() - QUEUE_TARGET_MS).abs() < 0.5,
        "back at the target"
    );
}

#[test]
fn pause_clears_and_pushes_nothing_resume_and_reload_re_anchor() {
    let mut sim = Sim::new();
    let fake = FakeSink::priming();
    let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
    player.record_ranges(true);
    for _ in 0..100 {
        sim.step(16);
        let inp = input(&sim, 16.0);
        player.frame(&mut sim.sounds, &inp);
        fake.drain_ms(16.0);
    }
    let clears = fake.log().clears;
    let pushes = fake.log().pushes.len();
    // pause: the simulation stops, the player clears once and pushes nothing
    for _ in 0..30 {
        let mut i = input(&sim, 16.0);
        i.paused = true;
        player.frame(&mut sim.sounds, &i);
    }
    assert_eq!(fake.log().clears, clears + 1);
    assert_eq!(fake.log().pushes.len(), pushes);
    assert_eq!(player.status(), HeartSoundStatus::Paused);
    assert!(player.display_delay_ms() < 1e-9, "no delay while paused");
    // resume: playback re-anchors at the newest sample
    let _ = player.take_ranges();
    let at_resume = sim.sounds.ring().pushed();
    for _ in 0..10 {
        sim.step(16);
        let inp = input(&sim, 16.0);
        player.frame(&mut sim.sounds, &inp);
    }
    let ranges = player.take_ranges();
    assert!(ranges[0].0 >= at_resume, "nothing from before the pause");
    // a new scenario: a new engine, its clock from zero
    let clears = fake.log().clears;
    let mut sim2 = Sim::new();
    sim2.sounds = sim.sounds; // the app keeps its ring and resets it
    sim2.sounds
        .reset(3, 12.0, &heartsim_core::Auscultation::default());
    sim2.step(16);
    let inp = input(&sim2, 16.0);
    player.frame(&mut sim2.sounds, &inp);
    assert_eq!(fake.log().clears, clears + 1, "a reload clears");
    let reload_head = sim2.sounds.ring().pushed();
    sim2.step(16);
    let inp = input(&sim2, 16.0);
    player.frame(&mut sim2.sounds, &inp);
    let ranges = player.take_ranges();
    assert!(
        ranges.last().unwrap().0 >= reload_head,
        "re-anchored after the reload"
    );
    // a clock that goes back without a reset
    let clears = fake.log().clears;
    let mut back = input(&sim2, 16.0);
    back.sim_now_ms -= 500.0;
    player.frame(&mut sim2.sounds, &back);
    assert_eq!(fake.log().clears, clears + 1, "a clock going back clears");
}

#[test]
fn above_real_time_nothing_is_pushed_and_back_at_one_nothing_is_replayed() {
    let mut sim = Sim::new();
    let fake = FakeSink::priming();
    let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
    player.record_ranges(true);
    for _ in 0..60 {
        sim.step(16);
        let inp = input(&sim, 16.0);
        player.frame(&mut sim.sounds, &inp);
        fake.drain_ms(16.0);
    }
    let pushes = fake.log().pushes.len();
    for _ in 0..60 {
        sim.step(32);
        let mut i = input(&sim, 16.0);
        i.speed = 2.0;
        player.frame(&mut sim.sounds, &i);
        fake.drain_ms(16.0);
    }
    assert_eq!(fake.log().pushes.len(), pushes, "muted above 1×");
    assert_eq!(player.status(), HeartSoundStatus::MutedFast);
    assert!(player.display_delay_ms() < 1e-9);
    let _ = player.take_ranges();
    let back_at_one = sim.sounds.ring().pushed();
    for _ in 0..20 {
        sim.step(16);
        let inp = input(&sim, 16.0);
        player.frame(&mut sim.sounds, &inp);
        fake.drain_ms(16.0);
    }
    let ranges = player.take_ranges();
    assert!(!ranges.is_empty(), "playing again");
    assert!(
        ranges[0].0 >= back_at_one,
        "nothing from the fast stretch replayed"
    );
}

#[test]
fn slow_motion_plays_the_wall_time_synthesiser_in_real_time() {
    let mut sim = Sim::new();
    let fake = FakeSink::priming();
    let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
    for _ in 0..30 {
        sim.step(16);
        let inp = input(&sim, 16.0);
        player.frame(&mut sim.sounds, &inp);
        fake.drain_ms(16.0);
    }
    let before = fake.pushed().len();
    // 0.25×: 4 ms of simulation per 16 ms frame, for 8 s of wall time
    for _ in 0..500 {
        sim.step(4);
        let mut i = input(&sim, 16.0);
        i.speed = 0.25;
        player.frame(&mut sim.sounds, &i);
        fake.drain_ms(16.0);
    }
    assert_eq!(player.status(), HeartSoundStatus::SlowMotion);
    let pushed = fake.pushed()[before..].to_vec();
    // wall time: 16 ms per frame, 64 samples
    assert!(pushed.len().abs_diff(500 * 64) < 200, "{} samples", pushed.len());
    assert!(pushed.iter().any(|v| v.abs() > 0.3), "the sounds are there");
    assert_eq!(fake.log().underruns, 0);
    let want = (f64::from(QUEUE_TARGET_MS) + 60.0) * 0.25;
    assert!(
        (player.display_delay_ms() - want).abs() < 1e-6,
        "delay in simulation ms"
    );
}

#[test]
fn a_slice_plays_exactly_and_its_playhead_moves_to_the_end() {
    let mut sim = Sim::new();
    let fake = FakeSink::new();
    let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
    for _ in 0..200 {
        sim.step(16);
        let inp = input(&sim, 16.0);
        player.frame(&mut sim.sounds, &inp);
        fake.drain_ms(16.0);
    }
    let newest = sim.sounds.ring().pushed();
    let (start, end) = (newest - 8000, newest - 2000);
    let mut paused = input(&sim, 16.0);
    paused.paused = true;
    player.frame(&mut sim.sounds, &paused);
    let pushes = fake.log().pushes.len();
    player.play_slice(start, end);
    let mut heads = Vec::new();
    for _ in 0..200 {
        player.frame(&mut sim.sounds, &paused);
        if let Some(h) = player.playhead() {
            heads.push(h);
        }
        fake.drain_ms(16.0);
    }
    let played: Vec<f32> = fake.log().pushes[pushes..].concat();
    assert_eq!(played, ring_slice(&sim.sounds, start, end), "exactly the slice");
    assert!(heads.windows(2).all(|w| w[0] <= w[1]), "monotone playhead");
    assert!(heads[0] < start + 200, "starts at the slice start");
    assert_eq!(*heads.last().unwrap(), end, "ends at the slice end");
    assert_eq!(player.playhead(), None, "and is gone after it");
    assert!(!player.slice_playing());
    // Stop ends a slice; resuming the simulation does too
    player.play_slice(start, end);
    player.frame(&mut sim.sounds, &paused);
    assert!(player.slice_playing());
    player.stop_slice();
    player.frame(&mut sim.sounds, &paused);
    assert!(!player.slice_playing());
    player.play_slice(start, end);
    player.frame(&mut sim.sounds, &paused);
    let inp = input(&sim, 16.0);
    player.frame(&mut sim.sounds, &inp);
    assert!(!player.slice_playing(), "resume ends it");
    // ignored while running
    player.play_slice(start, end);
    sim.step(16);
    let inp = input(&sim, 16.0);
    player.frame(&mut sim.sounds, &inp);
    assert!(!player.slice_playing());
}

static FORBIDDEN_CALLS: AtomicUsize = AtomicUsize::new(0);

fn forbidden() -> Result<Box<dyn PcmSink>, String> {
    FORBIDDEN_CALLS.fetch_add(1, Ordering::SeqCst);
    Err("must not be called".into())
}

#[test]
fn with_the_switch_off_no_sink_is_ever_created() {
    let mut sim = Sim::new();
    let mut player = HeartSoundPlayer::with_factory(forbidden);
    for k in 0..200 {
        sim.step(16);
        let mut i = input(&sim, 16.0);
        i.enabled = false;
        i.paused = k % 50 > 40;
        i.speed = if k % 30 > 20 { 0.5 } else { 1.0 };
        player.frame(&mut sim.sounds, &i);
    }
    assert_eq!(
        FORBIDDEN_CALLS.load(Ordering::SeqCst),
        0,
        "the factory was not called"
    );
    assert_eq!(player.available(), None);
    assert_eq!(player.status(), HeartSoundStatus::Off);
    assert!(player.display_delay_ms() < 1e-9);
}

static FAILED: AtomicUsize = AtomicUsize::new(0);

fn no_device() -> Result<Box<dyn PcmSink>, String> {
    FAILED.fetch_add(1, Ordering::SeqCst);
    Err("no audio device".into())
}

#[test]
fn a_missing_device_is_tried_once_and_reported() {
    let mut sim = Sim::new();
    let mut player = HeartSoundPlayer::with_factory(no_device);
    for _ in 0..20 {
        sim.step(16);
        let inp = input(&sim, 16.0);
        player.frame(&mut sim.sounds, &inp);
    }
    assert_eq!(FAILED.load(Ordering::SeqCst), 1);
    assert_eq!(player.available(), Some(false));
    assert_eq!(player.status(), HeartSoundStatus::NoDevice);
    assert!(
        player.display_delay_ms() < 1e-9,
        "nothing plays, nothing is delayed"
    );
}

#[test]
fn the_display_delay_follows_the_sound_with_a_ramp() {
    let mut sim = Sim::new();
    let fake = FakeSink::priming();
    let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
    let mut seen = Vec::new();
    for _ in 0..40 {
        sim.step(16);
        let inp = input(&sim, 16.0);
        player.frame(&mut sim.sounds, &inp);
        fake.drain_ms(16.0);
        seen.push(player.display_delay_ms());
    }
    let full = f64::from(QUEUE_TARGET_MS) + 60.0;
    assert!(
        (player.display_delay_ms() - full).abs() < 1e-9,
        "target + offset behind"
    );
    assert!(seen[0] < 10.0, "no jump on the first frame");
    assert!(
        seen.windows(2).all(|w| w[1] >= w[0] && w[1] - w[0] < 15.0),
        "a ramp: {seen:?}"
    );
    assert!(
        seen.iter().any(|d| *d > 20.0 && *d < 80.0),
        "with values in between"
    );
    // the offset slider moves it
    for _ in 0..30 {
        sim.step(16);
        let mut i = input(&sim, 16.0);
        i.offset_ms = 150.0;
        player.frame(&mut sim.sounds, &i);
        fake.drain_ms(16.0);
    }
    assert!((player.display_delay_ms() - (f64::from(QUEUE_TARGET_MS) + 150.0)).abs() < 1e-9);
    // switched off: back to zero, again as a ramp
    let mut last = player.display_delay_ms();
    for _ in 0..30 {
        sim.step(16);
        let mut i = input(&sim, 16.0);
        i.enabled = false;
        player.frame(&mut sim.sounds, &i);
        assert!(player.display_delay_ms() <= last);
        last = player.display_delay_ms();
    }
    assert!(player.display_delay_ms().abs() < 1e-9);
}

#[test]
fn sound_on_or_off_gives_the_same_rings() {
    fn run(sound: bool) -> (Vec<f32>, u64) {
        let mut sim = Sim::new();
        let fake = FakeSink::priming();
        let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
        for k in 0..400 {
            let dt = FRAMES[k % FRAMES.len()];
            sim.step(dt);
            let mut i = input(&sim, f64::from(dt));
            i.enabled = sound;
            i.speed = if k % 100 > 70 { 0.5 } else { 1.0 };
            player.frame(&mut sim.sounds, &i);
            fake.drain_ms(f64::from(dt));
        }
        let ring = sim.sounds.ring();
        let mut out = Vec::new();
        ring.range(ring.oldest_index(), ring.pushed(), &mut out);
        (out, sim.engine.now().0.to_bits())
    }
    assert_eq!(run(true), run(false));
}

#[test]
fn the_sync_check_clicks_once_a_second_and_flashes_with_the_offset() {
    let mut sim = Sim::new();
    let fake = FakeSink::priming();
    let mut player = HeartSoundPlayer::with_sink(Box::new(fake.clone()));
    player.set_calibrating(true);
    let mut flashes = Vec::new();
    for k in 0..300 {
        let mut i = input(&sim, 16.0);
        i.enabled = true;
        player.frame(&mut sim.sounds, &i);
        fake.drain_ms(16.0);
        flashes.push(player.sync_flash());
        let _ = k;
    }
    assert_eq!(player.status(), HeartSoundStatus::Calibrating);
    let pushed = fake.pushed();
    let clicks: Vec<usize> = pushed
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] == 0.0 && w[1] != 0.0)
        .map(|(i, _)| i + 1)
        .collect();
    assert!(clicks.len() >= 4, "{} clicks", clicks.len());
    assert!(clicks.windows(2).all(|w| w[1] - w[0] == 4000), "one per second");
    // the marker lights about once a second
    let onsets = flashes.windows(2).filter(|w| !w[0] && w[1]).count();
    assert!((4..=5).contains(&onsets), "{onsets} flashes in 4.8 s");
    player.set_calibrating(false);
    let mut i = input(&sim, 16.0);
    i.enabled = false;
    player.frame(&mut sim.sounds, &i);
    assert!(!player.sync_flash());
}

fn scenario(stem: &str) -> Tier0Scenario {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../assets/scenarios/{stem}.ron"));
    Tier0Scenario::from_ron(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_loudest_s1_of_complete_av_block_stays_within_full_scale() {
    for stem in ["avb_3rd_junctional", "avb_3rd_ventricular"] {
        let rec = scenario(stem).record(30.0).unwrap();
        let loudest = rec.pcg.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(loudest > 1.0, "{stem}: complete block has loud beats ({loudest})");
        let out = |v: f32| soft_limit(HEADROOM_GAIN * 1.0 * v);
        let peak = rec.pcg.iter().map(|v| out(*v).abs()).fold(0.0f32, f32::max);
        assert!(peak <= 1.0, "{stem}: {peak}");
        // the headroom keeps even the loudest beat below the knee: the limiter changes nothing
        assert!(HEADROOM_GAIN * loudest <= LIMITER_KNEE, "{stem}: {loudest}");
    }
}

/// Brief B27: with the loudest lesion the B25 limiter is not driven beyond its knee on a normal
/// beat (every beat of these presets is a normal one), and the output stays within full scale.
#[test]
fn the_new_sounds_and_murmurs_stay_below_the_limiter_knee() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios");
    let mut checked = 0;
    let mut loudest = (String::new(), 0.0f32);
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "ron") {
            continue;
        }
        let s = Tier0Scenario::from_ron(&std::fs::read_to_string(&path).unwrap()).unwrap();
        if s.scenario.auscultation.is_empty() {
            continue;
        }
        let rec = s.record(20.0).unwrap();
        let peak = rec.pcg.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let out = rec
            .pcg
            .iter()
            .map(|v| soft_limit(HEADROOM_GAIN * *v).abs())
            .fold(0.0f32, f32::max);
        assert!(out <= 1.0, "{}: {out}", s.scenario.id);
        assert!(
            HEADROOM_GAIN * peak <= LIMITER_KNEE,
            "{}: peak {peak} drives the limiter",
            s.scenario.id
        );
        if peak > loudest.1 {
            loudest = (s.scenario.id.clone(), peak);
        }
        checked += 1;
    }
    eprintln!("loudest: {loudest:?}");
    assert!(checked >= 17, "{checked} scenarios with sounds");
}
