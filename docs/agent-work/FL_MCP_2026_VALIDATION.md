# FL MCP 2026 Validation

Status for `feat/fl-mcp-2026`.

This document records the accepted runtime baseline for the first standards-based MCP export of the FL Studio/Gopher surface.

## Accepted baseline

```text
Status: PROVEN FOR MCP/HARNESS INTEROPERABILITY
Validated code commit: 2da835743a20a3a4ed95d6392e8b2b705556df65
Validation date: 2026-08-25
Protocol target: MCP 2026-07-28
Transport: stdio
Rust MCP SDK: rmcp 3.0.1
```

The user-machine validation established that:

- `ghost-fl-mcp` builds successfully as an executable;
- an external MCP-capable harness can launch/connect to the executable;
- the agent receives the MCP tool surface;
- the agent can invoke the exported FL tools successfully against the live integration.

This proves the architectural question of the parity experiment: the existing Ghost FL/Gopher capability surface can be projected through MCP and used by an external harness without routing through `ghost-codex`.

## What is proven

The live-proven path is:

```text
external MCP harness / agent
        │
        │ MCP 2026-07-28 over stdio
        ▼
apps/ghost-fl-mcp
        │
        │ dynamic tools/list + tools/call
        ▼
ghost-fl-studio
        │
        ▼
Gopher / FL Studio
```

The implementation remains driven by the live `FlStudioManifest`. It does not statically recreate the known Gopher tools.

The MCP edge preserves the Gopher tool name, description and input schema, uses deterministic tool ordering, and forwards calls to `GopherNativeAdapter::call_native`. `ghost-fl-studio` therefore remains responsible for the real FL/Gopher invariants such as live schema lookup, argument canonicalization, result normalization, native-error detection and single-flight dispatch.

## What this acceptance does not claim

The 2026-08-25 report proves executable + harness + agent/tool interoperability. It does **not** by itself record completion of every broader parity benchmark originally proposed in `FL_MCP_2026_BASELINE.md`.

In particular, the acceptance report did not separately record:

- a full `setup-benchmark-session.md` run through the MCP harness;
- a tool-by-tool count comparison against the live Gopher manifest;
- official MCP conformance-suite results;
- performance comparison versus the direct Codex baseline;
- scripting-surface export through MCP.

Those remain separate evaluation questions rather than blockers for accepting the MCP edge as functional.

## Static validation status

The validated executable was built successfully on the user machine. The GitHub connector did not find a pull-request-triggered CI workflow run attached to commit `2da835743a20a3a4ed95d6392e8b2b705556df65`, so this record does not invent a repository CI pass for that exact commit.

When this branch is integrated with the current workspace, the combined branch should run the normal repository matrix:

```text
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The existing app-local MCP tests should remain green for deterministic ordering, schema preservation, dynamic dispatch, result/error mapping and dependency isolation.

## Architectural conclusion

MCP is proven as an **edge projection / harness protocol**, not as Ghost's internal bus.

The useful separation is:

```text
Core / SDK primitives
    own reusable capabilities and integration invariants

Apps
    decide which capabilities are exposed through MCP,
    how they are grouped, and which product policies apply
```

The first MCP implementation remains app-owned. Reusable MCP export machinery may be promoted into Core only after the merged Gopher+scripting workspace reveals which pieces are protocol invariants versus FL/app policy.

## Regression gate

Future MCP refactors should preserve at minimum:

1. `ghost-fl-mcp` builds as a standalone executable;
2. stdout remains reserved for MCP stdio protocol traffic;
3. an external harness can launch/connect;
4. `tools/list` is generated from the live FL capability source rather than a stale static copy;
5. tool name/description/schema are preserved exactly where parity is intended;
6. `tools/call` reaches the real adapter and returns visible native failures as failures;
7. the coarse live-write opt-in remains explicit for raw mutation surfaces;
8. no accidental dependency on `ghost-codex` is introduced;
9. MCP remains an app/edge concern unless reuse is explicitly promoted.

## Result record

```text
Date: 2026-08-25
Validated code commit: 2da835743a20a3a4ed95d6392e8b2b705556df65
Executable build: PASS
External harness connection: PASS
Agent tool discovery/use: PASS
Live FL tool invocation: PASS
Full benchmark-session parity: NOT RE-RECORDED IN THIS ACCEPTANCE
Official MCP conformance: NOT CLAIMED
Overall status: PROVEN FOR MCP/HARNESS INTEROPERABILITY
```

This commit is the MCP parity baseline to preserve during the next integration phase.