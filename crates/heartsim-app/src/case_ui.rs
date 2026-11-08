//! Scripted patient cases (brief B18): the case list and a minimal player.
//!
//! A case (`assets/cases/<id>.ron`, format in `heartsim_core::case`) is played stage by stage: the
//! reader shows the vignette, the stage's simulator actions run through the same
//! [`TutorialHost`] as tutorial pages, and each multiple-choice question is answered once. The
//! first answer counts for the score; the explanation is shown either way. Independent of the GPU
//! and of [`crate::HeartSimApp`], like [`crate::tutorial_ui`].

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use heartsim_core::case::Case;
use heartsim_ui::{cards, t};
#[cfg(feature = "native")]
use std::path::{Path, PathBuf};

use crate::tutorial::{PageRun, TutorialHost};

/// Every case in a directory, sorted by id. Broken files are logged and skipped.
#[derive(Debug, Clone, Default)]
pub struct CaseLibrary {
    pub cases: Vec<Case>,
}

impl CaseLibrary {
    /// Every `*.ron` in `dir`.
    ///
    /// # Errors
    /// When the directory cannot be read.
    #[cfg(feature = "native")]
    pub fn load_dir(dir: &Path) -> std::io::Result<Self> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "ron"))
            .collect();
        files.sort();
        let cases = files
            .iter()
            .filter_map(|p| {
                let text = std::fs::read_to_string(p)
                    .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                    .ok()?;
                Case::from_ron(&text)
                    .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                    .ok()
            })
            .collect();
        Ok(Self { cases })
    }

    /// Every `cases/*.ron` of a downloaded bundle, in id order.
    #[must_use]
    pub fn from_bundle(bundle: &crate::assets::AssetBundle) -> Self {
        let cases = bundle
            .files_in("cases", ".ron")
            .filter_map(|p| {
                Case::from_ron(&bundle.text(p)?)
                    .inspect_err(|e| tracing::warn!("{p}: {e}"))
                    .ok()
            })
            .collect();
        Self { cases }
    }

    /// The installed cases (none without the `native` feature: the web build uses
    /// [`Self::from_bundle`]).
    #[must_use]
    pub fn load_default() -> Self {
        #[cfg(feature = "native")]
        {
            crate::scenarios::default_scenario_dir()
                .and_then(|d| d.parent().map(|p| p.join("cases")))
                .filter(|d| d.is_dir())
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

/// Where the learner is in the open case.
#[derive(Debug)]
struct Session {
    case: usize,
    stage: usize,
    /// Question of the stage being asked (`== questions.len()` once all are answered).
    question: usize,
    run: PageRun,
    /// Choice picked for the current question.
    chosen: Option<usize>,
    /// Questions answered correctly at the first try, and questions answered.
    correct: usize,
    answered: usize,
    finished: bool,
}

/// Case library and the open case.
pub struct Cases {
    pub library: CaseLibrary,
    session: Option<Session>,
    /// Best score so far per case id (this run of the application), `(correct, of)`.
    best: std::collections::BTreeMap<String, (usize, usize)>,
    markdown: CommonMarkCache,
}

impl std::fmt::Debug for Cases {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cases")
            .field("cases", &self.library.cases.len())
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

impl Cases {
    #[must_use]
    pub fn new(library: CaseLibrary) -> Self {
        Self {
            library,
            session: None,
            best: std::collections::BTreeMap::new(),
            markdown: CommonMarkCache::default(),
        }
    }

    /// The installed cases.
    #[must_use]
    pub fn load_default() -> Self {
        Self::new(CaseLibrary::load_default())
    }

    /// The cases of a downloaded bundle.
    #[must_use]
    pub fn from_bundle(bundle: &crate::assets::AssetBundle) -> Self {
        Self::new(CaseLibrary::from_bundle(bundle))
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.session.is_some()
    }

    /// Index of the open case and its current stage.
    #[must_use]
    pub fn position(&self) -> Option<(usize, usize)> {
        self.session.as_ref().map(|s| (s.case, s.stage))
    }

    /// Questions answered correctly at the first try, and questions answered, in the open case.
    #[must_use]
    pub fn score(&self) -> Option<(usize, usize)> {
        self.session.as_ref().map(|s| (s.correct, s.answered))
    }

    /// Open case `index` at its first stage.
    pub fn open(&mut self, index: usize) {
        let Some(c) = self.library.cases.get(index) else {
            return;
        };
        let Some(first) = c.stages.first() else {
            return;
        };
        self.session = Some(Session {
            case: index,
            stage: 0,
            question: 0,
            run: PageRun::from_steps(first.actions.clone()),
            chosen: None,
            correct: 0,
            answered: 0,
            finished: false,
        });
    }

    /// Close the reader (the best score of a finished case is kept).
    pub fn close(&mut self) {
        self.session = None;
    }

    /// Answer the current question with `choice` (only the first answer of a question counts).
    pub fn answer(&mut self, choice: usize) {
        let Some(s) = &mut self.session else { return };
        if s.chosen.is_some() {
            return;
        }
        let Some(q) = self.library.cases[s.case].stages[s.stage]
            .questions
            .get(s.question)
        else {
            return;
        };
        s.chosen = Some(choice);
        s.answered += 1;
        if choice == q.answer {
            s.correct += 1;
        }
    }

    /// `true` when the current question was answered, or the stage has none left to ask.
    #[must_use]
    pub fn can_advance(&self) -> bool {
        self.session.as_ref().is_some_and(|s| {
            let n = self.library.cases[s.case].stages[s.stage].questions.len();
            !s.finished && (s.question >= n || s.chosen.is_some())
        })
    }

    /// Next question, next stage, or the end of the case.
    pub fn advance(&mut self) {
        if !self.can_advance() {
            return;
        }
        let Some(s) = &mut self.session else { return };
        let case = &self.library.cases[s.case];
        s.chosen = None;
        let n = case.stages[s.stage].questions.len();
        if s.question < n {
            s.question += 1;
        }
        if s.question >= n {
            // all of this stage's questions are done: move on
            if let Some(next) = case.stages.get(s.stage + 1) {
                s.stage += 1;
                s.question = 0;
                s.run = PageRun::from_steps(next.actions.clone());
                // a stage without questions has nothing to ask: its button reads "Continue"
            } else {
                s.finished = true;
                let entry = self.best.entry(case.id.clone()).or_insert((0, 0));
                if s.correct >= entry.0 {
                    *entry = (s.correct, s.answered);
                }
            }
        }
    }

    /// Run the open stage's due actions against `host`. Call once per frame.
    pub fn poll(&mut self, host: &mut impl TutorialHost) {
        if let Some(s) = &mut self.session {
            s.run.poll(host);
        }
    }

    /// Take the open stage's runner out, so the host that owns `self` can be borrowed by it; hand
    /// it back with [`Self::restore_run`].
    pub fn take_run(&mut self) -> Option<PageRun> {
        self.session.as_mut().map(|s| std::mem::take(&mut s.run))
    }

    pub fn restore_run(&mut self, run: PageRun) {
        if let Some(s) = &mut self.session {
            s.run = run;
        }
    }

    /// The case list with Start buttons.
    pub fn library_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading(t!("case-heading"));
        ui.label(t!("case-intro"));
        if self.library.cases.is_empty() {
            ui.label(t!("case-none"));
        }
        let mut start = None;
        for (i, c) in self.library.cases.iter().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.strong(&c.title);
                    if let Some((ok, of)) = self.best.get(&c.id) {
                        ui.weak(t!("case-best", correct = ok.to_string(), total = of.to_string()));
                    }
                });
                ui.label(&c.patient);
                ui.label(&c.summary);
                ui.weak(t!(
                    "case-size",
                    stages = c.stages.len().to_string(),
                    questions = c.question_count().to_string()
                ));
                if ui.button(t!("case-start", title = c.title.as_str())).clicked() {
                    start = Some(i);
                }
            });
        }
        if let Some(i) = start {
            self.open(i);
        }
    }

    /// The reader for the open case (nothing if none is open).
    pub fn reader_ui(&mut self, ui: &mut egui::Ui) {
        let Some(s) = &self.session else { return };
        let (stage_i, question_i, chosen, finished) = (s.stage, s.question, s.chosen, s.finished);
        let (correct, answered) = (s.correct, s.answered);
        let running = !s.run.is_finished();
        let case = &self.library.cases[s.case];
        let total = case.question_count();
        let stage = &case.stages[stage_i];
        let mut close = false;
        let mut pick = None;
        let mut advance = false;

        ui.horizontal(|ui| {
            ui.heading(&case.title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t!("case-close")).clicked() {
                    close = true;
                }
            });
        });
        ui.label(&case.patient);
        ui.add(
            egui::ProgressBar::new(answered as f32 / total.max(1) as f32).text(t!(
                "case-score",
                correct = correct.to_string(),
                answered = answered.to_string(),
                total = total.to_string()
            )),
        );
        ui.separator();
        if finished {
            ui.strong(t!("case-finished"));
            ui.label(t!(
                "case-final-score",
                correct = correct.to_string(),
                total = total.to_string()
            ));
            ui.label(t!("case-first-try-hint"));
        } else {
            ui.strong(t!(
                "case-stage-of",
                stage = (stage_i + 1).to_string(),
                total = case.stages.len().to_string(),
                title = stage.title.as_str()
            ));
            egui::ScrollArea::vertical()
                .id_salt("case_stage")
                .max_height((ui.available_height() - 90.0).max(120.0))
                .show(ui, |ui| {
                    CommonMarkViewer::new().show(ui, &mut self.markdown, &stage.markdown);
                    if let Some(q) = stage.questions.get(question_i) {
                        ui.separator();
                        ui.strong(t!(
                            "case-question-of",
                            question = (question_i + 1).to_string(),
                            total = stage.questions.len().to_string()
                        ));
                        ui.label(&q.prompt);
                        for (i, c) in q.choices.iter().enumerate() {
                            let r = ui.add_enabled(
                                chosen.is_none(),
                                egui::Button::selectable(chosen == Some(i), c),
                            );
                            if r.clicked() {
                                pick = Some(i);
                            }
                        }
                        if let Some(c) = chosen {
                            if c == q.answer {
                                cards::result_box(ui, true, t!("feedback-correct"));
                            } else {
                                cards::result_box(
                                    ui,
                                    false,
                                    t!("case-wrong", answer = q.choices[q.answer].as_str()),
                                );
                            }
                            CommonMarkViewer::new().show(ui, &mut self.markdown, &q.explanation);
                        }
                    }
                });
        }
        ui.separator();
        ui.horizontal(|ui| {
            if !finished {
                let last = stage_i + 1 == case.stages.len() && question_i + 1 >= stage.questions.len();
                let label = if stage.questions.is_empty() || chosen.is_some() {
                    if last {
                        t!("case-finish")
                    } else {
                        t!("case-continue")
                    }
                } else {
                    t!("case-answer-first")
                };
                if ui
                    .add_enabled(self.can_advance(), egui::Button::new(label))
                    .clicked()
                {
                    advance = true;
                }
            }
            if running {
                ui.spinner();
                ui.weak(t!("tut-running"));
            }
        });
        if let Some(i) = pick {
            self.answer(i);
        }
        if advance {
            self.advance();
        }
        if close {
            self.close();
        }
    }
}
