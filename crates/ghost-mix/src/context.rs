use schemars::schema_for;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use ghost_context::{
    CompiledContext, ContextCompiler, ContextError, ContextInputs, JsonComponent, OutputContract,
    TextComponent,
};
use ghost_core::ParameterDescriptor;
use ghost_core::{AnalysisBundle, UserIntent};

use crate::MixPlan;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginCapabilitySummary {
    pub plugin: String,
    pub version: String,
    pub supported_operations: Vec<String>,
    pub safety_notes: Vec<String>,
    #[serde(default)]
    pub public_parameters: Vec<ParameterDescriptor>,
}

/// Compatibility envelope for persisted v1 runs. New runtimes consume `compiled` only.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PromptBundle {
    pub schema_version: String,
    pub system_prompt: String,
    pub user_intent: UserIntent,
    pub analysis_text_json: String,
    pub capability_text_json: String,
    pub output_contract: String,
    pub compiled: CompiledContext,
}

pub fn build_prompt_bundle(
    system_prompt: impl Into<String>,
    intent: UserIntent,
    analysis: &AnalysisBundle,
    capabilities: &[PluginCapabilitySummary],
) -> Result<PromptBundle, ContextError> {
    let system_prompt = system_prompt.into();
    let output = OutputContract::Json {
        schema_name: MixPlan::SCHEMA.into(),
        schema: serde_json::to_value(schema_for!(MixPlan))?,
    };
    let compiler = ContextCompiler::new(output)
        .system(TextComponent::new("system"))
        .user(JsonComponent::new("intent", "USER INTENT"))
        .user(JsonComponent::new("analysis", "ANALYSIS"))
        .user(JsonComponent::new("capabilities", "PROCESSOR CAPABILITIES"))
        .user(TextComponent::new("contract").heading("OUTPUT CONTRACT"));
    let contract = "Return exactly one JSON object matching the supplied response schema. Use semantic processor operations only; do not invent raw parameter IDs or perform filesystem operations.";
    let inputs = ContextInputs::default()
        .text("system", system_prompt.clone())
        .json("intent", &intent)?
        .json("analysis", analysis)?
        .json("capabilities", capabilities)?
        .text("contract", contract);
    let compiled = compiler.compile(inputs.values())?;
    Ok(PromptBundle {
        schema_version: "ghost.prompt-bundle/2".into(),
        system_prompt,
        user_intent: intent,
        analysis_text_json: serde_json::to_string_pretty(analysis)?,
        capability_text_json: serde_json::to_string_pretty(capabilities)?,
        output_contract: contract.into(),
        compiled,
    })
}
