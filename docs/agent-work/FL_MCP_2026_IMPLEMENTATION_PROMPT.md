# Implementation prompt — Raw FL baseline over MCP 2026-07-28

You are implementing an MCP export of Ghost & Guild's proven raw FL Studio/Gopher baseline.

Repository:

```text
lucasbojakowski/ghost-agent-host
```

Work branch:

```text
feat/fl-mcp-2026
```

Do not work from another feature/fix branch. Do not merge or close existing PRs, delete remote branches, or change unrelated architecture.

This is an implementation task, not a planning exercise.

## Read first

Before touching code, read these files in full from this branch:

- `README.md`
- `apps/ghost-fl-agent/README.md`
- `docs/TECHNICAL_RETROSPECTIVE.md`
- `docs/WORKSPACE_MIGRATION_PLAN.md`
- `docs/decisions/001-transparent-fl-studio-adapter.md`
- `docs/agent-work/FL_MCP_2026_BASELINE.md`
- `crates/ghost-fl-studio/src/lib.rs`
- `crates/ghost-fl-studio/src/adapter.rs`

Also inspect `apps/ghost-fl-agent/src/main.rs` to understand exactly how the live manifest is currently turned into direct Codex dynamic tools. The existing app is the behavioral baseline, not an architecture template you must copy mechanically.

Treat `docs/agent-work/FL_MCP_2026_BASELINE.md` as the authoritative scope for this branch.

## Verify current MCP implementation details

The target protocol is MCP `2026-07-28`.

Use only current official MCP specification/documentation and the official Rust SDK repository for protocol/API details. At the time this branch was prepared, `rmcp` 3.0.1 is the current stable Rust SDK release supporting MCP `2026-07-28`.

Before coding:

1. verify the current stable `rmcp` 3.x release;
2. read its 3.x migration/current server examples;
3. verify how a `2026-07-28` stdio server is served without hand-writing legacy `initialize`/session behavior;
4. verify the dynamic `ServerHandler` APIs for `tools/list` and `tools/call` rather than assuming the static `#[tool]` macros are appropriate.

Do not use old MCP tutorials that assume `Mcp-Session-Id`, `initialize`/`initialized`, legacy HTTP+SSE, or the 2025 protocol lifecycle as the implementation model.

## Proven baseline to preserve

The existing raw FL agent has already passed live studio tests including broad project setup, channel/mixer creation and naming, routing, colors, and step-sequencer musical edits.

The Gopher remake is frozen as Raw FL Baseline v1.

Do not redesign these tools.

The public raw FL adapter is already exactly what you need:

```rust
GopherNativeAdapter::connect(...)
GopherNativeAdapter::manifest()
GopherNativeAdapter::call_native(tool, arguments)
```

`FlStudioManifest` contains live `NativeToolDefinition` values with:

```text
name
description
input_schema
```

`GopherNativeAdapter` already owns the Gopher-specific invariants:

- live catalog discovery;
- order-sensitive argument canonicalization;
- callback normalization;
- native error detection;
- serialized/single-flight calls.

Do not duplicate any of that in the MCP server.

## Required architecture

Create a dedicated app, preferably:

```text
apps/ghost-fl-mcp/
```

and add it to the workspace.

The first architecture is:

```text
external MCP host
        │
        │ stdio / MCP 2026-07-28
        ▼
ghost-fl-mcp
        │
        ▼
ghost-fl-studio
        │
        ▼
Gopher / FL Studio
```

The MCP app should depend on `ghost-fl-studio`, `rmcp`, async/runtime dependencies actually required by `rmcp` (Tokio where appropriate), `serde`/`serde_json`, CLI/error utilities as needed, and nothing from the Ghost audio/mixing/application layers.

It must not depend on `ghost-codex`.

## Dynamic tool export

Do **not** manually define or copy the known 48 Gopher tools.

On startup, obtain the live manifest and export its tools dynamically through MCP `tools/list`.

For each tool preserve:

- exact live Gopher name;
- exact live description;
- exact live input schema.

MCP 2026 expects deterministic list ordering for caching/prompt stability. Make the MCP presentation deterministic, preferably with a stable sort by tool name, while leaving the underlying live manifest and adapter untouched.

Implement MCP `tools/call` as a thin edge adapter:

```text
MCP tool name + JSON arguments
        ↓
GopherNativeAdapter::call_native
        ↓
faithful MCP result/error
```

Do not add semantic aliases, batch APIs, hidden target selection, task state, context projection, or policy translation.

## Results/errors

Use current `rmcp` response types for MCP `2026-07-28`.

Inspect `NativeToolResult` and preserve the useful native tool result seen by the direct baseline. Use native text content as MCP tool content and structured content where doing so preserves actual native JSON rather than inventing a new schema.

Keep these distinctions clear:

```text
unknown MCP tool / malformed MCP request
invalid FL tool arguments
Gopher/CDP transport failure
FL native tool failure
```

Native FL errors must not become false success simply because the outer MCP JSON-RPC request succeeded.

Write focused tests around the conversion/mapping using app-local fixtures rather than requiring FL Studio in unit tests.

## Safety

The raw catalog includes destructive writes. Preserve a coarse explicit opt-in equivalent to:

```text
--i-accept-live-fl-writes
```

The server should refuse to expose/run the live raw surface without that explicit acceptance, matching the experimental nature of the existing raw agent.

Do not add a fine-grained write filter on this branch. That would change the baseline.

MRTR confirmations are intentionally **not** part of parity v1.

## MCP 2026 scope discipline

MCP 2026-07-28 introduces useful features, but parity v1 should **not** implement them yet:

- Tasks;
- MRTR/input-required confirmation;
- MCP Apps;
- resources;
- subscriptions;
- multi-instance handles;
- Streamable HTTP endpoint;
- OAuth/enterprise authorization.

Using the new protocol version does not require using every extension immediately.

The purpose of this branch is to establish a clean standards-based control group.

## CLI / live run

Match the existing FL connection ergonomics where useful:

```text
--debug-port
--target-match
--i-accept-live-fl-writes
```

Run as an MCP stdio server suitable for configuration in an MCP-capable host.

Keep logging off stdout because stdout belongs to the stdio protocol. Use stderr/tracing for diagnostics, consistent with modern MCP guidance.

Document one example host configuration without making assumptions about a single provider.

## Tests and validation

Work in small coherent commits.

Run:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Add deterministic tests for:

- manifest → MCP tool conversion;
- deterministic ordering;
- exact schema preservation;
- dynamic dispatch;
- native text/structured result mapping;
- adapter error mapping;
- stdio server construction/startup where testable;
- ensuring the new app has no `ghost-codex`/`ghost-application` dependency.

Use official MCP conformance tooling for the `2026-07-28` server where practical. If it cannot be run against the live/dynamic server in CI, create the smallest fixture mode needed to exercise protocol conformance without reimplementing the production server. Document what was and was not validated.

## Live studio acceptance test

The key runtime test is deliberately ambitious because the same surface already passed directly through Codex:

1. start FL Studio with Gopher/CDP enabled;
2. configure a current MCP host to launch `ghost-fl-mcp` with live-write acceptance;
3. confirm the host sees the live tool catalog;
4. use a fresh/disposable project;
5. run the existing benchmark-session setup prompt from `apps/ghost-fl-agent/prompts/setup-benchmark-session.md` through the external harness;
6. verify channels, mixer layout, routing, names/colors, buses, playlist organization and other requested state in FL;
7. follow up with a simple drum-groove prompt on existing/sample-loaded drum channels if appropriate;
8. record the external harness/model, protocol version, tool trace, successes, retries and any behavior differences from the direct Codex baseline.

Do not claim live parity until this or an equivalently broad real-FL test passes.

## Do not promote yet

Do not modify/populate `ghost-application`.

Do not create a universal `AgentRuntime` abstraction.

Do not extract an MCP shared crate merely because the first server works.

Do not merge in the FL scripting bridge.

Do not design a semantic cross-DAW model.

After implementation, report any repeated boundary that *might* deserve future promotion, but leave it app-local until another composition proves reuse.

## Final handoff

When done, provide:

- branch head/commits;
- exact new workspace/app structure;
- `rmcp` version and MCP protocol version;
- how the dynamic Gopher manifest is mapped;
- how results/errors are mapped;
- how to configure/run a current MCP host;
- deterministic validation results;
- MCP conformance results or blockers;
- exact Windows/FL live-validation instructions;
- what still needs the user's machine;
- no speculative claims of parity before live testing.
