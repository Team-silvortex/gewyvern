//! Ordered admission of one original effect record before checkpoint staging.

use std::fmt;

use crate::{RecoveryPayload, RecoveryRecord};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredRecordState {
    Pending,
    Completed,
}

/// Adapter-owned codecs and same-snapshot proofs. Header reads precede this
/// pass; image/receipt decoding and every native hook must bound their own work.
/// Observations must faithfully describe the original row, not repaired data.
/// Hooks release unused native owners before constructing return values; cleanup
/// inside a callback is adapter-owned, not something this pass can intercept.
pub trait RecordRecoveryHost {
    type Key;
    type Image;
    type Pending;
    type Completed;
    type Error;

    fn admit_image(&mut self) -> Result<Self::Image, Self::Error>;
    fn sequence(&mut self, image: &Self::Image) -> Result<u64, Self::Error>;
    fn validate_identity(&mut self, image: &Self::Image) -> Result<(), Self::Error>;
    fn validate_deadline(&mut self, image: &Self::Image) -> Result<(), Self::Error>;
    fn validate_owner(&mut self, image: &Self::Image) -> Result<(), Self::Error>;
    fn stored_state(&mut self) -> Result<StoredRecordState, Self::Error>;
    fn admit_completed(&mut self) -> Result<Self::Completed, Self::Error>;
    fn validate_successor(
        &mut self,
        image: &Self::Image,
        completed: Option<&Self::Completed>,
    ) -> Result<(), Self::Error>;
    /// Move the original admitted identity and pending payload, without new work.
    fn pending_payload(&mut self, image: Self::Image) -> (Self::Key, Self::Pending);
    /// Move the original identity/receipt; release unused image ownership.
    fn completed_payload(
        &mut self,
        image: Self::Image,
        completed: Self::Completed,
    ) -> (Self::Key, Self::Completed);
}

/// Refusal stage and unchanged native ownership, without payload diagnostics.
pub enum RecordRecoveryError<Native> {
    Image(Native),
    Sequence(Native),
    Identity(Native),
    Deadline(Native),
    Owner(Native),
    State(Native),
    Receipt(Native),
    Successor(Native),
}
impl<Native> fmt::Display for RecordRecoveryError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Image(_) => "stored effect image admission failed",
            Self::Sequence(_) => "stored effect sequence is invalid",
            Self::Identity(_) => "stored effect identity conflicts",
            Self::Deadline(_) => "stored effect deadline conflicts",
            Self::Owner(_) => "stored effect owner proof failed",
            Self::State(_) => "stored effect state is invalid",
            Self::Receipt(_) => "stored effect receipt admission failed",
            Self::Successor(_) => "stored effect successor proof failed",
        })
    }
}
impl<Native> fmt::Debug for RecordRecoveryError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for RecordRecoveryError<Native> {}

/// Native admitted data or an owned, stage-tagged refusal.
pub type RecordRecoveryResult<Key, Pending, Completed, Native> =
    Result<RecoveryRecord<Key, Pending, Completed>, RecordRecoveryError<Native>>;

/// Inspect image, sequence, identity, deadline, owner, then stored state. Pending
/// rows prove their successor without loading a receipt; completed rows admit the
/// original receipt before successor proof. Only then may payload ownership move
/// into a recovery record. Invalid state cannot hide an earlier owner/identity error.
///
/// Opaque images/keys/receipts/errors need no Clone, Debug, serde, equality or thread
/// traits. The host stays borrowed. Refusal drops admitted image/receipt owners;
/// hook or cleanup unwind propagates without retry, and a second panic may abort.
/// Native callbacks/destructors remain trusted and cannot be rolled back here.
///
/// This is not authentication or a recoverable execution handle. Bound ingress,
/// original authority/budgets, global graph/reservation proofs and atomic storage
/// commit remain host obligations. Stage the returned record in CheckpointRecovery
/// without exposing partial data. No fuel, evaluator, dispatch or write is added.
pub fn inspect_record_recovery<Host: RecordRecoveryHost + ?Sized>(
    host: &mut Host,
) -> RecordRecoveryResult<Host::Key, Host::Pending, Host::Completed, Host::Error> {
    let image = host.admit_image().map_err(RecordRecoveryError::Image)?;
    let sequence = host
        .sequence(&image)
        .map_err(RecordRecoveryError::Sequence)?;
    host.validate_identity(&image)
        .map_err(RecordRecoveryError::Identity)?;
    host.validate_deadline(&image)
        .map_err(RecordRecoveryError::Deadline)?;
    host.validate_owner(&image)
        .map_err(RecordRecoveryError::Owner)?;
    let state = host.stored_state().map_err(RecordRecoveryError::State)?;
    let (key, payload) = match state {
        StoredRecordState::Pending => {
            host.validate_successor(&image, None)
                .map_err(RecordRecoveryError::Successor)?;
            let (key, pending) = host.pending_payload(image);
            (key, RecoveryPayload::Pending(pending))
        }
        StoredRecordState::Completed => {
            let completed = host
                .admit_completed()
                .map_err(RecordRecoveryError::Receipt)?;
            host.validate_successor(&image, Some(&completed))
                .map_err(RecordRecoveryError::Successor)?;
            let (key, completed) = host.completed_payload(image, completed);
            (key, RecoveryPayload::Completed(completed))
        }
    };
    Ok(RecoveryRecord {
        key,
        sequence,
        payload,
    })
}
