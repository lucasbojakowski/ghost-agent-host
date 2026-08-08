use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProcessorDescriptor {
    pub stable_id: String,
    pub name: String,
    pub vendor: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<CapabilityDescriptor>,
    #[serde(default)]
    pub parameters: Vec<ParameterDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CapabilityDescriptor {
    pub namespace: String,
    pub kind: String,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ParameterDescriptor {
    pub stable_id: String,
    pub name: String,
    pub module: Option<String>,
    pub unit: Option<String>,
    pub minimum: f64,
    pub maximum: f64,
    pub default: f64,
    pub stepped: bool,
    pub read_only: bool,
    #[serde(default)]
    pub labels: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ParameterChange {
    pub processor_id: String,
    pub parameter_id: String,
    pub normalized_value: f64,
    pub smoothing_ms: f64,
}
