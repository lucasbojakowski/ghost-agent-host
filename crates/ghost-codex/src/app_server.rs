use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use ghost_context::{CompiledContext, OutputContract};
use serde_json::{json, Value};

use crate::{
    normalize_output_schema, resolve_codex_binary, AgentError, AgentEvent, AgentOutput, RpcTransport,
    StdioTransport, ToolRegistry, TurnOptions,
};

/// Handle for a Codex thread hosted by one long-lived app-server process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexThreadHandle {
    pub id: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct CodexThreadConfig {
    pub model: String,
    pub cwd: Option<PathBuf>,
    pub service_name: Option<String>,
}

impl CodexThreadConfig {
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

    pub fn service_name(mut self, service_name: impl Into<String>) -> Self {
        self.service_name = Some(service_name.into());
        self
    }
}

/// One initialized Codex App Server process that can own many Codex threads.
///
/// The host intentionally serializes turns for now, while preserving thread-scoped tools and
/// unrelated notifications. That gives Ghost a correct long-lived thread manager before we add a
/// background dispatcher for truly concurrent turns.
pub struct CodexAppServerHost {
    transport: Box<dyn RpcTransport>,
    next_id: u64,
    pending_messages: VecDeque<Value>,
    tools_by_thread: BTreeMap<String, ToolRegistry>,
}

impl CodexAppServerHost {
    pub fn spawn(binary: &str) -> Result<Self, AgentError> {
        let binary = resolve_codex_binary(binary)?;
        let transport = StdioTransport::spawn(&binary)?;
        Self::from_transport(Box::new(transport))
    }

    pub fn from_transport(transport: Box<dyn RpcTransport>) -> Result<Self, AgentError> {
        let mut host = Self {
            transport,
            next_id: 1,
            pending_messages: VecDeque::new(),
            tools_by_thread: BTreeMap::new(),
        };
        host.initialize()?;
        Ok(host)
    }

    fn initialize(&mut self) -> Result<(), AgentError> {
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
        self.send(json!({ "method": "initialized", "params": {} }))
    }

    pub fn start_thread(
        &mut self,
        config: CodexThreadConfig,
        tools: ToolRegistry,
    ) -> Result<CodexThreadHandle, AgentError> {
        let requested_model = config.model.clone();
        let mut params = json!({ "model": requested_model });
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
        self.tools_by_thread.insert(id.clone(), tools);
        Ok(CodexThreadHandle {
            id,
            model: config.model,
        })
    }

    pub fn resume_thread(
        &mut self,
        thread_id: &str,
        model: impl Into<String>,
        tools: ToolRegistry,
    ) -> Result<CodexThreadHandle, AgentError> {
        let model = model.into();
        let mut params = json!({ "threadId": thread_id, "model": model.clone() });
        if !tools.is_empty() {
            params["dynamicTools"] = serde_json::to_value(tools.definitions())?;
        }
        let result = self.request("thread/resume", params)?;
        let id = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::Protocol("thread/resume did not return a thread ID".into()))?
            .to_owned();
        self.tools_by_thread.insert(id.clone(), tools);
        Ok(CodexThreadHandle { id, model })
    }

    pub fn loaded_thread_ids(&mut self) -> Result<Vec<String>, AgentError> {
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

    pub fn list_threads(&mut self, limit: usize) -> Result<Value, AgentError> {
        self.request(
            "thread/list",
            json!({
                "cursor": Value::Null,
                "limit": limit,
                "sortKey": "updated_at"
            }),
        )
    }

    pub fn run_turn(
        &mut self,
        thread: &CodexThreadHandle,
        context: &CompiledContext,
        options: &TurnOptions,
        events: &mut dyn FnMut(AgentEvent),
    ) -> Result<AgentOutput, AgentError> {
        let mut params = json!({
            "threadId": thread.id.clone(),
            "input": [{ "type": "text", "text": context.text() }],
            "model": thread.model.clone(),
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

        let mut final_text = None;
        let mut turn_error = None;
        let mut deferred = VecDeque::new();
        loop {
            let message = self.read_message()?;
            if self.handle_tool_request(&message, &thread.id)? {
                continue;
            }

            if let Some(message_thread) = message_thread_id(&message) {
                if message_thread != thread.id {
                    deferred.push_back(message);
                    continue;
                }
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
                && message.pointer("/params/turn/id").and_then(Value::as_str)
                    == Some(turn_id.as_str())
            {
                let status = message
                    .pointer("/params/turn/status")
                    .and_then(Value::as_str)
                    .unwrap_or("failed");
                self.pending_messages.extend(deferred);
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

    fn handle_tool_request(
        &mut self,
        message: &Value,
        active_thread_id: &str,
    ) -> Result<bool, AgentError> {
        if message.get("method").and_then(Value::as_str) != Some("item/tool/call") {
            return Ok(false);
        }
        let Some(id) = message.get("id").cloned() else {
            return Err(AgentError::Protocol(
                "dynamic tool request omitted id".into(),
            ));
        };
        let tool = message
            .pointer("/params/tool")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::Protocol("dynamic tool request omitted tool".into()))?;
        let arguments = message
            .pointer("/params/arguments")
            .cloned()
            .unwrap_or(Value::Null);
        let thread_id = message
            .pointer("/params/threadId")
            .and_then(Value::as_str)
            .unwrap_or(active_thread_id);
        let registry = self.tools_by_thread.get(thread_id).ok_or_else(|| {
            AgentError::Protocol(format!(
                "dynamic tool request arrived for unregistered thread `{thread_id}`"
            ))
        })?;
        let response = match registry.call(tool, arguments) {
            Ok(value) => json!({
                "id": id,
                "result": {
                    "contentItems": [{ "type": "inputText", "text": value.to_string() }],
                    "success": true
                }
            }),
            Err(error) => json!({
                "id": id,
                "result": {
                    "contentItems": [{ "type": "inputText", "text": error.to_string() }],
                    "success": false
                }
            }),
        };
        self.send(response)?;
        Ok(true)
    }

    fn send(&mut self, value: Value) -> Result<(), AgentError> {
        self.transport.send(&value)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, AgentError> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({ "method": method, "id": id, "params": params }))?;
        loop {
            // Management requests read fresh wire messages so stale notifications cannot be
            // popped and requeued forever while waiting for this response id.
            let message = self.transport.receive()?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    return Err(AgentError::Protocol(error.to_string()));
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            self.pending_messages.push_back(message);
        }
    }

    fn read_message(&mut self) -> Result<Value, AgentError> {
        if let Some(message) = self.pending_messages.pop_front() {
            return Ok(message);
        }
        self.transport.receive()
    }
}

impl Drop for CodexAppServerHost {
    fn drop(&mut self) {
        self.transport.shutdown();
    }
}

fn message_thread_id(message: &Value) -> Option<&str> {
    message
        .pointer("/params/threadId")
        .and_then(Value::as_str)
        .or_else(|| {
            message
                .pointer("/params/thread/id")
                .and_then(Value::as_str)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct QueueTransport {
        incoming: VecDeque<Value>,
        sent: Arc<Mutex<Vec<Value>>>,
    }

    impl RpcTransport for QueueTransport {
        fn send(&mut self, message: &Value) -> Result<(), AgentError> {
            self.sent.lock().unwrap().push(message.clone());
            Ok(())
        }

        fn receive(&mut self) -> Result<Value, AgentError> {
            self.incoming
                .pop_front()
                .ok_or_else(|| AgentError::Protocol("script exhausted".into()))
        }

        fn shutdown(&mut self) {}
    }

    #[test]
    fn one_initialized_host_starts_multiple_threads() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let transport = QueueTransport {
            incoming: VecDeque::from([
                json!({"id": 1, "result": {}}),
                json!({"id": 2, "result": {"thread": {"id": "thr-a"}}}),
                json!({"method": "thread/started", "params": {"thread": {"id": "thr-a"}}}),
                json!({"id": 3, "result": {"thread": {"id": "thr-b"}}}),
                json!({"id": 4, "result": {"data": ["thr-a", "thr-b"]}}),
            ]),
            sent: Arc::clone(&sent),
        };
        let mut host = CodexAppServerHost::from_transport(Box::new(transport)).unwrap();
        let first = host
            .start_thread(CodexThreadConfig::new("test-model"), ToolRegistry::default())
            .unwrap();
        let second = host
            .start_thread(CodexThreadConfig::new("test-model"), ToolRegistry::default())
            .unwrap();
        assert_eq!(first.id, "thr-a");
        assert_eq!(second.id, "thr-b");
        assert_eq!(host.loaded_thread_ids().unwrap(), vec!["thr-a", "thr-b"]);

        let sent = sent.lock().unwrap();
        assert_eq!(
            sent.iter()
                .filter(|message| message.get("method").and_then(Value::as_str) == Some("initialize"))
                .count(),
            1
        );
        assert_eq!(
            sent.iter()
                .filter(|message| message.get("method").and_then(Value::as_str) == Some("thread/start"))
                .count(),
            2
        );
    }
}
