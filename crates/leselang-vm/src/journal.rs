use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::limits::Limit;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::{
    Cancellation, CancellationReason, ContinuationImage, ContinuationToken, DispatchLease,
    EffectError, EffectRequest, Fault, MAX_CONTINUATION_BYTES, MAX_DISPATCH_ATTEMPTS,
    MAX_DISPATCH_LEASE_MS, MAX_SEMANTIC_RETRIES, RetryDisposition, Step, Value, encode_json_capped,
    validate_effect_error, validate_effect_request, validate_image,
};

pub const JOURNAL_SCHEMA_VERSION: u32 = 4;
pub const MAX_JOURNAL_RECORDS: usize = 10_000;
pub const MAX_JOURNAL_ENTRY_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_JOURNAL_TOTAL_BYTES: usize = 64 * 1024 * 1024;
const JOURNAL_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct JournalSnapshot {
    pub pending: BTreeMap<ContinuationToken, ContinuationImage>,
    pub completed: BTreeMap<ContinuationToken, Step>,
    pub next_sequence: u64,
}

pub(crate) enum Journal {
    Ephemeral(EphemeralJournal),
    Sqlite(SqliteJournal),
}

#[derive(Default)]
pub(crate) struct EphemeralJournal {
    dispatches: BTreeMap<ContinuationToken, EphemeralDispatch>,
}

struct EphemeralDispatch {
    request: EffectRequest,
    attempt: u32,
    lease_expires_at_ms: Option<u64>,
    ready_at_ms: u64,
    retry_count: u32,
    last_error: Option<EffectError>,
    acknowledged: bool,
}

pub(crate) struct SqliteJournal {
    connection: Connection,
}

struct JournalRecord {
    state: String,
    image: Vec<u8>,
    terminal_step: Option<Vec<u8>>,
}

struct DispatchRecord {
    request: Vec<u8>,
    state: String,
    attempt: u32,
    lease_expires_at_ms: Option<u64>,
    ready_at_ms: u64,
    retry_count: u32,
}

type RawDispatchRow = (Vec<u8>, String, i64, Option<i64>, i64, i64, Option<Vec<u8>>);

impl Journal {
    pub fn ephemeral() -> Self {
        Self::Ephemeral(EphemeralJournal::default())
    }

    pub fn open(path: &Path) -> Result<(Self, JournalSnapshot), Fault> {
        let journal = SqliteJournal::open(path)?;
        let snapshot = journal.load()?;
        Ok((Self::Sqlite(journal), snapshot))
    }

    pub fn allocate_sequence(&mut self, local_next: u64) -> Result<u64, Fault> {
        match self {
            Self::Ephemeral(_) => Ok(local_next),
            Self::Sqlite(journal) => journal.allocate_sequence(local_next),
        }
    }

    pub fn record_pending(
        &mut self,
        image: &ContinuationImage,
        request: Option<&EffectRequest>,
    ) -> Result<(), Fault> {
        match self {
            Self::Ephemeral(journal) => journal.record_pending(request),
            Self::Sqlite(journal) => journal.record_pending(image, request),
        }
    }

    pub fn claim_dispatch(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<Option<DispatchLease>, Fault> {
        validate_lease_clock(now_ms, lease_ms)?;
        match self {
            Self::Ephemeral(journal) => journal.claim_dispatch(now_ms, lease_ms),
            Self::Sqlite(journal) => journal.claim_dispatch(now_ms, lease_ms),
        }
    }

    pub fn record_completed(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
    ) -> Result<Step, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.record_completed(image, step),
            Self::Sqlite(journal) => journal.record_completed(image, step),
        }
    }

    pub fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        step: &Step,
    ) -> Result<Step, Fault> {
        validate_lease_clock(now_ms, 1)?;
        match self {
            Self::Ephemeral(journal) => journal.acknowledge_dispatch(lease, now_ms, step),
            Self::Sqlite(journal) => journal.acknowledge_dispatch(lease, now_ms, step),
        }
    }

    pub fn cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.cancel(image, step),
            Self::Sqlite(journal) => journal.cancel(image, step),
        }
    }

    pub fn expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.expire_due(now_ms),
            Self::Sqlite(journal) => journal.expire_due(now_ms),
        }
    }

    pub fn report_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        disposition: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        match self {
            Self::Ephemeral(journal) => journal.report_error(lease, now_ms, disposition),
            Self::Sqlite(journal) => journal.report_error(lease, now_ms, disposition),
        }
    }
}

impl EphemeralJournal {
    fn record_pending(&mut self, request: Option<&EffectRequest>) -> Result<(), Fault> {
        let Some(request) = request else {
            return Ok(());
        };
        validate_effect_request(request)?;
        let token = request.continuation.token.clone();
        if let Some(existing) = self.dispatches.get(&token) {
            return if existing.request == *request {
                Ok(())
            } else {
                Err(journal_fault(
                    "LSV4011",
                    "effect request conflicts with pending state",
                ))
            };
        }
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
            },
        );
        Ok(())
    }

    fn claim_dispatch(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<Option<DispatchLease>, Fault> {
        let candidate = self.dispatches.values_mut().find(|dispatch| {
            !dispatch.acknowledged
                && dispatch.ready_at_ms <= now_ms
                && dispatch
                    .lease_expires_at_ms
                    .is_none_or(|expires_at| expires_at <= now_ms)
        });
        let Some(dispatch) = candidate else {
            return Ok(None);
        };
        dispatch.attempt = next_attempt(dispatch.attempt)?;
        let lease_expires_at_ms = lease_expiration(now_ms, lease_ms)?;
        dispatch.lease_expires_at_ms = Some(lease_expires_at_ms);
        Ok(Some(DispatchLease {
            request: dispatch.request.clone(),
            attempt: dispatch.attempt,
            retry_count: dispatch.retry_count,
            lease_expires_at_ms,
        }))
    }

    fn record_completed(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        if let Some(dispatch) = self.dispatches.get_mut(&image.token) {
            if dispatch.lease_expires_at_ms.is_some() && !dispatch.acknowledged {
                return Err(journal_fault(
                    "LSV4024",
                    "leased effect must be completed through dispatch acknowledgement",
                ));
            }
            dispatch.acknowledged = true;
        }
        Ok(step.clone())
    }

    fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        step: &Step,
    ) -> Result<Step, Fault> {
        validate_terminal_step(step)?;
        let token = &lease.request.continuation.token;
        let Some(dispatch) = self.dispatches.get_mut(token) else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        if dispatch.acknowledged {
            return Ok(step.clone());
        }
        if dispatch.request != lease.request
            || dispatch.attempt != lease.attempt
            || dispatch.retry_count != lease.retry_count
            || dispatch.lease_expires_at_ms != Some(lease.lease_expires_at_ms)
        {
            return Err(journal_fault(
                "LSV4022",
                "dispatch lease has been superseded",
            ));
        }
        if lease.lease_expires_at_ms < now_ms {
            return Err(journal_fault("LSV4023", "dispatch lease has expired"));
        }
        dispatch.acknowledged = true;
        dispatch.lease_expires_at_ms = None;
        Ok(step.clone())
    }

    fn cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        validate_cancellation_step(image, step)?;
        if let Some(dispatch) = self.dispatches.get_mut(&image.token) {
            dispatch.acknowledged = true;
            dispatch.lease_expires_at_ms = None;
        }
        Ok(step.clone())
    }

    fn expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        let mut expired = Vec::new();
        for dispatch in self.dispatches.values_mut() {
            let image = &dispatch.request.continuation;
            if dispatch.acknowledged
                || image
                    .deadline_at_ms
                    .is_none_or(|deadline| deadline > now_ms)
            {
                continue;
            }
            let step = deadline_cancellation(image, now_ms);
            dispatch.acknowledged = true;
            dispatch.lease_expires_at_ms = None;
            expired.push((image.clone(), step));
        }
        Ok(expired)
    }

    fn report_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        disposition: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        let token = &lease.request.continuation.token;
        let Some(dispatch) = self.dispatches.get_mut(token) else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        validate_retry_lease(dispatch, lease, now_ms)?;
        match disposition {
            RetryDisposition::Scheduled(schedule) => {
                if schedule.retry_count != dispatch.retry_count + 1
                    || schedule.ready_at_ms <= now_ms
                    || validate_effect_error(&schedule.error).is_err()
                {
                    return Err(journal_fault("LSV4026", "retry schedule is invalid"));
                }
                dispatch.retry_count = schedule.retry_count;
                dispatch.ready_at_ms = schedule.ready_at_ms;
                dispatch.last_error = Some(schedule.error.clone());
                dispatch.lease_expires_at_ms = None;
            }
            RetryDisposition::Terminal(step) => {
                validate_terminal_step(step)?;
                dispatch.acknowledged = true;
                dispatch.lease_expires_at_ms = None;
            }
        }
        Ok(disposition.clone())
    }
}

fn validate_retry_lease(
    dispatch: &EphemeralDispatch,
    lease: &DispatchLease,
    now_ms: u64,
) -> Result<(), Fault> {
    if dispatch.acknowledged
        || dispatch.request != lease.request
        || dispatch.attempt != lease.attempt
        || dispatch.retry_count != lease.retry_count
        || dispatch.ready_at_ms > now_ms
        || dispatch.lease_expires_at_ms != Some(lease.lease_expires_at_ms)
    {
        return Err(journal_fault(
            "LSV4022",
            "dispatch lease has been superseded",
        ));
    }
    if lease.lease_expires_at_ms < now_ms {
        return Err(journal_fault("LSV4023", "dispatch lease has expired"));
    }
    Ok(())
}

impl SqliteJournal {
    fn open(path: &Path) -> Result<Self, Fault> {
        if path.as_os_str().is_empty() {
            return Err(journal_fault("LSV4001", "journal path is empty"));
        }
        if fs::symlink_metadata(path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(journal_fault(
                "LSV4001",
                "journal path must not be a symbolic link",
            ));
        }
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if !parent.is_dir() {
            return Err(journal_fault(
                "LSV4001",
                "journal parent directory does not exist",
            ));
        }
        let file_name = path
            .file_name()
            .ok_or_else(|| journal_fault("LSV4001", "journal path must include a file name"))?;
        let canonical_parent = fs::canonicalize(parent)
            .map_err(|error| journal_error("LSV4001", "failed to resolve journal parent", error))?;
        let resolved_path = canonical_parent.join(file_name);
        let path = resolved_path.as_path();

        if !path.exists() {
            create_private_journal(path)?;
        }

        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let connection = Connection::open_with_flags(path, flags)
            .map_err(|error| journal_error("LSV4002", "failed to open journal", error))?;
        tighten_permissions(path)?;
        connection
            .set_limit(
                Limit::SQLITE_LIMIT_LENGTH,
                i32::try_from(MAX_JOURNAL_ENTRY_BYTES + MAX_CONTINUATION_BYTES + 4096)
                    .expect("journal SQLite length limit fits i32"),
            )
            .map_err(|error| journal_error("LSV4002", "failed to bound journal", error))?;
        connection
            .busy_timeout(JOURNAL_BUSY_TIMEOUT)
            .map_err(|error| journal_error("LSV4002", "failed to configure journal", error))?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = DELETE;
                 PRAGMA synchronous = FULL;
                 PRAGMA foreign_keys = ON;
                 PRAGMA trusted_schema = OFF;",
            )
            .map_err(|error| journal_error("LSV4002", "failed to configure journal", error))?;

        let version: u32 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| journal_error("LSV4003", "failed to read journal version", error))?;
        if version == 0 {
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     CREATE TABLE IF NOT EXISTS vm_metadata (
                       singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                       next_sequence INTEGER NOT NULL CHECK (next_sequence >= 1)
                     ) STRICT;
                     INSERT OR IGNORE INTO vm_metadata(singleton, next_sequence) VALUES (1, 1);
                     CREATE TABLE IF NOT EXISTS vm_effects (
                       token TEXT PRIMARY KEY,
                       state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                       image BLOB NOT NULL,
                       deadline_at_ms INTEGER CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0),
                       terminal_step BLOB,
                       CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                              (state = 'completed' AND terminal_step IS NOT NULL))
                     ) STRICT;
                     CREATE TABLE IF NOT EXISTS vm_dispatches (
                       token TEXT PRIMARY KEY REFERENCES vm_effects(token) ON DELETE CASCADE,
                       request BLOB NOT NULL,
                       state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'acknowledged')),
                       attempt INTEGER NOT NULL CHECK (attempt >= 0),
                       lease_expires_at_ms INTEGER,
                       ready_at_ms INTEGER NOT NULL DEFAULT 0 CHECK (ready_at_ms >= 0),
                       retry_count INTEGER NOT NULL DEFAULT 0
                         CHECK (retry_count >= 0 AND retry_count <= 16),
                       last_error BLOB,
                       CHECK ((state = 'ready' AND lease_expires_at_ms IS NULL) OR
                              (state = 'leased' AND lease_expires_at_ms IS NOT NULL) OR
                              (state = 'acknowledged' AND lease_expires_at_ms IS NULL))
                     ) STRICT;
                     CREATE INDEX vm_effect_deadline_idx
                       ON vm_effects(state, deadline_at_ms);
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
                )
                .map_err(|error| journal_error("LSV4003", "failed to initialize journal", error))?;
        } else if version == 1 {
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     CREATE TABLE vm_dispatches (
                       token TEXT PRIMARY KEY REFERENCES vm_effects(token) ON DELETE CASCADE,
                       request BLOB NOT NULL,
                       state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'acknowledged')),
                       attempt INTEGER NOT NULL CHECK (attempt >= 0),
                       lease_expires_at_ms INTEGER,
                       ready_at_ms INTEGER NOT NULL DEFAULT 0 CHECK (ready_at_ms >= 0),
                       retry_count INTEGER NOT NULL DEFAULT 0
                         CHECK (retry_count >= 0 AND retry_count <= 16),
                       last_error BLOB,
                       CHECK ((state = 'ready' AND lease_expires_at_ms IS NULL) OR
                              (state = 'leased' AND lease_expires_at_ms IS NOT NULL) OR
                              (state = 'acknowledged' AND lease_expires_at_ms IS NULL))
                     ) STRICT;
                     ALTER TABLE vm_effects ADD COLUMN deadline_at_ms INTEGER
                       CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0);
                     CREATE INDEX vm_effect_deadline_idx
                       ON vm_effects(state, deadline_at_ms);
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
                )
                .map_err(|error| journal_error("LSV4003", "failed to migrate journal", error))?;
        } else if version == 2 {
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     ALTER TABLE vm_effects ADD COLUMN deadline_at_ms INTEGER
                       CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0);
                     CREATE INDEX vm_effect_deadline_idx
                       ON vm_effects(state, deadline_at_ms);
                     ALTER TABLE vm_dispatches ADD COLUMN ready_at_ms INTEGER NOT NULL DEFAULT 0
                       CHECK (ready_at_ms >= 0);
                     ALTER TABLE vm_dispatches ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0
                       CHECK (retry_count >= 0 AND retry_count <= 16);
                     ALTER TABLE vm_dispatches ADD COLUMN last_error BLOB;
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
                )
                .map_err(|error| journal_error("LSV4003", "failed to migrate journal", error))?;
        } else if version == 3 {
            connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     ALTER TABLE vm_dispatches ADD COLUMN ready_at_ms INTEGER NOT NULL DEFAULT 0
                       CHECK (ready_at_ms >= 0);
                     ALTER TABLE vm_dispatches ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0
                       CHECK (retry_count >= 0 AND retry_count <= 16);
                     ALTER TABLE vm_dispatches ADD COLUMN last_error BLOB;
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
                )
                .map_err(|error| journal_error("LSV4003", "failed to migrate journal", error))?;
        } else if version != JOURNAL_SCHEMA_VERSION {
            return Err(journal_fault(
                "LSV4004",
                format!("unsupported journal version {version}, expected {JOURNAL_SCHEMA_VERSION}"),
            ));
        }

        Ok(Self { connection })
    }

    fn load(&self) -> Result<JournalSnapshot, Fault> {
        let count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row.get(0))
            .map_err(|error| journal_error("LSV4005", "failed to count journal records", error))?;
        let count = usize::try_from(count)
            .map_err(|_| journal_fault("LSV4007", "journal record count is invalid"))?;
        if count > MAX_JOURNAL_RECORDS {
            return Err(journal_fault(
                "LSV4006",
                format!("journal has {count} records, limit is {MAX_JOURNAL_RECORDS}"),
            ));
        }
        let total_bytes = journal_payload_bytes(&self.connection)?;
        if total_bytes > MAX_JOURNAL_TOTAL_BYTES {
            return Err(journal_fault(
                "LSV4006",
                format!(
                    "journal payload is {total_bytes} bytes, limit is {MAX_JOURNAL_TOTAL_BYTES}"
                ),
            ));
        }

        let next_sequence: i64 = self
            .connection
            .query_row(
                "SELECT next_sequence FROM vm_metadata WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| journal_error("LSV4005", "failed to load journal metadata", error))?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT token, state, image, deadline_at_ms, terminal_step
                 FROM vm_effects ORDER BY token ASC",
            )
            .map_err(|error| journal_error("LSV4005", "failed to prepare journal load", error))?;
        let mut rows = statement
            .query([])
            .map_err(|error| journal_error("LSV4005", "failed to load journal", error))?;
        let mut pending = BTreeMap::new();
        let mut completed = BTreeMap::new();

        while let Some(row) = rows
            .next()
            .map_err(|error| journal_error("LSV4005", "failed to read journal record", error))?
        {
            let token: String = row
                .get(0)
                .map_err(|error| journal_error("LSV4005", "invalid journal token", error))?;
            let state: String = row
                .get(1)
                .map_err(|error| journal_error("LSV4005", "invalid journal state", error))?;
            let image_bytes: Vec<u8> = row
                .get(2)
                .map_err(|error| journal_error("LSV4005", "invalid journal image", error))?;
            let image: ContinuationImage = decode_bounded(&image_bytes, MAX_CONTINUATION_BYTES)?;
            validate_image(&image)?;
            if image.token.as_str() != token {
                return Err(journal_fault(
                    "LSV4007",
                    "journal token does not match continuation image",
                ));
            }
            let stored_deadline: Option<i64> = row.get(3).map_err(|error| {
                journal_error("LSV4005", "invalid journal effect deadline", error)
            })?;
            let stored_deadline = stored_deadline
                .map(|value| {
                    u64::try_from(value)
                        .map_err(|_| journal_fault("LSV4007", "effect deadline is invalid"))
                })
                .transpose()?;
            if stored_deadline != image.deadline_at_ms {
                return Err(journal_fault(
                    "LSV4007",
                    "journal deadline does not match continuation image",
                ));
            }

            match state.as_str() {
                "pending" => {
                    pending.insert(image.token.clone(), image);
                }
                "completed" => {
                    let step_bytes: Vec<u8> = row.get(4).map_err(|error| {
                        journal_error("LSV4005", "invalid completed journal record", error)
                    })?;
                    let step: Step = decode_bounded(&step_bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                    validate_terminal_step(&step)?;
                    completed.insert(image.token, step);
                }
                _ => return Err(journal_fault("LSV4007", "invalid journal record state")),
            }
        }

        drop(rows);
        drop(statement);
        validate_dispatch_records(&self.connection)?;

        let next_sequence = u64::try_from(next_sequence)
            .map_err(|_| journal_fault("LSV4007", "journal sequence is invalid"))?;
        Ok(JournalSnapshot {
            pending,
            completed,
            next_sequence: next_sequence.max(1),
        })
    }

    fn allocate_sequence(&mut self, local_next: u64) -> Result<u64, Fault> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4008", "failed to lock journal", error))?;
        let stored: i64 = transaction
            .query_row(
                "SELECT next_sequence FROM vm_metadata WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| journal_error("LSV4008", "failed to allocate sequence", error))?;
        let stored = u64::try_from(stored)
            .map_err(|_| journal_fault("LSV4007", "journal sequence is invalid"))?;
        let sequence = stored.max(local_next);
        let next = sequence.checked_add(1).ok_or_else(|| {
            journal_fault("LSV4009", "journal continuation sequence is exhausted")
        })?;
        let next = i64::try_from(next)
            .map_err(|_| journal_fault("LSV4009", "journal continuation sequence is exhausted"))?;
        transaction
            .execute(
                "UPDATE vm_metadata SET next_sequence = ?1 WHERE singleton = 1",
                [next],
            )
            .map_err(|error| journal_error("LSV4008", "failed to store sequence", error))?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4008", "failed to commit sequence", error))?;
        Ok(sequence)
    }

    fn record_pending(
        &mut self,
        image: &ContinuationImage,
        request: Option<&EffectRequest>,
    ) -> Result<(), Fault> {
        validate_image(image)?;
        if let Some(request) = request {
            validate_effect_request(request)?;
            if &request.continuation != image {
                return Err(journal_fault(
                    "LSV4011",
                    "effect request continuation does not match pending image",
                ));
            }
        }
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let request_bytes = request
            .map(|request| encode_json_capped(request, MAX_JOURNAL_ENTRY_BYTES, "effect request"))
            .transpose()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4010", "failed to lock journal", error))?;
        let existing = load_record(&transaction, image.token.as_str())?;
        if let Some(existing) = existing {
            if existing.state == "pending" && existing.image == image_bytes {
                if request_bytes.is_some() {
                    ensure_dispatch_matches(
                        &transaction,
                        image.token.as_str(),
                        request.expect("request bytes require request"),
                    )?;
                }
                return transaction.commit().map_err(|error| {
                    journal_error("LSV4010", "failed to commit pending journal record", error)
                });
            }
            return Err(journal_fault(
                "LSV4011",
                "continuation token conflicts with durable journal state",
            ));
        }
        let count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row.get(0))
            .map_err(|error| journal_error("LSV4010", "failed to count journal records", error))?;
        let count = usize::try_from(count)
            .map_err(|_| journal_fault("LSV4007", "journal record count is invalid"))?;
        if count >= MAX_JOURNAL_RECORDS {
            return Err(journal_fault(
                "LSV4006",
                format!("journal record limit {MAX_JOURNAL_RECORDS} reached"),
            ));
        }
        ensure_growth(
            &transaction,
            image_bytes.len() + request_bytes.as_ref().map_or(0, Vec::len),
        )?;
        transaction
            .execute(
                "INSERT INTO vm_effects(token, state, image, deadline_at_ms, terminal_step)
                 VALUES (?1, 'pending', ?2, ?3, NULL)",
                params![
                    image.token.as_str(),
                    image_bytes,
                    image
                        .deadline_at_ms
                        .map(i64::try_from)
                        .transpose()
                        .map_err(|_| {
                            journal_fault("LSV4010", "effect deadline is out of range")
                        })?
                ],
            )
            .map_err(|error| journal_error("LSV4010", "failed to store pending effect", error))?;
        if let Some(request_bytes) = request_bytes {
            transaction
                .execute(
                    "INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms)
                     VALUES (?1, ?2, 'ready', 0, NULL)",
                    params![image.token.as_str(), request_bytes],
                )
                .map_err(|error| {
                    journal_error("LSV4010", "failed to store effect dispatch", error)
                })?;
        }
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4010", "failed to commit pending effect", error))
    }

    fn claim_dispatch(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<Option<DispatchLease>, Fault> {
        let now = i64::try_from(now_ms)
            .map_err(|_| journal_fault("LSV4015", "dispatch clock is out of range"))?;
        let expires_at = lease_expiration(now_ms, lease_ms)?;
        let expires_at_sql = i64::try_from(expires_at)
            .map_err(|_| journal_fault("LSV4015", "dispatch lease expiration is out of range"))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4016", "failed to lock dispatch journal", error))?;
        let candidate = transaction
            .query_row(
                "SELECT d.token, d.request, d.attempt, d.retry_count
                 FROM vm_dispatches d
                 JOIN vm_effects e ON e.token = d.token
                 WHERE e.state = 'pending'
                   AND ((d.state = 'ready' AND d.ready_at_ms <= ?1) OR
                        (d.state = 'leased' AND d.lease_expires_at_ms <= ?1))
                 ORDER BY d.token ASC
                 LIMIT 1",
                [now],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| journal_error("LSV4016", "failed to select effect dispatch", error))?;
        let Some((token, request_bytes, attempt, retry_count)) = candidate else {
            transaction.commit().map_err(|error| {
                journal_error("LSV4016", "failed to close dispatch transaction", error)
            })?;
            return Ok(None);
        };
        let request: EffectRequest = decode_bounded(&request_bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_effect_request(&request)?;
        if request.continuation.token.as_str() != token {
            return Err(journal_fault(
                "LSV4007",
                "dispatch token does not match effect request",
            ));
        }
        let attempt = u32::try_from(attempt)
            .map_err(|_| journal_fault("LSV4007", "dispatch attempt is invalid"))?;
        let attempt = next_attempt(attempt)?;
        let retry_count = u32::try_from(retry_count)
            .map_err(|_| journal_fault("LSV4007", "dispatch retry count is invalid"))?;
        if retry_count > MAX_SEMANTIC_RETRIES {
            return Err(journal_fault(
                "LSV4007",
                "dispatch retry count exceeds runtime limit",
            ));
        }
        transaction
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'leased', attempt = ?2, lease_expires_at_ms = ?3
                 WHERE token = ?1",
                params![token, i64::from(attempt), expires_at_sql],
            )
            .map_err(|error| journal_error("LSV4016", "failed to lease effect dispatch", error))?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4016", "failed to commit effect lease", error))?;
        Ok(Some(DispatchLease {
            request,
            attempt,
            retry_count,
            lease_expires_at_ms: expires_at,
        }))
    }

    fn record_completed(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        validate_image(image)?;
        validate_terminal_step(step)?;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4012", "failed to lock journal", error))?;
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault(
                "LSV4013",
                "pending continuation is missing from durable journal",
            ));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "continuation image conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let bytes = existing.terminal_step.ok_or_else(|| {
                journal_fault("LSV4007", "completed journal record has no terminal step")
            })?;
            let authoritative: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4012", "failed to close journal transaction", error)
            })?;
            return Ok(authoritative);
        }
        if let Some(dispatch) = load_dispatch(&transaction, image.token.as_str())?
            && dispatch.state == "leased"
        {
            return Err(journal_fault(
                "LSV4024",
                "leased effect must be completed through dispatch acknowledgement",
            ));
        }
        ensure_growth(&transaction, step_bytes.len())?;
        transaction
            .execute(
                "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
                params![image.token.as_str(), step_bytes],
            )
            .map_err(|error| journal_error("LSV4012", "failed to complete effect", error))?;
        transaction
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'acknowledged', lease_expires_at_ms = NULL
                 WHERE token = ?1",
                [image.token.as_str()],
            )
            .map_err(|error| journal_error("LSV4012", "failed to acknowledge dispatch", error))?;
        transaction.commit().map_err(|error| {
            journal_error("LSV4012", "failed to commit completed effect", error)
        })?;
        Ok(step.clone())
    }

    fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        step: &Step,
    ) -> Result<Step, Fault> {
        validate_effect_request(&lease.request)?;
        validate_terminal_step(step)?;
        if lease.attempt == 0 || lease.attempt > MAX_DISPATCH_ATTEMPTS {
            return Err(journal_fault("LSV4022", "dispatch attempt is invalid"));
        }
        let image = &lease.request.continuation;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        encode_json_capped(&lease.request, MAX_JOURNAL_ENTRY_BYTES, "effect request")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4020", "failed to lock dispatch journal", error))?;
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch continuation is unknown"));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "dispatch continuation conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let bytes = existing.terminal_step.ok_or_else(|| {
                journal_fault("LSV4007", "completed journal record has no terminal step")
            })?;
            let authoritative: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4020", "failed to close dispatch transaction", error)
            })?;
            return Ok(authoritative);
        }
        let Some(dispatch) = load_dispatch(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        let durable_request: EffectRequest =
            decode_bounded(&dispatch.request, MAX_JOURNAL_ENTRY_BYTES)?;
        if durable_request != lease.request
            || dispatch.state != "leased"
            || dispatch.attempt != lease.attempt
            || dispatch.retry_count != lease.retry_count
            || dispatch.ready_at_ms > now_ms
            || dispatch.lease_expires_at_ms != Some(lease.lease_expires_at_ms)
        {
            return Err(journal_fault(
                "LSV4022",
                "dispatch lease has been superseded",
            ));
        }
        if lease.lease_expires_at_ms < now_ms {
            return Err(journal_fault("LSV4023", "dispatch lease has expired"));
        }
        ensure_growth(&transaction, step_bytes.len())?;
        transaction
            .execute(
                "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
                params![image.token.as_str(), step_bytes],
            )
            .map_err(|error| journal_error("LSV4020", "failed to complete dispatch", error))?;
        transaction
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'acknowledged', lease_expires_at_ms = NULL
                 WHERE token = ?1",
                [image.token.as_str()],
            )
            .map_err(|error| journal_error("LSV4020", "failed to acknowledge dispatch", error))?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4020", "failed to commit dispatch result", error))?;
        Ok(step.clone())
    }

    fn report_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        disposition: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        validate_effect_request(&lease.request)?;
        let image = &lease.request.continuation;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4026", "failed to lock retry journal", error))?;
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch continuation is unknown"));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "dispatch continuation conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let authoritative: Step = decode_bounded(
                &existing.terminal_step.ok_or_else(|| {
                    journal_fault("LSV4007", "completed journal record has no terminal step")
                })?,
                MAX_JOURNAL_ENTRY_BYTES,
            )?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4026", "failed to close retry transaction", error)
            })?;
            return Ok(RetryDisposition::Terminal(authoritative));
        }
        let Some(dispatch) = load_dispatch(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        let durable_request: EffectRequest =
            decode_bounded(&dispatch.request, MAX_JOURNAL_ENTRY_BYTES)?;
        if durable_request != lease.request
            || dispatch.state != "leased"
            || dispatch.attempt != lease.attempt
            || dispatch.retry_count != lease.retry_count
            || dispatch.ready_at_ms > now_ms
            || dispatch.lease_expires_at_ms != Some(lease.lease_expires_at_ms)
        {
            return Err(journal_fault(
                "LSV4022",
                "dispatch lease has been superseded",
            ));
        }
        if lease.lease_expires_at_ms < now_ms {
            return Err(journal_fault("LSV4023", "dispatch lease has expired"));
        }

        match disposition {
            RetryDisposition::Scheduled(schedule) => {
                if schedule.retry_count != dispatch.retry_count + 1
                    || schedule.retry_count > MAX_SEMANTIC_RETRIES
                    || schedule.ready_at_ms <= now_ms
                    || schedule.ready_at_ms > i64::MAX as u64
                    || validate_effect_error(&schedule.error).is_err()
                {
                    return Err(journal_fault("LSV4026", "retry schedule is invalid"));
                }
                let error_bytes = encode_json_capped(&schedule.error, 2_048, "effect error")?;
                ensure_growth(&transaction, error_bytes.len())?;
                transaction
                    .execute(
                        "UPDATE vm_dispatches
                         SET state = 'ready', retry_count = ?2, ready_at_ms = ?3,
                             last_error = ?4, lease_expires_at_ms = NULL
                         WHERE token = ?1",
                        params![
                            image.token.as_str(),
                            i64::from(schedule.retry_count),
                            i64::try_from(schedule.ready_at_ms).expect("validated retry clock"),
                            error_bytes
                        ],
                    )
                    .map_err(|error| {
                        journal_error("LSV4026", "failed to schedule effect retry", error)
                    })?;
            }
            RetryDisposition::Terminal(step) => {
                validate_terminal_step(step)?;
                let step_bytes =
                    encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
                ensure_growth(&transaction, step_bytes.len())?;
                complete_terminal_record(&transaction, image.token.as_str(), &step_bytes)?;
            }
        }
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4026", "failed to commit effect retry", error))?;
        Ok(disposition.clone())
    }

    fn cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        validate_image(image)?;
        validate_cancellation_step(image, step)?;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                journal_error("LSV4025", "failed to lock cancellation journal", error)
            })?;
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4013", "pending continuation is missing"));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "continuation image conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let authoritative: Step = decode_bounded(
                &existing.terminal_step.ok_or_else(|| {
                    journal_fault("LSV4007", "completed journal record has no terminal step")
                })?,
                MAX_JOURNAL_ENTRY_BYTES,
            )?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4025", "failed to close cancellation transaction", error)
            })?;
            return Ok(authoritative);
        }
        ensure_growth(&transaction, step_bytes.len())?;
        complete_terminal_record(&transaction, image.token.as_str(), &step_bytes)?;
        transaction.commit().map_err(|error| {
            journal_error("LSV4025", "failed to commit effect cancellation", error)
        })?;
        Ok(step.clone())
    }

    fn expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        let now = i64::try_from(now_ms)
            .map_err(|_| journal_fault("LSV4015", "dispatch clock is out of range"))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4025", "failed to lock timeout journal", error))?;
        let mut statement = transaction
            .prepare(
                "SELECT token, image FROM vm_effects
                 WHERE state = 'pending' AND deadline_at_ms IS NOT NULL AND deadline_at_ms <= ?1
                 ORDER BY token ASC",
            )
            .map_err(|error| journal_error("LSV4025", "failed to prepare timeout scan", error))?;
        let records = statement
            .query_map([now], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|error| journal_error("LSV4025", "failed to scan timed effects", error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| journal_error("LSV4025", "failed to read timed effect", error))?;
        drop(statement);

        let mut expired = Vec::with_capacity(records.len());
        for (token, image_bytes) in records {
            let image: ContinuationImage = decode_bounded(&image_bytes, MAX_CONTINUATION_BYTES)?;
            validate_image(&image)?;
            if image.token.as_str() != token
                || image
                    .deadline_at_ms
                    .is_none_or(|deadline| deadline > now_ms)
            {
                return Err(journal_fault(
                    "LSV4007",
                    "timed effect deadline conflicts with continuation image",
                ));
            }
            let step = deadline_cancellation(&image, now_ms);
            let step_bytes = encode_json_capped(&step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
            ensure_growth(&transaction, step_bytes.len())?;
            complete_terminal_record(&transaction, &token, &step_bytes)?;
            expired.push((image, step));
        }
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4025", "failed to commit effect timeouts", error))?;
        Ok(expired)
    }
}

fn complete_terminal_record(
    transaction: &rusqlite::Transaction<'_>,
    token: &str,
    step_bytes: &[u8],
) -> Result<(), Fault> {
    transaction
        .execute(
            "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
            params![token, step_bytes],
        )
        .map_err(|error| journal_error("LSV4025", "failed to cancel effect", error))?;
    transaction
        .execute(
            "UPDATE vm_dispatches
             SET state = 'acknowledged', lease_expires_at_ms = NULL
             WHERE token = ?1",
            [token],
        )
        .map_err(|error| journal_error("LSV4025", "failed to close cancelled dispatch", error))?;
    Ok(())
}

fn journal_payload_bytes(connection: &Connection) -> Result<usize, Fault> {
    let bytes: i64 = connection
        .query_row(
            "SELECT
               COALESCE((SELECT SUM(length(image) + COALESCE(length(terminal_step), 0))
                         FROM vm_effects), 0) +
               COALESCE((SELECT SUM(length(request) + COALESCE(length(last_error), 0))
                         FROM vm_dispatches), 0)",
            [],
            |row| row.get(0),
        )
        .map_err(|error| journal_error("LSV4005", "failed to size journal payload", error))?;
    usize::try_from(bytes).map_err(|_| journal_fault("LSV4007", "journal payload size is invalid"))
}

fn ensure_growth(transaction: &rusqlite::Transaction<'_>, additional: usize) -> Result<(), Fault> {
    let current = journal_payload_bytes(transaction)?;
    if current.saturating_add(additional) > MAX_JOURNAL_TOTAL_BYTES {
        return Err(journal_fault(
            "LSV4006",
            format!("journal payload limit {MAX_JOURNAL_TOTAL_BYTES} reached"),
        ));
    }
    Ok(())
}

fn load_record(
    transaction: &rusqlite::Transaction<'_>,
    token: &str,
) -> Result<Option<JournalRecord>, Fault> {
    transaction
        .query_row(
            "SELECT state, image, terminal_step FROM vm_effects WHERE token = ?1",
            [token],
            |row| {
                Ok(JournalRecord {
                    state: row.get(0)?,
                    image: row.get(1)?,
                    terminal_step: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| journal_error("LSV4005", "failed to read journal record", error))
}

fn load_dispatch(
    transaction: &rusqlite::Transaction<'_>,
    token: &str,
) -> Result<Option<DispatchRecord>, Fault> {
    transaction
        .query_row(
            "SELECT request, state, attempt, lease_expires_at_ms, ready_at_ms, retry_count,
                    last_error
             FROM vm_dispatches WHERE token = ?1",
            [token],
            |row| {
                let attempt: i64 = row.get(2)?;
                let expires: Option<i64> = row.get(3)?;
                let ready_at: i64 = row.get(4)?;
                let retry_count: i64 = row.get(5)?;
                let last_error: Option<Vec<u8>> = row.get(6)?;
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    attempt,
                    expires,
                    ready_at,
                    retry_count,
                    last_error,
                ))
            },
        )
        .optional()
        .map_err(|error| journal_error("LSV4005", "failed to read dispatch record", error))?
        .map(
            |(request, state, attempt, expires, ready_at, retry_count, last_error): RawDispatchRow| {
                let retry_count = u32::try_from(retry_count)
                    .map_err(|_| journal_fault("LSV4007", "dispatch retry count is invalid"))?;
                if (retry_count == 0) != last_error.is_none() {
                    return Err(journal_fault(
                        "LSV4007",
                        "dispatch retry history is inconsistent",
                    ));
                }
                if let Some(bytes) = last_error {
                    let error: EffectError = decode_bounded(&bytes, 2_048)?;
                    validate_effect_error(&error)?;
                }
                Ok(DispatchRecord {
                    request,
                    state,
                    attempt: u32::try_from(attempt)
                        .map_err(|_| journal_fault("LSV4007", "dispatch attempt is invalid"))?,
                    lease_expires_at_ms: expires
                        .map(|value| {
                            u64::try_from(value).map_err(|_| {
                                journal_fault("LSV4007", "dispatch lease expiration is invalid")
                            })
                        })
                        .transpose()?,
                    ready_at_ms: u64::try_from(ready_at)
                        .map_err(|_| journal_fault("LSV4007", "dispatch ready clock is invalid"))?,
                    retry_count,
                })
            },
        )
        .transpose()
}

fn ensure_dispatch_matches(
    transaction: &rusqlite::Transaction<'_>,
    token: &str,
    request: &EffectRequest,
) -> Result<(), Fault> {
    let Some(existing) = load_dispatch(transaction, token)? else {
        return Err(journal_fault(
            "LSV4011",
            "pending effect is missing its durable dispatch",
        ));
    };
    let durable_request: EffectRequest =
        decode_bounded(&existing.request, MAX_JOURNAL_ENTRY_BYTES)?;
    if durable_request != *request {
        return Err(journal_fault(
            "LSV4011",
            "effect request conflicts with durable dispatch state",
        ));
    }
    Ok(())
}

fn validate_dispatch_records(connection: &Connection) -> Result<(), Fault> {
    let mut statement = connection
        .prepare(
            "SELECT d.token, d.request, d.state, d.attempt, d.lease_expires_at_ms,
                    d.ready_at_ms, d.retry_count, d.last_error, e.state, e.image
             FROM vm_dispatches d JOIN vm_effects e ON e.token = d.token
             ORDER BY d.token ASC",
        )
        .map_err(|error| journal_error("LSV4005", "failed to prepare dispatch load", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| journal_error("LSV4005", "failed to load dispatch records", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| journal_error("LSV4005", "failed to read dispatch record", error))?
    {
        let token: String = row
            .get(0)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch token", error))?;
        let bytes: Vec<u8> = row
            .get(1)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch request", error))?;
        let request: EffectRequest = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_effect_request(&request)?;
        if request.continuation.token.as_str() != token {
            return Err(journal_fault(
                "LSV4007",
                "dispatch token does not match effect request",
            ));
        }
        let effect_image: Vec<u8> = row.get(9).map_err(|error| {
            journal_error("LSV4005", "invalid dispatch continuation image", error)
        })?;
        let request_image = encode_json_capped(
            &request.continuation,
            MAX_CONTINUATION_BYTES,
            "continuation",
        )?;
        if request_image != effect_image {
            return Err(journal_fault(
                "LSV4007",
                "dispatch request conflicts with continuation state",
            ));
        }
        let state: String = row
            .get(2)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch state", error))?;
        let attempt: i64 = row
            .get(3)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch attempt", error))?;
        let effect_state: String = row
            .get(8)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch effect state", error))?;
        let ready_at: i64 = row
            .get(5)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch ready clock", error))?;
        let retry_count: i64 = row
            .get(6)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch retry count", error))?;
        let last_error: Option<Vec<u8>> = row
            .get(7)
            .map_err(|error| journal_error("LSV4005", "invalid dispatch retry error", error))?;
        if attempt < 0 || attempt > i64::from(MAX_DISPATCH_ATTEMPTS) {
            return Err(journal_fault("LSV4007", "dispatch attempt is invalid"));
        }
        if ready_at < 0 || retry_count < 0 || retry_count > i64::from(MAX_SEMANTIC_RETRIES) {
            return Err(journal_fault("LSV4007", "dispatch retry state is invalid"));
        }
        if (retry_count == 0) != last_error.is_none() {
            return Err(journal_fault(
                "LSV4007",
                "dispatch retry history is inconsistent",
            ));
        }
        if let Some(bytes) = last_error {
            let error: EffectError = decode_bounded(&bytes, 2_048)?;
            validate_effect_error(&error)?;
        }
        if (state == "acknowledged") != (effect_state == "completed") {
            return Err(journal_fault(
                "LSV4007",
                "dispatch and continuation states are inconsistent",
            ));
        }
    }
    Ok(())
}

fn validate_lease_clock(now_ms: u64, lease_ms: u64) -> Result<(), Fault> {
    if now_ms > i64::MAX as u64 {
        return Err(journal_fault("LSV4015", "dispatch clock is out of range"));
    }
    if lease_ms == 0 || lease_ms > MAX_DISPATCH_LEASE_MS {
        return Err(journal_fault(
            "LSV4015",
            format!("dispatch lease must be between 1 and {MAX_DISPATCH_LEASE_MS} ms"),
        ));
    }
    lease_expiration(now_ms, lease_ms).map(|_| ())
}

fn lease_expiration(now_ms: u64, lease_ms: u64) -> Result<u64, Fault> {
    now_ms
        .checked_add(lease_ms)
        .filter(|value| *value <= i64::MAX as u64)
        .ok_or_else(|| journal_fault("LSV4015", "dispatch lease expiration is out of range"))
}

fn next_attempt(current: u32) -> Result<u32, Fault> {
    let next = current
        .checked_add(1)
        .ok_or_else(|| journal_fault("LSV4017", "dispatch attempt counter is exhausted"))?;
    if next > MAX_DISPATCH_ATTEMPTS {
        return Err(journal_fault(
            "LSV4017",
            format!("dispatch attempt limit {MAX_DISPATCH_ATTEMPTS} reached"),
        ));
    }
    Ok(next)
}

fn decode_bounded<T: serde::de::DeserializeOwned>(bytes: &[u8], limit: usize) -> Result<T, Fault> {
    if bytes.len() > limit {
        return Err(journal_fault(
            "LSV4006",
            format!("journal entry exceeds {limit} bytes"),
        ));
    }
    serde_json::from_slice(bytes)
        .map_err(|error| journal_error("LSV4007", "invalid journal JSON", error))
}

fn validate_terminal_step(step: &Step) -> Result<(), Fault> {
    match step {
        Step::Done(Value::RuntimeList { runtimes, .. }) if runtimes.len() <= 10_000 => Ok(()),
        Step::Done(Value::RuntimeRefresh { .. }) => Ok(()),
        Step::Cancelled(cancellation)
            if !cancellation.continuation.as_str().is_empty()
                && cancellation.observed_at_ms <= i64::MAX as u64 =>
        {
            Ok(())
        }
        Step::Failed(failure)
            if failure.retry_count <= MAX_SEMANTIC_RETRIES
                && validate_effect_error(&failure.error).is_ok() =>
        {
            Ok(())
        }
        Step::Fault(fault) if !fault.code.is_empty() && fault.code.len() <= 32 => Ok(()),
        Step::Done(_) => Err(journal_fault(
            "LSV4007",
            "completed journal output exceeds runtime limit",
        )),
        Step::Effect(_) | Step::Yield(_) => Err(journal_fault(
            "LSV4007",
            "journal completion must be a terminal step",
        )),
        Step::Fault(_) | Step::Cancelled(_) | Step::Failed(_) => Err(journal_fault(
            "LSV4007",
            "journal fault has an invalid diagnostic code",
        )),
    }
}

fn validate_cancellation_step(image: &ContinuationImage, step: &Step) -> Result<(), Fault> {
    validate_terminal_step(step)?;
    match step {
        Step::Cancelled(cancellation) if cancellation.continuation == image.token => Ok(()),
        _ => Err(journal_fault(
            "LSV4007",
            "cancellation does not match continuation image",
        )),
    }
}

fn deadline_cancellation(image: &ContinuationImage, observed_at_ms: u64) -> Step {
    Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason: CancellationReason::DeadlineExceeded,
        observed_at_ms,
    })
}

#[cfg(unix)]
fn tighten_permissions(path: &Path) -> Result<(), Fault> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| journal_error("LSV4002", "failed to secure journal permissions", error))
}

#[cfg(unix)]
fn create_private_journal(path: &Path) -> Result<(), Fault> {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map(|_| ())
        .map_err(|error| journal_error("LSV4002", "failed to create private journal", error))
}

#[cfg(not(unix))]
fn create_private_journal(path: &Path) -> Result<(), Fault> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|error| journal_error("LSV4002", "failed to create private journal", error))
}

#[cfg(not(unix))]
fn tighten_permissions(_path: &Path) -> Result<(), Fault> {
    Ok(())
}

fn journal_error(code: &str, context: &str, error: impl std::fmt::Display) -> Fault {
    journal_fault(code, format!("{context}: {error}"))
}

fn journal_fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.to_string(),
        message: message.into(),
    }
}
