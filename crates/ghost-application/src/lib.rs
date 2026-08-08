//! UI-, transport-, storage-, and agent-neutral application boundary.

use std::path::Path;

use ghost_context::CompiledContext;
use ghost_core::{analyze_audio, read_audio, AnalysisBundle, AnalysisConfig};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("media input failed: {0}")]
    Media(String),
    #[error("analysis failed: {0}")]
    Analysis(String),
    #[error("agent failed: {0}")]
    Agent(String),
    #[error("repository failed: {0}")]
    Repository(String),
    #[error("render failed: {0}")]
    Render(String),
}

pub trait AgentPort: Send {
    fn backend_name(&self) -> &str;
    fn execute(&mut self, context: &CompiledContext) -> Result<Value, ApplicationError>;
}

pub trait RepositoryPort: Send + Sync {
    fn store_artifact(&self, kind: &str, value: &Value) -> Result<String, ApplicationError>;
}

pub trait RenderPort: Send {
    fn apply(&mut self, plan: &Value) -> Result<Value, ApplicationError>;
}

pub trait ProgressPort: Send + Sync {
    fn report(&self, phase: &str, fraction: Option<f32>, message: &str);
    fn cancelled(&self) -> bool;
}

#[derive(Default)]
pub struct NoProgress;

impl ProgressPort for NoProgress {
    fn report(&self, _phase: &str, _fraction: Option<f32>, _message: &str) {}
    fn cancelled(&self) -> bool {
        false
    }
}

/// Deterministic analysis use case shared by CLI, daemon, and in-process frontends.
pub fn analyze_path(
    path: impl AsRef<Path>,
    config: &AnalysisConfig,
    progress: &dyn ProgressPort,
) -> Result<AnalysisBundle, ApplicationError> {
    progress.report("decode", Some(0.0), "Decoding media");
    let audio = read_audio(&path).map_err(|error| ApplicationError::Media(error.to_string()))?;
    if progress.cancelled() {
        return Err(ApplicationError::Analysis("request cancelled".into()));
    }
    progress.report("analysis", Some(0.25), "Extracting audio features");
    let result = analyze_audio(path.as_ref().display().to_string(), &audio, config)
        .map_err(|error| ApplicationError::Analysis(error.to_string()))?;
    progress.report("analysis", Some(1.0), "Analysis complete");
    Ok(result)
}

pub fn execute_context(
    agent: &mut dyn AgentPort,
    context: &CompiledContext,
    progress: &dyn ProgressPort,
) -> Result<Value, ApplicationError> {
    if progress.cancelled() {
        return Err(ApplicationError::Agent("request cancelled".into()));
    }
    progress.report("agent", Some(0.0), "Starting agent turn");
    let result = agent.execute(context)?;
    progress.report("agent", Some(1.0), "Agent turn complete");
    Ok(result)
}
