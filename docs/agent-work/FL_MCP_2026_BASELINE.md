# FL Studio MCP 2026 baseline experiment

Branch: `feat/fl-mcp-2026`

## Purpose

The current raw Gopher agent is runtime-proven and is now frozen as **Raw FL Baseline v1**:

```text
live Gopher manifest
+ faithful `ghost-fl-studio` calls
+ minimal agent instructions
```

This branch exports that same FL capability surface through the current Model Context Protocol so other providers/harnesses can operate the exact same DAW surface.

The goal is **protocol parity and harness interoperability**, not a new semantic FL API.

Do not combine this branch with the FL Studio MIDI Scripting bridge experiment. That work proceeds independently on `feat/fl-scripting-bridge`.

## Protocol target

Target the current stable MCP protocol release:

```text
2026-07-28
```

Use the official Rust SDK (`rmcp`) 3.x. At branch creation/research time, `rmcp` 3.0.1 is the current stable release and the official SDK documents support for MCP `2026-07-28` while retaining compatibility with older protocol versions.

Before implementation, verify the exact current 3.x API against the official `modelcontextprotocol/rust-sdk` repository and MCP specification. Do not hand-write the MCP wire protocol when the official SDK already implements it.

Important 2026-07-28 properties for this branch:

- protocol-level sessions are gone;
- the old `initialize` / `initialized` lifecycle is gone for 2026-07-28;
- requests are self-describing and carry protocol/client metadata;
- `server/discover` is available;
- normal results have explicit result typing;
- tool list results are cacheable and should have deterministic ordering;
- full JSON Schema 2020-12 is supported;
- Tasks and MCP Apps are official extensions;
- MRTR exists for multi-round-trip input/confirmation;
- Streamable HTTP is stateless and no longer uses `Mcp-Session-Id` / `Last-Event-ID` replay.

For **this first parity branch**, only the core tool server is required. Do not add Tasks, MRTR, MCP Apps, resources, subscriptions, authorization, or semantic projections unless the SDK requires minimal declarations. Those features are future experiments after raw parity is proven.

## Architecture for this branch

Create a dedicated local MCP app, tentatively:

```text
apps/ghost-fl-mcp/
```

with this dependency shape:

```text
MCP host
(ChatGPT / Claude / IDE / harness)
        │
        │ MCP 2026-07-28 over stdio
        ▼
apps/ghost-fl-mcp
        │
        │ direct Rust calls
        ▼
ghost-fl-studio
        │
        ▼
Gopher / FL Studio
```

Do not route through `ghost-codex`. The point is to let an external harness own the agent loop while Ghost owns the FL capability surface.

Do not add MCP behavior to `ghost-fl-studio`; MCP is an edge protocol in this experiment.

## Why stdio first

Use MCP stdio for the first parity server because this is a local desktop integration and external harnesses can launch the Rust binary directly.

That gives the cleanest first comparison:

```text
Codex direct baseline
        vs
external harness → MCP stdio → same Gopher surface
```

Do not build Streamable HTTP merely because MCP supports it. HTTP can be added later if a persistent Ghost desktop process needs to serve multiple clients.

## Raw tool parity

At startup:

1. connect `GopherNativeAdapter`;
2. read the live `FlStudioManifest`;
3. expose every live `NativeToolDefinition` as an MCP tool;
4. keep the native Gopher tool **name**, **description**, and **input schema**;
5. handle MCP `tools/call` by invoking `GopherNativeAdapter::call_native(name, arguments)`;
6. preserve the Gopher adapter's single-flight behavior and argument-order canonicalization;
7. map results/errors faithfully without inventing semantic DAW behavior.

Do not manually duplicate the 48 known tools as Rust functions. The server must remain driven by the **live manifest**, because the current adapter contract is explicitly dynamic/version-probed.

`tools/list` should have deterministic ordering for MCP caching. A stable sort by tool name at the MCP edge is acceptable; do not alter tool semantics or schemas.

## Result mapping

Inspect actual `NativeToolResult` values and use the official `rmcp` result types.

The MCP client should receive the same useful content the direct raw agent sees. Preserve native text results and expose structured native data when it is meaningful, but do not transform results into a new domain model.

Distinguish:

- MCP/protocol errors;
- invalid tool/arguments;
- transport failures;
- FL/Gopher native tool failures.

Do not report a failed FL native operation as a successful MCP tool call merely because JSON-RPC transport succeeded.

## Safety baseline

The raw Gopher catalog includes destructive operations. Preserve a coarse explicit live-write acceptance gate equivalent in spirit to `ghost-fl-agent`'s:

```text
--i-accept-live-fl-writes
```

The MCP parity server must not silently become safer or more restrictive than Raw FL Baseline v1, because that would invalidate the harness comparison. Likewise, do not remove the coarse opt-in.

Fine-grained permissions, MRTR confirmations, and semantic risk policy are future layers after parity.

## App-local implementation rule

Do not promote MCP abstractions into `ghost-application` or a new shared crate simply because MCP is useful once.

The first MCP server belongs in `apps/*`. Reuse can be extracted only after another app/provider path proves the boundary.

Do not turn `ghost-codex` into a universal agent runtime on this branch.

## Validation

Static/deterministic:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Add focused MCP tests where practical for:

- deterministic `tools/list` conversion from a fixture `FlStudioManifest`;
- exact preservation of name/description/input schema;
- dynamic dispatch by tool name;
- result mapping;
- invalid arguments / unknown tools / native errors;
- no accidental dependency on `ghost-codex`;
- protocol server startup over stdio.

If the official MCP conformance tooling is practical for a local Rust server, run the relevant `2026-07-28` server scenarios or at minimum document the exact command and any blocker. Do not claim conformance solely from compilation.

Live FL validation:

1. start FL Studio with Gopher/CDP exactly as for `ghost-fl-agent`;
2. launch `ghost-fl-mcp` from an MCP 2026-07-28-capable host;
3. list tools and confirm parity with the current live Gopher manifest;
4. run the already-proven benchmark-session setup prompt in a fresh/disposable FL project through the external harness;
5. confirm the same kinds of channel/mixer/routing/color/sequencer operations work;
6. inspect the MCP tool trace and compare it with the Codex-direct raw baseline.

## What not to build yet

Do not implement in the parity phase:

- semantic tools such as `fl.channel.update` or `fl.mixer.update_track`;
- a new typed DAW/domain model;
- MCP resources mirroring the project;
- Tasks for long-running FL operations;
- MRTR confirmation flows;
- MCP App mixer/project UI;
- subscriptions/live state events;
- multi-FL instance handles;
- FL MIDI Scripting/socket RPC;
- Windows MIDI Services/CoreMIDI;
- `ghost-application` promotion;
- a generic multi-provider in-process agent trait.

Those are valuable follow-ups, but adding them now would prevent us from answering the first experiment cleanly:

> Can another harness operate the exact same proven raw FL surface through standards-based MCP with no Ghost-specific semantic redesign?

## Follow-up sequence after parity

Once parity is live-proven, the branch findings should distinguish future work into separate experiments:

1. **Harness benchmark** — same FL fixture/prompts across Codex direct and external MCP hosts.
2. **Scripting surface composition** — after the independent scripting bridge is proven, decide whether/how to expose it through MCP.
3. **MCP 2026 features** — test MRTR for destructive ambiguity, Tasks for long-running/batched operations, resources/subscriptions for observations, and MCP Apps for visual interaction.
4. **Semantic FL API** — only if the raw benchmark produces evidence that composed tools improve correctness/efficiency.
5. **Persistent Ghost desktop MCP endpoint** — evaluate Streamable HTTP only when the product runtime needs it.

The raw parity server is the control group for all of those experiments.