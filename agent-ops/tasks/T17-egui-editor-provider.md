# T17 — Extract the egui Editor Provider

## Dispatch

- Branch from: IMPLEMENTATION_SHA
- Parallel with: T13, T14, T18
- Produces: EGUI_PROVIDER_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-17-egui-provider -b agent/17-egui-provider <IMPLEMENTATION_SHA>
    Set-Location ..\gha-wt-17-egui-provider

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Move the working baseview/egui native editor lifecycle into ghost-editor-egui and expose it through EditorProvider. Leave only minimal temporary composition in the CLAP plugin.

## Owned paths

- crates/ghost-editor-egui/**
- Existing plugin editor files strictly needed for extraction
- Provider lifecycle tests/harness

Do not add daemon networking or WebView code.

## Required work

- Move window creation, event loop, egui integration, sizing, scale, and teardown into the provider.
- Inject FrontendApi and the pure ghost-ui renderer.
- Preserve the fixed hide/reopen behavior: hide must not destroy required render state.
- Make destroy idempotent and thread-affinity explicit.
- Add a fake FrontendApi harness for open, hide, reopen, resize, and destroy cycles.

## Acceptance

- ghost-editor-egui implements EditorProvider.
- It contains no Codex, database, host-rendering, or daemon-server ownership.
- Repeated hide/reopen cycles retain content.
- Destroy/recreate starts from a valid state and leaks no window thread.
- The CLAP plugin still packages and opens with temporary composition.

## Handoff

Make one commit named refactor: extract egui editor provider. Return its SHA and manual lifecycle smoke results. The coordinator records it as EGUI_PROVIDER_SHA.
