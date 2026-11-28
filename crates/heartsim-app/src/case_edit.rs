//! The model of the case editor (brief B42): a draft of a patient case that a teacher writes on the
//! Cases page, with the operations the form offers and the checks that decide whether it may be
//! saved as a `.hs` file. No egui here: the page is [`crate::case_edit_ui`], the platform part of
//! saving is [`crate::case_io`].
//!
//! **What blocks saving and what only warns.** The structural rules of the format
//! ([`Case::problems`]: no stage, an answer outside its choices), the limits of the case file
//! (`heartsim_casefile::MAX_*`: the id rule, field lengths), an action row that does not parse, a
//! rhythm the case loads that exists nowhere, and a rhythm that came from another case file or from
//! this session but is not bundled all stop the save: the file would not open on another computer.
//! The editorial rules of the shipped cases ([`Case::content_problems`]: five questions, three to
//! five choices, an explanation, a reference) are warnings, since a teacher's own case may be
//! shorter.

use crate::scenarios::ScenarioLibrary;
use heartsim_casefile::{
    CasePackage, MAX_AUTHORS, MAX_ENTRY_BYTES, MAX_FIELD_CHARS, MAX_ID_LEN, MAX_LANGUAGE_CHARS, Manifest,
    valid_id,
};
use heartsim_core::Lead;
use heartsim_core::case::{CASE_FORMAT_VERSION, Case, Stage};
use heartsim_core::tutorial::{
    CameraView, ChestPieceChoice, Panel, ProbeWindow, Question, Step, StethoscopeSite, UiAction, WaitEvent,
};
use heartsim_ui::t;

/// Most choices a question of the editor may have (the editorial rule of the shipped cases).
pub const MAX_CHOICES: usize = 5;
/// Fewest choices the editor lets a question keep.
pub const MIN_CHOICES: usize = 3;

/// The id a title gives: lower case, accents folded to ASCII, every other run of characters
/// replaced by one `_`, at most [`MAX_ID_LEN`] characters of `[a-z0-9_]`; `case` for a title with
/// no letter or digit. The result always satisfies [`valid_id`] and depends on the title only.
#[must_use]
pub fn id_from_title(title: &str) -> String {
    let mut out = String::new();
    let mut pending_sep = false;
    for c in title.chars().flat_map(fold) {
        if c.is_ascii_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push('_');
            }
            pending_sep = false;
            out.push(c.to_ascii_lowercase());
        } else {
            pending_sep = true;
        }
    }
    out.truncate(MAX_ID_LEN);
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("case");
    }
    out
}

/// A letter with a diacritic as plain ASCII (Latin-1 and the common Latin Extended-A letters of the
/// shipped languages); other characters unchanged.
fn fold(c: char) -> impl Iterator<Item = char> {
    let s: &str = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā'
        | 'Ă' | 'Ą' => "a",
        'ç' | 'ć' | 'č' | 'Ç' | 'Ć' | 'Č' => "c",
        'ď' | 'đ' | 'Ď' | 'Đ' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' | 'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ę' | 'Ě' => {
            "e"
        }
        'ğ' | 'Ğ' => "g",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ı' | 'Ì' | 'Í' | 'Î' | 'Ï' | 'Ī' | 'İ' => "i",
        'ł' | 'Ł' => "l",
        'ñ' | 'ń' | 'ň' | 'Ñ' | 'Ń' | 'Ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ő' => {
            "o"
        }
        'ř' | 'Ř' => "r",
        'ś' | 'š' | 'ş' | 'Ś' | 'Š' | 'Ş' => "s",
        'ß' => "ss",
        'ť' | 'ţ' | 'Ť' | 'Ţ' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' | 'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ū' | 'Ů' | 'Ű' => {
            "u"
        }
        'ý' | 'ÿ' | 'Ý' | 'Ÿ' => "y",
        'ź' | 'ż' | 'ž' | 'Ź' | 'Ż' | 'Ž' => "z",
        'æ' | 'Æ' => "ae",
        'œ' | 'Œ' => "oe",
        _ => "",
    };
    let (own, mapped) = if s.is_empty() { (Some(c), "") } else { (None, s) };
    own.into_iter().chain(mapped.chars())
}

/// Licences offered in the file details (SPDX ids); the field also takes free text. No
/// no-derivatives licence: such a case could not be shared as the format intends.
pub const LICENCES: [&str; 8] = [
    "CC-BY-4.0",
    "CC-BY-SA-4.0",
    "CC-BY-NC-4.0",
    "CC-BY-NC-SA-4.0",
    "CC0-1.0",
    "MIT",
    "Apache-2.0",
    "MIT OR Apache-2.0",
];

/// One row of a stage's actions: the text as the user wrote it, and what it parsed to.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub text: String,
    pub parsed: Result<Step, String>,
}

impl Row {
    /// A row with this text, parsed.
    #[must_use]
    pub fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let parsed = parse_step(&text);
        Self { text, parsed }
    }

    /// A row that writes `step`.
    #[must_use]
    pub fn from_step(step: &Step) -> Self {
        Self {
            text: step_text(step),
            parsed: Ok(step.clone()),
        }
    }

    /// Replace the text and parse it again.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.reparse();
    }

    /// Parse the text again (after the text field changed it).
    pub fn reparse(&mut self) {
        self.parsed = parse_step(&self.text);
    }
}

/// The text of an action row: RON, one line, as the case files write it
/// (`Ui(LoadScenario("stemi_inferior"))`, `Sim(Block(region: AvNodeCompact, blocked: true))`).
#[must_use]
pub fn step_text(step: &Step) -> String {
    spaced(&ron::to_string(step).unwrap_or_default())
}

/// A row's text read as an action.
///
/// # Errors
/// The parser's message when the text is not an action.
pub fn parse_step(text: &str) -> Result<Step, String> {
    ron::from_str::<Step>(text).map_err(|e| e.to_string())
}

/// `ron`'s compact text with a space after each `,` and `:` that is outside a string, so a row
/// reads like the case files.
fn spaced(compact: &str) -> String {
    let mut out = String::with_capacity(compact.len() + 8);
    let (mut in_str, mut escaped) = (false, false);
    let mut chars = compact.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
        } else if (c == ',' || c == ':') && chars.peek().is_some_and(|n| !matches!(n, ')' | ']' | ' ')) {
            out.push(' ');
        }
    }
    out
}

/// The variant name of a step: `Sim(Block(…))` is `Block`, `Ui(LoadScenario(…))` is `LoadScenario`.
#[must_use]
pub fn step_kind(step: &Step) -> String {
    let text = ron::to_string(step).unwrap_or_default();
    let inner = text
        .strip_prefix("Sim(")
        .or_else(|| text.strip_prefix("Ui("))
        .unwrap_or(&text);
    inner
        .split(['(', ')', ' ', ','])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// One entry of the "Other action" list: the kind and a complete, valid example row.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionExample {
    /// The variant name (`Block`, `SelectLead`, …).
    pub kind: String,
    pub step: Step,
}

impl ActionExample {
    /// The example row's text.
    #[must_use]
    pub fn text(&self) -> String {
        step_text(&self.step)
    }
}

/// One example of every kind of action a stage may use: the scenario interventions of the timeline
/// editor's list ([`heartsim_ui::timeline_editor::templates`]) and every application command, each
/// with real values so that inserting it gives a row that parses. A test parses every one.
#[must_use]
pub fn action_examples() -> Vec<ActionExample> {
    let mut steps: Vec<Step> = heartsim_ui::timeline_editor::templates()
        .into_iter()
        .map(Step::Sim)
        .collect();
    let ui = |a: UiAction| Step::Ui(a);
    steps.extend([
        ui(UiAction::LoadScenario("sinus_normal".into())),
        ui(UiAction::StartReentry("af_persistent".into())),
        ui(UiAction::FocusPanel(Panel::TwelveLead)),
        ui(UiAction::Highlight("pause".into())),
        ui(UiAction::CameraPreset(CameraView::Lao)),
        ui(UiAction::SetSpeed(0.5)),
        ui(UiAction::Pause),
        ui(UiAction::Resume),
        ui(UiAction::WaitForEvent(WaitEvent::ElapsedMs(3000.0))),
        ui(UiAction::WaitForEvent(WaitEvent::Qrs)),
        ui(UiAction::SetQrsAxis(60.0)),
        ui(UiAction::SelectLead(Lead::II)),
        ui(UiAction::HeartSoundAudio(true)),
        ui(UiAction::HeartSoundLane(true)),
        ui(UiAction::Listen {
            site: StethoscopeSite::Mitral,
            chest: ChestPieceChoice::Diaphragm,
        }),
        ui(UiAction::SoundCheck),
        ui(UiAction::ProbeAtWindow(ProbeWindow::Apical)),
        ui(UiAction::ProbeAtView("a4c".into())),
        ui(UiAction::SetEchoView("plax".into())),
        ui(UiAction::EchoMeasure("lvid".into())),
    ]);
    steps
        .into_iter()
        .map(|step| ActionExample {
            kind: step_kind(&step),
            step,
        })
        .collect()
}

/// The panels the "Show panel" helper offers, in the order of the page tabs.
pub const PANELS: [Panel; 11] = [
    Panel::Simulator,
    Panel::TwelveLead,
    Panel::HeartVector,
    Panel::AxisTrainer,
    Panel::AxisQuiz,
    Panel::Electrodes,
    Panel::RhythmQuiz,
    Panel::Tutorials,
    Panel::SoundTrainer,
    Panel::AuscultationQuiz,
    Panel::Echo,
];

/// The English name of a panel, the variant's own (`TwelveLead`), as written in the rows.
#[must_use]
pub fn panel_name(p: Panel) -> String {
    ron::to_string(&p).unwrap_or_default()
}

/// One stage of the draft.
#[derive(Debug, Clone, PartialEq)]
pub struct StageDraft {
    pub title: String,
    /// The vignette, CommonMark.
    pub markdown: String,
    pub rows: Vec<Row>,
    pub questions: Vec<Question>,
}

impl StageDraft {
    /// A stage with nothing written yet.
    #[must_use]
    pub fn blank() -> Self {
        Self {
            title: String::new(),
            markdown: String::new(),
            rows: Vec::new(),
            questions: Vec::new(),
        }
    }

    fn from_stage(s: &Stage) -> Self {
        Self {
            title: s.title.clone(),
            markdown: s.markdown.clone(),
            rows: s.actions.iter().map(Row::from_step).collect(),
            questions: s.questions.clone(),
        }
    }
}

/// A question with empty text and the least number of choices; the first choice is the answer
/// until the teacher sets it.
#[must_use]
pub fn blank_question() -> Question {
    Question {
        prompt: String::new(),
        choices: vec![String::new(); MIN_CHOICES],
        answer: 0,
        explanation: String::new(),
    }
}

/// One line of the problems panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The stage (0-based) the line names, if any; a click selects it.
    pub stage: Option<usize>,
    pub text: String,
    /// `true` when it stops Save as…; `false` for editorial advice.
    pub blocking: bool,
}

/// The case being written: everything the form edits, in the text the teacher typed.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    /// Equals the case's id and the file's name.
    pub id: String,
    /// `true` once the teacher typed in the id field: the title no longer rewrites it.
    pub id_edited: bool,
    pub title: String,
    pub summary: String,
    /// Who the patient is, one line.
    pub patient: String,
    /// BCP 47 tag of the case text.
    pub language: String,
    /// One author per line.
    pub authors: String,
    /// SPDX id or free text.
    pub licence: String,
    /// One citation per line.
    pub references: String,
    pub stages: Vec<StageDraft>,
    /// Rhythms bundled into the file: `(id, rhythm file text)`, sorted by id.
    pub bundled: Vec<(String, String)>,
}

impl Default for Draft {
    fn default() -> Self {
        Self::new()
    }
}

impl Draft {
    /// An empty case with one blank stage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: id_from_title(""),
            id_edited: false,
            title: String::new(),
            summary: String::new(),
            patient: String::new(),
            language: heartsim_ui::i18n::language().to_owned(),
            authors: String::new(),
            licence: String::new(),
            references: String::new(),
            stages: vec![StageDraft::blank()],
            bundled: Vec::new(),
        }
    }

    /// A draft that starts as a copy of `p` (the installed case is not touched). The id and the
    /// bundled rhythms are kept: saving the copy of a file of My cases replaces that file; the copy
    /// of a built-in case gets its own id and title (`new_id`, `new_title`).
    #[must_use]
    pub fn from_package(p: &CasePackage, rename: Option<(&str, &str)>) -> Self {
        let c = &p.case;
        let (id, title) = rename.map_or((c.id.clone(), c.title.clone()), |(i, t)| {
            (i.to_owned(), t.to_owned())
        });
        Self {
            id,
            id_edited: true,
            title,
            summary: c.summary.clone(),
            patient: c.patient.clone(),
            language: p.manifest.language.clone(),
            authors: p.manifest.authors.join("\n"),
            licence: p.manifest.licence.clone(),
            references: c.references.join("\n"),
            stages: c.stages.iter().map(StageDraft::from_stage).collect(),
            bundled: p.scenarios.clone(),
        }
    }

    /// The id a copy of the built-in case `id` gets: `<id>_copy` cut to the id limit.
    #[must_use]
    pub fn copy_id(id: &str) -> String {
        let mut s = format!("{id}_copy");
        if s.len() > MAX_ID_LEN {
            s = format!("{}_copy", &id[..MAX_ID_LEN - 5]);
        }
        s
    }

    /// Set the title; while the id was not typed by hand it follows the title.
    pub fn set_title(&mut self, title: &str) {
        title.clone_into(&mut self.title);
        if !self.id_edited {
            self.id = id_from_title(title);
        }
    }

    /// Set the id by hand.
    pub fn set_id(&mut self, id: &str) {
        id.clone_into(&mut self.id);
        self.id_edited = true;
    }

    /// Let the id follow the title again.
    pub fn reset_id(&mut self) {
        self.id_edited = false;
        self.id = id_from_title(&self.title);
    }

    /// The authors, one per non-blank line.
    #[must_use]
    pub fn author_list(&self) -> Vec<String> {
        lines(&self.authors)
    }

    /// The case as it will be written. A row that does not parse is left out (the problems panel
    /// names it and Save as… is off meanwhile).
    #[must_use]
    pub fn to_case(&self) -> Case {
        Case {
            format_version: CASE_FORMAT_VERSION,
            id: self.id.clone(),
            title: self.title.clone(),
            summary: self.summary.clone(),
            patient: self.patient.clone(),
            references: lines(&self.references),
            stages: self
                .stages
                .iter()
                .map(|s| Stage {
                    title: s.title.clone(),
                    markdown: s.markdown.clone(),
                    actions: s.rows.iter().filter_map(|r| r.parsed.clone().ok()).collect(),
                    questions: s.questions.clone(),
                })
                .collect(),
        }
    }

    /// The case file's content: the case, a manifest made from the file details (`created` and
    /// `generator` are given by the caller: the clock and the program version) and the bundled
    /// rhythms.
    #[must_use]
    pub fn to_package(&self, created: &str, generator: &str) -> CasePackage {
        let case = self.to_case();
        let mut manifest =
            Manifest::for_case(&case, self.author_list(), self.licence.trim(), created, generator);
        self.language.trim().clone_into(&mut manifest.language);
        CasePackage::new(manifest, case, self.bundled.clone())
    }

    // ---- stages ------------------------------------------------------------------------------

    /// Append a blank stage; returns its index.
    pub fn add_stage(&mut self) -> usize {
        self.stages.push(StageDraft::blank());
        self.stages.len() - 1
    }

    /// Insert a copy of stage `i` after it; returns the copy's index.
    pub fn duplicate_stage(&mut self, i: usize) -> usize {
        let Some(s) = self.stages.get(i).cloned() else {
            return i;
        };
        self.stages.insert(i + 1, s);
        i + 1
    }

    /// Delete stage `i` unless it is the only one; returns the stage to select.
    pub fn delete_stage(&mut self, i: usize) -> usize {
        if self.stages.len() > 1 && i < self.stages.len() {
            self.stages.remove(i);
        }
        i.min(self.stages.len() - 1)
    }

    /// Move stage `i` one place up (`up`) or down; returns where it is now.
    pub fn move_stage(&mut self, i: usize, up: bool) -> usize {
        let j = if up { i.checked_sub(1) } else { Some(i + 1) };
        match j {
            Some(j) if i < self.stages.len() && j < self.stages.len() => {
                self.stages.swap(i, j);
                j
            }
            _ => i,
        }
    }

    // ---- rows --------------------------------------------------------------------------------

    /// Append a row that writes `step` to stage `si`.
    pub fn add_step(&mut self, si: usize, step: &Step) {
        if let Some(s) = self.stages.get_mut(si) {
            s.rows.push(Row::from_step(step));
        }
    }

    /// The "Load rhythm" helper: a row that loads rhythm `id`.
    pub fn insert_load_rhythm(&mut self, si: usize, id: &str) {
        self.add_step(si, &Step::Ui(UiAction::LoadScenario(id.to_owned())));
    }

    /// Delete row `ri` of stage `si`.
    pub fn delete_row(&mut self, si: usize, ri: usize) {
        if let Some(s) = self.stages.get_mut(si)
            && ri < s.rows.len()
        {
            s.rows.remove(ri);
        }
    }

    /// Move row `ri` of stage `si` one place up or down.
    pub fn move_row(&mut self, si: usize, ri: usize, up: bool) {
        if let Some(s) = self.stages.get_mut(si) {
            let j = if up { ri.checked_sub(1) } else { Some(ri + 1) };
            if let Some(j) = j
                && ri < s.rows.len()
                && j < s.rows.len()
            {
                s.rows.swap(ri, j);
            }
        }
    }

    // ---- questions ---------------------------------------------------------------------------

    /// Append a blank question to stage `si`; returns its index.
    pub fn add_question(&mut self, si: usize) -> usize {
        let Some(s) = self.stages.get_mut(si) else {
            return 0;
        };
        s.questions.push(blank_question());
        s.questions.len() - 1
    }

    /// Delete question `qi` of stage `si`.
    pub fn delete_question(&mut self, si: usize, qi: usize) {
        if let Some(s) = self.stages.get_mut(si)
            && qi < s.questions.len()
        {
            s.questions.remove(qi);
        }
    }

    /// Move question `qi` of stage `si` one place up or down; its right answer goes with it.
    pub fn move_question(&mut self, si: usize, qi: usize, up: bool) {
        if let Some(s) = self.stages.get_mut(si) {
            let j = if up { qi.checked_sub(1) } else { Some(qi + 1) };
            if let Some(j) = j
                && qi < s.questions.len()
                && j < s.questions.len()
            {
                s.questions.swap(qi, j);
            }
        }
    }

    /// Mark choice `ci` as the right answer of the question.
    pub fn set_answer(&mut self, si: usize, qi: usize, ci: usize) {
        if let Some(q) = self.question_mut(si, qi)
            && ci < q.choices.len()
        {
            q.answer = ci;
        }
    }

    /// Add an empty choice (up to [`MAX_CHOICES`]).
    pub fn add_choice(&mut self, si: usize, qi: usize) {
        if let Some(q) = self.question_mut(si, qi)
            && q.choices.len() < MAX_CHOICES
        {
            q.choices.push(String::new());
        }
    }

    /// Remove choice `ci` (down to [`MIN_CHOICES`]). The right answer stays the same choice; when
    /// it is the one removed, the first choice becomes the answer.
    pub fn remove_choice(&mut self, si: usize, qi: usize, ci: usize) {
        if let Some(q) = self.question_mut(si, qi)
            && q.choices.len() > MIN_CHOICES
            && ci < q.choices.len()
        {
            q.choices.remove(ci);
            q.answer = match q.answer.cmp(&ci) {
                std::cmp::Ordering::Greater => q.answer - 1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Less => q.answer,
            };
        }
    }

    fn question_mut(&mut self, si: usize, qi: usize) -> Option<&mut Question> {
        self.stages.get_mut(si)?.questions.get_mut(qi)
    }

    // ---- bundled rhythms ---------------------------------------------------------------------

    /// Ids of the rhythms the actions load (parsed rows only), in order, without repeats.
    #[must_use]
    pub fn loaded_rhythms(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in &self.stages {
            for r in &s.rows {
                if let Ok(Step::Ui(UiAction::LoadScenario(id))) = &r.parsed
                    && !out.contains(id)
                {
                    out.push(id.clone());
                }
            }
        }
        out
    }

    /// Whether the rhythm `id` is bundled in the file.
    #[must_use]
    pub fn is_bundled(&self, id: &str) -> bool {
        self.bundled.iter().any(|(b, _)| b == id)
    }

    /// Rhythms the draft loads that came from another case file or were added in this session,
    /// and are not bundled yet: the editor offers to bundle them.
    #[must_use]
    pub fn to_bundle(&self, scenarios: &ScenarioLibrary) -> Vec<String> {
        self.loaded_rhythms()
            .into_iter()
            .filter(|id| !self.is_bundled(id) && scenarios.is_bundleable(id))
            .collect()
    }

    /// Bundle the library's rhythm `id` into the file.
    ///
    /// # Errors
    /// When the rhythm is a built-in one (every copy of heartsim has it; a file may never replace
    /// it), is not in the library, or cannot be written.
    pub fn bundle(&mut self, id: &str, scenarios: &ScenarioLibrary) -> Result<(), String> {
        if scenarios.is_builtin(id) {
            return Err(t!("ced-bundle-builtin", id = id));
        }
        let entry = scenarios
            .index_of(id)
            .and_then(|i| scenarios.entries.get(i))
            .ok_or_else(|| t!("ced-bundle-unknown", id = id))?;
        let text = crate::scenario_file::to_ron(entry).map_err(|e| e.to_string())?;
        self.bundled.retain(|(b, _)| b != id);
        self.bundled.push((id.to_owned(), text));
        self.bundled.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(())
    }

    /// Stop bundling rhythm `id`.
    pub fn unbundle(&mut self, id: &str) {
        self.bundled.retain(|(b, _)| b != id);
    }

    // ---- problems ----------------------------------------------------------------------------

    /// What is wrong with the draft: blocking problems first (each stops Save as…), then the
    /// editorial advice of [`Case::content_problems`]. `scenarios` is the session's rhythm
    /// library.
    #[must_use]
    pub fn problems(&self, scenarios: &ScenarioLibrary) -> Vec<Problem> {
        let mut out = Vec::new();
        let block = |stage: Option<usize>, text: String| Problem {
            stage,
            text,
            blocking: true,
        };

        // the rules of the file's manifest
        if !valid_id(&self.id) {
            out.push(block(None, t!("ced-p-id", max = MAX_ID_LEN.to_string())));
        }
        for (field, text, max) in [
            (t!("ced-title"), self.title.as_str(), MAX_FIELD_CHARS),
            (t!("ced-licence"), self.licence.trim(), MAX_FIELD_CHARS),
            (t!("ced-language"), self.language.trim(), MAX_LANGUAGE_CHARS),
        ] {
            if text.chars().count() > max {
                out.push(block(
                    None,
                    t!("ced-p-long", field = field, max = max.to_string()),
                ));
            }
        }
        let tag = self.language.trim();
        if tag.is_empty() || !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            out.push(block(None, t!("ced-p-language")));
        }
        let authors = self.author_list();
        if authors.len() > MAX_AUTHORS {
            out.push(block(
                None,
                t!("ced-p-authors-many", max = MAX_AUTHORS.to_string()),
            ));
        }
        if authors.iter().any(|a| a.chars().count() > MAX_FIELD_CHARS) {
            out.push(block(
                None,
                t!(
                    "ced-p-long",
                    field = t!("ced-authors"),
                    max = MAX_FIELD_CHARS.to_string()
                ),
            ));
        }

        // the case itself
        let case = self.to_case();
        let structural = case.problems();
        for p in &structural {
            out.push(block(stage_of(p), p.clone()));
        }
        if self.approx_bytes() > MAX_ENTRY_BYTES / 2
            && !case.to_ron().is_ok_and(|s| s.len() as u64 <= MAX_ENTRY_BYTES)
        {
            out.push(block(None, t!("ced-p-too-big")));
        }

        // the action rows
        for (si, s) in self.stages.iter().enumerate() {
            for (ri, r) in s.rows.iter().enumerate() {
                match &r.parsed {
                    Err(e) => out.push(block(
                        Some(si),
                        t!(
                            "ced-p-row",
                            stage = (si + 1).to_string(),
                            row = (ri + 1).to_string(),
                            error = e.as_str()
                        ),
                    )),
                    Ok(Step::Ui(UiAction::LoadScenario(id))) if !self.rhythm_known(id, scenarios) => {
                        out.push(block(
                            Some(si),
                            t!(
                                "ced-p-unknown-rhythm",
                                stage = (si + 1).to_string(),
                                row = (ri + 1).to_string(),
                                id = id.as_str()
                            ),
                        ));
                    }
                    Ok(_) => {}
                }
            }
        }

        // rhythms of another file or of this session must travel with the case
        for id in self.to_bundle(scenarios) {
            out.push(block(None, t!("ced-p-bundle", id = id.as_str())));
        }
        for (id, _) in &self.bundled {
            if !valid_id(id) || scenarios.is_builtin(id) {
                out.push(block(None, t!("ced-bundle-builtin", id = id.as_str())));
            }
        }

        // editorial advice
        for p in case.content_problems() {
            if !structural.contains(&p) {
                out.push(Problem {
                    stage: stage_of(&p),
                    text: p,
                    blocking: false,
                });
            }
        }
        if authors.is_empty() {
            out.push(Problem {
                stage: None,
                text: t!("ced-w-authors"),
                blocking: false,
            });
        }
        if self.licence.trim().is_empty() {
            out.push(Problem {
                stage: None,
                text: t!("ced-w-licence"),
                blocking: false,
            });
        }
        out
    }

    /// Whether `id` names a rhythm the case can load: bundled, or in the session's library.
    #[must_use]
    pub fn rhythm_known(&self, id: &str, scenarios: &ScenarioLibrary) -> bool {
        self.is_bundled(id) || scenarios.index_of(id).is_some()
    }

    /// `true` when nothing stops Save as….
    #[must_use]
    pub fn can_save(&self, scenarios: &ScenarioLibrary) -> bool {
        !self.problems(scenarios).iter().any(|p| p.blocking)
    }

    /// Rough size of the texts, bytes: decides whether the exact size is worth computing.
    fn approx_bytes(&self) -> u64 {
        let mut n = self.summary.len() + self.patient.len() + self.references.len();
        for s in &self.stages {
            n += s.title.len() + s.markdown.len();
            n += s.rows.iter().map(|r| r.text.len()).sum::<usize>();
            for q in &s.questions {
                n += q.prompt.len() + q.explanation.len() + q.choices.iter().map(String::len).sum::<usize>();
            }
        }
        n as u64
    }
}

/// The non-blank lines of a text, trimmed.
fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The stage number a [`Case::problems`] or [`Case::content_problems`] line names (`… stage 3 …`),
/// as a 0-based index.
#[must_use]
pub fn stage_of(problem: &str) -> Option<usize> {
    let at = problem.find(" stage ")? + " stage ".len();
    let digits: String = problem[at..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<usize>().ok()?.checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> ScenarioLibrary {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios");
        ScenarioLibrary::load_dir(&dir).unwrap()
    }

    #[test]
    fn the_id_of_a_title_is_valid_and_stable() {
        for (title, id) in [
            ("Chest pain at the fair", "chest_pain_at_the_fair"),
            ("  Syncope -- 62 y/o!  ", "syncope_62_y_o"),
            ("Dolore toracico: caffè e crème", "dolore_toracico_caffe_e_creme"),
            ("Größe Ärzte", "grosse_arzte"),
            ("", "case"),
            ("日本語", "case"),
            ("___", "case"),
        ] {
            let got = id_from_title(title);
            assert_eq!(got, id, "{title:?}");
            assert!(valid_id(&got));
            assert_eq!(id_from_title(title), got, "stable");
            assert_eq!(id_from_title(&got), got, "an id is its own title's id");
        }
        let long = id_from_title(&"word ".repeat(40));
        assert!(valid_id(&long), "{long}");
        assert!(long.len() <= MAX_ID_LEN && !long.ends_with('_'));
    }

    #[test]
    fn every_action_example_parses_to_the_step_it_names() {
        let examples = action_examples();
        assert!(examples.len() >= 25, "{}", examples.len());
        for e in &examples {
            let text = e.text();
            assert_eq!(parse_step(&text).as_ref(), Ok(&e.step), "{text}");
            assert!(text.starts_with(&format!(
                "{}(",
                if matches!(e.step, Step::Sim(_)) {
                    "Sim"
                } else {
                    "Ui"
                }
            )));
            assert!(
                text.contains(&e.kind) && !e.kind.is_empty(),
                "{} does not name {}",
                text,
                e.kind
            );
            assert!(!text.contains('\n'), "one line: {text}");
        }
        // one example per variant of the application commands, none twice
        let mut kinds: Vec<_> = examples.iter().map(|e| e.kind.clone()).collect();
        kinds.sort();
        let n = kinds.len();
        kinds.dedup();
        // `WaitForEvent` has two examples (a time and the next QRS)
        assert_eq!(kinds.len(), n - 1, "{kinds:?}");
        // the ids the examples name exist
        let lib = library();
        assert!(lib.index_of("sinus_normal").is_some());
        for id in ["a4c", "plax"] {
            assert!(heartsim_echo::ViewId::from_id(id).is_some(), "{id}");
        }
        assert!(heartsim_ui::highlight::TOOLS.contains(&"pause"));
    }

    #[test]
    fn rows_read_like_the_case_files_and_mark_what_does_not_parse() {
        let block = Step::Sim(heartsim_core::Action::Block {
            region: heartsim_core::Region::AvNodeCompact,
            blocked: true,
        });
        assert_eq!(
            step_text(&block),
            "Sim(Block(region: AvNodeCompact, blocked: true))"
        );
        assert_eq!(
            step_text(&Step::Ui(UiAction::LoadScenario("a,b: c".into()))),
            "Ui(LoadScenario(\"a,b: c\"))",
            "text inside a string is not spaced"
        );
        let mut r = Row::from_text("Ui(LoadScenario(\"x\"))");
        assert!(r.parsed.is_ok());
        r.set_text("Ui(LoadScenari(\"x\"))".into());
        assert!(r.parsed.is_err());
        assert_eq!(step_kind(&block), "Block");
    }

    fn filled() -> Draft {
        let mut d = Draft::new();
        d.set_title("A fainting spell");
        d.summary = "A walk to the shop.".into();
        d.patient = "62-year-old man".into();
        d.language = "en".into();
        d.authors = "A. Teacher".into();
        d.licence = "CC-BY-4.0".into();
        d.references = "Somebody 2026".into();
        d.stages[0].title = "At the door".into();
        d.stages[0].markdown = "He **fell**.".into();
        d.insert_load_rhythm(0, "sinus_normal");
        for n in 0..5 {
            let q = d.add_question(0);
            d.stages[0].questions[q].prompt = format!("Question {n}?");
            for (i, c) in d.stages[0].questions[q].choices.iter_mut().enumerate() {
                *c = format!("choice {i}");
            }
            d.stages[0].questions[q].explanation = "Because.".into();
            d.set_answer(0, q, n % 3);
        }
        d
    }

    #[test]
    fn a_draft_built_by_the_editors_operations_passes_the_case_rules() {
        let lib = library();
        let mut d = Draft::new();
        d.set_title("Operations only");
        d.insert_load_rhythm(0, "sinus_normal");
        let q = d.add_question(0);
        d.set_answer(0, q, 2);
        d.add_stage();
        assert!(d.to_case().problems().is_empty(), "{:?}", d.to_case().problems());
        assert_eq!(d.id, "operations_only");
        // the editorial advice does not block, the rules of the file do
        let p = d.problems(&lib);
        assert!(p.iter().any(|p| !p.blocking));
        assert!(p.iter().all(|p| !p.blocking), "{p:?}");
        assert!(d.can_save(&lib));
        let d = filled();
        assert!(d.problems(&lib).is_empty(), "{:?}", d.problems(&lib));
        // and the file it makes is one the reader accepts, with the same case
        let bytes = d.to_package("2026-11-27", "test").write().unwrap();
        let back = CasePackage::read(&bytes).unwrap();
        assert_eq!(back.case, d.to_case());
        assert_eq!(back.manifest.authors, ["A. Teacher"]);
        assert_eq!(back.manifest.licence, "CC-BY-4.0");
    }

    #[test]
    fn moving_and_deleting_keep_the_right_answer_with_its_question() {
        let mut d = filled();
        let before: Vec<(String, String)> = d.stages[0]
            .questions
            .iter()
            .map(|q| (q.prompt.clone(), q.choices[q.answer].clone()))
            .collect();
        d.move_question(0, 0, false);
        d.move_question(0, 3, true);
        d.move_question(0, 0, true); // already first: nothing
        d.move_question(0, 4, false); // already last: nothing
        let after: Vec<(String, String)> = d.stages[0]
            .questions
            .iter()
            .map(|q| (q.prompt.clone(), q.choices[q.answer].clone()))
            .collect();
        for pair in &before {
            assert!(after.contains(pair), "{pair:?} lost its answer");
        }
        assert_ne!(before, after);
        d.delete_question(0, 1);
        assert_eq!(d.stages[0].questions.len(), 4);
        for q in &d.stages[0].questions {
            assert!(before.contains(&(q.prompt.clone(), q.choices[q.answer].clone())));
        }
        // choices: removing one before the answer keeps the same choice as the answer
        d.add_choice(0, 0);
        d.stages[0].questions[0].choices[3] = "extra".into();
        d.set_answer(0, 0, 3);
        d.remove_choice(0, 0, 1);
        let q = &d.stages[0].questions[0];
        assert_eq!(q.choices[q.answer], "extra");
        let answer = q.answer;
        d.add_choice(0, 0);
        d.remove_choice(0, 0, answer);
        assert_eq!(d.stages[0].questions[0].answer, 0);
        for _ in 0..5 {
            d.add_choice(0, 1);
        }
        assert_eq!(d.stages[0].questions[1].choices.len(), MAX_CHOICES);
        for _ in 0..5 {
            d.remove_choice(0, 1, 0);
        }
        assert_eq!(d.stages[0].questions[1].choices.len(), MIN_CHOICES);

        // stages carry their questions and rows
        let mut d = filled();
        d.stages[0].title = "one".into();
        let b = d.add_stage();
        d.stages[b].title = "two".into();
        d.stages[b].questions.push(blank_question());
        d.stages[b].questions[0].prompt = "second?".into();
        d.stages[b].questions[0].answer = 2;
        assert_eq!(d.move_stage(b, true), 0);
        assert_eq!(d.stages[0].title, "two");
        assert_eq!(d.stages[0].questions[0].answer, 2);
        assert_eq!(d.stages[1].questions.len(), 5);
        let c = d.duplicate_stage(0);
        assert_eq!(c, 1);
        assert_eq!(d.stages[1], d.stages[0]);
        assert_eq!(d.stages.len(), 3);
        assert_eq!(d.delete_stage(0), 0);
        assert_eq!(d.stages[0].title, "two");
        assert_eq!(d.delete_stage(2), 1);
        assert_eq!(d.delete_stage(0), 0);
        assert_eq!(d.stages.len(), 1);
        assert_eq!(d.delete_stage(0), 0, "the last stage stays");
        assert_eq!(d.stages.len(), 1);
    }

    #[test]
    fn problems_name_rows_stages_limits_and_rhythms() {
        let lib = library();
        let mut d = filled();
        d.stages[0].rows[0].set_text("Ui(LoadScenari".into());
        d.add_stage();
        d.insert_load_rhythm(1, "no_such_rhythm");
        d.set_id("Bad Id");
        d.title = "x".repeat(MAX_FIELD_CHARS + 1);
        d.language = "en us".into();
        let p = d.problems(&lib);
        assert!(p.iter().any(|p| p.blocking && p.text.contains("id")), "{p:?}");
        assert!(
            p.iter()
                .any(|p| p.blocking && p.stage == Some(0) && p.text.contains("Stage 1, action 1")),
            "{p:?}"
        );
        assert!(
            p.iter()
                .any(|p| p.blocking && p.stage == Some(1) && p.text.contains("no_such_rhythm")),
            "{p:?}"
        );
        assert!(p.iter().any(|p| p.blocking && p.text.contains("200")), "{p:?}");
        assert!(
            p.iter().any(|p| p.blocking && p.text.contains("anguage")),
            "{p:?}"
        );
        assert!(!d.can_save(&lib));
        // the content rules name their stage
        let d = filled();
        let mut d2 = d.clone();
        d2.stages[0].markdown.clear();
        let p = d2.problems(&lib);
        assert!(
            p.iter()
                .any(|p| !p.blocking && p.stage == Some(0) && p.text.contains("empty text"))
        );
        assert_eq!(stage_of("c stage 12: x"), Some(11));
        assert_eq!(stage_of("c: no stages"), None);
    }

    #[test]
    fn a_rhythm_of_another_file_or_session_must_be_bundled_and_a_built_in_never() {
        let mut lib = library();
        let mut custom = lib.entries[lib.index_of("sinus_normal").unwrap()].clone();
        custom.scenario.id = "teachers_own".into();
        custom.scenario.title = "Teacher's own".into();
        crate::export_panel::add_to_library(&mut lib, custom);
        assert!(lib.is_bundleable("teachers_own") && !lib.is_builtin("teachers_own"));
        assert!(lib.is_builtin("sinus_normal") && !lib.is_bundleable("sinus_normal"));

        let mut d = filled();
        d.insert_load_rhythm(0, "teachers_own");
        assert_eq!(d.to_bundle(&lib), ["teachers_own"]);
        assert!(!d.can_save(&lib), "not bundled yet");
        d.bundle("teachers_own", &lib).unwrap();
        assert_eq!(d.to_bundle(&lib).len(), 0);
        assert!(d.can_save(&lib), "{:?}", d.problems(&lib));
        assert!(d.bundle("sinus_normal", &lib).is_err(), "a built-in id");
        assert!(d.bundle("nothing", &lib).is_err());
        // the file reads back, and it holds the rhythm
        let bytes = d.to_package("2026-11-27", "t").write().unwrap();
        let back = CasePackage::read_for(&bytes, |id| lib.is_builtin(id)).unwrap();
        assert_eq!(back.scenarios.len(), 1);
        d.unbundle("teachers_own");
        assert_eq!(d.bundled.len(), 0);

        // a bundled rhythm with a built-in id blocks saving whatever the way it got there
        d.bundled.push(("sinus_normal".into(), "x".into()));
        assert!(!d.can_save(&lib));
    }

    #[test]
    fn a_copy_keeps_the_text_and_leaves_the_original_alone() {
        let case = filled().to_case();
        let p = CasePackage::new(
            Manifest::for_case(&case, vec!["X".into()], "MIT", "2026-01-01", "t"),
            case.clone(),
            Vec::new(),
        );
        let mut d = Draft::from_package(&p, Some((&Draft::copy_id("a_case"), "Copy of A")));
        assert_eq!(d.id, "a_case_copy");
        d.stages[0].title = "changed".into();
        assert_eq!(p.case, case, "the package is not touched");
        assert_eq!(d.to_case().stages[0].questions, case.stages[0].questions);
        assert_eq!(Draft::copy_id(&"a".repeat(64)).len(), 64);
        assert!(valid_id(&Draft::copy_id(&"a".repeat(64))));
        d.set_title("Another");
        assert_eq!(d.id, "a_case_copy", "a copy keeps its id");
        d.reset_id();
        assert_eq!(d.id, "another");
    }
}
