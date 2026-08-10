# Ghost FL Native Bridge (experiment)

Headless Rust probe for FL Studio 2026's Gopher WebView control surface.

This experiment intentionally **does not replace or navigate the Gopher panel**. It attaches to the existing Gopher WebView through Chrome DevTools Protocol (CDP), verifies that FL Studio's projected `script_handler` host object is reachable, requests the native MCP tool catalog, and can invoke one exact tool by name.

## Why

The normal FL Studio MIDI Controller Python API is a strong supported state/control surface, but it does not expose several project-construction actions we need (notably general plugin/channel insertion). FL Studio's experimental Gopher feature demonstrably has a deeper native tool surface. This probe answers one narrow question:

> Can Ghost reach that native tool surface directly from Rust without taking over Gopher's UI?

If the answer is yes on a registered FL Studio 2026 install, we can treat this as an experimental adapter while pursuing a supported Image-Line developer interface.

## Important status

- Experimental and based on an undocumented Gopher/WebView host bridge.
- Do not ship this as a production dependency without a supported Image-Line contract.
- Ghost and Gopher should not issue tool calls concurrently. The observed `onRunJson` callback does not provide a reliable correlation ID, so this probe deliberately serializes calls and assumes exclusive use while a call is in flight.
- The probe does not inject DLLs, hook FL Studio, replace Gopher's page, or automate mouse/keyboard UI.

## First probe

Close every running FL Studio process first so the WebView2 environment is created with the debugging flag.

From the repository root:

```powershell
cargo run -p ghost-fl-native-bridge -- --launch probe
```

The bridge checks the normal FL Studio 2026 install path. If necessary:

```powershell
cargo run -p ghost-fl-native-bridge -- --launch --fl "C:\Program Files\Image-Line\FL Studio 2026\FL64.exe" probe
```

When prompted, open Gopher in FL Studio (`Alt+F1`). The probe will:

1. wait for the WebView2 CDP endpoint on `127.0.0.1:9222`;
2. find the target whose title/URL contains `gopher`;
3. connect to that target's debugger WebSocket;
4. verify `script_handler` through either the direct global or `window.chrome.webview.hostObjects` projection;
5. set `script_handler.MCPTools = "1"` and await `window.flHelper.onMCPTools`;
6. print the native tool names and descriptions.

To print the complete schemas:

```powershell
cargo run -p ghost-fl-native-bridge -- --launch probe --raw
```

If FL is already running *with* the debugging environment, omit `--launch` and attach directly:

```powershell
cargo run -p ghost-fl-native-bridge -- probe --raw
```

## Calling one discovered tool

Only call tools after inspecting the exact catalog returned by the installed FL build.

```powershell
cargo run -p ghost-fl-native-bridge -- call <exact-tool-name> --args '{}'
```

For a tool that accepts arguments:

```powershell
cargo run -p ghost-fl-native-bridge -- call <exact-tool-name> --args '{"someField":123}'
```

The bridge sends the same JSON-RPC/MCP shape observed in Gopher:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "<tool-name>",
    "arguments": {}
  }
}
```

It temporarily wraps `window.flHelper.onRunJson`, restores the previous callback after the result/timeout, and leaves the Gopher DOM untouched.

## Expected next experiment

Once `probe --raw` works on the target machine, capture the catalog and test in this order:

1. one read-only project/session inspection tool;
2. transport play/stop if exposed;
3. one reversible mixer property change;
4. plugin/channel insertion only after the exact schema and target semantics are understood.

After that, move the CDP code behind a proper `FlNativeAdapter` capability boundary and add transaction journaling/read-back verification before exposing mutations to an agent.
