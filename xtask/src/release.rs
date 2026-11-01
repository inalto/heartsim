//! Release gating: version consistency between the workspace, `Packager.toml`, `CHANGELOG.md` and the git
//! tag, plus the licence/advisory checks that must pass before an installer is published.
use anyhow::{Context, Result, bail};
use std::{fs, path::Path, process::Command};

/// Everything the version checks look at, gathered from the repository so the checks stay pure.
#[derive(Debug, Clone, Default)]
pub struct ReleaseInputs {
    /// `[workspace.package].version` from the root `Cargo.toml`.
    pub workspace_version: String,
    /// `(name, version)` for member crates that pin a literal version instead of `version.workspace = true`, and for
    /// internal path dependencies in `[workspace.dependencies]` that carry a `version`.
    pub literal_member_versions: Vec<(String, String)>,
    /// `version` in `Packager.toml`.
    pub packager_version: String,
    /// Full text of `CHANGELOG.md`.
    pub changelog: String,
    /// Git tag being released (`v1.2.3` or `refs/tags/v1.2.3`); `None` for a local check.
    pub tag: Option<String>,
    /// Accept an `[Unreleased]` changelog section in place of `[X.Y.Z]` (workflow dry runs).
    pub allow_unreleased: bool,
}

/// Returns one message per inconsistency; empty means the release metadata agrees with itself.
pub fn version_problems(inputs: &ReleaseInputs) -> Vec<String> {
    let version = inputs.workspace_version.as_str();
    let mut problems = Vec::new();

    if !is_installer_safe_semver(version) {
        problems.push(format!(
            "workspace version `{version}` must be MAJOR.MINOR.PATCH with an optional numeric-only pre-release \
             (`0.2.0-1`): the WiX MSI product version cannot carry `-rc.1`-style identifiers"
        ));
    }
    for (name, v) in &inputs.literal_member_versions {
        if v != version {
            problems.push(format!("`{name}` pins version {v}, workspace is {version}"));
        }
    }
    if inputs.packager_version != version {
        problems.push(format!(
            "Packager.toml version is {}, workspace is {version}",
            inputs.packager_version
        ));
    }
    if let Some(tag) = &inputs.tag {
        let bare = tag.strip_prefix("refs/tags/").unwrap_or(tag);
        if bare != format!("v{version}") {
            problems.push(format!(
                "tag `{bare}` does not match workspace version (expected `v{version}`)"
            ));
        }
    }
    match changelog_section(&inputs.changelog, version, inputs.allow_unreleased) {
        None => problems.push(format!(
            "CHANGELOG.md has no `## [{version}]` section{}",
            if inputs.allow_unreleased {
                " or `## [Unreleased]`"
            } else {
                ""
            }
        )),
        Some(body) if body.trim().is_empty() => {
            problems.push(format!("CHANGELOG.md section for {version} is empty"));
        }
        Some(_) => {}
    }
    problems
}

/// MAJOR.MINOR.PATCH, optionally `-N` where N is a number ≤ 65535 (WiX encodes it as the MSI build field).
fn is_installer_safe_semver(v: &str) -> bool {
    let (core, pre) = match v.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (v, None),
    };
    let numeric = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let core_ok = core.split('.').count() == 3 && core.split('.').all(numeric);
    let pre_ok = pre.is_none_or(|p| numeric(p) && p.parse::<u32>().is_ok_and(|n| n <= 65535));
    core_ok && pre_ok
}

/// Body of the changelog section whose heading is `## [<version>]` (any text after the bracket is ignored,
/// so `## [0.2.0] — 2026-11-20` matches). With `allow_unreleased`, the first `## [Unreleased]` also matches.
pub fn changelog_section(changelog: &str, version: &str, allow_unreleased: bool) -> Option<String> {
    let heading_name = |line: &str| -> Option<String> {
        let rest = line.strip_prefix("## [")?;
        Some(rest.split_once(']')?.0.to_owned())
    };
    // The exact version always wins; `[Unreleased]` is only a fallback for dry runs.
    let section = |wanted: &dyn Fn(&str) -> bool| -> Option<String> {
        let mut lines = changelog.lines();
        lines
            .by_ref()
            .find(|line| heading_name(line).is_some_and(|n| wanted(&n)))?;
        let body: Vec<&str> = lines.take_while(|l| !l.starts_with("## ")).collect();
        Some(body.join("\n").trim().to_owned())
    };
    section(&|n| n == version).or_else(|| {
        allow_unreleased
            .then(|| section(&|n| n.eq_ignore_ascii_case("unreleased")))
            .flatten()
    })
}

/// Read the repository state rooted at `root` into [`ReleaseInputs`].
pub fn gather(root: &Path, tag: Option<String>, allow_unreleased: bool) -> Result<ReleaseInputs> {
    let read_toml = |rel: &str| -> Result<toml::Table> {
        let path = root.join(rel);
        fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?
            .parse::<toml::Table>()
            .with_context(|| format!("parsing {}", path.display()))
    };
    let root_toml = read_toml("Cargo.toml")?;
    let workspace_version = root_toml
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(toml::Value::as_str)
        .context("root Cargo.toml has no [workspace.package].version")?
        .to_owned();

    let mut literal_member_versions = Vec::new();
    // Internal path dependencies carry `version = "…"` (cargo-deny rejects wildcard requirements on public crates).
    if let Some(deps) = root_toml
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(toml::Value::as_table)
    {
        for (name, dep) in deps {
            if dep.get("path").is_some()
                && let Some(v) = dep.get("version").and_then(toml::Value::as_str)
            {
                literal_member_versions.push((format!("workspace.dependencies.{name}"), v.to_owned()));
            }
        }
    }
    let mut manifests = vec!["xtask/Cargo.toml".to_owned()];
    for entry in fs::read_dir(root.join("crates")).context("listing crates/")? {
        let entry = entry?;
        if entry.path().join("Cargo.toml").is_file() {
            manifests.push(format!(
                "crates/{}/Cargo.toml",
                entry.file_name().to_string_lossy()
            ));
        }
    }
    manifests.sort();
    for rel in manifests {
        let manifest = read_toml(&rel)?;
        let package = manifest
            .get("package")
            .with_context(|| format!("{rel} has no [package]"))?;
        if let Some(literal) = package.get("version").and_then(toml::Value::as_str) {
            let name = package.get("name").and_then(toml::Value::as_str).unwrap_or(&rel);
            literal_member_versions.push((name.to_owned(), literal.to_owned()));
        }
    }

    let packager_version = read_toml("Packager.toml")?
        .get("version")
        .and_then(toml::Value::as_str)
        .context("Packager.toml has no `version`")?
        .to_owned();
    let changelog = fs::read_to_string(root.join("CHANGELOG.md")).context("reading CHANGELOG.md")?;

    Ok(ReleaseInputs {
        workspace_version,
        literal_member_versions,
        packager_version,
        changelog,
        tag,
        allow_unreleased,
    })
}

fn tool_available(subcommand: &str) -> bool {
    Command::new("cargo")
        .args([subcommand, "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Options for [`release_check`].
#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    pub tag: Option<String>,
    pub allow_unreleased: bool,
    /// Missing `cargo-deny` / `cargo-about` is an error instead of a skipped step (used in CI).
    pub require_tools: bool,
}

/// Version consistency, `cargo deny check`, and a dry licence-report generation.
pub fn release_check(root: &Path, opts: &CheckOptions) -> Result<()> {
    let inputs = gather(root, opts.tag.clone(), opts.allow_unreleased)?;
    let problems = version_problems(&inputs);
    if !problems.is_empty() {
        bail!(
            "release metadata is inconsistent:\n  - {}",
            problems.join("\n  - ")
        );
    }
    eprintln!(
        "version {} is consistent (workspace, Packager.toml, CHANGELOG, tag)",
        inputs.workspace_version
    );

    if tool_available("deny") {
        crate::run(&["deny", "check"])?;
    } else if opts.require_tools {
        bail!("cargo-deny is required for release-check (cargo install cargo-deny --locked)");
    } else {
        eprintln!("cargo-deny not installed; skipping licence/advisory check");
    }

    if tool_available("about") {
        let out = root.join("target").join("THIRD_PARTY_LICENSES.html");
        fs::create_dir_all(root.join("target"))?;
        crate::run(&[
            "about",
            "generate",
            "about.hbs",
            "--output-file",
            &out.to_string_lossy(),
        ])?;
    } else if opts.require_tools {
        bail!("cargo-about is required for release-check (cargo install cargo-about --locked)");
    } else {
        eprintln!("cargo-about not installed; skipping licence report");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> ReleaseInputs {
        ReleaseInputs {
            workspace_version: "0.2.0".into(),
            literal_member_versions: vec![],
            packager_version: "0.2.0".into(),
            changelog: "# Changelog\n\n## [Unreleased]\n- later\n\n## [0.2.0] — 2026-11-20\n### Added\n- thing\n\n## [0.1.0]\n- old\n"
                .into(),
            tag: Some("v0.2.0".into()),
            allow_unreleased: false,
        }
    }

    #[test]
    fn consistent_release_passes() {
        assert_eq!(version_problems(&good()), Vec::<String>::new());
    }

    #[test]
    fn tag_not_matching_workspace_version_fails() {
        let mut i = good();
        i.tag = Some("v0.2.1".into());
        let p = version_problems(&i);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("does not match workspace version"));
    }

    #[test]
    fn full_ref_tag_is_accepted() {
        let mut i = good();
        i.tag = Some("refs/tags/v0.2.0".into());
        assert_eq!(version_problems(&i), Vec::<String>::new());
    }

    #[test]
    fn packager_and_member_version_mismatch_fail() {
        let mut i = good();
        i.packager_version = "0.1.0".into();
        i.literal_member_versions = vec![("heartsim-sim".into(), "0.3.0".into())];
        let p = version_problems(&i);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(p.iter().any(|m| m.contains("Packager.toml")));
        assert!(p.iter().any(|m| m.contains("heartsim-sim")));
    }

    #[test]
    fn changelog_without_release_section_fails_unless_unreleased_allowed() {
        let mut i = good();
        i.workspace_version = "0.3.0".into();
        i.packager_version = "0.3.0".into();
        i.tag = None;
        let p = version_problems(&i);
        assert!(p.iter().any(|m| m.contains("no `## [0.3.0]` section")), "{p:?}");
        i.allow_unreleased = true;
        assert_eq!(version_problems(&i), Vec::<String>::new());
    }

    #[test]
    fn empty_changelog_section_fails() {
        let mut i = good();
        i.changelog = "## [0.2.0]\n\n## [0.1.0]\n- old\n".into();
        assert!(version_problems(&i).iter().any(|m| m.contains("is empty")));
    }

    #[test]
    fn msi_incompatible_prerelease_is_rejected() {
        for bad in ["0.2", "0.2.0-rc.1", "0.2.0-70000", "v0.2.0"] {
            assert!(!is_installer_safe_semver(bad), "{bad}");
        }
        for ok in ["0.2.0", "1.10.3-4"] {
            assert!(is_installer_safe_semver(ok), "{ok}");
        }
    }

    #[test]
    fn section_extraction_stops_at_next_heading() {
        let body = changelog_section(&good().changelog, "0.2.0", false).unwrap();
        assert_eq!(body, "### Added\n- thing");
    }

    #[test]
    fn exact_version_section_beats_empty_unreleased() {
        let log = "# Changelog\n\n## [Unreleased]\n\n## [1.2.0] — 2026-01-01\n### Added\n- thing\n\n## [1.1.0]\n- old\n";
        assert_eq!(
            changelog_section(log, "1.2.0", true).as_deref(),
            Some("### Added\n- thing")
        );
        assert_eq!(changelog_section(log, "1.3.0", true).as_deref(), Some(""));
        assert_eq!(changelog_section(log, "1.3.0", false), None);
    }

    #[test]
    fn repository_metadata_is_currently_readable() {
        // Guards the file layout `gather` depends on; the version itself may legitimately be un-released.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let inputs = gather(&root, None, true).unwrap();
        assert_eq!(inputs.packager_version, inputs.workspace_version);
        assert!(
            inputs
                .literal_member_versions
                .iter()
                .all(|(_, v)| *v == inputs.workspace_version)
        );
        assert!(
            version_problems(&inputs).is_empty(),
            "{:?}",
            version_problems(&inputs)
        );
    }
}
