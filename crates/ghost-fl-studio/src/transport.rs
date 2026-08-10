use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

const WS_IO_TIMEOUT_SECONDS: u64 = 30;
const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct TransportConfig {
    pub debug_port: u16,
    pub target_match: String,
    pub connect_timeout: Duration,
    pub bridge_timeout: Duration,
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

pub(crate) struct GopherConnection {
    cdp: CdpClient,
    pub target_id: String,
    pub target_title: String,
    pub target_kind: String,
    bridge_timeout_ms: u128,
}

impl GopherConnection {
    pub fn connect(config: &TransportConfig) -> Result<Self> {
        wait_for_cdp(config.debug_port, config.connect_timeout)?;
        let target = wait_for_gopher_target(
            config.debug_port,
            &config.target_match,
            config.connect_timeout,
        )?;
        let mut cdp = CdpClient::connect(&target.web_socket_debugger_url, config.debug_port)?;
        cdp.call("Runtime.enable", json!({}))?;
        let probe = cdp.evaluate(handler_probe_script(), false)?;
        if !probe
            .get("present")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            bail!(
                "Gopher target was found, but script_handler is not visible in its page context"
            );
        }
        Ok(Self {
            cdp,
            target_id: target.id,
            target_title: target.title,
            target_kind: target.kind,
            bridge_timeout_ms: config.bridge_timeout.as_millis(),
        })
    }

    pub fn request_catalog_payload(&mut self) -> Result<Value> {
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
            timeout = self.bridge_timeout_ms,
        );
        self.cdp.evaluate(&script, true)
    }

    pub fn call_tool_request(&mut self, request_json: &str) -> Result<Value> {
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
            timeout = self.bridge_timeout_ms,
        );
        let payload = self.cdp.evaluate(&script, true)?;
        Ok(parse_maybe_json_string(payload))
    }
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
        let timeout = Some(Duration::from_secs(WS_IO_TIMEOUT_SECONDS));
        stream.set_read_timeout(timeout)?;
        stream.set_write_timeout(timeout)?;

        const WS_KEY: &str = "R2hvc3RGTENEUFByb2JlIQ==";
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {WS_KEY}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        )?;
        stream.flush()?;
        let headers = read_http_headers(&mut stream)?;
        let status = headers.lines().next().unwrap_or_default();
        if !status.contains(" 101 ") {
            bail!("CDP WebSocket upgrade failed: {status}");
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
            if len as usize > MAX_MESSAGE_BYTES {
                bail!("CDP WebSocket frame exceeded {MAX_MESSAGE_BYTES} bytes");
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
                    if assembled.len() + payload.len() > MAX_MESSAGE_BYTES {
                        bail!("fragmented CDP WebSocket message exceeded {MAX_MESSAGE_BYTES} bytes");
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
        let request = json!({"id": id, "method": method, "params": params});
        self.socket
            .send_text(&request.to_string())
            .with_context(|| format!("failed to send CDP request {method}"))?;
        loop {
            let message = self.socket.read_text().context("failed to read CDP response")?;
            let payload: Value =
                serde_json::from_str(&message).context("CDP returned malformed JSON")?;
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
                "userGesture": false
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

fn wait_for_cdp(port: u16, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match cdp_targets(port) {
            Ok(_) => return Ok(()),
            Err(error) => last_error = Some(error.to_string()),
        }
        thread::sleep(Duration::from_millis(300));
    }
    bail!(
        "WebView2 CDP endpoint did not become usable on 127.0.0.1:{port}: {}",
        last_error.unwrap_or_else(|| "no response".into())
    )
}

fn wait_for_gopher_target(port: u16, needle: &str, timeout: Duration) -> Result<CdpTarget> {
    let needle = needle.to_lowercase();
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Ok(targets) = cdp_targets(port) {
            if let Some(target) = targets
                .into_iter()
                .find(|target| target_matches(target, &needle))
            {
                return Ok(target);
            }
        }
        thread::sleep(Duration::from_millis(500));
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
    let mut errors = Vec::new();
    for path in ["/json/list", "/json"] {
        match http_get(port, path) {
            Ok(body) => match serde_json::from_str(&body) {
                Ok(targets) => return Ok(targets),
                Err(error) => errors.push(format!("{path}: invalid JSON: {error}")),
            },
            Err(error) => errors.push(format!("{path}: {error}")),
        }
    }
    bail!("failed to read Chrome DevTools target list ({})", errors.join("; "))
}

fn http_get(port: u16, path: &str) -> Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .with_context(|| format!("failed to connect to CDP port {port}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;
    let headers = read_http_headers(&mut stream)?;
    let status = headers.lines().next().unwrap_or_default();
    if !status.contains(" 200 ") {
        bail!("CDP HTTP request failed: {status}");
    }
    let body = if header_value(&headers, "transfer-encoding")
        .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"))
    {
        read_chunked_body(&mut stream)?
    } else if let Some(length) = header_value(&headers, "content-length") {
        let length = length
            .parse::<usize>()
            .context("invalid Content-Length from CDP endpoint")?;
        if length > MAX_MESSAGE_BYTES {
            bail!("CDP HTTP body exceeded {MAX_MESSAGE_BYTES} bytes");
        }
        let mut body = vec![0u8; length];
        stream.read_exact(&mut body)?;
        body
    } else {
        let mut body = Vec::new();
        stream.read_to_end(&mut body)?;
        if body.len() > MAX_MESSAGE_BYTES {
            bail!("CDP HTTP body exceeded {MAX_MESSAGE_BYTES} bytes");
        }
        body
    };
    String::from_utf8(body).context("CDP HTTP body was not UTF-8")
}

fn header_value<'a>(headers: &'a str, wanted: &str) -> Option<&'a str> {
    headers.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case(wanted)
            .then_some(value.trim())
    })
}

fn read_http_headers(stream: &mut TcpStream) -> Result<String> {
    let mut bytes = Vec::with_capacity(1024);
    let mut byte = [0u8; 1];
    while bytes.len() < 64 * 1024 {
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return String::from_utf8(bytes).context("HTTP headers were not UTF-8");
        }
    }
    bail!("HTTP headers exceeded 64 KiB")
}

fn read_crlf_line(stream: &mut TcpStream) -> Result<String> {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    while bytes.len() < 64 * 1024 {
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n") {
            bytes.truncate(bytes.len() - 2);
            return String::from_utf8(bytes).context("chunked HTTP line was not UTF-8");
        }
    }
    bail!("chunked HTTP line exceeded 64 KiB")
}

fn read_chunked_body(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line = read_crlf_line(stream)?;
        let size_text = line.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16)
            .with_context(|| format!("invalid chunk size '{size_text}'"))?;
        if size == 0 {
            loop {
                if read_crlf_line(stream)?.is_empty() {
                    break;
                }
            }
            break;
        }
        if body.len() + size > MAX_MESSAGE_BYTES {
            bail!("chunked CDP HTTP body exceeded {MAX_MESSAGE_BYTES} bytes");
        }
        let start = body.len();
        body.resize(start + size, 0);
        stream.read_exact(&mut body[start..])?;
        let mut crlf = [0u8; 2];
        stream.read_exact(&mut crlf)?;
        if crlf != *b"\r\n" {
            bail!("malformed chunk terminator from CDP endpoint");
        }
    }
    Ok(body)
}

fn parse_ws_url(url: &str) -> Result<(String, u16, String)> {
    let rest = url
        .strip_prefix("ws://")
        .ok_or_else(|| anyhow!("only ws:// CDP URLs are supported"))?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, tail)) => (authority, format!("/{tail}")),
        None => (rest, "/".to_owned()),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(']') => {
            let port = port.parse::<u16>().context("invalid CDP WebSocket port")?;
            (host.to_owned(), port)
        }
        _ => (authority.to_owned(), 80),
    };
    if host.is_empty() {
        bail!("missing WebSocket host");
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
  return { present: direct || projected, direct, projected, title: document.title };
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

fn parse_maybe_json_string(value: Value) -> Value {
    match value {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_missing_debug_port_in_cdp_url() {
        assert_eq!(
            fix_debug_ws_url("ws://127.0.0.1/devtools/page/abc", 9222),
            "ws://127.0.0.1:9222/devtools/page/abc"
        );
    }

    #[test]
    fn parses_local_websocket_url() {
        let (host, port, path) = parse_ws_url("ws://127.0.0.1:9222/devtools/page/abc").unwrap();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 9222);
        assert_eq!(path, "/devtools/page/abc");
    }
}
