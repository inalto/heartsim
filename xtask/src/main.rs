//! `cargo xtask <task>` — project automation that would otherwise live in shell scripts.
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::process::Command;

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
    }
}
