# FL Studio native transport probe

This directory contains a deliberately small CPython 3.12 Windows extension used to test a native transport boundary inside FL Studio's MIDI scripting subinterpreter.

It is **not wired into `device_Ghost.py` yet**. The purpose of this step is only to prove that our own subinterpreter-compatible `.pyd` can create a native nonblocking WinSock socket, then attempt a loopback connection without using Python's broken audited `_socket.socket()` path.

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

Copy that file into FL Studio's shared Python library directory, for example:

```text
<FL Studio install>\Shared\Python\Lib\
```

Restart/reload the FL scripting environment before importing a replaced `.pyd`.

## Probe 1: native socket creation

Run from FL Studio's scripting interpreter:

```python
import ghost_native

print(ghost_native.runtime)
print(ghost_native.API_VERSION)
print(ghost_native.ping())
print(ghost_native.pid())
print(ghost_native.socket_probe())
```

Expected shape:

```python
{
    "ok": True,
    "stage": "complete",
    "winsock_version": 514,
    "winsock_high_version": 514,
    "nonblocking": True,
}
```

The exact WinSock version integers are diagnostic; `ok=True` is the important result.

`socket_probe()` performs only native calls inside the extension:

1. `WSAStartup(2.2)`
2. `socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)`
3. `ioctlsocket(FIONBIO)`
4. `closesocket()`
5. `WSACleanup()`

No Python `_socket.socket` object is created.

## Probe 2: native loopback connect

Only run this after `socket_probe()` returns `ok=True`.

Start `ghost-fl-agent` so its scripting listener is active on `127.0.0.1:48766`, then run in FL:

```python
print(ghost_native.connect_probe("127.0.0.1", 48766))
```

A successful native start returns either:

```python
{"ok": True, "stage": "connect", "status": "connected", ...}
```

or:

```python
{"ok": True, "stage": "connect", "status": "in_progress", ...}
```

The probe intentionally closes the socket immediately and does not send the Ghost hello frame. The Rust bridge may therefore briefly observe a connection followed by EOF; that is expected for this probe.

A native failure returns a structured WinSock error instead of raising through FL's Python audit path, for example:

```python
{"ok": False, "stage": "connect", "winerror": 10061}
```

## Scope

This probe does not change the scripting protocol or expose any new tool surface. If both native probes pass, the next implementation step is to replace only the FL-side `_socket` transport with a small native nonblocking transport API while leaving the Rust listener, NDJSON framing, request IDs, bounded dispatch, allowlisted FL calls, and agent boundary unchanged.
