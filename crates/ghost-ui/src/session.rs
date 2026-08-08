use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use crate::patch::{PatchApplicationState, ProposalPreview};
use crate::work::{
    AnalysisJobResult, AnalysisResult, CaptureJobResult, CapturedMaterial, ProposalJobResult,
    ScanJobResult,
};
use crate::Stage;
use ghost_host::PluginDescriptorRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginIdentity {
    pub canonical_path: PathBuf,
    pub plugin_id: String,
}

impl PluginIdentity {
    pub fn new(path: &Path, plugin_id: &str) -> Self {
        Self {
            canonical_path: std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
            plugin_id: plugin_id.to_owned(),
        }
    }

    pub fn matches(&self, plugin: &PluginDescriptorRecord) -> bool {
        self.plugin_id == plugin.id
            && self.canonical_path
                == std::fs::canonicalize(&plugin.path).unwrap_or_else(|_| plugin.path.clone())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectRuntimeSnapshot {
    pub active_graph_revision: Option<u64>,
    pub pending_graph_revision: Option<u64>,
    pub runtime_notice: String,
}

/// Non-serialized editor workflow state. One instance lives for the entire outer plugin instance,
/// while any number of transient egui windows may borrow it over time.
pub struct UiSession {
    pub(crate) capture_receiver: Option<mpsc::Receiver<CaptureJobResult>>,
    pub(crate) analysis_receiver: Option<mpsc::Receiver<AnalysisJobResult>>,
    pub(crate) proposal_receiver: Option<mpsc::Receiver<ProposalJobResult>>,
    pub(crate) scan_receiver: Option<mpsc::Receiver<ScanJobResult>>,
    pub(crate) captured: Option<CapturedMaterial>,
    pub(crate) analysis: Option<AnalysisResult>,
    pub(crate) proposal: Option<ProposalPreview>,
    pub(crate) patch_state: PatchApplicationState,
    pub(crate) last_applied_patch: Option<ghost_host::CompiledParameterPatch>,
    pub(crate) clear_patch_after_transaction: bool,
    pub(crate) pending_bypass_changes: Vec<ghost_host::CompiledBypassChange>,
    pub(crate) pending_acknowledgements: usize,
    pub(crate) parameter_feedback: BTreeMap<(String, String), f64>,
    pub(crate) plugins: Vec<PluginDescriptorRecord>,
    pub(crate) scan_errors: usize,
    pub(crate) selected_plugin: Option<PluginIdentity>,
    pub(crate) selected_node_id: Option<String>,
    pub(crate) status: String,
    pub(crate) active_stage: Option<Stage>,
    pub(crate) armed_tap: String,
    pub(crate) runtime: ProjectRuntimeSnapshot,
    pub(crate) last_transport_generation: u64,
    pub(crate) last_transport_observed: Option<Instant>,
}

impl Default for UiSession {
    fn default() -> Self {
        Self {
            capture_receiver: None,
            analysis_receiver: None,
            proposal_receiver: None,
            scan_receiver: None,
            captured: None,
            analysis: None,
            proposal: None,
            patch_state: PatchApplicationState::Preview,
            last_applied_patch: None,
            clear_patch_after_transaction: false,
            pending_bypass_changes: Vec::new(),
            pending_acknowledgements: 0,
            parameter_feedback: BTreeMap::new(),
            plugins: Vec::new(),
            scan_errors: 0,
            selected_plugin: None,
            selected_node_id: None,
            status: "Ready".into(),
            active_stage: None,
            armed_tap: "output".into(),
            runtime: ProjectRuntimeSnapshot::default(),
            last_transport_generation: 0,
            last_transport_observed: None,
        }
    }
}

impl UiSession {
    pub fn runtime_snapshot(&self) -> ProjectRuntimeSnapshot {
        self.runtime.clone()
    }

    pub fn set_runtime_notice(&mut self, notice: impl Into<String>) {
        self.runtime.runtime_notice = notice.into();
    }

    pub fn graph_committed(&mut self, revision: u64) {
        self.runtime.pending_graph_revision = Some(revision);
    }

    pub fn graph_activated(&mut self, revision: u64) {
        self.runtime.active_graph_revision = Some(revision);
        if self.runtime.pending_graph_revision == Some(revision) {
            self.runtime.pending_graph_revision = None;
        }
    }

    pub fn record_parameter_feedback(&mut self, node_id: String, parameter_id: String, value: f64) {
        self.parameter_feedback
            .insert((node_id, parameter_id), value);
        let Some(patch) = self.last_applied_patch.as_ref() else {
            return;
        };
        let all_confirmed = patch.parameter_changes.iter().all(|change| {
            self.parameter_feedback
                .get(&(change.target_node_id.clone(), change.parameter_id.clone()))
                .is_some_and(|observed| {
                    let tolerance = (change.maximum - change.minimum).abs() * 1.0e-6 + 1.0e-9;
                    (*observed - change.plain_value).abs() <= tolerance
                })
        });
        if all_confirmed
            && matches!(
                self.patch_state,
                PatchApplicationState::Applied { .. } | PatchApplicationState::Queued { .. }
            )
        {
            self.patch_state = PatchApplicationState::Verified {
                transaction_id: patch.transaction_id,
            };
            self.status = "Parameter transaction verified by child feedback".into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_and_pending_revisions_are_independent() {
        let mut session = UiSession::default();
        session.graph_activated(2);
        session.graph_committed(3);
        assert_eq!(session.runtime.active_graph_revision, Some(2));
        assert_eq!(session.runtime.pending_graph_revision, Some(3));
        session.graph_activated(3);
        assert_eq!(session.runtime.active_graph_revision, Some(3));
        assert_eq!(session.runtime.pending_graph_revision, None);
    }
}
