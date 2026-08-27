use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::process::Command;

use serde_json::Value;
use thiserror::Error;

mod parallel;
mod runtime;
mod tools;
mod transport;

pub use parallel::*;
pub use runtime::*;
pub use tools::*;

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
    // `where.exe` returns candidates in PATH precedence order. Keep that order instead of
    // preferring an arbitrary later `.exe`: npm installs expose an extensionless launcher and a
    // sibling `.cmd`, while editor extensions may append their own bundled `codex.exe` to PATH.
    // `transport::windows_command_shim` resolves the sibling `.cmd` when the first candidate is
    // the npm launcher.
    candidates.first().cloned()
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn codex_resolution_preserves_path_precedence() {
        let candidates = vec![
            PathBuf::from(r"C:\npm\codex"),
            PathBuf::from(r"C:\npm\codex.cmd"),
            PathBuf::from(r"C:\editor\codex.exe"),
        ];

        assert_eq!(
            preferred_windows_candidate(&candidates),
            Some(PathBuf::from(r"C:\npm\codex"))
        );
    }

    #[test]
    fn codex_resolution_accepts_a_direct_executable() {
        let candidates = vec![PathBuf::from(r"C:\tools\codex.exe")];

        assert_eq!(
            preferred_windows_candidate(&candidates),
            Some(PathBuf::from(r"C:\tools\codex.exe"))
        );
    }
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
