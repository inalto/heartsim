//! Guided tutorials (brief B15) in the application: progress, the library of installed tutorials
//! and the reader's mode.
//!
//! The file format and the page runner ([`Tutorial`], [`Page`], [`Step`], [`UiAction`],
//! [`PageRun`], [`TutorialHost`], …) live in `heartsim_core::tutorial` so that the headless
//! `heartsim-cli run-tutorial` reads and runs the very same files; they are re-exported here.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
#[cfg(feature = "native")]
use std::path::PathBuf;

pub use heartsim_core::tutorial::{
    CameraView, Page, PageRun, Panel, Question, Step, TUTORIAL_FORMAT_VERSION, Tutorial, TutorialError,
    TutorialHost, UiAction, WaitEvent,
};

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
    #[cfg(feature = "native")]
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

    /// Parse the text written by [`Self::to_text`] (the web build keeps it in `localStorage`
    /// through eframe's storage); unreadable text is an empty store, never an error.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        ron::from_str(text)
            .inspect_err(|e| tracing::warn!("saved tutorial progress: {e}"))
            .unwrap_or_default()
    }

    /// The store as RON text.
    #[must_use]
    pub fn to_text(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap_or_default()
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
#[cfg(feature = "native")]
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
    #[cfg(feature = "native")]
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

    /// Every `tutorials/<id>/tutorial.ron` of a downloaded bundle, in id order (as `load_dir`).
    #[must_use]
    pub fn from_bundle(bundle: &crate::assets::AssetBundle) -> Self {
        let tutorials = bundle
            .subdirs_with("tutorials", "tutorial.ron")
            .filter_map(|id| {
                let path = format!("tutorials/{id}/tutorial.ron");
                Tutorial::from_ron(&bundle.text(&path)?)
                    .inspect_err(|e| tracing::warn!("{path}: {e}"))
                    .ok()
            })
            .collect();
        Self { tutorials }
    }

    /// The installed tutorials (none without the `native` feature: the web build uses
    /// [`Self::from_bundle`]).
    #[must_use]
    pub fn load_default() -> Self {
        #[cfg(feature = "native")]
        {
            default_tutorial_dir()
                .and_then(|d| {
                    Self::load_dir(&d)
                        .inspect_err(|e| tracing::warn!("{}: {e}", d.display()))
                        .ok()
                })
                .unwrap_or_default()
        }
        #[cfg(not(feature = "native"))]
        {
            Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_core::{Action, Region, StimSite};
    use heartsim_sim::{Engine, TimingEngine, TimingParams};

    fn sample() -> Tutorial {
        Tutorial {
            format_version: TUTORIAL_FORMAT_VERSION,
            id: "t".into(),
            title: "T".into(),
            summary: "s".into(),
            references: Vec::new(),
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
