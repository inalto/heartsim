//! Guided tutorials (briefs B15, B18): `assets/tutorials/<id>/tutorial.ron`, an ordered list of
//! pages, each a Markdown text, the actions that set the simulator up for it, and an optional
//! knowledge check.
//!
//! The data model lives here, below the application, so that the desktop app, the web build and the
//! headless `heartsim-cli run-tutorial` all read and run the same files.
//!
//! **Actions are data.** A page's [`Step::Sim`] carries a scenario [`Action`] verbatim and is handed
//! to the host exactly like a scenario timeline entry, so every intervention the engines learn
//! (drugs, shocks, ischaemia, …) works in tutorials without a change here. [`Step::Ui`] covers what
//! only the application can do: open a rhythm, switch panels, move the camera, pause.
//!
//! The [`PageRun`] executes a page's steps in order through the [`TutorialHost`] trait, stopping at
//! [`UiAction::WaitForEvent`] until simulated time or the heart catches up; the application and
//! the tests implement the same trait, so a page drives the model exactly as applying its actions
//! one by one would.

use crate::lead::Lead;
use crate::scenario::Action;
use serde::{Deserialize, Serialize};

pub const TUTORIAL_FORMAT_VERSION: u32 = 1;

/// A guided tutorial.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tutorial {
    pub format_version: u32,
    /// Stable id, equal to the directory name; keys the saved progress.
    pub id: String,
    pub title: String,
    /// One or two sentences for the tutorial list.
    pub summary: String,
    /// Open literature the pages were checked against (guideline, textbook chapter, LITFL page),
    /// one citation per entry.
    #[serde(default)]
    pub references: Vec<String>,
    pub pages: Vec<Page>,
}

/// One page: what the learner reads, what the simulator does on entering it, and an optional
/// question that must be answered correctly before moving on in self-study mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    /// Short title for the page index.
    pub title: String,
    /// CommonMark text.
    pub markdown: String,
    /// Run in order when the page is entered.
    #[serde(default)]
    pub actions: Vec<Step>,
    #[serde(default)]
    pub check: Option<Question>,
}

/// A page action: a scenario intervention (same vocabulary as scenario timelines) or an
/// application command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Step {
    Sim(Action),
    Ui(UiAction),
}

impl Step {
    /// `true` if the step changes what the simulation does (a scenario action, a new rhythm, a new
    /// axis), as opposed to moving the camera or a panel. Every tutorial page needs one.
    #[must_use]
    pub fn changes_simulation(&self) -> bool {
        matches!(
            self,
            Step::Sim(_)
                | Step::Ui(UiAction::LoadScenario(_) | UiAction::StartReentry(_) | UiAction::SetQrsAxis(_))
        )
    }
}

/// What only the application can do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UiAction {
    /// Load a rhythm from the scenario library by id (restarts the clock at zero).
    LoadScenario(String),
    /// Start a re-entry preset (`assets/scenarios/reentry/<id>.ron`, e.g. `"af_persistent"`) on the
    /// reaction–diffusion engine; it runs slower than real time.
    StartReentry(String),
    /// Bring a panel to the front.
    FocusPanel(Panel),
    /// Outline a control so the learner finds it; cleared on the next page. Ids are listed in
    /// `heartsim_ui::highlight` (e.g. `"pause"`, `"speed"`, `"rhythm_picker"`).
    Highlight(String),
    /// Move the 3-D camera to a standard view.
    CameraPreset(CameraView),
    /// Simulation speed relative to real time (0.05–2).
    SetSpeed(f64),
    Pause,
    Resume,
    /// Hold the remaining steps of the page until this happens.
    WaitForEvent(WaitEvent),
    /// Rotate the frontal QRS axis to this angle, degrees (the Axis Trainer's dial).
    SetQrsAxis(f64),
    /// Lead shown on the single-lead strip.
    SelectLead(Lead),
}

/// Panels a tutorial can bring forward (the application's tabs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Panel {
    Simulator,
    TwelveLead,
    HeartVector,
    AxisTrainer,
    AxisQuiz,
    Electrodes,
    RhythmQuiz,
    Tutorials,
}

/// Standard radiological views of the heart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraView {
    Ap,
    Pa,
    Lao,
    Rao,
    Left,
    Cranial,
    Caudal,
    /// Zoom to fit the whole heart.
    Frame,
}

/// What [`UiAction::WaitForEvent`] waits for, counted from the moment the wait starts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WaitEvent {
    /// This much simulated time, ms.
    ElapsedMs(f64),
    /// The next ventricular activation (QRS onset).
    Qrs,
}

/// A multiple-choice question (tutorial knowledge check, patient-case MCQ).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub prompt: String,
    pub choices: Vec<String>,
    /// Index into `choices`.
    pub answer: usize,
    /// Shown once answered (Markdown).
    #[serde(default)]
    pub explanation: String,
}

impl Question {
    /// Content errors of the question, each prefixed with `where_`: fewer than two choices, an
    /// answer out of range, an empty prompt or explanation, repeated choices.
    #[must_use]
    pub fn problems(&self, where_: &str) -> Vec<String> {
        let mut out = Vec::new();
        if self.prompt.trim().is_empty() {
            out.push(format!("{where_}: empty question"));
        }
        if self.choices.len() < 2 {
            out.push(format!("{where_}: {} choices (need ≥ 2)", self.choices.len()));
        }
        if self.answer >= self.choices.len() {
            out.push(format!(
                "{where_}: answer {} of {} choices",
                self.answer,
                self.choices.len()
            ));
        }
        for (i, c) in self.choices.iter().enumerate() {
            if c.trim().is_empty() {
                out.push(format!("{where_}: choice {} is empty", i + 1));
            }
            if self.choices[..i].contains(c) {
                out.push(format!("{where_}: choice `{c}` appears twice"));
            }
        }
        if self.explanation.trim().is_empty() {
            out.push(format!("{where_}: no explanation"));
        }
        out
    }
}

impl Tutorial {
    /// Parse a tutorial file.
    ///
    /// # Errors
    /// Malformed RON, a format newer than this build, or content errors (see [`Self::problems`]).
    pub fn from_ron(text: &str) -> Result<Self, TutorialError> {
        let t: Self = ron::from_str(text)?;
        if t.format_version > TUTORIAL_FORMAT_VERSION {
            return Err(TutorialError::Version(t.format_version));
        }
        if let Some(p) = t.problems().into_iter().next() {
            return Err(TutorialError::Content(p));
        }
        Ok(t)
    }

    /// Pretty RON with struct names, as the files are written.
    ///
    /// # Errors
    /// Only if serialisation itself fails.
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default().struct_names(true))
    }

    /// Structural errors that make a tutorial unusable: no pages, a question without choices or
    /// whose answer is out of range. [`Self::from_ron`] rejects them.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.pages.is_empty() {
            out.push(format!("{}: no pages", self.id));
        }
        for (i, p) in self.pages.iter().enumerate() {
            if let Some(q) = &p.check
                && (q.answer >= q.choices.len() || q.choices.is_empty())
            {
                out.push(format!(
                    "{} page {}: answer {} of {} choices",
                    self.id,
                    i + 1,
                    q.answer,
                    q.choices.len()
                ));
            }
        }
        out
    }

    /// Editorial rules of the shipped content (brief B18), checked by `heartsim-cli run-tutorial`
    /// on top of [`Self::problems`]: a title, summary and references; every page has text, at
    /// least one action that changes the simulation, and a check (when it has one) is well formed with an
    /// explanation.
    #[must_use]
    pub fn content_problems(&self) -> Vec<String> {
        let mut out = self.problems();
        let id = &self.id;
        if self.title.trim().is_empty() {
            out.push(format!("{id}: empty title"));
        }
        if self.summary.trim().is_empty() {
            out.push(format!("{id}: empty summary"));
        }
        if self.references.iter().all(|r| r.trim().is_empty()) {
            out.push(format!("{id}: no references"));
        }
        for (i, p) in self.pages.iter().enumerate() {
            let at = format!("{id} page {}", i + 1);
            if p.title.trim().is_empty() {
                out.push(format!("{at}: empty title"));
            }
            if p.markdown.trim().is_empty() {
                out.push(format!("{at}: empty text"));
            }
            if !p.actions.iter().any(Step::changes_simulation) {
                out.push(format!("{at}: no action that changes the simulation"));
            }
            if let Some(q) = &p.check {
                out.extend(q.problems(&at));
            }
        }
        out
    }
}

#[derive(Debug)]
pub enum TutorialError {
    Parse(ron::error::SpannedError),
    Version(u32),
    Content(String),
}

impl std::fmt::Display for TutorialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "tutorial parse error: {e}"),
            Self::Version(v) => write!(
                f,
                "tutorial format version {v} is newer than supported ({TUTORIAL_FORMAT_VERSION})"
            ),
            Self::Content(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for TutorialError {}

impl From<ron::error::SpannedError> for TutorialError {
    fn from(e: ron::error::SpannedError) -> Self {
        Self::Parse(e)
    }
}

/// The model a tutorial page drives. Implemented by the application and by test doubles.
pub trait TutorialHost {
    /// Apply a scenario intervention now, as a timeline entry due at this instant would be.
    fn apply_action(&mut self, action: &Action);
    /// Carry out an application command. Never called with [`UiAction::WaitForEvent`].
    fn apply_ui(&mut self, action: &UiAction);
    /// Simulated time, ms.
    fn sim_now_ms(&self) -> f64;
    /// Time of the latest ventricular activation, ms (anything before the start if none yet).
    fn last_qrs_ms(&self) -> f64;
}

/// Execution state of one page's steps.
#[derive(Debug, Clone, Default)]
pub struct PageRun {
    steps: Vec<Step>,
    next: usize,
    /// Simulated time at which the current wait started.
    waiting_since_ms: Option<f64>,
}

impl PageRun {
    #[must_use]
    pub fn new(page: &Page) -> Self {
        Self::from_steps(page.actions.clone())
    }

    /// A runner for any list of steps (a patient-case stage).
    #[must_use]
    pub fn from_steps(steps: Vec<Step>) -> Self {
        Self {
            steps,
            next: 0,
            waiting_since_ms: None,
        }
    }

    /// Run every step that is due: all of them up to the first wait that is not over yet.
    /// Returns `true` once the page's steps are all done. Call once per frame.
    pub fn poll(&mut self, host: &mut impl TutorialHost) -> bool {
        while let Some(step) = self.steps.get(self.next) {
            match step {
                Step::Sim(a) => host.apply_action(a),
                Step::Ui(UiAction::WaitForEvent(ev)) => {
                    let since = *self.waiting_since_ms.get_or_insert(host.sim_now_ms());
                    let over = match *ev {
                        WaitEvent::ElapsedMs(ms) => host.sim_now_ms() - since >= ms,
                        WaitEvent::Qrs => host.last_qrs_ms() > since,
                    };
                    if !over {
                        return false;
                    }
                    self.waiting_since_ms = None;
                }
                Step::Ui(a) => host.apply_ui(a),
            }
            self.next += 1;
        }
        true
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.next >= self.steps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(actions: Vec<Step>, with_check: bool) -> Page {
        Page {
            title: "t".into(),
            markdown: "text".into(),
            actions,
            check: with_check.then(|| Question {
                prompt: "?".into(),
                choices: vec!["a".into(), "b".into()],
                answer: 0,
                explanation: "because".into(),
            }),
        }
    }

    fn tutorial(pages: Vec<Page>) -> Tutorial {
        Tutorial {
            format_version: TUTORIAL_FORMAT_VERSION,
            id: "t".into(),
            title: "T".into(),
            summary: "s".into(),
            references: vec!["Ref 2020".into()],
            pages,
        }
    }

    #[test]
    fn file_without_references_still_loads() {
        let text = r#"Tutorial(format_version: 1, id: "x", title: "X", summary: "s",
            pages: [Page(title: "p", markdown: "m", actions: [Ui(LoadScenario("sinus_normal"))])])"#;
        let t = Tutorial::from_ron(text).unwrap();
        assert_eq!(t.references, Vec::<String>::new());
        assert!(t.content_problems().iter().any(|p| p.contains("no references")));
    }

    #[test]
    fn roundtrip_keeps_every_action_kind() {
        let t = tutorial(vec![page(
            vec![
                Step::Ui(UiAction::StartReentry("af_persistent".into())),
                Step::Ui(UiAction::SelectLead(Lead::II)),
                Step::Sim(Action::SetGlobalSinusRate { bpm: 50.0 }),
            ],
            true,
        )]);
        assert_eq!(Tutorial::from_ron(&t.to_ron().unwrap()).unwrap(), t);
    }

    #[test]
    fn content_rules_flag_a_page_that_does_not_change_the_simulation() {
        let t = tutorial(vec![page(vec![Step::Ui(UiAction::Pause)], true)]);
        let p = t.content_problems();
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("no action that changes the simulation"));
        let t = tutorial(vec![page(vec![Step::Ui(UiAction::SetQrsAxis(-60.0))], false)]);
        assert!(t.content_problems().is_empty(), "a check is optional");
        let ok = tutorial(vec![page(
            vec![Step::Ui(UiAction::LoadScenario("lbbb".into()))],
            true,
        )]);
        assert_eq!(ok.content_problems(), Vec::<String>::new());
    }

    #[test]
    fn question_rules() {
        let mut q = Question {
            prompt: "?".into(),
            choices: vec!["a".into(), "a".into()],
            answer: 2,
            explanation: String::new(),
        };
        assert_eq!(q.problems("q").len(), 3);
        q.choices[1] = "b".into();
        q.answer = 1;
        q.explanation = "x".into();
        assert_eq!(q.problems("q"), Vec::<String>::new());
    }

    struct Clock {
        now: f64,
        qrs: f64,
        ui: Vec<UiAction>,
        sim: usize,
    }
    impl TutorialHost for Clock {
        fn apply_action(&mut self, _: &Action) {
            self.sim += 1;
        }
        fn apply_ui(&mut self, a: &UiAction) {
            self.ui.push(a.clone());
        }
        fn sim_now_ms(&self) -> f64 {
            self.now
        }
        fn last_qrs_ms(&self) -> f64 {
            self.qrs
        }
    }

    #[test]
    fn waits_hold_the_remaining_steps() {
        let steps = vec![
            Step::Sim(Action::SetGlobalSinusRate { bpm: 60.0 }),
            Step::Ui(UiAction::WaitForEvent(WaitEvent::ElapsedMs(400.0))),
            Step::Ui(UiAction::Pause),
        ];
        let mut h = Clock {
            now: 1000.0,
            qrs: -1.0,
            ui: Vec::new(),
            sim: 0,
        };
        let mut run = PageRun::from_steps(steps);
        assert!(!run.poll(&mut h));
        assert_eq!(h.sim, 1);
        assert_eq!(h.ui, Vec::<UiAction>::new());
        h.now = 1399.0;
        assert!(!run.poll(&mut h));
        h.now = 1400.0;
        assert!(run.poll(&mut h));
        assert_eq!(h.ui, [UiAction::Pause]);
        assert!(run.is_finished());
    }
}
