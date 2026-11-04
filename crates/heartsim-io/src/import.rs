//! Loading a recording for review: a CSV, a WFDB record (`.hea`, or its `.dat`), or a share
//! bundle zip (its WFDB record, else its CSV). The result goes into the strip rings in place of the
//! live simulation, where the calipers and the interval read-out work as usual.

use crate::record::EcgRecord;
use crate::{ImportError, csv, wfdb};
use std::io::Read as _;
use std::path::Path;

/// An imported recording and what to call it on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub record: EcgRecord,
    /// File name, plus the scenario when the file names one (heartsim exports do).
    pub label: String,
}

fn label_of(file: &str, header_comments: &[String]) -> String {
    header_comments
        .iter()
        .find_map(|c| c.strip_prefix("scenario:"))
        .map_or_else(|| file.to_string(), |s| format!("{file} ({})", s.trim()))
}

fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// Import a file. For a WFDB record either the `.hea` or the `.dat` may be given; the other one
/// (and any further data files) are read from the same directory.
pub fn import_path(path: &Path) -> Result<Imported, ImportError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = path.parent().unwrap_or(Path::new("."));
    let path = if extension(&name) == "dat" {
        path.with_extension("hea")
    } else {
        path.to_path_buf()
    };
    let bytes = std::fs::read(&path)?;
    let display = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or(name);
    import_bytes(&display, &bytes, |f| Ok(std::fs::read(dir.join(f))?))
}

/// Import from memory: `name` picks the format by extension (`.csv`, `.hea`, `.zip`);
/// `sibling(file)` supplies the WFDB data files a header names.
pub fn import_bytes(
    name: &str,
    bytes: &[u8],
    sibling: impl FnMut(&str) -> Result<Vec<u8>, ImportError>,
) -> Result<Imported, ImportError> {
    let text = || String::from_utf8_lossy(bytes).into_owned();
    match extension(name).as_str() {
        "csv" | "txt" => Ok(Imported {
            record: csv::read_csv(&text())?,
            label: name.to_string(),
        }),
        "hea" => {
            let header = text();
            Ok(Imported {
                record: wfdb::read_wfdb(&header, sibling)?,
                label: label_of(name, &wfdb::header_comments(&header)),
            })
        }
        "zip" => import_zip(name, bytes),
        other => Err(ImportError::Format(format!(
            "cannot import `.{other}` files (use .csv, .hea/.dat or a heartsim .zip bundle)"
        ))),
    }
}

fn import_zip(name: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut read = |entry: &str| -> Result<Vec<u8>, ImportError> {
        let mut f = zip.by_name(entry)?;
        let mut out = Vec::new();
        f.read_to_end(&mut out)?;
        Ok(out)
    };
    let names: Vec<String> = {
        let z = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        let mut n = z
            .file_names()
            .map(|name| name.map(std::borrow::Cow::into_owned))
            .collect::<Result<Vec<String>, _>>()?;
        n.sort();
        n
    };
    let pick = |ext: &str| names.iter().find(|n| extension(n) == ext).cloned();
    if let Some(hea) = pick("hea") {
        let header = String::from_utf8_lossy(&read(&hea)?).into_owned();
        let dir = hea.rsplit_once('/').map_or("", |(d, _)| d).to_string();
        let record = wfdb::read_wfdb(&header, |f| {
            read(&if dir.is_empty() {
                f.to_string()
            } else {
                format!("{dir}/{f}")
            })
        })?;
        return Ok(Imported {
            record,
            label: label_of(name, &wfdb::header_comments(&header)),
        });
    }
    if let Some(c) = pick("csv") {
        let text = String::from_utf8_lossy(&read(&c)?).into_owned();
        return Ok(Imported {
            record: csv::read_csv(&text)?,
            label: format!("{name} ({c})"),
        });
    }
    Err(ImportError::Format(format!(
        "{name} holds no .hea or .csv recording"
    )))
}
