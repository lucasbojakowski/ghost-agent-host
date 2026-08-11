# Ghost & Guild Workspace Cleanup and Migration Plan

Status: execution plan for the vertical-slice reset

Date: 2026-08-10

Starting baseline: `8fa7d5f0d17c7019767f5e7b4fa6a084c191cb70`

Companion: `TECHNICAL_RETROSPECTIVE.md`

## 1. Goal

Transform the repository from an accumulation of several explored Ghost architectures into a small workspace that expresses the currently proven product:

```text
capture → analysis → agent → DAW
```

The reset is intentionally aggressive. The current machine state is being backed up separately and Git history already preserves the implementation archaeology. We therefore do **not** need to keep abandoned systems in HEAD merely because they once required substantial work.

The success criterion is cognitive as much as technical:

> A new engineer or agent entering the repository should infer the current product from the default workspace without first learning the nested-host, egui, mock-mix, daemon, or old plan-application architectures.

The cleanup should remove obsolete concepts from active code search, Cargo dependency resolution, CI, examples, docs, and agent retrieval—not only mark them deprecated.

## 2. Non-goals for this migration

This reset should not simultaneously design every next product layer.

In particular, this migration does **not** yet define:

- the final target architecture document;
- the permanent cross-DAW abstraction;
- the final database schema;
- the final Svelte/Tauri application structure;
- plugin-profile persistence;
- closed-loop before/after evaluation;
- multi-agent coordination policy;
- Convex integration.

Those should be designed from the cleaned workspace after the current vertical slice is isolated.

The migration is therefore a subtraction and consolidation phase, not another speculative expansion.

## 3. Preserve the proven baseline before destructive cleanup

Before deleting code from the canonical line:

1. preserve the current successful head (`8fa7d5f...`) with a milestone tag or equivalent immutable Git reference;
2. keep the existing PR history (#1–#16) as the narrative/implementation archive;
3. record the final successful live FL semantic-control run in PR #16 or the milestone notes;
4. perform destructive cleanup on a dedicated reset branch;
5. do not create an `archive/legacy-*` source tree in HEAD unless a file is genuinely needed by current tests.

Git is the archive. A legacy directory would continue to pollute code search and future-agent retrieval.

The current `phase/vertical-slice-reset` branch is the planning/execution line for this transformation.

## 4. Desired repository signal after the reset

Before the later architecture pass, the repository should be approximately this small:

```text
Cargo.toml
Cargo.lock
README.md

crates/
  ghost-core/            # proven audio/capture/analysis primitives, trimmed
  ghost-context/         # task-specific context/evidence compilation
  ghost-codex/           # Codex App Server runtime, tools, thread dispatcher
  ghost-fl-studio/       # FL native adapter + scoped semantic tool surface
  ghost-application/     # small orchestration/use-case boundary, rewritten
  ghost-tap/             # renamed minimal DAW-loaded capture plugin

apps/
  ghost-workflow/        # first-class capture→analysis→agent→DAW executable

tools/
  fl-gopher-probe/       # compatibility/diagnostic tool, not product runtime

docs/
  TECHNICAL_RETROSPECTIVE.md
  WORKSPACE_MIGRATION_PLAN.md
  # follow-up phase adds target architecture, invariants, roadmap

scripts/
  package_ghost_tap.ps1  # only if still needed

tests/ or crate-local tests
  # small deterministic fixtures only
```

This is a migration target, not yet the final architecture. Names may change in the follow-up architecture pass, but every retained component must already serve the proven vertical slice.

## 5. Top-level cleanup

### 5.1 Delete `agent-ops/`

Disposition: **delete from HEAD**.

The directory contains planning, memory, journal, report, progress, configuration, and many numbered tasks for prior architectural phases. It includes assumptions about egui, editor providers, daemon migration, nested host responsibilities, older application ports, and other concepts that are no longer authoritative.

Keeping it is especially harmful for agentic development because retrieval treats prose as intent. A future agent can easily give an obsolete design document more weight than recently evolved code.

Historical value is already preserved in Git.

### 5.2 Replace the stale README

Disposition: **rewrite during cleanup**.

The current README still calls the project “Ghost Agent Host,” describes a vendor-neutral CLAP child graph, and points at old validation flows.

The reset README should be deliberately short. Until the target architecture document exists, it should say only what is already proven:

```text
Ghost & Guild

capture → analysis → agent → DAW

Current reference environment:
- Ghost Tap for audio capture
- Rust analysis
- Codex App Server
- FL Studio native/Gopher adapter
```

It should link only to documents and commands that exist after the reset.

### 5.3 Retire the existing `docs/ARCHITECTURE.md`

Disposition: **delete during cleanup; replace in the next documentation phase**.

The current file is an architecture decision for the revisioned nested-CLAP host. It describes `ProjectDocument`, egui sessions, `NativeClapAudio`, child GUI ownership, detached Windows shells, graph revisions, child parameter patch transactions, and other abandoned product responsibilities.

Do not edit this document into the new architecture line by line. That risks carrying old vocabulary forward. Delete it, then write the target architecture fresh after cleanup.

The technical retrospective is the historical bridge.

### 5.4 Delete checked-in generated `artifacts/`

Disposition: **delete**.

Current generated analysis, mock-evaluation, plots, first-agent-response output, and sandbox-validation artifacts belong to historical validation workflows.

Runtime artifacts should live in user-local storage, a test temp directory, or a deliberately curated fixture location. Generated product output should not make HEAD look like the old experiment pipeline is still current.

### 5.5 Delete `SHA256SUMS.txt` unless a current release process regenerates it

Disposition: **delete now**.

It belongs to the earlier packaging/archive workflow and has no current product role.

### 5.6 Audit/remove root `ISSUES.md`, old reports, visualizers, schemas, and one-off experiment files

Disposition: **delete anything not referenced by the surviving build or current docs**.

Do not preserve files because they are informative in isolation. Preserve only files that are part of the current build, current deterministic tests, or current product documentation.

## 6. CI cleanup

### 6.1 Delete `host-hardening-validation.yml`

Disposition: **delete**.

It targets the historical `fix/clap-host-hardening` branch and explicitly tests `ghost-host`.

### 6.2 Delete `windows-child-integration.yml`

Disposition: **delete**.

It watches `ghost-host`, `ghost-ui`, child integration, and an old fix branch. All are outside the new product boundary.

### 6.3 Rewrite the main CI after workspace reduction

Disposition: **retain only after simplifying it**.

The current CI validates the entire old workspace and runs a Python artifact-generation/mock-evaluation pipeline. After the reset, CI should initially be much smaller:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Add platform matrices only where surviving crates truly require them.

Live FL Studio/FabFilter interoperability remains a local/manual integration gate. CI should validate everything it can without pretending to validate proprietary DAW behavior.

## 7. Config and script cleanup

### 7.1 Delete/reset `config/default.toml`

Disposition: **delete first; reintroduce configuration later from actual application needs**.

The current config still declares mock agent backends, structured MixPlan output, nested-host roles, plugin discovery, and host parameter smoothing. Migrating those keys would preserve abandoned architecture through configuration vocabulary.

A new config should be designed only after the clean runtime boundary exists.

### 7.2 Scripts to delete

Delete historical pipeline scripts whose primary purpose was the old mock/reference/artifact workflow:

- `build_examples.py`
- `generate_fixtures.py` if its only consumers are removed artifact tests;
- `mock_evaluate.py`
- `reference_analysis.py`
- `run_sandbox_validation.sh`
- `send_agentd_request.py`
- `validate_artifacts.py`
- old generic `package.sh` / `package_clap.py` if superseded by the Windows Tap packager.

### 7.3 Packaging script to keep and rename

Keep the proven Windows Ghost Tap packaging/install flow, but rename it so its responsibility is obvious:

```text
scripts/package_clap.ps1
    ↓
scripts/package_ghost_tap.ps1
```

Remove any remaining nested-host packaging branches or comments from that script.

## 8. Fixture cleanup

The current repository contains several multi-megabyte synthetic WAV files used to develop/evaluate the analyzer and earlier mock pipeline.

Disposition: **remove large fixture binaries from the default repository unless a surviving deterministic test genuinely requires them**.

Preferred rule:

```text
unit/integration test needs audio
  → generate a tiny deterministic fixture in test code or build step

research/evaluation needs representative audio
  → external/local fixture set, not normal source checkout
```

A small `silence_then_signal` style fixture may remain conceptually useful for validating capture trigger/pre-roll behavior, but it should be tiny or generated programmatically rather than a multi-megabyte checked-in WAV.

The analysis engine should retain focused deterministic tests for its features without carrying an entire evaluation corpus in HEAD.

## 9. Crate disposition

### 9.1 `ghost-clap-plugin` → rename to `ghost-tap`

Disposition: **keep and rename**.

This crate has already been reduced to the correct product responsibility: stereo passthrough, transport publication, bounded capture, and a non-realtime capture worker.

Actions:

- rename directory/package from `ghost-clap-plugin` to `ghost-tap` (or `ghost-tap-clap` if Cargo naming conflicts make that clearer);
- keep plugin identity `ai.konko.ghost-tap`;
- preserve the realtime callback constraints;
- remove any remaining comments, test names, build rules, or dependency aliases implying nested child hosting;
- keep the crate independent of the agent and DAW-control adapters.

Do not add a product GUI to this crate later. That decision belongs in the future invariants document, but the cleanup should already make the tap's narrow responsibility obvious.

### 9.2 `ghost-core`

Disposition: **keep, then aggressively trim**.

The crate currently mixes proven audio/capture primitives with generic abstractions from older architectures.

Keep:

- audio decoding/encoding primitives needed by current analysis/capture;
- `analysis/` and validated feature extraction;
- realtime capture state/buffer and trigger/pre-roll logic;
- transport/audio configuration state needed by Ghost Tap;
- Tap protocol/discovery/capture artifact primitives.

Audit and likely remove:

- `AtomicGraphControl` and any graph-specific control state left from the old nested processor graph;
- `processor.rs` generic processor/parameter structures if no surviving crate needs them after `ghost-mix`/`ghost-host` removal;
- `task.rs` generic `TaskPlan` / `TaskOperation` if the current App Server tool workflow does not use them;
- old request/response `protocol.rs` if it exists only for deleted `ghost-agentd`/CLI transport;
- `validation.rs` if it validates old TaskPlan/mix-plan structures;
- `model.rs` intent types if they are only consumed by the removed mix pipeline.

Refactor `daw.rs` after deleting consumers. It currently contains both useful Tap-facing transport/capture primitives and remnants of the old processor graph. Prefer a small set of modules whose names describe what remains (`transport`, `capture`, `tap`) rather than preserving `daw.rs` as an unrelated collection.

Do not rewrite the proven DSP algorithms during this cleanup unless dead dependencies require a mechanical change.

### 9.3 `ghost-context`

Disposition: **keep with minimal cleanup**.

This crate is already small and independent of the historical host. It provides compiled context/message/output structures and reusable context composition.

Actions:

- remove recipe/components only if unreferenced after the old mix pipeline disappears;
- keep the core context model/compiler used by Codex App Server turns;
- avoid adding audio/FL-specific concepts here during cleanup.

This is likely to become an important transition boundary between deterministic evidence and agent reasoning, but the target architecture document should make that final decision.

### 9.4 `ghost-codex`

Disposition: **keep the App Server runtime; delete the old mixing-agent layer**.

Keep:

- stdio transport and Windows shim handling;
- App Server initialization/protocol helpers;
- `ToolRegistry` and dynamic tool definitions;
- persistent thread support;
- `CodexParallelRuntime` dispatcher;
- request-ID routing;
- per-thread tool registries;
- `AgentEvent`, `AgentOutput`, `TurnOptions`;
- tests for protocol routing and ambiguity failure.

Delete or migrate out:

- `MockMixingAgent`;
- the old `MixingAgent` trait;
- the old `CodexAppServerAgent` one-agent wrapper if no current test/product path still requires it;
- `PromptBundle`/`MixPlan` coupling;
- `ghost-mix` dependency;
- tests whose only purpose is the old mock structured mixing output.

The result should be a domain-neutral Codex App Server adapter/runtime. Audio meaning belongs in evidence/context and DAW tool layers, not in the App Server transport crate.

### 9.5 `ghost-fl-studio`

Disposition: **keep and consolidate**.

This is the first real DAW adapter and currently owns valuable runtime knowledge:

- Gopher target discovery/transport;
- live capability catalog;
- live-schema argument ordering;
- nested JSON normalization;
- inner-tool error detection;
- typed FL operations;
- mutation journal/readback;
- scoped processor tool registration;
- semantic parameter discovery and display-domain tuning;
- direct effect-slot probing.

The primary cleanup issue is duplication created during rapid iteration.

`codex_tools.rs` still contains earlier processor context/insertion logic that depended on session-context parsing, while `processor_tools.rs` wraps/replaces parts of that registration with the direct-slot-probe implementation.

Actions:

1. choose one product-facing registration path;
2. move the resilient direct-slot behavior into that path directly;
3. delete superseded session-context-dependent processor helpers;
4. keep raw `get_session_context` support only as an explicit diagnostic/read capability if it remains useful—not as a safety dependency;
5. ensure one semantic parameter implementation exists rather than an old path plus replacement path;
6. keep raw native tools internal to the adapter; expose scoped tools to agents.

Do not extract a theoretical universal `DawAdapter` during this cleanup. First make the FL implementation internally coherent. Generalize later from demonstrated workflows.

### 9.6 `ghost-application`

Disposition: **keep, but rewrite around the real vertical slice**.

This crate is small enough that it is not historical baggage by volume, but its abstractions are still partly from the previous application-port design (`RenderPort`, generic repository port, standalone `execute_context`).

Use it as the first orchestration/use-case boundary for the product rather than deleting it.

During cleanup:

- retain deterministic analysis use-case code if useful;
- replace `RenderPort` terminology with DAW/workspace actuation terminology only when the real call path is extracted from `ghost-fl-workflow`;
- remove ports with no current consumer;
- do not invent generic repository/database abstractions yet;
- make the surviving orchestration read naturally as capture → analysis → agent → DAW.

The target architecture pass will decide whether these four domains become modules, crates, traits, or use-case types. The cleanup only needs to stop this crate from advertising old abstractions.

### 9.7 `ghost-mix`

Disposition: **delete from active workspace and HEAD**.

It encodes the old plan-first domain:

- `MixPlan`;
- semantic EQ/compressor operation structs;
- conversion to generic `TaskPlan`;
- plugin capability summaries;
- prompt bundle construction around structured MixPlan output;
- validation of plans before compilation into a hosted processor graph.

The current live workflow does not use this execution model. The agent operates through scoped semantic DAW tools and receives text output while actions are verified independently.

Do not salvage the crate merely because some type names sound useful. If future product work needs typed proposals/evaluations, design them from the actual capture→analysis→agent→DAW workflow and its execution ledger.

### 9.8 `ghost-host`

Disposition: **delete from HEAD**.

This is the largest repository source of obsolete product semantics: child CLAP discovery, hosted chains, native child instances, plugin GUI hosting, graph topology, parameter event queues, smoothing, child state, and Windows window integration.

None of these responsibilities belong in the current vertical slice.

Do not move it to `archive/`. The PR/Git history is sufficient.

### 9.9 `ghost-ui`

Disposition: **delete from HEAD**.

The egui UI is tightly coupled to `ghost-host`, `ghost-mix`, patch preview/application, session state, and the nested-host product architecture.

Future product UI will be designed separately around the external Ghost application; Ghost Tap remains UI-free. Retaining this crate would bias future work toward obsolete concepts and dependencies (`egui`, `eframe`, baseview/Win32 ownership).

### 9.10 `ghost-fakes`

Disposition: **delete**.

The crate is primarily a fake CLAP child/plugin test environment coupled to `ghost-host`.

If the new vertical slice needs test doubles, create narrow fakes at the relevant current boundary:

- capture fixture;
- analysis fixture;
- App Server scripted transport;
- FL native adapter scripted transport;
- mutation/readback fixture.

Do not carry a generic fake nested host into the new phase.

### 9.11 `ghost-db`

Disposition: **remove from the active workspace for this reset; redesign persistence later**.

The current schema and API encode the previous product model:

- plugin binaries/manifests for Ghost-owned hosting;
- `mix_requests`;
- `MixPlan` storage;
- plan applications;
- Pro-Q/Pro-C-specific state snapshots;
- old ghost instance concepts;
- old prompt bundles.

There is no production migration obligation at this stage. Preserving schema compatibility would make abandoned concepts permanent.

Delete the current migrations and crate from the active workspace. Reintroduce SQLite once the clean domain model is established. Likely future persistence candidates include captures, analysis results, App Server thread associations, DAW workspace/resource bindings, verified mutations, semantic plugin profiles, and evaluation/user feedback—but those belong to the later target/next-phase design.

## 10. Application disposition

### 10.1 Extract the real `ghost-fl-workflow`

Disposition: **promote to first-class app**.

The actual product prototype currently lives awkwardly as `src/bin/ghost-fl-workflow.rs` inside `apps/ghost-fl-agent-smoke`.

Create:

```text
apps/ghost-workflow/
```

and move the live orchestration there.

Its current responsibilities are already the reference slice:

1. connect FL adapter;
2. discover Ghost Tap;
3. request capture;
4. start/stop FL transport;
5. analyze captured WAV;
6. build compact agent evidence;
7. register scoped semantic DAW tools;
8. start one thread on `CodexParallelRuntime`;
9. run the task;
10. require at least one verified mutation;
11. print/record resulting mutations.

During migration, prefer extracting reusable orchestration into `ghost-application` and leaving the binary thin. Do not redesign the workflow at the same time as moving it.

### 10.2 Remove the old tempo smoke binary from product apps

Disposition: **move to integration tooling or delete after equivalent tests exist**.

The tempo test was essential for proving App Server dynamic tools and multiple loaded threads. It is not the product application.

If retained, place it under a clearly diagnostic/integration location such as:

```text
tools/fl-app-server-smoke/
```

Do not keep “smoke” as the home of the actual product workflow.

### 10.3 `ghost-fl-native-bridge`

Disposition: **move to `tools/fl-gopher-probe`**.

The raw bridge remains valuable for compatibility diagnostics, catalog inspection, and isolating adapter failures. It should not appear beside product applications now that `ghost-fl-studio` owns the reusable implementation.

Where possible, make the probe consume `ghost-fl-studio` rather than maintaining a second copy of transport/catalog logic. If that would require a large rewrite during cleanup, keep it temporarily as a diagnostic tool and schedule deduplication before the reset branch lands.

### 10.4 `ghost-agentd`

Disposition: **delete**.

The daemon is built around the old TCP JSONL protocol, old `MixingAgent`, old `MixPlan` pipeline, old DB, and `ghost-host`.

Do not modernize it as part of cleanup. The future long-lived application backend should be designed around the new vertical slice and eventual Tauri runtime requirements rather than inheriting this daemon's protocol.

### 10.5 `ghost-cli`

Disposition: **delete**.

The CLI mixes useful analysis commands with obsolete host plugin discovery, native child smoke tests, mock demos, generated MixPlan schema, DB stats, and daemon health.

Rebuilding one small analysis/debug command later is cheaper and clearer than preserving the old CLI dependency graph.

### 10.6 `ghost-lab`

Disposition: **delete**.

It depends on the old egui `ghost-ui` and therefore belongs to the historical product architecture.

If an analysis visualizer becomes useful later, build it against the cleaned analysis boundary rather than preserving the old application shell.

## 11. Cargo dependency cleanup

After deleting old crates/apps, rewrite root workspace membership first, then remove unused workspace dependencies.

Expected removals include most or all of:

- `clack-host`;
- egui/eframe/egui-baseview;
- UI-specific Windows APIs;
- `crossbeam-queue` if no surviving runtime uses it;
- dependencies used only by old host/DB/mock pipeline;
- Python-generated artifact workflow assumptions.

Expected survivors include the CLAP plugin-side crates required by Ghost Tap, the DSP/analysis stack, serde/schema utilities, and dependencies used by Codex/FL transports.

Do not optimize the dependency list by guess. After workspace pruning, let compiler/reference search identify actual survivors.

## 12. Branch cleanup

The repository currently carries many merged `fix/*` branches plus experimental feature branches.

After the reset is safely established:

- delete merged historical `fix/*` remote branches;
- delete the merged `feat/fl-native-bridge` branch;
- land/close the stacked #15/#16 line coherently before making the new reset branch canonical;
- keep only active product/research branches with a clear owner and purpose.

Do not delete remote branches before the milestone reference and reset branch are safely available.

The objective is for branch discovery to communicate present work rather than the debugging history of the nested-host phase.

## 13. Migration execution sequence

Execute cleanup in dependency order so compiler errors reveal real remaining coupling rather than thousands of expected breakages.

### Phase A — freeze and document

- preserve the proven head with an immutable milestone reference;
- add `TECHNICAL_RETROSPECTIVE.md` and this migration plan;
- record the successful semantic-control run;
- no code behavior changes.

Gate: documents reviewed and reset baseline agreed.

### Phase B — remove obvious historical surface

- delete `agent-ops/`;
- delete old generated `artifacts/`;
- delete obsolete host/child GitHub workflows;
- delete stale config and historical validation scripts;
- remove stale docs/visualizers/reports not used by current product;
- simplify fixtures.

Gate: repository top level visibly reflects the current phase.

### Phase C — remove legacy applications and crates from workspace membership

First edit root `Cargo.toml` so the intended survivor set is explicit.

Remove from workspace:

```text
ghost-host
ghost-ui
ghost-fakes
ghost-mix
ghost-db
ghost-agentd
ghost-cli
ghost-lab
```

Then delete their directories and old migration files.

Gate: surviving workspace dependency graph no longer reaches nested-host or old MixPlan code.

### Phase D — trim survivor crates

- trim `ghost-core` to audio/capture/analysis/Tap primitives actually used;
- remove old mixing/mock API from `ghost-codex` and its `ghost-mix` dependency;
- consolidate duplicate FL processor tool registration;
- rewrite `ghost-application` toward the real four-stage use case;
- rename `ghost-clap-plugin` to `ghost-tap`.

Gate:

```text
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Phase E — promote the vertical slice

- create `apps/ghost-workflow`;
- move/extract current live workflow into it;
- move raw bridge to `tools/fl-gopher-probe`;
- retain tempo/App Server smoke only if it still provides unique integration coverage;
- rename packaging script around Ghost Tap.

Gate: product flow still compiles without historical crates.

### Phase F — rewrite repo entry points

- replace README;
- rewrite main CI for the reduced workspace;
- remove stale links;
- ensure `cargo metadata` shows only current crates/tools;
- ensure code search for `ghost-host`, `MixPlan`, old egui architecture, and `MockMixingAgent` returns no active product source.

Gate: a fresh clone exposes only current concepts.

### Phase G — live regression gate

On Windows/FL:

1. package/install Ghost Tap;
2. launch/connect FL Gopher path;
3. run the same known-good capture→analysis→agent→DAW workflow;
4. confirm semantic parameter discovery;
5. confirm display-domain writes;
6. confirm native verification/mutation journal;
7. confirm no regression in Tap capture or App Server thread execution.

Only after this gate should the reset become the new canonical baseline.

### Phase H — write the next documents

With the cleaned workspace in front of us, create separately:

- target architecture;
- invariants / lessons for future agents;
- next-phase roadmap.

Those documents should describe the code that survived the reset, not predict what an unclean historical workspace might become.

## 14. Survivor-specific trim checklist

This checklist is intentionally concrete so the cleanup does not stop after deleting top-level crates.

### `ghost-core`

Search for and remove dead references to:

```text
TaskPlan
TaskOperation
ExpectedOutcome
ProcessorDescriptor
ParameterDescriptor
ParameterChange
AtomicGraphControl
RequestEnvelope / ResponseEnvelope
old MixPlan validation support
```

Keep a type only if a surviving current crate uses it for the vertical slice or a focused deterministic test.

### `ghost-codex`

Search for and remove:

```text
MixingAgent
MockMixingAgent
MixPlan
PromptBundle
old one-agent wrappers superseded by persistent App Server runtime
```

Verify the crate can depend on `ghost-context` without depending on audio/mix domain types unless a concrete runtime requirement remains.

### `ghost-fl-studio`

Search for duplicate/superseded implementations of:

```text
scoped_track_context
fl_add_effect registration
session-context slot occupancy
parameter display tuning
native response normalization
```

There should be one authoritative product behavior for each operation.

### `ghost-application`

Remove unused port abstractions and make call sites reflect current use cases. Avoid creating a large “clean architecture” framework just because the crate is named application.

### `ghost-tap`

Check that no source/dependency includes:

```text
child plugin
nested host
GUI/editor
agent runtime
network/database
plugin parameter graph
```

The non-realtime filesystem control plane is intentional; filesystem work must remain outside the audio callback.

## 15. What should not be accidentally deleted

Aggressive cleanup should preserve the hard-won compatibility knowledge embedded in current code/tests:

- live-schema argument ordering for Gopher;
- recursive JSON callback normalization;
- inner native tool error detection;
- secret-safe Gopher target logging;
- Windows Codex `.cmd`/shim spawning;
- App Server thread/dynamic-tool handling;
- parallel dispatcher ambiguity safeguards;
- FL adapter single-flight serialization;
- direct effect-slot probes;
- semantic parameter OR search;
- filtering of irrelevant MIDI-CC parameter noise;
- display-domain parsing/calibration;
- normalized convergence + display settle polling;
- unjournaled temporary probes;
- durable native readback verification;
- restrictions on arbitrary continuous normalized writes;
- Ghost Tap realtime capture/trigger/pre-roll semantics;
- compact analysis evidence projection.

The point of the reset is to remove obsolete **responsibilities**, not to erase experimentally discovered runtime contracts.

## 16. Verification philosophy for the reset

Three levels of truth should remain explicit.

### Static/build truth

Rust/Cargo can prove ownership, type, feature, and dependency coherence.

### Deterministic integration truth

Scripted transports and small fixtures can prove App Server routing, Gopher serialization rules, capture state machines, analysis outputs, tool policy, and journal semantics.

### Proprietary runtime truth

Only the actual Windows + FL Studio + third-party plugin stack can prove the final integration behavior.

A cleanup PR should not be considered complete merely because it compiles, but live FL should also not be used to discover ordinary Rust compile regressions.

## 17. Exit criteria

The workspace reset is complete when all of the following are true:

- the default Cargo workspace contains only components used by or immediately supporting the vertical slice;
- `ghost-host`, old egui UI, MixPlan pipeline, fake child host, old daemon, and old validation CLI are absent from HEAD;
- Ghost Tap has a product-accurate crate/package name;
- the actual workflow is a first-class app rather than a smoke-test sub-binary;
- `ghost-codex` no longer depends on the old mix domain;
- `ghost-fl-studio` has one authoritative processor tool path;
- old generated artifacts and stale planning docs are gone;
- CI validates the reduced workspace rather than historical subsystems;
- the README describes `capture → analysis → agent → DAW` and nothing contradictory;
- a fresh clone builds/tests cleanly;
- the known-good live FL workflow remains green after cleanup.

At that point the repository itself becomes a trustworthy input to the next agent.

That is the real purpose of this migration.
