//! What the motion model reads from the simulation: the beats with their valve schedule, the
//! organised atrial activations, and the last few activations of every cell.
//!
//! [`MotionHistory`] is fed like the heart-sound synthesiser — the timing engine's events in
//! engine order, plus the cell state after each step — and keeps only what happened, never a
//! display state. The motion at a time `t` is then a pure function of this record, so the view
//! can be drawn at any time it holds (the live time, the delayed time while heart sounds play, a
//! scrub inside the cell record), at any frame rate.

use heartsim_core::Auscultation;
use heartsim_ecg::pcg::{Beat, BeatAssembler, Components, PcgParams, schedule_with};
use heartsim_sim::timing::RespiratoryClock;
use heartsim_sim::{CellState, Event, EventKind, Origin};

/// Activations kept per cell. Eight cover the display delay of the heart sounds and a few seconds
/// of scrubbing at any rhythm the engine makes; older times fall back to rest (see
/// [`CellHistory::latest_at`]).
pub const ACTIVATIONS_PER_CELL: usize = 8;

/// The last [`ACTIVATIONS_PER_CELL`] activations of every cell: `(activation_ms, apd_ms)`, newest
/// first.
#[derive(Debug, Clone)]
pub struct CellHistory {
    records: Vec<[(f32, f32); ACTIVATIONS_PER_CELL]>,
}

impl CellHistory {
    /// A history of `n` cells that have never activated.
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self {
            records: vec![[(f32::NAN, 0.0); ACTIVATIONS_PER_CELL]; n],
        }
    }

    /// Number of cells.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// No cells.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Note the activations in `cells` that are new since the last call. Calling it after every
    /// engine step or once per drawn frame gives the same record, unless a cell activates twice
    /// between two calls.
    pub fn record(&mut self, cells: &CellState) {
        if cells.activation_ms.len() != self.records.len() {
            *self = Self::new(cells.activation_ms.len());
        }
        for ((rec, &t), &apd) in self
            .records
            .iter_mut()
            .zip(&cells.activation_ms)
            .zip(&cells.apd_ms)
        {
            if !t.is_finite() || rec[0].0.to_bits() == t.to_bits() {
                continue;
            }
            if rec[0].0.is_finite() && t < rec[0].0 {
                // the engine went back in time (reset): forget this cell's past
                *rec = [(f32::NAN, 0.0); ACTIVATIONS_PER_CELL];
            }
            rec.copy_within(0..ACTIVATIONS_PER_CELL - 1, 1);
            rec[0] = (t, apd);
        }
    }

    /// Forget everything.
    pub fn clear(&mut self) {
        self.records.fill([(f32::NAN, 0.0); ACTIVATIONS_PER_CELL]);
    }

    /// The activations of cell `i` at or before `t_ms`, newest first.
    pub fn at_or_before(&self, i: usize, t_ms: f64) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.records[i]
            .iter()
            .filter(move |(a, _)| a.is_finite() && f64::from(*a) <= t_ms)
            .map(|&(a, d)| (f64::from(a), f64::from(d)))
    }

    /// The raw record of cell `i`: `(activation_ms, apd_ms)`, newest first, NaN where empty.
    #[must_use]
    pub fn record_of(&self, i: usize) -> &[(f32, f32); ACTIVATIONS_PER_CELL] {
        &self.records[i]
    }

    /// The newest activation of cell `i` at or before `t_ms`, if the record holds one.
    #[must_use]
    pub fn latest_at(&self, i: usize, t_ms: f64) -> Option<(f64, f64)> {
        self.at_or_before(i, t_ms).next()
    }
}

/// The beats, atrial activations and cell activations the motion is computed from (see the
/// module docs). Built like `heartsim_ecg::PcgSynth`: the same `PcgParams`, breathing rate,
/// `auscultation` section and respiratory clock give the same valve schedule as the heart sounds.
#[derive(Debug, Clone)]
pub struct MotionHistory {
    params: PcgParams,
    assembler: BeatAssembler,
    model: Auscultation,
    respiration_rate_bpm: f64,
    respiration: RespiratoryClock,
    beats: Vec<(Beat, Components)>,
    /// Onsets of organised atrial activations, ms (one per atrial beat).
    atrial_onsets_ms: Vec<f64>,
    /// Times of atrial flutter / fibrillation events, ms.
    atrial_circuit_ms: Vec<f64>,
    cells: CellHistory,
    last_event_ms: Option<f64>,
}

impl MotionHistory {
    /// An empty history for `cell_count` cells.
    #[must_use]
    pub fn new(params: PcgParams, respiration_rate_bpm: f64, cell_count: usize) -> Self {
        Self {
            assembler: BeatAssembler::new(&params),
            params,
            model: Auscultation::default(),
            respiration_rate_bpm,
            respiration: RespiratoryClock::default(),
            beats: Vec::new(),
            atrial_onsets_ms: Vec::new(),
            atrial_circuit_ms: Vec::new(),
            cells: CellHistory::new(cell_count),
            last_event_ms: None,
        }
    }

    /// Use a scenario's `auscultation` section, as `PcgSynth::with_auscultation` does (a stenosis
    /// that delays A2 then also delays the end of ejection).
    #[must_use]
    pub fn with_auscultation(mut self, model: &Auscultation) -> Self {
        self.model.clone_from(model);
        self
    }

    /// Respiratory phase at `t_ms` (`0..1`) on the clock this record was given, so a time other
    /// than the drawn one (an M-mode column) breathes as the engine does.
    #[must_use]
    pub fn respiratory_phase(&self, t_ms: f64) -> f64 {
        self.respiration.phase(self.respiration_rate_bpm, t_ms)
    }

    /// The engine's respiratory clock (breath-holds), as handed to the heart sounds.
    pub fn set_respiration(&mut self, clock: RespiratoryClock) {
        self.respiration = clock;
    }

    /// The heart-sound parameters the schedule is built with.
    #[must_use]
    pub fn pcg_params(&self) -> &PcgParams {
        &self.params
    }

    /// Feed one step's timing-engine events, in engine order. Time running backwards (a new
    /// engine) clears the history first.
    pub fn on_events(&mut self, events: &[Event]) {
        for ev in events {
            let t = ev.at.0;
            if self.last_event_ms.is_some_and(|l| t < l - 1e-9) {
                self.clear();
            }
            self.last_event_ms = Some(t);
            if ev.region.is_atrial() && ev.kind != EventKind::Blocked {
                match ev.origin {
                    Origin::Circuit(c) if !c.is_ventricular() => self.atrial_circuit_ms.push(t),
                    _ => {
                        let new_beat = self
                            .atrial_onsets_ms
                            .last()
                            .is_none_or(|&l| t - l >= self.params.same_beat_ms);
                        if new_beat {
                            self.atrial_onsets_ms.push(t);
                        }
                    }
                }
            }
            if let Some(beat) = self.assembler.push(ev) {
                let q = beat.qrs_onset.0;
                let phase = self.respiration.phase(self.respiration_rate_bpm, q);
                let c = schedule_with(&beat, &self.params, phase, &self.model);
                if beat.replaces_previous
                    && self
                        .beats
                        .last()
                        .is_some_and(|(b, _)| b.qrs_onset.0.to_bits() == q.to_bits())
                {
                    self.beats.pop();
                }
                self.beats.push((beat, c));
            }
        }
    }

    /// Note the activations of the cell state after a step (see [`CellHistory::record`]).
    pub fn record_cells(&mut self, cells: &CellState) {
        self.cells.record(cells);
    }

    /// Drop beats and atrial activations older than `t_ms` (keep the history bounded; the model
    /// looks back at most a few beats before the time it draws).
    pub fn forget_before(&mut self, t_ms: f64) {
        let keep_from = self
            .beats
            .iter()
            .position(|(b, _)| b.qrs_onset.0 >= t_ms)
            .unwrap_or(self.beats.len());
        // keep a few beats before the cut so the beat-to-beat recursion still has its start
        self.beats.drain(..keep_from.saturating_sub(4));
        let cut = |v: &mut Vec<f64>| {
            let k = v.iter().position(|&x| x >= t_ms).unwrap_or(v.len());
            v.drain(..k.saturating_sub(4));
        };
        cut(&mut self.atrial_onsets_ms);
        cut(&mut self.atrial_circuit_ms);
    }

    /// Forget everything (a new engine).
    pub fn clear(&mut self) {
        self.assembler = BeatAssembler::new(&self.params);
        self.beats.clear();
        self.atrial_onsets_ms.clear();
        self.atrial_circuit_ms.clear();
        self.cells.clear();
        self.last_event_ms = None;
    }

    /// The beats so far with their valve schedule, oldest first (what `PcgSynth::beats` holds for
    /// the same events).
    #[must_use]
    pub fn beats(&self) -> &[(Beat, Components)] {
        &self.beats
    }

    /// Onsets of the organised atrial activations, ms, oldest first.
    #[must_use]
    pub fn atrial_onsets_ms(&self) -> &[f64] {
        &self.atrial_onsets_ms
    }

    /// Times of the atrial flutter and fibrillation events, ms, oldest first.
    #[must_use]
    pub fn atrial_circuit_ms(&self) -> &[f64] {
        &self.atrial_circuit_ms
    }

    /// The cell activations.
    #[must_use]
    pub fn cells(&self) -> &CellHistory {
        &self.cells
    }

    /// Whether the atria are fibrillating or fluttering at `t_ms`: an atrial circuit event within
    /// `PcgParams::atrial_circuit_window_ms` before it (the beat assembler's rule).
    #[must_use]
    pub fn atria_disorganised_at(&self, t_ms: f64) -> bool {
        let w = self.params.atrial_circuit_window_ms;
        let k = self.atrial_circuit_ms.partition_point(|&x| x <= t_ms);
        k > 0 && t_ms - self.atrial_circuit_ms[k - 1] <= w
    }
}
