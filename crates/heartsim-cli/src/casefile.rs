//! `heartsim-cli case pack | unpack | check` (brief B41): make, take apart and check `.hs` case
//! files with the same reader and writer as the application (`heartsim-casefile`; the format is in
//! `docs/case-file.md`).

use crate::content::{assets_root, library_ids, run_package};
use anyhow::{Context, Result, bail};
use heartsim_casefile::{CASE_NAME, CasePackage, MANIFEST_NAME, MAX_FILE_BYTES, Manifest, SCENARIO_DIR};
use heartsim_core::case::Case;
use heartsim_ecg::Tier0Scenario;
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// What `case pack` writes into the manifest.
#[derive(Debug, Clone)]
pub(crate) struct PackArgs {
    pub case: PathBuf,
    pub scenarios: Vec<PathBuf>,
    pub out: PathBuf,
    pub authors: Vec<String>,
    pub licence: String,
    pub language: String,
    /// `None`: today.
    pub created: Option<String>,
    pub assets: Option<PathBuf>,
}

fn read_text(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| path.display().to_string())
}

/// A case file from disk, read without trusting its size.
fn read_file(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .with_context(|| path.display().to_string())?
        .take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .with_context(|| path.display().to_string())?;
    Ok(bytes)
}

/// `case pack <case.ron> [--scenario <file>]… -o <name.hs>`.
pub(crate) fn pack(a: &PackArgs) -> Result<()> {
    let case =
        Case::from_ron(&read_text(&a.case)?).map_err(|e| anyhow::anyhow!("{}: {e}", a.case.display()))?;
    let mut scenarios = Vec::new();
    for path in &a.scenarios {
        let text = read_text(path)?;
        let s = Tier0Scenario::from_ron(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        scenarios.push((s.scenario.id.clone(), text));
    }
    let mut manifest = Manifest::for_case(
        &case,
        a.authors.clone(),
        &a.licence,
        &a.created.clone().unwrap_or_else(heartsim_io::today),
        &heartsim_io::generator(),
    );
    manifest.language.clone_from(&a.language);
    let p = CasePackage::new(manifest, case, scenarios);
    let assets = assets_root(a.assets.as_deref())?;
    let ids = library_ids(&assets)?;
    p.resolve(|id| ids.iter().any(|i| i == id))?;
    let bytes = p.write()?;
    if let Some(dir) = a.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&a.out, &bytes).with_context(|| a.out.display().to_string())?;
    println!(
        "{}: {} ({} stages, {} questions, {} bundled rhythms), {} bytes, sha256 {}",
        a.out.display(),
        p.case.id,
        p.case.stages.len(),
        p.case.question_count(),
        p.scenarios.len(),
        bytes.len(),
        heartsim_casefile::sha256_hex(&bytes)
    );
    Ok(())
}

/// `case unpack <name.hs> -o <dir>`: the manifest, the case and the bundled rhythms, under the
/// allowed names only (the reader has refused anything else).
pub(crate) fn unpack(file: &Path, out: &Path) -> Result<()> {
    let p = CasePackage::read(&read_file(file)?).with_context(|| file.display().to_string())?;
    std::fs::create_dir_all(out.join(SCENARIO_DIR))?;
    let mut written = vec![
        (out.join(MANIFEST_NAME), p.manifest.to_ron()?),
        (out.join(CASE_NAME), p.case.to_ron()?),
    ];
    for (id, text) in &p.scenarios {
        // ids are `[a-z0-9_]` (checked by the reader): no separator, no `..`
        written.push((out.join(SCENARIO_DIR).join(format!("{id}.ron")), text.clone()));
    }
    for (path, text) in &written {
        std::fs::write(path, text).with_context(|| path.display().to_string())?;
        println!("{}", path.display());
    }
    Ok(())
}

/// `case check <name.hs>`: the reader's verdict against the installed rhythms (an error fails the
/// command), then the editorial rules of the shipped cases and a headless run of every stage as
/// warnings.
pub(crate) fn check(file: &Path, assets: Option<&Path>) -> Result<()> {
    let bytes = read_file(file)?;
    let assets = assets_root(assets)?;
    let ids = library_ids(&assets)?;
    let p = match CasePackage::read_for(&bytes, |id| ids.iter().any(|i| i == id)) {
        Ok(p) => p,
        Err(e) => bail!("{}: refused: {e}", file.display()),
    };
    let m = &p.manifest;
    println!(
        "{}: OK, case file version {}: {} \"{}\" ({}), {} stages, {} questions, {} bundled rhythms",
        file.display(),
        m.format_version,
        m.id,
        m.title,
        m.language,
        p.case.stages.len(),
        p.case.question_count(),
        p.scenarios.len()
    );
    println!(
        "  authors: {}; licence: {}; created {}; made by {}; sha256 {}",
        if m.authors.is_empty() {
            "not stated".to_owned()
        } else {
            m.authors.join(", ")
        },
        if m.licence.is_empty() {
            "not stated"
        } else {
            &m.licence
        },
        m.created,
        m.generator,
        heartsim_casefile::sha256_hex(&bytes)
    );
    let mut warnings = p.case.content_problems();
    warnings.extend(run_package(&p, &assets, None)?);
    for w in &warnings {
        println!("  warning: {w}");
    }
    println!("{} warnings", warnings.len());
    Ok(())
}
