//! Scripted patient cases (brief B18): `assets/cases/<id>.ron`.
//!
//! A case is a short story told in stages. Each stage shows a vignette (history, observations),
//! sets the simulator up with the same [`Step`] vocabulary as tutorial pages (load a rhythm, give a
//! drug, shock, …) and asks one or more multiple-choice questions with an explanation. The learner
//! reads the ECG the simulator shows and answers; a score closes the case.
//!
//! Like tutorials, cases are data only: the application and the headless
//! `heartsim-cli validate-cases` read the same files.

use crate::tutorial::{Question, Step, UiAction};
use serde::{Deserialize, Serialize};

pub const CASE_FORMAT_VERSION: u32 = 1;

/// Fewest questions a shipped case must ask.
pub const MIN_QUESTIONS: usize = 5;

/// A scripted patient case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    pub format_version: u32,
    /// Stable id, equal to the file stem; keys the saved score.
    pub id: String,
    pub title: String,
    /// One or two sentences for the case list (without giving the diagnosis away).
    pub summary: String,
    /// Who the patient is, one line: age, sex, setting, presenting complaint.
    pub patient: String,
    /// Open literature the clinical statements were checked against, one citation per entry.
    #[serde(default)]
    pub references: Vec<String>,
    pub stages: Vec<Stage>,
}

/// One step of the story.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    /// Short title for the stage index.
    pub title: String,
    /// CommonMark vignette: what the clinician knows and sees at this point.
    pub markdown: String,
    /// Run in order when the stage is entered (same actions as a tutorial page).
    #[serde(default)]
    pub actions: Vec<Step>,
    /// Asked in order; each is answered once, with feedback and an explanation.
    #[serde(default)]
    pub questions: Vec<Question>,
}

impl Case {
    /// Parse a case file.
    ///
    /// # Errors
    /// Malformed RON, a format newer than this build, or a structural error (no stages, a question
    /// whose answer is out of range).
    pub fn from_ron(text: &str) -> Result<Self, CaseError> {
        let c: Self = ron::from_str(text)?;
        if c.format_version > CASE_FORMAT_VERSION {
            return Err(CaseError::Version(c.format_version));
        }
        if let Some(p) = c.problems().into_iter().next() {
            return Err(CaseError::Content(p));
        }
        Ok(c)
    }

    /// Pretty RON with struct names, as the files are written.
    ///
    /// # Errors
    /// Only if serialisation itself fails.
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default().struct_names(true))
    }

    /// Total number of questions in all stages.
    #[must_use]
    pub fn question_count(&self) -> usize {
        self.stages.iter().map(|s| s.questions.len()).sum()
    }

    /// Structural errors that make a case unplayable (no stages, an answer out of range).
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.stages.is_empty() {
            out.push(format!("{}: no stages", self.id));
        }
        for (i, s) in self.stages.iter().enumerate() {
            for (j, q) in s.questions.iter().enumerate() {
                if q.choices.is_empty() || q.answer >= q.choices.len() {
                    out.push(format!(
                        "{} stage {} question {}: answer {} of {} choices",
                        self.id,
                        i + 1,
                        j + 1,
                        q.answer,
                        q.choices.len()
                    ));
                }
            }
        }
        out
    }

    /// Editorial rules of the shipped cases (brief B18), checked by `heartsim-cli validate-cases`
    /// on top of [`Self::problems`]: title, summary, patient and references; the first stage opens
    /// a rhythm; every stage has text; at least [`MIN_QUESTIONS`] questions, each with 3–5 distinct
    /// choices and an explanation.
    #[must_use]
    pub fn content_problems(&self) -> Vec<String> {
        let mut out = self.problems();
        let id = &self.id;
        for (name, text) in [
            ("title", &self.title),
            ("summary", &self.summary),
            ("patient", &self.patient),
        ] {
            if text.trim().is_empty() {
                out.push(format!("{id}: empty {name}"));
            }
        }
        if self.references.iter().all(|r| r.trim().is_empty()) {
            out.push(format!("{id}: no references"));
        }
        if let Some(first) = self.stages.first()
            && !first
                .actions
                .iter()
                .any(|a| matches!(a, Step::Ui(UiAction::LoadScenario(_) | UiAction::StartReentry(_))))
        {
            out.push(format!("{id} stage 1: does not load a rhythm"));
        }
        for (i, s) in self.stages.iter().enumerate() {
            let at = format!("{id} stage {}", i + 1);
            if s.title.trim().is_empty() {
                out.push(format!("{at}: empty title"));
            }
            if s.markdown.trim().is_empty() {
                out.push(format!("{at}: empty text"));
            }
            for (j, q) in s.questions.iter().enumerate() {
                let qat = format!("{at} question {}", j + 1);
                out.extend(q.problems(&qat));
                if !(3..=5).contains(&q.choices.len()) {
                    out.push(format!("{qat}: {} choices (use 3 to 5)", q.choices.len()));
                }
            }
        }
        if self.question_count() < MIN_QUESTIONS {
            out.push(format!(
                "{id}: {} questions (need ≥ {MIN_QUESTIONS})",
                self.question_count()
            ));
        }
        out
    }

    /// Ids of the library rhythms the case loads, in order.
    pub fn scenario_ids(&self) -> impl Iterator<Item = &str> {
        self.stages
            .iter()
            .flat_map(|s| &s.actions)
            .filter_map(|a| match a {
                Step::Ui(UiAction::LoadScenario(id)) => Some(id.as_str()),
                _ => None,
            })
    }
}

#[derive(Debug)]
pub enum CaseError {
    Parse(ron::error::SpannedError),
    Version(u32),
    Content(String),
}

impl std::fmt::Display for CaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "case parse error: {e}"),
            Self::Version(v) => write!(
                f,
                "case format version {v} is newer than supported ({CASE_FORMAT_VERSION})"
            ),
            Self::Content(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for CaseError {}

impl From<ron::error::SpannedError> for CaseError {
    fn from(e: ron::error::SpannedError) -> Self {
        Self::Parse(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(n: usize) -> Question {
        Question {
            prompt: format!("Q{n}?"),
            choices: vec!["a".into(), "b".into(), "c".into()],
            answer: 1,
            explanation: "because".into(),
        }
    }

    fn case(questions: usize) -> Case {
        Case {
            format_version: CASE_FORMAT_VERSION,
            id: "c".into(),
            title: "C".into(),
            summary: "s".into(),
            patient: "A patient".into(),
            references: vec!["Ref 2020".into()],
            stages: vec![Stage {
                title: "one".into(),
                markdown: "text".into(),
                actions: vec![Step::Ui(UiAction::LoadScenario("lbbb".into()))],
                questions: (0..questions).map(question).collect(),
            }],
        }
    }

    #[test]
    fn roundtrip_and_counts() {
        let c = case(5);
        assert_eq!(Case::from_ron(&c.to_ron().unwrap()).unwrap(), c);
        assert_eq!(c.question_count(), 5);
        assert!(c.content_problems().is_empty(), "{:?}", c.content_problems());
        assert_eq!(c.scenario_ids().collect::<Vec<_>>(), ["lbbb"]);
    }

    #[test]
    fn editorial_rules() {
        let p = case(4).content_problems();
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("4 questions"));
        let mut c = case(5);
        c.stages[0].actions.clear();
        c.stages[0].questions[2].explanation.clear();
        c.stages[0].questions[3].choices.truncate(2);
        c.references.clear();
        let p = c.content_problems();
        assert!(p.iter().any(|m| m.contains("does not load a rhythm")), "{p:?}");
        assert!(p.iter().any(|m| m.contains("no explanation")));
        assert!(p.iter().any(|m| m.contains("2 choices")));
        assert!(p.iter().any(|m| m.contains("no references")));
    }

    #[test]
    fn rejects_bad_answer_and_future_version() {
        let mut c = case(5);
        c.stages[0].questions[0].answer = 3;
        assert!(matches!(
            Case::from_ron(&c.to_ron().unwrap()),
            Err(CaseError::Content(_))
        ));
        let mut c = case(5);
        c.format_version += 1;
        assert!(matches!(
            Case::from_ron(&c.to_ron().unwrap()),
            Err(CaseError::Version(_))
        ));
    }
}
