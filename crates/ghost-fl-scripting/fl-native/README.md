# FL Studio native transport

This directory contains the CPython 3.12 Windows extension used by the FL Studio MIDI scripting bridge.

FL Studio 2026 runs MIDI scripts in a CPython 3.12 subinterpreter. Live probing showed that the runtime's audit path is broken for audited operations such as `_socket.socket()` and `_io.FileIO`, while a custom multi-phase native extension declaring per-interpreter support loads and executes normally. `ghost_native` therefore owns only the OS transport boundary and leaves the scripting protocol and FL API dispatch in `device_Ghost.py`.

The transport remains loopback TCP to the existing Rust listener on `127.0.0.1:48766`. No Python `_socket.socket`, `ctypes`, filesystem IPC, or named-pipe data plane is used by the bridge.

## Native API

`ghost_native` exposes a deliberately small nonblocking transport surface:

```python
status()
start(host="127.0.0.1", port=48766)  # -> "connected" | "connecting"
poll()                                # -> "connected" | "connecting" | "disconnected"
recv(max_bytes=4096)                  # -> bytes | None
send(data)                             # -> bytes written, 0 when it would block
close()
```

The extension also retains `socket_probe()` and `connect_probe()` as diagnostic helpers.

The extension uses per-module state instead of mutable C globals so each importing Python subinterpreter owns its own socket state. WinSock is initialized when the module executes and cleaned up when the module is freed.

## Build

Use 64-bit Python 3.12 with the MSVC build tools available:

```powershell
py -3.12 -m pip install setuptools
powershell -ExecutionPolicy Bypass -File .\build.ps1
```

The build should produce an artifact similar to:

```text
ghost_native.cp312-win_amd64.pyd
```

The root `.pyd` in this directory is the preserved live-proven distributable from the source experiment. The tracked `build/` directory contains setuptools compiler output retained temporarily as historical build evidence; it is not the preferred install artifact and should only be cleaned after the crate-owned Windows build and FL runtime gate succeed.

Copy the validated root `.pyd` into FL Studio's shared Python library directory:

```text
<FL Studio install>\Shared\Python\Lib\
```

FL cannot replace a loaded `.pyd` in-place safely. Close FL Studio before replacing the extension, then restart it.

## Live validation

With `ghost-fl-agent` or `ghost-fl-workspace` running and listening on `127.0.0.1:48766`, reload the `Ghost Bridge` MIDI script.

The expected sequence is:

1. `device_Ghost.py` imports `ghost_native` and verifies `API_VERSION == 1`.
2. `ghost_native.start()` creates a native nonblocking WinSock socket and begins the loopback connection.
3. `OnIdle()` calls `ghost_native.poll()` until the connection completes; it never waits for the Rust process.
4. The Python script sends the existing versioned NDJSON hello frame.
5. Rust reports the scripting adapter as connected.
6. `ghost-fl-agent` can run the existing developer probe, while `ghost-fl-workspace` can use the same adapter through its app-owned search/describe/call gateways.

The protocol above the transport is unchanged: bounded newline-delimited JSON, Rust-owned request IDs, explicitly imported FL modules, bounded work per `OnIdle()`, reconnect backoff, and no Ghost semantic/tool policy in the native extension or lower scripting crate.

## Diagnostic probes

The native socket probe remains available:

```python
import ghost_native
print(ghost_native.runtime)
print(ghost_native.API_VERSION)
print(ghost_native.socket_probe())
```

A healthy result has `ok=True`, `stage="complete"`, and `nonblocking=True`.

A one-shot loopback connect probe is also available:

```python
print(ghost_native.connect_probe("127.0.0.1", 48766))
```

It intentionally closes the temporary socket immediately, so the Rust listener may observe a connection followed by EOF. Use the wired `Ghost Bridge` script for the actual protocol validation.
