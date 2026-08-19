# FL Scripting Framework Validation

Status for `feat/fl-scripting-framework`.

This file separates what repository CI can establish from what still requires the real FL Studio runtime. Do not treat static success as a substitute for the native/live gate.

## Current validation state

At implementation handoff:

```text
Rust / cross-platform CI: PENDING
Python controller syntax CI: PENDING
crate-owned native rebuild CI: PENDING
real FL Studio extracted-adapter regression: PENDING
real FL Studio combined-workspace validation: PENDING
```

The implementation sandbox does not contain Rust tooling. A draft validation PR was created against the live-proven scripting-bridge branch so GitHub can run the repository workflow, but the available GitHub connector had not surfaced a workflow run at the time this record was written. Therefore this branch does **not** claim a static green gate yet.

## Static / deterministic gate

The branch must pass the repository CI workflow:

```text
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

on Linux, macOS and Windows, plus the Windows Python/native checks:

```text
python -m py_compile crates/ghost-fl-scripting/fl-script/device_Ghost.py
crates/ghost-fl-scripting/fl-native/build.ps1 -PythonLauncher python
```

Focused Rust tests cover the reusable scripting catalog/protocol boundary, evidence-backed module exposure, bounded framing, the combined app's progressive-disclosure definitions/search behavior, and the frozen raw-agent registry invariant.

## Preserved empirical baseline

The extraction deliberately preserves the behavior previously proven on the source experiment:

```text
FL Studio Producer Edition 26.1.3 build 5570
MIDI Scripting API 44
CPython 3.12.1 / cp312 win_amd64
protocol 1
loopback listener 127.0.0.1:48766
native transport ghost_native API 1
```

The known-good tracked distributable binary remains:

```text
crates/ghost-fl-scripting/fl-native/ghost_native.cp312-win_amd64.pyd
```

Its Git blob is intentionally preserved unchanged during framework extraction.

The source experiment's tracked setuptools build evidence is also retained under:

```text
crates/ghost-fl-scripting/fl-native/build/
```

Those files are generated intermediates, **not** the preferred distributable. They remain tracked only because the execution prompt requires native build evidence to be preserved until the new crate-owned build actually succeeds. After the Windows native CI gate and live runtime gate pass, a later cleanup may remove those intermediates while retaining the source, build script and validated distributable `.pyd`.

The prior experiment established that the CPython extension can create/use nonblocking WinSock from FL's subinterpreter, that the Python script can perform bounded `OnIdle` NDJSON dispatch, and that Rust can correlate requests/results and complete a reversible mixer-selection change/verify/restore sequence.

Those facts are the regression baseline for the extracted crate. They are not evidence that the new `ghost-fl-workspace` composition has run successfully yet.

## Required live framework gate — pending human/later agent

Run from a disposable FL project on the Windows validation host after static CI is green.

### A. Extracted scripting regression

1. Install `crates/ghost-fl-scripting/fl-script/device_Ghost.py` with its installer.
2. Install the preserved known-good `.pyd` into FL's shared Python library.
3. Start `ghost-fl-agent` with `--i-accept-live-fl-writes` and confirm the extracted adapter reports the same hello/version metadata as the source bridge.
4. Run the existing scripting developer probe.
5. Verify current-state reads plus the temporary mixer selection change/readback/restore/readback sequence.
6. Restart/reload the FL controller script and verify reconnect behavior.

### B. Combined workspace hybrid task

Start:

```powershell
cargo run -p ghost-fl-workspace -- --i-accept-live-fl-writes
```

Then ask for a task that requires both surfaces, for example:

- observe the currently selected mixer/channel context through MIDI Scripting;
- use the complete raw Gopher surface to perform a disposable mutation whose target depends on that context;
- re-observe/verify the resulting state through MIDI Scripting.

Capture the dynamic-tool trace and final verification evidence. The task is a pass only if the agent actually uses both surfaces and the final live state matches the requested result.

### C. Scripting-only gateway task

Ask the agent to:

1. search for a known MIDI Scripting primitive using `fl_scripting_search`;
2. inspect the exact function evidence using `fl_scripting_describe`;
3. invoke it with `fl_scripting_call`;
4. verify the observable result.

A read-only query such as current pattern name is appropriate for a first pass. A reversible mutation can follow once the read path is green.

### D. Frozen baseline check

Run `ghost-fl-agent` separately and confirm its Codex dynamic tool definitions remain exactly the live Gopher catalog. FL scripting must still appear only in its developer diagnostic path, never in its Codex registry.

## Result record

Fill this section only after the live/static gates are actually run.

```text
Date:
Static CI run / commit:
Rust matrix: PASS / FAIL
Python syntax: PASS / FAIL
Native crate-owned rebuild: PASS / FAIL
FL Studio version/build:
MIDI Scripting API version:
Python runtime:
Native .pyd hash/artifact:
Extracted scripting regression: PASS / FAIL
Hybrid Gopher+scripting task: PASS / FAIL
Scripting-only gateway task: PASS / FAIL
Frozen ghost-fl-agent baseline: PASS / FAIL
Notes / captured evidence:
```

Until that record is completed, branch status is **framework implementation complete in source, with static CI and live framework validation still pending**.
