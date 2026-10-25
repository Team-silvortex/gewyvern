use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_cancellation_commit::{
    CancellationCommit, DurableCancellationTransaction, StoredEffect,
};
use leselang_command::reference_cancellation_commit::{
    MemoryCancellationJournal, MemoryCancellationState,
};

impl MemoryCancellationJournal for EphemeralJournal {
    fn dispatch(&mut self, token: &ContinuationToken) -> Option<MemoryCancellationState<'_>> {
        self.dispatches
            .get_mut(token)
            .map(|dispatch| MemoryCancellationState {
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
pub(super) struct SqliteCancellationTransaction<'a>(pub(super) rusqlite::Transaction<'a>);

#[cfg(feature = "sqlite-journal")]
impl DurableCancellationTransaction for SqliteCancellationTransaction<'_> {
    fn effect(&mut self, token: &str) -> Result<Option<StoredEffect>, Fault> {
        Ok(load_record(&self.0, token)?.map(|record| StoredEffect {
            image: record.image,
            state: record.state,
            terminal_step: record.terminal_step,
        }))
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
    fn commit(self, kind: CancellationCommit) -> Result<(), Fault> {
        self.0.commit().map_err(|error| {
            journal_error(
                "LSV4025",
                match kind {
                    CancellationCommit::Replay => "failed to close cancellation transaction",
                    CancellationCommit::Cancellation => "failed to commit effect cancellation",
                },
                error,
            )
        })
    }
}
