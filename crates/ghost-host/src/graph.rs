use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    AudioBlock, ChildError, ChildProcessor, ChildStateBlob, ParameterRamp, ParentWindow,
    ProcessConfig, SmoothedValue,
};

struct Automation {
    slot_id: String,
    parameter_id: String,
    value: SmoothedValue,
}

struct ProcessorSlot {
    id: String,
    child: Box<dyn ChildProcessor>,
    bypassed: bool,
    gui_open: bool,
    gui_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessorSlotState {
    pub id: String,
    pub processor_stable_id: String,
    pub bypassed: bool,
    pub state: ChildStateBlob,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessorGraphState {
    pub schema_version: String,
    pub slots: Vec<ProcessorSlotState>,
}

pub struct ProcessorGraph {
    config: ProcessConfig,
    active: bool,
    slots: Vec<ProcessorSlot>,
    automation: Vec<Automation>,
}

impl ProcessorGraph {
    pub fn new(config: ProcessConfig) -> Self {
        Self {
            config,
            active: false,
            slots: Vec::new(),
            automation: Vec::new(),
        }
    }

    pub fn insert(
        &mut self,
        id: impl Into<String>,
        child: impl ChildProcessor + 'static,
    ) -> Result<(), ChildError> {
        let id = id.into();
        if self.slots.iter().any(|slot| slot.id == id) {
            return Err(ChildError::Failed(format!("duplicate slot `{id}`")));
        }
        self.slots.push(ProcessorSlot {
            id,
            child: Box::new(child),
            bypassed: false,
            gui_open: false,
            gui_visible: false,
        });
        Ok(())
    }

    pub fn activate(&mut self) -> Result<(), ChildError> {
        for slot in &mut self.slots {
            slot.child.activate(self.config)?;
        }
        self.active = true;
        Ok(())
    }

    pub fn set_bypassed(&mut self, id: &str, bypassed: bool) -> Result<(), ChildError> {
        self.slot_mut(id)?.bypassed = bypassed;
        Ok(())
    }

    pub fn set_parameter_smoothed(
        &mut self,
        slot_id: &str,
        parameter_id: &str,
        normalized: f64,
        smoothing_ms: f64,
    ) -> Result<(), ChildError> {
        if !(0.0..=1.0).contains(&normalized) {
            return Err(ChildError::InvalidValue(normalized));
        }
        let current = self.slot_mut(slot_id)?.child.parameter(parameter_id)?;
        let frames = (smoothing_ms.max(0.0) * self.config.sample_rate as f64 / 1_000.0) as usize;
        if let Some(automation) = self
            .automation
            .iter_mut()
            .find(|item| item.slot_id == slot_id && item.parameter_id == parameter_id)
        {
            automation.value.set_target(normalized, frames);
        } else {
            let mut value = SmoothedValue::new(current);
            value.set_target(normalized, frames);
            self.automation.push(Automation {
                slot_id: slot_id.into(),
                parameter_id: parameter_id.into(),
                value,
            });
        }
        Ok(())
    }

    /// No allocations, locks, serialization, I/O, or logging occur in this method.
    pub fn process(&mut self, block: &mut AudioBlock<'_>) -> Result<(), ChildError> {
        if !self.active {
            return Err(ChildError::NotActive);
        }
        for automation in &mut self.automation {
            let (start, end) = automation.value.advance(block.frames);
            let slot = self
                .slots
                .iter_mut()
                .find(|slot| slot.id == automation.slot_id)
                .ok_or_else(|| ChildError::Failed("automation slot disappeared".into()))?;
            slot.child.set_parameter_ramp(&ParameterRamp {
                parameter_id: &automation.parameter_id,
                start_normalized: start,
                end_normalized: end,
                frames: block.frames,
            })?;
        }
        for slot in &mut self.slots {
            if !slot.bypassed {
                slot.child.process(block)?;
            }
        }
        Ok(())
    }

    pub fn open_gui(&mut self, id: &str, parent: Option<ParentWindow>) -> Result<(), ChildError> {
        let slot = self.slot_mut(id)?;
        if !slot.gui_open {
            slot.child.open_gui(parent)?;
            slot.gui_open = true;
        }
        slot.child.set_gui_visible(true)?;
        slot.gui_visible = true;
        Ok(())
    }

    pub fn set_gui_visible(&mut self, id: &str, visible: bool) -> Result<(), ChildError> {
        let slot = self.slot_mut(id)?;
        if !slot.gui_open {
            return Err(ChildError::Failed("GUI has not been opened".into()));
        }
        slot.child.set_gui_visible(visible)?;
        slot.gui_visible = visible;
        Ok(())
    }

    pub fn close_gui(&mut self, id: &str) -> Result<(), ChildError> {
        let slot = self.slot_mut(id)?;
        if slot.gui_open {
            slot.child.close_gui();
            slot.gui_open = false;
            slot.gui_visible = false;
        }
        Ok(())
    }

    pub fn save_state(&self) -> Result<ProcessorGraphState, ChildError> {
        Ok(ProcessorGraphState {
            schema_version: "ghost.processor-graph-state/1".into(),
            slots: self
                .slots
                .iter()
                .map(|slot| {
                    Ok(ProcessorSlotState {
                        id: slot.id.clone(),
                        processor_stable_id: slot.child.descriptor().stable_id.clone(),
                        bypassed: slot.bypassed,
                        state: slot.child.save_state()?,
                    })
                })
                .collect::<Result<_, ChildError>>()?,
        })
    }

    pub fn load_state(&mut self, state: &ProcessorGraphState) -> Result<(), ChildError> {
        if state.schema_version != "ghost.processor-graph-state/1" {
            return Err(ChildError::Failed(format!(
                "unsupported graph state {}",
                state.schema_version
            )));
        }
        let by_id: BTreeMap<_, _> = state.slots.iter().map(|slot| (&slot.id, slot)).collect();
        for slot in &mut self.slots {
            let saved = by_id
                .get(&slot.id)
                .ok_or_else(|| ChildError::Failed(format!("state missing slot `{}`", slot.id)))?;
            if saved.processor_stable_id != slot.child.descriptor().stable_id {
                return Err(ChildError::Failed(format!(
                    "processor mismatch in slot `{}`",
                    slot.id
                )));
            }
            slot.child.load_state(&saved.state)?;
            slot.bypassed = saved.bypassed;
        }
        Ok(())
    }

    fn slot_mut(&mut self, id: &str) -> Result<&mut ProcessorSlot, ChildError> {
        self.slots
            .iter_mut()
            .find(|slot| slot.id == id)
            .ok_or_else(|| ChildError::Failed(format!("unknown slot `{id}`")))
    }
}

impl Drop for ProcessorGraph {
    fn drop(&mut self) {
        for slot in self.slots.iter_mut().rev() {
            if slot.gui_open {
                slot.child.close_gui();
            }
            slot.child.deactivate();
        }
    }
}
