# Ghost DAW Integration Plan — Redesign 02

## Objective

Turn the valid outer CLAP shell into a useful DAW-native agent host. The DAW owns realtime audio
configuration and transport. The processor graph owns an editable sequence of typed nodes. Capture
taps are derived from that graph. Analysis and proposal are explicit, independently triggered
workflow stages. Native CLAP children are discovered, inspected, instantiated, processed, saved,
and (when offered by the child) shown through a stable host adapter.

The implementation remains vendor-neutral. Equalizer and compressor are initial context recipes,
not fixed graph nodes or required taps.

## Dependency and thread rule

```text
CLAP callbacks / egui / CLI / daemon
              |
              v
       ghost-application ports
              |
      +-------+--------+
      v                v
 ghost-host        ghost-context
 native child       recipes
      |                |
      +-------+--------+
              v
          ghost-core
```

- The audio thread may copy bounded samples, update atomics, and process an already-prepared graph.
  It never allocates, locks, scans, opens files, serializes, logs, or invokes an agent.
- Main/UI/background workers configure graphs, scan/load children, compile context, analyze captured
  snapshots, and produce proposals.
- Sample rate, block size, steady sample time, tempo, song position, time signature, and play/record
  state come from the CLAP host when available. No UI-owned sample-rate fiction is presented.
- UI state is a serializable projection. Native runtime handles and realtime buffers are excluded
  from project state; child identities, node types, bypass, mapping, parameters, and state blobs are
  included.

## Delivery phases

1. **Baseline and contracts** — preserve the uncommitted redesign, record this plan/ledger, inspect
   Clack APIs, and add DAW transport, capture-source, workflow-stage, processor-class, graph-node,
   and dynamic-tap models with migration-safe persisted state.
2. **Realtime DAW capture** — add explicit stereo audio ports; bind activation to host sample rate
   and frame limits; publish transport metadata; implement a bounded lock-free capture bridge;
   support arm/cancel/record/complete; and retain file input as a separate source.
3. **Editable graph and native children** — add preset graph templates (empty, equalizer,
   compressor, EQ + compressor), create/remove/reorder/bypass nodes, select processor class, derive
   taps from graph edges, inspect descriptor/parameters, and instantiate/process/save native CLAP
   children. Keep unsupported child GUI behavior explicit.
4. **Composed workflow** — split Capture, Analyze, and Propose into separate actions and state
   transitions; allow re-analysis and re-proposal; compile class-specific EQ/compressor context from
   measured evidence plus the selected child capability/parameter manifest.
5. **UI repair and redesign** — give Workflow, Analyzer, and Routing distinct views; isolate scroll
   areas and widget IDs; make discovery collapsible; make nodes and taps interactive; improve signal
   plots and transport/capture feedback; render proposals as intent/evidence/changes/cautions rather
   than raw system JSON.
6. **Fake and integration coverage** — extend the loadable fake CLAP with public parameters, state,
   audio ports, and a minimal GUI if the host path supports it; verify native load/process/state and
   deterministic DAW capture/transport without launching Codex.
7. **Documentation and acceptance** — update architecture, configuration, testing runbook, memory,
   journal, progress, and report; then use modern-web-guidance while creating
   `visualizer/redesign-02.html` with the requested Tailwind browser CDN.

## Conclusion criteria

- `cargo fmt --all -- --check`, workspace clippy with warnings denied, all tests, and release builds
  for the outer and fake CLAPs are green.
- Outer CLAP audio ports are explicit and its active configuration comes from the DAW.
- DAW audio can be armed, recorded into a bounded snapshot, cancelled, and analyzed; file analysis
  remains available independently.
- Available CLAP transport fields are captured and shown without inventing unavailable values.
- Workflow stages are independently actionable and cannot accidentally share pressed/loading state.
- Workflow, Analyzer, and Routing render genuinely different layouts with isolated scrolling.
- Graph nodes can be created from an empty graph or presets, removed, reordered, bypassed, assigned
  a processor class, and associated with a discovered plugin.
- Capture taps are enabled/disabled dynamically from the graph topology and can be selected for
  analysis.
- At least the loadable fake child is natively discovered, instantiated, audio-processed, parameter
  inspected/changed, state-round-tripped, and cleanly destroyed through the production adapter.
- EQ and compressor proposal recipes consume the chosen node class and public capability manifest;
  neither is forced into the graph.
- Proposal UI hides wire/schema fields and presents only user-relevant evidence, operations,
  expected changes, confidence, cautions, and verification guidance.
- `progress.md` distinguishes automated guarantees from manual FL Studio/proprietary-plugin checks.
- No test or health check launches Codex or creates a live Codex thread.

## Completion status

All in-repository conclusion criteria are implemented and covered by the final verification gate.
The proprietary FL Studio matrix, dynamic latency aggregation, automatic semantic parameter
application, and four additional specialized context compilers remain explicit follow-on boundaries,
not hidden qualifications on this delivery.

---

# Ghost Runtime Coherence Plan — Redesign 03

## Objective

Replace the four unsynchronized state owners with explicit document, session, main-thread, and
realtime boundaries. Graph edits become revisioned commits; the active audio revision is observable;
editor close/reopen preserves workflow and discovery state; detached child windows have independent
lifetimes; transport is fresh and musically correct; and proposals compile into reviewable parameter
transactions that can be applied, acknowledged, persisted, and undone.

## Stable boundaries

- `ProjectDocument`: the only serialized project model, owned and committed on the CLAP main-thread
  boundary. It carries a monotonically increasing document revision.
- `UiSession`: non-serialized scan/workflow/selection/job state, shared by every transient egui
  window for the lifetime of the outer plugin instance.
- `ProjectRuntimeSnapshot`: read-only active/pending revision and host notices for presentation.
- `RealtimeControl`: atomics and bounded queues used by the audio callback; no mutex, allocation,
  serialization, filesystem, GUI, or agent calls.
- `ChildWindowSession`: per-node floating or detached-container GUI lifecycle independent of the
  outer editor HWND.
- `CompiledParameterPatch`: revision-bound, range-checked, confidence-bearing concrete changes;
  preview generation is non-destructive and apply is an explicit transaction.

## Delivery slices

1. Add revisioned project/session primitives and remove whole-document clone/writeback from egui.
2. Commit graph edits before requesting restart; publish document, pending, and active revisions;
   reconcile DAW state loads with the active graph.
3. Move workflow jobs/results, plugin discovery, stable plugin/node selection, and status into a
   persistent `UiSession`.
4. Make outer resizing negotiable and responsive; bound/truncate unbounded strings with full hover
   values; repair proposal wrapping and narrow layouts.
5. Publish transport generation/freshness, repaint visible editors at a bounded cadence, calculate
   beat within bar, and consume intra-block transport events.
6. Implement nested-host core/GUI/params/state/latency/timer/thread-check/log routing without
   weakening thread affinity.
7. Prefer plugin-owned floating child windows, otherwise create a host-owned detached Win32
   container and embed the child; service resize/show/hide/closed callbacks.
8. Compile semantic proposal operations into concrete parameter changes, require complete mappings
   by default, queue application at the audio boundary, consume output feedback, acknowledge the
   transaction, save state, mark dirty, and support undo.
9. Add deterministic concurrency, state-load, close/reopen, transport, callback, apply, and undo
   coverage; run fmt, clippy, workspace tests, release builds, and native-fake health checks.
10. Refresh architecture/configuration/runbook/report and create a final Tailwind visual trajectory
    page after invoking modern-web-guidance.

## Conclusion criteria

- No egui frame replaces a stale clone of the full project document.
- Every structural edit is committed and revisioned before the outer host restart is requested.
- Active and pending graph revisions are independently observable and tested across load/activate.
- Closing/reopening the outer editor preserves discovery, selection, analysis, proposal, and jobs.
- Child GUI lifetime is independent of the outer editor HWND and repeated open/hide/close/reopen is
  covered by the production fake path.
- The outer editor advertises resize support, enforces a documented minimum, and its layouts remain
  usable at narrow and wide supported sizes.
- Transport repaints while visible, reports freshness, uses beat-within-bar, and accepts later
  transport events from the process event stream.
- The nested host exposes meaningful core, GUI, params, state, latency, timer, thread-check, and log
  behavior, with callbacks routed to the correct outer boundary.
- Proposal preview, mapping failure, queued, applied, verified, failed, and undo states are explicit;
  silent partial application is rejected by default.
- Child output parameter events are consumed and stopped-audio application has a `params.flush`
  route where supported.
- `cargo fmt --all -- --check`, workspace clippy with warnings denied, all mocked/local tests, release
  builds, and native fake health checks pass without launching Codex or creating live Codex tasks.
- Manual proprietary-plugin/FL Studio checks remain clearly labeled as manual evidence rather than
  being represented as automated verification.

## Completion

All automated conclusion criteria are satisfied on 2026-08-07. Format, warnings-denied clippy, 32
workspace/all-target tests, release outer/fake builds, active `params.flush`, native GUI recreation,
and schema-v3 outer→nested state/audio checks pass. The FL Studio/proprietary-plugin matrix remains
explicitly manual, as required; no vendor claim is inferred from the deterministic fake.
