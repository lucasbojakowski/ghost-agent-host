use std::path::PathBuf;

use ghost_host::{EditableGraph, GraphPreset};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSource {
    #[default]
    Daw,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PersistedUiState {
    pub schema_version: String,
    /// Monotonic revision of the committed processor topology.
    pub graph_revision: u64,
    pub input_path: String,
    pub prompt: String,
    pub profile: usize,
    pub selected_view: usize,
    pub capture_source: CaptureSource,
    pub capture_seconds: f32,
    pub selected_tap: String,
    pub scanner_open: bool,
    pub graph: EditableGraph,
    pub next_node_id: u64,
    pub child_paths: Vec<PathBuf>,
    pub editor_width: u32,
    pub editor_height: u32,
}

impl Default for PersistedUiState {
    fn default() -> Self {
        let mut graph = EditableGraph::default();
        graph.apply_preset(GraphPreset::EqualizerCompressor);
        Self {
            schema_version: "ghost.ui-state/3".into(),
            graph_revision: 1,
            input_path: "fixtures/muddy_bass.wav".into(),
            prompt: "Tighten the low mids while preserving punch.".into(),
            profile: 2,
            selected_view: 0,
            capture_source: CaptureSource::Daw,
            capture_seconds: 6.0,
            selected_tap: "input".into(),
            scanner_open: false,
            graph,
            next_node_id: 3,
            child_paths: Vec::new(),
            editor_width: 1180,
            editor_height: 760,
        }
    }
}

impl PersistedUiState {
    pub fn migrate(mut self) -> Self {
        if self.schema_version == "ghost.ui-state/1" {
            let mut graph = EditableGraph::default();
            graph.apply_preset(GraphPreset::EqualizerCompressor);
            self.graph = graph;
        }
        self.schema_version = "ghost.ui-state/3".into();
        self.graph_revision = self.graph_revision.max(1);
        self.profile = self.profile.min(2);
        self.selected_view = self.selected_view.min(2);
        self.capture_seconds = self.capture_seconds.clamp(0.5, 24.0);
        self.editor_width = self.editor_width.max(860);
        self.editor_height = self.editor_height.max(600);
        self
    }

    pub fn commit_graph(&mut self) -> u64 {
        self.graph_revision = self.graph_revision.saturating_add(1).max(1);
        self.graph_revision
    }
}

/// Public architectural name for the serialized project document.
pub type ProjectDocument = PersistedUiState;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_establishes_revision_and_minimum_editor_size() {
        let state = PersistedUiState {
            schema_version: "ghost.ui-state/2".into(),
            graph_revision: 0,
            editor_width: 320,
            editor_height: 200,
            ..PersistedUiState::default()
        }
        .migrate();
        assert_eq!(state.schema_version, "ghost.ui-state/3");
        assert_eq!(state.graph_revision, 1);
        assert_eq!((state.editor_width, state.editor_height), (860, 600));
    }
}
