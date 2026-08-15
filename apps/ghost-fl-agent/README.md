# ghost-fl-agent

Phase-one research app for a persistent raw agent over the live FL Studio/Gopher environment.

This app intentionally does **not** inherit the assumptions in `ghost-workflow`:

- no Ghost Tap requirement;
- no audio-analysis requirement;
- no mixer-only target;
- no slot range;
- no plugin allowlist;
- no semantic DAW projection;
- no promotion into `ghost-application`.

The experiment asks a narrower question:

> How far can a frontier agent get in real FL Studio work when it receives the faithful live Gopher tool catalog with only minimal harness instructions?

## Run

Prerequisites:

1. FL Studio is running with the WebView2 CDP debugging port enabled.
2. Gopher is open/available.
3. Codex App Server is available through the configured `codex` binary.
4. The current FL project is one you are willing to modify.

From the repository root:

```powershell
cargo run -p ghost-fl-agent -- --i-accept-live-fl-writes
```

Then open:

```text
http://127.0.0.1:48765
```

Useful options:

```text
--debug-port <port>
--target-match <text>
--bind <host:port>
--codex-binary <path-or-name>
--model <model>
--verbose-agent-events
```

## Raw means raw

At startup the app loads the complete live Gopher manifest and registers every advertised tool directly into one persistent Codex thread. `ghost-fl-studio` remains responsible only for the real integration invariants: live schemas, argument ordering, callback normalization, single-flight calls and native error detection.

This app adds only minimal agent instructions about live-state discipline, discovery before guessing, preserving unrelated routing, destructive ambiguity, and grounded final claims.

The coarse `--i-accept-live-fl-writes` gate exists because the raw catalog includes destructive operations. There is deliberately no hidden per-tool write policy in phase one.

## Browser chat

The app serves a dependency-free HTML chat page from the Rust executable itself. The HTTP server is intentionally small and synchronous for the first experiment:

```text
browser
  -> POST /api/chat
  -> persistent Codex thread
  -> raw live Gopher tools
  -> FL Studio
```

Completed turns return the final assistant text plus a compact native-tool trace for inspection in the chat UI. Streaming, profiles, approvals, persistence and richer trajectory capture belong to later phases after the raw baseline is measured.

## Benchmark Session A setup

The **Build benchmark session** button runs `prompts/setup-benchmark-session.md` as a real user turn on the same raw thread.

The setup prompt is intentionally demanding: from a fresh/disposable FL project the agent must inspect current state, establish a repeatable channel/mixer/bus/playlist fixture, create deliberately imperfect routing for later repair scenarios, and verify the final state. It must abort without mutation if the project does not appear fresh/disposable.

Success markers:

```text
BENCHMARK_SETUP_GREEN
BENCHMARK_SETUP_PARTIAL
BENCHMARK_SETUP_ABORTED
```

A GREEN result is an early signal that the raw agent can coordinate many independent FL operations coherently before Ghost adds semantic projections or optimized tools.

## Phase-one limits

This app intentionally does not yet provide:

- readonly/studio/raw profiles;
- structured scenario runner or deterministic verifiers;
- persistent JSON/JSONL episode logs;
- semantic workspace references;
- audio capture/analysis;
- reusable capability/plugin interfaces;
- any `ghost-application` abstractions.

Those should be driven by evidence from this app rather than designed in advance.
