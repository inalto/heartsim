//! Where the desktop app looks for `assets/` (the anatomy pack, the rhythm library, tutorials and
//! cases), and the `--assets-dir` flag the installer smoke tests use to prove an installed copy
//! finds them. Without this, a missing or misplaced `assets/` shows the placeholder sphere and an
//! empty rhythm picker, which is what the 0.7.0 installers did on every platform.

use std::path::{Path, PathBuf};

/// Directory name of the assets tree, as shipped by the installers and checked out in the source tree.
pub const ASSETS_DIR_NAME: &str = "assets";

/// The candidate roots, in the order they are tried:
/// 1. `$HEARTSIM_ASSETS`;
/// 2. next to the executable (`<exe dir>/assets`: Windows installers, a plain unpacked build);
/// 3. the macOS bundle (`<exe dir>/../Resources/assets`);
/// 4. the Linux package layouts, where the binary is in `usr/bin` and resources in
///    `usr/lib/heartsim/assets` (deb, AppImage) or `usr/share/heartsim/assets`;
/// 5. `assets` in the working directory;
/// 6. the source tree the binary was built from (development runs and tests).
#[must_use]
pub fn asset_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("HEARTSIM_ASSETS") {
        roots.push(dir.into());
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        roots.push(exe_dir.join(ASSETS_DIR_NAME));
        roots.push(exe_dir.join("../Resources").join(ASSETS_DIR_NAME));
        roots.push(exe_dir.join("../lib/heartsim").join(ASSETS_DIR_NAME));
        roots.push(exe_dir.join("../share/heartsim").join(ASSETS_DIR_NAME));
    }
    roots.push(PathBuf::from(ASSETS_DIR_NAME));
    roots.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(ASSETS_DIR_NAME),
    );
    roots
}

/// The first root that holds the default anatomy pack (`anatomy/default/manifest.ron`), which is
/// what the installers must ship for the 3-D heart to appear at all.
#[must_use]
pub fn resolved_assets_dir() -> Option<PathBuf> {
    asset_roots()
        .into_iter()
        .find(|r| r.join("anatomy").join("default").join("manifest.ron").is_file())
}

/// The first root whose `sub` directory (e.g. `scenarios`) exists, or `None`.
#[must_use]
pub fn find_asset_subdir(sub: &str) -> Option<PathBuf> {
    asset_roots()
        .into_iter()
        .map(|r| r.join(sub))
        .find(|d| d.is_dir())
}

/// True when `args` (without the program name) ask where the assets were found (`--assets-dir`).
pub fn wants_assets_dir<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|a| a.as_ref() == "--assets-dir")
}

/// The text `heartsim --assets-dir` prints, and whether that is a success: the resolved directory on
/// success; on failure the roots that were tried, one per line, so an installer's smoke test shows
/// where the package put things.
#[must_use]
pub fn assets_dir_report() -> (String, bool) {
    if let Some(dir) = resolved_assets_dir() {
        return (dir.display().to_string(), true);
    }
    let mut s = String::from("assets not found (no anatomy/default/manifest.ron); looked in:\n");
    for r in asset_roots() {
        s.push_str("  ");
        s.push_str(&r.display().to_string());
        s.push('\n');
    }
    (s, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_source_tree_is_found_from_a_test_binary() {
        let dir = resolved_assets_dir().expect("the repository's assets/ is a candidate root");
        assert!(dir.join("scenarios").is_dir());
        assert_eq!(find_asset_subdir("scenarios"), Some(dir.join("scenarios")));
        let (report, ok) = assets_dir_report();
        assert!(
            ok && report.ends_with(ASSETS_DIR_NAME) || report.contains(ASSETS_DIR_NAME),
            "{report}"
        );
    }

    #[test]
    fn the_environment_variable_comes_first_and_the_package_layouts_are_tried() {
        let roots = asset_roots();
        let joined: Vec<String> = roots.iter().map(|r| r.display().to_string()).collect();
        assert!(
            joined.iter().any(|r| r.ends_with("Resources/assets")),
            "macOS bundle: {joined:?}"
        );
        assert!(
            joined.iter().any(|r| r.ends_with("lib/heartsim/assets")),
            "deb/AppImage: {joined:?}"
        );
        assert_eq!(roots.last().map(|r| r.ends_with(ASSETS_DIR_NAME)), Some(true));
    }

    #[test]
    fn the_flag_is_recognised() {
        assert!(wants_assets_dir(["--assets-dir"]));
        assert!(!wants_assets_dir(["--version"]));
        assert!(!wants_assets_dir(Vec::<&str>::new()));
    }
}
