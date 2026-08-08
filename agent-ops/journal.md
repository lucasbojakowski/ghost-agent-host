# Engineering Journal

## 2026-08-07

The repo is small enough to reshape without a compatibility maze, but analysis already has useful
fixture coverage. The safest migration is additive contracts plus narrow compatibility façades,
followed by moving workflow-specific code outward. The most important risk is pretending that a
CLAP child host is validated merely because its abstraction and fake implementation work. Native
CLAP loading, extension negotiation, GUI parenting, and FL Studio lifecycle require real plugin/DAW
validation and will be documented as such.

T01 requires UI-agnostic ownership and daemon compatibility. The new design therefore treats UI as
a projection of serializable state/actions and keeps service requests correlated/versioned. The
user's broader prompt adds generic task/context/processor composition; those become the deeper core
under T01's application boundaries.

The implementation deliberately stopped short of labeling native nested hosting production-ready.
Clack already provides useful descriptor and GUI smoke primitives, but a correct native child
adapter must negotiate audio ports, parameter events, state extensions, latency, and main/audio
thread handoff against actual binaries. The generic `ChildProcessor` contract and loadable fake make
that remaining work bounded and testable rather than entangling it with agents or UI.

## 2026-08-07 — Redesign 02

The screenshot made four couplings visible: one receiver controlled unrelated animations, one body
rendered all tabs, nested scroll areas lacked stable identity, and the presentation projected a
fixed graph that did not exist in the runtime. Treating each as a widget bug would have preserved
the wrong ownership. The new state machine separates work stages and the graph is now the source for
both nodes and taps.

Clack's split lifecycle was the key to real nested hosting. A combined native session is convenient
for CLI validation but cannot be moved into an outer audio processor because `PluginInstance` is
correctly non-Send. Splitting main/audio halves lets the outer CLAP prepare on activation, process on
the audio thread, and reunite on deactivation without unsafe thread claims.

Dynamic graph loading is safest as a DAW restart request. Swapping instances through a mutex would
violate callback invariants and make destruction thread-ambiguous. Immediate bypass is small enough
for an atomic mask. The same principle led to selected-edge capture: fixed banks and precomputed
keys, with explicit Output fallback when an edge is not active.

The native fake exposed a useful testing inconsistency. Its trait sibling claimed GUI lifecycle, but
the actual CLAP binary was headless. Adding a tiny Win32 editor, params, and state produced a much
stronger end-to-end boundary and answered the user's question: the missing UI was intentional in the
old fake binary, but insufficient for the new acceptance target.

The nested outer smoke restored a 0.5× child state and produced `[0.125, -0.125]` from
`[0.25, -0.25]`, then saved 680 bytes of outer release project state. This is the first automated proof in
the repo that audio actually crosses the outer → child ABI boundary.

The closing size audit found two older aggregation files above the intended module boundary. Moving
spectral and dynamic analysis behind private sibling modules, and moving the mock agent and Codex
tests behind their own modules, preserved the public facade while making ownership easier to see.

## 2026-08-07 — Runtime coherence tranche begins

The follow-up defect report is architectural and matches the code. The highest-leverage first move is
not a collection of widget patches; it is to make ownership visible. The current `GhostUi::show`
transaction holds no lock between clone and replacement, so every concurrent main-thread write can
be erased. I will preserve immediate-mode ergonomics by rendering against a lock-owned document and
collecting structural commit intent, then trigger the host only after the revision is committed.

The second seam is lifetime. Background receivers and their results currently die with the window,
while native children live with the outer plugin main thread. A persistent session should bridge new
`GhostUi` instances without serializing transient scan and analysis data into DAW projects.

The child hosting work should avoid pretending every plugin can embed. CLAP's floating path is the
cleanest independent lifetime. The fallback needs a real host-owned top-level container, not the
outer editor child. Callback coverage and timer dispatch will be built around a small nested-host
bridge so `ghost-host` does not depend on the outer plugin crate.

Finally, proposal application must be a transaction, not a button wired directly to parameter names.
The mapping compiler will record why each concrete parameter was chosen, bind to a graph revision,
and reject incomplete mappings by default. The realtime side will accept only prevalidated bounded
commands and return bounded acknowledgements.

The smallest reliable ownership repair was to make each egui pass a single locked document
transaction. A universal action enum would add plumbing without changing thread ownership because
the editor already runs on the CLAP main thread. The important operation is the final structural
commit: revision and pending state become observable before restart is requested.

Stopped playback exposed a less obvious CLAP affordance. `request_process` is only a fallback; a
host may not produce blocks. The outer plugin therefore needs a zero-parameter `clap.params`
extension so it can request and receive `params.flush`, then forward pending events to active child
audio handles. When inactive, the same callback lands on the main-thread implementation and uses
the child's inactive flush API.

Detached child UI is a window-ownership issue, not a reopen flag. Floating-first removes the host
container when the plugin supports it; embedded fallback needs its own top-level Win32 lifetime.
That makes outer editor close/reopen irrelevant to the child HWND and turns `closed(true)` into a
real lifecycle acknowledgement instead of stale bookkeeping.

Parameter feedback is useful twice: it reports child-GUI changes and verifies agent-applied values.
Undo must use the actual previous value returned by the application boundary, not only the value
observed when the proposal was compiled.
