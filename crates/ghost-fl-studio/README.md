# ghost-fl-studio

Experimental FL Studio adapter for Ghost & Guild.

## Boundary

`GopherNativeAdapter` owns the external control path:

```text
Ghost application / Codex dynamic tools
              |
              v
      GopherNativeAdapter
      - live capability manifest
      - single-flight calls
      - schema validation
      - canonical argument ordering
      - structured native errors
      - mutation journal
      - readback verification
              |
              v
       localhost CDP
              |
              v
 existing Gopher WebView
              |
              v
 script_handler.runJson
              |
              v
    FL Studio native tools
```

This remains an experimental adapter because the Gopher/WebView host bridge is undocumented and version-sensitive. The adapter never replaces the Gopher DOM and does not use DLL/process injection or UI automation.

## Important runtime invariant

FL Studio 26.1.3 Gopher was observed to bind `tools/call.params.arguments` in JSON property order. The adapter therefore fetches the live MCP catalog and rebuilds every call in the schema/signature order before serializing it.

The Gopher callback surface also has no usable correlation ID, so Ghost serializes native calls through one mutex. Do not use Gopher's own agent concurrently with Ghost while this experimental adapter owns the link.

## Codex tool exposure

`register_codex_tools` deliberately exposes a scoped Ghost tool surface instead of passing all native FL tools directly to the model. Current policies include a read-only context/tempo set and a bounded tempo smoke policy.

Mutating wrappers perform native readback and append an in-memory `MutationRecord` containing before/after state and verification status.

## Live Codex smoke test

The `ghost-fl-agent-smoke` app gives Codex exactly two dynamic tools:

- `fl_get_tempo`
- `fl_set_tempo`

The agent is instructed to read the current tempo and set a requested BPM. `fl_set_tempo` executes through `GopherNativeAdapter`, reads FL Studio back, verifies the native mutation, and records it. The smoke app then restores the original integer BPM unless `--keep-change` is supplied.

Example:

```powershell
cargo test -p ghost-fl-studio
cargo run -p ghost-fl-agent-smoke -- --target-bpm 137 --codex-binary codex --model gpt-5.6-terra
```

Prerequisites:

1. FL Studio is running with the WebView2 CDP debugging port enabled (the existing `ghost-fl-native-bridge --launch probe` flow can establish this).
2. Gopher is open/available in FL Studio.
3. The original project tempo is an integer if you want automatic restoration.
4. Codex App Server is available through the configured binary.

A successful run ends with a `GREEN` line proving this path:

```text
Codex -> Ghost dynamic tool -> GopherNativeAdapter -> FL Studio mutation -> native readback -> restoration
```
