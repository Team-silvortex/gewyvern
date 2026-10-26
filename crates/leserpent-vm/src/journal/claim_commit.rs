use super::*;
#[cfg(feature = "sqlite-journal")]
use leselang_command::reference_claim_commit::{
    ClaimCommit, DurableClaimTransaction, StoredClaimCandidate,
};
use leselang_command::reference_claim_commit::{
    MemoryClaimCandidate, MemoryClaimJournal, MemoryLeaseState,
};

impl MemoryClaimJournal for EphemeralJournal {
    fn candidates(&self) -> impl Iterator<Item = MemoryClaimCandidate<'_>> {
        self.dispatches
            .iter()
            .map(|(token, d)| MemoryClaimCandidate {
                token,
                attempt: d.attempt,
                acknowledged: d.acknowledged,
                ready_at_ms: d.ready_at_ms,
                lease_expires_at_ms: d.lease_expires_at_ms,
            })
    }
    fn sequence_ready(&self, token: &ContinuationToken) -> bool {
        self.sequence_ready(token)
    }
    fn pressure(&self, now_ms: u64, limits: SchedulerLimits) -> SchedulerPressure {
        self.scheduler_pressure(now_ms, limits)
    }
    fn dispatch(&mut self, token: &ContinuationToken) -> Option<MemoryLeaseState<'_>> {
        self.dispatches.get_mut(token).map(|d| MemoryLeaseState {
            request: &d.request,
            attempt: &mut d.attempt,
            retry_count: d.retry_count,
            lease_expires_at_ms: &mut d.lease_expires_at_ms,
        })
    }
}

#[cfg(feature = "sqlite-journal")]
pub(super) struct SqliteClaimTransaction<'a> {
    pub transaction: rusqlite::Transaction<'a>,
    pub now: i64,
    pub expires_at: i64,
}

#[cfg(feature = "sqlite-journal")]
impl DurableClaimTransaction for SqliteClaimTransaction<'_> {
    fn capacity_exhausted(&mut self, limit: usize) -> Result<bool, Fault> {
        sql_lease_capacity_exhausted(&self.transaction, self.now, limit)
    }
    fn has_candidate(&mut self) -> Result<bool, Fault> {
        self.transaction
            .query_row(
                &format!("SELECT EXISTS({CLAIM_DISPATCH_SQL})"),
                [self.now],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4016", "failed to inspect dispatch readiness", error)
            })
    }
    fn pressure(
        &mut self,
        now_ms: u64,
        limits: SchedulerLimits,
    ) -> Result<SchedulerPressure, Fault> {
        sql_scheduler_pressure(&self.transaction, now_ms, limits)
    }
    fn candidate(&mut self) -> Result<Option<StoredClaimCandidate>, Fault> {
        self.transaction
            .query_row(CLAIM_DISPATCH_SQL, [self.now], |row| {
                Ok(StoredClaimCandidate {
                    token: row.get(0)?,
                    request: row.get(1)?,
                    attempt: row.get(2)?,
                    retry_count: row.get(3)?,
                    image: row.get(4)?,
                })
            })
            .optional()
            .map_err(|error| journal_error("LSV4016", "failed to select effect dispatch", error))
    }
    fn encode_image(&mut self, image: &ContinuationImage) -> Result<Vec<u8>, Fault> {
        encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")
    }
    fn lease(&mut self, token: &str, attempt: u32) -> Result<(), Fault> {
        self.transaction
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'leased', attempt = ?2, lease_expires_at_ms = ?3
                 WHERE token = ?1",
                params![token, i64::from(attempt), self.expires_at],
            )
            .map_err(|error| journal_error("LSV4016", "failed to lease effect dispatch", error))?;
        Ok(())
    }
    fn commit(self, reason: ClaimCommit) -> Result<(), Fault> {
        self.transaction.commit().map_err(|error| {
            journal_error(
                "LSV4016",
                match reason {
                    ClaimCommit::CapacityLimited => {
                        "failed to close capacity-limited dispatch transaction"
                    }
                    ClaimCommit::Idle => "failed to close dispatch transaction",
                    ClaimCommit::Leased => "failed to commit effect lease",
                },
                error,
            )
        })
    }
}
