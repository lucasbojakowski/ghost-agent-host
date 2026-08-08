use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StructuredIntent {
    pub source: String,
    pub role: String,
    pub style: String,
    pub goal: String,
    pub problem: Option<String>,
    pub intensity: String,
    pub preserve: Vec<String>,
    pub scope: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum UserIntent {
    Freeform { prompt: String },
    Structured { context: StructuredIntent },
}
