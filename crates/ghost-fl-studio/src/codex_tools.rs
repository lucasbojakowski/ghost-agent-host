use std::sync::Arc;

use ghost_codex::{ToolDefinition, ToolError, ToolRegistry};
use serde_json::{json, Value};

use crate::adapter::GopherNativeAdapter;

#[derive(Debug, Clone, Copy)]
pub struct FlAgentToolPolicy {
    pub inspect_session: bool,
    pub read_tempo: bool,
    pub set_tempo: bool,
    pub transport: bool,
}

impl FlAgentToolPolicy {
    pub const fn read_only() -> Self {
        Self {
            inspect_session: true,
            read_tempo: true,
            set_tempo: false,
            transport: false,
        }
    }

    pub const fn tempo_read_only() -> Self {
        Self {
            inspect_session: false,
            read_tempo: true,
            set_tempo: false,
            transport: false,
        }
    }

    pub const fn tempo_smoke() -> Self {
        Self {
            inspect_session: false,
            read_tempo: true,
            set_tempo: true,
            transport: false,
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
                        "bpm": {
                            "type": "integer",
                            "minimum": 20,
                            "maximum": 300,
                            "description": "Requested project tempo in BPM."
                        }
                    },
                    "required": ["bpm"],
                    "additionalProperties": false
                }),
            },
            move |arguments| {
                let bpm = required_u32(&arguments, "bpm")?;
                if !(20..=300).contains(&bpm) {
                    return Err(ToolError("fl_set_tempo smoke scope permits 20..=300 BPM".into()));
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

    Ok(())
}

fn empty_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "required": [],
        "additionalProperties": false
    })
}

fn required_u32(arguments: &Value, key: &str) -> Result<u32, ToolError> {
    let value = arguments
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError(format!("missing or invalid integer `{key}`")))?;
    u32::try_from(value).map_err(|_| ToolError(format!("`{key}` is out of range")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_required_integer() {
        assert_eq!(required_u32(&json!({"bpm": 137}), "bpm").unwrap(), 137);
        assert!(required_u32(&json!({"bpm": "137"}), "bpm").is_err());
    }
}
