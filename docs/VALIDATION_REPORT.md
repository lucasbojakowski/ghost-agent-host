# Sandbox Validation Report

**Date:** 2026-08-06  
**Scope:** Checks executable without a Rust compiler, DAW, Codex authentication, or proprietary FabFilter binaries.

## Result: 85 passed, 0 failed

Validated categories:

- Cargo TOML syntax and workspace membership.
- Local path dependencies and embedded asset paths.
- SQLite migration execution and required tables.
- JSON Schema correctness and example conformance.
- Text-only PromptBundle policy; no plot/image attachment keys.
- Six deterministic 48 kHz stereo float WAV fixtures.
- Independent fixture behavior for low-mid excess, stereo phase instability, and crest reduction.
- Independent mock EQ/compressor before-after render.
- Rust source placeholder and balanced-delimiter checks.

## Independent mock result

- Low-mid delta: **-3.492 dB**
- Crest-factor delta: **0.055 dB**
- Spectral-centroid delta: **92.8 Hz**
- Stereo-correlation delta: **0.000036**

This verifies the neutral mock evaluation path, not FabFilter equivalence.

## Unavoidable limitations

- The sandbox did not contain rustc/cargo and blocked external toolchain downloads.
- FabFilter binaries and a CLAP-capable DAW were not available.
- The real nested-child process callback, child GUI parenting, parameter manifest, latency, and state round-trip require target-machine integration tests.

## Detailed checks

| Check | Result |
|---|---|
| `toml:Cargo.toml` | PASS |
| `toml:apps/ghost-agentd/Cargo.toml` | PASS |
| `toml:apps/ghost-cli/Cargo.toml` | PASS |
| `toml:apps/ghost-lab/Cargo.toml` | PASS |
| `toml:crates/ghost-clap-plugin/Cargo.toml` | PASS |
| `toml:crates/ghost-codex/Cargo.toml` | PASS |
| `toml:crates/ghost-core/Cargo.toml` | PASS |
| `toml:crates/ghost-db/Cargo.toml` | PASS |
| `toml:crates/ghost-host/Cargo.toml` | PASS |
| `workspace-member:crates/ghost-core` | PASS |
| `workspace-member:crates/ghost-db` | PASS |
| `workspace-member:crates/ghost-codex` | PASS |
| `workspace-member:crates/ghost-host` | PASS |
| `workspace-member:crates/ghost-clap-plugin` | PASS |
| `workspace-member:apps/ghost-cli` | PASS |
| `workspace-member:apps/ghost-agentd` | PASS |
| `workspace-member:apps/ghost-lab` | PASS |
| `sqlite-migration` | PASS |
| `schema:mix_plan.schema.json` | PASS |
| `schema:prompt_bundle.schema.json` | PASS |
| `example-mix-plan` | PASS |
| `prompt-bundle-text-only` | PASS |
| `fixture:clean_reference.wav` | PASS |
| `fixture:muddy_bass.wav` | PASS |
| `fixture:harsh_presence.wav` | PASS |
| `fixture:phasey_wide.wav` | PASS |
| `fixture:crushed_drums.wav` | PASS |
| `fixture:silence_then_signal.wav` | PASS |
| `fixture-behavior:muddy-low-mid` | PASS |
| `fixture-behavior:phase-correlation` | PASS |
| `fixture-behavior:crushed-crest` | PASS |
| `mock-evaluation:low_mid_decreased` | PASS |
| `mock-evaluation:no_peak_over_plus_2_dBFS` | PASS |
| `mock-evaluation:stereo_correlation_preserved` | PASS |
| `analysis-config` | PASS |
| `path-dependency:apps/ghost-agentd/Cargo.toml:ghost-codex` | PASS |
| `path-dependency:apps/ghost-agentd/Cargo.toml:ghost-core` | PASS |
| `path-dependency:apps/ghost-agentd/Cargo.toml:ghost-db` | PASS |
| `path-dependency:apps/ghost-agentd/Cargo.toml:ghost-host` | PASS |
| `path-dependency:apps/ghost-cli/Cargo.toml:ghost-codex` | PASS |
| `path-dependency:apps/ghost-cli/Cargo.toml:ghost-core` | PASS |
| `path-dependency:apps/ghost-cli/Cargo.toml:ghost-db` | PASS |
| `path-dependency:apps/ghost-cli/Cargo.toml:ghost-host` | PASS |
| `path-dependency:apps/ghost-lab/Cargo.toml:ghost-codex` | PASS |
| `path-dependency:apps/ghost-lab/Cargo.toml:ghost-core` | PASS |
| `path-dependency:apps/ghost-lab/Cargo.toml:ghost-host` | PASS |
| `path-dependency:crates/ghost-codex/Cargo.toml:ghost-core` | PASS |
| `path-dependency:crates/ghost-db/Cargo.toml:ghost-core` | PASS |
| `path-dependency:crates/ghost-host/Cargo.toml:ghost-core` | PASS |
| `include-str:apps/ghost-lab/src/main.rs:../../../prompts/system.md` | PASS |
| `include-str:crates/ghost-db/src/lib.rs:../../../migrations/0001_init.sql` | PASS |
| `source-hygiene:apps/ghost-agentd/src/main.rs` | PASS |
| `source-delimiters:apps/ghost-agentd/src/main.rs` | PASS |
| `source-hygiene:apps/ghost-cli/src/main.rs` | PASS |
| `source-delimiters:apps/ghost-cli/src/main.rs` | PASS |
| `source-hygiene:apps/ghost-lab/src/main.rs` | PASS |
| `source-delimiters:apps/ghost-lab/src/main.rs` | PASS |
| `source-hygiene:crates/ghost-clap-plugin/src/lib.rs` | PASS |
| `source-delimiters:crates/ghost-clap-plugin/src/lib.rs` | PASS |
| `source-hygiene:crates/ghost-codex/src/lib.rs` | PASS |
| `source-delimiters:crates/ghost-codex/src/lib.rs` | PASS |
| `source-hygiene:crates/ghost-core/src/analysis.rs` | PASS |
| `source-delimiters:crates/ghost-core/src/analysis.rs` | PASS |
| `source-hygiene:crates/ghost-core/src/audio.rs` | PASS |
| `source-delimiters:crates/ghost-core/src/audio.rs` | PASS |
| `source-hygiene:crates/ghost-core/src/capture.rs` | PASS |
| `source-delimiters:crates/ghost-core/src/capture.rs` | PASS |
| `source-hygiene:crates/ghost-core/src/lib.rs` | PASS |
| `source-delimiters:crates/ghost-core/src/lib.rs` | PASS |
| `source-hygiene:crates/ghost-core/src/mock_dsp.rs` | PASS |
| `source-delimiters:crates/ghost-core/src/mock_dsp.rs` | PASS |
| `source-hygiene:crates/ghost-core/src/model.rs` | PASS |
| `source-delimiters:crates/ghost-core/src/model.rs` | PASS |
| `source-hygiene:crates/ghost-core/src/prompt.rs` | PASS |
| `source-delimiters:crates/ghost-core/src/prompt.rs` | PASS |
| `source-hygiene:crates/ghost-core/src/validation.rs` | PASS |
| `source-delimiters:crates/ghost-core/src/validation.rs` | PASS |
| `source-hygiene:crates/ghost-core/tests/audio_roundtrip.rs` | PASS |
| `source-delimiters:crates/ghost-core/tests/audio_roundtrip.rs` | PASS |
| `source-hygiene:crates/ghost-core/tests/fixture_analysis.rs` | PASS |
| `source-delimiters:crates/ghost-core/tests/fixture_analysis.rs` | PASS |
| `source-hygiene:crates/ghost-db/src/lib.rs` | PASS |
| `source-delimiters:crates/ghost-db/src/lib.rs` | PASS |
| `source-hygiene:crates/ghost-host/src/lib.rs` | PASS |
| `source-delimiters:crates/ghost-host/src/lib.rs` | PASS |
