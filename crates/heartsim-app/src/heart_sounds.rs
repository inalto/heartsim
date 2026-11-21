//! The live phonocardiogram (brief B24): a [`PcgSynth`] fed the same engine events as the ECG
//! synthesiser, sampled into a 4 kHz ring in step with the strip rings, plus the S1/S2 labels as ring
//! indices. Silent: nothing here touches an audio device, and nothing flows back into the engine.
//!
//! For listening (B25) it also counts its resets (so the player knows when to re-anchor) and, while
//! heart-sound audio is on, feeds a [`WallTimeSynth`] the same events for slow motion.

use heartsim_core::Millis;
use heartsim_ecg::pcg::PCG_PERIOD_MS;
use heartsim_ecg::{LeadRing, PCG_RATE_HZ, PcgParams, PcgSynth, SoundLabel, WallTimeSynth};
use heartsim_sim::Event;
use heartsim_ui::PcgLane;

/// PCG samples per strip sample: the strip runs at 500 Hz, the PCG at [`PCG_RATE_HZ`].
pub const PCG_PER_ECG_SAMPLE: usize = 8;

/// The PCG of the running simulation.
#[derive(Debug)]
pub struct HeartSounds {
    synth: PcgSynth,
    ring: LeadRing,
    /// Labels as `(absolute ring index, label)`, oldest first.
    marks: Vec<(u64, SoundLabel)>,
    /// Engine time of the newest sample pushed, ms.
    last_t_ms: Option<f64>,
    seed: u64,
    respiration_rate_bpm: f64,
    /// Incremented by every [`HeartSounds::reset`]: a new engine, whose time starts again.
    generation: u64,
    /// The slow-motion synthesiser, fed only while heart-sound audio is on.
    wall: Option<WallTimeSynth>,
}

impl HeartSounds {
    /// A ring of `seconds` (the strip rings' length) and a synthesiser for the default engine.
    #[must_use]
    pub fn new(seconds: f32) -> Self {
        let respiration_rate_bpm = heartsim_sim::timing::SinusParams::default().respiration_rate_bpm;
        Self {
            synth: PcgSynth::new(PcgParams::default(), 0, respiration_rate_bpm),
            ring: LeadRing::new((PCG_RATE_HZ * seconds) as usize, PCG_RATE_HZ),
            marks: Vec::new(),
            last_t_ms: None,
            seed: 0,
            respiration_rate_bpm,
            generation: 0,
            wall: None,
        }
    }

    /// Start over for a new engine (a rhythm loaded, the EP lab or a re-entry preset started): a
    /// fresh synthesiser with the scenario's seed and breathing rate, the ring and labels cleared.
    /// The ring keeps counting, as the strip rings do, so the two stay in step.
    pub fn reset(&mut self, seed: u64, respiration_rate_bpm: f64) {
        self.seed = seed;
        self.respiration_rate_bpm = respiration_rate_bpm;
        self.synth = PcgSynth::new(PcgParams::default(), seed, respiration_rate_bpm);
        self.ring.clear();
        self.marks.clear();
        self.last_t_ms = None;
        self.generation += 1;
        if self.wall.is_some() {
            self.wall = Some(self.new_wall());
        }
    }

    fn new_wall(&self) -> WallTimeSynth {
        WallTimeSynth::new(PcgParams::default(), self.seed, self.respiration_rate_bpm)
    }

    /// How many times the synthesiser has started over for a new engine (scenario loaded,
    /// restarted, EP lab or re-entry started, or the clock went back).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Feed (or stop feeding) the slow-motion synthesiser. Switching it on starts a fresh one with
    /// this engine's seed and breathing rate; it hears the events from then on.
    pub fn set_wall_active(&mut self, active: bool) {
        match (active, self.wall.is_some()) {
            (true, false) => self.wall = Some(self.new_wall()),
            (false, true) => self.wall = None,
            _ => {}
        }
    }

    /// The slow-motion synthesiser, while active.
    pub fn wall_mut(&mut self) -> Option<&mut WallTimeSynth> {
        self.wall.as_mut()
    }

    /// Feed one step's engine events.
    pub fn on_events(&mut self, events: &[Event]) {
        self.synth.on_events(events);
        if let Some(w) = &mut self.wall {
            w.on_events(events);
        }
    }

    /// Push the [`PCG_PER_ECG_SAMPLE`] samples that end at `now`, the time of the strip sample just
    /// pushed, so PCG sample `8k + 7` and strip sample `k` are taken at the same instant.
    pub fn push_for_ecg_sample(&mut self, now: Millis) {
        if self.last_t_ms.is_some_and(|t| now.0 < t) {
            // the engine's clock went back without a reset (a new engine): start over
            let (seed, rate) = (self.seed, self.respiration_rate_bpm);
            self.reset(seed, rate);
        }
        for i in (0..PCG_PER_ECG_SAMPLE).rev() {
            let t = now.0 - i as f64 * PCG_PERIOD_MS;
            self.ring.push(self.synth.sample(Millis(t)));
        }
        self.last_t_ms = Some(now.0);
        let newest = self.ring.pushed() - 1;
        for m in self.synth.take_settled_marks(now) {
            let offset = ((m.at.0 - now.0) / PCG_PERIOD_MS).round() as i64;
            if let Some(abs) = newest.checked_add_signed(offset) {
                self.marks.push((abs, m.label));
            }
        }
        let oldest = self.ring.oldest_index();
        let stale = self.marks.iter().take_while(|(abs, _)| *abs < oldest).count();
        self.marks.drain(..stale);
    }

    /// The ring (tests, exports).
    #[must_use]
    pub fn ring(&self) -> &LeadRing {
        &self.ring
    }

    /// What the strip's heart-sound lane draws.
    #[must_use]
    pub fn lane(&self) -> PcgLane<'_> {
        PcgLane {
            ring: &self.ring,
            marks: &self.marks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_sim::{Engine, TimingEngine, TimingParams};

    #[test]
    fn the_ring_runs_in_step_with_the_strip_and_carries_labels() {
        let mut sounds = HeartSounds::new(10.0);
        let mut engine = TimingEngine::new(TimingParams::default());
        let mut ecg = LeadRing::new(5000, 500.0);
        for k in 0..4000 {
            engine.step(Millis(1.0));
            sounds.on_events(&engine.drain_events());
            if k % 2 == 1 {
                ecg.push(0.0);
                sounds.push_for_ecg_sample(engine.now());
            }
        }
        assert_eq!(sounds.ring().pushed(), 8 * ecg.pushed());
        let lane = sounds.lane();
        assert_eq!(lane.factor(&ecg), Some(8));
        assert!(lane.marks.iter().any(|(_, l)| *l == SoundLabel::S1));
        assert!(lane.ring.iter().any(|v| v.abs() > 0.5), "S1 in the ring");
        // a new engine: time runs from zero again, the synthesiser starts over
        let mut engine = TimingEngine::new(TimingParams::default());
        engine.step(Millis(1.0));
        sounds.push_for_ecg_sample(engine.now());
        assert_eq!(sounds.lane().marks.len(), 0, "labels of the old engine dropped");
    }
}
