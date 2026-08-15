# Idea 02 — Workspace projection + semantic selection

## Thesis

Ghost should let a producer **point at the workspace first and describe intent second**.

Instead of making the user type fragile locators such as mixer numbers, slot numbers, plugin names and time ranges, the app should maintain a lightweight projection of the FL project and let the user select/mark entities that become structured references attached to an agent turn.

Example UX:

```text
[Lead Vocal] [Pro-Q 4] [Vocal Bus] [bars 49–57]

"this gets nasal when she pushes here; fix it without losing the intimate tone"
```

The visible chips are backed by live app references, not interpolated strings.

## Why this matters

Expert DAW users already know how to navigate and point. Requiring them to describe the workspace in prose is often slower than doing the task manually.

Selection can solve several problems at once:

- reduce prompt verbosity;
- reduce ambiguity around track/plugin/clip identity;
- constrain context retrieval to a meaningful neighborhood;
- make agent actions easier to preview and explain;
- create a natural boundary for task-specific tools;
- make audio capture/analysis attachable to exactly the region/entity that matters.

## Ableton research signal

Ableton's native Arrangement workflow is explicitly selection-based: the user selects a clip, point or time span and then applies commands. Ableton's 2026 Extensions SDK reinforces contextual invocation: extensions appear from the right-click context of relevant Set items and can read/edit tracks, clips, parameters and other Set structure.

This is not proof that Ghost should copy Ableton's UI. It is evidence for a productive interaction principle:

> A DAW already has a rich pointing/selection language. An agent UI should reuse it rather than forcing all context through text.

Primary research sources:

- Ableton Live 12 Arrangement View manual.
- Ableton Extensions SDK announcement, June 2026.
- Ableton Extensions FAQ.

## Initial FL projection

Do **not** build a universal graph database. Start with what Gopher can actually observe.

A first app-owned snapshot might look conceptually like:

```text
FlWorkspaceSnapshot
  project / tempo / transport
  channels
    generator identity
    mixer assignment
    volume/pan/state
  mixer tracks
    name/state
    effect slots
    sends/routing
  playlist tracks
  current selection/focus where observable
```

The projection is explicitly incomplete and refreshable. It must never imply state that FL/Gopher cannot provide.

## Semantic references

Potential provider-neutral app/context types:

```text
EntityRef
  DawProject
  Channel
  MixerTrack
  ProcessorSlot
  PluginInstance
  PlaylistTrack
  Clip
  TimeRange
  Parameter

Selection
  refs[]
  optional time range
  observation timestamp/version
```

The reference should carry enough observed identity to detect obvious drift:

```text
MixerTrackRef
  daw = FL Studio
  index = 12
  observed_name = "Drum Bus"
```

Before a mutation, re-resolve the reference against current FL state. The human can edit the DAW at any time; references are handles into a mutable workspace, not authority over it.

## Context construction

Selection should drive retrieval:

```text
user selects Drum Bus + Pro-C + four-bar range
        ↓
app resolves current state
        ↓
fetches only useful relationships/state
        ↓
optionally captures/listens to selected range
        ↓
context compiler produces agent turn
```

This gives us a principled answer to context bloat: don't dump the project; expand around the user's focus.

## UI implications

The eventual Tauri/Svelte app could present:

- searchable tree/graph of channels, tracks, routing and processors;
- chips in the composer for selected entities;
- command to attach current DAW selection/focus;
- explicit `listen` attachment for a region/track;
- hover/inspection state showing what Ghost currently resolves each chip to;
- stale reference warnings if the workspace changed.

A later integration could learn to mirror FL's own active selection if Gopher exposes enough of it. Do not make that a prerequisite.

## Dependencies

Best informed by:

- [01-general-fl-agent.md](01-general-fl-agent.md) — tells us which state is actually useful;
- [11-context-types-library.md](11-context-types-library.md) — provides provider-neutral reference/context vocabulary;
- [03-optimized-daw-tool-layer.md](03-optimized-daw-tool-layer.md) — can use selections as inputs to higher-level actions.

## Risks

- Gopher may not expose enough live selection/focus state, requiring Ghost to maintain its own projection UI.
- Names/indices are mutable and sometimes ambiguous.
- A full project mirror could become expensive/noisy if refreshed indiscriminately.
- Over-generalizing entity types too early could create a fake multi-DAW ontology.

Mitigation: keep the projection FL-specific in the app first, while references/context types remain minimal and provider-neutral only where proven.

## Score

- FL leverage: **5/5**
- Unlocks: **5/5**
- Differentiation: **5/5**
- Learning: **5/5**
- Effort: **4/5**
- Uncertainty: **3/5**
- Priority score: **25/30**

## Recommendation

**High-priority product direction, but build it after the raw FL benchmark starts producing traces.** The goal is not a beautiful graph first; it is a useful semantic pointing system derived from real agent needs.