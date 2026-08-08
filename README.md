# Ghost Agent Host

Ghost is a Windows-first, UI-agnostic audio-agent host. It composes deterministic feature
extraction, caller-defined context compilation, replaceable agent runtimes, and a vendor-neutral
CLAP child graph. EQ and compression are the first workflow, not hard-coded core concepts.

- [Architecture](docs/ARCHITECTURE.md)
- [Configuration](configuration.md)
- [Testing runbook](testing-runbook.md)
- [Implementation report](report.md)
- [Runtime coherence trajectory](visualizer/runtime-coherence.html)
- [Redesign 02 visual archive](visualizer/redesign-02.html)

Quick local validation (never launches Codex):

```powershell
cargo test --workspace --all-targets
cargo run -p ghost-cli -- analyze --input fixtures/clean_reference.wav
cargo run -p ghost-cli -- plugins
```
