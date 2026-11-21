//! The live phonocardiogram (brief B24): a [`PcgSynth`] fed the same engine events as the ECG
//! synthesiser, sampled into a 4 kHz ring in step with the strip rings, plus the S1/S2 labels as ring
//! indices. Silent: nothing here touches an audio device, and nothing flows back into the engine.
//!
//! For listening (B25) it also counts its resets (so the player knows when to re-anchor) and, while
//! heart-sound audio is on, feeds a [`WallTimeSynth`] the same events for slow motion.
//!
//! Both synthesisers hear the selected listening site and chest piece (B27 part B), so the lane and
//! the sound agree; the trace's schedule is kept for a ring's length so the 3-D heart's valve
//! highlights ([`HeartSounds::valve_glow_at`]) come from the sounds that are heard.
//!
//! Brief B28 adds two things. The synthesisers follow the engine's respiratory clock
//! ([`HeartSounds::set_respiration`]), so a breath-hold freezes the split with the sinus arrhythmia.
//! And in **small-speaker mode** ([`HeartSounds::set_small_speaker`]) a third synthesiser, fed the
//! same events with every frequency multiplied by [`SMALL_SPEAKER_FACTOR`], fills a second ring
//! with the same indices; the player reads that one ([`HeartSounds::audio_ring`]) while the lane keeps
//! drawing the true signal.

use heartsim_core::{Auscultation, Millis};
use heartsim_ecg::pcg::PCG_PERIOD_MS;
use heartsim_ecg::{
    LeadRing, Listening, ListeningChoice, PCG_RATE_HZ, PcgParams, PcgSynth, SMALL_SPEAKER_FACTOR, SiteTable,
    SoundLabel, WallTimeSynth,
};
use heartsim_sim::Event;
use heartsim_sim::timing::RespiratoryClock;
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
    /// The running scenario's `auscultation` section (brief B27): extra sounds and murmurs.
    auscultation: Auscultation,
    /// Incremented by every [`HeartSounds::reset`]: a new engine, whose time starts again.
    generation: u64,
    /// The slow-motion synthesiser, fed only while heart-sound audio is on.
    wall: Option<WallTimeSynth>,
    /// The listening site and chest piece both synthesisers apply, and what they resolve to.
    choice: ListeningChoice,
    listening: Listening,
    /// How long the schedule is kept for the valve highlights, ms (the ring's length).
    history_ms: f64,
    /// The engine's respiratory clock, handed to every synthesiser (B28 breath-hold).
    respiration: RespiratoryClock,
    /// Small-speaker mode (B28): the altered synthesiser and its ring, `None` when off.
    altered: Option<(PcgSynth, LeadRing)>,
}

impl HeartSounds {
    /// A ring of `seconds` (the strip rings' length) and a synthesiser for the default engine.
    #[must_use]
    pub fn new(seconds: f32) -> Self {
        let respiration_rate_bpm = heartsim_sim::timing::SinusParams::default().respiration_rate_bpm;
        let mut synth = PcgSynth::new(PcgParams::default(), 0, respiration_rate_bpm);
        synth.keep_beats(true);
        Self {
            synth,
            ring: LeadRing::new((PCG_RATE_HZ * seconds) as usize, PCG_RATE_HZ),
            marks: Vec::new(),
            last_t_ms: None,
            seed: 0,
            respiration_rate_bpm,
            auscultation: Auscultation::default(),
            generation: 0,
            wall: None,
            choice: ListeningChoice::default(),
            listening: Listening::overview(),
            history_ms: f64::from(seconds) * 1000.0,
            respiration: RespiratoryClock::default(),
            altered: None,
        }
    }

    /// Start over for a new engine (a rhythm loaded, the EP lab or a re-entry preset started): a
    /// fresh synthesiser with the scenario's seed, breathing rate and `auscultation` section (empty
    /// for the EP lab and the re-entry presets), the ring and labels cleared. The ring keeps
    /// counting, as the strip rings do, so the two stay in step.
    pub fn reset(&mut self, seed: u64, respiration_rate_bpm: f64, auscultation: &Auscultation) {
        self.seed = seed;
        self.respiration_rate_bpm = respiration_rate_bpm;
        self.auscultation.clone_from(auscultation);
        self.synth = PcgSynth::new(PcgParams::default(), seed, respiration_rate_bpm)
            .with_auscultation(&self.auscultation);
        self.synth.keep_beats(true);
        self.synth.set_listening(&self.listening);
        self.ring.clear();
        self.marks.clear();
        self.last_t_ms = None;
        self.generation += 1;
        // a new engine breathes freely until it is told otherwise
        self.respiration = RespiratoryClock::default();
        if self.wall.is_some() {
            self.wall = Some(self.new_wall());
        }
        if self.altered.is_some() {
            self.altered = Some(self.new_altered());
        }
    }

    fn new_wall(&self) -> WallTimeSynth {
        let mut wall = WallTimeSynth::new(PcgParams::default(), self.seed, self.respiration_rate_bpm)
            .with_auscultation(&self.auscultation);
        wall.set_listening(&self.listening);
        wall.set_respiration(self.respiration);
        if self.altered.is_some() {
            wall.set_frequency_factor(SMALL_SPEAKER_FACTOR);
        }
        wall
    }

    fn new_altered(&self) -> (PcgSynth, LeadRing) {
        let mut synth = PcgSynth::new(PcgParams::default(), self.seed, self.respiration_rate_bpm)
            .with_auscultation(&self.auscultation);
        synth.set_listening(&self.listening);
        synth.set_respiration(self.respiration);
        synth.set_frequency_factor(SMALL_SPEAKER_FACTOR);
        let ring = LeadRing::starting_at(self.ring.capacity(), PCG_RATE_HZ, self.ring.pushed());
        (synth, ring)
    }

    /// Small-speaker mode on or off (B28): while on, the sound is synthesised with every frequency
    /// multiplied by [`SMALL_SPEAKER_FACTOR`] into [`Self::audio_ring`]; the lane's ring is untouched.
    /// Switching on starts the altered synthesiser fresh: it hears the events from then on.
    pub fn set_small_speaker(&mut self, on: bool) {
        if on == self.altered.is_some() {
            return;
        }
        self.altered = on.then(|| self.new_altered());
        let factor = if on { SMALL_SPEAKER_FACTOR } else { 1.0 };
        if let Some(w) = &mut self.wall {
            w.set_frequency_factor(factor);
        }
    }

    /// Whether small-speaker mode is on.
    #[must_use]
    pub fn small_speaker(&self) -> bool {
        self.altered.is_some()
    }

    /// The ring the sound is played from: the altered one in small-speaker mode, else the lane's.
    /// Both carry the same absolute indices.
    #[must_use]
    pub fn audio_ring(&self) -> &LeadRing {
        self.altered.as_ref().map_or(&self.ring, |(_, r)| r)
    }

    /// Follow the engine's respiratory clock (`TimingEngine::respiration`, B28) in every
    /// synthesiser. Called after every engine step; an unchanged clock changes nothing.
    pub fn set_respiration(&mut self, clock: RespiratoryClock) {
        if clock == self.respiration {
            return;
        }
        self.respiration = clock;
        self.synth.set_respiration(clock);
        if let Some(w) = &mut self.wall {
            w.set_respiration(clock);
        }
        if let Some((a, _)) = &mut self.altered {
            a.set_respiration(clock);
        }
    }

    /// Listen at `choice` (B27 part B) from the next sample on, in the trace and in the slow-motion
    /// synthesiser alike. The ring keeps what it already holds. Setting the same choice changes
    /// nothing.
    pub fn set_listening(&mut self, choice: ListeningChoice) {
        if choice == self.choice {
            return;
        }
        self.choice = choice;
        self.listening = SiteTable::builtin().listening(choice);
        self.synth.set_listening(&self.listening);
        if let Some(w) = &mut self.wall {
            w.set_listening(&self.listening);
        }
        if let Some((a, _)) = &mut self.altered {
            a.set_listening(&self.listening);
        }
    }

    /// Engine time of the newest sample in the ring, ms (`None` before the first).
    #[must_use]
    pub fn newest_ms(&self) -> Option<f64> {
        self.last_t_ms
    }

    /// The listening site and chest piece in effect.
    #[must_use]
    pub fn listening(&self) -> ListeningChoice {
        self.choice
    }

    /// Highlight level of each valve (`heartsim_ecg::Valve::ALL` order, `0..=1`) at simulation time
    /// `t_ms`, from the schedule the trace and the sound are synthesised from: a flash at every
    /// closure, a glow over each valve murmur. The caller passes the time the views are drawn at
    /// (the engine's minus the display delay and the scrub).
    #[must_use]
    pub fn valve_glow_at(&self, t_ms: f64) -> [f32; 4] {
        heartsim_ecg::valve_glow_at(
            self.synth.scheduled_sounds(),
            self.synth.scheduled_murmurs(),
            t_ms,
        )
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
        if let Some((a, _)) = &mut self.altered {
            a.on_events(events);
        }
    }

    /// Push the [`PCG_PER_ECG_SAMPLE`] samples that end at `now`, the time of the strip sample just
    /// pushed, so PCG sample `8k + 7` and strip sample `k` are taken at the same instant.
    pub fn push_for_ecg_sample(&mut self, now: Millis) {
        if self.last_t_ms.is_some_and(|t| now.0 < t) {
            // the engine's clock went back without a reset (a new engine): start over
            let (seed, rate) = (self.seed, self.respiration_rate_bpm);
            let auscultation = std::mem::take(&mut self.auscultation);
            self.reset(seed, rate, &auscultation);
        }
        for i in (0..PCG_PER_ECG_SAMPLE).rev() {
            let t = now.0 - i as f64 * PCG_PERIOD_MS;
            self.ring.push(self.synth.sample(Millis(t)));
            if let Some((a, ring)) = &mut self.altered {
                ring.push(a.sample(Millis(t)));
            }
        }
        self.last_t_ms = Some(now.0);
        if self.ring.pushed().is_multiple_of(PCG_RATE_HZ as u64) {
            // once a second of PCG: the schedule older than the ring is of no use to the views
            self.synth.forget_before(now.0 - self.history_ms);
        }
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

/// The "altered sound" tag the lane shows while what is heard differs from the trace drawn (B28):
/// small-speaker mode whenever the heart-sound audio is on (playing, paused, a slice), slow motion,
/// or both; `None` when the sound is the trace or there is no sound.
#[must_use]
pub fn altered_sound_tag(status: heartsim_ui::HeartSoundStatus, small_speaker: bool) -> Option<String> {
    use heartsim_ui::{HeartSoundStatus, t};
    let silent = matches!(
        status,
        HeartSoundStatus::Off | HeartSoundStatus::NoDevice | HeartSoundStatus::Unsupported
    );
    match (status, small_speaker) {
        (HeartSoundStatus::SlowMotion, true) => Some(format!(
            "{} · {}",
            t!("ecg-sound-altered-slow"),
            t!("ecg-sound-altered-small")
        )),
        (HeartSoundStatus::SlowMotion, false) => Some(t!("ecg-sound-altered-slow")),
        (_, true) if !silent => Some(t!("ecg-sound-altered-small")),
        _ => None,
    }
}

/// The simulation time the valve highlights are drawn for: the newest heart-sound sample's time
/// minus the views' offset (the display delay the sound is heard with, plus the scrub), so a ring
/// flashes when its closure is heard and drawn on the lane.
#[must_use]
pub fn valve_display_time_ms(newest_ms: f64, view_offset_ms: f64) -> f64 {
    newest_ms - view_offset_ms.max(0.0)
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
