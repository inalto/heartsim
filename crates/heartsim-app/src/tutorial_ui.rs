//! The tutorial list and the page reader (brief B15). Independent of the GPU and of
//! [`crate::HeartSimApp`]: the application implements [`TutorialHost`] and calls [`Tutorials::poll`]
//! once per frame, so the page actions run against the live model; tests use a fake host.

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use heartsim_ui::{Palette, cards, t};
use std::path::PathBuf;

use crate::sound_check::{SoundCheck, Tone};
use crate::tutorial::{
    Mode, PageRun, ProgressStore, Step, Tutorial, TutorialHost, TutorialLibrary, TutorialProgress, UiAction,
};

/// The open tutorial and the reader's state for its current page.
#[derive(Debug)]
struct Session {
    tutorial: usize,
    page: usize,
    run: PageRun,
    /// Choice picked for the current page's check.
    chosen: Option<usize>,
}

/// Tutorial library, saved progress and the open tutorial.
pub struct Tutorials {
    pub library: TutorialLibrary,
    pub progress: ProgressStore,
    /// Where progress is saved; `None` keeps it in memory only (tests).
    progress_path: Option<PathBuf>,
    pub mode: Mode,
    session: Option<Session>,
    /// Control outlined for the learner (set by a page's `Highlight`, cleared on page change).
    pub highlight: Option<String>,
    markdown: CommonMarkCache,
    /// Whether heart sounds can be played here (set by the app every frame): the sound check offers
    /// its tones only then, and is text otherwise (B28).
    pub sound_available: bool,
    /// The learner's answer to the sound check.
    pub sound_check: SoundCheck,
    /// A tone the learner asked to hear this frame.
    tone_request: Option<Tone>,
}

impl std::fmt::Debug for Tutorials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tutorials")
            .field("tutorials", &self.library.tutorials.len())
            .field("mode", &self.mode)
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

/// Widest a tutorial card grows, points.
const TUTORIAL_CARD_MAX_WIDTH: f32 = 720.0;

impl Tutorials {
    #[must_use]
    pub fn new(library: TutorialLibrary, progress_path: Option<PathBuf>) -> Self {
        let progress = progress_path
            .as_deref()
            .map(ProgressStore::load)
            .unwrap_or_default();
        Self {
            library,
            progress,
            progress_path,
            mode: Mode::default(),
            session: None,
            highlight: None,
            markdown: CommonMarkCache::default(),
            sound_available: false,
            sound_check: SoundCheck::default(),
            tone_request: None,
        }
    }

    /// The tone of the sound check the learner asked for this frame, if any (the app plays it).
    pub fn take_tone_request(&mut self) -> Option<Tone> {
        self.tone_request.take()
    }

    /// The sound check (B28): headphone advice is in the page text; here the two tones, and what to
    /// do when the low one cannot be heard. Text only where there is no sound output.
    fn sound_check_ui(
        ui: &mut egui::Ui,
        available: bool,
        check: &mut SoundCheck,
        request: &mut Option<Tone>,
    ) {
        ui.separator();
        ui.strong(t!("tut-sound-check"));
        if !available {
            ui.add(egui::Label::new(t!("tut-sound-check-none")).wrap());
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button(t!("tut-sound-check-low")).clicked() {
                *request = Some(Tone::Low);
            }
            if ui.button(t!("tut-sound-check-high")).clicked() {
                *request = Some(Tone::High);
            }
        });
        ui.horizontal_wrapped(|ui| {
            let heard = &mut check.heard_low;
            if ui
                .selectable_label(*heard == Some(true), t!("tut-sound-check-heard"))
                .clicked()
            {
                *heard = Some(true);
            }
            if ui
                .selectable_label(*heard == Some(false), t!("tut-sound-check-not-heard"))
                .clicked()
            {
                *heard = Some(false);
            }
        });
        match check.heard_low {
            Some(true) => {
                cards::result_box(ui, true, t!("tut-sound-check-ok"));
            }
            Some(false) => {
                cards::result_box(ui, false, t!("tut-sound-check-small"));
                ui.add(egui::Label::new(t!("tut-sound-check-small-hint")).wrap());
            }
            None => {}
        }
    }

    /// The installed tutorials with progress in the settings directory (no tutorials and
    /// in-memory progress without the `native` feature).
    #[must_use]
    pub fn load_default() -> Self {
        #[cfg(feature = "native")]
        {
            Self::new(TutorialLibrary::load_default(), ProgressStore::default_path())
        }
        #[cfg(not(feature = "native"))]
        {
            Self::new(TutorialLibrary::default(), None)
        }
    }

    /// The tutorials of a downloaded bundle. Progress is not written to a file: the host stores
    /// [`ProgressStore::to_text`] (the web build: eframe's `localStorage`) and passes it back as
    /// `saved_progress` on the next visit.
    #[must_use]
    pub fn from_bundle(bundle: &crate::assets::AssetBundle, saved_progress: Option<&str>) -> Self {
        let mut t = Self::new(TutorialLibrary::from_bundle(bundle), None);
        if let Some(text) = saved_progress {
            t.progress = ProgressStore::from_text(text);
        }
        t
    }

    /// Index of the open tutorial and its current page.
    #[must_use]
    pub fn position(&self) -> Option<(usize, usize)> {
        self.session.as_ref().map(|s| (s.tutorial, s.page))
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.session.is_some()
    }

    fn progress_of(&self, tutorial: usize) -> TutorialProgress {
        self.progress.get(&self.library.tutorials[tutorial].id)
    }

    /// Open tutorial `index` at `page` (clamped) and start that page's actions.
    pub fn open(&mut self, index: usize, page: usize) {
        let Some(t) = self.library.tutorials.get(index) else {
            return;
        };
        let page = page.min(t.pages.len().saturating_sub(1));
        self.session = Some(Session {
            tutorial: index,
            page,
            run: PageRun::new(&t.pages[page]),
            chosen: None,
        });
        self.highlight = None;
        self.update_progress(|p| {
            p.page = page;
            p.furthest = p.furthest.max(page);
        });
    }

    /// Close the reader (progress is kept).
    pub fn close(&mut self) {
        self.session = None;
        self.highlight = None;
    }

    /// Go to `page` of the open tutorial if the mode allows it.
    pub fn go_to(&mut self, page: usize) {
        if let Some((t, current)) = self.position()
            && page != current
            && page < self.library.tutorials[t].pages.len()
            && self.may_visit(t, page)
        {
            self.open(t, page);
        }
    }

    /// Tutor mode visits any page; self-study only pages up to the first unpassed check.
    #[must_use]
    pub fn may_visit(&self, tutorial: usize, page: usize) -> bool {
        self.mode == Mode::Tutor || (0..page).all(|p| self.page_cleared(tutorial, p))
    }

    /// A page without a check, or whose check was answered correctly.
    fn page_cleared(&self, tutorial: usize, page: usize) -> bool {
        self.library.tutorials[tutorial].pages[page].check.is_none()
            || self.progress_of(tutorial).passed.contains(&page)
    }

    /// Answer the current page's check with choice `choice`.
    pub fn answer(&mut self, choice: usize) {
        let Some(s) = &mut self.session else { return };
        let page = s.page;
        s.chosen = Some(choice);
        let correct = self.library.tutorials[s.tutorial].pages[page]
            .check
            .as_ref()
            .is_some_and(|q| q.answer == choice);
        if correct {
            self.update_progress(|p| {
                p.passed.insert(page);
            });
        }
    }

    /// Run the open page's due actions against `host`. Call once per frame.
    pub fn poll(&mut self, host: &mut impl TutorialHost) {
        if let Some(s) = &mut self.session {
            s.run.poll(host);
        }
    }

    /// Take the open page's runner out, so the host that owns `self` can be borrowed by it; hand
    /// it back with [`Self::restore_run`].
    pub fn take_run(&mut self) -> Option<PageRun> {
        self.session.as_mut().map(|s| std::mem::take(&mut s.run))
    }

    pub fn restore_run(&mut self, run: PageRun) {
        if let Some(s) = &mut self.session {
            s.run = run;
        }
    }

    /// Re-run the current page's actions from the start.
    pub fn replay(&mut self) {
        if let Some((t, p)) = self.position() {
            self.open(t, p);
        }
    }

    fn update_progress(&mut self, f: impl FnOnce(&mut TutorialProgress)) {
        let Some(s) = &self.session else { return };
        let t = &self.library.tutorials[s.tutorial];
        let last = t.pages.len() - 1;
        let entry = self.progress.tutorials.entry(t.id.clone()).or_default();
        f(entry);
        entry.completed = entry.furthest >= last
            && t.pages
                .iter()
                .enumerate()
                .all(|(i, p)| p.check.is_none() || entry.passed.contains(&i));
        if let Some(path) = &self.progress_path
            && let Err(e) = self.progress.save(path)
        {
            tracing::warn!("{}: {e}", path.display());
        }
    }

    /// The tutorial list with progress, the mode switch and Start / Resume.
    pub fn library_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(t!("tut-mode"));
            ui.selectable_value(&mut self.mode, Mode::SelfStudy, t!("tut-self-study"))
                .on_hover_text(t!("tut-self-study-hint"));
            ui.selectable_value(&mut self.mode, Mode::Tutor, t!("tut-tutor"))
                .on_hover_text(t!("tut-tutor-hint"));
        });
        ui.separator();
        if self.library.tutorials.is_empty() {
            ui.label(t!("tut-none"));
        }
        let mut start = None;
        // equal-width cards, as wide as the column but no wider than a line of text reads well
        let width = ui.available_width().min(TUTORIAL_CARD_MAX_WIDTH);
        for (i, t) in self.library.tutorials.iter().enumerate() {
            let p = self.progress.get(&t.id);
            let total = t.pages.len().max(1);
            let reached = if p == TutorialProgress::default() {
                0
            } else {
                (p.furthest + 1).min(total)
            };
            ui.scope(|ui| {
                ui.set_max_width(width);
                cards::card(ui, |ui| {
                    ui.set_width(width - 22.0);
                    cards::heading(ui, &t.title);
                    ui.add(egui::Label::new(&t.summary).wrap());
                    ui.add_space(4.0);
                    // progress: "3 of 6 pages" with a thin bar, or "Completed" with a tick
                    ui.horizontal(|ui| {
                        if p.completed {
                            let good = Palette::of(ui).good();
                            let (rect, _) =
                                ui.allocate_exact_size(egui::Vec2::splat(18.0), egui::Sense::hover());
                            cards::status_icon(ui.painter(), rect.center(), 8.0, true, good);
                            ui.label(
                                egui::RichText::new(t!("tut-completed"))
                                    .color(good.on_soft)
                                    .strong(),
                            );
                        } else if reached > 0 {
                            ui.label(t!(
                                "tut-progress",
                                done = reached.to_string(),
                                total = total.to_string()
                            ));
                        } else {
                            ui.label(t!("tut-pages", count = total.to_string()));
                        }
                    });
                    if p.completed || reached > 0 {
                        ui.add(
                            egui::ProgressBar::new(if p.completed {
                                1.0
                            } else {
                                reached as f32 / total as f32
                            })
                            .desired_width(ui.available_width())
                            .desired_height(6.0)
                            .corner_radius(3),
                        );
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        // the visible word is short; the title is the accessible name and the hover
                        let named = |response: egui::Response, name: String| {
                            let label = name.clone();
                            response.widget_info(move || {
                                egui::WidgetInfo::labeled(egui::Role::Button, true, &label)
                            });
                            response.on_hover_text(name)
                        };
                        if p.completed {
                            let r = ui.button(t!("tut-start-again"));
                            if named(r, t!("tut-start-again-hint", title = t.title.as_str())).clicked() {
                                start = Some((i, 0));
                            }
                        } else if p.page > 0 {
                            let r = ui.button(t!("tut-continue"));
                            let name = t!(
                                "tut-resume",
                                title = t.title.as_str(),
                                page = (p.page + 1).to_string()
                            );
                            if named(r, name).clicked() {
                                start = Some((i, p.page));
                            }
                            let r = ui.button(t!("tut-start-again"));
                            if named(r, t!("tut-start-again-hint", title = t.title.as_str())).clicked() {
                                start = Some((i, 0));
                            }
                        } else {
                            let r = ui.button(t!("tut-start-short"));
                            if named(r, t!("tut-start", title = t.title.as_str())).clicked() {
                                start = Some((i, 0));
                            }
                        }
                    });
                });
            });
        }
        if let Some((i, page)) = start {
            self.open(i, page);
        }
    }

    /// The reader for the open tutorial (nothing if none is open): page index, Markdown text,
    /// knowledge check and Previous / Next.
    pub fn reader_ui(&mut self, ui: &mut egui::Ui) {
        crate::tutorial_ui::publish_highlight(ui.ctx(), self.highlight.as_deref());
        let Some(s) = &self.session else { return };
        let (ti, page) = (s.tutorial, s.page);
        let chosen = s.chosen;
        let running = !s.run.is_finished();
        let tutorial: &Tutorial = &self.library.tutorials[ti];
        let n = tutorial.pages.len();
        let p = &tutorial.pages[page];
        let mut nav: Option<Nav> = None;

        ui.horizontal(|ui| {
            ui.heading(&tutorial.title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t!("tut-close")).clicked() {
                    nav = Some(Nav::Close);
                }
            });
        });
        ui.add(egui::ProgressBar::new((page + 1) as f32 / n as f32).text(t!(
            "tut-page-of",
            page = (page + 1).to_string(),
            total = n.to_string()
        )));
        egui::CollapsingHeader::new(t!("tut-contents"))
            .id_salt("tutorial_contents")
            .show(ui, |ui| {
                for (i, pg) in tutorial.pages.iter().enumerate() {
                    let enabled = self.may_visit(ti, i);
                    let done = pg.check.is_some() && self.progress_of(ti).passed.contains(&i);
                    let label = format!(
                        "{}. {}{}",
                        i + 1,
                        pg.title,
                        if done {
                            format!(" {}", t!("tut-passed"))
                        } else {
                            String::new()
                        }
                    );
                    if ui
                        .add_enabled(enabled, egui::Button::selectable(i == page, label))
                        .clicked()
                    {
                        nav = Some(Nav::Go(i));
                    }
                }
            });
        ui.separator();
        ui.strong(&p.title);
        egui::ScrollArea::vertical()
            .id_salt("tutorial_page")
            .max_height((ui.available_height() - 140.0).max(120.0))
            .show(ui, |ui| {
                CommonMarkViewer::new().show(ui, &mut self.markdown, &p.markdown);
                if p.actions.contains(&Step::Ui(UiAction::SoundCheck)) {
                    Self::sound_check_ui(
                        ui,
                        self.sound_available,
                        &mut self.sound_check,
                        &mut self.tone_request,
                    );
                }
                if let Some(q) = &p.check {
                    ui.separator();
                    ui.strong(t!("tut-check"));
                    ui.label(&q.prompt);
                    for (i, c) in q.choices.iter().enumerate() {
                        if ui.selectable_label(chosen == Some(i), c).clicked() {
                            nav = Some(Nav::Answer(i));
                        }
                    }
                    match chosen {
                        Some(c) if c == q.answer => {
                            cards::result_box(ui, true, t!("feedback-correct"));
                            if !q.explanation.is_empty() {
                                CommonMarkViewer::new().show(ui, &mut self.markdown, &q.explanation);
                            }
                        }
                        Some(_) => {
                            cards::result_box(ui, false, t!("tut-try-again"));
                        }
                        None => {}
                    }
                }
            });
        ui.separator();
        let can_next = self.mode == Mode::Tutor || self.page_cleared(ti, page);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(page > 0, egui::Button::new(t!("tut-previous")))
                .clicked()
            {
                nav = Some(Nav::Go(page - 1));
            }
            let next_label = if page + 1 < n {
                t!("tut-next")
            } else {
                t!("tut-finish")
            };
            if ui
                .add_enabled(can_next, egui::Button::new(next_label))
                .on_disabled_hover_text(t!("tut-answer-first"))
                .clicked()
            {
                nav = Some(if page + 1 < n {
                    Nav::Go(page + 1)
                } else {
                    Nav::Close
                });
            }
            if ui
                .button(t!("tut-replay"))
                .on_hover_text(t!("tut-replay-hint"))
                .clicked()
            {
                nav = Some(Nav::Replay);
            }
            if running {
                ui.spinner();
                ui.weak(t!("tut-running"));
            }
        });
        match nav {
            Some(Nav::Go(i)) => self.go_to(i),
            Some(Nav::Answer(i)) => self.answer(i),
            Some(Nav::Replay) => self.replay(),
            Some(Nav::Close) => self.close(),
            None => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Nav {
    Go(usize),
    Answer(usize),
    Replay,
    Close,
}

/// Hand the highlighted tool id to the controls (see `heartsim_ui::highlight`).
pub fn publish_highlight(ctx: &egui::Context, tool: Option<&str>) {
    if heartsim_ui::highlight::current(ctx).as_deref() != tool {
        heartsim_ui::highlight::set(ctx, tool);
    }
}
