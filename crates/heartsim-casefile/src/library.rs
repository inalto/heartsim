//! Online case libraries (brief B43; the format is the "Libraries" chapter of `docs/case-file.md`).
//!
//! A library is a directory served over HTTPS: `<base>/index.ron` ([`CaseIndex`], the catalogue)
//! and one `<base>/<file>.hs` per case. This module is the pure part: the index types, the strict
//! index reader ([`parse_index`]) and the rules for a library address ([`LibraryBase`]), so that a
//! case's URL is always the base joined with a bare file name and never a URL taken from the index.
//! The transport (desktop `ureq`, browser `fetch`) lives in `heartsim-app` and `heartsim-web`.
//!
//! **Untrusted input.** The index comes from a web server. [`parse_index`] refuses an index over
//! [`MAX_INDEX_BYTES`], with more than [`MAX_INDEX_ENTRIES`] entries, of a newer
//! [`INDEX_FORMAT_VERSION`], or with a library title over [`MAX_INDEX_TITLE_CHARS`]; it drops (and
//! counts) every entry that breaks a rule of [`check_entry`]. A downloaded case still goes through
//! [`crate::CasePackage::read`] like a file opened from disk: nothing in the index can relax a
//! limit of the case file.
//!
//! **Trust.** The SHA-256 of an entry protects against a damaged or swapped file *given an honest
//! index*; it does not make the library trustworthy. The index is trusted exactly as much as the
//! HTTPS origin it came from (no signatures in version 1).

use crate::{MAX_AUTHORS, MAX_FIELD_CHARS, MAX_FILE_BYTES, MAX_LANGUAGE_CHARS, valid_id};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Read;

/// Version of the index this build reads and writes; a newer one is refused with a message.
pub const INDEX_FORMAT_VERSION: u32 = 1;
/// File name of the index in a library directory.
pub const INDEX_NAME: &str = "index.ron";
/// Largest index accepted, bytes (256 KiB).
pub const MAX_INDEX_BYTES: usize = 256 * 1024;
/// Most entries an index may list.
pub const MAX_INDEX_ENTRIES: usize = 500;
/// Longest title (of the library or of an entry), characters.
pub const MAX_INDEX_TITLE_CHARS: usize = 200;
/// Longest entry summary, characters.
pub const MAX_SUMMARY_CHARS: usize = 1000;
/// Longest `file` name of an entry, characters.
pub const MAX_FILE_NAME_CHARS: usize = 128;
/// Longest `updated` date or `min_app_version`, characters.
pub const MAX_SHORT_CHARS: usize = 32;
/// Longest library address, characters.
pub const MAX_BASE_CHARS: usize = 2048;

/// The catalogue of a library, `index.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "CaseIndex", deny_unknown_fields)]
pub struct CaseIndex {
    /// [`INDEX_FORMAT_VERSION`] when written by this build.
    pub format_version: u32,
    /// Shown above the list, at most [`MAX_INDEX_TITLE_CHARS`].
    pub title: String,
    /// ISO date of the last change, informational.
    #[serde(default)]
    pub updated: String,
    pub cases: Vec<CaseEntry>,
}

/// One case of a library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "CaseEntry", deny_unknown_fields)]
pub struct CaseEntry {
    /// The case id; the downloaded file's manifest id must equal it.
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    /// BCP 47 tag of the case text.
    pub language: String,
    #[serde(default)]
    pub authors: Vec<String>,
    /// SPDX id or free text; empty means "not stated".
    #[serde(default)]
    pub licence: String,
    /// A bare file name in the library directory ([`valid_file_name`]); never a URL.
    pub file: String,
    /// Exact size of the file; the download is read with this as its cap.
    pub size_bytes: u64,
    /// Lower-case hex SHA-256 of the file (64 characters).
    pub sha256: String,
    /// `heartsim_core::case::CASE_FORMAT_VERSION` of the case inside.
    pub case_format_version: u32,
    /// The oldest heartsim that can open the case, `MAJOR.MINOR.PATCH`, if it matters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_app_version: Option<String>,
}

/// Only the version, read before the strict parse so a newer index gets "newer version" rather
/// than a parse error about a field this build does not know.
#[derive(Deserialize)]
#[serde(rename = "CaseIndex")]
struct Probe {
    #[serde(default)]
    format_version: u32,
}

/// Why an index was refused as a whole. The messages are shown to the user as they are.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IndexError {
    #[error("the library's index is larger than {MAX_INDEX_BYTES} bytes")]
    TooLarge,
    #[error("the library's index is not UTF-8 text")]
    NotUtf8,
    #[error("the library's index could not be read: {0}")]
    Parse(String),
    #[error(
        "the library's index is of version {found}; this heartsim reads version {INDEX_FORMAT_VERSION}: update heartsim"
    )]
    NewerVersion { found: u32 },
    #[error("the library's index lists {count} cases; at most {MAX_INDEX_ENTRIES} are accepted")]
    TooManyEntries { count: usize },
    #[error("the library's title is longer than {MAX_INDEX_TITLE_CHARS} characters")]
    TitleTooLong,
    #[error("the library's index could not be written: {0}")]
    Write(String),
}

/// Why one entry of an index was dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryProblem {
    BadId,
    /// Not a bare `*.hs` file name (a path, `..`, a scheme, odd characters).
    BadFileName,
    /// Not 64 lower-case hex characters.
    BadDigest,
    /// Zero, or more than the case-file limit [`MAX_FILE_BYTES`].
    BadSize,
    /// A title, summary, author, licence, language or version over its bound, or a bad language tag.
    BadText,
    /// `min_app_version` is not `MAJOR.MINOR.PATCH`.
    BadVersion,
    /// A second entry with the id of an earlier one.
    DuplicateId,
}

/// A checked index: the entries that passed [`check_entry`] and how many were dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedIndex {
    pub index: CaseIndex,
    /// Entries dropped ("3 entries ignored").
    pub ignored: usize,
}

/// A file name an entry may give: 1 to [`MAX_FILE_NAME_CHARS`] characters of `[A-Za-z0-9._-]`,
/// not starting with `.`, ending in `.hs`. No `/`, `\`, `:`, `%`, `?` or `#` can occur, so it
/// cannot leave the library directory, name another host or carry a query.
#[must_use]
pub fn valid_file_name(name: &str) -> bool {
    let ext = format!(".{}", crate::EXTENSION);
    !name.is_empty()
        && name.len() <= MAX_FILE_NAME_CHARS
        && !name.starts_with('.')
        && name.len() > ext.len()
        && name.ends_with(&ext)
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && !name.contains("..")
}

/// 64 lower-case hex characters.
#[must_use]
pub fn valid_digest(hex: &str) -> bool {
    hex.len() == 64
        && hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `MAJOR.MINOR.PATCH` as numbers (a pre-release or build suffix is not accepted).
#[must_use]
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let mut it = text.split('.');
    let mut num = || -> Option<u64> {
        let s = it.next()?;
        if s.is_empty() || s.len() > 9 || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        s.parse().ok()
    };
    let v = (num()?, num()?, num()?);
    it.next().is_none().then_some(v)
}

/// The rules every entry must keep; an entry that breaks one is dropped by [`parse_index`].
///
/// # Errors
/// The first rule broken.
pub fn check_entry(e: &CaseEntry) -> Result<(), EntryProblem> {
    if !valid_id(&e.id) {
        return Err(EntryProblem::BadId);
    }
    if !valid_file_name(&e.file) {
        return Err(EntryProblem::BadFileName);
    }
    if !valid_digest(&e.sha256) {
        return Err(EntryProblem::BadDigest);
    }
    if e.size_bytes == 0 || e.size_bytes > MAX_FILE_BYTES as u64 {
        return Err(EntryProblem::BadSize);
    }
    let long = |s: &str, max: usize| s.chars().count() > max;
    let language_ok = !e.language.is_empty()
        && !long(&e.language, MAX_LANGUAGE_CHARS)
        && e.language.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if long(&e.title, MAX_INDEX_TITLE_CHARS)
        || e.title.trim().is_empty()
        || long(&e.summary, MAX_SUMMARY_CHARS)
        || long(&e.licence, MAX_FIELD_CHARS)
        || e.authors.len() > MAX_AUTHORS
        || e.authors.iter().any(|a| long(a, MAX_FIELD_CHARS))
        || !language_ok
    {
        return Err(EntryProblem::BadText);
    }
    if let Some(v) = &e.min_app_version
        && (long(v, MAX_SHORT_CHARS) || parse_version(v).is_none())
    {
        return Err(EntryProblem::BadVersion);
    }
    Ok(())
}

impl CaseEntry {
    /// `true` when this build cannot open the case: its case format is newer than `case_format`
    /// or it asks for a heartsim newer than `app_version` (`MAJOR.MINOR.PATCH`).
    #[must_use]
    pub fn needs_newer(&self, app_version: &str, case_format: u32) -> bool {
        if self.case_format_version > case_format {
            return true;
        }
        match (
            self.min_app_version.as_deref().and_then(parse_version),
            parse_version(app_version),
        ) {
            (Some(min), Some(app)) => min > app,
            _ => false,
        }
    }
}

impl CaseIndex {
    /// Pretty RON with struct names, as `cargo xtask case-library` writes it.
    ///
    /// # Errors
    /// Only if serialisation itself fails.
    pub fn to_ron(&self) -> Result<String, IndexError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default().struct_names(true))
            .map_err(|e| IndexError::Write(e.to_string()))
    }
}

/// Read an index from `reader` through a bounded reader (at most [`MAX_INDEX_BYTES`] + 1 bytes are
/// read) and check it with [`parse_index`].
///
/// # Errors
/// As [`parse_index`]; a read error is a [`IndexError::Parse`].
pub fn read_index(reader: impl Read) -> Result<ParsedIndex, IndexError> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_INDEX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| IndexError::Parse(e.to_string()))?;
    parse_index(&bytes)
}

/// Check an index from untrusted bytes: the size, the version, the number of entries and the
/// library title refuse the whole index; an entry that breaks [`check_entry`], or repeats an
/// earlier id, is dropped and counted.
///
/// # Errors
/// The first rule of the whole index that is broken.
pub fn parse_index(bytes: &[u8]) -> Result<ParsedIndex, IndexError> {
    if bytes.len() > MAX_INDEX_BYTES {
        return Err(IndexError::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| IndexError::NotUtf8)?;
    if text.contains('\0') {
        return Err(IndexError::NotUtf8);
    }
    let clip = |e: &ron::error::SpannedError| e.to_string().chars().take(200).collect::<String>();
    let probe: Probe = ron::from_str(text).map_err(|e| IndexError::Parse(clip(&e)))?;
    if probe.format_version > INDEX_FORMAT_VERSION {
        return Err(IndexError::NewerVersion {
            found: probe.format_version,
        });
    }
    let mut index: CaseIndex = ron::from_str(text).map_err(|e| IndexError::Parse(clip(&e)))?;
    if index.format_version == 0 {
        return Err(IndexError::Parse("format_version 0".into()));
    }
    if index.cases.len() > MAX_INDEX_ENTRIES {
        return Err(IndexError::TooManyEntries {
            count: index.cases.len(),
        });
    }
    if index.title.chars().count() > MAX_INDEX_TITLE_CHARS {
        return Err(IndexError::TitleTooLong);
    }
    if index.updated.chars().count() > MAX_SHORT_CHARS {
        index.updated.clear();
    }
    let before = index.cases.len();
    let mut seen = BTreeSet::new();
    index
        .cases
        .retain(|e| check_entry(e).is_ok() && seen.insert(e.id.clone()));
    Ok(ParsedIndex {
        ignored: before - index.cases.len(),
        index,
    })
}

/// Why a library address was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BaseError {
    #[error("the library address is empty")]
    Empty,
    #[error(
        "a library address must start with https:// (http:// only for localhost or 127.0.0.1, for testing)"
    )]
    NotHttps,
    #[error("the library address is not a plain web address (no user name, query, fragment or `..`)")]
    Malformed,
}

/// The address of a library directory, checked: `https://host[:port]/path/`, or
/// `http://localhost` / `http://127.0.0.1` (with any port) for testing, or, only where the caller
/// allows it (the browser build's own site), a relative path such as `../cases/`. It always ends
/// in `/`, so [`Self::join`] of a [`valid_file_name`] stays in the directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryBase {
    url: String,
    /// `None` for a relative base.
    host: Option<String>,
}

impl LibraryBase {
    /// Check an absolute library address typed by the user.
    ///
    /// # Errors
    /// When it is not an `https` address (or `http` on localhost), or carries anything else than a
    /// host, a port and a path.
    pub fn parse(text: &str) -> Result<Self, BaseError> {
        Self::parse_inner(text, false)
    }

    /// As [`Self::parse`], but a relative path (no scheme, no `//`) is accepted too: the browser
    /// build's default `../cases/`, resolved by the browser against the page's own address.
    ///
    /// # Errors
    /// As [`Self::parse`].
    pub fn parse_allowing_relative(text: &str) -> Result<Self, BaseError> {
        Self::parse_inner(text, true)
    }

    fn parse_inner(text: &str, allow_relative: bool) -> Result<Self, BaseError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(BaseError::Empty);
        }
        if text.len() > MAX_BASE_CHARS
            || text
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '\\' | '?' | '#' | '@'))
        {
            return Err(BaseError::Malformed);
        }
        let lower = text.to_ascii_lowercase();
        let (scheme, rest) = if let Some(r) = lower.strip_prefix("https://") {
            ("https", &text[text.len() - r.len()..])
        } else if let Some(r) = lower.strip_prefix("http://") {
            ("http", &text[text.len() - r.len()..])
        } else if allow_relative && !text.contains(':') && !text.starts_with("//") {
            let path = check_path(text)?;
            return Ok(Self {
                url: path,
                host: None,
            });
        } else {
            return Err(BaseError::NotHttps);
        };
        let (authority, path) = rest.find('/').map_or((rest, "/"), |i| rest.split_at(i));
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) => {
                if p.is_empty()
                    || p.len() > 5
                    || !p.bytes().all(|b| b.is_ascii_digit())
                    || !p.parse::<u32>().is_ok_and(|n| (1..=65_535).contains(&n))
                {
                    return Err(BaseError::Malformed);
                }
                (h, Some(p))
            }
            None => (authority, None),
        };
        let host = host.to_ascii_lowercase();
        let host_ok = !host.is_empty()
            && host.len() <= 253
            && !host.starts_with('.')
            && !host.ends_with('.')
            && !host.contains("..")
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
        if !host_ok {
            return Err(BaseError::Malformed);
        }
        if scheme == "http" && host != "localhost" && host != "127.0.0.1" {
            return Err(BaseError::NotHttps);
        }
        let path = check_path(path)?;
        let port = port.map(|p| format!(":{p}")).unwrap_or_default();
        Ok(Self {
            url: format!("{scheme}://{host}{port}{path}"),
            host: Some(host),
        })
    }

    /// The address, ending in `/`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// The host name (for "from <host>"), `None` for a relative base.
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    /// `true` for `http://localhost` and `http://127.0.0.1`: no redirect is followed then.
    #[must_use]
    pub fn is_plain_http(&self) -> bool {
        self.url.starts_with("http://")
    }

    /// The index's address.
    #[must_use]
    pub fn index_url(&self) -> String {
        format!("{}{INDEX_NAME}", self.url)
    }

    /// The address of `file` in the library: the base plus the bare name, or `None` when the name
    /// is not a [`valid_file_name`]. The result never leaves the base directory.
    #[must_use]
    pub fn join(&self, file: &str) -> Option<String> {
        valid_file_name(file).then(|| format!("{}{file}", self.url))
    }
}

/// A path of `/`-separated segments without `.`, `..` or `%` (no encoded escapes), ending in `/`.
fn check_path(path: &str) -> Result<String, BaseError> {
    if path.contains('%') {
        return Err(BaseError::Malformed);
    }
    let segments: Vec<&str> = path.split('/').collect();
    if segments
        .iter()
        .any(|s| *s == "." || (*s == ".." && !path.starts_with("..")))
    {
        return Err(BaseError::Malformed);
    }
    // a relative base may climb out of the page's directory with leading `../` only
    let mut climbing = true;
    for s in &segments {
        if *s == ".." {
            if !climbing {
                return Err(BaseError::Malformed);
            }
        } else {
            climbing = false;
        }
    }
    let mut p = path.to_owned();
    if !p.ends_with('/') {
        p.push('/');
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_parse_strictly() {
        assert_eq!(parse_version("0.27.0"), Some((0, 27, 0)));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("1.2.3-rc1"), None);
        assert_eq!(parse_version("a.b.c"), None);
    }
}
