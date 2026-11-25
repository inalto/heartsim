//! Wall motion of the 3-D heart in the app (brief B30): owns the kinematic [`MotionModel`], the
//! record it reads ([`MotionHistory`]), the frame it fills, and the policy that decides when it is
//! evaluated. Pure CPU, no GPU and no egui; [`crate::heart_view::HeartView`] uploads the frame.
//!
//! **One source of truth.** The motion is a function of the timing engine's events, the cell state
//! and the valve schedule recorded here, evaluated at the time the strip and the colour map are
//! drawn. It is never read back by an engine: the simulation neither waits for it nor is driven by
//! it, and a frame that is not evaluated simply shows the previous one.
//!
//! # Evaluation policy
//!
//! One evaluation costs about 8 ms natively on the development VM (more on `wasm32`), so
//! [`HeartMotion::tick`]
//! * runs at most once per drawn frame and at most every [`min_interval_s`] of wall-clock time
//!   ([`MAX_RATE_NATIVE_HZ`] natively, [`MAX_RATE_WEB_HZ`] on `wasm32`; the cap is wall-clock, so
//!   it does not depend on the simulation speed);
//! * does nothing when nothing changed: same drawn time, same breathing phase, same switches and
//!   no new injury or scenario (a paused simulation costs nothing);
//! * after the switch is turned off, writes the rest mesh once and then stays idle;
//! * with the 3-D view not drawn but the card shown, calls [`MotionModel::readout`] only (about a
//!   third of the cost, no vertex written).

use heartsim_core::Auscultation;
use heartsim_ecg::PcgParams;
use heartsim_echo::{
    EchoAnatomy, MotionError, MotionFrame, MotionHistory, MotionInput, MotionModel, MotionParams,
    MotionReadout, SliceState, mechanical_injury,
};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::timing::RespiratoryClock;
use heartsim_sim::{CellState, EikonalEngine, Event};

/// The largest exaggeration the slider offers. The model folds the thin walls at 2x at
/// end-systole, so the UI stops at 1.5x (the model itself accepts up to 3x).
pub const MAX_EXAGGERATION: f32 = 1.5;
/// The smallest exaggeration the slider offers (half the model's motion).
pub const MIN_EXAGGERATION: f32 = 0.5;

/// Evaluation rate cap on the desktop, Hz: wall motion at 30 Hz is smooth for a 1 Hz heart and
/// leaves a slow machine most of its frame (one evaluation is about 8 ms on the development VM).
/// Designed.
pub const MAX_RATE_NATIVE_HZ: f64 = 30.0;
/// Evaluation rate cap in the browser, Hz: `wasm32` is single-threaded and several times slower
/// than native, so fewer evaluations per second keep the UI responsive. Designed.
pub const MAX_RATE_WEB_HZ: f64 = 20.0;

/// Shortest wall-clock interval between two evaluations, s.
#[must_use]
pub const fn min_interval_s() -> f64 {
    if cfg!(target_arch = "wasm32") {
        1.0 / MAX_RATE_WEB_HZ
    } else {
        1.0 / MAX_RATE_NATIVE_HZ
    }
}

/// How far behind the drawn time the record is kept, ms. The view can be drawn up to a minute
/// behind the engine (scrubbing, heart-sound display delay), plus margin.
const KEEP_MS: f64 = 70_000.0;

/// What [`HeartMotion::tick`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tick {
    /// Nothing changed since the last evaluation.
    Unchanged,
    /// Something changed but the rate cap says wait: ask for a repaint after this many seconds.
    Wait(f64),
    /// The vertices and the read-out were evaluated (or the rest mesh written): upload them.
    Evaluated,
    /// Only the read-out was computed (the 3-D view is not drawn, or the motion is off).
    ReadOut,
}

/// Everything that makes one evaluation differ from the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    t_bits: u64,
    phase_bits: u64,
    enabled: bool,
    exaggeration_bits: u32,
    vertices: bool,
    epoch: u64,
}

/// What the last vertex evaluation was made with: the cross-section (B31) samples the same
/// instant, so it needs these and not the time the clock has reached since.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Evaluated {
    t_ms: f64,
    resp_phase: f64,
    enabled: bool,
    exaggeration: f32,
}

/// What the M-mode and tissue-Doppler scans (brief B34) read from the motion: the model they
/// evaluate at their own times, the record, the drawn time and the page's switches.
#[derive(Debug)]
pub struct ScanParts<'a> {
    pub model: &'a mut MotionModel,
    pub history: &'a MotionHistory,
    /// The drawn time (the strip's), ms.
    pub t_ms: f64,
    /// Wall motion drawn (the switch on, reduced motion off).
    pub enabled: bool,
    /// The page's exaggeration (clamped), so a trace and the sector agree.
    pub exaggeration: f32,
    /// Reduced motion: the traces hold still.
    pub reduced_motion: bool,
}

/// The wall-motion model, its record and the evaluation policy.
#[derive(Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "two user switches and two independent upload states"
)]
pub struct HeartMotion {
    model: MotionModel,
    history: MotionHistory,
    frame: MotionFrame,
    cell_count: usize,
    /// The user's switch ("Wall motion"), on by default.
    pub on: bool,
    /// Multiplier on the wall displacement, `MIN_EXAGGERATION..=MAX_EXAGGERATION`.
    pub exaggeration: f32,
    /// Reduced motion (Settings) forces the motion off.
    pub reduced_motion: bool,
    t_ms: f64,
    resp_phase: f64,
    last_key: Option<Key>,
    last_eval_s: f64,
    /// Bumped by everything that changes the output without changing the key (a new scenario, a
    /// lesion).
    epoch: u64,
    /// The frame holds the rest mesh and the GPU already has it.
    rest_uploaded: bool,
    /// The frame holds vertices the GPU has not seen yet.
    upload_pending: bool,
    readout: MotionReadout,
    last_forget_ms: f64,
    /// The inputs of the last evaluation that wrote the frame, and how many such evaluations
    /// there have been (the echo cross-section follows them).
    evaluated: Option<Evaluated>,
    serial: u64,
}

impl HeartMotion {
    /// Build the model from `pack` (before the heart view consumes it). `cell_count` is the number
    /// of cells of the 3-D engine.
    ///
    /// # Errors
    /// When the pack lacks what the model needs (landmarks, tags).
    pub fn new(pack: &AnatomyPack, cell_count: usize) -> Result<Self, MotionError> {
        let model = MotionModel::new(pack, MotionParams::default())?;
        let frame = model.new_frame();
        Ok(Self {
            model,
            history: MotionHistory::new(PcgParams::default(), 15.0, cell_count),
            frame,
            cell_count,
            on: true,
            exaggeration: 1.0,
            reduced_motion: false,
            t_ms: 0.0,
            resp_phase: 0.0,
            last_key: None,
            last_eval_s: f64::NEG_INFINITY,
            epoch: 0,
            rest_uploaded: true,
            upload_pending: false,
            readout: MotionReadout::default(),
            last_forget_ms: 0.0,
            evaluated: None,
            serial: 0,
        })
    }

    /// Is the wall motion drawn: the switch on and reduced motion off.
    #[must_use]
    pub fn active(&self) -> bool {
        self.on && !self.reduced_motion
    }

    /// A new scenario or engine: forget the record, the lesions and what was drawn. Takes the
    /// scenario's breathing rate and `auscultation` like the heart sounds do, so both schedule the
    /// valves alike.
    pub fn reset(&mut self, respiration_rate_bpm: f64, auscultation: &Auscultation) {
        self.history = MotionHistory::new(PcgParams::default(), respiration_rate_bpm, self.cell_count)
            .with_auscultation(auscultation);
        self.model.set_injury(&[]);
        self.t_ms = 0.0;
        self.last_forget_ms = 0.0;
        self.epoch += 1;
    }

    /// The breathing clock and the timing events of one engine step, in engine order.
    pub fn on_events(&mut self, events: &[Event], respiration: RespiratoryClock) {
        self.history.set_respiration(respiration);
        self.history.on_events(events);
    }

    /// Note the activations in the cell state on display (after a step, or once per frame for the
    /// externally driven engines).
    pub fn record_cells(&mut self, cells: &CellState) {
        self.history.record_cells(cells);
    }

    /// Hand the coronary lesions of `engine` to the model (after an `Action::Stenosis`).
    pub fn refresh_injury(&mut self, engine: &EikonalEngine, aha_segments: &[u8]) {
        self.model.set_injury(&mechanical_injury(engine, aha_segments));
        self.epoch += 1;
    }

    /// The time to draw (the strip's: engine time minus the display delay and the scrub, ms) and
    /// the respiratory phase there.
    pub fn set_time(&mut self, t_ms: f64, resp_phase: f64) {
        self.t_ms = t_ms.max(0.0);
        self.resp_phase = resp_phase;
        if self.t_ms - self.last_forget_ms > 5_000.0 || self.t_ms < self.last_forget_ms {
            self.last_forget_ms = self.t_ms;
            self.history.forget_before(self.t_ms - KEEP_MS);
        }
    }

    /// Evaluate if the policy says so (see the module docs). `wall_s` is a monotonic wall-clock
    /// time in seconds; `want_vertices` says the 3-D view is drawn this frame, `want_readout` that
    /// the read-out card is shown.
    pub fn tick(&mut self, wall_s: f64, want_vertices: bool, want_readout: bool) -> Tick {
        let enabled = self.active();
        if !(want_readout || want_vertices && (enabled || !self.rest_uploaded)) {
            // the motion is off and on the GPU as rest, and nobody reads the card
            return Tick::Unchanged;
        }
        let key = Key {
            t_bits: self.t_ms.to_bits(),
            phase_bits: self.resp_phase.to_bits(),
            enabled,
            exaggeration_bits: self.exaggeration.to_bits(),
            vertices: want_vertices,
            epoch: self.epoch,
        };
        if self.last_key == Some(key) {
            return Tick::Unchanged;
        }
        let since = wall_s - self.last_eval_s;
        if since < min_interval_s() {
            return Tick::Wait(min_interval_s() - since);
        }
        self.last_eval_s = wall_s;
        self.last_key = Some(key);
        let input = MotionInput {
            history: &self.history,
            t_ms: self.t_ms,
            resp_phase: self.resp_phase,
            enabled,
            exaggeration: self.exaggeration.clamp(0.0, MAX_EXAGGERATION),
        };
        if want_vertices && (enabled || !self.rest_uploaded) {
            self.model.evaluate(&input, &mut self.frame);
            self.evaluated = Some(Evaluated {
                t_ms: input.t_ms,
                resp_phase: input.resp_phase,
                enabled: input.enabled,
                exaggeration: input.exaggeration,
            });
            self.serial += 1;
            self.readout = self.frame.readout;
            self.rest_uploaded = !enabled;
            self.upload_pending = true;
            Tick::Evaluated
        } else {
            // the motion is off (the GPU holds the rest mesh) or the view is not drawn: the card
            // only, the read-out of the model at 1x
            self.readout = self.model.readout(&MotionInput {
                enabled: true,
                exaggeration: 1.0,
                ..input
            });
            Tick::ReadOut
        }
    }

    /// The vertices to upload, once: `Some` after an evaluation the GPU has not seen.
    pub fn take_upload(&mut self) -> Option<&MotionFrame> {
        std::mem::take(&mut self.upload_pending).then_some(&self.frame)
    }

    /// The frame as last evaluated (what is, or will be, uploaded): rest mesh bit for bit when the
    /// motion is off.
    #[must_use]
    pub fn frame(&self) -> &MotionFrame {
        &self.frame
    }

    /// The read-out of the current beat at the drawn time.
    #[must_use]
    pub fn readout(&self) -> &MotionReadout {
        &self.readout
    }

    /// The model (valve discs, rest parts).
    #[must_use]
    pub fn model(&self) -> &MotionModel {
        &self.model
    }

    /// The record the model reads (tests).
    #[must_use]
    pub fn history(&self) -> &MotionHistory {
        &self.history
    }

    /// Drawn time, ms.
    #[must_use]
    pub fn time_ms(&self) -> f64 {
        self.t_ms
    }

    /// The parts a scan needs, borrowed together (the model is evaluated at the scan's own times,
    /// never read back by an engine).
    pub fn scan_parts(&mut self) -> ScanParts<'_> {
        ScanParts {
            enabled: self.active(),
            exaggeration: self.exaggeration.clamp(0.0, MAX_EXAGGERATION),
            t_ms: self.t_ms,
            reduced_motion: self.reduced_motion,
            model: &mut self.model,
            history: &self.history,
        }
    }

    // ---- the echo cross-section (B31) ------------------------------------------------------

    /// Changes whenever the cross-section has something new to show, and not otherwise: the
    /// number of vertex evaluations (the moving wall; the rate cap of [`tick`](Self::tick)
    /// applies), and, while the frame is the rest mesh (motion off), the drawn time in bits (the
    /// leaflets still open and close by phase). A paused clock therefore never changes it.
    #[must_use]
    pub fn echo_key(&self) -> (u64, u64) {
        match self.evaluated {
            Some(e) if e.enabled => (self.serial, 0),
            _ => (self.serial, self.t_ms.to_bits()),
        }
    }

    /// Refresh `state` for the instant of the frame that is drawn: the displacement grid from
    /// **this frame** (the one the 3-D heart uploads; no second evaluation of the model) and the
    /// valve openings at its time. With the motion off the grid is at rest and the openings follow
    /// the drawn time.
    pub fn echo_update(&self, state: &mut SliceState, anatomy: &EchoAnatomy) {
        let (enabled, t_ms, resp_phase, exaggeration) = match self.evaluated {
            Some(e) if e.enabled => (true, e.t_ms, e.resp_phase, e.exaggeration),
            _ => (false, self.t_ms, self.resp_phase, 1.0),
        };
        let input = MotionInput {
            history: &self.history,
            t_ms,
            resp_phase,
            enabled,
            exaggeration,
        };
        state.update(anatomy, &self.model, &self.frame, &input);
    }
}
