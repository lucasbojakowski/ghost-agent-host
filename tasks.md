# Redesign 02 Task Ledger

## Planning and audit

- [x] Preserve and inspect the existing uncommitted redesign on `redesign/full`.
- [x] Diagnose shared UI work state, fixed topology, file-only capture, and passthrough-only CLAP.
- [x] Write the Redesign 02 plan and conclusion criteria.
- [x] Inspect Clack 0.1.1 audio-port, transport, params, state, GUI, activation, and process APIs.

## Domain and application contracts

- [x] Add DAW transport/audio-configuration models with unavailable fields represented as `None`.
- [x] Add bounded DAW capture lifecycle and separate workflow-stage state.
- [x] Add processor class, editable node graph, presets, assignments, and graph-derived taps.
- [x] Evolve persisted UI/project state to `ghost.ui-state/2` with v1 migration.
- [x] Compile/filter EQ and compressor proposals from typed nodes and public capability manifests.

## Realtime capture and outer CLAP

- [x] Declare explicit main stereo input/output ports.
- [x] Use DAW activation rate/block limits and show them in the UI.
- [x] Extract/forward available CLAP transport, tempo, timeline, signature, and play-state data.
- [x] Implement bounded lock-free arm/record/cancel/input/selected-edge snapshot transfer.
- [x] Feed captured DAW audio and file audio through the same analysis operation.

## Graph and child hosting

- [x] Implement preset/create/remove/reorder/bypass/class/assignment graph operations.
- [x] Derive selectable capture taps from graph input, node outputs, and output.
- [x] Implement production descriptor/params/audio/state/Win32 GUI CLAP adapter.
- [x] Connect prepared native child processing and state to the outer plugin.
- [x] Request safe DAW restarts for structural changes and use atomic live bypass.
- [x] Surface parameter manifests and explicit unsupported/inactive child errors.

## UI

- [x] Render distinct Workflow, Analyzer, and Routing views.
- [x] Separate Capture, Analyze, and Propose actions, status, and enablement.
- [x] Isolate scanner/proposal scroll regions with stable IDs.
- [x] Make discovery hideable, rescannable, selectable, and assignable.
- [x] Make graph nodes and capture taps interactive.
- [x] Replace the hard-coded signal header and improve plot/transport/capture feedback.
- [x] Replace raw proposal JSON with a user-oriented renderer.

## Fakes, verification, and documentation

- [x] Extend loadable fake with ports, gain parameter, state, and a real Win32 GUI.
- [x] Add deterministic transport/capture/topology coverage.
- [x] Verify native fake inspect/process/parameter/state/destruction and GUI lifecycle.
- [x] Verify outer → nested fake audio and project-state round trip.
- [x] Pass fmt, clippy, workspace tests, and mock-only native health checks.
- [x] Split oversized analysis and Codex aggregation files along existing responsibility seams.
- [x] Update architecture, configuration, testing runbook, memory, journal, progress, and report.
- [x] Invoke modern-web-guidance for the final visual artifact.
- [x] Create and verify `visualizer/redesign-02.html` with the requested Tailwind CDN.

## Explicit next boundary (not represented as complete)

- [x] Aggregate dynamic child latency and expose outer `clap.latency` notifications (Redesign 03).
- [x] Map accepted semantic proposals to live public parameters with undo/verification (Redesign 03).
- [ ] Add specialized context recipes for saturation, reverb, limiter, and multiband compression.
- [ ] Complete the manual FL Studio/proprietary-plugin acceptance matrix.

---

# Redesign 03 Task Ledger

## Audit and contracts

- [x] Preserve the existing dirty `redesign/full` worktree and read prior planning/memory/journal.
- [x] Confirm the reported races, lifecycle defects, transport semantics, and missing apply path.
- [x] Run the pre-change mocked workspace baseline (21 tests passed; no live Codex invocation).
- [x] Add revisioned `ProjectDocument`/store and read-only runtime revision snapshots.
- [x] Add persistent `UiSession` with stable plugin/node identity.

## State and lifecycle

- [x] Remove full project clone/writeback from every egui frame.
- [x] Commit and increment graph revisions before requesting restart.
- [x] Reconcile state loads with pending/active graph revisions.
- [x] Preserve child state and workflow state without stale-frame overwrites.
- [x] Add close/reopen and concurrent-update regression tests.

## GUI and UX

- [x] Implement independent floating/detached child window sessions and callback routing.
- [x] Implement outer `can_resize`/`adjust_size`, minimum size, persisted accepted size.
- [x] Add responsive breakpoints, wrapping, truncation, and hover disclosure.
- [x] Add bounded visible-editor repaint and transport freshness presentation.

## Nested host and realtime control

- [x] Implement core restart/process/callback routing for nested plugins.
- [x] Implement child-facing GUI, params, state-dirty, latency, timer, thread-check, and log extensions.
- [x] Parse intra-block transport events and forward the latest relevant transport.
- [x] Consume child output parameter events and expose bounded acknowledgements/telemetry.

## Proposal application

- [x] Add semantic-to-concrete compilation with confidence, ranges, previous values, and revision.
- [x] Add explicit review/apply/undo UX and reject incomplete mappings by default.
- [x] Queue changes for block-boundary application or stopped-audio params flush.
- [x] Acknowledge, persist child state, mark project dirty, and expose failure/verification states.

## Verification and handoff

- [x] Add deterministic tests for revisioning, lifecycle, transport, callbacks, apply, and undo.
- [x] Pass fmt, clippy, workspace tests, release builds, and native fake health checks.
- [x] Update architecture, configuration, testing runbook, memory, journal, progress, and report.
- [x] Invoke modern-web-guidance only when final HTML work begins.
- [x] Create and verify `visualizer/runtime-coherence.html` using the requested Tailwind CDN.
