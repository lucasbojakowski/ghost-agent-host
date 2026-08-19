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
- **ghost-fl-scripting** is the independent transparent FL MIDI Scripting adapter promoted from the live-proven scripting bridge experiment.
- **ghost-workflow** composes the proven capture → analysis → agent → DAW regression experiment.
- **ghost-fl-agent** remains the frozen raw-Gopher Codex baseline; its scripting surface is developer diagnostics only.
- **ghost-fl-workspace** is the empirical combined Gopher + MIDI Scripting agent harness for this framework branch.

## Workspace

```text
crates/
  ghost-audio/
  ghost-tap/
  ghost-context/
  ghost-codex/
  ghost-fl-studio/
  ghost-fl-scripting/
  ghost-application/

apps/
  ghost-workflow/
  ghost-fl-agent/
  ghost-fl-workspace/

tools/
  analyse-full/
  fl-gopher-probe/
```

Policy starts high in `apps/*` and moves downward only after repeated real workflows demonstrate a reusable requirement.

## FL scripting framework

`feat/fl-scripting-framework` promotes only the reusable FL-specific scripting boundary from the live-proven `feat/fl-scripting-bridge` experiment.

The resulting composition is:

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

`ghost-fl-studio` remains the transparent Gopher adapter. `ghost-fl-scripting` owns only the scripting transport/protocol/catalog boundary. `ghost-fl-workspace` composes the complete live Gopher catalog with exactly three progressive-disclosure scripting gateways: search, describe and call.

The frozen `ghost-fl-agent` Codex registry is intentionally unchanged: it still exposes the complete live Gopher catalog and no scripting tools. Its existing scripting status/probe endpoints consume `ghost-fl-scripting` only as a developer regression path.

The scripting runtime preserves the live-proven FL Studio 26.1.3 / MIDI Scripting API 44 topology:

```text
Rust listener
  ↕ bounded versioned NDJSON over loopback TCP
FL controller script
  ↕
native CPython 3.12 multi-phase extension
  ↕ nonblocking WinSock
Windows loopback
```

The checked-in FL scripting metadata under `docs/daw-apis/fl-studio/` is the capability evidence. Fully documented, explicitly imported modules may be called through the generic adapter when their argument/return shapes are JSON-compatible. Runtime-inspected or signature-incomplete functions remain discoverable but are not guessed into callable behavior.

Read:

- [FL scripting live journey](docs/FL_SCRIPTING_JOURNEY.md)
- [FL scripting framework architecture](docs/agent-work/FL_SCRIPTING_FRAMEWORK.md)
- [FL scripting framework implementation prompt](docs/agent-work/FL_SCRIPTING_FRAMEWORK_IMPLEMENTATION_PROMPT.md)
- [FL scripting framework validation](docs/agent-work/FL_SCRIPTING_FRAMEWORK_VALIDATION.md)

## Validation

Static and deterministic validation:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The Windows CI gate also compiles the promoted FL controller script and rebuilds the CPython 3.12 native extension from `crates/ghost-fl-scripting/fl-native`.

FL Studio, Gopher, Ghost Tap loading, scripting subinterpreter/native-extension behavior, and third-party plugin behavior require the proprietary Windows runtime. See [Windows / FL live validation](docs/WINDOWS_FL_LIVE_VALIDATION.md) for the general regression gate and [FL scripting framework validation](docs/agent-work/FL_SCRIPTING_FRAMEWORK_VALIDATION.md) for the branch-specific extracted-adapter and combined-workspace gates.

## Design sources

- [Technical retrospective](docs/TECHNICAL_RETROSPECTIVE.md)
- [Workspace migration plan](docs/WORKSPACE_MIGRATION_PLAN.md)
- [ADR 001 — transparent FL Studio adapter](docs/decisions/001-transparent-fl-studio-adapter.md)
- [Post-reset idea backlog and scorecard](docs/ideas/README.md)
