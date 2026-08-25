# ghost-fl-workspace

Live-proven direct-Codex composition of Ghost's two transparent FL Studio integration surfaces.

```text
Codex App Server
  |
  +-- complete live Gopher tool catalog
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

Status: **PROVEN as the combined Gopher + scripting primitive harness**.

The accepted live test established real FL context, scripting discovery/description/calls, and agent behavior that combines scripting observations with Gopher operations in one project.

`ghost-fl-agent` remains the frozen direct-Codex Gopher-only control group. Do not move the scripting gateways into it.

## Progressive scripting disclosure

The checked-in scripting catalog contains hundreds of functions and overloads. This app intentionally exposes only three scripting gateway definitions:

- `fl_scripting_search(query, module?)` — deterministic metadata search;
- `fl_scripting_describe(module, function)` — exact evidenced metadata, overloads and bridge support;
- `fl_scripting_call(module, function, args)` — invoke one metadata-approved primitive with positional JSON arguments.

This keeps the active model tool surface bounded while retaining access to the larger FL scripting API.

The lower `ghost-fl-scripting` adapter owns module/function validation and rejects calls whose checked-in evidence does not establish a bridge-compatible wire shape.

## Point-in-time FL context

Before every Codex turn the app reads a compact scripting snapshot containing available values for:

- scripting/FL version;
- project title and changed flag;
- `safeToEdit`;
- selected channel and mixer track;
- mixer track count;
- current pattern/count/name;
- arrangement selection;
- focused plugin/window;
- song position/hint;
- loop mode and playback state.

The snapshot is explicitly **not** a durable semantic world model. It is point-in-time reasoning evidence and becomes stale as soon as the producer changes FL. The agent is instructed to re-observe when correctness depends on current state.

## Run

Prerequisites:

1. FL Studio is running with Gopher/CDP enabled.
2. The promoted controller script from `crates/ghost-fl-scripting/fl-script/` is installed.
3. The validated `ghost_native.cp312-win_amd64.pyd` is installed into FL Studio's shared Python library.
4. `Ghost Midi` exists as the current temporary controller-script bootstrap.
5. Codex App Server is available through the configured binary.
6. The open FL project is safe for the intended writes.

```powershell
cargo run -p ghost-fl-workspace -- --i-accept-live-fl-writes
```

Defaults:

```text
Gopher CDP:         127.0.0.1:9222
workspace UI:       127.0.0.1:48775
scripting listener: 127.0.0.1:48766
```

Open `http://127.0.0.1:48775`.

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

## Control matrix / MCP

The corresponding raw external-harness control group is `ghost-fl-mcp`, which currently exports only Gopher through MCP 2026-07-28.

The next independent experiment is the missing matrix cell: expose this expanded FL capability meaning to an external MCP harness without changing either raw control group.

Recommended first expanded MCP surface:

```text
complete live Gopher tools
+ fl_scripting_search
+ fl_scripting_describe
+ fl_scripting_call
+ fl_context_snapshot
```

See `docs/FL_CAPABILITY_SURFACES.md`.

## Still out of scope

This app is a primitive composition harness. It does not yet define the production workspace model, skills, intents, semantic entity graph, plugin profiles, persistent episode model or dynamic semantic tool compiler.

Those are the next app-layer experiments now that the lower surfaces are proven.

Canonical status: `docs/PROVEN_BASELINES.md`.
