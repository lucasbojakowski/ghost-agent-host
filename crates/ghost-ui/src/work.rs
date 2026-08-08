use std::collections::BTreeMap;
use std::path::PathBuf;

use ghost_codex::{MixingAgent, MockMixingAgent};
use ghost_core::{
    analyze_audio, read_audio, AnalysisBundle, AnalysisConfig, AudioBuffer, UserIntent,
};
use ghost_host::{EditableGraph, PluginDescriptorRecord};
use ghost_mix::{build_prompt_bundle, validate_mix_plan, MixOperation, PluginCapabilitySummary};

use crate::patch::{compile_preview, ProposalPreview};
use crate::state::PersistedUiState;

pub(crate) enum CaptureJobResult {
    File(Result<CapturedMaterial, String>),
}

pub(crate) enum AnalysisJobResult {
    Complete(Result<AnalysisResult, String>),
}

pub(crate) enum ProposalJobResult {
    Complete(Result<ProposalPreview, String>),
}

pub(crate) enum ScanJobResult {
    Complete(Vec<PluginDescriptorRecord>, Vec<(PathBuf, String)>),
}

#[derive(Clone)]
pub(crate) struct CapturedMaterial {
    pub input: AudioBuffer,
    pub output: AudioBuffer,
    pub label: String,
    pub output_tap_id: String,
}

impl CapturedMaterial {
    pub fn tap(&self, tap: &str) -> Option<&AudioBuffer> {
        match tap {
            "input" => Some(&self.input),
            tap if tap == self.output_tap_id => Some(&self.output),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct AnalysisResult {
    pub bundle: AnalysisBundle,
    pub source_label: String,
    pub tap_label: String,
}

pub(crate) fn capture_file(path: String) -> CaptureJobResult {
    let result = read_audio(&path)
        .map(|audio| CapturedMaterial {
            output: audio.clone(),
            input: audio,
            label: path,
            output_tap_id: "output".into(),
        })
        .map_err(|error| error.to_string());
    CaptureJobResult::File(result)
}

pub(crate) fn analyze(
    material: CapturedMaterial,
    tap_id: String,
    tap_label: String,
    profile: usize,
) -> AnalysisJobResult {
    let result = (|| {
        let audio = material.tap(&tap_id).ok_or_else(|| {
            "This graph-edge tap needs an active native child graph; capture Input or Output for now."
                .to_owned()
        })?;
        let config = match profile {
            0 => AnalysisConfig::live(),
            1 => AnalysisConfig::high(),
            _ => AnalysisConfig::maximum(),
        };
        let bundle =
            analyze_audio(&material.label, audio, &config).map_err(|error| error.to_string())?;
        Ok(AnalysisResult {
            bundle,
            source_label: material.label,
            tap_label,
        })
    })();
    AnalysisJobResult::Complete(result)
}

pub(crate) fn propose(
    state: PersistedUiState,
    analysis: AnalysisResult,
    parameter_feedback: BTreeMap<(String, String), f64>,
) -> ProposalJobResult {
    ProposalJobResult::Complete((|| {
        let capabilities = capability_summaries(&state.graph);
        if capabilities.is_empty() {
            return Err(
                "Create at least one Equalizer or Compressor node before proposing.".into(),
            );
        }
        let bundle = build_prompt_bundle(
            include_str!("../../../prompts/system.md"),
            UserIntent::Freeform {
                prompt: state.prompt.clone(),
            },
            &analysis.bundle,
            &capabilities,
        )
        .map_err(|error| error.to_string())?;
        let mut agent = MockMixingAgent;
        let mut plan = agent.propose(&bundle).map_err(|error| error.to_string())?;
        let allow_equalizer = state
            .graph
            .nodes
            .iter()
            .any(|node| node.class == ghost_host::ProcessorClass::Equalizer);
        let allow_compressor = state
            .graph
            .nodes
            .iter()
            .any(|node| node.class == ghost_host::ProcessorClass::Compressor);
        plan.operations.retain(|operation| match operation {
            MixOperation::EqBand { .. } => allow_equalizer,
            MixOperation::Compressor { .. } => allow_compressor,
            MixOperation::Bypass { target, .. } => match target {
                ghost_mix::ProcessorRole::Equalizer => allow_equalizer,
                ghost_mix::ProcessorRole::Compressor => allow_compressor,
            },
        });
        if plan.operations.is_empty() {
            plan.summary =
                "No intervention for the available processor classes was justified.".into();
        }
        validate_mix_plan(&plan).map_err(|error| error.to_string())?;
        Ok(compile_preview(plan, &state, &parameter_feedback))
    })())
}

pub(crate) fn scan_plugins() -> ScanJobResult {
    let roots = ghost_host::default_clap_directories();
    let files = ghost_host::discover_clap_files(&roots);
    let mut records = Vec::new();
    let mut errors = Vec::new();
    for path in files {
        match ghost_host::inspect_clap_file(&path) {
            Ok(mut found) => records.append(&mut found),
            Err(error) => errors.push((path, error.to_string())),
        }
    }
    records.sort_by(|left, right| left.name.cmp(&right.name));
    ScanJobResult::Complete(records, errors)
}

fn capability_summaries(graph: &EditableGraph) -> Vec<PluginCapabilitySummary> {
    graph
        .nodes
        .iter()
        .filter(|node| node.class.context_available())
        .map(|node| PluginCapabilitySummary {
            plugin: node.plugin.as_ref().map_or_else(
                || format!("Unassigned {} node", node.class.label()),
                |item| item.name.clone(),
            ),
            version: node
                .plugin
                .as_ref()
                .and_then(|item| item.version.clone())
                .unwrap_or_else(|| "public-interface-pending".into()),
            supported_operations: vec![node.class.capability_kind().into()],
            safety_notes: vec![
                "Changes remain staged until accepted".into(),
                if node.plugin.is_some() {
                    "Map only against scanned public parameters".into()
                } else {
                    "No child is assigned; proposal is semantic only".into()
                },
            ],
            public_parameters: node
                .plugin
                .as_ref()
                .map_or_else(Vec::new, |item| item.public_parameters.clone()),
        })
        .collect()
}
