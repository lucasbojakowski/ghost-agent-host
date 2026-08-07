# T20 — Connect the CLAP Plugin to agentd

## Dispatch

- Branch from: a checkpoint containing DAEMON_CLIENT_SHA and EGUI_PROVIDER_SHA
- Parallel work: none on plugin composition
- Produces: PLUGIN_FRONTEND_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-20-plugin-frontend -b agent/20-plugin-frontend <PLUGIN_CLIENT_BASE_SHA>
    Set-Location ..\gha-wt-20-plugin-frontend

The coordinator supplies PLUGIN_CLIENT_BASE_SHA after reconciling both prerequisites. Read agent-ops/WORKTREE_CONTRACT.md.

## Objective

Compose the typed daemon client and egui editor provider in the CLAP plugin without putting networking or application work on the audio thread.

## Owned paths

- CLAP plugin crate composition/lifecycle files
- Plugin integration tests and packaging smoke scripts

Do not add live audio capture or child plugin hosting.

## Required work

- Create and connect the daemon client away from the audio callback.
- Inject FrontendApi into ghost-editor-egui through EditorProvider.
- Reflect connecting, unavailable, reconnecting, ready, and job states in UiState.
- Coordinate plugin deactivate/destroy with client and editor shutdown.
- Keep audio processing transparent and allocation-free relative to existing behavior.
- Add fake-daemon tests for unavailable startup, reconnect, progress, cancellation, hide/reopen, and destroy.

## Acceptance

- Opening the editor does not spawn daemon/application logic in the renderer.
- Daemon absence produces a usable diagnostic UI, not a hang or crash.
- Audio processing remains independent of UI/network state.
- Windows x64 CLAP package builds and opens.
- Integration tests pass with a fake daemon.

## Handoff

Make one commit named feat: connect clap editor to daemon frontend api. Return its SHA and FL Studio smoke checklist. The coordinator records it as PLUGIN_FRONTEND_SHA.
