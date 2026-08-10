use std::{
    env,
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::{json, Value};

const DEFAULT_DEBUG_PORT: u16 = 9222;
const DEFAULT_WAIT_SECONDS: u64 = 180;
const BRIDGE_TIMEOUT_MS: u64 = 20_000;
const WS_IO_TIMEOUT_SECONDS: u64 = 30;
const MAX_WS_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(
    name = "ghost-fl-native-bridge",
    about = "Experimental headless bridge to FL Studio 2026's Gopher WebView control surface"
)]
struct Cli {
    /// Chrome DevTools Protocol port exposed by WebView2.
    #[arg(long, default_value_t = DEFAULT_DEBUG_PORT)]
    debug_port: u16,

    /// Launch FL Studio with WebView2 remote debugging enabled before attaching.
    #[arg(long)]
    launch: bool,

    /// Explicit path to FL64.exe. If omitted, FL_STUDIO_EXE and common install paths are checked.
    #[arg(long)]
    fl: Option<PathBuf>,

    /// Maximum time to wait for FL and the Gopher WebView target.
    #[arg(long, default_value_t = DEFAULT_WAIT_SECONDS)]
    wait_seconds: u64,

    /// Case-insensitive text used to identify the Gopher CDP target by title or URL.
    #[arg(long, default_value = "gopher")]
    target_match: String,

    #[command(subcommand)]
    action: Option<Action>,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Verify script_handler and request FL's native MCP tool catalog.
    Probe {
        /// Print the complete raw tool catalog instead of the compact summary.
        #[arg(long)]
        raw: bool,
    },

    /// Call one tool from the catalog. Run `probe --raw` first to discover exact names/schemas.
    Call {
        /// Exact MCP tool name reported by FL Studio.
        tool: String,

        /// JSON object passed as the tool arguments.
        #[arg(long, default_value = "{}")]
        args: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpTarget {
    #[serde(default)]
    id: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    web_socket_debugger_url: String,
}

struct RawWebSocket {
    stream: TcpStream,
    mask_counter: u32,
}

impl RawWebSocket {
    fn connect(url: &str) -> Result<Self> {
        let (host, port, path) = parse_ws_url(url)?;
        let mut stream = TcpStream::connect((host.as_str(), port))
            .with_context(|| format!("failed to connect to CDP WebSocket at {host}:{port}"))?;
        let io_timeout = Some(Duration::from_secs(WS_IO_TIMEOUT_SECONDS));
        stream.set_read_timeout(io_timeout)?;
        stream.set_write_timeout(io_timeout)?;

        // A valid 16-byte WebSocket nonce encoded as base64. Randomness is not
        // security-sensitive here because CDP is bound to localhost and the key
        // only participates in the RFC 6455 handshake challenge.
        const WS_KEY: &str = "R2hvc3RGTENEUFByb2JlIQ==";
        write!(
            stream,
            "GET {path} HTTP/1.1\r\n\
             Host: {host}:{port}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {WS_KEY}\r\n\
             Sec-WebSocket-Version: 13\r\n\
             \r\n"
        )?;
        stream.flush()?;

        let headers = read_http_headers(&mut stream)?;
        let status = headers.lines().next().unwrap_or_default();
        if !status.contains(" 101 ") {
            bail!("CDP WebSocket upgrade failed: {status}\n{headers}");
        }

        Ok(Self {
            stream,
            mask_counter: 1,
        })
    }

    fn send_text(&mut self, text: &str) -> Result<()> {
        self.send_frame(0x1, text.as_bytes())
    }

    fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> Result<()> {
        let mut header = Vec::with_capacity(14);
        header.push(0x80 | (opcode & 0x0f));
        match payload.len() {
            len if len < 126 => header.push(0x80 | len as u8),
            len if len <= u16::MAX as usize => {
                header.push(0x80 | 126);
                header.extend_from_slice(&(len as u16).to_be_bytes());
            }
            len => {
                header.push(0x80 | 127);
                header.extend_from_slice(&(len as u64).to_be_bytes());
            }
        }

        let mask = self.mask_counter.to_be_bytes();
        self.mask_counter = self.mask_counter.wrapping_add(1).max(1);
        header.extend_from_slice(&mask);

        let mut masked = Vec::with_capacity(payload.len());
        for (index, byte) in payload.iter().enumerate() {
            masked.push(*byte ^ mask[index % 4]);
        }

        self.stream.write_all(&header)?;
        self.stream.write_all(&masked)?;
        self.stream.flush()?;
        Ok(())
    }

    fn read_text(&mut self) -> Result<String> {
        let mut assembled = Vec::new();
        let mut assembling_text = false;

        loop {
            let mut first = [0u8; 2];
            self.stream.read_exact(&mut first)?;
            let fin = first[0] & 0x80 != 0;
            let opcode = first[0] & 0x0f;
            let masked = first[1] & 0x80 != 0;
            let mut len = (first[1] & 0x7f) as u64;

            if len == 126 {
                let mut bytes = [0u8; 2];
                self.stream.read_exact(&mut bytes)?;
                len = u16::from_be_bytes(bytes) as u64;
            } else if len == 127 {
                let mut bytes = [0u8; 8];
                self.stream.read_exact(&mut bytes)?;
                len = u64::from_be_bytes(bytes);
            }
            if len as usize > MAX_WS_MESSAGE_BYTES {
                bail!("CDP WebSocket message exceeded {MAX_WS_MESSAGE_BYTES} bytes");
            }

            let mask = if masked {
                let mut key = [0u8; 4];
                self.stream.read_exact(&mut key)?;
                Some(key)
            } else {
                None
            };

            let mut payload = vec![0u8; len as usize];
            self.stream.read_exact(&mut payload)?;
            if let Some(mask) = mask {
                for (index, byte) in payload.iter_mut().enumerate() {
                    *byte ^= mask[index % 4];
                }
            }

            match opcode {
                0x0 => {
                    if !assembling_text {
                        bail!("unexpected WebSocket continuation frame");
                    }
                    assembled.extend_from_slice(&payload);
                    if fin {
                        return String::from_utf8(assembled)
                            .context("CDP returned non-UTF-8 text frame");
                    }
                }
                0x1 => {
                    if assembling_text {
                        bail!("new WebSocket text frame arrived during fragmented message");
                    }
                    if fin {
                        return String::from_utf8(payload)
                            .context("CDP returned non-UTF-8 text frame");
                    }
                    assembling_text = true;
                    assembled.extend_from_slice(&payload);
                }
                0x8 => bail!("CDP WebSocket closed"),
                0x9 => self.send_frame(0xA, &payload)?,
                0xA => {}
                _ => {}
            }
        }
    }
}

struct CdpClient {
    socket: RawWebSocket,
    next_id: u64,
}

impl CdpClient {
    fn connect(url: &str, debug_port: u16) -> Result<Self> {
        let fixed = fix_debug_ws_url(url, debug_port);
        Ok(Self {
            socket: RawWebSocket::connect(&fixed)?,
            next_id: 1,
        })
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        self.socket
            .send_text(&request.to_string())
            .with_context(|| format!("failed to send CDP request {method}"))?;

        loop {
            let message = self.socket.read_text().context("failed to read CDP response")?;
            let payload: Value = serde_json::from_str(&message)
                .context("CDP returned malformed JSON")?;
            if payload.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = payload.get("error") {
                bail!("CDP {method} failed: {error}");
            }
            return Ok(payload.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn evaluate(&mut self, expression: &str, await_promise: bool) -> Result<Value> {
        let result = self.call(
            "Runtime.evaluate",
            json!({
                "expression": expression,
                "awaitPromise": await_promise,
                "returnByValue": true,
                "userGesture": false,
            }),
        )?;
        if let Some(exception) = result.get("exceptionDetails") {
            let text = exception
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("JavaScript evaluation failed");
            let description = exception
                .pointer("/exception/description")
                .and_then(Value::as_str)
                .unwrap_or("");
            bail!("{text}: {description}");
        }
        Ok(result
            .pointer("/result/value")
            .cloned()
            .unwrap_or(Value::Null))
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let wait = Duration::from_secs(cli.wait_seconds);

    if cli.launch {
        if cdp_targets(cli.debug_port).is_err() {
            let fl = resolve_fl_executable(cli.fl.as_deref())?;
            launch_fl_with_debugging(&fl, cli.debug_port)?;
            eprintln!(
                "Launched {} with WebView2 debugging on port {}.",
                fl.display(),
                cli.debug_port
            );
        } else {
            eprintln!("CDP endpoint is already available; attaching to the existing FL Studio process.");
        }
    }

    wait_for_cdp(cli.debug_port, wait)?;
    let target = wait_for_gopher_target(cli.debug_port, &cli.target_match, wait)?;
    eprintln!(
        "Attached target: {} | {} | {}",
        target.title, target.kind, target.url
    );

    let mut cdp = CdpClient::connect(&target.web_socket_debugger_url, cli.debug_port)?;
    cdp.call("Runtime.enable", json!({}))?;

    let probe = cdp.evaluate(handler_probe_script(), false)?;
    let present = probe.get("present").and_then(Value::as_bool).unwrap_or(false);
    println!(
        "script_handler probe: {}",
        serde_json::to_string_pretty(&probe)?
    );
    if !present {
        bail!(
            "Gopher target was found, but script_handler is not visible in its page context. \
             This FL/Gopher build may have changed its WebView host-object exposure."
        );
    }

    match cli.action.unwrap_or(Action::Probe { raw: false }) {
        Action::Probe { raw } => {
            let catalog = request_tool_catalog(&mut cdp)?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&catalog)?);
            } else {
                print_catalog_summary(&catalog)?;
            }
        }
        Action::Call { tool, args } => {
            let args: Value = serde_json::from_str(&args)
                .with_context(|| format!("--args must be valid JSON, got: {args}"))?;
            if !args.is_object() {
                bail!("--args must be a JSON object");
            }
            let result = call_fl_tool(&mut cdp, &tool, args)?;
            println!("{}", pretty_payload(result));
        }
    }

    Ok(())
}

fn resolve_fl_executable(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return validate_fl_path(path);
    }
    if let Ok(path) = env::var("FL_STUDIO_EXE") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }

    let candidates = [
        r"D:\Image-Line\FL Studio 2026\FL64.exe",
        r"C:\Program Files\Image-Line\FL Studio 2025\FL64.exe",
        r"C:\Program Files\Image-Line\FL Studio 2024\FL64.exe",
    ];
    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Ok(path);
        }
    }

    bail!(
        "FL Studio executable not found. Pass --fl <path-to-FL64.exe> or set FL_STUDIO_EXE."
    )
}

fn validate_fl_path(path: &Path) -> Result<PathBuf> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("FL Studio executable does not exist: {}", path.display()))?;
    if !metadata.is_file() {
        bail!("FL Studio path is not a file: {}", path.display());
    }
    Ok(path.to_path_buf())
}

fn launch_fl_with_debugging(fl: &Path, port: u16) -> Result<()> {
    let existing = env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
    let debug_arg = format!("--remote-debugging-port={port}");
    let browser_args = if existing.trim().is_empty() {
        debug_arg
    } else if existing.contains("--remote-debugging-port=") {
        existing
    } else {
        format!("{existing} {debug_arg}")
    };

    ProcessCommand::new(fl)
        .env("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", browser_args)
        .spawn()
        .with_context(|| format!("failed to launch {}", fl.display()))?;
    Ok(())
}

fn wait_for_cdp(port: u16, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if cdp_targets(port).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }
    bail!(
        "WebView2 CDP endpoint did not appear on 127.0.0.1:{port}. \
         If FL Studio was already running, close it completely and relaunch it through this probe so the WebView2 environment receives the debugging flag."
    )
}

fn wait_for_gopher_target(port: u16, needle: &str, timeout: Duration) -> Result<CdpTarget> {
    let needle = needle.to_lowercase();
    let started = Instant::now();
    let mut prompted = false;
    while started.elapsed() < timeout {
        if let Ok(targets) = cdp_targets(port) {
            if let Some(target) = targets
                .into_iter()
                .find(|target| target_matches(target, &needle))
            {
                return Ok(target);
            }
        }
        if !prompted {
            eprintln!(
                "Waiting for the Gopher WebView. Open Gopher in FL Studio (Alt+F1) if it is not already visible..."
            );
            prompted = true;
        }
        thread::sleep(Duration::from_millis(750));
    }
    bail!("no CDP target matching '{needle}' appeared before timeout")
}

fn target_matches(target: &CdpTarget, needle: &str) -> bool {
    if target.web_socket_debugger_url.is_empty() {
        return false;
    }
    let haystack = format!("{} {} {}", target.title, target.url, target.id).to_lowercase();
    haystack.contains(needle)
}

fn cdp_targets(port: u16) -> Result<Vec<CdpTarget>> {
    let body = http_get(port, "/json")?;
    serde_json::from_str(&body).context("failed to parse Chrome DevTools target list")
}

fn http_get(port: u16, path: &str) -> Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .with_context(|| format!("failed to connect to CDP port {port}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| anyhow!("malformed HTTP response from CDP endpoint"))?;
    let status = headers.lines().next().unwrap_or_default();
    if !status.contains(" 200 ") {
        bail!("CDP HTTP request failed: {status}");
    }
    Ok(body.to_owned())
}

fn read_http_headers(stream: &mut TcpStream) -> Result<String> {
    let mut bytes = Vec::with_capacity(1024);
    let mut byte = [0u8; 1];
    while bytes.len() < 64 * 1024 {
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return String::from_utf8(bytes).context("WebSocket upgrade headers were not UTF-8");
        }
    }
    bail!("WebSocket upgrade headers exceeded 64 KiB")
}

fn parse_ws_url(url: &str) -> Result<(String, u16, String)> {
    let rest = url
        .strip_prefix("ws://")
        .ok_or_else(|| anyhow!("only ws:// CDP URLs are supported, got {url}"))?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, tail)) => (authority, format!("/{tail}")),
        None => (rest, "/".to_owned()),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(']') => {
            let port = port
                .parse::<u16>()
                .with_context(|| format!("invalid WebSocket port in {url}"))?;
            (host.to_owned(), port)
        }
        _ => (authority.to_owned(), 80),
    };
    if host.is_empty() {
        bail!("missing WebSocket host in {url}");
    }
    Ok((host, port, path))
}

fn fix_debug_ws_url(url: &str, debug_port: u16) -> String {
    for host in ["localhost", "127.0.0.1"] {
        let prefix = format!("ws://{host}/");
        if url.starts_with(&prefix) {
            return url.replacen(&prefix, &format!("ws://{host}:{debug_port}/"), 1);
        }
    }
    url.to_owned()
}

fn handler_probe_script() -> &'static str {
    r#"(() => {
  let direct = false;
  let projected = false;
  try { direct = typeof script_handler === 'object' && !!script_handler; } catch (_) {}
  try {
    projected = !!(window.chrome && window.chrome.webview && window.chrome.webview.hostObjects && window.chrome.webview.hostObjects.script_handler);
  } catch (_) {}
  return {
    present: direct || projected,
    direct,
    projected,
    title: document.title,
    href: location.href
  };
})()"#
}

fn host_resolver_js() -> &'static str {
    r#"function ghostGetScriptHandler() {
  try {
    if (typeof script_handler === 'object' && script_handler) return script_handler;
  } catch (_) {}
  try {
    if (window.chrome && window.chrome.webview && window.chrome.webview.hostObjects)
      return window.chrome.webview.hostObjects.script_handler || null;
  } catch (_) {}
  return null;
}"#
}

fn request_tool_catalog(cdp: &mut CdpClient) -> Result<Value> {
    let script = format!(
        r#"(() => {{
{resolver}
  return new Promise((resolve, reject) => {{
    const sh = ghostGetScriptHandler();
    if (!sh) return reject(new Error('script_handler unavailable'));
    const helper = window.flHelper = window.flHelper || {{}};
    const previous = helper.onMCPTools;
    let timer = null;
    const restore = () => {{
      if (timer) clearTimeout(timer);
      if (typeof previous === 'function') helper.onMCPTools = previous;
      else delete helper.onMCPTools;
    }};
    helper.onMCPTools = payload => {{ restore(); resolve(payload); }};
    timer = setTimeout(() => {{ restore(); reject(new Error('MCPTools timeout')); }}, {timeout});
    try {{ sh.MCPTools = '1'; }}
    catch (error) {{ restore(); reject(error); }}
  }});
}})()"#,
        resolver = host_resolver_js(),
        timeout = BRIDGE_TIMEOUT_MS,
    );
    let payload = cdp.evaluate(&script, true)?;
    Ok(parse_maybe_json_string(payload))
}

fn call_fl_tool(cdp: &mut CdpClient, tool: &str, args: Value) -> Result<Value> {
    let envelope = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool,
            "arguments": args,
        }
    });
    let request_json = envelope.to_string();
    let script = format!(
        r#"(() => {{
{resolver}
  const request = {request};
  return new Promise((resolve, reject) => {{
    const sh = ghostGetScriptHandler();
    if (!sh) return reject(new Error('script_handler unavailable'));
    const helper = window.flHelper = window.flHelper || {{}};
    const previous = helper.onRunJson;
    let timer = null;
    const restore = () => {{
      if (timer) clearTimeout(timer);
      if (typeof previous === 'function') helper.onRunJson = previous;
      else delete helper.onRunJson;
    }};
    helper.onRunJson = payload => {{ restore(); resolve(payload); }};
    timer = setTimeout(() => {{ restore(); reject(new Error('runJson timeout')); }}, {timeout});
    try {{ sh.runJson = JSON.stringify(request); }}
    catch (error) {{ restore(); reject(error); }}
  }});
}})()"#,
        resolver = host_resolver_js(),
        request = request_json,
        timeout = BRIDGE_TIMEOUT_MS,
    );
    let payload = cdp.evaluate(&script, true)?;
    Ok(parse_maybe_json_string(payload))
}

fn parse_maybe_json_string(value: Value) -> Value {
    match value {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        other => other,
    }
}

fn tool_array(catalog: &Value) -> Option<&Vec<Value>> {
    catalog
        .as_array()
        .or_else(|| catalog.get("tools").and_then(Value::as_array))
}

fn print_catalog_summary(catalog: &Value) -> Result<()> {
    let Some(tools) = tool_array(catalog) else {
        println!(
            "Tool catalog returned an unexpected shape:\n{}",
            serde_json::to_string_pretty(catalog)?
        );
        return Ok(());
    };

    println!("FL native tool catalog: {} tools", tools.len());
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unnamed>");
        let description = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .replace('\n', " ");
        if description.is_empty() {
            println!("  {name}");
        } else {
            println!("  {name} — {description}");
        }
    }
    println!("\nRun `probe --raw` to print exact input schemas before calling any mutation tool.");
    Ok(())
}

fn pretty_payload(value: Value) -> String {
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_gopher_target_by_title_or_url() {
        let target = CdpTarget {
            id: "abc".into(),
            kind: "page".into(),
            title: "Gopher".into(),
            url: "https://gopher-fls.image-line.com/".into(),
            web_socket_debugger_url: "ws://127.0.0.1:9222/devtools/page/abc".into(),
        };
        assert!(target_matches(&target, "gopher"));
        assert!(!target_matches(&target, "sounds.cloud"));
    }

    #[test]
    fn accepts_catalog_array_or_tools_envelope() {
        let array = json!([{ "name": "inspect_project" }]);
        assert_eq!(tool_array(&array).unwrap().len(), 1);
        let envelope = json!({ "tools": [{ "name": "inspect_project" }] });
        assert_eq!(tool_array(&envelope).unwrap().len(), 1);
    }

    #[test]
    fn parses_json_returned_as_bare_string() {
        let parsed = parse_maybe_json_string(Value::String("{\"ok\":true}".into()));
        assert_eq!(parsed["ok"], true);
    }

    #[test]
    fn fixes_missing_debug_port_in_chrome_target_url() {
        assert_eq!(
            fix_debug_ws_url("ws://localhost/devtools/page/abc", 9222),
            "ws://localhost:9222/devtools/page/abc"
        );
    }

    #[test]
    fn parses_local_cdp_ws_url() {
        let (host, port, path) =
            parse_ws_url("ws://127.0.0.1:9222/devtools/page/abc").unwrap();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 9222);
        assert_eq!(path, "/devtools/page/abc");
    }
}
