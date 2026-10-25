use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_audit_commit::{
    AuditCommit, DurableAuditTransaction, StoredAudit, StoredEffect,
};
use leselang_command::reference_audit_commit::{MemoryAudit, MemoryAuditJournal, MemoryEffect};

impl EphemeralDebuggerAudit {
    fn as_view(&self) -> MemoryAudit<'_> {
        MemoryAudit {
            token: &self.token,
            idempotency_key: &self.idempotency_key,
            principal_id: &self.principal_id,
            record: &self.record,
        }
    }
}

impl MemoryAuditJournal for EphemeralJournal {
    fn audit(&self, command_id: &str) -> Option<MemoryAudit<'_>> {
        self.debugger_audits
            .get(command_id)
            .map(EphemeralDebuggerAudit::as_view)
    }
    fn audits(&self) -> impl Iterator<Item = MemoryAudit<'_>> {
        self.debugger_audits
            .values()
            .map(EphemeralDebuggerAudit::as_view)
    }
    fn effect(&self, token: &ContinuationToken) -> Option<MemoryEffect<'_>> {
        self.dispatches.get(token).map(|dispatch| MemoryEffect {
            image: &dispatch.request.continuation,
            acknowledged: dispatch.acknowledged,
            terminal: dispatch.terminal_step.as_ref(),
        })
    }
    fn cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        EphemeralJournal::cancel(self, image, step)
    }
    fn insert(
        &mut self,
        image: &ContinuationImage,
        audit: &DebuggerAuditContext,
        record: &DebuggerAuditRecord,
    ) {
        self.debugger_audits.insert(
            audit.command_id.as_str().to_string(),
            EphemeralDebuggerAudit {
                token: image.token.clone(),
                idempotency_key: audit.idempotency_key.as_str().to_string(),
                principal_id: audit.principal.id.clone(),
                record: record.clone(),
            },
        );
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteAuditTransaction<'a>(pub(super) rusqlite::Transaction<'a>);

#[cfg(feature = "sqlite-journal")]
impl DurableAuditTransaction for SqliteAuditTransaction<'_> {
    fn audit(&mut self, audit: &DebuggerAuditContext) -> Result<Option<StoredAudit>, Fault> {
        load_debugger_audit(&self.0, audit)
    }
    fn effect(&mut self, token: &str) -> Result<Option<StoredEffect>, Fault> {
        Ok(load_record(&self.0, token)?.map(|record| StoredEffect {
            image: record.image,
            state: record.state,
            terminal_step: record.terminal_step,
        }))
    }
    fn insert(
        &mut self,
        image: &ContinuationImage,
        audit: &DebuggerAuditContext,
        revision: i64,
        observed_at_ms: i64,
    ) -> Result<(), Fault> {
        self.0
            .execute(
                "INSERT INTO vm_debugger_audit(
                    command_id, principal_id, idempotency_key, origin, session_id,
                    expected_revision, observed_at_ms, continuation_token
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    audit.command_id.as_str(),
                    audit.principal.id.as_str(),
                    audit.idempotency_key.as_str(),
                    command_origin_label(audit.origin),
                    audit.session_id.as_str(),
                    revision,
                    observed_at_ms,
                    image.token.as_str(),
                ],
            )
            .map_err(|error| {
                journal_error(
                    "LSV4033",
                    "failed to store debugger cancellation audit",
                    error,
                )
            })?;
        Ok(())
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        ensure_growth(&self.0, additional)
    }
    fn complete(&mut self, token: &str, step_bytes: &[u8]) -> Result<(), Fault> {
        complete_terminal_record(&self.0, token, step_bytes)
    }
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault> {
        finalize_merge_group(&self.0, token)
    }
    fn commit(self, kind: AuditCommit) -> Result<(), Fault> {
        self.0.commit().map_err(|error| {
            journal_error(
                "LSV4032",
                match kind {
                    AuditCommit::Replay => "failed to close debugger audit replay",
                    AuditCommit::Cancellation => "failed to commit debugger cancellation audit",
                },
                error,
            )
        })
    }
}
