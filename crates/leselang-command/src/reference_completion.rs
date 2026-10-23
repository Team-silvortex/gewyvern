//! Released terminal receipt policy, without journal state or settlement authority.
//!
//! These checks preserve the journal's legacy validation order and diagnostics.
//! They do not authenticate a receipt, prove a live cancellation/deadline, enforce
//! a per-execution output budget, restore a graph or commit a state transition.
//! Raw input bounds and current owner/lease/group checks stay with the caller.

use leselang_runtime_core::Fault;

use crate::reference_request::DEFAULT_MAX_OUTPUT_ITEMS;
use crate::reference_value::validate_value;
use crate::retry_policy::{MAX_SEMANTIC_RETRIES, validate_effect_error};
use crate::{
    BranchOutcome, Cancellation, CancellationReason, ContinuationImage, MAX_MERGE_BRANCHES, Step,
};

/// Inspect a bounded prefix in order, stopping before any later observation.
/// This is a summary of original terminal data, not permission to dispatch or settle.
pub fn sequence_stop<'a>(steps: impl IntoIterator<Item = &'a Step>) -> Result<Option<Step>, Fault> {
    use leselang_runtime_core::{OrderedOutcomeSummary, OutcomeOrder};
    let invalid = || {
        completion_fault(
            "LSV4007",
            "sequential result observations exceed runtime bounds",
        )
    };
    let overflow = |_, _| {
        Step::Fault(completion_fault(
            "LSV2404",
            "sequential output exceeds runtime bounds",
        ))
    };
    let mut outcomes = OrderedOutcomeSummary::new(
        OutcomeOrder::Sequential,
        MAX_MERGE_BRANCHES,
        DEFAULT_MAX_OUTPUT_ITEMS,
    )
    .map_err(|_| invalid())?;
    for step in steps {
        validate_terminal_step(step)?;
        if let Step::Done(value) = step {
            outcomes
                .observe_success(validate_value(value, 0)?, overflow)
                .map_err(|_| invalid())?;
        } else {
            outcomes.observe_stop(step.clone()).map_err(|_| invalid())?;
        }
        if outcomes.has_stop().map_err(|_| invalid())? {
            return Ok(outcomes.finish(overflow).map_err(|_| invalid())?.first_stop);
        }
    }
    Ok(outcomes.finish(overflow).map_err(|_| invalid())?.first_stop)
}

/// Move an admitted terminal payload unchanged; no branch membership is proven.
pub fn terminal_step_outcome(step: Step) -> Result<BranchOutcome, Fault> {
    validate_terminal_step(&step)?;
    match step {
        Step::Done(value) => Ok(BranchOutcome::Value(value)),
        Step::Cancelled(cancellation) => Ok(BranchOutcome::Cancelled(cancellation)),
        Step::Failed(failure) => Ok(BranchOutcome::Failed(failure)),
        Step::Fault(fault) => Ok(BranchOutcome::Fault(fault)),
        Step::Effect(_) | Step::Effects(_) | Step::Waiting(_) | Step::Yield(_) => Err(
            completion_fault("LSV4007", "merge branch completion is not terminal"),
        ),
    }
}

/// Preserve journal depth 1, nonempty cancellation identities and code-only faults.
/// This deliberately differs from the stricter structured branch-outcome policy.
pub fn validate_terminal_step(step: &Step) -> Result<(), Fault> {
    match step {
        Step::Done(value) if validate_value(value, 1).is_ok() => Ok(()),
        Step::Cancelled(cancellation)
            if !cancellation.continuation.as_str().is_empty()
                && cancellation.observed_at_ms <= i64::MAX as u64 =>
        {
            Ok(())
        }
        Step::Failed(failure)
            if failure.retry_count <= MAX_SEMANTIC_RETRIES
                && validate_effect_error(&failure.error).is_ok() =>
        {
            Ok(())
        }
        Step::Fault(fault) if !fault.code.is_empty() && fault.code.len() <= 32 => Ok(()),
        Step::Done(_) => Err(completion_fault(
            "LSV4007",
            "completed journal output exceeds runtime limit",
        )),
        Step::Effect(_) | Step::Effects(_) | Step::Waiting(_) | Step::Yield(_) => Err(
            completion_fault("LSV4007", "journal completion must be a terminal step"),
        ),
        Step::Fault(_) | Step::Cancelled(_) | Step::Failed(_) => Err(completion_fault(
            "LSV4007",
            "journal fault has an invalid diagnostic code",
        )),
    }
}

/// Check terminal shape before exact token equality, not current cancellation rights.
pub fn validate_cancellation_step(image: &ContinuationImage, step: &Step) -> Result<(), Fault> {
    validate_terminal_step(step)?;
    match step {
        Step::Cancelled(cancellation) if cancellation.continuation == image.token => Ok(()),
        _ => Err(completion_fault(
            "LSV4007",
            "cancellation does not match continuation image",
        )),
    }
}

/// Construct deadline observation data; the owner must prove the deadline is due.
pub fn deadline_cancellation(image: &ContinuationImage, observed_at_ms: u64) -> Step {
    Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason: CancellationReason::DeadlineExceeded,
        observed_at_ms,
    })
}

fn completion_fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
