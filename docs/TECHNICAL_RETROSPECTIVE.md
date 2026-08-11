# Ghost & Guild Technical Retrospective

Status: phase-reset source document

Date: 2026-08-10

Baseline: `8fa7d5f0d17c7019767f5e7b4fa6a084c191cb70`

## Purpose

This document records what the project actually learned while moving from an agentic nested-plugin host toward the first runtime-proven Ghost & Guild product slice.

It is intentionally retrospective rather than architectural. It explains what happened, which assumptions failed, which discoveries should survive the reset, and which development habits produced avoidable regressions. The target architecture, hard invariants, and next-phase roadmap are separate follow-up documents and should be written after the workspace cleanup described in `WORKSPACE_MIGRATION_PLAN.md`.

The most useful high-level representation we reached is:

```text
capture → analysis → agent → DAW
```

This is not merely a workflow diagram. It describes four distinct semantic domains and three meaningful transitions. The current product is the stable vertical slice formed by those domains: sense audio, derive evidence, reason with an agent, and act on the user's existing audio workspace.

## Executive conclusion

The central architectural correction was to stop treating Ghost as a plugin host and start treating it as an agentic layer over an existing audio workspace.

The early project put Ghost between FL Studio and third-party processors:

```text
FL Studio
  → Ghost outer plugin
    → Ghost child CLAP host
      → Pro-Q / Pro-C
```

That required Ghost to duplicate difficult responsibilities the DAW already owns well: plugin discovery, activation, audio topology, parameter event delivery, restart semantics, state, GUI parenting, focus, shortcuts, window lifetime, and processor routing.

The current vertical slice removes that duplication:

```text
FL Studio owns processors and routing
        ↑             ↓
    DAW control     audio
        ↑             ↓
      agent       Ghost Tap
        ↑             ↓
        └── analysis ─┘
```

Ghost Tap is now a deliberately small sensor. FL Studio remains the processor host. Ghost's Rust analysis turns captured sound into deterministic evidence. Codex App Server supplies persistent agent threads. The FL adapter exposes tightly scoped semantic actions and verifies the resulting DAW state.

The important product result is therefore not “an AI plugin successfully changed Pro-Q.” It is:

```text
real sound
  → deterministic measurement
  → model reasoning
  → semantic workspace action
  → native verification
```

That is the core of an agentic layer for audio workspaces.

## 1. The path we took

### 1.1 The original host-first architecture

The project began by making a Rust CLAP host capable of loading processors such as FabFilter Pro-Q 4 and Pro-C 3. The host grew into a substantial subsystem: child audio-port negotiation, restart handling, realtime parameter queues, child parameter flushing, plugin state, detached Windows GUI hosting, shortcut forwarding, semantic parameter mapping, and processor-specific behavior such as materializing Pro-Q bands before writing their values.

This work produced real technical knowledge. It also gradually moved the repository farther from the simplest product boundary.

The important mistake was not exploring nested hosting. Exploration was justified. The mistake was continuing to optimize that architecture after its cost became clearer than its product value.

### 1.2 The native FL control pivot

The project then tested FL Studio 2026's Gopher WebView/native tool surface. The raw Rust bridge proved that Ghost could:

- discover the Gopher target through WebView2/CDP;
- inspect the live native tool catalog;
- call FL tools from Rust;
- inspect third-party plugin parameters;
- write a plugin parameter and verify readback;
- insert an effect into an FL mixer slot and verify the result.

That changed the boundary. Ghost no longer needed to host a processor to control it.

### 1.3 The persistent agent-runtime pivot

The next correction was distinguishing the `codex` executable from the runtime architecture. The executable is the launcher for `codex app-server`; the useful product primitive is one persistent App Server process that owns multiple threads and dynamic tool registries.

The multi-thread smoke test proved that one initialized App Server could hold a controller thread and a differently scoped observer thread at the same time while Ghost handled tool execution and FL state verification.

This led to `CodexParallelRuntime`: a dispatcher capable of request-ID routing, per-thread tool registries, per-turn event delivery, concurrent turns on different threads, same-thread in-flight rejection, and fail-closed handling of ambiguous events.

The product workflow still uses one agent thread today. Building the parallel-capable runtime underneath it was valuable because it fixes the process/thread lifetime model without forcing a multi-agent product prematurely.

### 1.4 The Ghost Tap simplification

Loading the older nested host into the newer FL Studio build caused a crash without a useful error. Rather than add another child-GUI/host-lifetime patch, the DAW-loaded plugin was reduced to Ghost Tap:

- one stereo input;
- one stereo output;
- transparent passthrough;
- transport publication;
- preallocated bounded capture;
- no child processors;
- no custom GUI;
- no plugin parameter/state surface;
- no filesystem work on the realtime audio callback;
- a small non-realtime worker that publishes status and capture artifacts.

This is the point where the plugin became aligned with the product rather than competing with it.

### 1.5 The first real vertical slice

The current workflow now performs:

```text
Ghost Tap
  → capture a live FL signal
  → high-resolution Rust analysis
  → compact evidence projection
  → one Codex App Server thread
  → scoped FL processor tools
  → processor insertion / semantic parameter changes
  → native readback and mutation journal
```

The last major interoperability issue was continuous parameter translation. After settling plugin display reads correctly, the workflow succeeded with semantic display-domain settings rather than arbitrary normalized coordinates.

That is the milestone this retrospective freezes.

## 2. Common mistake pattern: architectural inertia

The most expensive recurring error was solving the next local problem without regularly re-evaluating the enclosing boundary.

Nested hosting created genuine problems and we solved many of them correctly. However, each solution made the subsystem more credible and therefore psychologically easier to keep. This is a classic local-optimization trap: a technically improving subsystem can still be moving away from the best product architecture.

Examples included:

- hardening child audio topology rather than asking whether Ghost should own child audio topology;
- fixing detached child GUI lifetime rather than asking whether Ghost should own vendor GUI lifetime;
- forwarding shortcuts rather than asking why a Ghost window should sit between the DAW and the vendor UI;
- compiling semantic mix plans into child parameter events rather than letting the DAW remain the authoritative host.

The corrective practice from now on should be explicit boundary review after every expensive integration discovery:

```text
Is this failure evidence that our implementation is incomplete,
or evidence that this responsibility belongs somewhere else?
```

A frontier agentic application benefits more from a thin trustworthy control boundary than from owning every underlying subsystem.

## 3. Common mistake pattern: believing apparent interface semantics

The Gopher integration repeatedly demonstrated that an interface's visible shape is not necessarily its runtime contract.

### 3.1 Named JSON arguments were order-sensitive

Gopher's tool call takes an `arguments` object, which naturally suggests ordinary named-argument semantics. In practice its dispatcher proved sensitive to insertion/property order. Incorrect order produced misleading runtime errors.

The successful rule became: fetch the live tool schema and emit arguments in that schema's property order before calling `runJson`.

This should be understood as a compatibility invariant, not an incidental serializer workaround.

### 3.2 Transport success was not tool success

A Gopher call could complete successfully at the transport layer while the inner native tool result reported an error. Treating those as the same success domain hid failures and made verification misleading.

The adapter now needs to preserve separate categories:

```text
transport failure
native tool failure
negative query result
policy rejection
verification failure
successful operation
```

This distinction matters even more once multiple agents and retries exist.

### 3.3 Callback JSON could be multiply encoded

Some Gopher callback payloads were direct JSON; others contained JSON strings containing another encoded payload. Simple tools such as tempo happened to work while richer inspection paths failed.

Normalization belongs at the transport boundary. Higher layers should receive one predictable representation and should not each invent their own decoding strategy.

### 3.4 Session context was not a reliable structured state API

`get_session_context` looked attractive because it promised one project snapshot. In practice it was large, escaped, variable in shape, and wasteful when the workflow only needed a small question such as “is slot 1 occupied?”

The better solution was not a more elaborate parser. It was a smaller query: probe the exact plugin slot through the same native resolver used for plugin inspection.

This became a general lesson:

> Prefer direct bounded observations over parsing broad descriptive context when the action requires a narrow safety fact.

### 3.5 Windows executable resolution is part of transport design

The first Codex launch on Windows failed with error 193 because a PATH lookup can resolve an npm-style `.cmd` shim rather than a native PE executable. Treating command resolution as platform-neutral was incorrect.

The transport now handles Windows command shims through `%COMSPEC% /D /C` while preserving stdio pipes and arguments.

### 3.6 Plugin state has more than one clock

The final parameter-calibration issue exposed another hidden runtime contract: normalized parameter readback could converge before the third-party plugin's human-readable value string updated.

A rapid normalized sweep therefore produced stale or plateaued display strings even though the underlying writes were valid.

The correct model became eventual consistency between two observations:

```text
normalized host value       plugin display value
        │                          │
        └── may settle earlier ────┘
```

Semantic calibration now waits for normalized convergence and stable display text before treating a sample as evidence.

## 4. Common mistake pattern: sending ordinary regressions to the live DAW gate

Several failures were not deep FL/agent issues at all:

- a feature was attached to the wrong `clack` crate;
- a refactor left a stale `tempo_read_only()` call;
- an `Arc` was moved into one closure and unavailable to another;
- `ToolError` did not implement `std::error::Error` where `?` expected it.

These regressions matter because every handoff to the Windows audio machine is expensive. The live FL machine should be the authority for DAW interoperability, not the first compiler for cross-crate refactors.

Future work should use a strict escalation ladder:

```text
format/static inspection
  → crate compile
  → focused unit tests
  → workspace compile/tests
  → non-DAW integration harness
  → Windows/FL live gate
```

When an environment prevents a local compile, changes should become smaller, assumptions should be stated explicitly, and the handoff should identify the exact first gate expected to fail.

## 5. Common mistake pattern: weak observability

Early workflow logs reduced failed dynamic tools to `success=false`. That discarded the information we most needed in an experimental bridge.

Once the inner dynamic-tool error text was printed, failures became directly actionable. A particularly useful example was:

```text
Could not resolve plugin target '1' (Slot: 1)
```

At first this appeared to be another failure. In the bounded processor workflow it is actually a useful negative observation: the requested plugin target does not resolve, which means the slot is empty.

The broader lesson is that observability should retain semantic class and payload without flooding the default log. The current compact model is appropriate:

```text
tool -> name arguments
tool <- name success duration
tool !! precise returned error, only on failure
```

Full protocol tracing can remain opt-in.

## 6. Common mistake pattern: blaming the model for the tools we gave it

One of the most important findings in the project came from a musically disappointing but mechanically successful run.

The agent inserted Pro-C 3 and moved Threshold only slightly. It explained that ratio, timing, and EQ mappings were not clearly available. At first this looked like excessive model conservatism.

The actual problem was our tool design.

`fl_find_plugin_parameters` treated a query such as:

```text
threshold ratio attack release knee mix output
```

as one literal substring. Naturally it found nothing. The model later found Threshold through a smaller search and rationally avoided controls it could not safely identify.

After changing parameter search to OR semantics and adding display-domain parameter writes, the model's effective reasoning space changed dramatically.

This is a product-level lesson:

> Tool ontology is part of model cognition.

Names, grouping, units, search semantics, schemas, defaults, scope, error messages, and available transitions all influence what the model can infer and how confidently it can act.

The `capture → analysis → agent → DAW` decomposition is valuable partly because each transition can now be designed as a semantic interface rather than a bag of raw implementation data.

## 7. Common mistake pattern: mixing measurement with mutation

The first display-domain calibrator temporarily swept many normalized values to infer how a plugin parameter mapped to dB, Hz, milliseconds, ratio, Q, or percent.

The concept was useful, but the implementation had two problems:

1. it sampled faster than plugin display state settled;
2. temporary probe writes appeared in the mutation journal.

The second problem is conceptually important. A journal is a record of durable project actions. Calibration is an observation procedure.

The corrected sequence is:

```text
read original
  → temporary unjournaled probe
  → wait for host convergence
  → wait for display stability
  → infer mapping
  → restore original
  → perform one durable verified write
  → journal that write
```

This distinction should survive beyond parameter calibration. Future spectrum probes, workspace inspections, hypothetical evaluations, audition passes, and dry-run actions should not become indistinguishable from user-visible mutations.

## 8. Common mistake pattern: unsafe degradation

When semantic display calibration initially failed, the agent briefly found a route back to arbitrary normalized parameter writes, including numerical parameter identifiers.

That is a bad fallback because an index is not a semantic contract. A giant third-party parameter manifest can contain meaningful controls, repeated bands, compatibility parameters, MIDI mappings, and sparse automation regions. “Write parameter 26” is not a trustworthy music-production instruction merely because the host accepts it.

The low-level normalized setter was therefore restricted to controls whose discrete/boolean meaning is already understood. Unknown continuous mappings now fail closed.

A useful principle is:

```text
loss of semantic certainty
        ↓
more observation or refusal
        ≠
less semantic tooling
```

The system should not become more dangerous as its confidence decreases.

## 9. Common mistake pattern: context abundance mistaken for context quality

The early FL agent path asked for a broad session dump. That produced a very large escaped payload containing much more project state than one processor task needed.

The current workflow instead exposes two compact forms of evidence:

- a reduced projection of the high-resolution audio analysis;
- direct target-track / slot observations relevant to the write scope.

The complete analysis artifact is still preserved for deterministic inspection. It simply is not copied wholesale into every agent turn.

This is the correct separation:

```text
complete machine evidence
        │
        ├── persisted / inspectable
        │
        └── task-specific semantic projection → model
```

More tokens are not automatically more grounding. Irrelevant detail can hide the causal measurements that should dominate reasoning.

## 10. What proved especially strong

### 10.1 Ghost Tap as a narrow sensor

The simplified plugin is one of the strongest components we now have because its responsibility is easy to state and test.

Ghost Tap should remain deliberately boring. It observes DAW audio and transport and exposes bounded captures to an external Ghost process. It should not grow into the application UI, an agent runtime, a child-plugin host, or a generic DAW extension point.

### 10.2 Deterministic Rust analysis

The analysis pipeline became useful before the agent integration was mature. The same capture produced coherent observations and proposals across model settings, which showed that the representation of audio evidence meaningfully constrains the model's reasoning.

That gives Ghost a strong research/product axis: experiment with evidence selection and semantic presentation while keeping the underlying measurement deterministic and inspectable.

### 10.3 Persistent Codex App Server

A persistent App Server process with multiple thread identities is a much better application primitive than spawning isolated agent calls.

The important properties now demonstrated or implemented include:

- persistent process lifetime;
- explicit thread lifetime;
- per-thread dynamic tool scopes;
- multiple loaded threads;
- request-ID routing;
- per-turn events;
- concurrent turns on different threads;
- fail-closed ambiguity handling.

The current workflow intentionally uses one processor thread, but it does so on the parallel-capable runtime rather than on a throwaway architecture.

### 10.4 Capability-scoped tools

The live FL catalog contains far more operations than one task should see. Giving the agent only a target track, bounded slot range, processor allow-list, parameter discovery, semantic writes, and safe context made the workflow both safer and cognitively cleaner.

The raw 48-tool catalog should remain an adapter capability surface, not the default agent interface.

### 10.5 Native readback verification

Verification is foundational.

There are three different claims:

```text
agent intended a change
adapter sent a change
DAW reports the resulting state
```

Only the third proves execution. The mutation journal should continue to record verified before/after state and should become more precise, not less, as the product grows.

### 10.6 Semantic display-domain control

The agent should reason in audio-engineering units. Ghost should own conversion between that semantic domain and plugin/host wire representation.

The successful direction is:

```text
agent: 350 Hz, -2.5 dB, Q 1.2, 20 ms, 3:1
                     ↓
              Ghost translation
                     ↓
          plugin normalized/wire value
                     ↓
               native readback
```

This keeps processor interoperability complexity out of the reasoning layer.

### 10.7 Serialized DAW mutation under potentially parallel cognition

The FL/Gopher adapter is single-flight. This is currently a strength, not a limitation to remove reflexively.

Multiple threads may eventually reason concurrently, but DAW mutations need ordering, resource ownership, and verification. A serialized native control link gives us a safe bottom-level invariant on which higher-level track/slot locks can later be built.

## 11. What the green workflow proves — and what it does not

The current milestone proves that Ghost can complete one meaningful live path:

```text
live FL signal
  → Ghost Tap capture
  → Rust analysis
  → compact evidence
  → Codex App Server thread
  → scoped semantic FL tools
  → real processor changes
  → native verification
```

It also proves that FL can remain the processor host and that Ghost can control third-party processors through the DAW without nesting them inside Ghost.

It does not yet prove:

- long-running application stability over hours/days;
- automatic binding of a Ghost Tap instance to the correct mixer resource without manual scope;
- general semantic profiles across many plugin vendors and versions;
- closed-loop judgement of before/after audio quality;
- multi-agent conflict coordination;
- robust persistence semantics for projects, threads, captures, profiles, and mutations;
- a user-facing Svelte/Tauri application;
- a general cross-DAW adapter abstraction.

Those are future product layers. They should not be backfilled into the current workspace as speculative abstractions before the vertical slice is isolated.

## 12. Development-process lessons

### Keep the real machine as the truth for proprietary integration

FL Studio and FabFilter behavior cannot be fully reproduced in a generic CI environment. Local Windows/DAW validation remains necessary.

The improvement is to reserve that gate for runtime facts that truly require the proprietary stack. Everything else should be eliminated earlier.

### Make integration findings executable

Every strange runtime fact that cost us a debugging cycle should become one of:

- a unit/regression test;
- a transport normalization rule;
- a typed error classification;
- a documented invariant;
- a small diagnostic probe.

A finding that exists only in a chat transcript will eventually be rediscovered.

### Prefer bounded product probes over broad debug APIs

Direct plugin-slot probing succeeded where broad session-context parsing was fragile. Similar future choices should prefer resource-scoped queries aligned with the intended action.

### Do not keep archaeology in the default retrieval path

Historical code and planning documents are not harmless. Future agents will search them, infer intent from them, and potentially rebuild concepts we have already abandoned.

Git history is the archive. HEAD should communicate current truth.

## 13. The product definition that emerged

The project began close to “AI mixing plugin” and then “agentic plugin host.” The demonstrated system is better described as:

> Ghost & Guild is an agentic layer for audio workspaces. It senses audio, derives deterministic evidence, runs persistent agent reasoning, and acts through the workspace's native control surface with scoped capabilities and verified execution.

The four semantic domains are:

```text
CAPTURE
  observed audio/workspace moment

ANALYSIS
  deterministic evidence derived from that observation

AGENT
  task reasoning over evidence, intent, state, and available capabilities

DAW
  authoritative workspace state and verified actions
```

The transitions matter just as much:

```text
capture → analysis
  converts signal into evidence

analysis → agent
  converts evidence into a task-specific reasoning representation

agent → DAW
  converts intent/reasoning into bounded, semantic, verifiable actions
```

This gives the model a much clearer world than “here is a giant project dump and a plugin parameter array.”

## 14. Closing assessment

The strongest outcome of this phase is not the amount of code written. It is that the responsibility boundary became simpler while the demonstrated capability became more powerful.

We can now remove substantial code and end up with a more complete product foundation.

That is a healthy signal.

The next action is therefore not another feature. It is to transform HEAD so that a new engineer or agent encounters the current vertical slice first, with historical host/UI/mix-plan machinery absent from the active workspace. `WORKSPACE_MIGRATION_PLAN.md` defines that reset.
