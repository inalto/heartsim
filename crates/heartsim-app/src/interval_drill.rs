//! Interval drill (brief B8 §5): measure the PR interval, the QRS duration or the QT interval of a
//! strip with the calipers and enter the result; answers within a tolerance count.
//!
//! The strip is a Tier-0 recording of a scenario of the library (the same recording the rhythm quiz
//! uses), shown in lead II at 50 mm/s so that 1 mm is 20 ms. The strip can be zoomed (25 to
//! 400 mm/s) and panned to place a caliper line precisely; the calipers are stored in
//! milliseconds from the start of the record, so a line stays on its sample through both, and
//! the reading, the grading and the tolerances never depend on the zoom. The reference value is the
//! simulator's own measurement of that strip ([`heartsim_ecg::Analyzer`]), and a question is only
//! asked when that measurement is trustworthy: a regular rhythm, a clean signal, and a value inside
//! the scenario's textbook range (`expected`). After the answer the strip shows the waves the
//! analyser delineated, so the learner sees where the reference was taken.
//!
//! **Tolerances** ([`Interval::tolerance_ms`]) are teaching values, about one 1 mm square at the
//! drill's speed (20 ms): 20 ms for PR and QT, 15 ms for QRS. Reading a tracing by eye with
//! calipers cannot be more exact than the line width and the learner's choice of where a wave
//! starts, so a tolerance of that size separates a careful measurement from a wrong one.

use crate::narrow::{Narrow, Pane, PaneSpec, Side};
use crate::rhythm_quiz::{DRILL_KEPT_S, record_strip_for};
use crate::scenarios::ScenarioLibrary;
use eframe::egui;
use heartsim_core::{LEAD_COUNT, Lead};
use heartsim_ecg::{Analysis, Analyzer, LeadRing, Regularity, SignalQuality, Tier0Scenario};
use heartsim_ui::annotations::EcgAnnotations;
use heartsim_ui::calipers::{CaliperSet, draw_calipers, record_interact};
use heartsim_ui::strip::{StripStyle, StripTrace, SweepMode, draw_record_strip};
use heartsim_ui::zoom::strip_zoom_factor;
use heartsim_ui::{cards, t};

/// Paper speed of the drill strip, mm/s: 1 mm is 20 ms.
pub const DRILL_SPEED_MM_S: f32 = 50.0;
/// Slowest and fastest effective paper speed of the drill strip when zoomed, mm/s.
pub const DRILL_MIN_MM_S: f32 = 25.0;
pub const DRILL_MAX_MM_S: f32 = 400.0;
/// What one press of a zoom button or key does to the paper speed.
pub const ZOOM_STEP: f32 = 2.0;
/// Height of the position bar under the strip, screen points.
const POSITION_BAR_H: f32 = 8.0;
/// Room the page keeps above and below the strip for the zoom row and the position bar.
const STRIP_CHROME_H: f32 = 56.0;
/// Strips tried before a question gives up (some rhythms have no measurable PR or a ragged QT).
const MAX_TRIES: usize = 24;
/// Smallest and largest height of the drill strip, screen points. The strip takes the height the
/// page has (a taller lane, not a stretched grid: the paper scale of 1 mm per 20 ms and 10 mm/mV
/// never changes) up to a maximum beyond which the trace would only float in empty paper.
const STRIP_MIN_HEIGHT: f32 = 240.0;
const STRIP_MAX_HEIGHT: f32 = 640.0;
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
    /// The calipers on the strip, in ms from the **start of the record** (not from the strip's
    /// left edge): they stay on their samples while the strip is zoomed and panned.
    pub calipers: CaliperSet,
    /// Time zoom of the strip: the paper speed is `DRILL_SPEED_MM_S × zoom`, within
    /// [`DRILL_MIN_MM_S`]..=[`DRILL_MAX_MM_S`]. 1 = the speed of the question.
    zoom: f32,
    /// First sample of the record at the strip's left edge (the pan).
    first: u64,
    /// Strip width and points per millimetre of the last frame: the zoom anchors on them.
    geometry: (f32, f32),
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
            zoom: 1.0,
            first: 0,
            geometry: (1000.0, StripStyle::default().px_per_mm),
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
        self.zoom_fit();
        self.entry_ms = 0.0;
        self.failed = false;
        // never an auscultation case: its strip is ordinary sinus rhythm (one rule for every pool)
        let pool = library.ecg_quiz_pool();
        if pool.is_empty() {
            self.failed = true;
            return;
        }
        for _ in 0..MAX_TRIES {
            let scenario = pool[below(&mut self.rng, pool.len())];
            let interval = Interval::ALL[below(&mut self.rng, Interval::ALL.len())];
            let entry = &library.entries[scenario];
            let Ok(rings) = record_strip_for(entry, DRILL_KEPT_S) else {
                continue;
            };
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

    /// Effective paper speed of the strip, mm/s.
    #[must_use]
    pub fn speed_mm_s(&self) -> f32 {
        DRILL_SPEED_MM_S * self.zoom
    }

    /// First sample of the record at the strip's left edge.
    #[must_use]
    pub fn first_sample(&self) -> u64 {
        self.first
    }

    /// Tell the drill how wide the strip is and how many points a millimetre is (done every frame
    /// by the page; the zoom keeps the sample under the anchor with them).
    pub fn set_geometry(&mut self, width_px: f32, px_per_mm: f32) {
        if width_px > 0.0 && px_per_mm > 0.0 {
            self.geometry = (width_px, px_per_mm);
        }
    }

    /// Samples across the strip at paper speed `speed_mm_s`.
    fn visible_at(&self, speed_mm_s: f32, fs: f32) -> u64 {
        let px_per_sample = speed_mm_s * self.geometry.1 / fs.max(1.0);
        (self.geometry.0 / px_per_sample).floor() as u64
    }

    /// Longest `first` that still fills the strip at `speed_mm_s`.
    fn max_first_at(&self, speed_mm_s: f32) -> u64 {
        self.rings.first().map_or(0, |r| {
            r.pushed()
                .saturating_sub(self.visible_at(speed_mm_s, r.sample_rate_hz))
        })
    }

    /// Multiply the paper speed by `factor` (clamped to 25–400 mm/s) keeping the sample at
    /// `anchor` (0 = left edge, 1 = right edge of the strip) where it is, and clamp the pan to
    /// the record.
    pub fn zoom_by(&mut self, factor: f32, anchor: f32) {
        let Some(fs) = self.rings.first().map(|r| r.sample_rate_hz) else {
            return;
        };
        let anchor = anchor.clamp(0.0, 1.0);
        let old_speed = self.speed_mm_s();
        let new_speed = (old_speed * factor).clamp(DRILL_MIN_MM_S, DRILL_MAX_MM_S);
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let pos = self.first as f64 + f64::from(anchor) * self.visible_at(old_speed, fs) as f64;
        let first = pos - f64::from(anchor) * self.visible_at(new_speed, fs) as f64;
        self.zoom = new_speed / DRILL_SPEED_MM_S;
        self.first = (first.round().max(0.0) as u64).min(self.max_first_at(new_speed));
    }

    /// Back to the question's paper speed, from the start of the record.
    pub fn zoom_fit(&mut self) {
        self.zoom = 1.0;
        self.first = 0;
    }

    /// Move the pan by `samples` (positive = later in the record), clamped to the record.
    pub fn pan_by(&mut self, samples: f64) {
        let max = self.max_first_at(self.speed_mm_s());
        self.first = ((self.first as f64 + samples).round().max(0.0) as u64).min(max);
    }

    /// Caliper coordinate (ms from the start of the record) of the strip's left edge.
    fn first_ms(&self) -> f32 {
        self.rings
            .first()
            .map_or(0.0, |r| self.first as f32 * 1000.0 / r.sample_rate_hz)
    }

    /// The first caliper pair's interval, ms.
    #[must_use]
    pub fn caliper_ms(&self) -> Option<f64> {
        self.calipers.pairs.first().map(|c| f64::from(c.interval_ms()))
    }

    /// The drill page: the question and controls on the left, the strip on the right.
    pub fn ui(&mut self, ui: &mut egui::Ui, library: &ScenarioLibrary, style: &StripStyle) {
        let spec = PaneSpec {
            id: "interval_drill",
            left: Some(Side {
                id: "interval_drill_panel",
                default_px: 380.0,
                cap_px: 100_000.0,
                name: "",
            }),
            right: None,
            centre: "",
            // on a phone the strip keeps a useful height on top and the controls scroll below it
            narrow: Narrow::Stacked {
                centre_height: STRIP_MIN_HEIGHT + STRIP_CHROME_H,
            },
        };
        crate::narrow::panes(ui, &spec, |ui, pane| match pane {
            Pane::Left => {
                egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui, library));
            }
            Pane::Centre => self.strip(ui, style),
            Pane::Right => {}
        });
    }

    fn controls(&mut self, ui: &mut egui::Ui, library: &ScenarioLibrary) {
        ui.horizontal_wrapped(|ui| {
            let label = if self.question.is_some() {
                t!("rq-next")
            } else {
                t!("app-quiz-new")
            };
            // the one primary action: the next question while none is open or the last one is
            // graded, Check answer while one is open
            let next_is_main = self.question.is_none() || self.grade.is_some();
            let next = ui
                .add_enabled_ui(!library.is_empty(), |ui| {
                    if next_is_main {
                        heartsim_ui::primary_button(ui, label)
                    } else {
                        ui.button(label)
                    }
                })
                .inner;
            if next.clicked() {
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
            cards::result_box(ui, false, t!("interval-no-strip"));
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
        heartsim_ui::info_disclosure(
            ui,
            "interval_definition",
            t!("interval-def-summary", interval = q.interval.name()),
            q.interval.definition(),
        );
        ui.separator();

        ui.horizontal_wrapped(|ui| {
            if ui.button(t!("interval-calipers-place")).clicked() && self.calipers.pairs.is_empty() {
                let at = self.first_ms();
                self.calipers.add_pair(at + 100.0, at + 300.0);
            }
            if ui.button(t!("interval-calipers-clear")).clicked() {
                self.calipers.reset();
            }
        });
        heartsim_ui::helper_text(ui, t!("interval-calipers-hint"));
        if let Some(ms) = self.caliper_ms() {
            ui.label(t!("interval-calipers-reading", ms = format!("{ms:.0}")));
        }
        let can_copy = self.caliper_ms().is_some() && self.grade.is_none();
        let mut use_button = ui.add_enabled(can_copy, egui::Button::new(t!("interval-calipers-use")));
        if self.grade.is_none() {
            use_button = use_button.on_disabled_hover_text(t!("interval-calipers-use-disabled"));
        }
        if use_button.clicked()
            && let Some(ms) = self.caliper_ms()
        {
            self.entry_ms = ms.round();
        }
        ui.horizontal(|ui| {
            let label = ui.strong(t!("interval-your-value"));
            // an input field: outlined, on the field background, empty until a value is entered
            let p = heartsim_ui::Palette::of(ui);
            let widgets = &mut ui.visuals_mut().widgets;
            for w in [&mut widgets.inactive, &mut widgets.hovered] {
                w.weak_bg_fill = if p.dark { p.panel } else { p.card };
                w.bg_fill = w.weak_bg_fill;
            }
            widgets.inactive.bg_stroke = egui::Stroke::new(1.0, p.muted);
            ui.add_enabled(
                self.grade.is_none(),
                egui::DragValue::new(&mut self.entry_ms)
                    .range(0.0..=1500.0)
                    .speed(1.0)
                    .max_decimals(0)
                    .custom_formatter(|v, _| {
                        if v < 0.5 {
                            "—".to_owned()
                        } else {
                            format!("{v:.0}")
                        }
                    })
                    .custom_parser(|text| text.trim().trim_end_matches("ms").trim().parse::<f64>().ok())
                    .suffix(MS),
            )
            .on_hover_text(t!("interval-value-hint"))
            .labelled_by(label.id);
        });
        let graded = self.grade.is_some();
        let can_check = !graded && self.entry_ms > 0.0;
        let mut check = ui
            .add_enabled_ui(can_check, |ui| {
                if graded {
                    // graded: Next question is the action; this one only rests, disabled
                    ui.button(t!("app-quiz-check"))
                } else {
                    heartsim_ui::primary_button(ui, t!("app-quiz-check"))
                }
            })
            .inner;
        if self.grade.is_none() {
            check = check.on_disabled_hover_text(t!("interval-check-disabled"));
            if !can_check {
                // the next step, in words, beside the reason on hover
                let next = if self.caliper_ms().is_some() {
                    t!("interval-next-reading")
                } else {
                    t!("interval-next-calipers")
                };
                heartsim_ui::helper_text(ui, next);
            }
        }
        if check.clicked() {
            self.answer(self.entry_ms);
        }
        if let Some(g) = self.grade {
            ui.separator();
            let args_entry = format!("{:.0}", g.entry_ms);
            let args_truth = format!("{:.0}", q.truth_ms);
            let tolerance = format!("{:.0}", q.interval.tolerance_ms());
            if g.correct {
                cards::result_box(
                    ui,
                    true,
                    t!(
                        "interval-correct",
                        entry = args_entry,
                        truth = args_truth,
                        tolerance = tolerance
                    ),
                );
            } else {
                cards::result_box(
                    ui,
                    false,
                    t!(
                        "interval-wrong",
                        entry = args_entry,
                        truth = args_truth,
                        off = format!("{:.0}", g.error_ms.abs()),
                        tolerance = tolerance
                    ),
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
        let base = StripStyle {
            mode: SweepMode::Scroll,
            ..*style
        };
        self.zoom_row(ui);
        let style = StripStyle {
            mm_per_s: self.speed_mm_s(),
            ..base
        };
        let width = ui.available_width();
        let height = (ui.available_height() - POSITION_BAR_H - ui.spacing().item_spacing.y)
            .clamp(STRIP_MIN_HEIGHT, STRIP_MAX_HEIGHT);
        let rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(width, height));
        self.set_geometry(width, style.px_per_mm);
        self.gestures(ui, rect);
        // the gestures may have changed the speed: draw at the one they chose
        let style = StripStyle {
            mm_per_s: self.speed_mm_s(),
            ..style
        };
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
        // a finished recording: sample `first` at the left edge
        let win = draw_record_strip(ui, id, rect, ring, &style, &trace, self.first);
        let resp = record_interact(ui, id, &win, ring, &mut self.calipers, &mut self.first);
        draw_calipers(ui, &win, ring, false, &self.calipers, &style);
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.bottom() + ui.spacing().item_spacing.y),
            egui::vec2(width, POSITION_BAR_H),
        );
        ui.allocate_rect(rect.union(bar), egui::Sense::hover());
        self.position_bar(ui, bar, &win);
        if resp.has_focus() {
            self.zoom_keys(ui);
        }
    }

    /// The zoom buttons and the scale in words.
    fn zoom_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let step = |ui: &mut egui::Ui, label: String, hint: String, enabled: bool| {
                ui.add_enabled(enabled, egui::Button::new(label))
                    .on_hover_text(hint)
                    .on_disabled_hover_text(t!("interval-zoom-limit"))
                    .clicked()
            };
            if step(
                ui,
                t!("interval-zoom-out"),
                t!("interval-zoom-out-hint"),
                self.speed_mm_s() > DRILL_MIN_MM_S + 1e-3,
            ) {
                self.zoom_by(1.0 / ZOOM_STEP, 0.5);
            }
            if step(
                ui,
                t!("interval-zoom-in"),
                t!("interval-zoom-in-hint"),
                self.speed_mm_s() < DRILL_MAX_MM_S - 1e-3,
            ) {
                self.zoom_by(ZOOM_STEP, 0.5);
            }
            if ui
                .button(t!("interval-zoom-fit"))
                .on_hover_text(t!("interval-zoom-fit-hint"))
                .clicked()
            {
                self.zoom_fit();
            }
            let speed = self.speed_mm_s();
            ui.label(t!(
                "interval-scale",
                ms = format_ms_per_mm(speed),
                speed = format!("{speed:.0}")
            ));
        });
    }

    /// Pinch, Ctrl/⌘ + wheel and the plain wheel zoom about the pointer; a horizontal wheel or a
    /// two-finger swipe pans. The strip sits in no scroll area, so the plain wheel is its own.
    fn gestures(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        if let Some((factor, x)) = strip_zoom_factor(ui, rect, true) {
            self.zoom_by(factor, (x - rect.left()) / rect.width().max(1.0));
        }
        if ui.rect_contains_pointer(rect) {
            let dx = ui.input(|i| i.smooth_scroll_delta.x);
            if dx != 0.0 {
                ui.input_mut(|i| i.smooth_scroll_delta.x = 0.0);
                let px_per_sample = self.speed_mm_s() * self.geometry.1 / self.rings[0].sample_rate_hz;
                self.pan_by(-f64::from(dx / px_per_sample));
            }
        }
    }

    /// `+`/`=` zoom in, `-` zoom out, `0` fit, while the strip has the keyboard focus.
    fn zoom_keys(&mut self, ui: &egui::Ui) {
        use egui::Key;
        let (plus, minus, zero) = ui.input(|i| {
            (
                i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals),
                i.key_pressed(Key::Minus),
                i.key_pressed(Key::Num0),
            )
        });
        if plus {
            self.zoom_by(ZOOM_STEP, 0.5);
        }
        if minus {
            self.zoom_by(1.0 / ZOOM_STEP, 0.5);
        }
        if zero {
            self.zoom_fit();
        }
    }

    /// The thin bar under the strip: which part of the record is in view; drag it to pan.
    fn position_bar(&mut self, ui: &mut egui::Ui, bar: egui::Rect, win: &heartsim_ui::strip::StripWindow) {
        let len = win.head.max(1) as f32;
        let from = bar.left() + bar.width() * self.first as f32 / len;
        let to = bar.left() + bar.width() * (self.first + win.visible_samples).min(win.head) as f32 / len;
        let id = ui.scope_id().with("interval_drill_position");
        let resp = ui
            .interact(bar, id, egui::Sense::click_and_drag())
            .on_hover_text(t!("interval-pan-hint"));
        if (resp.dragged() || resp.clicked())
            && let Some(p) = resp.interact_pointer_pos()
        {
            let centre = (p.x - bar.left()) / bar.width().max(1.0) * len;
            let first = centre - win.visible_samples as f32 / 2.0;
            self.first = (first.round().max(0.0) as u64).min(win.max_record_first());
        }
        let v = ui.visuals();
        let (track, thumb) = (v.extreme_bg_color, v.widgets.active.bg_fill);
        let p = ui.painter();
        p.rect_filled(bar, 3.0, track);
        p.rect_filled(
            egui::Rect::from_x_y_ranges(from..=to.max(from + 4.0), bar.y_range()),
            3.0,
            thumb,
        );
    }
}

/// Milliseconds one millimetre of paper stands for at `speed_mm_s`, as the page words it.
fn format_ms_per_mm(speed_mm_s: f32) -> String {
    let ms = 1000.0 / speed_mm_s;
    if (ms - ms.round()).abs() < 0.05 {
        format!("{ms:.0}")
    } else {
        format!("{ms:.1}")
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
    fn auscultation_cases_are_never_drawn() {
        let lib = library();
        for seed in 1..=20u64 {
            let mut drill = IntervalDrill::new(seed);
            drill.next_question(&lib);
            if let Some(q) = &drill.question {
                assert!(
                    lib.in_ecg_quiz_pool(q.scenario),
                    "seed {seed}: {}",
                    lib.entries[q.scenario].scenario.id
                );
            }
        }
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

    /// The window the strip would be drawn with, on a strip as wide as the drill was told.
    fn window_of(d: &IntervalDrill) -> heartsim_ui::strip::StripWindow {
        let style = StripStyle {
            mm_per_s: d.speed_mm_s(),
            ..StripStyle::default()
        };
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(d.geometry.0, 240.0));
        heartsim_ui::strip::StripWindow::record(
            rect,
            &style,
            RATE_HZ,
            d.first_sample(),
            d.rings()[0].pushed(),
        )
    }

    fn asked(seed: u64) -> IntervalDrill {
        let mut d = IntervalDrill::new(seed);
        d.next_question(&library());
        d.set_geometry(1000.0, StripStyle::default().px_per_mm);
        d
    }

    #[test]
    fn zoom_in_out_and_fit_stay_within_25_to_400_mm_per_s() {
        let mut d = asked(3);
        assert!((d.speed_mm_s() - 50.0).abs() < 1e-4);
        d.zoom_by(ZOOM_STEP, 0.5);
        assert!((d.speed_mm_s() - 100.0).abs() < 1e-3);
        for _ in 0..10 {
            d.zoom_by(ZOOM_STEP, 0.5);
        }
        assert!(
            (d.speed_mm_s() - DRILL_MAX_MM_S).abs() < 1e-3,
            "{}",
            d.speed_mm_s()
        );
        for _ in 0..10 {
            d.zoom_by(1.0 / ZOOM_STEP, 0.5);
        }
        assert!(
            (d.speed_mm_s() - DRILL_MIN_MM_S).abs() < 1e-3,
            "{}",
            d.speed_mm_s()
        );
        d.zoom_by(f32::NAN, 0.5);
        assert!(
            (d.speed_mm_s() - DRILL_MIN_MM_S).abs() < 1e-3,
            "NaN changes nothing"
        );
        d.zoom_by(8.0, 0.5);
        d.pan_by(100.0);
        d.zoom_fit();
        assert!((d.speed_mm_s() - DRILL_SPEED_MM_S).abs() < 1e-4);
        assert_eq!(d.first_sample(), 0);
    }

    #[test]
    fn zooming_keeps_the_sample_under_the_anchor_and_the_pan_inside_the_record() {
        let mut d = asked(3);
        d.zoom_by(4.0, 0.5);
        d.pan_by(700.0);
        for anchor in [0.0_f32, 0.25, 0.8] {
            let before = window_of(&d);
            let x = before.rect.left() + anchor * before.rect.width();
            let s0 = before.sample_at_x(x).unwrap();
            d.zoom_by(0.5, anchor);
            let after = window_of(&d);
            let s1 = after
                .sample_at_x(after.rect.left() + anchor * after.rect.width())
                .unwrap();
            assert!(s0.abs_diff(s1) <= 2, "anchor {anchor}: {s0} became {s1}");
            d.zoom_by(2.0, anchor);
        }
        d.pan_by(-1e9);
        assert_eq!(d.first_sample(), 0);
        d.pan_by(1e9);
        let w = window_of(&d);
        assert_eq!(d.first_sample(), w.max_record_first());
        assert!(
            w.sample_at_x(w.rect.right() - 0.5).is_some(),
            "still a full strip"
        );
    }

    #[test]
    fn a_caliper_pair_stays_on_its_samples_through_zoom_pan_and_fit() {
        let mut d = asked(5);
        // two samples, in ms from the start of the record
        let (sa, sb) = (800_u64, 1_100_u64);
        let ms = |s: u64| s as f32 * 1000.0 / RATE_HZ;
        d.calipers.add_pair(ms(sa), ms(sb));
        let reading = d.caliper_ms().unwrap();
        let on = |d: &IntervalDrill, s: u64, ms: f32| {
            let w = window_of(d);
            w.x_of_sample(s).map(|x| (x, w.x_of_caliper_ms(ms)))
        };
        let check = |d: &IntervalDrill, label: &str| {
            for (s, c) in [(sa, d.calipers.pairs[0].a_ms), (sb, d.calipers.pairs[0].b_ms)] {
                if let Some((x_sample, x_line)) = on(d, s, c) {
                    let px = window_of(d).px_per_sample;
                    assert!(
                        (x_sample - x_line).abs() <= px,
                        "{label}: sample {s} at {x_sample}, line at {x_line}"
                    );
                }
            }
            assert!(
                (d.caliper_ms().unwrap() - reading).abs() < 1e-9,
                "{label}: reading unchanged"
            );
        };
        check(&d, "start");
        d.zoom_by(4.0, 0.3);
        check(&d, "zoom in");
        d.pan_by(250.0);
        check(&d, "pan");
        // at 4x the pair is on screen after panning to it
        d.pan_by(-1e9);
        d.pan_by(sa as f64 - 100.0);
        assert!(
            on(&d, sa, d.calipers.pairs[0].a_ms).is_some(),
            "the first sample is in view"
        );
        check(&d, "panned onto it");
        d.zoom_fit();
        check(&d, "fit");
    }

    #[test]
    fn the_next_question_resets_the_zoom_and_the_pan() {
        let lib = library();
        let mut d = asked(7);
        d.zoom_by(8.0, 0.5);
        d.pan_by(900.0);
        assert!(d.first_sample() > 0 && d.speed_mm_s() > 100.0);
        d.next_question(&lib);
        assert!((d.speed_mm_s() - DRILL_SPEED_MM_S).abs() < 1e-4);
        assert_eq!(d.first_sample(), 0);
    }

    #[test]
    fn the_scale_in_words_follows_the_paper_speed() {
        assert_eq!(format_ms_per_mm(50.0), "20");
        assert_eq!(format_ms_per_mm(25.0), "40");
        assert_eq!(format_ms_per_mm(400.0), "2.5");
        assert_eq!(format_ms_per_mm(100.0), "10");
    }
}
