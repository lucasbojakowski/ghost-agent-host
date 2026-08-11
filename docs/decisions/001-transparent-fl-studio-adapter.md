# ADR 001: Keep the FL Studio adapter transparent and policy-free

- Status: Accepted
- Date: 2026-08-11
- Decision owners: Ghost & Guild
- Applies to: `ghost-fl-studio`, app/tool composition, context construction

## Context

The first successful Ghost & Guild vertical slice emerged while we were still discovering the runtime behavior of FL Studio 2026's Gopher/native tool surface.

During that discovery we encountered several genuine integration problems:

- Gopher tool arguments are sensitive to the live schema/property order;
- callback payloads can be multiply JSON encoded;
- transport-level success can contain an inner native-tool error;
- the Gopher bridge must be treated as single-flight;
- third-party plugin value strings may lag normalized parameter state or be unsupported;
- Windows/runtime discovery details must be handled carefully.

Those findings are properties of the real FL/Gopher integration and belong in the FL adapter.

At the same time, the vertical-slice implementation accumulated a second class of behavior inside `ghost-fl-studio`:

- one target mixer track;
- one allowed effect-slot interval;
- a Pro-Q / Pro-C allowlist;
- compact target-track context intended specifically for one agent workflow;
- rules such as “do not overwrite an occupied slot”;
- restrictions on which parameter identifiers or normalized values an agent may use;
- semantic display-domain calibration and fallback policy;
- mutation journaling and workflow-specific verification;
- Codex-facing tool registration and replacement.

These were useful while stabilizing the experiment, but they are not truths about FL Studio. They are policy and reasoning choices made by one Ghost application.

This distinction became obvious during live testing. The user could manually remove or change a processor after the agent had observed it. That is normal behavior in a shared DAW workspace. Encoding previous observations and workflow assumptions deep in the adapter made later commands appear to regress even when the underlying native operation still worked.

The resulting architecture also caused proven commands to acquire new semantics over time. `ghost-fl-studio` first registered a base tool and later replaced it with stricter product-facing implementations. A command that had previously meant “invoke this FL capability” could later mean “invoke it only if this Ghost-specific scope, slot, identifier, calibration, and fallback policy all permit it.” This made integration bugs and product-policy behavior difficult to distinguish.

## Decision

`ghost-fl-studio` will be a **transparent, faithful adapter to the FL Studio/Gopher API**.

Its responsibility is:

> Represent what FL Studio exposes and invoke it reliably without inventing Ghost-specific policy.

The adapter may expose both:

1. a raw catalog/call interface driven by the live Gopher schemas; and
2. thin typed Rust wrappers where one Rust operation maps directly to one FL/Gopher operation.

The adapter must not decide how much FL capability an agent or application should receive.

### Layer boundary

The intended direction is:

```text
FL Studio / Gopher
        │
        ▼
ghost-fl-studio
  transparent API mirror
        │
        ▼
caller
  app / application / context / tool composition
```

A useful rule is:

```text
Does this behavior exist because FL/Gopher behaves this way?
    → ghost-fl-studio

Does this behavior exist because Ghost wants to behave this way?
    → outside ghost-fl-studio
```

## What belongs in `ghost-fl-studio`

The crate should retain and test integration invariants such as:

- discovery/connection to the Gopher WebView/CDP target;
- live capability catalog and schemas;
- canonical argument ordering derived from the live schema;
- CDP/native message framing;
- recursive/multiply encoded JSON normalization;
- preservation of native result data;
- clear distinction between transport failure and inner FL tool failure;
- single-flight serialization required by the native bridge;
- secret-safe connection logging;
- stable error types that preserve enough native information for callers to reason about failures;
- thin typed wrappers for real FL operations such as tempo, transport, effect insertion/removal, parameter list/read/write, routing, and other tools actually present in the live catalog.

Typed wrappers are allowed when their semantic relationship is approximately:

```text
one FL operation ↔ one Rust operation
```

A raw escape hatch must remain available so a caller is not limited by the subset for which typed wrappers currently exist.

## What does not belong in `ghost-fl-studio`

The following concepts must move out of the adapter:

- `FlAgentToolPolicy`;
- `FlPluginWriteScope` or equivalent Ghost write scopes;
- target-track or slot-range policy;
- processor allowlists;
- “must be empty” / “never replace” workflow policy;
- agent-specific tool filtering;
- compact agent context projections;
- Codex `ToolRegistry` construction;
- parameter-search relevance policy such as MIDI filtering for one workflow;
- continuous-parameter safety policy;
- restrictions such as boolean-only normalized writes;
- display-domain calibration as a mandatory mutation path;
- mutation journals owned by one workflow;
- requirements such as “the task must produce at least one mutation.”

Some of those mechanisms may remain useful, but their initial owner should be the concrete app that requires them.

## Where policy starts

Policy should begin as high in the stack as practical.

For the current reference workflow:

```text
apps/ghost-workflow
    chooses target resources
    chooses context projection
    chooses which FL tools the model receives
    optionally wraps mutations with preconditions/verification
    chooses plugin/slot policy
    chooses whether raw operations are exposed
```

`ghost-application` coordinates reusable use-case sequencing, but it should not automatically absorb app policy merely because the policy exists. A rule should move from `apps/*` into `ghost-application` only after repeated real workflows demonstrate that it is a reusable product requirement.

`ghost-context` may own reusable transformations from observations/evidence into agent-visible context, but the app chooses which observations and context components are used.

## Raw access is a supported mode

An application may intentionally expose a broad or nearly raw FL surface to an agent.

For example:

```text
live Gopher catalog
        ↓
app selects none / some / all suitable tools
        ↓
ToolRegistry
        ↓
Codex thread
```

Another application may expose only five wrapped tools. Both are valid consumers of the same `ghost-fl-studio` crate.

The adapter must not privilege the constrained-agent use case over raw expert access.

## Live workspace assumption

The system will assume:

> FL Studio state may change independently of Ghost at any time.

Therefore:

- observations are snapshots, not authoritative persistent state;
- callers that need concurrency safety may attach preconditions to mutations;
- a failed precondition should cause re-observation/reasoning, not corruption of adapter state;
- the DAW/native result is the source of truth, not previously supplied model context.

If an app wants an operation such as `insert_if_empty`, it should compose that behavior above the raw adapter:

```text
read current slot
    ↓
check app precondition
    ↓
call raw add_effect
    ↓
optional app verification
```

The adapter may provide the raw reads/writes needed to implement this, but it does not own the policy itself.

## Verification and journaling

Readback verification and mutation journaling were essential to proving the initial workflow, but they are not part of the minimum transparent FL API mirror.

The primitive adapter call should report the native operation faithfully.

A caller may then choose to:

- read the value back;
- wait/poll for plugin state;
- create a mutation ledger entry;
- retry;
- restore prior state;
- require user confirmation;
- reject a changed precondition.

If later multiple applications require the exact same verification semantics, that behavior can be promoted into a reusable layer. It should not be embedded in the adapter preemptively.

## Parameter semantics

FL exposes normalized plugin parameter values and, for some plugins, human-readable value strings. The adapter should expose those facts faithfully.

The current display-domain calibration machinery is higher-level inference over those primitives. It should therefore move out of `ghost-fl-studio` during the reset.

An app may choose to:

- expose raw normalized writes directly;
- hide them from the agent;
- provide semantic value calibration;
- use cached plugin profiles;
- use only binary/discrete controls;
- expose both raw and semantic tools.

These are application and reasoning-design choices.

## Codex dependency

`ghost-fl-studio` should not depend on `ghost-codex` merely to register agent tools.

The FL crate exposes FL capabilities. The app composes those capabilities into a `ghost-codex::ToolRegistry` when an agent needs them.

This keeps both infrastructure crates reusable independently:

```text
ghost-fl-studio
    knows FL

ghost-codex
    knows Codex App Server

apps/ghost-workflow
    knows why/how to combine them
```

## Middleware/callbacks

We considered adding callback or middleware hooks inside the FL adapter so an app could intercept tool calls.

We will **not add that abstraction now**.

The caller can already wrap raw FL operations when constructing its own tools or use cases. That provides equivalent flexibility without coupling the adapter to a middleware framework before concrete repeated requirements exist.

If multiple apps later need common pre/post-call hooks, middleware can be introduced from demonstrated use cases.

## Consequences

### Positive

- native integration bugs become distinguishable from Ghost policy failures;
- commands keep stable meanings across product experiments;
- raw FL capabilities remain available to future apps;
- app experiments can vary context/tool exposure without rewriting the adapter;
- human edits to the DAW are treated as normal live-state changes;
- `ghost-fl-studio` can be tested as a faithful protocol adapter;
- `ghost-codex` no longer needs to be coupled into the FL crate;
- policy can evolve rapidly at the app level without destabilizing infrastructure.

### Costs

- the reference app temporarily owns more tool-composition code;
- safe processing behavior that was previously deep in the adapter must be moved upward carefully during migration;
- some higher-level operations may be duplicated between apps until a real shared abstraction emerges;
- callers that choose raw access accept responsibility for filtering, verification, and policy.

These costs are intentional. We prefer visible duplication at the outer layer over premature deep abstractions that change the meaning of the underlying API.

## Migration implications

During the workspace reset:

1. preserve the existing green vertical slice as a regression reference;
2. separate raw FL/Gopher mechanics from Ghost-specific tool/policy code;
3. remove `ghost-codex` as a dependency of `ghost-fl-studio` if possible;
4. move current scoped processor-tool composition into `apps/ghost-workflow` initially;
5. move compact context projection to app/context composition;
6. move display calibration, normalized-write restrictions, slot policy, plugin allowlists, verification, and journaling upward;
7. retain raw typed/native FL operations and the runtime invariants listed above;
8. add/retain tests proving the raw adapter behavior independently of any agent policy;
9. keep a raw catalog/call path so newly exposed Gopher tools can be used before a typed wrapper exists.

The migration should preserve successful behavior at the reference-app level while simplifying the lower layer.

## Promotion rule

New behavior starts at the highest practical layer.

```text
apps/*
  ↓ repeated requirement across real apps

ghost-application / ghost-context
  ↓ only if it is actually an infrastructure invariant

infrastructure crate
```

We do not move policy downward because it feels generally useful. We move it only after repeated real use demonstrates the correct abstraction.

## Summary

`ghost-fl-studio` is a mirror of reality, not a guardian of one Ghost workflow.

Ghost policy lives above it.

The initial product can still be conservative, scoped, verified, and semantically rich, but those properties are composed by the app rather than baked into the FL Studio adapter.