use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cargo xtask", about = "Ghost Rust ↔ web workspace tooling")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Generate TypeScript contracts from the runtime's Rust types.
    Bindings,
    /// Run Svelte/TypeScript static validation through Bun.
    WebCheck,
    /// Produce the static SvelteKit bundle consumed by ghost-fl-runtime.
    WebBuild,
    /// Generate contracts, validate TypeScript, and build optimized assets.
    Web,
}

fn main() -> Result<()> {
    let root = workspace_root()?;
    match Cli::parse().command {
        Task::Bindings => bindings(&root),
        Task::WebCheck => bun(&root, "check"),
        Task::WebBuild => bun(&root, "build"),
        Task::Web => {
            bindings(&root)?;
            bun(&root, "check")?;
            bun(&root, "build")
        }
    }
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .context("failed to resolve the Ghost workspace root")
}

fn bindings(root: &Path) -> Result<()> {
    let destination = root.join("web/packages/runtime-contracts/src/index.ts");
    fs::write(&destination, ghost_fl_runtime::typescript_bindings())
        .with_context(|| format!("failed to write {}", destination.display()))?;
    println!("generated {}", destination.display());
    Ok(())
}

fn bun(root: &Path, script: &str) -> Result<()> {
    let status = Command::new("bun")
        .args(["run", "--cwd", "runtime", script])
        .current_dir(root.join("web"))
        .status()
        .with_context(|| format!("failed to start Bun for web:{script}"))?;
    if !status.success() {
        bail!("Bun web:{script} failed with {status}");
    }
    Ok(())
}
