# Ghost & Guild

Ghost & Guild is an SDK/runtime for building agentic music-production applications.

The project keeps FL Studio authoritative for the real DAW state and processors while Ghost adds sensing, analysis, agent runtimes and higher-level workspace capabilities.

```text
capture → analysis → agent → DAW
```

## Repository architecture

The monorepo is intentionally split into two architectural projects:

```text
crates/*  = reusable Ghost Core / SDK
apps/*    = product, harness and UI composition
```

Core crates own reusable mechanisms and integration invariants. Apps decide what is exposed to humans/agents, how tools and context are composed, which harness/protocol is used, and which product semantics apply.

See [SDK architecture](docs/SDK_ARCHITECTURE.md).

## Core / SDK

```text
crates/
  ghost-audio/          deterministic audio analysis/evidence
  ghost-tap/            transparent CLAP capture/sensing primitive
  ghost-context/        provider-neutral context support
  ghost-codex/          reusable Codex App Server runtime
  ghost-fl-studio/      transparent FL Studio Gopher/CDP adapter
  ghost-fl-scripting/   transparent FL MIDI Scripting adapter
  ghost-application/    reserved promotion boundary
```

The two FL crates intentionally remain separate. Gopher and MIDI Scripting are different real FL surfaces with different transports, lifecycles and capabilities; applications compose them when needed.

## Current applications

```text
apps/
  ghost-fl-agent/       frozen direct-Codex raw Gopher control group
  ghost-fl-workspace/   live-proven Codex Gopher + scripting composition
  ghost-fl-mcp/         live-proven external MCP raw Gopher control group
  ghost-fl-runtime/     experimental FL/session/app lifecycle supervisor
  ghost-workflow/       capture → analysis → scoped FL regression workflow
web/
  runtime/              Bun + SvelteKit runtime shell and routed app host
  packages/             generated runtime contracts and shared UI primitives
```

`ghost-fl-runtime` is intentionally app-owned while lifecycle supervision is still being proven. It attaches to or launches one FL Studio session, establishes Gopher readiness, supervises registered Ghost app fixtures, records structured operational state/events and exposes a small diagnostic control panel without moving product orchestration into `ghost-application` prematurely.

The runtime web workspace is a projection of that Rust-owned lifecycle, not a TypeScript server or workspace-kernel implementation. Rust serves its optimized static assets, streams state over WebSockets, and exposes registered applications through shell routes. Use `cargo xtask web` to generate Rust-derived TypeScript contracts, validate the frontend and build the embedded bundle.

Current harness/control matrix:

```text
                         Direct Codex                 External MCP
Raw Gopher               ghost-fl-agent               ghost-fl-mcp
                         PROVEN / CONTROL GROUP        PROVEN / CONTROL GROUP

Expanded FL              ghost-fl-workspace           next experiment
Gopher + Scripting        PROVEN
```

See [FL capability surfaces](docs/FL_CAPABILITY_SURFACES.md).

## Proven baselines

Do not infer current status from old experiment prompts or branch names. The canonical baseline index is:

- [Proven baselines](docs/PROVEN_BASELINES.md)

Current accepted evidence includes:

- raw live Gopher agent baseline;
- promoted `ghost-fl-scripting` with live FL context and progressive scripting search/describe/call;
- live hybrid Gopher + scripting agent behavior;
- MCP 2026-07-28 stdio executable/harness/tool interoperability.

Historical investigation details remain in [FL scripting journey](docs/FL_SCRIPTING_JOURNEY.md).

## Current integration phase

`phase/workspace-foundation` is the experimental integration spine for the accepted scripting and MCP work. The `feat/fl-runtime-shell` experiment adds an operational shell above those primitives without changing the frozen baseline applications or promoting new orchestration into Core.

The immediate architectural direction remains to establish the first app-owned workspace feature slices around:

```text
Entity
Feature
Relation
Binding
Revision
Diff
```

and then derive human UI, agent context/tools and external protocol projections from the same state/capabilities.

See [Workspace foundation phase](docs/agent-work/WORKSPACE_FOUNDATION.md).

## Validation

Static/deterministic repository gate:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The promoted FL scripting integration also has a Windows Python/native build gate. Real FL Studio, Gopher, scripting subinterpreter behavior, Ghost Tap loading and third-party plugin behavior require the proprietary Windows runtime.

See:

- [Windows / FL live validation](docs/WINDOWS_FL_LIVE_VALIDATION.md)
- [FL scripting framework validation](docs/agent-work/FL_SCRIPTING_FRAMEWORK_VALIDATION.md)
- [FL MCP 2026 validation](docs/agent-work/FL_MCP_2026_VALIDATION.md)

## Design history

The technical retrospective, migration plan, ADRs and experiment journeys remain valuable historical evidence. Current implementation work should prefer the baseline/SDK/capability/phase documents above when older planning text conflicts with live-proven architecture.
