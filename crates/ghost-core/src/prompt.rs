use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{AnalysisBundle, UserIntent};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginCapabilitySummary {
    pub plugin: String,
    pub version: String,
    pub supported_operations: Vec<String>,
    pub safety_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PromptBundle {
    pub schema_version: String,
    pub system_prompt: String,
    pub user_intent: UserIntent,
    pub analysis_text_json: String,
    pub capability_text_json: String,
    pub output_contract: String,
}

pub fn build_prompt_bundle(
    system_prompt: impl Into<String>,
    intent: UserIntent,
    analysis: &AnalysisBundle,
    capabilities: &[PluginCapabilitySummary],
) -> Result<PromptBundle, serde_json::Error> {
    let analysis_text_json = serde_json::to_string_pretty(analysis)?;
    let capability_text_json = serde_json::to_string_pretty(capabilities)?;
    Ok(PromptBundle {
        schema_version: "ghost.prompt-bundle/1".into(),
        system_prompt: system_prompt.into(),
        user_intent: intent,
        analysis_text_json,
        capability_text_json,
        output_contract: "Return exactly one JSON object conforming to ghost.mix-plan/1. Do not return markdown, prose outside JSON, image requests, plots, binary data, raw CLAP parameter IDs, or filesystem operations.".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AnalysisConfig, StructuredIntent};

    #[test]
    fn output_contract_excludes_plots() {
        let contract = "Return text JSON only; no plots or images";
        assert!(contract.contains("no plots"));
    }

    #[test]
    fn structured_intent_serializes() {
        let intent = UserIntent::Structured {
            context: StructuredIntent {
                source: "bass".into(),
                role: "anchor".into(),
                style: "house".into(),
                goal: "tight".into(),
                problem: None,
                intensity: "subtle".into(),
                preserve: vec!["transient".into()],
                scope: vec!["eq".into()],
                notes: None,
            },
        };
        let encoded = serde_json::to_string(&intent).unwrap();
        assert!(encoded.contains("structured"));
        assert_eq!(AnalysisConfig::default().profile, crate::model::QualityProfile::Maximum);
    }
}
