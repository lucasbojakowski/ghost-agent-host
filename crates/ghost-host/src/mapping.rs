use std::collections::BTreeMap;

use ghost_core::{ParameterDescriptor, ProcessorDescriptor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticParameterSpec {
    pub semantic_id: String,
    pub aliases: Vec<String>,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParameterBinding {
    pub semantic_id: String,
    pub parameter_id: String,
}

pub fn map_public_parameters(
    processor: &ProcessorDescriptor,
    specs: &[SemanticParameterSpec],
) -> Vec<ParameterBinding> {
    specs
        .iter()
        .filter_map(|spec| {
            best_match(&processor.parameters, spec).map(|parameter| ParameterBinding {
                semantic_id: spec.semantic_id.clone(),
                parameter_id: parameter.stable_id.clone(),
            })
        })
        .collect()
}

pub fn binding_map(bindings: &[ParameterBinding]) -> BTreeMap<&str, &str> {
    bindings
        .iter()
        .map(|binding| (binding.semantic_id.as_str(), binding.parameter_id.as_str()))
        .collect()
}

fn best_match<'a>(
    parameters: &'a [ParameterDescriptor],
    spec: &SemanticParameterSpec,
) -> Option<&'a ParameterDescriptor> {
    parameters
        .iter()
        .filter(|parameter| !parameter.read_only)
        .filter(|parameter| {
            spec.unit.as_ref().is_none_or(|unit| {
                parameter
                    .unit
                    .as_ref()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(unit))
            })
        })
        .find(|parameter| {
            let name = normalize(&parameter.name);
            spec.aliases.iter().any(|alias| name == normalize(alias))
        })
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
