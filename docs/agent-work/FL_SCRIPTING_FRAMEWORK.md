# FL Studio scripting framework promotion

Branch: `feat/fl-scripting-framework`

## Decision

The FL Studio MIDI Scripting experiment has crossed the promotion threshold.

The previous `feat/fl-scripting-bridge` branch established a live-proven second FL control surface in addition to Gopher:

```text
Ghost Midi
  │ bootstrap/autoload only
  ▼
device_Ghost.py inside FL Studio
  │
  ▼
ghost_native.cp312-win_amd64.pyd
  │ native nonblocking WinSock
  ▼
127.0.0.1:48766
  │
  ▼
Rust request/response bridge
  │
  ▼
real FL Studio MIDI Scripting API
```

The live proof reached FL Studio Producer Edition v26.1.3 build 5570, MIDI Scripting API 44, CPython 3.12.1 / cp312 / win_amd64, returned real project/selection/transport state, and completed a reversible mixer-selection mutation with exact restoration. See `docs/FL_SCRIPTING_JOURNEY.md` for the runtime investigation and baseline.

This branch intentionally changes one earlier conclusion: the scripting bridge is no longer merely app-local experimental glue. Scripting is a real FL-specific integration surface that multiple applications will need. Promote the transparent integration boundary into a dedicated lower-layer crate.

The target crate is:

```text
crates/ghost-fl-scripting/
```

It should sit beside, not inside, `crates/ghost-fl-studio/`:

```text
crates/
  ghost-fl-studio/      # transparent Gopher/CDP/native MCP surface
  ghost-fl-scripting/   # transparent FL MIDI Scripting surface
```

The two APIs have different transports, lifecycles, capability shapes, and runtime constraints. Combining them in one crate would blur proven boundaries. Applications may compose them.

## Architectural invariant

Both FL crates are lower-layer mirrors of real FL Studio surfaces.

`ghost-fl-studio` owns facts imposed by Gopher/CDP.

`ghost-fl-scripting` owns facts imposed by FL MIDI Scripting and the proven CPython/native bridge.

Neither crate owns Ghost product policy, semantic production workflows, intent models, agent prompts, skills, target scopes, plugin preferences, or business-domain constraints.

Use the same rule as ADR 001:

> If behavior exists because FL Studio / the scripting runtime behaves that way, it belongs in `ghost-fl-scripting`. If behavior exists because Ghost wants to behave that way, it belongs above the crate.

## Current live-proven baseline to preserve

The source branch contains the proven implementation under `apps/ghost-fl-agent`:

```text
apps/ghost-fl-agent/
  src/scripting_bridge.rs
  fl-script/device_Ghost.py
  fl-script/install.ps1
  fl-native/ghost_native.c
  fl-native/setup.py
  fl-native/build.ps1
  fl-native/ghost_native.cp312-win_amd64.pyd
```

The repository implementation currently uses **native nonblocking WinSock TCP plus versioned NDJSON**, not Python's `socket` module. Do not replace this merely because ordinary Python socket code looks simpler: the FL CPython subinterpreter's audit path was live-proven broken for `_socket.socket()` and ordinary file I/O. `_ctypes` was also unavailable in the subinterpreter. A subinterpreter-compatible native extension is the proven boundary.

The known-good live baseline recorded in the journey is:

```text
live-proven code commit: b38f1810fd2fd5b48ece57cccb66cac2790304a9
FL Studio: Producer Edition v26.1.3 [build 5570]
MIDI scripting API: 44
embedded Python: CPython 3.12.1 / cp312 / win_amd64
native extension API: 1
Rust listener: 127.0.0.1:48766
wire protocol: NDJSON v1
bootstrap MIDI device: Ghost Midi
```

Current runtime invariants:

- virtual MIDI is bootstrap/autoload only;
- FL initiates the outbound loopback connection;
- Python performs bounded nonblocking work from `OnIdle()`;
- the native extension owns the OS networking boundary;
- Rust owns request IDs, single-flight correlation, timeouts, status and diagnostics;
- protocol frames are bounded;
- reconnect uses bounded backoff;
- no arbitrary Python execution is exposed;
- public scripting functions are invoked by explicit module/function identity;
- the first write proof checks `general.safeToEdit()`, mutates selection, verifies, restores, and verifies again.

Preserve these until a replacement has passed the same real FL gate.

## Crate responsibility

`ghost-fl-scripting` should own the reusable FL-specific scripting integration:

```text
crates/ghost-fl-scripting/
  Cargo.toml
  README.md
  src/
    lib.rs
    adapter.rs
    protocol.rs
    ...
  fl-script/
    device_Ghost.py
    install.ps1
  fl-native/
    ghost_native.c
    setup.py
    build.ps1
    ... intentional distributable artifact if retained
```

Exact file splitting is implementation detail. The ownership boundary is not.

The crate should expose an API along these lines, using names that fit the implementation cleanly:

```text
FlScriptingConfig
FlScriptingAdapter
FlScriptingStatus
FlScriptingHello
FlScriptingError
FlScriptingCallResult / serde_json::Value
```

Core operations:

```text
start/listen
status
call(module, function, positional_args)
```

The adapter may remain synchronous/single-flight initially because the live bridge currently serializes calls and FL executes bounded work on `OnIdle()`. Do not introduce fake concurrency that outruns the FL callback model.

The crate must not depend on:

- `ghost-codex`;
- `ghost-context`;
- `ghost-application`;
- `ghost-audio`;
- `ghost-fl-studio`.

It may use normal low-level Rust dependencies needed for serialization/errors/transport implementation.

## Protocol and Python/native ownership

The scripting protocol is part of this FL integration boundary and should move with the crate.

Rust side owns:

- loopback listener;
- connection acceptance;
- hello/version validation;
- request IDs and correlation;
- timeout/disconnect semantics;
- bounded frame parsing;
- status/diagnostics;
- explicit FL scripting calls.

`device_Ghost.py` owns:

- FL controller metadata / autoload identity;
- FL callback lifecycle;
- bounded `OnIdle()` scheduling;
- reconnect backoff;
- framing buffers;
- mapping known FL scripting modules to imported module objects;
- validating public function identifiers;
- invoking FL scripting functions positionally;
- converting supported values to JSON-compatible results.

The native CPython extension owns only the subinterpreter-safe OS transport primitive. It must not learn about FL modules, project entities, agent tools, production semantics, or Ghost policy.

Do not use `eval`, `exec`, arbitrary import-by-name, arbitrary filesystem access, or model-generated Python source.

## Transparent scripting surface

The current proof script contains a small module allowlist selected for the original experiment. The promoted framework must treat module/function support as an FL capability question rather than a Ghost policy question.

Use the runtime/documentation artifacts under `docs/daw-apis/fl-studio/` as evidence. Search for:

```text
fl_studio_api_dump.enriched.signatures.txt
MCPTools.api.txt
MCPTools.api.json
```

The enriched scripting artifact describes hundreds of functions across FL modules. If the enriched signatures file is present, use it as the primary checked-in scripting catalog artifact. If it is absent, do not invent signatures; preserve the runtime bridge and document the missing artifact clearly.

A transparent framework may still reject calls that cannot cross the JSON bridge safely. Unsupported argument/result shapes must fail explicitly. Do not silently coerce FL objects, callbacks/eventData, bytes, or opaque handles into guessed semantics.

The Python side should import only explicitly known FL scripting modules. That is a security/runtime boundary, not a product tool policy.

## Capability metadata

The lower layer should make it possible for applications to discover what the scripting bridge knows without exposing hundreds of tool schemas directly to an LLM.

Preferred shape:

```text
ScriptingFunctionDefinition {
    module
    function
    signature / positional parameters when known
    return metadata when known
    description when known
    minimum API version when known
    bridge-callable status / unsupported reason when needed
}

FlScriptingManifest {
    connected scripting API version
    FL version
    functions/modules metadata
}
```

The manifest is descriptive. It must not classify functions as "mixing", "safe for agents", "preferred", "destructive", etc. Those are application concerns.

Do not hand-author hundreds of Rust structs for individual FL functions. Preserve a generic transparent call boundary and machine-readable metadata.

## Event direction

FL MIDI Scripting is also an event surface. The long-term framework should be capable of forwarding raw, bounded FL callback events upward so applications can maintain fresh projections without polling the whole project.

Examples of useful categories include project load/change, dirty mixer/channel state, transport state, focus/selection changes, and refresh invalidation.

For this branch, request/response extraction is mandatory. Generic event framing may be implemented if it can be done without destabilizing the proven call path. If added:

- forward raw callback identity and primitive arguments;
- bound event queues;
- coalesce only when the coalescing rule is imposed by the FL callback semantics and is loss-safe;
- do not build a semantic project graph in the crate;
- do not stream high-frequency meters through the general agent/event path.

## Existing `ghost-fl-agent`

`apps/ghost-fl-agent` is a valuable frozen behavioral baseline.

After extraction it may depend on `ghost-fl-scripting` for its existing scripting status/probe developer panel, but its **agent tool registry must remain the frozen raw Gopher catalog**. Do not silently add scripting tools to that baseline app.

Move the reusable transport/protocol out of the app. Keep the probe composition app-local (or in a test/example) because the particular list of probe observations and the reversible mutation are a validation workflow, not the transparent adapter itself.

The original baseline continues to answer:

> How far can a frontier agent get using raw Gopher alone?

## New combined app

Create a new research app:

```text
apps/ghost-fl-workspace/
```

Its purpose is to answer the next question:

> What becomes possible when an agent and user-facing workspace can compose the frozen Gopher surface with the richer FL scripting observation/control surface?

Dependencies should be explicit:

```text
ghost-fl-workspace
  ├── ghost-fl-studio
  ├── ghost-fl-scripting
  └── ghost-codex
```

Do not route one FL adapter through the other.

Initial architecture:

```text
                        FL Studio
                     /             \
                Gopher/CDP      MIDI Scripting
                    │                │
                    ▼                ▼
          ghost-fl-studio    ghost-fl-scripting
                    \                /
                     \              /
                      ▼            ▼
                   ghost-fl-workspace
                           │
                     persistent agent
                           │
                          UI
```

### Combined app tool exposure

Preserve the direct Gopher tools exactly as the proven baseline does.

Do **not** register hundreds of individual scripting functions into the model context.

Instead add a small app-owned progressive-disclosure gateway over the scripting manifest, for example:

```text
fl_scripting_search(query, optional_module)
fl_scripting_describe(module, function)
fl_scripting_call(module, function, args)
```

Exact names may vary. The important properties are:

- search/describe are metadata/discovery;
- call remains a raw scripting operation;
- the lower crate has no agent-tool dependency;
- the app decides what is exposed to the model;
- schemas/descriptions come from real scripting metadata, not invented Ghost semantics.

A compact app-owned current-workspace snapshot may be compiled from scripting reads for UI/context, but keep it explicitly as an application projection, not a new lower-layer truth model.

Useful first snapshot fields are already live-proven:

```text
FL/scripting version
project title / changed flag / safeToEdit
selected channel
selected mixer track / mixer count
current pattern / pattern count / pattern name
arrangement selection
focused plugin / focused window
song position / musical position hint
loop mode / playing state
```

The app should make the source of each observation explicit and re-resolve live state before mutations whose correctness depends on it.

## First hybrid acceptance scenario

The new app must prove at least one task where scripting supplies context and Gopher supplies the mutation.

Example shape:

```text
1. User selects a mixer track in FL.
2. Scripting resolves the selected mixer index/name.
3. User asks Ghost to route "this" track to an existing bus.
4. Agent uses the frozen Gopher routing tools.
5. The app re-observes enough state to verify the result.
```

Use a fresh/disposable project and preserve unrelated routing.

Also prove at least one scripting-only task through the progressive gateway, such as pattern metadata, arrangement selection, project dirty/safety state, plugin focus, or richer step data.

Do not change the original Gopher benchmark prompt to force scripting usage.

## Native artifact/build hygiene

The source branch currently contains both source and generated native build outputs. Treat the known-good `.pyd` as runtime evidence until a rebuilt artifact passes the live gate.

During extraction:

- do not delete the live-proven native artifact before replacement validation;
- do not proliferate transient compiler intermediates;
- move/clean build outputs only after understanding which files are intentional distributables versus generated scratch files;
- keep FL closed when replacing a loaded `.pyd`;
- keep the CPython ABI/version constraint explicit.

A future Rust-owned native core behind the proven CPython shim is reasonable, as described in `docs/FL_SCRIPTING_JOURNEY.md`, but it is not required for this extraction. Change one boundary at a time.

## What is explicitly NOT part of this branch

Do not yet implement the full higher-level Ghost harness discussed for later work:

- intent fields;
- entity/project semantic graphs;
- plugin preference/profile databases;
- automatic plugin-parameter subset generation;
- skill registries;
- specialist agent activation;
- semantic action tool compiler;
- generic DAW adapter;
- multi-provider harness abstraction;
- audio-analysis-driven planning;
- automatic intent mutation;
- `ghost-application` promotion.

This branch creates the clean primitive layer and the combined app needed to investigate those components with real state and real tools.

## Validation

Deterministic Rust gate:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Preserve/add focused tests for:

- loopback-only binding;
- hello/protocol validation;
- framing and maximum frame size;
- partial-frame buffering;
- malformed frames;
- request correlation;
- timeout/disconnect/reconnect status;
- safe callable identifiers;
- scripting catalog parsing/filtering if implemented;
- app-level search/describe result determinism;
- original `ghost-fl-agent` Gopher-only registry behavior.

Windows/native gate:

- build the CPython 3.12 x64 native extension from its new location;
- install the controller script/native artifact using the new crate-owned paths;
- run the same live scripting probe;
- verify API 44 / FL version handshake;
- verify representative state observations;
- verify reversible mixer selection changes and restores exactly;
- restart the Rust app while FL remains open and verify reconnect;
- verify the frozen Gopher path still works.

Combined-app live gate:

- start both FL surfaces;
- connect `ghost-fl-workspace`;
- show scripting + Gopher readiness separately;
- execute one hybrid scripting-context → Gopher-mutation task;
- execute one scripting-only task;
- verify final FL state from live reads;
- record the trajectory so later tool/skill work has evidence.

## Promotion rationale

This promotion is justified by concrete reuse evidence rather than architectural preference:

1. the scripting transport is live-proven in the proprietary FL runtime;
2. it exposes state and controls materially absent from the frozen Gopher catalog;
3. future FL applications need selection/focus/project/pattern/timeline/undo-style capabilities regardless of any one agent workflow;
4. the transport/runtime invariants are FL-specific and should not be reimplemented independently by every app;
5. the Gopher and scripting boundaries are both real FL surfaces and should remain independently transparent.

This is the correct lower layer from which to investigate richer Ghost tools, workspace projections, skills, context compilation, and intent-driven applications.