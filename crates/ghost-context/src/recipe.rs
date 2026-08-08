use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::{ContextError, ContextValue};

#[derive(Default)]
pub struct ContextInputs {
    values: BTreeMap<String, ContextValue>,
}

impl ContextInputs {
    pub fn text(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.values
            .insert(key.into(), ContextValue::Text(value.into()));
        self
    }

    pub fn json(
        mut self,
        key: impl Into<String>,
        value: impl Serialize,
    ) -> Result<Self, ContextError> {
        self.values
            .insert(key.into(), ContextValue::Json(serde_json::to_value(value)?));
        Ok(self)
    }

    pub fn raw_json(mut self, key: impl Into<String>, value: Value) -> Self {
        self.values.insert(key.into(), ContextValue::Json(value));
        self
    }

    pub fn values(&self) -> &BTreeMap<String, ContextValue> {
        &self.values
    }
}
