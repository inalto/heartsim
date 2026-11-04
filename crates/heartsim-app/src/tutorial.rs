//! Guided tutorials (brief B15): `assets/tutorials/<id>/tutorial.ron`, an ordered list of pages,
//! each a Markdown text, the actions that set the simulator up for it, and an optional knowledge
//! check.
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

use heartsim_core::{Action, Lead};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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

/// What only the application can do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UiAction {
    /// Load a rhythm from the scenario library by id (restarts the clock at zero).
    LoadScenario(String),
    /// Bring a panel to the front.
    FocusPanel(Panel),
    /// Outline a control so the learner finds it; cleared on the next page. Ids are listed in
    /// `heartsim_ui::highlight` (e.g. `"pause"`, `"speed"`, `"rhythm_picker"`, `"axis_dial"`).
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

/// A multiple-choice knowledge check.
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

    /// Content errors: no pages, a question without choices or whose answer is out of range.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.pages.is_empty() {
            out.push(format!("{}: no pages", self.id));
        }
        for (i, p) in self.pages.iter().enumerate() {
            if let Some(q) = &p.check
                && q.answer >= q.choices.len()
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
        Self {
            steps: page.actions.clone(),
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

/// How the learner moves through a tutorial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Mode {
    /// Free navigation: any page, checks optional (a teacher presenting to a group).
    Tutor,
    /// Pages in order; a page's check must be answered correctly before the next one.
    #[default]
    SelfStudy,
}

/// Saved progress through one tutorial.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TutorialProgress {
    /// Page shown when the tutorial is resumed.
    pub page: usize,
    /// Highest page reached.
    pub furthest: usize,
    /// Pages whose check was answered correctly.
    pub passed: BTreeSet<usize>,
    /// The last page was reached with every check passed.
    pub completed: bool,
}

/// Progress of every tutorial, persisted as RON in the settings directory.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProgressStore {
    pub tutorials: BTreeMap<String, TutorialProgress>,
}

impl ProgressStore {
    /// `tutorial_progress.ron` next to eframe's own settings file.
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "heartsim").map(|d| d.data_dir().join("tutorial_progress.ron"))
    }

    /// Load from `path`; a missing or unreadable file is an empty store.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| {
                ron::from_str(&t)
                    .inspect_err(|e| tracing::warn!("{}: {e}", path.display()))
                    .ok()
            })
            .unwrap_or_default()
    }

    /// Write to `path`, creating its directory.
    ///
    /// # Errors
    /// I/O failures.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    #[must_use]
    pub fn get(&self, id: &str) -> TutorialProgress {
        self.tutorials.get(id).cloned().unwrap_or_default()
    }
}

/// Where the tutorials are looked for: next to the scenario library.
#[must_use]
pub fn default_tutorial_dir() -> Option<PathBuf> {
    crate::scenarios::default_scenario_dir()
        .and_then(|d| d.parent().map(|p| p.join("tutorials")))
        .filter(|d| d.is_dir())
}

/// Every tutorial in a directory (`<id>/tutorial.ron`), sorted by id so numbered ids give the
/// teaching order. Broken files are logged and skipped.
#[derive(Debug, Clone, Default)]
pub struct TutorialLibrary {
    pub tutorials: Vec<Tutorial>,
}

impl TutorialLibrary {
    /// # Errors
    /// When the directory cannot be read.
    pub fn load_dir(dir: &Path) -> std::io::Result<Self> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path().join("tutorial.ron")))
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        let tutorials = files
            .iter()
            .filter_map(|p| {
                let text = std::fs::read_to_string(p)
                    .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                    .ok()?;
                Tutorial::from_ron(&text)
                    .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                    .ok()
            })
            .collect();
        Ok(Self { tutorials })
    }

    #[must_use]
    pub fn load_default() -> Self {
        default_tutorial_dir()
            .and_then(|d| {
                Self::load_dir(&d)
                    .inspect_err(|e| tracing::warn!("{}: {e}", d.display()))
                    .ok()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_core::{Region, StimSite};
    use heartsim_sim::{Engine, TimingEngine, TimingParams};

    fn sample() -> Tutorial {
        Tutorial {
            format_version: TUTORIAL_FORMAT_VERSION,
            id: "t".into(),
            title: "T".into(),
            summary: "s".into(),
            pages: vec![
                Page {
                    title: "one".into(),
                    markdown: "# Hello\n\nText with **bold**.".into(),
                    actions: vec![
                        Step::Ui(UiAction::LoadScenario("sinus_normal".into())),
                        Step::Sim(Action::SetGlobalSinusRate { bpm: 50.0 }),
                        Step::Ui(UiAction::CameraPreset(CameraView::Lao)),
                        Step::Sim(Action::Block {
                            region: Region::LeftBundleBranch,
                            blocked: true,
                        }),
                        Step::Ui(UiAction::SetSpeed(0.5)),
                    ],
                    check: None,
                },
                Page {
                    title: "two".into(),
                    markdown: "Second".into(),
                    actions: vec![
                        Step::Sim(Action::SetGlobalSinusRate { bpm: 90.0 }),
                        Step::Ui(UiAction::WaitForEvent(WaitEvent::ElapsedMs(400.0))),
                        Step::Ui(UiAction::Highlight("pause".into())),
                        Step::Sim(Action::Stimulate {
                            site: StimSite::Landmark("rv_apex".into()),
                            strength: 2.0,
                            duration_ms: 2.0,
                        }),
                        Step::Ui(UiAction::Pause),
                    ],
                    check: Some(Question {
                        prompt: "?".into(),
                        choices: vec!["a".into(), "b".into()],
                        answer: 1,
                        explanation: String::new(),
                    }),
                },
            ],
        }
    }

    #[test]
    fn tutorial_file_roundtrip() {
        let t = sample();
        let text = t.to_ron().unwrap();
        assert_eq!(Tutorial::from_ron(&text).unwrap(), t);
    }

    #[test]
    fn rejects_bad_answer_and_future_version() {
        let mut t = sample();
        t.pages[1].check.as_mut().unwrap().answer = 2;
        assert!(matches!(
            Tutorial::from_ron(&t.to_ron().unwrap()),
            Err(TutorialError::Content(_))
        ));
        let mut t = sample();
        t.format_version += 1;
        assert!(matches!(
            Tutorial::from_ron(&t.to_ron().unwrap()),
            Err(TutorialError::Version(_))
        ));
    }

    /// A model with a real timing engine and a log of the application commands.
    #[derive(Debug)]
    struct Fake {
        engine: TimingEngine,
        ui: Vec<UiAction>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                engine: TimingEngine::new(TimingParams::default()),
                ui: Vec::new(),
            }
        }

        fn run_ms(&mut self, ms: u32) {
            for _ in 0..ms {
                self.engine.step(heartsim_core::Millis(1.0));
                self.engine.drain_events();
            }
        }

        /// Apply a page's steps by hand, skipping the waits (run in between by the caller).
        fn apply_directly(&mut self, steps: &[Step]) {
            for s in steps {
                match s {
                    Step::Sim(a) => self.apply_action(a),
                    Step::Ui(UiAction::WaitForEvent(_)) => {}
                    Step::Ui(a) => self.apply_ui(a),
                }
            }
        }
    }

    impl TutorialHost for Fake {
        fn apply_action(&mut self, action: &Action) {
            let now = self.engine.now();
            self.engine.apply(action, now).ok();
        }
        fn apply_ui(&mut self, action: &UiAction) {
            self.ui.push(action.clone());
        }
        fn sim_now_ms(&self) -> f64 {
            self.engine.now().0
        }
        fn last_qrs_ms(&self) -> f64 {
            f64::NEG_INFINITY
        }
    }

    #[test]
    fn page_actions_mutate_the_model_like_direct_application() {
        let t = sample();
        let (mut by_page, mut direct) = (Fake::new(), Fake::new());
        by_page.run_ms(100);
        direct.run_ms(100);

        let mut run = PageRun::new(&t.pages[0]);
        assert!(run.poll(&mut by_page), "page without waits finishes at once");
        direct.apply_directly(&t.pages[0].actions);
        assert_eq!(by_page.engine.params, direct.engine.params);
        assert_eq!(by_page.ui, direct.ui);
        assert!((by_page.engine.params.sinus.rate_bpm - 50.0).abs() < 1e-9);

        // page two waits 400 ms between the rate change and the stimulus
        let steps = &t.pages[1].actions;
        let mut run = PageRun::new(&t.pages[1]);
        assert!(!run.poll(&mut by_page));
        direct.apply_directly(&steps[..1]);
        assert_eq!(by_page.engine.params, direct.engine.params);
        assert!(by_page.ui.len() == direct.ui.len(), "nothing after the wait yet");
        for _ in 0..399 {
            by_page.run_ms(1);
            assert!(!run.poll(&mut by_page));
        }
        direct.run_ms(400);
        by_page.run_ms(1);
        assert!(run.poll(&mut by_page));
        assert!(run.is_finished());
        direct.apply_directly(&steps[2..]);
        assert_eq!(by_page.engine.params, direct.engine.params);
        assert_eq!(by_page.ui, direct.ui);
        // and the engines stay in lock-step afterwards (the stimulus landed at the same time)
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for _ in 0..1500 {
            by_page.engine.step(heartsim_core::Millis(1.0));
            direct.engine.step(heartsim_core::Millis(1.0));
            a.extend(by_page.engine.drain_events());
            b.extend(direct.engine.drain_events());
        }
        assert_eq!(a, b);
    }

    #[test]
    fn qrs_wait_ends_on_the_next_ventricular_activation() {
        struct Clock {
            now: f64,
            qrs: f64,
        }
        impl TutorialHost for Clock {
            fn apply_action(&mut self, _: &Action) {}
            fn apply_ui(&mut self, _: &UiAction) {}
            fn sim_now_ms(&self) -> f64 {
                self.now
            }
            fn last_qrs_ms(&self) -> f64 {
                self.qrs
            }
        }
        let page = Page {
            title: String::new(),
            markdown: String::new(),
            actions: vec![Step::Ui(UiAction::WaitForEvent(WaitEvent::Qrs))],
            check: None,
        };
        let mut h = Clock {
            now: 1000.0,
            qrs: 900.0,
        };
        let mut run = PageRun::new(&page);
        assert!(!run.poll(&mut h), "the previous QRS does not count");
        h.now = 1100.0;
        assert!(!run.poll(&mut h));
        h.qrs = 1050.0;
        assert!(run.poll(&mut h));
    }

    #[test]
    fn progress_store_roundtrip() {
        let dir = std::env::temp_dir().join(format!("heartsim-progress-{}", std::process::id()));
        let path = dir.join("sub").join("tutorial_progress.ron");
        let mut s = ProgressStore::default();
        s.tutorials.insert(
            "basics".into(),
            TutorialProgress {
                page: 2,
                furthest: 3,
                passed: [1, 3].into(),
                completed: false,
            },
        );
        s.save(&path).unwrap();
        assert_eq!(ProgressStore::load(&path), s);
        assert_eq!(
            ProgressStore::load(&dir.join("missing.ron")),
            ProgressStore::default()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn shipped_tutorials_load_and_name_known_scenarios() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let lib = TutorialLibrary::load_dir(&root.join("tutorials")).unwrap();
        assert!(lib.tutorials.len() >= 2, "only {} tutorials", lib.tutorials.len());
        let scenarios = crate::scenarios::ScenarioLibrary::load_dir(&root.join("scenarios")).unwrap();
        for t in &lib.tutorials {
            for p in &t.pages {
                for s in &p.actions {
                    if let Step::Ui(UiAction::LoadScenario(id)) = s {
                        assert!(
                            scenarios.index_of(id).is_some(),
                            "{}: unknown scenario {id}",
                            t.id
                        );
                    }
                }
            }
        }
    }
}
