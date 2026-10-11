use serde::{Deserialize, Serialize};

/// Owned saved effect data with explicit native identity, revision, result,
/// operation and capture schemas. This is unchecked mutable data, not an
/// executable handle, authentic receipt or authority to restore an execution.
///
/// The reference VM specializes this exact type, preserving its field order and
/// optional-field omission/defaults. Native traits are conditional; construction
/// imposes no Clone/Debug/Default/serde/Send/Sync bounds. Native Debug and codecs
/// are trusted and may expose payloads. Adapters bound ingress and validate the
/// original program/site, schema, captures, reply provenance, current authority
/// and durable replay state before constructing a new meter or evaluating work.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        deserialize = "Token: Deserialize<'de>, Revision: Deserialize<'de>, ResultType: Deserialize<'de>, Effect: Deserialize<'de>, Binding: Deserialize<'de>"
    )
)]
pub struct ContinuationFrame<Token, Revision, ResultType, Effect, Binding> {
    pub schema_version: u32,
    pub token: Token,
    pub program_counter: u32,
    pub expected_revision: Option<Revision>,
    pub result_type: ResultType,
    pub pending_effect: Effect,
    pub fuel_remaining: u64,
    pub deadline_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_at_ms: Option<u64>,
    pub max_output_items: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_binding: Option<Box<Binding>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_result: Option<Token>,
}

impl<Token, Revision, ResultType, Effect, Binding>
    ContinuationFrame<Token, Revision, ResultType, Effect, Binding>
{
    /// Copy accounting observations only. This neither authenticates the saved
    /// grant nor constructs/refills a Fuel meter or changes native saved data.
    pub fn resource_budget(&self) -> ResourceBudget {
        ResourceBudget {
            fuel_remaining: self.fuel_remaining,
            deadline_ms: self.deadline_ms,
            deadline_at_ms: self.deadline_at_ms,
            max_output_items: self.max_output_items,
        }
    }
}

/// Persisted accounting observations, never a live meter or implicit grant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudget {
    pub fuel_remaining: u64,
    pub deadline_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_at_ms: Option<u64>,
    pub max_output_items: usize,
}

/// Inclusive host-owned ceilings. Zero fuel/output ceilings admit zero values;
/// a relative or present absolute deadline must always be nonzero. No defaults,
/// saturation, normalization or coupling to a product clock domain are supplied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceBudgetLimits {
    pub max_fuel: u64,
    pub max_deadline_ms: u64,
    pub max_deadline_at_ms: u64,
    pub max_output_items: usize,
}

/// Allocation-free closed diagnostics do not expose input or invoke native code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceBudgetError {
    Fuel,
    Deadline,
    AbsoluteDeadline,
    Output,
}

impl ResourceBudget {
    /// Pure validation in fuel, relative deadline, absolute deadline, output
    /// order. Success is an instant metadata check, not provenance, a fuel grant,
    /// a current deadline check or an execution/replay certificate.
    pub fn validate(&self, limits: ResourceBudgetLimits) -> Result<(), ResourceBudgetError> {
        if self.fuel_remaining > limits.max_fuel {
            return Err(ResourceBudgetError::Fuel);
        }
        if self.deadline_ms == 0 || self.deadline_ms > limits.max_deadline_ms {
            return Err(ResourceBudgetError::Deadline);
        }
        if self
            .deadline_at_ms
            .is_some_and(|value| value == 0 || value > limits.max_deadline_at_ms)
        {
            return Err(ResourceBudgetError::AbsoluteDeadline);
        }
        if self.max_output_items > limits.max_output_items {
            return Err(ResourceBudgetError::Output);
        }
        Ok(())
    }
}
