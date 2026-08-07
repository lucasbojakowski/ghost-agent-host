# Installation and Build Guide

## 1. Requirements

### All platforms

- Rust stable toolchain with Cargo, rustfmt, and Clippy.
- Python 3.11+ for fixture and independent reference validation.
- A C/C++ build toolchain for bundled SQLite dependencies.
- A CLAP-capable DAW for plugin integration tests.
- User-installed, licensed CLAP versions of FabFilter Pro-Q 4 and Pro-C 3 for the real child-host milestone.
- Codex CLI installed and authenticated for live agent operation.

### Linux development packages

The exact package names vary by distribution. Native `eframe` builds commonly need X11/Wayland, OpenGL, font, and audio development packages. The headless CLI and core libraries need fewer system packages.

## 2. Prepare the repository

```bash
rustup show
cargo --version
python3 --version
python3 scripts/generate_fixtures.py
python3 scripts/reference_analysis.py
python3 scripts/build_examples.py
python3 scripts/mock_evaluate.py
python3 scripts/validate_artifacts.py
```

Install Python validation packages where missing:

```bash
python3 -m pip install numpy scipy soundfile matplotlib jsonschema pyyaml
```

## 3. Build and test

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

The outer plugin dynamic library is produced by `ghost-clap-plugin`. Packaging it as a platform-native `.clap` bundle requires the normal CLAP bundle directory structure and platform-specific binary placement. Use a bundle helper or copy the release library into the correct bundle layout after confirming the generated library name.

## 4. Run the standalone laboratory

```bash
cargo run -p ghost-lab --release
```

The laboratory defaults to `fixtures/muddy_bass.wav`. It runs Maximum analysis, compiles a text-only prompt bundle, generates a deterministic mock plan, validates it, renders the mock chain, and reports before/after metrics. Its egui surface is provided by the shared `ghost-ui` crate. The standalone uses `eframe`; the Windows CLAP editor uses `egui-baseview` so the DAW owns a native child window with plugin-safe hide/show and teardown behavior.

## 5. Run the CLI

Analyze a file:

```bash
cargo run -p ghost-cli --release -- analyze \
  --input fixtures/clean_reference.wav \
  --analysis-config config/default.toml \
  --output artifacts/clean-analysis.json
```

Run the complete mock validation flow:

```bash
cargo run -p ghost-cli --release -- demo \
  --fixture fixtures/muddy_bass.wav \
  --intent "Tighten the low mids while preserving punch" \
  --agent mock
```

Run with Codex App Server:

```bash
cargo run -p ghost-cli --release -- demo \
  --fixture fixtures/muddy_bass.wav \
  --intent "Tighten the low mids while preserving punch" \
  --agent codex \
  --codex-binary codex \
  --model gpt-5.6-terra
```

The Codex path sends only text and JSON. Human-facing plots are exported separately and are not attached to the model turn.

On Windows, the default `--codex-binary codex` searches `PATH` and prefers a native `codex.exe` over PowerShell, batch, or extensionless command shims. Pass an absolute path with `--codex-binary` to pin a specific Codex installation.

## 6. SQLite data

Default paths:

```text
.ghost/ghost.db
.ghost/artifacts/
```

Inspect counts:

```bash
cargo run -p ghost-cli -- db-stats
```

For internal validation, audio retention is enabled in `config/default.toml`. Change the artifact policy directly when a different experimental setup is required.

## 7. Real FabFilter integration sequence

1. Build the `clack-runtime` scanner feature.
2. Scan the user-selected Pro-Q 4 and Pro-C 3 `.clap` bundles.
3. Record descriptor IDs, versions, binary hashes, public parameters, and supported extensions.
4. Implement one child slot in `ghost-host` using Clack lifecycle types.
5. Forward stereo f32 audio and transport.
6. Aggregate latency and tail.
7. Save and restore opaque state.
8. Add separate child GUI windows.
9. Validate semantic parameter mappings against audible/state round trips.
10. Connect the child chain to the outer CLAP processor.

See `docs/FABFILTER_INTEGRATION.md` for the verification matrix.

## 8. Run the local daemon

```bash
cargo run -p ghost-agentd --release -- --agent mock
python3 scripts/send_agentd_request.py '{"method":"health"}'
python3 scripts/send_agentd_request.py '{"method":"propose","path":"fixtures/muddy_bass.wav","intent":{"mode":"freeform","prompt":"Tighten the low mids while preserving punch."}}'
```

Use `--agent codex --model gpt-5.6-terra` after Codex CLI authentication. The daemon communicates with Codex App Server over stdio JSONL and persists the resulting text-only request/plan history in SQLite.

## 9. Package the outer CLAP shell

### Windows 11 x64

Build, validate, and package the plugin from PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package_clap.ps1
```

This builds the explicit `x86_64-pc-windows-msvc` target and verifies both the PE architecture and required `clap_entry` export. The install-ready plugin and its distribution archive are written to:

```text
dist/windows-x86_64/Ghost Agent Host.clap
dist/windows-x86_64/ghost-agent-host-0.1.0-windows-x86_64.zip
```

To install directly into the system CLAP directory, run an elevated PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package_clap.ps1 -Install
```

The default Windows system location is `C:\Program Files\Common Files\CLAP`. After installing, open FL Studio's Plugin Manager, enable **Verify plugins** and **Rescan previously verified plugins**, then select **Find installed plugins**. Do not install the plugin inside the FL Studio application directory.

On macOS or Linux, first build the release library and pass it to the cross-platform helper:

```bash
python3 scripts/package_clap.py target/release/<ghost-clap-library>
```

The current outer shell is transparent and exposes the `ghost-lab` egui surface through an embedded Win32 `clap.gui` editor. The editor still runs the standalone file-based mock workflow; daemon-backed capture, Codex, and nested FabFilter state are later integration milestones described in `FABFILTER_INTEGRATION.md`.

Close FL Studio before replacing an installed development build, because the loaded `.clap` file may be locked. After reinstalling, restart FL Studio and rescan with **Verify plugins** and **Rescan previously verified plugins** enabled.

## 10. Custom analysis

See `CONFIGURATION.md`. `--analysis-config config/default.toml` overrides the built-in profile and persists the run as `custom`.
