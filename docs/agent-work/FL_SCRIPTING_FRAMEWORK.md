# FL Studio Scripting Framework

Status: **PROVEN / PROMOTED**

Current crate:

```text
crates/ghost-fl-scripting/
```

Current combined app:

```text
apps/ghost-fl-workspace/
```

The implementation/extraction phase is complete. Historical transport investigation is preserved in `docs/FL_SCRIPTING_JOURNEY.md`; accepted runtime/CI status is recorded in `FL_SCRIPTING_FRAMEWORK_VALIDATION.md`.

## Decision

FL MIDI Scripting is a reusable FL-specific integration surface and belongs in Core beside, not inside, `ghost-fl-studio`.

```text
crates/
  ghost-fl-studio/      transparent Gopher/CDP surface
  ghost-fl-scripting/   transparent FL MIDI Scripting surface
```

The two APIs have different transports, lifecycles and capability shapes. Applications may compose them; neither lower crate should route through the other.

## Architectural invariant

Both FL crates mirror real FL Studio behavior.

`ghost-fl-scripting` owns behavior required because the FL scripting runtime behaves that way:

- loopback listener and connection lifecycle;
- versioned NDJSON request/result protocol;
- request IDs/correlation and timeout/disconnect semantics;
- scripting catalog/evidence metadata;
- public module/function validation;
- FL controller-script lifecycle;
- the subinterpreter-compatible native transport boundary.

It does **not** own:

- agent prompts;
- product permissions/policy;
- semantic entities or intents;
- skills;
- plugin preferences;
- semantic production tools;
- MCP/Codex projection policy.

Rule:

> If behavior exists because FL Studio / its scripting runtime behaves that way, it belongs in `ghost-fl-scripting`. If behavior exists because Ghost wants to behave that way, it belongs above the crate.

## Proven runtime boundary

The accepted transport remains:

```text
FL controller script
  -> ghost_native CPython 3.12 multi-phase extension
  -> native nonblocking WinSock
  -> loopback NDJSON
  -> Rust ghost-fl-scripting adapter
```

Source runtime baseline from the live investigation:

```text
FL Studio: Producer Edition v26.1.3 build 5570
MIDI Scripting API: 44
embedded Python: CPython 3.12.1 / cp312 / win_amd64
native extension API: 1
wire protocol: 1
listener default: 127.0.0.1:48766
bootstrap MIDI device: Ghost Midi
```

The native extension exists because FL's embedded CPython subinterpreter was live-proven unreliable for ordinary audited Python socket/file constructors while the custom multi-phase extension works.

Do not replace this with normal Python `socket` code without reproducing the full live regression gate.

## Transparency and catalog

The crate exposes a generic module/function/positional-arguments call boundary plus checked-in capability metadata.

It must not hand-author semantic wrappers for hundreds of FL scripting functions merely to make them convenient for an agent.

Unsupported or insufficiently evidenced wire shapes fail explicitly rather than being guessed.

The scripting catalog is descriptive metadata, not product policy. It may include:

```text
module
function
signature/overloads
return metadata
description
minimum API version
bridge-callable status / unsupported reason
```

It must not classify functions as preferred mixing actions, safe for agents or business-domain skills.

## App-level progressive disclosure

`ghost-fl-workspace` proved the first useful agent projection:

```text
complete live Gopher catalog
+
fl_scripting_search
fl_scripting_describe
fl_scripting_call
+
compact point-in-time scripting context
```

The hundreds of scripting functions are therefore reachable without permanently adding hundreds of tool schemas to model context.

This projection remains app-owned. The lower crate has no dependency on `ghost-codex` or MCP.

## Frozen raw baseline

`apps/ghost-fl-agent` remains the Gopher-only direct-Codex control group.

It may use `ghost-fl-scripting` for developer diagnostics, but its Codex registry must remain exactly the live Gopher manifest.

## Future direction

The scripting surface is also a source of events/invalidation. Later work may forward bounded raw FL callback events upward so workspace projections can update incrementally.

If added, keep the lower event layer raw and bounded. Do not build semantic project graphs, skills or high-frequency model-facing meter streams in the crate.

## Regression gate

Future changes must preserve at least:

1. cross-platform Rust fmt/check/test/clippy;
2. Windows Python syntax + native extension build;
3. FL `.pyd` import in the scripting subinterpreter;
4. outbound nonblocking loopback connection;
5. hello/version metadata;
6. live state reads;
7. scripting search/describe/call behavior;
8. hybrid Gopher + scripting task behavior in the combined app;
9. bounded reconnect/OnIdle behavior;
10. no arbitrary Python execution.

See `FL_SCRIPTING_FRAMEWORK_VALIDATION.md` for the accepted baseline.
