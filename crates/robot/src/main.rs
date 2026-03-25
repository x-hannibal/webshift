//! robot — internal dev tool for the webgate workspace.
//!
//! Commands:
//!   bump [X.Y.Z]  — update workspace version (auto-increment patch if omitted)
//!   test          — run full test suite (cargo test --workspace)
//!   promote       — build + test + merge dev→main + tag + push + checkout dev
//!   unpromote     — undo the last promote
//!   publish       — cargo publish both crates to crates.io (M5+)
//!
//! Run from the workspace root:
//!   cargo run -p robot -- <command> [args]

mod harness;

use clap::{Parser, Subcommand};
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "robot", about = "webgate workspace dev tool")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Bump the workspace version. Auto-increments patch if no version given.
    Bump {
        /// Explicit version to set (e.g. 0.2.0). Omit to increment Z.
        version: Option<String>,
    },
    /// Run the full test suite (cargo test --workspace).
    Test,
    /// build + test + merge dev→main + tag + push + checkout dev.
    Promote,
    /// Undo the last promote: delete tag locally and remotely, reset main.
    Unpromote,
    /// Publish webgate then webgate-mcp to crates.io (use after promote).
    Publish,
    /// Run the full query pipeline against real services using test.toml config.
    Harness {
        /// The search query to run.
        query: String,
        /// Override backend (default: from test.toml).
        #[arg(short, long)]
        backend: Option<String>,
        /// Number of results per query.
        #[arg(short = 'n', long)]
        num_results: Option<usize>,
        /// Enable verbose output (content previews).
        #[arg(short, long, default_value_t = true)]
        verbose: bool,
    },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("robot error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Cmd::Bump { version } => cmd_bump(version),
        Cmd::Test => cmd_test(),
        Cmd::Promote => cmd_promote(),
        Cmd::Unpromote => cmd_unpromote(),
        Cmd::Publish => cmd_publish(),
        Cmd::Harness {
            query,
            backend,
            num_results,
            verbose,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(harness::run_harness(
                &query,
                backend.as_deref(),
                num_results,
                verbose,
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Version helpers
// ---------------------------------------------------------------------------

fn read_workspace_version() -> Result<String, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string("Cargo.toml")?;
    let doc: toml::Value = content.parse()?;
    let version = doc
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .ok_or("version not found in [workspace.package]")?;
    Ok(version.to_string())
}

fn increment_patch(version: &str) -> Result<String, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err(format!("version must be X.Y.Z, got '{version}'").into());
    }
    let z: u32 = parts[2]
        .parse()
        .map_err(|_| format!("patch component '{}' is not a number", parts[2]))?;
    Ok(format!("{}.{}.{}", parts[0], parts[1], z + 1))
}

fn write_workspace_version(new_version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string("Cargo.toml")?;
    let current = read_workspace_version()?;
    let old = format!("version = \"{}\"", current);
    let new = format!("version = \"{}\"", new_version);
    if !content.contains(&old) {
        return Err(format!("could not find '{old}' in Cargo.toml").into());
    }
    // Replace only the first occurrence (in [workspace.package])
    let updated = content.replacen(&old, &new, 1);
    std::fs::write("Cargo.toml", updated)?;
    Ok(())
}

fn update_readme_version_badge(
    old_version: &str,
    new_version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string("README.md")?;
    // Match the hardcoded "Latest Release" badge: release-vX.Y.Z-purple and /tag/vX.Y.Z
    let old_badge = format!("release-v{}-purple.svg", old_version);
    let new_badge = format!("release-v{}-purple.svg", new_version);
    let old_tag = format!("/releases/tag/v{}", old_version);
    let new_tag = format!("/releases/tag/v{}", new_version);
    if !content.contains(&old_badge) {
        eprintln!("  warning: version badge not found in README.md, skipping");
        return Ok(());
    }
    let updated = content
        .replace(&old_badge, &new_badge)
        .replace(&old_tag, &new_tag);
    std::fs::write("README.md", updated)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Shell helpers
// ---------------------------------------------------------------------------

/// Run a command, streaming stdout/stderr to the terminal.
/// Returns Err if the process exits with non-zero status.
fn run_cmd(program: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let display = format!("{} {}", program, args.join(" "));
    println!("  $ {display}");
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        return Err(format!("command failed: {display}").into());
    }
    Ok(())
}

/// Run a command and capture its stdout as a trimmed string.
fn capture_cmd(program: &str, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new(program).args(args).output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{} {:?} failed: {}", program, args, stderr).into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn cmd_bump(explicit: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let current = read_workspace_version()?;
    let new_version = match explicit {
        Some(v) => v,
        None => increment_patch(&current)?,
    };

    println!("bump: {} → {}", current, new_version);
    write_workspace_version(&new_version)?;
    update_readme_version_badge(&current, &new_version)?;

    // Stage all tracked changes + CHANGELOG + Cargo files + README
    run_cmd("git", &["add", "-u"])?;
    run_cmd("git", &["add", "CHANGELOG.md", "Cargo.toml", "Cargo.lock", "README.md"])?;
    run_cmd(
        "git",
        &[
            "commit",
            "-m",
            &format!("chore(release): bump to {}", new_version),
        ],
    )?;

    println!("bumped to {new_version}");
    Ok(())
}

fn cmd_test() -> Result<(), Box<dyn std::error::Error>> {
    println!("test: running full test suite…");
    run_cmd("cargo", &["test", "--workspace"])?;
    println!("all tests passed ✓");
    Ok(())
}

fn cmd_promote() -> Result<(), Box<dyn std::error::Error>> {
    let version = read_workspace_version()?;
    let tag = format!("v{version}");

    println!("promote: building and testing workspace…");
    run_cmd("cargo", &["build", "--release"])?;
    run_cmd("cargo", &["test"])?;

    println!("promote: merging dev → main, tagging {tag}…");
    run_cmd("git", &["checkout", "main"])?;
    run_cmd(
        "git",
        &["merge", "dev", "--no-ff", "-m", &format!("release: {tag}")],
    )?;
    run_cmd("git", &["tag", &tag])?;
    run_cmd("git", &["push", "origin", "main", "--tags"])?;
    run_cmd("git", &["checkout", "dev"])?;

    println!("promoted {tag} ✓");
    Ok(())
}

fn cmd_unpromote() -> Result<(), Box<dyn std::error::Error>> {
    let tag = capture_cmd("git", &["describe", "--tags", "--abbrev=0"])?;
    if tag.is_empty() {
        return Err("no tags found".into());
    }

    println!("unpromote: reverting {tag}");

    // Delete remote tag
    run_cmd("git", &["push", "origin", &format!(":refs/tags/{tag}")])?;
    // Delete local tag
    run_cmd("git", &["tag", "-d", &tag])?;
    // Reset main to pre-merge
    run_cmd("git", &["checkout", "main"])?;
    run_cmd("git", &["reset", "--hard", "HEAD~1"])?;
    run_cmd("git", &["push", "origin", "main", "--force-with-lease"])?;
    run_cmd("git", &["checkout", "dev"])?;

    println!("unpromoted {tag} ✓");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn increment_patch_basic() {
        assert_eq!(increment_patch("0.1.9").unwrap(), "0.1.10");
    }

    #[test]
    fn increment_patch_from_zero() {
        assert_eq!(increment_patch("1.0.0").unwrap(), "1.0.1");
    }

    #[test]
    fn increment_patch_invalid_format() {
        assert!(increment_patch("1.2").is_err());
    }

    #[test]
    fn increment_patch_non_numeric() {
        assert!(increment_patch("1.2.abc").is_err());
    }

    #[test]
    fn increment_patch_large_number() {
        assert_eq!(increment_patch("0.1.99").unwrap(), "0.1.100");
    }
}

fn cmd_publish() -> Result<(), Box<dyn std::error::Error>> {
    let version = read_workspace_version()?;
    println!("publish: releasing webgate + webgate-mcp v{version} to crates.io");

    run_cmd("cargo", &["publish", "-p", "webgate"])?;

    // Give crates.io index time to propagate before publishing the binary
    println!("waiting 15 s for crates.io index…");
    std::thread::sleep(std::time::Duration::from_secs(15));

    run_cmd("cargo", &["publish", "-p", "webgate-mcp"])?;

    println!("published v{version} ✓");
    Ok(())
}
