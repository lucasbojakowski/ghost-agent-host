# T14 — Migrate agentd to Controller and Protocol

## Dispatch

- Branch from: IMPLEMENTATION_SHA
- Parallel with: T13, T17, T18
- Produces: AGENTD_MIGRATED_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-14-agentd-migrate -b agent/14-agentd-migrate <IMPLEMENTATION_SHA>
    Set-Location ..\gha-wt-14-agentd-migrate

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Replace daemon-specific application orchestration with the shared controller and versioned protocol while preserving current endpoint compatibility where practical.

## Owned paths

- agentd crate
- Daemon compatibility tests and documentation

Do not perform the full reliability hardening assigned to T15.

## Required work

- Compose concrete providers and the application controller at daemon startup.
- Decode protocol commands and encode correlated events/results.
- Preserve current endpoint/address configuration and provide a documented compatibility path for existing clients.
- Keep transport mapping separate from application behavior.
- Add fake-provider request/response integration tests.

## Acceptance

- agentd contains no duplicate analysis workflow.
- Protocol errors are separated from application failures.
- Existing startup/listening behavior remains.
- One client can complete current analysis through the versioned protocol.
- Daemon tests and workspace checks pass.

## Handoff

Make one commit named refactor: migrate agentd to shared controller protocol. Return its SHA and compatibility notes. The coordinator records it as AGENTD_MIGRATED_SHA.
