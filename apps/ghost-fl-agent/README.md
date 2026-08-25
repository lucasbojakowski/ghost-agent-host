# ghost-fl-agent

`ghost-fl-agent` is the **frozen direct-Codex raw Gopher control group**.

Its Codex dynamic-tool registry must remain exactly the complete live Gopher catalog. FL MIDI Scripting is not registered as agent tools in this app.

```text
browser/chat
    -> persistent Codex thread
    -> complete live Gopher manifest
    -> ghost-fl-studio
    -> FL Studio
```

This baseline proved that a frontier agent can coordinate substantial real FL work through the faithful Gopher surface with only minimal live-state instructions.

Canonical baseline status: `docs/PROVEN_BASELINES.md`.

## Why keep this app frozen

Later applications add scripting, semantic workspace state, audio evidence, optimized tools and skills. Keeping this app raw gives those layers a behavioral control group.

Do not add to its Codex registry:

- FL scripting gateways;
- semantic entity tools;
- Ghost Tap/audio-analysis tools;
- plugin profiles;
- skills or intent abstractions.

If those capabilities are needed, use or extend a separate app.

## Gopher integration

At startup the app loads the complete live `FlStudioManifest` and registers every advertised `NativeToolDefinition` directly into one persistent Codex thread.

`ghost-fl-studio` owns only real Gopher integration invariants such as:

- live schemas;
- argument ordering/canonicalization;
- callback/result normalization;
- native error detection;
- single-flight calls.

The coarse `--i-accept-live-fl-writes` gate remains because the raw catalog includes destructive operations.

## Developer scripting diagnostics

The app retains developer-only scripting status/probe endpoints as a regression convenience. They now consume the reusable `ghost-fl-scripting` crate.

```text
browser
  -> GET /api/scripting/status
  -> POST /api/scripting/probe
  -> ghost-fl-scripting
  -> FL MIDI Scripting API
```

This diagnostic path does **not** alter the Codex tool registry.

For the promoted scripting framework and live hybrid agent, see:

- `crates/ghost-fl-scripting/`
- `apps/ghost-fl-workspace/`
- `docs/agent-work/FL_SCRIPTING_FRAMEWORK_VALIDATION.md`
- `docs/FL_SCRIPTING_JOURNEY.md`

## Run

Prerequisites:

1. FL Studio is running with Gopher/CDP enabled.
2. Codex App Server is available through the configured binary.
3. The open project is safe for the intended live writes.

```powershell
cargo run -p ghost-fl-agent -- --i-accept-live-fl-writes
```

Open `http://127.0.0.1:48765`.

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

The scripting options affect developer diagnostics only.

## Benchmark Session A

The benchmark-session prompt remains available under:

```text
apps/ghost-fl-agent/prompts/setup-benchmark-session.md
```

It is useful as a repeatable raw-Gopher control fixture for later harness/tool comparisons.

## Current architecture references

Prefer these documents over completed experiment prompts:

- `docs/PROVEN_BASELINES.md`
- `docs/SDK_ARCHITECTURE.md`
- `docs/FL_CAPABILITY_SURFACES.md`

Historical scripting transport investigation is preserved in `docs/FL_SCRIPTING_JOURNEY.md`.
