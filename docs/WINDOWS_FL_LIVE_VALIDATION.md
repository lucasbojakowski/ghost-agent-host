# Windows / FL Studio live validation

This is the proprietary-runtime gate for the vertical slice. Run it on the Windows machine that has FL Studio/Gopher, Codex, and the required third-party plugins installed.

## 1. Validate the Rust workspace

From the repository root on `phase/vertical-slice-reset`:

```powershell
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## 2. Package and install Ghost Tap

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\package_ghost_tap.ps1 -Install
```

The script builds package `ghost-tap` for `x86_64-pc-windows-msvc`, verifies the produced PE/CLAP entry point, packages `Ghost Tap.clap`, and installs it to the system CLAP directory unless `-InstallDirectory` is supplied.

In FL Studio, rescan plugins if required and load **Ghost Tap** on the mixer signal you want the workflow to observe. Confirm audio passes through unchanged.

## 3. Start the FL/Gopher debug target

Start FL Studio with the Gopher/WebView2 remote-debugging setup used by the proven baseline. The Rust adapter defaults to CDP port `9222` and target match `gopher`.

Verify discovery and inspect the live catalog:

```powershell
cargo run -p fl-gopher-probe -- catalog
```

This should print the current target plus the live Gopher tool catalog. Do not continue if discovery fails.

## 4. Exercise one raw read through the transparent adapter

Choose a non-mutating tool name from the live catalog and invoke it through the probe. For example, if the catalog exposes `get_tempo` with no required arguments:

```powershell
cargo run -p fl-gopher-probe -- call get_tempo --arguments '{}'
```

Use the live catalog/schema as the source of truth if the tool name or arguments differ. This gate confirms that catalog discovery, schema-driven argument ordering, callback normalization, and native result handling remain live.

## 5. Run the vertical slice

Position the playhead so Ghost Tap receives representative audio, ensure the intended mixer slots are safe for this test, and run:

```powershell
cargo run -p ghost-workflow -- --track 1 --slot-start 1 --slot-end 4 --plugin "Pro-Q 4" --plugin "Pro-C 3" --i-accept-live-fl-writes
```

Optional overrides include `--tap-instance`, `--capture-seconds`, `--intent`, `--processing-intensity`, `--model`, `--codex-binary`, `--debug-port`, and `--target-match`.

Confirm the conceptual path:

```text
Ghost Tap
  → capture
  → Rust analysis
  → Codex App Server thread
  → app-selected FL tool calls
  → FL Studio
```

## 6. Live regression checks

Confirm all of the following on the real runtime:

- Ghost Tap remains a transparent stereo passthrough and produces a bounded capture without realtime-thread filesystem/network work.
- The analysis bundle is produced from the captured audio.
- One persistent Codex App Server process/thread can run the turn and execute its app-selected dynamic tools.
- `ghost-fl-studio` discovers the live Gopher catalog and raw calls still work.
- Effect insertion and plugin parameter reads/writes work for the plugins actually installed on the machine.
- Native tool failures remain distinguishable from transport failures.
- The workflow respects its app-local track/slot/plugin write policy without relying on hidden adapter policy.
- Manual DAW edits made between observations are treated as current FL state; stale model/context snapshots do not become adapter invariants.

Do not treat static CI as proof of FL/FabFilter interoperability. This live gate is intentionally the final authority for proprietary runtime behavior.
