//! The heartsim case file, `name.hs` (brief B41; the format is documented in `docs/case-file.md`).
//!
//! A case file carries one patient case ([`heartsim_core::case::Case`]) and the rhythms it loads
//! that are not in the built-in library, so a teacher can hand a case to a student as one file. It
//! is a zip archive (deflate) with a short fixed list of entries:
//!
//! | entry | required | content |
//! |---|---|---|
//! | [`MANIFEST_NAME`] | yes, first entry | the [`Manifest`] |
//! | [`CASE_NAME`] | yes | the case, as in `assets/cases/` |
//! | `scenarios/<id>.ron` | no | a bundled rhythm (`Tier0Scenario` file) |
//! | [`README_NAME`] | no | written by the writer for a person who unzips the file; ignored on open |
//!
//! **Untrusted input.** A case file comes from someone else, so [`CasePackage::read`] treats it as
//! hostile: the size of the file, the number of entries and the bytes each entry inflates to are
//! bounded by the `MAX_*` constants and counted while inflating (the sizes in the zip headers are
//! never trusted); only exactly the names above are accepted; nothing is written to disk; text is
//! UTF-8 without NUL and is parsed by `ron` (whose recursion limit, 128, is on). Every refusal is a
//! [`CaseFileError`] variant, never a panic.
//!
//! **Deterministic writer.** [`CasePackage::write`] produces the same bytes for the same package
//! (fixed entry order, the 1980 zip epoch as every timestamp, fixed permissions, deflate), so the
//! SHA-256 of a file ([`sha256_hex`]) identifies it.
//!
//! Pure: no file system, no clock, no GUI; builds for `wasm32-unknown-unknown`.

use heartsim_core::case::Case;
use heartsim_ecg::Tier0Scenario;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::{Cursor, Read, Write as _};

/// Version of the container this build reads and writes. A reader refuses a newer one.
pub const FORMAT_VERSION: u32 = 1;
/// The manifest's `kind`; anything else is not a case file.
pub const KIND: &str = "heartsim-case";
/// File-name extension of a case file (without the dot).
pub const EXTENSION: &str = "hs";
/// Media type of a case file.
pub const MEDIA_TYPE: &str = "application/vnd.heartsim.case+zip";

/// The manifest entry; must be the first entry of the archive.
pub const MANIFEST_NAME: &str = "heartsim-case.ron";
/// The case entry.
pub const CASE_NAME: &str = "case.ron";
/// The optional note for a person who unzips the file; its content is ignored on open.
pub const README_NAME: &str = "README.txt";
/// Folder of the bundled rhythms: `scenarios/<id>.ron`.
pub const SCENARIO_DIR: &str = "scenarios/";

/// Largest case file accepted, bytes (2 MiB).
pub const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
/// Most entries an archive may have.
pub const MAX_ENTRIES: usize = 16;
/// Largest unpacked entry, bytes (1 MiB), counted while inflating.
pub const MAX_ENTRY_BYTES: u64 = 1024 * 1024;
/// Largest total of all unpacked entries, bytes (4 MiB), counted while inflating.
pub const MAX_TOTAL_BYTES: u64 = 4 * 1024 * 1024;
/// Longest id (case and scenario ids alike), characters of `[a-z0-9_]`.
pub const MAX_ID_LEN: usize = 64;
/// Longest manifest text field (title, an author, the licence, the generator), characters.
pub const MAX_FIELD_CHARS: usize = 200;
/// Most authors a manifest may list.
pub const MAX_AUTHORS: usize = 32;
/// Longest language tag, characters (BCP 47 tags in practice are far shorter).
pub const MAX_LANGUAGE_CHARS: usize = 35;
/// Longest `created` date, characters (an ISO date is 10).
pub const MAX_DATE_CHARS: usize = 32;

/// The manifest, `heartsim-case.ron`: what the file is, who wrote it and under which licence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "CaseFile", deny_unknown_fields)]
pub struct Manifest {
    /// Version of the container ([`FORMAT_VERSION`] when written by this build).
    pub format_version: u32,
    /// Always [`KIND`].
    pub kind: String,
    /// Equals the case's id: `[a-z0-9_]{1,64}`.
    pub id: String,
    pub title: String,
    /// BCP 47 language tag of the case text, e.g. `en`.
    pub language: String,
    /// Free text, may be empty.
    #[serde(default)]
    pub authors: Vec<String>,
    /// SPDX id or free text; empty means "not stated".
    #[serde(default)]
    pub licence: String,
    /// ISO date, informational.
    #[serde(default)]
    pub created: String,
    /// The program that wrote the file, e.g. `heartsim 0.27.0`.
    #[serde(default)]
    pub generator: String,
    /// Ids of the bundled scenarios; must equal the `scenarios/<id>.ron` entries exactly.
    #[serde(default)]
    pub scenarios: Vec<String>,
}

/// Only the two fields every version has, read before the strict parse so a newer file gets
/// "newer version" rather than a parse error about a field this build does not know.
#[derive(Deserialize)]
#[serde(rename = "CaseFile")]
struct Probe {
    #[serde(default)]
    format_version: u32,
    #[serde(default)]
    kind: String,
}

impl Manifest {
    /// A manifest for `case` with no bundled scenarios (the writer fills `scenarios`).
    #[must_use]
    pub fn for_case(
        case: &Case,
        authors: Vec<String>,
        licence: &str,
        created: &str,
        generator: &str,
    ) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            kind: KIND.to_owned(),
            id: case.id.clone(),
            title: case.title.clone(),
            language: "en".to_owned(),
            authors,
            licence: licence.to_owned(),
            created: created.to_owned(),
            generator: generator.to_owned(),
            scenarios: Vec::new(),
        }
    }

    /// Pretty RON with struct names, as the writer stores it.
    ///
    /// # Errors
    /// Only if serialisation itself fails.
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default().struct_names(true))
    }

    /// The field rules of a version-1 manifest (kind and version are checked before).
    fn check(&self) -> Result<(), CaseFileError> {
        if !valid_id(&self.id) {
            return Err(CaseFileError::BadId(clip(&self.id)));
        }
        let long = |field: &'static str, text: &str, max: usize| {
            if text.chars().count() > max {
                Err(CaseFileError::FieldTooLong { field, max })
            } else {
                Ok(())
            }
        };
        long("title", &self.title, MAX_FIELD_CHARS)?;
        long("licence", &self.licence, MAX_FIELD_CHARS)?;
        long("generator", &self.generator, MAX_FIELD_CHARS)?;
        long("created", &self.created, MAX_DATE_CHARS)?;
        long("language", &self.language, MAX_LANGUAGE_CHARS)?;
        if self.language.is_empty()
            || !self
                .language
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(CaseFileError::BadLanguage(clip(&self.language)));
        }
        if self.authors.len() > MAX_AUTHORS {
            return Err(CaseFileError::FieldTooLong {
                field: "authors",
                max: MAX_AUTHORS,
            });
        }
        for a in &self.authors {
            long("authors", a, MAX_FIELD_CHARS)?;
        }
        let mut seen = BTreeSet::new();
        for id in &self.scenarios {
            if !valid_id(id) {
                return Err(CaseFileError::BadScenarioId(clip(id)));
            }
            if !seen.insert(id.as_str()) {
                return Err(CaseFileError::ScenarioListedTwice(id.clone()));
            }
        }
        Ok(())
    }
}

/// One case file in memory: the manifest, the case and the bundled scenarios as `(id, RON text)`,
/// sorted by id.
#[derive(Debug, Clone, PartialEq)]
pub struct CasePackage {
    pub manifest: Manifest,
    pub case: Case,
    pub scenarios: Vec<(String, String)>,
}

/// Why a case file was refused. One variant per rule of the reader (see `docs/case-file.md`); the
/// messages are shown to the user as they are.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaseFileError {
    #[error("the file is {size} bytes; a case file may have at most {MAX_FILE_BYTES}")]
    FileTooLarge { size: usize },
    #[error("not a case file (not a readable zip archive): {0}")]
    NotZip(String),
    #[error("the archive has {count} entries; a case file may have at most {MAX_ENTRIES}")]
    TooManyEntries { count: usize },
    #[error("the entry `{name}` unpacks to more than {MAX_ENTRY_BYTES} bytes")]
    EntryTooLarge { name: String },
    #[error("the entries unpack to more than {MAX_TOTAL_BYTES} bytes in all")]
    TotalTooLarge,
    #[error("the entry `{name}` uses a compression method other than stored or deflate")]
    UnsupportedMethod { name: String },
    #[error("the entry `{name}` is encrypted; case files are never encrypted")]
    Encrypted { name: String },
    #[error("the entry name `{name}` is not a safe relative path")]
    UnsafeName { name: String },
    #[error("an entry name appears twice in the archive")]
    DuplicateName,
    #[error("the entry `{name}` is not part of a version-{FORMAT_VERSION} case file")]
    UnknownEntry { name: String },
    #[error("`{0}` is not a valid id (1 to {MAX_ID_LEN} characters of a-z, 0-9 and _)")]
    BadScenarioId(String),
    #[error("the first entry must be `{MANIFEST_NAME}`")]
    ManifestNotFirst,
    #[error("the archive has no `{0}`")]
    MissingEntry(&'static str),
    #[error("the entry `{name}` could not be read: {message}")]
    Corrupt { name: String, message: String },
    #[error("the entry `{name}` is not UTF-8 text")]
    NotUtf8 { name: String },
    #[error("the entry `{name}` contains a NUL character")]
    Nul { name: String },
    #[error("the manifest could not be read: {0}")]
    Manifest(String),
    #[error(
        "this is a case file of version {found}; this heartsim reads version {FORMAT_VERSION}: update heartsim"
    )]
    NewerVersion { found: u32 },
    #[error("not a heartsim case file (kind `{0}`)")]
    WrongKind(String),
    #[error("the case id `{0}` is not valid (1 to {MAX_ID_LEN} characters of a-z, 0-9 and _)")]
    BadId(String),
    #[error("the language tag `{0}` is not valid")]
    BadLanguage(String),
    #[error("the manifest field `{field}` is longer than {max}")]
    FieldTooLong { field: &'static str, max: usize },
    #[error("the manifest names the case `{manifest}` but the case is `{case}`")]
    IdMismatch { manifest: String, case: String },
    #[error("the case could not be read: {0}")]
    Case(String),
    #[error("the manifest lists the rhythm `{0}` twice")]
    ScenarioListedTwice(String),
    #[error("the manifest lists the rhythm `{0}` but the file does not contain it")]
    ScenarioMissing(String),
    #[error("the file contains the rhythm `{0}` but the manifest does not list it")]
    ScenarioNotListed(String),
    #[error("the bundled rhythm `{id}` could not be read: {message}")]
    Scenario { id: String, message: String },
    #[error("the entry `scenarios/{entry}.ron` holds the rhythm `{inner}`")]
    ScenarioIdMismatch { entry: String, inner: String },
    #[error("the bundled rhythm `{0}` has the id of a built-in rhythm; a case file cannot replace one")]
    ScenarioClash(String),
    #[error("the case needs the rhythm {}, which this heartsim does not have", list(.0))]
    NeedsRhythm(Vec<String>),
    #[error("the case file could not be written: {0}")]
    Write(String),
}

fn list(ids: &[String]) -> String {
    ids.iter()
        .map(|i| format!("`{i}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// An id as the format allows it: `[a-z0-9_]{1,64}`.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// A name or id cut for an error message (a hostile file may carry a very long one).
fn clip(s: &str) -> String {
    const MAX: usize = 80;
    if s.chars().count() <= MAX {
        s.to_owned()
    } else {
        format!("{}…", s.chars().take(MAX).collect::<String>())
    }
}

/// Lower-case hex SHA-256 of `bytes` (64 characters): the identity of a case file.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    let digest = sha2::Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// What a name in the archive is.
enum EntryKind {
    Manifest,
    Case,
    Readme,
    Scenario(String),
    /// The `scenarios/` folder entry archive tools add; skipped.
    Folder,
}

/// Check one entry name against the safety rules and the allowed names.
fn classify(name: &str, enclosed: bool, is_dir: bool) -> Result<EntryKind, CaseFileError> {
    let unsafe_name = || CaseFileError::UnsafeName { name: clip(name) };
    if !enclosed
        || name.is_empty()
        || name.starts_with('/')
        || name.contains('\\')
        || name.contains('\0')
        || name.contains(':')
        || name.split('/').any(|c| c == ".." || c == ".")
    {
        return Err(unsafe_name());
    }
    match name {
        MANIFEST_NAME => return Ok(EntryKind::Manifest),
        CASE_NAME => return Ok(EntryKind::Case),
        README_NAME => return Ok(EntryKind::Readme),
        "scenarios/" if is_dir => return Ok(EntryKind::Folder),
        _ => {}
    }
    if let Some(file) = name.strip_prefix(SCENARIO_DIR)
        && let Some(id) = file.strip_suffix(".ron")
        && !is_dir
    {
        if !valid_id(id) {
            return Err(CaseFileError::BadScenarioId(clip(id)));
        }
        return Ok(EntryKind::Scenario(id.to_owned()));
    }
    Err(CaseFileError::UnknownEntry { name: clip(name) })
}

/// Entry bytes as text: UTF-8 without NUL.
fn text(name: &str, bytes: Vec<u8>) -> Result<String, CaseFileError> {
    let s = String::from_utf8(bytes).map_err(|_| CaseFileError::NotUtf8 { name: clip(name) })?;
    if s.contains('\0') {
        return Err(CaseFileError::Nul { name: clip(name) });
    }
    Ok(s)
}

impl CasePackage {
    /// A package of `case` and the bundled scenarios `(id, RON text)`; the manifest's `scenarios`
    /// list is set from them.
    #[must_use]
    pub fn new(mut manifest: Manifest, case: Case, mut scenarios: Vec<(String, String)>) -> Self {
        scenarios.sort_by(|a, b| a.0.cmp(&b.0));
        manifest.scenarios = scenarios.iter().map(|(id, _)| id.clone()).collect();
        Self {
            manifest,
            case,
            scenarios,
        }
    }

    /// Read a case file from untrusted bytes, checking every rule of the format that does not
    /// depend on the running build's rhythm library (see [`Self::resolve`] for those).
    ///
    /// # Errors
    /// The first rule the file breaks.
    pub fn read(bytes: &[u8]) -> Result<Self, CaseFileError> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(CaseFileError::FileTooLarge { size: bytes.len() });
        }
        let mut zip =
            zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| CaseFileError::NotZip(e.to_string()))?;
        if zip.len() > MAX_ENTRIES {
            return Err(CaseFileError::TooManyEntries { count: zip.len() });
        }
        if zip.has_duplicate_names() {
            return Err(CaseFileError::DuplicateName);
        }
        if zip.is_empty() {
            return Err(CaseFileError::MissingEntry(MANIFEST_NAME));
        }
        let mut manifest_text = None;
        let mut case_text = None;
        let mut scenario_texts: Vec<(String, String)> = Vec::new();
        let mut names = BTreeSet::new();
        let mut total = 0u64;
        for i in 0..zip.len() {
            // metadata first, without inflating anything
            let (name, kind) = {
                let entry = zip
                    .by_index_data(i)
                    .map_err(|e| CaseFileError::NotZip(e.to_string()))?;
                let name =
                    String::from_utf8(entry.name_raw().to_vec()).map_err(|_| CaseFileError::UnsafeName {
                        name: String::from_utf8_lossy(entry.name_raw())
                            .chars()
                            .take(80)
                            .collect(),
                    })?;
                let kind = classify(&name, entry.enclosed_name().is_some(), entry.is_dir())?;
                if entry.encrypted() {
                    return Err(CaseFileError::Encrypted { name: clip(&name) });
                }
                let method_ok = matches!(
                    entry.compression(),
                    zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
                );
                if !method_ok {
                    return Err(CaseFileError::UnsupportedMethod { name: clip(&name) });
                }
                (name, kind)
            };
            if !names.insert(name.clone()) {
                return Err(CaseFileError::DuplicateName);
            }
            if matches!(kind, EntryKind::Manifest) != (i == 0) {
                return Err(CaseFileError::ManifestNotFirst);
            }
            if matches!(kind, EntryKind::Folder) {
                continue;
            }
            let mut file = zip.by_index(i).map_err(|e| CaseFileError::Corrupt {
                name: clip(&name),
                message: e.to_string(),
            })?;
            // the header's size is not trusted: count what is actually inflated
            let mut data = Vec::new();
            (&mut file)
                .take(MAX_ENTRY_BYTES + 1)
                .read_to_end(&mut data)
                .map_err(|e| CaseFileError::Corrupt {
                    name: clip(&name),
                    message: e.to_string(),
                })?;
            if data.len() as u64 > MAX_ENTRY_BYTES {
                return Err(CaseFileError::EntryTooLarge { name: clip(&name) });
            }
            total += data.len() as u64;
            if total > MAX_TOTAL_BYTES {
                return Err(CaseFileError::TotalTooLarge);
            }
            match kind {
                EntryKind::Manifest => manifest_text = Some(text(&name, data)?),
                EntryKind::Case => case_text = Some(text(&name, data)?),
                EntryKind::Scenario(id) => scenario_texts.push((id, text(&name, data)?)),
                EntryKind::Readme | EntryKind::Folder => {}
            }
        }
        let manifest_text = manifest_text.ok_or(CaseFileError::MissingEntry(MANIFEST_NAME))?;
        let manifest = parse_manifest(&manifest_text)?;
        let case_text = case_text.ok_or(CaseFileError::MissingEntry(CASE_NAME))?;
        let case = Case::from_ron(&case_text).map_err(|e| CaseFileError::Case(e.to_string()))?;
        if case.id != manifest.id {
            return Err(CaseFileError::IdMismatch {
                manifest: manifest.id,
                case: clip(&case.id),
            });
        }
        for id in &manifest.scenarios {
            if !scenario_texts.iter().any(|(s, _)| s == id) {
                return Err(CaseFileError::ScenarioMissing(id.clone()));
            }
        }
        for (id, text) in &scenario_texts {
            if !manifest.scenarios.contains(id) {
                return Err(CaseFileError::ScenarioNotListed(id.clone()));
            }
            let s = Tier0Scenario::from_ron(text).map_err(|e| CaseFileError::Scenario {
                id: id.clone(),
                message: e.to_string(),
            })?;
            if &s.scenario.id != id {
                return Err(CaseFileError::ScenarioIdMismatch {
                    entry: id.clone(),
                    inner: clip(&s.scenario.id),
                });
            }
        }
        scenario_texts.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Self {
            manifest,
            case,
            scenarios: scenario_texts,
        })
    }

    /// The rules that depend on the running build: no bundled rhythm may have the id of a built-in
    /// one (`is_builtin`), and every rhythm the case loads must be bundled or built in.
    ///
    /// # Errors
    /// [`CaseFileError::ScenarioClash`] or [`CaseFileError::NeedsRhythm`].
    pub fn resolve(&self, is_builtin: impl Fn(&str) -> bool) -> Result<(), CaseFileError> {
        if let Some((id, _)) = self.scenarios.iter().find(|(id, _)| is_builtin(id)) {
            return Err(CaseFileError::ScenarioClash(id.clone()));
        }
        let mut missing: Vec<String> = Vec::new();
        for id in self.case.scenario_ids() {
            let bundled = self.scenarios.iter().any(|(s, _)| s == id);
            if !bundled && !is_builtin(id) && !missing.iter().any(|m| m == id) {
                missing.push(clip(id));
            }
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(CaseFileError::NeedsRhythm(missing))
        }
    }

    /// [`Self::read`] then [`Self::resolve`].
    ///
    /// # Errors
    /// The first rule the file breaks.
    pub fn read_for(bytes: &[u8], is_builtin: impl Fn(&str) -> bool) -> Result<Self, CaseFileError> {
        let p = Self::read(bytes)?;
        p.resolve(is_builtin)?;
        Ok(p)
    }

    /// The bundled scenarios, parsed (they were checked by [`Self::read`]).
    ///
    /// # Errors
    /// Only for a package built in memory with a scenario that does not parse.
    pub fn tier0_scenarios(&self) -> Result<Vec<Tier0Scenario>, CaseFileError> {
        self.scenarios
            .iter()
            .map(|(id, text)| {
                Tier0Scenario::from_ron(text).map_err(|e| CaseFileError::Scenario {
                    id: id.clone(),
                    message: e.to_string(),
                })
            })
            .collect()
    }

    /// Ids of the rhythms the case loads that are not bundled (they must be built in).
    pub fn library_scenario_ids(&self) -> impl Iterator<Item = &str> {
        self.case
            .scenario_ids()
            .filter(|id| !self.scenarios.iter().any(|(s, _)| s == id))
    }

    /// The case file's bytes: deterministic (the same package gives the same bytes). The result is
    /// read back before it is returned, so the writer never makes a file the reader refuses.
    ///
    /// # Errors
    /// When the package breaks a rule of the format (a bad id, an entry too large, a case with
    /// problems), or serialisation fails.
    pub fn write(&self) -> Result<Vec<u8>, CaseFileError> {
        let mut manifest = self.manifest.clone();
        let mut scenarios = self.scenarios.clone();
        scenarios.sort_by(|a, b| a.0.cmp(&b.0));
        manifest.scenarios = scenarios.iter().map(|(id, _)| id.clone()).collect();
        let werr = |e: &dyn std::fmt::Display| CaseFileError::Write(e.to_string());
        let mut entries: Vec<(String, Vec<u8>)> = vec![
            (
                MANIFEST_NAME.to_owned(),
                manifest.to_ron().map_err(|e| werr(&e))?.into_bytes(),
            ),
            (
                CASE_NAME.to_owned(),
                self.case.to_ron().map_err(|e| werr(&e))?.into_bytes(),
            ),
        ];
        for (id, text) in &scenarios {
            entries.push((format!("{SCENARIO_DIR}{id}.ron"), text.clone().into_bytes()));
        }
        entries.push((README_NAME.to_owned(), readme(&manifest, &self.case).into_bytes()));
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o644);
        for (name, bytes) in &entries {
            zip.start_file(name.as_str(), opts).map_err(|e| werr(&e))?;
            zip.write_all(bytes).map_err(|e| werr(&e))?;
        }
        let bytes = zip.finish().map_err(|e| werr(&e))?.into_inner();
        Self::read(&bytes)?;
        Ok(bytes)
    }

    /// The file name the writer suggests: `<id>.hs`.
    #[must_use]
    pub fn file_name(&self) -> String {
        format!("{}.{EXTENSION}", self.manifest.id)
    }
}

fn parse_manifest(text: &str) -> Result<Manifest, CaseFileError> {
    let probe: Probe = ron::from_str(text).map_err(|e| CaseFileError::Manifest(clip(&e.to_string())))?;
    if probe.kind != KIND {
        return Err(CaseFileError::WrongKind(clip(&probe.kind)));
    }
    if probe.format_version > FORMAT_VERSION {
        return Err(CaseFileError::NewerVersion {
            found: probe.format_version,
        });
    }
    let m: Manifest = ron::from_str(text).map_err(|e| CaseFileError::Manifest(clip(&e.to_string())))?;
    if m.format_version == 0 {
        return Err(CaseFileError::Manifest("format_version 0".into()));
    }
    m.check()?;
    Ok(m)
}

/// The note for a person who unzips the file.
fn readme(m: &Manifest, case: &Case) -> String {
    let authors = if m.authors.is_empty() {
        "not stated".to_owned()
    } else {
        m.authors.join(", ")
    };
    let licence = if m.licence.is_empty() {
        "not stated"
    } else {
        &m.licence
    };
    let mut s = format!(
        "heartsim case file\n==================\n\n\
         Title      {title}\n\
         Case id    {id}\n\
         Authors    {authors}\n\
         Licence    {licence}\n\
         Language   {language}\n\
         Stages     {stages}, {questions} questions\n\
         Made by    {generator}\n\n\
         This is a teaching case for the heartsim ECG simulator, a synthetic story, not a real\n\
         person's record. It is not medical advice. Open it in heartsim (Cases page, \"Open case\n\
         file...\", or drop the file on the window).\n\n\
         Files\n-----\n\
         {MANIFEST_NAME:<24} what this file is (format version {version})\n\
         {CASE_NAME:<24} the case: stages, vignettes, questions\n",
        title = m.title,
        id = m.id,
        language = m.language,
        stages = case.stages.len(),
        questions = case.question_count(),
        generator = m.generator,
        version = m.format_version,
    );
    for id in &m.scenarios {
        let name = format!("{SCENARIO_DIR}{id}.ron");
        let _ = writeln!(s, "{name:<24} a rhythm the case loads");
    }
    let _ = writeln!(s, "{README_NAME:<24} this note (ignored when the file is opened)");
    s.push_str("\nThe format is described in docs/case-file.md of the heartsim repository.\n");
    s
}
