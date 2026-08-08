//! Bounded control and acknowledgement queues crossing the UI/audio boundary.

use crossbeam_queue::ArrayQueue;

pub const MAXIMUM_PATCH_CHANGES: usize = 32;
const TRANSACTION_CAPACITY: usize = 8;
const ACK_CAPACITY: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledParameterChange {
    pub target_node_id: String,
    pub expected_graph_revision: u64,
    pub semantic_field: String,
    pub parameter_id: String,
    pub plain_value: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub mapping_confidence: f32,
    pub previous_value: f64,
    pub requires_restart: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledBypassChange {
    pub target_node_id: String,
    pub expected_graph_revision: u64,
    pub bypassed: bool,
    pub previous_bypassed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledParameterPatch {
    pub transaction_id: u64,
    pub expected_graph_revision: u64,
    pub parameter_changes: Vec<CompiledParameterChange>,
    pub bypass_changes: Vec<CompiledBypassChange>,
    pub mapping_issues: Vec<String>,
}

impl CompiledParameterPatch {
    pub fn is_complete(&self) -> bool {
        self.mapping_issues.is_empty()
            && (!self.parameter_changes.is_empty() || !self.bypass_changes.is_empty())
            && self.parameter_changes.len() <= MAXIMUM_PATCH_CHANGES
    }

    pub fn can_apply(&self) -> bool {
        self.mapping_issues.is_empty()
            && (!self.parameter_changes.is_empty() || !self.bypass_changes.is_empty())
            && self.parameter_changes.len() <= MAXIMUM_PATCH_CHANGES
    }

    pub fn undo_patch(&self, transaction_id: u64) -> Self {
        Self {
            transaction_id,
            expected_graph_revision: self.expected_graph_revision,
            parameter_changes: self
                .parameter_changes
                .iter()
                .map(|change| {
                    let mut inverse = change.clone();
                    inverse.plain_value = change.previous_value;
                    inverse.previous_value = change.plain_value;
                    inverse
                })
                .collect(),
            bypass_changes: self
                .bypass_changes
                .iter()
                .map(|change| CompiledBypassChange {
                    target_node_id: change.target_node_id.clone(),
                    expected_graph_revision: change.expected_graph_revision,
                    bypassed: change.previous_bypassed,
                    previous_bypassed: change.bypassed,
                })
                .collect(),
            mapping_issues: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterAckStatus {
    Applied,
    GraphRevisionMismatch,
    NodeUnavailable,
    ParameterRejected,
    TransactionRejected,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterAck {
    pub transaction_id: u64,
    pub node_id: String,
    pub parameter_id: String,
    pub value: f64,
    pub previous_value: Option<f64>,
    pub status: ParameterAckStatus,
}

#[derive(Debug)]
pub enum ParameterQueueError {
    Empty,
    Incomplete,
    TooLarge,
    Busy,
}

pub struct ParameterTransaction {
    pub transaction_id: u64,
    pub expected_graph_revision: u64,
    pub changes: Vec<CompiledParameterChange>,
}

pub struct RealtimeParameterControl {
    transactions: ArrayQueue<ParameterTransaction>,
    acknowledgements: ArrayQueue<ParameterAck>,
}

impl Default for RealtimeParameterControl {
    fn default() -> Self {
        Self::new()
    }
}

impl RealtimeParameterControl {
    pub fn new() -> Self {
        Self {
            transactions: ArrayQueue::new(TRANSACTION_CAPACITY),
            acknowledgements: ArrayQueue::new(ACK_CAPACITY),
        }
    }

    pub fn enqueue_patch(&self, patch: &CompiledParameterPatch) -> Result<(), ParameterQueueError> {
        if !patch.mapping_issues.is_empty() {
            return Err(ParameterQueueError::Incomplete);
        }
        if patch.parameter_changes.is_empty() {
            return Err(ParameterQueueError::Empty);
        }
        if patch.parameter_changes.len() > MAXIMUM_PATCH_CHANGES {
            return Err(ParameterQueueError::TooLarge);
        }

        // Multiband plugins such as Pro-Q expose separate `Used` and `Enabled` controls. `Used`
        // materializes a physical band slot, while `Enabled` controls an already-materialized
        // band's active state. Preserve relative order within each phase while forcing the host to
        // create a band before enabling it and before sending frequency/gain/Q values.
        let mut changes = patch.parameter_changes.clone();
        changes.sort_by_key(parameter_change_priority);

        self.transactions
            .push(ParameterTransaction {
                transaction_id: patch.transaction_id,
                expected_graph_revision: patch.expected_graph_revision,
                changes,
            })
            .map_err(|_| ParameterQueueError::Busy)
    }

    pub fn pop_transaction(&self) -> Option<ParameterTransaction> {
        self.transactions.pop()
    }

    pub fn acknowledge(&self, acknowledgement: ParameterAck) {
        let _ = self.acknowledgements.push(acknowledgement);
    }

    pub fn drain_acknowledgements(&self, output: &mut Vec<ParameterAck>) {
        while let Some(acknowledgement) = self.acknowledgements.pop() {
            output.push(acknowledgement);
        }
    }
}

fn parameter_change_priority(change: &CompiledParameterChange) -> u8 {
    match change.semantic_field.as_str() {
        "used" => 0,
        "enabled" => 1,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change() -> CompiledParameterChange {
        CompiledParameterChange {
            target_node_id: "eq-1".into(),
            expected_graph_revision: 4,
            semantic_field: "gain_db".into(),
            parameter_id: "1".into(),
            plain_value: -2.0,
            minimum: -24.0,
            maximum: 24.0,
            mapping_confidence: 1.0,
            previous_value: 0.0,
            requires_restart: false,
        }
    }

    #[test]
    fn bounded_queue_preserves_transaction_metadata() {
        let control = RealtimeParameterControl::new();
        let patch = CompiledParameterPatch {
            transaction_id: 9,
            expected_graph_revision: 4,
            parameter_changes: vec![change()],
            bypass_changes: Vec::new(),
            mapping_issues: Vec::new(),
        };
        control.enqueue_patch(&patch).unwrap();
        let transaction = control.pop_transaction().unwrap();
        assert_eq!(transaction.transaction_id, 9);
        assert_eq!(transaction.expected_graph_revision, 4);
        assert_eq!(transaction.changes[0].expected_graph_revision, 4);
    }

    #[test]
    fn materialization_and_activation_are_queued_before_band_values() {
        let control = RealtimeParameterControl::new();
        let mut frequency = change();
        frequency.semantic_field = "frequency_hz".into();
        frequency.parameter_id = "10".into();
        frequency.plain_value = 250.0;

        let mut enabled = change();
        enabled.semantic_field = "enabled".into();
        enabled.parameter_id = "11".into();
        enabled.minimum = 0.0;
        enabled.maximum = 1.0;
        enabled.plain_value = 1.0;

        let mut used = change();
        used.semantic_field = "used".into();
        used.parameter_id = "12".into();
        used.minimum = 0.0;
        used.maximum = 1.0;
        used.plain_value = 1.0;

        let mut q = change();
        q.semantic_field = "q".into();
        q.parameter_id = "13".into();
        q.minimum = 0.05;
        q.maximum = 40.0;
        q.plain_value = 0.7;

        let patch = CompiledParameterPatch {
            transaction_id: 10,
            expected_graph_revision: 4,
            parameter_changes: vec![frequency, enabled, q, used],
            bypass_changes: Vec::new(),
            mapping_issues: Vec::new(),
        };
        control.enqueue_patch(&patch).unwrap();
        let transaction = control.pop_transaction().unwrap();
        let semantics: Vec<_> = transaction
            .changes
            .iter()
            .map(|change| change.semantic_field.as_str())
            .collect();
        assert_eq!(semantics, vec!["used", "enabled", "frequency_hz", "q"]);
    }

    #[test]
    fn incomplete_patch_is_rejected_and_undo_swaps_values() {
        let control = RealtimeParameterControl::new();
        let mut patch = CompiledParameterPatch {
            transaction_id: 9,
            expected_graph_revision: 4,
            parameter_changes: vec![change()],
            bypass_changes: vec![CompiledBypassChange {
                target_node_id: "eq-1".into(),
                expected_graph_revision: 4,
                bypassed: true,
                previous_bypassed: false,
            }],
            mapping_issues: vec!["frequency was ambiguous".into()],
        };
        assert!(matches!(
            control.enqueue_patch(&patch),
            Err(ParameterQueueError::Incomplete)
        ));

        patch.mapping_issues.clear();
        let undo = patch.undo_patch(10);
        assert_eq!(undo.parameter_changes[0].plain_value, 0.0);
        assert_eq!(undo.parameter_changes[0].previous_value, -2.0);
        assert!(!undo.bypass_changes[0].bypassed);
    }
}
