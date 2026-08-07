# T23 — Route Capture Sessions Through agentd

## Dispatch

- Branch from: CAPTURE_SHA reconciled with the latest daemon/client line
- Parallel work: none on capture protocol or plugin integration
- Produces: CAPTURE_FLOW_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-23-capture-flow -b agent/23-capture-flow <CAPTURE_DAEMON_BASE_SHA>
    Set-Location ..\gha-wt-23-capture-flow

The coordinator supplies CAPTURE_DAEMON_BASE_SHA containing CAPTURE_SHA and DAEMON_CLIENT_SHA. Read agent-ops/WORKTREE_CONTRACT.md.

## Objective

Implement start, stop, cancel, transfer, analysis, and result delivery for captured audio while keeping all serialization and transport outside the realtime callback.

## Owned paths

- Capture-related ghost-protocol messages
- ghost-daemon-client capture transport
- agentd capture handlers
- Plugin capture worker integration
- End-to-end fake-provider tests

## Required work

- Finalize versioned capture session commands and correlated events.
- Transfer bounded audio chunks or completed snapshots from the worker.
- Validate format, size, sequence, and session ownership server-side.
- Publish transfer and analysis progress separately.
- Handle cancellation, disconnect, partial transfer, timeout, and daemon restart.
- Add a full plugin-worker/client/server/controller fake-provider test.

## Acceptance

- No network or serialization enters the audio callback.
- A capture is exactly associated with one session/request.
- Partial or duplicated chunks cannot silently produce an analysis.
- Cancellation cleans both client and server state.
- End-to-end tests cover success and all named failure modes.

## Handoff

Make one commit named feat: add daemon-backed audio capture flow. Return its SHA, transfer bounds, and end-to-end test command. The coordinator records it as CAPTURE_FLOW_SHA.
