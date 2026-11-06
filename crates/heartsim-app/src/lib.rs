//! Application model and eframe `App` implementation. `main.rs` only sets up logging and the
//! native window so the same `HeartSimApp` can be mounted in the web build.

pub mod app;
pub mod assets;
pub mod case_ui;
pub mod electrode_tab;
#[cfg(feature = "export")]
pub mod export_panel;
pub mod heart_history;
pub mod heart_view;
pub mod rhythm_quiz;
pub mod scenario_file;
pub mod scenarios;
pub mod settings;
pub mod snapshot_ring;
pub mod tutorial;
pub mod tutorial_ui;
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
