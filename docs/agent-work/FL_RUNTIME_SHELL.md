# FL Runtime Shell — validated path and known issues

## Status

As of 2026-08-26, the validated runtime flow is the externally started FL Studio path driven by `scripts/fl_init.ps1` and `ghost-fl-runtime --bootstrap active` (the default bootstrap mode).

The runtime-owned FL startup path (`--bootstrap start`) remains experimental and blocked. It is intentionally isolated in `apps/ghost-fl-runtime/src/bootstrap_start.rs` so it can be revisited without destabilizing the working path.

## Green path

The validated sequence is:

1. `scripts/fl_init.ps1` sets `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`.
2. The script starts FL Studio.
3. The script waits until `http://localhost:9222/json` exposes a target containing `Gopher`.
4. The script starts `ghost-fl-runtime.exe --i-accept-live-fl-writes`.
5. `ghost-fl-runtime` selects `bootstrap=active` by default.
6. `bootstrap_active` attaches to the existing `FL64.exe` process and waits for its main window.
7. The runtime performs a short native Gopher adapter probe against the already-visible Gopher target.
8. Shared runtime bootstrap completion marks Gopher ready, starts the configured Ghost FL app, waits for app health, waits for MIDI Scripting connectivity, opens the app webview, and transitions to `ready`.

This path has been live-tested with the workspace app fully functioning.

## Bootstrap ownership split

### `bootstrap_active.rs` — validated

This path assumes FL Studio and Gopher are already available. It does not launch FL Studio and does not attempt to open Gopher. Its job is attachment, readiness verification, and handoff into shared app startup.

### `bootstrap_start.rs` — deferred / failing

This path owns FL Studio startup and attempts Gopher activation after launch. It currently does not reproduce the reliable behavior of the PowerShell-owned startup path. Keep it as a comparator and do not fold its behavior back into the active path until the failure is understood.

## Resolved issue: runtime HTTP `10035`

Observed on Windows even while the green path was otherwise healthy:

```text
[ghost-fl-runtime] HTTP request failed: Uma operação de soquete sem bloqueio não pôde ser concluída imediatamente. (os error 10035)
```

Root cause: the runtime control `TcpListener` is nonblocking so the server loop can poll shutdown state. On Windows, an accepted `TcpStream` can retain nonblocking behavior. `read_request()` is a synchronous parser and treated the resulting `WouldBlock` / Winsock `10035` as a request failure.

Fix: keep the listener nonblocking, but normalize each accepted client stream to blocking mode with bounded read/write timeouts before parsing HTTP. This removes the spurious log without changing bootstrap, Gopher, app, or scripting behavior.

Validated result: the log no longer appears on the working path.

## Known issues / follow-up

- Runtime-owned FL startup (`--bootstrap start`) is still blocked and should be investigated separately from the validated path.
- The runtime currently has a deliberately simple synchronous HTTP control server. Some lifecycle POST handlers perform blocking work inline; this is acceptable for the current experimental shell but is not the desired long-term control-plane architecture.
- Gopher target visibility and Gopher bridge readiness are related but not identical states. Future work should preserve that distinction rather than infer full bridge health from target discovery alone.
- Runtime and workspace both probe integration health. Longer term, runtime supervision and app-owned adapter state should have clearer ownership boundaries to avoid duplicate readiness semantics.
- The current validated PowerShell launch flow leaves `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` in the parent environment. Do not change this while it remains part of the proven green path; revisit environment scoping only as an isolated experiment.

## Promotion rule

Treat the active bootstrap path as the runtime-shell baseline. Changes to FL/Gopher startup should be developed behind an alternate bootstrap strategy or separate branch until they reproduce the full green-path checks reliably.
