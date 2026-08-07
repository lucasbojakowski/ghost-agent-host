# T27 — Forward Child State and Latency

## Dispatch

- Branch from: a coordinator checkpoint containing CHILD_AUDIO_SHA and PARAMETER_ADAPTER_SHA
- Parallel work: none on child extension/lifecycle state
- Produces: STATE_LATENCY_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-27-state-latency -b agent/27-state-latency <CHILD_RUNTIME_BASE_SHA>
    Set-Location ..\gha-wt-27-state-latency

The coordinator supplies CHILD_RUNTIME_BASE_SHA after reconciling T25 and T26. Read agent-ops/WORKTREE_CONTRACT.md.

## Objective

Persist opaque child state safely and aggregate/report latency changes to the parent host.

## Owned paths

- Child state persistence module
- Child latency extension handling
- Parent plugin state/latency integration
- Fake-child tests

## Required work

- Save and restore ordered, versioned child descriptors plus opaque state blobs.
- Bound blob sizes and handle missing/incompatible children without corruption.
- Restore mappings/configuration in a defined order relative to child state.
- Query aggregate series latency and notify the parent host when it changes.
- Separate main-thread extension calls from realtime observations.
- Test reload, missing child, corrupt state, reordered chain, and dynamic latency.

## Acceptance

- A save/load round trip restores fake-child behavior.
- Opaque state is not interpreted as internal plugin structure.
- Partial restore reports precise failures and leaves a defined safe chain.
- Parent latency equals the tested aggregate and updates safely.
- State and latency tests pass.

## Handoff

Make one commit named feat: persist child state and forward latency. Return its SHA, state format version, and failure policy. The coordinator records it as STATE_LATENCY_SHA.
