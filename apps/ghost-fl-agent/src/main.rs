mod scripting_bridge;

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
use ghost_fl_studio::{
    FlStudioAdapterConfig, FlStudioManifest, GopherNativeAdapter, NativeToolDefinition,
};
use scripting_bridge::ScriptingBridge;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const INDEX_HTML: &str = include_str!("../web/index.html");
const BENCHMARK_SETUP_PROMPT: &str = include_str!("../prompts/setup-benchmark-session.md");
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;

const SYSTEM_PROMPT: &str = r#"You are Ghost's raw FL Studio agent operating a live project through the exact native tool catalog exposed by FL Studio Gopher.

FL Studio is the current source of truth. The human may edit the project between any two of your actions, so earlier observations are snapshots rather than permanent facts.

Use the live tool schemas and descriptions as authoritative. Inspect current state before actions whose correctness depends on that state. Do not guess exact plugin, browser, parameter, channel, track, slot, or UI-element names when a discovery/read operation can establish them.

For relative value changes, read the current value before calculating the target. For routing changes, inspect current routing first and preserve unrelated routing unless the user explicitly asks otherwise. For plugin insertion, discover the exact installed plugin name first when Gopher provides a browser-discovery path.

The app intentionally exposes the raw Gopher tool surface. Do not infer extra Ghost policies that are not present in the tool schemas or the user's request. Do not make unrelated changes. Destructive or irreversible actions require a clear user request; if the target is materially ambiguous, ask instead of guessing.

Do not claim a native change happened unless the tool call succeeded. When the result can be inspected, re-observe it before making a strong claim about final state. Act directly when the request and target are clear, and keep the final summary grounded in calls and observations you actually made."#;

#[derive(Debug, Parser)]
#[command(
    name = "ghost-fl-agent",
    about = "Raw persistent Codex agent over the live FL Studio/Gopher tool catalog"
)]
struct Cli {
    #[arg(long, default_value_t = 9222)]
    debug_port: u16,

    #[arg(long, default_value = "gopher")]
    target_match: String,

    #[arg(long, default_value = "127.0.0.1:48765")]
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
    tool_count: usize,
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
    trace: Vec<TraceEvent>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InfoResponse {
    model: String,
    thread_id: String,
    tool_count: usize,
    profile: &'static str,
    benchmark_available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
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
            "ghost-fl-agent exposes the complete live Gopher tool catalog, including destructive tools; pass --i-accept-live-fl-writes only after opening a project you are willing to modify"
        );
    }

    let adapter = Arc::new(
        GopherNativeAdapter::connect(FlStudioAdapterConfig {
            debug_port: cli.debug_port,
            target_match: cli.target_match.clone(),
            ..Default::default()
        })
        .context("failed to connect to the live FL Studio Gopher target")?,
    );
    let manifest = adapter.manifest()?;
    let tool_count = manifest.tools.len();
    let registry = build_raw_registry(&manifest, Arc::clone(&adapter))?;

    println!(
        "[ghost-fl-agent] connected to '{}' with {} raw Gopher tools",
        manifest.target_title, tool_count
    );

    let runtime = CodexParallelRuntime::spawn(&cli.codex_binary)
        .context("failed to launch persistent Codex App Server")?;
    let cwd = std::env::current_dir().context("failed to resolve current working directory")?;
    let thread = runtime.start_thread(
        ParallelThreadConfig::new(cli.model.clone())
            .cwd(cwd)
            .service_name("ghost_fl_agent"),
        registry,
    )?;

    println!(
        "[ghost-fl-agent] started persistent thread {} using {}",
        thread.id, thread.model
    );

    let scripting = ScriptingBridge::start(
        &cli.scripting_bind,
        Duration::from_millis(cli.scripting_timeout_ms),
    )?;
    println!(
        "[ghost-fl-agent] FL scripting listener: {}",
        scripting.status().bind
    );

    let mut session = AgentSession {
        runtime,
        thread,
        tool_count,
        bootstrapped: false,
        verbose_agent_events: cli.verbose_agent_events,
    };

    serve(&cli.bind, &mut session, &scripting)
}

fn build_raw_registry(
    manifest: &FlStudioManifest,
    adapter: Arc<GopherNativeAdapter>,
) -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::default();
    for tool in &manifest.tools {
        register_raw_tool(&mut registry, tool, Arc::clone(&adapter))?;
    }
    Ok(registry)
}

fn register_raw_tool(
    registry: &mut ToolRegistry,
    tool: &NativeToolDefinition,
    adapter: Arc<GopherNativeAdapter>,
) -> Result<()> {
    let tool_name = tool.name.clone();
    let handler_name = tool_name.clone();
    registry.register(
        ToolDefinition {
            name: tool_name,
            description: tool.description.clone(),
            input_schema: tool.input_schema.clone(),
        },
        move |arguments| {
            adapter
                .call_native(&handler_name, arguments)
                .map(|result| result.raw)
                .map_err(|error| ToolError(error.to_string()))
        },
    )?;
    Ok(())
}

impl AgentSession {
    fn info(&self) -> InfoResponse {
        InfoResponse {
            model: self.thread.model.clone(),
            thread_id: self.thread.id.clone(),
            tool_count: self.tool_count,
            profile: "raw",
            benchmark_available: true,
        }
    }

    fn run_user_turn(&mut self, message: &str) -> Result<ChatResponse> {
        let message = message.trim();
        if message.is_empty() {
            bail!("message must not be empty");
        }

        let input_text = if self.bootstrapped {
            message.to_owned()
        } else {
            self.bootstrapped = true;
            format!("{SYSTEM_PROMPT}\n\nUSER REQUEST:\n{message}")
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
            &TurnOptions::default(),
            &mut |event| {
                if verbose {
                    println!("[ghost-fl-agent] agent event: {event:?}");
                }
                if let Some(trace_event) = trace_event(&event) {
                    if !verbose {
                        match trace_event.kind {
                            "tool_started" => {
                                let arguments = trace_event
                                    .arguments
                                    .as_ref()
                                    .map(Value::to_string)
                                    .unwrap_or_else(|| "{}".into());
                                println!(
                                    "[ghost-fl-agent] tool -> {} {arguments}",
                                    trace_event.tool
                                );
                            }
                            "tool_completed" => println!(
                                "[ghost-fl-agent] tool <- {} success={} duration_ms={}",
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
            trace,
        })
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

fn serve(bind: &str, session: &mut AgentSession, scripting: &ScriptingBridge) -> Result<()> {
    let listener = TcpListener::bind(bind)
        .with_context(|| format!("failed to bind ghost-fl-agent web UI at {bind}"))?;
    let address = listener.local_addr()?;
    println!("[ghost-fl-agent] chat UI: http://{address}");
    println!("[ghost-fl-agent] WARNING: raw profile exposes every live Gopher tool to the agent");

    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("[ghost-fl-agent] failed to accept HTTP connection: {error}");
                continue;
            }
        };
        if let Err(error) = handle_connection(&mut stream, session, scripting) {
            eprintln!("[ghost-fl-agent] HTTP request failed: {error:#}");
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

fn handle_connection(
    stream: &mut TcpStream,
    session: &mut AgentSession,
    scripting: &ScriptingBridge,
) -> Result<()> {
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
            send_json(stream, "200 OK", &scripting.status())
        }
        ("GET", "/api/benchmark-prompt") => send_response(
            stream,
            "200 OK",
            "text/plain; charset=utf-8",
            BENCHMARK_SETUP_PROMPT.as_bytes(),
        ),
        ("POST", "/api/chat") => {
            let request: ChatRequest =
                serde_json::from_slice(&request.body).context("invalid /api/chat JSON body")?;
            let response = session.run_user_turn(&request.message)?;
            send_json(stream, "200 OK", &response)
        }
        ("POST", "/api/setup-benchmark") => {
            let response = session.run_user_turn(BENCHMARK_SETUP_PROMPT)?;
            send_json(stream, "200 OK", &response)
        }
        ("POST", "/api/scripting/probe") => {
            let response = scripting.run_probe()?;
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
    fn benchmark_prompt_contains_guard_and_success_markers() {
        assert!(BENCHMARK_SETUP_PROMPT.contains("fresh or disposable"));
        assert!(BENCHMARK_SETUP_PROMPT.contains("BENCHMARK_SETUP_GREEN"));
        assert!(BENCHMARK_SETUP_PROMPT.contains("BENCHMARK_SETUP_ABORTED"));
    }
}
