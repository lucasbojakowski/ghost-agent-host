# T02 — Scaffold Architecture Crates

## Dispatch

- Branch from: ADR_SHA
- Parallel work: none
- Produces: SCAFFOLD_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-02-scaffold -b agent/02-scaffold <ADR_SHA>
    Set-Location ..\gha-wt-02-scaffold

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Add compileable placeholder crates that encode the architecture without migrating behavior yet.

## Owned paths

- Root Cargo.toml and Cargo.lock
- crates/ghost-protocol/**
- crates/ghost-app/**
- crates/ghost-ui-model/**
- crates/ghost-editor-api/**
- crates/ghost-daemon-client/**
- crates/ghost-editor-egui/**
- crates/ghost-editor-webview/**

## Required work

1. Add all seven crates to the workspace with minimal library roots and crate documentation.
2. Express only dependency edges approved by the ADR.
3. Keep default features minimal and platform-specific dependencies correctly gated.
4. Add compile-time boundary comments or tests where useful.
5. Do not move existing implementation or design final traits.

## Acceptance

- cargo check --workspace passes.
- cargo test --workspace passes.
- cargo tree shows no concrete UI dependency from ghost-app, ghost-protocol, or ghost-ui-model.
- The WebView crate does not make non-Windows workspace checks impossible.
- Existing binaries and plugin still compile.

## Handoff

Make one commit named chore: scaffold application and editor boundary crates. Return its SHA. The coordinator records it as SCAFFOLD_SHA.
