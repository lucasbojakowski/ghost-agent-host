# FL Scripting Framework Validation

Status for `feat/fl-scripting-framework`.

This document records the accepted baseline for the promoted FL Studio MIDI Scripting framework. Static CI and live FL Studio evidence are kept separate so future refactors have an exact regression target.

## Accepted baseline

```text
Status: PROVEN
Validated code commit: 9dc510cf4ede8ab50d860e8e3d2c1aa4e832d84d
Validation date: 2026-08-25
GitHub Actions run: 32828978193
```

The framework is accepted as a reusable lower-layer FL integration primitive.

The live validation established that:

- `ghost-fl-scripting` connects through the extracted native CPython extension + nonblocking WinSock transport;
- the app receives real live FL project/context state;
- the agent can progressively search the scripting catalog, inspect a scripting function, and invoke it;
- the agent can use Gopher and MIDI Scripting together in one live FL workflow;
- the scripting framework is therefore no longer only a transport probe or app-local experiment.

The user machine remains authoritative for proprietary FL runtime behavior.

## Static / deterministic gate

GitHub Actions run `32828978193` passed at the validated commit.

Rust matrix:

```text
ubuntu-latest   PASS
macos-latest    PASS
windows-latest  PASS
```

Each Rust job passed:

```text
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The dedicated Windows `fl-native` job also passed:

```text
python -m py_compile crates/ghost-fl-scripting/fl-script/device_Ghost.py
crates/ghost-fl-scripting/fl-native/build.ps1 -PythonLauncher python
```

This proves the crate-owned Python source and native extension rebuild in CI in addition to the cross-platform Rust workspace gate.

## Preserved runtime baseline

The extraction preserves the runtime boundary established by the scripting bridge investigation:

```text
FL Studio source baseline: Producer Edition 26.1.3 build 5570
MIDI Scripting API source baseline: 44
embedded Python source baseline: CPython 3.12.1 / cp312 win_amd64
wire protocol: 1
loopback listener default: 127.0.0.1:48766
native transport API: ghost_native API 1
```

The 2026-08-25 framework acceptance confirmed the promoted framework in live FL Studio. The exact FL build/runtime strings were not separately re-recorded in that acceptance report, so the values above remain the preserved source-runtime baseline rather than a claim of a second independent version capture.

The known-good distributable remains:

```text
crates/ghost-fl-scripting/fl-native/ghost_native.cp312-win_amd64.pyd
```

The native module exists for a specific empirical reason: FL's CPython subinterpreter was live-proven unreliable for audited Python socket/file constructors, while the multi-phase subinterpreter-compatible extension can use native nonblocking WinSock safely. Do not replace it with ordinary Python `socket` code without reproducing the complete live regression gate.

## Proven composition

The promoted branch adds two distinct layers:

```text
crates/ghost-fl-scripting
    transparent FL MIDI Scripting primitive adapter

apps/ghost-fl-workspace
    app-owned composition of:
      ghost-fl-studio / Gopher
      + ghost-fl-scripting
      + agent/tool exposure
```

The accepted live result proves that this separation works: the agent can discover scripting primitives when needed and combine them with the higher-level Gopher surface.

This is important architecture evidence. The lower crate owns FL/runtime invariants; the app owns how the two FL surfaces are exposed and composed for an agent.

## Regression gate for future changes

A future refactor of `ghost-fl-scripting` should preserve at minimum:

1. cross-platform Rust fmt/check/test/clippy;
2. Windows Python syntax + native extension rebuild;
3. FL imports the `.pyd` from the scripting subinterpreter;
4. outbound nonblocking loopback connection succeeds;
5. hello/version metadata is received;
6. live FL state can be read;
7. `fl_scripting_search` can discover a real scripting function;
8. `fl_scripting_describe` can expose its evidence/signature;
9. `fl_scripting_call` can invoke it and return the real result;
10. a hybrid task can use both scripting context and Gopher mutation/inspection;
11. reconnect remains bounded and nonblocking;
12. no arbitrary Python execution is introduced.

## Frozen control-group invariant

`ghost-fl-agent` remains the raw Gopher control group. The scripting framework may be used by `ghost-fl-workspace` and later apps, but the frozen raw-agent Codex registry must not silently absorb scripting tools.

Focused tests preserve this invariant; a separate live raw-agent rerun was not part of the 2026-08-25 acceptance report.

## Result record

```text
Date: 2026-08-25
Validated code commit: 9dc510cf4ede8ab50d860e8e3d2c1aa4e832d84d
GitHub Actions run: 32828978193
Rust matrix: PASS
Python controller syntax: PASS
Native crate-owned rebuild: PASS
Live FL context: PASS
Scripting search/describe/call path: PASS
Hybrid Gopher+scripting agent task: PASS
Frozen ghost-fl-agent live rerun: NOT RE-RUN IN THIS ACCEPTANCE
Overall status: PROVEN
```

This commit is the scripting-framework baseline to preserve when integrating later branches.