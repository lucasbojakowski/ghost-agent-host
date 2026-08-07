# Ghost Agent Host — Agent Operations

This directory contains the coordinated worktree plan for separating application logic from editor implementations, hardening the daemon, adding interchangeable egui and Svelte/WebView providers, and continuing toward realtime capture and real child-plugin hosting.

## Dispatch rules

1. Read [`WORKTREE_CONTRACT.md`](WORKTREE_CONTRACT.md) before assigning any task.
2. Resolve the checkpoint named by the task in [`CHECKPOINTS.md`](CHECKPOINTS.md).
3. Give one task file to one agent. Do not combine tasks.
4. Tasks in the same wave may run in parallel only when the dependency table below permits it.
5. Agents return commits; they never merge, rebase, or edit the coordinator worktree.
6. Reconcile each wave according to [`RECONCILIATION.md`](RECONCILIATION.md).

## Execution waves

| Wave | Tasks | Parallel? | Required checkpoint |
|---|---|---:|---|
| 0 | T00 | No | `OPS_SHA` |
| 1 | T01 → T02 → T03 | No | Previous task merged |
| 2 | T04, T05, T06, T07 | Yes | `PROTOCOL_SHA` |
| 3 | T08, T09, T10, T11, T12 | Yes | `CONTRACTS_SHA` |
| 4 | T13, T14, T17, T18 | Yes | `IMPLEMENTATION_SHA` |
| 5a | T15 and T19 | Yes | Their individual dependencies |
| 5b | T16 | No | `AGENTD_HARDENED_SHA` |
| 6a | T20 | No | `DAEMON_CLIENT_SHA` and egui provider merged |
| 6b | T21 | No | T19 and T20 merged |
| 7a | T22 or T24 | No shared implementation window | `EDITOR_SELECTION_SHA` |
| 7b | T23 after T22; T25 after T24 | Dependency-bound | Respective predecessor |
| 7c | T26 may parallel T25 with disjoint ownership | Limited | `CHILD_DISCOVERY_SHA` |
| 7d | T27 → T28 | No | Previous task merged |

## Task index

- [T00 — Freeze baseline](tasks/T00-freeze-baseline.md)
- [T01 — Architecture decision record](tasks/T01-architecture-decision.md)
- [T02 — Workspace scaffolding](tasks/T02-workspace-scaffolding.md)
- [T03 — Versioned service protocol](tasks/T03-service-protocol.md)
- [T04 — Application provider ports](tasks/T04-application-ports.md)
- [T05 — Pure UI state reducer](tasks/T05-ui-state-reducer.md)
- [T06 — Editor-provider API](tasks/T06-editor-provider-api.md)
- [T07 — Schemas and TypeScript bindings](tasks/T07-protocol-bindings.md)
- [T08 — Application controller](tasks/T08-application-controller.md)
- [T09 — Codex provider adapter](tasks/T09-codex-provider.md)
- [T10 — Render provider adapter](tasks/T10-render-provider.md)
- [T11 — Repository provider adapter](tasks/T11-repository-provider.md)
- [T12 — Pure egui renderer](tasks/T12-egui-renderer.md)
- [T13 — CLI migration](tasks/T13-cli-migration.md)
- [T14 — Initial daemon migration](tasks/T14-agentd-migration.md)
- [T15 — Daemon hardening](tasks/T15-agentd-hardening.md)
- [T16 — Typed daemon client](tasks/T16-daemon-client.md)
- [T17 — egui editor provider extraction](tasks/T17-egui-editor-provider.md)
- [T18 — Svelte 5 protocol prototype](tasks/T18-svelte-prototype.md)
- [T19 — Rust WebView provider](tasks/T19-webview-provider.md)
- [T20 — CLAP-to-daemon integration](tasks/T20-clap-daemon-integration.md)
- [T21 — Selectable editor providers](tasks/T21-editor-selection.md)
- [T22 — Realtime capture bridge](tasks/T22-realtime-capture.md)
- [T23 — Capture commands through daemon](tasks/T23-capture-daemon-flow.md)
- [T24 — Child CLAP discovery and instantiation](tasks/T24-child-discovery.md)
- [T25 — Child audio and event forwarding](tasks/T25-child-audio-forwarding.md)
- [T26 — Semantic parameter adapter](tasks/T26-semantic-parameter-adapter.md)
- [T27 — State and latency forwarding](tasks/T27-state-latency.md)
- [T28 — Child GUI hosting](tasks/T28-child-gui-hosting.md)
