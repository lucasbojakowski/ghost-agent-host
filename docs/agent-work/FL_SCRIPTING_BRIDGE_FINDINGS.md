# FL scripting bridge implementation findings

Branch: `feat/fl-scripting-bridge`

Status: deterministic implementation complete; live Windows/FL validation pending on the user's machine.

## Implemented topology

```text
apps/ghost-fl-agent
    ├── existing frozen Gopher/CDP path -> FL Studio/Gopher
    │
    └── 127.0.0.1:48766 TCP listener
             ^
             | outbound nonblocking socket
             |
      fl-script/device_Ghost.py
             ^
             | bootstrap/autoload only
             |
          Ghost Midi
```

The scripting bridge is app-local. No scripting code or policy was promoted into `ghost-application`, `ghost-fl-studio`, or a generic DAW/RPC abstraction.

## Wire protocol v1

The transport is newline-delimited JSON with one object per line.

Handshake:

```json
{"type":"hello","protocol":1,"bridge":"ghost-fl-scripting","fl_version":"...","scripting_api_version":44}
```

Call:

```json
{"type":"call","id":17,"module":"patterns","function":"getPatternName","args":[4]}
```

Success:

```json
{"type":"result","id":17,"ok":true,"value":"Drums"}
```

Failure:

```json
{"type":"result","id":17,"ok":false,"error":{"kind":"call_failed","message":"..."}}
```

Rust owns request IDs, correlation, timeouts, connection state, and diagnostics. The FL script only frames messages, validates an explicit module/function target, invokes the FL scripting call, and serializes JSON-compatible values.

## Python-side lifecycle constraints encoded

`device_Ghost.py` uses a nonblocking socket and does bounded work from `OnIdle()`:

- bounded reads per idle callback;
- bounded calls dispatched per idle callback;
- bounded writes per idle callback;
- bounded receive/send buffers;
- bounded frame size;
- reconnect backoff with a fixed upper bound;
- immediate reconnect attempt after script initialization;
- clean socket reset on device/script shutdown.

The allowlisted FL modules are:

```text
arrangement
channels
general
mixer
patterns
playlist
plugins
transport
ui
```

Function names must be plain public Python identifiers. The script does not use `eval`, `exec`, arbitrary imports, filesystem request/response files, MIDI SysEx, or MIDI messages as the RPC data plane.

## Probe surface

The developer probe currently observes:

- MIDI scripting API version;
- FL version string;
- project title;
- project changed flag;
- `general.safeToEdit()`;
- selected Channel Rack channel;
- selected mixer track and mixer track count;
- current pattern number/count/name;
- arrangement selection start/end plus a derived active-selection flag;
- focused plugin name;
- focused window/form caption;
- song position and position hint;
- loop mode;
- playback state.

The signatures were selected against the user-provided runtime-enriched FL Studio scripting dump generated on 2026-08-15, rather than inferred from Gopher's MCP surface.

## Reversible mutation proof

The probe uses mixer-track selection as the first harmless reversible setter test:

```text
read mixer.trackNumber()
  -> choose a different existing track
  -> mixer.setTrackNumber(temp)
  -> verify trackNumber() == temp
  -> mixer.setTrackNumber(original)
  -> verify trackNumber() == original
```

The mutation is skipped unless `general.safeToEdit()` returns `1` and an alternate mixer track exists. It does not rename project content or change routing/plugins.

This behavior is deterministic in the Rust implementation but still requires live FL confirmation that the embedded scripting runtime behaves exactly as expected.

## Installation

The bundled script is:

```text
apps/ghost-fl-agent/fl-script/device_Ghost.py
```

Default Windows test installation:

```powershell
powershell -ExecutionPolicy Bypass -File .\apps\ghost-fl-agent\fl-script\install.ps1
```

The installer defaults to:

```text
%USERPROFILE%\Documents\Image-Line\FL Studio\Settings\Hardware\Ghost Bridge\device_Ghost.py
```

If the FL Studio user-data directory is elsewhere, override the Hardware directory:

```powershell
powershell -ExecutionPolicy Bypass -File .\apps\ghost-fl-agent\fl-script\install.ps1 `
  -HardwareRoot "D:\FL User Data\FL Studio\Settings\Hardware"
```

The installer does not create or manage a virtual MIDI endpoint.

## Run command

With the existing Gopher/CDP prerequisite running:

```powershell
cargo run -p ghost-fl-agent -- --i-accept-live-fl-writes --scripting-bind 127.0.0.1:48766
```

Then open the existing UI:

```text
http://127.0.0.1:48765
```

The scripting panel reports listener/handshake state and exposes **Run scripting probe**. The scripting surface is deliberately not registered as Codex tools on this branch.

For a custom TCP port, configure both processes before FL loads/reloads the script:

```powershell
$env:GHOST_FL_SCRIPTING_HOST = "127.0.0.1"
$env:GHOST_FL_SCRIPTING_PORT = "48767"
cargo run -p ghost-fl-agent -- --i-accept-live-fl-writes --scripting-bind 127.0.0.1:48767
```

## Exact live Windows / FL validation gate

1. Start loopMIDI and create/enable an endpoint named exactly `Ghost Midi`.
2. Install `device_Ghost.py` with `install.ps1` or copy it manually into FL Studio's user-data Hardware scripting directory.
3. Start `ghost-fl-agent` with the command above. The app should report the scripting listener at `127.0.0.1:48766`.
4. Start or reload FL Studio. Confirm the `Ghost Bridge` controller script is loaded/bound for `Ghost Midi`.
5. Open `http://127.0.0.1:48765` and confirm the **FL scripting bridge** indicator becomes connected.
6. Click **Run scripting probe**. Compare the reported selected channel, selected mixer track, pattern, arrangement selection, focused plugin/window, project state, song position, and loop mode against the visible FL UI.
7. Confirm the reversible mixer-selection test changes selection briefly and restores the exact original mixer track.
8. Keep FL Studio running, stop/restart the Rust app, and confirm `device_Ghost.py` reconnects without MIDI-device reconfiguration or script reinstallation.
9. Repeat the probe after reconnect.
10. Record any embedded-Python socket errors, callback timing issues, unexpected return types, or lifecycle differences before promoting any behavior into shared architecture.

## Deterministic validation

The branch must pass:

```text
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Focused Rust tests cover NDJSON framing, malformed messages, request-ID mismatch detection, bounded buffering, disconnect/reconnect state, loopback-only binding, and callable-name validation.

The bundled Python script has also been syntax-compiled with the local Python interpreter. That checks ordinary Python syntax only; it does not emulate FL Studio's embedded modules or callback runtime.

## Evidence not established here

This implementation does **not** claim live evidence yet for:

- FL automatically binding `Ghost Bridge` to `Ghost Midi`;
- embedded-Python socket behavior under real `OnIdle()` scheduling;
- exact live return values for every selected probe;
- reconnect behavior across a real Rust-process restart;
- the reversible mixer-selection mutation in the proprietary FL runtime.

Those facts remain the user's Windows/FL live gate.

## Shared-architecture implications

No shared architecture change is justified yet. If live testing repeatedly confirms the same lifecycle and framing requirements across more than this app, the useful evidence to reconsider later is:

- FL controller scripts can act as outbound loopback clients without MIDI carrying application payloads;
- bounded `OnIdle()` work is sufficient for request/response scripting calls;
- reconnect semantics survive Ghost process restarts;
- scripting-only current-state observations materially complement the frozen Gopher surface.

Until repeated evidence exists, the bridge remains app-local as required by ADR 001 and the workspace reset promotion rule.
