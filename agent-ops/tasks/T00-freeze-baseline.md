# T00 — Freeze the Baseline

## Dispatch

- Branch from: OPS_SHA
- Parallel work: none
- Produces: BASE_SHA

Create and enter the worktree:

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-00-baseline -b agent/00-baseline <OPS_SHA>
    Set-Location ..\gha-wt-00-baseline

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Establish a reproducible, clean baseline for all later agents. Confirm the currently functional CLI, ghost-lab, daemon startup, and packaged Windows x64 CLAP plugin without changing architecture.

## Owned paths

- Root Cargo.toml and Cargo.lock only when required to make the baseline reproducible
- .gitignore and repository-level tooling/configuration
- Existing tests and packaging scripts
- docs/baseline.md

Do not begin the architecture extraction planned by later tasks.

## Required work

1. Record the Rust toolchain, Windows target, build commands, package command, artifact location, and manual FL Studio smoke procedure.
2. Ensure generated build, CLAP bundle, editor asset, log, and local configuration artifacts are ignored unless intentionally versioned.
3. Run the existing workspace checks and repair only baseline regressions.
4. Confirm the package layout for a Windows 11 x64 CLAP bundle.
5. Document known limitations separately from failures.

## Acceptance

- git status is clean after committing.
- cargo fmt --all -- --check passes.
- cargo check --workspace passes.
- Existing workspace tests pass, or an independently reproducible pre-existing failure is documented.
- The CLAP packaging command produces the expected bundle layout.
- docs/baseline.md contains exact commands and results.

## Handoff

Make one commit named chore: freeze reproducible baseline. Return its SHA and the handoff block required by WORKTREE_CONTRACT.md. The coordinator records that commit as BASE_SHA.
