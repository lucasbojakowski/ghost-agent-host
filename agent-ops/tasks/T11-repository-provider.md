# T11 — Adapt Persistence Behind RepositoryProvider

## Dispatch

- Branch from: CONTRACTS_SHA
- Parallel with: T08, T09, T10, T12
- Produces: one input to IMPLEMENTATION_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-11-repository -b agent/11-repository <CONTRACTS_SHA>
    Set-Location ..\gha-wt-11-repository

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Make ghost-db a concrete RepositoryProvider with safe concurrent access and explicit migrations.

## Owned paths

- crates/ghost-db/**

Do not edit GUI or daemon call sites.

## Required work

- Implement RepositoryProvider over the current storage engine.
- Keep database models private and map them to application/domain types.
- Make initialization and migrations explicit and idempotent.
- Support the controller's concurrency model without blocking unrelated UI state reads.
- Normalize corruption, lock, migration, and serialization errors.
- Add temporary-database tests for concurrent saves/reads and migration replay.

## Acceptance

- No UI, CLAP editor, or daemon dependency.
- A new database and an existing database both initialize predictably.
- Concurrent tests are deterministic and do not share developer data.
- Failed writes do not expose partially persisted records.
- cargo test -p ghost-db passes.

## Handoff

Make one commit named refactor: implement repository provider adapter. Return its SHA and migration compatibility notes.
