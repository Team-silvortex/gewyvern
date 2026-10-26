use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_retry_commit::{
    DurableRetryTransaction, RetryCommit, StoredEffect,
};
use leselang_command::reference_retry_commit::{MemoryRetryJournal, MemoryRetryState};

impl MemoryRetryJournal for EphemeralJournal {
    fn dispatch(&mut self, lease: &DispatchLease) -> Option<MemoryRetryState<'_>> {
        self.dispatches
            .get_mut(&lease.request.continuation.token)
            .map(|dispatch| MemoryRetryState {
                observation: DispatchLeaseObservation {
                    same_request: dispatch.request == lease.request,
                    active: !dispatch.acknowledged,
                    attempt: dispatch.attempt,
                    retry_count: dispatch.retry_count,
                    ready_at_ms: dispatch.ready_at_ms,
                    expires_at_ms: dispatch.lease_expires_at_ms,
                },
                retry_count: &mut dispatch.retry_count,
                ready_at_ms: &mut dispatch.ready_at_ms,
                last_error: &mut dispatch.last_error,
                lease_expires_at_ms: &mut dispatch.lease_expires_at_ms,
                acknowledged: &mut dispatch.acknowledged,
                terminal_step: &mut dispatch.terminal_step,
            })
    }
    fn finalize_group(&mut self, token: &ContinuationToken) -> Result<(), Fault> {
        self.finalize_merge_group(token)
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteRetryTransaction<'a>(pub(super) rusqlite::Transaction<'a>);

#[cfg(feature = "sqlite-journal")]
impl DurableRetryTransaction for SqliteRetryTransaction<'_> {
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
    fn encode_error(&mut self, error: &EffectError) -> Result<Vec<u8>, Fault> {
        encode_json_capped(error, 2_048, "effect error")
    }
    fn encode_terminal(&mut self, step: &Step) -> Result<Vec<u8>, Fault> {
        encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        ensure_growth(&self.0, additional)
    }
    fn schedule(
        &mut self,
        token: &str,
        retry_count: u32,
        ready_at_ms: i64,
        error_bytes: &[u8],
    ) -> Result<(), Fault> {
        self.0
            .execute(
                "UPDATE vm_dispatches
                         SET state = 'ready', retry_count = ?2, ready_at_ms = ?3,
                             last_error = ?4, lease_expires_at_ms = NULL
                         WHERE token = ?1",
                params![token, i64::from(retry_count), ready_at_ms, error_bytes],
            )
            .map_err(|error| journal_error("LSV4026", "failed to schedule effect retry", error))?;
        Ok(())
    }
    fn complete(&mut self, token: &str, step_bytes: &[u8]) -> Result<(), Fault> {
        complete_terminal_record(&self.0, token, step_bytes)
    }
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault> {
        finalize_merge_group(&self.0, token)
    }
    fn commit(self, kind: RetryCommit) -> Result<(), Fault> {
        self.0.commit().map_err(|error| {
            journal_error(
                "LSV4026",
                match kind {
                    RetryCommit::Replay => "failed to close retry transaction",
                    RetryCommit::Retry => "failed to commit effect retry",
                },
                error,
            )
        })
    }
}
