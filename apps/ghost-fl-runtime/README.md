# ghost-fl-runtime

Experimental single-session operational shell for Ghost & Guild on FL Studio.

This app owns startup and supervision policy. It does **not** absorb Gopher, MIDI Scripting, Codex, or workspace semantics from the proven lower layers and apps.

## Responsibilities

- attach to one existing `FL64.exe` process or launch one with WebView2 CDP enabled;
- wait for a real FL top-level window instead of sleeping for a fixed delay;
- probe Gopher through `ghost-fl-studio` and make at most one automatic `Alt+F1` activation attempt;
- launch a closed set of registered Ghost app fixtures (`ghost-fl-workspace` or `ghost-fl-agent`);
- poll each app's existing HTTP health and scripting-status endpoints;
- expose a small local control panel for state, logs, app start/stop/restart, one-shot Gopher activation and Ghost shutdown;
- persist `runtime/session.json` plus an append-only per-session JSONL event log.

`ghost-application` remains a reserved promotion boundary. Nothing in this experiment is promoted there yet.

## Development run

From the workspace root on Windows:

```powershell
cargo run -p ghost-fl-runtime -- --i-accept-live-fl-writes
```

The default registered app is `ghost-fl-workspace`. Select the frozen raw control with:

```powershell
cargo run -p ghost-fl-runtime -- --app agent --i-accept-live-fl-writes
```

To establish only the FL/Gopher operational session without starting an agent app:

```powershell
cargo run -p ghost-fl-runtime -- --no-app
```

The default FL executable follows the current live-validation machine (`D:\Image-Line\FL Studio 2026\FL64.exe`). Override it with `--fl-executable` on another installation.

## Existing FL instances

The runtime never silently terminates an attached FL process. If an existing FL instance was launched without the WebView2 debugging environment, the runtime will make one Gopher activation attempt and then fail with an instruction to restart FL through Ghost. `--shutdown-fl-on-exit` only applies to an FL process launched by this runtime.

## Registered application boundary

The UI does not accept arbitrary commands. The runtime maps the `workspace` and `agent` profiles to known packages and validated arguments. By default it builds the selected package and spawns the resulting workspace binary directly so the supervised PID is the real app process rather than `cargo run`.

Use `--app-binary` for a prebuilt/packaged executable.

## State and logs

On Windows the default root is:

```text
%LOCALAPPDATA%\Konko\Ghost\
  runtime\session.json
  logs\<session-id>.jsonl
```

The session snapshot is fast recovery/diagnostic metadata, not authoritative DAW state. The JSONL journal is operational evidence and should not be confused with the future semantic workspace revision model.
