# T19 — Implement the Rust WebView Editor Provider

## Dispatch

- Branch from: a coordinator checkpoint containing CONTRACTS_SHA and SVELTE_SHA
- May run in parallel with: T15
- Produces: WEBVIEW_PROVIDER_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-19-webview -b agent/19-webview <SVELTE_INTEGRATION_SHA>
    Set-Location ..\gha-wt-19-webview

The coordinator must supply SVELTE_INTEGRATION_SHA after reconciling T18 onto the contracts/implementation line. Read agent-ops/WORKTREE_CONTRACT.md.

## Objective

Implement ghost-editor-webview as an EditorProvider that embeds the built Svelte assets and connects its typed bridge to an injected FrontendApi.

## Owned paths

- crates/ghost-editor-webview/**
- WebView asset embedding/build integration
- Provider-specific tests and harness

Do not change CLAP composition or make this the selected editor.

## Required work

- Host the Svelte build in a Windows-compatible WebView with no network requirement.
- Implement open/show/hide/resize/focus/destroy lifecycle.
- Bridge typed JSON commands, snapshots, progress, completion, failure, and cancellation.
- Validate messages and protocol versions at the Rust boundary.
- Prevent callbacks after destroy and avoid blocking UI/WebView callbacks.
- Add a fake FrontendApi harness and bridge transcript tests.

## Acceptance

- Implements EditorProvider.
- Built assets are embedded or packaged deterministically.
- No daemon-server, Codex, database, or audio-host ownership.
- Malformed bridge input cannot panic the host.
- Repeated hide/reopen and destroy/recreate work in the harness.
- Windows x64 build and provider tests pass.

## Handoff

Make one commit named feat: add svelte webview editor provider. Return its SHA, required Windows runtime, asset build command, and lifecycle smoke results. The coordinator records it as WEBVIEW_PROVIDER_SHA.
