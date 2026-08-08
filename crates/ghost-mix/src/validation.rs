use thiserror::Error;

use crate::{MixOperation, MixPlan};

#[derive(Debug, Error, PartialEq)]
pub enum MixValidationError {
    #[error("unsupported mix plan schema {0}")]
    UnsupportedSchema(String),
    #[error("confidence must be between 0 and 1")]
    InvalidConfidence,
    #[error("plan contains too many operations: {0}")]
    TooManyOperations(usize),
    #[error("invalid operation: {0}")]
    InvalidOperation(String),
}

pub fn validate_mix_plan(plan: &MixPlan) -> Result<(), MixValidationError> {
    if plan.schema_version != MixPlan::SCHEMA {
        return Err(MixValidationError::UnsupportedSchema(
            plan.schema_version.clone(),
        ));
    }
    if !(0.0..=1.0).contains(&plan.confidence) {
        return Err(MixValidationError::InvalidConfidence);
    }
    if plan.operations.len() > 32 {
        return Err(MixValidationError::TooManyOperations(plan.operations.len()));
    }
    for operation in &plan.operations {
        match operation {
            MixOperation::EqBand { settings } => {
                if !(10.0..=30_000.0).contains(&settings.frequency_hz)
                    || !(-18.0..=18.0).contains(&settings.gain_db)
                    || !(0.05..=40.0).contains(&settings.q)
                {
                    return Err(MixValidationError::InvalidOperation(format!(
                        "EQ band {} exceeds workflow safety bounds",
                        settings.band_id
                    )));
                }
            }
            MixOperation::Compressor { settings } => {
                if !(-80.0..=12.0).contains(&settings.threshold_db)
                    || !(1.0..=100.0).contains(&settings.ratio)
                    || !(0.005..=2_000.0).contains(&settings.attack_ms)
                    || !(1.0..=10_000.0).contains(&settings.release_ms)
                    || !(0.0..=100.0).contains(&settings.mix_percent)
                {
                    return Err(MixValidationError::InvalidOperation(
                        "compressor settings exceed workflow safety bounds".into(),
                    ));
                }
            }
            MixOperation::Bypass { .. } => {}
        }
    }
    Ok(())
}
