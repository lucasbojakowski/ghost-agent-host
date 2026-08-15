# Idea 09 — Multi-DAW support through transparent adapters

## Thesis

Eventually support multiple DAWs by repeating the successful FL rule:

> Each DAW adapter should faithfully expose what that DAW actually provides, without forcing all DAWs through a prematurely generic interface.

Do not build `DawAdapter` now. Build a **crossing-point ledger** while the FL product deepens, then add a second real adapter and compare concrete surfaces.

## Why multi-DAW matters

Long-term product value is larger if Ghost's core capabilities are not tied to one workstation:

- audio sensing/analysis is already largely DAW-neutral;
- `ghost-codex` is fully domain-neutral;
- `ghost-context` can remain provider-neutral;
- selection/reference concepts may generalize;
- task-specific capabilities such as reference matching or closed-loop listening could work in several hosts.

But the control APIs, project models and runtime constraints differ significantly between DAWs. Pretending otherwise too early would recreate the coupling problems the reset removed.

## Proposed strategy

### Phase 1 — document FL crossing points now

As the general FL agent is built, maintain a table such as:

| Concept | FL representation | Likely portable? | Notes |
|---|---|---|---|
| transport | Gopher play/stop/tempo | yes | exact positioning TBD |
| channel rack channel | FL-specific | maybe maps to instrument/source track imperfectly | do not normalize yet |
| mixer track | FL mixer insert | broadly | numbering/routing differs |
| effect slot | mixer slot 1–10 | maybe | many DAWs use ordered device chains without fixed slots |
| send routing | mixer routing matrix | broadly | topology/semantics differ |
| plugin parameter | target + slot + normalized parameter | broadly | discovery/display semantics differ |
| playlist track | FL playlist track | broadly-ish | relationship to mixer/channel is FL-specific |
| piano-roll script | FL-specific Gopher action | no/unknown | likely adapter-specific capability |

The purpose is evidence, not API design.

### Phase 2 — choose a second DAW for maximum contrast

A good second adapter should teach us something rather than merely resemble FL.

Ableton is a strong candidate because:

- its object model differs substantially from FL's channel-rack/mixer/playlist split;
- Arrangement and Session introduce different selection concepts;
- the 2026 Extensions SDK provides structured Set access through JavaScript extensions;
- a large third-party agent ecosystem already exists around Live, giving us external benchmarks.

REAPER could also be valuable because its scripting/control surface is broad and relatively automation-friendly.

Choose based on access, API quality and learning value, not market coverage alone.

### Phase 3 — compare real adapters

Only after two adapters exist should we identify genuinely shared application concepts.

Potential shared vocabulary may include:

```text
WorkspaceRef
TrackRef
DeviceRef / ProcessorRef
ClipRef
TimeRange
ParameterRef
RoutingEdge
Selection
TransportObservation
```

But even then, allow adapter-specific extensions rather than forcing lossless translation into one lowest-common-denominator model.

## Architecture hypothesis

Long-term:

```text
FL Studio          Ableton          Future DAW
    │                 │                 │
transparent         transparent        transparent
adapter              adapter             adapter
    │                 │                 │
    └──────── app/context capability layer ────────┘
```

The common layer should express **Ghost concepts**, not pretend the DAWs themselves are identical.

## Agent portability

The general agent experiments should explicitly record where prompts/tools contain FL-specific assumptions.

Examples:

- "slot number" is FL-specific enough that a generic agent instruction should not depend on it;
- "selected processor/device" may generalize;
- "channel rack" should remain FL vocabulary;
- "track" can be ambiguous even within one DAW and needs typed references.

A cross-DAW agent might receive adapter-specific tools plus a small shared context vocabulary rather than one universal tool set.

## Risks

- multi-DAW work can dilute FL product velocity;
- each DAW may require a radically different bridge/runtime strategy;
- a generic interface can become lowest-common-denominator and hide powerful native capabilities;
- testing proprietary DAW versions multiplies operational cost.

## Score

- FL leverage: **2/5**
- Unlocks: **5/5**
- Differentiation: **3/5**
- Learning: **4/5**
- Effort: **5/5**
- Uncertainty: **4/5**
- Priority score: **17/30**

## Recommendation

**Do not implement a second DAW yet.** Start the crossing-point ledger immediately while building the general FL agent. Add the second adapter only after the FL interaction/product model is materially clearer.