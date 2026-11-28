//! `cargo xtask case-library --out <dir>` — the official online case library (brief B43).
//!
//! Packs every `assets/cases/*.ron` into `<id>.hs` with `heartsim-casefile` (the same writer as the
//! application and the CLI) and writes the catalogue `index.ron` beside them, in the format of the
//! "Libraries" chapter of `docs/case-file.md`. `.github/workflows/docs.yml` runs it into
//! `docs/book/book/cases/`, so the library is published with GitHub Pages next to the guide and the
//! browser app.
//!
//! Deterministic: the manifests carry the fixed [`CREATED`] date and [`GENERATOR`] text (no
//! clock, no version number), so a case file's bytes, and its SHA-256, change only when the case
//! changes; a user who imported a case is offered an update only then. The index's `updated` is
//! what `--updated` says (the workflow passes the commit date) and is left empty without it. Two
//! runs with the same arguments write the same bytes.

use anyhow::{Context, Result, bail, ensure};
use heartsim_casefile::library::{CaseEntry, CaseIndex, INDEX_FORMAT_VERSION, INDEX_NAME, parse_index};
use heartsim_casefile::{CasePackage, Manifest, sha256_hex};
use heartsim_core::case::Case;
use std::path::{Path, PathBuf};

/// `created` of every manifest the library writes (the day the library format was made).
pub const CREATED: &str = "2026-11-27";
/// `generator` of every manifest the library writes.
pub const GENERATOR: &str = "heartsim case-library";
/// Authors of the shipped cases, as the application writes them (`case_ui::BUILTIN_AUTHORS`).
pub const AUTHORS: &str = "heartsim authors";
/// Licence of the shipped case texts (`ASSETS-LICENSES.md`, `case_ui::BUILTIN_LICENCE`).
pub const LICENCE: &str = "MIT OR Apache-2.0";
/// Title of the official library.
pub const TITLE: &str = "heartsim case library";

#[derive(clap::Args, Debug)]
pub struct CaseLibraryArgs {
    /// Directory to write `index.ron` and the `.hs` files into (created if missing).
    #[arg(long)]
    pub out: PathBuf,
    /// The cases to pack (`*.ron`).
    #[arg(long, default_value = "assets/cases")]
    pub cases: PathBuf,
    /// The rhythm library the cases must resolve against (`*.ron`, by file stem).
    #[arg(long, default_value = "assets/scenarios")]
    pub scenarios: PathBuf,
    /// The index's `updated` date (`YYYY-MM-DD`); empty when not given.
    #[arg(long, default_value = "")]
    pub updated: String,
}

fn ron_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| dir.display().to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .collect();
    v.sort();
    Ok(v)
}

/// Write the library; returns the index as written.
///
/// # Errors
/// When a case does not parse or resolve, a file cannot be written, or the written library does
/// not read back (every file through `CasePackage::read`, its digest and size against the index).
pub fn run(root: &Path, args: &CaseLibraryArgs) -> Result<CaseIndex> {
    let at = |p: &Path| {
        if p.is_absolute() {
            p.to_owned()
        } else {
            root.join(p)
        }
    };
    let (cases_dir, scenario_dir, out) = (at(&args.cases), at(&args.scenarios), at(&args.out));
    let library: Vec<String> = ron_files(&scenario_dir)?
        .iter()
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    std::fs::create_dir_all(&out).with_context(|| out.display().to_string())?;
    let mut entries = Vec::new();
    for path in ron_files(&cases_dir)? {
        let text = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        let case = Case::from_ron(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        let manifest = Manifest::for_case(&case, vec![AUTHORS.to_owned()], LICENCE, CREATED, GENERATOR);
        let p = CasePackage::new(manifest, case, Vec::new());
        p.resolve(|id| library.iter().any(|l| l == id))
            .with_context(|| path.display().to_string())?;
        let bytes = p.write().with_context(|| path.display().to_string())?;
        let file = p.file_name();
        std::fs::write(out.join(&file), &bytes).with_context(|| file.clone())?;
        entries.push(CaseEntry {
            id: p.case.id.clone(),
            title: p.case.title.clone(),
            summary: p.case.summary.clone(),
            language: p.manifest.language.clone(),
            authors: p.manifest.authors.clone(),
            licence: p.manifest.licence.clone(),
            file,
            size_bytes: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
            case_format_version: p.case.format_version,
            min_app_version: None,
        });
    }
    if entries.is_empty() {
        bail!("no case in {}", cases_dir.display());
    }
    // a file left from an earlier run that the index no longer lists
    for old in std::fs::read_dir(&out)?.filter_map(|e| e.ok().map(|e| e.path())) {
        let name = old
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if old.extension().is_some_and(|x| x == heartsim_casefile::EXTENSION)
            && !entries.iter().any(|e| e.file == name)
        {
            std::fs::remove_file(&old).with_context(|| old.display().to_string())?;
        }
    }
    let index = CaseIndex {
        format_version: INDEX_FORMAT_VERSION,
        title: TITLE.to_owned(),
        updated: args.updated.clone(),
        cases: entries,
    };
    let text = index.to_ron()?;
    std::fs::write(out.join(INDEX_NAME), &text)?;
    check(&out, &index)?;
    println!("{}: {} cases and {INDEX_NAME}", out.display(), index.cases.len());
    Ok(index)
}

/// Read the written library back as a client would.
fn check(out: &Path, written: &CaseIndex) -> Result<()> {
    let parsed = parse_index(&std::fs::read(out.join(INDEX_NAME))?)?;
    ensure!(
        parsed.ignored == 0,
        "{} entries of the index do not pass the reader",
        parsed.ignored
    );
    ensure!(&parsed.index == written, "the index does not read back equal");
    for e in &parsed.index.cases {
        let bytes = std::fs::read(out.join(&e.file))?;
        ensure!(bytes.len() as u64 == e.size_bytes, "{}: size", e.file);
        ensure!(sha256_hex(&bytes) == e.sha256, "{}: digest", e.file);
        let p = CasePackage::read(&bytes).with_context(|| e.file.clone())?;
        ensure!(p.manifest.id == e.id, "{}: id", e.file);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(out: &Path) -> CaseLibraryArgs {
        CaseLibraryArgs {
            out: out.to_owned(),
            cases: "assets/cases".into(),
            scenarios: "assets/scenarios".into(),
            updated: "2026-11-28".into(),
        }
    }

    fn read_dir_bytes(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let p = e.unwrap().path();
                (
                    p.file_name().unwrap().to_string_lossy().into_owned(),
                    std::fs::read(&p).unwrap(),
                )
            })
            .collect();
        v.sort();
        v
    }

    /// B43 acceptance: the index's digests match the files written, every file passes the case-file
    /// reader, and two runs are byte-identical.
    #[test]
    fn the_library_of_the_shipped_cases_is_consistent_and_deterministic() {
        let root = crate::repo_root();
        let base = std::env::temp_dir().join(format!("heartsim-b43-library-{}", std::process::id()));
        let (one, two) = (base.join("one"), base.join("two"));
        let _ = std::fs::remove_dir_all(&base);
        let index = run(&root, &args(&one)).unwrap();
        run(&root, &args(&two)).unwrap();
        assert!(index.cases.len() >= 9, "{} cases", index.cases.len());
        let files = read_dir_bytes(&one);
        assert_eq!(files.len(), index.cases.len() + 1, "the cases and index.ron");
        for e in &index.cases {
            let bytes = &files.iter().find(|(n, _)| n == &e.file).unwrap().1;
            assert_eq!(sha256_hex(bytes), e.sha256, "{}", e.file);
            assert_eq!(bytes.len() as u64, e.size_bytes);
            let p = CasePackage::read(bytes).unwrap();
            assert_eq!(p.manifest.created, CREATED);
            assert_eq!(p.manifest.generator, GENERATOR);
        }
        assert_eq!(files, read_dir_bytes(&two), "two runs are byte-identical");
        // a stale case file is removed by the next run
        std::fs::write(one.join("case_gone.hs"), b"old").unwrap();
        run(&root, &args(&one)).unwrap();
        assert!(!one.join("case_gone.hs").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
