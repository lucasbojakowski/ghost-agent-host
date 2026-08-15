# FL Studio scripting bridge experiment

Branch: `feat/fl-scripting-bridge`

## Status and purpose

The raw Gopher integration is frozen as the proven FL baseline. `ghost-fl-studio` already mirrors the live Gopher catalog faithfully and `apps/ghost-fl-agent` has proven that a frontier agent can coordinate substantial real studio work through that surface.

This branch explores a second native FL surface: FL Studio MIDI Scripting. The goal is not to replace Gopher or invent a DAW abstraction. The goal is to establish the cleanest reliable Rust ↔ FL scripting transport and prove a small set of scripting-only capabilities.

Keep this experiment app-local until repeated use demonstrates a reusable boundary. Do not promote it into `ghost-application` during this branch.

## Architectural constraint

Use virtual MIDI only to make FL Studio auto-load the controller script. Do not use MIDI as the application RPC/data plane.

Target architecture:

```text
apps/ghost-fl-agent (Rust)
    │
    ├── existing Gopher/CDP path ───────────────► FL Studio/Gopher
    │
    └── localhost TCP server
             ▲
             │ outbound connection
             │
      device_Ghost.py inside FL
             ▲
             │
      virtual MIDI device
      auto-load/bootstrap only
```

For the first live test the user already has a loopMIDI endpoint. Use the device name:

```text
Ghost Midi
```

The installed script metadata should therefore begin with:

```python
# name=Ghost Bridge
# supportedDevices=Ghost Midi
```

Do **not** implement Windows MIDI Services/CoreMIDI virtual-device creation on this branch. That is a later platform/bootstrap experiment after the socket bridge is proven.

## Transport direction

The Rust app owns/listens on localhost. FL Studio connects outbound from the MIDI script.

For the first implementation, use an explicit configurable loopback address with a stable default suitable for manual testing, e.g. `127.0.0.1:48766`. Keep the endpoint configurable from the Rust app. A production discovery/authentication mechanism may follow later; do not let that block the transport proof.

The FL script must never block FL's UI/audio/control thread waiting for network data. The script should:

- use a nonblocking localhost socket;
- reconnect from `OnIdle()` with bounded backoff;
- maintain bounded receive/send buffers;
- process only bounded work per `OnIdle()` call;
- never run an unbounded receive/read loop;
- survive the Rust app restarting;
- close/reset cleanly on script/device shutdown.

Rust owns connection management, request correlation, timeouts, and diagnostics.

## Wire protocol v1

Prefer a tiny versioned newline-delimited JSON protocol for this first experiment. One JSON object per line keeps the FL implementation simple while still allowing arbitrary JSON strings safely.

Handshake/event example:

```json
{"type":"hello","protocol":1,"bridge":"ghost-fl-scripting","fl_version":"...","scripting_api_version":44}
```

Call:

```json
{"type":"call","id":17,"module":"patterns","function":"getPatternName","args":[4]}
```

Result:

```json
{"type":"result","id":17,"ok":true,"value":"Drums"}
```

Failure:

```json
{"type":"result","id":17,"ok":false,"error":{"kind":"call_failed","message":"..."}}
```

The protocol is an app-local experiment. Do not create a reusable RPC crate yet.

## Python bridge rules

The Python side must remain deliberately dumb:

```text
decode message
→ validate module/function
→ invoke FL scripting function
→ encode primitive/JSON-compatible result
→ return result
```

Do not put agent policy, semantic DAW concepts, workflow logic, retries, or orchestration in Python.

Do not use `eval`, `exec`, or arbitrary imports. Resolve only an explicit allowlist of FL scripting modules. Start with the domains that are relevant to the surface-gap experiment:

- `arrangement`
- `channels`
- `general`
- `mixer`
- `patterns`
- `playlist`
- `plugins`
- `transport`
- `ui`

A function call may use `getattr()` only after validating the requested module against that allowlist and validating that the named member exists and is callable.

## Initial proof surface

The first milestone is transport + observation, not exposing hundreds of scripting functions to the agent.

Prove at least these scripting-only/current-state reads through the socket bridge:

- FL/scripting API version;
- project title and changed flag;
- `general.safeToEdit()`;
- selected Channel Rack channel;
- selected mixer track;
- current pattern number/count/name;
- arrangement selection start/end and whether a selection is active when available;
- focused plugin/window caption;
- current song position / loop mode.

Then prove one small reversible mutation such as renaming/restoring a pattern or changing/restoring a harmless selection. Avoid destructive tests.

## Integration with `ghost-fl-agent`

Do not alter the frozen Gopher tool behavior or minimal raw-agent prompt.

Add scripting diagnostics as a separate experimental capability. Good first UX:

- scripting connection status in the existing HTML page;
- a `Run scripting probe` developer action;
- compact display of returned scripting observations;
- no automatic injection of the scripting surface into the Codex tool registry yet unless the transport proof is already clean and the change is trivial.

The goal of this branch is to prove the bridge. Model-facing tool design comes after transport evidence.

## Script installation

Keep the script source in the repository under the app, for example:

```text
apps/ghost-fl-agent/fl-script/device_Ghost.py
```

Implement or document a convenient Windows test installation path under the FL Studio user-data Hardware scripting directory. Support an override because Image-Line user-data locations can vary. Do not make virtual-MIDI creation part of this branch.

## Evidence/artifacts

The user has generated two important artifacts from the live/runtime environment:

- `fl_studio_api_dump.enriched.signatures.txt` — enriched/runtime scripting API signatures and documentation;
- `MCPTools.api.txt` — the 48-tool Gopher/MCP public surface.

The user intends to add them to the repository. If they are present, inspect them before implementation and use them to choose probes and avoid confusing Gopher capabilities with scripting-only capabilities. If they are not present yet, do not block the transport work; ask the user only if an exact scripting signature is required and cannot be established from the official Image-Line docs.

## Boundaries

Do not:

- modify `ghost-application`;
- introduce a generic `DawAdapter`;
- redesign `ghost-context`;
- replace the Gopher path;
- implement Windows MIDI Services/CoreMIDI virtual devices yet;
- use filesystem JSON as the request/response data plane;
- add a second agent/harness architecture;
- expose arbitrary Python execution.

Changes to `ghost-fl-studio` should happen only if this work reveals a genuine bug in the existing Gopher adapter. The scripting bridge itself stays in the app for now.

## Validation

Static/deterministic:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Add focused Rust tests for framing/parsing, request correlation, disconnect/reconnect state, malformed messages, and bounded buffering where practical.

Live Windows/FL test:

1. Create/enable loopMIDI endpoint `Ghost Midi`.
2. Install `device_Ghost.py` in FL's Hardware scripting directory.
3. Start the Rust app with its scripting listener enabled.
4. Start/reload FL Studio and confirm the script auto-binds to `Ghost Midi`.
5. Confirm FL initiates the localhost connection.
6. Run the scripting probe and verify the selected channel/mixer/pattern/selection/focus observations against the visible FL UI.
7. Restart the Rust app and confirm the FL script reconnects without reconfiguration.
8. Perform the reversible mutation test and restore the original state.

Record any embedded-Python/socket lifecycle findings in this document or a dedicated branch findings file before finishing.

## Success gate

This branch is successful when we have runtime evidence that:

```text
virtual MIDI autoload
        ↓
FL device script
        ↓
outbound nonblocking localhost socket
        ↓
Rust request/response correlation
        ↓
real FL scripting API calls
        ↓
verified live state
```

with no filesystem RPC and no MIDI payload protocol.