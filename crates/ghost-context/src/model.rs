use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ContextMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum OutputContract {
    Text,
    Json { schema_name: String, schema: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CompiledContext {
    pub schema_version: String,
    pub messages: Vec<ContextMessage>,
    pub output: OutputContract,
    #[serde(default)]
    pub metadata: Value,
}

impl CompiledContext {
    pub const SCHEMA: &'static str = "ghost.compiled-context/1";

    pub fn text(&self) -> String {
        self.messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextValue {
    Text(String),
    Json(Value),
}
