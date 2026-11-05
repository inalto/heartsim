//! `cargo xtask <task>` — project automation that would otherwise live in shell scripts.
mod bake;
mod icons;
mod release;
mod web;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Parser, Debug)]
#[command(about = "heartsim developer tasks")]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand, Debug)]
enum Task {
    /// Run the same checks as CI: fmt, clippy (deny warnings), tests, deny.
    Ci,
    /// Format all crates.
    Fmt,
    /// Render `assets/icon.svg` into PNG/ICO/ICNS under `assets/generated/icons/` for cargo-packager.
    Icons,
    /// Write `THIRD_PARTY_LICENSES.html` with cargo-about (needs `cargo install cargo-about --locked`).
    Licenses,
    /// Everything cargo-packager needs beforehand: icons and the licence report (its `beforePackagingCommand`).
    PreparePackaging,
    /// Print the workspace version.
    Version,
    /// Verify workspace, Packager.toml, CHANGELOG and tag agree, then run `cargo deny` and the licence report.
    ReleaseCheck(ReleaseCheckArgs),
    /// Print the CHANGELOG section for the workspace version (the GitHub Release body).
    ReleaseNotes(ReleaseNotesArgs),
    /// Build an anatomy pack (procedural by default; `--source` for an open-licence mesh).
    BakeAnatomy(bake::BakeArgs),
    /// Write `heartsim-assets.zip` (anatomy pack, rhythms, tutorials) for the browser build; trunk's `pre_build` hook.
    WebAssets(web::AssetsArgs),
    /// Fail if the `trunk build` output (wasm + asset archive) is over the 25 MiB budget.
    WebBudget(web::BudgetArgs),
}

#[derive(Args, Debug)]
struct ReleaseCheckArgs {
    /// Git tag being released, e.g. `v0.2.0` (`refs/tags/v0.2.0` is accepted).
    #[arg(long)]
    tag: Option<String>,
    /// Accept an `[Unreleased]` CHANGELOG section instead of `[X.Y.Z]` (workflow dry runs).
    #[arg(long)]
    allow_unreleased: bool,
    /// Fail when `cargo-deny` or `cargo-about` is missing instead of skipping that step.
    #[arg(long)]
    require_tools: bool,
}

#[derive(Args, Debug)]
struct ReleaseNotesArgs {
    /// Accept an `[Unreleased]` CHANGELOG section instead of `[X.Y.Z]`.
    #[arg(long)]
    allow_unreleased: bool,
}

fn run(args: &[&str]) -> Result<()> {
    eprintln!("$ cargo {}", args.join(" "));
    let status = Command::new("cargo")
        .args(args)
        .status()
        .context("spawning cargo")?;
    if !status.success() {
        bail!("`cargo {}` failed", args.join(" "));
    }
    Ok(())
}

fn licenses(root: &Path) -> Result<()> {
    let out = root.join("THIRD_PARTY_LICENSES.html");
    run(&[
        "about",
        "generate",
        "about.hbs",
        "--output-file",
        &out.to_string_lossy(),
    ])
}

/// Repository root: xtask lives one level below it.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn main() -> Result<()> {
    match Cli::parse().task {
        Task::Ci => {
            run(&["fmt", "--all", "--check"])?;
            run(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
            run(&["test", "--workspace"])?;
            if Command::new("cargo")
                .args(["deny", "--version"])
                .output()
                .is_ok_and(|o| o.status.success())
            {
                run(&["deny", "check"])?;
            } else {
                eprintln!("cargo-deny not installed; skipping licence/advisory check");
            }
            Ok(())
        }
        Task::Fmt => run(&["fmt", "--all"]),
        Task::Icons => {
            let root = repo_root();
            icons::run(
                &root.join("assets/icon.svg"),
                &root.join("assets/generated/icons"),
            )
        }
        Task::Licenses => licenses(&repo_root()),
        Task::PreparePackaging => {
            let root = repo_root();
            icons::run(
                &root.join("assets/icon.svg"),
                &root.join("assets/generated/icons"),
            )?;
            licenses(&root)
        }
        Task::Version => {
            println!("{}", release::gather(&repo_root(), None, true)?.workspace_version);
            Ok(())
        }
        Task::ReleaseCheck(args) => release::release_check(
            &repo_root(),
            &release::CheckOptions {
                tag: args.tag,
                allow_unreleased: args.allow_unreleased,
                require_tools: args.require_tools,
            },
        ),
        Task::ReleaseNotes(args) => {
            let inputs = release::gather(&repo_root(), None, args.allow_unreleased)?;
            let Some(notes) = release::changelog_section(
                &inputs.changelog,
                &inputs.workspace_version,
                args.allow_unreleased,
            ) else {
                bail!("CHANGELOG.md has no section for {}", inputs.workspace_version);
            };
            println!("{notes}");
            Ok(())
        }
        Task::BakeAnatomy(args) => bake::run(&args),
        Task::WebAssets(args) => web::assets(&repo_root(), &args),
        Task::WebBudget(args) => web::budget(&repo_root(), &args),
    }
}
