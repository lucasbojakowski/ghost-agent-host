use std::path::PathBuf;

use ghost_core::audio::AudioBuffer;
use ghost_core::mock_dsp::render_mock_chain;
use ghost_core::MixPlan;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HostError {
    #[error("plugin host backend is unavailable: {0}")]
    Unavailable(String),
    #[error("plugin state error: {0}")]
    State(String),
    #[error("plugin processing error: {0}")]
    Processing(String),
    #[error("plugin scan error: {0}")]
    Scan(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescriptorRecord {
    pub id: String,
    pub name: String,
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainState {
    pub schema_version: String,
    pub pro_q_state: Vec<u8>,
    pub pro_c_state: Vec<u8>,
    pub accepted_plan: Option<MixPlan>,
}

pub trait HostedChain {
    fn backend_name(&self) -> &'static str;
    fn render(&mut self, source: &AudioBuffer, plan: &MixPlan) -> Result<AudioBuffer, HostError>;
    fn save_state(&self) -> Result<ChainState, HostError>;
    fn load_state(&mut self, state: &ChainState) -> Result<(), HostError>;
}

#[derive(Default)]
pub struct MockFabFilterChain {
    current_plan: Option<MixPlan>,
}

impl HostedChain for MockFabFilterChain {
    fn backend_name(&self) -> &'static str {
        "mock-fabfilter-chain"
    }

    fn render(&mut self, source: &AudioBuffer, plan: &MixPlan) -> Result<AudioBuffer, HostError> {
        self.current_plan = Some(plan.clone());
        Ok(render_mock_chain(source, plan))
    }

    fn save_state(&self) -> Result<ChainState, HostError> {
        let plan_bytes = serde_json::to_vec(&self.current_plan)
            .map_err(|error| HostError::State(error.to_string()))?;
        Ok(ChainState {
            schema_version: "ghost.chain-state/1".into(),
            pro_q_state: plan_bytes.clone(),
            pro_c_state: plan_bytes,
            accepted_plan: self.current_plan.clone(),
        })
    }

    fn load_state(&mut self, state: &ChainState) -> Result<(), HostError> {
        if state.schema_version != "ghost.chain-state/1" {
            return Err(HostError::State(format!(
                "unsupported chain state {}",
                state.schema_version
            )));
        }
        self.current_plan = state.accepted_plan.clone();
        Ok(())
    }
}

#[cfg(feature = "clack-runtime")]
pub mod clack_runtime {
    use std::path::Path;

    use super::*;
    use clack_host::prelude::PluginEntry;

    pub fn scan_clap_file(path: impl AsRef<Path>) -> Result<Vec<PluginDescriptorRecord>, HostError> {
        let path = path.as_ref();
        let entry = unsafe { PluginEntry::load(path) }
            .map_err(|error| HostError::Scan(error.to_string()))?;
        let factory = entry
            .get_plugin_factory()
            .ok_or_else(|| HostError::Scan("CLAP file has no plugin factory".into()))?;
        let mut records = Vec::new();
        for descriptor in factory.plugin_descriptors() {
            let id = descriptor
                .id()
                .and_then(|value| value.to_str().ok())
                .unwrap_or("unknown")
                .to_owned();
            let name = descriptor
                .name()
                .and_then(|value| value.to_str().ok())
                .unwrap_or("Unnamed CLAP plugin")
                .to_owned();
            let vendor = descriptor
                .vendor()
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let version = descriptor
                .version()
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            records.push(PluginDescriptorRecord {
                id,
                name,
                vendor,
                version,
                path: path.to_path_buf(),
            });
        }
        Ok(records)
    }
}

pub fn is_expected_fabfilter(record: &PluginDescriptorRecord) -> bool {
    let normalized = format!("{} {}", record.id, record.name).to_lowercase();
    normalized.contains("fabfilter")
        && (normalized.contains("pro-q 4")
            || normalized.contains("pro q 4")
            || normalized.contains("pro-c 3")
            || normalized.contains("pro c 3"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_state_round_trip() {
        let chain = MockFabFilterChain::default();
        let state = chain.save_state().unwrap();
        let mut restored = MockFabFilterChain::default();
        restored.load_state(&state).unwrap();
        assert_eq!(state.schema_version, "ghost.chain-state/1");
    }
}
