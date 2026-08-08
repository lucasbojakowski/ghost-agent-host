use ghost_context::CompiledContext;
use serde_json::Value;

use crate::AgentError;

#[derive(Debug, Clone)]
pub struct TurnOptions {
    pub effort: String,
    pub summary: String,
    pub approval_policy: String,
    pub sandbox_policy: Value,
}

impl Default for TurnOptions {
    fn default() -> Self {
        Self {
            effort: "high".into(),
            summary: "concise".into(),
            approval_policy: "never".into(),
            sandbox_policy: serde_json::json!({
                "type": "readOnly",
                "access": { "type": "fullAccess" }
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    TurnStarted {
        turn_id: Option<String>,
    },
    ItemStarted {
        item: Value,
    },
    ItemCompleted {
        item: Value,
    },
    TurnCompleted {
        status: String,
    },
    Other {
        method: Option<String>,
        payload: Value,
    },
}

impl AgentEvent {
    pub(crate) fn from_wire(message: &Value) -> Self {
        let method = message.get("method").and_then(Value::as_str);
        match method {
            Some("turn/started") => Self::TurnStarted {
                turn_id: message
                    .pointer("/params/turn/id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            },
            Some("item/started") => Self::ItemStarted {
                item: message
                    .pointer("/params/item")
                    .cloned()
                    .unwrap_or(Value::Null),
            },
            Some("item/completed") => Self::ItemCompleted {
                item: message
                    .pointer("/params/item")
                    .cloned()
                    .unwrap_or(Value::Null),
            },
            Some("turn/completed") => Self::TurnCompleted {
                status: message
                    .pointer("/params/turn/status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_owned(),
            },
            _ => Self::Other {
                method: method.map(str::to_owned),
                payload: message.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentOutput {
    pub text: String,
    pub structured: Option<Value>,
}

pub trait AgentRuntime: Send {
    fn backend_name(&self) -> &'static str;
    fn thread_id(&self) -> Option<&str>;
    fn run_turn(
        &mut self,
        context: &CompiledContext,
        options: &TurnOptions,
        events: &mut dyn FnMut(AgentEvent),
    ) -> Result<AgentOutput, AgentError>;
}
