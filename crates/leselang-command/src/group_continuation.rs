//! Leserpent group continuation data and product classification, without VM state.

use leserpent_hir::Effect;
use serde::{Deserialize, Serialize};

use crate::{ResultBinding, continuation_schema};

/// Mutable, unchecked saved group data. Serde preserves the released closed wire shape.
///
/// Decoding does not validate IR structure, reservations, fuel, authority or journals.
/// Bound ingress and preflight the binding before classifying; callers must still
/// validate the original plan, request correspondence and durable state before reuse.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupResultBinding {
    pub pending: Effect,
    pub binding: Box<ResultBinding>,
    pub fuel_remaining: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub successor_sequence: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_successor_sequences: Vec<u64>,
}

impl GroupResultBinding {
    /// Classify bounded, preflighted IR only; this does not admit a saved group.
    pub fn is_conditional(&self) -> bool {
        !self.binding.body.is_pure()
            && !self.binding.body.is_result_chain()
            && self.binding.body.is_result_flow()
    }

    /// Recognize a multi-effect chain after structure preflight, without executing it.
    pub fn is_dataflow(&self) -> bool {
        self.binding.body.is_result_chain()
            && self
                .binding
                .body
                .atomic_chain_bound()
                .is_some_and(|bound| bound > 1)
    }

    /// Classify capture shape after structure preflight, not recovery eligibility.
    pub fn captures_successor(&self) -> bool {
        self.binding.body.is_atomic_capture() || self.is_dataflow() || self.is_conditional()
    }

    /// Observe stored sequence numbers in original order, without reserving or validating them.
    pub fn reserved_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.successor_sequence
            .into_iter()
            .chain(self.additional_successor_sequences.iter().copied())
    }

    /// Select the released group schema after structure preflight; no identity or authority check.
    pub fn continuation_schema(&self) -> u32 {
        if self.is_conditional() {
            continuation_schema::GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        } else if self.is_dataflow() {
            continuation_schema::GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
        } else if self.successor_sequence.is_some() && matches!(self.pending, Effect::All { .. }) {
            continuation_schema::PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION
        } else if self.captures_successor() {
            continuation_schema::GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION
        } else if self.successor_sequence.is_some() {
            continuation_schema::GROUP_TAIL_CONTINUATION_SCHEMA_VERSION
        } else {
            continuation_schema::GROUP_RESULT_CONTINUATION_SCHEMA_VERSION
        }
    }
}
