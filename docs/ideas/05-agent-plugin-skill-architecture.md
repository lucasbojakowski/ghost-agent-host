# Idea 05 — Agent plugins / skills as the app boundary

## Thesis

Explore whether some behavior we currently imagine as `apps/*` or `ghost-application` should instead be packaged as **agent-loadable capabilities**: skills, plugins, tool bundles, context recipes and policies that a general Ghost shell can activate per task.

This is a research direction, not an architectural decision.

## Why revisit the app layer

The current post-reset workspace intentionally left `ghost-application` almost empty because reusable use cases had not yet been proven. Meanwhile `ghost-codex` already supports persistent threads and dynamic tool registries.

That suggests another possible product shape:

```text
Ghost shell / desktop
      ↓
persistent agent thread
      ↓
load capability bundle(s)
      ↓
context recipe + tools + policy + UI hints
```

A "mix" capability could be loaded only when needed. An "arrange" capability could expose a different projection and tool set. A "reference" capability could attach audio/reference-analysis tools.

## Current harness research signal

I could not verify a single new DeepSeek-owned general-purpose harness that should be copied directly. The more important current signal is ecosystem convergence.

DeepSeek's official `awesome-deepseek-agent` repository now documents integrations with many independent harnesses rather than prescribing one application shell. Current harnesses in that ecosystem commonly support combinations of:

- project/user skills;
- MCP servers;
- plugins/extensions;
- persistent/tree-structured sessions;
- subagents;
- approval modes;
- lifecycle hooks;
- runtime APIs for embedding agent sessions in other UIs.

A community DeepSeek-TUI integration, for example, describes skills, MCP client/server operation, hooks, subagents and an HTTP runtime API. Pi is presented as a minimal harness extended through TypeScript extensions, skills and prompt templates.

The lesson for Ghost is not "use DeepSeek" or "replace Codex." It is:

> Agent behavior may be easier to evolve when domain capabilities are packageable independently from the harness and desktop shell.

## Possible Ghost capability package

Conceptually:

```text
GhostCapability
  id / version
  instructions
  context recipe
  tool registrations
  optional pre/post hooks
  required underlying capabilities
  optional UI contributions
  optional eval suite
```

Example:

```text
fl.mix.selected-track
  requires:
    ghost-fl-studio
    optional ghost-tap
    optional ghost-audio
  provides:
    selected-track context expansion
    relative level/routing helpers
    plugin inspection tools
    optional listen/compare action
```

The actual implementation does not need to be dynamic-library/plugin loading. A Rust trait or static registry may be enough for the first experiment. "Plugin" here means a capability boundary, not necessarily a binary ABI.

## Why this could be powerful

- app behavior becomes composable without making `ghost-application` a framework bucket;
- the desktop can remain a stable shell while capabilities evolve quickly;
- different agent/model harnesses could consume the same high-level Ghost capability definition;
- capability-specific evals can travel with the capability;
- tools/context/policy are versioned together instead of drifting independently;
- users could eventually enable purpose-specific agents without loading the entire DAW surface.

## Why this could be over-engineering

We currently have one real app and one DAW adapter. A plugin framework built now could encode imaginary extension points.

We should therefore test the idea with **static composition first**:

```text
trait / struct representing a capability bundle
        ↓
register into existing ToolRegistry
        ↓
append/compile context fragments
```

If that immediately feels forced, abandon it.

## Research questions

1. Can tools + instructions + context recipes be treated as one versioned unit?
2. Can multiple units compose without prompt/tool-name conflicts?
3. Does the Codex App Server thread need to be restarted when capabilities change, or can its dynamic tool surface evolve cleanly?
4. Should capability activation be visible to the model as an explicit mode or simply reflected in available tools/context?
5. Can a capability carry its own eval cases and acceptance criteria?
6. Would the same package shape work with a non-Codex harness later?

## Relationship to `ghost-application`

This idea may reveal that `ghost-application` should become:

- a small set of reusable capability/use-case types; or
- unnecessary until multiple shells actually need shared orchestration.

Do not preserve the crate merely to satisfy an architecture diagram.

## Score

- FL leverage: **4/5**
- Unlocks: **4/5**
- Differentiation: **4/5**
- Learning: **5/5**
- Effort: **3/5**
- Uncertainty: **4/5**
- Priority score: **22/30**

## Recommendation

**Prototype only after the general FL agent exists.** Use one or two real FL capabilities as the test. Avoid dynamic loading, marketplace concepts or a generic plugin ABI until static capability bundles prove useful.