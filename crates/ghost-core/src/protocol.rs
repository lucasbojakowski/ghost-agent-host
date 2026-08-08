use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const PROTOCOL_VERSION: &str = "ghost.protocol/1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RequestEnvelope {
    pub protocol: String,
    pub request_id: Uuid,
    pub operation: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ResponseEnvelope {
    pub protocol: String,
    pub request_id: Uuid,
    pub status: ResponseStatus,
    #[serde(default)]
    pub payload: Value,
    pub error: Option<ProtocolError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Accepted,
    Progress,
    Complete,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProgressEvent {
    pub request_id: Uuid,
    pub phase: String,
    pub fraction: Option<f32>,
    pub message: String,
}
