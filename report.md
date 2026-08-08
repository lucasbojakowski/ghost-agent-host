# Ghost Runtime Coherence — Implementation Report

## Outcome

Redesign 03 removes the unsynchronized four-owner behavior at its boundaries. Project data is
revisioned and updated transactionally; workflow state survives window recreation; active and
pending graph revisions are explicit; child windows have independent floating/detached lifetimes;
transport repaints and reports freshness; and proposal preview now leads to a strict, acknowledged
Apply/Undo transaction.

The original whole-state frame clone/writeback is gone. Structural edits increment the committed
revision before requesting restart, so rapid host reactivation cannot observe the old topology.
State load migrates versions 1/2/3 and assigns a monotonic new revision before restart.

## Runtime and UX changes

- Added persistent `UiSession` for discovery, stable selection, receivers/results, analysis,
  proposal, statuses, revision projection, and parameter feedback.
- Added responsive workflow/routing breakpoints, supported 860×600 minimum, wrapping, truncation,
  hover disclosure, separate scroll regions, and persisted accepted outer size.
- Added 25 Hz visible repaint, transport generation/freshness, correct beat-within-bar, tempo
  increment/loop/pre-roll projection, and last intra-block transport event forwarding.
- Added floating-first child GUI negotiation with an independent top-level Win32 embedded fallback;
  outer editor destruction no longer owns child HWND lifetime.
- Added nested host core, GUI, params, state, latency, timer, thread-check, and log callbacks with a
  bounded main-thread event bridge.
- Added aggregate child latency and child-produced parameter feedback.

## Proposal delivery

`MixPlan` remains a non-destructive preview. EQ/compressor operations compile against the assigned
public parameter manifest into a revision-bound patch with explicit IDs, ranges, confidence, and
previous values. Required missing or ambiguous mappings reject the entire transaction.

Apply uses a bounded lock-free queue. Active audio applies at the block boundary; stopped audio uses
the outer CLAP parameter flush callback and forwards it to child `params.flush`. Acknowledgements
record actual prior values, update UI state, capture child state, and mark the project dirty. Undo
inverts the acknowledged patch. Child output parameter events update feedback and can verify the
result.

## Composed boundaries

The work was split into `session`, `patch`, `parameter_control`, `native_host`, and `child_window`
modules instead of adding new concerns to a single facade. Existing native/editor facades remain
long because they are lifecycle adapters, but cross-cutting policies are behind small public
interfaces. A general asynchronous project command bus was intentionally not introduced: egui and
CLAP editor callbacks already execute on the main-thread boundary, so one lock-owned document
transaction plus explicit structural commit intent removes the race with less machinery.

## Verification

- workspace tests: 32 passed, zero failed;
- minimum and wide egui layouts: headless render passed;
- native fake: descriptor/parameter/audio/state passed, 0.5× gain produced ±0.125;
- child GUI fake: 420×120 create/show/hide/destroy/recreate passed;
- outer → nested fake: revisioned state load, child state restore, audio, and save passed;
- format, warnings-denied clippy, release build, and final smoke evidence are recorded in
  `progress.md` after the closing gate;
- no live Codex process, task, or test was run.

## Honest remaining boundary

Specialized semantic compilers currently cover EQ and compressor. Saturation, reverb, limiter, and
multiband compression remain hostable graph classes but require deliberate recipes before automated
application. Public CLAP metadata cannot eliminate vendor-specific parameter ambiguity, bus/layout,
or window behavior. FL Studio and proprietary plugins therefore remain the manual matrix in
`testing-runbook.md`, not an automated compatibility claim.
