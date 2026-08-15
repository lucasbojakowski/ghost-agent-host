# Implementation prompt — FL scripting bridge

You are implementing the FL Studio scripting bridge experiment for Ghost & Guild.

Repository:

```text
lucasbojakowski/ghost-agent-host
```

Work branch:

```text
feat/fl-scripting-bridge
```

Do not work from another feature branch. Do not merge or close existing PRs, delete remote branches, or modify unrelated architecture.

This is an implementation task, not a planning exercise.

## Read first

Before changing code, read these files in full from this branch:

- `README.md`
- `apps/ghost-fl-agent/README.md`
- `docs/TECHNICAL_RETROSPECTIVE.md`
- `docs/WORKSPACE_MIGRATION_PLAN.md`
- `docs/decisions/001-transparent-fl-studio-adapter.md`
- `docs/agent-work/FL_SCRIPTING_BRIDGE.md`

Also locate and inspect these artifacts if the user has added them to the repository:
(docs/daw-apis/fl-studio)

- `fl_studio_api_dump.enriched.signatures.txt`
- `MCPTools.api.txt`

Search the repository rather than assuming their final paths.

Treat `docs/agent-work/FL_SCRIPTING_BRIDGE.md` as the authoritative scope for this branch.

## Proven baseline you must preserve

The existing `apps/ghost-fl-agent` is a runtime-proven raw FL agent over the full live Gopher catalog. The existing Gopher path is frozen for this experiment.

Do not redesign its tool catalog, raw-agent prompt, or `ghost-fl-studio` adapter semantics.

The architectural rule from the reset remains in force:

> Policy and experimental composition stay in `apps/*`; logic is promoted downward only after repeated real workflows demonstrate reuse.

Do not populate `ghost-application` with speculative abstractions.

## Goal

Implement a clean experimental FL Studio MIDI Scripting bridge with this topology:

```text
apps/ghost-fl-agent (Rust)
    │
    └── localhost TCP listener
             ▲
             │ FL initiates outbound connection
             │
      device_Ghost.py
             ▲
             │ auto-loaded by FL
             │
      Ghost Midi virtual MIDI endpoint
```

For the initial runtime test, the user already provides `Ghost Midi` using loopMIDI. Do not implement Windows MIDI Services or CoreMIDI virtual-device creation yet.

Virtual MIDI is bootstrap/autoload only. Application RPC goes exclusively over localhost sockets.

## Required implementation

1. Add an app-local scripting bridge server to `apps/ghost-fl-agent`.
2. Bind only to loopback, with a configurable address and a sensible test default such as `127.0.0.1:48766`.
3. Add the FL controller script under the app, e.g. `apps/ghost-fl-agent/fl-script/device_Ghost.py`.
4. The script metadata must support the current test device:

   ```python
   # name=Ghost Bridge
   # supportedDevices=Ghost Midi
   ```

5. FL must connect outbound to the Rust listener.
6. The FL script must use nonblocking socket I/O and perform bounded connection/read/dispatch/write work from `OnIdle()` so no FL callback blocks on network I/O.
7. Implement reconnect behavior so restarting Ghost does not require reconfiguring/reloading the MIDI device.
8. Use a tiny versioned NDJSON request/response protocol with correlated request IDs.
9. Python must expose only explicit FL modules and must never use `eval`, `exec`, or arbitrary imports.
10. Keep Python dumb: framing, validation, FL function invocation, response serialization.
11. Rust owns timeouts, correlation, diagnostics, and app state.
12. Add an observable connection/probe path to the existing app. Prefer a small status indicator plus a `Run scripting probe` developer action in the existing HTML UI rather than building a second UI.
13. Prove a set of scripting-only/current-state observations such as selected channel, selected mixer track, current pattern, arrangement selection, focused plugin, project changed flag, `safeToEdit`, song position, and loop mode.
14. Prove one reversible harmless scripting mutation and restore it.
15. Document the exact live validation procedure and any FL embedded-Python/socket lifecycle findings.

## Do not do yet

Do not:

- expose hundreds of scripting functions as Codex tools merely because the transport works;
- merge scripting and Gopher behind a new generic DAW abstraction;
- create reusable RPC/capability/application crates;
- change `ghost-context`;
- build virtual MIDI endpoints programmatically;
- use filesystem JSON for RPC;
- use MIDI SysEx as the RPC protocol;
- add an MCP server on this branch;
- implement semantic workspace projection;
- add audio analysis/Tap behavior to this bridge.

## Protocol constraints

The first protocol should be deliberately boring and inspectable. A request should look approximately like:

```json
{"type":"call","id":17,"module":"patterns","function":"getPatternName","args":[4]}
```

A successful response:

```json
{"type":"result","id":17,"ok":true,"value":"Drums"}
```

A failure must return a structured error without crashing/disconnecting the script unless the transport itself failed.

Allowlisted initial modules:

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

Validate the module and callable before invocation. JSON-compatible primitive/list/tuple results should serialize naturally. Represent unsupported return values as a clear bridge error rather than using arbitrary repr/eval behavior.

## Installation/testing ergonomics

Provide a clear way to install/copy the bundled script into the FL Studio user-data Hardware scripting directory for Windows testing. Support a path override. Do not spend the branch solving every possible Image-Line user-data layout.

The user's immediate environment uses loopMIDI and a device named `Ghost Midi`. Optimize the live instructions for that path while keeping the bridge code independent of loopMIDI.

## Validation and commits

Work in small coherent commits. Run static/deterministic validation between structural phases:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Add deterministic Rust tests around protocol/framing/correlation/disconnect behavior. FL Studio itself must be validated on the user's Windows machine; do not claim live success without that test.

When implementation is ready, summarize:

- files/architecture added;
- protocol shape;
- how to install the script;
- exact command to run Ghost;
- exact FL/loopMIDI validation steps;
- static test results;
- what still requires live validation;
- any evidence that should later influence `ghost-fl-studio` or another shared crate, without promoting it yet.
