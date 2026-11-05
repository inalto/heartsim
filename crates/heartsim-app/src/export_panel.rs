//! The app's side of exports and imports (brief B20): cutting the export excerpt from the strip
//! rings, the scenario file of the running rhythm, and the state of the Export / Import window.
//! The window itself is drawn by `app::exports`; everything here is free of the app model so the
//! CLI parity test can call exactly what the Export button calls.

use crate::scenarios::ScenarioLibrary;
use heartsim_core::{Lead, Scenario};
use heartsim_ecg::{GaussianSynth, LeadRing, MorphologyOverrides, Tier0Scenario};
use heartsim_io::{EcgRecord, ExportFile, ExportInfo, ExportOptions, Formats, Imported, Paper, export_files};
use heartsim_sim::TimingParams;
use heartsim_ui::t;
use heartsim_ui::{LeadOrder, PaceMarks, StripStyle};
use std::path::PathBuf;

/// Default export length, s (one 25 mm/s page).
pub const DEFAULT_WINDOW_S: f64 = 10.0;

/// Simulation time of the sample at absolute ring index `head - 1`. The strip samples at the end
/// of every `1000 / fs` ms of simulation (2, 4, 6 … ms after a scenario starts); `now_ms` is the
/// engine time, `pushed` the rings' sample count.
#[must_use]
pub fn newest_sample_ms(now_ms: f64, pushed: u64, head: u64, sample_rate_hz: f64) -> f64 {
    let period_ms = 1000.0 / sample_rate_hz;
    (now_ms / period_ms).floor() * period_ms - pushed.saturating_sub(head) as f64 * period_ms
}

/// The scenario file for the slider settings ("Custom"): the healthy default with the running
/// timing parameters and the synthesiser's axis and QRS-width knobs.
#[must_use]
pub fn custom_scenario(timing: &TimingParams, synth: &GaussianSynth) -> Tier0Scenario {
    Tier0Scenario {
        scenario: Scenario {
            id: "custom".into(),
            title: "Custom (sliders)".into(),
            description: "Exported from the heartsim sliders.".into(),
            tier: heartsim_core::scenario::EngineTier::Template,
            ..Scenario::sinus_normal()
        },
        timing: timing.clone(),
        morphology: MorphologyOverrides {
            axis_shift_deg: synth.axis_shift_deg,
            qrs_width_scale: synth.qrs_width_scale,
            ..MorphologyOverrides::default()
        },
    }
}

/// Everything an export needs besides the rings.
#[derive(Debug, Clone)]
pub struct ExportRequest<'a> {
    pub scenario: &'a Tier0Scenario,
    /// Paper speed and gain are taken from here (the screen's strip settings).
    pub style: &'a StripStyle,
    pub order: LeadOrder,
    pub strip_lead: Lead,
    pub window_s: f64,
    pub paper: Paper,
    pub formats: Formats,
    pub date: String,
}

impl ExportRequest<'_> {
    /// Header information of the export.
    #[must_use]
    pub fn info(&self) -> ExportInfo {
        ExportInfo {
            scenario_id: self.scenario.scenario.id.clone(),
            scenario_title: self.scenario.scenario.title.clone(),
            mm_per_s: self.style.mm_per_s,
            mm_per_mv: self.style.mm_per_mv,
            date: self.date.clone(),
            paper: self.paper,
        }
    }
}

/// The files of the Export button: the last `req.window_s` before ring index `head`, whose sample
/// `head - 1` was taken at `newest_ms`.
pub fn export_from_rings(
    rings: &[LeadRing],
    head: u64,
    newest_ms: f64,
    req: &ExportRequest<'_>,
) -> Result<Vec<ExportFile>, heartsim_io::bundle::ExportError> {
    let opts = ExportOptions {
        formats: req.formats,
        order: req.order,
        strip_lead: req.strip_lead,
    };
    let record = EcgRecord::from_rings(rings, head, req.window_s, newest_ms);
    export_files(&record, &req.info(), req.scenario, &opts)
}

/// Put a scenario file into the library (replacing the one with the same id, for this session) so
/// the picker and `load_scenario` can run it. Other entries keep their indices. Returns its index.
pub fn add_to_library(library: &mut ScenarioLibrary, s: Tier0Scenario) -> usize {
    if let Some(i) = library.index_of(&s.scenario.id) {
        library.entries[i] = s;
        i
    } else {
        library.entries.push(s);
        library.len() - 1
    }
}

/// Where exports go by default: `Documents/heartsim`, else `heartsim-exports` in the working
/// directory.
#[must_use]
pub fn default_export_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|h| PathBuf::from(h).join("Documents"))
        .filter(|d| d.is_dir())
        .map_or_else(|| PathBuf::from("heartsim-exports"), |d| d.join("heartsim"))
}

/// Minimum ring capacity while reviewing, samples (60 s at 500 Hz, the live history).
const REVIEW_MIN_CAPACITY: usize = 30_000;

/// A recording under review, and the live state it displaced: the strip rings and the pacing marks,
/// whose sample indices belong to those rings and would draw false spikes over the recording.
#[derive(Debug)]
pub struct Review {
    pub label: String,
    pub duration_s: f64,
    live_rings: Vec<LeadRing>,
    live_marks: PaceMarks,
}

impl Review {
    /// Swap `imported` into `rings`. A review already under way keeps the original live state.
    pub fn begin(
        rings: &mut Vec<LeadRing>,
        marks: &mut PaceMarks,
        previous: Option<Review>,
        imported: &Imported,
    ) -> Self {
        let displaced = std::mem::replace(rings, imported.record.to_rings(REVIEW_MIN_CAPACITY));
        let displaced_marks = std::mem::take(marks);
        let (live_rings, live_marks) = match previous {
            Some(p) => (p.live_rings, p.live_marks),
            None => (displaced, displaced_marks),
        };
        Self {
            label: imported.label.clone(),
            duration_s: imported.record.duration_s(),
            live_rings,
            live_marks,
        }
    }

    /// Give the live state back. Empty `rings` mean a scenario was loaded during the review (a
    /// recording is never empty): it has restarted the clock, so the old samples are history of a
    /// rhythm no longer running and are dropped, as `load_scenario` drops them outside a review.
    pub fn end(self, rings: &mut Vec<LeadRing>, marks: &mut PaceMarks) {
        let restarted = rings.first().is_some_and(LeadRing::is_empty);
        *rings = self.live_rings;
        *marks = self.live_marks;
        if restarted {
            rings.iter_mut().for_each(LeadRing::clear);
        }
    }
}

/// State of the Export / Import window.
#[derive(Debug)]
pub struct ExportPanel {
    pub open: bool,
    pub dir: String,
    pub window_s: f64,
    pub paper: Paper,
    pub formats: Formats,
    pub import_path: String,
    /// Outcome of the last action, for the status line.
    pub status: Option<String>,
    pub review: Option<Review>,
    /// Result of a window screenshot, filled by the screenshot callback.
    pub screenshot: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

impl Default for ExportPanel {
    fn default() -> Self {
        Self {
            open: false,
            dir: default_export_dir().display().to_string(),
            window_s: DEFAULT_WINDOW_S,
            paper: Paper::A4,
            formats: Formats::default(),
            import_path: String::new(),
            status: None,
            review: None,
            screenshot: std::sync::Arc::default(),
        }
    }
}

impl ExportPanel {
    /// Status text for a finished export.
    #[must_use]
    pub fn describe(written: &[PathBuf]) -> String {
        let names: Vec<String> = written
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        match written.first().and_then(|p| p.parent()) {
            Some(dir) => t!(
                "export-wrote",
                names = names.join(", "),
                dir = dir.display().to_string()
            ),
            None => t!("export-nothing"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_sample_follows_the_strip_sampling() {
        // 12 000 steps of 1 ms: the newest sample is at 12 000 ms; scrubbed 500 samples back it is 11 000 ms
        assert!((newest_sample_ms(12_000.0, 6_000, 6_000, 500.0) - 12_000.0).abs() < 1e-9);
        assert!((newest_sample_ms(12_001.0, 6_000, 5_500, 500.0) - 11_000.0).abs() < 1e-9);
    }

    fn rings_of(value: f32, n: usize) -> Vec<LeadRing> {
        (0..12)
            .map(|_| {
                let mut r = LeadRing::new(100, 500.0);
                (0..n).for_each(|_| r.push(value));
                r
            })
            .collect()
    }

    fn imported(value: f32) -> Imported {
        let mut record = EcgRecord::empty(250.0);
        record.leads.iter_mut().for_each(|l| *l = vec![value; 40]);
        Imported {
            record,
            label: "test.csv".into(),
        }
    }

    #[test]
    fn review_swaps_the_recording_in_and_the_live_rings_back() {
        let mut rings = rings_of(1.0, 50);
        let mut marks = PaceMarks::default();
        let first = Review::begin(&mut rings, &mut marks, None, &imported(2.0));
        assert_eq!(rings[0].at(0), Some(2.0));
        assert!((rings[0].sample_rate_hz - 250.0).abs() < 1e-6);
        assert!((first.duration_s - 0.16).abs() < 1e-9);
        // a second import while reviewing must not lose the live rings
        let second = Review::begin(&mut rings, &mut marks, Some(first), &imported(3.0));
        assert_eq!(rings[0].at(0), Some(3.0));
        second.end(&mut rings, &mut marks);
        assert_eq!(rings[0].pushed(), 50);
        assert_eq!(rings[0].at(49), Some(1.0));
        assert!((rings[0].sample_rate_hz - 500.0).abs() < 1e-6);
    }

    #[test]
    fn loading_a_scenario_during_review_drops_the_old_samples() {
        let mut rings = rings_of(1.0, 50);
        let mut marks = PaceMarks::default();
        let review = Review::begin(&mut rings, &mut marks, None, &imported(2.0));
        rings.iter_mut().for_each(LeadRing::clear); // what load_scenario does
        review.end(&mut rings, &mut marks);
        assert!(rings[0].is_empty());
        assert_eq!(rings[0].pushed(), 50, "time never runs backwards");
    }

    #[test]
    fn scenario_files_join_the_library_without_moving_entries() {
        let mut lib = ScenarioLibrary::default();
        let scenario = |id: &str, bpm: f64| {
            let mut s = custom_scenario(&TimingParams::default(), &GaussianSynth::textbook());
            s.scenario.id = id.into();
            s.timing.sinus.rate_bpm = bpm;
            s
        };
        assert_eq!(add_to_library(&mut lib, scenario("a", 60.0)), 0);
        assert_eq!(add_to_library(&mut lib, scenario("b", 70.0)), 1);
        assert_eq!(add_to_library(&mut lib, scenario("a", 90.0)), 0);
        assert_eq!(lib.len(), 2);
        assert!((lib.entries[0].timing.sinus.rate_bpm - 90.0).abs() < 1e-12);
    }

    #[test]
    fn custom_scenario_round_trips_the_sliders() {
        let mut timing = TimingParams::default();
        timing.sinus.rate_bpm = 95.0;
        let mut synth = GaussianSynth::textbook();
        synth.axis_shift_deg = -40.0;
        let s = custom_scenario(&timing, &synth);
        let text = heartsim_io::scenario_file::scenario_ron(&s).unwrap();
        let back = Tier0Scenario::from_ron(&text).unwrap();
        assert!((back.timing.sinus.rate_bpm - 95.0).abs() < 1e-12);
        assert!((back.morphology.axis_shift_deg + 40.0).abs() < 1e-12);
        assert_eq!(back.scenario.id, "custom");
    }
}
