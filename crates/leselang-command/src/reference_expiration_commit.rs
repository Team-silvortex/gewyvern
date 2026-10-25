//! Reference deadline-expiration batches over native journal ownership.

use crate::reference_completion::deadline_cancellation;
use crate::{ContinuationImage, ContinuationToken, Step};
use leselang_runtime_core::Fault;

pub struct MemoryExpirationState<'a> {
    pub image: &'a ContinuationImage,
    pub acknowledged: &'a mut bool,
    pub lease_expires_at_ms: &'a mut Option<u64>,
    pub terminal_step: &'a mut Option<Step>,
}

/// Borrow original fields in native map order, without collecting or cloning the
/// map. All due dispatches are changed before the first group is finalized.
pub trait MemoryExpirationJournal {
    fn dispatches(&mut self) -> impl Iterator<Item = MemoryExpirationState<'_>>;
    fn finalize_group(&mut self, token: &ContinuationToken) -> Result<(), Fault>;
}

/// This low-level primitive adds no clock/image admission. The public entry owns
/// those gates. Finalization failure preserves the entire already-mutated batch;
/// it does not roll back, retry, or publish a partial result list to the caller.
pub fn expire_memory<Host: MemoryExpirationJournal>(
    host: &mut Host,
    now_ms: u64,
) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
    let mut expired = Vec::new();
    for state in host.dispatches() {
        let image = state.image;
        if *state.acknowledged
            || image
                .deadline_at_ms
                .is_none_or(|deadline| deadline > now_ms)
        {
            continue;
        }
        let step = deadline_cancellation(image, now_ms);
        *state.acknowledged = true;
        *state.lease_expires_at_ms = None;
        *state.terminal_step = Some(step.clone());
        expired.push((image.clone(), step));
    }
    for (image, _) in &expired {
        host.finalize_group(&image.token)?;
    }
    Ok(expired)
}

/// Raw rows selected by the native pending/deadline query, not admitted images.
pub struct TimedEffectRecord {
    pub token: String,
    pub image: Vec<u8>,
}

/// A single immediate transaction opened after native clock-range admission.
/// The scan uses that same clock and retains native token ordering and eager
/// collection before writes. Codecs, quota, storage, group writes and rollback
/// remain native. Commit consumes the owner, including for an empty batch.
pub trait DurableExpirationTransaction {
    fn due_records(&mut self) -> Result<Vec<TimedEffectRecord>, Fault>;
    fn admit_image(&mut self, bytes: &[u8]) -> Result<ContinuationImage, Fault>;
    fn encode_terminal(&mut self, step: &Step) -> Result<Vec<u8>, Fault>;
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault>;
    fn complete(&mut self, token: &str, step_bytes: &[u8]) -> Result<(), Fault>;
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault>;
    fn commit(self) -> Result<(), Fault>;
}

/// Snapshot rows are not reselected after group finalization changes siblings.
/// Each row is admitted and completed in original scan order, with no new request
/// or lease checks. Only successful commit returns the full batch for publication;
/// errors/unwinds rely on native rollback, never compensation or commit retry.
pub fn expire_durable<Host: DurableExpirationTransaction>(
    mut host: Host,
    now_ms: u64,
) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
    let records = host.due_records()?;
    let mut expired = Vec::with_capacity(records.len());
    for record in records {
        let image = host.admit_image(&record.image)?;
        if image.token.as_str() != record.token
            || image
                .deadline_at_ms
                .is_none_or(|deadline| deadline > now_ms)
        {
            return Err(Fault {
                code: "LSV4007".into(),
                message: "timed effect deadline conflicts with continuation image".into(),
            });
        }
        let step = deadline_cancellation(&image, now_ms);
        let step_bytes = host.encode_terminal(&step)?;
        host.ensure_growth(step_bytes.len())?;
        host.complete(&record.token, &step_bytes)?;
        host.finalize_group(&record.token)?;
        expired.push((image, step));
    }
    host.commit()?;
    Ok(expired)
}
