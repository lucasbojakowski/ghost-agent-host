# ghost-fl-agent

Phase-one research app for a persistent raw agent over the live FL Studio/Gopher environment.

This app is the **frozen Gopher-only behavioral baseline**. Its Codex dynamic-tool registry must remain exactly the complete live Gopher catalog; FL MIDI Scripting must not be registered there.

The earlier app-local scripting bridge proved transport feasibility in FL Studio 26.1.3 / MIDI Scripting API 44. On `feat/fl-scripting-framework`, the reusable transport/protocol/runtime assets now live in:

```text
crates/ghost-fl-scripting/
```

The separate combined Gopher + scripting experiment lives in:

```text
apps/ghost-fl-workspace/
```

Do not turn `ghost-fl-agent` itself into that combined app.

Read:

- [`docs/FL_SCRIPTING_JOURNEY.md`](../../docs/FL_SCRIPTING_JOURNEY.md)
- [`docs/agent-work/FL_SCRIPTING_BRIDGE.md`](../../docs/agent-work/FL_SCRIPTING_BRIDGE.md)
- [`docs/agent-work/FL_SCRIPTING_BRIDGE_FINDINGS.md`](../../docs/agent-work/FL_SCRIPTING_BRIDGE_FINDINGS.md)
- [`docs/agent-work/FL_SCRIPTING_FRAMEWORK.md`](../../docs/agent-work/FL_SCRIPTING_FRAMEWORK.md)
- [`docs/agent-work/FL_SCRIPTING_FRAMEWORK_IMPLEMENTATION_PROMPT.md`](../../docs/agent-work/FL_SCRIPTING_FRAMEWORK_IMPLEMENTATION_PROMPT.md)
- [`docs/agent-work/FL_SCRIPTING_FRAMEWORK_VALIDATION.md`](../../docs/agent-work/FL_SCRIPTING_FRAMEWORK_VALIDATION.md)

## Raw means raw

This app intentionally does **not** inherit the assumptions in `ghost-workflow`:

- no Ghost Tap requirement;
- no audio-analysis requirement;
- no mixer-only target;
- no slot range;
- no plugin allowlist;
- no semantic DAW projection;
- no scripting tools in the Codex registry;
- no promotion into `ghost-application`.

At startup it loads the complete live Gopher manifest and registers every advertised native tool directly into one persistent Codex thread. `ghost-fl-studio` owns only the real Gopher integration invariants: live schemas, argument ordering, callback normalization, single-flight calls and native error detection.

The coarse `--i-accept-live-fl-writes` gate exists because the raw catalog includes destructive operations. There is deliberately no hidden per-tool write policy in this baseline.

## FL scripting diagnostic path

The existing developer scripting status/probe endpoints remain in this app, but they now consume `ghost-fl-scripting` instead of owning transport code.

The proven topology remains:

```text
Rust ghost-fl-scripting listener
  ^
  | bounded versioned NDJSON over loopback TCP
  |
FL controller script device_Ghost.py
  |
  v
ghost_native CPython 3.12 extension
  -> nonblocking WinSock
```

Virtual MIDI is only the FL controller-script auto-load bootstrap; it is not the RPC data plane.

Install the promoted controller script/native artifact from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\crates\ghost-fl-scripting\fl-script\install.ps1
```

Rebuild the native extension only on the Windows validation host with 64-bit Python 3.12 and the required MSVC toolchain:

```powershell
powershell -ExecutionPolicy Bypass -File .\crates\ghost-fl-scripting\fl-native\build.ps1
```

The tracked `ghost_native.cp312-win_amd64.pyd` in that directory is the known-good live artifact. Do not replace/delete it until a replacement has passed the same FL runtime gate.

## Run

Prerequisites:

1. FL Studio is running with the WebView2 CDP debugging port enabled.
2. Gopher is open/available.
3. Codex App Server is available through the configured `codex` binary.
4. The current FL project is one you are willing to modify.
5. For scripting diagnostics, the promoted `Ghost Bridge` script is loaded and pointed at the same loopback scripting port.

```powershell
cargo run -p ghost-fl-agent -- --i-accept-live-fl-writes
```

Open:

```text
http://127.0.0.1:48765
```

Useful options:

```text
--debug-port <port>
--target-match <text>
--bind <host:port>
--scripting-bind <loopback-host:port>
--scripting-timeout-ms <milliseconds>
--codex-binary <path-or-name>
--model <model>
--verbose-agent-events
```

The scripting listener defaults to `127.0.0.1:48766`. For a custom port, configure the FL controller-script process before FL loads/reloads it:

```powershell
$env:GHOST_FL_SCRIPTING_HOST = "127.0.0.1"
$env:GHOST_FL_SCRIPTING_PORT = "48767"
cargo run -p ghost-fl-agent -- --i-accept-live-fl-writes --scripting-bind 127.0.0.1:48767
```

## Browser surfaces

The raw agent path remains:

```text
browser
  -> POST /api/chat
  -> persistent Codex thread
  -> complete raw live Gopher tools
  -> FL Studio
```

Scripting remains a separate developer-only diagnostic path:

```text
browser
  -> GET /api/scripting/status
  -> POST /api/scripting/probe
  -> ghost-fl-scripting
  -> FL MIDI Scripting API
```

The reversible mixer-selection probe remains application validation behavior. It is not part of the reusable scripting adapter and is not a Codex tool.

## Benchmark Session A

The **Build benchmark session** UI action still runs `prompts/setup-benchmark-session.md` as a real user turn on the same Gopher-only thread. The benchmark must abort without mutation if the project does not appear fresh/disposable.

Success markers:

```text
BENCHMARK_SETUP_GREEN
BENCHMARK_SETUP_PARTIAL
BENCHMARK_SETUP_ABORTED
```

## Baseline limits

This app intentionally does not itself provide semantic workspace references, audio capture/analysis, capability/plugin interfaces, scripting APIs registered as Codex tools, native virtual-MIDI endpoint creation, or the later skill/tool/intent architecture.
