# T21 — Make Editor Providers Selectable

## Dispatch

- Branch from: a checkpoint containing PLUGIN_FRONTEND_SHA and WEBVIEW_PROVIDER_SHA
- Parallel work: none on editor/plugin feature configuration
- Produces: EDITOR_SELECTION_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-21-editor-selection -b agent/21-editor-selection <EDITOR_PROVIDERS_BASE_SHA>
    Set-Location ..\gha-wt-21-editor-selection

The coordinator supplies EDITOR_PROVIDERS_BASE_SHA after reconciling T19 and T20. Read agent-ops/WORKTREE_CONTRACT.md.

## Objective

Allow the same CLAP plugin and frontend API to be packaged with either egui or Svelte/WebView, with an explicit deterministic selection policy.

## Owned paths

- Plugin and workspace feature configuration
- Packaging scripts
- Editor selection/composition module
- Packaging and smoke-test documentation

## Required work

- Add editor-egui and editor-webview features.
- Define default, mutually exclusive, and invalid combinations at compile or package time.
- Keep application/daemon behavior identical across editor choices.
- Add packaging commands and distinct artifact labels or metadata sufficient to avoid tester confusion.
- Run lifecycle smoke tests for both variants.

## Acceptance

- Each provider builds independently from a clean checkout.
- Invalid simultaneous selection fails with a clear message unless an intentional runtime selector is documented.
- Neither provider leaks its dependencies into the other's build unnecessarily.
- Both packages connect to the same fake/real daemon protocol.
- Windows x64 CLAP bundles for both variants are reproducible.

## Handoff

Make one commit named feat: support selectable egui and webview editors. Return its SHA and exact packaging commands. The coordinator records it as EDITOR_SELECTION_SHA.
