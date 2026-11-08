//! Download and unpack the open-licence sources of the default pack into the git-ignored
//! `assets/anatomy-src/`, verifying the MD5 that each Zenodo record publishes. Nothing here is
//! vendored: the pack is baked from these files and only the pack is committed.
//!
//! Sources (licences and attribution: `docs/research/anatomy-meshes.md`):
//! * heart — Rodero et al. 2021, the SSM *average* four-chamber mesh, Zenodo 4593739, CC BY 4.0;
//! * torso — Qian, Bishop et al. 2022, KCL whole-torso CT model 1, Zenodo 7253863, CC BY 4.0.

use super::md5::file_md5;
use anyhow::{Context, Result, bail};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How an archive unpacks.
#[derive(Debug, Clone, Copy)]
pub enum Unpack {
    TarGz,
    Zip,
}

/// One downloadable archive.
#[derive(Debug, Clone, Copy)]
pub struct Source {
    /// Sub-directory of the source root.
    pub dir: &'static str,
    pub url: &'static str,
    pub file_name: &'static str,
    pub bytes: u64,
    pub md5: &'static str,
    pub unpack: Unpack,
    /// A file that exists once the archive is unpacked (relative to `dir`).
    pub marker: &'static str,
}

/// Rodero et al. 2021, Zenodo record 4593739: `average.tar.gz` → `average.vtk` (ASCII legacy VTK,
/// 379 158 nodes, 1 766 006 tetrahedra, 24 tags, fibres and UVC).
pub const RODERO_AVERAGE: Source = Source {
    dir: "rodero-average",
    url: "https://zenodo.org/api/records/4593739/files/average.tar.gz/content",
    file_name: "average.tar.gz",
    bytes: 58_167_648,
    md5: "992f31e20c1aa73c10c5d9a6b6ac903a",
    unpack: Unpack::TarGz,
    marker: "average.vtk",
};

/// Qian & Bishop 2022, Zenodo record 7253863: `KCL_torso1.zip` → `KCL_torso1/KCL_torso1.{pts,elem,lon}`
/// (openCARP text, micrometres, 1 689 018 nodes, 9 686 213 tetrahedra, 35 tags).
pub const KCL_TORSO1: Source = Source {
    dir: "kcl-torso1",
    url: "https://zenodo.org/api/records/7253863/files/KCL_torso1.zip/content",
    file_name: "KCL_torso1.zip",
    bytes: 167_823_369,
    md5: "23082fcd5649f39671e3d831121948cb",
    unpack: Unpack::Zip,
    marker: "KCL_torso1/KCL_torso1.elem",
};

/// Ensure `source` is downloaded, verified and unpacked under `root`; returns its directory.
pub fn ensure(root: &Path, source: &Source, offline: bool) -> Result<PathBuf> {
    let dir = root.join(source.dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let archive = dir.join(source.file_name);
    if dir.join(source.marker).is_file() {
        return Ok(dir);
    }
    if !archive.is_file() || !verify(&archive, source)? {
        if offline {
            bail!(
                "{} is missing or corrupt and --offline was given; download it from {}",
                archive.display(),
                source.url
            );
        }
        download(&archive, source)?;
        if !verify(&archive, source)? {
            bail!(
                "{}: size or MD5 differs from the Zenodo record (expected {} bytes, md5 {})",
                archive.display(),
                source.bytes,
                source.md5
            );
        }
    }
    eprintln!("unpacking {}", archive.display());
    match source.unpack {
        Unpack::TarGz => untar_gz(&archive, &dir)?,
        Unpack::Zip => unzip(&archive, &dir)?,
    }
    if !dir.join(source.marker).is_file() {
        bail!("{} did not contain {}", archive.display(), source.marker);
    }
    Ok(dir)
}

fn verify(archive: &Path, source: &Source) -> Result<bool> {
    let len = archive.metadata()?.len();
    if len != source.bytes {
        eprintln!("{}: {len} bytes, record says {}", archive.display(), source.bytes);
        return Ok(false);
    }
    let sum = file_md5(archive)?;
    if sum != source.md5 {
        eprintln!("{}: md5 {sum}, record says {}", archive.display(), source.md5);
        return Ok(false);
    }
    eprintln!("{}: {len} bytes, md5 {sum} ok", archive.display());
    Ok(true)
}

/// `curl` does the transfer (resumable, proxy-aware, no TLS stack in the build tool).
fn download(archive: &Path, source: &Source) -> Result<()> {
    eprintln!("downloading {} ({:.0} MB)", source.url, source.bytes as f64 / 1e6);
    let part = archive.with_extension("part");
    let status = Command::new("curl")
        .args(["-L", "-sS", "--fail", "-C", "-", "-o"])
        .arg(&part)
        .arg(source.url)
        .status()
        .context("spawning curl (install curl or download the archive by hand)")?;
    if !status.success() {
        bail!("curl failed for {}", source.url);
    }
    std::fs::rename(&part, archive)?;
    Ok(())
}

/// Minimal POSIX/ustar tar reader over a gzip stream: regular files only, which is all the
/// Zenodo archives contain.
fn untar_gz(archive: &Path, out: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut gz = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
    let mut header = [0u8; 512];
    loop {
        if gz.read_exact(&mut header).is_err() || header.iter().all(|b| *b == 0) {
            return Ok(());
        }
        let name = cstr(&header[0..100]);
        let prefix = cstr(&header[345..500]);
        let size = usize::from_str_radix(cstr(&header[124..136]).trim(), 8)
            .with_context(|| format!("tar: bad size field for {name}"))?;
        let kind = header[156];
        let full = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let mut body = vec![0u8; size];
        gz.read_exact(&mut body)?;
        let pad = (512 - size % 512) % 512;
        let mut skip = vec![0u8; pad];
        gz.read_exact(&mut skip)?;
        if kind == b'0' || kind == 0 {
            let rel = safe_relative(&full)?;
            let path = out.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, &body)?;
            eprintln!("  {} ({:.1} MB)", path.display(), size as f64 / 1e6);
        }
    }
}

fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Reject archive members that would escape the output directory.
fn safe_relative(name: &str) -> Result<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute()
        || p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir))
    {
        bail!("archive member `{name}` has an unsafe path");
    }
    Ok(p.to_path_buf())
}

fn unzip(archive: &Path, out: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            bail!(
                "zip member {} has an unsafe path",
                entry.name_raw().escape_ascii()
            );
        };
        let path = out.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::fs::File::create(&path)?;
        let n = std::io::copy(&mut entry, &mut f)?;
        eprintln!("  {} ({:.1} MB)", path.display(), n as f64 / 1e6);
    }
    Ok(())
}
