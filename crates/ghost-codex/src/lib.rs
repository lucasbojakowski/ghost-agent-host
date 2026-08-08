use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;

use ghost_mix::{MixPlan, PromptBundle};
use serde_json::{json, Value};
use thiserror::Error;

mod mock;
mod runtime;
mod tools;
mod transport;

pub use mock::MockMixingAgent;
pub use runtime::*;
pub use tools::*;
pub use transport::*;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("failed to start Codex: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("Codex protocol error: {0}")]
    Protocol(String),
    #[error("invalid agent output: {0}")]
    InvalidOutput(#[from] serde_json::Error),
}

pub trait MixingAgent: Send {
    fn backend_name(&self) -> &'static str;
    fn propose(&mut self, bundle: &PromptBundle) -> Result<MixPlan, AgentError>;
}

pub struct CodexAppServerAgent {
    transport: Box<dyn RpcTransport>,
    next_id: u64,
    thread_id: Option<String>,
    model: String,
    pending_messages: VecDeque<Value>,
    tools: ToolRegistry,
}

impl CodexAppServerAgent {
    pub fn spawn(binary: &str, model: impl Into<String>) -> Result<Self, AgentError> {
        Self::spawn_with_tools(binary, model, ToolRegistry::default())
    }

    pub fn spawn_with_tools(
        binary: &str,
        model: impl Into<String>,
        tools: ToolRegistry,
    ) -> Result<Self, AgentError> {
        let binary = resolve_codex_binary(binary)?;
        let transport = StdioTransport::spawn(&binary)?;
        Self::from_transport(Box::new(transport), model, tools)
    }

    pub fn from_transport(
        transport: Box<dyn RpcTransport>,
        model: impl Into<String>,
        tools: ToolRegistry,
    ) -> Result<Self, AgentError> {
        let mut agent = Self {
            transport,
            next_id: 1,
            thread_id: None,
            model: model.into(),
            pending_messages: VecDeque::new(),
            tools,
        };
        agent.initialize()?;
        Ok(agent)
    }

    fn send(&mut self, value: Value) -> Result<(), AgentError> {
        self.transport.send(&value)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, AgentError> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({ "method": method, "id": id, "params": params }))?;
        loop {
            // Requests must read fresh wire messages. Reading the pending queue here would
            // repeatedly pop and requeue the same unrelated notification forever.
            let message = self.read_wire_message()?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    return Err(AgentError::Protocol(error.to_string()));
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            self.pending_messages.push_back(message);
        }
    }

    fn read_wire_message(&mut self) -> Result<Value, AgentError> {
        self.transport.receive()
    }

    fn read_message(&mut self) -> Result<Value, AgentError> {
        if let Some(message) = self.pending_messages.pop_front() {
            return Ok(message);
        }
        self.read_wire_message()
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
                    "optOutNotificationMethods": ["item/agentMessage/delta"],
                    "experimentalApi": !self.tools.is_empty()
                }
            }),
        )?;
        self.send(json!({ "method": "initialized", "params": {} }))?;
        let mut params = json!({ "model": self.model.clone() });
        if !self.tools.is_empty() {
            params["dynamicTools"] = serde_json::to_value(self.tools.definitions())?;
        }
        let result = self.request("thread/start", params)?;
        self.thread_id = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if self.thread_id.is_none() {
            return Err(AgentError::Protocol(
                "thread/start did not return a thread ID".into(),
            ));
        }
        Ok(())
    }

    fn handle_tool_request(&mut self, message: &Value) -> Result<bool, AgentError> {
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
        let response = match self.tools.call(tool, arguments) {
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
}

pub fn resolve_codex_binary(binary: &str) -> Result<PathBuf, AgentError> {
    let requested = Path::new(binary);
    if requested.is_absolute() || requested.components().count() > 1 {
        return requested
            .is_file()
            .then(|| requested.to_path_buf())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Codex binary does not exist: {}", requested.display()),
                )
                .into()
            });
    }

    #[cfg(target_os = "windows")]
    {
        let result = Command::new("where.exe").arg(binary).output()?;
        if result.status.success() {
            let candidates: Vec<PathBuf> = String::from_utf8_lossy(&result.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
                .collect();
            if let Some(candidate) = preferred_windows_candidate(&candidates) {
                return Ok(candidate);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Codex binary '{binary}' was not found on PATH"),
        )
        .into())
    }

    #[cfg(not(target_os = "windows"))]
    Ok(requested.to_path_buf())
}

#[cfg(target_os = "windows")]
fn preferred_windows_candidate(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        })
        .or_else(|| candidates.first())
        .cloned()
}

pub(crate) fn normalize_output_schema(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("$schema");
            object.remove("format");
            if let Some(variants) = object.remove("oneOf") {
                object.insert("anyOf".into(), variants);
            }
            for child in object.values_mut() {
                normalize_output_schema(child);
            }
            if let Some(properties) = object.get("properties").and_then(Value::as_object) {
                let required = properties.keys().cloned().map(Value::String).collect();
                object.insert("required".into(), Value::Array(required));
                object.insert("additionalProperties".into(), Value::Bool(false));
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_output_schema(item);
            }
        }
        _ => {}
    }
}

impl MixingAgent for CodexAppServerAgent {
    fn backend_name(&self) -> &'static str {
        "codex-app-server"
    }

    fn propose(&mut self, bundle: &PromptBundle) -> Result<MixPlan, AgentError> {
        let output = self.run_turn(&bundle.compiled, &TurnOptions::default(), &mut |_| {})?;
        Ok(serde_json::from_str(&output.text)?)
    }
}

impl AgentRuntime for CodexAppServerAgent {
    fn backend_name(&self) -> &'static str {
        "codex-app-server"
    }

    fn thread_id(&self) -> Option<&str> {
        self.thread_id.as_deref()
    }

    fn run_turn(
        &mut self,
        context: &ghost_context::CompiledContext,
        options: &TurnOptions,
        events: &mut dyn FnMut(AgentEvent),
    ) -> Result<AgentOutput, AgentError> {
        let thread_id = self
            .thread_id
            .clone()
            .ok_or_else(|| AgentError::Protocol("Codex thread is not initialized".into()))?;
        let mut params = json!({
            "threadId": thread_id,
            "input": [{ "type": "text", "text": context.text() }],
            "model": self.model.clone(),
            "effort": options.effort,
            "summary": options.summary,
            "approvalPolicy": options.approval_policy,
            "sandboxPolicy": options.sandbox_policy
        });
        if let ghost_context::OutputContract::Json { schema, .. } = &context.output {
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
        loop {
            let message = self.read_message()?;
            if self.handle_tool_request(&message)? {
                continue;
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
        let structured = match context.output {
            ghost_context::OutputContract::Text => None,
            ghost_context::OutputContract::Json { .. } => Some(serde_json::from_str(&text)?),
        };
        Ok(AgentOutput { text, structured })
    }
}

impl Drop for CodexAppServerAgent {
    fn drop(&mut self) {
        self.transport.shutdown();
    }
}

#[cfg(test)]
mod tests;
