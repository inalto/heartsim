//! The export file set and the "share bundle": every export plus a README in one zip a teacher
//! can hand out. The set is built in memory by [`export_files`], the one function both the app's
//! Export window and `heartsim-cli run` call, then written by [`write_files`].
//!
//! The zip is deterministic: fixed entry order, the zip epoch (1980-01-01) as every timestamp,
//! fixed permissions, deflate.

use crate::record::EcgRecord;
use crate::{ExportInfo, csv, pdf, png, scenario_file, wfdb};
use heartsim_core::Lead;
use heartsim_ecg::Tier0Scenario;
use heartsim_ui::LeadOrder;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

/// Which files to write: independent switches, one per format.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Formats {
    pub pdf: bool,
    /// 12-lead page and rhythm-strip PNGs.
    pub png: bool,
    pub csv: bool,
    /// `.hea` + `.dat`.
    pub wfdb: bool,
    /// The scenario file.
    pub ron: bool,
    /// A zip of everything above plus a README.
    pub bundle: bool,
}

impl Default for Formats {
    fn default() -> Self {
        Self {
            pdf: true,
            png: true,
            csv: true,
            wfdb: true,
            ron: true,
            bundle: true,
        }
    }
}

/// Export settings that are not part of the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    pub formats: Formats,
    /// Limb-lead order of the PDF and page PNG.
    pub order: LeadOrder,
    /// Lead of the strip PNG.
    pub strip_lead: Lead,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            formats: Formats::default(),
            order: LeadOrder::Standard,
            strip_lead: Lead::II,
        }
    }
}

/// One output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Why an export failed.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("PNG: {0}")]
    Image(#[from] image::ImageError),
    #[error("scenario: {0}")]
    Ron(#[from] ron::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("nothing to export: the strip is empty")]
    Empty,
}

fn readme(rec: &EcgRecord, info: &ExportInfo, files: &[ExportFile]) -> String {
    let stem = info.stem();
    let mut s = format!(
        "heartsim share bundle\n=====================\n\n\
         {disclaimer}.\n\n\
         Scenario   {title} ({id})\n\
         Excerpt    {dur:.1} s, simulation time {t0:.2}-{t1:.2} s, {fs:.0} Hz, 12 leads (I II III aVR aVL aVF V1-V6)\n\
         Paper      {speed} mm/s, {gain} mm/mV, {paper}\n\
         Date       {date}\n\
         Made by    {generator}\n\nFiles\n-----\n",
        disclaimer = crate::DISCLAIMER,
        title = info.scenario_title,
        id = info.scenario_id,
        dur = rec.duration_s(),
        t0 = rec.start_ms / 1000.0,
        t1 = rec.start_ms / 1000.0 + rec.duration_s(),
        fs = rec.sample_rate_hz,
        speed = info.mm_per_s,
        gain = info.mm_per_mv,
        paper = info.paper.label(),
        date = info.date,
        generator = crate::generator(),
    );
    for f in files {
        let what = match f.name.rsplit('.').next().unwrap_or("") {
            "pdf" => "12-lead printout (3 x 4 + lead II rhythm strip)",
            "png" if f.name.ends_with("-12lead.png") => "12-lead page image",
            "png" => "rhythm strip image",
            "csv" => "samples in mV: t_ms,I,...,V6",
            "hea" => "WFDB header (PhysioNet), open with wfdb.rdrecord",
            "dat" => "WFDB samples, format 16, 1000 units/mV",
            "ron" => "scenario file: drop it on heartsim or run `heartsim-cli run` on it",
            _ => "",
        };
        let _ = writeln!(s, "{:<34} {what}", f.name);
    }
    let _ = write!(
        s,
        "\nPython:  import wfdb; r = wfdb.rdrecord('{stem}'); wfdb.plot_wfdb(r)\n\
         Review:  drop the .hea, .csv or this zip on the heartsim window and use the calipers on the\n         \
         strip or the 12-lead page. Drop the .ron to run the scenario.\n"
    );
    s
}

fn zip_files(files: &[ExportFile]) -> Result<Vec<u8>, ExportError> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for f in files {
        zip.start_file(f.name.as_str(), opts)?;
        zip.write_all(&f.bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}

/// Every file `opts.formats` asks for, in a fixed order: PDF, page PNG, strip PNG, CSV, WFDB
/// header and data, scenario, bundle. `rec` is the excerpt (already cut to the export window);
/// the PDF and PNGs show its last page width.
pub fn export_files(
    rec: &EcgRecord,
    info: &ExportInfo,
    scenario: &Tier0Scenario,
    opts: &ExportOptions,
) -> Result<Vec<ExportFile>, ExportError> {
    if rec.is_empty() {
        return Err(ExportError::Empty);
    }
    let stem = info.stem();
    let f = opts.formats;
    let mut files = Vec::new();
    let mut add = |name: String, bytes: Vec<u8>| files.push(ExportFile { name, bytes });
    if f.pdf {
        add(format!("{stem}.pdf"), pdf::write_pdf(rec, info, opts.order));
    }
    if f.png {
        add(
            format!("{stem}-12lead.png"),
            png::page_png(rec, info, opts.order)?,
        );
        add(
            format!("{stem}-{}.png", opts.strip_lead.label()),
            png::strip_png(rec, info, opts.strip_lead)?,
        );
    }
    if f.csv {
        add(format!("{stem}.csv"), csv::write_csv(rec).into_bytes());
    }
    if f.wfdb {
        let comments = [
            format!("{} - simulated ECG, not a patient recording", crate::generator()),
            format!("scenario: {} - {}", info.scenario_id, info.scenario_title),
            format!("start_ms: {}", rec.start_ms),
            format!("date: {}", info.date),
        ];
        let w = wfdb::write_wfdb(rec, &stem, &comments);
        add(format!("{stem}.hea"), w.header.into_bytes());
        add(format!("{stem}.dat"), w.data);
    }
    if f.ron {
        add(
            format!("{stem}.ron"),
            scenario_file::scenario_ron(scenario)?.into_bytes(),
        );
    }
    if f.bundle {
        let mut inner = files.clone();
        inner.insert(
            0,
            ExportFile {
                name: "README.txt".into(),
                bytes: readme(rec, info, &files).into_bytes(),
            },
        );
        files.push(ExportFile {
            name: format!("{stem}.zip"),
            bytes: zip_files(&inner)?,
        });
    }
    Ok(files)
}

/// Write `files` into `dir` (created if needed). Returns the paths written.
pub fn write_files(dir: &Path, files: &[ExportFile]) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    files
        .iter()
        .map(|f| {
            let p = dir.join(&f.name);
            std::fs::write(&p, &f.bytes).map(|()| p)
        })
        .collect()
}
