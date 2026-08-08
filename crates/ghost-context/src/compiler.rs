use std::collections::BTreeMap;

use thiserror::Error;

use crate::{CompiledContext, ContextMessage, ContextValue, MessageRole, OutputContract};

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("context component `{0}` is missing")]
    MissingComponent(String),
    #[error("context component `{0}` has the wrong representation")]
    WrongRepresentation(String),
    #[error("failed to serialize context component: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub trait ContextComponent: Send + Sync {
    fn key(&self) -> &str;
    fn render(&self, value: &ContextValue) -> Result<String, ContextError>;
}

pub struct TextComponent {
    key: String,
    heading: Option<String>,
}

impl TextComponent {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            heading: None,
        }
    }

    pub fn heading(mut self, heading: impl Into<String>) -> Self {
        self.heading = Some(heading.into());
        self
    }
}

impl ContextComponent for TextComponent {
    fn key(&self) -> &str {
        &self.key
    }

    fn render(&self, value: &ContextValue) -> Result<String, ContextError> {
        let text = match value {
            ContextValue::Text(text) => text.clone(),
            ContextValue::Json(_) => {
                return Err(ContextError::WrongRepresentation(self.key.clone()))
            }
        };
        Ok(self
            .heading
            .as_ref()
            .map_or(text.clone(), |heading| format!("{heading}:\n{text}")))
    }
}

pub struct JsonComponent {
    key: String,
    heading: String,
    pretty: bool,
}

impl JsonComponent {
    pub fn new(key: impl Into<String>, heading: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            heading: heading.into(),
            pretty: true,
        }
    }

    pub fn compact(mut self) -> Self {
        self.pretty = false;
        self
    }
}

impl ContextComponent for JsonComponent {
    fn key(&self) -> &str {
        &self.key
    }

    fn render(&self, value: &ContextValue) -> Result<String, ContextError> {
        let value = match value {
            ContextValue::Json(value) => value,
            ContextValue::Text(_) => {
                return Err(ContextError::WrongRepresentation(self.key.clone()))
            }
        };
        let body = if self.pretty {
            serde_json::to_string_pretty(value)?
        } else {
            serde_json::to_string(value)?
        };
        Ok(format!("{}:\n{}", self.heading, body))
    }
}

pub struct ContextCompiler {
    system: Vec<Box<dyn ContextComponent>>,
    user: Vec<Box<dyn ContextComponent>>,
    output: OutputContract,
}

impl ContextCompiler {
    pub fn new(output: OutputContract) -> Self {
        Self {
            system: Vec::new(),
            user: Vec::new(),
            output,
        }
    }

    pub fn system(mut self, component: impl ContextComponent + 'static) -> Self {
        self.system.push(Box::new(component));
        self
    }

    pub fn user(mut self, component: impl ContextComponent + 'static) -> Self {
        self.user.push(Box::new(component));
        self
    }

    pub fn compile(
        &self,
        values: &BTreeMap<String, ContextValue>,
    ) -> Result<CompiledContext, ContextError> {
        let mut messages = Vec::new();
        for (role, components) in [
            (MessageRole::System, self.system.as_slice()),
            (MessageRole::User, self.user.as_slice()),
        ] {
            if components.is_empty() {
                continue;
            }
            let rendered = components
                .iter()
                .map(|component| {
                    let value = values
                        .get(component.key())
                        .ok_or_else(|| ContextError::MissingComponent(component.key().into()))?;
                    component.render(value)
                })
                .collect::<Result<Vec<_>, _>>()?;
            messages.push(ContextMessage {
                role,
                content: rendered.join("\n\n"),
            });
        }
        Ok(CompiledContext {
            schema_version: CompiledContext::SCHEMA.into(),
            messages,
            output: self.output.clone(),
            metadata: serde_json::Value::Null,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_values_can_be_presented_by_different_recipes() {
        let values = BTreeMap::from([
            (
                "persona".into(),
                ContextValue::Text("Be conservative".into()),
            ),
            (
                "analysis".into(),
                ContextValue::Json(serde_json::json!({"crest": 12})),
            ),
        ]);
        let compact = ContextCompiler::new(OutputContract::Text)
            .system(TextComponent::new("persona"))
            .user(JsonComponent::new("analysis", "Evidence").compact())
            .compile(&values)
            .unwrap();
        let pretty = ContextCompiler::new(OutputContract::Text)
            .system(TextComponent::new("persona").heading("PERSONA"))
            .user(JsonComponent::new("analysis", "DETAILED ANALYSIS"))
            .compile(&values)
            .unwrap();
        assert_ne!(compact.text(), pretty.text());
    }
}
