# T16 — Implement the Typed Daemon Client

## Dispatch

- Branch from: AGENTD_HARDENED_SHA
- Parallel work: none on the same client/protocol surface
- Produces: DAEMON_CLIENT_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-16-daemon-client -b agent/16-daemon-client <AGENTD_HARDENED_SHA>
    Set-Location ..\gha-wt-16-daemon-client

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Implement FrontendApi over the hardened daemon protocol so editor implementations can use remote application services exactly as they would an in-process controller.

## Owned paths

- crates/ghost-daemon-client/**

Only protocol conformance fixtures shared with agentd may be touched outside the crate.

## Required work

- Connect and negotiate protocol version/capabilities.
- Correlate concurrent requests and stream state/progress events.
- Implement cancellation, timeouts, disconnect detection, and bounded reconnect/backoff.
- Publish clear connection-state transitions to the UI model.
- Make shutdown deterministic and never block an editor callback indefinitely.
- Add a fake server and shared client/server conformance tests.

## Acceptance

- Implements FrontendApi without UI toolkit dependencies.
- Version mismatch is explicit and actionable.
- Reconnect cannot duplicate completed commands.
- Out-of-order responses route to the correct request.
- Cancellation works both before and after send.
- cargo test -p ghost-daemon-client passes.

## Handoff

Make one commit named feat: implement typed daemon frontend client. Return its SHA and reconnect/cancellation semantics. The coordinator records it as DAEMON_CLIENT_SHA.
