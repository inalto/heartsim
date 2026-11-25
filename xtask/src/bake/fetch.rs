//! Download and unpack the open-licence sources of the default pack into the git-ignored
//! `assets/anatomy-src/`, verifying the MD5 that each Zenodo record publishes. Nothing here is
//! vendored: the pack is baked from these files and only the pack is committed.
//!
//! Sources (licences and attribution: `docs/research/anatomy-meshes.md`):
//! * heart — Rodero et al. 2021, the SSM *average* four-chamber mesh, Zenodo 4593739, CC BY 4.0;
//! * torso — Qian, Bishop et al. 2022, KCL whole-torso CT model 1, Zenodo 7253863, CC BY 4.0;
//! * tricuspid valve — Mathur, Rausch et al., Texas TriValve 1.1 (GitHub, pinned commit), CC BY 4.0
//!   (brief B35; `docs/research/` has the licence check). Its files are fetched one by one at the
//!   pinned commit and checked against the sizes and MD5s recorded here when the commit was pinned.

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

/// One file of a repository fetched at a pinned commit.
#[derive(Debug, Clone, Copy)]
pub struct PinnedFile {
    /// Path in the repository (and under the source's directory).
    pub path: &'static str,
    pub bytes: u64,
    pub md5: &'static str,
}

/// A set of files of a public Git repository at one commit (no archive to unpack).
#[derive(Debug, Clone, Copy)]
pub struct PinnedRepo {
    /// Sub-directory of the source root.
    pub dir: &'static str,
    /// `https://raw.githubusercontent.com/<owner>/<repo>`.
    pub raw_base: &'static str,
    pub commit: &'static str,
    pub files: &'static [PinnedFile],
}

impl PinnedRepo {
    /// The URL of one file at the pinned commit.
    #[must_use]
    pub fn url(&self, f: &PinnedFile) -> String {
        format!("{}/{}/{}", self.raw_base, self.commit, f.path)
    }
}

macro_rules! pinned {
    ($($path:literal, $bytes:literal, $md5:literal;)*) => {
        &[$(PinnedFile { path: $path, bytes: $bytes, md5: $md5 }),*]
    };
}

/// Texas TriValve 1.1 (Mathur, Meador, Malinowski, Jazwiec, Timek, Rausch; CC BY 4.0), the
/// healthy quasi-static load case: the Abaqus model and its 21 result frames, with the licence and
/// the README. Commit of 2024-04-16 (the repository's `main` when pinned, 2026-11-25); the sizes
/// are those of the GitHub tree listing, the MD5s of the files downloaded then.
pub const TEXAS_TRIVALVE_1_1: PinnedRepo = PinnedRepo {
    dir: "texas-trivalve-1.1",
    raw_base: "https://raw.githubusercontent.com/SoftTissueBiomechanicsLab/Texas_TriValve_1.1",
    commit: "cfbdad5ab8f2e02934f317dde1c2566963c98f4b",
    files: pinned! {
        "LICENSE-CC-BY.txt", 18_650, "4a17ffc27c9f3b240fbf4fe17783c89c";
        "README.md", 2_993, "fe53ce74c01f090475bb45d279a8a1a5";
        "Healthy_QuasiStatic/Healthy_QuasiStatic.inp", 1_384_521, "9cc3ac8d543cfcd341702ba8c4f4256d";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_0.vtu", 183_738, "26a6d1156233d1c39a05f7be6b273720";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_1.vtu", 530_785, "00c0286d798013f53e69cf1378051151";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_2.vtu", 532_081, "259a86dafcb1703bff89d4ab98f1f02b";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_3.vtu", 532_177, "d9e7cf40c7649bd46852965116431bf3";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_4.vtu", 531_237, "d64f77cbf5e3cf0f724d83cedd9a2f49";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_5.vtu", 535_421, "69a925d1d29cb59cd0a16a999a1130ef";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_6.vtu", 533_681, "29f0dc37de01b16c109b0fc8a85079e0";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_7.vtu", 532_565, "6bf3552444f4d3301eb94d4ab7e59186";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_8.vtu", 536_289, "c59a887527e8781b330e86c14af875da";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_9.vtu", 536_453, "e20d4a65fdb9e0f0093c54ae93525919";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_10.vtu", 532_873, "03e3b2643c915cbc9b1da9bcb01a0f57";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_11.vtu", 537_677, "f299a73396e15b159b5989763a7a68a7";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_12.vtu", 533_017, "70c2d4c061956516395546e5a97050dc";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_13.vtu", 532_381, "737d2b4f4bf43aeb4e294722150a4af9";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_14.vtu", 535_905, "d075732dafd93997539851da038a03b8";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_15.vtu", 536_505, "13079af921ccabb894b349ce2921016e";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_16.vtu", 530_845, "2a3541f0a393b7c4be190c17056ad598";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_17.vtu", 535_993, "560f2696cd0564fc0b6a49510b255ee5";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_18.vtu", 538_889, "953b3e3922c185c05f68d1d5220c00b9";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_19.vtu", 533_817, "418e758a193842843f9e2127a25f47d6";
        "Healthy_QuasiStatic/Healthy_QuasiStatic_20.vtu", 534_657, "7fb2f01d5329d4899e294e8e1282b1f3";
    },
};

/// Ensure every file of `repo` is under `root/<dir>` with its recorded size and MD5, downloading
/// the missing or corrupt ones at the pinned commit; returns the directory.
pub fn ensure_repo(root: &Path, repo: &PinnedRepo, offline: bool) -> Result<PathBuf> {
    let dir = root.join(repo.dir);
    for f in repo.files {
        let path = dir.join(f.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let ok = |p: &Path| -> Result<bool> {
            Ok(p.is_file() && p.metadata()?.len() == f.bytes && file_md5(p)? == f.md5)
        };
        if ok(&path)? {
            continue;
        }
        let url = repo.url(f);
        if offline {
            bail!(
                "{} is missing or corrupt and --offline was given; download it from {url}",
                path.display()
            );
        }
        eprintln!("downloading {url}");
        let part = path.with_extension("part");
        let _ = std::fs::remove_file(&part);
        let status = Command::new("curl")
            .args(["-L", "-sS", "--fail", "-o"])
            .arg(&part)
            .arg(&url)
            .status()
            .context("spawning curl (install curl or download the file by hand)")?;
        if !status.success() {
            bail!("curl failed for {url}");
        }
        std::fs::rename(&part, &path)?;
        if !ok(&path)? {
            bail!(
                "{}: size or MD5 differs from the pinned file (expected {} bytes, md5 {})",
                path.display(),
                f.bytes,
                f.md5
            );
        }
    }
    eprintln!(
        "{}: {} pinned files ok ({})",
        dir.display(),
        repo.files.len(),
        repo.commit
    );
    Ok(dir)
}

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
