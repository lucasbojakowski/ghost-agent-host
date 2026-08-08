use std::collections::BTreeMap;

use serde_json::Value;
use thiserror::Error;

use crate::{TaskOperation, TaskPlan};

#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("unsupported task plan schema {0}")]
    UnsupportedSchema(String),
    #[error("confidence must be between 0 and 1")]
    InvalidConfidence,
    #[error("plan contains too many operations: {0}")]
    TooManyOperations(usize),
    #[error("operation {operation_id} is invalid: {reason}")]
    InvalidOperation {
        operation_id: String,
        reason: String,
    },
}

pub trait OperationValidator: Send + Sync {
    fn namespace(&self) -> &str;
    fn validate(&self, operation: &TaskOperation) -> Result<(), String>;
}

#[derive(Default)]
pub struct ValidationRegistry {
    maximum_operations: usize,
    validators: BTreeMap<String, Box<dyn OperationValidator>>,
}

impl ValidationRegistry {
    pub fn new(maximum_operations: usize) -> Self {
        Self {
            maximum_operations,
            validators: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, validator: impl OperationValidator + 'static) {
        self.validators
            .insert(validator.namespace().to_owned(), Box::new(validator));
    }

    pub fn validate(&self, plan: &TaskPlan) -> Result<(), ValidationError> {
        if plan.schema_version != TaskPlan::SCHEMA {
            return Err(ValidationError::UnsupportedSchema(
                plan.schema_version.clone(),
            ));
        }
        if !(0.0..=1.0).contains(&plan.confidence) {
            return Err(ValidationError::InvalidConfidence);
        }
        if plan.operations.len() > self.maximum_operations {
            return Err(ValidationError::TooManyOperations(plan.operations.len()));
        }
        for operation in &plan.operations {
            let validator = self.validators.get(&operation.namespace).ok_or_else(|| {
                ValidationError::InvalidOperation {
                    operation_id: operation.operation_id.clone(),
                    reason: format!("no validator registered for {}", operation.namespace),
                }
            })?;
            validator
                .validate(operation)
                .map_err(|reason| ValidationError::InvalidOperation {
                    operation_id: operation.operation_id.clone(),
                    reason,
                })?;
        }
        Ok(())
    }
}

pub fn finite_number(value: Option<&Value>, name: &str) -> Result<f64, String> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{name} must be a finite number"))
}
