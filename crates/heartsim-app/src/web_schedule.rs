//! Heart sounds through Web Audio, the pure half (brief B26): when each chunk of the phonocardiogram
//! starts on the browser's audio clock, and what samples it carries.
//!
//! In a browser there is no audio thread of ours to pull samples: the page hands the audio context
//! one `AudioBuffer` per chunk and tells it when to start (`AudioBufferSourceNode.start(when)`, the
//! "tale of two clocks" pattern). [`WebScheduler`] decides those times from the context clock read
//! at each push, and does everything to the samples that does not need the browser, so all of it is
//! tested natively with a fake clock; `heartsim-web` only copies its output into the Web Audio
//! objects.
//!
//! * **Contiguous.** A chunk starts on the output frame where the previous one ended, so the stream
//!   has no gap and no overlap, to the sample.
//! * **Look-ahead.** A stream (re)starts no earlier than [`LOOKAHEAD_MS`] after the context clock,
//!   which leaves the main thread one long frame to deliver the next chunk. The first chunk of a
//!   stream is placed so that its newest sample is heard [`LOOKAHEAD_MS`] + [`QUEUE_TARGET_MS`]
//!   after the push, the constant delay the sink reports ([`WebScheduler::latency_ms`]).
//! * **Re-anchor.** A stream continues as long as its scheduled end is at least
//!   [`CONTINUE_MARGIN_MS`] ahead of the clock, so a frame up to the look-ahead plus the queue long
//!   is absorbed without a gap and without changing the delay. When the scheduled audio has run out
//!   (an underrun, a hidden tab, a suspended context) the next chunk starts a new stream with a
//!   [`FADE_MS`] fade-in; [`WebScheduler::clear`] ends the stream at once.
//! * **Merging.** Pushes shorter than [`MIN_CHUNK_MS`] are held and joined, so a 144 Hz display does
//!   not create hundreds of audio nodes per second.
//! * **Drift.** The audio clock and the simulation's frame clock are different clocks. As on the
//!   desktop ([`crate::sound_out::StreamCore`]), the resampling ratio is nudged by at most
//!   [`MAX_DRIFT`] to hold the queue beyond the look-ahead at [`QUEUE_TARGET_MS`]; the simulation is
//!   never told.
//! * **Level.** Resampled from the PCG rate to the context rate with [`Resampler`] (an `AudioBuffer`
//!   below 8000 Hz is not guaranteed by the standard), gain ramped across each chunk, then
//!   [`soft_limit`].
//!
//! Write-only: nothing here reaches the simulation. The clock is an argument, never read here.

use std::collections::VecDeque;

use heartsim_ecg::PCG_RATE_HZ;

use crate::sound_out::{
    DRIFT_GAIN, DRIFT_SMOOTHING_S, FADE_MS, MAX_DRIFT, QUEUE_TARGET_MS, Resampler, soft_limit,
};

/// How far ahead of the audio clock a stream (re)starts, ms. Designed (maintainer's decision for
/// B26: 100 ms): enough for one long main-thread frame (a 12-lead redraw, a garbage collection)
/// between two pushes. A longer one survives slower frames but makes the drawn views lag more while
/// sound plays.
pub const LOOKAHEAD_MS: f64 = 100.0;
/// A stream whose scheduled end is less than this ahead of the clock is treated as ended (the next
/// chunk could not be started on time), ms. Designed: a few render quanta (128 frames, 2.7 ms at
/// 48 kHz) plus the time the main thread needs between reading the clock and `start()`.
pub const CONTINUE_MARGIN_MS: f64 = 10.0;
/// Pushes shorter than this are held and merged with the next, ms. Designed: at 60 Hz two frames
/// make one chunk (30 audio nodes a second), at 144 Hz three.
pub const MIN_CHUNK_MS: f64 = 20.0;

/// One chunk to hand to the audio context.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledChunk {
    /// Identifies the chunk in [`WebScheduler::clear`]'s answer.
    pub id: u64,
    /// First output frame on the context clock (`frame / rate` seconds).
    pub start_frame: u64,
    /// Samples at the context rate, gain and limiter applied.
    pub samples: Vec<f32>,
    /// This chunk starts a new stream (after start, a gap or a clear) and fades in.
    pub fade_in: bool,
    /// Context clock at the push that produced the chunk, s (for tests and statistics).
    pub scheduled_at_s: f64,
}

impl ScheduledChunk {
    /// Output frame after the last sample.
    #[must_use]
    pub fn end_frame(&self) -> u64 {
        self.start_frame + self.samples.len() as u64
    }
}

/// A chunk that was scheduled and has not finished, reported by [`WebScheduler::clear`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveChunk {
    pub id: u64,
    /// It was already playing at the clear (fade it out); otherwise it never started (stop it now).
    pub started: bool,
}

/// Counters for `window.heartsimStatus.audio`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScheduleStats {
    /// Chunks scheduled since creation.
    pub chunks: u64,
    /// Streams started (first chunk, and after every gap or clear).
    pub anchors: u64,
    /// Chunks whose start was already past the clock when scheduled. Zero by construction.
    pub late: u64,
}

/// Decides the start of each chunk on the audio clock. See the module documentation.
#[derive(Debug, Clone)]
pub struct WebScheduler {
    out_rate_hz: f64,
    resampler: Resampler,
    base_step: f64,
    /// Output frame where the scheduled stream ends; `None` before the first chunk and after a clear.
    end_frame: Option<u64>,
    /// PCG samples pushed and not yet scheduled (merging).
    pending: Vec<f32>,
    /// PCG samples the resampler has not consumed yet (less than one output's worth).
    carry: VecDeque<f32>,
    pushed_since_tick: bool,
    /// Scheduled chunks not known to have finished: `(id, start, end)` frames.
    live: VecDeque<(u64, u64, u64)>,
    next_id: u64,
    gain: f32,
    gain_target: f32,
    /// Output samples of the fade-in done so far in this stream.
    fade_pos: usize,
    fade_len: usize,
    /// Smoothed queue beyond the look-ahead, right after each schedule, s.
    level_s: f64,
    last_schedule_s: f64,
    stats: ScheduleStats,
}

impl WebScheduler {
    /// A scheduler for a context running at `out_rate_hz`.
    #[must_use]
    pub fn new(out_rate_hz: f64) -> Self {
        let out = out_rate_hz.max(1.0);
        let in_rate = f64::from(PCG_RATE_HZ);
        Self {
            out_rate_hz: out,
            resampler: Resampler::new(in_rate, out),
            base_step: in_rate / out,
            end_frame: None,
            pending: Vec::new(),
            carry: VecDeque::new(),
            pushed_since_tick: false,
            live: VecDeque::new(),
            next_id: 0,
            gain: 0.0,
            gain_target: 0.0,
            fade_pos: 0,
            fade_len: ((f64::from(FADE_MS) * out / 1000.0).round() as usize).max(1),
            level_s: f64::from(QUEUE_TARGET_MS) / 1000.0,
            last_schedule_s: 0.0,
            stats: ScheduleStats::default(),
        }
    }

    /// The context's sample rate, Hz.
    #[must_use]
    pub fn out_rate_hz(&self) -> f64 {
        self.out_rate_hz
    }

    /// The counters.
    #[must_use]
    pub fn stats(&self) -> ScheduleStats {
        self.stats
    }

    /// Linear gain before the limiter (reached across the next chunk, so a volume drag does not
    /// zipper).
    pub fn set_gain(&mut self, gain: f32) {
        self.gain_target = if gain.is_finite() { gain.max(0.0) } else { 0.0 };
    }

    /// The constant delay of a stream from the push of its newest sample to the speaker, ms: the
    /// look-ahead, the queue the player keeps beyond it, and the latencies the browser reports
    /// (`baseLatency`, `outputLatency`, s; a missing or negative one counts as zero).
    #[must_use]
    pub fn latency_ms(base_latency_s: f64, output_latency_s: f64) -> f32 {
        let s = |v: f64| if v.is_finite() { v.max(0.0) } else { 0.0 };
        (LOOKAHEAD_MS + f64::from(QUEUE_TARGET_MS) + 1000.0 * (s(base_latency_s) + s(output_latency_s)))
            as f32
    }

    fn frames(&self, ms: f64) -> u64 {
        (ms * self.out_rate_hz / 1000.0).round().max(0.0) as u64
    }

    fn now_frame(&self, now_s: f64) -> f64 {
        now_s.max(0.0) * self.out_rate_hz
    }

    /// Scheduled audio ahead of the clock, ms (scheduled end − now; 0 when nothing is scheduled).
    #[must_use]
    pub fn queued_ms(&self, now_s: f64) -> f32 {
        self.end_frame.map_or(0.0, |end| {
            ((end as f64 - self.now_frame(now_s)).max(0.0) * 1000.0 / self.out_rate_hz) as f32
        })
    }

    /// What the player sees as queued ([`crate::sound_out::PcmSink::queued_ms`]), ms: the scheduled
    /// audio beyond the look-ahead plus the samples held for merging. The look-ahead is the
    /// browser's equivalent of a device buffer: committed, like the part of a desktop device's buffer
    /// the kira queue does not count. In steady state this is about [`QUEUE_TARGET_MS`], as on the
    /// desktop, so the player's rules (underrun, backlog, slice) work unchanged. While a stream still
    /// runs but has eaten into the look-ahead (a long frame), at least one PCG sample is reported, so
    /// the player does not take it for an underrun and hands over the whole frame, which continues
    /// the stream at its constant delay; zero means the stream has ended, exactly when the next
    /// chunk re-anchors.
    #[must_use]
    pub fn sink_queued_ms(&self, now_s: f64) -> f32 {
        let lead = f64::from(self.queued_ms(now_s));
        let one_sample_ms = 1000.0 / f64::from(PCG_RATE_HZ);
        let beyond = if self.end_frame.is_some() && lead >= CONTINUE_MARGIN_MS {
            (lead - LOOKAHEAD_MS).max(one_sample_ms)
        } else {
            0.0
        };
        let held = self.pending.len() as f64 * one_sample_ms;
        (beyond + held) as f32
    }

    /// Hand over PCG samples at clock `now_s`; returns the chunk to start, if one is due (short
    /// pushes are held for merging).
    pub fn push(&mut self, now_s: f64, samples: &[f32]) -> Option<ScheduledChunk> {
        if samples.is_empty() {
            return None;
        }
        self.pushed_since_tick = true;
        self.pending.extend_from_slice(samples);
        let held_ms = self.pending.len() as f64 * 1000.0 / f64::from(PCG_RATE_HZ);
        if held_ms < MIN_CHUNK_MS {
            return None;
        }
        Some(self.schedule(now_s))
    }

    /// Once per frame after the player ran: samples held for merging are scheduled when no push
    /// came since the previous tick (the end of a slice, a pause), so a short tail is not stranded.
    pub fn tick(&mut self, now_s: f64) -> Option<ScheduledChunk> {
        let idle = !self.pushed_since_tick;
        self.pushed_since_tick = false;
        self.prune(now_s);
        (idle && !self.pending.is_empty()).then(|| self.schedule(now_s))
    }

    /// Forget chunks that have finished playing.
    pub fn prune(&mut self, now_s: f64) {
        let now = self.now_frame(now_s);
        while self.live.front().is_some_and(|&(_, _, end)| (end as f64) <= now) {
            self.live.pop_front();
        }
    }

    /// End the stream: drop what is held, and report the chunks not finished (the caller fades out
    /// the playing one and stops the others). The next push starts a new stream.
    pub fn clear(&mut self, now_s: f64) -> Vec<LiveChunk> {
        self.prune(now_s);
        let now = self.now_frame(now_s);
        let out = self
            .live
            .drain(..)
            .map(|(id, start, _)| LiveChunk {
                id,
                started: (start as f64) <= now,
            })
            .collect();
        self.pending.clear();
        self.carry.clear();
        self.end_frame = None;
        out
    }

    fn schedule(&mut self, now_s: f64) -> ScheduledChunk {
        let now = self.now_frame(now_s);
        let margin = self.frames(CONTINUE_MARGIN_MS) as f64;
        let input = std::mem::take(&mut self.pending);
        let input_ms = input.len() as f64 * 1000.0 / f64::from(PCG_RATE_HZ);
        let continuing = self.end_frame.filter(|&end| end as f64 >= now + margin);
        let fade_in = continuing.is_none();
        let start_frame = continuing.unwrap_or_else(|| {
            // a new stream: its newest sample is heard LOOKAHEAD + QUEUE_TARGET after now, and no
            // sample before now + LOOKAHEAD
            self.resampler.reset();
            self.carry.clear();
            self.fade_pos = 0;
            self.level_s = f64::from(QUEUE_TARGET_MS) / 1000.0;
            self.resampler.set_step(self.base_step);
            self.stats.anchors += 1;
            let wait_ms = (f64::from(QUEUE_TARGET_MS) - input_ms).max(0.0);
            now.ceil() as u64 + self.frames(LOOKAHEAD_MS) + self.frames(wait_ms)
        });
        if (start_frame as f64) < now {
            self.stats.late += 1;
        }
        let samples = self.render(&input);
        let id = self.next_id;
        self.next_id += 1;
        let chunk = ScheduledChunk {
            id,
            start_frame,
            samples,
            fade_in,
            scheduled_at_s: now_s,
        };
        let end = chunk.end_frame();
        self.end_frame = Some(end);
        self.live.push_back((id, start_frame, end));
        self.stats.chunks += 1;
        self.correct_drift(now_s, end);
        chunk
    }

    /// Resample `input` (after what was carried over), fade in, ramp the gain, limit.
    fn render(&mut self, input: &[f32]) -> Vec<f32> {
        self.carry.extend(input.iter().copied());
        let mut out = Vec::with_capacity((self.carry.len() as f64 / self.base_step) as usize + 2);
        while self.resampler.needs() <= self.carry.len() {
            let carry = &mut self.carry;
            out.push(self.resampler.next(|| carry.pop_front().unwrap_or(0.0)));
        }
        let n = out.len().max(1) as f32;
        let (g0, g1) = (self.gain, self.gain_target);
        for (i, y) in out.iter_mut().enumerate() {
            let fade = if self.fade_pos < self.fade_len {
                let x = self.fade_pos as f32 / self.fade_len as f32;
                self.fade_pos += 1;
                0.5 - 0.5 * (std::f32::consts::PI * x).cos()
            } else {
                1.0
            };
            let g = g0 + (g1 - g0) * (i as f32 + 1.0) / n;
            *y = soft_limit(*y * fade * g);
        }
        self.gain = g1;
        out
    }

    fn correct_drift(&mut self, now_s: f64, end: u64) {
        let ahead_s = (end as f64 - self.now_frame(now_s)) / self.out_rate_hz - LOOKAHEAD_MS / 1000.0;
        let dt = (now_s - self.last_schedule_s).max(0.0);
        self.last_schedule_s = now_s;
        let alpha = (dt / DRIFT_SMOOTHING_S).min(1.0);
        self.level_s += alpha * (ahead_s - self.level_s);
        let err = self.level_s / (f64::from(QUEUE_TARGET_MS) / 1000.0) - 1.0;
        let drift = (DRIFT_GAIN * err).clamp(-MAX_DRIFT, MAX_DRIFT);
        self.resampler.set_step(self.base_step * (1.0 + drift));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_out::samples_for_ms;

    const RATE: f64 = 48_000.0;

    /// A fake audio clock and a producer that pushes what the simulation made in each frame.
    struct Run {
        s: WebScheduler,
        now_s: f64,
        chunks: Vec<ScheduledChunk>,
        /// PCG samples produced and not yet whole (fractional sample per frame).
        acc: f64,
    }

    impl Run {
        fn new() -> Self {
            let mut s = WebScheduler::new(RATE);
            s.set_gain(0.5);
            Self {
                s,
                now_s: 3.0,
                chunks: Vec::new(),
                acc: 0.0,
            }
        }

        /// One frame of `dt_ms`: the clock advances, the frame's samples are pushed, then the tick.
        fn frame(&mut self, dt_ms: f64) {
            self.now_s += dt_ms / 1000.0;
            self.acc += dt_ms * f64::from(PCG_RATE_HZ) / 1000.0;
            let n = self.acc.floor();
            self.acc -= n;
            let samples = vec![0.25; n as usize];
            if let Some(c) = self.s.push(self.now_s, &samples) {
                self.chunks.push(c);
            }
            if let Some(c) = self.s.tick(self.now_s) {
                self.chunks.push(c);
            }
        }

        fn lookahead_frame(c: &ScheduledChunk) -> f64 {
            c.scheduled_at_s * RATE + LOOKAHEAD_MS * RATE / 1000.0
        }
    }

    fn steady(frame_ms: f64) -> Run {
        let mut r = Run::new();
        for _ in 0..(10_000.0 / frame_ms) as usize {
            r.frame(frame_ms);
        }
        r
    }

    #[test]
    fn steady_frames_give_contiguous_chunks_none_early() {
        for hz in [60.0, 144.0] {
            let r = steady(1000.0 / hz);
            assert!(r.chunks.len() > 100, "{hz} Hz: {} chunks", r.chunks.len());
            assert!(r.chunks[0].fade_in);
            assert_eq!(r.s.stats().anchors, 1, "{hz} Hz: one stream");
            for w in r.chunks.windows(2) {
                assert_eq!(
                    w[1].start_frame,
                    w[0].end_frame(),
                    "{hz} Hz: contiguous to the sample"
                );
                assert!(!w[1].fade_in);
            }
            let first = &r.chunks[0];
            assert!(first.start_frame as f64 >= Run::lookahead_frame(first) - 1.0);
            assert_eq!(r.s.stats().late, 0);
            // the queue stays near look-ahead + target, the delay the sink reports
            let q = f64::from(r.s.queued_ms(r.now_s));
            assert!((100.0..=150.0).contains(&q), "{hz} Hz: queued {q} ms");
        }
    }

    #[test]
    fn the_newest_sample_of_a_new_stream_is_heard_at_the_reported_delay() {
        let mut s = WebScheduler::new(RATE);
        let now = 1.0;
        let c = s
            .push(now, &vec![0.0; samples_for_ms(25.0)])
            .expect("long enough");
        let newest_ms = (c.end_frame() as f64 / RATE - now) * 1000.0;
        let want = f64::from(WebScheduler::latency_ms(0.0, 0.0));
        assert!((newest_ms - want).abs() < 3.0, "{newest_ms} vs {want}");
        assert!(c.start_frame as f64 >= (now * 1000.0 + LOOKAHEAD_MS) * RATE / 1000.0);
    }

    #[test]
    fn long_frames_continue_the_stream_without_a_gap() {
        // 8 fps (software rendering): each frame eats 125 ms of the 140 ms scheduled ahead
        let r = steady(125.0);
        assert_eq!(r.s.stats().anchors, 1, "one stream");
        for w in r.chunks.windows(2) {
            assert_eq!(w[1].start_frame, w[0].end_frame());
        }
        assert_eq!(r.s.stats().late, 0);
    }

    #[test]
    fn a_400_ms_gap_re_anchors_at_the_look_ahead_with_one_fade_in() {
        let mut r = steady(1000.0 / 60.0);
        let before = r.chunks.len();
        let last_end = r.chunks.last().unwrap().end_frame();
        r.now_s += 0.4;
        // what the player pushes after an underrun: the newest QUEUE_TARGET_MS
        let c =
            r.s.push(r.now_s, &vec![0.25; samples_for_ms(f64::from(QUEUE_TARGET_MS))])
                .expect("a chunk");
        assert!(c.fade_in);
        let at = r.now_s * RATE + LOOKAHEAD_MS * RATE / 1000.0;
        assert!((c.start_frame as f64 - at).abs() <= 1.0, "at now + look-ahead");
        assert!(c.start_frame >= last_end, "no overlap");
        r.chunks.push(c);
        for _ in 0..60 {
            r.frame(1000.0 / 60.0);
        }
        let after = &r.chunks[before..];
        assert_eq!(after.iter().filter(|c| c.fade_in).count(), 1, "one fade-in");
        for w in after.windows(2) {
            assert_eq!(w[1].start_frame, w[0].end_frame());
        }
        // the fade: the first samples rise from silence
        assert!(after[0].samples[0].abs() < 1e-3);
    }

    #[test]
    fn a_clear_reports_nothing_queued_and_the_next_push_re_anchors() {
        let mut r = steady(1000.0 / 60.0);
        assert!(r.s.queued_ms(r.now_s) > 50.0);
        let live = r.s.clear(r.now_s);
        assert_ne!(live.len(), 0);
        assert!(
            live.iter().any(|c| !c.started),
            "what had not started is reported to stop"
        );
        assert!(r.s.queued_ms(r.now_s).abs() < f32::EPSILON);
        assert!(r.s.sink_queued_ms(r.now_s).abs() < f32::EPSILON);
        let anchors = r.s.stats().anchors;
        let n = r.chunks.len();
        for _ in 0..10 {
            r.frame(1000.0 / 60.0);
        }
        assert_eq!(r.s.stats().anchors, anchors + 1);
        let c = &r.chunks[n];
        assert!(c.fade_in);
        assert!(c.start_frame as f64 >= Run::lookahead_frame(c) - 1.0);
    }

    #[test]
    fn a_hidden_tab_gap_schedules_nothing_in_the_past() {
        let mut r = steady(1000.0 / 60.0);
        // two seconds without frames (the clock runs, or was suspended and resumed: same here)
        r.now_s += 2.0;
        let n = r.chunks.len();
        // the first frame back is capped at 0.1 s by the app
        r.frame(100.0);
        for _ in 0..30 {
            r.frame(1000.0 / 60.0);
        }
        assert_eq!(r.s.stats().late, 0);
        for c in &r.chunks[n..] {
            assert!(c.start_frame as f64 >= c.scheduled_at_s * RATE, "not in the past");
        }
        assert!(r.chunks[n].fade_in);
        assert!(r.chunks[n].start_frame as f64 >= Run::lookahead_frame(&r.chunks[n]) - 1.0);
    }

    #[test]
    fn short_pushes_are_merged_to_the_minimum() {
        let min_out = (MIN_CHUNK_MS * RATE / 1000.0) as usize;
        for hz in [60.0, 144.0, 240.0] {
            let r = steady(1000.0 / hz);
            // the resampler's carry may shift a chunk by a sample
            assert!(
                r.chunks.iter().all(|c| c.samples.len() + 2 >= min_out),
                "{hz} Hz: a chunk below {MIN_CHUNK_MS} ms"
            );
            assert!(r.chunks.len() < (10.0 * 1000.0 / MIN_CHUNK_MS) as usize + 2);
        }
        // a tail shorter than the minimum is flushed on the first tick without a push
        let mut s = WebScheduler::new(RATE);
        assert!(s.push(1.0, &[0.1; 40]).is_none(), "10 ms: held");
        assert!(s.tick(1.0).is_none(), "a push came this frame");
        assert!(s.sink_queued_ms(1.0) > 9.0, "held samples count as queued");
        let tail = s.tick(1.016).expect("flushed when the pushes stop");
        assert!(tail.fade_in);
    }

    #[test]
    fn the_drift_correction_holds_the_queue_when_the_simulation_runs_fast() {
        // the frames deliver 0.3 % more audio than the clock plays
        let mut r = Run::new();
        for _ in 0..(120.0 * 60.0) as usize {
            r.now_s += 1.0 / 60.0;
            r.acc += 1.003 * f64::from(PCG_RATE_HZ) / 60.0;
            let n = r.acc.floor();
            r.acc -= n;
            if let Some(c) = r.s.push(r.now_s, &vec![0.0; n as usize]) {
                r.chunks.push(c);
            }
            let _ = r.s.tick(r.now_s);
        }
        let q = f64::from(r.s.queued_ms(r.now_s)) - LOOKAHEAD_MS;
        assert!(
            q < 2.0 * f64::from(QUEUE_TARGET_MS),
            "queue {q} ms beyond the look-ahead after 2 min"
        );
        assert_eq!(r.s.stats().anchors, 1, "no re-anchor needed");
    }

    #[test]
    fn gain_and_limiter_are_applied_in_the_samples() {
        let mut s = WebScheduler::new(RATE);
        s.set_gain(1.0);
        let c = s.push(0.0, &vec![3.0; samples_for_ms(200.0)]).expect("chunk");
        assert!(c.samples.iter().all(|v| v.abs() < 1.0), "limited");
        assert!(c.samples.iter().skip(2000).any(|v| *v > 0.9));
        let reported = WebScheduler::latency_ms(0.005, f64::NAN);
        assert!((reported - 145.0).abs() < 1e-3, "{reported}");
    }
}
