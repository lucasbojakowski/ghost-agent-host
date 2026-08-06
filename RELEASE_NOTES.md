# Ghost Agent Host 0.1.0 — Internal Evaluation Source Release

## Delivered

- Rust workspace for analysis, persistence, Codex integration, hosting abstractions, outer CLAP shell, CLI, daemon, and native laboratory UI.
- Configurable Live, High, Maximum, and custom analysis profiles.
- Text-only prompt and semantic MixPlan contracts.
- SQLite schema and content-addressed artifact store.
- Deterministic mock agent and neutral mock audio chain.
- Six generated sound fixtures, independent reference analyses, human-facing plots, and an audible mock before/after render.
- Internal noncommercial license and unpublished Cargo packages.

## Sandbox result

- 85 checks passed.
- 0 checks failed.

See `docs/VALIDATION_REPORT.md` and `artifacts/sandbox-validation.json`.

## Target-machine gate

This release does not claim completed FabFilter-in-DAW integration. The environment had no Rust toolchain, CLAP DAW, Codex authentication, or FabFilter binaries. The outer transparent CLAP shell and child-host adapter boundaries are present; nested child processing, GUI parenting, parameter mapping, latency/tail forwarding, and opaque state round trips must be completed and tested on a licensed target workstation.
