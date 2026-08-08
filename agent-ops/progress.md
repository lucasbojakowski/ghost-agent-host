# Progress

## 2026-08-07 — Baseline and redesign contract

- Branch `redesign/full` began clean at `4b5f75e`.
- Baseline `cargo test --workspace --all-targets` passed.
- T01 was found to be a dispatch/acceptance specification, not an implemented ADR on this branch.
  Its dependency/lifecycle requirements are incorporated in `plan.md`.
- Existing architecture confirmed as prototype-grade: a 666-line analysis module, fixed
  input/post-EQ/output capture, FabFilter-shaped core plans and validation, workflow behavior inside
  the Codex crate, duplicated CLI/daemon orchestration, and a CLAP editor shell without child audio
  hosting.

## Truthfulness policy

Automated contracts and fake-child integration can be completed here. Real FabFilter binaries,
their proprietary interfaces, FL Studio scanning, native child GUI embedding, and DAW project reload
must be called verified only after the manual Windows/FL Studio runbook is executed.

## 2026-08-07 — Implementation

- Added `ghost-context`, `ghost-application`, `ghost-mix`, and `ghost-fakes` boundaries.
- Moved EQ/compressor schemas, prompt recipe, validation, and mock rendering out of core.
- Added Symphonia media decode, dynamic named capture, generic processor/task/protocol models, and
  specialized analysis modules.
- Rebuilt Codex around injected wire transport, compiled output contracts, events, and tool registry.
  Scripted tests initialize a thread and complete a structured turn without starting Codex.
- Added vendor-neutral child graph, smoothing, bypass, state, public semantic mapping, discovery, GUI
  lifecycle, a trait fake, and a loadable fake CLAP pass-through binary.
- Added correlated daemon envelopes while retaining legacy JSONL and added CLI plugin/health commands.
- Reworked egui into a 1180×760 three-pane interface with off-thread work/discovery and CLAP project
  state persistence.
- Native third-party child processing/parameter extension bridging and FL Studio validation remain a
  declared manual boundary in `testing-runbook.md`; they are not represented as complete.

## 2026-08-07 — Verification

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace --all-targets`: passed (18 tests, no failures).
- Release builds for `ghost-clap-plugin` and `ghost-fakes`: passed.
- CLI to mock-daemon health check: passed with matching request ID and no Codex process.
- The final visual trajectory uses the requested Tailwind browser CDN and follows the modern web
  guidance for semantic landmarks, heading order, keyboard focus, contrast, and reduced motion.

## 2026-08-07 — Redesign 02 implementation

- Replaced the global UI receiver with independent capture, analysis, proposal, and scan work.
- Added distinct Workflow, Analyzer, and Routing bodies with isolated scroll identities.
- Added hide/rescan discovery, descriptor selection, parameter-manifest inspection, and assignment.
- Added editable presets, create/remove/reorder/reclassify/bypass, and graph-derived taps.
- Added separate Capture/Analyze/Propose actions and user-oriented proposal rendering.
- Added host-owned sample rate/block display and availability-safe tempo/timeline/transport capture.
- Added a 1,152,000-frame bounded stereo DAW recorder and selected graph-edge recording.
- Added explicit outer stereo audio ports.
- Added production `NativeClapMain`/`NativeClapAudio`: descriptor/params, activation, parameter
  events, audio, transport forwarding, state, Win32 child GUI, and deterministic destruction.
- Connected assigned native children to the outer CLAP callback. Structural edits request a DAW
  restart; bypass uses an atomic runtime mask.
- Upgraded the loadable fake with ports, gain param, state, and a real embedded 420×120 Win32 UI.
- Added CLI native-audio, generic CLAP-audio, and GUI smoke operations.

## Redesign 02 verification evidence

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace --all-targets`: passed (21 tests).
- Native fake session with parameter 1 = 0.5: first output `[0.125, -0.125]`, one parameter,
  eight-byte child state.
- Fake GUI lifecycle: `embedded GUI 420×120 passed` over two create/show/hide/destroy cycles.
- Outer nested smoke loaded `ghost.ui-state/2` with fake child state bytes
  `[0,0,0,0,0,0,224,63]`; first output `[0.125,-0.125]`; release saved-state size 680 bytes.
- No live Codex process or live agent thread was started.
- Final release builds for `ghost-clap-plugin` and `ghost-fakes` passed after the UI module split.
- Final release native, embedded-GUI, and outer-to-nested smoke checks all passed; the outer release
  state payload was 680 bytes in this checkout (the serialized child path makes this size contextual).
- Final size audit split `ghost-core::analysis` into entrypoint, spectrum, and dynamics modules and
  split `ghost-codex` into server, mock, and external test modules. Public entrypoints are unchanged;
  post-split fmt, warnings-denied Clippy, 21 tests, and release builds passed.

## Remaining truth boundary

- Proprietary plugins still require the FL Studio matrix for unusual port layouts, dynamic latency,
  resize negotiation, and vendor parameter semantics.
- Specialized context recipes are complete for Equalizer and Compressor. Saturation, Reverb,
  Limiter, and Multiband compressor are typed graph modes with visibly pending recipes.
- The native adapter can send parameter events, but proposal acceptance is still staged; semantic
  proposal operations are not automatically applied to arbitrary proprietary controls.
- Dynamic child latency aggregation and outer `clap.latency` reporting remain pending.

## Redesign 02 visual record

- Created `visualizer/redesign-02.html` as a self-contained, responsive implementation trajectory.
- Used the requested Tailwind v4 browser CDN and modern-web-guidance recommendations for semantic
  landmarks, heading order, keyboard-visible focus, accessible SVG description, tabular evidence,
  progressive scroll animation, and reduced-motion behavior.
- The page explicitly separates shipped contracts from the remaining proprietary-host acceptance
  boundary; it does not represent pending latency, semantic automation, or vendor testing as done.

## 2026-08-07 — Redesign 03 baseline

- Preserved the extensive uncommitted Redesign 02 work on `redesign/full`; no reset, checkout, or
  unrelated cleanup was performed.
- Read the existing plan, task ledger, architecture, memory, journal, configuration, runbook, and
  report before changing runtime code.
- Confirmed in source the stale whole-project frame writeback, disposable `GhostUi` jobs/results,
  inline restart requests before frame commit, child GUI parenting to the outer HWND, missing outer
  resize negotiation, transport repaint/beat defects, unit nested host handlers, discarded child
  output events, and absent proposal Apply operation.
- Pre-change `cargo test --workspace --all-targets`: 21 passed, zero failed. The command used only
  scripted/mock transports and did not launch Codex or create a live Codex task.

## 2026-08-07 — Redesign 03 implementation

- Replaced whole-document frame cloning with a lock-owned render transaction. Structural actions
  publish a monotonically increasing `ghost.ui-state/3` revision and pending snapshot before the DAW
  restart request can run.
- Added outer-instance `UiSession`; closing/reopening egui now preserves scan/work receivers,
  discoveries, stable selection, capture/analysis/proposal, status, revisions, and parameter
  feedback.
- State load migrates versions 1–3, assigns a revision newer than the current/loaded value, commits
  it, publishes pending state, and only then requests restart. Activation publishes the exact active
  revision.
- Child editors negotiate floating Win32 first and otherwise use a dedicated top-level detached
  container. Outer egui destruction no longer destroys or invalidates the child GUI session.
- Added child host core, GUI, params, state, latency, timer, thread-check, and log behavior; callback
  events cross a bounded queue. Dynamic latency is aggregated through outer `clap.latency`.
- Added outer resize negotiation (860×600 minimum), 950 px layout breakpoint, bounded text,
  wrapping/hover disclosure, and a 25 Hz visible-editor repaint.
- Transport now includes generation/freshness, beat within bar, tempo increment, loop ranges,
  pre-roll, and the latest intra-block event for publication/forwarding.
- Added strict EQ/compressor semantic compilation and explicit Preview/Mapping incomplete/Queued/
  Applied/Verified/Failed states. Apply and Undo use a 32-change lock-free queue, revision checks,
  acknowledgements, child state sync, and project-dirty notification.
- Added the stopped-audio route: the outer plugin advertises `clap.params`, requests host flush, and
  forwards active or inactive flushes to nested children. Child-produced parameter events are no
  longer discarded.
- Intermediate workspace gate after acceptance additions: 31 tests passed, zero failed. Native fake
  audio/state, GUI lifecycle, and outer→nested smokes had already passed in debug.
- Release artifacts built successfully. Release native fake returned ±0.125, one parameter, and an
  eight-byte state; GUI recreation reported 420×120 passed; outer→nested revision-7 state returned
  ±0.125 and a 692-byte location-dependent payload. The closing native run also exercises an
  explicit active child `params.flush` before audio processing.
- Deliberate simplicity: a general-purpose UI command bus was not added. egui and CLAP editor work
  already share the main-thread boundary, so a single lock-owned transaction plus typed structural
  commit intent supplies ordering without another asynchronous state owner.
- File-size review found lifecycle facades remain sizeable (`native.rs`, outer `editor.rs`, UI
  facade/views), but new policies were extracted into `native_host`, `parameter_control`, `session`,
  `patch`, and `child_window`; further mechanical splitting was avoided during lifecycle hardening.
- No live Codex process, test, thread, or task was invoked.

## 2026-08-07 — Redesign 03 closing gate

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace --all-targets`: 32 passed, zero failed.
- `cargo build --release -p ghost-clap-plugin -p ghost-fakes`: passed. MSVC emitted only its normal
  import-library creation messages.
- Release native smoke, including explicit active child `params.flush`: first output
  `[0.125,-0.125]`, one public parameter, eight-byte child state.
- Native child GUI smoke: `embedded GUI 420×120 passed`.
- Release outer→nested smoke loaded `ghost.ui-state/3` graph revision 7 and returned
  `[0.125,-0.125]`; saved state was 692 location-dependent bytes.
- `visualizer/runtime-coherence.html` uses the requested Tailwind browser CDN and passed browser QA
  at 1440×900 and 390×844 with no console warnings/errors and no horizontal document overflow.
- Automated conclusion criteria are complete. FL Studio/proprietary-plugin checks remain the honest
  manual boundary in `testing-runbook.md`.
- The closing audit caught and removed a mixed-patch partial-apply edge: parameter changes now cross
  as one bounded transaction, every revision/node/parameter is preflighted before mutation, and
  bypass changes commit only after every parameter acknowledgement succeeds. A regression test
  verifies success commits bypass and rejection leaves it untouched.
