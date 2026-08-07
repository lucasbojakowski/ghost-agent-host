# T06 — Define the Editor Provider API

## Dispatch

- Branch from: PROTOCOL_SHA
- Parallel with: T04, T05, T07
- Produces: one input to CONTRACTS_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-06-editor-api -b agent/06-editor-api <PROTOCOL_SHA>
    Set-Location ..\gha-wt-06-editor-api

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Define the native editor lifecycle shared by egui and future WebView implementations while keeping concrete windowing and rendering dependencies outside the API.

## Owned paths

- crates/ghost-editor-api/**

## Required work

- Define EditorProvider and EditorHandle.
- Define a platform-neutral NativeParent representation with explicit Windows conversion helpers behind cfg gates.
- Model open, show, hide, resize, scale, focus, and destroy behavior.
- Define event/callback interaction with FrontendApi without coupling to a runtime or renderer.
- State Send/Sync and thread-affinity requirements explicitly.
- Add a fake editor provider and lifecycle conformance tests.

## Acceptance

- No egui, baseview, wry, WebView2, Svelte, or CLAP dependency.
- Repeated hide/show is distinct from destroy/recreate.
- Destruction is idempotent and resources cannot outlive their frontend handle accidentally.
- cargo test -p ghost-editor-api passes.

## Handoff

Make one commit named feat: define interchangeable editor provider api. Return its SHA and any lifecycle assumptions for T17 and T19.
