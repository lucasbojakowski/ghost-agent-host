# FL Capability Surfaces

Ghost currently has two independent low-level FL Studio integrations and two proven agent/harness projections over them.

This document describes the current capability topology and the next composition boundary. It is intentionally about **surfaces and projections**, not the future semantic workspace/entity model.

## Lower FL surfaces

### Gopher / `ghost-fl-studio`

`ghost-fl-studio` is the transparent adapter for the live Gopher/CDP surface.

It provides the higher-level FL operations already proven by the raw agent baseline: project/session inspection, transport, channels, mixer routing and effects, playlist metadata, plugin parameters, sequencing and related native operations.

Important invariant:

> The crate mirrors Gopher behavior and integration constraints; it does not encode Ghost product policy.

### MIDI Scripting / `ghost-fl-scripting`

`ghost-fl-scripting` is the transparent adapter for FL Studio MIDI Scripting.

The scripting surface is broader and lower-level than Gopher in several areas, especially current user/workspace context and native state such as selection, focus, pattern/timeline state, project safety/undo primitives, richer mixer state, metering and additional sequencing operations.

The scripting API contains hundreds of functions. The proven app does **not** register every function as an agent tool. Instead it exposes progressive discovery over the checked-in runtime-evidenced catalog.

Important invariant:

> The crate owns FL scripting/runtime/transport facts; it does not decide which scripting primitives an agent should see.

## Proven projection matrix

The experiments now form a useful control matrix:

```text
                         Direct Codex                 External MCP
                         ------------                 ------------
Raw Gopher               ghost-fl-agent               ghost-fl-mcp
                         PROVEN / CONTROL GROUP        PROVEN / CONTROL GROUP

Expanded FL              ghost-fl-workspace           next experiment
Gopher + Scripting        PROVEN                       not yet implemented
```

This matrix is valuable because it lets us distinguish improvements caused by a richer FL surface from improvements caused by a different agent harness/protocol.

Do not silently expand either raw control group:

- `ghost-fl-agent` remains the direct-Codex raw Gopher baseline;
- `ghost-fl-mcp` remains the external-harness raw Gopher MCP baseline.

## Current expanded direct-Codex surface

`ghost-fl-workspace` has already established a practical first composition:

```text
complete live Gopher catalog
+
fl_scripting_search
fl_scripting_describe
fl_scripting_call
+
compact point-in-time FL scripting context before each turn
```

This is intentionally asymmetric.

Gopher is already a small, high-level agent-oriented surface, so every live Gopher tool is registered directly.

MIDI Scripting is much larger, so the agent receives three progressive-disclosure gateways rather than hundreds of generated tools.

The compact context snapshot currently includes project/version state, `safeToEdit`, selected channel/mixer track, pattern state, arrangement selection, focused plugin/window, song position, loop mode and playback state.

The snapshot is evidence, not durable project truth. The app instructs the agent to re-observe when correctness depends on current state.

## What the MCP parity app currently exports

`ghost-fl-mcp` is deliberately narrower:

```text
live Gopher manifest
    -> MCP tools/list
    -> MCP tools/call
```

Its server implementation is Gopher-specific by construction:

- it accepts `FlStudioManifest`;
- it converts `NativeToolDefinition` to MCP `Tool`;
- its caller dispatches to `GopherNativeAdapter`;
- its server instructions describe a raw Gopher surface.

That was correct for the parity experiment. It should remain as the raw MCP control group.

## The missing cell: expanded FL through MCP

The next MCP experiment should reproduce the **capability meaning** of `ghost-fl-workspace` without coupling the workspace to Codex.

A minimal first surface is:

```text
complete live Gopher tools
+
fl_scripting_search
fl_scripting_describe
fl_scripting_call
+
fl_context_snapshot
```

The first three scripting gateways should preserve the same progressive-disclosure behavior already proven with Codex.

`fl_context_snapshot` is recommended for the first MCP version because an MCP server does not control the external harness's turn prompt. The direct Codex app can inject a snapshot before a turn; an external harness needs an explicit capability to obtain the same state.

A read-only context tool is preferable to embedding volatile state in static MCP server instructions.

Later MCP-specific projections may use resources/subscriptions when there is evidence that they improve the product, but they are not required to prove the expanded capability surface.

## Do not expose every scripting function as an MCP tool

The progressive-disclosure result is already useful evidence.

Bad first expansion:

```text
48-ish Gopher tools
+ hundreds of scripting functions
```

Preferred:

```text
Gopher direct operations
+ scripting discovery gateway
+ explicit scripting call escape hatch
+ compact current-context read
```

This keeps model context bounded while preserving access to the full evidenced scripting capability catalog.

## Reusable capability boundary now justified by evidence

The current code reveals one real duplication pressure:

```text
ghost-fl-workspace
    ghost_codex::ToolDefinition + handler

ghost-fl-mcp
    rmcp::Tool + Gopher-specific caller
```

Both consumers need the same conceptual object:

```text
name
summary/description
JSON input schema
call handler
availability/source metadata
```

This is evidence for a future **provider-neutral capability/tool definition** in Core.

However, the next step should not be a universal harness framework. First implement the expanded MCP cell and identify the smallest shared contract that removes actual duplication.

Likely Core-owned pieces after that evidence:

- provider-neutral tool/capability definition;
- JSON-schema-bearing invocation contract;
- normalized tool result/error representation;
- perhaps capability catalogs/search metadata.

App-owned pieces should remain:

- which capabilities are enabled;
- how raw Gopher and scripting are grouped;
- system instructions;
- user/product permissions;
- context snapshot selection;
- skill/intent semantics;
- MCP server configuration and transport policy.

## MCP remains an edge protocol

Even after the expanded MCP experiment, internal applications should continue to compose Rust capabilities directly.

```text
                    capability layer
                      /          \
                     /            \
              Codex projection   MCP projection
```

Avoid introducing MCP between internal layers merely to make the architecture uniform.

`ghost-fl-studio` and `ghost-fl-scripting` must remain protocol-agnostic.

## Relation to the future workspace model

The expanded FL surface is not yet the semantic workspace.

It is the primitive evidence/action substrate from which the first workspace feature slices can be built:

```text
Gopher observations/actions
+
Scripting observations/actions
+
Ghost Tap/audio evidence
        ↓
Entity / Feature / Relation / Binding / Revision / Diff
```

The same workspace capability may later have multiple projections:

- human UI;
- direct agent tools/context;
- MCP tools/resources;
- diagnostics.

That is where the SDK direction and the harness experiments converge.

## Next evaluation

When the expanded MCP cell is implemented, compare at least:

```text
A. ghost-fl-workspace / Codex
B. expanded FL MCP / external harness
```

on tasks that require:

- current FL selection/focus context;
- scripting discovery;
- a higher-level Gopher action;
- live verification.

Measure tool-call count, discovery overhead, correctness, recovery and claim fidelity. Preserve the two raw Gopher control groups so any improvement can be attributed rather than assumed.
