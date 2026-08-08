use std::path::PathBuf;

use ghost_core::ParameterDescriptor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProcessorClass {
    #[default]
    Equalizer,
    Compressor,
    Saturation,
    Reverb,
    Limiter,
    MultibandCompressor,
}

impl ProcessorClass {
    pub const ALL: [Self; 6] = [
        Self::Equalizer,
        Self::Compressor,
        Self::Saturation,
        Self::Reverb,
        Self::Limiter,
        Self::MultibandCompressor,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Equalizer => "Equalizer",
            Self::Compressor => "Compressor",
            Self::Saturation => "Saturation",
            Self::Reverb => "Reverb",
            Self::Limiter => "Limiter",
            Self::MultibandCompressor => "Multiband compressor",
        }
    }

    pub fn capability_kind(self) -> &'static str {
        match self {
            Self::Equalizer => "equalizer.band",
            Self::Compressor => "compressor.settings",
            Self::Saturation => "saturation.settings",
            Self::Reverb => "reverb.settings",
            Self::Limiter => "limiter.settings",
            Self::MultibandCompressor => "multiband_compressor.settings",
        }
    }

    pub fn context_available(self) -> bool {
        matches!(self, Self::Equalizer | Self::Compressor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginAssignment {
    pub path: PathBuf,
    pub plugin_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub public_parameters: Vec<ParameterDescriptor>,
    #[serde(default)]
    pub state: Option<crate::ChildStateBlob>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNodeSpec {
    pub id: String,
    pub class: ProcessorClass,
    pub bypassed: bool,
    pub plugin: Option<PluginAssignment>,
}

impl GraphNodeSpec {
    pub fn new(id: impl Into<String>, class: ProcessorClass) -> Self {
        Self {
            id: id.into(),
            class,
            bypassed: false,
            plugin: None,
        }
    }

    pub fn display_name(&self) -> &str {
        self.plugin
            .as_ref()
            .map_or_else(|| self.class.label(), |plugin| plugin.name.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct EditableGraph {
    #[serde(default)]
    pub nodes: Vec<GraphNodeSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphPreset {
    Empty,
    Equalizer,
    Compressor,
    EqualizerCompressor,
}

impl GraphPreset {
    pub const ALL: [Self; 4] = [
        Self::Empty,
        Self::Equalizer,
        Self::Compressor,
        Self::EqualizerCompressor,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "Empty",
            Self::Equalizer => "Equalizer",
            Self::Compressor => "Compressor",
            Self::EqualizerCompressor => "EQ + compressor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphTap {
    pub id: String,
    pub label: String,
}

impl EditableGraph {
    pub fn apply_preset(&mut self, preset: GraphPreset) {
        self.nodes = match preset {
            GraphPreset::Empty => Vec::new(),
            GraphPreset::Equalizer => vec![GraphNodeSpec::new("eq-1", ProcessorClass::Equalizer)],
            GraphPreset::Compressor => {
                vec![GraphNodeSpec::new(
                    "compressor-1",
                    ProcessorClass::Compressor,
                )]
            }
            GraphPreset::EqualizerCompressor => vec![
                GraphNodeSpec::new("eq-1", ProcessorClass::Equalizer),
                GraphNodeSpec::new("compressor-1", ProcessorClass::Compressor),
            ],
        };
    }

    pub fn create_node(&mut self, id: impl Into<String>, class: ProcessorClass) {
        self.nodes.push(GraphNodeSpec::new(id, class));
    }

    pub fn remove(&mut self, index: usize) -> Option<GraphNodeSpec> {
        (index < self.nodes.len()).then(|| self.nodes.remove(index))
    }

    pub fn move_by(&mut self, index: usize, delta: isize) -> bool {
        let destination = index as isize + delta;
        if index >= self.nodes.len() || !(0..self.nodes.len() as isize).contains(&destination) {
            return false;
        }
        self.nodes.swap(index, destination as usize);
        true
    }

    pub fn taps(&self) -> Vec<GraphTap> {
        let mut taps = Vec::with_capacity(self.nodes.len() + 2);
        taps.push(GraphTap {
            id: "input".into(),
            label: "Input".into(),
        });
        taps.extend(self.nodes.iter().map(|node| GraphTap {
            id: format!("post:{}", node.id),
            label: format!("After {}", node.display_name()),
        }));
        taps.push(GraphTap {
            id: "output".into(),
            label: "Output".into(),
        });
        taps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_is_editable_and_taps_follow_topology() {
        let mut graph = EditableGraph::default();
        graph.apply_preset(GraphPreset::EqualizerCompressor);
        assert_eq!(graph.taps().len(), 4);
        assert!(graph.move_by(1, -1));
        assert_eq!(graph.nodes[0].class, ProcessorClass::Compressor);
        graph.remove(1);
        assert_eq!(graph.taps()[1].label, "After Compressor");
        graph.apply_preset(GraphPreset::Empty);
        assert_eq!(graph.taps().len(), 2);
    }
}
