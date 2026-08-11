//! Concurrent Codex App Server runtime.
//!
//! One reader thread owns App Server stdout and routes RPC responses, turn notifications, and
//! bidirectional dynamic-tool requests. Callers may drive different Codex threads concurrently over
//! the same long-lived App Server process. Routing deliberately fails closed when the server emits
//! an event without enough thread/turn identity while multiple turns are active.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::{Child, ChildStdin};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, Weak};
use std::thread;

use ghost_context::{CompiledContext, OutputContract};
use serde_json::{json, Value};

use crate::transport::{read_stdio_message, write_stdio_message, SplitStdioTransport};
use crate::{
    normalize_output_schema, resolve_codex_binary, AgentError, AgentEvent, AgentOutput, ToolRegistry,
    TurnOptions,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParallelCodexThread {
    pub id: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct ParallelThreadConfig {
    pub model: String,
    pub cwd: Option<PathBuf>,
    pub service_name: Option<String>,
}

impl ParallelThreadConfig {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            cwd: None,
            service_name: Some("ghost_agent_host".into()),
        }
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = Some(name.into());
        self
    }
}

/// Cloneable handle to one persistent Codex App Server process.
///
/// Requests are correlated by JSON-RPC id, stdout is consumed by one dispatcher thread, and each
/// in-flight Codex turn receives a private event channel. Different Codex threads may therefore be
/// driven concurrently without allowing multiple turns to race on the same thread.
#[derive(Clone)]
pub struct CodexParallelRuntime {
    inner: Arc<ParallelInner>,
}

struct TurnRoute {
    thread_id: String,
    sender: mpsc::Sender<Value>,
}

struct ParallelInner {
    stdin: Arc<Mutex<ChildStdin>>,
    child: Arc<Mutex<Child>>,
    next_id: AtomicU64,
    pending_requests: Mutex<BTreeMap<u64, mpsc::Sender<Result<Value, String>>>>,
    tools_by_thread: Mutex<BTreeMap<String, Arc<ToolRegistry>>>,
    turn_routes: Mutex<BTreeMap<String, TurnRoute>>,
    active_threads: Mutex<BTreeSet<String>>,
    orphan_events: Mutex<BTreeMap<String, Vec<Value>>>,
    closed: AtomicBool,
}

impl CodexParallelRuntime {
    pub fn spawn(binary: &str) -> Result<Self, AgentError> {
        let binary = resolve_codex_binary(binary)?;
        let transport = SplitStdioTransport::spawn(&binary)?;
        let inner = Arc::new(ParallelInner {
            stdin: transport.stdin,
            child: transport.child,
            next_id: AtomicU64::new(1),
            pending_requests: Mutex::new(BTreeMap::new()),
            tools_by_thread: Mutex::new(BTreeMap::new()),
            turn_routes: Mutex::new(BTreeMap::new()),
            active_threads: Mutex::new(BTreeSet::new()),
            orphan_events: Mutex::new(BTreeMap::new()),
            closed: AtomicBool::new(false),
        });
        spawn_reader_thread(Arc::downgrade(&inner), transport.stdout)?;
        let runtime = Self { inner };
        runtime.initialize()?;
        Ok(runtime)
    }

    fn initialize(&self) -> Result<(), AgentError> {
        self.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "ghost_agent_host",
                    "title": "Ghost Agent Host",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true,
                    "optOutNotificationMethods": ["item/agentMessage/delta"]
                }
            }),
        )?;
        self.send(json!({"method": "initialized", "params": {}}))
    }

    pub fn start_thread(
        &self,
        config: ParallelThreadConfig,
        tools: ToolRegistry,
    ) -> Result<ParallelCodexThread, AgentError> {
        let requested_model = config.model.clone();
        let mut params = json!({"model": config.model});
        if let Some(cwd) = config.cwd {
            params["cwd"] = Value::String(cwd.to_string_lossy().into_owned());
        }
        if let Some(service_name) = config.service_name {
            params["serviceName"] = Value::String(service_name);
        }
        if !tools.is_empty() {
            params["dynamicTools"] = serde_json::to_value(tools.definitions())?;
        }
        let result = self.request("thread/start", params)?;
        let id = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::Protocol("thread/start did not return a thread ID".into()))?
            .to_owned();
        let model = result
            .pointer("/thread/model")
            .and_then(Value::as_str)
            .unwrap_or(&requested_model)
            .to_owned();
        self.set_thread_tools(&id, tools)?;
        Ok(ParallelCodexThread { id, model })
    }

    pub fn resume_thread(
        &self,
        thread_id: &str,
        tools: ToolRegistry,
    ) -> Result<ParallelCodexThread, AgentError> {
        let mut params = json!({"threadId": thread_id});
        if !tools.is_empty() {
            params["dynamicTools"] = serde_json::to_value(tools.definitions())?;
        }
        let result = self.request("thread/resume", params)?;
        let id = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::Protocol("thread/resume did not return a thread ID".into()))?
            .to_owned();
        let model = result
            .pointer("/thread/model")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        self.set_thread_tools(&id, tools)?;
        Ok(ParallelCodexThread { id, model })
    }

    pub fn loaded_thread_ids(&self) -> Result<Vec<String>, AgentError> {
        let result = self.request("thread/loaded/list", json!({}))?;
        Ok(result
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect())
    }

    pub fn list_threads(&self, limit: usize) -> Result<Value, AgentError> {
        self.request(
            "thread/list",
            json!({"cursor": Value::Null, "limit": limit, "sortKey": "updated_at"}),
        )
    }

    /// Run a turn on one Codex thread. Separate caller threads may invoke this concurrently for
    /// different `ParallelCodexThread`s. A second turn on the same thread is rejected locally.
    pub fn run_turn(
        &self,
        thread: &ParallelCodexThread,
        context: &CompiledContext,
        options: &TurnOptions,
        events: &mut dyn FnMut(AgentEvent),
    ) -> Result<AgentOutput, AgentError> {
        {
            let mut active = self
                .inner
                .active_threads
                .lock()
                .map_err(|_| AgentError::Protocol("parallel active-thread lock poisoned".into()))?;
            if !active.insert(thread.id.clone()) {
                return Err(AgentError::Protocol(format!(
                    "thread `{}` already has an in-flight turn",
                    thread.id
                )));
            }
        }
        let _active_guard = ActiveThreadGuard {
            inner: Arc::clone(&self.inner),
            thread_id: thread.id.clone(),
        };

        let mut params = json!({
            "threadId": thread.id,
            "input": [{"type": "text", "text": context.text()}],
            "model": thread.model,
            "effort": options.effort,
            "summary": options.summary,
            "approvalPolicy": options.approval_policy,
            "sandboxPolicy": options.sandbox_policy
        });
        if let OutputContract::Json { schema, .. } = &context.output {
            let mut schema = schema.clone();
            normalize_output_schema(&mut schema);
            params["outputSchema"] = schema;
        }

        let result = self.request("turn/start", params)?;
        let turn_id = result
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::Protocol("turn/start did not return a turn ID".into()))?
            .to_owned();
        let (sender, receiver) = mpsc::channel();
        self.inner
            .turn_routes
            .lock()
            .map_err(|_| AgentError::Protocol("parallel turn-route lock poisoned".into()))?
            .insert(
                turn_id.clone(),
                TurnRoute {
                    thread_id: thread.id.clone(),
                    sender: sender.clone(),
                },
            );
        if let Some(buffered) = self
            .inner
            .orphan_events
            .lock()
            .map_err(|_| AgentError::Protocol("parallel orphan-event lock poisoned".into()))?
            .remove(&turn_id)
        {
            for message in buffered {
                let _ = sender.send(message);
            }
        }
        let _route_guard = TurnRouteGuard {
            inner: Arc::clone(&self.inner),
            turn_id: turn_id.clone(),
        };

        collect_turn_output(&turn_id, receiver, context, events)
    }

    fn set_thread_tools(&self, thread_id: &str, tools: ToolRegistry) -> Result<(), AgentError> {
        self.inner
            .tools_by_thread
            .lock()
            .map_err(|_| AgentError::Protocol("parallel tool registry lock poisoned".into()))?
            .insert(thread_id.to_owned(), Arc::new(tools));
        Ok(())
    }

    fn request(&self, method: &str, params: Value) -> Result<Value, AgentError> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(AgentError::Protocol("Codex App Server runtime is closed".into()));
        }
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.inner
            .pending_requests
            .lock()
            .map_err(|_| AgentError::Protocol("parallel request lock poisoned".into()))?
            .insert(id, sender);
        if let Err(error) = self.send(json!({"method": method, "id": id, "params": params})) {
            if let Ok(mut pending) = self.inner.pending_requests.lock() {
                pending.remove(&id);
            }
            return Err(error);
        }
        let message = receiver
            .recv()
            .map_err(|_| AgentError::Protocol(format!("App Server request `{method}` was abandoned")))?
            .map_err(AgentError::Protocol)?;
        if let Some(error) = message.get("error") {
            return Err(AgentError::Protocol(error.to_string()));
        }
        Ok(message.get("result").cloned().unwrap_or(Value::Null))
    }

    fn send(&self, value: Value) -> Result<(), AgentError> {
        write_stdio_message(&self.inner.stdin, &value)
    }
}

fn collect_turn_output(
    turn_id: &str,
    receiver: mpsc::Receiver<Value>,
    context: &CompiledContext,
    events: &mut dyn FnMut(AgentEvent),
) -> Result<AgentOutput, AgentError> {
    let mut final_text = None;
    let mut turn_error = None;
    loop {
        let message = receiver
            .recv()
            .map_err(|_| AgentError::Protocol("parallel App Server event channel closed".into()))?;
        if message.get("method").and_then(Value::as_str) == Some("ghost/routing-ambiguous") {
            return Err(AgentError::Protocol(
                message
                    .pointer("/params/message")
                    .and_then(Value::as_str)
                    .unwrap_or("App Server event could not be routed safely")
                    .to_owned(),
            ));
        }
        events(AgentEvent::from_wire(&message));
        if message.get("method").and_then(Value::as_str) == Some("error") {
            turn_error = message.pointer("/params/error").cloned();
        }
        if message.get("method").and_then(Value::as_str) == Some("item/completed") {
            let item = message
                .pointer("/params/item")
                .cloned()
                .unwrap_or(Value::Null);
            if item.get("type").and_then(Value::as_str) == Some("agentMessage") {
                final_text = item.get("text").and_then(Value::as_str).map(str::to_owned);
            }
        }
        if message.get("method").and_then(Value::as_str) == Some("turn/completed")
            && message.pointer("/params/turn/id").and_then(Value::as_str) == Some(turn_id)
        {
            let status = message
                .pointer("/params/turn/status")
                .and_then(Value::as_str)
                .unwrap_or("failed");
            if status != "completed" {
                let details = message
                    .pointer("/params/turn/error")
                    .or(turn_error.as_ref())
                    .map(Value::to_string)
                    .unwrap_or_else(|| "no error details supplied".into());
                return Err(AgentError::Protocol(format!(
                    "Codex turn ended with status {status}: {details}"
                )));
            }
            break;
        }
    }

    let text = final_text.ok_or_else(|| {
        AgentError::Protocol("Codex completed without a final agentMessage".into())
    })?;
    let structured = match &context.output {
        OutputContract::Text => None,
        OutputContract::Json { .. } => Some(serde_json::from_str(&text)?),
    };
    Ok(AgentOutput { text, structured })
}

struct ActiveThreadGuard {
    inner: Arc<ParallelInner>,
    thread_id: String,
}

impl Drop for ActiveThreadGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.inner.active_threads.lock() {
            active.remove(&self.thread_id);
        }
    }
}

struct TurnRouteGuard {
    inner: Arc<ParallelInner>,
    turn_id: String,
}

impl Drop for TurnRouteGuard {
    fn drop(&mut self) {
        if let Ok(mut routes) = self.inner.turn_routes.lock() {
            routes.remove(&self.turn_id);
        }
    }
}

impl Drop for ParallelInner {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Ok(mut pending) = self.pending_requests.lock() {
            for (_, sender) in std::mem::take(&mut *pending) {
                let _ = sender.send(Err("Codex App Server runtime shut down".into()));
            }
        }
    }
}

fn spawn_reader_thread(
    inner: Weak<ParallelInner>,
    mut stdout: std::io::BufReader<std::process::ChildStdout>,
) -> Result<(), AgentError> {
    thread::Builder::new()
        .name("ghost-codex-app-server-reader".into())
        .spawn(move || loop {
            let message = match read_stdio_message(&mut stdout) {
                Ok(message) => message,
                Err(error) => {
                    if let Some(inner) = inner.upgrade() {
                        fail_pending(&inner, error.to_string());
                    }
                    break;
                }
            };
            let Some(inner) = inner.upgrade() else {
                break;
            };
            dispatch_message(&inner, message);
        })?;
    Ok(())
}

fn dispatch_message(inner: &Arc<ParallelInner>, message: Value) {
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id").and_then(Value::as_u64);

    if method.is_none() {
        if let Some(id) = id {
            if let Ok(mut pending) = inner.pending_requests.lock() {
                if let Some(sender) = pending.remove(&id) {
                    let _ = sender.send(Ok(message));
                    return;
                }
            }
        }
    }

    if method == Some("item/tool/call") && id.is_some() {
        dispatch_tool_call(inner, message);
        return;
    }

    route_notification(inner, message);
}

fn dispatch_tool_call(inner: &Arc<ParallelInner>, message: Value) {
    let id = message.get("id").and_then(Value::as_u64).unwrap_or_default();
    let tool = message
        .pointer("/params/tool")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let arguments = message
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or(Value::Null);
    let thread_id = resolve_thread_id(inner, &message);
    let registry = thread_id.as_ref().and_then(|thread_id| {
        inner
            .tools_by_thread
            .lock()
            .ok()
            .and_then(|tools| tools.get(thread_id).cloned())
    });
    let stdin = Arc::clone(&inner.stdin);

    let _ = thread::Builder::new()
        .name(format!("ghost-codex-tool-{id}"))
        .spawn(move || {
            let response = match (thread_id, registry) {
                (Some(_), Some(registry)) if !tool.is_empty() => match registry.call(&tool, arguments) {
                    Ok(value) => json!({
                        "id": id,
                        "result": {
                            "contentItems": [{"type": "inputText", "text": value.to_string()}],
                            "success": true
                        }
                    }),
                    Err(error) => json!({
                        "id": id,
                        "result": {
                            "contentItems": [{"type": "inputText", "text": error.to_string()}],
                            "success": false
                        }
                    }),
                },
                _ => json!({
                    "id": id,
                    "result": {
                        "contentItems": [{
                            "type": "inputText",
                            "text": "Ghost refused a dynamic tool call because its thread/turn identity was ambiguous or unregistered."
                        }],
                        "success": false
                    }
                }),
            };
            let _ = write_stdio_message(&stdin, &response);
        });
}

fn route_notification(inner: &Arc<ParallelInner>, message: Value) {
    if let Some(turn_id) = message_turn_id(&message) {
        let sender = inner
            .turn_routes
            .lock()
            .ok()
            .and_then(|routes| routes.get(turn_id).map(|route| route.sender.clone()));
        if let Some(sender) = sender {
            let _ = sender.send(message);
        } else if let Ok(mut orphan) = inner.orphan_events.lock() {
            orphan.entry(turn_id.to_owned()).or_default().push(message);
        }
        return;
    }

    if let Some(thread_id) = message_thread_id(&message) {
        let senders = senders_for_thread(inner, thread_id);
        if senders.len() == 1 {
            let _ = senders[0].send(message);
            return;
        }
        if senders.is_empty() {
            return;
        }
    }

    let senders = all_turn_senders(inner);
    match senders.as_slice() {
        [only] => {
            let _ = only.send(message);
        }
        [] => {}
        many => {
            let ambiguity = json!({
                "method": "ghost/routing-ambiguous",
                "params": {
                    "message": "Codex App Server emitted an event without a routable threadId/turnId while multiple turns were active; Ghost failed closed instead of attributing it to the wrong agent thread.",
                    "wireMethod": message.get("method").cloned().unwrap_or(Value::Null)
                }
            });
            for sender in many {
                let _ = sender.send(ambiguity.clone());
            }
        }
    }
}

fn senders_for_thread(inner: &ParallelInner, thread_id: &str) -> Vec<mpsc::Sender<Value>> {
    inner
        .turn_routes
        .lock()
        .map(|routes| {
            routes
                .values()
                .filter(|route| route.thread_id == thread_id)
                .map(|route| route.sender.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn all_turn_senders(inner: &ParallelInner) -> Vec<mpsc::Sender<Value>> {
    inner
        .turn_routes
        .lock()
        .map(|routes| routes.values().map(|route| route.sender.clone()).collect())
        .unwrap_or_default()
}

fn resolve_thread_id(inner: &ParallelInner, message: &Value) -> Option<String> {
    if let Some(thread_id) = message_thread_id(message) {
        return Some(thread_id.to_owned());
    }
    if let Some(turn_id) = message_turn_id(message) {
        if let Ok(routes) = inner.turn_routes.lock() {
            if let Some(route) = routes.get(turn_id) {
                return Some(route.thread_id.clone());
            }
        }
    }
    let active = inner.active_threads.lock().ok()?;
    if active.len() == 1 {
        active.iter().next().cloned()
    } else {
        None
    }
}

fn message_thread_id(message: &Value) -> Option<&str> {
    message
        .pointer("/params/threadId")
        .and_then(Value::as_str)
        .or_else(|| message.pointer("/params/thread/id").and_then(Value::as_str))
}

fn message_turn_id(message: &Value) -> Option<&str> {
    message
        .pointer("/params/turnId")
        .and_then(Value::as_str)
        .or_else(|| message.pointer("/params/turn/id").and_then(Value::as_str))
}

fn fail_pending(inner: &ParallelInner, message: String) {
    inner.closed.store(true, Ordering::Release);
    if let Ok(mut pending) = inner.pending_requests.lock() {
        for (_, sender) in std::mem::take(&mut *pending) {
            let _ = sender.send(Err(message.clone()));
        }
    }
    let failure = json!({
        "method": "ghost/routing-ambiguous",
        "params": {"message": format!("Codex App Server reader stopped: {message}")}
    });
    if let Ok(routes) = inner.turn_routes.lock() {
        for route in routes.values() {
            let _ = route.sender.send(failure.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_thread_and_turn_identity_shapes() {
        assert_eq!(
            message_thread_id(&json!({"params": {"threadId": "thr-a"}})),
            Some("thr-a")
        );
        assert_eq!(
            message_thread_id(&json!({"params": {"thread": {"id": "thr-b"}}})),
            Some("thr-b")
        );
        assert_eq!(
            message_turn_id(&json!({"params": {"turnId": "turn-a"}})),
            Some("turn-a")
        );
        assert_eq!(
            message_turn_id(&json!({"params": {"turn": {"id": "turn-b"}}})),
            Some("turn-b")
        );
    }

    #[test]
    fn structured_output_collection_is_thread_local() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(json!({
                "method": "item/completed",
                "params": {"item": {"type": "agentMessage", "text": "{\"ok\":true}"}}
            }))
            .unwrap();
        sender
            .send(json!({
                "method": "turn/completed",
                "params": {"turn": {"id": "turn-a", "status": "completed"}}
            }))
            .unwrap();
        let context = CompiledContext {
            schema_version: CompiledContext::SCHEMA.into(),
            messages: vec![],
            output: OutputContract::Json {
                schema_name: "test".into(),
                schema: json!({"type": "object"}),
            },
            metadata: Value::Null,
        };
        let output = collect_turn_output("turn-a", receiver, &context, &mut |_| {}).unwrap();
        assert_eq!(output.structured, Some(json!({"ok": true})));
    }
}
