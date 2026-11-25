//! The standard echo views on the Echo page (brief B33): the words of the hints and names, and the
//! self-test (a sequence of views asked without hints, a time and a best score per view, a summary,
//! progress saved like the other quizzes).
//!
//! Targets, score and hints are `heartsim_echo::views` (pure functions of the pose); this module only
//! words them and keeps the self-test's clock. The clock is advanced by the caller with the frame's
//! real time, so tests drive it with fixed steps.

use std::collections::BTreeMap;

use heartsim_echo::view_hints::{Direction, Hint, HintKind, Movement, PatientDir};
use heartsim_echo::{EchoAnatomy, FoundTracker, ViewId, ViewTargets, ViewTolerances};
use heartsim_ui::i18n::{tr, tr_args};
use serde::{Deserialize, Serialize};

/// The views the self-test asks: those the anatomy can show, in teaching order. Searches every
/// view's reference pose the first time (about a second each natively): this blocks; the page
/// prepares them one per frame with [`ViewPreparation`] instead.
#[must_use]
pub fn shipped_views(anatomy: &EchoAnatomy, views: &ViewTargets) -> Vec<ViewId> {
    ViewId::ALL
        .into_iter()
        .filter(|id| views.reference(anatomy, *id).reachable)
        .collect()
}

/// The self-test's views prepared incrementally: each [`Self::step`] searches the reference pose of
/// one view (in [`ViewId::ALL`] order; a reference already searched costs nothing), so the page can
/// draw a progress bar between them and stay responsive, without threads (the web build has none).
/// The references are `ViewTargets`' own (each searched once, deterministically), so the result is
/// that of [`shipped_views`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewPreparation {
    next: usize,
}

impl ViewPreparation {
    /// Views prepared so far and the number to prepare.
    #[must_use]
    pub fn progress(&self) -> (usize, usize) {
        (self.next, ViewId::ALL.len())
    }

    /// Prepare the next view; when every view is prepared, the views the self-test asks.
    pub fn step(&mut self, anatomy: &EchoAnatomy, views: &ViewTargets) -> Option<Vec<ViewId>> {
        if let Some(id) = ViewId::ALL.get(self.next) {
            let _ = views.reference(anatomy, *id);
            self.next += 1;
            return None;
        }
        Some(shipped_views(anatomy, views))
    }
}

/// eframe storage key of the self-test's best results.
pub const STORAGE_KEY: &str = "echo_views_selftest";

/// The name of a view in the interface language.
#[must_use]
pub fn view_name(id: ViewId) -> String {
    tr(&format!("echo-view-{}", id.id()))
}

/// The name of a landmark or checklist key (`echo-lm-…`, `echo-chk-…`).
#[must_use]
pub fn landmark_name(key: &str) -> String {
    tr(&format!("echo-lm-{key}"))
}

/// The wording of a checklist line.
#[must_use]
pub fn checklist_text(key: &str) -> String {
    tr(&format!("echo-chk-{key}"))
}

/// Why a view is not reachable (`echo-unreachable-<key>`).
#[must_use]
pub fn unreachable_text(key: &str) -> String {
    tr(&format!("echo-unreachable-{key}"))
}

fn direction_words(d: PatientDir) -> String {
    tr(&format!("echo-dir-{}", d.key()))
}

/// The hint in words: what to do and, for a movement, which way on the patient.
#[must_use]
pub fn hint_text(h: &Hint) -> String {
    let dir = || match h.direction {
        Direction::Patient(p) => direction_words(p),
        _ => String::new(),
    };
    match h.kind {
        HintKind::Found => tr("echo-hint-found"),
        HintKind::LostContact => tr("echo-hint-lost-contact"),
        HintKind::OverRib => tr_args("echo-hint-rib", &[("dir", dir().into())]),
        HintKind::OverLung => tr_args("echo-hint-lung", &[("dir", dir().into())]),
        HintKind::WrongMarkerSide => tr("echo-hint-marker"),
        HintKind::Foreshortened => tr_args("echo-hint-foreshortened", &[("dir", dir().into())]),
        HintKind::Move => match (h.movement, h.direction) {
            (Movement::Slide, _) => tr_args("echo-hint-slide", &[("dir", dir().into())]),
            (Movement::Tilt, _) => tr_args("echo-hint-tilt", &[("dir", dir().into())]),
            (Movement::Rock, _) => tr_args("echo-hint-rock", &[("dir", dir().into())]),
            (Movement::Angle, _) => tr_args("echo-hint-angle", &[("dir", dir().into())]),
            (Movement::Rotate, Direction::Clockwise) => tr("echo-hint-rotate-cw"),
            (Movement::Rotate, _) => tr("echo-hint-rotate-ccw"),
            (Movement::Depth, Direction::Deeper) => tr("echo-hint-deeper"),
            (Movement::Depth, _) => tr("echo-hint-shallower"),
        },
    }
}

/// Best result of one view, kept between sessions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Best {
    /// Highest score reached.
    pub score: f32,
    /// Shortest time to find it, s (`None`: never found).
    pub time_s: Option<f32>,
}

/// The saved part of the self-test: the best result per view id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QuizProgress {
    pub best: BTreeMap<String, Best>,
}

impl QuizProgress {
    /// RON text for eframe storage.
    #[must_use]
    pub fn to_text(&self) -> String {
        ron::to_string(self).unwrap_or_default()
    }

    /// Parse [`Self::to_text`]; malformed text is an empty progress.
    #[must_use]
    pub fn from_text(s: &str) -> Self {
        ron::from_str(s).unwrap_or_default()
    }

    fn merge(&mut self, id: ViewId, score: f32, time_s: Option<f32>) {
        let e = self.best.entry(id.id().to_string()).or_insert(Best {
            score: 0.0,
            time_s: None,
        });
        e.score = e.score.max(score);
        e.time_s = match (e.time_s, time_s) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }
}

/// One view's result in the running self-test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunResult {
    pub view: ViewId,
    /// Highest score while the view was asked.
    pub best_score: f32,
    /// Time to find it, s; `None` if it was skipped.
    pub time_s: Option<f32>,
}

/// Where the self-test is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Not started.
    Idle,
    /// A view is asked and the clock runs.
    Running,
    /// The view was found or skipped; waiting for *Next*.
    Between,
    /// Every view asked; the summary table is shown.
    Summary,
}

/// The self-test: a fixed sequence of views, a clock and the best score for each.
#[derive(Debug, Clone)]
pub struct EchoQuiz {
    order: Vec<ViewId>,
    index: usize,
    phase: Phase,
    elapsed_s: f32,
    best_score: f32,
    tracker: FoundTracker,
    results: Vec<RunResult>,
    /// Best results of earlier sessions and this one.
    pub progress: QuizProgress,
}

impl Default for EchoQuiz {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            index: 0,
            phase: Phase::Idle,
            elapsed_s: 0.0,
            best_score: 0.0,
            tracker: FoundTracker::default(),
            results: Vec::new(),
            progress: QuizProgress::default(),
        }
    }
}

impl EchoQuiz {
    /// Start over with `views` (in teaching order): the first is asked at once.
    pub fn start(&mut self, views: Vec<ViewId>) {
        self.order = views;
        self.index = 0;
        self.results.clear();
        self.begin_view();
        self.phase = if self.order.is_empty() {
            Phase::Summary
        } else {
            Phase::Running
        };
    }

    fn begin_view(&mut self) {
        self.elapsed_s = 0.0;
        self.best_score = 0.0;
        self.tracker.reset();
    }

    /// Back to the idle page (the progress is kept).
    pub fn stop(&mut self) {
        self.phase = Phase::Idle;
    }

    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The view being asked (or just done).
    #[must_use]
    pub fn current(&self) -> Option<ViewId> {
        self.order.get(self.index).copied()
    }

    /// 1-based position and total.
    #[must_use]
    pub fn position(&self) -> (usize, usize) {
        ((self.index + 1).min(self.order.len()), self.order.len())
    }

    /// Seconds on the current view.
    #[must_use]
    pub fn elapsed_s(&self) -> f32 {
        self.elapsed_s
    }

    /// The results so far in this session.
    #[must_use]
    pub fn results(&self) -> &[RunResult] {
        &self.results
    }

    fn finish_view(&mut self, time_s: Option<f32>) {
        let Some(view) = self.current() else { return };
        self.results.push(RunResult {
            view,
            best_score: self.best_score,
            time_s,
        });
        self.progress.merge(view, self.best_score, time_s);
        self.phase = Phase::Between;
    }

    /// Advance the clock by `dt_s` at `score` (the current pose's score for the asked view). The
    /// view is found when the score has been held; returns `true` on the frame it is.
    pub fn tick(&mut self, tol: &ViewTolerances, score: f32, dt_s: f32) -> bool {
        if self.phase != Phase::Running {
            return false;
        }
        self.elapsed_s += dt_s.max(0.0);
        self.best_score = self.best_score.max(score);
        if self.tracker.update(tol, score, dt_s) {
            // the time counted is when the score first reached the threshold, not the hold's end
            let t = (self.elapsed_s - tol.found_hold_s).max(0.0);
            self.finish_view(Some(t));
            return true;
        }
        false
    }

    /// Give up on the current view.
    pub fn skip(&mut self) {
        if self.phase == Phase::Running {
            self.finish_view(None);
        }
    }

    /// Next view, or the summary after the last. Returns the view now asked.
    #[allow(clippy::should_implement_trait, reason = "the next view, not an iterator")]
    pub fn next(&mut self) -> Option<ViewId> {
        if self.phase != Phase::Between {
            return None;
        }
        self.index += 1;
        if self.index >= self.order.len() {
            self.phase = Phase::Summary;
            return None;
        }
        self.begin_view();
        self.phase = Phase::Running;
        self.current()
    }

    /// Mean of the best scores of this session (0 with none).
    #[must_use]
    pub fn mean_score(&self) -> f32 {
        if self.results.is_empty() {
            0.0
        } else {
            self.results.iter().map(|r| r.best_score).sum::<f32>() / self.results.len() as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_round_trips_and_ignores_garbage() {
        let mut p = QuizProgress::default();
        p.merge(ViewId::ApicalFourChamber, 0.93, Some(41.5));
        p.merge(ViewId::ApicalFourChamber, 0.81, Some(30.0));
        p.merge(ViewId::ParasternalLongAxis, 0.4, None);
        let back = QuizProgress::from_text(&p.to_text());
        assert_eq!(back, p);
        let a = back.best["a4c"];
        assert!((a.score - 0.93).abs() < 1e-6 && a.time_s == Some(30.0));
        assert_eq!(QuizProgress::from_text("not ron"), QuizProgress::default());
    }

    #[test]
    fn the_clock_finds_a_view_after_the_hold_and_skips() {
        let tol = ViewTolerances::default();
        let mut q = EchoQuiz::default();
        q.start(vec![ViewId::ParasternalLongAxis, ViewId::ApicalFourChamber]);
        assert_eq!(q.phase(), Phase::Running);
        for _ in 0..30 {
            assert!(!q.tick(&tol, 0.3, 0.1));
        }
        let mut found = false;
        for _ in 0..12 {
            found |= q.tick(&tol, 0.95, 0.1);
        }
        assert!(found);
        assert_eq!(q.phase(), Phase::Between);
        let r = q.results()[0];
        assert!((r.best_score - 0.95).abs() < 1e-6);
        assert!((r.time_s.unwrap() - 3.0).abs() < 0.3, "{:?}", r.time_s);
        assert_eq!(q.next(), Some(ViewId::ApicalFourChamber));
        q.tick(&tol, 0.5, 0.5);
        q.skip();
        assert_eq!(q.results()[1].time_s, None);
        assert_eq!(q.next(), None);
        assert_eq!(q.phase(), Phase::Summary);
        assert!(q.mean_score() > 0.7);
        assert!(q.progress.best.contains_key("plax") && q.progress.best.contains_key("a4c"));
    }
}
