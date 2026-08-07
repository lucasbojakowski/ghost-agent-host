# T22 — Add a Realtime-Safe Capture Bridge

## Dispatch

- Branch from: EDITOR_SELECTION_SHA
- Do not overlap plugin implementation with: T24
- Produces: CAPTURE_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-22-capture -b agent/22-capture <EDITOR_SELECTION_SHA>
    Set-Location ..\gha-wt-22-capture

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Transfer bounded audio snapshots from the CLAP process callback to a non-realtime worker without violating realtime constraints. This task does not send audio to the daemon.

## Owned paths

- A dedicated realtime capture module/crate
- Minimal plugin process integration
- Capture stress tests and benchmarks

## Required work

- Preallocate all callback-visible buffers and queue metadata.
- Define start/stop snapshot control using atomics or a demonstrably realtime-safe mechanism.
- Move conversion, aggregation, serialization, logging, and I/O to a worker.
- Track sample rate, channel layout, transport position when available, dropped frames, and overruns.
- Choose and document bounded overflow behavior.
- Add stress tests for variable block sizes, channel counts, repeated activation, overflow, and teardown races.

## Realtime prohibitions

Inside the audio callback: no allocation, blocking locks, file/network I/O, JSON, logging, process creation, or unbounded loops.

## Acceptance

- Tests or instrumentation demonstrate no callback allocations after activation.
- Worker lag has bounded memory and a visible overrun count.
- Activate/deactivate/reset/destroy are race-safe.
- Audio passes through unchanged unless the plugin already has documented processing.
- Stress tests pass under repeated runs.

## Handoff

Make one commit named feat: add realtime-safe audio capture bridge. Return its SHA, buffer sizing rationale, overflow policy, and measurement method. The coordinator records it as CAPTURE_SHA.
