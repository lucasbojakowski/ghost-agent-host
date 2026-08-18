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
- **ghost-fl-agent** is the frozen raw-Gopher behavioral baseline and also hosts the live-proven FL scripting transport/probe on its source branch.

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
  analyse-full/
  fl-gopher-probe/
```

Policy starts high in `apps/*` and moves downward only after repeated real workflows demonstrate a reusable requirement.

## Branch plan: promote FL scripting

`feat/fl-scripting-framework` starts from the live-proven `feat/fl-scripting-bridge` experiment and promotes only the reusable FL-specific scripting boundary.

Target lower layer:

```text
crates/ghost-fl-scripting/
```

Target combined research app:

```text
apps/ghost-fl-workspace/
```

The intended composition is:

```text
                         FL Studio
                      /             \
                 Gopher/CDP      MIDI Scripting
                     │                │
                     ▼                ▼
           ghost-fl-studio    ghost-fl-scripting
                     \                /
                      \              /
                       ▼            ▼
                    ghost-fl-workspace
```

`ghost-fl-studio` remains the frozen transparent Gopher adapter. `ghost-fl-scripting` should become the independent transparent scripting adapter. Product policy and semantic tooling remain above both.

Read before implementing this branch:

- [FL scripting live journey](docs/FL_SCRIPTING_JOURNEY.md)
- [FL scripting framework architecture](docs/agent-work/FL_SCRIPTING_FRAMEWORK.md)
- [FL scripting framework implementation prompt](docs/agent-work/FL_SCRIPTING_FRAMEWORK_IMPLEMENTATION_PROMPT.md)

## Validation

Static and deterministic validation:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

FL Studio, Gopher, Ghost Tap loading, scripting subinterpreter/native-extension behavior, and third-party plugin behavior require the proprietary Windows runtime. See [Windows / FL live validation](docs/WINDOWS_FL_LIVE_VALIDATION.md) for the general regression gate and the branch-specific scripting documents for the scripting gate.

## Design sources

- [Technical retrospective](docs/TECHNICAL_RETROSPECTIVE.md)
- [Workspace migration plan](docs/WORKSPACE_MIGRATION_PLAN.md)
- [ADR 001 — transparent FL Studio adapter](docs/decisions/001-transparent-fl-studio-adapter.md)
- [Post-reset idea backlog and scorecard](docs/ideas/README.md)
