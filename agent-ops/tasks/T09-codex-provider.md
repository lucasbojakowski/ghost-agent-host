# T09 — Adapt Codex Behind AgentProvider

## Dispatch

- Branch from: CONTRACTS_SHA
- Parallel with: T08, T10, T11, T12
- Produces: one input to IMPLEMENTATION_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-09-codex-provider -b agent/09-codex-provider <CONTRACTS_SHA>
    Set-Location ..\gha-wt-09-codex-provider

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Make ghost-codex a concrete AgentProvider while preserving existing Codex binary resolution, strict response schema, and app-server behavior.

## Owned paths

- crates/ghost-codex/**

Minimal ghost-app feature/dependency adjustments are allowed only if a contract defect is demonstrated. Do not edit call sites.

## Required work

- Remove any provider trait owned by ghost-codex in favor of the ghost-app port.
- Adapt current request/result types at the crate boundary.
- Preserve explicit binary path, PATH lookup, environment override, diagnostics, and strict schema handling.
- Propagate cancellation and progress where the process protocol permits.
- Ensure child processes and pipes are cleaned up on cancellation, failure, and shutdown.
- Use a fake app-server executable or harness for repeated deterministic tests.

## Acceptance

- ghost-codex implements AgentProvider without depending on GUI or daemon crates.
- Resolver errors identify every attempted source.
- Malformed and schema-invalid responses produce typed failures.
- Repeated fake-server sessions do not leak processes.
- cargo test -p ghost-codex passes.

## Handoff

Make one commit named refactor: adapt codex integration to agent provider. Return its SHA and list preserved configuration inputs.
