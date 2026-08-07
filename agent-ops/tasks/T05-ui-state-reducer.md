# T05 — Build the Pure UI State Reducer

## Dispatch

- Branch from: PROTOCOL_SHA
- Parallel with: T04, T06, T07
- Produces: one input to CONTRACTS_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-05-ui-model -b agent/05-ui-model <PROTOCOL_SHA>
    Set-Location ..\gha-wt-05-ui-model

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Create a deterministic UI model that any renderer can consume and drive without owning application services.

## Owned paths

- crates/ghost-ui-model/**

## Required work

- Define serializable UiState, UiAction, and application event inputs.
- Implement a pure reducer that returns the next state and explicit effects/commands.
- Cover idle, connecting, ready, analyzing, progress, completed, failed, cancelled, and reconnecting states.
- Preserve request IDs so stale events cannot overwrite newer state.
- Define child-plugin presentation state without binding to a particular plugin API.
- Add exhaustive transition and serialization tests.

## Acceptance

- The crate performs no I/O and starts no threads or runtimes.
- No GUI, daemon-client, database, Codex, or CLAP dependencies.
- Given the same state and input, reduction is deterministic.
- Stale and out-of-order events have tested behavior.
- cargo test -p ghost-ui-model passes.

## Handoff

Make one commit named feat: add pure frontend state reducer. Return its SHA and a short state-machine summary.
