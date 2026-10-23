//! Released product value and branch-outcome admission, without execution state.
//!
//! These checks borrow protocol data and retain the original diagnostic order.
//! They do not bound raw ingress bytes, authenticate receipts, match a pending
//! operation, enforce a live execution budget, cancel work or commit a result.

use leselang_runtime_core::Fault;

use crate::continuation_identity::valid_continuation_token;
use crate::reference_request::DEFAULT_MAX_OUTPUT_ITEMS;
use crate::retry_policy::{MAX_SEMANTIC_RETRIES, validate_effect_error};
use crate::{BranchOutcome, MAX_MERGE_BRANCHES, Value, ValueBoundsError, ValueValidationLimits};

pub const MAX_STRUCTURED_VALUE_DEPTH: usize = 16;

/// Inclusive product ceilings, not an authenticated grant or per-execution budget.
pub fn value_validation_limits() -> ValueValidationLimits {
    ValueValidationLimits {
        max_depth: MAX_STRUCTURED_VALUE_DEPTH,
        max_fields: MAX_MERGE_BRANCHES,
        max_query_items: DEFAULT_MAX_OUTPUT_ITEMS,
    }
}

/// Count bounded values without cloning, normalization or serialization.
/// Starting depth belongs to the caller: raw replies use 0, merged values 1,
/// and branch values 2. Earlier child faults still precede later name faults.
pub fn validate_value(value: &Value, depth: usize) -> Result<usize, Fault> {
    crate::validate_value_items(value, depth, value_validation_limits(), |name| {
        crate::valid_merge_branch_name(name, crate::MAX_MERGE_BRANCH_NAME_BYTES)
    })
    .map_err(value_bounds_fault)
}

/// Shared released diagnostics for the VM's separate typed reply-admission gate.
pub fn value_bounds_fault(error: ValueBoundsError) -> Fault {
    Fault {
        code: "LSV2403".into(),
        message: match error {
            ValueBoundsError::Nesting => "structured value nesting exceeds runtime bounds",
            ValueBoundsError::Fields => "structured value fields are invalid",
            ValueBoundsError::Value => "structured value exceeds runtime bounds",
        }
        .into(),
    }
}

/// Check one untrusted terminal record, not completion-set membership or provenance.
/// Cancellation accepts legacy lexical tokens, not only canonical sequences.
/// Terminal records contribute zero output items; values retain merge depth 2.
pub fn validate_branch_outcome(outcome: &BranchOutcome) -> Result<usize, Fault> {
    match outcome {
        BranchOutcome::Value(value) => validate_value(value, 2),
        BranchOutcome::Cancelled(cancellation)
            if valid_continuation_token(&cancellation.continuation)
                && cancellation.observed_at_ms <= i64::MAX as u64 =>
        {
            Ok(0)
        }
        BranchOutcome::Failed(failure)
            if failure.retry_count <= MAX_SEMANTIC_RETRIES
                && validate_effect_error(&failure.error).is_ok() =>
        {
            Ok(0)
        }
        BranchOutcome::Fault(fault)
            if !fault.code.is_empty() && fault.code.len() <= 32 && fault.message.len() <= 4_096 =>
        {
            Ok(0)
        }
        _ => Err(Fault {
            code: "LSV2403".to_string(),
            message: "structured merge branch outcome is invalid".to_string(),
        }),
    }
}
