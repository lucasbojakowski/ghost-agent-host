# Windows / FL Studio live validation

This is the proprietary-runtime gate for `phase/vertical-slice-reset`. Run it on the Windows machine that has FL Studio/Gopher, Codex, and the required third-party plugins installed.

The expected path is:

```text
Ghost Tap -> capture -> Rust analysis -> Codex App Server thread -> app-selected FL/Gopher calls -> FL Studio
```

## 1. Validate the checkout

From the repository root:

```powershell
git switch phase/vertical-slice-reset
git pull --ff-only
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## 2. Package and install Ghost Tap

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\package_ghost_tap.ps1 -Install
```

The script builds package `ghost-tap` for `x86_64-pc-windows-msvc`, verifies the produced x64 PE and exported `clap_entry`, packages `Ghost Tap.clap`, and installs it to the system CLAP directory unless `-InstallDirectory` is supplied.

If FL Studio is open, close it before the next step so the Gopher WebView is created with the debugging flag and FL can rescan the installed Tap cleanly.

## 3. Launch FL Studio with WebView2 CDP enabled

Close every running FL Studio process first. From PowerShell, set WebView2's additional browser arguments and launch your installed `FL64.exe` from that same shell. Adjust only the executable path if needed:

```powershell
$debugArg = "--remote-debugging-port=9222"
$existing = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
if ([string]::IsNullOrWhiteSpace($existing)) {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $debugArg
} elseif ($existing -notmatch "--remote-debugging-port=") {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "$existing $debugArg"
}

& "D:\Image-Line\FL Studio 2026\FL64.exe"
```

Open Gopher in FL Studio (`Alt+F1`). The adapter defaults to CDP port `9222` and target match `gopher`.

Do not use Gopher's own agent concurrently with Ghost while Ghost is making native calls. The observed callback interface does not provide dependable call correlation, so `ghost-fl-studio` intentionally serializes calls.

## 4. Verify the transparent adapter and live catalog

```powershell
cargo run -p fl-gopher-probe -- catalog
```

This should print the current target metadata plus the complete live Gopher MCP tool catalog. The installed FL build's live catalog/schema is the source of truth.

Choose a non-mutating tool from that catalog and exercise one raw call. For example, only if the live catalog actually exposes `get_tempo` with no required arguments:

```powershell
cargo run -p fl-gopher-probe -- call get_tempo --arguments '{}'
```

Otherwise substitute the exact tool name and arguments from the live schema. This verifies target discovery, schema-driven argument ordering, recursive callback normalization, and raw native-result handling.

## 5. Prepare the mixer target

For the default command below:

1. Route representative source audio through mixer Insert 1.
2. Load **Ghost Tap** on that signal path and let FL activate/process it.
3. Confirm audio passes through unchanged.
4. Position the playhead immediately before real signal and stop transport.
5. Confirm mixer slots 1 through 4 are appropriate for this experiment.
6. Confirm the exact installed names `Pro-Q 4` and `Pro-C 3`, or pass different exact names with repeated `--plugin` flags.

FL Studio remains current truth. Human DAW edits can happen at any time; context/model observations are snapshots and must not become adapter invariants.

## 6. Run the full vertical slice

Ensure the Codex executable is available as `codex`, or replace `--codex-binary` with the explicit executable/`.cmd` shim path.

```powershell
cargo run -p ghost-workflow -- `
  --debug-port 9222 `
  --tap-instance 0 `
  --capture-seconds 4 `
  --track 1 `
  --slot-start 1 `
  --slot-end 4 `
  --plugin "Pro-Q 4" `
  --plugin "Pro-C 3" `
  --processing-intensity 0.70 `
  --model "gpt-5.6-terra" `
  --codex-binary "codex" `
  --i-accept-live-fl-writes
```

Add a task-specific intent when useful, for example:

```powershell
--intent "Tighten the low mids and dynamics while preserving punch."
```

The current app intentionally owns the target track, writable slot range, plugin allowlist, context selection, tool filtering, mutation journal, and the experimental requirement that at least one authorized mutation succeeds. Those choices are not hidden policy inside `ghost-fl-studio`.

## 7. Confirm the live regression

A successful run should show, in order:

- a live FL/Gopher target and live tool count;
- a fresh Ghost Tap instance;
- capture arming and a completed WAV artifact;
- high-resolution Rust analysis and an `.analysis.json` artifact;
- a persistent Codex App Server thread;
- app-selected dynamic calls sourced from the raw live FL catalog;
- at least one successful app-authorized processor mutation;
- the final `GREEN` line for the capture -> analysis -> agent -> DAW flow.

Then inspect FL Studio and confirm the resulting processor state is musically and technically plausible.

For third-party parameters, raw normalized state and human-readable display text are not guaranteed to settle simultaneously. `getParamValueString` may lag or be unavailable; stale or missing display text is not by itself proof that a normalized write failed. Re-read normalized native state when checking writes.

## 8. Common failures

If port `9222` is unavailable, fully close FL Studio and relaunch it from a shell carrying `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`.

If the CDP endpoint exists but no Gopher target is found, open Gopher with `Alt+F1` and retry.

If the workflow reports no fresh Ghost Tap, confirm the plugin is loaded on an active mixer signal path and FL has processed it recently.

If Codex cannot start, run `codex --version` and pass an explicit executable or `.cmd` shim with `--codex-binary`. `ghost-codex` handles Windows executable/shim launching, but the referenced program still has to exist.

If transport succeeds but the inner native FL result reports an error, treat the call as failed. `ghost-fl-studio` intentionally distinguishes native-tool failure from transport failure.

If the agent turn completes without a successful authorized mutation, `ghost-workflow` fails intentionally because that is this executable's current experimental success condition.

## Sign-off

Do not treat static CI as proof of FL/FabFilter interoperability. Record the FL Studio build, Ghost Tap package, Codex runtime/model, Gopher catalog availability, and whether the final mutation/read-back behavior was confirmed on the Windows machine.
