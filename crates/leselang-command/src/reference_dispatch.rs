//! Static reference-product dispatch policy, not a scheduler or journal.
//!
//! Shared numeric kernels remain host-neutral. These product limits and diagnostic
//! mappings neither authenticate observations nor read time, reserve capacity,
//! mutate counters, settle receipts or persist retries. Live request identity,
//! deadline checks, replay and atomic check/commit remain with the owning runtime.

use leselang_runtime_core::{
    ClockError, DispatchAttemptError, DispatchLeaseError, DispatchLeaseObservation,
    DispatchLeasePlanError, DispatchLeaseStamp, Fault, RetrySchedulePoint, checked_clock_add,
    next_dispatch_attempt, plan_dispatch_lease, validate_dispatch_lease, validate_retry_schedule,
};

use crate::retry_policy::{MAX_SEMANTIC_RETRIES, validate_effect_error};
use crate::{DispatchLease, RetrySchedule};

pub const DEFAULT_DISPATCH_LEASE_MS: u64 = 30_000;
pub const MAX_DISPATCH_LEASE_MS: u64 = 5 * 60 * 1_000;
pub const MAX_DISPATCH_ATTEMPTS: u32 = 100;

/// Validate numeric retry progression and error shape, not retryability or lease authority.
pub fn validate_proposed_retry(
    current_retry_count: u32,
    schedule: &RetrySchedule,
    now_ms: u64,
) -> Result<(), Fault> {
    if validate_retry_schedule(
        current_retry_count,
        RetrySchedulePoint {
            retry_count: schedule.retry_count,
            ready_at_ms: schedule.ready_at_ms,
        },
        now_ms,
        MAX_SEMANTIC_RETRIES,
    )
    .is_err()
        || validate_effect_error(&schedule.error).is_err()
    {
        return Err(dispatch_fault("LSV4026", "retry schedule is invalid"));
    }
    Ok(())
}

/// Compare caller-supplied observations. The owner must read and settle atomically;
/// success does not authorize reuse after yielding or a state change.
pub fn validate_observed_lease(
    lease: &DispatchLease,
    now_ms: u64,
    observation: DispatchLeaseObservation,
) -> Result<(), Fault> {
    validate_dispatch_lease(
        DispatchLeaseStamp {
            attempt: lease.attempt,
            retry_count: lease.retry_count,
            expires_at_ms: lease.lease_expires_at_ms,
        },
        observation,
        now_ms,
    )
    .map_err(|error| match error {
        DispatchLeaseError::Superseded => {
            dispatch_fault("LSV4022", "dispatch lease has been superseded")
        }
        DispatchLeaseError::Expired => dispatch_fault("LSV4023", "dispatch lease has expired"),
    })
}

/// Preflight issuance without reserving work or cleaning up expired executions.
pub fn validate_lease_clock(now_ms: u64, lease_ms: u64) -> Result<(), Fault> {
    lease_expiration(now_ms, lease_ms).map(|_| ())
}

/// Validate an observation clock without requiring room for a future lease.
pub fn validate_dispatch_clock(now_ms: u64) -> Result<(), Fault> {
    // Observing or completing work does not require a representable future lease.
    checked_clock_add(now_ms, 0)
        .map(|_| ())
        .map_err(|_| dispatch_fault("LSV4015", "dispatch clock is out of range"))
}

/// Propose an expiry under product ceilings without granting a lease.
pub fn lease_expiration(now_ms: u64, lease_ms: u64) -> Result<u64, Fault> {
    plan_dispatch_lease(now_ms, lease_ms, MAX_DISPATCH_LEASE_MS).map_err(|error| match error {
        DispatchLeasePlanError::Clock(ClockError::OutOfRange) => {
            dispatch_fault("LSV4015", "dispatch clock is out of range")
        }
        DispatchLeasePlanError::Clock(ClockError::Overflow) => {
            dispatch_fault("LSV4015", "dispatch lease expiration is out of range")
        }
        DispatchLeasePlanError::Duration => dispatch_fault(
            "LSV4015",
            format!("dispatch lease must be between 1 and {MAX_DISPATCH_LEASE_MS} ms"),
        ),
    })
}

/// Propose the next delivery attempt without consuming it.
pub fn next_attempt(current: u32) -> Result<u32, Fault> {
    next_dispatch_attempt(current, MAX_DISPATCH_ATTEMPTS).map_err(|error| match error {
        DispatchAttemptError::CounterExhausted => {
            dispatch_fault("LSV4017", "dispatch attempt counter is exhausted")
        }
        DispatchAttemptError::LimitReached => dispatch_fault(
            "LSV4017",
            format!("dispatch attempt limit {MAX_DISPATCH_ATTEMPTS} reached"),
        ),
    })
}

fn dispatch_fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
