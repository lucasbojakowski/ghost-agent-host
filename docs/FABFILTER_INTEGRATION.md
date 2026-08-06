# FabFilter Nested-Host Integration Plan

## Target matrix

Record and test each combination separately:

- Operating system.
- CPU architecture.
- DAW and DAW version.
- CLAP host behavior.
- Pro-Q 4 version and binary hash.
- Pro-C 3 version and binary hash.
- Sample rate.
- Maximum block size.
- Stereo and sidechain port arrangement.

## Required runtime capabilities

### Lifecycle

- Entry load and factory discovery.
- Exact descriptor selection.
- Instance creation.
- Main/audio/shared host handlers.
- Activation and deactivation.
- Start/stop processing.
- Restart, process, and callback requests.

### Processing

- Stereo f32 buffers first.
- Transport and steady-time forwarding.
- Input and output event buffers.
- Parameter flush outside active processing where required.
- Sample-accurate parameter events where supported.
- Latency and tail updates.

### State

- Save child-owned opaque state.
- Restore after re-instantiation.
- Confirm rendered output before and after restore.
- Keep version and binary hash with every state blob.
- Never parse undocumented state bytes.

### Parameters

- Enumerate ID, name, module, range, flags, default, value-to-text, and text-to-value behavior.
- Build a runtime manifest.
- Map semantic operations to a validated parameter set.
- Diff manifests after plugin updates.
- Reject unknown versions until fixture tests pass.

### Child GUI

- Create GUI only on the correct thread.
- Query supported window API.
- Create a separate owned child window first.
- Attach the plugin GUI to the platform parent.
- Handle scale, resize, focus, hide/show, and destruction.
- Do not destroy the processor when closing the window.

## Verification cases

1. Transparent default render.
2. Static bell EQ at known frequency/gain/Q.
3. Dynamic EQ event and state persistence.
4. Compressor threshold/ratio/attack/release mapping.
5. Bypass state.
6. Latency changes from processing modes.
7. Save, unload, reload, restore, and render comparison.
8. GUI edit reflected through parameter/state APIs.
9. Agent edit reflected in child GUI.
10. Missing plugin recovery.
11. Child activation failure.
12. DAW project duplication and instance independence.
