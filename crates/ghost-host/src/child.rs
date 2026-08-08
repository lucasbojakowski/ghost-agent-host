use std::ffi::c_void;

use ghost_core::{ParameterDescriptor, ProcessorDescriptor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ChildError {
    #[error("child is not active")]
    NotActive,
    #[error("unknown parameter `{0}`")]
    UnknownParameter(String),
    #[error("invalid normalized value {0}")]
    InvalidValue(f64),
    #[error("unsupported child operation: {0}")]
    Unsupported(String),
    #[error("child operation failed: {0}")]
    Failed(String),
    #[error("native child block shape mismatch")]
    BlockShapeMismatch,
    #[error("native child process callback failed")]
    ProcessFailed,
    #[error("unknown realtime parameter")]
    UnknownRealtimeParameter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessConfig {
    pub sample_rate: u32,
    pub maximum_frames: usize,
    pub channels: usize,
}

pub struct AudioBlock<'a> {
    pub channels: &'a mut [&'a mut [f32]],
    pub frames: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterRamp<'a> {
    pub parameter_id: &'a str,
    pub start_normalized: f64,
    pub end_normalized: f64,
    pub frames: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParentWindow {
    pub raw: *mut c_void,
}

// Window handles are opaque values whose thread-affinity is enforced by the UI adapter.
unsafe impl Send for ParentWindow {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChildStateBlob {
    pub format: String,
    pub bytes: Vec<u8>,
}

/// Stable interface implemented by native CLAP instances and deterministic test children.
pub trait ChildProcessor: Send {
    fn descriptor(&self) -> &ProcessorDescriptor;
    fn parameters(&self) -> &[ParameterDescriptor] {
        &self.descriptor().parameters
    }
    fn activate(&mut self, config: ProcessConfig) -> Result<(), ChildError>;
    fn deactivate(&mut self);
    fn process(&mut self, block: &mut AudioBlock<'_>) -> Result<(), ChildError>;
    fn parameter(&self, parameter_id: &str) -> Result<f64, ChildError>;
    fn set_parameter(&mut self, parameter_id: &str, normalized: f64) -> Result<(), ChildError>;
    fn set_parameter_ramp(&mut self, ramp: &ParameterRamp<'_>) -> Result<(), ChildError> {
        self.set_parameter(ramp.parameter_id, ramp.end_normalized)
    }
    fn save_state(&self) -> Result<ChildStateBlob, ChildError>;
    fn load_state(&mut self, state: &ChildStateBlob) -> Result<(), ChildError>;
    fn gui_supported(&self) -> bool;
    fn open_gui(&mut self, parent: Option<ParentWindow>) -> Result<(), ChildError>;
    fn set_gui_visible(&mut self, visible: bool) -> Result<(), ChildError>;
    fn close_gui(&mut self);
}
