/// Caller-supplied lease metadata, not an authenticated or replayable handle.
/// Request identity, counter bounds and the clock domain remain host contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchLeaseStamp {
    pub attempt: u32,
    pub retry_count: u32,
    pub expires_at_ms: u64,
}

/// Current observations from the same native owner/transaction as settlement.
/// `same_request` must cover the complete native request, including provenance;
/// `active` excludes cancelled, completed, unleased and superseded work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchLeaseObservation {
    pub same_request: bool,
    pub active: bool,
    pub attempt: u32,
    pub retry_count: u32,
    pub ready_at_ms: u64,
    pub expires_at_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchLeaseError {
    Superseded,
    Expired,
}

/// Check current ownership before expiration, without mutation or callbacks.
/// An exact expiry-time receipt remains valid only while this attempt is still
/// current. A host may re-lease at that instant; whichever transaction commits
/// first wins. A superseded receipt must not become merely an expiration error.
///
/// This pure comparison neither authenticates observations nor reserves a lease.
/// Hosts validate clock/counter bounds, identity and authority, and atomically
/// check/settle against current storage. Completed/cancelled receipt replay is a
/// separate host decision before this check, never a fresh execution grant.
/// Success is not permission to reuse an observation after a yield or mutation.
/// No allocation, codec, dispatch, retry, fuel or durable state is owned here.
pub fn validate_dispatch_lease(
    submitted: DispatchLeaseStamp,
    current: DispatchLeaseObservation,
    now_ms: u64,
) -> Result<(), DispatchLeaseError> {
    if !current.same_request
        || !current.active
        || current.attempt != submitted.attempt
        || current.retry_count != submitted.retry_count
        || current.ready_at_ms > now_ms
        || current.expires_at_ms != Some(submitted.expires_at_ms)
    {
        return Err(DispatchLeaseError::Superseded);
    }
    if submitted.expires_at_ms < now_ms {
        return Err(DispatchLeaseError::Expired);
    }
    Ok(())
}
