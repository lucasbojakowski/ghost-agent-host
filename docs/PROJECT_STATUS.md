# Project Status and Validation Boundary

## Implemented in source

- Cargo workspace and CI configuration.
- WAV I/O.
- Live, High, Maximum, and custom analysis configuration model.
- Multi-resolution STFT analysis.
- Standards-oriented EBU R128 integration through the `ebur128` crate.
- Spectral bands, centroid, rolloff, flatness, flux, tilt, and resonance candidates.
- Dynamics, transient, stereo, phase, integrity, and quality flags.
- Text-only prompt bundle.
- Technical mixing system prompt and context modules.
- Semantic Pro-Q 4 / Pro-C 3 mix plan model.
- Range and capability-independent safety validation.
- Mock mixing agent.
- Codex App Server JSON-RPC client using stdio JSONL and `outputSchema`.
- Mock bell-EQ and compressor renderer.
- SQLite schema, migration, repositories, and content-addressed artifacts.
- CLI, local JSONL daemon, and native Rust laboratory UI.
- Transparent outer CLAP plugin shell.
- Optional CLAP binary descriptor scanner.
- Fixtures, independent numerical reference evaluator, audible mock before/after render, and human-facing plots.

## Validated in this sandbox

- Workspace TOML parsing.
- SQLite migration execution.
- JSON Schema validity.
- Example plan validation.
- Prompt bundle exclusion of plots/images.
- Fixture generation and format properties.
- Expected fixture behavior for low-mid excess, phase instability, and crest reduction.
- Source hygiene and balanced-delimiter checks for all Rust source files.
- Cargo path dependencies and `include_str!` targets.
- Independent mock-processing expectations.

## Not validated in this sandbox

The environment contains no Rust compiler and blocks external toolchain installation. It also has no DAW or FabFilter installation. Therefore the following require target-machine work:

- `cargo build`, `cargo test`, Clippy, and rustfmt.
- Loading the outer `.clap` in a DAW.
- Loading Pro-Q 4 and Pro-C 3 as nested children.
- Realtime child processing.
- Public parameter enumeration and semantic adapter mapping.
- Child native editor windows.
- Latency and tail propagation.
- Opaque child state save/load.
- FabFilter license and activation behavior inside a nested host.
- Codex authentication and live model output.

The source isolates these gaps rather than simulating successful proprietary integration.
