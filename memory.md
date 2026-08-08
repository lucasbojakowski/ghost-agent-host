# Design Memory

- Current user prompt supersedes old task trajectories. Only T01's architectural constraints are
  authoritative; other task files are references at most.
- Preserve analysis output compatibility while decomposing its implementation.
- The agent runtime must never know that a response is a mix plan. The caller supplies prompt input,
  output mode/schema, tools, and workflow interpretation.
- Context compiler owns both request presentation and response contracts. Runtime treats compiled
  context as opaque.
- Core operations describe intent using generic values and capability identifiers. Plugin adapters
  translate them to scanned public interfaces.
- Audio callback invariant: no allocation, locks, filesystem/network/IPC, serialization, agents, or
  logging.
- No live Codex thread/test is allowed. Test process behavior with deterministic fakes.
- Windows 11 x64 and FL Studio are the first manual target; architecture must remain portable.
- Official app-server docs (checked 2026-08-07) place `outputSchema` on turns and experimental
  `dynamicTools` on thread start; runtime structure follows that ownership.
- Native CLAP/DAW truth boundary: automated fake graph and loadable descriptor are implemented;
  proprietary child audio/parameter/GUI extension behavior requires the manual runbook.

## Redesign 02 durable decisions

- DAW activation is the sole source of sample rate and frame limits. UI defaults may describe
  capture duration but never audio engine truth.
- Preserve CLAP thread affinity by splitting native children into `NativeClapMain` and
  `NativeClapAudio`. Never add `unsafe impl Send` to a main-thread instance.
- Graph structural mutations request a host restart; bypass remains realtime through an atomic bit
  mask. This is the composed-simple boundary for dynamic nested hosting.
- Capture one selected graph edge plus input in fixed preallocated banks. Stable tap hashes avoid
  audio-thread strings. Always return and report the actual captured edge.
- `ghost.ui-state/2` owns graph topology and nested state blobs. OS handles and active instances are
  runtime-only.
- That line records Redesign 02 history. Redesign 03 supersedes the live schema with
  `ghost.ui-state/3`, including graph revision and accepted editor size.
- Fake layers have distinct jobs: trait fake for fast deterministic unit behavior; loadable fake for
  real CLAP ABI, params, state, audio, and Win32 GUI lifecycle.
- The embedded DAW UI stays deterministic/mock-agent by default. Live Codex is explicit through the
  CLI and is never selected by tests.
- EQ/compressor are available context recipes, not forced graph topology. Additional processor
  classes must add recipes without changing the native host core.
- `ghost-core::analysis` keeps its stable facade in `analysis/mod.rs`; spectral and dynamics details
  live in private sibling modules. `ghost-codex::lib` likewise keeps the public facade while mock and
  test concerns live separately. Preserve these seams when adding analyzers or transports.

## Redesign 03 invariants

- A mutex does not prevent lost updates when callers clone, unlock, mutate, then replace a whole
  document. Render directly against one committed document transaction or submit typed commands.
- Structural UI actions must only set a local commit intent. Increment the document revision while
  holding its owner lock, release it, then request outer restart.
- `UiSession` is outer-plugin-instance state, not editor-window state and not project serialization.
- Stable discovery selection is `(canonical path, plugin ID)`; vector indices are presentation-only.
- `active_revision` names the graph currently processing; `pending_revision` names the newest
  committed structural graph awaiting activation. Never infer one from what the editor displays.
- Detached child GUI ownership must not depend on the outer egui HWND. Prefer CLAP floating mode;
  otherwise embed in a dedicated top-level Win32 container owned by the plugin instance.
- Proposal preview stays non-destructive. Apply requires a complete revision-bound concrete patch,
  bounded realtime delivery, acknowledgements, child state capture, dirty notification, and undo.
- Never claim proprietary plugin or FL Studio acceptance from the deterministic fake matrix.

## Redesign 03 delivered contracts

- Serialized state is `ghost.ui-state/3`; versions 1/2 migrate. Outer accepted size is persisted and
  clamped to 860×600.
- UI structural edits commit revision under the document lock, publish pending revision, release the
  lock, then call dirty/restart. Maintain this exact ordering.
- Outer `clap.params` exists to receive host flush callbacks even though Ghost exposes zero public
  parameters of its own. It drains the nested patch queue and calls child `flush_active`; the main
  implementation handles inactive children.
- Patch capacity is 32 changes. Mapping issues, revision mismatch, node absence, or parameter
  rejection must fail visibly; do not add best-effort partial semantics by default.
- Acknowledged actual previous values replace preview estimates before Undo is retained.
- Child output param events update session feedback; feedback matching the applied value may mark a
  transaction Verified.
- Child GUI selection is floating-first, then a dedicated top-level embedded container. Never parent
  fallback children to the transient outer egui HWND again.
- Nested logs are dropped during child process to prevent formatting/allocation; off-audio logs use
  the bounded main-thread event queue.
- The deterministic matrix is 32 tests plus native/GUI/outer smokes as of 2026-08-07. The manual
  vendor/FL Studio matrix remains evidence still to collect.
