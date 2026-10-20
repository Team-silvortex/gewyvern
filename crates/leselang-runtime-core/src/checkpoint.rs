use std::{collections::BTreeMap, fmt};

/// Validated native data supplied by a trusted storage adapter, not a receipt or
/// execution grant. The adapter owns decoding, schema, authority and graph checks.
pub struct RecoveryRecord<Key, Pending, Completed> {
    pub key: Key,
    pub sequence: u64,
    pub payload: RecoveryPayload<Pending, Completed>,
}

pub enum RecoveryPayload<Pending, Completed> {
    Pending(Pending),
    Completed(Completed),
}

/// Closed staging failures never format native keys or payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointError {
    Closed,
    RecordLimit,
    DuplicateKey,
    InvalidSequence,
}

/// Native storage errors retain their original ownership without formatter calls.
pub enum CheckpointCommitError<External> {
    Closed,
    Host(External),
}

impl<External> fmt::Debug for CheckpointCommitError<External> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => f.write_str("Closed"),
            Self::Host(_) => f.write_str("Host"),
        }
    }
}

/// Native checks and writes against the same transaction used to stage records.
/// Each hook bounds its own work and proves original data, never repaired grants.
/// Hosts without a particular graph/audit format explicitly provide that policy;
/// this interface does not authenticate storage or implement missing checks.
pub trait CheckpointFinalizer {
    type Commit;
    type Error;

    fn validate_dispatches(&mut self) -> Result<(), Self::Error>;
    /// Highest original graph/reservation sequence, or zero for no observations.
    fn validate_graphs(&mut self) -> Result<u64, Self::Error>;
    fn validate_audits(&mut self) -> Result<(), Self::Error>;
    /// Reconcile original storage with the exact maximum of row/graph sequences.
    /// Return owned native metadata; mutations must remain rollbackable until commit.
    fn advance_watermark(&mut self, highest_sequence: u64) -> Result<Self::Commit, Self::Error>;
    fn commit(&mut self) -> Result<(), Self::Error>;
}

/// Native errors move unchanged; diagnostics expose only the failing phase.
pub enum CheckpointFinalizationError<Native> {
    Dispatch(Native),
    Graph(Native),
    Audit(Native),
    Watermark(Native),
    Commit(Native),
}
impl<Native> fmt::Display for CheckpointFinalizationError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Dispatch(_) => "checkpoint dispatch validation failed",
            Self::Graph(_) => "checkpoint graph validation failed",
            Self::Audit(_) => "checkpoint audit validation failed",
            Self::Watermark(_) => "checkpoint watermark reconciliation failed",
            Self::Commit(_) => "checkpoint storage commit failed",
        })
    }
}
impl<Native> fmt::Debug for CheckpointFinalizationError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for CheckpointFinalizationError<Native> {}

/// A committed, owned data snapshot. This is deliberately not serializable or a
/// restorable execution handle; callers may inspect or move the original maps.
pub struct RecoveredCheckpoint<Key, Pending, Completed> {
    pending: BTreeMap<Key, Pending>,
    completed: BTreeMap<Key, Completed>,
    highest_sequence: u64,
}

/// Successful committed snapshot plus unchanged native commit metadata.
pub type CheckpointCommitResult<Key, Pending, Completed, Commit, External> =
    Result<(RecoveredCheckpoint<Key, Pending, Completed>, Commit), CheckpointCommitError<External>>;

impl<Key, Pending, Completed> RecoveredCheckpoint<Key, Pending, Completed> {
    pub fn pending(&self) -> &BTreeMap<Key, Pending> {
        &self.pending
    }

    pub fn completed(&self) -> &BTreeMap<Key, Completed> {
        &self.completed
    }

    /// Zero for an empty snapshot. Storage may reserve further graph identities.
    pub fn highest_sequence(&self) -> u64 {
        self.highest_sequence
    }

    pub fn into_parts(self) -> (BTreeMap<Key, Pending>, BTreeMap<Key, Completed>) {
        (self.pending, self.completed)
    }
}

/// Bounded, fail-closed staging for a complete storage recovery pass.
///
/// Nothing can inspect partial maps or obtain them before the final host check
/// and commit succeed. A rejected row poisons the pass and releases all staged
/// data. Native data need no Clone/Debug/serde/Send/Sync bounds; only keys need Ord.
/// Key comparison, callbacks and Drop are trusted native code. Unwind propagates
/// and a second cleanup panic may abort. There is no preemption or rollback of
/// native side effects; atomic storage validation/commit belongs to the adapter.
pub struct CheckpointRecovery<Key, Pending, Completed> {
    staged: Option<RecoveredCheckpoint<Key, Pending, Completed>>,
    max_records: usize,
    max_sequence: u64,
}

impl<Key: Ord, Pending, Completed> CheckpointRecovery<Key, Pending, Completed> {
    /// Limits are inclusive and never repaired. Zero records admits only empty
    /// snapshots; zero sequences admits no row. Every row sequence must have a
    /// representable successor. Storage-specific sequence ceilings remain host policy.
    pub fn new(max_records: usize, max_sequence: u64) -> Self {
        Self {
            staged: Some(RecoveredCheckpoint {
                pending: BTreeMap::new(),
                completed: BTreeMap::new(),
                highest_sequence: 0,
            }),
            max_records,
            max_sequence,
        }
    }

    /// Moves one already-decoded/validated row. Checks run in limit, sequence,
    /// identity order. Duplicate keys across either partition reject the pass;
    /// no overwrite, normalization or implicit idempotence is performed.
    pub fn push(
        &mut self,
        record: RecoveryRecord<Key, Pending, Completed>,
    ) -> Result<(), CheckpointError> {
        // Close before native Ord/Drop can unwind; only a complete insertion reopens.
        let mut staged = self.staged.take();
        let outcome = (|| {
            let staged = staged.as_mut().ok_or(CheckpointError::Closed)?;
            if staged.pending.len().saturating_add(staged.completed.len()) >= self.max_records {
                return Err(CheckpointError::RecordLimit);
            }
            if record.sequence == 0
                || record.sequence > self.max_sequence
                || record.sequence.checked_add(1).is_none()
            {
                return Err(CheckpointError::InvalidSequence);
            }
            if staged.pending.contains_key(&record.key)
                || staged.completed.contains_key(&record.key)
            {
                return Err(CheckpointError::DuplicateKey);
            }
            staged.highest_sequence = staged.highest_sequence.max(record.sequence);
            match record.payload {
                RecoveryPayload::Pending(value) => {
                    staged.pending.insert(record.key, value);
                }
                RecoveryPayload::Completed(value) => {
                    staged.completed.insert(record.key, value);
                }
            }
            Ok(())
        })();
        if outcome.is_ok() {
            self.staged = staged;
        } else {
            drop(staged);
        }
        outcome
    }

    /// Complete ordered finalization: dispatch, graph, audit, watermark, commit.
    /// No hook receives the staged maps. Graph observations are combined with
    /// original row sequences using max, never incremented or clamped here.
    /// Numeric ceilings, authentic authority and storage atomicity remain native.
    ///
    /// A closed pass calls no hook. Refusal/unwind stops without retry; original
    /// commit metadata is discarded on failed commit, while the native error is
    /// still locally owned. The existing finish boundary releases host state
    /// before any snapshot/metadata/error handoff. A committed transaction followed
    /// by cleanup unwind is uncertain, not permission to replay. Hooks and Drop
    /// remain trusted; the core cannot roll back arbitrary native side effects.
    pub fn finish_recovery<Host: CheckpointFinalizer>(
        self,
        host: Host,
    ) -> CheckpointCommitResult<
        Key,
        Pending,
        Completed,
        Host::Commit,
        CheckpointFinalizationError<Host::Error>,
    > {
        self.finish(host, |host, snapshot| {
            host.validate_dispatches()
                .map_err(CheckpointFinalizationError::Dispatch)?;
            let graph_sequence = host
                .validate_graphs()
                .map_err(CheckpointFinalizationError::Graph)?;
            let highest_sequence = snapshot.highest_sequence().max(graph_sequence);
            host.validate_audits()
                .map_err(CheckpointFinalizationError::Audit)?;
            let metadata = host
                .advance_watermark(highest_sequence)
                .map_err(CheckpointFinalizationError::Watermark)?;
            match host.commit() {
                Ok(()) => Ok(metadata),
                Err(error) => {
                    drop(metadata);
                    Err(CheckpointFinalizationError::Commit(error))
                }
            }
        })
    }

    /// One final read-only graph observation and atomic storage commit. The
    /// adapter must hold the same storage snapshot/transaction throughout loading
    /// and this callback. Commit metadata is returned unchanged with the maps.
    ///
    /// Framework-owned host state and uncalled callback captures are released
    /// while snapshots, commit metadata and errors remain local. Cleanup unwind
    /// cannot hand off recovered state. Callback-internal cleanup remains trusted.
    /// A commit followed by unwind is uncertain, not permission to replay effects.
    pub fn finish<Host, Commit, External>(
        self,
        mut host: Host,
        finalize: impl FnOnce(
            &mut Host,
            &RecoveredCheckpoint<Key, Pending, Completed>,
        ) -> Result<Commit, External>,
    ) -> CheckpointCommitResult<Key, Pending, Completed, Commit, External> {
        let mut staged = self.staged;
        let mut finalize = Some(finalize);
        let outcome = match (staged.as_ref(), finalize.take()) {
            (Some(snapshot), Some(finalize)) => {
                finalize(&mut host, snapshot).map_err(CheckpointCommitError::Host)
            }
            (_, callback) => {
                // An uncalled callback can itself own GUI-local native resources.
                finalize = callback;
                Err(CheckpointCommitError::Closed)
            }
        };
        drop(finalize);
        drop(host);
        match outcome {
            Ok(commit) => match staged.take() {
                Some(snapshot) => Ok((snapshot, commit)),
                None => Err(CheckpointCommitError::Closed),
            },
            Err(error) => {
                drop(staged);
                Err(error)
            }
        }
    }
}
