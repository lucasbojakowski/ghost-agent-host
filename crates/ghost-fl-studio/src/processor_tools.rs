use std::sync::Arc;

use ghost_codex::{ToolDefinition, ToolError, ToolRegistry};
use serde_json::{json, Value};

use crate::adapter::{AdapterError, GopherNativeAdapter, NativeToolResult};
use crate::codex_tools::{FlAgentToolPolicy, FlPluginWriteScope};

/// Product-facing FL tool registration.
///
/// The base registrar still owns the proven tempo/transport/parameter tools. For scoped processor
/// workflows we replace only the two operations that previously depended on parsing Gopher's large
/// `get_session_context` result: compact target context and safe effect insertion. Both replacements
/// inspect mixer effect slots directly with `get_plugin_parameter_list`, which is also the native
/// resolver used to verify inserted plugins.
pub fn register_codex_tools(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    policy: FlAgentToolPolicy,
) -> Result<(), ToolError> {
    let processor_scope = policy.plugin_write_scope.clone();
    crate::codex_tools::register_codex_tools(
        registry,
        Arc::clone(&adapter),
        policy,
    )?;

    if let Some(scope) = processor_scope {
        replace_processor_context_tool(registry, Arc::clone(&adapter), scope.clone())?;
        replace_add_effect_tool(registry, adapter, scope)?;
    }
    Ok(())
}

fn replace_processor_context_tool(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    scope: FlPluginWriteScope,
) -> Result<(), ToolError> {
    let description = format!(
        "Read a compact live view of mixer track {} across effect slots {}..={}. Ghost probes each slot directly through FL's plugin target resolver; it does not parse the large Gopher session-context text. This tool does not modify the project.",
        scope.target_track, scope.slot_start, scope.slot_end
    );
    registry.replace(
        ToolDefinition {
            name: "fl_get_target_track_context".into(),
            description,
            input_schema: empty_schema(),
        },
        move |_| scoped_track_context(&adapter, &scope),
    )
}

fn replace_add_effect_tool(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    scope: FlPluginWriteScope,
) -> Result<(), ToolError> {
    let allowed = scope.allowed_plugins.join(", ");
    let description = format!(
        "Insert one effect on FL Studio mixer track {}. Writes are restricted to slots {}..={} and these exact plugin names: {}. Ghost probes the requested slot immediately before insertion and refuses to overwrite any effect that already resolves there.",
        scope.target_track, scope.slot_start, scope.slot_end, allowed
    );
    let input_schema = json!({
        "type": "object",
        "properties": {
            "plugin": {"type": "string", "description": format!("One of: {allowed}")},
            "slot_number": {"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end}
        },
        "required": ["plugin", "slot_number"],
        "additionalProperties": false
    });

    registry.replace(
        ToolDefinition {
            name: "fl_add_effect".into(),
            description,
            input_schema,
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&scope, slot)?;
            let requested = required_str(&arguments, "plugin")?;
            let plugin = canonical_plugin(&scope, requested).ok_or_else(|| {
                ToolError(format!(
                    "plugin `{requested}` is outside the allowed processor scope"
                ))
            })?;

            match probe_effect_slot(&adapter, &scope, slot)? {
                EffectSlotProbe::Empty => {}
                EffectSlotProbe::Occupied { name } => {
                    return Err(ToolError(format!(
                        "refusing to insert `{plugin}` into mixer track {}, slot {slot}: `{name}` already resolves there",
                        scope.target_track
                    )));
                }
            }

            let result = adapter
                .add_effect_verified(plugin, &scope.target_track, slot)
                .map_err(|error| ToolError(error.to_string()))?;
            serde_json::to_value(result).map_err(|error| ToolError(error.to_string()))
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EffectSlotProbe {
    Empty,
    Occupied { name: String },
}

fn scoped_track_context(
    adapter: &GopherNativeAdapter,
    scope: &FlPluginWriteScope,
) -> Result<Value, ToolError> {
    validate_scope(scope)?;
    let tempo = adapter
        .get_tempo()
        .map_err(|error| ToolError(error.to_string()))?;
    let mut slot_states = Vec::new();
    let mut effects = Vec::new();

    for slot in scope.slot_start..=scope.slot_end {
        match probe_effect_slot(adapter, scope, slot)? {
            EffectSlotProbe::Empty => {
                slot_states.push(json!({"slot": slot, "occupied": false}));
            }
            EffectSlotProbe::Occupied { name } => {
                slot_states.push(json!({
                    "slot": slot,
                    "occupied": true,
                    "name": name
                }));
                effects.push(json!({"slot": slot, "name": name}));
            }
        }
    }

    Ok(json!({
        "contextSource": "direct_plugin_slot_probes",
        "tempoBpm": tempo,
        "targetMixerTrack": {
            "target": scope.target_track,
            "effect_plugins": effects
        },
        "effectSlots": slot_states,
        "writeScope": {
            "slotStart": scope.slot_start,
            "slotEnd": scope.slot_end,
            "allowedPlugins": scope.allowed_plugins
        }
    }))
}

fn probe_effect_slot(
    adapter: &GopherNativeAdapter,
    scope: &FlPluginWriteScope,
    slot: u32,
) -> Result<EffectSlotProbe, ToolError> {
    ensure_slot(scope, slot)?;
    match adapter.plugin_parameter_list(&scope.target_track, slot) {
        Ok(result) => {
            let manifest = native_content_text(&result).join("\n");
            let name = resolve_plugin_name(&manifest, &scope.allowed_plugins)
                .unwrap_or_else(|| "existing effect".to_owned());
            Ok(EffectSlotProbe::Occupied { name })
        }
        Err(AdapterError::NativeTool(message)) if is_empty_plugin_target_error(&message) => {
            Ok(EffectSlotProbe::Empty)
        }
        Err(error) => Err(ToolError(error.to_string())),
    }
}

fn is_empty_plugin_target_error(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains("could not resolve plugin target")
}

fn resolve_plugin_name(manifest: &str, allowed_plugins: &[String]) -> Option<String> {
    let lower = manifest.to_ascii_lowercase();
    allowed_plugins
        .iter()
        .find(|plugin| lower.contains(&plugin.to_ascii_lowercase()))
        .cloned()
}

fn native_content_text(result: &NativeToolResult) -> Vec<String> {
    if !result.content_text.is_empty() {
        return result.content_text.clone();
    }
    result
        .raw
        .pointer("/result/content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn validate_scope(scope: &FlPluginWriteScope) -> Result<(), ToolError> {
    if scope.slot_start == 0 || scope.slot_end < scope.slot_start || scope.slot_end > 10 {
        return Err(ToolError(
            "processor write scope must use FL mixer slots 1..=10".into(),
        ));
    }
    if scope.allowed_plugins.is_empty() {
        return Err(ToolError(
            "processor write scope must allow at least one plugin".into(),
        ));
    }
    Ok(())
}

fn ensure_slot(scope: &FlPluginWriteScope, slot: u32) -> Result<(), ToolError> {
    validate_scope(scope)?;
    if slot >= scope.slot_start && slot <= scope.slot_end {
        Ok(())
    } else {
        Err(ToolError(format!(
            "slot {slot} is outside the allowed range {}..={} for mixer track {}",
            scope.slot_start, scope.slot_end, scope.target_track
        )))
    }
}

fn canonical_plugin<'a>(scope: &'a FlPluginWriteScope, requested: &str) -> Option<&'a str> {
    scope
        .allowed_plugins
        .iter()
        .find(|plugin| plugin.eq_ignore_ascii_case(requested))
        .map(String::as_str)
}

fn required_u32(arguments: &Value, key: &str) -> Result<u32, ToolError> {
    let value = arguments
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError(format!("missing or invalid integer `{key}`")))?;
    u32::try_from(value).map_err(|_| ToolError(format!("`{key}` is out of range")))
}

fn required_str<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ToolError(format!("missing or invalid string `{key}`")))
}

fn empty_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "required": [],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_plugin_target_is_an_empty_slot_signal() {
        assert!(is_empty_plugin_target_error(
            "Error: Could not resolve plugin target '1' (Slot: 1)."
        ));
        assert!(!is_empty_plugin_target_error("Error: something else"));
    }

    #[test]
    fn resolves_known_plugin_name_from_parameter_manifest() {
        let allowed = vec!["Pro-Q 4".to_owned(), "Pro-C 3".to_owned()];
        assert_eq!(
            resolve_plugin_name(
                "Plugin Pro-C 3 parameter list\nIndex 1: Threshold",
                &allowed
            ),
            Some("Pro-C 3".to_owned())
        );
    }

    #[test]
    fn canonical_plugin_is_case_insensitive_and_scope_is_bounded() {
        let scope = FlPluginWriteScope::new("1", 1, 4, ["Pro-Q 4", "Pro-C 3"]);
        assert_eq!(canonical_plugin(&scope, "pro-q 4"), Some("Pro-Q 4"));
        assert!(ensure_slot(&scope, 1).is_ok());
        assert!(ensure_slot(&scope, 5).is_err());
    }
}
