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
- **ghost-workflow** composes the current experiment: capture policy, context selection, Codex tool exposure, FL write scope, and verification choices.

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
