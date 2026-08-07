# T25 — Forward Audio and Events to Child Plugins

## Dispatch

- Branch from: CHILD_DISCOVERY_SHA
- T26 may run in parallel only with coordinator-confirmed disjoint files
- Produces: CHILD_AUDIO_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-25-child-audio -b agent/25-child-audio <CHILD_DISCOVERY_SHA>
    Set-Location ..\gha-wt-25-child-audio

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Host activated child effects in series and forward audio, parameter/note events, transport, and lifecycle calls safely through the parent CLAP plugin.

## Owned paths

- Child-host realtime processing and event modules
- Parent plugin audio-port and processing integration
- Fake-child conformance tests

Avoid parameter-semantic mapping files reserved for T26.

## Required work

- Negotiate compatible audio port/channel layouts.
- Activate/start/process/stop/deactivate children in correct order.
- Forward audio and supported input events; collect and expose output events safely.
- Handle bypass and child processing status.
- Preallocate conversion/routing storage and establish realtime-safe failure behavior.
- Add fake children that mutate audio, emit events, sleep/fail, and report lifecycle calls.

## Acceptance

- No allocation, blocking lock, scanning, loading, logging, or I/O in process().
- Series ordering is proven by deterministic fake-child output.
- Variable block sizes and supported channel layouts work.
- Child failure cannot unwind across the host ABI.
- Lifecycle ordering and teardown races are tested.

## Handoff

Make one commit named feat: forward realtime audio through child clap chain. Return its SHA, supported layouts, and realtime audit. The coordinator records it as CHILD_AUDIO_SHA.
