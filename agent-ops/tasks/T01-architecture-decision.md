# T01 — Record the UI-Agnostic Architecture

## Dispatch

- Branch from: BASE_SHA
- Parallel work: none
- Produces: ADR_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-01-architecture -b agent/01-architecture <BASE_SHA>
    Set-Location ..\gha-wt-01-architecture

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Write the architecture decision that keeps core analysis, Codex integration, persistence, daemon transport, and editor technology independent. This decision is the contract for subsequent agents.

## Owned paths

- docs/architecture/**
- README links to the new architecture document

Do not edit Rust implementation files.

## Required work

Define:

- Dependency direction: domain/protocol inward, adapters outward.
- Separate application ports for agent, rendering, and persistence.
- A pure serializable UI state/action model.
- An editor-provider lifecycle that can host egui or WebView.
- The daemon boundary and versioned wire protocol.
- Realtime-thread prohibitions and ownership of audio transfer.
- Cancellation, progress, request correlation, and error semantics.
- How native CLAP child hosting remains separate from the analysis application.
- Compatibility and migration policy for the existing CLI and daemon.

Include a dependency diagram, lifecycle sequence, rejected alternatives, and enforceable acceptance rules.

## Acceptance

- The document makes it impossible to interpret a GUI crate as the owner of Codex, storage, or audio analysis.
- Crate boundaries and dependency directions are explicit.
- Both an in-process frontend and daemon-backed frontend fit the same frontend API.
- The document describes how egui and Svelte/WebView can evolve independently.
- No implementation code changes.

## Handoff

Make one commit named docs: define ui-agnostic application architecture. Return its SHA. The coordinator records it as ADR_SHA.
