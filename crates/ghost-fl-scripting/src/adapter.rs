use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::catalog::{FlScriptingCatalog, FlScriptingManifest};
use crate::protocol::{
    decode_result, encode_call, parse_incoming, take_frame, HelloMessage, IncomingMessage,
    IO_CHUNK_BYTES, MAX_RECEIVE_BUFFER_BYTES,
};

pub const PROTOCOL_VERSION: u32 = 1;
pub const BRIDGE_NAME: &str = "ghost-fl-scripting";
pub const DEFAULT_SCRIPTING_BIND: &str = "127.0.0.1:48766";
pub const BRIDGE_MODULES: &[&str] = &[
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

const SOCKET_POLL: Duration = Duration::from_millis(20);

#[derive(Debug, Clone)]
pub struct FlScriptingConfig {
    pub bind: String,
    pub call_timeout: Duration,
}

impl Default for FlScriptingConfig {
    fn default() -> Self {
        Self {
            bind: DEFAULT_SCRIPTING_BIND.into(),
            call_timeout: Duration::from_millis(1500),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FlScriptingHello {
    pub bridge: String,
    pub protocol: u32,
    pub fl_version: Option<String>,
    pub scripting_api_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FlScriptingStatus {
    pub listening: bool,
    pub bind: String,
    pub connected: bool,
    pub hello: Option<FlScriptingHello>,
    pub reconnects: u64,
    pub malformed_messages: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Error)]
pub enum FlScriptingError {
    #[error("FL scripting configuration failed: {0}")]
    Configuration(String),
    #[error("FL scripting transport failed: {0}")]
    Transport(String),
    #[error("FL scripting protocol failed: {0}")]
    Protocol(String),
    #[error("FL scripting call is unavailable: {0}")]
    UnsupportedCall(String),
    #[error("FL scripting device is not connected")]
    NotConnected,
    #[error("FL scripting remote call failed: {0}")]
    RemoteCall(String),
}

#[derive(Clone)]
pub struct FlScriptingAdapter {
    state: Arc<Mutex<AdapterState>>,
    next_id: Arc<AtomicU64>,
    call_timeout: Duration,
    catalog: Arc<FlScriptingCatalog>,
}

struct AdapterState {
    connection: Option<BridgeConnection>,
    status: AdapterStatus,
}

#[derive(Debug, Default)]
struct AdapterStatus {
    listening: bool,
    bind: String,
    connected: bool,
    hello: Option<FlScriptingHello>,
    reconnects: u64,
    malformed_messages: u64,
    last_error: Option<String>,
}

impl AdapterStatus {
    fn snapshot(&self) -> FlScriptingStatus {
        FlScriptingStatus {
            listening: self.listening,
            bind: self.bind.clone(),
            connected: self.connected,
            hello: self.hello.clone(),
            reconnects: self.reconnects,
            malformed_messages: self.malformed_messages,
            last_error: self.last_error.clone(),
        }
    }

    fn connected(&mut self, hello: FlScriptingHello) {
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

struct CallFailure {
    message: String,
    disconnect: bool,
    remote: bool,
}

impl FlScriptingAdapter {
    pub fn start(config: FlScriptingConfig) -> Result<Self, FlScriptingError> {
        let address = parse_loopback_address(&config.bind)?;
        let listener = TcpListener::bind(address).map_err(|error| {
            FlScriptingError::Transport(format!(
                "failed to bind FL scripting bridge at {}: {error}",
                config.bind
            ))
        })?;
        let actual_bind = listener
            .local_addr()
            .map_err(|error| FlScriptingError::Transport(error.to_string()))?
            .to_string();
        let catalog = Arc::new(
            FlScriptingCatalog::bundled()
                .map_err(|error| FlScriptingError::Configuration(error.to_string()))?,
        );
        let state = Arc::new(Mutex::new(AdapterState {
            connection: None,
            status: AdapterStatus {
                listening: true,
                bind: actual_bind,
                ..AdapterStatus::default()
            },
        }));
        let accept_state = Arc::clone(&state);
        let timeout = config.call_timeout;
        thread::Builder::new()
            .name("ghost-fl-scripting-listener".into())
            .spawn(move || accept_loop(listener, accept_state, timeout))
            .map_err(|error| {
                FlScriptingError::Transport(format!(
                    "failed to start FL scripting listener thread: {error}"
                ))
            })?;

        Ok(Self {
            state,
            next_id: Arc::new(AtomicU64::new(1)),
            call_timeout: timeout,
            catalog,
        })
    }

    pub fn status(&self) -> FlScriptingStatus {
        lock_state(&self.state).status.snapshot()
    }

    pub fn catalog(&self) -> Arc<FlScriptingCatalog> {
        Arc::clone(&self.catalog)
    }

    pub fn manifest(&self) -> FlScriptingManifest {
        let status = self.status();
        let scripting_api_version = status
            .hello
            .as_ref()
            .and_then(|hello| hello.scripting_api_version);
        FlScriptingManifest {
            bridge: BRIDGE_NAME,
            protocol: PROTOCOL_VERSION,
            fl_version: status
                .hello
                .as_ref()
                .and_then(|hello| hello.fl_version.clone()),
            scripting_api_version,
            functions: self.catalog.manifest_functions(scripting_api_version),
        }
    }

    pub fn call(
        &self,
        module: &str,
        function: &str,
        args: Vec<Value>,
    ) -> Result<Value, FlScriptingError> {
        if !BRIDGE_MODULES.contains(&module) {
            return Err(FlScriptingError::UnsupportedCall(format!(
                "module `{module}` is not one of the explicitly imported FL scripting modules"
            )));
        }
        if !is_safe_identifier(function) {
            return Err(FlScriptingError::UnsupportedCall(format!(
                "function `{function}` is not a public callable identifier"
            )));
        }
        self.catalog
            .ensure_bridge_callable(module, function)
            .map_err(FlScriptingError::UnsupportedCall)?;

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let frame = encode_call(id, module, function, &args).map_err(FlScriptingError::Protocol)?;
        let mut state = lock_state(&self.state);
        let AdapterState {
            connection: connection_slot,
            status,
        } = &mut *state;
        let Some(connection) = connection_slot.as_mut() else {
            return Err(FlScriptingError::NotConnected);
        };

        match perform_call(
            connection,
            status,
            id,
            &frame,
            module,
            function,
            self.call_timeout,
        ) {
            Ok(value) => Ok(value),
            Err(failure) => {
                if failure.disconnect {
                    *connection_slot = None;
                    status.disconnected(failure.message.clone());
                } else {
                    status.last_error = Some(failure.message.clone());
                }
                if failure.remote {
                    Err(FlScriptingError::RemoteCall(failure.message))
                } else {
                    Err(FlScriptingError::Transport(failure.message))
                }
            }
        }
    }
}

fn perform_call(
    connection: &mut BridgeConnection,
    status: &mut AdapterStatus,
    id: u64,
    frame: &[u8],
    module: &str,
    function: &str,
    timeout: Duration,
) -> Result<Value, CallFailure> {
    connection
        .stream
        .write_all(frame)
        .map_err(|error| CallFailure {
            message: format!("FL scripting write failed: {error}"),
            disconnect: true,
            remote: false,
        })?;
    let deadline = Instant::now() + timeout;
    loop {
        match connection.read_frame() {
            Ok(Some(frame)) => match parse_incoming(&frame) {
                Ok(IncomingMessage::Result(result)) if result.id == id => {
                    return decode_result(result).map_err(|error| CallFailure {
                        message: error,
                        disconnect: false,
                        remote: true,
                    });
                }
                Ok(IncomingMessage::Result(result)) => {
                    return Err(CallFailure {
                        message: format!(
                            "FL scripting result correlation mismatch: expected id {id}, got {}",
                            result.id
                        ),
                        disconnect: true,
                        remote: false,
                    });
                }
                Ok(IncomingMessage::Hello(hello)) => {
                    if hello.protocol != PROTOCOL_VERSION || hello.bridge != BRIDGE_NAME {
                        return Err(CallFailure {
                            message: "FL scripting hello changed protocol during a call".into(),
                            disconnect: true,
                            remote: false,
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
                    remote: false,
                });
            }
        }
        if Instant::now() >= deadline {
            return Err(CallFailure {
                message: format!("FL scripting call {module}.{function} timed out"),
                disconnect: true,
                remote: false,
            });
        }
    }
}

fn accept_loop(listener: TcpListener, state: Arc<Mutex<AdapterState>>, timeout: Duration) {
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
            Err(error) => lock_state(&state).status.disconnected(error),
        }
    }
}

fn prepare_connection(
    stream: TcpStream,
    timeout: Duration,
) -> Result<(BridgeConnection, FlScriptingHello), String> {
    stream
        .set_read_timeout(Some(SOCKET_POLL))
        .map_err(|error| format!("failed to configure FL scripting socket read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("failed to configure FL scripting socket write timeout: {error}"))?;
    stream
        .set_nodelay(true)
        .map_err(|error| format!("failed to configure FL scripting TCP_NODELAY: {error}"))?;
    let mut connection = BridgeConnection {
        stream,
        receive_buffer: Vec::with_capacity(IO_CHUNK_BYTES * 2),
    };
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(frame) = connection
            .read_frame()
            .map_err(|error| format!("FL scripting hello read failed: {error}"))?
        {
            match parse_incoming(&frame)? {
                IncomingMessage::Hello(hello)
                    if hello.protocol == PROTOCOL_VERSION && hello.bridge == BRIDGE_NAME =>
                {
                    return Ok((connection, hello_snapshot(hello)));
                }
                IncomingMessage::Hello(hello) => {
                    return Err(format!(
                        "FL scripting protocol mismatch: bridge='{}' protocol={}",
                        hello.bridge, hello.protocol
                    ));
                }
                IncomingMessage::Result(_) => {
                    return Err("FL scripting client sent a result before the hello handshake".into());
                }
            }
        }
        if Instant::now() >= deadline {
            return Err("FL scripting hello handshake timed out".into());
        }
    }
}

fn hello_snapshot(hello: HelloMessage) -> FlScriptingHello {
    FlScriptingHello {
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

fn parse_loopback_address(bind: &str) -> Result<SocketAddr, FlScriptingError> {
    let address: SocketAddr = bind.parse().map_err(|error| {
        FlScriptingError::Configuration(format!(
            "invalid FL scripting bind address `{bind}`: {error}"
        ))
    })?;
    if !address.ip().is_loopback() {
        return Err(FlScriptingError::Configuration(format!(
            "FL scripting adapter must bind to loopback, got {address}"
        )));
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

fn lock_state(state: &Arc<Mutex<AdapterState>>) -> MutexGuard<'_, AdapterState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn status_models_disconnect_then_reconnect() {
        let mut status = AdapterStatus::default();
        status.connected(FlScriptingHello {
            bridge: BRIDGE_NAME.into(),
            protocol: PROTOCOL_VERSION,
            fl_version: Some("FL Studio".into()),
            scripting_api_version: Some(44),
        });
        status.disconnected("restart");
        status.connected(FlScriptingHello {
            bridge: BRIDGE_NAME.into(),
            protocol: PROTOCOL_VERSION,
            fl_version: None,
            scripting_api_version: Some(44),
        });
        assert!(status.connected);
        assert_eq!(status.reconnects, 1);
    }

    #[test]
    fn manifest_filters_connected_api_version_without_hiding_metadata() {
        let catalog = FlScriptingCatalog::bundled().unwrap();
        let functions = catalog.manifest_functions(Some(43));
        let clear_pattern = functions
            .iter()
            .find(|entry| entry.module == "patterns" && entry.function == "clearPattern")
            .unwrap();
        assert_eq!(clear_pattern.minimum_api_version, Some(44));
        assert_eq!(clear_pattern.available_in_connected_api, Some(false));
    }
}
