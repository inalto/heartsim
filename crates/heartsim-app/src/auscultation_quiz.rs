//! The auscultation quiz (brief B28, Practise group; built like [`crate::rhythm_quiz`]): a looping
//! clip of a hidden auscultation case, first "what do you hear?" (the features), then "what is it?"
//! (four diagnoses), then feedback with the trace, its labels and the ECG above it.
//!
//! Features before diagnosis is the self-testing the research supports (Watsjold et al. 2021: a
//! feature checklist, then the diagnosis, then immediate feedback). The trace and the scenario's
//! name stay hidden until both steps are answered, so the question is heard, not read; without
//! a sound output the trace is shown from the start and the page says so (the name stays hidden).
//! Slow motion is offered only in the feedback step: halving the speed changes what is tested.
//!
//! Questions, distractors and clips are deterministic: a seeded generator picks them from the
//! auscultation cases only ([`ScenarioLibrary::auscultation_pool`]), and a clip is an offline
//! recording. The Rhythm Quiz pool is untouched.

use crate::narrow::{Narrow, Pane, PaneSpec, Side};
use std::rc::Rc;

use eframe::egui;
use heartsim_core::{ExpectedFindings, ExtraSound, MurmurTiming, SplitMode};
use heartsim_ecg::{ChestPiece, ClipOptions, ListeningChoice, ListeningSite};
use heartsim_ui::cards::AnswerState;
use heartsim_ui::{ClipTrace, StripStyle, cards, clip_view, t};

use crate::heart_sound_player::HeartSoundPlayer;
use crate::rhythm_quiz::{OPTIONS, explanation};
use crate::scenarios::ScenarioLibrary;
use crate::sound_clips::{ClipCache, ClipEntry, sound_available};

/// eframe storage key of the score.
pub const SCORE_KEY: &str = "auscultation_quiz_score";

/// One thing the learner may tick in step 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FeatureItem {
    /// An extra sound just before S1 (S4).
    BeforeS1,
    /// An extra sound after S2 (S3, opening snap).
    AfterS2,
    /// S2 split wide enough to hear in both phases of the breath (wide, fixed, paradoxical).
    SplitS2,
    /// A click in systole (ejection or mid-systolic).
    Click,
    SystolicMurmur,
    DiastolicMurmur,
    /// When the murmur lies in its phase of the cycle.
    Early,
    Mid,
    Late,
    Throughout,
}

impl FeatureItem {
    /// Every item, in the order of the checklist.
    pub const ALL: [FeatureItem; 10] = [
        FeatureItem::BeforeS1,
        FeatureItem::AfterS2,
        FeatureItem::SplitS2,
        FeatureItem::Click,
        FeatureItem::SystolicMurmur,
        FeatureItem::DiastolicMurmur,
        FeatureItem::Early,
        FeatureItem::Mid,
        FeatureItem::Late,
        FeatureItem::Throughout,
    ];

    /// The checklist's wording.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            FeatureItem::BeforeS1 => t!("aq-f-before-s1"),
            FeatureItem::AfterS2 => t!("aq-f-after-s2"),
            FeatureItem::SplitS2 => t!("aq-f-split"),
            FeatureItem::Click => t!("aq-f-click"),
            FeatureItem::SystolicMurmur => t!("aq-f-systolic"),
            FeatureItem::DiastolicMurmur => t!("aq-f-diastolic"),
            FeatureItem::Early => t!("aq-f-early"),
            FeatureItem::Mid => t!("aq-f-mid"),
            FeatureItem::Late => t!("aq-f-late"),
            FeatureItem::Throughout => t!("aq-f-throughout"),
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|i| *i == self).unwrap_or(0)
    }
}

/// The ticks of step 1, one per [`FeatureItem`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Features(pub [bool; 10]);

impl Features {
    #[must_use]
    pub fn get(&self, item: FeatureItem) -> bool {
        self.0[item.index()]
    }

    pub fn set(&mut self, item: FeatureItem, on: bool) {
        self.0[item.index()] = on;
    }
}

/// What the case's `ExpectedFindings` say about each item, and which items are scored. The split
/// is not scored for a physiological split (heard in inspiration only, so either answer is fair)
/// nor when the file does not say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Expected {
    pub features: Features,
    pub scored: [bool; 10],
}

/// The expected ticks of a case, from the auscultation fields of its `expected` block (brief B27):
/// S4 is "before S1"; S3 and the opening snap are "after S2"; a wide, fixed or paradoxical split is
/// "split S2"; either click is "click"; each murmur gives its phase and its timing (early-systolic
/// and early-diastolic → early, mid-systolic and mid-diastolic → mid, late-systolic and presystolic →
/// late, holosystolic → throughout).
#[must_use]
pub fn expected_features(e: &ExpectedFindings) -> Expected {
    let mut f = Features::default();
    let extra = e.extra_sounds.clone().unwrap_or_default();
    f.set(FeatureItem::BeforeS1, extra.contains(&ExtraSound::S4));
    f.set(
        FeatureItem::AfterS2,
        extra.contains(&ExtraSound::S3) || extra.contains(&ExtraSound::OpeningSnap),
    );
    f.set(
        FeatureItem::Click,
        extra.contains(&ExtraSound::EjectionClick) || extra.contains(&ExtraSound::MidSystolicClick),
    );
    for m in e.murmurs.clone().unwrap_or_default() {
        let (phase, timing) = match m {
            MurmurTiming::EarlySystolic => (FeatureItem::SystolicMurmur, FeatureItem::Early),
            MurmurTiming::MidSystolic => (FeatureItem::SystolicMurmur, FeatureItem::Mid),
            MurmurTiming::LateSystolic => (FeatureItem::SystolicMurmur, FeatureItem::Late),
            MurmurTiming::Holosystolic => (FeatureItem::SystolicMurmur, FeatureItem::Throughout),
            MurmurTiming::EarlyDiastolic => (FeatureItem::DiastolicMurmur, FeatureItem::Early),
            MurmurTiming::MidDiastolic => (FeatureItem::DiastolicMurmur, FeatureItem::Mid),
            MurmurTiming::Presystolic => (FeatureItem::DiastolicMurmur, FeatureItem::Late),
        };
        f.set(phase, true);
        f.set(timing, true);
    }
    let split_scored = matches!(
        e.s2_split,
        Some(SplitMode::Wide | SplitMode::Fixed | SplitMode::Paradoxical | SplitMode::Single)
    );
    f.set(
        FeatureItem::SplitS2,
        matches!(
            e.s2_split,
            Some(SplitMode::Wide | SplitMode::Fixed | SplitMode::Paradoxical)
        ),
    );
    let mut scored = [true; 10];
    scored[FeatureItem::SplitS2.index()] = split_scored;
    Expected { features: f, scored }
}

/// How the ticks compare with the case: per scored item, whether the tick (or its absence) was
/// right; `(right, scored)` overall.
#[must_use]
pub fn score_features(expected: &Expected, given: &Features) -> (Vec<(FeatureItem, bool)>, u32, u32) {
    let mut marks = Vec::new();
    let (mut right, mut total) = (0, 0);
    for item in FeatureItem::ALL {
        if !expected.scored[item.index()] {
            continue;
        }
        let ok = expected.features.get(item) == given.get(item);
        marks.push((item, ok));
        total += 1;
        right += u32::from(ok);
    }
    (marks, right, total)
}

/// One question: the library index of the answer and of the four choices, in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub answer: usize,
    pub options: Vec<usize>,
}

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

/// Draw a question from the auscultation cases, avoiding `previous` as the answer when there is a
/// choice: the distractors come first from the case's `confused_with` (those that are auscultation
/// cases too), then at random from the other cases. `None` with fewer than two cases.
#[must_use]
pub fn make_question(library: &ScenarioLibrary, rng: &mut u64, previous: Option<usize>) -> Option<Question> {
    let pool = library.auscultation_pool();
    let n = pool.len();
    if n < 2 {
        return None;
    }
    let mut k = below(rng, n);
    if Some(pool[k]) == previous {
        k = (k + 1 + below(rng, n - 1)) % n;
    }
    let answer = pool[k];
    let mut options = vec![answer];
    let mut differential: Vec<usize> = library.entries[answer]
        .scenario
        .confused_with
        .iter()
        .filter_map(|id| library.index_of(id))
        .filter(|i| *i != answer && pool.contains(i))
        .collect();
    shuffle(&mut differential, rng);
    for i in differential {
        if options.len() < OPTIONS && !options.contains(&i) {
            options.push(i);
        }
    }
    while options.len() < OPTIONS.min(n) {
        let i = pool[below(rng, n)];
        if !options.contains(&i) {
            options.push(i);
        }
    }
    shuffle(&mut options, rng);
    Some(Question { answer, options })
}

/// Where the learner is in a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Tick what is heard.
    Features,
    /// Choose the diagnosis.
    Diagnosis,
    /// Both answered: the trace, the name and the teaching text are shown.
    Feedback,
}

/// What the page asks the app for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenInSimulator {
    /// Library index of the case.
    pub scenario: usize,
    /// Where the learner was listening.
    pub listening: ListeningChoice,
}

/// Quiz state. See the module documentation.
#[derive(Debug)]
pub struct AuscultationQuiz {
    rng: u64,
    pub question: Option<Question>,
    pub step: Step,
    pub given: Features,
    /// The diagnosis chosen (library index).
    pub chosen: Option<usize>,
    pub listening: ListeningChoice,
    /// Half speed in the feedback step (never while a question is open).
    pub half_speed: bool,
    /// Small-speaker mode (the Settings value, set by the app).
    pub small_speaker: bool,
    pub playing: bool,
    /// Feature items right and scored, over every question answered.
    pub features_right: u32,
    pub features_scored: u32,
    /// Diagnoses right and asked.
    pub correct: u32,
    pub asked: u32,
    cache: ClipCache,
    pub error: Option<String>,
}

impl Default for AuscultationQuiz {
    fn default() -> Self {
        Self::new(0x51AB_D00D_2B28_0001)
    }
}

impl AuscultationQuiz {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            rng: seed,
            question: None,
            step: Step::Features,
            given: Features::default(),
            chosen: None,
            listening: ListeningChoice::default(),
            half_speed: false,
            small_speaker: false,
            playing: false,
            features_right: 0,
            features_scored: 0,
            correct: 0,
            asked: 0,
            cache: ClipCache::default(),
            error: None,
        }
    }

    /// Ask a new question: playback stops, the ticks clear, the stethoscope returns to the overview.
    pub fn next_question(&mut self, library: &ScenarioLibrary, player: &mut HeartSoundPlayer) {
        self.stop(player);
        let previous = self.question.as_ref().map(|q| q.answer);
        self.question = make_question(library, &mut self.rng, previous);
        self.step = Step::Features;
        self.given = Features::default();
        self.chosen = None;
        self.listening = ListeningChoice::default();
        self.half_speed = false;
        self.error = None;
    }

    /// Whether the trace may be shown: always without sound, otherwise only in the feedback step.
    #[must_use]
    pub fn trace_visible(&self, sound: bool) -> bool {
        !sound || self.step == Step::Feedback
    }

    /// Whether the case's name may be shown (feedback only, with or without sound).
    #[must_use]
    pub fn name_visible(&self) -> bool {
        self.step == Step::Feedback
    }

    /// The case's expected features.
    #[must_use]
    pub fn expected(&self, library: &ScenarioLibrary) -> Option<Expected> {
        let q = self.question.as_ref()?;
        let e = library.entries[q.answer].scenario.expected.as_ref()?;
        Some(expected_features(e))
    }

    /// Step 1 done: the ticks are scored and kept.
    pub fn submit_features(&mut self, library: &ScenarioLibrary) {
        if self.step != Step::Features || self.question.is_none() {
            return;
        }
        if let Some(e) = self.expected(library) {
            let (_, right, total) = score_features(&e, &self.given);
            self.features_right += right;
            self.features_scored += total;
        }
        self.step = Step::Diagnosis;
    }

    /// Step 2: the diagnosis (once per question); then the feedback.
    pub fn answer(&mut self, choice: usize) {
        let Some(q) = &self.question else { return };
        if self.step != Step::Diagnosis {
            return;
        }
        self.chosen = Some(choice);
        self.asked += 1;
        if choice == q.answer {
            self.correct += 1;
        }
        self.step = Step::Feedback;
    }

    /// The clip of the question at the current site.
    ///
    /// # Errors
    /// No question, or the case cannot be recorded.
    pub fn entry(&mut self, library: &ScenarioLibrary) -> Result<Rc<ClipEntry>, String> {
        let q = self.question.as_ref().ok_or_else(|| t!("aq-intro"))?;
        let id = library.entries[q.answer].scenario.id.clone();
        let options = ClipOptions {
            listening: self.listening,
            breath_hold: None,
            small_speaker: self.small_speaker,
        };
        self.cache.get(library, &id, options)
    }

    /// Loop the clip (from the same point of the cycle when it was already playing).
    pub fn play(
        &mut self,
        library: &ScenarioLibrary,
        player: &mut HeartSoundPlayer,
        phase: Option<(usize, f64)>,
    ) {
        let half = self.half_speed && self.step == Step::Feedback;
        match self.entry(library) {
            Ok(e) => {
                let start = phase.map_or(0, |(c, f)| e.playback_index(e.clip.index_at_phase(c, f), half));
                player.play_clip(e.samples(half), start, true);
                self.playing = true;
                self.error = None;
            }
            Err(err) => self.error = Some(err),
        }
    }

    pub fn stop(&mut self, player: &mut HeartSoundPlayer) {
        if self.playing {
            player.stop_clip();
        }
        self.playing = false;
    }

    /// Once per frame after the player ran.
    pub fn update(&mut self, player: &HeartSoundPlayer) {
        if self.playing && !player.clip_active() {
            self.playing = false;
        }
    }

    /// Score as stored: `features right/scored;diagnoses right/asked`.
    #[must_use]
    pub fn score_string(&self) -> String {
        format!(
            "{}/{};{}/{}",
            self.features_right, self.features_scored, self.correct, self.asked
        )
    }

    /// Restore a score saved with [`Self::score_string`]; malformed text is ignored.
    pub fn restore_score(&mut self, s: &str) {
        let pair = |p: &str| -> Option<(u32, u32)> {
            let (a, b) = p.split_once('/')?;
            let (a, b) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
            (a <= b).then_some((a, b))
        };
        if let Some((f, d)) = s.split_once(';')
            && let (Some((fr, fs)), Some((c, a))) = (pair(f), pair(d))
        {
            self.features_right = fr;
            self.features_scored = fs;
            self.correct = c;
            self.asked = a;
        }
    }

    /// The page. Returns the case to open in the simulator when asked.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        library: &ScenarioLibrary,
        player: &mut HeartSoundPlayer,
        style: &StripStyle,
    ) -> Option<OpenInSimulator> {
        let mut open = None;
        let sound = sound_available(player);
        let (question_name, listen_name) = (t!("pane-question"), t!("pane-listen"));
        let spec = PaneSpec {
            id: "auscultation_quiz",
            left: Some(Side {
                id: "auscultation_quiz_steps",
                default_px: 380.0,
                cap_px: 100_000.0,
                name: &question_name,
            }),
            right: None,
            centre: &listen_name,
            // on a phone the question and the listening page are two segments
            narrow: Narrow::Segmented { start: Pane::Left },
        };
        crate::narrow::panes(ui, &spec, |ui, pane| match pane {
            Pane::Left => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    open = self.steps_ui(ui, library, player);
                });
            }
            Pane::Centre => self.listen_ui(ui, library, player, style, sound),
            Pane::Right => {}
        });
        open
    }

    fn score_line(&self) -> String {
        t!(
            "aq-score",
            features = self.features_right.to_string(),
            scored = self.features_scored.to_string(),
            correct = self.correct.to_string(),
            asked = self.asked.to_string()
        )
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the three steps of a question, top to bottom"
    )]
    fn steps_ui(
        &mut self,
        ui: &mut egui::Ui,
        library: &ScenarioLibrary,
        player: &mut HeartSoundPlayer,
    ) -> Option<OpenInSimulator> {
        let mut open = None;
        ui.horizontal_wrapped(|ui| {
            let label = if self.question.is_some() {
                t!("rq-next")
            } else {
                t!("app-quiz-new")
            };
            // the one primary action of this panel: the next question when none is open or the
            // last is answered; the features and diagnosis steps have their own main action
            let next_is_main = self.question.is_none() || self.step == Step::Feedback;
            let next = ui
                .add_enabled_ui(library.auscultation_pool().len() >= 2, |ui| {
                    if next_is_main {
                        heartsim_ui::primary_button(ui, label)
                    } else {
                        ui.button(label)
                    }
                })
                .inner;
            if next.clicked() {
                self.next_question(library, player);
            }
            ui.label(self.score_line());
        });
        ui.separator();
        let Some(q) = self.question.clone() else {
            ui.add(egui::Label::new(t!("aq-intro")).wrap());
            return None;
        };
        match self.step {
            Step::Features => {
                ui.strong(t!("aq-step-features"));
                ui.add(egui::Label::new(t!("aq-step-features-hint")).wrap());
                for (n, item) in FeatureItem::ALL.into_iter().enumerate() {
                    if n == 6 {
                        ui.add_space(4.0);
                        ui.label(t!("aq-timing"));
                    }
                    let mut on = self.given.get(item);
                    if ui.checkbox(&mut on, item.label()).changed() {
                        self.given.set(item, on);
                    }
                }
                ui.add_space(6.0);
                if heartsim_ui::primary_button(ui, t!("aq-features-done")).clicked() {
                    self.submit_features(library);
                }
            }
            Step::Diagnosis => {
                ui.strong(t!("aq-step-diagnosis"));
                for &i in &q.options {
                    let title = &library.entries[i].scenario.title;
                    if cards::answer_option(ui, AnswerState::Open, title).clicked() {
                        self.answer(i);
                    }
                    ui.add_space(2.0);
                }
            }
            Step::Feedback => {
                let chosen = self.chosen.unwrap_or(q.answer);
                for &i in &q.options {
                    let title = &library.entries[i].scenario.title;
                    let (state, text) = if i == q.answer {
                        (
                            AnswerState::Correct,
                            t!("rq-correct-choice", title = title.as_str()),
                        )
                    } else if i == chosen {
                        (AnswerState::Wrong, t!("rq-your-choice", title = title.as_str()))
                    } else {
                        (AnswerState::Inactive, title.clone())
                    };
                    cards::answer_option(ui, state, &text);
                    ui.add_space(2.0);
                }
                ui.separator();
                if ui.button(t!("rq-open")).clicked() {
                    open = Some(OpenInSimulator {
                        scenario: q.answer,
                        listening: self.listening,
                    });
                }
                if let Some(e) = self.expected(library) {
                    let (marks, right, total) = score_features(&e, &self.given);
                    ui.strong(t!(
                        "aq-features-result",
                        right = right.to_string(),
                        total = total.to_string()
                    ));
                    for (item, ok) in marks {
                        let heard = if e.features.get(item) {
                            t!("aq-present")
                        } else {
                            t!("aq-absent")
                        };
                        let mark = if ok {
                            t!("aq-mark-right")
                        } else {
                            t!("aq-mark-wrong")
                        };
                        ui.label(format!("{mark} {}: {heard}", item.label()));
                    }
                    if !e.scored[FeatureItem::SplitS2.index()] {
                        heartsim_ui::helper_text(ui, t!("aq-split-not-scored"));
                    }
                }
                ui.separator();
                let x = explanation(library, q.answer, chosen);
                if chosen == q.answer {
                    cards::result_box(ui, true, t!("feedback-correct"));
                } else {
                    cards::result_box(ui, false, t!("rq-wrong", title = x.title.as_str()));
                }
                ui.add(egui::Label::new(&x.description).wrap());
                if let Some((title, description)) = &x.chosen {
                    ui.add_space(4.0);
                    ui.strong(t!("rq-yours-was", title = title.as_str()));
                    ui.add(egui::Label::new(description).wrap());
                }
                if let Some(teaching) = &x.teaching {
                    ui.add_space(4.0);
                    ui.strong(t!("rq-why"));
                    ui.add(egui::Label::new(teaching).wrap());
                }
                if !x.confused_with.is_empty() {
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(t!("rq-confused", list = x.confused_with.join(", "))).wrap());
                }
            }
        }
        if let Some(e) = &self.error {
            cards::result_box(ui, false, e.clone());
        }
        open
    }

    fn listen_ui(
        &mut self,
        ui: &mut egui::Ui,
        library: &ScenarioLibrary,
        player: &mut HeartSoundPlayer,
        style: &StripStyle,
        sound: bool,
    ) {
        if self.question.is_none() {
            ui.add(egui::Label::new(t!("aq-intro-listen")).wrap());
            return;
        }
        let old = (self.listening, self.half_speed);
        let phase = self
            .playing
            .then(|| self.current_phase(library, player))
            .flatten();
        ui.horizontal_wrapped(|ui| {
            let label = if self.playing {
                t!("st-stop")
            } else {
                t!("st-play")
            };
            // Play is the one primary action of the listening panel; Stop is a plain button
            let playing = self.playing;
            let play = ui
                .add_enabled_ui(sound, |ui| {
                    if playing {
                        ui.button(label)
                    } else {
                        heartsim_ui::primary_button(ui, label)
                    }
                })
                .inner
                .on_disabled_hover_text(t!("st-no-sound"));
            if play.clicked() {
                if self.playing {
                    self.stop(player);
                } else {
                    self.play(library, player, None);
                }
            }
            ui.label(t!("ecg-listening-site"));
            let combo = egui::ComboBox::from_id_salt("aq_site")
                .selected_text(heartsim_ui::auscultation_chest::site_name(self.listening.site))
                .show_ui(ui, |ui| {
                    for site in ListeningSite::ALL {
                        ui.selectable_value(
                            &mut self.listening.site,
                            site,
                            heartsim_ui::auscultation_chest::site_name(site),
                        );
                    }
                });
            let name = t!("ecg-listening-site");
            combo
                .response
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &name));
            ui.add_enabled_ui(!self.listening.is_overview(), |ui| {
                for chest in ChestPiece::ALL {
                    ui.radio_value(
                        &mut self.listening.chest,
                        chest,
                        heartsim_ui::pcg_lane::chest_name(chest),
                    );
                }
            });
        });
        ui.horizontal(|ui| {
            ui.add_enabled_ui(self.step == Step::Feedback, |ui| {
                ui.checkbox(&mut self.half_speed, t!("st-half-speed"))
                    .on_hover_text(t!("st-half-speed-hint"))
                    .on_disabled_hover_text(t!("aq-no-slow-motion"));
            });
        });
        if (self.listening, self.half_speed) != old && self.playing {
            self.play(library, player, phase);
        }
        heartsim_ui::helper_text(ui, t!("st-model-note"));
        if !sound && self.step != Step::Feedback {
            heartsim_ui::helper_text(ui, t!("aq-no-sound"));
        }
        ui.separator();
        let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
        if !self.trace_visible(sound) {
            ui.label(t!("aq-trace-hidden"));
            return;
        }
        let half = self.half_speed && self.step == Step::Feedback;
        match self.entry(library) {
            Ok(e) => {
                let playhead = self
                    .playing
                    .then(|| player.clip_playhead())
                    .flatten()
                    .map(|p| e.trace_index(p, half) as u64);
                let title = heartsim_ui::pcg_lane::lane_title(self.listening);
                let tag = match (half, self.small_speaker) {
                    (false, false) => None,
                    (true, false) => Some(t!("ecg-sound-altered-slow")),
                    (false, true) => Some(t!("ecg-sound-altered-small")),
                    (true, true) => Some(format!(
                        "{} · {}",
                        t!("ecg-sound-altered-slow"),
                        t!("ecg-sound-altered-small")
                    )),
                };
                clip_view(
                    ui,
                    rect,
                    &ClipTrace {
                        ecg: &e.ecg_ii,
                        lead_label: "II",
                        pcg: &e.pcg_ring,
                        marks: &e.clip.marks,
                        playhead,
                        title: &title,
                        tag: tag.as_deref(),
                    },
                    style,
                );
            }
            Err(err) => {
                cards::result_box(ui, false, err);
            }
        }
    }

    fn current_phase(
        &mut self,
        library: &ScenarioLibrary,
        player: &HeartSoundPlayer,
    ) -> Option<(usize, f64)> {
        let at = player.clip_playhead()?;
        let half = self.half_speed && self.step == Step::Feedback;
        let e = self.entry(library).ok()?;
        e.clip.cycle_phase(e.trace_index(at, half))
    }
}
