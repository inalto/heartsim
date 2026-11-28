//! Scripted patient cases (brief B18) and their page (brief B41): the built-in cases, the user's
//! own case files ("My cases", see [`crate::case_io`]) and a minimal player.
//!
//! A case (`assets/cases/<id>.ron`, format in `heartsim_core::case`) is played stage by stage: the
//! reader shows the vignette, the stage's simulator actions run through the same
//! [`TutorialHost`] as tutorial pages, and each multiple-choice question is answered once. The
//! first answer counts for the score; the explanation is shown either way. Independent of the GPU
//! and of [`crate::HeartSimApp`], like [`crate::tutorial_ui`].

use crate::case_edit::Draft;
use crate::case_edit_ui::{Editor, EditorAction, editor_ui};
use crate::case_io::{CaseFileIo, CaseStore, IoEvent, NoIo, StorageStore};
use crate::scenarios::ScenarioLibrary;
use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use heartsim_casefile::{CaseFileError, CasePackage, Manifest};
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

/// Which case: one of the build's own (index into [`CaseLibrary::cases`]), one of My cases
/// (index into [`Cases::user`]) or the draft of the case editor played with "Try it" (B42; the
/// reader plays a snapshot of the draft, so the draft can be edited meanwhile without surprises).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseRef {
    Builtin(usize),
    User(usize),
    Draft,
}

/// What opens the case editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditStart {
    /// An empty case.
    New,
    /// A copy of this case; the case itself is never changed.
    Copy(CaseRef),
}

/// A case of My cases: opened from a file (or saved from a built-in one) by the user.
#[derive(Debug, Clone)]
pub struct UserCase {
    pub package: CasePackage,
    /// The file's bytes as they were opened: Save as… writes them unchanged.
    pub bytes: Vec<u8>,
    /// SHA-256 of [`Self::bytes`] ([`heartsim_casefile::sha256_hex`]): the identity of the file.
    pub sha256: String,
    /// Where it came from, shown on the card (a file name, "saved here", "from <host>" for B43).
    pub provenance: String,
}

/// Why a case file was not opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// The file breaks a rule of the format (`heartsim_casefile`).
    File(CaseFileError),
    /// It bundles a rhythm under an id another opened case file uses for a different rhythm.
    RhythmInUse { id: String, case: String },
    /// It bundles a rhythm under the id of a different rhythm the user added in this session.
    SessionRhythm { id: String },
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(e) => write!(f, "{e}"),
            Self::RhythmInUse { id, case } => {
                f.write_str(&t!("cases-rhythm-in-use", id = id.as_str(), case = case.as_str()))
            }
            Self::SessionRhythm { id } => f.write_str(&t!("cases-rhythm-session", id = id.as_str())),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<CaseFileError> for OpenError {
    fn from(e: CaseFileError) -> Self {
        Self::File(e)
    }
}

/// Authors written into the manifest when a built-in case is saved as a file.
pub const BUILTIN_AUTHORS: &str = "heartsim authors";
/// Licence of the shipped case texts (`ASSETS-LICENSES.md`), written into the manifest when a
/// built-in case is saved as a file.
pub const BUILTIN_LICENCE: &str = "MIT OR Apache-2.0";

/// Where the learner is in the open case.
#[derive(Debug)]
struct Session {
    case: CaseRef,
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

/// A message on the page: what the last open, save or remove did.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Status {
    ok: bool,
    text: String,
}

/// What [`Cases::library_ui`] asked the host to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PageOutcome {
    /// A case was started: show the reader.
    pub started: bool,
}

/// The cases (built in and My cases), the open case, and the Cases page.
pub struct Cases {
    pub library: CaseLibrary,
    /// My cases, in the order they were opened (those kept from earlier runs first, by id).
    pub user: Vec<UserCase>,
    session: Option<Session>,
    /// Best score so far per case (this run of the application), `(correct, of)`, keyed by
    /// [`Cases::score_key`] so a file with the id of a built-in case keeps its own score.
    best: std::collections::BTreeMap<String, (usize, usize)>,
    markdown: CommonMarkCache,
    io: Box<dyn CaseFileIo>,
    store: Box<dyn CaseStore>,
    status: Option<Status>,
    /// A user case waiting for the user to confirm its removal.
    confirm_remove: Option<usize>,
    /// Bytes handed to the save dialog for a built-in case or for the draft: they join My cases
    /// once written.
    pending_save: Option<Vec<u8>>,
    /// The draft the case editor (B42) holds, kept for the session (also while the page shows the
    /// list or another page is open).
    editor: Option<Editor>,
    /// `true` while the page shows the editor in place of the list.
    editing: bool,
    /// The case the reader plays for "Try it": a snapshot of the draft.
    trial: Option<Case>,
    /// What the draft was when Save as… was clicked: it counts as saved once the dialog reports the
    /// file written.
    pending_draft: Option<Draft>,
    /// An edit waiting for the user to confirm replacing the unsaved draft.
    confirm_replace: Option<EditStart>,
    /// The user was asked whether to discard the draft.
    confirm_discard: bool,
}

impl std::fmt::Debug for Cases {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cases")
            .field("cases", &self.library.cases.len())
            .field("user", &self.user.len())
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

/// The case `r` names, from the two lists (a free function so the reader can borrow the
/// markdown cache at the same time).
fn case_in<'a>(
    library: &'a CaseLibrary,
    user: &'a [UserCase],
    trial: Option<&'a Case>,
    r: CaseRef,
) -> Option<&'a Case> {
    match r {
        CaseRef::Builtin(i) => library.cases.get(i),
        CaseRef::User(i) => user.get(i).map(|u| &u.package.case),
        CaseRef::Draft => trial,
    }
}

/// The title shown for `r`: a user case with the id of a built-in one gets "(file)".
fn display_title(library: &CaseLibrary, user: &[UserCase], trial: Option<&Case>, r: CaseRef) -> String {
    let Some(c) = case_in(library, user, trial, r) else {
        return String::new();
    };
    match r {
        CaseRef::User(_) if library.cases.iter().any(|b| b.id == c.id) => {
            t!("cases-title-file", title = c.title.as_str())
        }
        _ => c.title.clone(),
    }
}

impl Cases {
    /// The cases of `library`, with no dialogs and My cases kept for the session only (tests; the
    /// application uses [`Self::with_platform`]).
    #[must_use]
    pub fn new(library: CaseLibrary) -> Self {
        Self::with_io(library, Box::new(NoIo), Box::new(StorageStore::session()))
    }

    /// The cases of `library` with the given dialogs and store; nothing is loaded from the store
    /// until [`Self::load_store`].
    #[must_use]
    pub fn with_io(library: CaseLibrary, io: Box<dyn CaseFileIo>, store: Box<dyn CaseStore>) -> Self {
        Self {
            library,
            user: Vec::new(),
            session: None,
            best: std::collections::BTreeMap::new(),
            markdown: CommonMarkCache::default(),
            io,
            store,
            status: None,
            confirm_remove: None,
            pending_save: None,
            editor: None,
            editing: false,
            trial: None,
            pending_draft: None,
            confirm_replace: None,
            confirm_discard: false,
        }
    }

    /// The application's: file dialogs (`rfd`), and My cases in `<user data dir>/cases/` on the
    /// desktop or in eframe storage (`storage`, the browser's `localStorage`) without `native`.
    #[must_use]
    pub fn with_platform(library: CaseLibrary, storage: Option<&dyn eframe::Storage>) -> Self {
        #[cfg(feature = "native")]
        let store: Box<dyn CaseStore> = {
            let _ = storage;
            match crate::case_io::DirStore::user_dir() {
                Some(d) => Box::new(d),
                None => Box::new(StorageStore::session()),
            }
        };
        #[cfg(not(feature = "native"))]
        let store: Box<dyn CaseStore> = Box::new(StorageStore::persistent(
            storage
                .and_then(|s| s.get_string(crate::case_io::STORAGE_KEY))
                .as_deref(),
        ));
        Self::with_io(library, Box::new(crate::case_io::DialogIo::default()), store)
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

    /// The key of the best score of `r`: the id for a built-in case, `file:<id>` for a user case.
    #[must_use]
    pub fn score_key(&self, r: CaseRef) -> Option<String> {
        let c = case_in(&self.library, &self.user, self.trial.as_ref(), r)?;
        match r {
            CaseRef::Builtin(_) => Some(c.id.clone()),
            CaseRef::User(_) => Some(format!("file:{}", c.id)),
            // a trial of the draft keeps no score
            CaseRef::Draft => None,
        }
    }

    /// The best score of `r` this session, `(correct, of)`.
    #[must_use]
    pub fn best(&self, r: CaseRef) -> Option<(usize, usize)> {
        self.best.get(&self.score_key(r)?).copied()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.session.is_some()
    }

    /// The open case and its current stage.
    #[must_use]
    pub fn position(&self) -> Option<(CaseRef, usize)> {
        self.session.as_ref().map(|s| (s.case, s.stage))
    }

    /// Questions answered correctly at the first try, and questions answered, in the open case.
    #[must_use]
    pub fn score(&self) -> Option<(usize, usize)> {
        self.session.as_ref().map(|s| (s.correct, s.answered))
    }

    /// Open built-in case `index` at its first stage.
    pub fn open(&mut self, index: usize) {
        self.start(CaseRef::Builtin(index));
    }

    /// Open case `r` at its first stage.
    pub fn start(&mut self, r: CaseRef) {
        if r != CaseRef::Draft {
            self.trial = None;
        }
        self.start_at(r, 0);
    }

    /// Open case `r` at stage `stage` (0-based; the editor's "Try it" starts at the selected one).
    fn start_at(&mut self, r: CaseRef, stage: usize) {
        let Some(first) =
            case_in(&self.library, &self.user, self.trial.as_ref(), r).and_then(|c| c.stages.get(stage))
        else {
            return;
        };
        self.session = Some(Session {
            case: r,
            stage,
            question: 0,
            run: PageRun::from_steps(first.actions.clone()),
            chosen: None,
            correct: 0,
            answered: 0,
            finished: false,
        });
    }

    /// Close the reader (the best score of a finished case is kept). When it played the draft of
    /// the case editor, the page shows the editor again.
    pub fn close(&mut self) {
        self.session = None;
        self.trial = None;
    }

    /// The case open in the reader.
    fn open_case(&self) -> Option<&Case> {
        let s = self.session.as_ref()?;
        case_in(&self.library, &self.user, self.trial.as_ref(), s.case)
    }

    /// Answer the current question with `choice` (only the first answer of a question counts).
    pub fn answer(&mut self, choice: usize) {
        let Some(s) = &self.session else { return };
        if s.chosen.is_some() {
            return;
        }
        let Some(right) = self
            .open_case()
            .and_then(|c| c.stages.get(s.stage))
            .and_then(|st| st.questions.get(s.question))
            .map(|q| q.answer)
        else {
            return;
        };
        let Some(s) = &mut self.session else { return };
        s.chosen = Some(choice);
        s.answered += 1;
        if choice == right {
            s.correct += 1;
        }
    }

    /// `true` when the current question was answered, or the stage has none left to ask.
    #[must_use]
    pub fn can_advance(&self) -> bool {
        self.session.as_ref().is_some_and(|s| {
            let n = self
                .open_case()
                .and_then(|c| c.stages.get(s.stage))
                .map_or(0, |st| st.questions.len());
            !s.finished && (s.question >= n || s.chosen.is_some())
        })
    }

    /// Next question, next stage, or the end of the case.
    pub fn advance(&mut self) {
        if !self.can_advance() {
            return;
        }
        let key = self.session.as_ref().and_then(|s| self.score_key(s.case));
        let Some(s) = &mut self.session else { return };
        let Some(case) = case_in(&self.library, &self.user, self.trial.as_ref(), s.case) else {
            return;
        };
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
                if let Some(key) = key {
                    let entry = self.best.entry(key).or_insert((0, 0));
                    if s.correct >= entry.0 {
                        *entry = (s.correct, s.answered);
                    }
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

    /// Open a case file from any source (a dialog, a dropped file, B43's online library): check it
    /// with the strict reader against the rhythms of this build, add its bundled rhythms to
    /// `scenarios` for the session, add it to My cases (replacing one with the same id) and keep a
    /// copy in the store. `provenance` is shown on the card. Returns its index in [`Self::user`].
    ///
    /// # Errors
    /// When the reader refuses the file, or a bundled rhythm's id is taken by another opened case.
    /// The page shows the message either way.
    pub fn open_bytes(
        &mut self,
        bytes: &[u8],
        provenance: &str,
        scenarios: &mut ScenarioLibrary,
    ) -> Result<usize, OpenError> {
        let result = self.open_bytes_inner(bytes, provenance, scenarios, true);
        match &result {
            Ok(i) => {
                let title = display_title(&self.library, &self.user, self.trial.as_ref(), CaseRef::User(*i));
                let opened = t!("cases-opened", title = title.as_str());
                self.status = Some(match self.status.take() {
                    Some(Status { ok: false, text: why }) => Status {
                        ok: false,
                        text: format!("{opened} {why}"),
                    },
                    _ => Status {
                        ok: true,
                        text: opened,
                    },
                });
            }
            Err(e) => {
                self.status = Some(Status {
                    ok: false,
                    text: t!("cases-open-failed", name = provenance, error = e.to_string()),
                });
            }
        }
        result
    }

    fn open_bytes_inner(
        &mut self,
        bytes: &[u8],
        provenance: &str,
        scenarios: &mut ScenarioLibrary,
        persist: bool,
    ) -> Result<usize, OpenError> {
        self.status = None;
        let package = CasePackage::read_for(bytes, |id| scenarios.is_builtin(id))?;
        let id = package.manifest.id.clone();
        let parsed = package.tier0_scenarios()?;
        // a rhythm id another opened case uses for a different rhythm would change that case
        for (sid, text) in &package.scenarios {
            if let Some(other) = self.user.iter().find(|u| {
                u.package.manifest.id != id && u.package.scenarios.iter().any(|(o, t)| o == sid && t != text)
            }) {
                return Err(OpenError::RhythmInUse {
                    id: sid.clone(),
                    case: other.package.case.title.clone(),
                });
            }
        }
        if let Some(s) = parsed.iter().find(|s| scenarios.would_replace_session_rhythm(s)) {
            return Err(OpenError::SessionRhythm {
                id: s.scenario.id.clone(),
            });
        }
        for s in parsed {
            let sid = s.scenario.id.clone();
            scenarios
                .add_from_case(s)
                .map_err(|_| CaseFileError::ScenarioClash(sid))?;
        }
        let user = UserCase {
            package,
            bytes: bytes.to_vec(),
            sha256: heartsim_casefile::sha256_hex(bytes),
            provenance: provenance.to_owned(),
        };
        let index = if let Some(i) = self.user.iter().position(|u| u.package.manifest.id == id) {
            if self.session.as_ref().is_some_and(|s| s.case == CaseRef::User(i)) {
                self.session = None;
            }
            self.user[i] = user;
            i
        } else {
            self.user.push(user);
            self.user.len() - 1
        };
        if persist && let Err(e) = self.store.put(&id, bytes) {
            self.status = Some(Status {
                ok: false,
                text: t!("cases-not-kept", error = e),
            });
        }
        Ok(index)
    }

    /// Open every case file of the store (My cases of earlier runs). Files that no longer open
    /// (a rhythm this build lacks) are skipped and named on the page.
    pub fn load_store(&mut self, scenarios: &mut ScenarioLibrary) {
        let mut failed = Vec::new();
        for (id, bytes) in self.store.load() {
            if let Err(e) = self.open_bytes_inner(&bytes, &t!("cases-from-earlier"), scenarios, false) {
                tracing::warn!("my case {id}: {e}");
                failed.push(format!("{id}: {e}"));
            }
        }
        self.status = (!failed.is_empty()).then(|| Status {
            ok: false,
            text: t!("cases-stored-failed", list = failed.join("; ")),
        });
    }

    /// Take the open dialog's result, if one finished, and open or report it. Call once per frame.
    /// Returns `true` when a case file was opened.
    pub fn poll_io(&mut self, scenarios: &mut ScenarioLibrary) -> bool {
        let Some(event) = self.io.poll() else {
            return false;
        };
        match event {
            IoEvent::Opened { name, bytes } => return self.open_bytes(&bytes, &name, scenarios).is_ok(),
            IoEvent::Saved { name } => {
                let mut text = t!("cases-saved", path = name.as_str());
                if let Some(bytes) = self.pending_save.take() {
                    match self.open_bytes_inner(&bytes, &t!("cases-saved-here"), scenarios, true) {
                        Ok(_) => text = format!("{text} {}", t!("cases-saved-added")),
                        Err(e) => text = format!("{text} {e}"),
                    }
                }
                self.status = Some(Status { ok: true, text });
                if let Some(saved) = self.pending_draft.take()
                    && let Some(ed) = &mut self.editor
                {
                    ed.mark_saved(saved);
                }
            }
            IoEvent::Cancelled => {
                self.pending_save = None;
                self.pending_draft = None;
            }
            IoEvent::Failed(e) => {
                self.pending_save = None;
                self.pending_draft = None;
                self.status = Some(Status { ok: false, text: e });
            }
        }
        false
    }

    /// The text to write into eframe storage under [`crate::case_io::STORAGE_KEY`] when My cases
    /// changed (browser only; `None` on the desktop, whose cases are files).
    pub fn take_storage_text(&mut self) -> Option<String> {
        self.store.take_storage_text()
    }

    /// The bytes Save as… writes for `r`: the file as opened for a user case, a new file for a
    /// built-in one (authors [`BUILTIN_AUTHORS`], licence [`BUILTIN_LICENCE`]).
    ///
    /// # Errors
    /// When the writer refuses the case (a built-in case always passes; a test guards it).
    pub fn file_for(&self, r: CaseRef) -> Result<(String, Vec<u8>), CaseFileError> {
        match r {
            CaseRef::User(i) => {
                let u = self
                    .user
                    .get(i)
                    .ok_or(CaseFileError::MissingEntry(heartsim_casefile::CASE_NAME))?;
                Ok((u.package.file_name(), u.bytes.clone()))
            }
            CaseRef::Draft => Err(CaseFileError::MissingEntry(heartsim_casefile::CASE_NAME)),
            CaseRef::Builtin(i) => {
                let c = self
                    .library
                    .cases
                    .get(i)
                    .ok_or(CaseFileError::MissingEntry(heartsim_casefile::CASE_NAME))?;
                let manifest = Manifest::for_case(
                    c,
                    vec![BUILTIN_AUTHORS.to_owned()],
                    BUILTIN_LICENCE,
                    &crate::case_io::today_iso(),
                    &crate::version_line(),
                );
                let p = CasePackage::new(manifest, c.clone(), Vec::new());
                Ok((p.file_name(), p.write()?))
            }
        }
    }

    /// Start Save as… for `r`.
    pub fn save_as(&mut self, r: CaseRef) {
        match self.file_for(r) {
            Ok((name, bytes)) => {
                if matches!(r, CaseRef::Builtin(_)) {
                    self.pending_save = Some(bytes.clone());
                }
                self.io.save(&name, bytes);
            }
            Err(e) => {
                self.status = Some(Status {
                    ok: false,
                    text: e.to_string(),
                });
            }
        }
    }

    /// Remove user case `i` from My cases and delete its stored copy (the page asks first). Its
    /// rhythms stay in the library for the rest of the session.
    pub fn remove_user(&mut self, i: usize) {
        if i >= self.user.len() {
            return;
        }
        match self.session.as_ref().map(|s| s.case) {
            Some(CaseRef::User(j)) if j == i => self.session = None,
            Some(CaseRef::User(j)) if j > i => {
                if let Some(s) = &mut self.session {
                    s.case = CaseRef::User(j - 1);
                }
            }
            _ => {}
        }
        let u = self.user.remove(i);
        let title = u.package.case.title.clone();
        self.status = Some(match self.store.remove(&u.package.manifest.id) {
            Ok(()) => Status {
                ok: true,
                text: t!("cases-removed", title = title.as_str()),
            },
            Err(e) => Status {
                ok: false,
                text: t!("cases-not-removed", error = e),
            },
        });
        // scores are keyed by id: a later file with this id starts afresh
        self.best.remove(&format!("file:{}", u.package.manifest.id));
    }

    /// Show that the file `name` could not be read (before the reader saw it).
    pub fn report(&mut self, name: &str, error: &str) {
        self.status = Some(Status {
            ok: false,
            text: t!("cases-open-failed", name = name, error = error),
        });
    }

    /// The message of the last open, save or remove, and whether it reports a success.
    #[must_use]
    pub fn status(&self) -> Option<(bool, &str)> {
        self.status.as_ref().map(|s| (s.ok, s.text.as_str()))
    }

    // ---- the case editor (B42) -----------------------------------------------------------------

    /// `true` while the page shows the editor in place of the list.
    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    /// The draft the editor holds, if any (it is kept for the session, also while the list shows).
    #[must_use]
    pub fn draft(&self) -> Option<&Draft> {
        self.editor.as_ref().map(|e| &e.draft)
    }

    /// The draft for changing it without the page (tests, scripted use).
    pub fn draft_mut(&mut self) -> Option<&mut Draft> {
        self.editor.as_mut().map(|e| &mut e.draft)
    }

    /// `true` when there is a draft with changes that are not saved.
    #[must_use]
    pub fn draft_is_unsaved(&self) -> bool {
        self.editor.as_ref().is_some_and(Editor::is_dirty)
    }

    /// Open the editor on a new case or on a copy of case `r`. When the editor holds a draft with
    /// unsaved changes nothing is replaced yet: the page asks first.
    pub fn begin_edit(&mut self, start: EditStart) {
        if self.draft_is_unsaved() {
            self.confirm_replace = Some(start);
        } else {
            self.force_edit(start);
        }
    }

    /// Open the editor on `start`, dropping the draft it holds.
    fn force_edit(&mut self, start: EditStart) {
        self.confirm_replace = None;
        self.confirm_discard = false;
        let draft = match start {
            EditStart::New => Some(Draft::new()),
            EditStart::Copy(CaseRef::Builtin(i)) => self.library.cases.get(i).map(|c| {
                let manifest = Manifest::for_case(
                    c,
                    vec![BUILTIN_AUTHORS.to_owned()],
                    BUILTIN_LICENCE,
                    &crate::case_io::today_iso(),
                    &crate::version_line(),
                );
                let p = CasePackage::new(manifest, c.clone(), Vec::new());
                Draft::from_package(
                    &p,
                    Some((
                        &Draft::copy_id(&c.id),
                        &t!("ced-copy-title", title = c.title.as_str()),
                    )),
                )
            }),
            EditStart::Copy(CaseRef::User(i)) => {
                self.user.get(i).map(|u| Draft::from_package(&u.package, None))
            }
            EditStart::Copy(CaseRef::Draft) => None,
        };
        if let Some(d) = draft {
            self.editor = Some(Editor::new(d));
            self.editing = true;
            self.session = None;
            self.trial = None;
        }
    }

    /// Leave the editor for the list; the draft stays.
    pub fn leave_editor(&mut self) {
        self.editing = false;
    }

    /// Drop the draft.
    pub fn discard_draft(&mut self) {
        self.editor = None;
        self.editing = false;
        self.confirm_discard = false;
        self.pending_draft = None;
    }

    /// Play the draft in the reader from stage `stage`: a snapshot of it, so the draft is not
    /// touched and closing the reader returns to the editor.
    pub fn try_draft(&mut self, stage: usize) {
        let Some(ed) = &self.editor else { return };
        self.trial = Some(ed.draft.to_case());
        self.start_at(CaseRef::Draft, stage);
    }

    /// Start Save as… for the draft. The checks the editor shows (`Draft::can_save`) have been made
    /// by the button; the writer reads the file back, so a refusal is shown on the page.
    pub fn save_draft(&mut self) {
        let Some(ed) = &self.editor else { return };
        let draft = ed.draft.clone();
        let package = draft.to_package(&crate::case_io::today_iso(), &crate::version_line());
        match package.write() {
            Ok(bytes) => {
                self.pending_save = Some(bytes.clone());
                self.pending_draft = Some(draft);
                self.io.save(&package.file_name(), bytes);
            }
            Err(e) => {
                self.status = Some(Status {
                    ok: false,
                    text: e.to_string(),
                });
            }
        }
    }

    /// The Cases page: the editor while one is open, else the list ([`Self::library_ui`]).
    /// `scenarios` is the session's rhythm library (the editor's rhythm picker and its checks).
    pub fn page_ui(
        &mut self,
        ui: &mut egui::Ui,
        primary_allowed: bool,
        scenarios: &ScenarioLibrary,
    ) -> PageOutcome {
        if !(self.editing && self.editor.is_some()) {
            return self.library_ui(ui, primary_allowed);
        }
        let busy = self.io.busy();
        let Some(mut ed) = self.editor.take() else {
            return PageOutcome::default();
        };
        let status = self.status.as_ref().map(|s| (s.ok, s.text.as_str()));
        let action = editor_ui(ui, &mut ed, scenarios, &mut self.markdown, busy, status);
        let stage = ed.selected;
        self.editor = Some(ed);
        let mut outcome = PageOutcome::default();
        match action {
            Some(EditorAction::Back) => self.leave_editor(),
            Some(EditorAction::TryIt) => {
                self.try_draft(stage);
                outcome.started = self.is_open();
            }
            Some(EditorAction::Save) => self.save_draft(),
            None => {}
        }
        outcome
    }

    /// The Cases page (B41): Open case file…, then the built-in cases, My cases and the online
    /// library's slot, each case a card with Start and Save as… (and Remove for a user case).
    /// `primary_allowed`: whether one Start may be the page's primary button.
    pub fn library_ui(&mut self, ui: &mut egui::Ui, primary_allowed: bool) -> PageOutcome {
        let mut outcome = PageOutcome::default();
        ui.heading(t!("case-heading"));
        ui.label(t!("case-intro"));
        heartsim_ui::helper_text(ui, t!("cases-trust"));
        let busy = self.io.busy();
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            let r = ui
                .add_enabled(!busy, egui::Button::new(t!("cases-open-file")))
                .on_hover_text(t!("cases-open-file-hint"));
            if r.clicked() {
                self.io.open();
            }
            if busy {
                ui.spinner();
            }
            if ui
                .button(t!("ced-new"))
                .on_hover_text(t!("ced-new-hint"))
                .clicked()
            {
                action = Some(CardAction::Edit(EditStart::New));
            }
            heartsim_ui::helper_text(ui, t!("cases-drop-hint"));
        });
        if let Some(a) = self.draft_ui(ui) {
            action = Some(a);
        }
        if let Some(s) = &self.status {
            if s.ok {
                cards::result_box(ui, true, s.text.as_str());
            } else {
                ui.colored_label(ui.visuals().error_fg_color, s.text.as_str());
            }
        }
        // the one primary button of the page: the first built-in case not finished yet
        let primary = primary_allowed
            .then(|| (0..self.library.cases.len()).find(|&i| self.best(CaseRef::Builtin(i)).is_none()))
            .flatten();

        ui.add_space(6.0);
        cards::subheading(ui, t!("cases-builtin"));
        if self.library.cases.is_empty() {
            ui.label(t!("case-none"));
        }
        for i in 0..self.library.cases.len() {
            let r = CaseRef::Builtin(i);
            if let Some(a) = self.card(ui, r, primary == Some(i), busy) {
                action = Some(a);
            }
        }

        ui.add_space(6.0);
        cards::subheading(ui, t!("cases-mine"));
        if self.user.is_empty() {
            ui.label(t!("cases-mine-none"));
        }
        for i in 0..self.user.len() {
            if let Some(a) = self.card(ui, CaseRef::User(i), false, busy) {
                action = Some(a);
            }
        }
        heartsim_ui::helper_text(ui, self.store.describe());

        ui.add_space(6.0);
        cards::subheading(ui, t!("cases-online"));
        heartsim_ui::helper_text(ui, t!("cases-online-later"));

        match action {
            Some(CardAction::Start(r)) => {
                self.start(r);
                outcome.started = self.is_open();
            }
            Some(CardAction::Save(r)) => self.save_as(r),
            Some(CardAction::AskRemove(i)) => self.confirm_remove = Some(i),
            Some(CardAction::Remove(i)) => {
                self.confirm_remove = None;
                self.remove_user(i);
            }
            Some(CardAction::KeepIt) => self.confirm_remove = None,
            Some(CardAction::Edit(start)) => self.begin_edit(start),
            Some(CardAction::Resume) => {
                self.editing = true;
                self.confirm_replace = None;
            }
            Some(CardAction::AskDiscard) => self.confirm_discard = true,
            Some(CardAction::Discard) => self.discard_draft(),
            Some(CardAction::KeepDraft) => {
                self.confirm_discard = false;
                self.confirm_replace = None;
            }
            Some(CardAction::ReplaceDraft(start)) => self.force_edit(start),
            None => {}
        }
        outcome
    }

    /// The draft of the editor on the list: continue or discard it, and the confirmation of a
    /// replacement the user asked for.
    fn draft_ui(&self, ui: &mut egui::Ui) -> Option<CardAction> {
        let ed = self.editor.as_ref()?;
        let mut action = None;
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(t!("ced-draft-card", title = ed.display_title()));
                if ed.is_dirty() {
                    cards::pill(ui, t!("ced-not-saved"));
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button(t!("ced-continue")).clicked() {
                    action = Some(CardAction::Resume);
                }
                if self.confirm_discard {
                    ui.label(t!("ced-discard-confirm"));
                    if ui.button(t!("ced-discard-yes")).clicked() {
                        action = Some(CardAction::Discard);
                    }
                    if ui.button(t!("ced-discard-no")).clicked() {
                        action = Some(CardAction::KeepDraft);
                    }
                } else if ui.button(t!("ced-discard")).clicked() {
                    action = Some(CardAction::AskDiscard);
                }
            });
            if let Some(start) = self.confirm_replace {
                ui.label(t!("ced-replace-confirm"));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(t!("ced-replace-yes")).clicked() {
                        action = Some(CardAction::ReplaceDraft(start));
                    }
                    if ui.button(t!("ced-replace-no")).clicked() {
                        action = Some(CardAction::KeepDraft);
                    }
                });
            }
        });
        action
    }

    /// One case card; returns what was clicked.
    fn card(&self, ui: &mut egui::Ui, r: CaseRef, primary: bool, busy: bool) -> Option<CardAction> {
        let c = case_in(&self.library, &self.user, self.trial.as_ref(), r)?;
        let title = display_title(&self.library, &self.user, self.trial.as_ref(), r);
        let mut action = None;
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.strong(&title);
                if let Some((ok, of)) = self.best(r) {
                    heartsim_ui::helper_text(
                        ui,
                        t!("case-best", correct = ok.to_string(), total = of.to_string()),
                    );
                }
            });
            ui.label(&c.patient);
            ui.label(&c.summary);
            heartsim_ui::helper_text(
                ui,
                t!(
                    "case-size",
                    stages = c.stages.len().to_string(),
                    questions = c.question_count().to_string()
                ),
            );
            if let CaseRef::User(i) = r {
                let m = &self.user[i].package.manifest;
                let not_stated = t!("cases-not-stated");
                let authors = if m.authors.is_empty() {
                    not_stated.clone()
                } else {
                    m.authors.join(", ")
                };
                let licence = if m.licence.trim().is_empty() {
                    not_stated
                } else {
                    m.licence.clone()
                };
                heartsim_ui::helper_text(
                    ui,
                    t!(
                        "cases-credits",
                        authors = authors,
                        licence = licence,
                        source = self.user[i].provenance.as_str()
                    ),
                );
            }
            ui.horizontal_wrapped(|ui| {
                let label = t!("case-start", title = title.as_str());
                let button = if primary {
                    heartsim_ui::primary_button(ui, label)
                } else {
                    ui.button(label)
                };
                if button.clicked() {
                    action = Some(CardAction::Start(r));
                }
                let save = ui
                    .add_enabled(!busy, egui::Button::new(t!("cases-save-as")))
                    .on_hover_text(t!("cases-save-as-hint", title = title.as_str()));
                if save.clicked() {
                    action = Some(CardAction::Save(r));
                }
                if ui
                    .button(t!("ced-edit-copy"))
                    .on_hover_text(t!("ced-edit-copy-hint", title = title.as_str()))
                    .clicked()
                {
                    action = Some(CardAction::Edit(EditStart::Copy(r)));
                }
                if let CaseRef::User(i) = r {
                    if self.confirm_remove == Some(i) {
                        ui.label(t!("cases-remove-confirm", title = title.as_str()));
                        if ui.button(t!("cases-remove-yes")).clicked() {
                            action = Some(CardAction::Remove(i));
                        }
                        if ui.button(t!("cases-remove-no")).clicked() {
                            action = Some(CardAction::KeepIt);
                        }
                    } else if ui
                        .button(t!("cases-remove"))
                        .on_hover_text(t!("cases-remove-hint"))
                        .clicked()
                    {
                        action = Some(CardAction::AskRemove(i));
                    }
                }
            });
        });
        action
    }
}

/// What a click on a card asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardAction {
    Start(CaseRef),
    Save(CaseRef),
    AskRemove(usize),
    Remove(usize),
    KeepIt,
    /// Open the editor (B42).
    Edit(EditStart),
    /// Back to the editor with the draft it holds.
    Resume,
    AskDiscard,
    Discard,
    /// Keep the draft: answers either confirmation.
    KeepDraft,
    ReplaceDraft(EditStart),
}

impl Cases {
    /// The reader for the open case (nothing if none is open).
    pub fn reader_ui(&mut self, ui: &mut egui::Ui) {
        let Some(s) = &self.session else { return };
        let (stage_i, question_i, chosen, finished) = (s.stage, s.question, s.chosen, s.finished);
        let (correct, answered) = (s.correct, s.answered);
        let running = !s.run.is_finished();
        let Some(case) = case_in(&self.library, &self.user, self.trial.as_ref(), s.case) else {
            return;
        };
        let title = display_title(&self.library, &self.user, self.trial.as_ref(), s.case);
        let total = case.question_count();
        let stage = &case.stages[stage_i];
        let mut close = false;
        let mut pick = None;
        let mut advance = false;

        ui.horizontal(|ui| {
            ui.heading(&title);
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
                    .add_enabled_ui(self.can_advance(), |ui| heartsim_ui::primary_button(ui, label))
                    .inner
                    .clicked()
                {
                    advance = true;
                }
            }
            if running {
                ui.spinner();
                heartsim_ui::helper_text(ui, t!("tut-running"));
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
