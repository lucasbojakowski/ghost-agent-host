//! Strict semantic-plan compilation. Preview remains inert until this produces a complete patch.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use ghost_core::ParameterDescriptor;
use ghost_host::{
    CompiledBypassChange, CompiledParameterChange, CompiledParameterPatch, GraphNodeSpec,
    PluginAssignment, ProcessorClass,
};
use ghost_mix::{MixOperation, MixPlan, ProcessorRole};

use crate::PersistedUiState;

static NEXT_TRANSACTION_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_transaction_id() -> u64 {
    NEXT_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub struct ProposalPreview {
    pub plan: MixPlan,
    pub patch: CompiledParameterPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PatchApplicationState {
    #[default]
    Preview,
    MappingIncomplete,
    Queued {
        transaction_id: u64,
    },
    Applied {
        transaction_id: u64,
    },
    Verified {
        transaction_id: u64,
    },
    Failed {
        transaction_id: u64,
        reason: String,
    },
}

pub(crate) fn compile_preview(
    plan: MixPlan,
    state: &PersistedUiState,
    current_values: &BTreeMap<(String, String), f64>,
) -> ProposalPreview {
    let transaction_id = next_transaction_id();
    let mut patch = CompiledParameterPatch {
        transaction_id,
        expected_graph_revision: state.graph_revision,
        parameter_changes: Vec::new(),
        bypass_changes: Vec::new(),
        mapping_issues: Vec::new(),
    };

    for operation in &plan.operations {
        match operation {
            MixOperation::EqBand { settings } => {
                let Some(node) = target_node(state, ProcessorClass::Equalizer, &mut patch) else {
                    continue;
                };
                compile_fields(
                    node,
                    state.graph_revision,
                    &[
                        Field::required(
                            "frequency_hz",
                            &["frequency", "freq", "band frequency"],
                            settings.frequency_hz,
                            Some("hz"),
                        ),
                        Field::required(
                            "gain_db",
                            &["gain", "band gain", "gain db"],
                            settings.gain_db,
                            Some("db"),
                        ),
                        Field::required("q", &["q", "quality", "band q"], settings.q, None),
                    ],
                    current_values,
                    &mut patch,
                );
            }
            MixOperation::Compressor { settings } => {
                let Some(node) = target_node(state, ProcessorClass::Compressor, &mut patch) else {
                    continue;
                };
                compile_fields(
                    node,
                    state.graph_revision,
                    &[
                        Field::required(
                            "threshold_db",
                            &["threshold", "threshold db"],
                            settings.threshold_db,
                            Some("db"),
                        ),
                        Field::required("ratio", &["ratio"], settings.ratio, None),
                        Field::required(
                            "attack_ms",
                            &["attack", "attack time"],
                            settings.attack_ms,
                            Some("ms"),
                        ),
                        Field::required(
                            "release_ms",
                            &["release", "release time"],
                            settings.release_ms,
                            Some("ms"),
                        ),
                        Field::required(
                            "mix_percent",
                            &["mix", "dry wet", "wet"],
                            settings.mix_percent,
                            Some("%"),
                        ),
                        Field::required(
                            "output_gain_db",
                            &["output gain", "makeup gain", "gain"],
                            settings.output_gain_db,
                            Some("db"),
                        ),
                        Field::optional(
                            "knee_db",
                            &["knee", "knee db"],
                            settings.knee_db,
                            Some("db"),
                        ),
                        Field::optional(
                            "range_db",
                            &["range", "range db"],
                            settings.range_db,
                            Some("db"),
                        ),
                    ],
                    current_values,
                    &mut patch,
                );
            }
            MixOperation::Bypass { target, bypassed } => {
                let class = match target {
                    ProcessorRole::Equalizer => ProcessorClass::Equalizer,
                    ProcessorRole::Compressor => ProcessorClass::Compressor,
                };
                if let Some(node) = state.graph.nodes.iter().find(|node| node.class == class) {
                    patch.bypass_changes.push(CompiledBypassChange {
                        target_node_id: node.id.clone(),
                        expected_graph_revision: state.graph_revision,
                        bypassed: *bypassed,
                        previous_bypassed: node.bypassed,
                    });
                } else {
                    patch
                        .mapping_issues
                        .push(format!("No {} node exists for bypass", class.label()));
                }
            }
        }
    }

    if patch.parameter_changes.is_empty()
        && patch.bypass_changes.is_empty()
        && patch.mapping_issues.is_empty()
    {
        patch
            .mapping_issues
            .push("The proposal contains no applicable operations".into());
    }
    ProposalPreview { plan, patch }
}

fn target_node<'a>(
    state: &'a PersistedUiState,
    class: ProcessorClass,
    patch: &mut CompiledParameterPatch,
) -> Option<&'a GraphNodeSpec> {
    let candidates: Vec<_> = state
        .graph
        .nodes
        .iter()
        .filter(|node| node.class == class && node.plugin.is_some())
        .collect();
    match candidates.as_slice() {
        [node] => Some(*node),
        [] => {
            patch.mapping_issues.push(format!(
                "No assigned {} node can receive this operation",
                class.label()
            ));
            None
        }
        _ => {
            patch.mapping_issues.push(format!(
                "Multiple assigned {} nodes exist; choose an explicit proposal target",
                class.label()
            ));
            None
        }
    }
}

struct Field<'a> {
    semantic: &'a str,
    aliases: &'a [&'a str],
    value: f64,
    unit: Option<&'a str>,
    required: bool,
}

impl<'a> Field<'a> {
    const fn required(
        semantic: &'a str,
        aliases: &'a [&'a str],
        value: f64,
        unit: Option<&'a str>,
    ) -> Self {
        Self {
            semantic,
            aliases,
            value,
            unit,
            required: true,
        }
    }

    const fn optional(
        semantic: &'a str,
        aliases: &'a [&'a str],
        value: f64,
        unit: Option<&'a str>,
    ) -> Self {
        Self {
            semantic,
            aliases,
            value,
            unit,
            required: false,
        }
    }
}

fn compile_fields(
    node: &GraphNodeSpec,
    revision: u64,
    fields: &[Field<'_>],
    current_values: &BTreeMap<(String, String), f64>,
    patch: &mut CompiledParameterPatch,
) {
    let Some(plugin) = node.plugin.as_ref() else {
        return;
    };
    for field in fields {
        match best_parameter(plugin, field) {
            MatchResult::Found(parameter, confidence) => {
                let value = adapt_value(parameter, field.value, field.unit);
                if !(parameter.minimum..=parameter.maximum).contains(&value) {
                    patch.mapping_issues.push(format!(
                        "{} maps to {} but {:.3} is outside {:.3}…{:.3}",
                        field.semantic, parameter.name, value, parameter.minimum, parameter.maximum
                    ));
                    continue;
                }
                patch.parameter_changes.push(CompiledParameterChange {
                    target_node_id: node.id.clone(),
                    expected_graph_revision: revision,
                    semantic_field: field.semantic.into(),
                    parameter_id: parameter.stable_id.clone(),
                    plain_value: value,
                    minimum: parameter.minimum,
                    maximum: parameter.maximum,
                    mapping_confidence: confidence,
                    previous_value: current_values
                        .get(&(node.id.clone(), parameter.stable_id.clone()))
                        .copied()
                        .unwrap_or(parameter.default),
                    requires_restart: false,
                });
            }
            MatchResult::Ambiguous => patch.mapping_issues.push(format!(
                "{} has multiple equally plausible parameters in {}",
                field.semantic, plugin.name
            )),
            MatchResult::Missing if field.required => patch.mapping_issues.push(format!(
                "{} has no safe public-parameter mapping in {}",
                field.semantic, plugin.name
            )),
            MatchResult::Missing => {}
        }
    }
}

enum MatchResult<'a> {
    Found(&'a ParameterDescriptor, f32),
    Ambiguous,
    Missing,
}

fn best_parameter<'a>(plugin: &'a PluginAssignment, field: &Field<'_>) -> MatchResult<'a> {
    let mut candidates: Vec<_> = plugin
        .public_parameters
        .iter()
        .filter(|parameter| !parameter.read_only)
        .filter_map(|parameter| score(parameter, field).map(|score| (parameter, score)))
        .collect();
    candidates.sort_by(|left, right| right.1.total_cmp(&left.1));
    let Some((best, score)) = candidates.first().copied() else {
        return MatchResult::Missing;
    };
    if candidates
        .get(1)
        .is_some_and(|(_, second)| (score - *second).abs() < 0.03)
    {
        return MatchResult::Ambiguous;
    }
    MatchResult::Found(best, score.min(1.0))
}

fn score(parameter: &ParameterDescriptor, field: &Field<'_>) -> Option<f32> {
    let name = normalize(&parameter.name);
    let mut best = field.aliases.iter().fold(0.0_f32, |best, alias| {
        let alias = normalize(alias);
        best.max(if name == alias {
            0.92
        } else if name.ends_with(&alias) || name.starts_with(&alias) {
            0.78
        } else if name.contains(&alias) {
            0.68
        } else {
            0.0
        })
    });
    if best == 0.0 {
        return None;
    }
    if let (Some(expected), Some(actual)) = (field.unit, parameter.unit.as_deref()) {
        if normalize(expected) == normalize(actual) {
            best += 0.08;
        } else {
            best -= 0.12;
        }
    }
    Some(best)
}

fn adapt_value(parameter: &ParameterDescriptor, value: f64, unit: Option<&str>) -> f64 {
    if unit == Some("%") && parameter.maximum <= 1.0 && parameter.minimum >= 0.0 {
        value / 100.0
    } else {
        value
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use ghost_host::{EditableGraph, GraphNodeSpec};
    use ghost_mix::{CompressorOperation, EqBandOperation, EqShape, MixOperation};

    use super::*;

    #[test]
    fn compiler_rejects_incomplete_mapping_instead_of_partial_apply() {
        let mut state = PersistedUiState::default();
        state.graph = EditableGraph {
            nodes: vec![GraphNodeSpec {
                id: "comp-1".into(),
                class: ProcessorClass::Compressor,
                bypassed: false,
                plugin: Some(PluginAssignment {
                    path: PathBuf::from("fake.clap"),
                    plugin_id: Some("fake".into()),
                    name: "Incomplete compressor".into(),
                    vendor: None,
                    version: None,
                    public_parameters: vec![ParameterDescriptor {
                        stable_id: "1".into(),
                        name: "Threshold".into(),
                        module: None,
                        unit: Some("dB".into()),
                        minimum: -60.0,
                        maximum: 0.0,
                        default: -12.0,
                        stepped: false,
                        read_only: false,
                        labels: BTreeMap::new(),
                    }],
                    state: None,
                }),
            }],
        };
        let plan = MixPlan {
            schema_version: MixPlan::SCHEMA.into(),
            summary: "test".into(),
            confidence: 1.0,
            assumptions: Vec::new(),
            operations: vec![MixOperation::Compressor {
                settings: CompressorOperation {
                    enabled: true,
                    style: "clean".into(),
                    threshold_db: -18.0,
                    ratio: 2.0,
                    knee_db: 3.0,
                    attack_ms: 20.0,
                    release_ms: 100.0,
                    range_db: 6.0,
                    mix_percent: 100.0,
                    output_gain_db: 0.0,
                    rationale: String::new(),
                    evidence: Vec::new(),
                },
            }],
            expected_changes: Vec::new(),
            cautions: Vec::new(),
        };
        let current = BTreeMap::from([(("comp-1".into(), "1".into()), -21.0)]);
        let preview = compile_preview(plan, &state, &current);
        assert!(!preview.patch.can_apply());
        assert!(preview.patch.mapping_issues.len() >= 5);
        assert_eq!(preview.patch.parameter_changes[0].previous_value, -21.0);
    }

    #[test]
    fn compiler_produces_complete_revision_bound_eq_patch() {
        let mut state = PersistedUiState::default();
        state.graph_revision = 12;
        state.graph = EditableGraph {
            nodes: vec![GraphNodeSpec {
                id: "eq-1".into(),
                class: ProcessorClass::Equalizer,
                bypassed: false,
                plugin: Some(PluginAssignment {
                    path: PathBuf::from("eq.clap"),
                    plugin_id: Some("eq".into()),
                    name: "Mapped EQ".into(),
                    vendor: None,
                    version: None,
                    public_parameters: vec![
                        parameter("10", "Frequency", "Hz", 20.0, 20_000.0),
                        parameter("11", "Gain", "dB", -24.0, 24.0),
                        parameter("12", "Q", "", 0.1, 20.0),
                    ],
                    state: None,
                }),
            }],
        };
        let plan = MixPlan {
            schema_version: MixPlan::SCHEMA.into(),
            summary: "test".into(),
            confidence: 1.0,
            assumptions: Vec::new(),
            operations: vec![MixOperation::EqBand {
                settings: EqBandOperation {
                    band_id: "low-mid".into(),
                    enabled: true,
                    shape: EqShape::Bell,
                    frequency_hz: 280.0,
                    gain_db: -2.5,
                    q: 1.2,
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
        let current = BTreeMap::from([
            (("eq-1".into(), "10".into()), 440.0),
            (("eq-1".into(), "11".into()), 0.0),
            (("eq-1".into(), "12".into()), 0.7),
        ]);
        let preview = compile_preview(plan, &state, &current);
        assert!(preview.patch.can_apply());
        assert_eq!(preview.patch.expected_graph_revision, 12);
        assert_eq!(preview.patch.parameter_changes.len(), 3);
        assert!(preview
            .patch
            .parameter_changes
            .iter()
            .all(|change| change.mapping_confidence >= 0.9));
    }

    fn parameter(
        stable_id: &str,
        name: &str,
        unit: &str,
        minimum: f64,
        maximum: f64,
    ) -> ParameterDescriptor {
        ParameterDescriptor {
            stable_id: stable_id.into(),
            name: name.into(),
            module: None,
            unit: (!unit.is_empty()).then(|| unit.into()),
            minimum,
            maximum,
            default: 0.0_f64.clamp(minimum, maximum),
            stepped: false,
            read_only: false,
            labels: BTreeMap::new(),
        }
    }
}
