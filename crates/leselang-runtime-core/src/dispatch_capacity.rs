/// Why an additional dispatch batch cannot be admitted. A batch larger than the
/// configured limit is not retryable pressure, even when the queue is also full.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchAdmissionError {
    BatchTooLarge,
    Backpressured,
}

/// Check pending-dispatch capacity without reserving, publishing or mutating work.
/// Batch size takes precedence over current pressure. An overflowing sum refuses
/// rather than saturating into an apparently full-but-valid `usize::MAX` limit.
/// Zero additional work is accepted only when the observed count is within limit;
/// a zero limit therefore admits only an empty batch into an empty queue.
///
/// The host validates its policy ceilings and counts all outstanding dispatches,
/// including leased, delayed and dependency-blocked work. Previously admitted work
/// is drained, not re-admitted through this check. The observation is not authority
/// or a reservation: check and publish under the same owner or storage transaction,
/// and never reuse an observation across a yield. No storage or lock is provided.
pub fn check_dispatch_admission(
    pending: usize,
    additional: usize,
    limit: usize,
) -> Result<(), DispatchAdmissionError> {
    if additional > limit {
        return Err(DispatchAdmissionError::BatchTooLarge);
    }
    if pending
        .checked_add(additional)
        .is_none_or(|total| total > limit)
    {
        return Err(DispatchAdmissionError::Backpressured);
    }
    Ok(())
}

/// Whether an observed active-lease count leaves no slot, including after a host
/// lowers its limit below current occupancy. Zero slots always means exhausted.
///
/// This does not find eligible work: a queue with no runnable candidate is idle,
/// not backpressured, even when slots are full. Hosts determine eligibility,
/// expiry and current occupancy, then check/claim atomically. Existing leases are
/// not revoked by changing the limit. This function never grants a lease.
pub fn dispatch_lease_capacity_exhausted(active: usize, limit: usize) -> bool {
    active >= limit
}
