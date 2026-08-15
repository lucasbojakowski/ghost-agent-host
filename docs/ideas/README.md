# Ghost & Guild idea backlog

Status: exploratory backlog after the vertical-slice reset

Date: 2026-08-14

This folder is intentionally **not** an architecture specification. Each file captures a product/research branch that is now possible because the reset produced clean capabilities:

```text
ghost-tap       sensing / capture
ghost-audio     deterministic audio evidence
ghost-context   provider-neutral context vocabulary
ghost-codex     general Codex App Server runtime
ghost-fl-studio transparent FL/Gopher interface
apps/*          policy and concrete composition
```

The current `ghost-workflow` remains a valuable regression experiment, but it is deliberately narrow: Ghost Tap, one mixer target, a bounded slot range, a small plugin set, and an audio-processing task. The next app should not inherit those assumptions by default.

## Scoring

Scores are 1–5. Higher is better except **Effort** and **Uncertainty**, where lower is better.

- **FL leverage** — how directly the idea improves what we can build now on the already-working FL path.
- **Unlocks** — how many later ideas become easier or better if this exists.
- **Differentiation** — how much it helps Ghost become more than generic chat/MCP DAW control.
- **Learning** — how much concrete product/agent evidence we gain even if the idea is later discarded.
- **Effort** — expected engineering/research cost.
- **Uncertainty** — risk that external APIs/model behavior make the approach unproductive.
- **Priority score** — `FL leverage + Unlocks + Differentiation + Learning + (6 - Effort) + (6 - Uncertainty)`, max 30.

These scores are directional, not commitments. Re-score them as experiments produce evidence.

| Idea | FL leverage | Unlocks | Differentiation | Learning | Effort | Uncertainty | Score | Suggested horizon |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| [General FL agent + capability benchmark](01-general-fl-agent.md) | 5 | 5 | 3 | 5 | 2 | 2 | **26** | Now |
| [Context types as an app toolkit](11-context-types-library.md) | 5 | 5 | 4 | 4 | 2 | 2 | **26** | Now / alongside |
| [Workspace projection + semantic selection](02-workspace-projection-selection.md) | 5 | 5 | 5 | 5 | 4 | 3 | **25** | Next after raw benchmark |
| [Evals, trajectories + harness optimization](10-evals-harness-optimization.md) | 5 | 5 | 5 | 5 | 3 | 3 | **24** | Start now, deepen continuously |
| [Optimized DAW tool layer](03-optimized-daw-tool-layer.md) | 5 | 5 | 4 | 5 | 3 | 3 | **24** | After raw-tool evidence |
| [Task-specific FL projections/compositions](04-task-specific-apps.md) | 5 | 4 | 4 | 4 | 3 | 2 | **24** | After general agent |
| [Agent plugins / skills as the app boundary](05-agent-plugin-skill-architecture.md) | 4 | 4 | 4 | 5 | 3 | 4 | **22** | Research + prototype |
| [Plugin semantic profiles](12-plugin-semantic-profiles.md) | 4 | 4 | 4 | 5 | 4 | 4 | **19** | Focused R&D |
| [Closed-loop observe → act → listen → evaluate](08-closed-loop-audio-evaluation.md) | 4 | 4 | 5 | 5 | 4 | 4 | **20** | After general interaction works |
| [Structured reference matching](07-reference-matching.md) | 4 | 4 | 5 | 5 | 4 | 4 | **20** | Parallel audio R&D |
| [Deep audio understanding](06-deep-audio-understanding.md) | 3 | 3 | 4 | 5 | 4 | 4 | **19** | Parallel research track |
| [Multi-DAW transparent adapters](09-multi-daw-support.md) | 2 | 5 | 3 | 4 | 5 | 4 | **17** | Later; document crossings now |

## Recommended weave

The ideas reinforce each other most coherently as four tracks.

### Track A — learn the FL environment before abstracting it

```text
01 General FL agent
        ↓
02 Workspace projection + selection
        ↓
03 Optimized DAW tool layer
        ↓
04 Task-specific apps
```

The first experiment should expose the broad live Gopher surface to a generic agent and build a benchmark of normal FL tasks. We should discover where the raw interface is already sufficient and where the model repeatedly needs better state, references, or composed operations.

### Track B — make audio a first-class source of truth

```text
06 Deep audio understanding
        ├──→ 07 Reference matching
        └──→ 08 Closed-loop evaluation
                 ↑
12 Plugin semantic profiles ──┘
```

This is where Ghost can become materially different from generic DAW-control agents. Audio analysis should be attached when the task needs acoustic evidence rather than being mandatory for every interaction.

### Track C — turn agent behavior into an inspectable, evolvable system

```text
11 Context type toolkit
        ↓
05 Agent plugins / skills
        ↓
10 Evals + harness optimization
```

The goal is not to build a large framework immediately. It is to keep context, tool packaging, prompts, policies, and verification observable so successful app patterns can be promoted only after they are proven.

### Track D — preserve the path to multiple DAWs without paying for it yet

```text
FL experiments
   ↓
record every crossing point:
FL-specific fact vs DAW-general concept
   ↓
09 Multi-DAW adapters when a second real adapter exists
```

Do not create a universal `DawAdapter` from imagination. Build a crossing-point ledger while developing FL so a second DAW later has concrete interfaces to compare against.

## Recommended immediate program

1. Keep `apps/ghost-workflow` as the proven audio-processing regression slice.
2. Create a **new general FL agent app** with no Ghost Tap requirement and no mixer/plugin assumptions.
3. Build a task/eval corpus covering the actual Gopher surface: transport, channel rack, mixer, routing, plugin inspection/control, playlist, piano roll, browser/plugin discovery, naming/coloring and non-destructive organization.
4. Capture every trajectory: user request, initial observations, tools exposed, calls, errors, resulting state, latency/cost, and human judgement.
5. Use those traces to decide which state projections, context types and higher-level tools are actually worth building.
6. Only then build the first polished Tauri/Svelte interaction around semantic selection + chat.

## Research notes informing this backlog

### Ableton selection model

Ableton's own Arrangement workflow is fundamentally selection-based: select a clip, track or time span, then apply a command. The 2026 Extensions SDK strengthens this pattern by making extensions contextual to the selected Set item via right-click and allowing extensions to read/edit tracks, clips, parameters and other Set structure. This supports the hypothesis that Ghost context should be created by **pointing/selection first, language second**, rather than asking expert producers to type fragile locators.

Primary sources:

- Ableton Live 12 Arrangement View manual — selection-based editing.
- Ableton Extensions SDK announcement and Extensions FAQ — contextual extension invocation on selected Set items and structured Set access.

### Agent harness / plugin direction

I could not verify a single new DeepSeek-owned general-purpose harness that should dictate Ghost's architecture. The stronger current signal is the wider harness ecosystem: DeepSeek's official `awesome-deepseek-agent` repository documents use of DeepSeek models through many independent harnesses, while contemporary harnesses converge on skills/plugins, MCP/tool servers, persistent sessions, approvals, observability and runtime APIs. This argues for keeping Ghost capabilities packageable and composable rather than making one monolithic application layer.

The idea in [05-agent-plugin-skill-architecture.md](05-agent-plugin-skill-architecture.md) is therefore an experiment, not a decision.

## Backlog rule

An idea moves from this folder into architecture/ADR/implementation only after an experiment gives it evidence. Until then, optimize for learning and preserve the clean boundaries created by the reset.