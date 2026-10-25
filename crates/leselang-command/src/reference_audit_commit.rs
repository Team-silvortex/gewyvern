//! Reference audited-cancellation policy after native admission, not authority.

use crate::debugger_audit::{audit_matches_record, debugger_audit_record};
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::{
    ContinuationImage, ContinuationToken, DebuggerAuditContext, DebuggerAuditRecord, Step,
};
use leselang_runtime_core::Fault;

pub struct MemoryAudit<'a> {
    pub token: &'a ContinuationToken,
    pub idempotency_key: &'a str,
    pub principal_id: &'a str,
    pub record: &'a DebuggerAuditRecord,
}

pub struct MemoryEffect<'a> {
    pub image: &'a ContinuationImage,
    pub acknowledged: bool,
    pub terminal: Option<&'a Step>,
}

/// Borrowed observations from the original local owner. Cancellation and insertion
/// retain native mutation/clone order. In-memory cancellation may already have
/// mutated dispatches when group finalization fails; no rollback is manufactured.
/// Hooks are synchronous and trusted, and may borrow non-Send local state.
pub trait MemoryAuditJournal {
    fn audit(&self, command_id: &str) -> Option<MemoryAudit<'_>>;
    fn audits(&self) -> impl Iterator<Item = MemoryAudit<'_>>;
    fn effect(&self, token: &ContinuationToken) -> Option<MemoryEffect<'_>>;
    fn cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault>;
    fn insert(
        &mut self,
        image: &ContinuationImage,
        audit: &DebuggerAuditContext,
        record: &DebuggerAuditRecord,
    );
}

/// Exact replay precedes input-image and new-cancellation validation, as before.
/// Only the original command may attribute a completed cancellation. Neither a
/// matching record nor this function bypasses the caller's debugger access checks.
pub fn cancel_memory<Host: MemoryAuditJournal>(
    host: &mut Host,
    image: &ContinuationImage,
    step: &Step,
    audit: &DebuggerAuditContext,
    now_ms: u64,
) -> Result<(Step, DebuggerAuditRecord), Fault> {
    if let Some(existing) = host.audit(audit.command_id.as_str()) {
        if existing.token == &image.token
            && existing.idempotency_key == audit.idempotency_key.as_str()
            && existing.principal_id == audit.principal.id
            && audit_matches_record(audit, existing.record)
        {
            let authoritative = host
                .effect(&image.token)
                .and_then(|effect| effect.terminal.cloned())
                .ok_or_else(|| fault("LSV4032", "debugger audit has no terminal step"))?;
            return Ok((authoritative, existing.record.clone()));
        }
        return Err(fault("LSV4033", "debugger command audit conflicts"));
    }
    if host.audits().any(|existing| {
        existing.principal_id == audit.principal.id
            && existing.idempotency_key == audit.idempotency_key.as_str()
    }) {
        return Err(fault("LSV4033", "debugger idempotency key was reused"));
    }
    let effect = host
        .effect(&image.token)
        .ok_or_else(|| fault("LSV4013", "pending continuation is missing"))?;
    if effect.image != image {
        return Err(fault(
            "LSV4014",
            "continuation image conflicts with journal state",
        ));
    }
    if effect.acknowledged {
        return Err(fault(
            "LSV4033",
            "completed effect has no matching debugger audit",
        ));
    }
    let authoritative = host.cancel(image, step)?;
    let record = debugger_audit_record(audit, now_ms);
    host.insert(image, audit, &record);
    Ok((authoritative, record))
}

pub use crate::reference_audit_read::StoredAudit;
pub use crate::reference_recovery::StoredEffect;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditCommit {
    Replay,
    Cancellation,
}

/// One already-open native transaction. Reads, quota checks, audit insertion,
/// cancellation writes and group finalization share this transaction. Consuming
/// commit retains the native replay/new-write diagnostics. Drop on error/unwind
/// retains native rollback; an uncertain or failed commit must never be retried.
pub trait DurableAuditTransaction {
    fn audit(&mut self, audit: &DebuggerAuditContext) -> Result<Option<StoredAudit>, Fault>;
    fn effect(&mut self, token: &str) -> Result<Option<StoredEffect>, Fault>;
    fn insert(
        &mut self,
        image: &ContinuationImage,
        audit: &DebuggerAuditContext,
        revision: i64,
        observed_at_ms: i64,
    ) -> Result<(), Fault>;
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault>;
    fn complete(&mut self, token: &str, step_bytes: &[u8]) -> Result<(), Fault>;
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault>;
    fn commit(self, kind: AuditCommit) -> Result<(), Fault>;
}

/// Native owners must validate image/cancellation and perform bounded encoding
/// before opening the transaction, supplying those exact encoded buffers here.
/// Fresh audit insertion precedes quota/finalization; replay only decodes the
/// original terminal, without adding broader terminal validation or new writes.
pub fn cancel_durable<Host: DurableAuditTransaction>(
    mut host: Host,
    image: &ContinuationImage,
    step: &Step,
    audit: &DebuggerAuditContext,
    now_ms: u64,
    image_bytes: &[u8],
    step_bytes: &[u8],
) -> Result<(Step, DebuggerAuditRecord), Fault> {
    if let Some((existing_command, token, key, record)) = host.audit(audit)? {
        if existing_command == audit.command_id.as_str()
            && token == image.token.as_str()
            && key == audit.idempotency_key.as_str()
            && audit_matches_record(audit, &record)
        {
            let existing = host
                .effect(image.token.as_str())?
                .ok_or_else(|| fault("LSV4032", "debugger audit effect is missing"))?;
            let authoritative = decode_journal_value(
                &existing
                    .terminal_step
                    .ok_or_else(|| fault("LSV4032", "debugger audit has no terminal step"))?,
                MAX_JOURNAL_ENTRY_BYTES,
            )?;
            host.commit(AuditCommit::Replay)?;
            return Ok((authoritative, record));
        }
        return Err(fault("LSV4033", "debugger command audit conflicts"));
    }
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
        return Err(fault(
            "LSV4033",
            "completed effect has no matching debugger audit",
        ));
    }
    let record = debugger_audit_record(audit, now_ms);
    let revision = i64::try_from(audit.expected_revision.0)
        .map_err(|_| fault("LSV4033", "debugger audit revision is out of range"))?;
    let observed_at_ms = i64::try_from(now_ms)
        .map_err(|_| fault("LSV4033", "debugger audit clock is out of range"))?;
    host.insert(image, audit, revision, observed_at_ms)?;
    host.ensure_growth(step_bytes.len())?;
    host.complete(image.token.as_str(), step_bytes)?;
    host.finalize_group(image.token.as_str())?;
    host.commit(AuditCommit::Cancellation)?;
    Ok((step.clone(), record))
}

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
