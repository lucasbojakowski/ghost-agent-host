use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use ghost_core::{CapabilityDescriptor, ParameterDescriptor, ProcessorDescriptor};
use ghost_host::{
    AudioBlock, ChildError, ChildProcessor, ChildStateBlob, ParameterRamp, ParentWindow,
    ProcessConfig,
};
use serde::{Deserialize, Serialize};

pub const FAKE_CHILD_ID: &str = "ai.konko.ghost.fake-child";

#[derive(Default)]
struct ProbeInner {
    activations: AtomicUsize,
    process_calls: AtomicUsize,
    gui_opens: AtomicUsize,
    gui_closes: AtomicUsize,
    gui_visible: AtomicBool,
    destroyed: AtomicBool,
}

#[derive(Clone, Default)]
pub struct FakeChildProbe(Arc<ProbeInner>);

impl FakeChildProbe {
    pub fn activations(&self) -> usize {
        self.0.activations.load(Ordering::Relaxed)
    }

    pub fn process_calls(&self) -> usize {
        self.0.process_calls.load(Ordering::Relaxed)
    }

    pub fn gui_opens(&self) -> usize {
        self.0.gui_opens.load(Ordering::Relaxed)
    }

    pub fn gui_closes(&self) -> usize {
        self.0.gui_closes.load(Ordering::Relaxed)
    }

    pub fn gui_visible(&self) -> bool {
        self.0.gui_visible.load(Ordering::Relaxed)
    }

    pub fn destroyed(&self) -> bool {
        self.0.destroyed.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone, Copy)]
enum FakeKind {
    Equalizer,
    Compressor,
}

#[derive(Debug, Clone)]
struct PendingRamp {
    parameter: &'static str,
    start: f64,
    end: f64,
    frames: usize,
}

#[derive(Serialize, Deserialize)]
struct SavedState {
    parameters: BTreeMap<String, f64>,
}

pub struct FakeClapChild {
    descriptor: ProcessorDescriptor,
    kind: FakeKind,
    parameters: BTreeMap<String, f64>,
    config: Option<ProcessConfig>,
    ramp: Option<PendingRamp>,
    gui_open: bool,
    probe: FakeChildProbe,
}

impl FakeClapChild {
    pub fn equalizer() -> (Self, FakeChildProbe) {
        Self::new(FakeKind::Equalizer)
    }

    pub fn compressor() -> (Self, FakeChildProbe) {
        Self::new(FakeKind::Compressor)
    }

    fn new(kind: FakeKind) -> (Self, FakeChildProbe) {
        let (name, stable_id, parameters, capabilities) = match kind {
            FakeKind::Equalizer => (
                "Ghost Fake Equalizer",
                "ai.konko.ghost.fake-eq",
                vec![
                    parameter("frequency", "Frequency", "Hz", 0.5),
                    parameter("gain", "Gain", "dB", 0.5),
                    parameter("q", "Q", "", 0.25),
                ],
                vec![CapabilityDescriptor {
                    namespace: "audio.mix".into(),
                    kind: "equalizer.band".into(),
                    attributes: BTreeMap::new(),
                }],
            ),
            FakeKind::Compressor => (
                "Ghost Fake Compressor",
                "ai.konko.ghost.fake-compressor",
                vec![
                    parameter("threshold", "Threshold", "dB", 0.75),
                    parameter("ratio", "Ratio", ":1", 0.1),
                    parameter("output", "Output", "dB", 0.5),
                ],
                vec![CapabilityDescriptor {
                    namespace: "audio.mix".into(),
                    kind: "compressor.settings".into(),
                    attributes: BTreeMap::new(),
                }],
            ),
        };
        let values = parameters
            .iter()
            .map(|parameter| (parameter.stable_id.clone(), parameter.default))
            .collect();
        let probe = FakeChildProbe::default();
        (
            Self {
                descriptor: ProcessorDescriptor {
                    stable_id: stable_id.into(),
                    name: name.into(),
                    vendor: Some("Konko test double".into()),
                    version: Some("1".into()),
                    capabilities,
                    parameters,
                },
                kind,
                parameters: values,
                config: None,
                ramp: None,
                gui_open: false,
                probe: probe.clone(),
            },
            probe,
        )
    }

    fn known_parameter(&self, id: &str) -> Option<&'static str> {
        match (self.kind, id) {
            (FakeKind::Equalizer, "frequency") => Some("frequency"),
            (FakeKind::Equalizer, "gain") => Some("gain"),
            (FakeKind::Equalizer, "q") => Some("q"),
            (FakeKind::Compressor, "threshold") => Some("threshold"),
            (FakeKind::Compressor, "ratio") => Some("ratio"),
            (FakeKind::Compressor, "output") => Some("output"),
            _ => None,
        }
    }
}

impl ChildProcessor for FakeClapChild {
    fn descriptor(&self) -> &ProcessorDescriptor {
        &self.descriptor
    }

    fn activate(&mut self, config: ProcessConfig) -> Result<(), ChildError> {
        self.config = Some(config);
        self.probe.0.activations.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn deactivate(&mut self) {
        self.config = None;
    }

    fn process(&mut self, block: &mut AudioBlock<'_>) -> Result<(), ChildError> {
        if self.config.is_none() {
            return Err(ChildError::NotActive);
        }
        self.probe.0.process_calls.fetch_add(1, Ordering::Relaxed);
        let ramp = self.ramp.take();
        for frame in 0..block.frames {
            let gain = match self.kind {
                FakeKind::Equalizer => {
                    let normalized =
                        ramp_value(&ramp, "gain", frame).unwrap_or_else(|| self.parameters["gain"]);
                    10.0_f64.powf(((normalized - 0.5) * 36.0) / 20.0)
                }
                FakeKind::Compressor => {
                    let output = ramp_value(&ramp, "output", frame)
                        .unwrap_or_else(|| self.parameters["output"]);
                    10.0_f64.powf(((output - 0.5) * 24.0) / 20.0)
                }
            } as f32;
            for channel in block.channels.iter_mut() {
                channel[frame] *= gain;
            }
        }
        if let Some(ramp) = ramp {
            self.parameters.insert(ramp.parameter.into(), ramp.end);
        }
        Ok(())
    }

    fn parameter(&self, parameter_id: &str) -> Result<f64, ChildError> {
        self.parameters
            .get(parameter_id)
            .copied()
            .ok_or_else(|| ChildError::UnknownParameter(parameter_id.into()))
    }

    fn set_parameter(&mut self, parameter_id: &str, normalized: f64) -> Result<(), ChildError> {
        if !(0.0..=1.0).contains(&normalized) {
            return Err(ChildError::InvalidValue(normalized));
        }
        let parameter = self
            .known_parameter(parameter_id)
            .ok_or_else(|| ChildError::UnknownParameter(parameter_id.into()))?;
        self.parameters.insert(parameter.into(), normalized);
        Ok(())
    }

    fn set_parameter_ramp(&mut self, ramp: &ParameterRamp<'_>) -> Result<(), ChildError> {
        let parameter = self
            .known_parameter(ramp.parameter_id)
            .ok_or_else(|| ChildError::UnknownParameter(ramp.parameter_id.into()))?;
        self.ramp = Some(PendingRamp {
            parameter,
            start: ramp.start_normalized,
            end: ramp.end_normalized,
            frames: ramp.frames,
        });
        Ok(())
    }

    fn save_state(&self) -> Result<ChildStateBlob, ChildError> {
        Ok(ChildStateBlob {
            format: "ghost.fake-child-state/1".into(),
            bytes: serde_json::to_vec(&SavedState {
                parameters: self.parameters.clone(),
            })
            .map_err(|error| ChildError::Failed(error.to_string()))?,
        })
    }

    fn load_state(&mut self, state: &ChildStateBlob) -> Result<(), ChildError> {
        if state.format != "ghost.fake-child-state/1" {
            return Err(ChildError::Failed("unsupported fake state".into()));
        }
        let state: SavedState = serde_json::from_slice(&state.bytes)
            .map_err(|error| ChildError::Failed(error.to_string()))?;
        self.parameters = state.parameters;
        Ok(())
    }

    fn gui_supported(&self) -> bool {
        true
    }

    fn open_gui(&mut self, _parent: Option<ParentWindow>) -> Result<(), ChildError> {
        self.gui_open = true;
        self.probe.0.gui_opens.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn set_gui_visible(&mut self, visible: bool) -> Result<(), ChildError> {
        if !self.gui_open {
            return Err(ChildError::Failed("fake GUI is closed".into()));
        }
        self.probe.0.gui_visible.store(visible, Ordering::Relaxed);
        Ok(())
    }

    fn close_gui(&mut self) {
        if self.gui_open {
            self.probe.0.gui_closes.fetch_add(1, Ordering::Relaxed);
        }
        self.gui_open = false;
        self.probe.0.gui_visible.store(false, Ordering::Relaxed);
    }
}

impl Drop for FakeClapChild {
    fn drop(&mut self) {
        self.close_gui();
        self.probe.0.destroyed.store(true, Ordering::Relaxed);
    }
}

fn parameter(id: &str, name: &str, unit: &str, default: f64) -> ParameterDescriptor {
    ParameterDescriptor {
        stable_id: id.into(),
        name: name.into(),
        module: None,
        unit: (!unit.is_empty()).then(|| unit.into()),
        minimum: 0.0,
        maximum: 1.0,
        default,
        stepped: false,
        read_only: false,
        labels: BTreeMap::new(),
    }
}

fn ramp_value(ramp: &Option<PendingRamp>, parameter: &str, frame: usize) -> Option<f64> {
    ramp.as_ref()
        .filter(|ramp| ramp.parameter == parameter)
        .map(|ramp| {
            let fraction = (frame + 1).min(ramp.frames) as f64 / ramp.frames.max(1) as f64;
            ramp.start + (ramp.end - ramp.start) * fraction
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghost_host::{map_public_parameters, ProcessorGraph, SemanticParameterSpec};

    #[test]
    fn graph_exercises_audio_state_bypass_smoothing_and_gui_lifecycle() {
        let (equalizer, eq_probe) = FakeClapChild::equalizer();
        let descriptor = equalizer.descriptor().clone();
        let bindings = map_public_parameters(
            &descriptor,
            &[SemanticParameterSpec {
                semantic_id: "eq.gain_db".into(),
                aliases: vec!["Gain".into()],
                unit: Some("dB".into()),
            }],
        );
        assert_eq!(bindings[0].parameter_id, "gain");

        let config = ProcessConfig {
            sample_rate: 48_000,
            maximum_frames: 64,
            channels: 2,
        };
        let mut graph = ProcessorGraph::new(config);
        graph.insert("eq", equalizer).unwrap();
        graph.activate().unwrap();
        graph.open_gui("eq", None).unwrap();
        graph.set_gui_visible("eq", false).unwrap();
        graph.open_gui("eq", None).unwrap();
        assert_eq!(eq_probe.gui_opens(), 1);
        assert!(eq_probe.gui_visible());

        graph
            .set_parameter_smoothed("eq", "gain", 0.75, 1.0)
            .unwrap();
        let mut left = [0.1_f32; 48];
        let mut right = [0.1_f32; 48];
        let mut channels: [&mut [f32]; 2] = [&mut left, &mut right];
        graph
            .process(&mut AudioBlock {
                channels: &mut channels,
                frames: 48,
            })
            .unwrap();
        assert!(left[47] > left[0]);
        let state = graph.save_state().unwrap();
        graph.set_bypassed("eq", true).unwrap();
        let before = left;
        let mut channels: [&mut [f32]; 2] = [&mut left, &mut right];
        graph
            .process(&mut AudioBlock {
                channels: &mut channels,
                frames: 48,
            })
            .unwrap();
        assert_eq!(left, before);
        graph.load_state(&state).unwrap();
        drop(graph);
        assert_eq!(eq_probe.gui_closes(), 1);
        assert!(eq_probe.destroyed());
    }
}
