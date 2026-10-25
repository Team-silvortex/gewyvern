use super::*;
use leselang_command::reference_pending_commit::MemoryPendingJournal;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_pending_commit::{
    DurablePendingTransaction, PendingCommit, StoredEffect,
};

impl MemoryPendingJournal for EphemeralJournal {
    fn request(&self, token: &ContinuationToken) -> Option<&EffectRequest> {
        self.dispatches.get(token).map(|dispatch| &dispatch.request)
    }
    fn check_capacity(&self, limits: SchedulerLimits) -> Result<(), Fault> {
        limits.check_admission(self.scheduler_pressure(0, limits).pending_dispatches, 1)
    }
    fn insert_pending(&mut self, token: ContinuationToken, request: &EffectRequest) {
        self.dispatches.insert(
            token,
            EphemeralDispatch {
                request: request.clone(),
                attempt: 0,
                lease_expires_at_ms: None,
                ready_at_ms: 0,
                retry_count: 0,
                last_error: None,
                acknowledged: false,
                terminal_step: None,
            },
        );
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqlitePendingTransaction<'a>(pub(super) rusqlite::Transaction<'a>);

#[cfg(feature = "sqlite-journal")]
impl DurablePendingTransaction for SqlitePendingTransaction<'_> {
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
    fn record_count(&mut self) -> Result<usize, Fault> {
        journal_record_count(&self.0)
    }
    fn check_capacity(&mut self, limits: SchedulerLimits) -> Result<(), Fault> {
        limits.check_admission(
            sql_scheduler_pressure(&self.0, 0, limits)?.pending_dispatches,
            1,
        )
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        ensure_growth(&self.0, additional)
    }
    fn insert_effect(&mut self, image: &ContinuationImage, bytes: &[u8]) -> Result<(), Fault> {
        self.0
            .execute(
                "INSERT INTO vm_effects(token, state, image, deadline_at_ms, terminal_step)
                 VALUES (?1, 'pending', ?2, ?3, NULL)",
                params![
                    image.token.as_str(),
                    bytes,
                    image
                        .deadline_at_ms
                        .map(i64::try_from)
                        .transpose()
                        .map_err(|_| journal_fault("LSV4010", "effect deadline is out of range"))?
                ],
            )
            .map_err(|error| journal_error("LSV4010", "failed to store pending effect", error))?;
        Ok(())
    }
    fn insert_dispatch(&mut self, token: &str, bytes: &[u8]) -> Result<(), Fault> {
        self.0
            .execute(
                "INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms)
                     VALUES (?1, ?2, 'ready', 0, NULL)",
                params![token, bytes],
            )
            .map_err(|error| journal_error("LSV4010", "failed to store effect dispatch", error))?;
        Ok(())
    }
    fn advance_watermark(&mut self, sequence: u64) -> Result<(), Fault> {
        advance_sequence_watermark(&self.0, sequence).map(|_| ())
    }
    fn commit(self, kind: PendingCommit) -> Result<(), Fault> {
        self.0.commit().map_err(|error| {
            journal_error(
                "LSV4010",
                match kind {
                    PendingCommit::Existing => "failed to commit pending journal record",
                    PendingCommit::New => "failed to commit pending effect",
                },
                error,
            )
        })
    }
}
