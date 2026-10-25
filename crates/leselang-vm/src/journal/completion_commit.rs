use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_completion_commit::{
    CompletionCommit, DurableCompletionTransaction, StoredEffect,
};
use leselang_command::reference_completion_commit::{
    MemoryCompletionJournal, MemoryCompletionState,
};

impl MemoryCompletionJournal for EphemeralJournal {
    fn sequence_ready(&self, token: &ContinuationToken) -> bool {
        self.sequence_ready(token)
    }
    fn dispatch(&mut self, token: &ContinuationToken) -> Option<MemoryCompletionState<'_>> {
        self.dispatches
            .get_mut(token)
            .map(|dispatch| MemoryCompletionState {
                acknowledged: &mut dispatch.acknowledged,
                lease_expires_at_ms: dispatch.lease_expires_at_ms,
                terminal_step: &mut dispatch.terminal_step,
            })
    }
    fn finalize_group(&mut self, token: &ContinuationToken) -> Result<(), Fault> {
        self.finalize_merge_group(token)
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteCompletionTransaction<'a>(pub(super) rusqlite::Transaction<'a>);

#[cfg(feature = "sqlite-journal")]
impl DurableCompletionTransaction for SqliteCompletionTransaction<'_> {
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
    fn ensure_sequence_ready(&mut self, token: &str) -> Result<(), Fault> {
        ensure_sequence_ready(&self.0, token)
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
            .map_err(|error| journal_error("LSV4012", "failed to complete effect", error))?;
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
            .map_err(|error| journal_error("LSV4012", "failed to acknowledge dispatch", error))?;
        Ok(())
    }
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault> {
        finalize_merge_group(&self.0, token)
    }
    fn commit(self, kind: CompletionCommit) -> Result<(), Fault> {
        self.0.commit().map_err(|error| {
            journal_error(
                "LSV4012",
                match kind {
                    CompletionCommit::Replay => "failed to close journal transaction",
                    CompletionCommit::Completion => "failed to commit completed effect",
                },
                error,
            )
        })
    }
}
