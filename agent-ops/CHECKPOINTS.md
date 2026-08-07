# Reconciliation Checkpoints

The coordinator fills in commit SHAs as waves are reconciled. Agents must branch from exact SHAs, not moving branch names.

| Name | Commit SHA | Produced after |
|---|---|---|
| `OPS_SHA` | `063546ad256e5e6df369f1616452ea95a16da2f9` | This operations packet and current functional code are committed |
| `BASE_SHA` | `<unset>` | T00 baseline audit |
| `ADR_SHA` | `<unset>` | T01 |
| `SCAFFOLD_SHA` | `<unset>` | T02 |
| `PROTOCOL_SHA` | `<unset>` | T03 |
| `CONTRACTS_SHA` | `<unset>` | T04–T07 reconciliation |
| `IMPLEMENTATION_SHA` | `<unset>` | T08–T12 reconciliation |
| `CLI_SHA` | `<unset>` | T13 |
| `AGENTD_MIGRATED_SHA` | `<unset>` | T14 |
| `AGENTD_HARDENED_SHA` | `<unset>` | T15 |
| `DAEMON_CLIENT_SHA` | `<unset>` | T16 |
| `EGUI_PROVIDER_SHA` | `<unset>` | T17 |
| `SVELTE_SHA` | `<unset>` | T18 |
| `SVELTE_INTEGRATION_SHA` | `<unset>` | T18 reconciled onto the implementation line for T19 |
| `WEBVIEW_PROVIDER_SHA` | `<unset>` | T19 |
| `PLUGIN_CLIENT_BASE_SHA` | `<unset>` | `DAEMON_CLIENT_SHA` and `EGUI_PROVIDER_SHA` reconciled for T20 |
| `PLUGIN_FRONTEND_SHA` | `<unset>` | T20 |
| `EDITOR_PROVIDERS_BASE_SHA` | `<unset>` | `PLUGIN_FRONTEND_SHA` and `WEBVIEW_PROVIDER_SHA` reconciled for T21 |
| `EDITOR_SELECTION_SHA` | `<unset>` | T21 |
| `CAPTURE_SHA` | `<unset>` | T22 |
| `CAPTURE_DAEMON_BASE_SHA` | `<unset>` | `CAPTURE_SHA` reconciled with the latest daemon/client line for T23 |
| `CAPTURE_FLOW_SHA` | `<unset>` | T23 |
| `CHILD_DISCOVERY_SHA` | `<unset>` | T24 |
| `CHILD_AUDIO_SHA` | `<unset>` | T25 |
| `PARAMETER_ADAPTER_SHA` | `<unset>` | T26 |
| `CHILD_RUNTIME_BASE_SHA` | `<unset>` | `CHILD_AUDIO_SHA` and `PARAMETER_ADAPTER_SHA` reconciled for T27 |
| `STATE_LATENCY_SHA` | `<unset>` | T27 |
| `CHILD_GUI_SHA` | `<unset>` | T28 |
