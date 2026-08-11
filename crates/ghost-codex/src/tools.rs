use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError(pub String);

impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for ToolError {}

type ToolHandler = Arc<dyn Fn(Value) -> Result<Value, ToolError> + Send + Sync>;

struct RegisteredTool {
    definition: ToolDefinition,
    handler: ToolHandler,
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, RegisteredTool>,
}

impl ToolRegistry {
    pub fn register(
        &mut self,
        definition: ToolDefinition,
        handler: impl Fn(Value) -> Result<Value, ToolError> + Send + Sync + 'static,
    ) -> Result<(), ToolError> {
        validate_tool_name(&definition.name)?;
        if self.tools.contains_key(&definition.name) {
            return Err(ToolError(format!("duplicate tool `{}`", definition.name)));
        }
        self.tools.insert(
            definition.name.clone(),
            RegisteredTool {
                definition,
                handler: Arc::new(handler),
            },
        );
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn definitions(&self) -> Vec<&ToolDefinition> {
        self.tools.values().map(|tool| &tool.definition).collect()
    }

    pub fn call(&self, name: &str, arguments: Value) -> Result<Value, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError(format!("unknown tool `{name}`")))?;
        (tool.handler)(arguments)
    }
}

fn validate_tool_name(name: &str) -> Result<(), ToolError> {
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(ToolError(format!("invalid tool name `{name}`")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_injects_and_dispatches_caller_tools() {
        let mut registry = ToolRegistry::default();
        registry
            .register(
                ToolDefinition {
                    name: "capture_analysis".into(),
                    description: "Capture a named graph tap and analyze it".into(),
                    input_schema: serde_json::json!({"type": "object"}),
                },
                |arguments| Ok(serde_json::json!({"received": arguments})),
            )
            .unwrap();
        assert_eq!(registry.definitions().len(), 1);
        assert_eq!(
            registry.call("capture_analysis", serde_json::json!({"tap": "input"})),
            Ok(serde_json::json!({"received": {"tap": "input"}}))
        );
    }

    #[test]
    fn rejects_duplicate_tools() {
        let definition = ToolDefinition {
            name: "probe".into(),
            description: "probe".into(),
            input_schema: serde_json::json!({"type": "object"}),
        };
        let mut registry = ToolRegistry::default();
        registry.register(definition.clone(), |_| Ok(Value::Null)).unwrap();
        assert!(registry.register(definition, |_| Ok(Value::Null)).is_err());
    }

    #[test]
    fn tool_error_is_a_standard_error() {
        fn assert_error<T: std::error::Error>() {}
        assert_error::<ToolError>();
    }
}
