# FL Studio scripting journey

Date: 2026-08-16  
Branch: `feat/fl-scripting-bridge`

This document records the path from the original FL Studio MIDI Scripting bridge design to the first live-proven Ghost ↔ FL scripting control loop.

It is intentionally a journey/baseline document rather than a replacement for the architecture prompt in `docs/agent-work/FL_SCRIPTING_BRIDGE.md`.

- `docs/agent-work/FL_SCRIPTING_BRIDGE.md` remains the original architecture and scope document.
- `docs/agent-work/FL_SCRIPTING_BRIDGE_FINDINGS.md` records the deterministic implementation state before live Windows validation.
- this document records what the proprietary FL runtime actually did, what failed, what worked, and the baseline we should preserve before the next Rust-owned native transport attempt.

## Result

The experiment succeeded.

The live-proven path is:

```text
Ghost Midi
  │
  │ virtual MIDI bootstrap/autoload only
  ▼
device_Ghost.py inside FL Studio
  │
  │ import/call
  ▼
ghost_native.cp312-win_amd64.pyd
  │
  │ native nonblocking WinSock
  ▼
127.0.0.1:48766
  │
  ▼
apps/ghost-fl-agent (Rust)
  │
  ├── request IDs / correlation / timeouts
  ├── NDJSON protocol v1
  ├── scripting probe
  └── existing frozen Gopher path remains separate
```

The final live probe proved both current-state reads and one reversible mutation with exact restoration.

## Starting architecture

The intended architecture was already correct at the application level:

```text
apps/ghost-fl-agent (Rust)
    │
    └── loopback TCP server
             ▲
             │ outbound nonblocking connection
             │
      device_Ghost.py inside FL
             ▲
             │
      Ghost Midi
      bootstrap only
```

The important constraints were preserved throughout the investigation:

- Rust listens; FL connects outbound.
- virtual MIDI is only for script binding/autoload, never the RPC payload plane.
- protocol is small, versioned NDJSON.
- `OnIdle()` work is bounded and nonblocking.
- Python remains a thin FL API dispatcher, not an agent/orchestration layer.
- the FL scripting module/function surface is explicitly allowlisted.
- no `eval`, `exec`, arbitrary Python execution, or generic DAW abstraction.
- the scripting bridge remains app-local.
- the frozen Gopher adapter and Codex tool behavior are not modified by this experiment.

What turned out to be wrong was only the assumption that FL's embedded Python could safely construct a normal Python socket.

## Runtime discovery

Live testing established the relevant FL runtime as:

```text
FL Studio: Producer Edition v26.1.3 [build 5570]
MIDI scripting API: 44
Python: CPython 3.12.1
ABI: cp312, win_amd64
execution model: CPython subinterpreter
```

The subinterpreter fact was established directly by runtime behavior: `_ctypes` refused to load because it does not support loading in subinterpreters in this environment.

The more important discovery was that the Python audit path inside this FL scripting interpreter is broken globally. A direct `sys.audit(...)` call failed even for arbitrary custom audit event names with:

```text
SystemError('error return without exception set')
```

That explained several otherwise unrelated failures.

## Failed paths, concisely

### Python `socket` / `_socket`

The original script used Python's socket implementation. Socket construction failed before any connection attempt:

```text
SystemError("<class '_socket.socket'> returned NULL without setting an exception")
```

Importing `_socket` worked and the extension was the expected CPython 3.12 x64 binary, but its constructor still failed.

Inspection of CPython 3.12.1 showed that socket construction raises the `socket.__new__` audit event before WinSock socket creation. Because the FL audit path itself is broken, bypassing `socket.py` and constructing `_socket.socket` directly could not solve the problem.

### Standard filesystem I/O

Filesystem IPC was considered as a deliberately simple fallback.

A direct `open()` probe failed with:

```text
SystemError("<class '_io.FileIO'> returned NULL without setting an exception")
```

So ordinary Python file I/O was affected by the same runtime boundary and was not a reliable escape hatch.

### `ctypes`

`ctypes` was not a viable native escape path:

```text
ImportError: module _ctypes does not support loading in subinterpreters
```

We did not bypass this safety check.

### `_overlapped` / Windows named pipes

This path taught us useful things but was not retained.

The following native operations worked from FL:

- `_overlapped.CreateEvent` / `SetEvent` / `ResetEvent`;
- `_overlapped.Overlapped()` construction;
- `_overlapped.ConnectPipe()` reaching Win32 and returning a normal `FileNotFoundError` for a missing pipe;
- a real named-pipe connection to an external PowerShell server;
- `_winapi.CloseHandle()`.

This proved that the FL process was not generally sandboxed away from native Win32 IPC.

However, the first one-off asynchronous payload I/O experiment using `_overlapped` crashed FL Studio at the native boundary. At that point the path was intentionally abandoned rather than continuing to risk the DAW process.

The useful conclusion was not "named pipes do not work". It was that raw `_overlapped` lifetime/completion handling inside the FL Python layer was too fragile for this experiment.

## The breakthrough: our own subinterpreter-safe native module

The decisive experiment was a deliberately tiny CPython extension:

```text
ghost_native.cp312-win_amd64.pyd
```

It used CPython multi-phase module initialization and declared support for multiple interpreters. The first version exposed only simple calls:

```python
ghost_native.ping()
ghost_native.add(20, 22)
ghost_native.pid()
```

FL loaded it successfully from its shared Python `Lib` directory:

```text
runtime: cp312-subinterpreter-probe
ping: ghost-native-ok
add: 42
pid: <FL Studio process id>
```

That established a clean boundary:

```text
FL Python subinterpreter
        │
        │ ordinary extension call
        ▼
our .pyd
        │
        └── native Windows APIs
```

No private CPython bypasses were required.

## Native WinSock proof

The next extension revision added a native WinSock probe that performed only native calls:

```text
WSAStartup(2.2)
socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)
ioctlsocket(FIONBIO)
closesocket
```

Live FL result:

```python
{
    'ok': True,
    'stage': 'complete',
    'winsock_version': 514,
    'winsock_high_version': 514,
    'nonblocking': True,
}
```

A second probe attempted a real nonblocking connection to the existing Rust scripting listener:

```python
{
    'ok': True,
    'stage': 'connect',
    'status': 'in_progress',
    'winerror': 10035,
    'port': 48766,
}
```

`10035` (`WSAEWOULDBLOCK`) is the expected result for a nonblocking connection that has started and must be polled for completion.

This recovered the original architecture without relying on Python's audited socket constructor.

## Final transport implementation

The native module was then promoted from probe to a minimal persistent transport API:

```text
start(host, port)
poll()
recv(max_bytes)
send(bytes)
status()
close()
```

Its responsibilities are intentionally narrow:

- own WinSock initialization and cleanup;
- own the native socket handle;
- create an IPv4 TCP socket;
- put it in nonblocking mode;
- advance connection completion without waiting;
- perform bounded nonblocking `recv` / `send`;
- expose transport state/errors to Python;
- keep state per CPython module/interpreter rather than in process-global mutable Python extension state.

`device_Ghost.py` continues to own the scripting-specific layer:

- bounded `OnIdle()` scheduling;
- reconnect backoff;
- NDJSON framing;
- receive/send buffering;
- protocol validation;
- FL module/function allowlisting;
- FL API invocation;
- JSON-compatible result conversion;
- hello/result generation.

Rust continues to own request correlation, timeouts, diagnostics, the developer probe, and the application-side listener.

This is the key separation to preserve:

```text
Python: FL callback + protocol adapter
native module: OS transport boundary
Rust app: request/orchestration owner
```

## Live end-to-end proof

With the native transport wired into the real controller script, the scripting probe completed successfully inside FL Studio.

Observed live state:

```text
scriptingApiVersion: 44
flVersion: "Producer Edition v26.1.3 [build 5570]"
projectTitle: ""
projectChangedFlag: 1
safeToEdit: 1
selectedChannel: 2
selectedMixerTrack: 0
mixerTrackCount: 18
currentPattern: 1
patternCount: 0
currentPatternName: "Pattern 1"
arrangementSelectionStart: -1
arrangementSelectionEnd: -1
focusedPluginName: ""
focusedWindowCaption: "Channel rack"
songPosition: 0
songPositionHint: "1:01:00"
loopMode: 0
isPlaying: 0
arrangementSelectionActive: false
```

The first live write proof also succeeded:

```text
reversible mutation: attempted=true changed=true restored=true
```

The mutation is mixer-track selection. Rust first checks `general.safeToEdit()`, captures the current mixer track, selects another existing track, verifies the change, restores the original track, and verifies exact restoration.

This proves the complete loop:

```text
Rust request
  ↓
NDJSON over localhost TCP
  ↓
native WinSock extension inside FL
  ↓
device_Ghost.py dispatch
  ↓
real FL scripting API
  ↓
result serialization
  ↓
native WinSock extension
  ↓
Rust correlation/result
```

## Live-proven baseline

The baseline to preserve before further transport refactors is:

```text
branch: feat/fl-scripting-bridge
live-proven code commit: b38f1810fd2fd5b48ece57cccb66cac2790304a9
FL Studio: Producer Edition v26.1.3 [build 5570]
FL scripting API: 44
embedded Python: CPython 3.12.1 / cp312 / win_amd64
native extension API: 1
Rust listener: 127.0.0.1:48766
wire protocol: NDJSON v1
bootstrap MIDI device: Ghost Midi
```

The baseline architectural invariants are:

1. Gopher remains the frozen proven surface and is not rewritten around scripting.
2. scripting remains an additional app-local capability.
3. loopMIDI is bootstrap/autoload only.
4. TCP is local loopback and FL is the outbound client.
5. Python never blocks waiting for Ghost.
6. each `OnIdle()` performs bounded reads, dispatches, and writes.
7. scripting targets remain explicitly allowlisted.
8. no arbitrary Python execution is exposed.
9. scripting tools are not automatically injected into the Codex tool catalog at this stage.
10. live writes remain explicitly safety-gated and the initial proof is reversible.

The `patternCount: 0` / `currentPattern: 1` / `currentPatternName: "Pattern 1"` combination should be treated as observed FL API behavior to characterize later, not normalized away without further evidence.

## Why keep the C native module as the current baseline

The current `.pyd` is small and does one important job: it crosses from FL's unusual CPython subinterpreter into native OS networking without touching the broken Python audit path.

That implementation is now evidence, not speculation. Replacing it should therefore be treated as a controlled refactor with the live probe above as the regression gate.

The goal of the next experiment is not to redesign the scripting bridge. It is only to move ownership of the native transport implementation toward Rust while preserving this exact behavior.

## Future Rust-owned attempt

The safest next Rust-owned layout is a two-layer native boundary, not an immediate rewrite of the entire Python extension mechanism.

```text
FL Studio MIDI scripting subinterpreter
        │
        │ Python calls
        ▼
minimal ghost_native CPython shim
        │
        │ narrow C ABI / opaque handle
        ▼
Rust-owned transport core
        │
        │ native WinSock
        ▼
127.0.0.1:48766
        │
        ▼
ghost-fl-agent Rust listener
```

Suggested app-local repository shape:

```text
apps/ghost-fl-agent/
├── fl-script/
│   └── device_Ghost.py
│
└── fl-native/
    ├── python/
    │   └── ghost_native.c       # CPython/subinterpreter shim only
    ├── rust/
    │   ├── Cargo.toml
    │   └── src/lib.rs           # transport implementation
    ├── setup.py
    └── build.ps1
```

Keep this under `apps/ghost-fl-agent` until repeated evidence justifies a reusable shared crate.

### Rust-owned responsibilities

The Rust native core should own:

- socket creation/destruction;
- WinSock/native networking details;
- connection state machine;
- nonblocking connect completion;
- read/write error normalization;
- transport buffer limits that belong below Python;
- native lifecycle/cleanup;
- an opaque transport context with no process-global mutable connection state.

A small C ABI is enough for the first attempt, conceptually:

```text
ghost_transport_new
ghost_transport_start
ghost_transport_poll
ghost_transport_recv
ghost_transport_send
ghost_transport_status
ghost_transport_close
ghost_transport_free
```

The CPython shim should own only:

- multi-phase module initialization;
- `Py_mod_multiple_interpreters` declaration;
- per-interpreter storage of the opaque Rust transport pointer;
- conversion between Python bytes/strings and the narrow Rust C ABI;
- Python exception/result construction.

This preserves the part we have already proven inside FL while moving the OS/state-machine implementation into the project's primary systems language.

### Why not jump directly to a pure-Rust `.pyd`

A pure-Rust extension may eventually be a good endpoint, but it should be a later experiment rather than the first refactor.

The current hand-written CPython module has already proven the exact multi-phase/subinterpreter shape accepted by this FL runtime. The next step should change one variable at a time:

```text
current proven state:
CPython C shim + C WinSock

next controlled state:
CPython C shim + Rust transport core

possible later state:
pure Rust CPython extension, only after its subinterpreter/module-slot behavior is proven in FL
```

Do not trade away the known-good CPython boundary merely to remove a few hundred lines of C.

## Regression gate for the Rust-owned attempt

A Rust-owned transport refactor is successful only if it reproduces the current baseline without changing higher layers.

Minimum gate:

1. Windows CI builds the CPython 3.12 x64 extension and Rust native core.
2. FL imports the rebuilt `.pyd` in the same scripting subinterpreter.
3. native health/socket probe succeeds.
4. FL establishes the outbound nonblocking loopback connection.
5. hello handshake reports scripting API 44 and the live FL version.
6. the complete scripting probe returns the same classes of current-state observations.
7. the mixer-selection mutation again reports `attempted=true changed=true restored=true`.
8. Ghost can restart while FL remains open and the script reconnects cleanly.
9. repeated probes do not leak handles, stall `OnIdle()`, or leave stale native state.
10. the frozen Gopher path and Codex behavior remain unchanged.

Only after that gate should we consider whether the Rust native transport deserves a reusable crate or whether the entire `.pyd` should become Rust-owned.

## What this experiment established

The important finding is not simply that "FL scripting works."

It established a precise boundary for this FL version:

```text
FL's Python-level audited OS constructors are unreliable
                │
                ▼
a subinterpreter-compatible native extension is reliable
                │
                ▼
native nonblocking WinSock is reliable
                │
                ▼
the original localhost Rust ↔ FL architecture works
```

That lets future work return to the actual product questions: which pieces of FL scripting state materially complement Gopher, which operations should be exposed, and how constrained agent actions should be represented.

Transport feasibility is now a proven baseline rather than an open research question.
