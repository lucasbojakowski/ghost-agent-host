use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use ghost_core::{ExpectedOutcome, TaskOperation, TaskPlan};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessorRole {
    Equalizer,
    Compressor,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EqShape {
    Bell,
    LowShelf,
    HighShelf,
    LowCut,
    HighCut,
    Notch,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DynamicEqSettings {
    pub enabled: bool,
    pub range_db: f64,
    pub threshold_db: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EqBandOperation {
    pub band_id: String,
    pub enabled: bool,
    pub shape: EqShape,
    pub frequency_hz: f64,
    pub gain_db: f64,
    pub q: f64,
    pub slope_db_oct: Option<f64>,
    pub channel_mode: String,
    pub dynamic: Option<DynamicEqSettings>,
    pub rationale: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompressorOperation {
    pub enabled: bool,
    pub style: String,
    pub threshold_db: f64,
    pub ratio: f64,
    pub knee_db: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub range_db: f64,
    pub mix_percent: f64,
    pub output_gain_db: f64,
    pub rationale: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum MixOperation {
    EqBand {
        settings: EqBandOperation,
    },
    Compressor {
        settings: CompressorOperation,
    },
    Bypass {
        target: ProcessorRole,
        bypassed: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExpectedChange {
    pub metric: String,
    pub direction: String,
    pub maximum_delta: Option<f64>,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MixPlan {
    pub schema_version: String,
    pub summary: String,
    pub confidence: f64,
    pub assumptions: Vec<String>,
    pub operations: Vec<MixOperation>,
    pub expected_changes: Vec<ExpectedChange>,
    pub cautions: Vec<String>,
}

impl MixPlan {
    pub const SCHEMA: &'static str = "ghost.mix-plan/1";

    pub fn to_task_plan(&self) -> TaskPlan {
        let operations = self
            .operations
            .iter()
            .enumerate()
            .map(|(index, operation)| match operation {
                MixOperation::EqBand { settings } => TaskOperation {
                    operation_id: settings.band_id.clone(),
                    namespace: "audio.mix".into(),
                    kind: "equalizer.band".into(),
                    target: Some("role:equalizer".into()),
                    arguments: IndexMap::from([
                        ("enabled".into(), json!(settings.enabled)),
                        ("shape".into(), json!(settings.shape)),
                        ("frequency_hz".into(), json!(settings.frequency_hz)),
                        ("gain_db".into(), json!(settings.gain_db)),
                        ("q".into(), json!(settings.q)),
                        ("channel_mode".into(), json!(settings.channel_mode)),
                        ("dynamic".into(), json!(settings.dynamic)),
                    ]),
                    evidence: settings.evidence.clone(),
                    rationale: settings.rationale.clone(),
                },
                MixOperation::Compressor { settings } => TaskOperation {
                    operation_id: format!("compressor-{index}"),
                    namespace: "audio.mix".into(),
                    kind: "compressor.settings".into(),
                    target: Some("role:compressor".into()),
                    arguments: IndexMap::from([
                        ("enabled".into(), json!(settings.enabled)),
                        ("style".into(), json!(settings.style)),
                        ("threshold_db".into(), json!(settings.threshold_db)),
                        ("ratio".into(), json!(settings.ratio)),
                        ("knee_db".into(), json!(settings.knee_db)),
                        ("attack_ms".into(), json!(settings.attack_ms)),
                        ("release_ms".into(), json!(settings.release_ms)),
                        ("range_db".into(), json!(settings.range_db)),
                        ("mix_percent".into(), json!(settings.mix_percent)),
                        ("output_gain_db".into(), json!(settings.output_gain_db)),
                    ]),
                    evidence: settings.evidence.clone(),
                    rationale: settings.rationale.clone(),
                },
                MixOperation::Bypass { target, bypassed } => TaskOperation {
                    operation_id: format!("bypass-{index}"),
                    namespace: "audio.mix".into(),
                    kind: "processor.bypass".into(),
                    target: Some(format!(
                        "role:{}",
                        match target {
                            ProcessorRole::Equalizer => "equalizer",
                            ProcessorRole::Compressor => "compressor",
                        }
                    )),
                    arguments: IndexMap::from([("bypassed".into(), json!(bypassed))]),
                    evidence: Vec::new(),
                    rationale: String::new(),
                },
            })
            .collect();
        TaskPlan {
            schema_version: TaskPlan::SCHEMA.into(),
            task_id: Uuid::new_v4(),
            summary: self.summary.clone(),
            confidence: self.confidence,
            assumptions: self.assumptions.clone(),
            operations,
            expected_outcomes: self
                .expected_changes
                .iter()
                .map(|change| ExpectedOutcome {
                    subject: change.metric.clone(),
                    predicate: change.direction.clone(),
                    value: change.maximum_delta.map(|value| {
                        json!({
                            "maximum_delta": value,
                            "unit": change.unit,
                        })
                    }),
                })
                .collect(),
            cautions: self.cautions.clone(),
            extensions: IndexMap::new(),
        }
    }
}
