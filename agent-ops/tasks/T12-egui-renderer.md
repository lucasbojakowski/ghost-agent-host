# T12 — Extract a Pure egui Renderer

## Dispatch

- Branch from: CONTRACTS_SHA
- Parallel with: T08, T09, T10, T11
- Produces: one input to IMPLEMENTATION_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-12-egui-renderer -b agent/12-egui-renderer <CONTRACTS_SHA>
    Set-Location ..\gha-wt-12-egui-renderer

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Turn ghost-ui into a pure egui view of UiState that emits UiAction values. Update ghost-lab into a scripted renderer harness.

## Owned paths

- crates/ghost-ui/**
- crates/ghost-lab/**

Do not implement native window lifecycle or daemon networking.

## Required work

- Remove direct Codex, host, database, daemon, and plugin ownership from ghost-ui.
- Render immutable UiState and return actions/effects to the caller.
- Keep egui-only ephemeral view state local and distinguish it from application state.
- Cover all reducer states, including progress, reconnect, failure, and cancellation.
- Add scripted ghost-lab scenarios and screenshot/manual inspection modes.

## Acceptance

- cargo tree -p ghost-ui contains no ghost-codex, ghost-host, ghost-db, or daemon crate.
- Rendering can be tested with scripted state and no running services.
- User interaction produces UiAction values rather than invoking services.
- cargo test -p ghost-ui and cargo test -p ghost-lab pass.

## Handoff

Make one commit named refactor: make egui a pure state renderer. Return its SHA and the lab commands for reviewing every state.
