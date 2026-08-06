use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use ghost_core::{
    CompressorOperation, DynamicEqSettings, EqBandOperation, EqShape, ExpectedChange,
    MixOperation, MixPlan, PromptBundle,
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
                    rationale: "Reduce persistent low-mid concentration without removing bass weight."
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

        if signal.loudness.crest_factor_db > 12.0
            && signal.dynamics.transient_density_hz > 1.0
        {
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
                    rationale: "Control event-to-event level variation while preserving initial attack."
                        .into(),
                    evidence: vec![format!(
                        "crest_factor_db={:.2}; transient_density_hz={:.2}",
                        signal.loudness.crest_factor_db,
                        signal.dynamics.transient_density_hz
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
                        rationale: "Control the most prominent persistent narrow-band concentration."
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
                "The mock backend approximates, but does not duplicate, FabFilter processing.".into(),
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
        let schema = serde_json::to_value(schema_for!(MixPlan))?;
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
        loop {
            let message = self.read_message()?;
            if message.get("method").and_then(Value::as_str) == Some("item/completed") {
                let item = message.pointer("/params/item").cloned().unwrap_or(Value::Null);
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
                    return Err(AgentError::Protocol(format!(
                        "Codex turn ended with status {status}"
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
