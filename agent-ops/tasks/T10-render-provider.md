# T10 — Adapt Audio Rendering Behind RenderProvider

## Dispatch

- Branch from: CONTRACTS_SHA
- Parallel with: T08, T09, T11, T12
- Produces: one input to IMPLEMENTATION_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-10-render-provider -b agent/10-render-provider <CONTRACTS_SHA>
    Set-Location ..\gha-wt-10-render-provider

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Expose existing ghost-host rendering and analysis preparation through RenderProvider without pulling CLAP editor behavior into the adapter.

## Owned paths

- crates/ghost-host/**
- Adapter-focused tests and fixtures

Do not edit plugin editor composition or UI crates.

## Required work

- Implement RenderProvider using the existing render pipeline.
- Isolate host-specific types at the adapter boundary.
- Forward cancellation and structured progress at meaningful stages.
- Normalize filesystem and plugin-host failures into application errors.
- Add a mock child/render backend so tests do not require installed commercial plugins.

## Acceptance

- ghost-host implements RenderProvider.
- Tests cover success, missing source, child failure, cancellation, and cleanup.
- The adapter performs no UI work and does not own application state.
- Existing host behavior remains available.
- cargo test -p ghost-host passes.

## Handoff

Make one commit named refactor: expose host rendering as application provider. Return its SHA and call out any behavior intentionally preserved.
