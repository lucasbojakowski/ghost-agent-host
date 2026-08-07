# T15 — Harden the Daemon Service

## Dispatch

- Branch from: AGENTD_MIGRATED_SHA
- May run in parallel with: T19 after its separate prerequisites exist
- Produces: AGENTD_HARDENED_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-15-agentd-hardening -b agent/15-agentd-hardening <AGENTD_MIGRATED_SHA>
    Set-Location ..\gha-wt-15-agentd-hardening

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Make agentd reliable enough for a plugin frontend: observable readiness, bounded concurrency, cancellation, timeouts, clean shutdown, and actionable errors.

## Owned paths

- agentd crate
- Daemon operational tests and docs

## Required work

- Add readiness and capability reporting.
- Enforce unique request/session IDs and bounded queues/concurrency.
- Support repeated requests and multiple clients according to an explicit policy.
- Implement protocol cancellation and server-side timeouts.
- Shut down listeners, workers, providers, and child processes gracefully.
- Return typed errors without panics or connection-wide ambiguity.
- Add fake-provider integration tests for disconnects, cancellation races, overload, timeouts, and shutdown.

## Acceptance

- A readiness probe distinguishes listening from operational.
- Overload is rejected predictably rather than exhausting resources.
- Client disconnect cannot orphan unbounded work.
- Shutdown completes within a tested bound.
- Repeated test runs do not leak ports or processes.
- Daemon integration tests pass.

## Handoff

Make one commit named feat: harden agentd lifecycle and concurrency. Return its SHA, operational defaults, and readiness command. The coordinator records it as AGENTD_HARDENED_SHA.
