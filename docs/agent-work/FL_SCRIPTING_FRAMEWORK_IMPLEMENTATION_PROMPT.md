# Implementation prompt — promote FL scripting into a reusable lower layer

You are implementing the next FL Studio integration phase for Ghost & Guild.

Repository:

```text
lucasbojakowski/ghost-agent-host
```

Work branch:

```text
feat/fl-scripting-framework
```

Do not work from another feature/fix branch. Fetch this branch first and confirm you are on it before changing anything.

This is an implementation task, not a planning exercise.

## Read first, in full

Before changing code, read these files from this branch:

1. `README.md`
2. `apps/ghost-fl-agent/README.md`
3. `docs/TECHNICAL_RETROSPECTIVE.md`
4. `docs/WORKSPACE_MIGRATION_PLAN.md`
5. `docs/decisions/001-transparent-fl-studio-adapter.md`
6. `docs/FL_SCRIPTING_JOURNEY.md`
7. `docs/agent-work/FL_SCRIPTING_BRIDGE.md`
8. `docs/agent-work/FL_SCRIPTING_BRIDGE_FINDINGS.md`
9. `docs/agent-work/FL_SCRIPTING_FRAMEWORK.md`
10. `crates/ghost-fl-studio/src/lib.rs`
11. `crates/ghost-fl-studio/src/adapter.rs`
12. `apps/ghost-fl-agent/src/main.rs`
13. `apps/ghost-fl-agent/src/scripting_bridge.rs`
14. `apps/ghost-fl-agent/fl-script/device_Ghost.py`
15. `apps/ghost-fl-agent/fl-native/README.md`
16. `apps/ghost-fl-agent/fl-native/ghost_native.c`

Treat:

```text
docs/agent-work/FL_SCRIPTING_FRAMEWORK.md
```

as the authoritative architecture/scope document for this branch.

Also search the repository before implementation for these research artifacts:

```text
fl_studio_api_dump.enriched.signatures.txt
MCPTools.api.txt
MCPTools.api.json
```

Current known repository location for the Gopher artifacts is under `docs/daw-apis/fl-studio/`, but do not assume the enriched scripting artifact has the same final path. Search.

## Proven live facts — do not regress them

The preceding scripting experiment is no longer speculative.

The user live-proved the complete loop in FL Studio:

```text
Ghost Midi
  -> FL auto-loads device_Ghost.py
  -> device_Ghost.py imports ghost_native
  -> ghost_native uses native nonblocking WinSock
  -> FL connects outbound to 127.0.0.1:48766
  -> Rust sends versioned NDJSON calls
  -> Python dispatches real FL scripting functions
  -> results return to Rust
```

Observed runtime:

```text
FL Studio: Producer Edition v26.1.3 [build 5570]
MIDI scripting API: 44
embedded Python: CPython 3.12.1 / cp312 / win_amd64
native extension API: 1
wire protocol: NDJSON v1
bootstrap MIDI device: Ghost Midi
```

Representative live observations included project dirty/safety state, selected channel, selected mixer track, mixer count, current pattern, arrangement selection, focused plugin/window, song position, loop mode and transport state.

The reversible mutation proof passed:

```text
attempted=true changed=true restored=true
```

The current repository journey records the known-good live code commit as:

```text
b38f1810fd2fd5b48ece57cccb66cac2790304a9
```

Use the current branch head as your source tree, but treat that live-proven behavior as the regression baseline.

Important runtime discovery:

- ordinary Python `_socket.socket()` is not viable in FL's subinterpreter because the audit path is broken;
- ordinary Python file I/O hit the same audit problem;
- `_ctypes` cannot load in this subinterpreter;
- a custom CPython multi-phase native extension declaring subinterpreter support loads correctly;
- native nonblocking WinSock from that extension works.

Do not "simplify" the bridge back to Python sockets, filesystem RPC, `ctypes`, or arbitrary native tricks.

The implementation currently uses raw loopback TCP + bounded NDJSON. Preserve the actual proven protocol unless a change is independently justified and regression-tested.

## Goal 1 — create the lower-layer crate

Create:

```text
crates/ghost-fl-scripting/
```

Add it to the Rust workspace.

This crate is the transparent FL MIDI Scripting adapter, analogous in layering discipline to `ghost-fl-studio` but independent of it.

It should own the reusable Rust bridge plus the controller/native assets that are inseparable from this FL-specific integration.

Move/extract the appropriate code/assets from:

```text
apps/ghost-fl-agent/src/scripting_bridge.rs
apps/ghost-fl-agent/fl-script/
apps/ghost-fl-agent/fl-native/
```

into the crate.

Do not merely duplicate the implementation and leave two divergent copies.

A reasonable target shape is:

```text
crates/ghost-fl-scripting/
  Cargo.toml
  README.md
  src/
    lib.rs
    adapter.rs
    protocol.rs
  fl-script/
    device_Ghost.py
    install.ps1
  fl-native/
    ghost_native.c
    setup.py
    build.ps1
    ...
```

You may adjust file names/splitting if that makes the crate clearer.

## Public API intent

Expose a small reusable Rust API. Exact names may vary, but the responsibilities should be obvious, for example:

```rust
FlScriptingConfig
FlScriptingAdapter
FlScriptingStatus
FlScriptingHello
FlScriptingError
```

Core behavior:

```text
start/listen
status
call(module, function, positional_args)
```

Keep calls generic and transparent. Do not hand-code one Rust method per FL scripting function.

Preserve the current single-flight semantics unless you implement and prove a better correlation/queue model. FL runs bounded work from `OnIdle()`; do not create apparent parallelism that can race the embedded scripting runtime.

The crate must not depend on:

```text
ghost-codex
ghost-context
ghost-application
ghost-audio
ghost-fl-studio
```

No model/tool/harness types belong in this crate.

## Goal 2 — keep the Python/native boundary narrow

The Python controller remains a thin FL adapter.

It may:

- participate in FL controller callbacks;
- reconnect from `OnIdle()`;
- process bounded frames/work;
- import explicitly known FL scripting modules;
- validate public function identifiers;
- call FL scripting functions positionally;
- serialize supported primitive/list/tuple/dict return values.

It must not contain:

- agent prompts or policies;
- semantic project entities;
- mixing logic;
- plugin preferences;
- task planning;
- retries that belong to the caller;
- arbitrary imports from model input;
- `eval` / `exec`;
- arbitrary Python source execution.

The native `.pyd` remains an OS transport primitive only.

Do not teach the C extension about FL Studio modules, project state, tool schemas or Ghost semantics.

A later Rust-owned native transport core behind the proven CPython shim is allowed only if it is a controlled refactor and the live gate remains intact. It is not required to complete this branch.

## Goal 3 — make scripting capability metadata reusable

Inspect the enriched FL scripting API artifact if present.

Create a machine-readable scripting capability/catalog layer in `ghost-fl-scripting` sufficient for applications to discover functions without dumping hundreds of schemas into an agent context.

Preferred metadata contains, when evidence exists:

```text
module
function
signature / positional parameters
return metadata
description
minimum scripting API version
whether the current JSON bridge can represent the call
explicit unsupported reason when it cannot
```

Do not invent signatures or return types.

Do not classify functions by Ghost business semantics in the crate.

Do not hide unsupported wire shapes by coercing them. Fail explicitly.

The current Python module allowlist was selected for the first probe. Revisit it as an FL integration boundary using the actual scripting artifact. Expand only to explicitly known FL modules and keep arbitrary import-by-name impossible.

## Goal 4 — preserve `ghost-fl-agent` as the Gopher-only behavioral baseline

Refactor `apps/ghost-fl-agent` to consume `ghost-fl-scripting` for its existing developer scripting status/probe path if needed.

But preserve this invariant:

```text
Codex registry in ghost-fl-agent == complete live Gopher catalog only
```

Do not register scripting functions or scripting gateway tools into this baseline app.

Its minimal Gopher system prompt must remain behaviorally unchanged except for necessary path/build documentation updates.

The app's scripting probe composition may remain app-local because the exact probe list and reversible selection workflow are validation behavior, not the lower adapter contract.

Prove with a deterministic test that building the raw agent registry still registers exactly the live Gopher definitions supplied by the manifest, with no scripting definitions mixed in.

## Goal 5 — create a new combined app

Create:

```text
apps/ghost-fl-workspace/
```

Add it to the workspace.

This is a new research application. Do not overload `ghost-fl-agent` with the next experiment.

Dependencies:

```text
ghost-fl-studio
ghost-fl-scripting
ghost-codex
```

No `ghost-application` dependency unless an existing unavoidable compile dependency already exists; do not promote new abstractions there.

The app should connect both FL surfaces independently and report readiness separately.

### Agent surface

Register the complete live Gopher manifest exactly as the proven baseline does.

Do not register hundreds of scripting functions individually.

Add a small progressive-disclosure scripting gateway owned by the app, conceptually:

```text
fl_scripting_search
fl_scripting_describe
fl_scripting_call
```

The exact tool names/schemas are yours to implement cleanly.

Requirements:

- search/describe use the scripting catalog/manifest;
- call delegates to `ghost-fl-scripting`;
- results preserve raw FL scripting values/errors;
- tool definitions live in the app, not the crate;
- no semantic production policy is encoded yet;
- no arbitrary Python execution is possible;
- discovered descriptions/signatures come from real checked-in metadata.

If the enriched scripting artifact is absent, do not fabricate a fake full catalog. Implement the adapter cleanly, expose only evidence-backed metadata, and clearly report the missing input in the final branch notes.

### Initial workspace projection

Build an app-owned compact current workspace snapshot for UI/context using scripting reads that have already been live-proven.

Good initial fields:

```text
scripting API / FL version
project title
project changed flag
general.safeToEdit
selected channel
selected mixer track
mixer track count
current pattern / count / name
arrangement selection start/end/active
focused plugin
focused window
song position / hint
loop mode
playing state
```

Keep this structure inside the app. Do not promote it into `ghost-context` or a new generic project model yet.

Expose the snapshot to the local UI and make it available to the agent in a compact form without replacing live re-observation.

Earlier observations are snapshots, not permanent truth.

### UI

A small developer/research UI is sufficient. Reuse the existing dependency-free HTML approach if that is fastest.

It should show at minimum:

```text
Gopher status/tool count
scripting connection/hello status
current workspace snapshot
chat
agent tool trajectory
```

The goal is empirical combined-surface testing, not polished product UI.

## Goal 6 — prove hybrid behavior

Create at least one explicit hybrid live scenario where scripting supplies current user/project context and Gopher performs the mutation.

Suggested scenario:

```text
1. user selects a mixer track in FL;
2. scripting observes its current index/name;
3. user asks the agent to route "this" track to an existing named bus;
4. agent resolves current routing before changing it;
5. Gopher performs the routing mutation;
6. final state is re-read/verified;
7. unrelated sends/routes remain intact.
```

Also exercise at least one scripting-only capability through the search/describe/call gateway, e.g. current pattern metadata, arrangement selection, project safety/dirty state, focused plugin/window, plugin preset navigation, mixer wet mix, or richer step state.

Do not change `apps/ghost-fl-agent/prompts/setup-benchmark-session.md` to require scripting. That prompt belongs to the Gopher-only baseline.

## Optional raw event support

FL scripting callbacks are valuable for later realtime project projections.

If it is straightforward without destabilizing the proven call path, extend protocol v1 compatibly to support bounded raw event messages such as:

```json
{"type":"event","name":"...","args":[...]}
```

Only forward evidence-backed FL callback events and primitive arguments.

Do not build a semantic event bus/project graph in `ghost-fl-scripting`.

Do not make high-frequency meter streaming part of the acceptance gate.

If event support materially expands risk, document it as the immediate next task instead of forcing it into this extraction.

## Build/native artifact hygiene

The source branch currently tracks native sources plus generated build outputs and a known-good `.pyd`.

Be careful:

- preserve the known-good live artifact until its replacement is runtime-proven;
- do not delete evidence before validation;
- avoid keeping duplicate/transient compiler intermediates after the new build path is established;
- document exactly which native artifact is distributable versus generated scratch output;
- FL must be closed before replacing a loaded `.pyd`;
- keep the cp312/win_amd64 ABI requirement explicit.

If you clean tracked build intermediates, do it only after the new crate-owned native build succeeds and the reason is documented.

## Do not implement the next harness architecture yet

The user intends to design a richer layer after this branch involving semantic deterministic tools, skills, progressive disclosure, plugin profiles, workspace projections, intent fields, references and dynamic agent context.

Do not preempt that design by building speculative generic registries here.

Specifically, do not create:

```text
UniversalTool
SkillRegistry
DawAdapter
IntentGraph
EntityGraph
PluginKnowledgeBase
UniversalAgentRuntime
```

unless an existing type with that exact responsibility is already required by the current code (it should not be).

This branch should deliver the real lower primitives and a combined empirical app from which those abstractions can be designed.

## Validation

Run the repository deterministic gate:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Also syntax-check the Python controller with an ordinary matching Python interpreter where practical. This does not replace FL runtime validation.

Add focused tests for the extracted crate and combined app.

### Required live Windows/FL regression

On the user's Windows/FL machine:

1. make the `Ghost Midi` loopMIDI endpoint available;
2. build/install the controller script and native extension from the **new crate-owned paths**;
3. start the relevant Rust app/listener;
4. verify FL auto-loads the Ghost controller;
5. verify hello reports scripting API 44 and the expected FL build;
6. run the existing state probe;
7. compare returned selection/pattern/timeline/project values to visible FL state;
8. run the reversible mixer-selection test and confirm exact restoration;
9. restart the Rust app while FL remains open and verify reconnect;
10. verify the frozen Gopher path still operates normally.

### Combined app acceptance

Then run `ghost-fl-workspace` and verify:

1. both Gopher and scripting are independently connected;
2. scripting metadata search/describe works;
3. scripting call works;
4. initial workspace snapshot matches FL;
5. one scripting-context → Gopher-mutation scenario succeeds;
6. one scripting-only task succeeds;
7. final claims are backed by live reads/tool results.

Record the exact live result in a new findings/validation document before declaring the branch complete.

## Working style

Implement in small coherent commits.

Do not merge or close unrelated PRs.

Do not delete remote branches.

Do not modify the canonical/default branch.

Do not force-push existing shared branches.

If the repository state conflicts with this prompt, inspect the actual history/files and preserve proven runtime behavior first. Document the discrepancy rather than guessing.