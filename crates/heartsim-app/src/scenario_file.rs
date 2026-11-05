//! Writing a scenario back to a file (the timeline editor's "Save"). `heartsim_core::Scenario`
//! cannot name the engine sections (layering), so the file is its RON with the `timing` and
//! `morphology` sections appended inside the same `Scenario( … )`, the layout the library files
//! use and [`Tier0Scenario::from_ron`] reads.

use heartsim_ecg::Tier0Scenario;
#[cfg(feature = "native")]
use std::path::PathBuf;

/// The scenario file text of `s`.
///
/// # Errors
/// Only if serialisation itself fails.
pub fn to_ron(s: &Tier0Scenario) -> Result<String, ron::Error> {
    let pretty = || ron::ser::PrettyConfig::default().struct_names(true);
    let body = s.scenario.to_ron()?;
    let body = body.trim_end();
    let body = body.strip_suffix(')').unwrap_or(body).trim_end();
    let sep = if body.ends_with(',') || body.ends_with('(') {
        ""
    } else {
        ","
    };
    let timing = ron::ser::to_string_pretty(&s.timing, pretty())?;
    let morphology = ron::ser::to_string_pretty(&s.morphology, pretty())?;
    Ok(format!(
        "{body}{sep}\n    timing: {timing},\n    morphology: {morphology},\n)\n"
    ))
}

/// Where edited scenarios are saved by default: `<user data dir>/scenarios`.
#[cfg(feature = "native")]
#[must_use]
pub fn user_scenario_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "heartsim").map(|d| d.data_dir().join("scenarios"))
}
