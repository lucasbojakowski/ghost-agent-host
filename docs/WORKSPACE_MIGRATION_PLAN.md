# Ghost & Guild Workspace Cleanup and Migration Plan

Status: execution plan for the vertical-slice reset

Date: 2026-08-11

Starting baseline: `8fa7d5f0d17c7019767f5e7b4fa6a084c191cb70`

Companions:

- `TECHNICAL_RETROSPECTIVE.md`
- `decisions/001-transparent-fl-studio-adapter.md`

## 1. Goal

Transform the repository from an accumulation of several explored Ghost architectures into a small workspace that expresses the currently proven product:

```text
capture → analysis → agent → DAW
```

The reset is intentionally aggressive. Git history and the user's local backup preserve the implementation archaeology, so abandoned systems do not need to remain in HEAD.

The success criterion is cognitive as much as technical:

> A new engineer or agent entering the repository should infer the current product and current boundaries directly from the default workspace.

The cleanup must remove obsolete concepts from active code search, Cargo dependency resolution, CI, examples, docs, and agent retrieval—not merely mark them deprecated.

The final package layout should make the product legible:

```text
capture          analysis          agent                 DAW
   │                │                │                    │
ghost-tap ───► ghost-audio ───► ghost-context       ghost-fl-studio
                                  ghost-codex
                    \                |                   /
                     \────── ghost-application ─────────/
                                  ▲
                                  │
                         apps/ghost-workflow
```

This diagram is semantic. The later target-architecture document will formalize exact dependency direction.

## 2. Decisions already made for this migration

This reset is not only deletion. Two structural decisions are already sufficiently supported by the working vertical slice and should be completed before migration exit.

### 2.1 Retire `ghost-core`

`ghost-core` is a transitional catch-all. After historical code is removed and the surviving boundary is visible, it will be split into:

```text
ghost-audio
  raw audio representation / I/O
  deterministic analysis

ghost-tap
  realtime sensing
  transport publication
  Tap protocol/control
  minimal CLAP plugin
```

There should be no `ghost-core` package in the final workspace.

### 2.2 Make `ghost-fl-studio` a transparent FL/Gopher adapter

ADR 001 is accepted and governs this migration.

`ghost-fl-studio` will answer:

> What does FL Studio expose, and how do we invoke it reliably?

It will **not** answer:

> What should this Ghost app allow an agent to do?

The adapter keeps real integration invariants and raw/typed mirrors of FL operations. Policy, context selection, capability filtering, slot/plugin scope, verification strategy, semantic calibration, and agent tool composition move upward.

A useful rule for every FL-related line of code is:

```text
Exists because FL/Gopher behaves this way?
    → ghost-fl-studio

Exists because Ghost wants to behave this way?
    → app / application / context
```

## 3. What this migration does not decide

This migration does **not** define:

- the final cross-DAW trait/interface;
- the final database schema;
- the final Svelte/Tauri application structure;
- plugin-profile persistence;
- closed-loop before/after evaluation;
- multi-agent coordination policy;
- Convex integration;
- the final shared application policy model;
- a general middleware/callback framework for workspace tools.

Those should emerge from the cleaned workspace and later real applications.

## 4. Preserve the proven baseline before destructive cleanup

Before destructive changes become canonical:

1. preserve the successful `8fa7d5f...` state with an immutable Git reference;
2. keep PR history (#1–#16) as the implementation narrative/archive;
3. preserve the final successful live FL semantic-control run in milestone notes or PR history;
4. perform the reset on `phase/vertical-slice-reset`;
5. do not create a legacy source tree in HEAD.

Git is the archive. Legacy source in HEAD would continue to pollute search and future-agent retrieval.

## 5. Final repository target

The migration is complete only when the repository is approximately:

```text
Cargo.toml
Cargo.lock
README.md

crates/
  ghost-audio/           # audio representation, I/O, deterministic analysis
  ghost-tap/             # realtime sensing + Tap protocol + minimal CLAP plugin
  ghost-context/         # reusable evidence/context representation transforms
  ghost-codex/           # domain-neutral Codex App Server runtime
  ghost-fl-studio/       # transparent FL Studio/Gopher API adapter
  ghost-application/     # reusable Ghost use-case orchestration

apps/
  ghost-workflow/        # current concrete product composition and policy

tools/
  fl-gopher-probe/       # raw integration/compatibility diagnostics

docs/
  TECHNICAL_RETROSPECTIVE.md
  WORKSPACE_MIGRATION_PLAN.md
  decisions/
    001-transparent-fl-studio-adapter.md

scripts/
  package_ghost_tap.ps1  # only if still needed

tests/ or crate-local tests
  # focused deterministic fixtures only
```

The app is intentionally the highest-policy layer. Shared behavior is promoted downward only after repeated real use demonstrates that it is genuinely reusable.

## 6. Top-level cleanup

### 6.1 Delete `agent-ops/`

Disposition: **delete from HEAD**.

It contains planning/memory/task material for prior architectures and is particularly dangerous in an agentic repository because retrieval can mistake historical prose for current intent.

### 6.2 Rewrite README

The reset README should be short and describe only the proven current system:

```text
Ghost & Guild

capture → analysis → agent → DAW

Current reference environment:
- Ghost Tap
- Rust audio analysis
- Codex App Server
- FL Studio/Gopher adapter
```

### 6.3 Delete the historical architecture document

Delete the current host-era `docs/ARCHITECTURE.md` rather than editing it into the new system.

The technical retrospective is the historical bridge. A fresh target architecture will be written after the reset.

### 6.4 Delete stale generated/historical files

Remove generated `artifacts/`, obsolete checksums, old reports, old visualizers, obsolete schemas, one-off experiment outputs, and files not used by the surviving build/tests/current docs.

## 7. CI, config, scripts, fixtures

### CI

Delete host-era workflows such as:

- `host-hardening-validation.yml`;
- `windows-child-integration.yml`.

Initial reduced CI should be truthful and small:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Live FL/FabFilter validation remains a local proprietary-runtime gate.

### Config

Delete/reset the old `config/default.toml` rather than migrating mock-agent, MixPlan, host-role, plugin-hosting, or smoothing vocabulary into the new phase.

### Scripts

Delete historical mock/reference/artifact-pipeline scripts when no current test consumes them.

Keep the proven Tap packager and rename it:

```text
scripts/package_clap.ps1
    ↓
scripts/package_ghost_tap.ps1
```

### Fixtures

Remove large checked-in WAV corpora unless a focused deterministic test truly needs them. Prefer tiny generated fixtures for unit/integration tests and keep larger research/evaluation audio outside a normal source checkout.

## 8. Crate disposition

### 8.1 `ghost-core`: transitional only

Disposition: **trim in place, prove the reduced slice, then retire the package**.

Delete/audit out first:

- graph/host-specific state such as `AtomicGraphControl`;
- generic hosted-processor/parameter abstractions;
- `TaskPlan` / `TaskOperation` / `ExpectedOutcome`;
- old daemon request/response protocol;
- old plan/task validation;
- old MixPlan-oriented user-intent/model leftovers.

Keep temporarily:

- audio buffer/I/O;
- deterministic analysis;
- realtime capture state/buffer;
- transport/audio publication required by Tap;
- Tap discovery/control/artifact protocol.

Do not relocate dead abstractions merely to preserve them.

### 8.2 `ghost-audio`

Create from the surviving audio/analysis half of `ghost-core`.

Responsibility:

> Represent audio and deterministically derive evidence from it.

Keep free of:

- Codex/App Server concepts;
- FL/Gopher concepts;
- application policy;
- plugin hosting;
- DAW mutation semantics.

Do not over-split into many micro-crates yet.

### 8.3 `ghost-tap`

Merge/rename the current minimal CLAP plugin plus the Tap/capture/transport half of `ghost-core`.

Own:

```text
realtime capture buffer/state
transport sensing/publication
Tap status/command/artifact protocol
Tap discovery/control helpers
minimal CLAP plugin
```

Preserve:

- plugin identity `ai.konko.ghost-tap`;
- transparent passthrough;
- realtime safety;
- non-realtime filesystem worker.

Keep independent of Codex and FL control.

### 8.4 `ghost-context`

Keep as reusable representation/transformation for evidence supplied to agents.

Important ownership rule after ADR 001:

- the crate may provide reusable context building blocks;
- the **app chooses which observations/evidence are selected**;
- FL-specific “compact target-track context” is not an FL adapter primitive.

Do not turn `ghost-context` into another policy bucket during cleanup.

### 8.5 `ghost-codex`

Keep the App Server runtime and remove audio/mixing-domain coupling.

Keep:

- stdio transport + Windows shim handling;
- initialization/protocol helpers;
- `ToolRegistry` / tool definitions;
- persistent threads;
- `CodexParallelRuntime`;
- request/turn routing;
- per-thread tool registries;
- event/output/turn options;
- routing/ambiguity tests.

Remove:

- `MixingAgent`;
- `MockMixingAgent`;
- `MixPlan` / `PromptBundle` coupling;
- `ghost-mix` dependency;
- obsolete one-agent wrapper if unused.

Target direction: a domain-neutral Rust Codex App Server runtime that other workspaces could consume.

### 8.6 `ghost-fl-studio`: transparent adapter

Disposition: **keep, but simplify more aggressively than the earlier plan**.

Preserve only the FL/Gopher-facing mechanics and stable raw interface.

#### Keep in the adapter

- Gopher target discovery/connection;
- live tool catalog/schema retrieval;
- schema-order argument canonicalization;
- CDP/native transport framing;
- recursive JSON normalization;
- native result preservation;
- transport error vs inner FL tool error distinction;
- secret-safe target logging;
- native single-flight serialization;
- raw `call(tool, args)` / catalog access;
- thin typed wrappers that map one-to-one onto actual FL operations.

Examples of acceptable typed wrappers:

```text
get_tempo
set_tempo
play / stop
add_effect
remove_effect
get_plugin_parameter_list
get_plugin_parameter_value
set_plugin_parameter_value
routing/channel operations that directly mirror real tools
```

#### Move out of the adapter

Remove/move upward:

- `FlAgentToolPolicy`;
- `FlPluginWriteScope`;
- Codex `ToolRegistry` registration from the FL crate;
- slot/plugin allowlists;
- fixed target-track policy;
- `fl_get_target_track_context` as a Ghost-composed agent projection;
- “slot must be empty” / “never replace” policy;
- requirement to make at least one mutation;
- semantic parameter relevance filtering for one workflow;
- MIDI-CC filtering policy;
- boolean-only normalized-write restrictions;
- numeric-identifier policy;
- display-domain calibration as a mandatory FL mutation abstraction;
- automatic restore/journal behavior tied to calibration;
- workflow mutation journals;
- application-specific readback/verification policy.

The current `codex_tools.rs` / `processor_tools.rs` evolution should not be consolidated into one bigger FL-owned policy layer. Instead, separate raw adapter mechanics from Ghost workflow composition and delete the adapter-side agent-policy surface.

#### Raw access requirement

The crate must keep a raw escape hatch based on the live Gopher catalog so callers are not blocked by the typed wrapper subset.

An app may choose to expose none, some, or nearly all suitable raw FL tools to an agent. That decision is outside the adapter.

#### No premature middleware

Do not add callback/middleware hooks inside `ghost-fl-studio` during this reset. The app can wrap raw calls while constructing its own tools. Introduce reusable middleware only after multiple apps demonstrate the same requirement.

#### Dependency direction

Target: `ghost-fl-studio` should not depend on `ghost-codex` simply to create agent tools.

```text
ghost-fl-studio  = knows FL

ghost-codex      = knows Codex App Server

apps/*            = decides how/why to combine them
```

### 8.7 `ghost-application`

Keep and rewrite as reusable use-case/orchestration semantics.

Its reason to exist:

> Turn capabilities into reusable Ghost operations.

It may own sequencing such as:

```text
capture artifact
    ↓
analysis/evidence
    ↓
agent turn
    ↓
workspace outcome
```

But policy should initially remain higher when it is specific to one app.

During migration:

- remove unused historical ports;
- keep explicit request/result/use-case vocabulary;
- move logic here only when it is already reusable across the working product flow;
- do not automatically absorb slot/plugin/context/capability policy from `ghost-workflow`;
- do not invent a universal DAW abstraction yet.

Promotion rule:

```text
apps/*
  ↓ repeated real requirement

ghost-application / ghost-context
  ↓ only if truly infrastructure-level

lower infrastructure
```

### 8.8 Delete historical crates

Delete from HEAD/workspace:

```text
ghost-mix
ghost-host
ghost-ui
ghost-fakes
ghost-db
```

Rationale remains unchanged: each encodes responsibilities from the abandoned nested-host/MixPlan architecture or persistence model.

Reintroduce persistence later from the cleaned domain model rather than migrating the old schema.

## 9. Application and tool disposition

### 9.1 `apps/ghost-workflow`

Promote the real workflow out of the smoke-test package and make it the concrete reference application.

After ADR 001, this app initially owns the policy that was previously embedded in the FL crate.

Expected responsibilities include:

```text
choose target mixer/resource
choose capture behavior
choose analysis profile
choose context projection
choose which FL capabilities become Codex tools
choose slot/plugin constraints if desired
choose raw vs semantic parameter tools
choose mutation preconditions
choose verification/journaling behavior
run one Codex thread
report outcome
```

The current safe vertical-slice behavior should be preserved by **moving** these choices upward before deleting them from `ghost-fl-studio`.

This app is also the experimental surface for comparing:

- raw FL tools vs composed semantic tools;
- broad vs compact context;
- permissive vs constrained capability sets;
- direct normalized access vs semantic calibration;
- different verification strategies.

Do not treat one successful experiment as infrastructure law.

### 9.2 `tools/fl-gopher-probe`

Move/rename the raw bridge into a diagnostic tool.

It should be the easiest place to inspect:

- live catalog;
- live schemas;
- raw calls/results;
- transport behavior;
- adapter compatibility.

Prefer consuming `ghost-fl-studio` so raw diagnostics exercise the same adapter used by the app.

### 9.3 Other historical apps

Delete:

```text
ghost-agentd
ghost-cli
ghost-lab
```

Move/delete tempo/App Server smokes after equivalent focused integration coverage exists.

## 10. Live workspace semantics

The migration should encode one important assumption in app/use-case design, not in FL adapter state:

> The user may change the DAW independently of Ghost at any time.

Therefore:

```text
observation = snapshot
DAW/native response = truth
model context != authoritative workspace state
```

If an app needs optimistic concurrency or safety, it may compose:

```text
observe current state
    ↓
check app precondition
    ↓
invoke raw FL mutation
    ↓
optional readback verification
    ↓
re-observe/reason if precondition failed
```

Examples of app-owned preconditions:

```text
SlotMustBeEmpty
SlotMustContain("Pro-Q 4")
NeverReplace
AskBeforeReplace
```

These must not become hidden semantics of the raw FL operation.

## 11. Verification, calibration, and journaling migration

The successful slice relied on strong verification and display-domain calibration. We should preserve the capability without confusing it with the FL API itself.

During migration:

1. keep raw parameter read/write primitives in `ghost-fl-studio`;
2. move display-value parsing/probing/settle/calibration into the reference app initially;
3. keep temporary calibration probes out of durable app mutation history;
4. move readback verification and mutation journaling upward;
5. preserve tests for the observed plugin/display behavior;
6. only promote a shared verifier/calibrator into `ghost-application` or another reusable crate after repeated real use demonstrates the abstraction.

The adapter may expose all native fields necessary for these higher-level behaviors, including normalized values and available display strings.

## 12. Cargo/dependency cleanup

After deleting old crates/apps, rewrite root workspace membership and prune dependencies from actual compiler/reference evidence.

Expected removals include most/all host/UI/database/mock-only dependencies.

Expected survivors include:

- plugin-side CLAP crates needed by `ghost-tap`;
- audio/DSP dependencies needed by `ghost-audio`;
- serde/schema utilities;
- Codex App Server runtime dependencies;
- FL/Gopher transport dependencies.

Important target dependency removal:

```text
ghost-fl-studio -X-> ghost-codex
```

Agent tool composition belongs in the app.

## 13. Branch cleanup

After the reset is safely established:

- delete merged historical `fix/*` branches;
- delete merged bridge/experiment branches;
- land/close the stacked #15/#16 line coherently;
- keep only active product/research branches with clear purpose.

Do not remove references before the milestone/reset line is safe.

## 14. Migration execution sequence

### Phase A — freeze and document

- preserve proven milestone;
- add technical retrospective;
- add this migration plan;
- add ADR 001;
- preserve final successful semantic-control evidence.

Gate: baseline and decisions agreed.

### Phase B — remove obvious historical surface

- delete `agent-ops/`;
- delete generated artifacts;
- delete host-era CI/config/scripts/docs;
- simplify fixtures.

Gate: repository top level reflects the current phase.

### Phase C — remove historical apps/crates

Remove from workspace and delete:

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

Gate: surviving graph no longer reaches nested-host/MixPlan architecture.

### Phase D — trim survivors in place

Do not split `ghost-core` yet.

- trim `ghost-core` to live audio/analysis/Tap code;
- remove mixing/mock coupling from `ghost-codex`;
- remove stale application ports;
- identify raw FL adapter mechanics vs Ghost-specific policy without changing reference behavior yet.

Static gate:

```text
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Phase E — move FL policy upward while preserving behavior

This is a dedicated migration phase because the previous vertical slice mixed adapter correctness and app policy.

1. establish raw catalog/call + one-to-one typed FL operations as the `ghost-fl-studio` public surface;
2. move Codex tool construction out of `ghost-fl-studio`;
3. move target track, slots, plugin allowlist, context projection, parameter safety policy, semantic calibration, verification, and journaling into `apps/ghost-workflow` initially;
4. remove `FlAgentToolPolicy` / `FlPluginWriteScope` from the FL adapter;
5. remove `ghost-codex` dependency from `ghost-fl-studio` where practical;
6. preserve native integration invariants and their tests;
7. preserve a raw catalog/call escape hatch.

Gate: the reference app behaves the same even though the FL crate is now policy-free.

### Phase F — checkpoint the proven live slice

Run the known-good workflow after policy relocation:

```text
Ghost Tap → capture → analysis → Codex → app-composed FL calls → native outcome
```

Confirm that moving policy upward did not regress:

- capture;
- App Server turns/tools;
- effect insertion;
- parameter reads/writes;
- semantic behavior currently required by the app;
- optional readback/journaling behavior.

This checkpoint separates architectural relocation regressions from the later `ghost-core` split.

### Phase G — retire `ghost-core`

1. create `ghost-audio` from audio/I/O + analysis;
2. move capture/transport/Tap protocol into the minimal CLAP package;
3. rename package/directory to `ghost-tap`;
4. update imports/tests mechanically;
5. delete `ghost-core`;
6. verify no catch-all dependency remains.

Gate:

```text
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Do not rewrite DSP/Tap behavior during this package split.

### Phase H — promote the vertical slice cleanly

- finalize `apps/ghost-workflow`;
- extract only demonstrably reusable sequencing into `ghost-application`;
- keep app-specific policy in the app;
- move raw bridge to `tools/fl-gopher-probe`;
- rename Tap packaging script.

Gate: final package names compile and the app owns its explicit capability/context policy.

### Phase I — rewrite repository entry points

- rewrite README;
- rewrite CI;
- clean `cargo metadata`;
- remove stale references;
- ensure active source search no longer finds obsolete host/MixPlan/core/FL-agent-policy architecture.

Gate: fresh clone communicates the current system directly.

### Phase J — final live regression

On Windows/FL:

1. package/install `ghost-tap`;
2. connect through the transparent FL adapter;
3. run `capture → analysis → agent → DAW`;
4. confirm raw FL calls remain stable;
5. confirm the app's current semantic/policy layer still produces the expected processing flow;
6. confirm user/manual DAW changes are handled as live state, not stale adapter assumptions.

Only after this gate should the reset become canonical.

### Phase K — next documents

From the cleaned workspace write:

- target architecture;
- invariants / lessons;
- next-phase roadmap.

## 15. Survivor-specific trim checklist

### Transitional `ghost-core`

Remove dead task/processor/protocol/graph abstractions before splitting. Final condition: package deleted.

### `ghost-audio`

Reject dependencies/concepts involving Codex, Gopher, FL, host graphs, MixPlan, or DAW mutation.

### `ghost-tap`

Reject child hosting, GUI/editor, agent runtime, FL control, DB/network, and processor-graph responsibilities. Preserve realtime safety.

### `ghost-codex`

Remove mixing-domain types and keep App Server runtime/tool/thread/event semantics general.

### `ghost-fl-studio`

Final checklist:

```text
[ ] raw live catalog available
[ ] raw call path available
[ ] typed wrappers mirror real FL operations one-to-one
[ ] live-schema argument ordering preserved
[ ] recursive response normalization preserved
[ ] inner native errors preserved/distinguished
[ ] native single-flight preserved
[ ] no FlAgentToolPolicy
[ ] no FlPluginWriteScope
[ ] no Codex ToolRegistry construction
[ ] no fixed track/slot/plugin policy
[ ] no compact agent context projection
[ ] no workflow mutation requirement
[ ] no mandatory semantic calibration/normalized-write policy
[ ] no app mutation journal ownership
```

### `ghost-application`

Keep reusable verbs/use cases. Do not absorb app policy without repeated evidence.

### `apps/ghost-workflow`

Make context/tool/capability policy explicit and easy to change. It should be possible to experiment with a broader/raw FL surface without modifying `ghost-fl-studio`.

## 16. Knowledge that must survive cleanup

### Infrastructure/runtime invariants

These belong in the relevant low-level crates/tests:

- live-schema argument ordering for Gopher;
- recursive JSON callback normalization;
- transport failure vs inner FL tool failure;
- secret-safe Gopher logging;
- Windows Codex `.cmd`/shim handling;
- App Server thread/dynamic-tool routing;
- parallel routing ambiguity safeguards;
- FL/Gopher single-flight serialization;
- Ghost Tap realtime capture/trigger/pre-roll behavior.

### Product/application discoveries

These must not be lost, but they should live above the raw FL adapter:

- compact evidence/context can outperform huge session dumps;
- semantic parameter search is useful;
- MIDI parameter noise may need filtering for mixing tasks;
- plugin display strings may settle later than normalized values;
- semantic display calibration can work when value strings are usable;
- temporary probes should not be recorded as durable mutations;
- readback verification is useful for trustworthy agent execution;
- unrestricted normalized fallback can produce poor agent behavior;
- a live DAW can change independently of the agent;
- app/tool affordances materially influence model reasoning.

The reset preserves knowledge while correcting ownership.

## 17. Verification philosophy

Three truths remain separate.

### Static/build truth

Rust/Cargo proves type/dependency/ownership coherence.

### Deterministic integration truth

Scripted transports and small fixtures prove protocol routing, response normalization, capture state machines, app policy wrappers, and context/tool composition.

### Proprietary runtime truth

Only Windows + FL Studio + real third-party plugins prove final native behavior.

Live FL should validate the proprietary boundary, not discover ordinary Rust compile errors.

## 18. Exit criteria

The migration is complete when:

- only current vertical-slice/supporting components remain in the workspace;
- old nested host, egui UI, MixPlan, fake host, DB schema, daemon, and legacy CLI are absent;
- `ghost-core` is absent;
- `ghost-audio` owns deterministic audio understanding;
- `ghost-tap` owns realtime sensing/Tap protocol/minimal CLAP plugin;
- `ghost-codex` is free of the old mix domain;
- `ghost-fl-studio` is a transparent FL/Gopher adapter with raw access and no Ghost agent policy;
- `ghost-fl-studio` no longer owns `FlAgentToolPolicy`, `FlPluginWriteScope`, compact agent context, or fixed processor scope;
- app/Codex tool composition occurs outside `ghost-fl-studio`;
- `apps/ghost-workflow` explicitly owns the current experiment's context/capability/policy choices;
- reusable use-case sequencing lives in `ghost-application` only where justified;
- the raw FL diagnostic probe consumes the same adapter where practical;
- CI validates the reduced workspace;
- README describes `capture → analysis → agent → DAW` without contradictory architecture;
- a fresh clone builds/tests cleanly;
- the known-good live workflow remains green after both policy relocation and package split.

At that point the repository itself becomes a trustworthy input to the target-architecture pass and to future agents.

That is the purpose of this migration.