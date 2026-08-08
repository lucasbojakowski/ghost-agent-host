//! Strict semantic-plan compilation. Preview remains inert until this produces a complete patch.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use ghost_core::ParameterDescriptor;
use ghost_host::{
    CompiledBypassChange, CompiledParameterChange, CompiledParameterPatch, GraphNodeSpec,
    PluginAssignment, ProcessorClass,
};
use ghost_mix::{EqBandOperation, EqShape, MixOperation, MixPlan, ProcessorRole};

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

#[derive(Default)]
struct EqBandAllocations {
    logical_to_family: BTreeMap<(String, String), String>,
    used_by_node: BTreeMap<String, BTreeSet<String>>,
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
    let mut eq_allocations = EqBandAllocations::default();

    for operation in &plan.operations {
        match operation {
            MixOperation::EqBand { settings } => {
                let Some(node) = target_node(state, ProcessorClass::Equalizer, &mut patch) else {
                    continue;
                };
                compile_eq_band(
                    node,
                    state.graph_revision,
                    settings,
                    current_values,
                    &mut eq_allocations,
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

fn compile_eq_band(
    node: &GraphNodeSpec,
    revision: u64,
    settings: &EqBandOperation,
    current_values: &BTreeMap<(String, String), f64>,
    allocations: &mut EqBandAllocations,
    patch: &mut CompiledParameterPatch,
) {
    let Some(plugin) = node.plugin.as_ref() else {
        return;
    };

    let frequency = Field::required(
        "frequency_hz",
        &["frequency", "freq", "band frequency"],
        settings.frequency_hz,
        Some("hz"),
    );
    let gain = Field::required(
        "gain_db",
        &["gain", "band gain", "gain db"],
        settings.gain_db,
        Some("db"),
    );
    let q = Field::required("q", &["q", "quality", "band q"], settings.q, None);

    // Pro-Q 4 exposes both `Band N Used` and `Band N Enabled`. They are not synonyms: `Used`
    // materializes/removes a physical band slot, while `Enabled` controls the active state of a
    // band that already exists. Keeping them in one alias set makes both equally plausible and the
    // optional mapping silently disappears, which is why edits worked only after manual creation.
    let used = Field::optional(
        "used",
        &["used", "band used", "in use"],
        if settings.enabled { 1.0 } else { 0.0 },
        None,
    );
    let enabled = Field::optional(
        "enabled",
        &["enabled", "enable", "active", "band enabled"],
        if settings.enabled { 1.0 } else { 0.0 },
        None,
    );
    let shape = Field::optional(
        "shape",
        &["shape", "type", "filter type", "band type"],
        0.0,
        None,
    );
    let slope = settings.slope_db_oct.map(|value| {
        Field::required(
            "slope_db_oct",
            &["slope", "slope db oct", "slope db/oct"],
            value,
            Some("db/oct"),
        )
    });

    let mut grouping_fields = vec![&frequency, &gain, &q, &used, &enabled, &shape];
    if let Some(slope) = slope.as_ref() {
        grouping_fields.push(slope);
    }
    let families = eq_parameter_families(plugin, &grouping_fields);
    let mut required_fields = vec![&frequency, &gain, &q];
    if let Some(slope) = slope.as_ref() {
        required_fields.push(slope);
    }

    let mut complete: Vec<_> = families
        .iter()
        .filter(|(_, parameters)| {
            required_fields.iter().all(|field| {
                matches!(
                    best_parameter_in(parameters, field),
                    MatchResult::Found(_, _)
                )
            })
        })
        .collect();
    complete.sort_by(|(left_key, left), (right_key, right)| {
        family_minimum_parameter_id(left)
            .cmp(&family_minimum_parameter_id(right))
            .then_with(|| left_key.cmp(right_key))
    });

    if complete.is_empty() {
        patch.mapping_issues.push(format!(
            "{} has no coherent EQ band exposing frequency, gain and Q{}",
            plugin.name,
            if slope.is_some() { " plus slope" } else { "" }
        ));
        return;
    }

    let allocation_key = (node.id.clone(), settings.band_id.clone());
    let selected_family = if let Some(family) = allocations.logical_to_family.get(&allocation_key) {
        family.clone()
    } else {
        let allocated = allocations.used_by_node.entry(node.id.clone()).or_default();
        let available: Vec<_> = complete
            .iter()
            .filter(|(key, _)| !allocated.contains(key.as_str()))
            .copied()
            .collect();
        let Some((family, _)) = available.first() else {
            patch.mapping_issues.push(format!(
                "{} has no unused coherent EQ band for logical band `{}`",
                plugin.name, settings.band_id
            ));
            return;
        };
        let family = (*family).clone();
        allocated.insert(family.clone());
        allocations
            .logical_to_family
            .insert(allocation_key, family.clone());
        family
    };

    let Some(parameters) = families.get(&selected_family) else {
        patch.mapping_issues.push(format!(
            "EQ band mapping for `{}` became stale in {}",
            settings.band_id, plugin.name
        ));
        return;
    };

    for field in required_fields {
        compile_selected_field(
            node,
            revision,
            plugin,
            parameters,
            field,
            0.08,
            current_values,
            patch,
        );
    }

    compile_optional_band_switch(
        node,
        revision,
        plugin,
        parameters,
        &used,
        settings.enabled,
        current_values,
        patch,
    );
    compile_optional_band_switch(
        node,
        revision,
        plugin,
        parameters,
        &enabled,
        settings.enabled,
        current_values,
        patch,
    );

    match best_parameter_in(parameters, &shape) {
        MatchResult::Found(parameter, confidence) => {
            if let Some(value) = eq_shape_value(parameter, &settings.shape) {
                push_parameter_change(
                    node,
                    revision,
                    parameter,
                    "shape",
                    value,
                    (confidence + 0.08).min(1.0),
                    current_values,
                    patch,
                );
            } else {
                patch.mapping_issues.push(format!(
                    "shape {:?} has no safe label mapping for {} in {}",
                    settings.shape, parameter.name, plugin.name
                ));
            }
        }
        MatchResult::Ambiguous => patch.mapping_issues.push(format!(
            "shape has multiple plausible parameters inside the selected EQ band in {}",
            plugin.name
        )),
        MatchResult::Missing => {
            if !matches!(settings.shape, EqShape::Bell) {
                patch.mapping_issues.push(format!(
                    "shape {:?} cannot be represented safely in {}",
                    settings.shape, plugin.name
                ));
            }
        }
    }

    if settings
        .dynamic
        .as_ref()
        .is_some_and(|dynamic| dynamic.enabled)
    {
        patch.mapping_issues.push(format!(
            "dynamic EQ is requested for `{}` but dynamic-band controls are not safely mapped in {}",
            settings.band_id, plugin.name
        ));
    }
    if normalize(&settings.channel_mode) != "stereo" {
        patch.mapping_issues.push(format!(
            "channel mode `{}` is requested for `{}` but channel routing is not safely mapped in {}",
            settings.channel_mode, settings.band_id, plugin.name
        ));
    }
}

fn compile_optional_band_switch(
    node: &GraphNodeSpec,
    revision: u64,
    plugin: &PluginAssignment,
    parameters: &[&ParameterDescriptor],
    field: &Field<'_>,
    switched_on: bool,
    current_values: &BTreeMap<(String, String), f64>,
    patch: &mut CompiledParameterPatch,
) {
    match best_parameter_in(parameters, field) {
        MatchResult::Found(parameter, confidence) => {
            let value = if switched_on {
                parameter.maximum
            } else {
                parameter.minimum
            };
            push_parameter_change(
                node,
                revision,
                parameter,
                field.semantic,
                value,
                (confidence + 0.08).min(1.0),
                current_values,
                patch,
            );
        }
        MatchResult::Ambiguous => patch.mapping_issues.push(format!(
            "{} has multiple plausible parameters inside the selected EQ band in {}",
            field.semantic, plugin.name
        )),
        MatchResult::Missing => {}
    }
}

fn eq_parameter_families<'a>(
    plugin: &'a PluginAssignment,
    fields: &[&Field<'_>],
) -> BTreeMap<String, Vec<&'a ParameterDescriptor>> {
    let mut families = BTreeMap::<String, Vec<&ParameterDescriptor>>::new();
    for parameter in plugin
        .public_parameters
        .iter()
        .filter(|parameter| !parameter.read_only)
    {
        let mut matches: Vec<_> = fields
            .iter()
            .filter_map(|field| score(parameter, field).map(|score| (*field, score)))
            .collect();
        matches.sort_by(|left, right| right.1.total_cmp(&left.1));
        if let Some((field, _)) = matches.first() {
            families
                .entry(eq_family_key(parameter, field))
                .or_default()
                .push(parameter);
        }
    }
    families
}

fn eq_family_key(parameter: &ParameterDescriptor, field: &Field<'_>) -> String {
    if let Some(name_family) = name_family_key(&parameter.name, field) {
        if !name_family.is_empty() {
            return format!("name:{name_family}");
        }
    }
    if let Some(module) = parameter.module.as_deref().map(normalize) {
        if !module.is_empty() {
            return format!("module:{module}");
        }
    }
    "default".into()
}

fn name_family_key(name: &str, field: &Field<'_>) -> Option<String> {
    let name = normalize(name);
    field
        .aliases
        .iter()
        .filter_map(|alias| {
            let alias = normalize(alias);
            let start = name.find(&alias)?;
            let mut remainder = String::with_capacity(name.len().saturating_sub(alias.len()));
            remainder.push_str(&name[..start]);
            remainder.push_str(&name[start + alias.len()..]);
            Some((alias.len(), remainder))
        })
        .max_by_key(|(alias_length, _)| *alias_length)
        .map(|(_, remainder)| remainder)
}

fn family_minimum_parameter_id(parameters: &[&ParameterDescriptor]) -> u64 {
    parameters
        .iter()
        .filter_map(|parameter| parameter.stable_id.parse::<u64>().ok())
        .min()
        .unwrap_or(u64::MAX)
}

fn compile_selected_field(
    node: &GraphNodeSpec,
    revision: u64,
    plugin: &PluginAssignment,
    parameters: &[&ParameterDescriptor],
    field: &Field<'_>,
    confidence_bonus: f32,
    current_values: &BTreeMap<(String, String), f64>,
    patch: &mut CompiledParameterPatch,
) {
    match best_parameter_in(parameters, field) {
        MatchResult::Found(parameter, confidence) => {
            let value = adapt_value(parameter, field.value, field.unit);
            if !(parameter.minimum..=parameter.maximum).contains(&value) {
                patch.mapping_issues.push(format!(
                    "{} maps to {} but {:.3} is outside {:.3}…{:.3}",
                    field.semantic, parameter.name, value, parameter.minimum, parameter.maximum
                ));
                return;
            }
            push_parameter_change(
                node,
                revision,
                parameter,
                field.semantic,
                value,
                (confidence + confidence_bonus).min(1.0),
                current_values,
                patch,
            );
        }
        MatchResult::Ambiguous => patch.mapping_issues.push(format!(
            "{} has multiple plausible parameters inside the selected EQ band in {}",
            field.semantic, plugin.name
        )),
        MatchResult::Missing if field.required => patch.mapping_issues.push(format!(
            "{} is missing from the selected EQ band in {}",
            field.semantic, plugin.name
        )),
        MatchResult::Missing => {}
    }
}

fn eq_shape_value(parameter: &ParameterDescriptor, shape: &EqShape) -> Option<f64> {
    let aliases: &[&str] = match shape {
        EqShape::Bell => &["bell", "peak", "peaking"],
        EqShape::LowShelf => &["low shelf", "lowshelf"],
        EqShape::HighShelf => &["high shelf", "highshelf"],
        EqShape::LowCut => &["low cut", "lowcut", "high pass", "highpass"],
        EqShape::HighCut => &["high cut", "highcut", "low pass", "lowpass"],
        EqShape::Notch => &["notch"],
    };
    parameter.labels.iter().find_map(|(label, value)| {
        let label = normalize(label);
        aliases
            .iter()
            .any(|alias| {
                let alias = normalize(alias);
                label == alias || label.contains(&alias)
            })
            .then_some(*value)
    })
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
                push_parameter_change(
                    node,
                    revision,
                    parameter,
                    field.semantic,
                    value,
                    confidence,
                    current_values,
                    patch,
                );
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

fn push_parameter_change(
    node: &GraphNodeSpec,
    revision: u64,
    parameter: &ParameterDescriptor,
    semantic: &str,
    value: f64,
    confidence: f32,
    current_values: &BTreeMap<(String, String), f64>,
    patch: &mut CompiledParameterPatch,
) {
    if !(parameter.minimum..=parameter.maximum).contains(&value) {
        patch.mapping_issues.push(format!(
            "{} maps to {} but {:.3} is outside {:.3}…{:.3}",
            semantic, parameter.name, value, parameter.minimum, parameter.maximum
        ));
        return;
    }
    patch.parameter_changes.push(CompiledParameterChange {
        target_node_id: node.id.clone(),
        expected_graph_revision: revision,
        semantic_field: semantic.into(),
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

enum MatchResult<'a> {
    Found(&'a ParameterDescriptor, f32),
    Ambiguous,
    Missing,
}

fn best_parameter<'a>(plugin: &'a PluginAssignment, field: &Field<'_>) -> MatchResult<'a> {
    let parameters: Vec<_> = plugin
        .public_parameters
        .iter()
        .filter(|parameter| !parameter.read_only)
        .collect();
    best_parameter_in(&parameters, field)
}

fn best_parameter_in<'a>(
    parameters: &[&'a ParameterDescriptor],
    field: &Field<'_>,
) -> MatchResult<'a> {
    let mut candidates: Vec<_> = parameters
        .iter()
        .copied()
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
    use ghost_mix::{
        CompressorOperation, DynamicEqSettings, EqBandOperation, EqShape, MixOperation,
    };

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
                settings: eq_settings("low-mid", 280.0, -2.5, 1.2),
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

    #[test]
    fn repeated_eq_parameter_names_are_bound_as_coherent_physical_bands() {
        let mut state = PersistedUiState::default();
        state.graph_revision = 3;
        state.graph = EditableGraph {
            nodes: vec![GraphNodeSpec {
                id: "eq-1".into(),
                class: ProcessorClass::Equalizer,
                bypassed: false,
                plugin: Some(PluginAssignment {
                    path: PathBuf::from("multi-band.clap"),
                    plugin_id: Some("multi-band".into()),
                    name: "Multi Band EQ".into(),
                    vendor: None,
                    version: None,
                    public_parameters: vec![
                        parameter("101", "Band 1 Frequency", "Hz", 20.0, 20_000.0),
                        parameter("102", "Band 1 Gain", "dB", -24.0, 24.0),
                        parameter("103", "Band 1 Q", "", 0.1, 20.0),
                        parameter("201", "Band 2 Frequency", "Hz", 20.0, 20_000.0),
                        parameter("202", "Band 2 Gain", "dB", -24.0, 24.0),
                        parameter("203", "Band 2 Q", "", 0.1, 20.0),
                    ],
                    state: None,
                }),
            }],
        };
        let plan = MixPlan {
            schema_version: MixPlan::SCHEMA.into(),
            summary: "two bands".into(),
            confidence: 1.0,
            assumptions: Vec::new(),
            operations: vec![
                MixOperation::EqBand {
                    settings: eq_settings("mud", 280.0, -2.5, 1.2),
                },
                MixOperation::EqBand {
                    settings: eq_settings("presence", 3_200.0, 1.5, 0.9),
                },
            ],
            expected_changes: Vec::new(),
            cautions: Vec::new(),
        };
        let preview = compile_preview(plan, &state, &BTreeMap::new());
        assert!(preview.patch.can_apply(), "{:?}", preview.patch.mapping_issues);
        let ids: Vec<_> = preview
            .patch
            .parameter_changes
            .iter()
            .map(|change| change.parameter_id.as_str())
            .collect();
        assert_eq!(ids, vec!["101", "102", "103", "201", "202", "203"]);
    }

    #[test]
    fn pro_q_style_used_and_enabled_are_distinct_band_controls() {
        let mut state = PersistedUiState::default();
        state.graph = EditableGraph {
            nodes: vec![GraphNodeSpec {
                id: "eq-1".into(),
                class: ProcessorClass::Equalizer,
                bypassed: false,
                plugin: Some(PluginAssignment {
                    path: PathBuf::from("pro-q-4.clap"),
                    plugin_id: Some("pro-q-4".into()),
                    name: "Pro-Q 4".into(),
                    vendor: Some("FabFilter".into()),
                    version: None,
                    public_parameters: vec![
                        parameter("101", "Band 1 Frequency", "Hz", 20.0, 30_000.0),
                        parameter("102", "Band 1 Gain", "dB", -30.0, 30.0),
                        parameter("103", "Band 1 Q", "", 0.025, 40.0),
                        stepped_parameter("104", "Band 1 Used", 0.0, 1.0),
                        stepped_parameter("105", "Band 1 Enabled", 0.0, 1.0),
                        labelled_shape_parameter("106", "Band 1 Shape"),
                    ],
                    state: None,
                }),
            }],
        };
        let plan = MixPlan {
            schema_version: MixPlan::SCHEMA.into(),
            summary: "create a band".into(),
            confidence: 1.0,
            assumptions: Vec::new(),
            operations: vec![MixOperation::EqBand {
                settings: eq_settings("created", 250.0, -3.0, 0.7),
            }],
            expected_changes: Vec::new(),
            cautions: Vec::new(),
        };
        let preview = compile_preview(plan, &state, &BTreeMap::new());
        assert!(preview.patch.can_apply(), "{:?}", preview.patch.mapping_issues);
        assert!(preview.patch.parameter_changes.iter().any(|change| {
            change.parameter_id == "104"
                && change.semantic_field == "used"
                && change.plain_value == 1.0
        }));
        assert!(preview.patch.parameter_changes.iter().any(|change| {
            change.parameter_id == "105"
                && change.semantic_field == "enabled"
                && change.plain_value == 1.0
        }));
    }

    fn eq_settings(band_id: &str, frequency_hz: f64, gain_db: f64, q: f64) -> EqBandOperation {
        EqBandOperation {
            band_id: band_id.into(),
            enabled: true,
            shape: EqShape::Bell,
            frequency_hz,
            gain_db,
            q,
            slope_db_oct: None,
            channel_mode: "stereo".into(),
            dynamic: None::<DynamicEqSettings>,
            rationale: String::new(),
            evidence: Vec::new(),
        }
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

    fn stepped_parameter(
        stable_id: &str,
        name: &str,
        minimum: f64,
        maximum: f64,
    ) -> ParameterDescriptor {
        ParameterDescriptor {
            stable_id: stable_id.into(),
            name: name.into(),
            module: None,
            unit: None,
            minimum,
            maximum,
            default: minimum,
            stepped: true,
            read_only: false,
            labels: BTreeMap::from([("Off".into(), minimum), ("On".into(), maximum)]),
        }
    }

    fn labelled_shape_parameter(stable_id: &str, name: &str) -> ParameterDescriptor {
        ParameterDescriptor {
            stable_id: stable_id.into(),
            name: name.into(),
            module: None,
            unit: None,
            minimum: 0.0,
            maximum: 2.0,
            default: 0.0,
            stepped: true,
            read_only: false,
            labels: BTreeMap::from([
                ("Bell".into(), 0.0),
                ("Low Shelf".into(), 1.0),
                ("High Shelf".into(), 2.0),
            ]),
        }
    }
}
