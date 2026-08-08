# Ghost Agent Host — Runtime Configuration

## Build artifacts

```powershell
cargo build --release -p ghost-clap-plugin -p ghost-fakes
```

- `target/release/ghost_clap_plugin.dll` is the outer plugin. Copy/rename it to
  `Ghost Agent Host.clap` in a scanned CLAP folder.
- `target/release/ghost_fakes.dll` is the deterministic acceptance child. Rename it to
  `Ghost Fake Child.clap` only for testing.

The scanner searches standard Windows CLAP roots and `CLAP_PATH`, recursively to depth eight.
Descriptor and public `clap.params` inspection always occurs off the audio thread.

## Project and editor state

The serialized schema is `ghost.ui-state/3`. It stores graph revision/topology, child assignment and
state blobs, prompt/profile, selected capture tap, scanner visibility, and the last accepted editor
size. Versions 1 and 2 migrate automatically. Minimum editor size is 860×600; default is 1180×760.
The host may remember size too.

Discovery results, scan/job receivers, selection, capture, analysis, proposal, status, parameter
feedback, and active/pending revisions are `UiSession` data. They survive closing and reopening the
editor but are intentionally not serialized into the DAW project. Discovery selection uses
canonical path plus plugin ID rather than a vector index.

## Audio, transport, and capture

Ghost declares one main stereo input/output and uses DAW activation sample rate and block limits.
The editor shows transport at a bounded 25 Hz cadence. A publication older than 750 ms is labeled
stale; absent transport is unavailable. Beat display is one-based within the current bar.

DAW capture accepts 0.5–24 seconds subject to the fixed 1,152,000-frame buffer. At 48 kHz that is
24 seconds; at 192 kHz it is 6 seconds. Input, any active post-node edge, and Output are selectable.
File capture uses Symphonia on a background worker.

## Graph and revisions

Routing supports Empty, EQ, Compressor, and EQ + compressor presets plus Equalizer, Compressor,
Saturation, Reverb, Limiter, and Multiband compressor node classes. EQ and compressor currently have
specialized proposal compilers. Other classes remain valid hosting topology but do not receive
unsupported semantic operations.

Create/remove/reorder/reclassify/assign commits a new graph revision before requesting restart.
The header shows `rActive → rPending` while the DAW has not yet activated the committed graph.
Bypass is immediate through an atomic mask and is included in Apply/Undo transactions.

## Proposal Apply policy

Propose creates a preview only. Apply is enabled only when:

- the preview revision equals the document revision;
- exactly one assigned target exists for each processor role;
- every required semantic field maps to a writable public parameter;
- values are within public ranges; and
- the patch contains no more than 32 parameter changes.

Mapping uses normalized names/aliases and units inferred with the child's `value_to_text` where
available. The review displays concrete parameter IDs, current/previous values, values to apply, and
mapping confidence. Incomplete mappings fail closed. Apply is delivered at the next audio block or
through CLAP `params.flush` while stopped. Acknowledgements trigger child-state capture and project
dirty notification. Undo uses acknowledged previous values.

## Child windows and compatibility

Ghost prefers plugin-owned floating Win32 editors. If unsupported, it creates a separate top-level
host window and embeds the child there. Closing the outer Ghost editor does not destroy these child
windows. Child show/hide/resize/closed callbacks and requested timers are serviced on the correct
boundary.

The nested host exposes core, GUI, params, state, latency, timer, thread-check, and logging support.
Aggregate child latency is exposed by the outer plugin. This materially improves compatibility but
does not prove any proprietary product: use the manual matrix in `testing-runbook.md` for each
vendor/version.

## Agent safety

The embedded editor uses `MockMixingAgent`. Automated checks never launch Codex or create a live
Codex task. CLI live-agent use remains an explicit operator choice through `--agent codex`.
