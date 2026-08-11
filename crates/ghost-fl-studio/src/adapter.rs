use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

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

/// One tool exactly as advertised by the live Gopher MCP catalog.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Snapshot of the currently attached Gopher target and its live tool catalog.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlStudioManifest {
    pub adapter: String,
    pub target_title: String,
    pub target_kind: String,
    pub target_id: String,
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

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("FL Studio Gopher transport failed: {0}")]
    Transport(String),
    #[error("FL Studio native tool `{0}` is unavailable in the live catalog")]
    UnknownTool(String),
    #[error("invalid FL Studio tool arguments: {0}")]
    InvalidArguments(String),
    #[error("FL Studio native tool failed: {0}")]
    NativeTool(String),
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

#[derive(Debug, Default, Deserialize, Serialize)]
struct CatalogInputSchema {
    #[serde(default)]
    properties: IndexMap<String, Value>,
    #[serde(default)]
    required: Vec<String>,
    #[serde(flatten)]
    extra: IndexMap<String, Value>,
}

impl CatalogInputSchema {
    fn as_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| json!({"type": "object"}))
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
        let (source, reliable) = catalog_source(payload)?;
        let document: CatalogDocument = serde_json::from_str(&source).map_err(|error| {
            AdapterError::Transport(format!("invalid MCP tool catalog: {error}"))
        })?;
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

fn catalog_source(payload: Value) -> Result<(String, bool), AdapterError> {
    match payload {
        Value::String(mut text) => {
            // Gopher callbacks have been observed to arrive JSON-string encoded more than once.
            // Peel only string layers so the final catalog text is parsed directly into IndexMap,
            // preserving the live schema/signature property order used by tools/call.
            while let Ok(inner) = serde_json::from_str::<String>(&text) {
                text = inner;
            }
            Ok((text, true))
        }
        other => serde_json::to_string(&other)
            .map(|text| (text, false))
            .map_err(|error| AdapterError::Transport(error.to_string())),
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

/// Transparent, single-flight mirror of the live FL Studio/Gopher interface.
///
/// This adapter owns only behavior imposed by Gopher/CDP: discovery, the live catalog, schema-order
/// canonicalization, callback normalization, serialized calls, and native-error detection. Product
/// permissions and agent policy deliberately live above this crate.
pub struct GopherNativeAdapter {
    config: FlStudioAdapterConfig,
    session: Mutex<GopherSession>,
}

impl GopherNativeAdapter {
    pub fn connect(config: FlStudioAdapterConfig) -> Result<Self, AdapterError> {
        let session = GopherSession::connect(&config)?;
        Ok(Self {
            config,
            session: Mutex::new(session),
        })
    }

    pub fn reconnect(&self) -> Result<(), AdapterError> {
        let replacement = GopherSession::connect(&self.config)?;
        *self.lock_session()? = replacement;
        Ok(())
    }

    pub fn manifest(&self) -> Result<FlStudioManifest, AdapterError> {
        let session = self.lock_session()?;
        Ok(FlStudioManifest {
            adapter: "gopher_native".into(),
            target_title: session.connection.target_title.clone(),
            target_kind: session.connection.target_kind.clone(),
            target_id: session.connection.target_id.clone(),
            tools: session.catalog.manifest_tools(),
        })
    }

    /// Invoke one tool from the live Gopher catalog.
    ///
    /// The session mutex is intentional: the observed Gopher callback does not carry dependable
    /// call correlation, so calls remain single-flight even though callers may be concurrent.
    pub fn call_native(
        &self,
        tool: &str,
        arguments: Value,
    ) -> Result<NativeToolResult, AdapterError> {
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

    fn lock_session(&self) -> Result<MutexGuard<'_, GopherSession>, AdapterError> {
        self.session
            .lock()
            .map_err(|_| AdapterError::Transport("Gopher single-flight lock poisoned".into()))
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
    fn recursively_unwraps_string_encoded_catalogs_without_losing_order() {
        let source = r#"{"tools":[{"name":"probe","inputSchema":{"properties":{"z":{},"a":{}},"required":["z","a"]}}]}"#;
        let twice = Value::String(serde_json::to_string(source).unwrap());
        let catalog = CatalogSnapshot::from_callback_payload(twice).unwrap();
        let ordered = canonicalize_tool_args(&catalog, "probe", json!({"a": 1, "z": 2})).unwrap();
        assert_eq!(
            ordered.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["z", "a"]
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
    fn distinguishes_native_failures_from_transport_success() {
        let raw = json!({
            "result": {
                "content": [{"type": "text", "text": "Flapi Error: parameter unavailable"}],
                "isError": true
            }
        });
        assert_eq!(
            tool_failure_message(&raw).as_deref(),
            Some("Flapi Error: parameter unavailable")
        );
    }
}
