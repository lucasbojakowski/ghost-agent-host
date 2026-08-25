# Proven Baselines

This is the canonical index of runtime-proven Ghost & Guild integration baselines.

The repository contains historical experiment plans and implementation prompts. Those are useful evidence, but this file is the shortest source of truth for what has actually crossed a validation gate.

## Status vocabulary

- **PROVEN** — exercised successfully against the real target runtime or external harness and accepted as a regression baseline.
- **CONTROL GROUP** — intentionally preserved behavior used to compare later abstractions.
- **EXPERIMENTAL** — implemented but not yet accepted as a durable baseline.
- **HISTORICAL** — useful investigation/evidence that has been superseded by a promoted implementation.

## Raw FL / Gopher agent

```text
Status: PROVEN / CONTROL GROUP
Branch: feat/general-fl-agent-phase1
Baseline head: 21ab2ebbb347b12e122e3aa9ff0174ffcc835e09
```

The raw baseline is:

```text
live Gopher manifest
+ ghost-fl-studio
+ persistent Codex thread
+ minimal live-state instructions
```

`apps/ghost-fl-agent` must remain the Gopher-only behavioral control group. Its Codex tool registry is the complete live Gopher catalog and must not silently absorb scripting or semantic tools.

## FL MIDI Scripting transport investigation

```text
Status: PROVEN / HISTORICAL
Branch: feat/fl-scripting-bridge
Live-proven code commit: b38f1810fd2fd5b48ece57cccb66cac2790304a9
```

This investigation established the unusual FL Python runtime boundary and the working native transport:

```text
FL MIDI controller script
  -> subinterpreter-compatible CPython .pyd
  -> native nonblocking WinSock
  -> loopback NDJSON
  -> Rust listener
```

The detailed failure/success path is preserved in `docs/FL_SCRIPTING_JOURNEY.md`.

## Promoted FL Scripting Framework

```text
Status: PROVEN
Branch: feat/fl-scripting-framework
Validated code commit: 9dc510cf4ede8ab50d860e8e3d2c1aa4e832d84d
Baseline-record commit: 1e62630e718fda4e0f5fc189eb2f5af5cae0557e
GitHub Actions run: 32828978193
Validation date: 2026-08-25
```

The accepted framework provides:

- reusable `crates/ghost-fl-scripting`;
- live FL context through MIDI Scripting;
- deterministic scripting catalog search/describe/call support;
- successful hybrid agent behavior using Gopher + Scripting in the same FL session.

CI passed Rust fmt/check/test/clippy on Linux, macOS and Windows plus the Windows Python syntax/native-extension rebuild gate.

See `docs/agent-work/FL_SCRIPTING_FRAMEWORK_VALIDATION.md`.

## FL MCP 2026 raw parity edge

```text
Status: PROVEN FOR MCP/HARNESS INTEROPERABILITY
Branch: feat/fl-mcp-2026
Validated code commit: 2da835743a20a3a4ed95d6392e8b2b705556df65
Baseline-record commit: 8482e341f1c6e1fca05ce69c03ed78bd9ca54e24
Protocol: MCP 2026-07-28
Transport: stdio
Validation date: 2026-08-25
```

The accepted runtime evidence establishes that:

- `ghost-fl-mcp` builds as an executable;
- an external MCP harness can launch/connect to it;
- an agent receives and successfully uses the exported FL/Gopher tools.

This proves MCP as a functional edge/harness projection. It does not claim that every broad benchmark or official conformance scenario has been rerun.

See `docs/agent-work/FL_MCP_2026_VALIDATION.md`.

## Current integration phase

```text
Branch: phase/workspace-foundation
Status: EXPERIMENTAL INTEGRATION SPINE
Base: promoted scripting-framework baseline
Second source: MCP 2026 baseline
```

This branch exists to assemble accepted primitives without changing the meaning of the frozen experiments. Its immediate architecture question is how the expanded FL surface — live Gopher primitives + scripting discovery/calls + compact current context — should be projected to different harnesses and later into workspace entities/features.

Do not treat the phase branch itself as proven until its combined static/live gates are run.

## Regression principle

When a lower layer is refactored, validate against the nearest accepted baseline rather than against historical implementation intent.

In particular:

- Gopher changes regress against Raw FL Baseline v1;
- scripting changes regress against the promoted scripting-framework baseline;
- MCP edge changes regress against the executable/harness baseline;
- later semantic/workspace tooling should preserve access to the raw control groups so improvements can be measured rather than assumed.
