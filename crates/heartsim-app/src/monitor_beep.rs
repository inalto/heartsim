//! QRS-synchronous monitor beep (desktop builds with the `audio` feature).
//!
//! Bedside monitors sound a short tone on every detected QRS so that the rhythm can be followed by ear. This
//! module is the teaching-grade imitation: a 1 kHz tone synthesised here (no sound file, nothing taken from
//! any product), triggered by the ventricular activations the timing engine reports.
//!
//! The simulation never depends on audio. The app only *reports* ventricular events to [`MonitorBeep`]
//! and flushes it once per frame; the engines in `heartsim-sim`/`heartsim-ecg` know nothing about it. Everything
//! here is pure logic and always compiled; only [`crate::audio_out`] (feature `audio`) touches an audio device,
//! and it is created lazily the first time the beep is switched on, so a default run never opens a device.
//!
//! Timing: the beep is requested in the frame in which the ventricle activates. Its latency is therefore up
//! to one frame plus the audio buffer; it is not sample-accurate with the strip or the 3-D map.

use std::f64::consts::{PI, TAU};
use std::fmt;

/// Pitch of the beep, Hz. 1 kHz is the typical pitch of a bedside monitor's QRS tone.
pub const BEEP_FREQUENCY_HZ: f64 = 1000.0;
/// Total length of the beep, ms.
pub const BEEP_DURATION_MS: f64 = 100.0;
/// Raised-cosine fade-in, ms. Starting a tone at full amplitude would click.
pub const BEEP_ATTACK_MS: f64 = 5.0;
/// Raised-cosine fade-out, ms.
pub const BEEP_RELEASE_MS: f64 = 20.0;

/// Minimum spacing between two beeps, in **simulation** time. Activations closer than this to the last
/// beep are not sounded (a cap of about 330 beeps per minute of simulated time), so fibrillation, a flutter
/// or a fast-forward that packs many beats into one frame cannot turn the beep into a buzz. Because the cap is
/// in simulated time, at 10x speed beeps can still follow each other ten times faster in wall-clock time.
pub const MIN_SPACING_MS: f64 = 180.0;

/// A pending beep older than this (simulation ms, measured at the flush) is dropped instead of sounded: a
/// late beep after a long frame or a scrub would no longer match the QRS it announces.
pub const STALE_AFTER_MS: f64 = 250.0;

/// Number of samples that span `ms` at `sample_rate_hz` (rounded, never negative).
fn samples_for_ms(sample_rate_hz: u32, ms: f64) -> usize {
    (f64::from(sample_rate_hz) * ms / 1000.0).round().max(0.0) as usize
}

/// The beep as mono samples in `-1.0..=1.0` at `sample_rate_hz`.
///
/// A [`BEEP_FREQUENCY_HZ`] sine of [`BEEP_DURATION_MS`] with a [`BEEP_ATTACK_MS`] raised-cosine attack and a
/// [`BEEP_RELEASE_MS`] release, so the first and last samples are exactly zero and the tone does not click.
/// The peak is 1.0 (the caller scales it with the volume). A pure function of the sample rate: the same rate
/// always gives bit-identical samples.
#[must_use]
pub fn synth_beep(sample_rate_hz: u32) -> Vec<f32> {
    let n = samples_for_ms(sample_rate_hz, BEEP_DURATION_MS);
    let attack = samples_for_ms(sample_rate_hz, BEEP_ATTACK_MS);
    let release = samples_for_ms(sample_rate_hz, BEEP_RELEASE_MS);
    let rate = f64::from(sample_rate_hz.max(1));
    (0..n)
        .map(|i| {
            let phase = TAU * BEEP_FREQUENCY_HZ * (i as f64) / rate;
            let mut env = 1.0_f64;
            if i < attack {
                env = env.min(0.5 * (1.0 - (PI * i as f64 / attack as f64).cos()));
            }
            // distance from the last sample: 0 at the very end, `release` at the start of the fade
            let from_end = n - 1 - i;
            if from_end < release {
                env = env.min(0.5 * (1.0 - (PI * from_end as f64 / release as f64).cos()));
            }
            (env * phase.sin()) as f32
        })
        .collect()
}

/// Decides when a requested beep is actually sounded. Pure and clocked by simulation time, so it behaves the
/// same at any frame rate and simulation speed.
///
/// Rules, in order, applied by [`BeepGate::take`]: nothing pending means no beep; a pending beep older than
/// [`STALE_AFTER_MS`] is dropped; one closer than [`MIN_SPACING_MS`] to the previous beep is dropped. Many
/// events recorded between two `take` calls coalesce into one request that keeps the latest activation.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BeepGate {
    pending_at_ms: Option<f64>,
    last_beep_at_ms: Option<f64>,
}

impl BeepGate {
    /// An empty gate: nothing pending, nothing sounded yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a ventricular activation at simulation time `at_ms`. Several calls before the next
    /// [`BeepGate::take`] coalesce into one pending beep at the latest time. Non-finite times are ignored.
    pub fn on_ventricular_event(&mut self, at_ms: f64) {
        if !at_ms.is_finite() {
            return;
        }
        self.pending_at_ms = Some(self.pending_at_ms.map_or(at_ms, |p| p.max(at_ms)));
    }

    /// Whether to sound a beep now (`now_ms` is the current simulation time). Returns `true` at most once per
    /// call and consumes the pending request either way, so a dropped beep is not retried.
    pub fn take(&mut self, now_ms: f64) -> bool {
        let Some(at_ms) = self.pending_at_ms.take() else {
            return false;
        };
        if now_ms - at_ms > STALE_AFTER_MS {
            return false;
        }
        // A scenario reload restarts simulation time, so `at_ms` can precede the last beep: that is a new
        // timeline, not a beat that is too close to the previous one.
        if let Some(last_ms) = self.last_beep_at_ms {
            let since_last_ms = at_ms - last_ms;
            if (0.0..MIN_SPACING_MS).contains(&since_last_ms) {
                return false;
            }
        }
        self.last_beep_at_ms = Some(at_ms);
        true
    }

    /// Forgets the pending request and the last beep (used when the beep is muted).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Something that can sound the beep. The real implementation is `audio_out::KiraBeepSink` (feature
/// `audio`); tests use a fake that records the calls.
pub trait BeepSink {
    /// Sounds the beep once at linear `volume` in `0.0..=1.0` (already clamped by [`MonitorBeep`]).
    /// Failures are the sink's to swallow: a missed beep must never disturb the simulation.
    fn play(&mut self, volume: f32);
}

/// Creates the audio sink the first time the beep is switched on; the error is logged once.
type SinkFactory = fn() -> Result<Box<dyn BeepSink>, String>;

#[cfg(feature = "audio")]
fn create_device_sink() -> Result<Box<dyn BeepSink>, String> {
    Ok(Box::new(crate::audio_out::KiraBeepSink::new()?))
}

/// The factory of this build: the kira output with feature `audio`, none without it.
#[cfg(feature = "audio")]
const DEVICE_FACTORY: Option<SinkFactory> = Some(create_device_sink);
/// The factory of this build: the kira output with feature `audio`, none without it.
#[cfg(not(feature = "audio"))]
const DEVICE_FACTORY: Option<SinkFactory> = None;

enum SinkState {
    /// The device has not been asked for (the beep was never switched on).
    NotTried,
    Ready(Box<dyn BeepSink>),
    /// No device (or no audio support in this build): silent for the rest of the run, no retry.
    Failed,
}

/// The monitor beep as the app sees it: report ventricular events, set the mute/volume state, flush once
/// per frame.
///
/// Off by default. The audio sink is created only the first time [`MonitorBeep::set`] enables beeping, never
/// at start-up; if that fails (no audio device, as on CI) it logs one warning and stays silent for the rest of
/// the run without retrying. Without the `audio` feature it never creates a sink, so callers need no `cfg`.
pub struct MonitorBeep {
    enabled: bool,
    volume: f32,
    gate: BeepGate,
    state: SinkState,
    factory: Option<SinkFactory>,
}

impl fmt::Debug for MonitorBeep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonitorBeep")
            .field("enabled", &self.enabled)
            .field("volume", &self.volume)
            .field("available", &self.available())
            .finish_non_exhaustive()
    }
}

impl Default for MonitorBeep {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorBeep {
    /// A disabled beep that creates the platform audio sink on first use (feature `audio`), or none.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: false,
            volume: 0.5,
            gate: BeepGate::new(),
            state: SinkState::NotTried,
            factory: DEVICE_FACTORY,
        }
    }

    /// A disabled beep with a ready sink, for tests.
    #[must_use]
    pub fn with_sink(sink: Box<dyn BeepSink>) -> Self {
        Self {
            state: SinkState::Ready(sink),
            factory: None,
            ..Self::new()
        }
    }

    /// A disabled beep that creates its sink with `factory` on first enable, for tests of the failure path.
    #[must_use]
    pub fn with_factory(factory: fn() -> Result<Box<dyn BeepSink>, String>) -> Self {
        Self {
            factory: Some(factory),
            ..Self::new()
        }
    }

    /// Applies the user's choice; call it every frame. `volume` is linear, clamped to `0.0..=1.0` (NaN counts
    /// as 0). The first call with `enabled` creates the sink; muting drops anything still pending.
    pub fn set(&mut self, enabled: bool, volume: f32) {
        self.volume = if volume.is_nan() {
            0.0
        } else {
            volume.clamp(0.0, 1.0)
        };
        if enabled != self.enabled {
            self.enabled = enabled;
            if !enabled {
                self.gate.reset();
            }
        }
        if self.enabled && matches!(self.state, SinkState::NotTried) {
            self.state = self.open_sink();
        }
    }

    fn open_sink(&self) -> SinkState {
        let Some(factory) = self.factory else {
            tracing::debug!("monitor beep: this build has no audio output");
            return SinkState::Failed;
        };
        match factory() {
            Ok(sink) => SinkState::Ready(sink),
            Err(e) => {
                tracing::warn!("monitor beep unavailable, staying silent: {e}");
                SinkState::Failed
            }
        }
    }

    /// Reports a ventricular activation at simulation time `at_ms`. Ignored while the beep is off, so events
    /// that happened before it was switched on never sound.
    pub fn on_ventricular_event(&mut self, at_ms: f64) {
        if self.enabled {
            self.gate.on_ventricular_event(at_ms);
        }
    }

    /// Call once per frame after the simulation stepped, with the current simulation time: sounds the beep
    /// if it is on, a sink exists, the volume is above zero and the gate lets it through.
    pub fn flush(&mut self, now_ms: f64) {
        if !self.enabled || !self.gate.take(now_ms) || self.volume <= 0.0 {
            return;
        }
        if let SinkState::Ready(sink) = &mut self.state {
            sink.play(self.volume);
        }
    }

    /// `None` until the audio device has been asked for (the beep was never enabled), then `Some(true)` when
    /// a sink is ready and `Some(false)` when there is none (no device, or no `audio` feature).
    #[must_use]
    pub fn available(&self) -> Option<bool> {
        match self.state {
            SinkState::NotTried => None,
            SinkState::Ready(_) => Some(true),
            SinkState::Failed => Some(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn beep_has_the_documented_length() {
        assert_eq!(synth_beep(44_100).len(), 4_410);
        assert_eq!(synth_beep(48_000).len(), 4_800);
        assert_eq!(synth_beep(0), Vec::<f32>::new());
    }

    #[test]
    fn beep_peak_is_one_and_never_clips() {
        for rate in [22_050, 44_100, 48_000, 96_000] {
            let peak = synth_beep(rate).iter().fold(0.0_f32, |m, s| m.max(s.abs()));
            assert!(peak <= 1.0, "{rate} Hz: peak {peak}");
            assert!(peak > 0.98, "{rate} Hz: peak {peak}");
        }
    }

    #[test]
    fn beep_starts_and_ends_at_zero() {
        for rate in [44_100, 48_000] {
            let s = synth_beep(rate);
            assert!(s[0].abs() < 1e-6, "first sample {}", s[0]);
            assert!(s[s.len() - 1].abs() < 1e-6, "last sample {}", s[s.len() - 1]);
            // the first and last millisecond are inside the fades: far below full scale
            let ms = (rate / 1000) as usize;
            assert!(s[..ms].iter().all(|v| v.abs() < 0.2));
            assert!(s[s.len() - ms..].iter().all(|v| v.abs() < 0.2));
        }
    }

    #[test]
    fn beep_is_a_one_kilohertz_tone() {
        let s = synth_beep(48_000);
        let mut crossings = 0;
        let mut last_positive: Option<bool> = None;
        for v in s.iter().filter(|v| v.abs() > 1e-9) {
            let positive = *v > 0.0;
            if last_positive.is_some_and(|p| p != positive) {
                crossings += 1;
            }
            last_positive = Some(positive);
        }
        // 100 ms of 1 kHz is 100 cycles, 200 zero crossings
        assert!((195..=205).contains(&crossings), "{crossings} zero crossings");
    }

    #[test]
    fn beep_is_deterministic() {
        assert_eq!(synth_beep(44_100), synth_beep(44_100));
    }

    #[test]
    fn gate_is_silent_while_nothing_is_pending() {
        let mut g = BeepGate::new();
        assert!(!g.take(0.0));
        assert!(!g.take(10_000.0));
    }

    #[test]
    fn gate_sounds_a_single_event_once() {
        let mut g = BeepGate::new();
        g.on_ventricular_event(1_000.0);
        assert!(g.take(1_010.0));
        assert!(!g.take(1_020.0), "the request is consumed");
    }

    #[test]
    fn gate_coalesces_events_of_one_frame() {
        let mut g = BeepGate::new();
        for k in 0..20 {
            g.on_ventricular_event(1_000.0 + 10.0 * f64::from(k));
        }
        assert!(g.take(1_200.0));
        assert!(!g.take(1_201.0));
    }

    #[test]
    fn gate_enforces_the_minimum_spacing_in_simulation_time() {
        let mut g = BeepGate::new();
        g.on_ventricular_event(1_000.0);
        assert!(g.take(1_000.0));
        g.on_ventricular_event(1_000.0 + MIN_SPACING_MS - 1.0);
        assert!(!g.take(1_000.0 + MIN_SPACING_MS), "too close to the last beep");
        g.on_ventricular_event(1_000.0 + MIN_SPACING_MS);
        assert!(g.take(1_000.0 + MIN_SPACING_MS), "exactly the spacing is allowed");
    }

    #[test]
    fn gate_follows_a_300_bpm_rhythm() {
        // 200 ms apart is farther than the cap, so every beat sounds
        let mut g = BeepGate::new();
        let mut sounded = 0;
        for k in 0..10 {
            let t = 200.0 * f64::from(k);
            g.on_ventricular_event(t);
            sounded += i32::from(g.take(t + 16.0));
        }
        assert_eq!(sounded, 10);
    }

    #[test]
    fn gate_drops_a_stale_request() {
        let mut g = BeepGate::new();
        g.on_ventricular_event(1_000.0);
        assert!(!g.take(1_000.0 + STALE_AFTER_MS + 1.0));
        g.on_ventricular_event(5_000.0);
        assert!(g.take(5_000.0 + STALE_AFTER_MS), "just fresh enough");
    }

    #[test]
    fn gate_accepts_a_restarted_timeline() {
        let mut g = BeepGate::new();
        g.on_ventricular_event(60_000.0);
        assert!(g.take(60_010.0));
        // the scenario was reloaded: simulation time starts again at zero
        g.on_ventricular_event(300.0);
        assert!(g.take(310.0));
    }

    #[test]
    fn gate_ignores_non_finite_times() {
        let mut g = BeepGate::new();
        g.on_ventricular_event(f64::NAN);
        g.on_ventricular_event(f64::INFINITY);
        assert!(!g.take(0.0));
    }

    /// Records the volume of every beep.
    struct FakeSink(Rc<RefCell<Vec<f32>>>);

    impl BeepSink for FakeSink {
        fn play(&mut self, volume: f32) {
            self.0.borrow_mut().push(volume);
        }
    }

    fn fake() -> (MonitorBeep, Rc<RefCell<Vec<f32>>>) {
        let log = Rc::new(RefCell::new(Vec::new()));
        (MonitorBeep::with_sink(Box::new(FakeSink(log.clone()))), log)
    }

    #[test]
    fn disabled_by_default_plays_nothing() {
        let (mut b, log) = fake();
        b.on_ventricular_event(1_000.0);
        b.flush(1_010.0);
        assert!(log.borrow().is_empty());
        b.set(false, 1.0);
        b.on_ventricular_event(2_000.0);
        b.flush(2_010.0);
        assert!(log.borrow().is_empty());
    }

    #[test]
    fn enabling_plays_on_events_with_the_volume() {
        let (mut b, log) = fake();
        b.set(true, 0.25);
        b.on_ventricular_event(1_000.0);
        b.flush(1_010.0);
        b.on_ventricular_event(2_000.0);
        b.flush(2_010.0);
        assert_eq!(*log.borrow(), vec![0.25, 0.25]);
    }

    #[test]
    fn volume_is_clamped_to_the_unit_range() {
        let (mut b, log) = fake();
        b.set(true, 3.0);
        b.on_ventricular_event(1_000.0);
        b.flush(1_000.0);
        b.set(true, f32::NAN);
        b.on_ventricular_event(2_000.0);
        b.flush(2_000.0);
        b.set(true, -1.0);
        b.on_ventricular_event(3_000.0);
        b.flush(3_000.0);
        assert_eq!(*log.borrow(), vec![1.0], "zero volume is silent, not a beep");
    }

    #[test]
    fn muting_stops_the_beep_and_drops_what_is_pending() {
        let (mut b, log) = fake();
        b.set(true, 0.5);
        b.on_ventricular_event(1_000.0);
        b.set(false, 0.5);
        b.flush(1_010.0);
        assert!(log.borrow().is_empty(), "pending beep dropped by the mute");
        b.on_ventricular_event(2_000.0);
        b.flush(2_010.0);
        assert!(log.borrow().is_empty());
        b.set(true, 0.5);
        b.on_ventricular_event(3_000.0);
        b.flush(3_010.0);
        assert_eq!(log.borrow().len(), 1);
    }

    #[test]
    fn many_events_in_one_frame_sound_once() {
        let (mut b, log) = fake();
        b.set(true, 0.5);
        for k in 0..30 {
            b.on_ventricular_event(1_000.0 + f64::from(k));
        }
        b.flush(1_100.0);
        assert_eq!(log.borrow().len(), 1);
    }

    static FAILED_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

    fn no_device() -> Result<Box<dyn BeepSink>, String> {
        FAILED_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
        Err("no audio device".to_string())
    }

    #[test]
    fn a_missing_device_is_silent_and_tried_once() {
        let mut b = MonitorBeep::with_factory(no_device);
        assert_eq!(
            b.available(),
            None,
            "nothing is opened before the beep is enabled"
        );
        for frame in 0..5 {
            b.set(true, 1.0);
            b.on_ventricular_event(1_000.0 * f64::from(frame));
            b.flush(1_000.0 * f64::from(frame) + 5.0);
        }
        assert_eq!(b.available(), Some(false));
        assert_eq!(FAILED_ATTEMPTS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn the_sink_is_not_created_until_the_beep_is_enabled() {
        fn forbidden() -> Result<Box<dyn BeepSink>, String> {
            panic!("the audio device must not be opened while the beep is off");
        }
        let mut b = MonitorBeep::with_factory(forbidden);
        b.set(false, 0.5);
        b.on_ventricular_event(1_000.0);
        b.flush(1_010.0);
        assert_eq!(b.available(), None);
    }

    #[test]
    fn a_default_beep_is_off_and_untouched() {
        let b = MonitorBeep::new();
        assert_eq!(b.available(), None);
    }
}
