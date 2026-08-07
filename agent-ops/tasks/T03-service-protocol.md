# T03 — Define the Versioned Service Protocol

## Dispatch

- Branch from: SCAFFOLD_SHA
- Parallel work: none
- Produces: PROTOCOL_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-03-protocol -b agent/03-protocol <SCAFFOLD_SHA>
    Set-Location ..\gha-wt-03-protocol

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Define the transport-neutral, versioned messages shared by daemon clients and servers. The protocol must support the current analysis flow and later realtime capture without depending on a GUI toolkit.

## Owned paths

- crates/ghost-protocol/**

Only minimal workspace manifest updates are allowed.

## Required work

- Define protocol version negotiation and capability reporting.
- Define request IDs, correlation, command envelopes, event envelopes, success payloads, structured failures, progress, and cancellation.
- Model current analysis requests/results using stable domain-shaped DTOs.
- Reserve extensible commands for capture sessions without implementing audio transfer.
- Use serde-compatible tagged representations with documented compatibility rules.
- Add golden JSON fixtures and round-trip tests.
- Avoid transport, filesystem, UI, database, and Codex dependencies.

## Acceptance

- Every request can be correlated with completion, failure, progress, or cancellation.
- Unknown additive fields can be handled according to the documented compatibility policy.
- Golden fixtures are deterministic.
- cargo test -p ghost-protocol passes.
- The crate remains platform-independent and transport-independent.

## Handoff

Make one commit named feat: define versioned ghost service protocol. Return its SHA. The coordinator records it as PROTOCOL_SHA and dispatches T04–T07 from that exact commit.
