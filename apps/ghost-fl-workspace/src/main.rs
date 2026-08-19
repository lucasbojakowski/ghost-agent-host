mod gateway;
mod snapshot;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use gateway::{build_workspace_registry, SCRIPTING_TOOL_NAMES};
use ghost_codex::{
    AgentEvent, CodexParallelRuntime, ParallelCodexThread, ParallelThreadConfig, TurnInput,
    TurnOptions,
};
use ghost_fl_scripting::{FlScriptingAdapter, FlScriptingConfig, FlScriptingStatus};
use ghost_fl_studio::{FlStudioAdapterConfig, GopherNativeAdapter};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snapshot::{capture_workspace_snapshot, WorkspaceSnapshot};

const INDEX_HTML: &str = include_str!("../web/index.html");
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;

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
