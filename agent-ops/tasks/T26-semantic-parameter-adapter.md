# T26 — Map Semantic Controls to Child Parameters

## Dispatch

- Branch from: CHILD_DISCOVERY_SHA
- May run in parallel with: T25 only when ownership is disjoint
- Produces: PARAMETER_ADAPTER_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-26-parameters -b agent/26-parameters <CHILD_DISCOVERY_SHA>
    Set-Location ..\gha-wt-26-parameters

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Build a runtime capability and parameter adapter that converts stable semantic intentions into validated public CLAP parameter operations. Never depend on guessed private parameter IDs.

## Owned paths

- Dedicated child capability/parameter adapter modules
- Parameter mapping configuration and tests
- Fake-child parameter fixtures

Do not edit realtime audio forwarding files assigned to T25.

## Required work

- Enumerate public parameter metadata and capabilities at runtime.
- Define plugin/version-scoped mappings from semantic controls to public parameter IDs.
- Validate ranges, steps, units, automation flags, and readback.
- Produce timestamped CLAP parameter events through a queue suitable for later realtime consumption.
- Reject missing, ambiguous, stale, or incompatible mappings clearly.
- Add fake plugins and optional observed-data fixtures for supported commercial plugins.

## Acceptance

- No unverified parameter identifier appears in production defaults.
- Unsupported semantic controls fail explicitly.
- Mapping validation detects plugin version/capability drift.
- Event values are clamped/quantized according to live metadata.
- Tests pass without commercial plugins.

## Handoff

Make one commit named feat: add validated semantic parameter adapter. Return its SHA, supported semantic vocabulary, and evidence for any shipped mappings. The coordinator records it as PARAMETER_ADAPTER_SHA.
