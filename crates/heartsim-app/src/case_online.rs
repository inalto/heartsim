//! The Online section of the Cases page (brief B43): browse a case library on the web and import
//! one of its cases into My cases. The library format and its rules are in
//! [`heartsim_casefile::library`]; the transport is a [`CaseFetch`]; finishing an import (the
//! checks against the index entry and `Cases::open_bytes`) is
//! [`crate::case_ui::Cases::poll_online`].
//!
//! Nothing is requested until the user presses "Browse the online library" or "Import"; one
//! request runs at a time and can be cancelled.

use crate::case_fetch::{CaseFetch, FetchError, FetchPoll, NoFetch};
use crate::case_ui::{OpenError, UserCase};
use eframe::egui;
use heartsim_casefile::library::{
    CaseEntry, CaseIndex, IndexError, LibraryBase, MAX_INDEX_BYTES, parse_index,
};
use heartsim_casefile::{CaseFileError, CasePackage, sha256_hex};
use heartsim_ui::{cards, t};

/// The official library on the desktop: the project's GitHub Pages, next to the guide (`/`) and
/// the browser app (`/app/`); `.github/workflows/docs.yml` writes it with `cargo xtask
/// case-library`.
pub const DEFAULT_LIBRARY_URL: &str = "https://inalto.github.io/heartsim/cases/";
/// The official library in the browser build, which is served from `…/heartsim/app/`: the same
/// directory, relative, so the request stays on the page's own origin.
pub const WEB_DEFAULT_LIBRARY: &str = "../cases/";
/// The version this build is, compared with an entry's `min_app_version`.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Why the list of a library could not be shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexFailure {
    Fetch(FetchError),
    Index(IndexError),
}

/// Why a case of a library was not imported. Each has its own message; none adds anything to My
/// cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    Fetch(FetchError),
    /// The body does not have the listed number of bytes (`got: None`: more than listed).
    WrongLength {
        listed: u64,
        got: Option<usize>,
    },
    /// The body's SHA-256 is not the listed one.
    WrongDigest,
    /// The case-file reader refused the file.
    File(CaseFileError),
    /// The file holds another case than the entry names.
    IdMismatch {
        listed: String,
        found: String,
    },
    /// My cases has a different file with this id, and the user did not ask for an update.
    DifferentCase {
        title: String,
    },
    /// `Cases::open_bytes` refused it (a rhythm this build lacks, a rhythm id in use).
    Open(OpenError),
}

/// Where an entry stands against My cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryState {
    /// Not in My cases.
    New,
    /// The same file (same SHA-256) is in My cases.
    Imported,
    /// My cases has a case with this id from a different file: offered as an update.
    Update,
    /// The case needs a newer heartsim (`case_format_version` or `min_app_version`).
    NeedsNewer,
}

/// A library's list as it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// Where it came from: imports use this base, whatever the address field says meanwhile.
    pub base: LibraryBase,
    pub index: CaseIndex,
    /// Entries dropped by the reader.
    pub ignored: usize,
}

#[derive(Debug, Clone)]
enum Request {
    Index(LibraryBase),
    Case {
        base: LibraryBase,
        entry: Box<CaseEntry>,
        update: bool,
    },
}

/// A downloaded case, handed to `Cases::poll_online` to be checked and opened.
#[derive(Debug, Clone)]
pub(crate) struct Arrived {
    pub base: LibraryBase,
    pub entry: CaseEntry,
    pub update: bool,
    pub result: Result<Vec<u8>, FetchError>,
}

/// What a click in the Online section asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OnlineAction {
    Browse,
    Cancel,
    Import(String),
    AskUpdate(String),
    Update(String),
    KeepMine,
}

/// The Online section's state: the transport, the library address, the request running, the
/// last list read and the last message.
pub struct OnlineLibrary {
    fetch: Box<dyn CaseFetch>,
    /// The library used when the address field is empty; `None` when this build has none.
    default_base: Option<String>,
    address: String,
    address_changed: bool,
    request: Option<Request>,
    listing: Option<Listing>,
    message: Option<(bool, String)>,
    index_error: Option<IndexFailure>,
    import_error: Option<ImportError>,
    confirm_update: Option<String>,
}

impl std::fmt::Debug for OnlineLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnlineLibrary")
            .field("address", &self.address)
            .field("busy", &self.request.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for OnlineLibrary {
    fn default() -> Self {
        Self::new(Box::new(NoFetch), None)
    }
}

impl OnlineLibrary {
    /// A library section with `fetch` as transport and `default_base` as the library of an empty
    /// address field. Nothing is requested.
    #[must_use]
    pub fn new(fetch: Box<dyn CaseFetch>, default_base: Option<&str>) -> Self {
        Self {
            fetch,
            default_base: default_base.map(str::to_owned),
            address: String::new(),
            address_changed: false,
            request: None,
            listing: None,
            message: None,
            index_error: None,
            import_error: None,
            confirm_update: None,
        }
    }

    /// The desktop's: `ureq` with the `online-cases` feature and [`DEFAULT_LIBRARY_URL`]; none
    /// otherwise (the browser build injects its transport later).
    #[must_use]
    pub fn platform() -> Self {
        #[cfg(feature = "online-cases")]
        {
            Self::new(
                Box::new(crate::case_fetch::UreqFetch::default()),
                Some(DEFAULT_LIBRARY_URL),
            )
        }
        #[cfg(not(feature = "online-cases"))]
        {
            Self::default()
        }
    }

    /// `true` when there is a transport and a library to ask.
    #[must_use]
    pub fn available(&self) -> bool {
        self.fetch.available() && (self.default_base.is_some() || !self.address.trim().is_empty())
    }

    /// The text of the "Library address" field (empty: the default library).
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Set the address field (from the settings at start).
    pub fn set_address(&mut self, address: &str) {
        address.clone_into(&mut self.address);
    }

    /// The address, when the user changed it since the last call (for the settings).
    pub fn take_address_change(&mut self) -> Option<String> {
        std::mem::take(&mut self.address_changed).then(|| self.address.clone())
    }

    /// The library the address field names, or the default one when it is empty.
    ///
    /// # Errors
    /// When the address is not an https one (or http on localhost), in words.
    pub fn base(&self) -> Result<LibraryBase, String> {
        let typed = self.address.trim();
        if typed.is_empty() {
            let d = self
                .default_base
                .as_deref()
                .ok_or_else(|| t!("online-unavailable"))?;
            LibraryBase::parse_allowing_relative(d).map_err(|e| e.to_string())
        } else {
            LibraryBase::parse(typed).map_err(|e| e.to_string())
        }
    }

    /// `true` while a request runs.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.request.is_some()
    }

    /// The last list read.
    #[must_use]
    pub fn listing(&self) -> Option<&Listing> {
        self.listing.as_ref()
    }

    /// The last message of the section, and whether it reports a success.
    #[must_use]
    pub fn message(&self) -> Option<(bool, &str)> {
        self.message.as_ref().map(|(ok, s)| (*ok, s.as_str()))
    }

    /// Why the last list could not be read, if it could not.
    #[must_use]
    pub fn index_error(&self) -> Option<&IndexFailure> {
        self.index_error.as_ref()
    }

    /// Why the last import failed, if it did.
    #[must_use]
    pub fn import_error(&self) -> Option<&ImportError> {
        self.import_error.as_ref()
    }

    /// Ask the library for its list ("Browse the online library").
    pub fn browse(&mut self) {
        if self.busy() || !self.fetch.available() {
            return;
        }
        self.index_error = None;
        self.import_error = None;
        self.confirm_update = None;
        match self.base() {
            Ok(base) => {
                self.fetch.start(&base.index_url(), MAX_INDEX_BYTES);
                self.message = None;
                self.request = Some(Request::Index(base));
            }
            Err(e) => self.message = Some((false, e)),
        }
    }

    /// Download entry `id` of the list. `update`: the user confirmed replacing the case of My
    /// cases with the same id.
    pub fn import(&mut self, id: &str, update: bool) {
        if self.busy() || !self.fetch.available() {
            return;
        }
        self.confirm_update = None;
        self.import_error = None;
        let Some(l) = &self.listing else { return };
        let Some(entry) = l.index.cases.iter().find(|e| e.id == id).cloned() else {
            return;
        };
        // the URL is the base and the bare file name, never a URL from the index
        let Some(url) = l.base.join(&entry.file) else {
            return;
        };
        let cap = usize::try_from(entry.size_bytes).unwrap_or(usize::MAX);
        self.fetch.start(&url, cap);
        self.message = None;
        self.request = Some(Request::Case {
            base: l.base.clone(),
            entry: Box::new(entry),
            update,
        });
    }

    /// Abandon the running request; nothing is added.
    pub fn cancel(&mut self) {
        if self.request.take().is_some() {
            self.fetch.cancel();
            self.message = Some((true, t!("online-cancelled")));
        }
    }

    /// Take the transport's answer: a list is read here; a downloaded case is returned for
    /// `Cases::poll_online`.
    pub(crate) fn poll(&mut self) -> Option<Arrived> {
        let polled = self.fetch.poll();
        let result = match polled {
            FetchPoll::Idle | FetchPoll::Pending => return None,
            FetchPoll::Done(b) => Ok(b),
            FetchPoll::Failed(e) => Err(e),
        };
        // an answer that arrives after Cancel has no request any more
        match self.request.take()? {
            Request::Index(base) => {
                self.read_index(base, result);
                None
            }
            Request::Case { base, entry, update } => Some(Arrived {
                base,
                entry: *entry,
                update,
                result,
            }),
        }
    }

    fn read_index(&mut self, base: LibraryBase, result: Result<Vec<u8>, FetchError>) {
        let parsed = result
            .map_err(IndexFailure::Fetch)
            .and_then(|b| parse_index(&b).map_err(IndexFailure::Index));
        match parsed {
            Ok(p) => {
                self.index_error = None;
                self.message = None;
                self.listing = Some(Listing {
                    base,
                    index: p.index,
                    ignored: p.ignored,
                });
            }
            Err(e) => {
                self.message = Some((false, t!("online-index-failed", error = e.to_string())));
                self.index_error = Some(e);
            }
        }
    }

    /// Record the end of an import (from `Cases::poll_online`).
    pub(crate) fn finish_import(&mut self, title: &str, result: Result<String, ImportError>) {
        match result {
            Ok(text) => {
                self.import_error = None;
                self.message = Some((true, text));
            }
            Err(e) => {
                self.message = Some((
                    false,
                    t!("online-import-failed", title = title, error = e.to_string()),
                ));
                self.import_error = Some(e);
            }
        }
    }

    /// Where `entry` stands against My cases (`user`).
    #[must_use]
    pub fn entry_state(entry: &CaseEntry, user: &[UserCase]) -> EntryState {
        if entry.needs_newer(APP_VERSION, heartsim_core::case::CASE_FORMAT_VERSION) {
            return EntryState::NeedsNewer;
        }
        match user.iter().find(|u| u.package.manifest.id == entry.id) {
            None => EntryState::New,
            Some(u) if u.sha256 == entry.sha256 => EntryState::Imported,
            Some(_) => EntryState::Update,
        }
    }

    /// The "from <host>" of a case imported from `base`.
    #[must_use]
    pub fn provenance(base: &LibraryBase) -> String {
        match base.host() {
            Some(h) => t!("online-from", host = h),
            None => t!("online-from-site"),
        }
    }

    /// The Online section. `user`: My cases, for each entry's state.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, user: &[UserCase]) -> Option<OnlineAction> {
        let mut action = None;
        let busy = self.busy();
        let available = self.available();
        heartsim_ui::helper_text(ui, t!("online-intro"));
        if !self.fetch.available() {
            heartsim_ui::helper_text(ui, t!("online-unavailable"));
            return None;
        }
        heartsim_ui::helper_text(ui, t!("online-trust"));
        ui.horizontal_wrapped(|ui| {
            ui.label(t!("online-address"));
            let default = self.default_base.clone().unwrap_or_default();
            let r = ui
                .add_enabled(
                    !busy,
                    egui::TextEdit::singleline(&mut self.address)
                        .hint_text(default.as_str())
                        .desired_width(320.0),
                )
                .on_hover_text(t!("online-address-hint", default = default.as_str()));
            if r.changed() {
                self.address_changed = true;
            }
        });
        ui.horizontal_wrapped(|ui| {
            let browse = ui
                .add_enabled(!busy && available, egui::Button::new(t!("online-browse")))
                .on_hover_text(t!("online-browse-hint"));
            if browse.clicked() {
                action = Some(OnlineAction::Browse);
            }
            if let Some(req) = &self.request {
                ui.spinner();
                let text = match req {
                    Request::Index(_) => t!("online-loading-index"),
                    Request::Case { entry, .. } => t!("online-loading-case", title = entry.title.as_str()),
                };
                heartsim_ui::helper_text(ui, text);
                if ui.button(t!("online-cancel")).clicked() {
                    action = Some(OnlineAction::Cancel);
                }
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(100));
            }
        });
        if let Some((ok, text)) = &self.message {
            if *ok {
                cards::result_box(ui, true, text.as_str());
            } else {
                ui.colored_label(ui.visuals().error_fg_color, text.as_str());
            }
        }
        let Some(listing) = &self.listing else {
            return action;
        };
        ui.add_space(4.0);
        let mut head = t!(
            "online-listing",
            title = listing.index.title.as_str(),
            count = listing.index.cases.len().to_string()
        );
        if !listing.index.updated.is_empty() {
            head = format!(
                "{head} · {}",
                t!("online-updated", date = listing.index.updated.as_str())
            );
        }
        ui.strong(head);
        heartsim_ui::helper_text(ui, listing.base.as_str());
        if listing.ignored > 0 {
            heartsim_ui::helper_text(ui, t!("online-ignored", count = listing.ignored.to_string()));
        }
        if listing.index.cases.is_empty() {
            ui.label(t!("online-empty"));
        }
        for e in &listing.index.cases {
            let state = Self::entry_state(e, user);
            ui.group(|ui| {
                ui.strong(&e.title);
                if !e.summary.is_empty() {
                    ui.label(&e.summary);
                }
                let not_stated = t!("cases-not-stated");
                let authors = if e.authors.is_empty() {
                    not_stated.clone()
                } else {
                    e.authors.join(", ")
                };
                let licence = if e.licence.trim().is_empty() {
                    not_stated
                } else {
                    e.licence.clone()
                };
                heartsim_ui::helper_text(
                    ui,
                    t!(
                        "online-entry-details",
                        language = e.language.as_str(),
                        authors = authors,
                        licence = licence,
                        size = e.size_bytes.div_ceil(1024).to_string()
                    ),
                );
                ui.horizontal_wrapped(|ui| match state {
                    EntryState::New => {
                        let b = ui
                            .add_enabled(!busy, egui::Button::new(t!("online-import")))
                            .on_hover_text(t!("online-import-hint", title = e.title.as_str()));
                        if b.clicked() {
                            action = Some(OnlineAction::Import(e.id.clone()));
                        }
                    }
                    EntryState::Imported => {
                        ui.add_enabled(false, egui::Button::new(t!("online-imported")))
                            .on_disabled_hover_text(t!("online-imported-hint"));
                    }
                    EntryState::NeedsNewer => {
                        ui.add_enabled(false, egui::Button::new(t!("online-needs-newer")));
                    }
                    EntryState::Update => {
                        if self.confirm_update.as_deref() == Some(e.id.as_str()) {
                            ui.label(t!("online-update-confirm", title = e.title.as_str()));
                            if ui
                                .add_enabled(!busy, egui::Button::new(t!("online-update-yes")))
                                .clicked()
                            {
                                action = Some(OnlineAction::Update(e.id.clone()));
                            }
                            if ui.button(t!("online-update-no")).clicked() {
                                action = Some(OnlineAction::KeepMine);
                            }
                        } else {
                            let b = ui
                                .add_enabled(!busy, egui::Button::new(t!("online-update")))
                                .on_hover_text(t!("online-update-hint"));
                            if b.clicked() {
                                action = Some(OnlineAction::AskUpdate(e.id.clone()));
                            }
                        }
                    }
                });
            });
        }
        action
    }

    /// Carry out what [`Self::ui`] asked for.
    pub(crate) fn act(&mut self, action: OnlineAction) {
        match action {
            OnlineAction::Browse => self.browse(),
            OnlineAction::Cancel => self.cancel(),
            OnlineAction::Import(id) => self.import(&id, false),
            OnlineAction::AskUpdate(id) => self.confirm_update = Some(id),
            OnlineAction::Update(id) => self.import(&id, true),
            OnlineAction::KeepMine => self.confirm_update = None,
        }
    }
}

/// The checks of a downloaded case against its entry, in order: the exact length, the SHA-256,
/// the case-file reader, the id.
///
/// # Errors
/// The first check that fails.
pub fn verify_download(entry: &CaseEntry, bytes: &[u8]) -> Result<CasePackage, ImportError> {
    if bytes.len() as u64 != entry.size_bytes {
        return Err(ImportError::WrongLength {
            listed: entry.size_bytes,
            got: Some(bytes.len()),
        });
    }
    if sha256_hex(bytes) != entry.sha256 {
        return Err(ImportError::WrongDigest);
    }
    let p = CasePackage::read(bytes).map_err(ImportError::File)?;
    if p.manifest.id != entry.id {
        return Err(ImportError::IdMismatch {
            listed: entry.id.clone(),
            found: p.manifest.id,
        });
    }
    Ok(p)
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Status(s) => t!("online-err-status", status = s.to_string()),
            Self::TooLarge { cap } => t!("online-err-too-large", cap = cap.to_string()),
            Self::Blocked { cross_origin: true } => t!("online-err-blocked-cors"),
            Self::Blocked { cross_origin: false } => t!("online-err-blocked"),
            Self::Refused(url) => t!("online-err-refused", url = url.as_str()),
            Self::Network(e) => e.clone(),
        };
        f.write_str(&text)
    }
}

impl std::fmt::Display for IndexFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fetch(e) => write!(f, "{e}"),
            Self::Index(e) => write!(f, "{e}"),
        }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Fetch(e) => e.to_string(),
            Self::WrongLength { listed, got: Some(n) } => t!(
                "online-err-wrong-length",
                got = n.to_string(),
                listed = listed.to_string()
            ),
            Self::WrongLength { listed, got: None } => {
                t!("online-err-longer", listed = listed.to_string())
            }
            Self::WrongDigest => t!("online-err-digest"),
            Self::File(e) => e.to_string(),
            Self::IdMismatch { listed, found } => {
                t!("online-err-id", listed = listed.as_str(), found = found.as_str())
            }
            Self::DifferentCase { title } => t!("online-err-different", title = title.as_str()),
            Self::Open(e) => e.to_string(),
        };
        f.write_str(&text)
    }
}
