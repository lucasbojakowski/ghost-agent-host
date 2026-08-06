use thiserror::Error;

use crate::model::{MixOperation, MixPlan};

#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("unsupported mix plan schema {0}")]
    UnsupportedSchema(String),
    #[error("confidence must be between 0 and 1")]
    InvalidConfidence,
    #[error("plan contains too many operations: {0}")]
    TooManyOperations(usize),
    #[error("invalid EQ operation {band_id}: {reason}")]
    InvalidEq { band_id: String, reason: String },
    #[error("invalid compressor operation: {0}")]
    InvalidCompressor(String),
}

pub fn validate_mix_plan(plan: &MixPlan) -> Result<(), ValidationError> {
    if plan.schema_version != "ghost.mix-plan/1" {
        return Err(ValidationError::UnsupportedSchema(plan.schema_version.clone()));
    }
    if !(0.0..=1.0).contains(&plan.confidence) {
        return Err(ValidationError::InvalidConfidence);
    }
    if plan.operations.len() > 32 {
        return Err(ValidationError::TooManyOperations(plan.operations.len()));
    }

    for operation in &plan.operations {
        match operation {
            MixOperation::EqBand { settings } => {
                if !(10.0..=30_000.0).contains(&settings.frequency_hz) {
                    return Err(ValidationError::InvalidEq {
                        band_id: settings.band_id.clone(),
                        reason: "frequency outside 10 Hz to 30 kHz".into(),
                    });
                }
                if !(-18.0..=18.0).contains(&settings.gain_db) {
                    return Err(ValidationError::InvalidEq {
                        band_id: settings.band_id.clone(),
                        reason: "gain outside ±18 dB".into(),
                    });
                }
                if !(0.05..=40.0).contains(&settings.q) {
                    return Err(ValidationError::InvalidEq {
                        band_id: settings.band_id.clone(),
                        reason: "Q outside 0.05 to 40".into(),
                    });
                }
                if let Some(dynamic) = &settings.dynamic {
                    if dynamic.range_db.abs() > 12.0 {
                        return Err(ValidationError::InvalidEq {
                            band_id: settings.band_id.clone(),
                            reason: "dynamic range exceeds 12 dB".into(),
                        });
                    }
                }
            }
            MixOperation::Compressor { settings } => {
                if !(-80.0..=12.0).contains(&settings.threshold_db) {
                    return Err(ValidationError::InvalidCompressor(
                        "threshold outside -80 to +12 dB".into(),
                    ));
                }
                if !(1.0..=100.0).contains(&settings.ratio) {
                    return Err(ValidationError::InvalidCompressor(
                        "ratio outside 1:1 to 100:1".into(),
                    ));
                }
                if !(0.005..=2_000.0).contains(&settings.attack_ms)
                    || !(1.0..=10_000.0).contains(&settings.release_ms)
                {
                    return Err(ValidationError::InvalidCompressor(
                        "attack or release outside validation bounds".into(),
                    ));
                }
                if !(0.0..=100.0).contains(&settings.mix_percent) {
                    return Err(ValidationError::InvalidCompressor(
                        "mix outside 0 to 100 percent".into(),
                    ));
                }
            }
            MixOperation::Bypass { .. } => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EqBandOperation, EqShape, MixOperation, MixPlan};

    #[test]
    fn rejects_extreme_eq_gain() {
        let plan = MixPlan {
            schema_version: "ghost.mix-plan/1".into(),
            summary: String::new(),
            confidence: 0.8,
            assumptions: Vec::new(),
            operations: vec![MixOperation::EqBand {
                settings: EqBandOperation {
                    band_id: "test".into(),
                    enabled: true,
                    shape: EqShape::Bell,
                    frequency_hz: 200.0,
                    gain_db: 30.0,
                    q: 1.0,
                    slope_db_oct: None,
                    channel_mode: "stereo".into(),
                    dynamic: None,
                    rationale: String::new(),
                    evidence: Vec::new(),
                },
            }],
            expected_changes: Vec::new(),
            cautions: Vec::new(),
        };
        assert!(matches!(
            validate_mix_plan(&plan),
            Err(ValidationError::InvalidEq { .. })
        ));
    }
}
