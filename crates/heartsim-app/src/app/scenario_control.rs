//! Scenario loading, the scenario timeline and the per-frame scrub / heart-vector sync of
//! [`HeartSimApp`]. A child module of `app` so it can reach the model's fields while `app.rs`
//! keeps only one-line hooks.

use super::HeartSimApp;
use glam::DVec3;
use heartsim_core::{Millis, SimClock};
use heartsim_sim::{Engine, TimingEngine};

use crate::scenarios::Timeline;
use crate::vector_overlay::dipole_at;

impl HeartSimApp {
    /// Load library scenario `index`: a fresh timing engine and synthesiser from the file, the
    /// 3-D engine re-timed from the same parameters, empty strips, clock at zero (speed kept).
    pub(super) fn load_scenario(&mut self, index: usize) {
        let Some(s) = self.library.entries.get(index) else {
            return;
        };
        let synth = match s.synth() {
            Ok(synth) => synth,
            Err(e) => {
                tracing::warn!("{}: {e}", s.scenario.id);
                return;
            }
        };
        let speed = self.clock.speed;
        self.clock = SimClock::new(Millis(1.0));
        self.clock.speed = speed;
        self.engine = TimingEngine::new(s.timing.clone());
        self.engine.set_shock_seed(s.scenario.seed);
        self.synth = synth;
        self.timeline = Timeline::new(s.scenario.timeline.clone());
        self.sample_accumulator = 0.0;
        self.rings.iter_mut().for_each(heartsim_ecg::LeadRing::clear);
        self.last_v_activation = Millis(-1000.0);
        if let Some(h) = &mut self.heart {
            h.retime(&s.timing);
        }
        if self.ecg.is_paused() {
            self.ecg.toggle_pause();
        }
        self.scenario = Some(index);
    }

    /// Hand the timing engine the scenario actions due now (before the step, as
    /// `Tier0Scenario::record` does).
    pub(super) fn apply_timeline(&mut self) {
        let now = self.engine.now();
        let due: Vec<_> = self.timeline.due(now).cloned().collect();
        for a in due {
            if let Err(e) = self.engine.apply(&a.action, a.at) {
                tracing::warn!("timeline action at {:.0} ms: {e}", a.at.0);
            }
            self.forward_to_heart(&a.action);
        }
    }

    /// Record the heart's colour map for scrubbing (cheap unless a history period has passed).
    pub(super) fn record_heart_history(&mut self) {
        if let Some(h) = &mut self.heart {
            self.heart_history.record(self.engine.now(), h);
        }
    }

    /// While the strip is scrubbed, show the heart as it was at the cursor; live otherwise.
    pub(super) fn sync_scrub(&mut self) {
        let Some(h) = &mut self.heart else { return };
        let back = self.ecg.scrub_offset_ms();
        let cells = (back > 0.0)
            .then(|| Millis(self.engine.now().0 - back))
            .and_then(|t| self.heart_history.at(t, h.mode));
        h.set_scrub_cells(cells);
    }

    /// Heart vector at the strip cursor (the newest sample on screen), mV.
    pub(super) fn cursor_dipole(&self) -> Option<DVec3> {
        let head = self.ecg.head(&self.rings);
        dipole_at(&self.rings, head.checked_sub(1)?, self.vector.solver())
    }

    /// Upload the arrow and loop glyphs for this frame.
    pub(super) fn sync_vector_overlay(&mut self) {
        let dipole = self.cursor_dipole();
        let loop_mv = self
            .ecg
            .analysis()
            .map(|a| a.heart_vector.clone())
            .unwrap_or_default();
        self.vector.sync(dipole, &loop_mv);
    }
}
