//! Ordinary cancellation journal policy, distinct from audited cancellation.

use crate::reference_completion::{validate_cancellation_step, validate_terminal_step};
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::{ContinuationImage, ContinuationToken, Step};
use leselang_runtime_core::Fault;

pub use crate::reference_recovery::StoredEffect;

/// Exclusive native slots; no map copies, leases or independent mutation rights.
pub struct MemoryCancellationState<'a> {
    pub acknowledged: &'a mut bool,
    pub lease_expires_at_ms: &'a mut Option<u64>,
    pub terminal_step: &'a mut Option<Step>,
}

/// Trusted local owner after entry-level authority and replay checks. Missing
/// dispatches still enter group finalization. A finalization error can follow
/// mutation of these slots; the adapter neither undoes it nor retries the hook.
pub trait MemoryCancellationJournal {
    fn dispatch(&mut self, token: &ContinuationToken) -> Option<MemoryCancellationState<'_>>;
    fn finalize_group(&mut self, token: &ContinuationToken) -> Result<(), Fault>;
}

pub fn cancel_memory<Host: MemoryCancellationJournal + ?Sized>(
    host: &mut Host,
    image: &ContinuationImage,
    step: &Step,
) -> Result<Step, Fault> {
    validate_cancellation_step(image, step)?;
    if let Some(state) = host.dispatch(&image.token) {
        *state.acknowledged = true;
        *state.lease_expires_at_ms = None;
        *state.terminal_step = Some(step.clone());
    }
    host.finalize_group(&image.token)?;
    Ok(step.clone())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationCommit {
    Replay,
    Cancellation,
}

/// One native immediate transaction, already opened after input validation and
/// bounded encoding. Commit consumes the owner; uncommitted native state rolls
/// back on drop. No error/unwind retries or compensates for an uncertain commit.
pub trait DurableCancellationTransaction {
    fn effect(&mut self, token: &str) -> Result<Option<StoredEffect>, Fault>;
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault>;
    fn complete(&mut self, token: &str, step_bytes: &[u8]) -> Result<(), Fault>;
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault>;
    fn commit(self, kind: CancellationCommit) -> Result<(), Fault>;
}

/// Owners supply the exact admitted image/proposal encodings. Image matching
/// precedes replay decoding; ordinary replay retains terminal admission, unlike
/// the audit replay path. A saved non-completed state retains the released write
/// route, not new recovery admission. This function does not confer authority.
pub fn cancel_durable<Host: DurableCancellationTransaction>(
    mut host: Host,
    image: &ContinuationImage,
    step: &Step,
    image_bytes: &[u8],
    step_bytes: &[u8],
) -> Result<Step, Fault> {
    let existing = host
        .effect(image.token.as_str())?
        .ok_or_else(|| fault("LSV4013", "pending continuation is missing"))?;
    if existing.image != image_bytes {
        return Err(fault(
            "LSV4014",
            "continuation image conflicts with durable journal state",
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
        host.commit(CancellationCommit::Replay)?;
        return Ok(authoritative);
    }
    host.ensure_growth(step_bytes.len())?;
    host.complete(image.token.as_str(), step_bytes)?;
    host.finalize_group(image.token.as_str())?;
    host.commit(CancellationCommit::Cancellation)?;
    Ok(step.clone())
}

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
