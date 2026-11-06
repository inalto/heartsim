//! The display stage (requirement F-15): the bandwidth filter and the artefacts the user switches
//! on for the strip and the 12-lead page.
//!
//! **A display stage, nothing more.** [`DisplayStage`] reads the unfiltered rings the simulation
//! fills and writes *separate* rings that only the painters read. The engine state, the analysis
//! (intervals, axis, wave marks), the calipers' wave snapping and the default exports keep using
//! the raw rings, so they are unfiltered and deterministic whatever the display shows. The stage
//! itself is deterministic too: its output is a function of the raw samples, the filter mode and
//! the artefact seed, so rebuilding it (after a switch, or when a view is scrubbed back) draws the
//! same picture.
//!
//! Order of operations, as on an acquisition front end: artefacts are added to the ECG first
//! (respiration also modulates its amplitude), then the bandwidth filter acts on the sum. That is
//! why the 0.5 Hz monitor filter removes baseline wander and the 40 Hz one softens muscle tremor,
//! while 50/60 Hz hum passes through both (see [`crate::filter`]).

use crate::artefact::{ArtefactConfig, ArtefactGenerator};
use crate::filter::{BandFilter, FilterMode};
use crate::ring::LeadRing;
use serde::{Deserialize, Serialize};

/// What the display does to the ECG.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Bandwidth filter.
    pub filter: FilterMode,
    /// Artefacts added before the filter.
    pub artefacts: ArtefactConfig,
}

impl DisplayConfig {
    /// True when the display shows the signal untouched.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.filter == FilterMode::Off && !self.artefacts.any()
    }

    /// The statement an export or a snapshot carries when it shows the processed trace, in plain
    /// ASCII, e.g. `"Display filter: monitor 0.5-40 Hz. Artefacts added: 50 Hz mains (intensity
    /// 1.0, seed 7)."`; `None` when the trace is the unfiltered engine signal.
    #[must_use]
    pub fn describe(&self) -> Option<String> {
        if self.is_identity() {
            return None;
        }
        let mut s = format!("Display filter: {}.", self.filter.description());
        if let Some(a) = self.artefacts.describe() {
            s.push_str(" Artefacts added: ");
            s.push_str(&a);
            s.push('.');
        }
        Some(s)
    }
}

/// Seconds of history rebuilt when the stage is (re)started: the whole 60 s ring of the strip, so
/// scrubbing back shows the processed trace as far as the raw one goes (a switch costs a few tens of
/// milliseconds once).
pub const REBUILD_S: f32 = 60.0;

/// Filtered / artefact-laden copies of the 12 strip rings, kept in step with the raw ones.
#[derive(Debug, Clone)]
pub struct DisplayStage {
    config: DisplayConfig,
    generator: ArtefactGenerator,
    filters: Vec<BandFilter>,
    rings: Vec<LeadRing>,
    /// The rings no longer correspond to the config or the raw signal: rebuild on the next sync.
    stale: bool,
}

impl Default for DisplayStage {
    fn default() -> Self {
        Self::new(DisplayConfig::default())
    }
}

impl DisplayStage {
    /// A stage with `config`; it holds no samples until [`Self::sync`].
    #[must_use]
    pub fn new(config: DisplayConfig) -> Self {
        Self {
            config,
            generator: ArtefactGenerator::new(config.artefacts),
            filters: Vec::new(),
            rings: Vec::new(),
            stale: true,
        }
    }

    /// The current configuration.
    #[must_use]
    pub fn config(&self) -> &DisplayConfig {
        &self.config
    }

    /// Change the configuration; the display is rebuilt on the next [`Self::sync`] if it differs.
    pub fn set_config(&mut self, config: DisplayConfig) {
        if config != self.config {
            self.config = config;
            self.generator = ArtefactGenerator::new(config.artefacts);
            self.stale = true;
        }
    }

    /// The raw rings were replaced (a recording swapped in or out): forget the derived ones.
    pub fn invalidate(&mut self) {
        self.stale = true;
    }

    /// True when the display differs from the raw signal.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.config.is_identity()
    }

    /// The rings to draw: the processed ones, or `raw` itself when the display is untouched (or
    /// the stage has not caught up yet). Same length and absolute sample indices as `raw`.
    #[must_use]
    pub fn rings<'a>(&'a self, raw: &'a [LeadRing]) -> &'a [LeadRing] {
        if self.is_active() && !self.stale && self.rings.len() == raw.len() {
            &self.rings
        } else {
            raw
        }
    }

    /// Bring the processed rings up to the newest raw sample. Call once per frame, before drawing.
    /// Cheap when nothing changed; a switch or a swapped signal rebuilds [`REBUILD_S`] seconds.
    pub fn sync(&mut self, raw: &[LeadRing]) {
        if !self.is_active() || raw.is_empty() {
            self.rings.clear();
            self.filters.clear();
            self.stale = true;
            return;
        }
        let total = raw[0].pushed();
        let fs = raw[0].sample_rate_hz;
        let continues = !self.stale
            && self.rings.len() == raw.len()
            && self.rings.first().is_some_and(|d| {
                (d.sample_rate_hz - fs).abs() < 1e-3
                    && d.pushed() <= total
                    // the raw samples we still owe are still held, and the raw ring has not been
                    // cleared (a cleared one holds fewer samples than we do)
                    && raw[0].oldest_index() <= d.pushed()
                    && raw[0].len() >= d.len()
            });
        if continues {
            let from = self.rings[0].pushed();
            self.advance(raw, from, total);
        } else {
            self.rebuild(raw);
        }
    }

    fn rebuild(&mut self, raw: &[LeadRing]) {
        let fs = raw[0].sample_rate_hz;
        let total = raw[0].pushed();
        let want = (REBUILD_S * fs) as u64;
        let start = total.saturating_sub(want).max(raw[0].oldest_index());
        self.filters = raw
            .iter()
            .map(|_| BandFilter::new(self.config.filter, fs))
            .collect();
        self.rings = raw
            .iter()
            .map(|r| LeadRing::starting_at(r.capacity(), fs, start))
            .collect();
        self.stale = false;
        self.advance(raw, start, total);
    }

    /// Process raw samples `from..to` of every lead and append them.
    fn advance(&mut self, raw: &[LeadRing], from: u64, to: u64) {
        let mut buf = Vec::new();
        for (lead, (r, out)) in raw.iter().zip(self.rings.iter_mut()).enumerate() {
            let fs = r.sample_rate_hz;
            let first = r.range(from, to, &mut buf);
            for (k, &x) in buf.iter().enumerate() {
                let n = first + k as u64;
                let v = self.generator.apply(lead, n, fs, x);
                out.push(self.filters[lead].process(v));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artefact::MainsFrequency;
    use core::f32::consts::TAU;

    const FS: f32 = 500.0;

    /// Twelve raw rings holding a 1.2 Hz test waveform of `n` samples.
    fn raw_rings(n: usize) -> Vec<LeadRing> {
        (0..12)
            .map(|lead| {
                let mut r = LeadRing::new(30_000, FS);
                for i in 0..n {
                    let t = i as f32 / FS;
                    r.push(f32::midpoint((TAU * 1.2 * t).sin(), 0.2 * lead as f32));
                }
                r
            })
            .collect()
    }

    fn mains_config() -> DisplayConfig {
        DisplayConfig {
            filter: FilterMode::Monitor,
            artefacts: ArtefactConfig {
                mains: true,
                mains_hz: MainsFrequency::Hz50,
                muscle: true,
                seed: 11,
                ..ArtefactConfig::default()
            },
        }
    }

    fn contents(r: &LeadRing) -> Vec<f32> {
        r.iter().collect()
    }

    #[test]
    fn an_untouched_display_is_the_raw_ring_itself() {
        let raw = raw_rings(3000);
        let mut stage = DisplayStage::default();
        stage.sync(&raw);
        assert!(!stage.is_active());
        assert!(std::ptr::eq(stage.rings(&raw).as_ptr(), raw.as_ptr()));
        assert_eq!(stage.config().describe(), None);
    }

    #[test]
    fn the_raw_rings_are_never_modified() {
        let raw = raw_rings(5000);
        let before: Vec<Vec<f32>> = raw.iter().map(contents).collect();
        let mut stage = DisplayStage::new(mains_config());
        stage.sync(&raw);
        let shown = stage.rings(&raw);
        assert!(!std::ptr::eq(shown.as_ptr(), raw.as_ptr()));
        let after: Vec<Vec<f32>> = raw.iter().map(contents).collect();
        assert_eq!(before, after);
        // and the processed ring differs from the raw one
        assert_ne!(contents(&shown[1]), before[1]);
    }

    #[test]
    fn processed_rings_line_up_with_the_raw_ones() {
        let raw = raw_rings(5000);
        let mut stage = DisplayStage::new(mains_config());
        stage.sync(&raw);
        let shown = stage.rings(&raw);
        for (d, r) in shown.iter().zip(&raw) {
            assert_eq!(d.pushed(), r.pushed());
            assert!(d.oldest_index() >= r.oldest_index());
            assert!(d.at(r.pushed() - 1).is_some());
        }
    }

    #[test]
    fn growing_incrementally_equals_building_at_once() {
        let mut raw = raw_rings(4000);
        let mut stage = DisplayStage::new(mains_config());
        stage.sync(&raw);
        // the simulation pushes a few samples every frame
        for step in 0..30usize {
            for (lead, r) in raw.iter_mut().enumerate() {
                for i in 0..17usize {
                    let t = (4000 + step * 17 + i) as f32 / FS;
                    r.push(f32::midpoint((TAU * 1.2 * t).sin(), 0.2 * lead as f32));
                }
            }
            stage.sync(&raw);
        }
        let mut fresh = DisplayStage::new(mains_config());
        fresh.sync(&raw);
        for (a, b) in stage.rings(&raw).iter().zip(fresh.rings(&raw)) {
            assert_eq!(a.pushed(), b.pushed());
            // both start at sample 0 here (under REBUILD_S), so every sample is identical
            assert_eq!(contents(a), contents(b));
        }
    }

    #[test]
    fn switching_the_config_rebuilds_and_switching_off_returns_to_raw() {
        let raw = raw_rings(3000);
        let mut stage = DisplayStage::new(mains_config());
        stage.sync(&raw);
        let monitor = contents(&stage.rings(&raw)[0]);
        stage.set_config(DisplayConfig {
            filter: FilterMode::Diagnostic,
            ..mains_config()
        });
        stage.sync(&raw);
        assert_ne!(contents(&stage.rings(&raw)[0]), monitor);
        stage.set_config(DisplayConfig::default());
        stage.sync(&raw);
        assert!(std::ptr::eq(stage.rings(&raw).as_ptr(), raw.as_ptr()));
    }

    #[test]
    fn the_same_seed_draws_the_same_artefacts_after_a_rebuild() {
        let raw = raw_rings(3000);
        let mut a = DisplayStage::new(mains_config());
        a.sync(&raw);
        let first = contents(&a.rings(&raw)[3]);
        a.invalidate();
        a.sync(&raw);
        assert_eq!(first, contents(&a.rings(&raw)[3]));
        let mut other_seed = mains_config();
        other_seed.artefacts.seed = 12;
        let mut b = DisplayStage::new(other_seed);
        b.sync(&raw);
        assert_ne!(first, contents(&b.rings(&raw)[3]));
    }

    #[test]
    fn a_cleared_raw_ring_is_not_shown_stale() {
        let mut raw = raw_rings(3000);
        let mut stage = DisplayStage::new(mains_config());
        stage.sync(&raw);
        raw.iter_mut().for_each(LeadRing::clear);
        stage.sync(&raw);
        assert_eq!(stage.rings(&raw)[0].len(), 0, "old rhythm still on the strip");
        // new samples arrive and are shown
        for r in &mut raw {
            (0..100).for_each(|_| r.push(0.1));
        }
        stage.sync(&raw);
        assert_eq!(stage.rings(&raw)[0].len(), 100);
    }

    #[test]
    fn the_filter_removes_baseline_wander_but_keeps_the_waveform() {
        // 1.2 Hz "beats" on a 0.3 mV drift at 0.1 Hz: the monitor band removes the drift
        let mut raw: Vec<LeadRing> = (0..12).map(|_| LeadRing::new(30_000, FS)).collect();
        for r in &mut raw {
            for i in 0..10_000 {
                let t = i as f32 / FS;
                r.push((TAU * 1.2 * t).sin() * 0.5 + 0.3 * (TAU * 0.1 * t).sin());
            }
        }
        let mut stage = DisplayStage::new(DisplayConfig {
            filter: FilterMode::Monitor,
            artefacts: ArtefactConfig::default(),
        });
        stage.sync(&raw);
        let shown = contents(&stage.rings(&raw)[0]);
        let tail = &shown[shown.len() - 5000..];
        let mean = tail.iter().sum::<f32>() / tail.len() as f32;
        let peak = tail.iter().fold(0.0f32, |m, v| m.max((v - mean).abs()));
        assert!(mean.abs() < 0.05, "drift left: {mean}");
        assert!(
            (0.4..0.55).contains(&peak),
            "the 1.2 Hz wave should survive: {peak}"
        );
    }

    #[test]
    fn description_states_filter_and_artefacts() {
        let d = mains_config().describe().unwrap();
        assert!(d.starts_with("Display filter: monitor 0.5-40 Hz."), "{d}");
        assert!(d.contains("50 Hz mains") && d.contains("muscle tremor") && d.contains("seed 11"));
        assert!(d.is_ascii());
        let only_filter = DisplayConfig {
            filter: FilterMode::Diagnostic,
            ..DisplayConfig::default()
        };
        assert_eq!(
            only_filter.describe().as_deref(),
            Some("Display filter: diagnostic 0.05-150 Hz.")
        );
        let only_noise = DisplayConfig {
            artefacts: ArtefactConfig {
                baseline_wander: true,
                ..ArtefactConfig::default()
            },
            ..DisplayConfig::default()
        };
        assert!(only_noise.describe().unwrap().starts_with("Display filter: off."));
    }
}
