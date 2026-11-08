//! Axis drill (brief B8 §3): a hidden target axis is dialled into the synthesiser, the learner
//! reads the six limb leads and clicks where the axis points on the hexaxial circle.
//!
//! Questions come from [`heartsim_core::axis::draw_drill_axis`]: never north-west (170°–280° on the
//! clockwise wheel) and never within 7° of a class boundary, so a strip is never ambiguous. An
//! answer within [`TOLERANCE_DEG`] counts. After every answer the drill explains the reading in
//! the words of the textbook method ("lead I positive and lead II positive → normal axis").

use eframe::egui;
use heartsim_core::axis::draw_drill_axis;
use heartsim_core::{AxisClass, FrontalAxis, Lead};
use heartsim_ui::{AxisPanel, axis_panel, cards, lead_polarity, t};

/// Tolerance for a correct answer, degrees (half a hexaxial sector).
pub const TOLERANCE_DEG: f64 = 15.0;

/// xorshift64*: deterministic per seed, no extra dependency.
fn next_unit(state: &mut u64) -> f64 {
    if *state == 0 {
        *state = 0x9E37_79B9_7F4A_7C15;
    }
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    let r = state.wrapping_mul(0x2545_F491_4F6C_DD1D);
    (r >> 11) as f64 / (1u64 << 53) as f64
}

/// The explanation shown after an answer: which leads are positive or negative for `axis_deg` and
/// the class that follows. Uses the polarity words of the axis dial, so it matches the "net QRS"
/// table of the trainer tab.
#[must_use]
pub fn explanation(axis_deg: f64) -> String {
    let i = lead_polarity(axis_deg, Lead::I);
    let ii = lead_polarity(axis_deg, Lead::II);
    let avf = lead_polarity(axis_deg, Lead::AVF);
    match (FrontalAxis { degrees: axis_deg }).classify() {
        AxisClass::Normal => t!("axis-why-normal", i = i, ii = ii),
        AxisClass::LeftDeviation => t!("axis-why-left", i = i, ii = ii),
        AxisClass::RightDeviation => t!("axis-why-right", i = i, avf = avf),
        AxisClass::ExtremeDeviation => t!("axis-why-extreme", i = i, avf = avf),
    }
}

/// Axis quiz state.
#[derive(Debug)]
pub struct AxisQuiz {
    /// The hidden axis, degrees (−180°, 180°].
    pub target_deg: Option<f64>,
    /// The learner's click, degrees.
    pub guess_deg: Option<f64>,
    /// The answer has been checked and the target is shown.
    pub revealed: bool,
    pub asked: u32,
    pub correct: u32,
    /// Correct answers in a row.
    pub streak: u32,
    rng: u64,
}

impl Default for AxisQuiz {
    fn default() -> Self {
        Self::new(0x9E37_79B9_7F4A_7C15)
    }
}

impl AxisQuiz {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            target_deg: None,
            guess_deg: None,
            revealed: false,
            asked: 0,
            correct: 0,
            streak: 0,
            rng: seed,
        }
    }

    /// Draw the next hidden axis (guard bands respected) and clear the previous answer. Returns it
    /// so the caller can dial it into the synthesiser.
    pub fn next_question(&mut self) -> f64 {
        let mut state = self.rng;
        let target = draw_drill_axis(|| next_unit(&mut state));
        self.rng = state;
        self.target_deg = Some(target);
        self.guess_deg = None;
        self.revealed = false;
        target
    }

    /// Record the learner's click on the circle (ignored once the answer is revealed).
    pub fn set_guess(&mut self, deg: f64) {
        if !self.revealed {
            self.guess_deg = Some(FrontalAxis::normalise(deg));
        }
    }

    /// Whether Check answer can be pressed.
    #[must_use]
    pub fn can_check(&self) -> bool {
        self.target_deg.is_some() && self.guess_deg.is_some() && !self.revealed
    }

    /// Distance of the guess from the target, degrees (0…180).
    #[must_use]
    pub fn error_deg(&self) -> Option<f64> {
        Some(FrontalAxis::normalise(self.guess_deg? - self.target_deg?).abs())
    }

    /// Score the answer (once per question). Returns whether it was correct.
    pub fn check(&mut self) -> Option<bool> {
        if !self.can_check() {
            return None;
        }
        self.revealed = true;
        self.asked += 1;
        let ok = self.error_deg().is_some_and(|e| e <= TOLERANCE_DEG);
        if ok {
            self.correct += 1;
            self.streak += 1;
        } else {
            self.streak = 0;
        }
        Some(ok)
    }

    /// The left-hand panel: question button, score, status line, explanation and the dial.
    /// Returns the new target when a question was just asked, for the caller to apply.
    pub fn panel(&mut self, ui: &mut egui::Ui) -> Option<f64> {
        let mut asked_new = None;
        ui.horizontal_wrapped(|ui| {
            if ui.button(t!("app-quiz-new")).clicked() {
                asked_new = Some(self.next_question());
            }
            if ui
                .add_enabled(self.can_check(), egui::Button::new(t!("app-quiz-check")))
                .clicked()
            {
                self.check();
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
        match (self.target_deg, self.guess_deg, self.revealed) {
            (None, ..) => {
                ui.label(t!("app-quiz-intro"));
            }
            (Some(_), None, _) => {
                ui.label(t!("app-quiz-click"));
            }
            (Some(target), Some(guess), true) => {
                let err = FrontalAxis::normalise(guess - target);
                cards::result_box(
                    ui,
                    err.abs() <= TOLERANCE_DEG,
                    t!(
                        "app-quiz-answer",
                        answer = format!("{target:+.0}"),
                        chosen = format!("{guess:+.0}"),
                        off = format!("{:.0}", err.abs())
                    ),
                );
                ui.add(egui::Label::new(explanation(target)).wrap());
            }
            (Some(_), Some(guess), false) => {
                ui.label(t!("app-quiz-your-answer", chosen = format!("{guess:+.0}")));
            }
        }
        // the cards would give the class away: they appear with the answer
        let shown = if self.revealed { self.target_deg } else { None };
        let panel = AxisPanel {
            axis: shown,
            guess: self.guess_deg,
            interactive: !self.revealed && self.target_deg.is_some(),
            max_side: 420.0,
            show_cards: self.revealed,
            show_torso: false,
            note: None,
        };
        if let Some(g) = axis_panel(ui, &panel).dragged_to {
            self.set_guess(g);
        }
        asked_new
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_core::axis::drill_axis_allowed;

    #[test]
    fn every_question_respects_the_guard_bands() {
        let mut quiz = AxisQuiz::new(3);
        for _ in 0..10_000 {
            let a = quiz.next_question();
            assert!(drill_axis_allowed(a), "{a}");
            assert_eq!(quiz.target_deg, Some(a));
        }
    }

    #[test]
    fn the_same_seed_asks_the_same_questions() {
        let (mut a, mut b) = (AxisQuiz::new(77), AxisQuiz::new(77));
        for _ in 0..50 {
            assert_eq!(a.next_question().to_bits(), b.next_question().to_bits());
        }
    }

    #[test]
    fn scoring_counts_once_and_tracks_the_streak() {
        let mut q = AxisQuiz::new(1);
        assert_eq!(q.check(), None, "nothing to check before a question");
        let t = q.next_question();
        assert_eq!(q.check(), None, "no guess yet");
        q.set_guess(t + TOLERANCE_DEG - 1.0);
        assert_eq!(q.check(), Some(true));
        assert_eq!(q.check(), None, "scored once");
        q.set_guess(t + 100.0);
        assert_eq!(
            q.guess_deg.map(f64::round),
            Some(FrontalAxis::normalise(t + TOLERANCE_DEG - 1.0).round()),
            "revealed answers cannot change"
        );
        assert_eq!((q.asked, q.correct, q.streak), (1, 1, 1));

        let t = q.next_question();
        q.set_guess(t + TOLERANCE_DEG + 5.0);
        assert_eq!(q.check(), Some(false));
        assert_eq!((q.asked, q.correct, q.streak), (2, 1, 0));
        // wrap-around: 179° and -179° are 2° apart
        q.next_question();
        q.target_deg = Some(179.0);
        q.set_guess(-179.0);
        assert!((q.error_deg().unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn the_explanation_names_the_leads_and_the_class() {
        heartsim_ui::i18n::set_language("en");
        let normal = explanation(60.0);
        assert!(
            normal.contains("Lead I is positive") && normal.contains("lead II is positive"),
            "{normal}"
        );
        assert!(normal.contains("normal axis"), "{normal}");
        let left = explanation(-60.0);
        assert!(
            left.contains("lead II is negative") && left.contains("left axis deviation"),
            "{left}"
        );
        let right = explanation(120.0);
        assert!(
            right.contains("Lead I is negative") && right.contains("aVF is positive"),
            "{right}"
        );
        assert!(right.contains("right axis deviation"), "{right}");
        assert!(explanation(-135.0).contains("extreme"));
    }
}
