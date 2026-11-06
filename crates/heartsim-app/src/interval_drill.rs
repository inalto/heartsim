//! Interval drill (brief B8 §5): measure the PR interval, the QRS duration or the QT interval of a
//! strip with the calipers and enter the result; answers within a tolerance count.
//!
//! The strip is a Tier-0 recording of a scenario of the library (the same recording the rhythm quiz
//! uses), shown in lead II at 50 mm/s so that 1 mm is 20 ms. The reference value is the
//! simulator's own measurement of that strip ([`heartsim_ecg::Analyzer`]), and a question is only
//! asked when that measurement is trustworthy: a regular rhythm, a clean signal, and a value inside
//! the scenario's textbook range (`expected`). After the answer the strip shows the waves the
//! analyser delineated, so the learner sees where the reference was taken.
//!
//! **Tolerances** ([`Interval::tolerance_ms`]) are teaching values, about one 1 mm square at the
//! drill's speed (20 ms): 20 ms for PR and QT, 15 ms for QRS. Reading a tracing by eye with
//! calipers cannot be more exact than the line width and the learner's choice of where a wave
//! starts, so a tolerance of that size separates a careful measurement from a wrong one.

use crate::rhythm_quiz::record_strip;
use crate::scenarios::ScenarioLibrary;
use eframe::egui;
use heartsim_core::{LEAD_COUNT, Lead};
use heartsim_ecg::{Analysis, Analyzer, LeadRing, Regularity, SignalQuality, Tier0Scenario};
use heartsim_ui::annotations::EcgAnnotations;
use heartsim_ui::calipers::{CaliperSet, Playback, draw_calipers, strip_interact};
use heartsim_ui::strip::{StripStyle, StripTrace, SweepMode, draw_strip};
use heartsim_ui::t;

/// Paper speed of the drill strip, mm/s: 1 mm is 20 ms.
pub const DRILL_SPEED_MM_S: f32 = 50.0;
/// Strips tried before a question gives up (some rhythms have no measurable PR or a ragged QT).
const MAX_TRIES: usize = 24;
/// Height of the drill strip, screen points.
const STRIP_HEIGHT: f32 = 280.0;
/// Unit suffix of the measurement box.
const MS: &str = " ms";

/// What is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interval {
    /// Start of the P wave to start of the QRS.
    Pr,
    /// Duration of the QRS complex.
    Qrs,
    /// Start of the QRS to the end of the T wave.
    Qt,
}

impl Interval {
    /// All three, in menu order.
    pub const ALL: [Interval; 3] = [Self::Pr, Self::Qrs, Self::Qt];

    /// Half-width of the accepted answers, ms.
    #[must_use]
    pub const fn tolerance_ms(self) -> f64 {
        match self {
            Self::Pr | Self::Qt => 20.0,
            Self::Qrs => 15.0,
        }
    }

    fn name(self) -> String {
        match self {
            Self::Pr => t!("interval-name-pr"),
            Self::Qrs => t!("interval-name-qrs"),
            Self::Qt => t!("interval-name-qt"),
        }
    }

    fn definition(self) -> String {
        match self {
            Self::Pr => t!("interval-def-pr"),
            Self::Qrs => t!("interval-def-qrs"),
            Self::Qt => t!("interval-def-qt"),
        }
    }
}

/// Whether `entry_ms` is within `tolerance_ms` of `truth_ms`.
#[must_use]
pub fn within_tolerance(truth_ms: f64, entry_ms: f64, tolerance_ms: f64) -> bool {
    (entry_ms - truth_ms).abs() <= tolerance_ms
}

/// The reference value of `interval` on a recorded strip, or `None` if the strip is not a fair
/// question for it (see the module documentation).
#[must_use]
pub fn reference_ms(scenario: &Tier0Scenario, analysis: &Analysis, interval: Interval) -> Option<f64> {
    if analysis.quality != SignalQuality::Good || analysis.regularity != Regularity::Regular {
        return None;
    }
    let expected = scenario.scenario.expected.as_ref();
    let inside =
        |v: f64, range: Option<(f64, f64)>| range.is_none_or(|(lo, hi)| (lo - 10.0..=hi + 10.0).contains(&v));
    match interval {
        Interval::Pr => {
            let v = analysis.pr_ms.filter(|_| analysis.p_wave_present)?;
            // a PR question needs a stated textbook PR range to check the measurement against
            let range = expected?.pr_ms?;
            inside(v, Some(range)).then_some(v)
        }
        Interval::Qrs => {
            let v = analysis.qrs_ms?;
            inside(v, expected.and_then(|e| e.qrs_ms)).then_some(v)
        }
        Interval::Qt => {
            let v = analysis.qt_ms?;
            (200.0..=600.0).contains(&v).then_some(v)
        }
    }
}

/// One question.
#[derive(Debug, Clone, PartialEq)]
pub struct IntervalQuestion {
    /// Library index of the scenario whose strip is shown (named only after the answer).
    pub scenario: usize,
    pub interval: Interval,
    /// The reference value, ms.
    pub truth_ms: f64,
}

/// The graded answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grade {
    pub entry_ms: f64,
    /// `entry − truth`, ms.
    pub error_ms: f64,
    pub correct: bool,
}

/// xorshift64*: deterministic per seed.
fn next_u64(state: &mut u64) -> u64 {
    if *state == 0 {
        *state = 0x9E37_79B9_7F4A_7C15;
    }
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    state.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

fn below(state: &mut u64, n: usize) -> usize {
    (next_u64(state) % n as u64) as usize
}

/// Drill state.
#[derive(Debug)]
pub struct IntervalDrill {
    rng: u64,
    pub question: Option<IntervalQuestion>,
    rings: Vec<LeadRing>,
    annotations: Option<EcgAnnotations>,
    /// The calipers on the strip (ms from its left edge).
    pub calipers: CaliperSet,
    playback: Playback,
    /// The measurement typed or copied from the calipers, ms.
    pub entry_ms: f64,
    /// The graded answer; `Some` = revealed.
    pub grade: Option<Grade>,
    pub asked: u32,
    pub correct: u32,
    pub streak: u32,
    failed: bool,
    analyzer: Analyzer,
}

impl Default for IntervalDrill {
    fn default() -> Self {
        Self::new(0xA076_1D64_78BD_642F)
    }
}

impl IntervalDrill {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            rng: seed,
            question: None,
            rings: Vec::new(),
            annotations: None,
            calipers: CaliperSet::default(),
            playback: Playback::default(),
            entry_ms: 0.0,
            grade: None,
            asked: 0,
            correct: 0,
            streak: 0,
            failed: false,
            analyzer: Analyzer::default(),
        }
    }

    /// Ask a new question: pick scenarios at random until one gives a fair reference value.
    pub fn next_question(&mut self, library: &ScenarioLibrary) {
        self.question = None;
        self.grade = None;
        self.rings.clear();
        self.annotations = None;
        self.calipers.reset();
        self.entry_ms = 0.0;
        self.failed = false;
        if library.is_empty() {
            self.failed = true;
            return;
        }
        for _ in 0..MAX_TRIES {
            let scenario = below(&mut self.rng, library.len());
            let interval = Interval::ALL[below(&mut self.rng, Interval::ALL.len())];
            let entry = &library.entries[scenario];
            let Ok(rings) = record_strip(entry) else { continue };
            let head = rings[0].pushed();
            let annotations = EcgAnnotations::compute(&self.analyzer, &rings, head);
            if let Some(truth_ms) = reference_ms(entry, &annotations.analysis, interval) {
                self.question = Some(IntervalQuestion {
                    scenario,
                    interval,
                    truth_ms,
                });
                self.rings = rings;
                self.annotations = Some(annotations);
                return;
            }
        }
        self.failed = true;
    }

    /// Grade `entry_ms` (once per question).
    pub fn answer(&mut self, entry_ms: f64) -> Option<Grade> {
        let q = self.question.as_ref()?;
        if self.grade.is_some() {
            return None;
        }
        let correct = within_tolerance(q.truth_ms, entry_ms, q.interval.tolerance_ms());
        let grade = Grade {
            entry_ms,
            error_ms: entry_ms - q.truth_ms,
            correct,
        };
        self.asked += 1;
        if correct {
            self.correct += 1;
            self.streak += 1;
        } else {
            self.streak = 0;
        }
        self.grade = Some(grade);
        Some(grade)
    }

    /// The recorded strip (twelve rings at [`RATE_HZ`], empty before the first question).
    #[must_use]
    pub fn rings(&self) -> &[LeadRing] {
        &self.rings
    }

    /// The first caliper pair's interval, ms.
    #[must_use]
    pub fn caliper_ms(&self) -> Option<f64> {
        self.calipers.pairs.first().map(|c| f64::from(c.interval_ms()))
    }

    /// The drill page: the question and controls on the left, the strip on the right.
    pub fn ui(&mut self, ui: &mut egui::Ui, library: &ScenarioLibrary, style: &StripStyle) {
        egui::Panel::left("interval_drill_panel")
            .default_size(380.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui, library));
            });
        egui::CentralPanel::default().show(ui, |ui| self.strip(ui, style));
    }

    fn controls(&mut self, ui: &mut egui::Ui, library: &ScenarioLibrary) {
        ui.horizontal_wrapped(|ui| {
            let label = if self.question.is_some() {
                t!("rq-next")
            } else {
                t!("app-quiz-new")
            };
            if ui
                .add_enabled(!library.is_empty(), egui::Button::new(label))
                .clicked()
            {
                self.next_question(library);
            }
            ui.label(t!(
                "app-quiz-score",
                correct = self.correct.to_string(),
                asked = self.asked.to_string()
            ));
            if self.streak > 1 {
                ui.label(t!("rq-streak", count = self.streak.to_string()));
            }
        });
        ui.separator();
        if library.is_empty() {
            ui.label(t!("rq-no-library"));
            return;
        }
        if self.failed {
            ui.colored_label(egui::Color32::LIGHT_RED, t!("interval-no-strip"));
        }
        let Some(q) = self.question.clone() else {
            if !self.failed {
                ui.label(t!("interval-intro"));
            }
            return;
        };
        ui.add(
            egui::Label::new(t!(
                "interval-task",
                interval = q.interval.name(),
                speed = format!("{DRILL_SPEED_MM_S:.0}")
            ))
            .wrap(),
        );
        ui.add(egui::Label::new(q.interval.definition()).wrap());
        ui.separator();

        ui.horizontal_wrapped(|ui| {
            if ui.button(t!("interval-calipers-place")).clicked() && self.calipers.pairs.is_empty() {
                self.calipers.add_pair(100.0, 300.0);
            }
            if ui.button(t!("interval-calipers-clear")).clicked() {
                self.calipers.reset();
            }
        });
        ui.weak(t!("interval-calipers-hint"));
        if let Some(ms) = self.caliper_ms() {
            ui.label(t!("interval-calipers-reading", ms = format!("{ms:.0}")));
        }
        let can_copy = self.caliper_ms().is_some() && self.grade.is_none();
        if ui
            .add_enabled(can_copy, egui::Button::new(t!("interval-calipers-use")))
            .clicked()
            && let Some(ms) = self.caliper_ms()
        {
            self.entry_ms = ms.round();
        }
        ui.horizontal(|ui| {
            let label = ui.label(t!("interval-your-value"));
            ui.add_enabled(
                self.grade.is_none(),
                egui::DragValue::new(&mut self.entry_ms)
                    .range(0.0..=1500.0)
                    .speed(1.0)
                    .max_decimals(0)
                    .suffix(MS),
            )
            .labelled_by(label.id);
        });
        let can_check = self.grade.is_none() && self.entry_ms > 0.0;
        if ui
            .add_enabled(can_check, egui::Button::new(t!("app-quiz-check")))
            .clicked()
        {
            self.answer(self.entry_ms);
        }
        if let Some(g) = self.grade {
            ui.separator();
            let args_entry = format!("{:.0}", g.entry_ms);
            let args_truth = format!("{:.0}", q.truth_ms);
            let tolerance = format!("{:.0}", q.interval.tolerance_ms());
            if g.correct {
                ui.colored_label(
                    egui::Color32::LIGHT_GREEN,
                    t!(
                        "interval-correct",
                        entry = args_entry,
                        truth = args_truth,
                        tolerance = tolerance
                    ),
                );
            } else {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(t!(
                            "interval-wrong",
                            entry = args_entry,
                            truth = args_truth,
                            off = format!("{:.0}", g.error_ms.abs()),
                            tolerance = tolerance
                        ))
                        .color(egui::Color32::LIGHT_RED),
                    )
                    .wrap(),
                );
            }
            if let Some(entry) = library.entries.get(q.scenario) {
                ui.label(t!("interval-rhythm", title = entry.scenario.title.as_str()));
            }
            ui.add(egui::Label::new(t!("interval-reference-note")).wrap());
        }
    }

    fn strip(&mut self, ui: &mut egui::Ui, style: &StripStyle) {
        if self.rings.len() < LEAD_COUNT {
            return;
        }
        let style = StripStyle {
            mm_per_s: DRILL_SPEED_MM_S,
            mode: SweepMode::Scroll,
            ..*style
        };
        let size = egui::vec2(ui.available_width(), STRIP_HEIGHT.min(ui.available_height()));
        let rect = egui::Rect::from_min_size(ui.cursor().min, size);
        let revealed = self.grade.is_some();
        let ring = &self.rings[Lead::II as usize];
        let id = ui.scope_id().with("interval_drill_strip");
        let trace = StripTrace {
            label: Lead::II.label(),
            invert: false,
            head: ring.pushed(),
            marks: self
                .annotations
                .as_ref()
                .filter(|_| revealed)
                .map(|a| a.lead(Lead::II)),
            show_waves: revealed,
            show_intervals: revealed,
        };
        let win = draw_strip(ui, id, rect, ring, &style, &trace);
        strip_interact(ui, id, &win, ring, &mut self.calipers, &mut self.playback);
        draw_calipers(ui, &win, ring, false, &self.calipers, &style);
        ui.allocate_rect(rect, egui::Sense::hover());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhythm_quiz::RATE_HZ;
    use std::path::Path;

    fn library() -> ScenarioLibrary {
        ScenarioLibrary::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios"))
            .unwrap()
    }

    #[test]
    fn tolerance_is_symmetric_and_inclusive() {
        assert!(within_tolerance(160.0, 180.0, 20.0));
        assert!(within_tolerance(160.0, 140.0, 20.0));
        assert!(!within_tolerance(160.0, 180.5, 20.0));
        assert!(!within_tolerance(160.0, 139.0, 20.0));
    }

    #[test]
    fn tolerances_are_about_one_millimetre_at_the_drill_speed() {
        let mm_ms = 1000.0 / f64::from(DRILL_SPEED_MM_S);
        for i in Interval::ALL {
            let t = i.tolerance_ms();
            assert!(
                (0.5 * mm_ms..=1.5 * mm_ms).contains(&t),
                "{i:?}: {t} ms vs {mm_ms} ms per mm"
            );
        }
    }

    #[test]
    fn every_question_has_a_fair_reference() {
        let lib = library();
        let mut asked = std::collections::BTreeSet::new();
        for seed in 1..=12u64 {
            let mut drill = IntervalDrill::new(seed);
            drill.next_question(&lib);
            let q = drill
                .question
                .clone()
                .unwrap_or_else(|| panic!("seed {seed}: no question"));
            assert_eq!(drill.rings().len(), LEAD_COUNT);
            assert_eq!(drill.rings()[0].sample_rate_hz.to_bits(), RATE_HZ.to_bits());
            let range = match q.interval {
                Interval::Pr => 60.0..=400.0,
                Interval::Qrs => 40.0..=220.0,
                Interval::Qt => 200.0..=600.0,
            };
            assert!(range.contains(&q.truth_ms), "seed {seed}: {q:?}");
            // the reference is what the analyser says about the shown strip
            let a = &drill.annotations.as_ref().unwrap().analysis;
            let again = reference_ms(&lib.entries[q.scenario], a, q.interval).unwrap();
            assert!((again - q.truth_ms).abs() < 1e-9);
            asked.insert(format!("{:?}", q.interval));
        }
        assert!(asked.len() >= 2, "only {asked:?} asked in 12 questions");
    }

    #[test]
    fn grading_counts_once_and_tracks_the_streak() {
        let lib = library();
        let mut drill = IntervalDrill::new(4);
        assert_eq!(drill.answer(100.0), None, "no question yet");
        drill.next_question(&lib);
        let q = drill.question.clone().unwrap();
        let tol = q.interval.tolerance_ms();
        let g = drill.answer(q.truth_ms + tol - 1.0).unwrap();
        assert!(g.correct && (g.error_ms - (tol - 1.0)).abs() < 1e-9);
        assert_eq!(drill.answer(q.truth_ms), None, "scored once");
        assert_eq!((drill.asked, drill.correct, drill.streak), (1, 1, 1));

        drill.next_question(&lib);
        let q = drill.question.clone().unwrap();
        let g = drill
            .answer(q.truth_ms + q.interval.tolerance_ms() + 5.0)
            .unwrap();
        assert!(!g.correct);
        assert_eq!((drill.asked, drill.correct, drill.streak), (2, 1, 0));
    }

    #[test]
    fn the_same_seed_asks_the_same_question() {
        let lib = library();
        let (mut a, mut b) = (IntervalDrill::new(9), IntervalDrill::new(9));
        a.next_question(&lib);
        b.next_question(&lib);
        assert_eq!(a.question, b.question);
    }

    #[test]
    fn caliper_reading_is_the_first_pairs_interval() {
        let mut d = IntervalDrill::default();
        assert_eq!(d.caliper_ms(), None);
        d.calipers.add_pair(120.0, 280.0);
        assert!((d.caliper_ms().unwrap() - 160.0).abs() < 1e-6);
    }
}
