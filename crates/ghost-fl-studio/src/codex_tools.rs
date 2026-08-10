use std::sync::Arc;

use ghost_codex::{ToolDefinition, ToolError, ToolRegistry};
use serde_json::{json, Value};

use crate::adapter::GopherNativeAdapter;

const MAX_PARAMETER_SEARCH_MATCHES: usize = 64;

#[derive(Debug, Clone)]
pub struct FlPluginWriteScope {
    pub target_track: String,
    pub slot_start: u32,
    pub slot_end: u32,
    pub allowed_plugins: Vec<String>,
}

impl FlPluginWriteScope {
    pub fn new(
        target_track: impl Into<String>,
        slot_start: u32,
        slot_end: u32,
        allowed_plugins: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            target_track: target_track.into(),
            slot_start,
            slot_end,
            allowed_plugins: allowed_plugins.into_iter().map(Into::into).collect(),
        }
    }

    fn allows_slot(&self, slot: u32) -> bool {
        slot >= self.slot_start && slot <= self.slot_end
    }

    fn canonical_plugin(&self, requested: &str) -> Option<&str> {
        self.allowed_plugins
            .iter()
            .find(|plugin| plugin.eq_ignore_ascii_case(requested))
            .map(String::as_str)
    }
}

#[derive(Debug, Clone)]
pub struct FlAgentToolPolicy {
    pub inspect_session: bool,
    pub read_tempo: bool,
    pub set_tempo: bool,
    pub transport: bool,
    pub plugin_write_scope: Option<FlPluginWriteScope>,
}

impl FlAgentToolPolicy {
    pub fn read_only() -> Self {
        Self {
            inspect_session: true,
            read_tempo: true,
            set_tempo: false,
            transport: false,
            plugin_write_scope: None,
        }
    }

    pub fn tempo_smoke() -> Self {
        Self {
            inspect_session: false,
            read_tempo: true,
            set_tempo: true,
            transport: false,
            plugin_write_scope: None,
        }
    }

    pub fn single_track_processor(scope: FlPluginWriteScope) -> Self {
        Self {
            inspect_session: true,
            read_tempo: true,
            set_tempo: false,
            transport: false,
            plugin_write_scope: Some(scope),
        }
    }
}

pub fn register_codex_tools(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    policy: FlAgentToolPolicy,
) -> Result<(), ToolError> {
    if policy.inspect_session {
        let inspect_adapter = Arc::clone(&adapter);
        registry.register(
            ToolDefinition {
                name: "fl_get_session_context".into(),
                description: "Read the current FL Studio project/session context through the connected native adapter. This tool does not modify the project.".into(),
                input_schema: empty_schema(),
            },
            move |_| {
                let result = inspect_adapter
                    .session_context()
                    .map_err(|error| ToolError(error.to_string()))?;
                Ok(json!({
                    "tool": result.tool,
                    "content": result.content_text,
                    "raw": result.raw
                }))
            },
        )?;
    }

    if policy.read_tempo {
        let read_tempo_adapter = Arc::clone(&adapter);
        registry.register(
            ToolDefinition {
                name: "fl_get_tempo".into(),
                description: "Read the current FL Studio project tempo in BPM. This tool does not modify the project.".into(),
                input_schema: empty_schema(),
            },
            move |_| {
                let bpm = read_tempo_adapter
                    .get_tempo()
                    .map_err(|error| ToolError(error.to_string()))?;
                Ok(json!({"bpm": bpm}))
            },
        )?;
    }

    if policy.set_tempo {
        let set_tempo_adapter = Arc::clone(&adapter);
        registry.register(
            ToolDefinition {
                name: "fl_set_tempo".into(),
                description: "Set the FL Studio project tempo to an integer BPM, then read it back and verify the change. Use only when the user explicitly requested the tempo change.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "bpm": {"type": "integer", "minimum": 20, "maximum": 300}
                    },
                    "required": ["bpm"],
                    "additionalProperties": false
                }),
            },
            move |arguments| {
                let bpm = required_u32(&arguments, "bpm")?;
                if !(20..=300).contains(&bpm) {
                    return Err(ToolError("fl_set_tempo scope permits 20..=300 BPM".into()));
                }
                let mutation = set_tempo_adapter
                    .set_tempo_verified(bpm)
                    .map_err(|error| ToolError(error.to_string()))?;
                serde_json::to_value(mutation).map_err(|error| ToolError(error.to_string()))
            },
        )?;
    }

    if policy.transport {
        let play_adapter = Arc::clone(&adapter);
        registry.register(
            ToolDefinition {
                name: "fl_play".into(),
                description: "Start playback in FL Studio.".into(),
                input_schema: empty_schema(),
            },
            move |_| {
                play_adapter
                    .play()
                    .map(|result| json!({"content": result.content_text}))
                    .map_err(|error| ToolError(error.to_string()))
            },
        )?;

        let stop_adapter = Arc::clone(&adapter);
        registry.register(
            ToolDefinition {
                name: "fl_stop".into(),
                description: "Stop playback in FL Studio.".into(),
                input_schema: empty_schema(),
            },
            move |_| {
                stop_adapter
                    .stop()
                    .map(|result| json!({"content": result.content_text}))
                    .map_err(|error| ToolError(error.to_string()))
            },
        )?;
    }

    if let Some(scope) = policy.plugin_write_scope {
        register_processor_tools(registry, adapter, scope)?;
    }

    Ok(())
}

fn register_processor_tools(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    scope: FlPluginWriteScope,
) -> Result<(), ToolError> {
    if scope.slot_start == 0 || scope.slot_end < scope.slot_start || scope.slot_end > 10 {
        return Err(ToolError("processor write scope must use FL mixer slots 1..=10".into()));
    }
    if scope.allowed_plugins.is_empty() {
        return Err(ToolError("processor write scope must allow at least one plugin".into()));
    }

    let target = scope.target_track.clone();
    let allowed = scope.allowed_plugins.join(", ");
    let add_scope = scope.clone();
    let add_adapter = Arc::clone(&adapter);
    registry.register(
        ToolDefinition {
            name: "fl_add_effect".into(),
            description: format!(
                "Insert one effect on FL Studio mixer track {}. Writes are restricted to empty slots {}..={} and these exact plugin names: {}. Do not replace an existing effect.",
                target, scope.slot_start, scope.slot_end, allowed
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "plugin": {"type": "string", "description": format!("One of: {allowed}")},
                    "slot_number": {"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end}
                },
                "required": ["plugin", "slot_number"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&add_scope, slot)?;
            let requested = required_str(&arguments, "plugin")?;
            let plugin = add_scope
                .canonical_plugin(requested)
                .ok_or_else(|| ToolError(format!("plugin `{requested}` is outside the allowed processor scope")))?;
            let result = add_adapter
                .add_effect_verified(plugin, &add_scope.target_track, slot)
                .map_err(|error| ToolError(error.to_string()))?;
            serde_json::to_value(result).map_err(|error| ToolError(error.to_string()))
        },
    )?;

    let search_scope = scope.clone();
    let search_adapter = Arc::clone(&adapter);
    registry.register(
        ToolDefinition {
            name: "fl_find_plugin_parameters".into(),
            description: format!(
                "Search the published parameter manifest of the plugin on mixer track {}, restricted to processor slots {}..={}. Returns at most {} matching lines so large third-party parameter surfaces do not flood the agent context. Search for semantic terms such as frequency, gain, Q, threshold, ratio, attack, release, mix, bypass, or output.",
                scope.target_track,
                scope.slot_start,
                scope.slot_end,
                MAX_PARAMETER_SEARCH_MATCHES
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "slot_number": {"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end},
                    "query": {"type": "string", "minLength": 1}
                },
                "required": ["slot_number", "query"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&search_scope, slot)?;
            let query = required_str(&arguments, "query")?.trim().to_lowercase();
            let result = search_adapter
                .plugin_parameter_list(&search_scope.target_track, slot)
                .map_err(|error| ToolError(error.to_string()))?;
            let text = result.content_text.join("\n");
            let mut matches = Vec::new();
            let mut total_matches = 0_usize;
            for line in text.lines() {
                if line.to_lowercase().contains(&query) {
                    total_matches += 1;
                    if matches.len() < MAX_PARAMETER_SEARCH_MATCHES {
                        matches.push(line.to_owned());
                    }
                }
            }
            Ok(json!({
                "query": query,
                "matchCount": total_matches,
                "returned": matches.len(),
                "truncated": total_matches > matches.len(),
                "matches": matches
            }))
        },
    )?;

    let read_scope = scope.clone();
    let read_adapter = Arc::clone(&adapter);
    registry.register(
        ToolDefinition {
            name: "fl_get_plugin_parameter_value".into(),
            description: "Read one plugin parameter as the FL/Gopher normalized 0..1 value. Use the exact parameter identifier discovered with fl_find_plugin_parameters.".into(),
            input_schema: parameter_schema(&scope, false),
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&read_scope, slot)?;
            let parameter = required_str(&arguments, "param_identifier")?;
            let value = read_adapter
                .plugin_parameter_value(&read_scope.target_track, parameter, slot)
                .map_err(|error| ToolError(error.to_string()))?;
            Ok(json!({"normalizedValue": value}))
        },
    )?;

    let write_scope = scope;
    let write_adapter = adapter;
    registry.register(
        ToolDefinition {
            name: "fl_set_plugin_parameter_value".into(),
            description: "Set one published plugin parameter using a normalized 0..1 value and verify native readback. Only make small, purposeful changes whose semantic meaning is clear from fl_find_plugin_parameters; do not guess parameter identifiers or opaque normalized mappings.".into(),
            input_schema: parameter_schema(&write_scope, true),
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&write_scope, slot)?;
            let parameter = required_str(&arguments, "param_identifier")?;
            let value = arguments
                .get("value")
                .and_then(Value::as_f64)
                .ok_or_else(|| ToolError("missing or invalid number `value`".into()))?;
            if !(0.0..=1.0).contains(&value) {
                return Err(ToolError("`value` must be normalized to 0..=1".into()));
            }
            let result = write_adapter
                .set_plugin_parameter_verified(
                    &write_scope.target_track,
                    parameter,
                    value,
                    slot,
                )
                .map_err(|error| ToolError(error.to_string()))?;
            serde_json::to_value(result).map_err(|error| ToolError(error.to_string()))
        },
    )?;

    Ok(())
}

fn parameter_schema(scope: &FlPluginWriteScope, write: bool) -> Value {
    let mut properties = serde_json::Map::<String, Value>::new();
    properties.insert(
        "slot_number".into(),
        json!({"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end}),
    );
    properties.insert("param_identifier".into(), json!({"type": "string"}));
    let mut required = vec![json!("slot_number"), json!("param_identifier")];
    if write {
        properties.insert(
            "value".into(),
            json!({"type": "number", "minimum": 0.0, "maximum": 1.0}),
        );
        required.push(json!("value"));
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn ensure_slot(scope: &FlPluginWriteScope, slot: u32) -> Result<(), ToolError> {
    if scope.allows_slot(slot) {
        Ok(())
    } else {
        Err(ToolError(format!(
            "slot {slot} is outside the allowed range {}..={} for mixer track {}",
            scope.slot_start, scope.slot_end, scope.target_track
        )))
    }
}

fn empty_schema() -> Value {
    json!({"type": "object", "properties": {}, "required": [], "additionalProperties": false})
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_required_integer() {
        assert_eq!(required_u32(&json!({"bpm": 137}), "bpm").unwrap(), 137);
        assert!(required_u32(&json!({"bpm": "137"}), "bpm").is_err());
    }

    #[test]
    fn processor_scope_rejects_other_plugins_and_slots() {
        let scope = FlPluginWriteScope::new("1", 1, 4, ["Pro-Q 4", "Pro-C 3"]);
        assert!(scope.allows_slot(1));
        assert!(!scope.allows_slot(5));
        assert_eq!(scope.canonical_plugin("pro-q 4"), Some("Pro-Q 4"));
        assert_eq!(scope.canonical_plugin("Fruity Limiter"), None);
    }
}
