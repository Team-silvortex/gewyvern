use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_acknowledgement_commit::{
    AcknowledgementCommit, DurableAcknowledgementTransaction, StoredEffect,
};
use leselang_command::reference_acknowledgement_commit::{
    MemoryAcknowledgementJournal, MemoryAcknowledgementState,
};

impl MemoryAcknowledgementJournal for EphemeralJournal {
    fn dispatch(&mut self, token: &ContinuationToken) -> Option<MemoryAcknowledgementState<'_>> {
        self.dispatches
            .get_mut(token)
            .map(|dispatch| MemoryAcknowledgementState {
                request: &dispatch.request,
                attempt: dispatch.attempt,
                retry_count: dispatch.retry_count,
                ready_at_ms: dispatch.ready_at_ms,
                acknowledged: &mut dispatch.acknowledged,
                lease_expires_at_ms: &mut dispatch.lease_expires_at_ms,
                terminal_step: &mut dispatch.terminal_step,
            })
    }
    fn finalize_group(&mut self, token: &ContinuationToken) -> Result<(), Fault> {
        self.finalize_merge_group(token)
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteAcknowledgementTransaction<'a>(pub(super) rusqlite::Transaction<'a>);

#[cfg(feature = "sqlite-journal")]
impl DurableAcknowledgementTransaction for SqliteAcknowledgementTransaction<'_> {
    fn effect(&mut self, token: &str) -> Result<Option<StoredEffect>, Fault> {
        Ok(load_record(&self.0, token)?.map(|record| StoredEffect {
            image: record.image,
            state: record.state,
            terminal_step: record.terminal_step,
        }))
    }
    fn dispatch(&mut self, token: &str) -> Result<Option<DispatchRecord>, Fault> {
        load_dispatch(&self.0, token)
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        ensure_growth(&self.0, additional)
    }
    fn complete_effect(&mut self, token: &str, step_bytes: &[u8]) -> Result<(), Fault> {
        self.0
            .execute(
                "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
                params![token, step_bytes],
            )
            .map_err(|error| journal_error("LSV4020", "failed to complete dispatch", error))?;
        Ok(())
    }
    fn acknowledge_dispatch(&mut self, token: &str) -> Result<(), Fault> {
        self.0
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'acknowledged', lease_expires_at_ms = NULL
                 WHERE token = ?1",
                [token],
            )
            .map_err(|error| journal_error("LSV4020", "failed to acknowledge dispatch", error))?;
        Ok(())
    }
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault> {
        finalize_merge_group(&self.0, token)
    }
    fn commit(self, kind: AcknowledgementCommit) -> Result<(), Fault> {
        self.0.commit().map_err(|error| {
            journal_error(
                "LSV4020",
                match kind {
                    AcknowledgementCommit::Replay => "failed to close dispatch transaction",
                    AcknowledgementCommit::Acknowledgement => "failed to commit dispatch result",
                },
                error,
            )
        })
    }
}
