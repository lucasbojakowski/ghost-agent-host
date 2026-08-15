# Idea 11 — `ghost-context` as an app-facing type toolkit

## Thesis

Treat `ghost-context` primarily as a **provider-neutral vocabulary and compilation toolkit** that apps can use to describe what they want an agent to know.

It should not decide what information belongs in a particular task.

```text
app chooses evidence / references / observations
        ↓
ghost-context represents + compiles them
        ↓
agent runtime receives provider-ready turn input
```

This fits the architecture that emerged after the reset better than turning `ghost-context` into a mandatory global context pipeline.

## Why this matters now

The next FL experiments will need to represent more than simple text messages:

- selected DAW entities;
- observations of mutable workspace state;
- relationships between entities;
- audio evidence attachments;
- reference-track evidence;
- provenance/timestamps;
- task instructions;
- output contracts.

If every app invents incompatible JSON/text conventions, context experiments become hard to compare. A small shared type library can provide consistency without dictating selection policy.

## Possible vocabulary

Only add types when a real app needs them. Candidate concepts include:

```text
ContextMessage
OutputContract
ContextFragment
EntityRef
Selection
Observation<T>
Relationship
AudioEvidenceRef
ReferenceEvidenceRef
Provenance
ContextMetadata
CompiledContext
```

### `EntityRef`

A typed handle to an observed workspace entity.

It should be able to carry provider-specific payload without pretending every DAW has the same object model.

Conceptually:

```text
EntityRef
  provider = "fl-studio"
  kind = "mixer-track"
  locator = { index: 12 }
  observed_label = "Drum Bus"
```

### `Observation<T>`

Explicitly model that context is a snapshot:

```text
Observation<T>
  value
  observed_at
  source/provider
  optional version/fingerprint
```

This reinforces the invariant that FL can change independently of the agent.

### `Selection`

A group of references the user/app intentionally attached to a task, optionally including a time range.

Selection is product-neutral enough to be shared, while how a particular FL UI creates it remains app-specific.

### Evidence refs

Audio/reference evidence should be attachable by identity/provenance rather than forcing every app to inline giant analysis blobs.

A compiler can decide whether to inline a compact summary, load a detailed artifact, or provide a tool for deeper inspection.

## Compiler role

A context compiler may own:

- ordering;
- labels/serialization;
- compact vs expanded representation;
- deduplication;
- output contract translation;
- token-budget decisions when configured by the app.

But the compiler should receive the app's chosen material. It should not decide, for example, that mixer track 1 or the whole session must be included.

## Context recipes

The existing recipe/compiler concept may become useful once multiple apps need repeatable projections.

Example:

```text
SelectedMixerTrackRecipe
  input: Selection + FL observations
  output:
    selected track
    processor chain
    immediate sends/receives
    explicitly selected related entities
```

A recipe is a reusable presentation strategy, not a global policy. Apps remain free to construct context manually.

## Keep provider-specific semantics out

`ghost-context` should not know:

- Gopher tool names;
- FL mixer slot numbering;
- Ableton Session vs Arrangement semantics;
- Codex App Server protocol details;
- which tools an agent is allowed to call.

Provider-specific payload can be carried opaquely/typed at the edge and rendered by app recipes where needed.

## Relationship to harness plugins

A capability/skill from [05-agent-plugin-skill-architecture.md](05-agent-plugin-skill-architecture.md) could contribute:

```text
context fragments
context recipes
required evidence types
```

without owning the shared context type system.

## Tests to prioritize

- stable serialization/versioning;
- deterministic compilation from the same inputs;
- explicit handling of stale/missing references;
- ability to render compact and detailed variants;
- no dependency on FL, Codex, audio DSP or app crates unless unavoidable.

## Score

- FL leverage: **5/5**
- Unlocks: **5/5**
- Differentiation: **4/5**
- Learning: **4/5**
- Effort: **2/5**
- Uncertainty: **2/5**
- Priority score: **26/30**

## Recommendation

**Evolve incrementally alongside the general FL agent and selection work.** Do not rewrite the crate upfront. Let each new real context need add the smallest useful provider-neutral type.