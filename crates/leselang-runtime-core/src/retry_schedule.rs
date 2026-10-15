use crate::{ClockError, MAX_CLOCK_MS, capped_exponential_delay, checked_clock_add};

/// Explicit host-selected semantic retry budget and positive capped delay.
/// This is not the lease redelivery count or pre-admission attempt policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryTiming {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

/// Numeric proposal only, without a request, native error or execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetrySchedulePoint {
    pub retry_count: u32,
    pub ready_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryPlanError {
    Clock(ClockError),
    Timing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryScheduleError {
    Counter,
    ReadyTime,
}

/// Plan one semantic retry using the shared portable clock and capped backoff.
/// Checks clock, positive/ordered delays, then budget, in that order. `None`
/// means exhausted, including when the observed count already exceeds the grant;
/// it never resets the counter. The first retry uses the base delay.
///
/// The host classifies retryable errors, validates its policy ceilings and chooses
/// a terminal result on exhaustion. This function neither reads a clock nor owns
/// state, payloads, fuel, deadlines, timers, leases or persistence. A proposed time
/// beyond an execution deadline is not silently clamped: deadline handling stays
/// with the host. Planning success is not permission to dispatch or retry.
pub fn plan_retry_schedule(
    current_retry_count: u32,
    now_ms: u64,
    timing: RetryTiming,
) -> Result<Option<RetrySchedulePoint>, RetryPlanError> {
    checked_clock_add(now_ms, 0).map_err(RetryPlanError::Clock)?;
    if timing.base_delay_ms == 0 || timing.max_delay_ms < timing.base_delay_ms {
        return Err(RetryPlanError::Timing);
    }
    let Some(retry_count) = current_retry_count
        .checked_add(1)
        .filter(|next| *next <= timing.max_retries)
    else {
        return Ok(None);
    };
    let delay = capped_exponential_delay(
        timing.base_delay_ms,
        timing.max_delay_ms,
        current_retry_count,
    );
    let ready_at_ms = checked_clock_add(now_ms, delay).map_err(RetryPlanError::Clock)?;
    Ok(Some(RetrySchedulePoint {
        retry_count,
        ready_at_ms,
    }))
}

/// Check an already-proposed schedule against the current native count and grant.
/// Exact next count and inclusive retry ceiling precede strictly future portable
/// ready-time checks. Overflow, skipped counts and time repair are refused.
///
/// This does not prove delay/policy provenance, error classification, request
/// identity or lease currency. Hosts validate those and the native error, then
/// check/commit within the same owner or transaction. Never reuse observations
/// across a yield; terminal receipt replay and crash settlement remain host-owned.
pub fn validate_retry_schedule(
    current_retry_count: u32,
    proposed: RetrySchedulePoint,
    now_ms: u64,
    max_retries: u32,
) -> Result<(), RetryScheduleError> {
    if current_retry_count.checked_add(1) != Some(proposed.retry_count)
        || proposed.retry_count > max_retries
    {
        return Err(RetryScheduleError::Counter);
    }
    if proposed.ready_at_ms <= now_ms || proposed.ready_at_ms > MAX_CLOCK_MS {
        return Err(RetryScheduleError::ReadyTime);
    }
    Ok(())
}
