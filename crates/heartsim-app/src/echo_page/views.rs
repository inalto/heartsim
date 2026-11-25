//! The standard views on the Echo page (brief B33): the picker, the score bar, the hint line, the
//! checklist, *Show me*, the ghost of the target plane, the names on the image, and the self-test.
//!
//! Everything here reads `heartsim_echo::views`, whose score, hints and reference poses are pure
//! functions of the anatomy at rest and the probe's pose: nothing on this page changes what the
//! slice or the image show. The score is recomputed only when the pose or the asked view changed (a
//! few tens of microseconds); the checklist only when a new slice was made. The reference pose of a
//! view is searched the first time the view is asked for (about half a second, once).

use super::{EchoPage, EchoSection, ProbeFrame, t};
use crate::echo_views::{
    self, EchoQuiz, Phase, ViewPreparation, checklist_text, hint_text, landmark_name, shipped_views,
    unreachable_text, view_name,
};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2 as EVec2};
use heartsim_echo::views::lerp_pose;
use heartsim_echo::{
    EchoAnatomy, EchoWindow, FoundTracker, Hint, HintKind, ImageStats, ProbePose, Reach, ViewId, ViewScore,
    ViewTargets,
};
use heartsim_render::SectionLayout;
use heartsim_ui::cards;

/// How long *Show me* takes, s. **Designed**; instant with reduced motion.
const SHOW_ME_S: f32 = 1.4;

/// The page's view state, one struct so the main file keeps one field.
#[derive(Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "two user switches and independent frame requests"
)]
pub(super) struct ViewsState {
    /// The view asked for, `None` to explore freely.
    pub target: Option<ViewId>,
    pub tracker: FoundTracker,
    pub score: Option<ViewScore>,
    pub hint: Option<Hint>,
    /// What `score` and `hint` were made for.
    scored: Option<(ViewId, ProbePose)>,
    pub checklist: Vec<bool>,
    /// The slice generation (plus one) and view the checklist was made for.
    checklist_for: Option<(u64, ViewId)>,
    pub ghost: bool,
    pub labels: bool,
    show_me: Option<(ProbePose, ProbePose, f32)>,
    pub quiz: EchoQuiz,
    pub selftest: bool,
    /// The self-test was started and its views are being prepared, one per frame; the flag says the
    /// progress was drawn since the last step (so the first frame shows it before any work).
    preparation: Option<(ViewPreparation, bool)>,
    last_time: Option<f64>,
    pending_view: Option<ViewId>,
    pending_probe_at: Option<ViewId>,
    pending_script: bool,
    marks: Vec<MarkDraw>,
    /// Views whose reference was searched and found not reachable.
    pub known_unreachable: Vec<ViewId>,
}

impl Default for ViewsState {
    fn default() -> Self {
        Self {
            target: None,
            tracker: FoundTracker::default(),
            score: None,
            hint: None,
            scored: None,
            checklist: Vec::new(),
            checklist_for: None,
            ghost: true,
            labels: true,
            show_me: None,
            quiz: EchoQuiz::default(),
            selftest: false,
            preparation: None,
            last_time: None,
            pending_view: None,
            pending_probe_at: None,
            pending_script: false,
            marks: Vec::new(),
            known_unreachable: Vec::new(),
        }
    }
}

/// A structure named on the cross-section.
#[derive(Debug, Clone)]
struct MarkDraw {
    text: String,
    angle_deg: f32,
    range_mm: f32,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl EchoPage {
    // ---- state the tests and the app read ---------------------------------------------------

    /// The view asked for.
    #[must_use]
    pub fn target(&self) -> Option<ViewId> {
        self.views.target
    }

    /// Choose the view asked for (`None`: explore freely); the found clock starts over.
    pub fn set_target(&mut self, id: Option<ViewId>) {
        if id.is_some() {
            self.section = EchoSection::Views;
        }
        if self.views.target != id {
            self.views.target = id;
            self.views.tracker.reset();
            self.views.score = None;
            self.views.hint = None;
            self.views.scored = None;
            self.views.checklist_for = None;
            self.views.show_me = None;
        }
    }

    /// Ask for `id` as soon as the anatomy is built (now, if it is): the tutorials' action.
    pub fn request_view(&mut self, id: ViewId) {
        self.section = EchoSection::Views;
        self.views.pending_view = Some(id);
    }

    /// Put the probe at `id`'s reference pose as soon as the anatomy is built.
    pub fn request_probe_at_view(&mut self, id: ViewId) {
        self.views.pending_probe_at = Some(id);
    }

    /// The score of the current pose for the view asked for.
    #[must_use]
    pub fn view_score(&self) -> Option<&ViewScore> {
        self.views.score.as_ref()
    }

    /// The hint for the current pose (while a view is asked for and not found).
    #[must_use]
    pub fn view_hint(&self) -> Option<Hint> {
        self.views.hint
    }

    /// Which lines of the view's checklist the current slice satisfies.
    #[must_use]
    pub fn view_checklist(&self) -> &[bool] {
        &self.views.checklist
    }

    /// Whether `id` is the view asked for and its score has been held for a second.
    #[must_use]
    pub fn view_found(&self, id: ViewId) -> bool {
        self.views.target == Some(id) && self.views.tracker.found()
    }

    /// Whether *Show me* is playing.
    #[must_use]
    pub fn show_me_active(&self) -> bool {
        self.views.show_me.is_some()
    }

    /// The target plane's ghost in the 3-D heart is switched on.
    #[must_use]
    pub fn ghost_on(&self) -> bool {
        self.views.ghost
    }

    /// Switch the ghost plane.
    pub fn set_ghost(&mut self, on: bool) {
        self.views.ghost = on;
    }

    /// The names on the image are switched on.
    #[must_use]
    pub fn labels_on(&self) -> bool {
        self.views.labels
    }

    /// Switch the names on the image.
    pub fn set_labels(&mut self, on: bool) {
        self.views.labels = on;
    }

    /// The self-test.
    #[must_use]
    pub fn quiz(&self) -> &EchoQuiz {
        &self.views.quiz
    }

    /// The self-test, to restore saved progress.
    pub fn quiz_mut(&mut self) -> &mut EchoQuiz {
        &mut self.views.quiz
    }

    /// Whether the page is in self-test mode.
    #[must_use]
    pub fn selftest(&self) -> bool {
        self.views.selftest
    }

    /// Enter or leave the self-test mode.
    pub fn set_selftest(&mut self, on: bool) {
        if on {
            self.section = EchoSection::Views;
        }
        if self.views.selftest != on {
            self.views.selftest = on;
            self.views.quiz.stop();
            self.views.preparation = None;
            self.set_target(None);
        }
    }

    /// Start the self-test: its views are prepared one per frame (a progress bar meanwhile), then
    /// the first is asked.
    pub fn start_selftest(&mut self) {
        self.section = EchoSection::Views;
        self.views.selftest = true;
        self.views.quiz.stop();
        self.views.preparation = Some((ViewPreparation::default(), false));
    }

    /// While the self-test's views are being prepared: how many are done, of how many.
    #[must_use]
    pub fn selftest_preparing(&self) -> Option<(usize, usize)> {
        self.views.preparation.as_ref().map(|(p, _)| p.progress())
    }

    /// For the documentation tour only: fill the self-test with a scripted session (every view found
    /// at its reference pose after a made-up time, every fourth skipped), as soon as the anatomy is
    /// built, so that its summary can be photographed. The scores are the views' own.
    pub fn script_selftest(&mut self) {
        self.views.pending_script = true;
    }

    /// The self-test's best results as stored.
    #[must_use]
    pub fn quiz_string(&self) -> String {
        self.views.quiz.progress.to_text()
    }

    /// Restore results saved with [`Self::quiz_string`].
    pub fn restore_quiz(&mut self, s: &str) {
        self.views.quiz.progress = echo_views::QuizProgress::from_text(s);
    }

    /// Play *Show me*: the probe moves to the view's reference pose (at once with reduced motion).
    pub fn show_me(&mut self, anatomy: &EchoAnatomy, views: &ViewTargets, reduced_motion: bool) {
        let (Some(id), Some(pose)) = (self.views.target, self.pose) else {
            return;
        };
        let to = views.reference(anatomy, id).pose;
        if reduced_motion {
            self.pose = Some(to);
            self.window = None;
            self.views.show_me = None;
        } else {
            self.views.show_me = Some((pose, to, 0.0));
        }
    }

    // ---- per frame ----------------------------------------------------------------------------

    /// Everything the views need once per frame, before the controls are drawn: pending requests, the
    /// self-test, *Show me*, the score, the hint and the checklist.
    pub(super) fn update_views(&mut self, ui: &egui::Ui, heart: &crate::heart_view::HeartView) {
        let (Some(anatomy), Some(views)) = (heart.echo_anatomy(), heart.echo_views()) else {
            return;
        };
        let now = ui.input(|i| i.time);
        let dt = self
            .views
            .last_time
            .map_or(0.0, |l| (now - l).clamp(0.0, 0.25) as f32);
        self.views.last_time = Some(now);

        if let Some(id) = self.views.pending_view.take() {
            self.views.selftest = false;
            self.set_target(Some(id));
            self.views.tracker.reset();
        }
        if let Some(id) = self.views.pending_probe_at.take() {
            let r = views.reference(anatomy, id).pose;
            self.pose = Some(r);
            self.window = None;
        }
        // the self-test: prepare its views one per frame (each reference searched once, after the
        // progress was drawn), then ask the first
        if let Some((prep, drawn)) = &mut self.views.preparation {
            ui.ctx().request_repaint();
            if std::mem::take(drawn)
                && let Some(shipped) = prep.step(anatomy, views)
            {
                self.views.preparation = None;
                self.views.known_unreachable =
                    ViewId::ALL.into_iter().filter(|v| !shipped.contains(v)).collect();
                self.views.quiz.start(shipped);
                self.set_target(self.views.quiz.current());
                self.start_current_test_view(anatomy);
            }
        }
        if std::mem::take(&mut self.views.pending_script) {
            let shipped = shipped_views(anatomy, views);
            self.section = EchoSection::Views;
            self.views.selftest = true;
            self.views.quiz.start(shipped);
            let tol = &views.tolerances;
            for k in 0..self.views.quiz.position().1 {
                let id = self.views.quiz.current();
                self.views.quiz.tick(tol, 0.3, 9.0 + 5.0 * k as f32);
                if k % 4 == 3 {
                    self.views.quiz.skip();
                } else {
                    let s = id.map_or(0.95, |id| views.reference(anatomy, id).score.total);
                    self.views.quiz.tick(tol, s, tol.found_hold_s + 0.05);
                }
                self.views.quiz.next();
            }
            self.set_target(None);
        }
        // Show me
        if let Some((from, to, t)) = self.views.show_me {
            if self.drag.is_some() {
                self.views.show_me = None;
            } else {
                let t = t + dt / SHOW_ME_S;
                self.pose = Some(lerp_pose(anatomy, &from, &to, smooth(t)));
                self.window = None;
                self.views.show_me = (t < 1.0).then_some((from, to, t));
                ui.ctx().request_repaint();
            }
        }
        // the score, hint and found clock
        let Some(id) = self.views.target else {
            self.views.marks.clear();
            return;
        };
        let Some(pose) = self.pose else { return };
        let pose = pose.clamped();
        if self.views.scored != Some((id, pose)) {
            let s = views.score(anatomy, id, &pose);
            self.views.hint = Some(views.hint(anatomy, id, &pose));
            self.views.score = Some(s);
            self.views.scored = Some((id, pose));
            self.views.marks = Self::mark_positions(views, id, anatomy, &pose);
        }
        let total = self.views.score.as_ref().map_or(0.0, |s| s.total);
        let found = self.views.tracker.update(&views.tolerances, total, dt);
        if self.views.selftest && self.views.quiz.phase() == Phase::Running {
            if self.views.quiz.tick(&views.tolerances, total, dt) {
                self.views.hint = None;
            }
            ui.ctx().request_repaint();
        } else if self.views.target.is_some() && !found && self.views.tracker.held_s() > 0.0 {
            ui.ctx().request_repaint();
        }
        // the checklist, when a new slice was made
        if !self.views.selftest
            && self.views.checklist_for != Some((self.generation, id))
            && self.generation > 0
        {
            let stats = ImageStats::of(&self.image, anatomy);
            self.views.checklist = views.checklist(id, &stats, &pose, anatomy);
            self.views.checklist_for = Some((self.generation, id));
        }
    }

    fn start_current_test_view(&mut self, anatomy: &EchoAnatomy) {
        if let Some(v) = self.views.quiz.current() {
            let w: EchoWindow = v.window();
            self.go_to_window(anatomy, w);
        }
    }

    fn mark_positions(
        views: &ViewTargets,
        id: ViewId,
        anatomy: &EchoAnatomy,
        pose: &ProbePose,
    ) -> Vec<MarkDraw> {
        let f = pose.frame(anatomy.designed.contact_limit_deg);
        if !f.contact {
            return Vec::new();
        }
        views
            .target(id)
            .marks
            .iter()
            .map(|m| {
                let rel = m.pos_mm - f.apex_mm;
                MarkDraw {
                    text: landmark_name(m.key),
                    angle_deg: rel.dot(f.marker).atan2(rel.dot(f.beam)).to_degrees(),
                    range_mm: rel.length(),
                }
            })
            .collect()
    }

    /// The reference pose's frame, depth and sector, for the ghost plane in the 3-D heart: only while
    /// a reachable view is asked for in practice mode with the ghost on.
    pub(super) fn ghost_frame(&self, heart: &crate::heart_view::HeartView) -> Option<(ProbeFrame, f32, f32)> {
        if self.views.selftest || !self.views.ghost {
            return None;
        }
        let (anatomy, views, id) = (heart.echo_anatomy()?, heart.echo_views()?, self.views.target?);
        if !matches!(views.target(id).reach, Reach::Reachable) {
            return None;
        }
        let r = views.reference(anatomy, id);
        r.reachable.then(|| {
            (
                r.pose.frame(anatomy.designed.contact_limit_deg),
                r.pose.depth_mm,
                r.pose.sector_deg,
            )
        })
    }

    // ---- the names on the image ---------------------------------------------------------------

    pub(super) fn paint_marks(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        layout: &SectionLayout,
    ) {
        if !self.views.labels || self.views.selftest || self.views.target.is_none() || !self.image.contact {
            return;
        }
        let ink = ui.visuals().text_color();
        let plate = ui.visuals().extreme_bg_color.gamma_multiply(0.8);
        let lines = self.image.lines;
        for m in &self.views.marks {
            if m.angle_deg.abs() > layout.sector_deg * 0.5 || m.range_mm > layout.depth_mm {
                continue;
            }
            let line_f = lines as f32 * (m.angle_deg / layout.sector_deg + 0.5);
            let p = layout.point_of(line_f, lines, m.range_mm);
            let at = Pos2::new(rect.left() + p.x, rect.top() + p.y);
            painter.circle_filled(at, 3.5, Color32::WHITE);
            painter.circle_stroke(at, 3.5, Stroke::new(1.0, Color32::BLACK));
            let galley = painter.layout_no_wrap(m.text.clone(), egui::FontId::proportional(11.0), ink);
            let pos = at + EVec2::new(6.0, -galley.size().y * 0.5);
            painter.rect_filled(Rect::from_min_size(pos, galley.size()).expand(2.0), 2.0, plate);
            painter.galley(pos, galley, ink);
        }
    }

    // ---- the cards -----------------------------------------------------------------------------

    /// The Views section: the practise / self-test switch, then the picker with the score, hint and
    /// checklist, or the self-test.
    pub(super) fn views_card(&mut self, ui: &mut egui::Ui, heart: &crate::heart_view::HeartView) {
        let (Some(anatomy), Some(views)) = (heart.echo_anatomy(), heart.echo_views()) else {
            return;
        };
        let reduced = heart.motion().is_some_and(|m| m.reduced_motion);
        cards::card(ui, |ui| {
            let mut st = self.views.selftest;
            let options = [(false, t!("echo-mode-explore")), (true, t!("echo-mode-selftest"))];
            if heartsim_ui::segmented(ui, &mut st, &options) {
                self.set_selftest(st);
            }
            ui.add_space(4.0);
            if self.views.selftest {
                self.selftest_ui(ui, anatomy, views);
            } else {
                self.practice_ui(ui, anatomy, views, reduced);
            }
        });
    }

    fn practice_ui(&mut self, ui: &mut egui::Ui, anatomy: &EchoAnatomy, views: &ViewTargets, reduced: bool) {
        heartsim_ui::info_disclosure(
            ui,
            "echo_views_info",
            t!("echo-views-line"),
            t!("echo-views-note"),
        );
        // what the view asked for needs (score, hint, checklist) first, the list of views after it
        self.status_ui(ui, anatomy, views, reduced);
        ui.add_space(4.0);
        ui.strong(t!("echo-views-all"));
        for w in EchoWindow::ALL {
            let group: Vec<ViewId> = ViewId::ALL.into_iter().filter(|v| v.window() == w).collect();
            ui.add_space(2.0);
            ui.strong(match w {
                EchoWindow::Parasternal => t!("echo-views-group-parasternal"),
                EchoWindow::Apical => t!("echo-views-group-apical"),
                EchoWindow::Subcostal => t!("echo-views-group-subcostal"),
                EchoWindow::Suprasternal => t!("echo-views-group-suprasternal"),
            });
            for id in group {
                let unreachable = !matches!(views.target(id).reach, Reach::Reachable)
                    || self.views.known_unreachable.contains(&id);
                let mut name = view_name(id);
                if unreachable {
                    name = format!("{name} ({})", t!("echo-view-not-reachable"));
                }
                if ui
                    .add(egui::Button::selectable(self.views.target == Some(id), name))
                    .clicked()
                {
                    self.set_target(Some(id));
                    self.go_to_window(anatomy, id.window());
                }
            }
        }
    }

    fn status_ui(&mut self, ui: &mut egui::Ui, anatomy: &EchoAnatomy, views: &ViewTargets, reduced: bool) {
        let Some(id) = self.views.target else {
            heartsim_ui::helper_text(ui, t!("echo-view-none"));
            return;
        };
        ui.separator();
        ui.strong(view_name(id));
        let c = id.citation();
        heartsim_ui::helper_text(
            ui,
            format!(
                "{}{}",
                t!("echo-citation", pages = c.pages),
                if c.verified { "" } else { " (page not verified)" }
            ),
        );
        let target = views.target(id);
        let reference = match target.reach {
            Reach::NotReachable(why) => {
                ui.add(egui::Label::new(t!("echo-unreachable", why = unreachable_text(why))).wrap());
                return;
            }
            Reach::Reachable => views.reference(anatomy, id),
        };
        if !reference.reachable {
            if !self.views.known_unreachable.contains(&id) {
                self.views.known_unreachable.push(id);
            }
            ui.add(
                egui::Label::new(t!(
                    "echo-unreachable",
                    why = t!(
                        "echo-unreachable-score",
                        score = format!("{:.0}", reference.score.total * 100.0),
                        found = format!("{:.0}", views.tolerances.found_threshold * 100.0)
                    )
                ))
                .wrap(),
            );
            return;
        }
        let total = self.views.score.as_ref().map_or(0.0, |s| s.total);
        ui.label(t!("echo-score"));
        ui.add(
            egui::ProgressBar::new(total)
                .text(t!("echo-score-text", score = format!("{:.0}", total * 100.0))),
        );
        if self.views.tracker.found() {
            cards::result_box(ui, true, t!("echo-found"));
        } else {
            heartsim_ui::helper_text(
                ui,
                t!(
                    "echo-not-found",
                    found = format!("{:.0}", views.tolerances.found_threshold * 100.0),
                    hold = format!("{:.0}", views.tolerances.found_hold_s)
                ),
            );
            if let Some(h) = self.views.hint
                && h.kind != HintKind::Found
            {
                ui.strong(t!("echo-hint-heading"));
                ui.add(egui::Label::new(hint_text(&h)).wrap());
            }
        }
        // the card's one primary action
        if heartsim_ui::primary_button(ui, t!("echo-show-me"))
            .on_hover_text(t!("echo-show-me-hint"))
            .clicked()
        {
            self.show_me(anatomy, views, reduced);
        }
        ui.checkbox(&mut self.views.ghost, t!("echo-ghost"));
        ui.checkbox(&mut self.views.labels, t!("echo-labels"));
        ui.strong(t!("echo-checklist"));
        for (item, ok) in target.checklist.iter().zip(
            self.views
                .checklist
                .iter()
                .copied()
                .chain(std::iter::repeat(false)),
        ) {
            heartsim_ui::status_row(ui, ok, &checklist_text(item.key));
        }
    }

    fn selftest_ui(&mut self, ui: &mut egui::Ui, anatomy: &EchoAnatomy, views: &ViewTargets) {
        ui.strong(t!("echo-test-heading"));
        if let Some((prep, drawn)) = &mut self.views.preparation {
            let (n, total) = prep.progress();
            *drawn = true;
            let shown = (n + 1).min(total);
            ui.add(egui::ProgressBar::new(n as f32 / total.max(1) as f32).text(t!(
                "echo-test-preparing",
                n = shown.to_string(),
                total = total.to_string()
            )));
            heartsim_ui::helper_text(ui, t!("echo-test-preparing-note"));
            return;
        }
        match self.views.quiz.phase() {
            Phase::Idle => {
                heartsim_ui::helper_text(ui, t!("echo-test-note"));
                if heartsim_ui::primary_button(ui, t!("echo-test-start")).clicked() {
                    self.start_selftest();
                }
                self.records_ui(ui);
            }
            Phase::Running | Phase::Between => {
                let (n, total) = self.views.quiz.position();
                ui.label(t!(
                    "echo-test-progress",
                    n = n.to_string(),
                    total = total.to_string()
                ));
                if let Some(v) = self.views.quiz.current() {
                    ui.strong(t!("echo-test-find", view = view_name(v)));
                }
                if self.views.quiz.phase() == Phase::Running {
                    ui.label(t!(
                        "echo-test-time",
                        time = format!("{:.0}", self.views.quiz.elapsed_s())
                    ));
                    if ui.button(t!("echo-test-skip")).clicked() {
                        self.views.quiz.skip();
                    }
                } else {
                    if let Some(r) = self.views.quiz.results().last() {
                        let score = format!("{:.0}", r.best_score * 100.0);
                        match r.time_s {
                            Some(s) => cards::result_box(
                                ui,
                                true,
                                t!("echo-test-found", time = format!("{s:.0}"), score = score),
                            ),
                            None => cards::result_box(ui, false, t!("echo-test-skipped", score = score)),
                        };
                    }
                    if ui.button(t!("echo-test-next")).clicked() {
                        self.views.quiz.next();
                        self.set_target(self.views.quiz.current());
                        self.start_current_test_view(anatomy);
                    }
                }
            }
            Phase::Summary => {
                ui.strong(t!("echo-test-summary"));
                let rows: Vec<(ViewId, f32, Option<f32>)> = self
                    .views
                    .quiz
                    .results()
                    .iter()
                    .map(|r| (r.view, r.best_score, r.time_s))
                    .collect();
                results_table(ui, "echo_test_summary", &rows);
                ui.label(t!(
                    "echo-test-mean",
                    score = format!("{:.0}", self.views.quiz.mean_score() * 100.0)
                ));
                heartsim_ui::helper_text(
                    ui,
                    t!(
                        "echo-test-skin",
                        n = self.views.quiz.results().len().to_string(),
                        total = ViewId::ALL.len().to_string()
                    ),
                );
                // the best results over all sessions are not repeated here: the table is this run
                ui.horizontal_wrapped(|ui| {
                    if ui.button(t!("echo-test-restart")).clicked() {
                        self.start_selftest();
                    }
                    if ui.button(t!("echo-test-leave")).clicked() {
                        self.set_selftest(false);
                    }
                });
            }
        }
        let _ = views;
    }

    /// The best result of each view over all sessions, as the summary's table.
    fn records_ui(&self, ui: &mut egui::Ui) {
        if self.views.quiz.progress.best.is_empty() {
            return;
        }
        ui.add_space(4.0);
        ui.strong(t!("echo-test-col-record"));
        let rows: Vec<(ViewId, f32, Option<f32>)> = ViewId::ALL
            .into_iter()
            .filter_map(|id| {
                self.views
                    .quiz
                    .progress
                    .best
                    .get(id.id())
                    .map(|b| (id, b.score, b.time_s))
            })
            .collect();
        results_table(ui, "echo_test_records", &rows);
    }
}

/// Width of the number columns of the results table, logical px.
const NUMBER_COL_PX: f32 = 52.0;

/// The self-test's results as a table: the view's name (wrapping in the column left by the
/// numbers), the best score and the time, right-aligned in fixed columns; "—" for a view not found.
/// Rows are striped.
fn results_table(ui: &mut egui::Ui, id_salt: &str, rows: &[(ViewId, f32, Option<f32>)]) {
    let gap = ui.spacing().item_spacing.x;
    let width = ui.available_width();
    let name_w = (width - 2.0 * (NUMBER_COL_PX + gap)).max(60.0);
    let row_h = ui.spacing().interact_size.y;
    let stripe = ui.visuals().faint_bg_color;
    let cell = |ui: &mut egui::Ui, w: f32, h: f32, right: bool, add: &mut dyn FnMut(&mut egui::Ui)| {
        let layout = if right {
            egui::Layout::right_to_left(egui::Align::Center)
        } else {
            egui::Layout::left_to_right(egui::Align::Center)
        };
        ui.allocate_ui_with_layout(EVec2::new(w, h), layout, |ui| {
            ui.set_min_size(EVec2::new(w, h));
            add(ui);
        });
    };
    ui.push_id(id_salt, |ui| {
        ui.horizontal(|ui| {
            cell(ui, name_w, row_h, false, &mut |ui| {
                ui.strong(t!("echo-test-col-view"));
            });
            cell(ui, NUMBER_COL_PX, row_h, true, &mut |ui| {
                ui.strong(t!("echo-test-col-score"));
            });
            cell(ui, NUMBER_COL_PX, row_h, true, &mut |ui| {
                ui.strong(t!("echo-test-col-time"));
            });
        });
        for (i, (id, score, time)) in rows.iter().enumerate() {
            let name = view_name(*id);
            let galley = ui.painter().layout(
                name.clone(),
                egui::TextStyle::Body.resolve(ui.style()),
                Color32::WHITE,
                name_w,
            );
            let h = galley.size().y.max(row_h);
            let top = ui.cursor().min;
            if i % 2 == 0 {
                ui.painter()
                    .rect_filled(Rect::from_min_size(top, EVec2::new(width, h + 2.0)), 3.0, stripe);
            }
            ui.horizontal(|ui| {
                cell(ui, name_w, h, false, &mut |ui| {
                    ui.add(egui::Label::new(name.as_str()).wrap());
                });
                cell(ui, NUMBER_COL_PX, h, true, &mut |ui| {
                    ui.label(format!("{:.0} %", score * 100.0));
                });
                cell(ui, NUMBER_COL_PX, h, true, &mut |ui| {
                    if let Some(s) = time {
                        ui.label(format!("{s:.0} s"));
                    } else {
                        let not_found = t!("echo-test-not-found");
                        let dash = ui.label("—").on_hover_text(not_found.clone());
                        dash.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &not_found));
                    }
                });
            });
        }
    });
}
