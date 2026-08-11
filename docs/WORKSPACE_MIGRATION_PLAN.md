# Ghost & Guild Workspace Cleanup and Migration Plan

Status: execution plan for the vertical-slice reset

Date: 2026-08-11

Starting baseline: `8fa7d5f0d17c7019767f5e7b4fa6a084c191cb70`

Companion: `TECHNICAL_RETROSPECTIVE.md`

## 1. Goal

Transform the repository from an accumulation of several explored Ghost architectures into a small workspace that expresses the currently proven product:

```text
capture → analysis → agent → DAW
```

The reset is intentionally aggressive. The current machine state is being backed up separately and Git history already preserves the implementation archaeology. We therefore do **not** need to keep abandoned systems in HEAD merely because they once required substantial work.

The success criterion is cognitive as much as technical:

> A new engineer or agent entering the repository should infer the current product from the default workspace without first learning the nested-host, egui, mock-mix, daemon, old plan-application, or generic `ghost-core` catch-all architectures.

The cleanup should remove obsolete concepts from active code search, Cargo dependency resolution, CI, examples, docs, and agent retrieval—not only mark them deprecated.

The end of this migration should leave a repository whose package boundaries themselves communicate the product:

```text
capture          analysis          agent                 DAW
   │                │                │                    │
ghost-tap ───► ghost-audio ───► ghost-context ───► ghost-fl-studio
                                  ghost-codex
                    \                |                   /
                     \────── ghost-application ─────────/
```

The diagram is semantic, not a promise of exact Cargo dependency direction. The later target-architecture document will formalize dependency rules after the reset is complete.

## 2. What this migration does and does not decide

This phase has two jobs:

1. remove historical responsibilities and code paths;
2. make one small structural split that is already justified by the proven vertical slice: retire `ghost-core` into `ghost-audio` and `ghost-tap`.

The split is deliberately **not the first cleanup step**. We first subtract historical code while `ghost-core` still exists, re-establish the green slice, and only then move the surviving code across the now-visible boundary. This prevents us from carefully relocating abstractions that should simply be deleted.

This migration does **not** yet define:

- the final cross-DAW trait/interface;
- the final database schema;
- the final Svelte/Tauri application structure;
- plugin-profile persistence;
- closed-loop before/after evaluation;
- multi-agent coordination policy;
- Convex integration;
- the final form of every application-layer port or trait.

Those belong to the target architecture and next-phase documents written from the cleaned workspace.

## 3. Preserve the proven baseline before destructive cleanup

Before deleting code from the canonical line:

1. preserve the successful head (`8fa7d5f...`) with a milestone tag or equivalent immutable Git reference;
2. keep the existing PR history (#1–#16) as the narrative/implementation archive;
3. record the final successful live FL semantic-control run in PR #16 or milestone notes;
4. perform destructive cleanup on the dedicated reset branch;
5. do not create an `archive/legacy-*` source tree in HEAD unless a file is genuinely required by current tests.

Git is the archive. A legacy source directory would continue to pollute code search and future-agent retrieval.

The current `phase/vertical-slice-reset` branch is the planning/execution line for this transformation.

## 4. Two checkpoints: cleanup state and final migration state

A major sequencing rule is that the repository passes through a temporary cleanup checkpoint before the final package split.

### 4.1 Intermediate cleanup checkpoint

After historical removal but before the `ghost-core` split, the workspace may temporarily look like:

```text
crates/
  ghost-core/            # temporary survivor: trimmed to live audio/tap primitives only
  ghost-context/
  ghost-codex/
  ghost-fl-studio/
  ghost-application/
  ghost-clap-plugin/     # temporary name; already minimal Ghost Tap behavior

apps/
  ghost-fl-agent-smoke/  # temporary home of the proven workflow until promotion
```

This state is useful only as a regression checkpoint. It is **not** the end state of the migration.

Gate: the reduced historical-free workspace builds/tests, and preferably the known-good live workflow is re-run before structural relocation begins.

### 4.2 Final migration target

The migration is complete only when the repository is approximately:

```text
Cargo.toml
Cargo.lock
README.md

crates/
  ghost-audio/           # audio representation, I/O, deterministic analysis
  ghost-tap/             # realtime sensing, Tap protocol, minimal CLAP plugin
  ghost-context/         # task-specific context/evidence compilation
  ghost-codex/           # Codex App Server runtime, tools, thread dispatcher
  ghost-fl-studio/       # FL native adapter + scoped semantic tool surface
  ghost-application/     # product use cases / orchestration boundary

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

There should be **no `ghost-core` package in the final workspace**. Its retirement is part of the migration, not deferred future architecture work.

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
- Rust audio analysis
- Codex App Server
- FL Studio native/Gopher adapter
```

It should link only to documents and commands that exist after the reset.

### 5.3 Retire the existing `docs/ARCHITECTURE.md`

Disposition: **delete during cleanup; replace in the next documentation phase**.

The current file is an architecture decision for the revisioned nested-CLAP host. It describes `ProjectDocument`, egui sessions, `NativeClapAudio`, child GUI ownership, detached Windows shells, graph revisions, child parameter patch transactions, and other abandoned product responsibilities.

Do not edit this document into the new architecture line by line. Delete it and write the target architecture fresh after cleanup.

The technical retrospective is the historical bridge.

### 5.4 Delete checked-in generated `artifacts/`

Disposition: **delete**.

Current generated analysis, mock-evaluation, plots, first-agent-response output, and sandbox-validation artifacts belong to historical validation workflows.

Runtime artifacts should live in user-local storage, test temp directories, or deliberately curated fixtures. Generated product output should not make HEAD look like the old experiment pipeline is current.

### 5.5 Delete obsolete top-level reports/checksums/visualizers

Disposition: **delete anything not referenced by the surviving build or current docs**.

This includes `SHA256SUMS.txt`, old reports, old visualizers, obsolete schemas, and one-off experiment files unless a current build/test explicitly consumes them.

## 6. CI cleanup

### 6.1 Delete host-era workflows

Delete:

- `host-hardening-validation.yml`;
- `windows-child-integration.yml`.

They target `ghost-host`, `ghost-ui`, child integration, and historical fix branches.

### 6.2 Rewrite main CI after workspace reduction

Initial CI should be small and truthful:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Add platform matrices only where surviving crates require them.

Live FL Studio/FabFilter interoperability remains a local/manual integration gate. CI should validate everything it can without pretending to validate proprietary DAW behavior.

## 7. Config and script cleanup

### 7.1 Delete/reset `config/default.toml`

Disposition: **delete first; reintroduce configuration from actual application needs later**.

The current config declares mock agent backends, structured MixPlan output, nested-host roles, plugin discovery, and host parameter smoothing. Migrating those keys would preserve abandoned architecture through configuration vocabulary.

### 7.2 Delete historical pipeline scripts

Delete scripts whose primary purpose was the old mock/reference/artifact pipeline, including where no surviving test needs them:

- `build_examples.py`;
- `generate_fixtures.py` if its consumers disappear;
- `mock_evaluate.py`;
- `reference_analysis.py`;
- `run_sandbox_validation.sh`;
- `send_agentd_request.py`;
- `validate_artifacts.py`;
- old generic `package.sh` / `package_clap.py` if superseded.

### 7.3 Keep and rename the proven Tap packager

```text
scripts/package_clap.ps1
    ↓
scripts/package_ghost_tap.ps1
```

Remove nested-host packaging branches/comments from it.

## 8. Fixture cleanup

Remove multi-megabyte fixture binaries from the default repository unless a surviving deterministic test genuinely requires them.

Preferred rule:

```text
unit/integration test needs audio
  → generate a tiny deterministic fixture in test code/build step

research/evaluation needs representative audio
  → external/local fixture set, not normal source checkout
```

Preserve focused analysis tests and capture trigger/pre-roll tests without carrying the historical evaluation corpus.

## 9. Crate disposition

### 9.1 `ghost-core`: transitional only, then retire

Disposition: **trim in place during cleanup, then split and delete the package before migration exit**.

The current crate mixes several semantic domains:

```text
analysis
raw audio/I/O
realtime capture
transport sensing
Tap protocol
generic processor abstractions
generic task abstractions
old daemon protocol
validation/user-intent leftovers
```

The first cleanup pass should make the live/dead boundary obvious without moving code yet.

Delete or audit out first:

- `AtomicGraphControl` and graph-specific state;
- `processor.rs` generic hosted-processor/parameter structures if no surviving path needs them;
- `task.rs` / `TaskPlan` / `TaskOperation` / `ExpectedOutcome`;
- old request/response `protocol.rs` used only by deleted daemon/CLI paths;
- old plan/task validation;
- user-intent/model types whose only consumers are the removed MixPlan pipeline.

Keep temporarily:

- raw audio buffer/I/O used by current analysis and Tap artifact handling;
- `analysis/` and its validated feature extraction;
- realtime capture state/buffer and trigger/pre-roll logic;
- transport/audio configuration publication needed by Ghost Tap;
- Tap discovery/control/artifact protocol.

After this reduced state is green, perform the structural split described below. Do not keep `ghost-core` as a generic domain bucket after the migration.

### 9.2 New `ghost-audio`

Disposition: **create from the surviving audio/analysis half of `ghost-core` after cleanup**.

Responsibility:

> Represent audio and deterministically derive evidence from it.

Expected contents:

```text
ghost-audio/
  audio/
    buffer / decode / wav I/O
  analysis/
    levels
    spectrum
    dynamics
    transients
    stereo
    ...
  analysis models/configuration
```

Keep this crate free of:

- Codex/App Server concepts;
- FL/Gopher concepts;
- plugin hosting;
- application workflow sequencing;
- generic DAW actions.

Ghost Tap may depend on small audio primitives/WAV helpers from `ghost-audio`; that does not make analysis responsible for capture orchestration.

Do not split this immediately into `ghost-analysis`, `ghost-audio-io`, etc. The current codebase does not justify that package granularity.

### 9.3 `ghost-clap-plugin` + Tap half of `ghost-core` → `ghost-tap`

Disposition: **merge/rename into one product-accurate sensing crate after the trim checkpoint**.

The current CLAP crate is already behaviorally close to the target: one stereo input/output, transparent passthrough, transport publication, bounded capture, and a non-realtime worker.

The final `ghost-tap` crate should own the complete capture-side responsibility:

```text
ghost-tap/
  realtime capture buffer/state
  transport sensing/publication
  Tap status/command/artifact protocol
  live Tap discovery/control helpers
  minimal CLAP plugin implementation
```

Actions:

- move the surviving Tap/capture/transport code out of `ghost-core`;
- rename `ghost-clap-plugin` package/directory to `ghost-tap`;
- keep plugin identity `ai.konko.ghost-tap`;
- preserve realtime callback constraints;
- preserve the non-realtime filesystem worker/control plane;
- remove comments/build rules/dependencies implying nested child hosting;
- keep the crate independent of Codex and FL control adapters.

Do not add the Ghost product UI to this crate. Ghost Tap is sensing infrastructure loaded into the DAW.

### 9.4 `ghost-context`

Disposition: **keep with minimal cleanup**.

Keep the compiled context/message/output structures and reusable context composition used by App Server turns. Remove recipes/components only when unreferenced after the old mix pipeline disappears.

Do not add FL-specific or plugin-host-specific concepts here during cleanup.

This crate is likely to become the semantic transition from deterministic evidence to agent-visible context; the target architecture document will formalize that role.

### 9.5 `ghost-codex`

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
- protocol/routing/ambiguity tests.

Delete or migrate out:

- `MockMixingAgent`;
- old `MixingAgent` trait;
- old one-agent wrapper if no current path requires it;
- `PromptBundle`/`MixPlan` coupling;
- `ghost-mix` dependency;
- tests that exist only for old structured mock mixing.

The result should be a domain-neutral Codex App Server runtime.

### 9.6 `ghost-fl-studio`

Disposition: **keep and consolidate**.

Preserve the hard-won runtime knowledge:

- Gopher target discovery/transport;
- live capability catalog;
- live-schema argument ordering;
- recursive/nested JSON normalization;
- inner native-tool error detection;
- typed FL operations;
- mutation journal/readback;
- scoped processor tools;
- direct effect-slot probing;
- semantic parameter discovery;
- display-domain tuning/calibration.

Current cleanup priority is deduplication. `codex_tools.rs` and `processor_tools.rs` contain evolution layers from the session-context approach to direct slot probing.

Actions:

1. establish one product-facing registration path;
2. move resilient direct-slot behavior into it;
3. delete superseded session-context safety logic;
4. retain raw `get_session_context` only as diagnostic/read capability if useful;
5. keep one semantic parameter implementation;
6. keep raw native tools internal while exposing scoped semantic tools to agents.

Do **not** extract a theoretical universal `DawAdapter` during this migration. First make the FL implementation coherent; generalize later from demonstrated workflows.

### 9.7 `ghost-application`

Disposition: **keep and rewrite as the use-case/orchestration boundary**.

The current crate contains a good architectural idea but old generic ports (`RenderPort`, repository abstractions, standalone agent execution) from the previous application design.

Its reason to exist after the reset is:

> This is where Ghost turns capabilities into product operations.

In practical terms, it should become the home of the semantic sequencing around:

```text
CaptureArtifact
    ↓
AnalysisArtifact / evidence
    ↓
Agent context / turn
    ↓
verified workspace outcome
```

During migration:

- remove ports with no current consumer;
- retain/rehome deterministic analysis-use-case helpers only if they simplify current call sites;
- move compact evidence projection and other workflow semantics out of the executable when this can be done mechanically;
- define product-facing request/result types only when they are supported by the working slice;
- keep the binary thin where possible;
- do not invent a large “clean architecture” framework;
- do not force a final cross-DAW `WorkspacePort` abstraction yet merely to satisfy layering aesthetics.

The migration should make `ghost-application` read as **verbs/use cases**, while `ghost-audio`, `ghost-tap`, `ghost-codex`, and `ghost-fl-studio` provide capabilities.

### 9.8 `ghost-mix`

Disposition: **delete from HEAD**.

It encodes the old plan-first model: `MixPlan`, typed EQ/compressor plan operations, conversion to `TaskPlan`, plugin capability summaries, structured prompt bundles, and precompiled host plan validation.

The live workflow now operates through scoped semantic DAW tools and independently verified native mutations. If typed proposals/evaluations return later, design them from this real workflow rather than preserving this historical schema.

### 9.9 `ghost-host`

Disposition: **delete from HEAD**.

It owns abandoned responsibilities: child CLAP discovery, hosted chains, native child instances, GUI hosting, graph topology, parameter queues/smoothing, child state, and Windows window integration.

Git/PR history is the archive.

### 9.10 `ghost-ui`

Disposition: **delete from HEAD**.

The egui UI is coupled to the nested-host/MixPlan architecture. Future Ghost product UI will be designed separately as the external application; Ghost Tap remains UI-free.

### 9.11 `ghost-fakes`

Disposition: **delete**.

It primarily fakes nested child/plugin hosting. Future fakes should be narrow and attached to current seams: scripted App Server transport, scripted FL transport, capture fixtures, analysis fixtures, and mutation/readback fixtures.

### 9.12 `ghost-db`

Disposition: **remove from active workspace and delete current migrations for this reset**.

The existing schema encodes old plugin-hosting and MixPlan concepts. There is no production migration obligation yet, so schema compatibility would preserve abandoned architecture.

Reintroduce SQLite from the cleaned domain model later. Likely persistence candidates include captures, analysis results, App Server thread associations, DAW resource bindings, verified mutations, semantic plugin profiles, and evaluation/user feedback.

## 10. Application/tool disposition

### 10.1 Promote the real workflow to `apps/ghost-workflow`

The actual product prototype currently lives as `src/bin/ghost-fl-workflow.rs` under `apps/ghost-fl-agent-smoke`.

Promote it to:

```text
apps/ghost-workflow/
```

Current responsibilities already express the reference slice:

1. connect FL adapter;
2. discover Ghost Tap;
3. request capture;
4. start/stop FL transport;
5. analyze captured audio;
6. build compact agent evidence;
7. register scoped semantic DAW tools;
8. start one thread on `CodexParallelRuntime`;
9. run the task;
10. require verified mutation;
11. report resulting mutations.

Extract reusable orchestration toward `ghost-application` while keeping behavior unchanged. Do not redesign the workflow during the move.

### 10.2 Tempo/App Server smoke

Disposition: **move to diagnostic/integration tooling or delete once equivalent coverage exists**.

It was crucial for proving dynamic tools and persistent threads; it is not the product app.

### 10.3 `ghost-fl-native-bridge` → `tools/fl-gopher-probe`

Keep it as a compatibility/diagnostic probe. Prefer consuming `ghost-fl-studio` rather than maintaining duplicate transport/catalog logic, but do not create a large rewrite merely for the move.

### 10.4 Delete `ghost-agentd`

It is built around the old TCP JSONL protocol, old agent/mix pipeline, old DB, and old host responsibilities. Do not modernize it during cleanup.

### 10.5 Delete `ghost-cli`

It mixes useful analysis commands with obsolete host discovery, child smokes, mock demos, old DB/schema operations, and daemon health. Rebuild tiny current-purpose diagnostics later where needed.

### 10.6 Delete `ghost-lab`

It depends on the old egui UI. Any future analysis visualizer should be built against the cleaned audio boundary.

## 11. Cargo/dependency cleanup

After deleting old crates/apps, rewrite root workspace membership so the intended survivor set is explicit.

Expected removals include most/all of:

- `clack-host`;
- egui/eframe/egui-baseview;
- UI-specific Windows APIs;
- dependencies used only by old host/DB/mock pipeline;
- Python artifact-generation assumptions.

Expected survivors include:

- plugin-side CLAP crates needed by `ghost-tap`;
- DSP/audio-analysis dependencies used by `ghost-audio`;
- serde/schema utilities actually used by surviving models;
- Codex App Server transport/runtime dependencies;
- FL/Gopher transport dependencies.

Do not prune dependencies by guess. After workspace/code pruning, let compiler/reference search reveal the actual survivors.

## 12. Branch cleanup

After the reset is safely established:

- delete merged historical `fix/*` remote branches;
- delete merged experimental bridge branches;
- land/close the stacked #15/#16 line coherently before making the reset canonical;
- keep only active product/research branches with a clear purpose.

Do not delete remote branches before the milestone reference and reset branch are safe.

## 13. Migration execution sequence

Execute in dependency order so compiler errors reveal real coupling rather than expected breakage.

### Phase A — freeze and document

- preserve proven head with immutable milestone reference;
- add technical retrospective and migration plan;
- record successful semantic-control run;
- no behavior changes.

Gate: baseline agreed.

### Phase B — remove obvious historical surface

- delete `agent-ops/`;
- delete generated artifacts;
- delete obsolete host/child workflows;
- delete stale config/historical validation scripts;
- remove stale docs/visualizers/reports;
- simplify fixtures.

Gate: repository top level visibly reflects the current phase.

### Phase C — remove legacy applications and crates

Remove from workspace and then delete:

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

Delete old migrations and dead supporting files.

Gate: surviving dependency graph no longer reaches nested-host or old MixPlan code.

### Phase D — trim survivors in place

At this stage **do not split `ghost-core` yet**.

- trim `ghost-core` to only live audio/analysis/Tap primitives;
- remove old mixing/mock API from `ghost-codex`;
- consolidate FL processor tool registration;
- remove stale ports from `ghost-application`;
- keep `ghost-clap-plugin` behavior unchanged while historical dependencies disappear.

Static gate:

```text
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Phase E — checkpoint the proven slice before structural relocation

Promote/move only enough tooling to run the current known-good flow, then run:

```text
Ghost Tap → capture → Rust analysis → Codex thread → scoped FL mutation → native verification
```

This checkpoint separates cleanup regressions from later crate-move regressions.

If the live gate is inconvenient at this exact point, at minimum preserve a compile/test checkpoint and do not proceed through multiple structural phases without one known-good state.

### Phase F — retire `ghost-core`

With historical code gone and the remaining boundary visible:

1. create `ghost-audio` from raw audio/I/O + analysis code;
2. move realtime capture/transport/Tap protocol code into the minimal CLAP crate;
3. rename that crate/package to `ghost-tap`;
4. update imports/dependencies/tests mechanically;
5. delete `ghost-core` from workspace and HEAD;
6. verify no generic “core” dependency remains.

Gate:

```text
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Do not opportunistically rewrite DSP algorithms or Tap behavior during this phase. This is a semantic package split, not a new feature pass.

### Phase G — promote the vertical slice and application boundary

- create/finalize `apps/ghost-workflow`;
- make `ghost-application` own the reusable use-case semantics we can extract without changing behavior;
- move raw bridge to `tools/fl-gopher-probe`;
- retain tempo/App Server smoke only if unique integration coverage remains;
- rename packaging script to Ghost Tap terminology.

Gate: product flow compiles using only the final package names.

### Phase H — rewrite repository entry points

- replace README;
- rewrite main CI for reduced workspace;
- remove stale links;
- ensure `cargo metadata` exposes only current crates/tools;
- ensure active code search for `ghost-host`, `MixPlan`, old egui architecture, `MockMixingAgent`, and `ghost-core` returns no product source.

Gate: fresh clone communicates the present product directly.

### Phase I — final live regression gate

On Windows/FL:

1. package/install `ghost-tap`;
2. launch/connect FL Gopher path;
3. run the known-good capture→analysis→agent→DAW workflow;
4. confirm semantic parameter discovery;
5. confirm display-domain writes;
6. confirm native verification/mutation journal;
7. confirm no regression in capture or App Server thread execution.

Only after this gate should the reset become the new canonical baseline.

### Phase J — write the next documents

With the cleaned/split workspace in front of us, create separately:

- target architecture;
- invariants / lessons for future agents;
- next-phase roadmap.

These documents should describe the code that survived the reset.

## 14. Survivor-specific trim checklist

### Transitional `ghost-core`

Before splitting, remove dead references to:

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

Keep a type only if the current vertical slice or a focused deterministic test requires it.

Final condition: **the package itself is deleted after its live code is moved to `ghost-audio` and `ghost-tap`.**

### `ghost-audio`

After extraction verify it contains only audio representation/I/O, deterministic analysis, and analysis models/configuration.

Search for and reject accidental dependencies/concepts involving:

```text
Codex
Gopher
FL Studio
child plugin hosting
MixPlan
TaskPlan
DAW mutation
```

### `ghost-tap`

Verify no source/dependency includes product responsibilities such as:

```text
child plugin hosting
GUI/editor
agent runtime
FL/Gopher control
network/database
processor parameter graph
```

Preserve realtime safety and keep filesystem work on the non-realtime worker.

### `ghost-codex`

Remove:

```text
MixingAgent
MockMixingAgent
MixPlan
PromptBundle
old one-agent wrappers superseded by persistent runtime
```

Keep it domain-neutral around App Server threads, turns, events, and dynamic tools.

### `ghost-fl-studio`

Deduplicate:

```text
scoped track context
fl_add_effect registration
session-context slot occupancy
parameter display tuning
native response normalization
```

There should be one authoritative product behavior per operation.

### `ghost-application`

Remove unused historical ports. Prefer explicit use-case/request/result vocabulary over generic framework abstractions.

A useful mental rule:

```text
ghost-audio / ghost-tap / ghost-codex / ghost-fl-studio
    = capabilities and domain mechanics

ghost-application
    = verbs and product use cases
```

Do not force the final cross-DAW interface during this migration.

## 15. Runtime knowledge that must survive cleanup

Aggressive cleanup must preserve the experimentally discovered contracts embedded in current code/tests:

- live-schema argument ordering for Gopher;
- recursive JSON callback normalization;
- inner native-tool error detection;
- secret-safe Gopher target logging;
- Windows Codex `.cmd`/shim spawning;
- App Server thread/dynamic-tool handling;
- parallel dispatcher ambiguity safeguards;
- FL adapter single-flight serialization;
- direct effect-slot probes;
- semantic parameter OR search;
- filtering irrelevant MIDI-CC parameter noise;
- display-domain parsing/calibration;
- normalized convergence + display settle polling;
- unjournaled temporary probes;
- durable native readback verification;
- restrictions on arbitrary continuous normalized writes;
- Ghost Tap realtime capture/trigger/pre-roll behavior;
- compact analysis evidence projection.

The reset removes obsolete **responsibilities**, not hard-won runtime contracts.

## 16. Verification philosophy

Three levels of truth remain explicit.

### Static/build truth

Rust/Cargo proves ownership, type, feature, and dependency coherence.

### Deterministic integration truth

Scripted transports and small fixtures prove App Server routing, Gopher serialization rules, capture state machines, analysis behavior, tool policy, and journal semantics.

### Proprietary runtime truth

Only the real Windows + FL Studio + third-party plugin stack proves final integration behavior.

A migration phase should not be considered complete merely because it compiles, but live FL should also not be used to discover ordinary Rust compile regressions.

## 17. Exit criteria

The workspace reset/migration is complete when all of the following are true:

- the default Cargo workspace contains only components used by or immediately supporting the vertical slice;
- `ghost-host`, old egui UI, MixPlan pipeline, fake child host, old daemon, and old validation CLI are absent from HEAD;
- **`ghost-core` is absent from HEAD and workspace membership**;
- `ghost-audio` owns the surviving raw-audio/I/O + deterministic analysis responsibility;
- `ghost-tap` owns realtime sensing, Tap protocol/control, and the minimal CLAP plugin;
- the old `ghost-clap-plugin` package name is gone;
- the actual workflow is a first-class app rather than a smoke-test sub-binary;
- `ghost-application` contains current use-case semantics rather than old render/repository framework vocabulary;
- `ghost-codex` no longer depends on the old mix domain;
- `ghost-fl-studio` has one authoritative processor tool path;
- old generated artifacts and stale planning docs are gone;
- CI validates the reduced workspace rather than historical subsystems;
- README describes `capture → analysis → agent → DAW` and nothing contradictory;
- a fresh clone builds/tests cleanly;
- the known-good live FL workflow remains green using the **final package layout**.

At that point the repository itself becomes a trustworthy input to the target-architecture pass and to future agents.

That is the purpose of this migration.