# T08 — Implement the Application Controller

## Dispatch

- Branch from: CONTRACTS_SHA
- Parallel with: T09, T10, T11, T12
- Produces: one input to IMPLEMENTATION_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-08-controller -b agent/08-controller <CONTRACTS_SHA>
    Set-Location ..\gha-wt-08-controller

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Implement the UI-neutral orchestration layer behind FrontendApi. It coordinates providers and publishes snapshots/events without knowing whether the caller is CLI, egui, Svelte, or a daemon client.

## Owned paths

- crates/ghost-app/src/controller/**
- crates/ghost-app/src/lib.rs exports needed by the controller
- crates/ghost-app/tests/controller/**

Do not edit concrete providers or frontend crates.

## Required work

- Dispatch long-running work away from caller and UI threads.
- Coordinate AgentProvider, RenderProvider, and RepositoryProvider.
- Publish immutable state snapshots and correlated progress/completion/failure events.
- Implement cancellation, timeout, stale-result suppression, and bounded concurrency.
- Make shutdown deterministic.
- Build comprehensive tests with fake providers, including delayed and failing fakes.

## Acceptance

- Calling FrontendApi never blocks on analysis or provider I/O.
- Two requests cannot corrupt each other's state.
- Cancellation and timeout terminate observable work correctly.
- Late provider results cannot replace newer state.
- No concrete UI, daemon transport, database, audio host, or Codex dependency.
- cargo test -p ghost-app passes.

## Handoff

Make one commit named feat: implement ui-agnostic application controller. Return its SHA and describe the worker/runtime choice.
