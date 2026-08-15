# Idea 01 — General FL agent + capability benchmark

## Thesis

Build a new app whose purpose is simply:

> Let a frontier agent operate the broad live FL Studio/Gopher environment coherently.

This should be separate from `apps/ghost-workflow`. The existing workflow is a regression slice for `capture → analysis → agent → DAW`; it intentionally assumes Ghost Tap, a mixer target, a slot range, a small plugin set and an audio-processing intent. A general FL agent should inherit none of those assumptions.

## Why now

`ghost-fl-studio` already gives us the important baseline: a transparent, policy-free mirror of the live Gopher catalog. Before designing a richer workspace model, semantic tools or generic DAW abstractions, we should learn how well a strong agent performs when it receives that environment with minimal transformation.

This is the fastest way to distinguish:

- problems the model can already solve from raw Gopher schemas;
- missing/awkward FL observations;
- confusing tool naming or argument conventions;
- state that should be projected into context rather than repeatedly queried;
- tasks that need higher-level composed tools;
- tasks that are fundamentally limited by the FL/Gopher API.

## Proposed app

Create a new experimental app, tentatively:

```text
apps/ghost-fl-agent/
```

Its first version should be intentionally plain:

```text
connect to Gopher
    ↓
load live manifest
    ↓
select an explicit app-owned tool profile
    ↓
start persistent Codex thread
    ↓
chat / execute / observe
```

No Ghost Tap requirement. No default Pro-Q/Pro-C assumptions. No mixer-only scope. No universal write-policy framework yet.

The first profile can be broad but still split destructive tools from routine edits so we can benchmark safely. That split belongs in the app, not `ghost-fl-studio`.

## Benchmark corpus

Build a small repeatable suite of ordinary FL tasks rather than evaluating from anecdotes.

Suggested families:

### Read / navigation

- report tempo and current project structure;
- list channel rack generators;
- inspect mixer routing;
- inspect plugins on a track;
- find a plugin in the browser;
- open the piano roll for a named channel;
- locate project elements by human names.

### Organization

- rename channels/mixer/playlist tracks;
- color related tracks consistently;
- select/solo/mute a named group;
- route channels to a mixer track;
- create or repair bus routing.

### Musical editing

- write a simple step sequencer pattern;
- quantize a selected channel;
- run a bounded piano-roll script;
- create a small musical variation using existing project context.

### Mixing / processing

- insert a named effect discovered from the browser;
- inspect plugin parameter state;
- change an unambiguous normalized parameter;
- make a relative mixer-level change after reading current values;
- create a send/return route after inspecting the routing matrix.

## What to record

Every run should become an episode:

```text
request
initial app/tool configuration
observations requested by the model
tool calls + results
errors/retries
final workspace observation
latency/token/tool counts
human judgement
```

Do not optimize prompts from memory. Make failures reproducible.

## Cross-DAW ledger

While implementing the benchmark, record every concept we touch as one of:

- **FL-specific:** Gopher tool/schema behavior, mixer numbering, channel rack semantics, browser paths, piano-roll script mechanics;
- **likely DAW-general:** track identity, selected entity, clip/time range, routing edge, processor slot/device, parameter, transport, automation, project observation;
- **unknown:** concepts that need a second DAW before classification.

This is the evidence base for eventual multi-DAW support. Do not turn the ledger into a `DawAdapter` yet.

## Success criteria

The experiment is successful when we can answer with data:

1. Which Gopher tools can be exposed almost raw?
2. Which common tasks repeatedly fail because of context rather than tool capability?
3. Which tool calls need composition or safer semantics?
4. Which parts of FL state are worth projecting continuously?
5. Which tasks are impossible or awkward because Gopher does not expose the necessary state/action?
6. Which concepts already look portable to a second DAW?

## Score

- FL leverage: **5/5**
- Unlocks: **5/5**
- Differentiation: **3/5**
- Learning: **5/5**
- Effort: **2/5**
- Uncertainty: **2/5**
- Priority score: **26/30**

## Recommendation

**Build this next.** It gives the rest of the backlog evidence and prevents us from designing a semantic proxy, generic application layer or multi-DAW interface from assumptions.