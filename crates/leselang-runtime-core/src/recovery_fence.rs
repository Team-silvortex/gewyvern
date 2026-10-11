use std::{collections::BTreeSet, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReservationError {
    Closed,
    Limit,
    Sequence,
    Duplicate,
}

/// Counts only, not authenticated identities or permission to allocate/replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReservationSummary {
    pub count: usize,
    pub highest_sequence: u64,
}

/// Bounded once-only numeric reservations across one complete recovery pass.
/// Native parsing, original owner/group admission and aggregate storage bounds
/// precede insertion. No key normalization, allocation-watermark repair, durable
/// mutation or execution happens here. Limits are explicit inclusive host policy;
/// the host must choose finite aggregate limits, not untrusted input-derived grants.
/// Refusal closes the pass and discards its partial index; it cannot be queried or
/// finished afterward. Only successful finish exposes count/highest observations.
/// This is not a durable allocator, receipt, graph certificate or global registry.
pub struct SequenceReservations {
    staged: Option<BTreeSet<u64>>,
    max_reservations: usize,
    max_sequence: u64,
}

impl SequenceReservations {
    pub fn new(max_reservations: usize, max_sequence: u64) -> Self {
        Self {
            staged: Some(BTreeSet::new()),
            max_reservations,
            max_sequence,
        }
    }

    /// Count, sequence, duplicate checks preserve original insertion order.
    /// Zero is not a sequence; every sequence needs a representable successor.
    /// Gaps are valid and are neither filled nor made reusable.
    pub fn insert(&mut self, sequence: u64) -> Result<(), ReservationError> {
        let mut staged = self.staged.take();
        let outcome = (|| {
            let staged = staged.as_mut().ok_or(ReservationError::Closed)?;
            if staged.len() >= self.max_reservations {
                return Err(ReservationError::Limit);
            }
            if sequence == 0 || sequence > self.max_sequence || sequence.checked_add(1).is_none() {
                return Err(ReservationError::Sequence);
            }
            if !staged.insert(sequence) {
                return Err(ReservationError::Duplicate);
            }
            Ok(())
        })();
        if outcome.is_ok() {
            self.staged = staged;
        }
        outcome
    }

    /// Consume a complete structural index. This does not authenticate or commit
    /// storage; native final graph/receipt/authority checks must still precede
    /// snapshot handoff inside the same original transaction/snapshot.
    pub fn finish(self) -> Result<ReservationSummary, ReservationError> {
        let staged = self.staged.ok_or(ReservationError::Closed)?;
        Ok(ReservationSummary {
            count: staged.len(),
            highest_sequence: staged.last().copied().unwrap_or(0),
        })
    }
}

/// Same-snapshot native storage observations, not self-authenticating facts.
/// None represents an invalid/unrepresentable original allocation watermark.
pub struct ReservationObservation {
    pub allocation_watermark: Option<u64>,
    pub record_exists: bool,
    pub aliases_owner: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReservationProofError {
    Sequence,
    Watermark,
    Presence,
    Alias,
}

/// Check an already-admitted slot against the ORIGINAL allocation watermark,
/// exact expected record presence and owner-namespace exclusion, in that order.
/// This cannot prove record ownership/schema or cross-group uniqueness by itself.
/// Hosts must validate those separately and must not first advance/repair the
/// watermark using imported rows: that would launder an unallocated reservation.
/// No storage reads/writes, callbacks, normalization or budget construction occur.
pub fn validate_reservation_proof(
    sequence: u64,
    expected_record: bool,
    observation: ReservationObservation,
) -> Result<(), ReservationProofError> {
    if sequence == 0 {
        return Err(ReservationProofError::Sequence);
    }
    if observation
        .allocation_watermark
        .is_none_or(|watermark| watermark == 0 || sequence >= watermark)
    {
        return Err(ReservationProofError::Watermark);
    }
    if observation.record_exists != expected_record {
        return Err(ReservationProofError::Presence);
    }
    if observation.aliases_owner {
        return Err(ReservationProofError::Alias);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredGroupState {
    Pending,
    Completed,
}

pub enum GroupReceiptError<Native> {
    State,
    Native(Native),
}

impl<Native> fmt::Display for GroupReceiptError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::State => "group state conflicts with complete members or receipt presence",
            Self::Native(_) => "group receipt native admission failed",
        })
    }
}
impl<Native> fmt::Debug for GroupReceiptError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Native> std::error::Error for GroupReceiptError<Native> {}

/// Complete-member/state/receipt consistency before native receipt admission.
/// Pending requires incomplete members and no receipt. Completed requires every
/// member complete and one receipt; only that case invokes the once-only hook.
/// Native codecs/schema/value checks must bound their own work and ingress.
/// The exact native receipt/error moves without Clone/Debug/serde/Default/Send/
/// Sync bounds. Uncalled hook owners and rejected inputs clean up before result/
/// error handoff; cleanup unwind cannot publish the result, and a second panic
/// may abort. Callback-internal cleanup and side effects remain trusted native work.
///
/// This is transient structural observation, not settlement, receipt authenticity
/// or execution authority. Hosts still own original graph/owner/capture checks,
/// current policy, raw-result correspondence, storage atomicity and replay rules.
/// It never executes source, evaluates a saved body, dispatches or retries an effect.
pub fn validate_group_receipt<Input, Receipt, Native>(
    state: StoredGroupState,
    all_members_completed: bool,
    encoded: Option<Input>,
    admit: impl FnOnce(Input) -> Result<Receipt, Native>,
) -> Result<Option<Receipt>, GroupReceiptError<Native>> {
    let mut admit = Some(admit);
    let outcome = match (state, encoded) {
        (StoredGroupState::Pending, None) if !all_members_completed => Ok(None),
        (StoredGroupState::Completed, Some(input)) if all_members_completed => match admit.take() {
            Some(admit) => admit(input).map(Some).map_err(GroupReceiptError::Native),
            None => Err(GroupReceiptError::State),
        },
        (_, input) => {
            drop(input);
            Err(GroupReceiptError::State)
        }
    };
    drop(admit);
    outcome
}
