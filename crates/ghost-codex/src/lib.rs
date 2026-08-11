use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::process::Command;

use serde_json::Value;
use thiserror::Error;

mod app_server;
mod parallel;
mod runtime;
mod tools;
mod transport;

pub use app_server::*;
pub use parallel::*;
pub use runtime::*;
pub use tools::*;
pub use transport::*;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("failed to start Codex: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("Codex protocol error: {0}")]
    Protocol(String),
    #[error("invalid agent output: {0}")]
    InvalidOutput(#[from] serde_json::Error),
}

pub fn resolve_codex_binary(binary: &str) -> Result<PathBuf, AgentError> {
    let requested = Path::new(binary);
    if requested.is_absolute() || requested.components().count() > 1 {
        return requested
            .is_file()
            .then(|| requested.to_path_buf())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Codex binary does not exist: {}", requested.display()),
                )
                .into()
            });
    }

    #[cfg(target_os = "windows")]
    {
        let result = Command::new("where.exe").arg(binary).output()?;
        if result.status.success() {
            let candidates: Vec<PathBuf> = String::from_utf8_lossy(&result.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
                .collect();
            if let Some(candidate) = preferred_windows_candidate(&candidates) {
                return Ok(candidate);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Codex binary '{binary}' was not found on PATH"),
        )
        .into())
    }

    #[cfg(not(target_os = "windows"))]
    Ok(requested.to_path_buf())
}

#[cfg(target_os = "windows")]
fn preferred_windows_candidate(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        })
        .or_else(|| candidates.first())
        .cloned()
}

pub(crate) fn normalize_output_schema(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("$schema");
            object.remove("format");
            if let Some(variants) = object.remove("oneOf") {
                object.insert("anyOf".into(), variants);
            }
            for child in object.values_mut() {
                normalize_output_schema(child);
            }
            if let Some(properties) = object.get("properties").and_then(Value::as_object) {
                let required = properties.keys().cloned().map(Value::String).collect();
                object.insert("required".into(), Value::Array(required));
                object.insert("additionalProperties".into(), Value::Bool(false));
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_output_schema(item);
            }
        }
        _ => {}
    }
}
