//! `cargo xtask web-assets` / `web-budget` — the data side of the browser build (brief B21).
//!
//! The wasm binary carries no anatomy: the page downloads `heartsim-assets.zip` (the anatomy pack,
//! the rhythm library and the tutorials, laid out like `assets/`) and hands it to the app as an
//! in-memory asset bundle. `trunk` runs `web-assets` as its `pre_build` hook; CI runs `web-budget`
//! on the finished `dist/`.

use anyhow::{Context, Result, bail};
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};

/// File name the web crate requests; keep in step with `heartsim-web/src/archive.rs`.
pub const ARCHIVE_NAME: &str = "heartsim-assets.zip";

/// Wasm plus asset archive must stay below this (brief B21: "size budget < 25 MB").
pub const BUDGET_BYTES: u64 = 25 * 1024 * 1024;

#[derive(clap::Args, Debug)]
pub struct AssetsArgs {
    /// Directory to write `heartsim-assets.zip` into.
    #[arg(long, default_value = "crates/heartsim-web/generated")]
    pub out: PathBuf,
}

#[derive(clap::Args, Debug)]
pub struct BudgetArgs {
    /// The `trunk build` output directory.
    #[arg(long, default_value = "crates/heartsim-web/dist")]
    pub dist: PathBuf,
}

/// Directories of `assets/` that ship to the browser. `drugs.ron` is compiled into the simulation
/// crate; `icon.svg` and `generated/` are packaging inputs.
const INCLUDED: [&str; 3] = ["anatomy/default", "scenarios", "tutorials"];

/// All files below `dir` as `(path relative to root with '/', absolute path)`, sorted.
fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push((rel, path));
        }
    }
    Ok(())
}

/// The archive bytes for `assets_root` (the repository's `assets/`).
///
/// Entries are sorted and carry a fixed timestamp, so the same assets always give the same bytes.
pub fn build_archive(assets_root: &Path) -> Result<Vec<u8>> {
    let mut files = Vec::new();
    for dir in INCLUDED {
        let d = assets_root.join(dir);
        if !d.is_dir() {
            bail!("{} is missing", d.display());
        }
        walk(assets_root, &d, &mut files)?;
    }
    files.sort();
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9))
        .last_modified_time(zip::DateTime::default());
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, path) in &files {
        zip.start_file(name, options)?;
        zip.write_all(&fs::read(path).with_context(|| format!("reading {}", path.display()))?)?;
    }
    Ok(zip.finish()?.into_inner())
}

pub fn assets(root: &Path, args: &AssetsArgs) -> Result<()> {
    let out_dir = if args.out.is_absolute() {
        args.out.clone()
    } else {
        root.join(&args.out)
    };
    let bytes = build_archive(&root.join("assets"))?;
    fs::create_dir_all(&out_dir)?;
    let target = out_dir.join(ARCHIVE_NAME);
    // leave the file alone when nothing changed, so trunk's watcher does not rebuild in a loop
    if fs::read(&target).ok().as_deref() != Some(&bytes) {
        fs::write(&target, &bytes).with_context(|| format!("writing {}", target.display()))?;
    }
    eprintln!("{}: {} bytes", target.display(), bytes.len());
    Ok(())
}

/// Sum of the `.wasm` files and the asset archive in `dist`, bytes.
pub fn measure(dist: &Path) -> Result<(u64, u64)> {
    let (mut wasm, mut archive) = (0, 0);
    for entry in fs::read_dir(dist)
        .with_context(|| format!("reading {} (run `trunk build --release` first)", dist.display()))?
    {
        let path = entry?.path();
        let len = fs::metadata(&path)?.len();
        match path.extension().and_then(|e| e.to_str()) {
            Some("wasm") => wasm += len,
            Some("zip") => archive += len,
            _ => {}
        }
    }
    Ok((wasm, archive))
}

pub fn budget(root: &Path, args: &BudgetArgs) -> Result<()> {
    let dist = if args.dist.is_absolute() {
        args.dist.clone()
    } else {
        root.join(&args.dist)
    };
    let (wasm, archive) = measure(&dist)?;
    if wasm == 0 {
        bail!("no .wasm in {}", dist.display());
    }
    let mib = |b: u64| b as f64 / (1024.0 * 1024.0);
    println!(
        "wasm {:.2} MiB + assets {:.2} MiB = {:.2} MiB (budget {:.0} MiB)",
        mib(wasm),
        mib(archive),
        mib(wasm + archive),
        mib(BUDGET_BYTES)
    );
    if wasm + archive > BUDGET_BYTES {
        bail!(
            "web build is over the {} MiB budget",
            BUDGET_BYTES / (1024 * 1024)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn repo_assets() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets")
    }

    #[test]
    fn archive_holds_the_pack_rhythms_and_tutorials_and_nothing_else() {
        let bytes = build_archive(&repo_assets()).unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
            .collect();
        for must in [
            "anatomy/default/manifest.ron",
            "anatomy/default/cells.bin",
            "anatomy/default/torso.glb",
            "scenarios/sinus_normal.ron",
            "tutorials/basics/tutorial.ron",
        ] {
            assert!(names.iter().any(|n| n == must), "{must} missing from {names:?}");
        }
        assert!(
            names
                .iter()
                .all(|n| INCLUDED.iter().any(|d| n.starts_with(&format!("{d}/")))),
            "unexpected entry in {names:?}"
        );
        let mut manifest = String::new();
        zip.by_name("anatomy/default/manifest.ron")
            .unwrap()
            .read_to_string(&mut manifest)
            .unwrap();
        assert!(manifest.contains("format_version"));
    }

    #[test]
    fn archive_is_reproducible() {
        assert_eq!(
            build_archive(&repo_assets()).unwrap(),
            build_archive(&repo_assets()).unwrap()
        );
    }

    #[test]
    fn archive_fits_its_share_of_the_budget() {
        let bytes = build_archive(&repo_assets()).unwrap();
        assert!(
            (bytes.len() as u64) < BUDGET_BYTES / 2,
            "the asset archive alone is {} bytes",
            bytes.len()
        );
    }
}
