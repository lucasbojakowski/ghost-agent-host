# Ghost & Guild SDK Architecture

Ghost & Guild is being developed as an SDK/runtime plus applications built from that SDK.

The repository remains a monorepo while the interfaces are still moving quickly, but the architectural split is explicit:

```text
crates/*  = reusable Core / SDK
apps/*    = product and harness composition
```

## Core / SDK

Core owns mechanisms and integration invariants that have demonstrated reuse.

Current examples:

```text
crates/
  ghost-audio/          deterministic audio representation/analysis primitives
  ghost-tap/            DAW-loaded transparent sensing/capture primitive
  ghost-context/        provider-neutral context types/compiler support
  ghost-codex/          reusable Codex App Server runtime boundary
  ghost-fl-studio/      transparent live Gopher/CDP FL surface
  ghost-fl-scripting/   transparent FL MIDI Scripting surface
  ghost-application/    reserved promotion boundary; do not use as a dumping ground
```

A Core crate should exist because the external/runtime boundary or repeated app use has established a reusable contract, not because an abstraction seems generally useful.

Core should not know which Ghost product is being built.

## Apps

Apps own opinionated composition:

- which lower integrations are active;
- which project state is read or cached;
- which tools are exposed;
- which skills/knowledge are activated;
- product safety/permission policy;
- model/system prompts;
- human UI;
- agent-facing context projection;
- MCP/server/export policy;
- workflow semantics and evaluation.

Current examples:

```text
apps/ghost-fl-agent
    frozen Codex + raw Gopher control group

apps/ghost-fl-workspace
    Codex app composing Gopher + scripting discovery/calls + live context

apps/ghost-fl-mcp
    external-harness MCP projection of the raw Gopher surface

apps/ghost-workflow
    capture -> analysis -> agent -> scoped FL regression workflow
```

## Protocols are projections, not the internal architecture

Codex dynamic tools and MCP tools are two projections over executable Ghost capabilities.

Do not route internal Rust application logic through MCP merely because MCP is supported externally.

Good:

```text
app capability
    ├── Codex tool projection
    ├── MCP tool/resource projection
    └── human UI projection
```

Avoid:

```text
internal Rust -> MCP -> another internal Rust layer
```

MCP server policy remains app-owned. If multiple apps later need the same MCP conversion machinery, protocol-only helpers may be promoted into Core, but `ghost-fl-studio` and `ghost-fl-scripting` must remain protocol-agnostic.

## Human and agent frontends

The Svelte UI is not the only frontend.

A Ghost capability may have several manifestations:

```text
workspace/capability state
      │
      ├── human projection      Svelte/native UI
      ├── agent projection      compact context + tools
      ├── external projection   MCP/API
      └── diagnostic projection CLI/dev UI
```

These projections should share the same underlying identity, validation and operation semantics rather than each reimplementing the domain independently.

## Feature-slice organization in apps

Feature-Sliced Design is a useful app organization strategy because product complexity is vertical.

Conceptually:

```text
entities/
features/
widgets/
pages/
shared/
```

A feature can colocate its app-facing model, deterministic logic, public API and UI/agent projections while depending downward on Core crates.

Do not mirror every feature as a separate Rust crate. Crate boundaries are for hard reuse/runtime boundaries; feature slices are product-capability boundaries.

## Workspace direction

The next product foundation is expected to begin from a small workspace kernel:

```text
Entity
Feature
Relation
Binding
Revision
Diff
```

The kernel should distinguish:

- native/project entities from semantic/user entities;
- observed facts from user-authored intent, measured evidence, agent inference and plans;
- stable Ghost identity from volatile DAW indices;
- state from behavior.

Tool availability can later derive from feature composition rather than from one permanent global registry.

This is a direction, not yet a promoted Core contract. Implement the first slices in an app and promote only the pieces that repeat.

## Promotion rule

Use the same rule throughout the SDK:

> If behavior exists because an external system/runtime behaves that way, it belongs in the integration layer. If behavior exists because Ghost wants to behave that way, it belongs in an app until repeated evidence proves a reusable Core contract.

That rule currently keeps:

- Gopher invariants in `ghost-fl-studio`;
- FL scripting/runtime invariants in `ghost-fl-scripting`;
- agent/tool composition in apps;
- MCP server behavior in apps;
- future intent/skill/product semantics in apps until reuse is demonstrated.
