# Idea 04 — Task-specific FL projections and app compositions

## Thesis

After the general FL agent reveals what the raw environment looks like, build **small task-specific apps or modes** that compose only the state, analysis and tools needed for one job.

The goal is not to create a single universal Ghost agent with every capability always loaded. Different production tasks may benefit from very different context projections and tool surfaces.

## Examples

Potential experiments:

### Mix assistant

Inputs:

- selected mixer tracks/processors;
- optional Ghost Tap audio evidence;
- routing neighborhood;
- plugin semantic profiles where available.

Tools:

- inspect/set mixer levels;
- routing;
- processor insertion/removal when explicitly enabled;
- parameter inspection/control;
- capture/listen/relisten.

### Session organizer

Inputs:

- channel/mixer/playlist structure;
- names/colors/routing.

Tools:

- rename;
- color;
- route;
- create buses;
- select/mute/solo.

No audio analysis required.

### Arrangement assistant

Inputs:

- selected playlist region;
- track/clip neighborhood;
- optional audio/MIDI summaries.

Tools:

- playlist naming/organization;
- piano-roll operations exposed by Gopher;
- future clip/automation operations where available.

### Plugin assistant

Inputs:

- one selected plugin instance;
- parameter groups/profile;
- optional audio observation before/after.

Tools:

- inspect parameters;
- set values;
- compare state;
- optionally learn/profile parameter semantics.

## Why apps/modes rather than immediate shared abstractions

The reset established an important rule:

> Policy starts high and moves downward only after repeated real workflows prove a reusable requirement.

Task-specific compositions are the best environment for that rule. They let us experiment aggressively without contaminating `ghost-fl-studio`, `ghost-codex`, `ghost-audio` or `ghost-context` with one workflow's assumptions.

The current `ghost-workflow` is already an example of this pattern, but it is too narrow to be the product architecture. We can keep it as a regression fixture while creating new experiments with different assumptions.

## What an app owns

A task-specific app/mode can legitimately own:

- which DAW entities are selected;
- which state is attached to the turn;
- whether audio capture is required;
- which raw/composed tools are exposed;
- destructive-action policy;
- output format;
- verification/evaluation policy;
- task-specific system instructions;
- whether a persistent thread is reused;
- how results are rendered in UI.

This is exactly the behavior we do **not** want pushed into the transparent FL adapter.

## When to promote behavior downward

Only promote something into `ghost-application`, `ghost-context` or another shared crate when at least two real app modes need essentially the same semantic operation.

Examples that might eventually qualify:

- build an agent turn from selected workspace references;
- request and attach an audio observation;
- run an action and capture a post-action observation;
- represent an application-level selection/reference;
- persist an execution episode.

Until repetition exists, duplication inside experimental apps can be cheaper and clearer than a premature framework.

## UX implication

A polished Ghost desktop app may eventually expose these not as separate executables, but as **modes/projections** around one persistent conversation:

```text
General
Mix
Arrange
Organize
Plugin
Reference
```

The selected mode could change context expansion and tool availability without forcing the user to understand architecture.

This also connects to the plugin/skill idea: an app mode could eventually be packaged as a loadable agent capability rather than compiled into one monolith.

## Score

- FL leverage: **5/5**
- Unlocks: **4/5**
- Differentiation: **4/5**
- Learning: **4/5**
- Effort: **3/5**
- Uncertainty: **2/5**
- Priority score: **24/30**

## Recommendation

**Use task-specific compositions as the main product-learning mechanism after the general FL agent.** Do not try to predict the final universal Ghost application API first.