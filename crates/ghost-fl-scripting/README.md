# ghost-fl-scripting

Transparent FL Studio MIDI Scripting adapter for Ghost & Guild.

This crate owns the live-proven scripting transport boundary independently from the Gopher/CDP adapter in `ghost-fl-studio`. It deliberately exposes FL Studio MIDI Scripting primitives rather than Ghost workflow policy.

## Runtime topology

```text
Rust `FlScriptingAdapter`
  loopback TCP listener (default 127.0.0.1:48766)
        ^
        | bounded versioned NDJSON
        |
FL Studio MIDI script `fl-script/device_Ghost.py`
        |
        v
`fl-native/ghost_native.cp312-win_amd64.pyd`
  CPython 3.12 multi-phase extension
  native nonblocking WinSock only
```

The native extension exists because FL Studio 26.1.3's embedded CPython 3.12.1 runtime was live-proven to reject ordinary audited Python socket/file construction. The extension owns only OS transport. Python owns FL callbacks and explicit module/function dispatch. Rust owns listening, hello/version validation, request IDs, correlation, timeouts, reconnect state, bounded frames, and metadata-backed calls.

The tracked `ghost_native.cp312-win_amd64.pyd` is the known-good live artifact from FL Studio 26.1.3 / MIDI Scripting API 44. Keep it until a replacement has passed the same live runtime gate.

## Rust API

The primitive surface is intentionally small:

```rust
let scripting = FlScriptingAdapter::start(FlScriptingConfig::default())?;
let status = scripting.status();
let result = scripting.call("patterns", "getPatternName", vec![serde_json::json!(1)])?;
let manifest = scripting.manifest();
```

`FlScriptingCatalog::bundled()` parses the checked-in runtime-enriched FL scripting artifact under `docs/daw-apis/fl-studio/`. The catalog preserves signatures, descriptions, API-version evidence, overloads, and explicit unsupported wire shapes instead of generating hundreds of Rust wrappers.

## Install the FL controller script

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\crates\ghost-fl-scripting\fl-script\install.ps1
```

The script metadata remains:

```python
# name=Ghost Bridge
# supportedDevices=Ghost Midi
```

The current bootstrap uses the external virtual MIDI endpoint `Ghost Midi` only so FL auto-loads the controller script. MIDI is not the data plane.

## Build the native transport

Use 64-bit Python 3.12 and MSVC build tools:

```powershell
powershell -ExecutionPolicy Bypass -File .\crates\ghost-fl-scripting\fl-native\build.ps1
```

The build should produce `ghost_native.cp312-win_amd64.pyd` beside the source. Close FL Studio before replacing a loaded `.pyd`, then restart FL and repeat the scripting runtime gate.

## Boundaries

This crate does not depend on `ghost-codex`, `ghost-context`, `ghost-application`, `ghost-audio`, or `ghost-fl-studio`. It does not contain agent tools, semantic DAW concepts, skill/intent policy, or arbitrary Python evaluation/import behavior.

The source experiment's reversible mixer-selection probe intentionally stays in `apps/ghost-fl-agent`; combined Gopher+scripting agent composition belongs in `apps/ghost-fl-workspace`.
