# Ghost Agent Host — Testing Runbook

All automated commands are local and deterministic. None launches Codex or creates a live Codex
task.

## Automated quality gate

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --release -p ghost-clap-plugin -p ghost-fakes
```

Expected at this revision: 32 tests, zero failures. The tests cover transport generation and fields,
beat-within-bar, bounded capture, graph lifecycle, nested host callback routing, strict patch
compilation, rejection of incomplete mapping, undo inversion, session reuse after transient editor
destruction, atomic mixed bypass/parameter acceptance, and rendering at 860×600 and 1180×760.

## Native fake audio/state/parameter smoke

This exercises public parameter inspection, an explicit active `params.flush`, audio processing,
state save/load, and clean destruction.

```powershell
cargo build -p ghost-fakes
cargo run -q -p ghost-cli -- native-smoke `
  --path target/debug/ghost_fakes.dll `
  --plugin-id ai.konko.ghost.fake-child `
  --parameter-id 1 `
  --parameter-value 0.5
```

Expected:

```json
{"first_output":[0.125,-0.125],"parameters":1,"plugin":"Ghost Fake CLAP Child","state_bytes":8}
```

## Native child GUI lifecycle smoke

```powershell
cargo run -q -p ghost-cli -- clap-gui-smoke `
  --path target/debug/ghost_fakes.dll `
  --plugin-id ai.konko.ghost.fake-child
```

Expected: `embedded GUI 420×120 passed`. This exercises create/parent/show/hide/destroy/recreate.
The production outer plugin additionally selects floating-first and uses a dedicated top-level
container for embedded fallback.

## Outer → nested state/audio smoke

```powershell
$fake = (Resolve-Path 'target/release/ghost_fakes.dll').Path
$outer = (Resolve-Path 'target/release/ghost_clap_plugin.dll').Path
$plugin = @{
  path = $fake
  plugin_id = 'ai.konko.ghost.fake-child'
  name = 'Ghost Fake CLAP Child'
  public_parameters = @()
  state = @{ format = 'clap.state/1'; bytes = @(0,0,0,0,0,0,224,63) }
}
$node = @{ id = 'fake-1'; class = 'equalizer'; bypassed = $false; plugin = $plugin }
$state = @{
  schema_version = 'ghost.ui-state/3'
  graph_revision = 7
  graph = @{ nodes = @($node) }
} | ConvertTo-Json -Depth 10 -Compress
cargo run -q -p ghost-cli -- clap-audio-smoke `
  --path $outer `
  --plugin-id ai.konko.ghost-agent-host `
  --state-json $state
```

Expected first output is `[0.125,-0.125]`; the current release payload is 692 bytes. Serialized size
varies with absolute child path and build location. This verifies outer state load, revisioned graph
activation, child restore, nested audio, and outer save.

## Manual FL Studio and proprietary-plugin acceptance

1. Install the release outer plugin and rescan it in FL Studio.
2. Insert it on stereo material. Verify sample rate/block presentation follows DAW activation.
3. Resize to 860×600, intermediate widths, and wide. Verify the workflow stacks below the
   breakpoint; long plugin/status/parameter text truncates with hover disclosure.
4. Close/reopen the outer editor during scan and proposal work. Verify results/status/selection are
   retained; a rescan may reorder results without changing the selected identity.
5. Edit topology and observe `rActive → rPending` until reactivation. Save/load while active and
   confirm audio eventually matches the newly loaded revision.
6. Capture Input, post-node, and Output. Verify fallback is labeled Output when a requested edge is
   inactive.
7. During playback, verify BPM/bar/beat advances and beat is within bar. Stop long enough to see
   stale rather than a falsely live value. Exercise tempo ramps, loop, and pre-roll if available.
8. Open child UI. Record whether floating mode or detached embedded fallback was negotiated. Close
   and reopen the outer editor: the child remains usable. Exercise child resize/show/hide and close
   repeatedly.
9. Trigger child GUI parameter changes and confirm feedback updates. Save/reopen and verify child
   state persists.
10. Produce an EQ/compressor proposal. Confirm it says Preview. Review target IDs/ranges/confidence,
    Apply, verify audible/control movement and Applied/Verified state, then Undo.
11. Repeat Apply while transport is stopped to exercise the outer/child `params.flush` path.
12. Create missing/ambiguous mappings and confirm Apply is disabled with explicit issues; no subset
    changes silently.
13. Exercise a plugin that requests timers, main-thread callbacks, state dirty, latency change, and
    param rescan where possible. Confirm no dead UI or stale outer latency.
14. Save, close, and reopen the DAW project. Confirm graph order, assignment, bypass, editor size,
    prompt/profile, and child state return.
15. Level-match bypass and processed audio and check discontinuities, bus layouts, and latency
    compensation.

Record DAW version, vendor/plugin/version, floating/embedded mode, bus layout, pass/fail, logs, and
reproduction steps in `progress.md`. Deterministic fake success is not proprietary acceptance.

## Realtime review checklist

For changes to process, transport, capture, parameter delivery, or child output handling, verify:

- no mutex, filesystem, serialization, GUI, agent, or diagnostic allocation is introduced;
- buffers and queue capacity are prepared before processing;
- failure uses static/bounded handoff and remains observable off the audio thread;
- revision mismatch rejects stale commands; and
- process and `params.flush` cannot run concurrently under the CLAP host contract.
