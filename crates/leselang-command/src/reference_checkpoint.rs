//! Released checkpoint policy over one caller-owned storage transaction.
//!
//! This adapter coordinates recovery, including the owner's commit hook. It does
//! not implement storage, authorize effects, or execute a restored continuation.

use leselang_runtime_core::{
    CheckpointCommitError, CheckpointError, CheckpointFinalizationError, CheckpointFinalizer,
    CheckpointRecovery, Fault, RecoveredCheckpoint,
};

use crate::continuation_identity::MAX_EFFECT_SEQUENCE;
use crate::reference_recovery::RecoveredEffectRecord;
use crate::retention_policy::MAX_JOURNAL_RECORDS;
use crate::{ContinuationImage, ContinuationToken, Step};

pub const MAX_JOURNAL_TOTAL_BYTES: usize = 64 * 1024 * 1024;
pub type ReferenceCheckpoint = RecoveredCheckpoint<ContinuationToken, ContinuationImage, Step>;

/// All reads, proofs, watermark updates and commit must use the same original
/// transaction. The trusted owner bounds its reads and validates each record
/// before staging it; a staged record is not a grant to restore or dispatch.
///
/// Stop at the first read or staging error. Release cursors before returning from
/// `stage_records`. No Clone/Send/static requirement or retry is introduced.
/// Refusal/unwind before commit must leave rollback to the native owner. A commit
/// or cleanup unwind can have an uncertain outcome and is never permission to
/// replay writes. Hooks and cleanup must not publish the staged checkpoint.
pub trait ReferenceCheckpointSource: CheckpointFinalizer<Commit = u64, Error = Fault> {
    fn record_count(&mut self) -> Result<usize, Fault>;
    fn payload_bytes(&mut self) -> Result<usize, Fault>;
    fn stage_records(
        &mut self,
        push: &mut dyn FnMut(RecoveredEffectRecord) -> Result<(), Fault>,
    ) -> Result<(), Fault>;
}

/// Publish maps only after dispatch/graph/audit proofs, allocator repair, commit
/// and owner cleanup succeed. Native hook failures retain their original faults.
pub fn recover_checkpoint<Source: ReferenceCheckpointSource>(
    mut source: Source,
) -> Result<(ReferenceCheckpoint, u64), Fault> {
    let count = source.record_count()?;
    if count > MAX_JOURNAL_RECORDS {
        return Err(fault(
            "LSV4006",
            format!("journal has {count} records, limit is {MAX_JOURNAL_RECORDS}"),
        ));
    }
    let total_bytes = source.payload_bytes()?;
    if total_bytes > MAX_JOURNAL_TOTAL_BYTES {
        return Err(fault(
            "LSV4006",
            format!("journal payload is {total_bytes} bytes, limit is {MAX_JOURNAL_TOTAL_BYTES}"),
        ));
    }
    let mut recovery = CheckpointRecovery::new(MAX_JOURNAL_RECORDS, MAX_EFFECT_SEQUENCE);
    source.stage_records(&mut |record| recovery.push(record).map_err(checkpoint_fault))?;
    recovery
        .finish_recovery(source)
        .map_err(|error| match error {
            CheckpointCommitError::Host(error) => match error {
                CheckpointFinalizationError::Dispatch(error)
                | CheckpointFinalizationError::Graph(error)
                | CheckpointFinalizationError::Audit(error)
                | CheckpointFinalizationError::Watermark(error)
                | CheckpointFinalizationError::Commit(error) => error,
            },
            CheckpointCommitError::Closed => checkpoint_fault(CheckpointError::Closed),
        })
}

fn checkpoint_fault(error: CheckpointError) -> Fault {
    match error {
        CheckpointError::RecordLimit => fault("LSV4006", "journal recovery record limit exceeded"),
        CheckpointError::InvalidSequence => {
            fault("LSV4007", "journal continuation sequence is invalid")
        }
        CheckpointError::DuplicateKey => fault("LSV4007", "duplicate journal continuation token"),
        CheckpointError::Closed => fault("LSV4007", "journal recovery pass is closed"),
    }
}

fn fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
