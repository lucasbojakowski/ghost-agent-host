# ghost-fl-workspace

Empirical combined FL Studio research harness for the scripting-framework branch.

This app composes the two transparent FL integration surfaces without introducing the later Ghost skill/tool/intent architecture:

```text
Codex App Server
  |
  +-- complete live Gopher tool catalog (unchanged)
  |
  +-- fl_scripting_search
  +-- fl_scripting_describe
  +-- fl_scripting_call
        |
        v
     ghost-fl-scripting
        |
        v
     FL MIDI Scripting API
```

`ghost-fl-agent` remains the frozen Gopher-only behavioral baseline. Do not move the scripting gateways into that app.

## Why only three scripting tools

The checked-in FL scripting runtime artifact contains hundreds of functions and overloads. Registering every function as a Codex dynamic tool would create unnecessary context pressure and would blur the primitive adapter with a generated semantic layer.

Instead:

- `fl_scripting_search(query, module?)` performs deterministic metadata search;
- `fl_scripting_describe(module, function)` returns the exact checked-in signature/return/API-version/bridge-support evidence, including overloads;
- `fl_scripting_call(module, function, args)` invokes one metadata-approved primitive with positional JSON arguments through `ghost-fl-scripting`.

The lower adapter still owns module/function validation and refuses functions whose checked-in metadata does not establish a bridge-compatible wire shape.

## Point-in-time snapshot

Before every Codex turn the app reads a compact MIDI Scripting snapshot containing the currently available project title/changed flag, safe-to-edit state, selected channel and mixer track, mixer count, current pattern/name/count, arrangement selection, focused plugin/window, song position/hint, loop mode and playing state.

This snapshot is deliberately treated as stale immediately after capture. It is context for reasoning, not a cached semantic world model. The agent is instructed to re-observe through the live surfaces when correctness depends on current state.

## Run

Prerequisites:

1. FL Studio is running with the WebView2 CDP debugging port enabled and Gopher available.
2. The promoted `Ghost Bridge` MIDI controller script is installed from `crates/ghost-fl-scripting/fl-script/install.ps1`.
3. The known-good `ghost_native.cp312-win_amd64.pyd` is installed into FL Studio's shared Python library.
4. `Ghost Midi` exists as the current temporary virtual-MIDI auto-load bootstrap.
5. Codex App Server is available through the configured `codex` binary.
6. The open FL project is disposable or otherwise safe for live writes.

From the repository root:

```powershell
cargo run -p ghost-fl-workspace -- --i-accept-live-fl-writes
```

Defaults:

```text
Gopher CDP:        127.0.0.1:9222
workspace UI:      127.0.0.1:48775
scripting listener:127.0.0.1:48766
```

Open:

```text
http://127.0.0.1:48775
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

## Manual hybrid acceptance gate

The static CI gate cannot prove FL Studio runtime behavior. On a Windows host with FL Studio 26.1.3 / scripting API 44 (or the intended replacement target), use a disposable project and run at least these empirical tasks:

1. **Hybrid task:** ask the agent to identify the current mixer/channel context through scripting, make a reversible or disposable native Gopher mutation that depends on that context, then verify the resulting live state through scripting.
2. **Scripting gateway task:** ask the agent to discover a known scripting primitive with `fl_scripting_search`, inspect it with `fl_scripting_describe`, invoke it through `fl_scripting_call`, and verify the observed result.
3. Confirm Gopher still exposes the complete live manifest and that only the three scripting gateway definitions are added in this app.
4. Confirm `ghost-fl-agent` remains behaviorally Gopher-only.

Record the exact FL Studio version, scripting API version, native extension artifact and observed pass/fail results before declaring the branch live-complete.

## Explicitly out of scope

This app does not add skills, intents, semantic DAW entities, capability profiles, policy-generated tool subsets, generic DAW abstractions, audio analysis, persistent episode storage, or event subscriptions. Those decisions belong after the combined primitive experiment produces evidence.
