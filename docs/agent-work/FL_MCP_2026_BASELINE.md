# FL Studio MCP 2026 Raw Parity Baseline

Status: **PROVEN FOR MCP/HARNESS INTEROPERABILITY / CONTROL GROUP**

Current app:

```text
apps/ghost-fl-mcp/
```

Accepted runtime status is recorded in `FL_MCP_2026_VALIDATION.md`.

## Purpose

This app answers one narrow question:

> Can an external MCP harness operate Ghost's already-proven raw FL Studio/Gopher capability surface without a Ghost-specific semantic redesign?

The answer is yes for executable/harness/tool interoperability.

The app should therefore remain useful as a raw external-harness control group rather than silently absorbing later scripting or semantic workspace capabilities.

## Architecture

```text
external MCP host / agent
        │
        │ MCP 2026-07-28 over stdio
        ▼
apps/ghost-fl-mcp
        │
        ▼
ghost-fl-studio
        │
        ▼
Gopher / FL Studio
```

The app does not route through `ghost-codex`. The external harness owns the agent loop.

MCP is an edge protocol here, not Ghost's internal bus.

## Protocol target

```text
MCP: 2026-07-28
Rust SDK: rmcp 3.0.1
transport: stdio
```

The app uses the official Rust SDK rather than hand-writing MCP framing.

Protocol-specific extensions such as Tasks, MRTR, Apps, resources/subscriptions and Streamable HTTP are intentionally absent from this raw control group.

## Raw tool parity

At startup:

1. connect `GopherNativeAdapter`;
2. read the live `FlStudioManifest`;
3. convert every live `NativeToolDefinition` into an MCP tool;
4. preserve tool name, description and input schema;
5. sort tools deterministically at the MCP edge;
6. forward `tools/call` to `GopherNativeAdapter::call_native`;
7. preserve native failures as visible failures rather than transport success.

The known Gopher tool count is not hard-coded. The live manifest is authoritative.

`ghost-fl-studio` remains responsible for Gopher-specific invariants such as schema-order canonicalization, callback normalization, native-error detection and single-flight dispatch.

## Safety baseline

The raw Gopher surface contains destructive operations, so the app requires explicit operator acceptance:

```text
--i-accept-live-fl-writes
```

Do not add hidden semantic restrictions to this control group; doing so would invalidate comparisons with the direct raw baseline. Finer product permission policy belongs in later apps.

## What is proven

The 2026-08-25 user-machine acceptance established:

```text
standalone executable build          PASS
external MCP harness connection      PASS
agent tool discovery/use             PASS
live FL tool invocation              PASS
```

The acceptance does not claim every broader benchmark or official conformance scenario was rerun. See `FL_MCP_2026_VALIDATION.md` for the exact scope.

## Historical constraint that is now superseded

The original parity experiment explicitly said not to combine MCP with the independent scripting branch. That restriction applied only while both experiments were being isolated.

The experiments are now independently proven and are being integrated on:

```text
phase/workspace-foundation
```

Do not interpret the old isolation rule as a current architectural prohibition.

The raw `ghost-fl-mcp` app itself should still remain unchanged as a control group. Expanded Gopher + scripting MCP capability belongs in a separate app/experiment.

## Next experiment

The missing harness matrix cell is an external MCP projection of the already-proven expanded FL surface:

```text
complete live Gopher tools
+ fl_scripting_search
+ fl_scripting_describe
+ fl_scripting_call
+ fl_context_snapshot
```

That work should preserve progressive disclosure rather than generating hundreds of scripting tools.

It will also provide evidence for the smallest provider-neutral capability/tool contract worth promoting into Ghost Core.

See `docs/FL_CAPABILITY_SURFACES.md`.

## Ownership rule

The MCP server remains app-owned.

Do not put MCP behavior into `ghost-fl-studio` or `ghost-fl-scripting`. Protocol-only reusable machinery may be promoted into Core only after another MCP app proves real duplication.

## Regression gate

Future raw-MCP changes should preserve:

1. standalone executable build;
2. stdout reserved for MCP stdio protocol traffic;
3. external harness connection;
4. dynamic live-manifest `tools/list`;
5. exact parity name/description/schema where intended;
6. real adapter dispatch and error visibility;
7. explicit live-write acceptance;
8. no `ghost-codex` dependency;
9. no scripting/semantic expansion inside this control app.
