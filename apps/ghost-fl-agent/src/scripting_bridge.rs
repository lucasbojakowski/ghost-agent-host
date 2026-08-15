use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const PROTOCOL_VERSION: u32 = 1;
const BRIDGE_NAME: &str = "ghost-fl-scripting";
const MAX_FRAME_BYTES: usize = 64 * 1024;
const MAX_RECEIVE_BUFFER_BYTES: usize = 2 * MAX_FRAME_BYTES;
const IO_CHUNK_BYTES: usize = 4096;
const SOCKET_POLL: Duration = Duration::from_millis(20);
const ALLOWED_MODULES: &[&str] = &[
    "arrangement",
    "channels",
    "general",
    "mixer",
    "patterns",
    "playlist",
    "plugins",
    "transport",
    "ui",
];

#[derive(Clone)]
pub struct ScriptingBridge {
    state: Arc<Mutex<BridgeState>>,
    next_id: Arc<AtomicU64>,
    timeout: Duration,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptingStatusSnapshot {
    pub listening: bool,
    pub bind: String,
    pub connected: bool,
    pub hello: Option<HelloSnapshot>,
    pub reconnects: u64,
    pub malformed_messages: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloSnapshot {
    pub bridge: String,
    pub protocol: u32,
    pub fl_version: Option<String>,
    pub scripting_api_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptingProbe {
    pub status: ScriptingStatusSnapshot,
    pub observations: Vec<ProbeEntry>,
    pub reversible_mutation: ReversibleMutationProbe,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeEntry {
    pub label: String,
    pub module: String,
    pub function: String,
    pub args: Vec<Value>,
    pub ok: bool,
    pub value: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReversibleMutationProbe {
    pub operation: &'static str,
    pub attempted: bool,
    pub original_track: Option<i64>,
    pub temporary_track: Option<i64>,
    pub changed: bool,
    pub restored: bool,
    pub error: Option<String>,
}

struct BridgeState {
    connection: Option<BridgeConnection>,
    status: BridgeStatus,
}

#[derive(Debug, Default)]
struct BridgeStatus {
    listening: bool,
    bind: String,
    connected: bool,
    hello: Option<HelloSnapshot>,
    reconnects: u64,
    malformed_messages: u64,
    last_error: Option<String>,
}

impl BridgeStatus {
    fn snapshot(&self) -> ScriptingStatusSnapshot {
        ScriptingStatusSnapshot {
            listening: self.listening,
            bind: self.bind.clone(),
            connected: self.connected,
            hello: self.hello.clone(),
            reconnects: self.reconnects,
            malformed_messages: self.malformed_messages,
            last_error: self.last_error.clone(),
        }
    }

    fn connected(&mut self, hello: HelloSnapshot) {
        if self.hello.is_some() || self.last_error.is_some() {
            self.reconnects = self.reconnects.saturating_add(1);
        }
        self.connected = true;
        self.hello = Some(hello);
        self.last_error = None;
    }

    fn disconnected(&mut self, error: impl Into<String>) {
        self.connected = false;
        self.hello = None;
        self.last_error = Some(error.into());
    }
}

struct BridgeConnection {
    stream: TcpStream,
    receive_buffer: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct HelloMessage {
    protocol: u32,
    bridge: String,
    #[serde(default)]
    fl_version: Option<String>,
    #[serde(default)]
    scripting_api_version: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ResultMessage {
    id: u64,
    ok: bool,
    #[serde(default)]
    value: Option<Value>,
    #[serde(default)]
    error: Option<WireError>,
}

#[derive(Debug, Deserialize)]
struct WireError {
    #[serde(default)]
    kind: Option<String>,
    message: String,
}

#[derive(Debug)]
enum IncomingMessage {
    Hello(HelloMessage),
    Result(ResultMessage),
}

struct CallFailure {
    message: String,
    disconnect: bool,
}

impl ScriptingBridge {
    pub fn start(bind: &str, timeout: Duration) -> Result<Self> {
        let address = parse_loopback_address(bind)?;
        let listener = TcpListener::bind(address)
            .with_context(|| format!("failed to bind FL scripting bridge at {bind}"))?;
        let actual_bind = listener.local_addr()?.to_string();
        let state = Arc::new(Mutex::new(BridgeState {
            connection: None,
            status: BridgeStatus {
                listening: true,
                bind: actual_bind,
                ..BridgeStatus::default()
            },
        }));
        let accept_state = Arc::clone(&state);
        thread::Builder::new()
            .name("ghost-fl-scripting-listener".into())
            .spawn(move || accept_loop(listener, accept_state, timeout))
            .context("failed to start FL scripting listener thread")?;

        Ok(Self {
            state,
            next_id: Arc::new(AtomicU64::new(1)),
            timeout,
        })
    }

    pub fn status(&self) -> ScriptingStatusSnapshot {
        lock_state(&self.state).status.snapshot()
    }

    pub fn call(&self, module: &str, function: &str, args: Vec<Value>) -> Result<Value> {
        if !ALLOWED_MODULES.contains(&module) {
            bail!("FL scripting module '{module}' is not allowlisted");
        }
        if !is_safe_identifier(function) {
            bail!("FL scripting function '{function}' is not a safe callable identifier");
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let frame = encode_call(id, module, function, &args)?;
        let mut state = lock_state(&self.state);
        let BridgeState {
            connection: connection_slot,
            status,
        } = &mut *state;
        let Some(connection) = connection_slot.as_mut() else {
            bail!("FL scripting device is not connected");
        };

        match perform_call(
            connection,
            status,
            id,
            &frame,
            module,
            function,
            self.timeout,
        ) {
            Ok(value) => Ok(value),
            Err(failure) => {
                if failure.disconnect {
                    *connection_slot = None;
                    status.disconnected(failure.message.clone());
                } else {
                    status.last_error = Some(failure.message.clone());
                }
                Err(anyhow!(failure.message))
            }
        }
    }

    pub fn run_probe(&self) -> Result<ScriptingProbe> {
        let initial_status = self.status();
        if !initial_status.connected {
            bail!(
                "FL scripting bridge is not ready; waiting for device_Ghost.py handshake at {}",
                initial_status.bind
            );
        }

        let mut observations = vec![
            self.probe("scriptingApiVersion", "general", "getVersion", vec![]),
            self.probe("flVersion", "ui", "getVersion", vec![json!(5)]),
            self.probe("projectTitle", "general", "getProjectTitle", vec![]),
            self.probe("projectChangedFlag", "general", "getChangedFlag", vec![]),
            self.probe("safeToEdit", "general", "safeToEdit", vec![]),
            self.probe("selectedChannel", "channels", "channelNumber", vec![]),
            self.probe("selectedMixerTrack", "mixer", "trackNumber", vec![]),
            self.probe("mixerTrackCount", "mixer", "trackCount", vec![]),
            self.probe("currentPattern", "patterns", "patternNumber", vec![]),
            self.probe("patternCount", "patterns", "patternCount", vec![]),
        ];

        if let Some(pattern) = observation_i64(&observations, "currentPattern") {
            observations.push(self.probe(
                "currentPatternName",
                "patterns",
                "getPatternName",
                vec![json!(pattern)],
            ));
        }

        observations.extend([
            self.probe(
                "arrangementSelectionStart",
                "arrangement",
                "selectionStart",
                vec![],
            ),
            self.probe(
                "arrangementSelectionEnd",
                "arrangement",
                "selectionEnd",
                vec![],
            ),
            self.probe("focusedPluginName", "ui", "getFocusedPluginName", vec![]),
            self.probe(
                "focusedWindowCaption",
                "ui",
                "getFocusedFormCaption",
                vec![],
            ),
            self.probe("songPosition", "transport", "getSongPos", vec![]),
            self.probe("songPositionHint", "transport", "getSongPosHint", vec![]),
            self.probe("loopMode", "transport", "getLoopMode", vec![]),
            self.probe("isPlaying", "transport", "isPlaying", vec![]),
        ]);

        if let (Some(start), Some(end)) = (
            observation_i64(&observations, "arrangementSelectionStart"),
            observation_i64(&observations, "arrangementSelectionEnd"),
        ) {
            observations.push(ProbeEntry {
                label: "arrangementSelectionActive".into(),
                module: "derived".into(),
                function: "selectionStart!=selectionEnd".into(),
                args: vec![],
                ok: true,
                value: Some(json!(start != end)),
                error: None,
            });
        }

        let reversible_mutation = self.reversible_mixer_selection_probe(
            observation_i64(&observations, "safeToEdit"),
            observation_i64(&observations, "selectedMixerTrack"),
            observation_i64(&observations, "mixerTrackCount"),
        );
        Ok(ScriptingProbe {
            status: self.status(),
            observations,
            reversible_mutation,
        })
    }

    fn probe(&self, label: &str, module: &str, function: &str, args: Vec<Value>) -> ProbeEntry {
        match self.call(module, function, args.clone()) {
            Ok(value) => ProbeEntry {
                label: label.into(),
                module: module.into(),
                function: function.into(),
                args,
                ok: true,
                value: Some(value),
                error: None,
            },
            Err(error) => ProbeEntry {
                label: label.into(),
                module: module.into(),
                function: function.into(),
                args,
                ok: false,
                value: None,
                error: Some(error.to_string()),
            },
        }
    }

    fn reversible_mixer_selection_probe(
        &self,
        safe_to_edit: Option<i64>,
        original_track: Option<i64>,
        track_count: Option<i64>,
    ) -> ReversibleMutationProbe {
        let mut report = ReversibleMutationProbe {
            operation: "mixer.setTrackNumber temporary selection + restore",
            attempted: false,
            original_track,
            temporary_track: None,
            changed: false,
            restored: false,
            error: None,
        };
        if safe_to_edit != Some(1) {
            report.error = Some("skipped because general.safeToEdit() did not return 1".into());
            return report;
        }
        let Some(original) = original_track else {
            report.error = Some("skipped because current mixer track was unavailable".into());
            return report;
        };
        if original < 0 || track_count.unwrap_or(0) < 2 {
            report.error = Some("skipped because no alternate mixer track was available".into());
            return report;
        }

        let temporary = if original == 0 { 1 } else { 0 };
        report.temporary_track = Some(temporary);
        report.attempted = true;
        if let Err(error) = self.call("mixer", "setTrackNumber", vec![json!(temporary)]) {
            report.error = Some(format!("temporary selection failed: {error}"));
            return report;
        }
        match self.call("mixer", "trackNumber", vec![]) {
            Ok(value) if value.as_i64() == Some(temporary) => report.changed = true,
            Ok(value) => {
                report.error = Some(format!(
                    "temporary mixer selection read back as {value}, expected {temporary}"
                ));
            }
            Err(error) => report.error = Some(format!("temporary readback failed: {error}")),
        }

        if let Err(error) = self.call("mixer", "setTrackNumber", vec![json!(original)]) {
            report.error = Some(append_error(
                report.error.take(),
                format!("restore call failed: {error}"),
            ));
            return report;
        }
        match self.call("mixer", "trackNumber", vec![]) {
            Ok(value) if value.as_i64() == Some(original) => report.restored = true,
            Ok(value) => {
                report.error = Some(append_error(
                    report.error.take(),
                    format!("restore read back as {value}, expected {original}"),
                ));
            }
            Err(error) => {
                report.error = Some(append_error(
                    report.error.take(),
                    format!("restore readback failed: {error}"),
                ));
            }
        }
        report
    }
}

fn perform_call(
    connection: &mut BridgeConnection,
    status: &mut BridgeStatus,
    id: u64,
    frame: &[u8],
    module: &str,
    function: &str,
    timeout: Duration,
) -> std::result::Result<Value, CallFailure> {
    connection
        .stream
        .write_all(frame)
        .map_err(|error| CallFailure {
            message: format!("FL scripting write failed: {error}"),
            disconnect: true,
        })?;
    let deadline = Instant::now() + timeout;
    loop {
        match connection.read_frame() {
            Ok(Some(frame)) => match parse_incoming(&frame) {
                Ok(IncomingMessage::Result(result)) if result.id == id => {
                    return decode_result(result).map_err(|error| CallFailure {
                        message: error.to_string(),
                        disconnect: false,
                    });
                }
                Ok(IncomingMessage::Result(result)) => {
                    return Err(CallFailure {
                        message: format!(
                            "FL scripting result correlation mismatch: expected id {id}, got {}",
                            result.id
                        ),
                        disconnect: true,
                    });
                }
                Ok(IncomingMessage::Hello(hello)) => {
                    if hello.protocol != PROTOCOL_VERSION || hello.bridge != BRIDGE_NAME {
                        return Err(CallFailure {
                            message: "FL scripting hello changed protocol during a call".into(),
                            disconnect: true,
                        });
                    }
                    status.connected(hello_snapshot(hello));
                }
                Err(error) => {
                    status.malformed_messages = status.malformed_messages.saturating_add(1);
                    status.last_error = Some(error);
                }
            },
            Ok(None) => {}
            Err(error) => {
                return Err(CallFailure {
                    message: format!("FL scripting read failed: {error}"),
                    disconnect: true,
                });
            }
        }
        if Instant::now() >= deadline {
            return Err(CallFailure {
                message: format!("FL scripting call {module}.{function} timed out"),
                disconnect: true,
            });
        }
    }
}

fn accept_loop(listener: TcpListener, state: Arc<Mutex<BridgeState>>, timeout: Duration) {
    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                lock_state(&state).status.last_error =
                    Some(format!("FL scripting accept failed: {error}"));
                continue;
            }
        };
        let peer = match stream.peer_addr() {
            Ok(peer) => peer,
            Err(error) => {
                lock_state(&state).status.last_error =
                    Some(format!("failed to inspect scripting peer: {error}"));
                continue;
            }
        };
        if !peer.ip().is_loopback() {
            lock_state(&state).status.last_error =
                Some(format!("rejected non-loopback scripting peer {peer}"));
            continue;
        }
        match prepare_connection(stream, timeout) {
            Ok((connection, hello)) => {
                let mut state = lock_state(&state);
                state.connection = Some(connection);
                state.status.connected(hello);
            }
            Err(error) => lock_state(&state).status.disconnected(error.to_string()),
        }
    }
}

fn prepare_connection(
    stream: TcpStream,
    timeout: Duration,
) -> Result<(BridgeConnection, HelloSnapshot)> {
    stream
        .set_read_timeout(Some(SOCKET_POLL))
        .context("failed to configure FL scripting socket read timeout")?;
    stream
        .set_write_timeout(Some(timeout))
        .context("failed to configure FL scripting socket write timeout")?;
    stream
        .set_nodelay(true)
        .context("failed to configure FL scripting TCP_NODELAY")?;
    let mut connection = BridgeConnection {
        stream,
        receive_buffer: Vec::with_capacity(IO_CHUNK_BYTES * 2),
    };
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(frame) = connection.read_frame()? {
            match parse_incoming(&frame).map_err(anyhow::Error::msg)? {
                IncomingMessage::Hello(hello)
                    if hello.protocol == PROTOCOL_VERSION && hello.bridge == BRIDGE_NAME =>
                {
                    return Ok((connection, hello_snapshot(hello)));
                }
                IncomingMessage::Hello(hello) => bail!(
                    "FL scripting protocol mismatch: bridge='{}' protocol={}",
                    hello.bridge,
                    hello.protocol
                ),
                IncomingMessage::Result(_) => {
                    bail!("FL scripting client sent a result before the hello handshake");
                }
            }
        }
        if Instant::now() >= deadline {
            bail!("FL scripting hello handshake timed out");
        }
    }
}

fn hello_snapshot(hello: HelloMessage) -> HelloSnapshot {
    HelloSnapshot {
        bridge: hello.bridge,
        protocol: hello.protocol,
        fl_version: hello.fl_version,
        scripting_api_version: hello.scripting_api_version,
    }
}

impl BridgeConnection {
    fn read_frame(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        if let Some(frame) = take_frame(&mut self.receive_buffer)? {
            return Ok(Some(frame));
        }
        let mut chunk = [0_u8; IO_CHUNK_BYTES];
        match self.stream.read(&mut chunk) {
            Ok(0) => Err(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "FL scripting socket closed",
            )),
            Ok(count) => {
                if self.receive_buffer.len().saturating_add(count) > MAX_RECEIVE_BUFFER_BYTES {
                    self.receive_buffer.clear();
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidData,
                        "FL scripting receive buffer exceeded limit",
                    ));
                }
                self.receive_buffer.extend_from_slice(&chunk[..count]);
                take_frame(&mut self.receive_buffer)
            }
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

fn take_frame(buffer: &mut Vec<u8>) -> std::io::Result<Option<Vec<u8>>> {
    let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') else {
        if buffer.len() > MAX_FRAME_BYTES {
            buffer.clear();
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "FL scripting frame exceeded maximum size",
            ));
        }
        return Ok(None);
    };
    if newline > MAX_FRAME_BYTES {
        buffer.drain(..=newline);
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "FL scripting frame exceeded maximum size",
        ));
    }
    let mut frame: Vec<u8> = buffer.drain(..=newline).collect();
    frame.pop();
    if frame.last() == Some(&b'\r') {
        frame.pop();
    }
    if frame.is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "FL scripting frame was empty",
        ));
    }
    Ok(Some(frame))
}

fn encode_call(id: u64, module: &str, function: &str, args: &[Value]) -> Result<Vec<u8>> {
    let mut frame = serde_json::to_vec(&json!({
        "type": "call",
        "id": id,
        "module": module,
        "function": function,
        "args": args,
    }))?;
    if frame.len().saturating_add(1) > MAX_FRAME_BYTES {
        bail!("FL scripting call exceeded {MAX_FRAME_BYTES} bytes");
    }
    frame.push(b'\n');
    Ok(frame)
}

fn parse_incoming(frame: &[u8]) -> std::result::Result<IncomingMessage, String> {
    let value: Value = serde_json::from_slice(frame)
        .map_err(|error| format!("invalid FL scripting JSON frame: {error}"))?;
    let message_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "FL scripting frame is missing string 'type'".to_owned())?;
    match message_type {
        "hello" => serde_json::from_value(value)
            .map(IncomingMessage::Hello)
            .map_err(|error| format!("invalid FL scripting hello: {error}")),
        "result" => serde_json::from_value(value)
            .map(IncomingMessage::Result)
            .map_err(|error| format!("invalid FL scripting result: {error}")),
        other => Err(format!("unsupported FL scripting frame type '{other}'")),
    }
}

fn decode_result(result: ResultMessage) -> Result<Value> {
    if result.ok {
        return Ok(result.value.unwrap_or(Value::Null));
    }
    let error = result.error.map_or_else(
        || "FL scripting call failed without an error payload".into(),
        |error| match error.kind {
            Some(kind) => format!("{kind}: {}", error.message),
            None => error.message,
        },
    );
    Err(anyhow!(error))
}

fn observation_i64(observations: &[ProbeEntry], label: &str) -> Option<i64> {
    observations
        .iter()
        .find(|entry| entry.label == label && entry.ok)
        .and_then(|entry| entry.value.as_ref())
        .and_then(Value::as_i64)
}

fn parse_loopback_address(bind: &str) -> Result<SocketAddr> {
    let address: SocketAddr = bind
        .parse()
        .with_context(|| format!("invalid FL scripting bind address '{bind}'"))?;
    if !address.ip().is_loopback() {
        bail!("FL scripting bridge must bind to loopback, got {address}");
    }
    Ok(address)
}

fn is_safe_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first == '_' || !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn lock_state(state: &Arc<Mutex<BridgeState>>) -> MutexGuard<'_, BridgeState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn append_error(existing: Option<String>, next: String) -> String {
    match existing {
        Some(existing) => format!("{existing}; {next}"),
        None => next,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_encoding_is_ndjson_and_preserves_id() {
        let encoded = encode_call(17, "patterns", "getPatternName", &[json!(4)]).unwrap();
        assert_eq!(encoded.last(), Some(&b'\n'));
        let decoded: Value = serde_json::from_slice(&encoded[..encoded.len() - 1]).unwrap();
        assert_eq!(decoded["id"], 17);
        assert_eq!(decoded["module"], "patterns");
        assert_eq!(decoded["function"], "getPatternName");
        assert_eq!(decoded["args"], json!([4]));
    }

    #[test]
    fn malformed_json_is_reported_without_panicking() {
        let error = parse_incoming(br#"{"type":"result","id":1,"ok":true"#).unwrap_err();
        assert!(error.contains("invalid FL scripting JSON frame"));
    }

    #[test]
    fn request_id_mismatch_is_detectable() {
        let message =
            parse_incoming(br#"{"type":"result","id":18,"ok":true,"value":"x"}"#).unwrap();
        let IncomingMessage::Result(result) = message else {
            panic!("expected result");
        };
        assert_ne!(result.id, 17);
    }

    #[test]
    fn frame_buffering_is_bounded_and_preserves_partial_frames() {
        let mut partial = br#"{"type":"result""#.to_vec();
        assert!(take_frame(&mut partial).unwrap().is_none());
        partial.extend_from_slice(b"}\n");
        assert_eq!(
            take_frame(&mut partial).unwrap().unwrap(),
            br#"{"type":"result"}"#
        );
        let mut oversized = vec![b'x'; MAX_FRAME_BYTES + 1];
        assert!(take_frame(&mut oversized).is_err());
        assert!(oversized.is_empty());
    }

    #[test]
    fn status_models_disconnect_then_reconnect() {
        let mut status = BridgeStatus::default();
        status.connected(HelloSnapshot {
            bridge: BRIDGE_NAME.into(),
            protocol: PROTOCOL_VERSION,
            fl_version: Some("FL Studio".into()),
            scripting_api_version: Some(44),
        });
        status.disconnected("restart");
        status.connected(HelloSnapshot {
            bridge: BRIDGE_NAME.into(),
            protocol: PROTOCOL_VERSION,
            fl_version: None,
            scripting_api_version: Some(44),
        });
        assert!(status.connected);
        assert_eq!(status.reconnects, 1);
    }

    #[test]
    fn only_loopback_bind_addresses_are_allowed() {
        assert!(parse_loopback_address("127.0.0.1:48766").is_ok());
        assert!(parse_loopback_address("[::1]:48766").is_ok());
        assert!(parse_loopback_address("0.0.0.0:48766").is_err());
    }

    #[test]
    fn callable_names_reject_private_or_expression_syntax() {
        assert!(is_safe_identifier("getPatternName"));
        assert!(!is_safe_identifier("_private"));
        assert!(!is_safe_identifier("getattr(x)"));
        assert!(!is_safe_identifier("a.b"));
    }
}
