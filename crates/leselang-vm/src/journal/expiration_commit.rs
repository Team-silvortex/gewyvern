use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_expiration_commit::{
    DurableExpirationTransaction, TimedEffectRecord,
};
use leselang_command::reference_expiration_commit::{
    MemoryExpirationJournal, MemoryExpirationState,
};

impl MemoryExpirationJournal for EphemeralJournal {
    fn dispatches(&mut self) -> impl Iterator<Item = MemoryExpirationState<'_>> {
        self.dispatches
            .values_mut()
            .map(|dispatch| MemoryExpirationState {
                image: &dispatch.request.continuation,
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
pub(super) struct SqliteExpirationTransaction<'a> {
    pub(super) transaction: rusqlite::Transaction<'a>,
    pub(super) now: i64,
}

#[cfg(feature = "sqlite-journal")]
impl DurableExpirationTransaction for SqliteExpirationTransaction<'_> {
    fn due_records(&mut self) -> Result<Vec<TimedEffectRecord>, Fault> {
        let mut statement = self
            .transaction
            .prepare(
                "SELECT token, image FROM vm_effects
                 WHERE state = 'pending' AND deadline_at_ms IS NOT NULL AND deadline_at_ms <= ?1
                 ORDER BY token ASC",
            )
            .map_err(|error| journal_error("LSV4025", "failed to prepare timeout scan", error))?;
        let records = statement
            .query_map([self.now], |row| {
                Ok(TimedEffectRecord {
                    token: row.get(0)?,
                    image: row.get(1)?,
                })
            })
            .map_err(|error| journal_error("LSV4025", "failed to scan timed effects", error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| journal_error("LSV4025", "failed to read timed effect", error))?;
        drop(statement);
        Ok(records)
    }
    fn admit_image(&mut self, bytes: &[u8]) -> Result<ContinuationImage, Fault> {
        let image = decode_bounded(bytes, MAX_CONTINUATION_BYTES)?;
        validate_image(&image)?;
        Ok(image)
    }
    fn encode_terminal(&mut self, step: &Step) -> Result<Vec<u8>, Fault> {
        encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        ensure_growth(&self.transaction, additional)
    }
    fn complete(&mut self, token: &str, step_bytes: &[u8]) -> Result<(), Fault> {
        complete_terminal_record(&self.transaction, token, step_bytes)
    }
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault> {
        finalize_merge_group(&self.transaction, token)
    }
    fn commit(self) -> Result<(), Fault> {
        self.transaction
            .commit()
            .map_err(|error| journal_error("LSV4025", "failed to commit effect timeouts", error))
    }
}
