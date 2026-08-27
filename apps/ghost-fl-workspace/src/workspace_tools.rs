use std::sync::{Arc, Mutex};

use anyhow::Result;
use ghost_codex::{ToolDefinition, ToolError, ToolRegistry};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::audio_tools::{
    AnalyzeAudioRequest, AudioToolState, CompareAudioRequest, ReadAudioRequest, TapArmRequest,
    TapCollectRequest,
};
use crate::project::{PlanUpdate, ProjectUpdate, WorkspaceProjectHub};
use crate::skills;

pub(crate) const WORKSPACE_TOOL_NAMES: [&str; 12] = [
    "ghost_audio_analyze",
    "ghost_audio_read",
    "ghost_audio_compare",
    "ghost_tap_list",
    "ghost_tap_arm",
    "ghost_tap_collect",
    "workspace_project_get",
    "workspace_project_set",
    "workspace_plan_get",
    "workspace_plan_set",
    "workspace_skill_list",
    "workspace_skill_read",
];

#[derive(Clone)]
pub(crate) struct WorkspaceToolState {
    pub project: Arc<Mutex<WorkspaceProjectHub>>,
    pub audio: AudioToolState,
}

impl WorkspaceToolState {
    pub(crate) fn new(project: Arc<Mutex<WorkspaceProjectHub>>) -> Result<Self> {
        let audio = AudioToolState::new(Arc::clone(&project))?;
        Ok(Self { project, audio })
    }
}

#[derive(Debug, Deserialize)]
struct SkillReadRequest {
    name: String,
}

pub(crate) fn register_workspace_tools(
    registry: &mut ToolRegistry,
    state: WorkspaceToolState,
) -> Result<()> {
    register_audio_tools(registry, &state)?;
    register_project_tools(registry, &state)?;
    register_skill_tools(registry)?;
    Ok(())
}

fn register_audio_tools(registry: &mut ToolRegistry, state: &WorkspaceToolState) -> Result<()> {
    let audio = state.audio.clone();
    registry.register(
        ToolDefinition {
            name: "ghost_audio_analyze".into(),
            description: "Decode and deterministically analyze a local audio file with Ghost's Rust pipeline. Returns a compact summary plus an analysisId; use ghost_audio_read for progressive acoustic, timeline, rhythm, pitch, section or DAW-grid evidence. The file is also registered as a workspace project asset.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Local filesystem audio path"},
                    "label": {"type": "string", "description": "Human-facing asset name"},
                    "role": {"type": "string", "description": "Semantic stem role such as reference_mix, drums, kick, snare, hihat, bass, music, vocals, fx or other"},
                    "tempoBpm": {"type": "number", "description": "Optional producer/DAW tempo used to project onsets and notes onto bars/beats"},
                    "force": {"type": "boolean", "description": "Recompute even when this registered asset already has a cached analysis"}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let request: AnalyzeAudioRequest = serde_json::from_value(arguments)
                .map_err(|error| ToolError(format!("invalid ghost_audio_analyze arguments: {error}")))?;
            audio.analyze(request).map_err(tool_error)
        },
    )?;

    let audio = state.audio.clone();
    registry.register(
        ToolDefinition {
            name: "ghost_audio_read".into(),
            description: "Read one progressive view from a cached Ghost audio analysis. Prefer summary first, then request only the evidence needed: acoustic, timeline, rhythm, pitch, sections, or dawProjection.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "analysisId": {"type": "string"},
                    "view": {"type": "string", "enum": ["summary", "acoustic", "timeline", "rhythm", "pitch", "sections", "dawProjection"]}
                },
                "required": ["analysisId"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let request: ReadAudioRequest = serde_json::from_value(arguments)
                .map_err(|error| ToolError(format!("invalid ghost_audio_read arguments: {error}")))?;
            audio.read(request).map_err(tool_error)
        },
    )?;

    let audio = state.audio.clone();
    registry.register(
        ToolDefinition {
            name: "ghost_audio_compare".into(),
            description: "Compare two cached analyses and return meaningful right-minus-left deltas for loudness, spectral, dynamics, stereo and band-energy evidence. Use this for stem-to-reference relationships instead of manually subtracting large payloads.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "leftAnalysisId": {"type": "string"},
                    "rightAnalysisId": {"type": "string"}
                },
                "required": ["leftAnalysisId", "rightAnalysisId"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let request: CompareAudioRequest = serde_json::from_value(arguments)
                .map_err(|error| ToolError(format!("invalid ghost_audio_compare arguments: {error}")))?;
            audio.compare(request).map_err(tool_error)
        },
    )?;

    let audio = state.audio.clone();
    registry.register(
        ToolDefinition {
            name: "ghost_tap_list".into(),
            description: "List fresh Ghost Tap instances reported by the local control plane. This does not prove which FL mixer insert/slot a Tap belongs to; use the fl-audio-capture skill and inspect FL before choosing an instance.".into(),
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
        move |_| audio.list_taps().map_err(tool_error),
    )?;

    let audio = state.audio.clone();
    registry.register(
        ToolDefinition {
            name: "ghost_tap_arm".into(),
            description: "Arm one verified Ghost Tap for a bounded capture. Call this only after mixer insert/slot and transport start position are established. It returns a requestId; start FL playback after arming, then collect that exact request.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "instanceId": {"type": "integer", "minimum": 0},
                    "durationSeconds": {"type": "number", "minimum": 0.05, "maximum": 20.0}
                },
                "required": ["instanceId", "durationSeconds"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let request: TapArmRequest = serde_json::from_value(arguments)
                .map_err(|error| ToolError(format!("invalid ghost_tap_arm arguments: {error}")))?;
            audio.arm_tap(request).map_err(tool_error)
        },
    )?;

    let audio = state.audio.clone();
    registry.register(
        ToolDefinition {
            name: "ghost_tap_collect".into(),
            description: "Collect a previously armed Ghost Tap request after FL playback has started. Returns the exact capture artifact including WAV path and transport provenance. Do not use this as the playback-start step.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "instanceId": {"type": "integer", "minimum": 0},
                    "requestId": {"type": "integer", "minimum": 1},
                    "timeoutSeconds": {"type": "number", "minimum": 0.1, "maximum": 120.0}
                },
                "required": ["instanceId", "requestId"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let request: TapCollectRequest = serde_json::from_value(arguments)
                .map_err(|error| ToolError(format!("invalid ghost_tap_collect arguments: {error}")))?;
            audio.collect_tap(request).map_err(tool_error)
        },
    )?;

    Ok(())
}

fn register_project_tools(registry: &mut ToolRegistry, state: &WorkspaceToolState) -> Result<()> {
    let project = Arc::clone(&state.project);
    registry.register(
        ToolDefinition {
            name: "workspace_project_get".into(),
            description: "Read the selected thread's persistent production project context: title, producer description, tempo/time signature, registered reference/stem assets and current Production Plan.".into(),
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
        move |_| {
            let project = project
                .lock()
                .map_err(|_| ToolError("workspace project lock poisoned".into()))?
                .current()
                .map_err(tool_error)?;
            serde_json::to_value(project).map_err(|error| ToolError(error.to_string()))
        },
    )?;

    let project = Arc::clone(&state.project);
    registry.register(
        ToolDefinition {
            name: "workspace_project_set".into(),
            description: "Update semantic production-project metadata for the selected thread. This changes Ghost workspace intent/context only; it does not mutate FL Studio.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "description": {"type": "string"},
                    "tempoBpm": {"anyOf": [{"type": "number"}, {"type": "null"}]},
                    "timeSignature": {"type": "string"}
                },
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let update: ProjectUpdate = serde_json::from_value(arguments)
                .map_err(|error| ToolError(format!("invalid workspace_project_set arguments: {error}")))?;
            let project = project
                .lock()
                .map_err(|_| ToolError("workspace project lock poisoned".into()))?
                .update(update)
                .map_err(tool_error)?;
            serde_json::to_value(project).map_err(|error| ToolError(error.to_string()))
        },
    )?;

    let project = Arc::clone(&state.project);
    registry.register(
        ToolDefinition {
            name: "workspace_plan_get".into(),
            description: "Read the current structured Production Plan semantic artifact. The plan is intent, not proof of live FL state.".into(),
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
        move |_| {
            let plan = project
                .lock()
                .map_err(|_| ToolError("workspace project lock poisoned".into()))?
                .current()
                .map_err(tool_error)?
                .production_plan;
            Ok(plan)
        },
    )?;

    let project = Arc::clone(&state.project);
    registry.register(
        ToolDefinition {
            name: "workspace_plan_set".into(),
            description: "Replace the selected thread's structured Production Plan. Use it for agreed channels, playlist tracks, mixer inserts, sections, markers, timbres and next steps before applying those intentions to FL.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"plan": {"type": "object"}},
                "required": ["plan"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let request: PlanUpdate = serde_json::from_value(arguments)
                .map_err(|error| ToolError(format!("invalid workspace_plan_set arguments: {error}")))?;
            let project = project
                .lock()
                .map_err(|_| ToolError("workspace project lock poisoned".into()))?
                .set_plan(request.plan)
                .map_err(tool_error)?;
            Ok(project.production_plan)
        },
    )?;

    Ok(())
}

fn register_skill_tools(registry: &mut ToolRegistry) -> Result<()> {
    registry.register(
        ToolDefinition {
            name: "workspace_skill_list".into(),
            description: "List the app-bundled production skills exposed to this workspace.".into(),
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
        |_| serde_json::to_value(skills::list_skills()).map_err(|error| ToolError(error.to_string())),
    )?;

    registry.register(
        ToolDefinition {
            name: "workspace_skill_read".into(),
            description: "Load one bundled workspace skill by name. Read a relevant skill before executing its workflow.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"name": {"type": "string"}},
                "required": ["name"],
                "additionalProperties": false
            }),
        },
        |arguments| {
            let request: SkillReadRequest = serde_json::from_value(arguments)
                .map_err(|error| ToolError(format!("invalid workspace_skill_read arguments: {error}")))?;
            skills::validate_skill_name(&request.name).map_err(tool_error)?;
            skills::read_skill(&request.name).map_err(tool_error)
        },
    )?;

    Ok(())
}

fn tool_error(error: impl std::fmt::Display) -> ToolError {
    ToolError(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_tool_surface_is_bounded() {
        assert_eq!(WORKSPACE_TOOL_NAMES.len(), 12);
        let unique = WORKSPACE_TOOL_NAMES
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), WORKSPACE_TOOL_NAMES.len());
    }
}
