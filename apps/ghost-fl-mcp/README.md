# ghost-fl-mcp

`ghost-fl-mcp` exports Ghost & Guild's proven raw FL Studio/Gopher capability surface through MCP `2026-07-28` over stdio.

```text
external MCP host
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

This is a parity experiment, not a semantic FL API. The app does not depend on `ghost-codex` or `ghost-application`, and it does not add Tasks, MRTR, MCP Apps, resources, subscriptions, Streamable HTTP, scripting-bridge tools, or a generic DAW model.

## Protocol and SDK

- MCP protocol: `2026-07-28`
- Rust SDK: `rmcp = 3.0.1`
- Transport: stdio

The server uses `rmcp::ServerHandler` directly because the live Gopher tool catalog is dynamic. `tools/list` is built from `GopherNativeAdapter::manifest()` at startup and sorted by tool name for deterministic MCP presentation. Each tool preserves the live Gopher name, description and input schema.

`tools/call` forwards the requested name and JSON object directly to `GopherNativeAdapter::call_native()`. The adapter remains responsible for Gopher schema-order argument canonicalization, recursive callback normalization, native-error detection and single-flight serialization.

Successful native calls preserve Gopher text content as MCP content and expose the unmodified native JSON response as MCP `structuredContent`. FL argument/transport/native failures are returned as visible MCP tool errors. Unknown tools remain MCP protocol errors.

## Live-write gate

The raw catalog includes destructive operations. The server refuses to start unless the operator explicitly accepts live writes:

```powershell
cargo run -p ghost-fl-mcp --release -- \
  --debug-port 9222 \
  --target-match gopher \
  --i-accept-live-fl-writes
```

Use a fresh or disposable FL Studio project for broad agent tests.

All diagnostics go to stderr. Stdout is reserved for MCP stdio protocol traffic.

## Host configuration

Build the binary, then configure a current MCP host to launch it as a stdio server. Exact configuration keys vary by host; the command/argument shape is:

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

Use an MCP host that speaks the `2026-07-28` discovery/per-request-metadata lifecycle rather than a legacy-only initialize/session implementation.

## Deterministic validation

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The app-local tests cover deterministic tool ordering, exact name/description/schema preservation, dynamic dispatch, unknown-tool rejection, result mapping, native/transport/argument error mapping, and dependency isolation from `ghost-codex`/`ghost-application`.

### Official conformance tooling

The current official `@modelcontextprotocol/conformance` server runner accepts an HTTP `--url`; it does not launch/test a stdio server directly. This phase deliberately does not add Streamable HTTP solely for conformance, so the official URL-based server suite is a documented transport mismatch rather than a claimed pass.

For a later HTTP fixture or endpoint, the relevant current suite shape is:

```bash
npx -y @modelcontextprotocol/conformance server \
  --url http://127.0.0.1:8002/mcp \
  --suite all \
  --spec-version 2026-07-28
```

## Human-only live FL acceptance

The user machine remains authoritative for FL/Gopher runtime behavior.

1. Start FL Studio with Gopher/CDP enabled as for `ghost-fl-agent`.
2. Launch this server from an MCP `2026-07-28`-capable external host with `--i-accept-live-fl-writes`.
3. Confirm the host's tool list matches the current live Gopher manifest.
4. Open a fresh/disposable FL project.
5. Run `apps/ghost-fl-agent/prompts/setup-benchmark-session.md` through the external MCP harness.
6. Verify the requested channels, mixer layout, routing, names/colors, buses, playlist organization and other state in FL Studio.
7. Compare the external harness tool trace with Raw FL Baseline v1.

Do not claim live parity until this broad real-FL acceptance test passes.
