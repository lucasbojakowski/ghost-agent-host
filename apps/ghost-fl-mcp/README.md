# ghost-fl-mcp

`ghost-fl-mcp` is the live-proven **raw Gopher MCP control group**.

It exports the same dynamic FL Studio/Gopher capability surface used by the raw direct-Codex baseline through MCP `2026-07-28` over stdio.

```text
external MCP host / agent
        |
        | MCP 2026-07-28 / stdio
        v
   ghost-fl-mcp
        |
        v
 ghost-fl-studio
        |
        v
 Gopher / FL Studio
```

Status: **PROVEN for executable/harness/tool interoperability**.

The accepted user-machine validation established that the executable builds, an external MCP harness launches/connects to it, the agent receives the tools, and live FL tool calls succeed.

This app remains a parity/control experiment, not the expanded Gopher + scripting MCP surface.

## Protocol and SDK

- MCP protocol: `2026-07-28`
- Rust SDK: `rmcp = 3.0.1`
- transport: stdio

The server uses `rmcp::ServerHandler` directly because the live Gopher catalog is dynamic.

At startup:

```text
GopherNativeAdapter::manifest()
    -> deterministic MCP tools/list
```

Each tool preserves the live Gopher name, description and input schema. `tools/call` forwards the requested name and JSON object to `GopherNativeAdapter::call_native()`.

`ghost-fl-studio` continues to own Gopher-specific invariants: live schema lookup, argument-order canonicalization, recursive result normalization, native error detection and single-flight calls.

Successful native calls expose text content when present and the unmodified native JSON response as MCP `structuredContent`. Native adapter failures are visible tool errors; unknown tools remain protocol-level method-not-found failures.

## Live-write gate

The raw catalog includes destructive operations. The server refuses to start without explicit acceptance:

```powershell
cargo run -p ghost-fl-mcp --release -- \
  --debug-port 9222 \
  --target-match gopher \
  --i-accept-live-fl-writes
```

Use a disposable FL project for broad tests.

All diagnostics go to stderr. Stdout is reserved for MCP stdio protocol traffic.

## Host configuration

Configure a current MCP host to launch the built executable as a stdio server. Exact host configuration varies; conceptually:

```json
{
  "command": "C:\\path\\to\\ghost-fl-mcp.exe",
  "args": [
    "--debug-port", "9222",
    "--target-match", "gopher",
    "--i-accept-live-fl-writes"
  ]
}
```

Use a host compatible with the targeted MCP lifecycle/version.

## Validation scope

The accepted baseline proves:

```text
executable build                         PASS
external harness connection             PASS
agent tool discovery/use                PASS
live FL invocation through MCP          PASS
```

It does not claim that every broader evaluation has been rerun. In particular, the latest acceptance record does not separately claim:

- full `setup-benchmark-session.md` parity;
- official MCP conformance-suite completion;
- performance parity with direct Codex;
- scripting export through MCP.

See `docs/agent-work/FL_MCP_2026_VALIDATION.md`.

## Deterministic tests

The app-local tests cover:

- deterministic tool ordering;
- exact name/description/schema preservation;
- dynamic dispatch;
- unknown-tool rejection;
- result/error mapping;
- dependency isolation from `ghost-codex` and `ghost-application`.

The combined integration branch must still run the normal repository fmt/check/test/clippy matrix after its Cargo lockfile is reconciled.

## Why this app stays raw

`ghost-fl-workspace` has already proven a richer direct-Codex surface:

```text
Gopher
+ scripting search/describe/call
+ compact live context
```

Do **not** silently add those capabilities here. Keeping `ghost-fl-mcp` raw gives us a stable external-harness control group.

The expanded MCP surface should be a separate app/experiment so we can compare:

```text
raw vs expanded surface
and
Codex vs external MCP harness
```

See `docs/FL_CAPABILITY_SURFACES.md`.

## Ownership boundary

MCP is an app-owned edge protocol. `ghost-fl-studio` remains MCP-agnostic.

If a second MCP app proves reusable conversion/dispatch machinery, the smallest protocol-neutral pieces may later be promoted into Ghost Core. The product-specific server, capability selection and policy should remain app-owned.

Canonical baseline status: `docs/PROVEN_BASELINES.md`.
