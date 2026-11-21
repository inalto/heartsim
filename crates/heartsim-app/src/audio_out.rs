//! The `kira` audio output (feature `audio`, desktop only): the monitor beep and the heart sounds.
//!
//! The only place in the app that talks to an audio device. One [`AudioOutput`] owns the device
//! stream (a kira `AudioManager`); it is opened the first time the beep or the heart sounds are
//! switched on and hands out both sinks, so the two share one stream. Nothing is loaded from disk:
//! the beep comes from [`crate::monitor_beep::synth_beep`], the heart sounds from the PCG ring.
//!
//! The heart sounds are a custom kira [`Sound`] fed through an `rtrb` lock-free single-producer
//! single-consumer queue. On the audio thread [`HeartSoundStream::process`] only pops from that
//! queue and runs [`StreamCore`] (resample, fade, gain, limiter): no lock, no allocation, no
//! logging, no blocking; an empty queue plays silence. Nothing flows back to the simulation: the
//! UI thread reads the queue depth (atomics) only to decide how much to push.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use kira::info::Info;
use kira::sound::static_sound::StaticSoundData;
use kira::sound::{Sound, SoundData};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Frame};

use crate::monitor_beep::{BeepSink, synth_beep};
use crate::sound_out::{PcmSink, PcmSource, StreamCore, ms_for_samples, samples_for_ms};

/// Sample rate of the synthesised beep, Hz. `kira` does not report the device's rate; a static sound is
/// resampled to the output rate on playback, so the tone keeps its pitch.
const BEEP_SAMPLE_RATE_HZ: u32 = 44_100;
/// Capacity of the heart-sound queue, ms of PCG. Designed: far above what the player keeps queued
/// (at most three times the target plus a frame), so a push never fails in normal use.
const QUEUE_CAPACITY_MS: f64 = 2000.0;
/// Device rate assumed when the stream is built (the first callback corrects it). Designed.
const NOMINAL_DEVICE_RATE_HZ: f64 = 48_000.0;

type Manager = Rc<RefCell<AudioManager<DefaultBackend>>>;

enum OutputState {
    NotTried,
    Ready(Manager),
    Failed(String),
}

thread_local! {
    /// The app's one device stream, opened on first use from the UI thread and kept for the run.
    static OUTPUT: RefCell<OutputState> = const { RefCell::new(OutputState::NotTried) };
}

/// The shared audio output: a handle on the one device stream.
#[derive(Clone)]
pub struct AudioOutput {
    manager: Manager,
}

impl std::fmt::Debug for AudioOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioOutput").finish_non_exhaustive()
    }
}

impl AudioOutput {
    /// The device stream, opened the first time this is called; later calls share it. If opening
    /// fails the error is kept and returned again without retrying.
    ///
    /// # Errors
    /// The device error as text when there is no output device or the stream cannot be started (headless
    /// machines, CI, a sound server that is not running).
    pub fn shared() -> Result<Self, String> {
        OUTPUT.with(|cell| {
            let mut state = cell.borrow_mut();
            match &*state {
                OutputState::Ready(m) => Ok(Self {
                    manager: Rc::clone(m),
                }),
                OutputState::Failed(e) => Err(e.clone()),
                OutputState::NotTried => {
                    match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
                        Ok(m) => {
                            let m = Rc::new(RefCell::new(m));
                            *state = OutputState::Ready(Rc::clone(&m));
                            Ok(Self { manager: m })
                        }
                        Err(e) => {
                            let e = e.to_string();
                            *state = OutputState::Failed(e.clone());
                            Err(e)
                        }
                    }
                }
            }
        })
    }

    /// The monitor beep on this output.
    #[must_use]
    pub fn beep_sink(&self) -> KiraBeepSink {
        let frames: Vec<Frame> = synth_beep(BEEP_SAMPLE_RATE_HZ)
            .into_iter()
            .map(Frame::from_mono)
            .collect();
        let sound = StaticSoundData {
            sample_rate: BEEP_SAMPLE_RATE_HZ,
            frames: frames.into(),
            settings: kira::sound::static_sound::StaticSoundSettings::default(),
            slice: None,
        };
        KiraBeepSink {
            manager: Rc::clone(&self.manager),
            sound,
        }
    }

    /// The heart-sound stream on this output: starts a [`HeartSoundStream`] (silent until samples
    /// arrive) and returns the producer side.
    ///
    /// # Errors
    /// When kira refuses another sound (its sound capacity is reached).
    pub fn heart_sound_sink(&self) -> Result<KiraPcmSink, String> {
        let (producer, consumer) = rtrb::RingBuffer::new(samples_for_ms(QUEUE_CAPACITY_MS));
        let shared = Arc::new(Shared::default());
        let stream = HeartSoundStream {
            core: StreamCore::new(NOMINAL_DEVICE_RATE_HZ),
            rate_hz: NOMINAL_DEVICE_RATE_HZ,
            source: RtrbSource {
                consumer,
                shared: Arc::clone(&shared),
                popped: 0,
            },
        };
        self.manager
            .borrow_mut()
            .play(HeartSoundData { stream })
            .map_err(|e| e.to_string())?;
        Ok(KiraPcmSink {
            producer,
            shared,
            pushed: 0,
            _manager: Some(Rc::clone(&self.manager)),
        })
    }
}

/// The heart-sound sink of this build, on the shared output.
///
/// # Errors
/// No device, or kira refused the stream.
pub fn heart_sound_sink() -> Result<KiraPcmSink, String> {
    AudioOutput::shared()?.heart_sound_sink()
}

/// Plays the beep through the shared output device.
pub struct KiraBeepSink {
    manager: Manager,
    sound: StaticSoundData,
}

impl std::fmt::Debug for KiraBeepSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KiraBeepSink").finish_non_exhaustive()
    }
}

impl KiraBeepSink {
    /// Opens the shared output device (if not open yet) and prepares the beep.
    ///
    /// # Errors
    /// The device error as text when there is no output device or the stream cannot be started (headless
    /// machines, CI, a sound server that is not running).
    pub fn new() -> Result<Self, String> {
        Ok(AudioOutput::shared()?.beep_sink())
    }
}

/// Linear gain `0.0..=1.0` as `kira` decibels; zero (or less) is `Decibels::SILENCE`, which kira renders as
/// silence (at or below -60 dB the amplitude is exactly 0).
fn volume_to_decibels(volume: f32) -> Decibels {
    if volume <= 0.0 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * volume.min(1.0).log10())
    }
}

impl BeepSink for KiraBeepSink {
    fn play(&mut self, volume: f32) {
        if volume <= 0.0 {
            return;
        }
        let sound = self.sound.volume(volume_to_decibels(volume));
        if let Err(e) = self.manager.borrow_mut().play(sound) {
            tracing::debug!("monitor beep: could not start the sound: {e}");
        }
    }
}

/// What the UI thread and the audio thread share besides the queue: counters and the gain, as
/// atomics.
#[derive(Debug, Default)]
struct Shared {
    /// Samples the audio thread has taken from the queue (played or dropped).
    popped: AtomicU64,
    /// Samples with an index below this were queued before the last clear and are to be dropped.
    discard_until: AtomicU64,
    /// Linear gain, `f32` bits.
    gain_bits: AtomicU32,
    /// The sink was dropped: the sound may be unloaded.
    closed: AtomicBool,
}

/// The UI-thread side of the heart-sound stream.
pub struct KiraPcmSink {
    producer: rtrb::Producer<f32>,
    shared: Arc<Shared>,
    /// Samples pushed so far.
    pushed: u64,
    /// Keeps the device stream open while the sink exists (`None` only in tests).
    _manager: Option<Manager>,
}

impl std::fmt::Debug for KiraPcmSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KiraPcmSink")
            .field("pushed", &self.pushed)
            .finish_non_exhaustive()
    }
}

impl PcmSink for KiraPcmSink {
    fn push(&mut self, samples: &[f32]) -> usize {
        let n = samples.len().min(self.producer.slots());
        if n == 0 {
            return 0;
        }
        let Ok(chunk) = self.producer.write_chunk_uninit(n) else {
            return 0;
        };
        let written = chunk.fill_from_iter(samples[..n].iter().copied());
        self.pushed += written as u64;
        written
    }

    fn queued_ms(&self) -> f32 {
        let popped = self.shared.popped.load(Ordering::Acquire);
        let cleared = self.shared.discard_until.load(Ordering::Relaxed);
        let queued = self.pushed.saturating_sub(popped.max(cleared));
        ms_for_samples(usize::try_from(queued).unwrap_or(usize::MAX))
    }

    fn clear(&mut self) {
        self.shared.discard_until.store(self.pushed, Ordering::Release);
    }

    fn set_gain(&mut self, gain: f32) {
        self.shared.gain_bits.store(gain.to_bits(), Ordering::Relaxed);
    }
}

impl Drop for KiraPcmSink {
    fn drop(&mut self) {
        self.shared.closed.store(true, Ordering::Release);
    }
}

/// The audio-thread side of the queue.
#[derive(Debug)]
struct RtrbSource {
    consumer: rtrb::Consumer<f32>,
    shared: Arc<Shared>,
    popped: u64,
}

impl RtrbSource {
    fn take(&mut self) -> Option<f32> {
        let v = self.consumer.pop().ok()?;
        self.popped += 1;
        self.shared.popped.store(self.popped, Ordering::Release);
        Some(v)
    }

    fn boundary(&self) -> u64 {
        self.shared.discard_until.load(Ordering::Acquire)
    }
}

impl PcmSource for RtrbSource {
    fn ready(&self) -> usize {
        let old = self.boundary().saturating_sub(self.popped);
        let available = self.consumer.slots() as u64;
        usize::try_from(available.saturating_sub(old)).unwrap_or(usize::MAX)
    }

    fn pop(&mut self) -> Option<f32> {
        if self.clear_pending() {
            return None;
        }
        self.take()
    }

    fn clear_pending(&self) -> bool {
        self.popped < self.boundary()
    }

    fn pop_cleared(&mut self) -> Option<f32> {
        if self.clear_pending() { self.take() } else { None }
    }

    fn finish_clear(&mut self) {
        let boundary = self.boundary();
        while self.popped < boundary {
            if self.take().is_none() {
                break;
            }
        }
    }
}

/// The heart sounds as a kira sound: runs [`StreamCore`] on the audio thread.
#[derive(Debug)]
pub struct HeartSoundStream {
    core: StreamCore,
    rate_hz: f64,
    source: RtrbSource,
}

impl Sound for HeartSoundStream {
    fn process(&mut self, out: &mut [Frame], dt: f64, _info: &Info<'_>) {
        // the device reports its rate through `dt`; adapting allocates nothing
        if dt > 0.0 {
            let rate = 1.0 / dt;
            if (rate - self.rate_hz).abs() > 0.5 {
                self.core.set_out_rate(rate);
                self.rate_hz = rate;
            }
        }
        self.core.set_gain(f32::from_bits(
            self.source.shared.gain_bits.load(Ordering::Relaxed),
        ));
        for frame in out.iter_mut() {
            *frame = Frame::from_mono(self.core.next_sample(&mut self.source));
        }
    }

    fn finished(&self) -> bool {
        self.source.shared.closed.load(Ordering::Acquire)
    }
}

/// Hands a [`HeartSoundStream`] to `AudioManager::play`.
#[derive(Debug)]
struct HeartSoundData {
    stream: HeartSoundStream,
}

impl SoundData for HeartSoundData {
    type Error = std::convert::Infallible;
    type Handle = ();

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        Ok((Box::new(self.stream), ()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_volume_is_unity_gain() {
        assert!((volume_to_decibels(1.0).as_amplitude() - 1.0).abs() < 1e-6);
        assert!((volume_to_decibels(7.0).as_amplitude() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn half_volume_is_about_minus_six_decibels() {
        let db = volume_to_decibels(0.5);
        assert!((db.0 + 6.02).abs() < 0.01, "{db:?}");
        assert!((db.as_amplitude() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn zero_volume_is_silent() {
        assert!(volume_to_decibels(0.0).as_amplitude() <= 0.0);
        assert!(volume_to_decibels(-1.0).as_amplitude() <= 0.0);
    }

    fn pair() -> (KiraPcmSink, HeartSoundStream) {
        let (producer, consumer) = rtrb::RingBuffer::new(samples_for_ms(QUEUE_CAPACITY_MS));
        let shared = Arc::new(Shared::default());
        let stream = HeartSoundStream {
            core: StreamCore::new(NOMINAL_DEVICE_RATE_HZ),
            rate_hz: NOMINAL_DEVICE_RATE_HZ,
            source: RtrbSource {
                consumer,
                shared: Arc::clone(&shared),
                popped: 0,
            },
        };
        let sink = KiraPcmSink {
            producer,
            shared,
            pushed: 0,
            _manager: None,
        };
        (sink, stream)
    }

    /// The queue between the two threads, without a device: pushes are counted, a clear drops what
    /// was queued, the stream plays what follows, and dropping the sink finishes the sound.
    #[test]
    fn the_queue_counts_clears_and_closes() {
        let (mut sink, mut stream) = pair();
        sink.set_gain(1.0);
        let tone: Vec<f32> = (0..800).map(|k| (k as f32 * 0.314).sin()).collect();
        assert_eq!(sink.push(&tone), 800);
        assert!((sink.queued_ms() - 200.0).abs() < 1e-3);
        let info = kira::info::MockInfoBuilder::new().build();
        let mut out = vec![Frame::ZERO; 4800];
        stream.process(&mut out, 1.0 / 48_000.0, &info);
        assert!(out.iter().any(|f| f.left.abs() > 0.1), "plays after priming");
        assert!(out.iter().all(|f| f.left.to_bits() == f.right.to_bits()), "mono");
        assert!(sink.queued_ms() < 200.0, "consumed");
        sink.clear();
        assert!(sink.queued_ms().abs() < 1e-6, "cleared at once for the player");
        stream.process(&mut out, 1.0 / 48_000.0, &info);
        assert!(
            out[2000..].iter().all(|f| f.left.abs() < 1e-3),
            "silent after the fade"
        );
        assert!(!stream.finished());
        drop(sink);
        assert!(stream.finished());
    }
}
