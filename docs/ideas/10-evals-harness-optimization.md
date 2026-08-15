# Idea 10 — Evals, trajectory datasets + harness optimization

## Thesis

Treat Ghost's prompts, context selection, tool surface, policies and verification behavior as an **experimental harness** that should be measured from execution traces rather than tuned by intuition.

Every meaningful FL agent run should produce a reproducible episode. Over time, use those episodes to evaluate and improve the harness while holding the underlying model constant or deliberately comparing models.

## Why this is foundational

The vertical-slice work repeatedly showed that agent behavior changed dramatically when we changed:

- which tools were exposed;
- how tool descriptions were written;
- how much DAW state was dumped into context;
- whether semantic vs normalized plugin controls were available;
- whether the agent was required to mutate;
- how failures were reported;
- whether verification was deterministic or left to the model.

Those are harness variables.

Without an eval corpus, every improvement risks recreating the earlier pattern where a fix for one scenario silently regresses another.

## Episode format

A first app-owned trace could include:

```text
Episode
  id
  app/capability versions
  model + reasoning settings
  user request
  selected references
  initial workspace observations
  audio/reference evidence ids
  context sent to model
  tools exposed + schemas
  agent events
  tool calls/results/errors
  final workspace observations
  optional before/after audio delta
  token/latency counts
  human rating/correction
  deterministic verifier results
```

Do not persist secrets or raw session-token URLs.

## Eval dimensions

Task success alone is insufficient.

Possible metrics:

- **completion:** did the requested state/result happen?
- **scope correctness:** did it touch only intended entities?
- **recovery:** did it handle stale state or tool errors coherently?
- **efficiency:** tool calls, retries, tokens, wall time;
- **context efficiency:** useful evidence per token;
- **verification:** did the final claim match observed state?
- **musical judgement:** human rating when objective verification is impossible;
- **audio outcome:** optional structured before/after/reference delta;
- **interaction quality:** did the agent ask when ambiguity truly required user input?

## Harness variants to test

The same task corpus can compare:

```text
raw tools vs curated tools
full session dump vs selected projection
text locators vs semantic references
single generic prompt vs task capability
agent verification vs deterministic wrapper verification
one model tier vs another
one reasoning effort vs another
```

This makes architectural choices testable.

## Current research signal

Recent agent-harness research increasingly treats the harness as part of the effective agent capability rather than mere plumbing. `AI Harness Engineering` frames context selection, tool access, project memory, verification, permissions and observability as runtime responsibilities. `Self-Harness` explores using execution traces to mine weaknesses, propose minimal harness changes and accept them only after regression testing. A July 2026 control-system paper similarly treats prompt/tool/memory/planning/verification choices as a small auditable action space that can be optimized against multi-objective rewards.

Ghost does not need self-modifying production agents now. The useful lesson is simpler:

> capture traces, classify failures, make small harness changes, regression-test them.

## Backlog mechanics

For every failure worth fixing:

1. add the scenario to the eval corpus first;
2. reproduce the failure;
3. classify whether it is adapter, context, tool, prompt, policy, model or external-state failure;
4. make the smallest change at the highest appropriate layer;
5. rerun the full relevant corpus;
6. keep the change only if it improves the target without unacceptable regressions.

This should become the standard way agent behavior is changed.

## Storage

Do not rush to reintroduce a large database schema. JSON/JSONL episode bundles on disk are enough initially and are easy to inspect/version.

A future persistence layer can emerge once we know which episode fields actually matter.

## Relationship to other ideas

This idea should accompany nearly all others:

- general FL agent → creates baseline episodes;
- selection/projection → compare context efficiency;
- optimized tools → compare tool-call count and reliability;
- plugin semantic profiles → compare parameter success;
- closed-loop audio → adds outcome evidence;
- task-specific apps/plugins → each can carry its own eval subset.

## Score

- FL leverage: **5/5**
- Unlocks: **5/5**
- Differentiation: **5/5**
- Learning: **5/5**
- Effort: **3/5**
- Uncertainty: **3/5**
- Priority score: **24/30**

## Recommendation

**Start immediately with the general FL agent.** Keep the first format simple and inspectable. The priority is reproducibility and failure attribution, not an elaborate evaluation platform.