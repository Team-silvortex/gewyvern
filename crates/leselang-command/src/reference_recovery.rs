//! Product effect-record admission over caller-owned, same-snapshot storage reads.
//!
//! The shared recovery kernel owns phase ordering, not a database or execution.
//! Returned records are uncommitted observations. Complete graph/dispatch/audit
//! validation, current authority and atomic checkpoint publication stay with the
//! owner. A successful inspection cannot restore, dispatch or settle anything.

use leselang_runtime_core::{
    Fault, RecordRecoveryError, RecordRecoveryHost, RecoveryRecord, StoredRecordState,
    inspect_record_recovery,
};

use crate::bounded_json::{self, BoundedJsonDecodeError};
use crate::continuation_identity::canonical_continuation_sequence;
use crate::reference_completion::validate_terminal_step;
use crate::reference_request::validate_continuation_image;
use crate::{ContinuationImage, ContinuationToken, MAX_CONTINUATION_BYTES, Step};

pub const MAX_JOURNAL_ENTRY_BYTES: usize = 8 * 1024 * 1024;
pub type RecoveredEffectRecord = RecoveryRecord<ContinuationToken, ContinuationImage, Step>;

/// Storage access and graph proofs from the same original snapshot as the header.
/// Read errors retain their native diagnostics. Hooks must bound their own work,
/// must not dispatch or commit, and must not repair corrupt stored observations.
/// Pending rows never read a completed payload. No Clone/Send requirement is added.
pub trait ReferenceRecordSource {
    type Bytes: AsRef<[u8]>;

    fn read_image(&mut self) -> Result<Self::Bytes, Fault>;
    fn read_deadline(&mut self) -> Result<Option<i64>, Fault>;
    fn validate_owner(&mut self, image: &ContinuationImage) -> Result<(), Fault>;
    fn read_completed(&mut self) -> Result<Self::Bytes, Fault>;
    fn validate_successor(
        &mut self,
        image: &ContinuationImage,
        completed: Option<&Step>,
    ) -> Result<(), Fault>;
}

/// Headers must already have been read in token/state order by the storage owner.
/// Keep the source borrowed until the original checkpoint commits; no observation
/// here authenticates a caller-supplied header or a permissive graph-proof hook.
pub fn inspect_effect_record<Source: ReferenceRecordSource + ?Sized>(
    source: &mut Source,
    token: &str,
    state: &str,
) -> Result<RecoveredEffectRecord, Fault> {
    inspect_record_recovery(&mut RecordPolicy {
        source,
        token,
        state,
    })
    .map_err(|error| match error {
        RecordRecoveryError::Image(error)
        | RecordRecoveryError::Sequence(error)
        | RecordRecoveryError::Identity(error)
        | RecordRecoveryError::Deadline(error)
        | RecordRecoveryError::Owner(error)
        | RecordRecoveryError::State(error)
        | RecordRecoveryError::Receipt(error)
        | RecordRecoveryError::Successor(error) => error,
    })
}

/// Released bounded journal JSON diagnostics, without semantic admission.
pub fn decode_journal_value<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    limit: usize,
) -> Result<T, Fault> {
    bounded_json::decode(bytes, limit).map_err(|error| match error {
        BoundedJsonDecodeError::Limit { limit, .. } => {
            recovery_fault("LSV4006", format!("journal entry exceeds {limit} bytes"))
        }
        BoundedJsonDecodeError::Deserialize(error) => {
            recovery_fault("LSV4007", format!("invalid journal JSON: {error}"))
        }
    })
}

struct RecordPolicy<'a, Source: ReferenceRecordSource + ?Sized> {
    source: &'a mut Source,
    token: &'a str,
    state: &'a str,
}

impl<Source: ReferenceRecordSource + ?Sized> RecordRecoveryHost for RecordPolicy<'_, Source> {
    type Key = ContinuationToken;
    type Image = ContinuationImage;
    type Pending = ContinuationImage;
    type Completed = Step;
    type Error = Fault;

    fn admit_image(&mut self) -> Result<ContinuationImage, Fault> {
        let bytes = self.source.read_image()?;
        let image = decode_journal_value(bytes.as_ref(), MAX_CONTINUATION_BYTES)?;
        validate_continuation_image(&image)?;
        Ok(image)
    }
    fn sequence(&mut self, image: &ContinuationImage) -> Result<u64, Fault> {
        canonical_continuation_sequence(&image.token)
            .ok_or_else(|| recovery_fault("LSV4007", "journal continuation sequence is invalid"))
    }
    fn validate_identity(&mut self, image: &ContinuationImage) -> Result<(), Fault> {
        if image.token.as_str() != self.token {
            return Err(recovery_fault(
                "LSV4007",
                "journal token does not match continuation image",
            ));
        }
        Ok(())
    }
    fn validate_deadline(&mut self, image: &ContinuationImage) -> Result<(), Fault> {
        let stored_deadline = self.source.read_deadline()?;
        let stored_deadline = stored_deadline
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| recovery_fault("LSV4007", "effect deadline is invalid"))
            })
            .transpose()?;
        if stored_deadline != image.deadline_at_ms {
            return Err(recovery_fault(
                "LSV4007",
                "journal deadline does not match continuation image",
            ));
        }
        Ok(())
    }
    fn validate_owner(&mut self, image: &ContinuationImage) -> Result<(), Fault> {
        self.source.validate_owner(image)
    }
    fn stored_state(&mut self) -> Result<StoredRecordState, Fault> {
        match self.state {
            "pending" => Ok(StoredRecordState::Pending),
            "completed" => Ok(StoredRecordState::Completed),
            _ => Err(recovery_fault("LSV4007", "invalid journal record state")),
        }
    }
    fn admit_completed(&mut self) -> Result<Step, Fault> {
        let bytes = self.source.read_completed()?;
        let step = decode_journal_value(bytes.as_ref(), MAX_JOURNAL_ENTRY_BYTES)?;
        validate_terminal_step(&step)?;
        Ok(step)
    }
    fn validate_successor(
        &mut self,
        image: &ContinuationImage,
        completed: Option<&Step>,
    ) -> Result<(), Fault> {
        self.source.validate_successor(image, completed)
    }
    fn pending_payload(
        &mut self,
        image: ContinuationImage,
    ) -> (ContinuationToken, ContinuationImage) {
        (image.token.clone(), image)
    }
    fn completed_payload(
        &mut self,
        image: ContinuationImage,
        completed: Step,
    ) -> (ContinuationToken, Step) {
        let ContinuationImage {
            token,
            expected_revision,
            result_type,
            pending_effect,
            result_binding,
            group_result,
            schema_version: _,
            program_counter: _,
            fuel_remaining: _,
            deadline_ms: _,
            deadline_at_ms: _,
            max_output_items: _,
        } = image;
        // Release unused image owners before constructing the handoff value.
        drop((
            expected_revision,
            result_type,
            pending_effect,
            result_binding,
            group_result,
        ));
        (token, completed)
    }
}

fn recovery_fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
