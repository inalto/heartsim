//! The downloaded asset archive: one zip with the anatomy pack, the rhythm library and the
//! tutorials, laid out like the repository's `assets/` directory (`cargo xtask web-assets` writes
//! it). Deflate is the gzip algorithm, and one file means one request and one progress bar.

use heartsim_app::AssetBundle;
use std::io::{Cursor, Read};

/// File name the page requests, next to `index.html`.
pub const ARCHIVE_NAME: &str = "heartsim-assets.zip";

/// Refuse archives that unpack to more than this (a corrupt or hostile download must not take the
/// tab down); the real pack is ~10 MB.
const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;

/// What can go wrong unpacking the archive.
#[derive(Debug)]
pub enum ArchiveError {
    Zip(zip::result::ZipError),
    Io(std::io::Error),
    TooLarge,
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zip(e) => write!(f, "the asset archive is damaged: {e}"),
            Self::Io(e) => write!(f, "the asset archive could not be read: {e}"),
            Self::TooLarge => write!(
                f,
                "the asset archive unpacks to more than {MAX_UNPACKED_BYTES} bytes"
            ),
        }
    }
}

impl std::error::Error for ArchiveError {}

/// Unpack `bytes` into a bundle; directory entries are skipped, names keep their `/` separators.
///
/// # Errors
/// On a malformed archive or when it is implausibly large.
pub fn unpack(bytes: &[u8]) -> Result<AssetBundle, ArchiveError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(ArchiveError::Zip)?;
    let mut bundle = AssetBundle::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let file = zip.by_index(i).map_err(ArchiveError::Zip)?;
        if !file.is_file() {
            continue;
        }
        // `enclosed_name` rejects `..` and absolute paths
        let Some(name) = file
            .enclosed_name()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
        else {
            continue;
        };
        total += file.size();
        if total > MAX_UNPACKED_BYTES {
            return Err(ArchiveError::TooLarge);
        }
        let mut data = Vec::with_capacity(file.size() as usize);
        file.take(MAX_UNPACKED_BYTES)
            .read_to_end(&mut data)
            .map_err(ArchiveError::Io)?;
        bundle.insert(name, data);
    }
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts =
            zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn unpacks_files_under_their_asset_paths() {
        let b = unpack(&archive(&[
            ("scenarios/a.ron", b"A"),
            ("anatomy/default/manifest.ron", b"M"),
        ]))
        .unwrap();
        assert_eq!(b.len(), 2);
        assert_eq!(b.get("scenarios/a.ron"), Some(&b"A"[..]));
        assert_eq!(b.text("anatomy/default/manifest.ron").as_deref(), Some("M"));
    }

    #[test]
    fn path_traversal_entries_are_dropped_and_absolute_ones_made_relative() {
        let b = unpack(&archive(&[
            ("../evil.ron", b"x"),
            ("a/../../evil2.ron", b"x"),
            ("/abs.ron", b"y"),
            ("ok.ron", b"z"),
        ]))
        .unwrap();
        assert!(
            b.get("../evil.ron").is_none() && b.get("evil.ron").is_none() && b.get("evil2.ron").is_none()
        );
        assert!(b.get("ok.ron").is_some());
        // a leading `/` cannot point outside the virtual tree; the file lands inside it
        assert_eq!(b.get("abs.ron"), Some(&b"y"[..]));
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(unpack(b"definitely not a zip").is_err());
        assert!(unpack(&[]).is_err());
    }
}
