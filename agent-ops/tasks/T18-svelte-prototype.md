# T18 — Build the Svelte 5 Protocol Prototype

## Dispatch

- Branch from: IMPLEMENTATION_SHA
- Parallel with: T13, T14, T17
- Produces: SVELTE_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-18-svelte -b agent/18-svelte <IMPLEMENTATION_SHA>
    Set-Location ..\gha-wt-18-svelte

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Required skill

Use the modern-web-guidance skill before any HTML, CSS, or client-side JavaScript work, and record relevant decisions in the handoff.

## Objective

Create a Svelte 5 frontend prototype driven only by generated protocol/UI types and an injected bridge. It must work in a normal browser with recorded transcripts before WebView embedding.

## Owned paths

- web/ghost-ui/**
- web/generated consumption fixes that do not change the source protocol

Do not implement Rust WebView hosting or daemon networking.

## Required work

- Scaffold a reproducible Svelte 5 application and locked toolchain.
- Define an injected bridge interface for commands, snapshots, and events.
- Implement a mock bridge that replays recorded transcripts.
- Cover ready, analyzing, progress, completed, failure, cancellation, and reconnect states.
- Keep styling and components independent from Rust internals.
- Add typecheck, unit, and production-build commands.

## Acceptance

- No hand-written duplicate of generated service DTOs.
- The app runs entirely from a recorded transcript.
- Bridge calls are the only service boundary.
- npm/pnpm install, typecheck, tests, and production build are deterministic.
- Built assets are suitable for offline embedding; no runtime CDN dependency.

## Handoff

Make one commit named feat: prototype svelte frontend over typed bridge. Return its SHA, commands, build output path, and transcript inventory. The coordinator records it as SVELTE_SHA.
