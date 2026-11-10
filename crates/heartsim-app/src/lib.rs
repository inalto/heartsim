//! Application model and eframe `App` implementation. `main.rs` only sets up logging and the
//! native window so the same `HeartSimApp` can be mounted in the web build.

pub mod app;
#[cfg(feature = "native")]
pub mod asset_dirs;
pub mod assets;
#[cfg(feature = "audio")]
pub mod audio_out;
pub mod axis_quiz;
pub mod case_ui;
pub mod disclaimer;
pub mod electrode_tab;
#[cfg(feature = "export")]
pub mod export_panel;
pub mod heart_history;
pub mod heart_view;
pub mod interval_drill;
pub mod monitor_beep;
pub mod responsive;
pub mod rhythm_quiz;
pub mod scenario_file;
pub mod scenarios;
pub mod settings;
pub mod snapshot_ring;
pub mod tutorial;
pub mod tutorial_ui;
pub mod update_check;
#[cfg(feature = "update-check")]
pub mod update_install;
pub mod vector_overlay;
pub use app::{AppStatus, HeartSimApp};
pub use assets::AssetBundle;

/// The line printed by `heartsim --version`; installers' smoke tests assert on the `heartsim <semver>` prefix.
pub fn version_line() -> String {
    format!("heartsim {}", env!("CARGO_PKG_VERSION"))
}

/// True when `args` (without the program name) ask for the version. Handled before any window or GPU
/// initialisation so the flag works on machines without a display.
pub fn wants_version<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|a| matches!(a.as_ref(), "--version" | "-V"))
}

/// `HEARTSIM_WINDOW_SIZE=WIDTHxHEIGHT` (logical points, at least 320 × 240): the window size for
/// this run, for checking the layout on a small screen or taking screenshots at a set size. It
/// wins over the remembered size (the app resizes itself on its first frame) and is not stored.
#[must_use]
pub fn window_size_from_env() -> Option<[f32; 2]> {
    let s = std::env::var("HEARTSIM_WINDOW_SIZE").ok()?;
    let (w, h) = s.split_once(['x', 'X'])?;
    let size = [w.trim().parse::<f32>().ok()?, h.trim().parse::<f32>().ok()?];
    (size[0] >= 320.0 && size[1] >= 240.0).then_some(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_flag_is_recognised() {
        assert!(wants_version(["--version"]));
        assert!(wants_version(["scenario.heartsim.ron", "-V"]));
        assert!(!wants_version(["scenario.heartsim.ron"]));
        assert!(!wants_version(Vec::<String>::new()));
    }

    #[test]
    fn version_line_matches_package_version() {
        assert_eq!(version_line(), format!("heartsim {}", env!("CARGO_PKG_VERSION")));
    }
}
