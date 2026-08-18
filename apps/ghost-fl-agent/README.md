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

## Frozen behavioral baseline

The raw agent behavior in this app is now a control group. Its Codex tool registry must remain the complete live Gopher catalog only.

The scripting bridge was originally implemented here to prove transport feasibility. That proof succeeded in the real FL Studio runtime, but scripting calls are still deliberately separate from the raw agent registry.

On `feat/fl-scripting-framework`, reusable scripting transport/protocol/assets are promoted into a dedicated lower-layer crate while this app remains the Gopher-only behavioral baseline.

Read:

- [`docs/FL_SCRIPTING_JOURNEY.md`](../../docs/FL_SCRIPTING_JOURNEY.md)
- [`docs/agent-work/FL_SCRIPTING_FRAMEWORK.md`](../../docs/agent-work/FL_SCRIPTING_FRAMEWORK.md)
- [`docs/agent-work/FL_SCRIPTING_FRAMEWORK_IMPLEMENTATION_PROMPT.md`](../../docs/agent-work/FL_SCRIPTING_FRAMEWORK_IMPLEMENTATION_PROMPT.md)

The target reusable crate is:

```text
crates/ghost-fl-scripting/
```

The target combined Gopher + scripting research app is:

```text
apps/ghost-fl-workspace/
```

Do not turn `ghost-fl-agent` itself into that combined app.

## Source experiment: FL scripting bridge

`feat/fl-scripting-bridge` preserved the proven Gopher baseline and added a second, initially app-local experiment for FL Studio MIDI Scripting.

Historical branch documents:

- [`docs/agent-work/FL_SCRIPTING_BRIDGE.md`](../../docs/agent-work/FL_SCRIPTING_BRIDGE.md)
- [`docs/agent-work/FL_SCRIPTING_BRIDGE_IMPLEMENTATION_PROMPT.md`](../../docs/agent-work/FL_SCRIPTING_BRIDGE_IMPLEMENTATION_PROMPT.md)
- [`docs/agent-work/FL_SCRIPTING_BRIDGE_FINDINGS.md`](../../docs/agent-work/FL_SCRIPTING_BRIDGE_FINDINGS.md)

The final live-proven topology is **virtual MIDI for FL auto-load only** plus an outbound, nonblocking localhost connection implemented through a subinterpreter-compatible native CPython extension. The Python-level socket constructor was not viable in FL's embedded runtime; `ghost_native` owns native WinSock while `device_Ghost.py` owns bounded FL callback/protocol dispatch.

The current test bootstrap uses the user's loopMIDI endpoint `Ghost Midi`; native Windows MIDI Services/CoreMIDI endpoint creation is explicitly later work.

Do not redesign or "improve" the Gopher tool surface while extracting scripting.

### Install the FL controller script for the Windows test

On the source experiment, the script is bundled at:

```text
apps/ghost-fl-agent/fl-script/device_Ghost.py
```

Its controller metadata is:

```python
# name=Ghost Bridge
# supportedDevices=Ghost Midi
```

Default source-branch install:

```powershell
powershell -ExecutionPolicy Bypass -File .\apps\ghost-fl-agent\fl-script\install.ps1
```

The installer targets the normal user-data Hardware scripting directory under Documents and creates a `Ghost Bridge` folder. If your FL Studio user-data root differs, pass the Hardware directory explicitly:

```powershell
powershell -ExecutionPolicy Bypass -File .\apps\ghost-fl-agent\fl-script\install.ps1 `
  -HardwareRoot "D:\FL User Data\FL Studio\Settings\Hardware"
```

The installer does not create a virtual MIDI endpoint. `Ghost Midi` remains temporary external test infrastructure.

`feat/fl-scripting-framework` should update these commands/paths once the script/native assets move into `crates/ghost-fl-scripting`.

### Run the raw-agent + scripting diagnostic source app

Prerequisites remain the normal Gopher/CDP prerequisites below. The scripting listener defaults to `127.0.0.1:48766`:

```powershell
cargo run -p ghost-fl-agent -- --i-accept-live-fl-writes --scripting-bind 127.0.0.1:48766
```

Then open:

```text
http://127.0.0.1:48765
```

The UI shows a separate **FL scripting bridge** status and a **Run scripting probe** developer action. Scripting calls are not added to the Codex tool registry.

Useful scripting options:

```text
--scripting-bind <loopback-host:port>
--scripting-timeout-ms <milliseconds>
```

If using a custom port, configure the FL script process before FL loads/reloads the script:

```powershell
$env:GHOST_FL_SCRIPTING_HOST = "127.0.0.1"
$env:GHOST_FL_SCRIPTING_PORT = "48767"
cargo run -p ghost-fl-agent -- --i-accept-live-fl-writes --scripting-bind 127.0.0.1:48767
```

See the journey document for the exact live runtime findings and the regression baseline.

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
--scripting-bind <loopback-host:port>
--scripting-timeout-ms <milliseconds>
--codex-binary <path-or-name>
--model <model>
--verbose-agent-events
```

## Raw means raw

At startup the app loads the complete live Gopher manifest and registers every advertised tool directly into one persistent Codex thread. `ghost-fl-studio` remains responsible only for the real integration invariants: live schemas, argument ordering, callback normalization, single-flight calls and native error detection.

This app adds only minimal agent instructions about live-state discipline, discovery before guessing, preserving unrelated routing, destructive ambiguity, and grounded final claims.

The coarse `--i-accept-live-fl-writes` gate exists because the raw catalog includes destructive operations. There is deliberately no hidden per-tool write policy in phase one.

The scripting bridge is separate from that agent tool surface. It is a transport/state probe used from the developer UI, not an additional raw-tool registry.

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

The same page also exposes scripting diagnostics:

```text
browser
  -> GET /api/scripting/status
  -> POST /api/scripting/probe
  -> Rust scripting adapter/bridge
  -> outbound-connected FL device script
  -> FL Studio MIDI Scripting API
```

This path remains separate from Codex/Gopher in this baseline app even after its reusable transport is extracted.

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

This app intentionally does not itself provide:

- readonly/studio/raw profiles;
- structured scenario runner or deterministic verifiers;
- persistent JSON/JSONL episode logs;
- semantic workspace references;
- audio capture/analysis;
- reusable capability/plugin interfaces;
- scripting APIs registered as Codex tools;
- native Windows MIDI Services/CoreMIDI virtual endpoint creation;
- any `ghost-application` abstractions for the scripting bridge.

The new `ghost-fl-workspace` app is where combined-surface experiments should happen.