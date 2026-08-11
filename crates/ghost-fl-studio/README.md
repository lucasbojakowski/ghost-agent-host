# ghost-fl-studio

`ghost-fl-studio` is the transparent FL Studio/Gopher adapter for Ghost & Guild.

Its question is deliberately narrow:

> What does the live FL/Gopher interface expose, and how do we invoke it faithfully and reliably?

It does **not** decide what Ghost or an agent should be allowed to do. Track scopes, slot ranges, plugin allowlists, context selection, agent-facing descriptions, semantic parameter policy, verification strategy, and workflow mutation journals belong above this crate.

## Boundary

```text
app / caller policy
        |
        v
GopherNativeAdapter
- CDP target discovery
- live MCP catalog + schemas
- raw tool invocation
- live-schema argument ordering
- recursive callback normalization
- transport/native error distinction
- single-flight serialization
- secret-safe target logging
- faithful raw results
        |
        v
localhost WebView2 CDP
        |
        v
existing Gopher WebView
        |
        v
script_handler / native FL tools
```

The adapter does not depend on `ghost-codex` and does not construct a Codex `ToolRegistry`. A concrete app may expose every raw FL tool, expose a filtered subset, wrap selected calls with policy, or keep the adapter completely outside an agent tool surface.

## Runtime invariants

The following behavior is integration mechanism, not product policy, and is intentionally preserved here:

- Gopher `tools/call` arguments have been observed to behave positionally despite being represented as named JSON properties. The adapter fetches the live schema and serializes arguments in the schema/signature order.
- Callback payloads can be JSON-string encoded more than once, so catalog/result normalization peels recursive string layers.
- A successful transport callback can contain an inner native-tool failure; native errors are surfaced separately from transport failures.
- Calls remain single-flight because the observed Gopher callback does not provide dependable call correlation.
- Target URLs are not logged because current Gopher URLs can contain session/token material.
- FL Studio can change independently while Ghost is running. Adapter and agent observations are snapshots; FL itself remains current truth.
- Third-party normalized parameter state and human-readable display text are not guaranteed to settle together. Display text may lag or be unavailable.

## Live catalog is authoritative

Do not add a manually curated capability enum that narrows or reinterprets the live API unless it is mechanically faithful and cannot drift from the catalog.

Inspect the live catalog through the repository probe:

```text
cargo run -p fl-gopher-probe -- catalog
```

Invoke an exact tool after inspecting its schema:

```text
cargo run -p fl-gopher-probe -- call <exact-tool-name> --arguments '{}'
```

## Experimental status

The Gopher/WebView host interface is undocumented and version-sensitive. This crate attaches to the existing Gopher WebView; it does not inject into FL Studio, replace Gopher's DOM, or automate mouse/keyboard UI.

See `docs/decisions/001-transparent-fl-studio-adapter.md` for the accepted architectural decision and `docs/WINDOWS_FL_LIVE_VALIDATION.md` for the current live regression procedure.
