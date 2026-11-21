//! Heart-sound audio, the device-independent half (brief B25).
//!
//! The phonocardiogram the strip draws (B24, 4 kHz, mono) is played by handing its samples to a
//! [`PcmSink`]. Everything here is pure, always compiled (also without the `audio` feature and on
//! `wasm32`) and tested without a sound card:
//!
//! * [`PcmSink`], the write-only interface the player talks to, and [`FakeSink`] for tests;
//! * [`Resampler`], a band-limited (Kaiser-windowed sinc, polyphase table) conversion from the PCG
//!   rate to the device rate: plain linear interpolation would leave images of every sound above
//!   2 kHz, heard as hiss;
//! * [`StreamCore`], what the audio thread runs per output sample: prime the queue, resample,
//!   absorb the drift between the simulation's clock and the sound card's by nudging the resampling
//!   ratio, fade in after an underrun and out on a clear, apply the gain and the soft limiter. It
//!   reads its input through [`PcmSource`]; `audio_out` implements that over a lock-free queue.
//!
//! Nothing here reads a clock or feeds anything back into the simulation: the sink only receives.
//! Every constant is marked **designed** (a choice for this program) or carries its source.

use std::cell::{Ref, RefCell};
use std::rc::Rc;

use heartsim_ecg::PCG_RATE_HZ;

/// Audio the player keeps queued ahead of the sound card, ms. Designed: two to three 60 fps frames,
/// so a frame that is late by one frame does not run the queue dry (research note, "Why
/// synchronisation needs a fixed delay": about 40 ms on desktop).
pub const QUEUE_TARGET_MS: f32 = 40.0;
/// A queue longer than this many times [`QUEUE_TARGET_MS`] is a backlog (a stalled frame, a debugger
/// stop): the oldest audio is dropped and playback re-anchors. Designed.
pub const BACKLOG_FACTOR: f32 = 3.0;
/// Fade-in after an underrun and fade-out on a clear, ms. Designed: long enough not to click, short
/// enough not to swallow a sound.
pub const FADE_MS: f32 = 10.0;
/// Default of the user's audio offset (the sound card's own latency, which kira does not report),
/// ms. Designed (maintainer's decision for B25): typical of a desktop mixer's buffer. In the browser
/// (B26) the default is 0: Web Audio reports its own output latency and the sink adds it itself
/// ([`PcmSink::latency_ms`]), so a desktop-sized default there would draw the views later than the
/// sound is heard, which "never early" forbids.
pub const DEFAULT_AUDIO_OFFSET_MS: f32 = if cfg!(target_arch = "wasm32") { 0.0 } else { 60.0 };
/// Range of the audio-offset slider, ms. Designed: 0 for a low-latency interface, up to 300 ms for
/// Bluetooth headphones (100–300 ms, research note §8).
pub const AUDIO_OFFSET_RANGE_MS: std::ops::RangeInclusive<f32> = 0.0..=300.0;
/// Default heart-sound volume, linear `0.0..=1.0`. Designed.
pub const DEFAULT_HEART_SOUND_VOLUME: f32 = 0.7;
/// Scale from PCG units (a normal S1 peaks at 1.0, a loud S1 at about 1.5) to full scale at full
/// volume. Designed: a normal S1 at half scale leaves the loudest beat of complete AV block below the
/// limiter's knee, so the limiter only catches what the model has never produced.
pub const HEADROOM_GAIN: f32 = 0.5;
/// Output level from which the soft limiter bends the signal, full scale. Designed.
pub const LIMITER_KNEE: f32 = 0.8;
/// Passband edge of the resampling filter, Hz. Designed: heart sounds have little energy above
/// 500 Hz (research note); 1.6 kHz leaves the filter room to reach its stopband before the PCG's
/// 2 kHz Nyquist frequency, where the images begin.
pub const RESAMPLER_CUTOFF_HZ: f64 = 1600.0;
/// Taps of the resampling filter, input samples (4 ms at 4 kHz; delay half of it). Designed.
pub const RESAMPLER_TAPS: usize = 16;
/// Fractional positions tabulated in the polyphase table (linearly interpolated between). Designed.
pub const RESAMPLER_PHASES: usize = 256;
/// Kaiser window shape of the resampling filter. Designed: β = 6 gives about 60 dB of stopband
/// attenuation (Kaiser's formula, `A ≈ β / 0.1102 + 8.7`).
pub const KAISER_BETA: f64 = 6.0;
/// Resampling-ratio change per unit of relative queue error (queue / target − 1). Designed.
pub const DRIFT_GAIN: f64 = 0.01;
/// Largest resampling-ratio change for drift, fraction. Designed: ±0.5 % is below the pitch
/// difference a listener notices in a heart sound, and far more than two quartz clocks drift.
pub const MAX_DRIFT: f64 = 0.005;
/// Time constant of the queue level the drift correction reads, s. Designed: longer than the
/// frame-to-frame sawtooth of the queue.
pub const DRIFT_SMOOTHING_S: f64 = 0.5;
/// Time constant of gain changes on the audio thread, s. Designed: a volume drag does not zipper.
pub const GAIN_SMOOTHING_S: f64 = 0.02;

/// Length of the sync-check click, ms. Designed.
pub const CLICK_MS: f64 = 6.0;
/// Pitch of the sync-check click, Hz. Designed: well inside the PCG band (below the resampler's
/// passband edge) and easy to hear on any speaker.
pub const CLICK_HZ: f64 = 1000.0;
/// Peak of the click in PCG units. Designed: at [`HEADROOM_GAIN`] it plays at 0.75 of full scale.
pub const CLICK_PEAK: f32 = 1.5;

/// PCG samples in `ms`, rounded.
#[must_use]
pub fn samples_for_ms(ms: f64) -> usize {
    (ms * f64::from(PCG_RATE_HZ) / 1000.0).round().max(0.0) as usize
}

/// Duration of `samples` PCG samples, ms.
#[must_use]
pub fn ms_for_samples(samples: usize) -> f32 {
    samples as f32 * 1000.0 / PCG_RATE_HZ
}

/// Whether a sink can play now (B26). A desktop device is always [`SinkReadiness::Running`] once
/// opened; a browser's audio context waits for the learner's click and can be suspended or
/// interrupted later, which is a normal state, not a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SinkReadiness {
    /// Samples pushed now are heard.
    #[default]
    Running,
    /// The output exists but waits for a click or a key press (browser autoplay policy, a hidden
    /// tab, an interruption). The player pushes nothing meanwhile.
    WaitingForGesture,
    /// The output turned out to be unusable after it was created (the browser refused the audio
    /// context).
    Unavailable,
}

/// Where heart-sound samples go: mono `f32` at the PCG rate, write-only. Nothing the sink knows
/// (queue depth, device clock) may reach the simulation; the player reads [`PcmSink::queued_ms`]
/// only to decide how much to hand over.
pub trait PcmSink {
    /// Queue `samples` for playback; returns how many were accepted (the rest did not fit).
    fn push(&mut self, samples: &[f32]) -> usize;
    /// Audio queued and not yet taken by the device, ms.
    fn queued_ms(&self) -> f32;
    /// Drop what is queued. The sink fades to silence itself ([`FADE_MS`]).
    fn clear(&mut self);
    /// Linear gain applied before the limiter: [`HEADROOM_GAIN`] times the user's volume.
    fn set_gain(&mut self, gain: f32);
    /// The sink's own constant delay from handing over the newest sample to hearing it, ms, as far
    /// as the sink knows it (B26): the [`QUEUE_TARGET_MS`] the player keeps queued plus whatever
    /// the output reports beyond it. The views are drawn this much plus the user's audio offset
    /// behind the engine. The kira sink reports only the queue (its device latency is unknown and is
    /// the user's offset); the Web Audio sink adds its look-ahead and the browser's reported
    /// latencies. Never below [`QUEUE_TARGET_MS`].
    fn latency_ms(&self) -> f32 {
        QUEUE_TARGET_MS
    }
    /// Whether samples pushed now are heard.
    fn readiness(&self) -> SinkReadiness {
        SinkReadiness::Running
    }
}

/// What a [`FakeSink`] has been asked to do.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FakeLog {
    /// Every push, as accepted.
    pub pushes: Vec<Vec<f32>>,
    /// Samples queued now.
    pub queued: usize,
    /// Samples drained by [`FakeSink::drain_ms`] so far.
    pub drained: u64,
    /// Calls to [`PcmSink::clear`].
    pub clears: usize,
    /// Times the queue ran dry while draining.
    pub underruns: usize,
    /// The last gain set.
    pub gain: f32,
    /// Like the real sink, start draining only once this many samples are queued (0: at once).
    pub prime_samples: usize,
    primed: bool,
    /// Most samples the queue holds.
    pub capacity: usize,
    /// What [`PcmSink::latency_ms`] reports; 0 stands for the default, [`QUEUE_TARGET_MS`].
    pub latency_ms: f32,
    /// What [`PcmSink::readiness`] reports.
    pub readiness: SinkReadiness,
}

/// A sink for tests: records pushes and drains when the test says so, at the rate it chooses.
/// Clones share the record, so a test keeps one while the player owns the boxed other.
#[derive(Debug, Clone)]
pub struct FakeSink(Rc<RefCell<FakeLog>>);

impl Default for FakeSink {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeSink {
    /// A fake that drains as soon as anything is queued, with room for 10 s.
    #[must_use]
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(FakeLog {
            capacity: samples_for_ms(10_000.0),
            ..FakeLog::default()
        })))
    }

    /// A fake that, like the real sink, waits after creation, an underrun or a clear until
    /// [`QUEUE_TARGET_MS`] is queued before it drains.
    #[must_use]
    pub fn priming() -> Self {
        let s = Self::new();
        s.0.borrow_mut().prime_samples = samples_for_ms(f64::from(QUEUE_TARGET_MS));
        s
    }

    /// A fake that reports `latency_ms` as its constant delay, like the Web Audio sink (B26).
    #[must_use]
    pub fn with_latency_ms(self, latency_ms: f32) -> Self {
        self.0.borrow_mut().latency_ms = latency_ms;
        self
    }

    /// Make the fake report `readiness` from now on.
    pub fn set_readiness(&self, readiness: SinkReadiness) {
        self.0.borrow_mut().readiness = readiness;
    }

    /// Let the "device" consume `ms` of audio; returns the samples taken.
    pub fn drain_ms(&self, ms: f64) -> usize {
        let mut l = self.0.borrow_mut();
        if !l.primed {
            if l.queued >= l.prime_samples.max(1) {
                l.primed = true;
            } else {
                return 0;
            }
        }
        let want = samples_for_ms(ms);
        let n = want.min(l.queued);
        l.queued -= n;
        l.drained += n as u64;
        if n < want {
            l.underruns += 1;
            l.primed = false;
        }
        n
    }

    /// The record.
    #[must_use]
    pub fn log(&self) -> Ref<'_, FakeLog> {
        self.0.borrow()
    }

    /// Every sample pushed, in order.
    #[must_use]
    pub fn pushed(&self) -> Vec<f32> {
        self.0.borrow().pushes.concat()
    }
}

impl PcmSink for FakeSink {
    fn push(&mut self, samples: &[f32]) -> usize {
        let mut l = self.0.borrow_mut();
        let n = samples.len().min(l.capacity.saturating_sub(l.queued));
        if n > 0 {
            l.pushes.push(samples[..n].to_vec());
            l.queued += n;
        }
        n
    }

    fn queued_ms(&self) -> f32 {
        ms_for_samples(self.0.borrow().queued)
    }

    fn clear(&mut self) {
        let mut l = self.0.borrow_mut();
        l.queued = 0;
        l.clears += 1;
        l.primed = false;
    }

    fn set_gain(&mut self, gain: f32) {
        self.0.borrow_mut().gain = gain;
    }

    fn latency_ms(&self) -> f32 {
        let l = self.0.borrow().latency_ms;
        if l > 0.0 { l } else { QUEUE_TARGET_MS }
    }

    fn readiness(&self) -> SinkReadiness {
        self.0.borrow().readiness
    }
}

/// Soft limiter: the identity up to [`LIMITER_KNEE`], then a `tanh` bend that approaches full
/// scale and never reaches it, so no beat clips however loud the model makes it. Monotone, so a
/// louder beat stays louder.
#[must_use]
pub fn soft_limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= LIMITER_KNEE {
        return x;
    }
    let room = 1.0 - LIMITER_KNEE;
    // `tanh` rounds to 1 in `f32` for large arguments: stay a hair below full scale
    (LIMITER_KNEE + room * ((a - LIMITER_KNEE) / room).tanh())
        .min(1.0 - f32::EPSILON)
        .copysign(x)
}

/// The sync-check click at the PCG rate: a [`CLICK_HZ`] tone of [`CLICK_MS`] under a Hann window,
/// peak [`CLICK_PEAK`].
#[must_use]
pub fn sync_click() -> Vec<f32> {
    let n = samples_for_ms(CLICK_MS);
    let rate = f64::from(PCG_RATE_HZ);
    (0..n)
        .map(|i| {
            let x = i as f64 / (n.max(2) - 1) as f64;
            let w = 0.5 - 0.5 * (std::f64::consts::TAU * x).cos();
            let s = (std::f64::consts::TAU * CLICK_HZ * (i as f64 + 0.5) / rate).sin();
            (f64::from(CLICK_PEAK) * w * s) as f32
        })
        .collect()
}

/// Zeroth-order modified Bessel function of the first kind (the Kaiser window), by its series.
fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > 1e-12 * sum {
        term *= (x / (2.0 * k)).powi(2);
        sum += term;
        k += 1.0;
    }
    sum
}

/// Half the tap count: the filter's centre.
const HALF_TAPS: usize = RESAMPLER_TAPS / 2;

/// Band-limited sample-rate conversion from the PCG rate to a device rate, for a stream: one output
/// sample per [`Resampler::next`] call, pulling input samples as needed.
///
/// A Kaiser-windowed sinc low-pass at [`RESAMPLER_CUTOFF_HZ`] with [`RESAMPLER_TAPS`] taps,
/// tabulated at [`RESAMPLER_PHASES`] fractional positions (each normalised to unit DC gain) and
/// interpolated between them. The table is built once on creation; `next` allocates nothing and
/// takes no lock, so it can run on the audio thread. The output lags the newest input by
/// `HALF_TAPS + 1 − frac` input samples ([`Resampler::input_time`]).
#[derive(Debug, Clone)]
pub struct Resampler {
    table: Box<[[f32; RESAMPLER_TAPS]]>,
    history: [f32; RESAMPLER_TAPS],
    /// Position between the two centre taps, `0.0..1.0`.
    frac: f64,
    /// Input samples per output sample.
    step: f64,
    pulled: u64,
}

impl Resampler {
    /// A converter from `in_rate_hz` to `out_rate_hz`.
    #[must_use]
    pub fn new(in_rate_hz: f64, out_rate_hz: f64) -> Self {
        let fc = 2.0 * RESAMPLER_CUTOFF_HZ / in_rate_hz;
        let i0b = bessel_i0(KAISER_BETA);
        let h = HALF_TAPS as f64;
        let kernel = |d: f64| {
            let r = d / h;
            if r.abs() >= 1.0 {
                return 0.0;
            }
            let x = std::f64::consts::PI * fc * d;
            let sinc = if x.abs() < 1e-12 { 1.0 } else { x.sin() / x };
            fc * sinc * bessel_i0(KAISER_BETA * (1.0 - r * r).sqrt()) / i0b
        };
        let table: Vec<[f32; RESAMPLER_TAPS]> = (0..=RESAMPLER_PHASES)
            .map(|p| {
                let frac = p as f64 / RESAMPLER_PHASES as f64;
                let mut row = [0.0f64; RESAMPLER_TAPS];
                for (j, c) in row.iter_mut().enumerate() {
                    // distance from tap j to the output point, input samples
                    *c = kernel((RESAMPLER_TAPS - 1 - HALF_TAPS) as f64 - j as f64 + frac);
                }
                let sum: f64 = row.iter().sum();
                let mut out = [0.0f32; RESAMPLER_TAPS];
                for (o, c) in out.iter_mut().zip(row) {
                    *o = (c / sum) as f32;
                }
                out
            })
            .collect();
        Self {
            table: table.into_boxed_slice(),
            history: [0.0; RESAMPLER_TAPS],
            frac: 0.0,
            step: in_rate_hz / out_rate_hz.max(1.0),
            pulled: 0,
        }
    }

    /// Input samples per output sample (the nominal ratio times any drift correction).
    pub fn set_step(&mut self, step: f64) {
        self.step = step.max(0.0);
    }

    /// The next output sample; `pull` is called once for every input sample consumed.
    pub fn next(&mut self, mut pull: impl FnMut() -> f32) -> f32 {
        self.frac += self.step;
        while self.frac >= 1.0 {
            self.frac -= 1.0;
            self.history.copy_within(1.., 0);
            self.history[RESAMPLER_TAPS - 1] = pull();
            self.pulled += 1;
        }
        let pos = self.frac * RESAMPLER_PHASES as f64;
        let p = (pos as usize).min(RESAMPLER_PHASES - 1);
        let w = (pos - p as f64) as f32;
        let (a, b) = (&self.table[p], &self.table[p + 1]);
        let mut y = 0.0f32;
        for j in 0..RESAMPLER_TAPS {
            y += self.history[j] * (a[j] + (b[j] - a[j]) * w);
        }
        y
    }

    /// How many input samples the next [`Resampler::next`] call will pull (a caller converting a
    /// finite block stops before it runs out).
    #[must_use]
    pub fn needs(&self) -> usize {
        (self.frac + self.step).floor() as usize
    }

    /// Time of the last output sample in input samples, counting the first input pulled as 0.
    #[must_use]
    pub fn input_time(&self) -> f64 {
        self.pulled as f64 - 1.0 - HALF_TAPS as f64 + self.frac
    }

    /// Forget the input history (silence) and the position.
    pub fn reset(&mut self) {
        self.history = [0.0; RESAMPLER_TAPS];
        self.frac = 0.0;
    }
}

/// The audio thread's view of the queue of PCG samples. Implementations must not lock, allocate or
/// block: `audio_out` implements it over an `rtrb` single-producer single-consumer ring.
pub trait PcmSource {
    /// Samples ready to play (not counting those a pending clear will drop).
    fn ready(&self) -> usize;
    /// The next sample, `None` when the queue is empty (underrun).
    fn pop(&mut self) -> Option<f32>;
    /// The player asked for a clear that is not finished yet.
    fn clear_pending(&self) -> bool;
    /// During the fade-out of a clear: the next sample queued before the clear, if any.
    fn pop_cleared(&mut self) -> Option<f32>;
    /// Drop the rest of what was queued before the clear.
    fn finish_clear(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamState {
    /// Silence until enough is queued (after start, an underrun or a clear).
    Priming {
        waited: usize,
    },
    Playing,
    /// Fading out the audio queued before a clear.
    FadingOut,
}

/// What the device callback runs for the heart sounds, one output sample at a time:
///
/// 1. **Prime.** After start, an underrun or a clear, output silence until [`QUEUE_TARGET_MS`] is
///    queued (or until that long has passed with something queued, so a short slice still plays).
///    Waiting for a full queue is what makes the delay behind the simulation constant.
/// 2. **Play** with a [`FADE_MS`] raised-cosine fade-in, resampling to the device rate. On an
///    underrun the filter rings down on silence and priming starts again: nothing old is repeated.
/// 3. **Drift.** The queue level, smoothed over [`DRIFT_SMOOTHING_S`], nudges the resampling ratio
///    by at most [`MAX_DRIFT`] so that the sound card neither drains nor floods a queue the
///    simulation fills at its own pace. The simulation is never told.
/// 4. **Clear.** Fade out over at most [`FADE_MS`] on what was queued before the clear, drop the
///    rest, prime again.
/// 5. Gain (smoothed), then [`soft_limit`].
#[derive(Debug, Clone)]
pub struct StreamCore {
    resampler: Resampler,
    state: StreamState,
    base_step: f64,
    fade_len: usize,
    fade_pos: usize,
    prime_samples: usize,
    prime_timeout: usize,
    target_samples: f64,
    level: f64,
    level_alpha: f64,
    gain: f32,
    gain_target: f32,
    gain_alpha: f32,
}

impl StreamCore {
    /// A stream to a device running at `out_rate_hz`. Allocates the resampler's table, so it is
    /// built off the audio thread; [`StreamCore::set_out_rate`] adapts it to another rate without
    /// allocating.
    #[must_use]
    pub fn new(out_rate_hz: f64) -> Self {
        let in_rate = f64::from(PCG_RATE_HZ);
        let target = samples_for_ms(f64::from(QUEUE_TARGET_MS));
        let mut core = Self {
            // the table depends only on the input rate; the output rate sets the step
            resampler: Resampler::new(in_rate, out_rate_hz.max(1.0)),
            state: StreamState::Priming { waited: 0 },
            base_step: 1.0,
            fade_len: 1,
            fade_pos: 0,
            prime_samples: target,
            prime_timeout: 0,
            target_samples: target as f64,
            level: target as f64,
            level_alpha: 0.0,
            gain: 0.0,
            gain_target: 0.0,
            gain_alpha: 0.0,
        };
        core.set_out_rate(out_rate_hz);
        core
    }

    /// The device runs at `out_rate_hz` (it reports its rate on every callback and may change it).
    /// No allocation.
    pub fn set_out_rate(&mut self, out_rate_hz: f64) {
        let out = out_rate_hz.max(1.0);
        let in_rate = f64::from(PCG_RATE_HZ);
        self.base_step = in_rate / out;
        self.resampler.set_step(self.base_step);
        self.fade_len = ((f64::from(FADE_MS) * out / 1000.0).round() as usize).max(1);
        self.fade_pos = self.fade_pos.min(self.fade_len);
        self.prime_timeout = (f64::from(QUEUE_TARGET_MS) * out / 1000.0).round() as usize;
        self.level_alpha = 1.0 / (DRIFT_SMOOTHING_S * out);
        self.gain_alpha = (1.0 / (GAIN_SMOOTHING_S * out)) as f32;
    }

    /// Target linear gain before the limiter (reached within [`GAIN_SMOOTHING_S`]).
    pub fn set_gain(&mut self, gain: f32) {
        self.gain_target = if gain.is_finite() { gain.max(0.0) } else { 0.0 };
    }

    /// Set the gain at once (tests).
    pub fn set_gain_now(&mut self, gain: f32) {
        self.set_gain(gain);
        self.gain = self.gain_target;
    }

    /// Whether the stream is playing (not priming or fading out).
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.state == StreamState::Playing
    }

    fn fade_gain(&self) -> f32 {
        let x = self.fade_pos as f32 / self.fade_len as f32;
        0.5 - 0.5 * (std::f32::consts::PI * x).cos()
    }

    /// The next output sample, full scale `−1.0..=1.0`.
    pub fn next_sample(&mut self, src: &mut impl PcmSource) -> f32 {
        if self.state != StreamState::FadingOut && src.clear_pending() {
            self.state = StreamState::FadingOut;
        }
        let y = match self.state {
            StreamState::Priming { waited } => {
                let ready = src.ready();
                if ready >= self.prime_samples || (ready > 0 && waited >= self.prime_timeout) {
                    self.state = StreamState::Playing;
                    self.fade_pos = 0;
                    self.level = self.target_samples;
                } else {
                    self.state = StreamState::Priming { waited: waited + 1 };
                }
                self.resampler.next(|| 0.0)
            }
            StreamState::Playing => {
                self.level += self.level_alpha * (src.ready() as f64 - self.level);
                let err = self.level / self.target_samples - 1.0;
                let drift = (DRIFT_GAIN * err).clamp(-MAX_DRIFT, MAX_DRIFT);
                self.resampler.set_step(self.base_step * (1.0 + drift));
                let mut underrun = false;
                let y = self.resampler.next(|| {
                    src.pop().unwrap_or_else(|| {
                        underrun = true;
                        0.0
                    })
                });
                self.fade_pos = (self.fade_pos + 1).min(self.fade_len);
                if underrun {
                    self.state = StreamState::Priming { waited: 0 };
                }
                y
            }
            StreamState::FadingOut => {
                let y = self.resampler.next(|| src.pop_cleared().unwrap_or(0.0));
                if self.fade_pos == 0 {
                    src.finish_clear();
                    self.resampler.reset();
                    self.state = StreamState::Priming { waited: 0 };
                } else {
                    self.fade_pos -= 1;
                }
                y
            }
        };
        self.gain += self.gain_alpha * (self.gain_target - self.gain);
        soft_limit(y * self.fade_gain() * self.gain)
    }
}

/// A [`PcmSource`] over a plain queue, for tests of [`StreamCore`] (single-threaded).
#[derive(Debug, Clone, Default)]
pub struct VecSource {
    queue: std::collections::VecDeque<f32>,
    /// Samples at the front that a pending clear will drop.
    cleared: usize,
}

impl VecSource {
    /// Queue `samples`.
    pub fn push(&mut self, samples: &[f32]) {
        self.queue.extend(samples);
    }

    /// Ask for a clear of everything queued now.
    pub fn clear(&mut self) {
        self.cleared = self.queue.len();
    }

    /// Samples queued, cleared ones included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Nothing queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl PcmSource for VecSource {
    fn ready(&self) -> usize {
        self.queue.len() - self.cleared
    }

    fn pop(&mut self) -> Option<f32> {
        if self.cleared > 0 {
            return None;
        }
        self.queue.pop_front()
    }

    fn clear_pending(&self) -> bool {
        self.cleared > 0
    }

    fn pop_cleared(&mut self) -> Option<f32> {
        if self.cleared == 0 {
            return None;
        }
        self.cleared -= 1;
        self.queue.pop_front()
    }

    fn finish_clear(&mut self) {
        let n = self.cleared.min(self.queue.len());
        self.queue.drain(..n);
        self.cleared = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IN: f64 = 4000.0;

    fn sine(f: f64, k: f64) -> f64 {
        (std::f64::consts::TAU * f * k / IN).sin()
    }

    /// `n` outputs of a 200 Hz input sine at `out` Hz, with each output's input time.
    fn resample_sine(out: f64, n: usize) -> Vec<(f64, f32)> {
        let mut r = Resampler::new(IN, out);
        let mut k = 0u64;
        (0..n)
            .map(|_| {
                let y = r.next(|| {
                    let v = sine(200.0, k as f64) as f32;
                    k += 1;
                    v
                });
                (r.input_time(), y)
            })
            .collect()
    }

    #[test]
    fn a_200_hz_sine_resamples_to_the_ideal_sine_within_one_percent() {
        for out in [44_100.0, 48_000.0] {
            let ys = resample_sine(out, 9000);
            // past the filter's start-up (the history starts silent)
            let worst = ys
                .iter()
                .skip(1000)
                .map(|(t, y)| (f64::from(*y) - sine(200.0, *t)).abs())
                .fold(0.0, f64::max);
            assert!(worst < 0.01, "{out} Hz: error {worst}");
        }
    }

    /// Power of `x` at frequency `f` (Hz) at rate `fs`, Hann-windowed DFT bin.
    fn power(x: &[f32], f: f64, fs: f64) -> f64 {
        let n = x.len() as f64;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n).cos();
            let ph = std::f64::consts::TAU * f * i as f64 / fs;
            re += w * f64::from(*v) * ph.cos();
            im += w * f64::from(*v) * ph.sin();
        }
        re * re + im * im
    }

    #[test]
    fn images_above_2500_hz_are_40_db_below_the_tone() {
        for out in [44_100.0, 48_000.0] {
            let ys: Vec<f32> = resample_sine(out, 6000)
                .into_iter()
                .skip(1000)
                .map(|(_, y)| y)
                .collect();
            // 0.1 s window: 10 Hz bins
            let x = &ys[..(out / 10.0) as usize];
            let tone = power(x, 200.0, out);
            let mut image = 0.0;
            let mut f = 2500.0;
            while f < out / 2.0 {
                image += power(x, f, out);
                f += 10.0;
            }
            let db = 10.0 * (image / tone).log10();
            assert!(db < -40.0, "{out} Hz: images at {db:.1} dB");
        }
    }

    #[test]
    fn the_resampler_runs_at_the_ratio_of_the_rates() {
        let mut r = Resampler::new(IN, 48_000.0);
        let mut pulled = 0;
        for _ in 0..48_000 {
            r.next(|| {
                pulled += 1;
                0.0
            });
        }
        assert!((3999..=4001).contains(&pulled), "{pulled}");
    }

    fn run(core: &mut StreamCore, src: &mut VecSource, n: usize) -> Vec<f32> {
        (0..n).map(|_| core.next_sample(src)).collect()
    }

    fn tone(n: usize, start: usize) -> Vec<f32> {
        (start..start + n)
            .map(|k| sine(200.0, k as f64 + 5.0) as f32)
            .collect()
    }

    #[test]
    fn the_stream_waits_for_the_target_then_fades_in() {
        let out = 48_000.0;
        let mut core = StreamCore::new(out);
        core.set_gain_now(1.0);
        let mut src = VecSource::default();
        src.push(&tone(100, 0));
        // less than the target and less than the timeout: silence
        let quiet = run(&mut core, &mut src, 1000);
        assert!(quiet.iter().all(|v| *v == 0.0));
        assert_eq!(src.len(), 100, "nothing taken while priming");
        src.push(&tone(400, 100));
        let y = run(&mut core, &mut src, 4000);
        assert!(core.is_playing());
        assert!(y.iter().any(|v| v.abs() > 0.5));
    }

    #[test]
    fn an_underrun_then_data_has_no_step_larger_than_the_fade_allows() {
        let out = 44_100.0;
        let mut core = StreamCore::new(out);
        core.set_gain_now(1.0);
        let mut src = VecSource::default();
        src.push(&tone(400, 0));
        let mut y = run(&mut core, &mut src, 20_000);
        assert!(!core.is_playing(), "ran dry");
        // data again: a full-scale tone that starts at its peak
        let peak: Vec<f32> = (0..2000)
            .map(|k| (std::f64::consts::TAU * 200.0 * f64::from(k) / IN).cos() as f32)
            .collect();
        src.push(&peak);
        // the end of the first tone was not announced and cannot be faded; what is checked is the
        // return of data after the underrun
        let resume = y.len() - 1;
        y.extend(run(&mut core, &mut src, 20_000));
        let y = &y[resume..];
        let fade_samples = f64::from(FADE_MS) * out / 1000.0;
        // a raised-cosine fade of a unit signal steps by at most π / (2 N), the tone itself by
        // 2π f / fs; a hard start would step by about 1
        let bound = std::f64::consts::PI / (2.0 * fade_samples) + std::f64::consts::TAU * 200.0 / out;
        let worst = y
            .windows(2)
            .map(|w| f64::from((w[1] - w[0]).abs()))
            .fold(0.0, f64::max);
        assert!(worst <= 1.2 * bound, "step {worst} > {bound}");
    }

    #[test]
    fn a_clear_fades_out_and_drops_the_old_audio() {
        let out = 48_000.0;
        let mut core = StreamCore::new(out);
        core.set_gain_now(1.0);
        let mut src = VecSource::default();
        src.push(&tone(4000, 0));
        let _ = run(&mut core, &mut src, 2400);
        assert!(core.is_playing());
        src.clear();
        let y = run(&mut core, &mut src, 4800);
        // within the fade: the output decays; then silence and the old queue is gone
        let fade = (f64::from(FADE_MS) * out / 1000.0) as usize;
        assert!(y[fade + 1000..].iter().all(|v| v.abs() < 1e-3));
        assert!(src.is_empty(), "the cleared audio was dropped");
        let steps = y.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
        assert!(steps < 0.05, "no click on the clear ({steps})");
    }

    #[test]
    fn the_drift_correction_holds_the_queue_without_touching_the_input() {
        // the producer delivers 0.3 % faster than the device consumes: the stream speeds up a
        // little and the queue stays near the target instead of growing without bound
        let out = 48_000.0;
        let mut core = StreamCore::new(out);
        core.set_gain_now(1.0);
        let mut src = VecSource::default();
        let mut fed = 0usize;
        let per_block = 4.0 * 1.003; // input samples per ms
        let mut acc = 0.0f64;
        for _ms in 0..120_000 {
            acc += per_block;
            let n = acc.floor() as usize;
            acc -= n as f64;
            src.push(&vec![0.0; n]);
            fed += n;
            let _ = run(&mut core, &mut src, 48);
        }
        let queued_ms = src.len() as f64 / 4.0;
        assert!(fed > 400_000);
        assert!(
            queued_ms < 2.0 * f64::from(QUEUE_TARGET_MS),
            "queue {queued_ms} ms after 2 minutes"
        );
    }

    #[test]
    fn the_limiter_never_reaches_full_scale_and_keeps_the_order() {
        assert!((soft_limit(0.5) - 0.5).abs() < f32::EPSILON);
        assert!((soft_limit(-0.7) + 0.7).abs() < f32::EPSILON);
        let mut last = 0.0;
        for i in 0..1000 {
            let x = i as f32 * 0.01;
            let y = soft_limit(x);
            assert!(y < 1.0 && y >= last, "{x} → {y}");
            assert!((soft_limit(-x) + y).abs() < 1e-6);
            last = y;
        }
    }

    #[test]
    fn the_fake_sink_primes_drains_and_counts_underruns() {
        let mut s = FakeSink::priming();
        assert_eq!(s.push(&[0.0; 100]), 100);
        assert_eq!(s.drain_ms(10.0), 0, "waits for the target");
        s.push(&[0.0; 100]);
        assert_eq!(s.drain_ms(10.0), 40);
        assert!((s.queued_ms() - 40.0).abs() < 1e-3);
        assert_eq!(s.drain_ms(100.0), 160);
        assert_eq!(s.log().underruns, 1);
        s.clear();
        assert_eq!(s.log().clears, 1);
        s.set_gain(0.25);
        assert!((s.log().gain - 0.25).abs() < f32::EPSILON);
        assert_eq!(s.pushed().len(), 200);
    }

    #[test]
    fn the_click_is_short_band_limited_and_starts_and_ends_quiet() {
        let c = sync_click();
        assert_eq!(c.len(), samples_for_ms(CLICK_MS));
        assert!(c[0].abs() < 1e-3 && c[c.len() - 1].abs() < 1e-3);
        let peak = c.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.5 * CLICK_PEAK && peak <= CLICK_PEAK);
    }
}
