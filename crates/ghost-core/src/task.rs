use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// A workflow-neutral operation interpreted by an adapter identified by `namespace`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TaskOperation {
    pub operation_id: String,
    pub namespace: String,
    pub kind: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub arguments: IndexMap<String, Value>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TaskPlan {
    pub schema_version: String,
    pub task_id: Uuid,
    pub summary: String,
    pub confidence: f64,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub operations: Vec<TaskOperation>,
    #[serde(default)]
    pub expected_outcomes: Vec<ExpectedOutcome>,
    #[serde(default)]
    pub cautions: Vec<String>,
    #[serde(default)]
    pub extensions: IndexMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ExpectedOutcome {
    pub subject: String,
    pub predicate: String,
    #[serde(default)]
    pub value: Option<Value>,
}

impl TaskPlan {
    pub const SCHEMA: &'static str = "ghost.task-plan/1";
}
