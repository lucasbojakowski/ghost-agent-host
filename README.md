# Ghost & Guild

Ghost & Guild is an agentic layer for audio workspaces.

```text
capture → analysis → agent → DAW
```

The current reference slice keeps FL Studio authoritative for processors and routing:

- **Ghost Tap** captures a bounded stereo observation from the DAW while remaining a transparent CLAP passthrough.
- **ghost-audio** turns captured audio into deterministic, inspectable evidence.
- **ghost-context** represents and compiles provider-neutral reasoning context.
- **ghost-codex** runs persistent Codex App Server threads and dynamic tools without audio/mixing-domain coupling.
- **ghost-fl-studio** is a transparent, policy-free mirror of the live FL Studio/Gopher interface.
- **ghost-workflow** composes the proven capture → analysis → agent → DAW regression experiment.
- **ghost-fl-agent** is the next research app: one persistent raw agent over the complete live Gopher catalog, with a local browser chat UI and no Ghost Tap/mixer/plugin assumptions.

## Branch experiment: raw FL over MCP 2026

`feat/fl-mcp-2026` preserves the proven Gopher baseline and exports that same live surface through MCP `2026-07-28` for external-provider/harness comparison.

Read these branch documents before implementing that work:

- [FL MCP 2026 baseline experiment](docs/agent-work/FL_MCP_2026_BASELINE.md)
- [FL MCP 2026 implementation prompt](docs/agent-work/FL_MCP_2026_IMPLEMENTATION_PROMPT.md)

The first MCP phase is deliberately a **raw parity server**, not a semantic FL redesign: current official Rust `rmcp` 3.x, stdio transport, dynamic export of the live Gopher manifest, no scripting bridge, no Tasks/MRTR/MCP Apps yet, and no promotion into `ghost-application`.

## Workspace

```text
crates/
  ghost-audio/
  ghost-tap/
  ghost-context/
  ghost-codex/
  ghost-fl-studio/
  ghost-application/

apps/
  ghost-workflow/
  ghost-fl-agent/

tools/
  fl-gopher-probe/
```

Policy starts high in `apps/*` and moves downward only after repeated real workflows demonstrate a reusable requirement.

## Validation

Static and deterministic validation:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

FL Studio, Gopher, Ghost Tap loading, and third-party plugin behavior require the proprietary Windows runtime. See [Windows / FL live validation](docs/WINDOWS_FL_LIVE_VALIDATION.md) for the final regression gate.

## Design sources

- [Technical retrospective](docs/TECHNICAL_RETROSPECTIVE.md)
- [Workspace migration plan](docs/WORKSPACE_MIGRATION_PLAN.md)
- [ADR 001 — transparent FL Studio adapter](docs/decisions/001-transparent-fl-studio-adapter.md)
- [Post-reset idea backlog and scorecard](docs/ideas/README.md)
