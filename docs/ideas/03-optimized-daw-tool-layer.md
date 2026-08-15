# Idea 03 — Optimized DAW tool layer above raw adapters

## Thesis

Keep `ghost-fl-studio` raw and transparent, then build **optional higher-level tools above it** only where benchmark traces prove that raw Gopher calls are inefficient, ambiguous or repeatedly composed the same way.

This is not a replacement for the raw adapter. It is an app/harness optimization layer.

```text
raw FL/Gopher tools
        ↓
optional composed tools
        ↓
agent
```

An app should remain free to expose either layer or both.

## Why this idea exists

Raw APIs are optimized for completeness and implementation, not necessarily for model cognition.

Examples of likely friction:

- relative level changes require read → calculate → write;
- safe routing changes require inspect routing → reason → set route → inspect again;
- plugin insertion requires browser/plugin discovery plus target/slot selection;
- plugin parameter work requires resolving a plugin instance, listing parameters, understanding names and then writing normalized values;
- a user intent such as "send these vocals to the vocal bus" may map to several low-level calls.

The vertical-slice era taught us the danger of solving this too early: we accumulated policy and assumptions inside the FL adapter and made previously proven operations appear to regress. This idea must preserve ADR 001.

## Design rule

A composed tool belongs above the adapter if it says something about how **Ghost wants to work**, rather than how FL must be invoked.

Good candidate:

```text
change_track_level_relative(target, delta_db)
```

implemented as:

```text
get_mixer_tracks_volume
calculate app-level target
set_mixer_tracks_volume_db
optional app-level verification
```

Bad candidate inside `ghost-fl-studio`:

```text
mix_vocals_better()
```

The first is a useful operation; the second is product reasoning disguised as infrastructure.

## Categories to discover from traces

### Resolution tools

Resolve user/app references into exact raw API coordinates.

Examples:

- resolve mixer track by semantic reference;
- resolve selected processor slot;
- resolve browser plugin name;
- resolve a channel and its routed mixer track.

### Read/modify/write tools

Hide deterministic arithmetic or state plumbing that should not consume model effort.

Examples:

- relative dB changes;
- relative pan/send changes;
- set only if current value differs;
- route while preserving existing unrelated sends.

### Transaction-like tools

Perform a bounded operation with explicit preconditions and post-observation.

Examples:

```text
insert_effect_if_slot_empty
move_channels_to_bus
create_send_if_missing
```

These should return rich observations, not simply `success=true`.

### Context/projection tools

Return a compact neighborhood around a selected entity rather than a giant session dump.

Examples:

- describe selected mixer track and immediate routing;
- describe selected channel → mixer → processor chain;
- describe selected plugin and useful parameter groups.

These may eventually become app-generated context rather than model-callable tools; the benchmark should tell us which shape performs better.

## Avoid a fixed universal semantic API too early

The danger is creating a pseudo-DAW API from imagined commonality.

For now:

- implement FL-specific compositions in the FL-focused app or an app-owned module;
- record which compositions recur;
- preserve the raw Gopher manifest alongside them;
- only extract a reusable crate/application API after repetition is obvious.

## Tool-surface experiments

For the same benchmark tasks, compare:

1. **Raw:** all relevant Gopher tools.
2. **Curated raw:** selected Gopher tools only.
3. **Hybrid:** raw tools + a few composed helpers.
4. **Semantic:** mostly composed helpers with raw escape hatch.

Measure:

- task success;
- number of tool calls;
- model tokens;
- latency;
- incorrect assumptions;
- destructive mistakes;
- human preference;
- ability to recover after the user changes FL state.

This turns tool design into an empirical agent-harness problem rather than taste.

## Relationship to selection

The most powerful composed tools may take structured references from [02-workspace-projection-selection.md](02-workspace-projection-selection.md):

```text
set_relative_level([Lead Vocal], -1.5 dB)
route([Backing Vocals], [Vocal Bus])
inspect([Drum Bus, Pro-C 3])
```

The app resolves those references immediately before execution.

## Score

- FL leverage: **5/5**
- Unlocks: **5/5**
- Differentiation: **4/5**
- Learning: **5/5**
- Effort: **3/5**
- Uncertainty: **3/5**
- Priority score: **24/30**

## Recommendation

**Do not build a large semantic layer now.** Start a notebook/backlog of repeated raw-tool pain during the general FL agent benchmark. Implement the first composed tools only when the same failure/call pattern appears repeatedly.