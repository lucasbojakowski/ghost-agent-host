# T04 — Define Application Provider Ports

## Dispatch

- Branch from: PROTOCOL_SHA
- Parallel with: T05, T06, T07
- Produces: one input to CONTRACTS_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-04-app-ports -b agent/04-app-ports <PROTOCOL_SHA>
    Set-Location ..\gha-wt-04-app-ports

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Define stable, UI-agnostic application interfaces for agent execution, result rendering, persistence, and frontend control.

## Owned paths

- crates/ghost-app/src/ports/**
- crates/ghost-app/src/error.rs
- crates/ghost-app/src/lib.rs
- crates/ghost-app/tests/compile_boundaries.rs

Do not implement concrete providers or the controller.

## Required work

- Define AgentProvider, RenderProvider, and RepositoryProvider.
- Define FrontendApi as the UI-facing command/query/event surface.
- Define typed application errors, cancellation tokens, progress events, and correlation types.
- Choose concurrency bounds that allow native async or worker-thread adapters without forcing a UI runtime.
- Document thread-safety and ownership guarantees.
- Add compile tests using minimal fake implementations.

## Acceptance

- Ports expose no egui, baseview, WebView, Svelte, CLAP, database-driver, or Codex-process types.
- FrontendApi can be implemented both in-process and over the daemon protocol.
- Cancellation and progress are part of contracts rather than globals.
- cargo test -p ghost-app passes.

## Handoff

Make one commit named feat: define application provider ports. Return its SHA and note any assumptions the reconciliation agent must align with T05–T07.
