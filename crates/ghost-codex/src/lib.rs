use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use ghost_core::{
    CompressorOperation, DynamicEqSettings, EqBandOperation, EqShape, ExpectedChange, MixOperation,
    MixPlan, PromptBundle,
};
use schemars::schema_for;
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("failed to start Codex: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("Codex protocol error: {0}")]
    Protocol(String),
    #[error("invalid agent output: {0}")]
    InvalidOutput(#[from] serde_json::Error),
}

pub trait MixingAgent: Send {
    fn backend_name(&self) -> &'static str;
    fn propose(&mut self, bundle: &PromptBundle) -> Result<MixPlan, AgentError>;
}

#[derive(Default)]
pub struct MockMixingAgent;

impl MixingAgent for MockMixingAgent {
    fn backend_name(&self) -> &'static str {
        "mock"
    }

    fn propose(&mut self, bundle: &PromptBundle) -> Result<MixPlan, AgentError> {
        let analysis: ghost_core::AnalysisBundle =
            serde_json::from_str(&bundle.analysis_text_json)?;
        let signal = &analysis.signal;
        let mut operations = Vec::new();
        let mut expected_changes = Vec::new();
        let bands = &signal.spectrum.bands;

        if bands.low_mid_db > bands.mid_db + 4.0 {
            operations.push(MixOperation::EqBand {
                settings: EqBandOperation {
                    band_id: "agent-low-mid-control".into(),
                    enabled: true,
                    shape: EqShape::Bell,
                    frequency_hz: 260.0,
                    gain_db: -2.0,
                    q: 1.05,
                    slope_db_oct: None,
                    channel_mode: "stereo".into(),
                    dynamic: Some(DynamicEqSettings {
                        enabled: true,
                        range_db: -1.5,
                        threshold_db: None,
                    }),
                    rationale:
                        "Reduce persistent low-mid concentration without removing bass weight."
                            .into(),
                    evidence: vec![format!(
                        "low_mid_db={:.2}; mid_db={:.2}",
                        bands.low_mid_db, bands.mid_db
                    )],
                },
            });
            expected_changes.push(ExpectedChange {
                metric: "spectrum.bands.low_mid_db".into(),
                direction: "decrease".into(),
                maximum_delta: Some(4.0),
                unit: Some("dB".into()),
            });
        }

        if signal.loudness.crest_factor_db > 12.0 && signal.dynamics.transient_density_hz > 1.0 {
            operations.push(MixOperation::Compressor {
                settings: CompressorOperation {
                    enabled: true,
                    style: "clean".into(),
                    threshold_db: -18.0,
                    ratio: 2.0,
                    knee_db: 6.0,
                    attack_ms: 25.0,
                    release_ms: 140.0,
                    range_db: 3.0,
                    mix_percent: 70.0,
                    output_gain_db: 0.0,
                    rationale:
                        "Control event-to-event level variation while preserving initial attack."
                            .into(),
                    evidence: vec![format!(
                        "crest_factor_db={:.2}; transient_density_hz={:.2}",
                        signal.loudness.crest_factor_db, signal.dynamics.transient_density_hz
                    )],
                },
            });
            expected_changes.push(ExpectedChange {
                metric: "loudness.crest_factor_db".into(),
                direction: "decrease".into(),
                maximum_delta: Some(3.0),
                unit: Some("dB".into()),
            });
        }

        if let Some(resonance) = signal.spectrum.resonances.first() {
            if resonance.prominence_db > 7.0 {
                operations.push(MixOperation::EqBand {
                    settings: EqBandOperation {
                        band_id: "agent-resonance-control".into(),
                        enabled: true,
                        shape: EqShape::Bell,
                        frequency_hz: resonance.frequency_hz,
                        gain_db: -resonance.prominence_db.min(4.5) * 0.55,
                        q: (1.0 / resonance.bandwidth_octaves.max(0.08)).clamp(1.0, 12.0),
                        slope_db_oct: None,
                        channel_mode: "stereo".into(),
                        dynamic: Some(DynamicEqSettings {
                            enabled: true,
                            range_db: -2.0,
                            threshold_db: None,
                        }),
                        rationale:
                            "Control the most prominent persistent narrow-band concentration."
                                .into(),
                        evidence: vec![format!(
                            "resonance_hz={:.1}; prominence_db={:.2}",
                            resonance.frequency_hz, resonance.prominence_db
                        )],
                    },
                });
            }
        }

        Ok(MixPlan {
            schema_version: "ghost.mix-plan/1".into(),
            summary: if operations.is_empty() {
                "No conservative EQ or compression intervention was justified by the current text evidence."
                    .into()
            } else {
                "Conservative plugin-in-the-loop proposal derived from measured spectral and dynamic evidence."
                    .into()
            },
            confidence: if operations.is_empty() { 0.55 } else { 0.78 },
            assumptions: vec![
                "The captured region is representative of the requested source.".into(),
                "The mock backend approximates, but does not duplicate, FabFilter processing."
                    .into(),
            ],
            operations,
            expected_changes,
            cautions: vec!["Verify the result in context and with level-matched A/B.".into()],
        })
    }
}

pub struct CodexAppServerAgent {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    thread_id: Option<String>,
    model: String,
    pending_messages: VecDeque<Value>,
}

impl CodexAppServerAgent {
    pub fn spawn(binary: &str, model: impl Into<String>) -> Result<Self, AgentError> {
        let binary = resolve_codex_binary(binary)?;
        let mut child = Command::new(binary)
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentError::Protocol("Codex stdin unavailable".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::Protocol("Codex stdout unavailable".into()))?;
        let mut agent = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
            thread_id: None,
            model: model.into(),
            pending_messages: VecDeque::new(),
        };
        agent.initialize()?;
        Ok(agent)
    }

    fn send(&mut self, value: Value) -> Result<(), AgentError> {
        serde_json::to_writer(&mut self.stdin, &value)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, AgentError> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({ "method": method, "id": id, "params": params }))?;
        loop {
            // Requests must read fresh wire messages. Reading the pending queue here would
            // repeatedly pop and requeue the same unrelated notification forever.
            let message = self.read_wire_message()?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    return Err(AgentError::Protocol(error.to_string()));
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            self.pending_messages.push_back(message);
        }
    }

    fn read_wire_message(&mut self) -> Result<Value, AgentError> {
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line)?;
        if read == 0 {
            return Err(AgentError::Protocol("Codex closed stdout".into()));
        }
        Ok(serde_json::from_str(&line)?)
    }

    fn read_message(&mut self) -> Result<Value, AgentError> {
        if let Some(message) = self.pending_messages.pop_front() {
            return Ok(message);
        }
        self.read_wire_message()
    }

    fn initialize(&mut self) -> Result<(), AgentError> {
        self.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "ghost_agent_host",
                    "title": "Ghost Agent Host",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "optOutNotificationMethods": ["item/agentMessage/delta"]
                }
            }),
        )?;
        self.send(json!({ "method": "initialized", "params": {} }))?;
        let result = self.request("thread/start", json!({ "model": self.model.clone() }))?;
        self.thread_id = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if self.thread_id.is_none() {
            return Err(AgentError::Protocol(
                "thread/start did not return a thread ID".into(),
            ));
        }
        Ok(())
    }

    fn prompt_text(bundle: &PromptBundle) -> Result<String, AgentError> {
        Ok(format!(
            "{}\n\nUSER INTENT:\n{}\n\nANALYSIS JSON:\n{}\n\nPLUGIN CAPABILITIES:\n{}\n\nOUTPUT CONTRACT:\n{}",
            bundle.system_prompt,
            serde_json::to_string_pretty(&bundle.user_intent)?,
            bundle.analysis_text_json,
            bundle.capability_text_json,
            bundle.output_contract
        ))
    }

    fn output_schema() -> Result<Value, AgentError> {
        let mut schema = serde_json::to_value(schema_for!(MixPlan))?;
        normalize_output_schema(&mut schema);
        Ok(schema)
    }
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

fn normalize_output_schema(value: &mut Value) {
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

impl MixingAgent for CodexAppServerAgent {
    fn backend_name(&self) -> &'static str {
        "codex-app-server"
    }

    fn propose(&mut self, bundle: &PromptBundle) -> Result<MixPlan, AgentError> {
        let thread_id = self
            .thread_id
            .clone()
            .ok_or_else(|| AgentError::Protocol("Codex thread is not initialized".into()))?;
        let schema = Self::output_schema()?;
        let result = self.request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [{ "type": "text", "text": Self::prompt_text(bundle)? }],
                "model": self.model.clone(),
                "effort": "high",
                "summary": "concise",
                "approvalPolicy": "never",
                "sandboxPolicy": { "type": "readOnly", "access": { "type": "fullAccess" } },
                "outputSchema": schema
            }),
        )?;
        let turn_id = result
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::Protocol("turn/start did not return a turn ID".into()))?
            .to_owned();

        let mut final_text = None;
        let mut turn_error = None;
        loop {
            let message = self.read_message()?;
            if message.get("method").and_then(Value::as_str) == Some("error") {
                turn_error = message.pointer("/params/error").cloned();
            }
            if message.get("method").and_then(Value::as_str) == Some("item/completed") {
                let item = message
                    .pointer("/params/item")
                    .cloned()
                    .unwrap_or(Value::Null);
                if item.get("type").and_then(Value::as_str) == Some("agentMessage") {
                    final_text = item.get("text").and_then(Value::as_str).map(str::to_owned);
                }
            }
            if message.get("method").and_then(Value::as_str) == Some("turn/completed")
                && message.pointer("/params/turn/id").and_then(Value::as_str)
                    == Some(turn_id.as_str())
            {
                let status = message
                    .pointer("/params/turn/status")
                    .and_then(Value::as_str)
                    .unwrap_or("failed");
                if status != "completed" {
                    let details = message
                        .pointer("/params/turn/error")
                        .or(turn_error.as_ref())
                        .map(Value::to_string)
                        .unwrap_or_else(|| "no error details supplied".into());
                    return Err(AgentError::Protocol(format!(
                        "Codex turn ended with status {status}: {details}"
                    )));
                }
                break;
            }
        }
        let text = final_text.ok_or_else(|| {
            AgentError::Protocol("Codex completed without a final agentMessage".into())
        })?;
        Ok(serde_json::from_str(&text)?)
    }
}

impl Drop for CodexAppServerAgent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_output_schema_is_strict() {
        let schema = CodexAppServerAgent::output_schema().unwrap();
        assert_strict_objects(&schema);
        assert!(!contains_key(&schema, "$schema"));
        assert!(!contains_key(&schema, "format"));
        assert!(!contains_key(&schema, "oneOf"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn native_windows_binary_is_preferred_over_command_shims() {
        let candidates = vec![
            PathBuf::from(r"C:\tools\codex.cmd"),
            PathBuf::from(r"C:\apps\codex.exe"),
        ];
        assert_eq!(
            preferred_windows_candidate(&candidates),
            Some(PathBuf::from(r"C:\apps\codex.exe"))
        );
    }

    fn assert_strict_objects(value: &Value) {
        match value {
            Value::Object(object) => {
                if let Some(properties) = object.get("properties").and_then(Value::as_object) {
                    assert_eq!(
                        object.get("additionalProperties"),
                        Some(&Value::Bool(false))
                    );
                    let required = object
                        .get("required")
                        .and_then(Value::as_array)
                        .expect("objects with properties must list required fields");
                    assert_eq!(required.len(), properties.len());
                }
                object.values().for_each(assert_strict_objects);
            }
            Value::Array(items) => items.iter().for_each(assert_strict_objects),
            _ => {}
        }
    }

    fn contains_key(value: &Value, key: &str) -> bool {
        match value {
            Value::Object(object) => {
                object.contains_key(key) || object.values().any(|child| contains_key(child, key))
            }
            Value::Array(items) => items.iter().any(|child| contains_key(child, key)),
            _ => false,
        }
    }
}
