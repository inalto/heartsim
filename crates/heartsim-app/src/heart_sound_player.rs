//! Playing the phonocardiogram (brief B25): what to hand the [`PcmSink`] each frame, and how far
//! behind the engine the views are drawn while sound plays.
//!
//! Pure and always compiled: the sink is a trait object made by a factory that the embedding crate
//! provides (B26): the kira output compiled in with the `audio` feature ([`DEVICE_FACTORY`]), the
//! Web Audio output that `heartsim-web` registers, or none. "Is sound available here" means "is there
//! a factory" ([`HeartSoundPlayer::has_output`]). The factory is called the first time the learner
//! switches the heart sounds on. Nothing here reaches back into the simulation: the player reads the PCG ring
//! and the engine's clock, and writes to the sink.
//!
//! * **Live at 1×** every new ring sample is pushed once, in order. The sink waits for
//!   [`QUEUE_TARGET_MS`] before it plays, so the sound runs that far behind the newest sample. An
//!   underrun re-anchors at the newest samples (nothing is replayed); a backlog above
//!   [`BACKLOG_FACTOR`] × the target (a stalled frame) clears the sink and keeps only the newest
//!   target's worth.
//! * **Slow motion** (speed below 1×) pushes the [`heartsim_ecg::WallTimeSynth`] instead: natural
//!   pitch, stretched intervals.
//! * **Above 1×** the heart sounds are muted (each sound would crowd the next); the beep is not.
//! * **Paused**: the sink is cleared (it fades out) and nothing is pushed, except a slice of the ring
//!   played on request ([`HeartSoundPlayer::play_slice`]) with a moving playhead.
//! * A new engine (scenario load, restart, EP lab, re-entry) or a clock that goes back: clear and
//!   re-anchor.
//!
//! The views follow through [`HeartSoundPlayer::display_delay_ms`]: the sink's own constant delay
//! ([`PcmSink::latency_ms`]: [`QUEUE_TARGET_MS`] for kira, look-ahead plus the browser's reported
//! latency on the web) plus the user's audio offset while sound is playing, zero otherwise, changed
//! over [`DELAY_RAMP_S`] so the strip never jumps. While a browser waits for the learner's click
//! ([`SinkReadiness::WaitingForGesture`]) nothing is pushed and nothing is delayed.

use std::fmt;

use heartsim_ecg::PCG_RATE_HZ;
pub use heartsim_ui::HeartSoundStatus;

use crate::heart_sounds::HeartSounds;
use crate::sound_out::{
    BACKLOG_FACTOR, HEADROOM_GAIN, PcmSink, QUEUE_TARGET_MS, SinkReadiness, ms_for_samples, samples_for_ms,
    sync_click,
};

/// Time over which the display delay moves to a new value, s. Designed (brief B25: about 0.3 s):
/// slow enough that the strip does not visibly jump, fast enough to settle before the next beat.
pub const DELAY_RAMP_S: f64 = 0.3;
/// Speeds within this of 1 count as real time (the ring is played as drawn). Designed: the speed
/// slider is logarithmic, and a learner who drags it back to 1 lands near, not on, 1.
pub const REAL_TIME_TOLERANCE: f64 = 0.005;
/// Interval between sync-check clicks, ms. Designed (brief B25: once a second).
pub const SYNC_CLICK_PERIOD_MS: f64 = 1000.0;
/// How long the sync-check marker stays lit after its click, ms. Designed: long enough to see, short
/// enough that its onset is the event.
pub const SYNC_FLASH_MS: f64 = 120.0;

/// A plain function that creates a sink (the compiled-in desktop output, tests).
pub type PcmSinkFn = fn() -> Result<Box<dyn PcmSink>, String>;
/// Creates the sink the first time the heart sounds are switched on. A closure, so that an
/// embedding crate can hand over state it shares with the sink (the web build's audio context and
/// its gesture listeners).
pub type PcmSinkFactory = Box<dyn FnMut() -> Result<Box<dyn PcmSink>, String>>;

#[cfg(feature = "audio")]
fn create_device_sink() -> Result<Box<dyn PcmSink>, String> {
    Ok(Box::new(crate::audio_out::heart_sound_sink()?))
}

/// The factory compiled into this crate: the kira output with feature `audio`, none without it.
/// An embedding crate may register another one ([`HeartSoundPlayer::set_factory`]).
#[cfg(feature = "audio")]
pub const DEVICE_FACTORY: Option<PcmSinkFn> = Some(create_device_sink);
/// The factory compiled into this crate: the kira output with feature `audio`, none without it.
/// An embedding crate may register another one ([`HeartSoundPlayer::set_factory`]).
#[cfg(not(feature = "audio"))]
pub const DEVICE_FACTORY: Option<PcmSinkFn> = None;

/// Whether this crate was built with the desktop audio output (the `audio` feature). Whether a
/// running app can play heart sounds is [`HeartSoundPlayer::has_output`]: the web build registers
/// its own factory.
pub const AUDIO_BUILD: bool = DEVICE_FACTORY.is_some();

/// What the app tells the player once per frame, after stepping the simulation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerInput {
    /// The learner's "Play heart sounds" switch.
    pub enabled: bool,
    /// Linear volume `0.0..=1.0`.
    pub volume: f32,
    /// The user's audio offset (device latency), ms.
    pub offset_ms: f32,
    pub paused: bool,
    /// Simulation speed, multiples of real time.
    pub speed: f64,
    /// The engine's clock after this frame's steps, ms.
    pub sim_now_ms: f64,
    /// Wall time of this frame, s.
    pub dt_s: f64,
    /// No sound for a reason outside the player (an imported recording is in the strip).
    pub muted: bool,
}

/// How far the views are drawn behind the engine, ms of simulation time: moves to each new target
/// along a smoothstep over [`DELAY_RAMP_S`] of wall time.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DisplayDelay {
    value_ms: f64,
    from_ms: f64,
    to_ms: f64,
    t_s: f64,
}

impl DisplayDelay {
    /// Advance by `dt_s` towards `target_ms`; returns the delay to draw with now.
    pub fn update(&mut self, target_ms: f64, dt_s: f64) -> f64 {
        let target_ms = target_ms.max(0.0);
        if target_ms.to_bits() != self.to_ms.to_bits() {
            self.from_ms = self.value_ms;
            self.to_ms = target_ms;
            self.t_s = 0.0;
        }
        self.t_s += dt_s.max(0.0);
        let x = (self.t_s / DELAY_RAMP_S).clamp(0.0, 1.0);
        let s = x * x * (3.0 - 2.0 * x);
        self.value_ms = self.from_ms + (self.to_ms - self.from_ms) * s;
        self.value_ms
    }

    /// The delay now, ms.
    #[must_use]
    pub fn value_ms(&self) -> f64 {
        self.value_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Off,
    Live,
    Slow,
    Fast,
    Paused,
    Slice,
    Calibrate,
}

/// A ring slice being played while paused.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Slice {
    start: u64,
    end: u64,
    next: u64,
    pushed: u64,
    /// Wall time since everything was consumed, in samples (the offset still to elapse).
    tail: f64,
    playhead: u64,
    /// The playhead reached the end this frame: reported once more, then the slice is over.
    done: bool,
}

enum SinkState {
    NotTried,
    Ready(Box<dyn PcmSink>),
    Failed,
}

/// Decides, frame by frame, what the heart-sound sink plays. See the module documentation.
pub struct HeartSoundPlayer {
    sink: SinkState,
    factory: Option<PcmSinkFactory>,
    /// What to report when the factory fails.
    unavailable: HeartSoundStatus,
    mode: Mode,
    generation: Option<u64>,
    last_sim_ms: f64,
    /// Next ring index to push in live mode.
    next: u64,
    /// Something has been pushed since live playback (re)started.
    started: bool,
    slice: Option<Slice>,
    slice_request: Option<(u64, u64)>,
    calibrate: bool,
    /// Sync-check stream: samples generated, and the flash state.
    calib_pos: u64,
    calib_pushed: u64,
    flash: bool,
    delay: DisplayDelay,
    status: HeartSoundStatus,
    scratch: Vec<f32>,
    /// Ring ranges pushed in live mode, when recording (tests).
    ranges: Option<Vec<(u64, u64)>>,
}

impl fmt::Debug for HeartSoundPlayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeartSoundPlayer")
            .field("mode", &self.mode)
            .field("status", &self.status)
            .field("delay_ms", &self.delay.value_ms())
            .finish_non_exhaustive()
    }
}

impl Default for HeartSoundPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl HeartSoundPlayer {
    /// A player that opens this build's device sink the first time it is enabled (none without
    /// the `audio` feature, until an embedding crate registers one with [`Self::set_factory`]).
    #[must_use]
    pub fn new() -> Self {
        Self::with_factory_opt(DEVICE_FACTORY.map(|f| Box::new(f) as PcmSinkFactory))
    }

    /// A player with no output at all (the web build before it registers its own, a desktop
    /// build without `audio`): silent, and the interface shows no sound controls.
    #[must_use]
    pub fn without_output() -> Self {
        Self::with_factory_opt(None)
    }

    /// A player whose sink comes from `factory` on first enable.
    #[must_use]
    pub fn with_factory(factory: impl FnMut() -> Result<Box<dyn PcmSink>, String> + 'static) -> Self {
        Self::with_factory_opt(Some(Box::new(factory)))
    }

    /// Use `factory` for the sink from now on (the web build registers its Web Audio output when it
    /// creates the app); a sink already open is dropped. `unavailable` is the status shown when
    /// the factory fails ([`HeartSoundStatus::NoDevice`] on a desktop,
    /// [`HeartSoundStatus::Unsupported`] in a browser).
    pub fn set_factory(
        &mut self,
        factory: impl FnMut() -> Result<Box<dyn PcmSink>, String> + 'static,
        unavailable: HeartSoundStatus,
    ) {
        self.factory = Some(Box::new(factory));
        self.unavailable = unavailable;
        self.sink = SinkState::NotTried;
        self.mode = Mode::Off;
    }

    /// Whether heart sounds can be played at all here: there is a factory (or a sink). The Display
    /// popup's switch and the Settings card exist exactly then.
    #[must_use]
    pub fn has_output(&self) -> bool {
        self.factory.is_some() || matches!(self.sink, SinkState::Ready(_))
    }

    /// A player with a ready sink (tests).
    #[must_use]
    pub fn with_sink(sink: Box<dyn PcmSink>) -> Self {
        Self {
            sink: SinkState::Ready(sink),
            ..Self::with_factory_opt(None)
        }
    }

    fn with_factory_opt(factory: Option<PcmSinkFactory>) -> Self {
        Self {
            sink: SinkState::NotTried,
            factory,
            unavailable: HeartSoundStatus::NoDevice,
            mode: Mode::Off,
            generation: None,
            last_sim_ms: f64::NEG_INFINITY,
            next: 0,
            started: false,
            slice: None,
            slice_request: None,
            calibrate: false,
            calib_pos: 0,
            calib_pushed: 0,
            flash: false,
            delay: DisplayDelay::default(),
            status: HeartSoundStatus::Off,
            scratch: Vec::new(),
            ranges: None,
        }
    }

    /// `None` until the device has been asked for, then whether a sink is ready.
    #[must_use]
    pub fn available(&self) -> Option<bool> {
        match self.sink {
            SinkState::NotTried => None,
            SinkState::Ready(_) => Some(true),
            SinkState::Failed => Some(false),
        }
    }

    /// What the sound is doing, for the Display popup and Settings.
    #[must_use]
    pub fn status(&self) -> HeartSoundStatus {
        self.status
    }

    /// How far the strip, the 12-lead page, the heart-sound lane and the 3-D map are drawn behind
    /// the engine, ms of simulation time.
    #[must_use]
    pub fn display_delay_ms(&self) -> f64 {
        self.delay.value_ms()
    }

    /// While a slice plays: the PCG ring index being heard now.
    #[must_use]
    pub fn playhead(&self) -> Option<u64> {
        self.slice
            .filter(|_| self.mode == Mode::Slice)
            .map(|s| s.playhead)
    }

    /// A slice is playing (the lane's button says "Stop").
    #[must_use]
    pub fn slice_playing(&self) -> bool {
        self.mode == Mode::Slice
    }

    /// While paused: play PCG ring samples `start..end` once (the next frame starts it). Ignored
    /// while the simulation runs.
    pub fn play_slice(&mut self, start: u64, end: u64) {
        if end > start {
            self.slice_request = Some((start, end));
        }
    }

    /// End a slice that is playing.
    pub fn stop_slice(&mut self) {
        self.slice_request = None;
        self.slice = None;
    }

    /// Run (or stop) the sync check: a click once a second through the heart-sound sink.
    pub fn set_calibrating(&mut self, on: bool) {
        self.calibrate = on;
    }

    /// The sync check is running.
    #[must_use]
    pub fn calibrating(&self) -> bool {
        self.calibrate
    }

    /// The sync-check marker is lit: a click is being heard now, if the offset is right.
    #[must_use]
    pub fn sync_flash(&self) -> bool {
        self.flash && self.mode == Mode::Calibrate
    }

    /// Record the ring ranges pushed in live mode (tests).
    pub fn record_ranges(&mut self, on: bool) {
        self.ranges = on.then(Vec::new);
    }

    /// The ranges recorded since the last call.
    pub fn take_ranges(&mut self) -> Vec<(u64, u64)> {
        self.ranges.as_mut().map(std::mem::take).unwrap_or_default()
    }

    fn open_sink(&mut self) {
        let Some(factory) = self.factory.as_mut() else {
            tracing::debug!("heart sounds: this build has no audio output");
            self.sink = SinkState::Failed;
            return;
        };
        self.sink = match factory() {
            Ok(sink) => SinkState::Ready(sink),
            Err(e) => {
                tracing::warn!("heart sounds unavailable, staying silent: {e}");
                SinkState::Failed
            }
        };
    }

    fn sink(&mut self) -> Option<&mut Box<dyn PcmSink>> {
        match &mut self.sink {
            SinkState::Ready(s) => Some(s),
            _ => None,
        }
    }

    fn clear_sink(&mut self) {
        if let Some(s) = self.sink() {
            s.clear();
        }
    }

    fn queued_samples(&mut self) -> u64 {
        self.sink()
            .map_or(0, |s| samples_for_ms(f64::from(s.queued_ms())) as u64)
    }

    /// Push `samples`; returns how many the sink took.
    fn push(&mut self, samples: &[f32]) -> usize {
        match &mut self.sink {
            SinkState::Ready(s) if !samples.is_empty() => s.push(samples),
            _ => 0,
        }
    }

    /// One frame: decide the mode, push what it calls for, move the display delay.
    pub fn frame(&mut self, sounds: &mut HeartSounds, input: &PlayerInput) {
        let delay_target = self.frame_inner(sounds, input);
        self.delay.update(delay_target, input.dt_s);
    }

    /// Returns the display-delay target, ms.
    fn frame_inner(&mut self, sounds: &mut HeartSounds, input: &PlayerInput) -> f64 {
        let reanchor = self.generation != Some(sounds.generation()) || input.sim_now_ms < self.last_sim_ms;
        self.generation = Some(sounds.generation());
        self.last_sim_ms = input.sim_now_ms;
        if !input.enabled && !self.calibrate {
            if self.mode != Mode::Off {
                self.clear_sink();
                self.mode = Mode::Off;
            }
            sounds.set_wall_active(false);
            self.slice = None;
            self.status = HeartSoundStatus::Off;
            return 0.0;
        }
        if matches!(self.sink, SinkState::NotTried) {
            self.open_sink();
        }
        let readiness = match &self.sink {
            SinkState::Ready(s) => s.readiness(),
            _ => SinkReadiness::Unavailable,
        };
        if readiness != SinkReadiness::Running {
            // a browser waiting for a click is a normal state: nothing is pushed, nothing delayed,
            // and the stream re-anchors once it runs
            if self.mode != Mode::Off {
                self.clear_sink();
                self.mode = Mode::Off;
            }
            sounds.set_wall_active(false);
            self.status = if readiness == SinkReadiness::WaitingForGesture {
                HeartSoundStatus::WaitingForClick
            } else if matches!(self.sink, SinkState::Ready(_)) {
                HeartSoundStatus::Unsupported
            } else {
                self.unavailable
            };
            return 0.0;
        }
        let volume = if input.volume.is_nan() {
            0.0
        } else {
            input.volume.clamp(0.0, 1.0)
        };
        if let Some(s) = self.sink() {
            s.set_gain(HEADROOM_GAIN * volume);
        }
        sounds.set_wall_active(true);
        if let Some((start, end)) = self.slice_request.take()
            && input.paused
        {
            let oldest = sounds.ring().oldest_index();
            let newest = sounds.ring().pushed();
            let (start, end) = (start.clamp(oldest, newest), end.clamp(oldest, newest));
            if end > start {
                self.slice = Some(Slice {
                    start,
                    end,
                    next: start,
                    pushed: 0,
                    tail: 0.0,
                    playhead: start,
                    done: false,
                });
                // a new slice always starts from silence
                self.mode = Mode::Paused;
            }
        }
        let want = if self.calibrate {
            Mode::Calibrate
        } else if input.paused {
            if self.slice.is_some() {
                Mode::Slice
            } else {
                Mode::Paused
            }
        } else if input.muted {
            Mode::Paused
        } else if input.speed > 1.0 + REAL_TIME_TOLERANCE {
            Mode::Fast
        } else if input.speed < 1.0 - REAL_TIME_TOLERANCE {
            Mode::Slow
        } else {
            Mode::Live
        };
        if !input.paused {
            self.slice = None;
        }
        if want != self.mode || reanchor {
            // live → slow continues seamlessly: the wall-time synth picks up where the ring stops
            let seamless = self.mode == Mode::Live && want == Mode::Slow && !reanchor;
            if !seamless {
                self.clear_sink();
            }
            self.next = sounds.ring().pushed();
            self.started = false;
            if want == Mode::Calibrate {
                self.calib_pos = 0;
                self.calib_pushed = 0;
            }
            self.mode = want;
        }
        let target_ms = f64::from(self.sink_latency_ms()) + f64::from(input.offset_ms.max(0.0));
        match self.mode {
            Mode::Live => {
                self.live(sounds);
                if let Some(w) = sounds.wall_mut() {
                    w.follow(input.sim_now_ms);
                }
                self.status = HeartSoundStatus::Playing;
                target_ms
            }
            Mode::Slow => {
                self.slow(sounds, input);
                self.status = HeartSoundStatus::SlowMotion;
                target_ms * input.speed
            }
            Mode::Fast => {
                if let Some(w) = sounds.wall_mut() {
                    w.follow(input.sim_now_ms);
                }
                self.status = HeartSoundStatus::MutedFast;
                0.0
            }
            Mode::Paused | Mode::Off => {
                self.status = if input.muted && !input.paused {
                    HeartSoundStatus::MutedReview
                } else {
                    HeartSoundStatus::Paused
                };
                0.0
            }
            Mode::Slice => {
                self.slice_frame(sounds, input);
                self.status = HeartSoundStatus::PlayingSlice;
                0.0
            }
            Mode::Calibrate => {
                self.calibrate_frame(input);
                self.status = HeartSoundStatus::Calibrating;
                0.0
            }
        }
    }

    /// The sink's constant delay, ms (never below the queue target).
    fn sink_latency_ms(&self) -> f32 {
        match &self.sink {
            SinkState::Ready(s) => s.latency_ms().max(QUEUE_TARGET_MS),
            _ => QUEUE_TARGET_MS,
        }
    }

    /// The part of the sink's delay beyond what [`PcmSink::queued_ms`] counts, samples: zero for
    /// kira, the look-ahead and the browser's latencies on the web. The playhead and the sync-check
    /// marker subtract it from what has been handed over.
    fn beyond_queue_samples(&self) -> u64 {
        samples_for_ms(f64::from(self.sink_latency_ms() - QUEUE_TARGET_MS)) as u64
    }

    fn target_samples() -> u64 {
        samples_for_ms(f64::from(QUEUE_TARGET_MS)) as u64
    }

    fn backlog_samples() -> u64 {
        samples_for_ms(f64::from(QUEUE_TARGET_MS * BACKLOG_FACTOR)) as u64
    }

    fn live(&mut self, sounds: &HeartSounds) {
        let ring = sounds.ring();
        let pushed = ring.pushed();
        if self.next > pushed || self.next < ring.oldest_index() {
            self.next = pushed;
        }
        let new = pushed - self.next;
        if new == 0 {
            return;
        }
        let q = self.queued_samples();
        let target = Self::target_samples();
        let mut from = self.next;
        if q + new > Self::backlog_samples() {
            // a stalled frame: drop the old audio and play only the newest target's worth
            if q > 0 {
                self.clear_sink();
            }
            from = pushed - new.min(target);
        } else if q == 0 && new > target {
            // the device ran dry, or the stream starts after a long frame (B26): (re-)anchor at the
            // newest target's worth, never replay, so the delay is the constant one
            from = pushed - target;
        }
        let mut buf = std::mem::take(&mut self.scratch);
        let first = ring.range(from, pushed, &mut buf);
        let took = self.push(&buf) as u64;
        self.scratch = buf;
        if let Some(r) = &mut self.ranges {
            r.push((first, first + took));
        }
        self.next = pushed;
        self.started = true;
    }

    fn slow(&mut self, sounds: &mut HeartSounds, input: &PlayerInput) {
        let Some(wall) = sounds.wall_mut() else { return };
        // a synthesiser that has fallen far behind (just switched on) jumps to now first
        if input.sim_now_ms - wall.sim_at_wall(wall.wall_now_ms()) > 500.0 {
            wall.follow(input.sim_now_ms);
        }
        wall.set_speed(input.speed);
        let mut buf = std::mem::take(&mut self.scratch);
        buf.clear();
        wall.render_until_sim(input.sim_now_ms, &mut buf);
        let q = self.queued_samples();
        let n = buf.len() as u64;
        let mut from = 0;
        if q + n > Self::backlog_samples() {
            if q > 0 {
                self.clear_sink();
            }
            from = n.saturating_sub(Self::target_samples()) as usize;
        }
        self.push(&buf[from..]);
        self.scratch = buf;
    }

    fn slice_frame(&mut self, sounds: &HeartSounds, input: &PlayerInput) {
        let q = self.queued_samples();
        let Some(mut s) = self.slice else { return };
        if s.done {
            self.slice = None;
            self.mode = Mode::Paused;
            return;
        }
        let ring = sounds.ring();
        // keep about two targets queued: the playhead stays smooth and Stop stays quick
        let want = (2 * Self::target_samples()).saturating_sub(q);
        let until = s.end.min(s.next + want);
        if until > s.next {
            let mut buf = std::mem::take(&mut self.scratch);
            let first = ring.range(s.next, until, &mut buf);
            let took = self.push(&buf) as u64;
            self.scratch = buf;
            s.next = first + took;
            s.pushed += took;
        }
        let consumed = s.pushed.saturating_sub(q);
        if s.next >= s.end && q == 0 {
            s.tail += input.dt_s.max(0.0) * f64::from(PCG_RATE_HZ);
        }
        let offset =
            (samples_for_ms(f64::from(input.offset_ms.max(0.0))) as u64 + self.beyond_queue_samples()) as f64;
        let pos = (s.start + consumed) as f64 + s.tail - offset;
        let head = (pos.max(s.start as f64) as u64).clamp(s.playhead, s.end);
        s.playhead = head;
        s.done = head >= s.end;
        self.slice = Some(s);
    }

    fn calibrate_frame(&mut self, input: &PlayerInput) {
        let q = self.queued_samples();
        let frame = (input.dt_s.max(0.0) * f64::from(PCG_RATE_HZ)).round() as u64;
        let n = (Self::target_samples() + frame).saturating_sub(q);
        let period = samples_for_ms(SYNC_CLICK_PERIOD_MS) as u64;
        let click = sync_click();
        let mut buf = std::mem::take(&mut self.scratch);
        buf.clear();
        buf.extend((0..n).map(|i| {
            let k = ((self.calib_pos + i) % period) as usize;
            click.get(k).copied().unwrap_or(0.0)
        }));
        let took = self.push(&buf) as u64;
        self.scratch = buf;
        self.calib_pos += took;
        self.calib_pushed += took;
        let offset = samples_for_ms(f64::from(input.offset_ms.max(0.0))) as u64 + self.beyond_queue_samples();
        let q = self.queued_samples();
        // what the listener hears now, if the offset equals the device's latency
        self.flash = self
            .calib_pushed
            .checked_sub(q + offset)
            .is_some_and(|pos| ms_for_samples((pos % period) as usize) < SYNC_FLASH_MS as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_out::FakeSink;

    #[test]
    fn the_delay_ramps_to_its_target_and_back() {
        let mut d = DisplayDelay::default();
        let mut last = 0.0;
        let mut steps = 0;
        while d.value_ms() < 100.0 - 1e-9 {
            let v = d.update(100.0, 1.0 / 60.0);
            assert!(v >= last && v <= 100.0, "monotone ramp: {last} → {v}");
            last = v;
            steps += 1;
        }
        assert!(
            (17..=19).contains(&steps),
            "about 0.3 s at 60 fps: {steps} frames"
        );
        // the first frames move by little: no visible jump
        let mut d2 = DisplayDelay::default();
        assert!(d2.update(100.0, 1.0 / 60.0) < 5.0);
        for _ in 0..30 {
            d.update(0.0, 1.0 / 60.0);
        }
        assert!(d.value_ms().abs() < 1e-9);
    }

    #[test]
    fn the_build_flag_follows_the_feature() {
        assert_eq!(AUDIO_BUILD, cfg!(feature = "audio"));
        let p = HeartSoundPlayer::new();
        assert_eq!(p.available(), None, "nothing opened at start");
        let _ = FakeSink::new();
    }
}
