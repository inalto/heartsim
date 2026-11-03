//! Tier-0 scenario runs: a scenario file (`assets/scenarios/*.ron`) drives the timing engine
//! and the Gaussian synthesiser, and the B4 analyser checks the result against the file's
//! `expected` block.
//!
//! The file is one `heartsim_core::Scenario`. Two engine sections ride along in the same text:
//! `timing` (read by [`TimingParams::from_scenario_ron`]) and `morphology`
//! ([`MorphologyOverrides`], read here). Running is a pure function of the file: fixed 1 ms step,
//! no randomness, no I/O.

use crate::analysis::{Analysis, Analyzer};
use crate::synth::{FRIDERICIA_EXPONENT, GaussianSynth, MorphologyOverrides, UnknownWave};
use heartsim_core::scenario::{ExpectedFindings, ScenarioError};
use heartsim_core::{LEAD_COUNT, Lead, Millis, Scenario};
use heartsim_sim::{ActionError, Engine, Event, TimingEngine, TimingParams};
use serde::Deserialize;
use std::fmt::Write as _;

/// Simulation step of scenario runs, ms (the timing engine's resolution).
pub const STEP_MS: f64 = 1.0;
/// Length of the validation run, s: long enough for every preset to settle.
pub const VALIDATION_RUN_S: f64 = 20.0;
/// Analysis window at the end of the validation run, s (the analyser's default window).
pub const VALIDATION_WINDOW_S: f64 = 10.0;
/// Sampling rate the analyser is fed with, Hz.
pub const ANALYSIS_RATE_HZ: f64 = 500.0;

#[derive(Debug, Default, Deserialize)]
#[serde(rename = "Scenario")]
struct MorphologySection {
    #[serde(default)]
    morphology: MorphologyOverrides,
}

/// Why a scenario cannot be loaded or run.
#[derive(Debug)]
pub enum Tier0Error {
    /// The common scenario fields failed to parse (or the version is too new).
    Scenario(ScenarioError),
    /// The `timing` or `morphology` section failed to parse.
    Section(ron::error::SpannedError),
    /// A morphology override names a wave that does not exist.
    Morphology(UnknownWave),
    /// A timeline action the timing engine cannot honour.
    Action(ActionError),
}

impl std::fmt::Display for Tier0Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tier0Error::Scenario(e) => write!(f, "{e}"),
            Tier0Error::Section(e) => write!(f, "engine section: {e}"),
            Tier0Error::Morphology(e) => write!(f, "morphology: {e}"),
            Tier0Error::Action(e) => write!(f, "timeline: {e}"),
        }
    }
}

impl std::error::Error for Tier0Error {}

/// A scenario with its Tier-0 engine sections.
#[derive(Debug, Clone)]
pub struct Tier0Scenario {
    pub scenario: Scenario,
    pub timing: TimingParams,
    pub morphology: MorphologyOverrides,
}

/// Twelve leads sampled every [`STEP_MS`] (oldest first) plus the engine events that made them.
#[derive(Debug, Clone)]
pub struct Recording {
    /// `leads[l][i]` = lead `l` (`Lead as usize`) at `t = (i + 1) · STEP_MS`, mV.
    pub leads: Vec<Vec<f32>>,
    pub events: Vec<Event>,
}

impl Tier0Scenario {
    pub fn from_ron(text: &str) -> Result<Self, Tier0Error> {
        let scenario = Scenario::from_ron(text).map_err(Tier0Error::Scenario)?;
        let timing = TimingParams::from_scenario_ron(text).map_err(Tier0Error::Section)?;
        let morphology = ron::from_str::<MorphologySection>(text)
            .map_err(Tier0Error::Section)?
            .morphology;
        let s = Self {
            scenario,
            timing,
            morphology,
        };
        s.synth().map_err(Tier0Error::Morphology)?;
        Ok(s)
    }

    /// The synthesiser for this scenario: textbook waves, Fridericia QT adaptation, then the
    /// file's morphology overrides.
    pub fn synth(&self) -> Result<GaussianSynth, UnknownWave> {
        let mut s = GaussianSynth::textbook();
        s.qt_rate_exponent = FRIDERICIA_EXPONENT;
        s.apply_overrides(&self.morphology)?;
        Ok(s)
    }

    /// Run the scenario for `seconds`, applying its timeline.
    pub fn record(&self, seconds: f64) -> Result<Recording, Tier0Error> {
        let mut synth = self.synth().map_err(Tier0Error::Morphology)?;
        let mut eng = TimingEngine::new(self.timing.clone());
        let mut timeline = self.scenario.timeline.clone();
        timeline.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
        let mut next = 0;
        let steps = (seconds * 1000.0 / STEP_MS).round() as usize;
        let mut leads: Vec<Vec<f32>> = (0..LEAD_COUNT).map(|_| Vec::with_capacity(steps)).collect();
        let mut events = Vec::new();
        for _ in 0..steps {
            while let Some(a) = timeline.get(next)
                && a.at.0 <= eng.now().0
            {
                eng.apply(&a.action, a.at).map_err(Tier0Error::Action)?;
                next += 1;
            }
            eng.step(Millis(STEP_MS));
            let evs = eng.drain_events();
            synth.on_events(&evs);
            synth.on_pacer_markers(&eng.drain_pacer_markers());
            events.extend(evs);
            let s = synth.sample(eng.now());
            for (buf, v) in leads.iter_mut().zip(s.0) {
                buf.push(v as f32);
            }
        }
        Ok(Recording { leads, events })
    }

    /// Static checks of the teaching content (non-empty texts, ≥ 2 differentials, an
    /// `expected` block). Returns the problems found.
    #[must_use]
    pub fn content_problems(&self) -> Vec<String> {
        let s = &self.scenario;
        let mut p = Vec::new();
        if s.title.trim().is_empty() {
            p.push("empty title".into());
        }
        if s.description.trim().is_empty() {
            p.push("empty description".into());
        }
        if s.notes.as_deref().is_none_or(|n| n.trim().is_empty()) {
            p.push("no teaching notes".into());
        }
        if s.confused_with.len() < 2 {
            p.push(format!(
                "{} confused_with entries (need ≥ 2)",
                s.confused_with.len()
            ));
        }
        if s.expected.is_none() {
            p.push("no expected block".into());
        }
        p
    }

    /// Run [`VALIDATION_RUN_S`], analyse the last [`VALIDATION_WINDOW_S`] and compare with the
    /// `expected` block. Returns the analysis and every violated criterion.
    pub fn validate_run(&self) -> Result<(Analysis, Vec<String>), Tier0Error> {
        let rec = self.record(VALIDATION_RUN_S)?;
        let a = rec.analyze_tail(VALIDATION_WINDOW_S);
        let v = self
            .scenario
            .expected
            .as_ref()
            .map(|e| violations(e, &a))
            .unwrap_or_default();
        Ok((a, v))
    }
}

impl Recording {
    /// Samples per lead.
    #[must_use]
    pub fn len(&self) -> usize {
        self.leads.first().map_or(0, Vec::len)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Lead `l` resampled to `rate_hz` (every n-th sample, aligned so that sample times are
    /// multiples of the new period) over the last `seconds`.
    #[must_use]
    pub fn tail(&self, l: usize, seconds: f64, rate_hz: f64) -> Vec<f32> {
        let every = (1000.0 / rate_hz / STEP_MS).round().max(1.0) as usize;
        let n = self.len();
        let want = (seconds * 1000.0 / STEP_MS).round() as usize;
        let from = n.saturating_sub(want);
        (from..n)
            .filter(|i| (i + 1).is_multiple_of(every))
            .map(|i| self.leads[l][i])
            .collect()
    }

    /// B4 analysis of the last `seconds`, at [`ANALYSIS_RATE_HZ`].
    #[must_use]
    pub fn analyze_tail(&self, seconds: f64) -> Analysis {
        let t: Vec<Vec<f32>> = (0..LEAD_COUNT)
            .map(|l| self.tail(l, seconds, ANALYSIS_RATE_HZ))
            .collect();
        let refs: [&[f32]; LEAD_COUNT] = std::array::from_fn(|i| t[i].as_slice());
        Analyzer::default().analyze(&refs, ANALYSIS_RATE_HZ as f32)
    }

    /// CSV (`t_ms,I,…,V6`) of the last `seconds` at `rate_hz` with `decimals` places, time
    /// relative to the start of the excerpt. Negative zero is printed as zero so the text is
    /// stable across platforms.
    #[must_use]
    pub fn csv_tail(&self, seconds: f64, rate_hz: f64, decimals: usize) -> String {
        let cols: Vec<Vec<f32>> = (0..LEAD_COUNT).map(|l| self.tail(l, seconds, rate_hz)).collect();
        let mut out = String::from("t_ms");
        for l in Lead::ALL {
            out.push(',');
            out.push_str(l.label());
        }
        out.push('\n');
        let period = 1000.0 / rate_hz;
        for i in 0..cols[0].len() {
            let _ = write!(out, "{:.0}", i as f64 * period);
            for c in &cols {
                let s = format!("{:.*}", decimals, c[i]);
                let zero = s.trim_start_matches('-').chars().all(|ch| ch == '0' || ch == '.');
                out.push(',');
                out.push_str(if zero { s.trim_start_matches('-') } else { &s });
            }
            out.push('\n');
        }
        out
    }
}

/// Every criterion of `e` that the analysis `a` violates (a missing measurement violates).
#[must_use]
pub fn violations(e: &ExpectedFindings, a: &Analysis) -> Vec<String> {
    let mut v = Vec::new();
    let mut check = |name: &str, want: Option<(f64, f64)>, got: Option<f64>| {
        if let Some((lo, hi)) = want {
            match got {
                Some(x) if (lo..=hi).contains(&x) => {}
                Some(x) => v.push(format!("{name} {x:.1} outside {lo}..{hi}")),
                None => v.push(format!("{name} not measured (expected {lo}..{hi})")),
            }
        }
    };
    check("HR bpm", Some(e.hr_bpm), a.hr_bpm);
    check("PR ms", e.pr_ms, a.pr_ms);
    check("QRS ms", e.qrs_ms, a.qrs_ms);
    if let Some(r) = e.axis_deg {
        match a.qrs_axis {
            Some(ax) if ExpectedFindings::axis_contains(r, ax.degrees) => {}
            Some(ax) => v.push(format!("axis {:.0}° outside {}..{}", ax.degrees, r.0, r.1)),
            None => v.push(format!("axis not measured (expected {}..{})", r.0, r.1)),
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"Scenario(
        format_version: 1,
        id: "t",
        title: "T",
        description: "d",
        tier: Template,
        timing: TimingParams(sinus: SinusParams(rate_bpm: 60.0)),
        morphology: MorphologyOverrides(waves: [WaveOverride(template: Normal, wave: "R", amplitude_mv: Some(2.0))]),
    )"#;

    #[test]
    fn sections_are_read_from_the_same_text() {
        let s = Tier0Scenario::from_ron(MINIMAL).unwrap();
        assert!((s.timing.sinus.rate_bpm - 60.0).abs() < 1e-12);
        let synth = s.synth().unwrap();
        assert!((synth.ventricular[1].amplitude - 2.0).abs() < 1e-12);
        assert!((synth.qt_rate_exponent - FRIDERICIA_EXPONENT).abs() < 1e-12);
    }

    #[test]
    fn unknown_wave_is_rejected() {
        let bad = MINIMAL.replace("wave: \"R\"", "wave: \"X\"");
        assert!(matches!(
            Tier0Scenario::from_ron(&bad),
            Err(Tier0Error::Morphology(_))
        ));
    }

    #[test]
    fn run_is_deterministic_and_csv_is_resampled() {
        let s = Tier0Scenario::from_ron(MINIMAL).unwrap();
        let a = s.record(3.0).unwrap();
        let b = s.record(3.0).unwrap();
        assert_eq!(a.leads, b.leads);
        let csv = a.csv_tail(2.0, 100.0, 3);
        assert_eq!(csv.lines().count(), 1 + 200);
        assert!(csv.starts_with("t_ms,I,II,III,aVR,aVL,aVF,V1"));
    }
}
