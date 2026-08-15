# Idea 08 — Closed-loop observe → act → listen → evaluate

## Thesis

The strongest long-term Ghost workflow may be closed-loop:

```text
observe workspace + capture A
        ↓
analyze A
        ↓
agent acts
        ↓
observe workspace + capture B
        ↓
analyze B
        ↓
evaluate change
        ↓
accept / refine / revert / ask user
```

Native readback tells us whether FL executed a command. Re-listening tells us whether the command produced the intended acoustic result.

## Why this matters

Most agentic DAW demos stop at:

```text
intent → tool call → changed DAW state
```

For many production tasks, that is weak verification. A compressor parameter can change correctly while the resulting dynamics are worse. An EQ can be inserted successfully while the selected band misses the actual problem.

Ghost already owns the pieces needed to explore a stronger definition of success:

- `ghost-tap` can acquire live audio;
- `ghost-audio` can produce deterministic before/after evidence;
- `ghost-codex` can keep the same reasoning thread alive;
- `ghost-fl-studio` can mutate and re-observe the workspace.

## First experiment

Use a tightly controlled task rather than a whole mix.

Example:

```text
selected mixer track
selected 4–8 bar region
user intent: reduce harshness without reducing presence
```

Flow:

1. capture the selected region as A;
2. analyze A;
3. agent inspects current FL state and makes one bounded change;
4. replay the same region;
5. capture B;
6. analyze B;
7. produce a structured `AudioDelta`;
8. ask the same thread whether the result moved toward the stated intent;
9. allow at most one refinement pass initially.

The goal is not autonomous endless optimization. It is to prove that feedback changes agent quality.

## Deterministic delta

A reusable delta representation might include:

```text
peak / true-peak change
integrated/rms change
crest/dynamic change
band balance change
spectral tilt change
resonance prominence change
stereo correlation/width change
flags introduced/removed
```

The agent should receive both the delta and enough absolute state to avoid optimizing one metric blindly.

## Capture repeatability

Closed-loop work requires more precise capture semantics than the current "play and wait for signal" regression slice.

Questions:

- Can the app reliably replay the same time range?
- What transport/position information can Gopher expose?
- Should Ghost Tap capture a fixed sample/time range tied to a DAW selection?
- How do plugin latency and tails affect alignment?
- Should comparisons ignore onset/offset windows?
- Can the app detect that the user changed unrelated project state between A and B?

These are product/runtime questions and may become one of the strongest reasons to improve the DAW workspace projection.

## Evaluation policy

Do not reduce "better" to one score initially.

A structured evaluation could contain:

```text
measured changes
agent interpretation
intent-alignment judgement
constraint checks
uncertainties
human preference
```

For reference matching, add reference-distance changes.

For a simple level task, deterministic verification may be enough and no audio loop is necessary. Closed-loop listening should be invoked only when acoustic outcome matters.

## Reversibility

This idea becomes much stronger if app-level mutation episodes can record reversible actions or enough before-state to restore them.

Potential loop:

```text
change
  ↓
listen
  ↓
regressed badly?
  ├─ yes → restore
  └─ no  → keep / refine
```

Do not push reversibility into `ghost-fl-studio`; it is workflow policy and may vary by operation.

## Differentiation hypothesis

A generic DAW agent can control a project. A closed-loop Ghost agent can **hear the consequence of its own actions** and reason over measurable before/after evidence.

That combination is closer to an actual production collaborator than a chat-controlled macro system.

## Risks

- capture alignment and repeatability can dominate engineering effort;
- feature deltas can reward metric gaming rather than musical quality;
- iterative loops can become slow/costly and annoying;
- user changes during the loop make attribution difficult;
- many creative tasks do not have an objectively better acoustic outcome.

## Score

- FL leverage: **4/5**
- Unlocks: **4/5**
- Differentiation: **5/5**
- Learning: **5/5**
- Effort: **4/5**
- Uncertainty: **4/5**
- Priority score: **20/30**

## Recommendation

**Do after basic general FL interaction and selection/reference mechanics exist.** Start with one measurable bounded task and one refinement pass. Treat human preference as part of evaluation rather than trying to automate taste.