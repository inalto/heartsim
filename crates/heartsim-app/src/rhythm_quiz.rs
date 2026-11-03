//! Rhythm-recognition quiz (brief B8 §4): a hidden rhythm from the scenario library is recorded
//! and shown as a strip; the learner picks the diagnosis from four choices. Distractors come
//! first from the scenario's own `confused_with` list (the teaching differential), then at random.
//!
//! The strip is a Tier-0 recording of the scenario ([`heartsim_ecg::Tier0Scenario::record`]),
//! deterministic and independent of the simulator, so answering never disturbs the running
//! simulation and the rhythm's name is not visible anywhere until the answer is checked.

use crate::scenarios::ScenarioLibrary;
use eframe::egui;
use heartsim_core::{LEAD_COUNT, Lead};
use heartsim_ecg::LeadRing;
use heartsim_ui::{StripStyle, TwelveLeadLayout, lead_column, twelve_lead_page};

/// Choices per question.
pub const OPTIONS: usize = 4;
/// Length of the recording, s; the first [`SETTLE_S`] are dropped so rate-dependent rhythms
/// (Wenckebach cycles, re-entry started by a timed premature beat) have settled.
pub const RECORD_S: f64 = 14.0;
pub const SETTLE_S: f64 = 4.0;
/// Sampling rate of the quiz strip, Hz (the simulator's rate).
pub const RATE_HZ: f32 = 500.0;

/// One question: the library index of the answer and of the four choices, in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub answer: usize,
    pub options: Vec<usize>,
}

/// xorshift64*: deterministic per seed, no extra dependency (same generator as the axis quiz).
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

fn shuffle<T>(v: &mut [T], state: &mut u64) {
    for i in (1..v.len()).rev() {
        v.swap(i, below(state, i + 1));
    }
}

/// Draw a question from `library`, avoiding `previous` as the answer when there is a choice.
#[must_use]
pub fn make_question(library: &ScenarioLibrary, rng: &mut u64, previous: Option<usize>) -> Option<Question> {
    let n = library.len();
    if n < 2 {
        return None;
    }
    let mut answer = below(rng, n);
    if Some(answer) == previous {
        answer = (answer + 1 + below(rng, n - 1)) % n;
    }
    let mut options = vec![answer];
    let mut differential: Vec<usize> = library.entries[answer]
        .scenario
        .confused_with
        .iter()
        .filter_map(|id| library.index_of(id))
        .filter(|&i| i != answer)
        .collect();
    shuffle(&mut differential, rng);
    for i in differential {
        if options.len() < OPTIONS && !options.contains(&i) {
            options.push(i);
        }
    }
    while options.len() < OPTIONS.min(n) {
        let i = below(rng, n);
        if !options.contains(&i) {
            options.push(i);
        }
    }
    shuffle(&mut options, rng);
    Some(Question { answer, options })
}

/// Quiz state: the current question, its recording, the learner's choice and the score.
#[derive(Debug)]
pub struct RhythmQuiz {
    rng: u64,
    pub question: Option<Question>,
    /// The learner's choice (library index); `Some` = answered and revealed.
    pub chosen: Option<usize>,
    pub asked: u32,
    pub correct: u32,
    pub streak: u32,
    /// Show all twelve leads instead of II and V1.
    pub twelve_lead: bool,
    rings: Vec<LeadRing>,
    error: Option<String>,
}

impl Default for RhythmQuiz {
    fn default() -> Self {
        Self::new(0x2545_F491_4F6C_DD1D)
    }
}

impl RhythmQuiz {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            rng: seed,
            question: None,
            chosen: None,
            asked: 0,
            correct: 0,
            streak: 0,
            twelve_lead: false,
            rings: Vec::new(),
            error: None,
        }
    }

    /// Ask a new question and record its strip.
    pub fn next_question(&mut self, library: &ScenarioLibrary) {
        let previous = self.question.as_ref().map(|q| q.answer);
        self.chosen = None;
        self.question = make_question(library, &mut self.rng, previous);
        self.rings.clear();
        self.error = None;
        let Some(q) = &self.question else { return };
        match library.entries[q.answer].record(RECORD_S) {
            Ok(rec) => {
                let step = (1000.0 / f64::from(RATE_HZ) / heartsim_ecg::scenario::STEP_MS).round() as usize;
                let skip = (SETTLE_S * 1000.0 / heartsim_ecg::scenario::STEP_MS) as usize;
                let kept = (RECORD_S - SETTLE_S) * f64::from(RATE_HZ);
                self.rings = rec
                    .leads
                    .iter()
                    .map(|l| {
                        let mut r = LeadRing::new(kept as usize, RATE_HZ);
                        for &v in l.iter().skip(skip + step - 1).step_by(step.max(1)) {
                            r.push(v);
                        }
                        r
                    })
                    .collect();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Record the learner's answer (once per question).
    pub fn answer(&mut self, choice: usize) {
        let Some(q) = &self.question else { return };
        if self.chosen.is_some() {
            return;
        }
        self.chosen = Some(choice);
        self.asked += 1;
        if choice == q.answer {
            self.correct += 1;
            self.streak += 1;
        } else {
            self.streak = 0;
        }
    }

    /// The recorded strip (twelve rings at [`RATE_HZ`], empty before the first question).
    #[must_use]
    pub fn rings(&self) -> &[LeadRing] {
        &self.rings
    }

    /// Score as stored between sessions (`"correct/asked"`).
    #[must_use]
    pub fn score_string(&self) -> String {
        format!("{}/{}", self.correct, self.asked)
    }

    /// Restore a score saved with [`Self::score_string`]; malformed text is ignored.
    pub fn restore_score(&mut self, s: &str) {
        if let Some((c, a)) = s.split_once('/')
            && let (Ok(c), Ok(a)) = (c.trim().parse::<u32>(), a.trim().parse::<u32>())
            && c <= a
        {
            self.correct = c;
            self.asked = a;
        }
    }

    /// The quiz page. Returns the library index to open in the simulator when the learner asks
    /// to explore the revealed rhythm there.
    pub fn ui(&mut self, ui: &mut egui::Ui, library: &ScenarioLibrary, style: &StripStyle) -> Option<usize> {
        let mut open = None;
        egui::Panel::left("rhythm_quiz_choices")
            .default_size(360.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let label = if self.question.is_some() {
                        "Next question"
                    } else {
                        "New question"
                    };
                    if ui
                        .add_enabled(!library.is_empty(), egui::Button::new(label))
                        .clicked()
                    {
                        self.next_question(library);
                    }
                    ui.label(format!("Score {}/{}", self.correct, self.asked));
                    if self.streak > 1 {
                        ui.label(format!("streak {}", self.streak));
                    }
                });
                ui.checkbox(&mut self.twelve_lead, "Show all 12 leads");
                ui.separator();
                if library.is_empty() {
                    ui.label("No scenario library found (assets/scenarios).");
                    return;
                }
                if let Some(e) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, e);
                }
                let Some(q) = self.question.clone() else {
                    ui.label("Press New question, read the strip, then choose the rhythm.");
                    return;
                };
                ui.label(if self.chosen.is_none() {
                    "What is the rhythm?"
                } else {
                    "Answer:"
                });
                for &i in &q.options {
                    let title = &library.entries[i].scenario.title;
                    let text = match self.chosen {
                        Some(_) if i == q.answer => egui::RichText::new(format!("{title}  (correct)"))
                            .color(egui::Color32::LIGHT_GREEN),
                        Some(c) if c == i => egui::RichText::new(format!("{title}  (your answer)"))
                            .color(egui::Color32::LIGHT_RED),
                        _ => egui::RichText::new(title),
                    };
                    if ui
                        .add_enabled(self.chosen.is_none(), egui::Button::new(text).wrap())
                        .clicked()
                    {
                        self.answer(i);
                    }
                }
                if let Some(c) = self.chosen {
                    ui.separator();
                    let s = &library.entries[q.answer].scenario;
                    if c == q.answer {
                        ui.colored_label(egui::Color32::LIGHT_GREEN, "Correct.");
                    } else {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            format!("Not quite: this is {}.", s.title),
                        );
                    }
                    ui.label(&s.description);
                    if let Some(notes) = &s.notes {
                        let teaching = notes.split("\n\nSources:").next().unwrap_or(notes);
                        ui.collapsing("Why", |ui| ui.label(teaching));
                    }
                    if ui.button("Open in simulator").clicked() {
                        open = Some(q.answer);
                    }
                }
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if self.rings.len() < LEAD_COUNT {
                return;
            }
            let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
            if self.twelve_lead {
                twelve_lead_page(ui, rect, &self.rings, style, TwelveLeadLayout::Clinical);
                ui.allocate_rect(rect, egui::Sense::hover());
            } else {
                lead_column(ui, rect, &self.rings, style, &[Lead::II, Lead::V1]);
            }
        });
        open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn library() -> ScenarioLibrary {
        ScenarioLibrary::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios"))
            .unwrap()
    }

    #[test]
    fn questions_have_four_distinct_choices_including_the_answer() {
        let lib = library();
        let mut rng = 7;
        let mut prev = None;
        let mut answers = std::collections::BTreeSet::new();
        for _ in 0..500 {
            let q = make_question(&lib, &mut rng, prev).unwrap();
            assert_eq!(q.options.len(), OPTIONS);
            assert!(q.options.contains(&q.answer));
            let mut o = q.options.clone();
            o.sort_unstable();
            o.dedup();
            assert_eq!(o.len(), OPTIONS, "duplicate choice in {q:?}");
            assert_ne!(Some(q.answer), prev, "same rhythm twice in a row");
            prev = Some(q.answer);
            answers.insert(q.answer);
        }
        assert_eq!(answers.len(), lib.len(), "every rhythm gets asked");
    }

    #[test]
    fn distractors_come_from_the_teaching_differential_first() {
        let lib = library();
        let mut rng = 11;
        for _ in 0..200 {
            let q = make_question(&lib, &mut rng, None).unwrap();
            let s = &lib.entries[q.answer].scenario;
            let in_library: Vec<usize> = s.confused_with.iter().filter_map(|id| lib.index_of(id)).collect();
            let want = in_library.len().min(OPTIONS - 1);
            let got = q.options.iter().filter(|i| in_library.contains(i)).count();
            assert!(got >= want, "{}: {got} of {want} differential choices", s.id);
        }
    }

    #[test]
    fn same_seed_same_questions() {
        let lib = library();
        let (mut a, mut b) = (99, 99);
        for _ in 0..20 {
            assert_eq!(
                make_question(&lib, &mut a, None),
                make_question(&lib, &mut b, None)
            );
        }
    }

    #[test]
    fn answering_scores_once_and_records_a_ten_second_strip() {
        let lib = library();
        let mut quiz = RhythmQuiz::new(3);
        quiz.next_question(&lib);
        assert_eq!(quiz.rings().len(), LEAD_COUNT);
        let ten_s = (10.0 * RATE_HZ) as usize;
        assert_eq!(quiz.rings()[0].len(), ten_s);
        let answer = quiz.question.as_ref().unwrap().answer;
        quiz.answer(answer);
        quiz.answer(answer);
        assert_eq!((quiz.correct, quiz.asked, quiz.streak), (1, 1, 1));
        quiz.next_question(&lib);
        let wrong = *quiz
            .question
            .as_ref()
            .unwrap()
            .options
            .iter()
            .find(|&&i| i != quiz.question.as_ref().unwrap().answer)
            .unwrap();
        quiz.answer(wrong);
        assert_eq!((quiz.correct, quiz.asked, quiz.streak), (1, 2, 0));
    }

    #[test]
    fn score_round_trips_and_ignores_garbage() {
        let mut q = RhythmQuiz::default();
        q.restore_score("7/12");
        assert_eq!((q.correct, q.asked), (7, 12));
        assert_eq!(q.score_string(), "7/12");
        for bad in ["", "x/2", "5/3", "3"] {
            q.restore_score(bad);
            assert_eq!((q.correct, q.asked), (7, 12), "{bad:?}");
        }
    }
}
