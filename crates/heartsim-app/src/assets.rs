//! Assets held in memory: the web build has no installed `assets/` directory to look in, so the
//! page downloads one archive and hands its files over as an [`AssetBundle`].
//!
//! Paths are relative to the repository's `assets/` directory and use `/` on every platform
//! (`anatomy/default/manifest.ron`, `scenarios/sinus_normal.ron`, `tutorials/basics/tutorial.ron`),
//! so the same loaders can be tested natively against a bundle read from disk.

use heartsim_geometry::pack::PackError;
use std::collections::BTreeMap;

/// Where the default anatomy pack sits inside a bundle.
pub const PACK_DIR: &str = "anatomy/default";

/// A virtual `assets/` tree: relative path → file bytes.
#[derive(Debug, Clone, Default)]
pub struct AssetBundle {
    files: BTreeMap<String, Vec<u8>>,
}

impl AssetBundle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add (or replace) a file. `path` is relative to `assets/` and uses `/`.
    pub fn insert(&mut self, path: impl Into<String>, bytes: Vec<u8>) {
        self.files.insert(path.into(), bytes);
    }

    #[must_use]
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    /// The file as text (invalid UTF-8 is replaced, as the disk loaders do).
    #[must_use]
    pub fn text(&self, path: &str) -> Option<String> {
        self.get(path).map(|b| String::from_utf8_lossy(b).into_owned())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// A copy holding only the files at or below one of `prefixes` (`anatomy/default`, matched on
    /// whole path segments), for a part of the app that keeps its own copy of what it needs.
    #[must_use]
    pub fn subset(&self, prefixes: &[&str]) -> Self {
        let files = self
            .files
            .iter()
            .filter(|(p, _)| {
                prefixes
                    .iter()
                    .any(|d| p.strip_prefix(d).is_some_and(|rest| rest.starts_with('/')))
            })
            .map(|(p, b)| (p.clone(), b.clone()))
            .collect();
        Self { files }
    }

    /// Total size of all files, bytes.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.files.values().map(Vec::len).sum()
    }

    /// Paths of the files directly inside `dir` (no trailing slash) that end with `suffix`, in
    /// sorted order, exactly as a sorted `read_dir` would list them.
    pub fn files_in<'a>(&'a self, dir: &'a str, suffix: &'a str) -> impl Iterator<Item = &'a str> {
        self.files.keys().map(String::as_str).filter(move |p| {
            p.strip_prefix(dir)
                .and_then(|rest| rest.strip_prefix('/'))
                .is_some_and(|name| !name.contains('/') && name.ends_with(suffix))
        })
    }

    /// Names of the sub-directories of `dir` that contain a file called `file`
    /// (`tutorials/<id>/tutorial.ron`), sorted.
    pub fn subdirs_with<'a>(&'a self, dir: &'a str, file: &'a str) -> impl Iterator<Item = &'a str> {
        self.files.keys().map(String::as_str).filter_map(move |p| {
            let rest = p.strip_prefix(dir)?.strip_prefix('/')?;
            let (sub, name) = rest.split_once('/')?;
            (name == file).then_some(sub)
        })
    }

    /// Read every file below `root` (the repository's `assets/`) into a bundle: what the web page
    /// downloads, built from disk so tests can check the in-memory loaders against the disk ones.
    ///
    /// # Errors
    /// On any I/O failure.
    #[cfg(feature = "native")]
    pub fn from_dir(root: &std::path::Path) -> std::io::Result<Self> {
        fn walk(dir: &std::path::Path, prefix: &str, out: &mut AssetBundle) -> std::io::Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let rel = if prefix.is_empty() {
                    name
                } else {
                    format!("{prefix}/{name}")
                };
                if entry.file_type()?.is_dir() {
                    walk(&entry.path(), &rel, out)?;
                } else {
                    out.insert(rel, std::fs::read(entry.path())?);
                }
            }
            Ok(())
        }
        let mut bundle = Self::new();
        walk(root, "", &mut bundle)?;
        Ok(bundle)
    }

    /// Load the default anatomy pack from the bundle (same size and CRC checks as from disk).
    ///
    /// # Errors
    /// When a file is missing or fails verification.
    pub fn load_pack(&self) -> Result<heartsim_geometry::pack::AnatomyPack, PackError> {
        heartsim_geometry::pack::AnatomyPack::load_with(|rel| {
            let path = format!("{PACK_DIR}/{rel}");
            self.get(&path)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| PackError::Format(format!("{path}: not in the downloaded assets")))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> AssetBundle {
        let mut b = AssetBundle::new();
        for (p, t) in [
            ("scenarios/b.ron", "b"),
            ("scenarios/a.ron", "a"),
            ("scenarios/readme.md", "x"),
            ("scenarios/sub/c.ron", "c"),
            ("tutorials/two/tutorial.ron", "2"),
            ("tutorials/one/tutorial.ron", "1"),
            ("tutorials/one/extra.ron", "e"),
            ("tutorials/stray/other.ron", "s"),
        ] {
            b.insert(p, t.as_bytes().to_vec());
        }
        b
    }

    #[test]
    fn lists_files_like_a_sorted_read_dir() {
        let b = bundle();
        let got: Vec<_> = b.files_in("scenarios", ".ron").collect();
        assert_eq!(
            got,
            ["scenarios/a.ron", "scenarios/b.ron"],
            "no sub-directories, no other suffixes"
        );
    }

    #[test]
    fn subset_matches_whole_path_segments() {
        let mut b = bundle();
        b.insert("tutorials-extra/x.ron", b"x".to_vec());
        let s = b.subset(&["tutorials"]);
        assert_eq!(s.len(), 4, "tutorials/* only, not tutorials-extra or scenarios");
        assert!(s.get("tutorials/one/tutorial.ron").is_some());
        assert!(s.get("tutorials-extra/x.ron").is_none());
    }

    #[test]
    fn finds_tutorial_directories_by_their_marker_file() {
        let b = bundle();
        let got: Vec<_> = b.subdirs_with("tutorials", "tutorial.ron").collect();
        assert_eq!(got, ["one", "two"]);
    }

    #[cfg(feature = "native")]
    fn repo_assets() -> AssetBundle {
        AssetBundle::from_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"))
            .expect("assets dir")
    }

    /// The web build must show exactly the rhythm library the desktop build shows.
    #[cfg(feature = "native")]
    #[test]
    fn bundle_scenarios_match_the_disk_library() {
        use crate::scenarios::ScenarioLibrary;
        let disk = ScenarioLibrary::load_default();
        let mem = ScenarioLibrary::from_bundle(&repo_assets());
        assert!(mem.len() >= 27, "only {} scenarios in the bundle", mem.len());
        let ids = |l: &ScenarioLibrary| {
            l.entries
                .iter()
                .map(|e| e.scenario.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&mem), ids(&disk));
    }

    #[cfg(feature = "native")]
    #[test]
    fn bundle_tutorials_match_the_disk_library() {
        use crate::tutorial::TutorialLibrary;
        let disk = TutorialLibrary::load_default();
        let mem = TutorialLibrary::from_bundle(&repo_assets());
        assert!(
            mem.tutorials.len() >= 2,
            "only {} tutorials in the bundle",
            mem.tutorials.len()
        );
        let ids = |l: &TutorialLibrary| l.tutorials.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&mem), ids(&disk));
    }

    /// The pack loads from memory with the same verification as from disk, and a flipped byte is
    /// caught by the manifest checksum.
    #[cfg(feature = "native")]
    #[test]
    fn pack_loads_from_memory_and_detects_corruption() {
        let mut bundle = repo_assets();
        let pack = bundle.load_pack().expect("default pack from memory");
        assert!(pack.cells.len() > 1000);
        let path = format!("{PACK_DIR}/cells.bin");
        let mut bytes = bundle.get(&path).expect("cells.bin").to_vec();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        bundle.insert(path, bytes);
        assert!(matches!(bundle.load_pack(), Err(PackError::Checksum { .. })));
    }

    #[test]
    fn missing_pack_is_an_error_not_a_panic() {
        assert!(AssetBundle::new().load_pack().is_err());
    }
}
