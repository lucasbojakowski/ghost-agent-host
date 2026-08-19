use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use ghost_codex::{
    AgentEvent, CodexParallelRuntime, ParallelCodexThread, ParallelThreadConfig, ToolDefinition,
    ToolError, ToolRegistry, TurnInput, TurnOptions,
};
use ghost_fl_scripting::{
    FlScriptingAdapter, FlScriptingCatalog, FlScriptingConfig, FlScriptingFunction,
    FlScriptingStatus,
};
use ghost_fl_studio::{
    FlStudioAdapterConfig, FlStudioManifest, GopherNativeAdapter, NativeToolDefinition,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const INDEX_HTML: &str = include_str!("../web/index.html");
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const SCRIPTING_TOOL_NAMES: [&str; 3] = [
    "fl_scripting_search",
    "fl_scripting_describe",
    "fl_scripting_call",
];

const SYSTEM_PROMPT: &str = r#"You are Ghost's empirical FL Studio workspace agent. You have two transparent live surfaces over the same FL Studio session:

1. Every raw native Gopher tool advertised by the live Gopher catalog. Their schemas and descriptions are authoritative and are exposed unchanged.
2. Exactly three FL MIDI Scripting gateway tools: fl_scripting_search, fl_scripting_describe, and fl_scripting_call. These let you discover and invoke the checked-in, runtime-evidenced MIDI Scripting catalog without hundreds of generated tools.

The FL scripting snapshot injected before every user request is a point-in-time convenience observation, not durable state and not a semantic world model. The human may edit FL Studio between any two actions. Re-observe live state whenever correctness depends on it.

Use fl_scripting_search and fl_scripting_describe when you need to discover a scripting primitive or its evidence-backed signature. Use fl_scripting_call only with an explicit module/function and positional JSON arguments. An unsupported scripting call means the bridge metadata or wire shape does not establish that primitive safely; do not bypass that boundary.

For Gopher calls, use the live schemas exactly. For relative changes, inspect current values first. For routing or structural changes, inspect current state and preserve unrelated state unless the user requests otherwise. Do not guess exact plugin, browser, channel, track, slot, parameter, pattern, or UI names when either surface can establish them.

Do not claim a mutation succeeded unless its tool call succeeded. When the result can be inspected through either live surface, verify it before making a strong final claim. Do not invent Ghost skills, intents, entities, capability profiles, or hidden safety classifications; this app is intentionally the primitive combined-surface experiment."#;

#[derive(Debug, Parser)]
#[command(
    name = "ghost-fl-workspace",
    about = "Empirical combined Gopher + FL MIDI Scripting workspace agent"
)]
struct Cli {
    #[arg(long, default_value_t = 9222)]
    debug_port: u16,

    #[arg(long, default_value = "gopher")]
    target_match: String,

    #[arg(long, default_value = "127.0.0.1:48775")]
    bind: String,

    #[arg(long, default_value = "127.0.0.1:48766")]
    scripting_bind: String,

    #[arg(long, default_value_t = 1500)]
    scripting_timeout_ms: u64,

    #[arg(long, default_value = "codex")]
    codex_binary: String,

    #[arg(long, default_value = "gpt-5.6-terra")]
    model: String,

    #[arg(long)]
    verbose_agent_events: bool,

    #[arg(long = "i-accept-live-fl-writes")]
    i_accept_live_fl_writes: bool,
}

struct AgentSession {
    runtime: CodexParallelRuntime,
    thread: ParallelCodexThread,
    scripting: Arc<FlScriptingAdapter>,
    gopher_tool_count: usize,
    bootstrapped: bool,
    verbose_agent_events: bool,
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatResponse {
    text: String,
    thread_id: String,
    snapshot: WorkspaceSnapshot,
    trace: Vec<TraceEvent>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InfoResponse {
    model: String,
    thread_id: String,
    gopher_tool_count: usize,
    scripting_tool_count: usize,
    total_tool_count: usize,
    profile: &'static str,
    scripting: FlScriptingStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceEvent {
    kind: &'static str,
    tool: String,
    arguments: Option<Value>,
    success: Option<bool>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSnapshot {
    connected: bool,
    values: BTreeMap<String, Value>,
    errors: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ScriptingSearchArgs {
    query: String,
    #[serde(default)]
    module: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScriptingDescribeArgs {
    module: String,
    function: String,
}

#[derive(Debug, Deserialize)]
struct ScriptingCallArgs {
    module: String,
    function: String,
    args: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScriptingSearchMatch {
    score: u8,
    module: String,
    function: String,
    signature: Option<String>,
    returns: Option<String>,
    description: Option<String>,
    minimum_api_version: Option<u32>,
    bridge_callable: bool,
    unsupported_reason: Option<String>,
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.i_accept_live_fl_writes {
        bail!(
            "ghost-fl-workspace exposes the complete live Gopher catalog plus raw FL scripting calls; pass --i-accept-live-fl-writes only after opening a project you are willing to modify"
        );
    }

    let gopher = Arc::new(
        GopherNativeAdapter::connect(FlStudioAdapterConfig {
            debug_port: cli.debug_port,
            target_match: cli.target_match.clone(),
            ..Default::default()
        })
        .context("failed to connect to the live FL Studio Gopher target")?,
    );
    let manifest = gopher.manifest()?;
    let gopher_tool_count = manifest.tools.len();

    let scripting = Arc::new(FlScriptingAdapter::start(FlScriptingConfig {
        bind: cli.scripting_bind.clone(),
        call_timeout: Duration::from_millis(cli.scripting_timeout_ms),
    })?);

    let registry = build_workspace_registry(&manifest, Arc::clone(&gopher), Arc::clone(&scripting))?;
    let total_tool_count = registry.definitions().len();

    println!(
        "[ghost-fl-workspace] connected to '{}' with {} raw Gopher tools + {} scripting gateways",
        manifest.target_title,
        gopher_tool_count,
        SCRIPTING_TOOL_NAMES.len()
    );
    println!(
        "[ghost-fl-workspace] FL scripting listener: {}",
        scripting.status().bind
    );

    let runtime = CodexParallelRuntime::spawn(&cli.codex_binary)
        .context("failed to launch persistent Codex App Server")?;
    let cwd = std::env::current_dir().context("failed to resolve current working directory")?;
    let thread = runtime.start_thread(
        ParallelThreadConfig::new(cli.model.clone())
            .cwd(cwd)
            .service_name("ghost_fl_workspace"),
        registry,
    )?;

    println!(
        "[ghost-fl-workspace] started persistent thread {} using {} with {} dynamic tools",
        thread.id, thread.model, total_tool_count
    );
    println!("[ghost-fl-workspace] WARNING: Codex turns run with full host filesystem access");
    println!("[ghost-fl-workspace] WARNING: combined surface can perform live FL Studio writes");

    let mut session = AgentSession {
        runtime,
        thread,
        scripting,
        gopher_tool_count,
        bootstrapped: false,
        verbose_agent_events: cli.verbose_agent_events,
    };
    serve(&cli.bind, &mut session)
}

fn build_workspace_registry(
    manifest: &FlStudioManifest,
    gopher: Arc<GopherNativeAdapter>,
    scripting: Arc<FlScriptingAdapter>,
) -> Result<ToolRegistry> {
    for gateway_name in SCRIPTING_TOOL_NAMES {
        if manifest.tools.iter().any(|tool| tool.name == gateway_name) {
            bail!("live Gopher catalog collides with workspace gateway tool `{gateway_name}`");
        }
    }

    let definitions = workspace_tool_definitions(manifest);
    let gopher_count = manifest.tools.len();
    let mut registry = ToolRegistry::default();
    for (index, definition) in definitions.into_iter().enumerate() {
        if index < gopher_count {
            let handler_name = definition.name.clone();
            let adapter = Arc::clone(&gopher);
            registry.register(definition, move |arguments| {
                adapter
                    .call_native(&handler_name, arguments)
                    .map(|result| result.raw)
                    .map_err(|error| ToolError(error.to_string()))
            })?;
            continue;
        }

        match definition.name.as_str() {
            "fl_scripting_search" => {
                let catalog = scripting.catalog();
                registry.register(definition, move |arguments| {
                    let request: ScriptingSearchArgs = serde_json::from_value(arguments)
                        .map_err(|error| ToolError(format!("invalid scripting search arguments: {error}")))?;
                    search_scripting_catalog(&catalog, &request.query, request.module.as_deref())
                        .map_err(ToolError)
                })?;
            }
            "fl_scripting_describe" => {
                let catalog = scripting.catalog();
                registry.register(definition, move |arguments| {
                    let request: ScriptingDescribeArgs = serde_json::from_value(arguments)
                        .map_err(|error| ToolError(format!("invalid scripting describe arguments: {error}")))?;
                    describe_scripting_function(&catalog, &request.module, &request.function)
                        .map_err(ToolError)
                })?;
            }
            "fl_scripting_call" => {
                let adapter = Arc::clone(&scripting);
                registry.register(definition, move |arguments| {
                    let request: ScriptingCallArgs = serde_json::from_value(arguments)
                        .map_err(|error| ToolError(format!("invalid scripting call arguments: {error}")))?;
                    adapter
                        .call(&request.module, &request.function, request.args)
                        .map_err(|error| ToolError(error.to_string()))
                })?;
            }
            other => bail!("unexpected workspace tool definition `{other}`"),
        }
    }
    Ok(registry)
}

fn workspace_tool_definitions(manifest: &FlStudioManifest) -> Vec<ToolDefinition> {
    let mut definitions: Vec<ToolDefinition> = manifest
        .tools
        .iter()
        .map(gopher_tool_definition)
        .collect();
    definitions.extend(scripting_gateway_definitions());
    definitions
}

fn gopher_tool_definition(tool: &NativeToolDefinition) -> ToolDefinition {
    ToolDefinition {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: tool.input_schema.clone(),
    }
}

fn scripting_gateway_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "fl_scripting_search".into(),
            description: "Search the checked-in FL MIDI Scripting runtime catalog. Use this before scripting calls when the module/function is not already established.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Function/module/signature/description terms"},
                    "module": {"type": "string", "description": "Optional exact FL scripting module filter"}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "fl_scripting_describe".into(),
            description: "Return the evidence-backed FL MIDI Scripting metadata for one exact module/function, including overloads and bridge support.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "module": {"type": "string"},
                    "function": {"type": "string"}
                },
                "required": ["module", "function"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "fl_scripting_call".into(),
            description: "Invoke one explicitly cataloged FL MIDI Scripting primitive with positional JSON arguments through the live loopback bridge.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "module": {"type": "string"},
                    "function": {"type": "string"},
                    "args": {"type": "array", "items": {}}
                },
                "required": ["module", "function", "args"],
                "additionalProperties": false
            }),
        },
    ]
}

fn search_scripting_catalog(
    catalog: &FlScriptingCatalog,
    query: &str,
    module: Option<&str>,
) -> Result<Value, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("scripting search query must not be empty".into());
    }
    let module = module.map(str::trim).filter(|value| !value.is_empty());
    let lowered_query = query.to_ascii_lowercase();
    let terms: Vec<&str> = lowered_query.split_whitespace().collect();
    let mut matches: Vec<(u8, &FlScriptingFunction)> = catalog
        .functions()
        .iter()
        .filter(|entry| {
            module.is_none_or(|module| entry.module.eq_ignore_ascii_case(module))
        })
        .filter_map(|entry| search_score(entry, &lowered_query, &terms).map(|score| (score, entry)))
        .collect();
    matches.sort_by(|(score_a, entry_a), (score_b, entry_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| entry_a.module.cmp(&entry_b.module))
            .then_with(|| entry_a.function.cmp(&entry_b.function))
            .then_with(|| entry_a.signature.cmp(&entry_b.signature))
    });

    let matches: Vec<ScriptingSearchMatch> = matches
        .into_iter()
        .take(25)
        .map(|(score, entry)| ScriptingSearchMatch {
            score,
            module: entry.module.clone(),
            function: entry.function.clone(),
            signature: entry.signature.clone(),
            returns: entry.returns.clone(),
            description: entry.description.clone(),
            minimum_api_version: entry.minimum_api_version,
            bridge_callable: entry.bridge_callable,
            unsupported_reason: entry.unsupported_reason.clone(),
        })
        .collect();
    Ok(json!({
        "query": query,
        "module": module,
        "matches": matches
    }))
}

fn search_score(entry: &FlScriptingFunction, query: &str, terms: &[&str]) -> Option<u8> {
    let signature = entry.signature.as_deref().unwrap_or_default();
    let description = entry.description.as_deref().unwrap_or_default();
    let haystack = format!(
        "{} {} {} {}",
        entry.module, entry.function, signature, description
    )
    .to_ascii_lowercase();
    if !terms.iter().all(|term| haystack.contains(term)) {
        return None;
    }
    let qualified = format!("{}.{}", entry.module, entry.function).to_ascii_lowercase();
    let function = entry.function.to_ascii_lowercase();
    let score = if qualified == query {
        100
    } else if function == query {
        95
    } else if function.starts_with(query) {
        85
    } else if function.contains(query) {
        75
    } else if signature.to_ascii_lowercase().contains(query) {
        65
    } else {
        50
    };
    Some(score)
}

fn describe_scripting_function(
    catalog: &FlScriptingCatalog,
    module: &str,
    function: &str,
) -> Result<Value, String> {
    let overloads = catalog.describe(module.trim(), function.trim());
    if overloads.is_empty() {
        return Err(format!(
            "FL scripting function `{}.{}` was not found in the checked-in runtime catalog",
            module.trim(),
            function.trim()
        ));
    }
    serde_json::to_value(json!({
        "module": module.trim(),
        "function": function.trim(),
        "overloads": overloads
    }))
    .map_err(|error| format!("failed to serialize scripting metadata: {error}"))
}

impl AgentSession {
    fn info(&self) -> InfoResponse {
        InfoResponse {
            model: self.thread.model.clone(),
            thread_id: self.thread.id.clone(),
            gopher_tool_count: self.gopher_tool_count,
            scripting_tool_count: SCRIPTING_TOOL_NAMES.len(),
            total_tool_count: self.gopher_tool_count + SCRIPTING_TOOL_NAMES.len(),
            profile: "workspace",
            scripting: self.scripting.status(),
        }
    }

    fn run_user_turn(&mut self, message: &str) -> Result<ChatResponse> {
        let message = message.trim();
        if message.is_empty() {
            bail!("message must not be empty");
        }
        let snapshot = capture_workspace_snapshot(&self.scripting);
        let snapshot_text = serde_json::to_string_pretty(&snapshot)?;
        let turn_text = format!(
            "POINT-IN-TIME FL MIDI SCRIPTING SNAPSHOT (re-observe if correctness depends on it):\n{snapshot_text}\n\nUSER REQUEST:\n{message}"
        );
        let input_text = if self.bootstrapped {
            turn_text
        } else {
            self.bootstrapped = true;
            format!("{SYSTEM_PROMPT}\n\n{turn_text}")
        };
        let input = TurnInput {
            text: input_text,
            output_schema: None,
        };
        let mut trace = Vec::new();
        let verbose = self.verbose_agent_events;
        let output = self.runtime.run_turn(
            &self.thread,
            &input,
            &full_access_turn_options(),
            &mut |event| {
                if verbose {
                    println!("[ghost-fl-workspace] agent event: {event:?}");
                }
                if let Some(trace_event) = trace_event(&event) {
                    if !verbose {
                        match trace_event.kind {
                            "tool_started" => println!(
                                "[ghost-fl-workspace] tool -> {} {}",
                                trace_event.tool,
                                trace_event
                                    .arguments
                                    .as_ref()
                                    .map(Value::to_string)
                                    .unwrap_or_else(|| "{}".into())
                            ),
                            "tool_completed" => println!(
                                "[ghost-fl-workspace] tool <- {} success={} duration_ms={}",
                                trace_event.tool,
                                trace_event.success.unwrap_or(false),
                                trace_event.duration_ms.unwrap_or(0)
                            ),
                            _ => {}
                        }
                    }
                    trace.push(trace_event);
                }
            },
        )?;

        Ok(ChatResponse {
            text: output.text,
            thread_id: self.thread.id.clone(),
            snapshot,
            trace,
        })
    }
}

fn capture_workspace_snapshot(scripting: &FlScriptingAdapter) -> WorkspaceSnapshot {
    let status = scripting.status();
    let mut snapshot = WorkspaceSnapshot {
        connected: status.connected,
        values: BTreeMap::new(),
        errors: BTreeMap::new(),
    };
    if !status.connected {
        snapshot.errors.insert(
            "connection".into(),
            status
                .last_error
                .unwrap_or_else(|| format!("waiting for FL scripting device at {}", status.bind)),
        );
        return snapshot;
    }

    for (key, module, function, args) in [
        ("scriptingApiVersion", "general", "getVersion", vec![]),
        ("flVersion", "ui", "getVersion", vec![json!(5)]),
        ("projectTitle", "general", "getProjectTitle", vec![]),
        ("projectChangedFlag", "general", "getChangedFlag", vec![]),
        ("safeToEdit", "general", "safeToEdit", vec![]),
        ("selectedChannel", "channels", "channelNumber", vec![]),
        ("selectedMixerTrack", "mixer", "trackNumber", vec![]),
        ("mixerTrackCount", "mixer", "trackCount", vec![]),
        ("currentPattern", "patterns", "patternNumber", vec![]),
        ("patternCount", "patterns", "patternCount", vec![]),
        ("arrangementSelectionStart", "arrangement", "selectionStart", vec![]),
        ("arrangementSelectionEnd", "arrangement", "selectionEnd", vec![]),
        ("focusedPluginName", "ui", "getFocusedPluginName", vec![]),
        ("focusedWindowCaption", "ui", "getFocusedFormCaption", vec![]),
        ("songPosition", "transport", "getSongPos", vec![]),
        ("songPositionHint", "transport", "getSongPosHint", vec![]),
        ("loopMode", "transport", "getLoopMode", vec![]),
        ("isPlaying", "transport", "isPlaying", vec![]),
    ] {
        observe_snapshot(scripting, &mut snapshot, key, module, function, args);
    }

    if let Some(pattern) = snapshot
        .values
        .get("currentPattern")
        .and_then(Value::as_i64)
    {
        observe_snapshot(
            scripting,
            &mut snapshot,
            "currentPatternName",
            "patterns",
            "getPatternName",
            vec![json!(pattern)],
        );
    }
    if let (Some(start), Some(end)) = (
        snapshot
            .values
            .get("arrangementSelectionStart")
            .and_then(Value::as_i64),
        snapshot
            .values
            .get("arrangementSelectionEnd")
            .and_then(Value::as_i64),
    ) {
        snapshot
            .values
            .insert("arrangementSelectionActive".into(), json!(start != end));
    }
    snapshot
}

fn observe_snapshot(
    scripting: &FlScriptingAdapter,
    snapshot: &mut WorkspaceSnapshot,
    key: &str,
    module: &str,
    function: &str,
    args: Vec<Value>,
) {
    match scripting.call(module, function, args) {
        Ok(value) => {
            snapshot.values.insert(key.into(), value);
        }
        Err(error) => {
            snapshot.errors.insert(key.into(), error.to_string());
        }
    }
}

fn full_access_turn_options() -> TurnOptions {
    TurnOptions {
        sandbox_policy: json!({"type": "dangerFullAccess"}),
        ..TurnOptions::default()
    }
}

fn trace_event(event: &AgentEvent) -> Option<TraceEvent> {
    match event {
        AgentEvent::ItemStarted { item }
            if item.get("type").and_then(Value::as_str) == Some("dynamicToolCall") =>
        {
            Some(TraceEvent {
                kind: "tool_started",
                tool: item
                    .get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>")
                    .to_owned(),
                arguments: item.get("arguments").cloned(),
                success: None,
                duration_ms: None,
            })
        }
        AgentEvent::ItemCompleted { item }
            if item.get("type").and_then(Value::as_str) == Some("dynamicToolCall") =>
        {
            Some(TraceEvent {
                kind: "tool_completed",
                tool: item
                    .get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>")
                    .to_owned(),
                arguments: item.get("arguments").cloned(),
                success: item.get("success").and_then(Value::as_bool),
                duration_ms: item.get("durationMs").and_then(Value::as_u64),
            })
        }
        _ => None,
    }
}

fn serve(bind: &str, session: &mut AgentSession) -> Result<()> {
    let listener = TcpListener::bind(bind)
        .with_context(|| format!("failed to bind ghost-fl-workspace web UI at {bind}"))?;
    let address = listener.local_addr()?;
    println!("[ghost-fl-workspace] UI: http://{address}");

    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("[ghost-fl-workspace] failed to accept HTTP connection: {error}");
                continue;
            }
        };
        if let Err(error) = handle_connection(&mut stream, session) {
            eprintln!("[ghost-fl-workspace] HTTP request failed: {error:#}");
            let _ = send_json(
                &mut stream,
                "500 Internal Server Error",
                &ErrorResponse {
                    error: error.to_string(),
                },
            );
        }
    }
    Ok(())
}

fn handle_connection(stream: &mut TcpStream, session: &mut AgentSession) -> Result<()> {
    let Some(request) = read_request(stream)? else {
        return Ok(());
    };
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => send_response(
            stream,
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML.as_bytes(),
        ),
        ("GET", "/api/info") => send_json(stream, "200 OK", &session.info()),
        ("GET", "/api/scripting/status") => {
            send_json(stream, "200 OK", &session.scripting.status())
        }
        ("GET", "/api/snapshot") => {
            let snapshot = capture_workspace_snapshot(&session.scripting);
            send_json(stream, "200 OK", &snapshot)
        }
        ("POST", "/api/chat") => {
            let request: ChatRequest =
                serde_json::from_slice(&request.body).context("invalid /api/chat JSON body")?;
            let response = session.run_user_turn(&request.message)?;
            send_json(stream, "200 OK", &response)
        }
        _ => send_json(
            stream,
            "404 Not Found",
            &ErrorResponse {
                error: "not found".into(),
            },
        ),
    }
}

fn read_request(stream: &mut TcpStream) -> Result<Option<HttpRequest>> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    let header_end = loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            if bytes.is_empty() {
                return Ok(None);
            }
            bail!("HTTP connection closed before request headers completed");
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            bail!("HTTP request exceeded {MAX_REQUEST_BYTES} bytes");
        }
        if let Some(position) = find_bytes(&bytes, b"\r\n\r\n") {
            break position + 4;
        }
    };

    let headers =
        std::str::from_utf8(&bytes[..header_end]).context("HTTP headers were not UTF-8")?;
    let mut lines = headers.split("\r\n");
    let request_line = lines.next().context("missing HTTP request line")?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .context("missing HTTP method")?
        .to_owned();
    let path = request_parts
        .next()
        .context("missing HTTP path")?
        .to_owned();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>())
        .transpose()
        .context("invalid Content-Length header")?
        .unwrap_or(0);

    let expected = header_end
        .checked_add(content_length)
        .context("HTTP request length overflow")?;
    if expected > MAX_REQUEST_BYTES {
        bail!("HTTP request exceeded {MAX_REQUEST_BYTES} bytes");
    }
    while bytes.len() < expected {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            bail!("HTTP connection closed before request body completed");
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            bail!("HTTP request exceeded {MAX_REQUEST_BYTES} bytes");
        }
    }

    Ok(Some(HttpRequest {
        method,
        path,
        body: bytes[header_end..expected].to_vec(),
    }))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn send_json<T: Serialize>(stream: &mut TcpStream, status: &str, value: &T) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    send_response(stream, status, "application/json; charset=utf-8", &body)
}

fn send_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_manifest() -> FlStudioManifest {
        FlStudioManifest {
            adapter: "gopher-native".into(),
            target_title: "FL Studio".into(),
            target_kind: "page".into(),
            target_id: "fixture".into(),
            tools: vec![
                NativeToolDefinition {
                    name: "native_alpha".into(),
                    description: "alpha description".into(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {"value": {"type": "number"}},
                        "required": ["value"]
                    }),
                },
                NativeToolDefinition {
                    name: "native_beta".into(),
                    description: "beta description".into(),
                    input_schema: json!({"type": "object", "properties": {}}),
                },
            ],
        }
    }

    #[test]
    fn combined_definition_surface_preserves_every_gopher_definition_then_adds_three_gateways() {
        let manifest = fixture_manifest();
        let definitions = workspace_tool_definitions(&manifest);
        assert_eq!(definitions.len(), manifest.tools.len() + 3);
        for (definition, native) in definitions.iter().zip(&manifest.tools) {
            assert_eq!(definition.name, native.name);
            assert_eq!(definition.description, native.description);
            assert_eq!(definition.input_schema, native.input_schema);
        }
        let gateway_names: Vec<&str> = definitions[manifest.tools.len()..]
            .iter()
            .map(|definition| definition.name.as_str())
            .collect();
        assert_eq!(gateway_names, SCRIPTING_TOOL_NAMES);
    }

    #[test]
    fn gateway_names_do_not_expand_into_per_function_tools() {
        let names: BTreeSet<&str> = scripting_gateway_definitions()
            .iter()
            .map(|definition| definition.name.as_str())
            .collect();
        assert_eq!(names.len(), 3);
        assert_eq!(names, SCRIPTING_TOOL_NAMES.into_iter().collect());
    }

    #[test]
    fn scripting_search_is_deterministic_and_module_filterable() {
        let catalog = FlScriptingCatalog::bundled().unwrap();
        let first = search_scripting_catalog(&catalog, "pattern name", Some("patterns")).unwrap();
        let second = search_scripting_catalog(&catalog, "pattern name", Some("patterns")).unwrap();
        assert_eq!(first, second);
        let matches = first["matches"].as_array().unwrap();
        assert!(matches.iter().any(|entry| {
            entry["module"] == "patterns" && entry["function"] == "getPatternName"
        }));
        assert!(matches.iter().all(|entry| entry["module"] == "patterns"));
    }

    #[test]
    fn scripting_describe_preserves_overloads() {
        let catalog = FlScriptingCatalog::bundled().unwrap();
        let described = describe_scripting_function(&catalog, "device", "midiOutMsg").unwrap();
        assert_eq!(described["overloads"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn empty_scripting_search_is_rejected() {
        let catalog = FlScriptingCatalog::bundled().unwrap();
        assert!(search_scripting_catalog(&catalog, "  ", None).is_err());
    }

    #[test]
    fn finds_http_header_boundary() {
        let request = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\nbody";
        assert_eq!(find_bytes(request, b"\r\n\r\n"), Some(31));
    }

    #[test]
    fn turns_use_full_access_without_changing_global_defaults() {
        let options = full_access_turn_options();
        assert_eq!(options.sandbox_policy, json!({"type": "dangerFullAccess"}));
        assert_eq!(options.approval_policy, "never");
        assert_eq!(
            TurnOptions::default().sandbox_policy,
            json!({
                "type": "readOnly",
                "access": {"type": "fullAccess"}
            })
        );
    }
}
