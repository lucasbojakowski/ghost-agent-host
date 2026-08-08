mod patch;
mod session;
mod state;
mod views;
mod widgets;
mod work;

use std::sync::{mpsc, Arc, Mutex};

use ghost_core::{
    capture_tap_key, AtomicDawState, AtomicGraphControl, DawTransportSnapshot,
    RealtimeCaptureBuffer, RealtimeCaptureState,
};
use ghost_host::PluginAssignment;
use widgets::accent;
use work::{
    AnalysisJobResult, CaptureJobResult, CapturedMaterial, ProposalJobResult, ScanJobResult,
};

pub use patch::{PatchApplicationState, ProposalPreview};
pub use session::{PluginIdentity, ProjectRuntimeSnapshot, UiSession};
pub use state::{CaptureSource, PersistedUiState, ProjectDocument};

pub const DEFAULT_EDITOR_WIDTH: u32 = 1180;
pub const DEFAULT_EDITOR_HEIGHT: u32 = 760;

pub trait HostControl: Send + Sync {
    fn request_graph_restart(&self) {}
    fn request_process(&self) {}
    fn mark_project_dirty(&self) {}
    fn queue_parameter_patch(
        &self,
        _patch: &ghost_host::CompiledParameterPatch,
    ) -> Result<(), String> {
        Err("Parameter application is unavailable outside an active plugin host".into())
    }
    fn drain_parameter_acknowledgements(&self, _output: &mut Vec<ghost_host::ParameterAck>) {}
    fn parameter_transaction_complete(&self, _transaction_id: u64) {}
    fn show_child_gui(&self, _node_id: &str) {}
    fn hide_child_gui(&self, _node_id: &str) {}
}

struct NoHostControl;
impl HostControl for NoHostControl {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Capture,
    Analyze,
    Propose,
}

pub struct GhostUi {
    persisted: Arc<Mutex<PersistedUiState>>,
    session: Arc<Mutex<UiSession>>,
    daw: Arc<AtomicDawState>,
    capture: Arc<RealtimeCaptureBuffer>,
    graph_control: Arc<AtomicGraphControl>,
    host_control: Arc<dyn HostControl>,
    graph_commit_requested: bool,
    document_dirty_requested: bool,
}

impl Default for GhostUi {
    fn default() -> Self {
        Self::with_persisted(Arc::new(Mutex::new(PersistedUiState::default())))
    }
}

impl GhostUi {
    pub fn with_persisted(persisted: Arc<Mutex<PersistedUiState>>) -> Self {
        Self::with_runtime(
            persisted,
            Arc::new(AtomicDawState::default()),
            Arc::new(RealtimeCaptureBuffer::new(288_000)),
            Arc::new(AtomicGraphControl::default()),
            Arc::new(NoHostControl),
        )
    }

    pub fn with_runtime(
        persisted: Arc<Mutex<PersistedUiState>>,
        daw: Arc<AtomicDawState>,
        capture: Arc<RealtimeCaptureBuffer>,
        graph_control: Arc<AtomicGraphControl>,
        host_control: Arc<dyn HostControl>,
    ) -> Self {
        Self::with_runtime_and_session(
            persisted,
            Arc::new(Mutex::new(UiSession::default())),
            daw,
            capture,
            graph_control,
            host_control,
        )
    }

    pub fn with_runtime_and_session(
        persisted: Arc<Mutex<PersistedUiState>>,
        session: Arc<Mutex<UiSession>>,
        daw: Arc<AtomicDawState>,
        capture: Arc<RealtimeCaptureBuffer>,
        graph_control: Arc<AtomicGraphControl>,
        host_control: Arc<dyn HostControl>,
    ) -> Self {
        Self {
            persisted,
            session,
            daw,
            capture,
            graph_control,
            host_control,
            graph_commit_requested: false,
            document_dirty_requested: false,
        }
    }

    pub fn persisted(&self) -> Arc<Mutex<PersistedUiState>> {
        Arc::clone(&self.persisted)
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.poll(ui.ctx());
        widgets::configure_style(ui.ctx());
        self.graph_commit_requested = false;
        self.document_dirty_requested = false;
        let project = Arc::clone(&self.persisted);
        let session = Arc::clone(&self.session);
        let mut committed_revision = None;
        if let (Ok(mut state), Ok(mut session)) = (project.lock(), session.lock()) {
            self.poll_parameter_acknowledgements(&mut state, &mut session);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(8, 12, 18)))
                .show(ui, |ui| {
                    self.top_bar(ui, &mut state, &session);
                    ui.separator();
                    match state.selected_view {
                        1 => self.analyzer_view(ui, &mut state, &mut session),
                        2 => self.routing_view(ui, &mut state, &mut session),
                        _ => self.workflow_view(ui, &mut state, &mut session),
                    }
                });
            let bypass_mask = state
                .graph
                .nodes
                .iter()
                .take(64)
                .enumerate()
                .fold(0_u64, |mask, (index, node)| {
                    mask | ((node.bypassed as u64) << index)
                });
            self.graph_control.set_bypass_mask(bypass_mask);
            if self.graph_commit_requested {
                let revision = state.commit_graph();
                session.graph_committed(revision);
                committed_revision = Some(revision);
            }
        }
        // The project mutation and its pending revision are visible before the DAW can reactivate.
        if committed_revision.is_some() {
            self.host_control.mark_project_dirty();
            self.host_control.request_graph_restart();
        } else if self.document_dirty_requested {
            self.host_control.mark_project_dirty();
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(40));
    }

    pub fn show_contents(&mut self, ui: &mut egui::Ui) {
        self.show(ui);
    }

    fn poll(&mut self, context: &egui::Context) {
        let session_handle = Arc::clone(&self.session);
        let Ok(mut session) = session_handle.lock() else {
            return;
        };
        self.poll_capture(&mut session);
        self.poll_analysis(&mut session);
        self.poll_proposal(&mut session);
        self.poll_scan(&mut session);
        let generation = self.daw.transport_generation();
        if generation != session.last_transport_generation {
            session.last_transport_generation = generation;
            session.last_transport_observed = Some(std::time::Instant::now());
        }
        if self.capture.state() == RealtimeCaptureState::Complete {
            if let Some(snapshot) = self.capture.snapshot(self.daw.transport()) {
                let seconds = snapshot.input.duration_seconds();
                let actual_tap = if snapshot.tap_key == capture_tap_key(&session.armed_tap) {
                    session.armed_tap.clone()
                } else {
                    "output".into()
                };
                session.captured = Some(CapturedMaterial {
                    input: snapshot.input,
                    output: snapshot.output,
                    label: format!("DAW capture · {seconds:.2} s"),
                    output_tap_id: actual_tap.clone(),
                });
                session.analysis = None;
                session.proposal = None;
                session.status = if actual_tap == session.armed_tap {
                    format!("Captured {seconds:.2} s from DAW")
                } else {
                    format!("Requested tap was inactive; captured Output for {seconds:.2} s")
                };
                session.active_stage = None;
                self.capture.cancel();
            }
        }
        if self.capture.state() == RealtimeCaptureState::Recording
            || session.capture_receiver.is_some()
            || session.analysis_receiver.is_some()
            || session.proposal_receiver.is_some()
            || session.scan_receiver.is_some()
        {
            context.request_repaint();
        }
    }

    fn poll_parameter_acknowledgements(
        &self,
        state: &mut PersistedUiState,
        session: &mut UiSession,
    ) {
        let mut acknowledgements = Vec::new();
        self.host_control
            .drain_parameter_acknowledgements(&mut acknowledgements);
        for acknowledgement in acknowledgements {
            let PatchApplicationState::Queued { transaction_id } = session.patch_state else {
                continue;
            };
            if acknowledgement.transaction_id != transaction_id {
                continue;
            }
            if acknowledgement.status != ghost_host::ParameterAckStatus::Applied {
                session.patch_state = PatchApplicationState::Failed {
                    transaction_id,
                    reason: format!(
                        "{}:{} was rejected ({:?})",
                        acknowledgement.node_id,
                        acknowledgement.parameter_id,
                        acknowledgement.status
                    ),
                };
                session.pending_acknowledgements = 0;
                session.pending_bypass_changes.clear();
                if !session.clear_patch_after_transaction {
                    session.last_applied_patch = None;
                }
                session.clear_patch_after_transaction = false;
                session.status = "Parameter transaction failed".into();
                continue;
            }
            if let (Some(previous_value), Some(patch)) = (
                acknowledgement.previous_value,
                session.last_applied_patch.as_mut(),
            ) {
                if let Some(change) = patch.parameter_changes.iter_mut().find(|change| {
                    change.target_node_id == acknowledgement.node_id
                        && change.parameter_id == acknowledgement.parameter_id
                }) {
                    change.previous_value = previous_value;
                }
            }
            session.pending_acknowledgements = session.pending_acknowledgements.saturating_sub(1);
            if session.pending_acknowledgements == 0 {
                for change in &session.pending_bypass_changes {
                    if let Some(node) = state
                        .graph
                        .nodes
                        .iter_mut()
                        .find(|node| node.id == change.target_node_id)
                    {
                        node.bypassed = change.bypassed;
                    }
                }
                session.pending_bypass_changes.clear();
                session.patch_state = PatchApplicationState::Applied { transaction_id };
                session.status = "Parameter transaction applied; awaiting child feedback".into();
                self.host_control
                    .parameter_transaction_complete(transaction_id);
                if session.clear_patch_after_transaction {
                    session.last_applied_patch = None;
                    session.clear_patch_after_transaction = false;
                }
            }
        }
    }

    fn poll_capture(&mut self, session: &mut UiSession) {
        let message = session
            .capture_receiver
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(CaptureJobResult::File(result)) = message {
            session.capture_receiver = None;
            session.active_stage = None;
            match result {
                Ok(material) => {
                    session.status = format!("Loaded {} frames", material.input.frames());
                    session.captured = Some(material);
                    session.analysis = None;
                    session.proposal = None;
                }
                Err(error) => session.status = format!("Capture failed: {error}"),
            }
        }
    }

    fn poll_analysis(&mut self, session: &mut UiSession) {
        let message = session
            .analysis_receiver
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(AnalysisJobResult::Complete(result)) = message {
            session.analysis_receiver = None;
            session.active_stage = None;
            match result {
                Ok(result) => {
                    session.status = "Analysis ready".into();
                    session.analysis = Some(result);
                    session.proposal = None;
                }
                Err(error) => session.status = format!("Analysis failed: {error}"),
            }
        }
    }

    fn poll_proposal(&mut self, session: &mut UiSession) {
        let message = session
            .proposal_receiver
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(ProposalJobResult::Complete(result)) = message {
            session.proposal_receiver = None;
            session.active_stage = None;
            match result {
                Ok(plan) => {
                    session.status = "Proposal preview ready".into();
                    session.patch_state = if plan.patch.can_apply() {
                        PatchApplicationState::Preview
                    } else {
                        PatchApplicationState::MappingIncomplete
                    };
                    session.proposal = Some(plan);
                }
                Err(error) => session.status = format!("Proposal failed: {error}"),
            }
        }
    }

    fn poll_scan(&mut self, session: &mut UiSession) {
        let message = session
            .scan_receiver
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(ScanJobResult::Complete(records, errors)) = message {
            session.scan_receiver = None;
            session.scan_errors = errors.len();
            session.status = format!("Found {} CLAP processors", records.len());
            session.plugins = records;
            if session.selected_plugin.as_ref().is_some_and(|selected| {
                !session
                    .plugins
                    .iter()
                    .any(|plugin| selected.matches(plugin))
            }) {
                session.selected_plugin = None;
            }
        }
    }

    fn top_bar(&self, ui: &mut egui::Ui, state: &mut PersistedUiState, session: &UiSession) {
        let transport = self.transport_summary(session);
        let visible_status =
            if session.status == "Ready" && !session.runtime.runtime_notice.is_empty() {
                &session.runtime.runtime_notice
            } else {
                &session.status
            };
        ui.horizontal_wrapped(|ui| {
            ui.heading(egui::RichText::new("GHOST").color(accent()).strong());
            ui.label(egui::RichText::new("AGENT HOST").color(egui::Color32::GRAY));
            ui.add_space(22.0);
            for (index, label) in ["WORKFLOW", "ANALYZER", "ROUTING"].iter().enumerate() {
                ui.selectable_value(&mut state.selected_view, index, *label);
            }
            let status_response = ui.add(
                egui::Label::new(egui::RichText::new(visible_status).small().color(accent()))
                    .truncate(),
            );
            status_response.on_hover_text(visible_status);
            let transport_response = ui.add(egui::Label::new(&transport).truncate());
            transport_response.on_hover_text(&transport);
        });
    }

    fn start_capture(&mut self, state: &PersistedUiState, session: &mut UiSession) {
        session.active_stage = Some(Stage::Capture);
        match state.capture_source {
            CaptureSource::File => {
                let path = state.input_path.clone();
                let (sender, receiver) = mpsc::channel();
                session.capture_receiver = Some(receiver);
                session.status = "Loading media".into();
                std::thread::spawn(move || {
                    let _ = sender.send(work::capture_file(path));
                });
            }
            CaptureSource::Daw => {
                let Some(rate) = self.daw.audio_configuration().sample_rate else {
                    session.status = "DAW is not supplying an active audio configuration".into();
                    session.active_stage = None;
                    return;
                };
                let frames = (state.capture_seconds as f64 * rate).round() as usize;
                if self
                    .capture
                    .arm_tap(frames, rate.round() as u32, &state.selected_tap)
                {
                    session.armed_tap = state.selected_tap.clone();
                    session.status = "Armed — playback audio will be recorded".into();
                } else {
                    session.status = format!(
                        "Capture length exceeds {:.1} seconds at this sample rate",
                        self.capture.capacity_frames() as f64 / rate
                    );
                    session.active_stage = None;
                }
            }
        }
    }

    fn start_analysis(&mut self, state: &PersistedUiState, session: &mut UiSession) {
        let Some(material) = session.captured.clone() else {
            return;
        };
        let tap = state
            .graph
            .taps()
            .into_iter()
            .find(|tap| tap.id == state.selected_tap)
            .unwrap_or_else(|| ghost_host::GraphTap {
                id: "input".into(),
                label: "Input".into(),
            });
        let profile = state.profile;
        let (sender, receiver) = mpsc::channel();
        session.analysis_receiver = Some(receiver);
        session.active_stage = Some(Stage::Analyze);
        session.status = format!("Analyzing {}", tap.label);
        std::thread::spawn(move || {
            let _ = sender.send(work::analyze(material, tap.id, tap.label, profile));
        });
    }

    fn start_proposal(&mut self, state: &PersistedUiState, session: &mut UiSession) {
        let Some(analysis) = session.analysis.clone() else {
            return;
        };
        let state = state.clone();
        let parameter_feedback = session.parameter_feedback.clone();
        let (sender, receiver) = mpsc::channel();
        session.proposal_receiver = Some(receiver);
        session.active_stage = Some(Stage::Propose);
        session.status = "Compiling proposal preview".into();
        std::thread::spawn(move || {
            let _ = sender.send(work::propose(state, analysis, parameter_feedback));
        });
    }

    fn start_scan(&mut self, session: &mut UiSession) {
        let (sender, receiver) = mpsc::channel();
        session.scan_receiver = Some(receiver);
        session.status = "Scanning CLAP folders".into();
        std::thread::spawn(move || {
            let _ = sender.send(work::scan_plugins());
        });
    }

    fn assign_selected_plugin(
        &mut self,
        state: &mut PersistedUiState,
        session: &mut UiSession,
        node_index: usize,
    ) {
        let Some(plugin) = session.selected_plugin.as_ref().and_then(|selected| {
            session
                .plugins
                .iter()
                .find(|plugin| selected.matches(plugin))
        }) else {
            return;
        };
        if let Some(node) = state.graph.nodes.get_mut(node_index) {
            node.plugin = Some(PluginAssignment {
                path: plugin.path.clone(),
                plugin_id: Some(plugin.id.clone()),
                name: plugin.name.clone(),
                vendor: plugin.vendor.clone(),
                version: plugin.version.clone(),
                public_parameters: plugin.public_parameters.clone(),
                state: None,
            });
            if !state.child_paths.contains(&plugin.path) {
                state.child_paths.push(plugin.path.clone());
            }
            session.status = format!("Assigned {} to {}", plugin.name, node.class.label());
            self.graph_commit_requested = true;
        }
    }

    pub(crate) fn apply_preview(&mut self, state: &mut PersistedUiState, session: &mut UiSession) {
        let Some(preview) = session.proposal.as_ref() else {
            return;
        };
        let patch = preview.patch.clone();
        if patch.expected_graph_revision != state.graph_revision {
            session.patch_state = PatchApplicationState::Failed {
                transaction_id: patch.transaction_id,
                reason: "The graph changed after this preview was compiled".into(),
            };
            session.status = "Re-propose before applying to the changed graph".into();
            return;
        }
        if !patch.can_apply() {
            session.patch_state = PatchApplicationState::MappingIncomplete;
            session.status = "Resolve every parameter mapping before applying".into();
            return;
        }
        if patch.parameter_changes.is_empty() {
            for change in &patch.bypass_changes {
                if let Some(node) = state
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == change.target_node_id)
                {
                    node.bypassed = change.bypassed;
                }
            }
            session.patch_state = PatchApplicationState::Applied {
                transaction_id: patch.transaction_id,
            };
            session.status = "Bypass transaction applied".into();
            session.last_applied_patch = Some(patch);
            self.host_control.mark_project_dirty();
            return;
        }
        match self.host_control.queue_parameter_patch(&patch) {
            Ok(()) => {
                session.pending_acknowledgements = patch.parameter_changes.len();
                session.patch_state = PatchApplicationState::Queued {
                    transaction_id: patch.transaction_id,
                };
                session.status = format!(
                    "Queued {} parameter changes at the audio boundary",
                    patch.parameter_changes.len()
                );
                session.last_applied_patch = Some(patch);
                session.clear_patch_after_transaction = false;
                session.pending_bypass_changes = session
                    .last_applied_patch
                    .as_ref()
                    .map(|patch| patch.bypass_changes.clone())
                    .unwrap_or_default();
                self.host_control.request_process();
            }
            Err(reason) => {
                session.patch_state = PatchApplicationState::Failed {
                    transaction_id: patch.transaction_id,
                    reason: reason.clone(),
                };
                session.status = format!("Apply failed: {reason}");
            }
        }
    }

    pub(crate) fn undo_last_patch(
        &mut self,
        state: &mut PersistedUiState,
        session: &mut UiSession,
    ) {
        let Some(applied) = session.last_applied_patch.clone() else {
            return;
        };
        let undo = applied.undo_patch(crate::patch::next_transaction_id());
        if undo.expected_graph_revision != state.graph_revision {
            session.status = "Undo is unavailable because the graph revision changed".into();
            return;
        }
        if undo.parameter_changes.is_empty() {
            for change in &undo.bypass_changes {
                if let Some(node) = state
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == change.target_node_id)
                {
                    node.bypassed = change.bypassed;
                }
            }
            session.patch_state = PatchApplicationState::Applied {
                transaction_id: undo.transaction_id,
            };
            session.status = "Undo applied".into();
            session.last_applied_patch = None;
            self.host_control.mark_project_dirty();
            return;
        }
        match self.host_control.queue_parameter_patch(&undo) {
            Ok(()) => {
                session.pending_acknowledgements = undo.parameter_changes.len();
                session.patch_state = PatchApplicationState::Queued {
                    transaction_id: undo.transaction_id,
                };
                session.status = "Undo queued at the audio boundary".into();
                session.clear_patch_after_transaction = true;
                session.pending_bypass_changes = undo.bypass_changes;
                self.host_control.request_process();
            }
            Err(reason) => session.status = format!("Undo failed: {reason}"),
        }
    }

    fn transport_summary(&self, session: &UiSession) -> String {
        let config = self.daw.audio_configuration();
        let transport = self.daw.transport();
        let rate = config.sample_rate.map_or_else(
            || "DAW idle".into(),
            |value| format!("{:.1} kHz", value / 1_000.0),
        );
        let freshness = match session.last_transport_observed {
            None => "unavailable",
            Some(observed) if observed.elapsed() > std::time::Duration::from_millis(750) => "stale",
            Some(_) => "live",
        };
        let revisions = match (
            session.runtime.active_graph_revision,
            session.runtime.pending_graph_revision,
        ) {
            (Some(active), Some(pending)) => format!(" · graph r{active} → r{pending}"),
            (Some(active), None) => format!(" · graph r{active}"),
            _ => String::new(),
        };
        format!(
            "{} · {} · {freshness}{revisions}",
            rate,
            format_transport(transport)
        )
    }
}

fn format_transport(transport: DawTransportSnapshot) -> String {
    let state = if transport.recording {
        "REC"
    } else if transport.playing {
        "PLAY"
    } else {
        "STOP"
    };
    match (
        transport.tempo_bpm,
        transport.bar_number,
        transport.song_position_beats,
    ) {
        (Some(tempo), Some(bar), Some(beats)) => {
            let beat_in_bar = transport
                .bar_start_beats
                .map_or(beats, |bar_start| beats - bar_start)
                + 1.0;
            format!(
                "{state} · {tempo:.1} BPM · bar {} · beat {:.2}",
                bar + 1,
                beat_in_bar
            )
        }
        (Some(tempo), _, _) => format!("{state} · {tempo:.1} BPM"),
        _ => state.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AckHost {
        acknowledgements: Mutex<Vec<ghost_host::ParameterAck>>,
    }

    impl HostControl for AckHost {
        fn drain_parameter_acknowledgements(&self, output: &mut Vec<ghost_host::ParameterAck>) {
            if let Ok(mut acknowledgements) = self.acknowledgements.lock() {
                output.append(&mut acknowledgements);
            }
        }
    }

    #[test]
    fn transport_uses_beat_within_bar() {
        let summary = format_transport(DawTransportSnapshot {
            tempo_bpm: Some(120.0),
            bar_number: Some(3),
            song_position_beats: Some(14.5),
            bar_start_beats: Some(12.0),
            playing: true,
            ..DawTransportSnapshot::default()
        });
        assert!(summary.contains("bar 4 · beat 3.50"));
        assert!(!summary.contains("beat 14.50"));
    }

    #[test]
    fn ui_session_survives_transient_editor_instances() {
        let project = Arc::new(Mutex::new(PersistedUiState::default()));
        let session = Arc::new(Mutex::new(UiSession::default()));
        session.lock().unwrap().status = "Analysis ready".into();
        let runtime = || {
            GhostUi::with_runtime_and_session(
                Arc::clone(&project),
                Arc::clone(&session),
                Arc::new(AtomicDawState::default()),
                Arc::new(RealtimeCaptureBuffer::new(16)),
                Arc::new(AtomicGraphControl::default()),
                Arc::new(NoHostControl),
            )
        };
        drop(runtime());
        let reopened = runtime();
        drop(reopened);
        assert_eq!(session.lock().unwrap().status, "Analysis ready");
    }

    #[test]
    fn supported_narrow_and_wide_layouts_render() {
        for (width, height) in [(860.0, 600.0), (1180.0, 760.0)] {
            let context = egui::Context::default();
            let mut application = GhostUi::default();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, height),
                )),
                ..Default::default()
            };
            let output = context.run_ui(input, |ui| application.show(ui));
            assert!(!output.shapes.is_empty());
        }
    }

    #[test]
    fn bypass_commits_only_after_successful_parameter_transaction() {
        for (status, expected_bypass) in [
            (ghost_host::ParameterAckStatus::Applied, true),
            (ghost_host::ParameterAckStatus::ParameterRejected, false),
        ] {
            let host = Arc::new(AckHost {
                acknowledgements: Mutex::new(vec![ghost_host::ParameterAck {
                    transaction_id: 41,
                    node_id: "eq-1".into(),
                    parameter_id: "1".into(),
                    value: -2.0,
                    previous_value: Some(0.0),
                    status,
                }]),
            });
            let application = GhostUi::with_runtime(
                Arc::new(Mutex::new(PersistedUiState::default())),
                Arc::new(AtomicDawState::default()),
                Arc::new(RealtimeCaptureBuffer::new(16)),
                Arc::new(AtomicGraphControl::default()),
                host,
            );
            let mut state = PersistedUiState::default();
            state.graph.nodes[0].id = "eq-1".into();
            state.graph.nodes[0].bypassed = false;
            let mut session = UiSession {
                patch_state: PatchApplicationState::Queued { transaction_id: 41 },
                pending_acknowledgements: 1,
                pending_bypass_changes: vec![ghost_host::CompiledBypassChange {
                    target_node_id: "eq-1".into(),
                    expected_graph_revision: state.graph_revision,
                    bypassed: true,
                    previous_bypassed: false,
                }],
                ..UiSession::default()
            };
            application.poll_parameter_acknowledgements(&mut state, &mut session);
            assert_eq!(state.graph.nodes[0].bypassed, expected_bypass);
            assert!(session.pending_bypass_changes.is_empty());
        }
    }
}
