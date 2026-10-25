//! Reference dispatch acknowledgement policy over native journal ownership.

use crate::reference_completion::validate_terminal_step;
use crate::reference_dispatch::validate_observed_lease;
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::{ContinuationToken, DispatchLease, EffectRequest, Step};
use leselang_runtime_core::{DispatchLeaseObservation, Fault};

pub use crate::reference_recovery::{StoredDispatch, StoredEffect};

/// Borrowed original fields, not an admitted lease. Request comparison and lease
/// validation are lazy: already-acknowledged records must bypass both checks.
pub struct MemoryAcknowledgementState<'a> {
    pub request: &'a EffectRequest,
    pub attempt: u32,
    pub retry_count: u32,
    pub ready_at_ms: u64,
    pub acknowledged: &'a mut bool,
    pub lease_expires_at_ms: &'a mut Option<u64>,
    pub terminal_step: &'a mut Option<Step>,
}

pub trait MemoryAcknowledgementJournal {
    fn dispatch(&mut self, token: &ContinuationToken) -> Option<MemoryAcknowledgementState<'_>>;
    fn finalize_group(&mut self, token: &ContinuationToken) -> Result<(), Fault>;
}

/// This low-level primitive validates the proposal even for missing/completed
/// records, then returns that proposal for already-acknowledged records without
/// mutation. Public VM entry owns winner replay. Finalization errors can follow
/// mutation; there is no compensation or callback retry.
pub fn acknowledge_memory<Host: MemoryAcknowledgementJournal + ?Sized>(
    host: &mut Host,
    lease: &DispatchLease,
    now_ms: u64,
    step: &Step,
) -> Result<Step, Fault> {
    validate_terminal_step(step)?;
    let token = &lease.request.continuation.token;
    let state = host
        .dispatch(token)
        .ok_or_else(|| fault("LSV4021", "dispatch lease is unknown"))?;
    if *state.acknowledged {
        return Ok(step.clone());
    }
    validate_observed_lease(
        lease,
        now_ms,
        DispatchLeaseObservation {
            same_request: state.request == &lease.request,
            active: !*state.acknowledged,
            attempt: state.attempt,
            retry_count: state.retry_count,
            ready_at_ms: state.ready_at_ms,
            expires_at_ms: *state.lease_expires_at_ms,
        },
    )?;
    *state.acknowledged = true;
    *state.lease_expires_at_ms = None;
    *state.terminal_step = Some(step.clone());
    host.finalize_group(token)?;
    Ok(step.clone())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcknowledgementCommit {
    Replay,
    Acknowledgement,
}

/// One original immediate transaction after native request/proposal/attempt
/// admission and bounded encoding. Reads retain native counter/history checks.
/// Commit consumes the owner; native storage alone owns rollback. No failure or
/// unwind retries the transaction or compensates for an uncertain commit.
pub trait DurableAcknowledgementTransaction {
    fn effect(&mut self, token: &str) -> Result<Option<StoredEffect>, Fault>;
    fn dispatch(&mut self, token: &str) -> Result<Option<StoredDispatch>, Fault>;
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault>;
    fn complete_effect(&mut self, token: &str, step_bytes: &[u8]) -> Result<(), Fault>;
    fn acknowledge_dispatch(&mut self, token: &str) -> Result<(), Fault>;
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault>;
    fn commit(self, kind: AcknowledgementCommit) -> Result<(), Fault>;
}

/// The owner supplies exact admitted encodings, not arbitrary serialized input.
/// Image equality precedes terminal decoding; completed replay closes the native
/// transaction without dispatch lookup. Otherwise exact request and lease checks
/// precede quota, effect, dispatch, group and commit, in that order. Raw states do
/// not gain recovery admission and these hooks do not grant execution authority.
pub fn acknowledge_durable<Host: DurableAcknowledgementTransaction>(
    mut host: Host,
    lease: &DispatchLease,
    now_ms: u64,
    step: &Step,
    image_bytes: &[u8],
    step_bytes: &[u8],
) -> Result<Step, Fault> {
    let token = lease.request.continuation.token.as_str();
    let existing = host
        .effect(token)?
        .ok_or_else(|| fault("LSV4021", "dispatch continuation is unknown"))?;
    if existing.image != image_bytes {
        return Err(fault(
            "LSV4014",
            "dispatch continuation conflicts with durable journal state",
        ));
    }
    if existing.state == "completed" {
        let bytes = existing
            .terminal_step
            .ok_or_else(|| fault("LSV4007", "completed journal record has no terminal step"))?;
        let authoritative = decode_journal_value(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_terminal_step(&authoritative)?;
        host.commit(AcknowledgementCommit::Replay)?;
        return Ok(authoritative);
    }
    let dispatch = host
        .dispatch(token)?
        .ok_or_else(|| fault("LSV4021", "dispatch lease is unknown"))?;
    let request: EffectRequest = decode_journal_value(&dispatch.request, MAX_JOURNAL_ENTRY_BYTES)?;
    validate_observed_lease(
        lease,
        now_ms,
        DispatchLeaseObservation {
            same_request: request == lease.request,
            active: dispatch.state == "leased",
            attempt: dispatch.attempt,
            retry_count: dispatch.retry_count,
            ready_at_ms: dispatch.ready_at_ms,
            expires_at_ms: dispatch.lease_expires_at_ms,
        },
    )?;
    host.ensure_growth(step_bytes.len())?;
    host.complete_effect(token, step_bytes)?;
    host.acknowledge_dispatch(token)?;
    host.finalize_group(token)?;
    host.commit(AcknowledgementCommit::Acknowledgement)?;
    Ok(step.clone())
}

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
