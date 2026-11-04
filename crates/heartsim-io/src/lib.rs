//! ECG files in and out (brief B20), shared by the desktop app and `heartsim-cli` so both write
//! byte-identical files for the same scenario run.
//!
//! * [`record`] — [`EcgRecord`], the 12-lead excerpt every writer takes and every reader returns.
//! * [`pdf`] — the 3 × 4 + rhythm-strip printout on A4 / Letter.
//! * [`png`] — page and single-strip raster snapshots (the strip's own CPU rasteriser).
//! * [`csv`], [`wfdb`] — sample data: CSV in mV, WFDB format 16 at 1000 units/mV.
//! * [`scenario_file`] — the running scenario as a loadable RON file.
//! * [`bundle`] — every export plus a README in one deterministic zip, and the file set itself.
//! * [`import`] — reading a CSV, WFDB record or share bundle back for review.
//!
//! Exports carry no patient fields: the header names the scenario, paper speed, gain and date.
//! Every writer is a pure function of its inputs (no clock, no randomness): the date is an input.

pub mod bundle;
pub mod csv;
pub mod import;
pub mod pdf;
pub mod png;
pub mod record;
pub mod scenario_file;
pub mod wfdb;

pub use bundle::{ExportFile, ExportOptions, Formats, export_files, write_files};
pub use import::{Imported, import_bytes, import_path};
pub use pdf::Paper;
pub use record::{EXPORT_RATE_HZ, EcgRecord};

/// Printed on every export.
pub const DISCLAIMER: &str =
    "Simulated ECG for education only - not a patient recording, not for clinical use";

/// What the export header says about the recording. No patient data: a simulation has none.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportInfo {
    pub scenario_id: String,
    pub scenario_title: String,
    /// Paper speed of the printout and PNGs, mm/s.
    pub mm_per_s: f32,
    /// Gain of the printout and PNGs, mm/mV.
    pub mm_per_mv: f32,
    /// `YYYY-MM-DD`, printed in the header (an input so exports are reproducible).
    pub date: String,
    pub paper: Paper,
}

impl ExportInfo {
    /// File-name stem: `heartsim_<scenario id>` with anything but letters, digits, `_` and `-`
    /// replaced (it is also the WFDB record name).
    #[must_use]
    pub fn stem(&self) -> String {
        let id: String = self
            .scenario_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("heartsim_{id}")
    }
}

/// `heartsim <version>`.
#[must_use]
pub fn generator() -> String {
    format!("heartsim {}", env!("CARGO_PKG_VERSION"))
}

/// Why a file could not be imported.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("no samples")]
    Empty,
    #[error("{0}")]
    Format(String),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Midnight UTC of a `YYYY-MM-DD` date, Unix seconds.
#[must_use]
pub fn unix_time_of_date(date: &str) -> Option<i64> {
    let mut it = date.trim().splitn(3, '-').map(str::parse::<i64>);
    let (y, m, d) = (it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?);
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| days_from_civil(y, m, d) * 86_400)
}

/// `YYYY-MM-DD` (UTC) of Unix time `secs`.
#[must_use]
pub fn date_of_unix_time(secs: i64) -> String {
    // inverse of days_from_civil
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Today's date (UTC) from the system clock, for the export header.
#[must_use]
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().cast_signed());
    date_of_unix_time(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        assert_eq!(unix_time_of_date("1970-01-01"), Some(0));
        assert_eq!(unix_time_of_date("2026-11-03"), Some(1_793_664_000));
        for s in [0, 951_782_400, 1_793_664_000, 4_107_542_400] {
            assert_eq!(unix_time_of_date(&date_of_unix_time(s)), Some(s));
        }
        assert_eq!(date_of_unix_time(951_782_400), "2000-02-29");
        assert_eq!(unix_time_of_date("2026-13-01"), None);
        assert_eq!(unix_time_of_date("today"), None);
    }

    #[test]
    fn stem_is_a_valid_record_name() {
        let info = ExportInfo {
            scenario_id: "avb 2nd/mobitz-1".into(),
            scenario_title: String::new(),
            mm_per_s: 25.0,
            mm_per_mv: 10.0,
            date: "2026-01-01".into(),
            paper: Paper::A4,
        };
        assert_eq!(info.stem(), "heartsim_avb_2nd_mobitz-1");
    }
}
