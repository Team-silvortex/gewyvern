//! Reference dispatch selection and lease-commit policy over native journal owners.

use crate::continuation_identity::continuation_age_order;
use crate::reference_dispatch::next_attempt;
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::reference_request::validate_effect_request;
use crate::retry_policy::MAX_SEMANTIC_RETRIES;
use crate::{
    ContinuationImage, ContinuationToken, DispatchClaim, DispatchLease, EffectRequest,
    SchedulerLimits, SchedulerPressure,
};
use leselang_runtime_core::Fault;

/// Borrowed scheduling observations only; payloads stay in the native map.
pub struct MemoryClaimCandidate<'a> {
    pub token: &'a ContinuationToken,
    pub attempt: u32,
    pub acknowledged: bool,
    pub ready_at_ms: u64,
    pub lease_expires_at_ms: Option<u64>,
}
pub struct MemoryLeaseState<'a> {
    pub request: &'a EffectRequest,
    pub attempt: &'a mut u32,
    pub retry_count: u32,
    pub lease_expires_at_ms: &'a mut Option<u64>,
}
pub trait MemoryClaimJournal {
    fn candidates(&self) -> impl Iterator<Item = MemoryClaimCandidate<'_>>;
    fn sequence_ready(&self, token: &ContinuationToken) -> bool;
    fn pressure(&self, now_ms: u64, limits: SchedulerLimits) -> SchedulerPressure;
    fn dispatch(&mut self, token: &ContinuationToken) -> Option<MemoryLeaseState<'_>>;
}

/// Native entry checks the lease clock first. Idle precedes backpressure; graph
/// dependencies are queried only for candidates that can improve the current
/// minimum. Clone one winning token, then pressure/lookup/counter checks precede
/// mutation. This low-level memory path adds no request decoding or admission.
pub fn claim_memory<Host: MemoryClaimJournal + ?Sized>(
    host: &mut Host,
    now_ms: u64,
    expires_at_ms: u64,
    limits: SchedulerLimits,
) -> Result<DispatchClaim, Fault> {
    let mut eligible: Option<MemoryClaimCandidate<'_>> = None;
    for candidate in host.candidates() {
        if candidate.acknowledged
            || candidate.ready_at_ms > now_ms
            || candidate
                .lease_expires_at_ms
                .is_some_and(|expires| expires > now_ms)
        {
            continue;
        }
        let already_preferred = eligible.as_ref().is_some_and(|best| {
            best.attempt
                .cmp(&candidate.attempt)
                .then_with(|| continuation_age_order(best.token, candidate.token))
                .is_le()
        });
        if !already_preferred && host.sequence_ready(candidate.token) {
            eligible = Some(candidate);
        }
    }
    let Some(token) = eligible.map(|candidate| candidate.token.clone()) else {
        return Ok(DispatchClaim::Idle);
    };
    let pressure = host.pressure(now_ms, limits);
    if crate::scheduler_policy::scheduler_lease_capacity_exhausted(pressure) {
        return Ok(DispatchClaim::Backpressured(pressure));
    }
    let state = host
        .dispatch(&token)
        .ok_or_else(|| fault("LSV4007", "dispatch disappeared"))?;
    *state.attempt = next_attempt(*state.attempt)?;
    *state.lease_expires_at_ms = Some(expires_at_ms);
    Ok(DispatchClaim::Leased(Box::new(DispatchLease {
        request: state.request.clone(),
        attempt: *state.attempt,
        retry_count: state.retry_count,
        lease_expires_at_ms: expires_at_ms,
    })))
}

/// Raw native query output, not a validated dispatch or lease authority.
pub struct StoredClaimCandidate {
    pub token: String,
    pub request: Vec<u8>,
    pub attempt: i64,
    pub retry_count: i64,
    pub image: Vec<u8>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimCommit {
    CapacityLimited,
    Idle,
    Leased,
}

/// One native immediate transaction after signed clock conversion. Selection SQL,
/// indexed eligibility, graph predecessors, bounded capacity counts, image codec,
/// updates, diagnostics and atomic commit/rollback remain with the native owner.
pub trait DurableClaimTransaction {
    fn capacity_exhausted(&mut self, limit: usize) -> Result<bool, Fault>;
    fn has_candidate(&mut self) -> Result<bool, Fault>;
    fn pressure(
        &mut self,
        now_ms: u64,
        limits: SchedulerLimits,
    ) -> Result<SchedulerPressure, Fault>;
    fn candidate(&mut self) -> Result<Option<StoredClaimCandidate>, Fault>;
    fn encode_image(&mut self, image: &ContinuationImage) -> Result<Vec<u8>, Fault>;
    fn lease(&mut self, token: &str, attempt: u32) -> Result<(), Fault>;
    fn commit(self, reason: ClaimCommit) -> Result<(), Fault>;
}

/// A saturated owner reads eligibility only, never candidate payloads. Empty and
/// capacity-limited outcomes still commit. Decode/request/token/image/counter
/// checks retain their original precedence; only a committed update publishes a
/// lease. Errors and uncertain commits are never retried or compensated here.
pub fn claim_durable<Host: DurableClaimTransaction>(
    mut host: Host,
    now_ms: u64,
    expires_at_ms: u64,
    limits: SchedulerLimits,
) -> Result<DispatchClaim, Fault> {
    if host.capacity_exhausted(limits.max_active_leases)? {
        let claim = if host.has_candidate()? {
            DispatchClaim::Backpressured(host.pressure(now_ms, limits)?)
        } else {
            DispatchClaim::Idle
        };
        host.commit(ClaimCommit::CapacityLimited)?;
        return Ok(claim);
    }
    let Some(candidate) = host.candidate()? else {
        host.commit(ClaimCommit::Idle)?;
        return Ok(DispatchClaim::Idle);
    };
    let request: EffectRequest = decode_journal_value(&candidate.request, MAX_JOURNAL_ENTRY_BYTES)?;
    validate_effect_request(&request)?;
    if request.continuation.token.as_str() != candidate.token {
        return Err(fault(
            "LSV4007",
            "dispatch token does not match effect request",
        ));
    }
    if candidate.image != host.encode_image(&request.continuation)? {
        return Err(fault(
            "LSV4014",
            "dispatch continuation conflicts with durable journal state",
        ));
    }
    let attempt = u32::try_from(candidate.attempt)
        .map_err(|_| fault("LSV4007", "dispatch attempt is invalid"))?;
    let attempt = next_attempt(attempt)?;
    let retry_count = u32::try_from(candidate.retry_count)
        .map_err(|_| fault("LSV4007", "dispatch retry count is invalid"))?;
    if retry_count > MAX_SEMANTIC_RETRIES {
        return Err(fault(
            "LSV4007",
            "dispatch retry count exceeds runtime limit",
        ));
    }
    host.lease(&candidate.token, attempt)?;
    host.commit(ClaimCommit::Leased)?;
    Ok(DispatchClaim::Leased(Box::new(DispatchLease {
        request,
        attempt,
        retry_count,
        lease_expires_at_ms: expires_at_ms,
    })))
}

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
