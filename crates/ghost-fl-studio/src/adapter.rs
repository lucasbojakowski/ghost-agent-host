use std::{
    collections::BTreeSet,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use crate::transport::{GopherConnection, TransportConfig};

pub const DEFAULT_DEBUG_PORT: u16 = 9222;

#[derive(Debug, Clone)]
pub struct FlStudioAdapterConfig {
    pub debug_port: u16,
    pub target_match: String,
    pub connect_timeout: Duration,
    pub bridge_timeout: Duration,
}

impl Default for FlStudioAdapterConfig {
    fn default() -> Self {
        Self {
            debug_port: DEFAULT_DEBUG_PORT,
            target_match: "gopher".into(),
            connect_timeout: Duration::from_secs(20),
            bridge_timeout: Duration::from_secs(20),
        }
    }
}

impl FlStudioAdapterConfig {
    fn transport(&self) -> TransportConfig {
        TransportConfig {
            debug_port: self.debug_port,
            target_match: self.target_match.clone(),
            connect_timeout: self.connect_timeout,
            bridge_timeout: self.bridge_timeout,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum FlCapability {
    InspectSession,
    ReadTempo,
    SetTempo,
    Play,
    Stop,
    InspectChannels,
    InsertChannel,
    InspectMixer,
    SetMixerLevel,
    SetMixerPan,
    SetMixerRouting,
    InsertEffect,
    RemoveEffect,
    InspectPluginParameters,
    SetPluginParameter,
    PianoRollScript,
}

impl FlCapability {
    pub fn required_tool(self) -> &'static str {
        match self {
            Self::InspectSession => "get_session_context",
            Self::ReadTempo => "get_tempo",
            Self::SetTempo => "set_tempo",
            Self::Play => "play",
            Self::Stop => "stop",
            Self::InspectChannels => "list_channel_names",
            Self::InsertChannel => "add_channel",
            Self::InspectMixer => "get_mixer_tracks_volume",
            Self::SetMixerLevel => "set_mixer_tracks_volume_db",
            Self::SetMixerPan => "set_mixer_tracks_pan",
            Self::SetMixerRouting => "set_mixer_routing",
            Self::InsertEffect => "add_effect",
            Self::RemoveEffect => "remove_effect",
            Self::InspectPluginParameters => "get_plugin_parameter_value",
            Self::SetPluginParameter => "set_plugin_parameter_value",
            Self::PianoRollScript => "run_piano_roll_script",
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::InspectSession,
            Self::ReadTempo,
            Self::SetTempo,
            Self::Play,
            Self::Stop,
            Self::InspectChannels,
            Self::InsertChannel,
            Self::InspectMixer,
            Self::SetMixerLevel,
            Self::SetMixerPan,
            Self::SetMixerRouting,
            Self::InsertEffect,
            Self::RemoveEffect,
            Self::InspectPluginParameters,
            Self::SetPluginParameter,
            Self::PianoRollScript,
        ]
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityManifest {
    pub adapter: String,
    pub experimental: bool,
    pub target_title: String,
    pub target_kind: String,
    pub target_id: String,
    pub capabilities: BTreeSet<FlCapability>,
    pub tools: Vec<NativeToolDefinition>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeToolResult {
    pub tool: String,
    pub raw: Value,
    pub content_text: Vec<String>,
}

impl NativeToolResult {
    pub fn primary_text(&self) -> Option<&str> {
        self.content_text.first().map(String::as_str)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedMutation {
    pub tool: String,
    pub before: Value,
    pub requested: Value,
    pub after: Value,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationRecord {
    pub sequence: u64,
    pub tool: String,
    pub arguments: Value,
    pub before: Value,
    pub after: Value,
    pub verified: bool,
    pub reversible: bool,
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("FL Studio native adapter transport failed: {0}")]
    Transport(String),
    #[error("FL Studio native capability is unsupported: {0:?}")]
    UnsupportedCapability(FlCapability),
    #[error("FL Studio native tool `{0}` is unavailable in the live catalog")]
    UnknownTool(String),
    #[error("invalid FL Studio tool arguments: {0}")]
    InvalidArguments(String),
    #[error("FL Studio native tool failed: {0}")]
    NativeTool(String),
    #[error("FL Studio mutation verification failed: {0}")]
    Verification(String),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CatalogDocument {
    Array(Vec<CatalogTool>),
    Envelope { tools: Vec<CatalogTool> },
}

impl CatalogDocument {
    fn into_tools(self) -> Vec<CatalogTool> {
        match self {
            Self::Array(tools) => tools,
            Self::Envelope { tools } => tools,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CatalogTool {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "inputSchema", default)]
    input_schema: CatalogInputSchema,
}

#[derive(Debug, Default, Deserialize)]
struct CatalogInputSchema {
    #[serde(default)]
    properties: IndexMap<String, Value>,
    #[serde(default)]
    required: Vec<String>,
}

impl CatalogInputSchema {
    fn as_json(&self) -> Value {
        json!({
            "type": "object",
            "properties": self.properties,
            "required": self.required,
            "additionalProperties": false
        })
    }
}

#[derive(Debug)]
struct CatalogSnapshot {
    tools: IndexMap<String, CatalogEntry>,
    schema_property_order_reliable: bool,
}

#[derive(Debug)]
struct CatalogEntry {
    description: String,
    schema: CatalogInputSchema,
}

impl CatalogSnapshot {
    fn from_callback_payload(payload: Value) -> Result<Self, AdapterError> {
        let (source, reliable) = match payload {
            Value::String(text) => (text, true),
            other => (
                serde_json::to_string(&other)
                    .map_err(|error| AdapterError::Transport(error.to_string()))?,
                false,
            ),
        };
        let document: CatalogDocument = serde_json::from_str(&source)
            .map_err(|error| AdapterError::Transport(format!("invalid MCP tool catalog: {error}")))?;
        let mut tools = IndexMap::new();
        for tool in document.into_tools() {
            tools.insert(
                tool.name,
                CatalogEntry {
                    description: tool.description,
                    schema: tool.input_schema,
                },
            );
        }
        Ok(Self {
            tools,
            schema_property_order_reliable: reliable,
        })
    }

    fn definition(&self, name: &str) -> Option<&CatalogEntry> {
        self.tools.get(name)
    }

    fn manifest_tools(&self) -> Vec<NativeToolDefinition> {
        self.tools
            .iter()
            .map(|(name, entry)| NativeToolDefinition {
                name: name.clone(),
                description: entry.description.clone(),
                input_schema: entry.schema.as_json(),
            })
            .collect()
    }
}

struct GopherSession {
    connection: GopherConnection,
    catalog: CatalogSnapshot,
}

impl GopherSession {
    fn connect(config: &FlStudioAdapterConfig) -> Result<Self, AdapterError> {
        let mut connection = GopherConnection::connect(&config.transport())
            .map_err(|error| AdapterError::Transport(error.to_string()))?;
        let payload = connection
            .request_catalog_payload()
            .map_err(|error| AdapterError::Transport(error.to_string()))?;
        let catalog = CatalogSnapshot::from_callback_payload(payload)?;
        Ok(Self {
            connection,
            catalog,
        })
    }
}

pub struct GopherNativeAdapter {
    config: FlStudioAdapterConfig,
    session: Mutex<GopherSession>,
    journal: Mutex<Vec<MutationRecord>>,
}

impl GopherNativeAdapter {
    pub fn connect(config: FlStudioAdapterConfig) -> Result<Self, AdapterError> {
        let session = GopherSession::connect(&config)?;
        Ok(Self {
            config,
            session: Mutex::new(session),
            journal: Mutex::new(Vec::new()),
        })
    }

    pub fn reconnect(&self) -> Result<(), AdapterError> {
        let replacement = GopherSession::connect(&self.config)?;
        *self.lock_session()? = replacement;
        Ok(())
    }

    pub fn capability_manifest(&self) -> Result<CapabilityManifest, AdapterError> {
        let session = self.lock_session()?;
        let capabilities = FlCapability::all()
            .iter()
            .copied()
            .filter(|capability| session.catalog.tools.contains_key(capability.required_tool()))
            .collect();
        Ok(CapabilityManifest {
            adapter: "gopher_native".into(),
            experimental: true,
            target_title: session.connection.target_title.clone(),
            target_kind: session.connection.target_kind.clone(),
            target_id: session.connection.target_id.clone(),
            capabilities,
            tools: session.catalog.manifest_tools(),
        })
    }

    pub fn supports(&self, capability: FlCapability) -> Result<bool, AdapterError> {
        Ok(self
            .lock_session()?
            .catalog
            .tools
            .contains_key(capability.required_tool()))
    }

    pub fn require(&self, capability: FlCapability) -> Result<(), AdapterError> {
        if self.supports(capability)? {
            Ok(())
        } else {
            Err(AdapterError::UnsupportedCapability(capability))
        }
    }

    pub fn journal_snapshot(&self) -> Result<Vec<MutationRecord>, AdapterError> {
        Ok(self.lock_journal()?.clone())
    }

    pub fn call_native(&self, tool: &str, arguments: Value) -> Result<NativeToolResult, AdapterError> {
        let mut session = self.lock_session()?;
        let ordered = canonicalize_tool_args(&session.catalog, tool, arguments)?;
        let request = tool_call_request_json(tool, &ordered)?;
        let raw = session
            .connection
            .call_tool_request(&request)
            .map_err(|error| AdapterError::Transport(error.to_string()))?;
        if let Some(message) = tool_failure_message(&raw) {
            return Err(AdapterError::NativeTool(message));
        }
        Ok(NativeToolResult {
            tool: tool.to_owned(),
            content_text: content_text(&raw),
            raw,
        })
    }

    pub fn session_context(&self) -> Result<NativeToolResult, AdapterError> {
        self.require(FlCapability::InspectSession)?;
        self.call_native("get_session_context", json!({}))
    }

    pub fn get_tempo(&self) -> Result<f64, AdapterError> {
        self.require(FlCapability::ReadTempo)?;
        let result = self.call_native("get_tempo", json!({}))?;
        extract_first_number(&result.content_text.join(" ")).ok_or_else(|| {
            AdapterError::Verification(format!(
                "could not parse tempo from native response: {}",
                result.primary_text().unwrap_or("<empty>")
            ))
        })
    }

    pub fn set_tempo_verified(&self, bpm: u32) -> Result<VerifiedMutation, AdapterError> {
        self.require(FlCapability::SetTempo)?;
        let before = self.get_tempo()?;
        self.call_native("set_tempo", json!({"bpm": bpm}))?;
        let after = self.get_tempo()?;
        let verified = (after - bpm as f64).abs() <= 0.01;
        let mutation = VerifiedMutation {
            tool: "set_tempo".into(),
            before: json!(before),
            requested: json!(bpm),
            after: json!(after),
            verified,
        };
        self.record_mutation(
            "set_tempo",
            json!({"bpm": bpm}),
            mutation.before.clone(),
            mutation.after.clone(),
            verified,
            true,
        )?;
        if !verified {
            return Err(AdapterError::Verification(format!(
                "requested {bpm} BPM, read back {after} BPM"
            )));
        }
        Ok(mutation)
    }

    pub fn play(&self) -> Result<NativeToolResult, AdapterError> {
        self.require(FlCapability::Play)?;
        self.call_native("play", json!({}))
    }

    pub fn stop(&self) -> Result<NativeToolResult, AdapterError> {
        self.require(FlCapability::Stop)?;
        self.call_native("stop", json!({}))
    }

    pub fn plugin_parameter_list(
        &self,
        target: &str,
        slot_number: u32,
    ) -> Result<NativeToolResult, AdapterError> {
        self.require(FlCapability::InspectPluginParameters)?;
        self.call_native(
            "get_plugin_parameter_list",
            json!({"target": target, "slot_number": slot_number}),
        )
    }

    pub fn plugin_parameter_value(
        &self,
        target: &str,
        param_identifier: &str,
        slot_number: u32,
    ) -> Result<f64, AdapterError> {
        self.require(FlCapability::InspectPluginParameters)?;
        let result = self.call_native(
            "get_plugin_parameter_value",
            json!({
                "target": target,
                "param_identifier": param_identifier,
                "slot_number": slot_number
            }),
        )?;
        extract_normalized_value(&result.content_text.join("\n")).ok_or_else(|| {
            AdapterError::Verification(format!(
                "could not parse normalized plugin parameter value from native response: {}",
                result.primary_text().unwrap_or("<empty>")
            ))
        })
    }

    pub fn set_plugin_parameter_verified(
        &self,
        target: &str,
        param_identifier: &str,
        value: f64,
        slot_number: u32,
    ) -> Result<VerifiedMutation, AdapterError> {
        self.require(FlCapability::SetPluginParameter)?;
        if !(0.0..=1.0).contains(&value) {
            return Err(AdapterError::InvalidArguments(
                "plugin parameter value must be normalized to 0..=1".into(),
            ));
        }
        let before = self.plugin_parameter_value(target, param_identifier, slot_number)?;
        self.call_native(
            "set_plugin_parameter_value",
            json!({
                "target": target,
                "param_identifier": param_identifier,
                "value": value,
                "slot_number": slot_number
            }),
        )?;
        let after = self.plugin_parameter_value(target, param_identifier, slot_number)?;
        let verified = (after - value).abs() <= 0.002;
        let args = json!({
            "target": target,
            "param_identifier": param_identifier,
            "value": value,
            "slot_number": slot_number
        });
        let mutation = VerifiedMutation {
            tool: "set_plugin_parameter_value".into(),
            before: json!(before),
            requested: json!(value),
            after: json!(after),
            verified,
        };
        self.record_mutation(
            "set_plugin_parameter_value",
            args,
            mutation.before.clone(),
            mutation.after.clone(),
            verified,
            true,
        )?;
        if !verified {
            return Err(AdapterError::Verification(format!(
                "requested normalized value {value}, read back {after}"
            )));
        }
        Ok(mutation)
    }

    pub fn add_effect_verified(
        &self,
        plugin: &str,
        target_tracks: &str,
        slot_number: u32,
    ) -> Result<VerifiedMutation, AdapterError> {
        self.require(FlCapability::InsertEffect)?;
        let args = json!({
            "plugin": plugin,
            "target_tracks": target_tracks,
            "slot_number": slot_number
        });
        self.call_native("add_effect", args.clone())?;
        let inspection = self.plugin_parameter_list(target_tracks, slot_number)?;
        let joined = inspection.content_text.join("\n");
        let verified = joined.contains(plugin);
        let mutation = VerifiedMutation {
            tool: "add_effect".into(),
            before: Value::Null,
            requested: args.clone(),
            after: json!({"resolvedPlugin": plugin, "parameterListMatched": verified}),
            verified,
        };
        self.record_mutation(
            "add_effect",
            args,
            Value::Null,
            mutation.after.clone(),
            verified,
            false,
        )?;
        if !verified {
            return Err(AdapterError::Verification(format!(
                "add_effect returned, but `{plugin}` did not resolve at target {target_tracks}, slot {slot_number}"
            )));
        }
        Ok(mutation)
    }

    fn lock_session(&self) -> Result<MutexGuard<'_, GopherSession>, AdapterError> {
        self.session
            .lock()
            .map_err(|_| AdapterError::Transport("Gopher single-flight lock poisoned".into()))
    }

    fn lock_journal(&self) -> Result<MutexGuard<'_, Vec<MutationRecord>>, AdapterError> {
        self.journal
            .lock()
            .map_err(|_| AdapterError::Transport("mutation journal lock poisoned".into()))
    }

    fn record_mutation(
        &self,
        tool: &str,
        arguments: Value,
        before: Value,
        after: Value,
        verified: bool,
        reversible: bool,
    ) -> Result<(), AdapterError> {
        let mut journal = self.lock_journal()?;
        let sequence = journal.len() as u64 + 1;
        journal.push(MutationRecord {
            sequence,
            tool: tool.into(),
            arguments,
            before,
            after,
            verified,
            reversible,
        });
        Ok(())
    }
}

fn canonicalize_tool_args(
    catalog: &CatalogSnapshot,
    tool: &str,
    arguments: Value,
) -> Result<IndexMap<String, Value>, AdapterError> {
    let entry = catalog
        .definition(tool)
        .ok_or_else(|| AdapterError::UnknownTool(tool.to_owned()))?;
    let Value::Object(object) = arguments else {
        return Err(AdapterError::InvalidArguments(format!(
            "tool `{tool}` requires a JSON object"
        )));
    };
    let mut args: IndexMap<String, Value> = object.into_iter().collect();
    for required in &entry.schema.required {
        if !args.contains_key(required) {
            return Err(AdapterError::InvalidArguments(format!(
                "tool `{tool}` is missing required argument `{required}`"
            )));
        }
    }
    if !entry.schema.properties.is_empty() {
        for key in args.keys() {
            if !entry.schema.properties.contains_key(key) {
                return Err(AdapterError::InvalidArguments(format!(
                    "tool `{tool}` does not declare argument `{key}`"
                )));
            }
        }
    } else if !args.is_empty() {
        return Err(AdapterError::InvalidArguments(format!(
            "tool `{tool}` declares no arguments"
        )));
    }

    let mut canonical = Vec::new();
    if catalog.schema_property_order_reliable {
        canonical.extend(entry.schema.properties.keys().cloned());
    } else {
        canonical.extend(entry.schema.required.iter().cloned());
        for key in entry.schema.properties.keys() {
            if !canonical.contains(key) {
                canonical.push(key.clone());
            }
        }
    }
    let mut ordered = IndexMap::with_capacity(args.len());
    for key in canonical {
        if let Some(value) = args.shift_remove(&key) {
            ordered.insert(key, value);
        }
    }
    for (key, value) in args {
        ordered.insert(key, value);
    }
    Ok(ordered)
}

#[derive(Serialize)]
struct ToolCallEnvelope<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: ToolCallParams<'a>,
}

#[derive(Serialize)]
struct ToolCallParams<'a> {
    name: &'a str,
    arguments: &'a IndexMap<String, Value>,
}

fn tool_call_request_json(
    tool: &str,
    args: &IndexMap<String, Value>,
) -> Result<String, AdapterError> {
    serde_json::to_string(&ToolCallEnvelope {
        jsonrpc: "2.0",
        id: 1,
        method: "tools/call",
        params: ToolCallParams {
            name: tool,
            arguments: args,
        },
    })
    .map_err(|error| AdapterError::Transport(error.to_string()))
}

fn content_text(value: &Value) -> Vec<String> {
    value
        .pointer("/result/content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn tool_failure_message(value: &Value) -> Option<String> {
    let is_error = value
        .pointer("/result/isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    for text in content_text(value) {
        let trimmed = text.trim_start();
        if is_error
            || trimmed.starts_with("Error:")
            || trimmed.starts_with("Flapi Error:")
            || trimmed.starts_with("Traceback")
        {
            return Some(trimmed.lines().next().unwrap_or(trimmed).to_owned());
        }
    }
    is_error.then(|| "unknown FL native tool error".into())
}

fn extract_first_number(text: &str) -> Option<f64> {
    let mut token = String::new();
    let mut started = false;
    for ch in text.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit() || (ch == '.' && started) || ((ch == '-' || ch == '+') && !started) {
            token.push(ch);
            started = true;
        } else if started {
            if let Ok(value) = token.parse::<f64>() {
                if value.is_finite() {
                    return Some(value);
                }
            }
            token.clear();
            started = false;
        }
    }
    None
}

fn extract_normalized_value(text: &str) -> Option<f64> {
    let marker = "Normalized Value:";
    let tail = text.split_once(marker)?.1;
    extract_first_number(tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> CatalogSnapshot {
        CatalogSnapshot::from_callback_payload(Value::String(
            r#"{"tools":[{"name":"set_plugin_parameter_value","description":"set","inputSchema":{"type":"object","properties":{"target":{"type":"string"},"param_identifier":{"type":"string"},"value":{"type":"number"},"slot_number":{"type":"integer"}},"required":["target","param_identifier","value","slot_number"]}}]}"#.into(),
        ))
        .unwrap()
    }

    #[test]
    fn canonicalizes_live_schema_order() {
        let ordered = canonicalize_tool_args(
            &catalog(),
            "set_plugin_parameter_value",
            json!({"slot_number": 10, "value": 0.51, "target": "1", "param_identifier": "558"}),
        )
        .unwrap();
        assert_eq!(
            ordered.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["target", "param_identifier", "value", "slot_number"]
        );
    }

    #[test]
    fn rejects_unknown_argument() {
        let error = canonicalize_tool_args(
            &catalog(),
            "set_plugin_parameter_value",
            json!({"target":"1","param_identifier":"558","value":0.5,"slot_number":10,"oops":1}),
        )
        .unwrap_err();
        assert!(matches!(error, AdapterError::InvalidArguments(_)));
    }

    #[test]
    fn parses_gopher_numeric_text() {
        assert_eq!(extract_first_number("Current tempo: 140.0 BPM"), Some(140.0));
        assert_eq!(
            extract_normalized_value("Value for 'Output Pan': Normalized Value: 0.5000, String Value: '0.00'"),
            Some(0.5)
        );
    }
}
