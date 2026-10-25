//! Reference retry journal coordination; planning does not grant dispatch rights.

use crate::reference_completion::validate_terminal_step;
use crate::reference_dispatch::{validate_observed_lease, validate_proposed_retry};
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::{ContinuationToken, DispatchLease, EffectError, EffectRequest, RetryDisposition, Step};
use leselang_runtime_core::{DispatchLeaseObservation, Fault};

pub use crate::reference_recovery::{StoredDispatch, StoredEffect};

/// Exclusive native slots and their same-snapshot lease observation. The owner
/// must compare the original request before borrowing, without copying the map.
pub struct MemoryRetryState<'a> {
    pub observation: DispatchLeaseObservation,
    pub retry_count: &'a mut u32,
    pub ready_at_ms: &'a mut u64,
    pub last_error: &'a mut Option<EffectError>,
    pub lease_expires_at_ms: &'a mut Option<u64>,
    pub acknowledged: &'a mut bool,
    pub terminal_step: &'a mut Option<Step>,
}

pub trait MemoryRetryJournal {
    fn dispatch(&mut self, lease: &DispatchLease) -> Option<MemoryRetryState<'_>>;
    fn finalize_group(&mut self, token: &ContinuationToken) -> Result<(), Fault>;
}

/// Lease admission precedes proposal checks, including for acknowledged records.
/// Terminal finalization can fail after mutation; no rollback or retry is added.
pub fn report_memory<Host: MemoryRetryJournal + ?Sized>(
    host: &mut Host,
    lease: &DispatchLease,
    now_ms: u64,
    disposition: &RetryDisposition,
) -> Result<RetryDisposition, Fault> {
    let state = host
        .dispatch(lease)
        .ok_or_else(|| fault("LSV4021", "dispatch lease is unknown"))?;
    validate_observed_lease(lease, now_ms, state.observation)?;
    match disposition {
        RetryDisposition::Scheduled(schedule) => {
            validate_proposed_retry(*state.retry_count, schedule, now_ms)?;
            *state.retry_count = schedule.retry_count;
            *state.ready_at_ms = schedule.ready_at_ms;
            *state.last_error = Some(schedule.error.clone());
            *state.lease_expires_at_ms = None;
        }
        RetryDisposition::Terminal(step) => {
            validate_terminal_step(step)?;
            *state.acknowledged = true;
            *state.lease_expires_at_ms = None;
            *state.terminal_step = Some(step.clone());
        }
    }
    if matches!(disposition, RetryDisposition::Terminal(_)) {
        host.finalize_group(&lease.request.continuation.token)?;
    }
    Ok(disposition.clone())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryCommit {
    Replay,
    Retry,
}

/// One original immediate transaction, opened after request/image admission.
/// Reads retain native decoding of dispatch counters/history. Encoding hooks keep
/// bounded native codecs and diagnostics, without writes. Only native storage
/// owns rollback. Commit consumes its owner; errors/unwinds never cause retries.
pub trait DurableRetryTransaction {
    fn effect(&mut self, token: &str) -> Result<Option<StoredEffect>, Fault>;
    fn dispatch(&mut self, token: &str) -> Result<Option<StoredDispatch>, Fault>;
    fn encode_error(&mut self, error: &EffectError) -> Result<Vec<u8>, Fault>;
    fn encode_terminal(&mut self, step: &Step) -> Result<Vec<u8>, Fault>;
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault>;
    fn schedule(
        &mut self,
        token: &str,
        retry_count: u32,
        ready_at_ms: i64,
        error_bytes: &[u8],
    ) -> Result<(), Fault>;
    fn complete(&mut self, token: &str, step_bytes: &[u8]) -> Result<(), Fault>;
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault>;
    fn commit(self, kind: RetryCommit) -> Result<(), Fault>;
}

/// Match the admitted image before replay. Completed rows return their original
/// terminal before lease/proposal checks; pending rows compare the exact decoded
/// request and lease before any encoding, quota check or mutation. Raw state is
/// not new recovery admission, and these trusted hooks do not confer authority.
pub fn report_durable<Host: DurableRetryTransaction>(
    mut host: Host,
    lease: &DispatchLease,
    now_ms: u64,
    disposition: &RetryDisposition,
    image_bytes: &[u8],
) -> Result<RetryDisposition, Fault> {
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
        let authoritative = decode_journal_value(
            &existing
                .terminal_step
                .ok_or_else(|| fault("LSV4007", "completed journal record has no terminal step"))?,
            MAX_JOURNAL_ENTRY_BYTES,
        )?;
        validate_terminal_step(&authoritative)?;
        host.commit(RetryCommit::Replay)?;
        return Ok(RetryDisposition::Terminal(authoritative));
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
    match disposition {
        RetryDisposition::Scheduled(schedule) => {
            validate_proposed_retry(dispatch.retry_count, schedule, now_ms)?;
            let error_bytes = host.encode_error(&schedule.error)?;
            let ready_at_ms = i64::try_from(schedule.ready_at_ms)
                .map_err(|_| fault("LSV4026", "retry clock is invalid"))?;
            host.ensure_growth(error_bytes.len())?;
            host.schedule(token, schedule.retry_count, ready_at_ms, &error_bytes)?;
        }
        RetryDisposition::Terminal(step) => {
            validate_terminal_step(step)?;
            let step_bytes = host.encode_terminal(step)?;
            host.ensure_growth(step_bytes.len())?;
            host.complete(token, &step_bytes)?;
            host.finalize_group(token)?;
        }
    }
    host.commit(RetryCommit::Retry)?;
    Ok(disposition.clone())
}

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
