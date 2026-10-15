use crate::{ClockError, checked_clock_add};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchLeasePlanError {
    Clock(ClockError),
    Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchAttemptError {
    CounterExhausted,
    LimitReached,
}

/// Propose the expiry of a new positive-duration dispatch lease. Clock range,
/// duration policy, then future-time arithmetic are checked in that order. A zero
/// host ceiling grants no duration; no wrapping or deadline clamping is performed.
///
/// This is issuance preflight, not validation of an existing lease or completion:
/// a receipt can be valid at the last portable clock even though no new lease can
/// be issued there. It does not read time, select work, reserve capacity or grant
/// execution. Hosts preflight before cleanup/locking, select under their native
/// owner/transaction, and commit against current state. The returned number has
/// no identity, clock-domain proof or replay protection and must not be reused
/// across a yield. Clock monotonicity, deadlines and live authority remain native.
pub fn plan_dispatch_lease(
    now_ms: u64,
    lease_ms: u64,
    max_lease_ms: u64,
) -> Result<u64, DispatchLeasePlanError> {
    checked_clock_add(now_ms, 0).map_err(DispatchLeasePlanError::Clock)?;
    if lease_ms == 0 || lease_ms > max_lease_ms {
        return Err(DispatchLeasePlanError::Duration);
    }
    checked_clock_add(now_ms, lease_ms).map_err(DispatchLeasePlanError::Clock)
}

/// Propose the exact next delivery attempt under an inclusive host ceiling.
/// Numeric exhaustion precedes the ceiling check. Zero grants no attempts, and
/// neither an exhausted nor an already-over-limit counter is reset or saturated.
///
/// Delivery attempts are distinct from semantic retries and pre-admission polls.
/// Call only after eligibility/capacity checks; a refusal or idle queue must not
/// consume a delivery attempt. This pure proposal changes no counter, owns no
/// request and authenticates no observed state. The host checks and commits it
/// atomically with the lease; successful planning alone is not dispatch authority.
pub fn next_dispatch_attempt(current: u32, max_attempts: u32) -> Result<u32, DispatchAttemptError> {
    let next = current
        .checked_add(1)
        .ok_or(DispatchAttemptError::CounterExhausted)?;
    if next > max_attempts {
        return Err(DispatchAttemptError::LimitReached);
    }
    Ok(next)
}
