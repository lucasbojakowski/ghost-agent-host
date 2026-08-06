# Ghost Agent Host

Rust-first, internal noncommercial validation application for an agent-controlled CLAP child host targeting FabFilter Pro-Q 4 and Pro-C 3.

## Included

- Configurable high-resolution Rust audio analyzer.
- Text-only prompt bundle compiler.
- Strict semantic `MixPlan` contract and validator.
- Codex App Server JSON-RPC adapter.
- Deterministic mock mixing agent for offline testing.
- Deterministic mock EQ/compressor renderer for full pipeline evaluation.
- SQLite persistence and content-addressed artifacts.
- DAW-loadable transparent outer CLAP shell.
- Feature-gated CLAP descriptor scanner boundary.
- Native Rust validation UI (`ghost-lab`).
- CLI for analysis, schemas, database inspection, and end-to-end demos.
- Synthetic sound fixtures, independent Python analysis, audible mock before/after render, human-facing plots, and validation scripts.
- Local JSONL agent daemon and request client.
- Custom TOML analysis profiles and CLAP packaging helper.

## Repository status

The complete text-analysis-agent-persistence-mock-processing loop is implemented. The outer CLAP shell is implemented as a transparent plugin. The real nested Pro-Q 4/Pro-C 3 process path, parameter adapter, opaque state round-trip, latency forwarding, and child native GUI parenting require a target machine with licensed FabFilter binaries and a CLAP-capable DAW; those operations cannot be tested or completed faithfully in this sandbox.

See [docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md) for the exact boundary.

## Quick validation

```bash
python3 scripts/generate_fixtures.py
python3 scripts/reference_analysis.py
python3 scripts/build_examples.py
python3 scripts/mock_evaluate.py
python3 scripts/validate_artifacts.py
```

With Rust installed:

```bash
cargo test --workspace
cargo run -p ghost-cli -- analyze \
  --input fixtures/muddy_bass.wav \
  --analysis-config config/default.toml \
  --output artifacts/muddy-analysis.json

cargo run -p ghost-cli -- demo \
  --fixture fixtures/muddy_bass.wav \
  --intent "Tighten the low mids while preserving punch"

cargo run -p ghost-lab
```

## Design invariant

> FabFilter produces the target sound. Rust measures evidence. Codex proposes semantic intent. Ghost validates, applies, measures, and persists the result.

## License

Internal noncommercial evaluation only. See `LICENSE`. All Cargo packages are marked `publish = false`.
