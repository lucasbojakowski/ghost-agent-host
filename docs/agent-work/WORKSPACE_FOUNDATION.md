# Workspace Foundation Phase

Branch: `phase/workspace-foundation`

Status: **EXPERIMENTAL INTEGRATION SPINE**

This branch assembles the currently accepted FL integration/harness baselines before the first semantic workspace feature slices are implemented.

## Inputs

### Promoted FL Scripting Framework

```text
validated code: 9dc510cf4ede8ab50d860e8e3d2c1aa4e832d84d
baseline record: 1e62630e718fda4e0f5fc189eb2f5af5cae0557e
CI run: 32828978193
status: PROVEN
```

This provides the reusable `ghost-fl-scripting` crate and the live-proven `ghost-fl-workspace` direct-Codex composition.

### MCP 2026 raw parity edge

```text
validated code: 2da835743a20a3a4ed95d6392e8b2b705556df65
baseline record: 8482e341f1c6e1fca05ce69c03ed78bd9ca54e24
status: PROVEN FOR MCP/HARNESS INTEROPERABILITY
```

This provides the app-owned `ghost-fl-mcp` stdio server over the raw Gopher capability surface.

## Why integrate now

The main architectural uncertainty has shifted.

Previously we needed to prove:

- direct Gopher control;
- reusable Gopher adapter behavior;
- FL scripting transport feasibility;
- scripting discovery/calls;
- combined Gopher + scripting agent behavior;
- external MCP harness interoperability.

Those questions now have runtime evidence.

The next question is how Ghost represents and progressively exposes a live music-production workspace to both humans and agents.

## Current app/control matrix

```text
                         Direct Codex                 External MCP
Raw Gopher               ghost-fl-agent               ghost-fl-mcp
                         frozen control               frozen control

Expanded FL              ghost-fl-workspace           next experiment
Gopher + Scripting        proven
```

The phase branch should preserve all three proven applications while exploring the missing expanded-MCP cell and then the workspace kernel.

## Immediate integration rules

1. Do not merge this phase into the repository default branch merely to normalize history.
2. Preserve exact validated commits in `docs/PROVEN_BASELINES.md`.
3. Keep raw control-group apps behaviorally stable.
4. Reconcile the combined Cargo workspace/lockfile and run the normal CI matrix before calling this phase green.
5. MCP remains app-owned; no new generic MCP crate is justified yet.
6. `ghost-fl-studio` and `ghost-fl-scripting` remain transparent/protocol-agnostic integrations.
7. New semantic workspace abstractions begin in apps and are promoted only after reuse evidence.

## MCP analysis result

The current MCP implementation exports `FlStudioManifest` directly, while `ghost-fl-workspace` builds a richer surface from Gopher plus three scripting gateways and a current-context snapshot.

The next MCP slice should therefore test:

```text
raw Gopher tools
+ fl_scripting_search
+ fl_scripting_describe
+ fl_scripting_call
+ fl_context_snapshot
```

without modifying the raw `ghost-fl-mcp` control group in place.

That experiment will provide the first concrete evidence for a provider-neutral capability/tool contract shared by Codex and MCP projections.

See `docs/FL_CAPABILITY_SURFACES.md`.

## Workspace kernel direction

After the integration branch is clean, the first semantic feature slice should be built around a deliberately small app-owned kernel:

```text
Entity
Feature
Relation
Binding
Revision
Diff
```

Important initial rules:

- entities carry stable Ghost identity;
- native DAW indices belong to bindings/observations, not permanent identity;
- features are typed state/evidence, not behavior containers;
- relations represent cross-entity facts/intent;
- provenance distinguishes observed, measured, user-authored, agent-derived and planned state;
- revision/diff infrastructure exists from the first slice;
- tools are deterministic operations over available features/bindings;
- UI, agent context and MCP are projections of the same capability/state rather than separate domain models.

The first implementation should remain app-local until repeated slices reveal a stable Core contract.

## Documentation source of truth

For current work, prefer:

- `docs/PROVEN_BASELINES.md` — what is actually proven;
- `docs/SDK_ARCHITECTURE.md` — Core/SDK vs app ownership;
- `docs/FL_CAPABILITY_SURFACES.md` — FL + harness surface composition;
- this document — current phase scope.

Historical retrospectives, journeys and experiment briefs remain useful evidence but should not override these current documents.

## Exit gate

This phase is ready for the first workspace/entity feature branch when:

- MCP app and scripting framework coexist in one Cargo workspace;
- combined lockfile is generated and committed;
- repository fmt/check/test/clippy matrix passes;
- scripting native build gate remains green;
- raw control groups remain intact;
- current docs no longer instruct agents to follow superseded experiment architecture;
- expanded MCP design is recorded without prematurely implementing a generic harness SDK.
