use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::types::Value;
use rusqlite::{
    Connection, MappedRows, OpenFlags, Row, Transaction, TransactionBehavior, params,
    params_from_iter,
};

use leserpent_domain::{RuntimeId, RuntimeLogLevel, RuntimeLogRecord};

use crate::{EFFECT_QUEUE_CAPACITY, EffectEnqueue, EffectQueueStats, MAX_EFFECT_ENQUEUE_BATCH};

const RUNTIME_JOURNAL_SCHEMA_VERSION: i64 = 13;
pub const AUTHORITY_KIND_DAEMON_BOOTSTRAP: &str = "daemon_bootstrap";
pub const AUTHORITY_KIND_GEWYVERN_PROVISIONING: &str = "gewyvern_provisioning";
pub const AUTHORITY_KIND_GEWYVERN_RETIREMENT: &str = "gewyvern_retirement";
const MAX_JOURNAL_RECORDS: i64 = 100_000;
const MAX_JOURNAL_PAYLOAD_BYTES: usize = 64 * 1024;
const MAX_SNAPSHOT_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
const MAX_COMPACTION_RECORDS: i64 = 1_000;
const OWNER_LEASE_DURATION_MS: i64 = 30_000;
const MAX_EFFECT_TASKS: i64 = EFFECT_QUEUE_CAPACITY as i64;
const MAX_EFFECT_LEASE_MS: i64 = 5 * 60 * 1_000;
const MAX_ORCHESTRA_ENVELOPE_BYTES: usize = 1024 * 1024;
pub const MAX_PERSISTED_RUNTIME_LOG_ENTRIES: i64 = 4_096;
static OWNER_TOKEN_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalEntryKind {
    RuntimeRegistration,
    RuntimeUnregistration,
    CommandPlan,
    RuntimeStatusObservation,
    RuntimeCapabilityObservation,
}

impl JournalEntryKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeRegistration => "runtime_registration",
            Self::RuntimeUnregistration => "runtime_unregistration",
            Self::CommandPlan => "command_plan",
            Self::RuntimeStatusObservation => "runtime_status_observation",
            Self::RuntimeCapabilityObservation => "runtime_capability_observation",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "runtime_registration" => Ok(Self::RuntimeRegistration),
            "runtime_unregistration" => Ok(Self::RuntimeUnregistration),
            "command_plan" => Ok(Self::CommandPlan),
            "runtime_status_observation" => Ok(Self::RuntimeStatusObservation),
            "runtime_capability_observation" => Ok(Self::RuntimeCapabilityObservation),
            other => Err(format!("unknown runtime journal entry kind '{other}'")),
        }
    }
}

pub struct JournalEntry {
    pub sequence: i64,
    pub kind: JournalEntryKind,
    pub payload: Vec<u8>,
    pub outcome: Option<Vec<u8>>,
    pub terminal_error: Option<String>,
    pub created_at_unix_ms: i64,
}

pub struct JournalAppend {
    pub sequence: i64,
    pub created_at_unix_ms: i64,
}

pub struct JournalSnapshot {
    pub generation: i64,
    pub schema_version: u32,
    pub through_sequence: i64,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectLease {
    pub effect_id: String,
    pub kind: String,
    pub payload: Vec<u8>,
    pub attempt: u32,
    lease_token: String,
    pub expires_at_unix_ms: i64,
}

pub struct EffectRecord {
    pub kind: String,
    pub payload: Vec<u8>,
    pub state: String,
    pub attempt: u32,
    pub outcome: Option<Vec<u8>>,
    pub last_error: Option<String>,
}

pub struct OrchestraPersistenceRecord {
    pub run: Vec<u8>,
    pub event: Vec<u8>,
    pub event_count: u64,
}

pub struct OrchestraHistoryRecord {
    pub runs: Vec<Vec<u8>>,
    pub events: Vec<(u64, Vec<u8>)>,
    pub next_offset: Option<u32>,
}

pub struct OrchestraDeleteRecord {
    pub deleted_runtime_count: u32,
    pub deleted_run_count: u64,
    pub deleted_event_count: u64,
}

pub struct AuthorityCheckpointRecord {
    pub revision: u64,
    pub phase: String,
    pub payload: Vec<u8>,
}

pub struct Journal {
    connection: Connection,
    owner_token: String,
}

impl Journal {
    pub fn open(path: &Path) -> Result<Self, String> {
        reject_unsafe_path(path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        create_private_file_if_missing(path)?;
        let open_path = canonical_open_path(path)?;
        let mut connection = Connection::open_with_flags(
            &open_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(|error| error.to_string())?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=DELETE;
                 PRAGMA synchronous=FULL;
                 PRAGMA foreign_keys=ON;
                 CREATE TABLE IF NOT EXISTS runtime_metadata (
                     key TEXT PRIMARY KEY,
                     value INTEGER NOT NULL
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS runtime_journal (
                     sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                     kind TEXT NOT NULL,
                     payload BLOB NOT NULL,
                     outcome BLOB,
                     terminal_error TEXT
                 ) STRICT;",
            )
            .map_err(|error| error.to_string())?;
        let schema: Option<i64> = connection
            .query_row(
                "SELECT value FROM runtime_metadata WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let mut schema = match schema {
            Some(version) if version > RUNTIME_JOURNAL_SCHEMA_VERSION => {
                return Err(format!(
                    "unsupported runtime journal schema {version}, expected {RUNTIME_JOURNAL_SCHEMA_VERSION}"
                ));
            }
            None => {
                connection
                    .execute(
                        "INSERT INTO runtime_metadata (key, value) VALUES ('schema_version', ?1)",
                        [1_i64],
                    )
                    .map_err(|error| error.to_string())?;
                1
            }
            Some(version) if version >= 1 => version,
            Some(version) => {
                return Err(format!("unsupported runtime journal schema {version}"));
            }
        };
        while schema < RUNTIME_JOURNAL_SCHEMA_VERSION {
            schema = migrate_schema(&mut connection, schema)?;
        }
        validate_current_schema(&connection)?;
        let owner_token = new_owner_token()?;
        acquire_owner(&mut connection, &owner_token)?;
        set_private_permissions(path)?;
        Ok(Self {
            connection,
            owner_token,
        })
    }

    pub fn append_stamped(
        &mut self,
        kind: JournalEntryKind,
        payload: &[u8],
    ) -> Result<JournalAppend, String> {
        self.ensure_owner()?;
        validate_blob("payload", payload)?;
        let count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM runtime_journal", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        if count >= MAX_JOURNAL_RECORDS {
            return Err(format!(
                "runtime journal record limit {MAX_JOURNAL_RECORDS} reached"
            ));
        }
        let created_at_unix_ms = unix_time_ms()?;
        self.connection
            .execute(
                "INSERT INTO runtime_journal (kind, payload, created_at_unix_ms) VALUES (?1, ?2, ?3)",
                params![kind.as_str(), payload, created_at_unix_ms],
            )
            .map_err(|error| error.to_string())?;
        Ok(JournalAppend {
            sequence: self.connection.last_insert_rowid(),
            created_at_unix_ms,
        })
    }

    pub fn complete(&mut self, sequence: i64, outcome: &[u8]) -> Result<(), String> {
        self.ensure_owner()?;
        validate_blob("outcome", outcome)?;
        let changed = self
            .connection
            .execute(
                "UPDATE runtime_journal SET outcome = ?1 WHERE sequence = ?2 AND outcome IS NULL AND terminal_error IS NULL",
                params![outcome, sequence],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err(format!(
                "runtime journal sequence {sequence} is missing or already complete"
            ));
        }
        Ok(())
    }

    pub fn fail(&mut self, sequence: i64, error: &str) -> Result<(), String> {
        self.ensure_owner()?;
        if error.len() > MAX_JOURNAL_PAYLOAD_BYTES {
            return Err("runtime journal terminal error is too large".into());
        }
        let changed = self
            .connection
            .execute(
                "UPDATE runtime_journal SET terminal_error = ?1 WHERE sequence = ?2 AND outcome IS NULL AND terminal_error IS NULL",
                params![error, sequence],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err(format!(
                "runtime journal sequence {sequence} is missing or already terminal"
            ));
        }
        Ok(())
    }

    pub fn load(&self, after_sequence: i64) -> Result<Vec<JournalEntry>, String> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT sequence, kind, payload, outcome, terminal_error, created_at_unix_ms
                 FROM runtime_journal
                 WHERE sequence > ?1
                 ORDER BY sequence ASC
                 LIMIT ?2",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![after_sequence, MAX_JOURNAL_RECORDS + 1], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut entries = Vec::new();
        for row in rows {
            let (sequence, kind, payload, outcome, terminal_error, created_at_unix_ms) =
                row.map_err(|error| error.to_string())?;
            if entries.len() as i64 >= MAX_JOURNAL_RECORDS {
                return Err(format!(
                    "runtime journal exceeds record limit {MAX_JOURNAL_RECORDS}"
                ));
            }
            validate_blob("payload", &payload)?;
            if let Some(outcome) = &outcome {
                validate_blob("outcome", outcome)?;
            }
            entries.push(JournalEntry {
                sequence,
                kind: JournalEntryKind::parse(&kind)?,
                payload,
                outcome,
                terminal_error,
                created_at_unix_ms,
            });
        }
        Ok(entries)
    }

    pub fn append_runtime_log(
        &mut self,
        runtime_id: &RuntimeId,
        level: RuntimeLogLevel,
        message: &str,
    ) -> Result<u64, String> {
        self.ensure_owner()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO runtime_logs (runtime_id, level, message, created_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    runtime_id.as_str(),
                    log_level_label(level),
                    message,
                    unix_time_ms()?
                ],
            )
            .map_err(|error| error.to_string())?;
        let sequence = transaction.last_insert_rowid();
        transaction
            .execute(
                "DELETE FROM runtime_logs
                 WHERE runtime_id = ?1 AND sequence <= (
                     SELECT COALESCE(MAX(sequence), 0) FROM (
                         SELECT sequence FROM runtime_logs
                         WHERE runtime_id = ?1
                         ORDER BY sequence DESC
                         LIMIT -1 OFFSET ?2
                     )
                 )",
                params![runtime_id.as_str(), MAX_PERSISTED_RUNTIME_LOG_ENTRIES],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        u64::try_from(sequence).map_err(|_| "runtime log sequence is out of range".to_string())
    }

    pub fn load_runtime_logs(
        &self,
        runtime_id: &RuntimeId,
        after_sequence: Option<u64>,
        limit: u16,
    ) -> Result<Vec<RuntimeLogRecord>, String> {
        let rows = match after_sequence {
            Some(value) => {
                let after = match i64::try_from(value) {
                    Ok(value) => value,
                    Err(_) => return Ok(Vec::new()),
                };
                let mut statement = self
                    .connection
                    .prepare(
                        "SELECT sequence, level, message FROM runtime_logs
                         WHERE runtime_id = ?1 AND sequence > ?2
                         ORDER BY sequence ASC LIMIT ?3",
                    )
                    .map_err(|error| error.to_string())?;
                collect_log_rows(
                    statement
                        .query_map(
                            params![runtime_id.as_str(), after, i64::from(limit)],
                            map_log_row,
                        )
                        .map_err(|error| error.to_string())?,
                )?
            }
            None => {
                let mut statement = self
                    .connection
                    .prepare(
                        "SELECT sequence, level, message FROM (
                             SELECT sequence, level, message FROM runtime_logs
                             WHERE runtime_id = ?1 ORDER BY sequence DESC LIMIT ?2
                         ) ORDER BY sequence ASC",
                    )
                    .map_err(|error| error.to_string())?;
                collect_log_rows(
                    statement
                        .query_map(params![runtime_id.as_str(), i64::from(limit)], map_log_row)
                        .map_err(|error| error.to_string())?,
                )?
            }
        };
        let mut records = Vec::with_capacity(rows.len());
        for (sequence, level, message) in rows {
            records.push(RuntimeLogRecord {
                sequence: u64::try_from(sequence)
                    .map_err(|_| "runtime log sequence is out of range".to_string())?,
                level: parse_log_level(&level)?,
                message,
            });
        }
        Ok(records)
    }

    pub fn load_snapshots(&self) -> Result<Vec<JournalSnapshot>, String> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT generation, domain_schema, through_sequence, payload, checksum
                 FROM runtime_snapshots ORDER BY generation DESC LIMIT 2",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut snapshots = Vec::new();
        let mut found = 0;
        for row in rows {
            found += 1;
            let (generation, schema_version, through_sequence, payload, checksum) =
                row.map_err(|error| error.to_string())?;
            if validate_snapshot_blob(&payload).is_err()
                || checksum != snapshot_checksum(schema_version, through_sequence, &payload)
            {
                continue;
            }
            snapshots.push(JournalSnapshot {
                generation,
                schema_version,
                through_sequence,
                payload,
            });
        }
        if found != 0 && snapshots.is_empty() {
            return Err("all runtime snapshot generations failed integrity validation".into());
        }
        Ok(snapshots)
    }

    pub fn save_snapshot(&mut self, domain_schema: u32, payload: &[u8]) -> Result<i64, String> {
        self.ensure_owner()?;
        validate_snapshot_blob(payload)?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let pending: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM runtime_journal
                 WHERE outcome IS NULL AND terminal_error IS NULL AND kind = 'command_plan'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if pending != 0 {
            return Err("runtime snapshot requires a fully terminal journal".into());
        }
        let through_sequence: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM runtime_journal",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let generation: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(generation), 0) + 1 FROM runtime_snapshots",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO runtime_snapshots
                     (generation, domain_schema, through_sequence, payload, checksum, created_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    generation,
                    i64::from(domain_schema),
                    through_sequence,
                    payload,
                    snapshot_checksum(domain_schema, through_sequence, payload),
                    unix_time_ms()?
                ],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM runtime_snapshots
                 WHERE generation NOT IN (
                     SELECT generation FROM runtime_snapshots ORDER BY generation DESC LIMIT 2
                 )",
                [],
            )
            .map_err(|error| error.to_string())?;
        let fallback_boundary: Option<i64> = transaction
            .query_row(
                "SELECT MIN(through_sequence) FROM runtime_snapshots
                 HAVING COUNT(*) = 2",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .flatten();
        if let Some(boundary) = fallback_boundary {
            transaction
                .execute(
                    "DELETE FROM runtime_journal WHERE sequence IN (
                         SELECT sequence FROM runtime_journal
                         WHERE sequence <= ?1
                         ORDER BY sequence ASC LIMIT ?2
                     )",
                    params![boundary, MAX_COMPACTION_RECORDS],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(through_sequence)
    }

    pub fn enqueue_effect(
        &mut self,
        effect_id: &str,
        kind: &str,
        payload: &[u8],
        max_attempts: u32,
    ) -> Result<(), String> {
        self.enqueue_effect_batch(&[EffectEnqueue {
            effect_id: effect_id.to_string(),
            kind: kind.to_string(),
            payload: payload.to_vec(),
            max_attempts,
        }])?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn enqueue_effect_with_authority_checkpoint(
        &mut self,
        effect_id: &str,
        kind: &str,
        effect_payload: &[u8],
        max_attempts: u32,
        authority_kind: &str,
        operation_id: &str,
        phase: &str,
        revision: u64,
        checkpoint: &[u8],
    ) -> Result<(), String> {
        self.ensure_owner()?;
        validate_scheduler_id("effect_id", effect_id)?;
        validate_scheduler_id("effect kind", kind)?;
        validate_scheduler_id("authority checkpoint kind", authority_kind)?;
        validate_scheduler_id("authority operation_id", operation_id)?;
        validate_authority_phase(authority_kind, phase)?;
        validate_blob("effect payload", effect_payload)?;
        validate_blob("bootstrap checkpoint", checkpoint)?;
        if max_attempts == 0 || max_attempts > 100 {
            return Err("effect max_attempts must be between 1 and 100".into());
        }
        let revision = i64::try_from(revision)
            .map_err(|_| "authority checkpoint revision is out of range".to_string())?;
        if revision == 0 {
            return Err("authority checkpoint revision must be positive".into());
        }
        let now = unix_time_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let existing_effect: Option<(String, Vec<u8>, i64)> = transaction
            .query_row(
                "SELECT kind, payload, max_attempts FROM runtime_effect_tasks WHERE effect_id = ?1",
                [effect_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let existing_checkpoint: Option<(i64, String, Vec<u8>)> = transaction
            .query_row(
                "SELECT revision, phase, checkpoint FROM authority_checkpoints
                 WHERE operation_kind = ?1 AND operation_id = ?2",
                params![authority_kind, operation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        match (existing_effect, existing_checkpoint) {
            (
                Some((stored_kind, stored_payload, stored_attempts)),
                Some((stored_revision, stored_phase, stored_checkpoint)),
            ) if stored_kind == kind
                && stored_payload == effect_payload
                && stored_attempts == i64::from(max_attempts)
                && stored_revision == revision
                && stored_phase == phase
                && stored_checkpoint == checkpoint =>
            {
                transaction.commit().map_err(|error| error.to_string())?;
                return Ok(());
            }
            (Some(_), Some(_)) => {
                return Err("authority operation identity was reused with different input".into());
            }
            (None, None) => {}
            _ => return Err("authority submission persistence is incomplete".into()),
        }
        let active: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM runtime_effect_tasks WHERE state IN ('ready', 'leased')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if active >= MAX_EFFECT_TASKS {
            return Err(format!(
                "runtime effect queue limit {MAX_EFFECT_TASKS} reached"
            ));
        }
        transaction
            .execute(
                "INSERT INTO runtime_effect_tasks
                     (effect_id, kind, payload, state, attempt, max_attempts,
                      available_at_unix_ms, created_at_unix_ms, updated_at_unix_ms)
                 VALUES (?1, ?2, ?3, 'ready', 0, ?4, ?5, ?5, ?5)",
                params![
                    effect_id,
                    kind,
                    effect_payload,
                    i64::from(max_attempts),
                    now
                ],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO authority_checkpoints
                     (operation_kind, operation_id, revision, phase, checkpoint,
                      updated_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    authority_kind,
                    operation_id,
                    revision,
                    phase,
                    checkpoint,
                    now
                ],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn effect_record(&mut self, effect_id: &str) -> Result<Option<EffectRecord>, String> {
        self.ensure_owner()?;
        validate_scheduler_id("effect_id", effect_id)?;
        type EffectRecordRow = (
            String,
            Vec<u8>,
            String,
            i64,
            Option<Vec<u8>>,
            Option<String>,
        );
        let record: Option<EffectRecordRow> = self
            .connection
            .query_row(
                "SELECT kind, payload, state, attempt, outcome, last_error
                 FROM runtime_effect_tasks WHERE effect_id = ?1",
                [effect_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?;
        record
            .map(|(kind, payload, state, attempt, outcome, last_error)| {
                if !matches!(state.as_str(), "ready" | "leased" | "completed" | "failed") {
                    return Err("effect record has an invalid state".to_string());
                }
                Ok(EffectRecord {
                    kind,
                    payload,
                    state,
                    attempt: u32::try_from(attempt)
                        .map_err(|_| "effect record has an invalid attempt".to_string())?,
                    outcome,
                    last_error,
                })
            })
            .transpose()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn persist_orchestra_run_event(
        &mut self,
        run_id: &str,
        runtime_id: &str,
        request_id: Option<&str>,
        event_type: &str,
        to_outcome: &str,
        recorded_at: &str,
        run: &[u8],
        event: &[u8],
    ) -> Result<OrchestraPersistenceRecord, String> {
        self.ensure_owner()?;
        for (field, value) in [
            ("run_id", run_id),
            ("runtime_id", runtime_id),
            ("event_type", event_type),
            ("to_outcome", to_outcome),
        ] {
            validate_scheduler_id(field, value)?;
        }
        if let Some(request_id) = request_id {
            validate_scheduler_id("request_id", request_id)?;
        }
        if recorded_at.is_empty()
            || recorded_at.len() > 64
            || recorded_at.chars().any(char::is_control)
        {
            return Err("recorded_at is invalid".into());
        }
        validate_orchestra_blob("run envelope", run)?;
        validate_orchestra_blob("event envelope", event)?;
        let now = unix_time_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let existing_run: Option<(String, Vec<u8>)> = transaction
            .query_row(
                "SELECT runtime_id, envelope FROM orchestra_runs WHERE run_id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if existing_run
            .as_ref()
            .is_some_and(|(existing_runtime, _)| existing_runtime != runtime_id)
        {
            return Err("Orchestra run identity was reused for another runtime".into());
        }
        let existing_event: Option<Vec<u8>> = transaction
            .query_row(
                "SELECT envelope FROM orchestra_events
                 WHERE run_id = ?1 AND event_type = ?2 AND to_outcome = ?3 AND recorded_at = ?4",
                params![run_id, event_type, to_outcome, recorded_at],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        match existing_event {
            Some(existing) if existing != event => {
                return Err("Orchestra event identity was reused with different content".into());
            }
            Some(_)
                if existing_run
                    .as_ref()
                    .is_some_and(|(_, stored)| stored != run) =>
            {
                return Err("Orchestra event replay changed its run content".into());
            }
            Some(_) => {}
            None => {
                transaction
                    .execute(
                        "INSERT INTO orchestra_runs (
                             run_id, runtime_id, request_id, envelope, updated_at_unix_ms)
                         VALUES (?1, ?2, ?3, ?4, ?5)
                         ON CONFLICT(run_id) DO UPDATE SET
                             request_id = excluded.request_id,
                             envelope = excluded.envelope,
                             updated_at_unix_ms = excluded.updated_at_unix_ms",
                        params![run_id, runtime_id, request_id, run, now],
                    )
                    .map_err(|error| error.to_string())?;
                transaction
                    .execute(
                        "DELETE FROM orchestra_runs
                         WHERE runtime_id = ?1 AND run_id NOT IN (
                             SELECT run_id FROM orchestra_runs
                             WHERE runtime_id = ?1
                             ORDER BY updated_at_unix_ms DESC, run_id ASC LIMIT 32
                         )",
                        [runtime_id],
                    )
                    .map_err(|error| error.to_string())?;
                transaction
                    .execute(
                        "INSERT INTO orchestra_events (
                             run_id, runtime_id, event_type, to_outcome, recorded_at,
                             envelope, created_at_unix_ms)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            run_id,
                            runtime_id,
                            event_type,
                            to_outcome,
                            recorded_at,
                            event,
                            now
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
        }
        let stored_run: Vec<u8> = transaction
            .query_row(
                "SELECT envelope FROM orchestra_runs WHERE run_id = ?1 AND runtime_id = ?2",
                params![run_id, runtime_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let stored_event: Vec<u8> = transaction
            .query_row(
                "SELECT envelope FROM orchestra_events
                 WHERE run_id = ?1 AND event_type = ?2 AND to_outcome = ?3 AND recorded_at = ?4",
                params![run_id, event_type, to_outcome, recorded_at],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let event_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM orchestra_events WHERE run_id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(OrchestraPersistenceRecord {
            run: stored_run,
            event: stored_event,
            event_count: u64::try_from(event_count)
                .map_err(|_| "invalid Orchestra event count".to_string())?,
        })
    }

    pub fn load_orchestra_history(
        &mut self,
        runtime_id: Option<&str>,
        run_id: Option<&str>,
        offset: u32,
        limit: u16,
    ) -> Result<OrchestraHistoryRecord, String> {
        self.ensure_owner()?;
        if limit == 0 || limit > 64 || offset > 10_000 {
            return Err("Orchestra history page is out of bounds".into());
        }
        if let Some(runtime_id) = runtime_id {
            validate_scheduler_id("runtime_id", runtime_id)?;
        }
        if let Some(run_id) = run_id {
            validate_scheduler_id("run_id", run_id)?;
            if runtime_id.is_none() {
                return Err("Orchestra event history requires runtime_id".into());
            }
        }
        let fetch = i64::from(limit) + 1;
        let offset = i64::from(offset);
        if let Some(run_id) = run_id {
            let mut statement = self
                .connection
                .prepare(
                    "SELECT event_id, envelope FROM orchestra_events
                     WHERE runtime_id = ?1 AND run_id = ?2
                     ORDER BY event_id ASC LIMIT ?3 OFFSET ?4",
                )
                .map_err(|error| error.to_string())?;
            let raw_rows = statement
                .query_map(
                    params![runtime_id.unwrap_or_default(), run_id, fetch, offset],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
                )
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<(i64, Vec<u8>)>, _>>()
                .map_err(|error| error.to_string())?;
            let mut rows = raw_rows
                .into_iter()
                .map(|(event_id, envelope)| {
                    u64::try_from(event_id)
                        .map(|event_id| (event_id, envelope))
                        .map_err(|_| "Orchestra event ID is invalid".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let has_more = rows.len() > usize::from(limit);
            rows.truncate(usize::from(limit));
            return Ok(OrchestraHistoryRecord {
                runs: Vec::new(),
                events: rows,
                next_offset: history_next_offset(has_more, offset, limit),
            });
        }
        let mut rows = if let Some(runtime_id) = runtime_id {
            let mut statement = self
                .connection
                .prepare(
                    "SELECT envelope FROM orchestra_runs WHERE runtime_id = ?1
                     ORDER BY updated_at_unix_ms DESC, run_id ASC LIMIT ?2 OFFSET ?3",
                )
                .map_err(|error| error.to_string())?;
            statement
                .query_map(params![runtime_id, fetch, offset], |row| row.get(0))
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<Vec<u8>>, _>>()
                .map_err(|error| error.to_string())?
        } else {
            let mut statement = self
                .connection
                .prepare(
                    "SELECT envelope FROM orchestra_runs
                     ORDER BY updated_at_unix_ms DESC, run_id ASC LIMIT ?1 OFFSET ?2",
                )
                .map_err(|error| error.to_string())?;
            statement
                .query_map(params![fetch, offset], |row| row.get(0))
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<Vec<u8>>, _>>()
                .map_err(|error| error.to_string())?
        };
        let has_more = rows.len() > usize::from(limit);
        rows.truncate(usize::from(limit));
        Ok(OrchestraHistoryRecord {
            runs: rows,
            events: Vec::new(),
            next_offset: history_next_offset(has_more, offset, limit),
        })
    }

    pub fn delete_orchestra_runtimes(
        &mut self,
        runtime_ids: &[String],
    ) -> Result<OrchestraDeleteRecord, String> {
        self.ensure_owner()?;
        if runtime_ids.is_empty() || runtime_ids.len() > 128 {
            return Err("Orchestra delete must contain between 1 and 128 runtime IDs".into());
        }
        let mut unique = BTreeSet::new();
        for runtime_id in runtime_ids {
            validate_scheduler_id("runtime_id", runtime_id)?;
            if !unique.insert(runtime_id.as_str()) {
                return Err("Orchestra delete contains a duplicate runtime ID".into());
            }
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let mut deleted_runtime_count = 0_u32;
        let mut deleted_run_count = 0_u64;
        let mut deleted_event_count = 0_u64;
        for runtime_id in runtime_ids {
            let run_count: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM orchestra_runs WHERE runtime_id = ?1",
                    [runtime_id],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            let event_count: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM orchestra_events WHERE runtime_id = ?1",
                    [runtime_id],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            if run_count > 0 {
                deleted_runtime_count = deleted_runtime_count.saturating_add(1);
            }
            deleted_run_count = deleted_run_count
                .checked_add(u64::try_from(run_count).map_err(|_| "invalid run count")?)
                .ok_or_else(|| "Orchestra run count overflow".to_string())?;
            deleted_event_count = deleted_event_count
                .checked_add(u64::try_from(event_count).map_err(|_| "invalid event count")?)
                .ok_or_else(|| "Orchestra event count overflow".to_string())?;
            transaction
                .execute(
                    "DELETE FROM orchestra_runs WHERE runtime_id = ?1",
                    [runtime_id],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(OrchestraDeleteRecord {
            deleted_runtime_count,
            deleted_run_count,
            deleted_event_count,
        })
    }

    pub fn enqueue_effect_batch(&mut self, effects: &[EffectEnqueue]) -> Result<u64, String> {
        self.ensure_owner()?;
        if effects.is_empty() || effects.len() > MAX_EFFECT_ENQUEUE_BATCH {
            return Err(format!(
                "effect enqueue batch must contain between 1 and {MAX_EFFECT_ENQUEUE_BATCH} tasks"
            ));
        }
        let mut batch_ids = BTreeSet::new();
        for effect in effects {
            validate_scheduler_id("effect_id", &effect.effect_id)?;
            validate_scheduler_id("effect kind", &effect.kind)?;
            validate_blob("effect payload", &effect.payload)?;
            if effect.max_attempts == 0 || effect.max_attempts > 100 {
                return Err("effect max_attempts must be between 1 and 100".into());
            }
            if !batch_ids.insert(effect.effect_id.as_str()) {
                return Err(format!(
                    "effect id '{}' is duplicated within the enqueue batch",
                    effect.effect_id
                ));
            }
        }
        let now = unix_time_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let active: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM runtime_effect_tasks
                 WHERE state IN ('ready', 'leased')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let mut new_effects = Vec::with_capacity(effects.len());
        for effect in effects {
            let existing: Option<(String, Vec<u8>, i64)> = transaction
                .query_row(
                    "SELECT kind, payload, max_attempts FROM runtime_effect_tasks
                     WHERE effect_id = ?1",
                    [&effect.effect_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            match existing {
                Some((kind, payload, max_attempts))
                    if kind == effect.kind
                        && payload == effect.payload
                        && max_attempts == i64::from(effect.max_attempts) => {}
                Some(_) => {
                    return Err(format!(
                        "effect id '{}' was reused with different input",
                        effect.effect_id
                    ));
                }
                None => new_effects.push(effect),
            }
        }
        let new_count = i64::try_from(new_effects.len())
            .map_err(|_| "effect enqueue batch is too large".to_string())?;
        if active.saturating_add(new_count) > MAX_EFFECT_TASKS {
            return Err(format!(
                "runtime effect queue limit {MAX_EFFECT_TASKS} reached"
            ));
        }
        for effect in new_effects {
            transaction
                .execute(
                    "INSERT INTO runtime_effect_tasks
                     (effect_id, kind, payload, state, attempt, max_attempts,
                      available_at_unix_ms, created_at_unix_ms, updated_at_unix_ms)
                 VALUES (?1, ?2, ?3, 'ready', 0, ?4, ?5, ?5, ?5)",
                    params![
                        effect.effect_id,
                        effect.kind,
                        effect.payload,
                        i64::from(effect.max_attempts),
                        now
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        u64::try_from(new_count).map_err(|_| "effect enqueue count overflow".into())
    }

    pub fn effect_queue_stats(&mut self) -> Result<EffectQueueStats, String> {
        self.ensure_owner()?;
        let (ready, leased, completed, failed): (i64, i64, i64, i64) = self
            .connection
            .query_row(
                "SELECT
                     COALESCE(SUM(CASE WHEN state = 'ready' THEN 1 ELSE 0 END), 0),
                     COALESCE(SUM(CASE WHEN state = 'leased' THEN 1 ELSE 0 END), 0),
                     COALESCE(SUM(CASE WHEN state = 'completed' THEN 1 ELSE 0 END), 0),
                     COALESCE(SUM(CASE WHEN state = 'failed' THEN 1 ELSE 0 END), 0)
                 FROM runtime_effect_tasks",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|error| error.to_string())?;
        Ok(EffectQueueStats {
            ready: u64::try_from(ready).map_err(|_| "invalid ready effect count")?,
            leased: u64::try_from(leased).map_err(|_| "invalid leased effect count")?,
            completed: u64::try_from(completed).map_err(|_| "invalid completed effect count")?,
            failed: u64::try_from(failed).map_err(|_| "invalid failed effect count")?,
            capacity: EFFECT_QUEUE_CAPACITY,
        })
    }

    pub fn prune_terminal_effects(&mut self, retain: u64, batch_limit: u64) -> Result<u64, String> {
        self.ensure_owner()?;
        if batch_limit == 0 || batch_limit > 1_000 {
            return Err("effect retention batch_limit must be between 1 and 1000".into());
        }
        let retain = i64::try_from(retain)
            .map_err(|_| "effect terminal retention is too large".to_string())?;
        let batch_limit = i64::try_from(batch_limit)
            .map_err(|_| "effect retention batch limit is too large".to_string())?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let terminal: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM runtime_effect_tasks
                 WHERE state IN ('completed', 'failed')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let remove = terminal.saturating_sub(retain).min(batch_limit);
        if remove == 0 {
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(0);
        }
        let changed = transaction
            .execute(
                "DELETE FROM runtime_effect_tasks WHERE effect_id IN (
                     SELECT effect_id FROM runtime_effect_tasks
                     WHERE state IN ('completed', 'failed')
                     ORDER BY updated_at_unix_ms ASC, created_at_unix_ms ASC, effect_id ASC
                     LIMIT ?1
                 )",
                [remove],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        u64::try_from(changed).map_err(|_| "effect retention count overflow".into())
    }

    pub fn claim_effect_excluding(
        &mut self,
        worker_id: &str,
        lease_duration: Duration,
        excluded_kinds: &[String],
    ) -> Result<Option<EffectLease>, String> {
        self.ensure_owner()?;
        validate_scheduler_id("worker_id", worker_id)?;
        for kind in excluded_kinds {
            validate_scheduler_id("excluded effect kind", kind)?;
        }
        let lease_ms = validate_lease_duration(lease_duration)?;
        let now = unix_time_ms()?;
        let expires_at = now
            .checked_add(lease_ms)
            .ok_or_else(|| "effect lease timestamp overflow".to_string())?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "UPDATE runtime_effect_tasks SET state = 'failed',
                     last_error = COALESCE(last_error, 'lease expired after final attempt'),
                     lease_token = NULL, lease_expires_at_unix_ms = NULL,
                     updated_at_unix_ms = ?1
                 WHERE state = 'leased' AND lease_expires_at_unix_ms <= ?1
                   AND attempt >= max_attempts",
                [now],
            )
            .map_err(|error| error.to_string())?;
        let excluded_clause = if excluded_kinds.is_empty() {
            String::new()
        } else {
            let placeholders = (2..excluded_kinds.len() + 2)
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!(" AND kind NOT IN ({placeholders})")
        };
        let query = format!(
            "SELECT effect_id, kind, payload, attempt FROM runtime_effect_tasks
                 WHERE attempt < max_attempts AND (
                     (state = 'ready' AND available_at_unix_ms <= ?1) OR
                     (state = 'leased' AND lease_expires_at_unix_ms <= ?1)
                 ){excluded_clause}
                 ORDER BY available_at_unix_ms, created_at_unix_ms, effect_id LIMIT 1"
        );
        let mut query_params = Vec::with_capacity(excluded_kinds.len() + 1);
        query_params.push(Value::Integer(now));
        query_params.extend(excluded_kinds.iter().cloned().map(Value::Text));
        let candidate: Option<(String, String, Vec<u8>, i64)> = transaction
            .query_row(&query, params_from_iter(query_params.iter()), |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .optional()
            .map_err(|error| error.to_string())?;
        let Some((effect_id, kind, payload, previous_attempt)) = candidate else {
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(None);
        };
        let attempt = previous_attempt + 1;
        let lease_token = format!("{worker_id}:{}", new_owner_token()?);
        let changed = transaction
            .execute(
                "UPDATE runtime_effect_tasks SET
                     state = 'leased', attempt = ?1, lease_token = ?2,
                     lease_expires_at_unix_ms = ?3, updated_at_unix_ms = ?4
                 WHERE effect_id = ?5 AND attempt = ?6 AND (
                     (state = 'ready' AND available_at_unix_ms <= ?4) OR
                     (state = 'leased' AND lease_expires_at_unix_ms <= ?4)
                 )",
                params![
                    attempt,
                    lease_token,
                    expires_at,
                    now,
                    effect_id,
                    previous_attempt
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("effect claim lost its transactional candidate".into());
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(Some(EffectLease {
            effect_id,
            kind,
            payload,
            attempt: u32::try_from(attempt).map_err(|_| "effect attempt overflow")?,
            lease_token,
            expires_at_unix_ms: expires_at,
        }))
    }

    pub fn renew_effect(
        &mut self,
        lease: &EffectLease,
        lease_duration: Duration,
    ) -> Result<EffectLease, String> {
        self.ensure_owner()?;
        let lease_ms = validate_lease_duration(lease_duration)?;
        let now = unix_time_ms()?;
        let expires_at = now
            .checked_add(lease_ms)
            .ok_or_else(|| "effect lease timestamp overflow".to_string())?;
        let changed = self
            .connection
            .execute(
                "UPDATE runtime_effect_tasks SET lease_expires_at_unix_ms = ?1, updated_at_unix_ms = ?2
                 WHERE effect_id = ?3 AND state = 'leased' AND lease_token = ?4
                   AND attempt = ?5 AND lease_expires_at_unix_ms > ?2",
                params![expires_at, now, lease.effect_id, lease.lease_token, i64::from(lease.attempt)],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("effect lease was lost or expired".into());
        }
        let mut renewed = lease.clone();
        renewed.expires_at_unix_ms = expires_at;
        Ok(renewed)
    }

    pub fn complete_effect(&mut self, lease: &EffectLease, outcome: &[u8]) -> Result<(), String> {
        self.finish_effect(lease, Some(outcome), None, Duration::ZERO)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_effect_with_authority_checkpoint(
        &mut self,
        lease: &EffectLease,
        outcome: &[u8],
        authority_kind: &str,
        operation_id: &str,
        phase: &str,
        revision: u64,
        checkpoint: &[u8],
    ) -> Result<(), String> {
        self.ensure_owner()?;
        validate_scheduler_id("authority checkpoint kind", authority_kind)?;
        validate_scheduler_id("authority operation_id", operation_id)?;
        validate_authority_phase(authority_kind, phase)?;
        validate_blob("effect outcome", outcome)?;
        validate_blob("bootstrap checkpoint", checkpoint)?;
        let revision = i64::try_from(revision)
            .map_err(|_| "authority checkpoint revision is out of range".to_string())?;
        if revision == 0 {
            return Err("authority checkpoint revision must be positive".into());
        }
        let now = unix_time_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let existing: Option<(i64, String, Vec<u8>)> = transaction
            .query_row(
                "SELECT revision, phase, checkpoint FROM authority_checkpoints
                 WHERE operation_kind = ?1 AND operation_id = ?2",
                params![authority_kind, operation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        match existing {
            Some((stored_revision, _, stored))
                if stored_revision == revision && stored == checkpoint => {}
            Some((stored_revision, stored_phase, _))
                if stored_phase == "planned"
                    && stored_revision.checked_add(1) == Some(revision) =>
            {
                let changed = transaction
                    .execute(
                        "UPDATE authority_checkpoints SET revision = ?1, phase = ?2,
                             checkpoint = ?3, updated_at_unix_ms = ?4
                         WHERE operation_kind = ?5 AND operation_id = ?6
                           AND revision = ?7 AND phase = 'planned'",
                        params![
                            revision,
                            phase,
                            checkpoint,
                            now,
                            authority_kind,
                            operation_id,
                            stored_revision
                        ],
                    )
                    .map_err(|error| error.to_string())?;
                if changed != 1 {
                    return Err("authority checkpoint changed during effect completion".into());
                }
            }
            Some(_) => {
                return Err("authority operation identity was reused with different state".into());
            }
            None if revision == 1 => {
                transaction
                    .execute(
                        "INSERT INTO authority_checkpoints
                             (operation_kind, operation_id, revision, phase, checkpoint,
                              updated_at_unix_ms)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            authority_kind,
                            operation_id,
                            revision,
                            phase,
                            checkpoint,
                            now
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
            None => return Err("authority planned checkpoint is missing".into()),
        }
        complete_leased_effect(&transaction, lease, outcome, now)?;
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn commit_provisioning_registration(
        &mut self,
        leased_effect: Option<(&EffectLease, &[u8])>,
        registration: Option<&[u8]>,
        operation_id: &str,
        expected_revision: u64,
        final_revision: u64,
        checkpoint: &[u8],
    ) -> Result<i64, String> {
        self.ensure_owner()?;
        validate_scheduler_id("authority operation_id", operation_id)?;
        validate_blob("provisioning checkpoint", checkpoint)?;
        if let Some((_, outcome)) = leased_effect {
            validate_blob("effect outcome", outcome)?;
        }
        if let Some(payload) = registration {
            validate_blob("runtime registration", payload)?;
        }
        let expected_revision = i64::try_from(expected_revision)
            .map_err(|_| "authority checkpoint revision is out of range".to_string())?;
        let final_revision = i64::try_from(final_revision)
            .map_err(|_| "authority checkpoint revision is out of range".to_string())?;
        let expected_phase = if leased_effect.is_some() {
            if expected_revision != 1 || final_revision != 3 {
                return Err("new provisioning registration must advance revision 1 to 3".into());
            }
            "planned"
        } else {
            if expected_revision != 2 || final_revision != 3 {
                return Err("ready provisioning recovery must advance revision 2 to 3".into());
            }
            "service_ready"
        };
        let now = unix_time_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let stored: Option<(i64, String)> = transaction
            .query_row(
                "SELECT revision, phase FROM authority_checkpoints
                 WHERE operation_kind = ?1 AND operation_id = ?2",
                params![AUTHORITY_KIND_GEWYVERN_PROVISIONING, operation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if stored != Some((expected_revision, expected_phase.to_string())) {
            return Err("provisioning checkpoint changed before registration".into());
        }
        if let Some(payload) = registration {
            let count: i64 = transaction
                .query_row("SELECT COUNT(*) FROM runtime_journal", [], |row| row.get(0))
                .map_err(|error| error.to_string())?;
            if count >= MAX_JOURNAL_RECORDS {
                return Err(format!(
                    "runtime journal record limit {MAX_JOURNAL_RECORDS} reached"
                ));
            }
            transaction
                .execute(
                    "INSERT INTO runtime_journal (kind, payload, created_at_unix_ms)
                     VALUES (?1, ?2, ?3)",
                    params![JournalEntryKind::RuntimeRegistration.as_str(), payload, now],
                )
                .map_err(|error| error.to_string())?;
        }
        let changed = transaction
            .execute(
                "UPDATE authority_checkpoints SET revision = ?1, phase = 'runtime_registered',
                     checkpoint = ?2, updated_at_unix_ms = ?3
                 WHERE operation_kind = ?4 AND operation_id = ?5
                   AND revision = ?6 AND phase = ?7",
                params![
                    final_revision,
                    checkpoint,
                    now,
                    AUTHORITY_KIND_GEWYVERN_PROVISIONING,
                    operation_id,
                    expected_revision,
                    expected_phase
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("provisioning checkpoint changed during registration".into());
        }
        if let Some((lease, outcome)) = leased_effect {
            complete_leased_effect(&transaction, lease, outcome, now)?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(now)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn commit_retirement_unregistration(
        &mut self,
        lease: &EffectLease,
        outcome: &[u8],
        unregistration: &[u8],
        operation_id: &str,
        expected_revision: u64,
        final_revision: u64,
        checkpoint: &[u8],
    ) -> Result<(), String> {
        self.ensure_owner()?;
        validate_scheduler_id("authority operation_id", operation_id)?;
        validate_blob("effect outcome", outcome)?;
        validate_blob("runtime unregistration", unregistration)?;
        validate_blob("retirement checkpoint", checkpoint)?;
        if expected_revision != 1 || final_revision != 3 {
            return Err("retirement unregistration must advance revision 1 to 3".into());
        }
        let expected_revision = i64::try_from(expected_revision)
            .map_err(|_| "authority checkpoint revision is out of range".to_string())?;
        let final_revision = i64::try_from(final_revision)
            .map_err(|_| "authority checkpoint revision is out of range".to_string())?;
        let now = unix_time_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let stored: Option<(i64, String)> = transaction
            .query_row(
                "SELECT revision, phase FROM authority_checkpoints
                 WHERE operation_kind = ?1 AND operation_id = ?2",
                params![AUTHORITY_KIND_GEWYVERN_RETIREMENT, operation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if stored != Some((expected_revision, "planned".to_string())) {
            return Err("retirement checkpoint changed before unregistration".into());
        }
        let count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM runtime_journal", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        if count >= MAX_JOURNAL_RECORDS {
            return Err(format!(
                "runtime journal record limit {MAX_JOURNAL_RECORDS} reached"
            ));
        }
        transaction
            .execute(
                "INSERT INTO runtime_journal (kind, payload, created_at_unix_ms)
                 VALUES (?1, ?2, ?3)",
                params![
                    JournalEntryKind::RuntimeUnregistration.as_str(),
                    unregistration,
                    now
                ],
            )
            .map_err(|error| error.to_string())?;
        let changed = transaction
            .execute(
                "UPDATE authority_checkpoints SET revision = ?1,
                     phase = 'runtime_unregistered', checkpoint = ?2,
                     updated_at_unix_ms = ?3
                 WHERE operation_kind = ?4 AND operation_id = ?5
                   AND revision = ?6 AND phase = 'planned'",
                params![
                    final_revision,
                    checkpoint,
                    now,
                    AUTHORITY_KIND_GEWYVERN_RETIREMENT,
                    operation_id,
                    expected_revision
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("retirement checkpoint changed during unregistration".into());
        }
        complete_leased_effect(&transaction, lease, outcome, now)?;
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn authority_checkpoint(
        &mut self,
        authority_kind: &str,
        operation_id: &str,
    ) -> Result<Option<AuthorityCheckpointRecord>, String> {
        self.ensure_owner()?;
        validate_scheduler_id("authority checkpoint kind", authority_kind)?;
        validate_scheduler_id("authority operation_id", operation_id)?;
        let record: Option<(i64, String, Vec<u8>)> = self
            .connection
            .query_row(
                "SELECT revision, phase, checkpoint FROM authority_checkpoints
                 WHERE operation_kind = ?1 AND operation_id = ?2",
                params![authority_kind, operation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        record
            .map(|(revision, phase, payload)| {
                validate_authority_phase(authority_kind, &phase)?;
                validate_blob("authority checkpoint", &payload)?;
                Ok(AuthorityCheckpointRecord {
                    revision: u64::try_from(revision)
                        .map_err(|_| "authority checkpoint revision is invalid".to_string())?,
                    phase,
                    payload,
                })
            })
            .transpose()
    }

    pub fn update_authority_checkpoint(
        &mut self,
        authority_kind: &str,
        operation_id: &str,
        expected_revision: u64,
        phase: &str,
        checkpoint: &[u8],
    ) -> Result<(), String> {
        self.ensure_owner()?;
        validate_scheduler_id("authority checkpoint kind", authority_kind)?;
        validate_scheduler_id("authority operation_id", operation_id)?;
        validate_authority_phase(authority_kind, phase)?;
        validate_blob("authority checkpoint", checkpoint)?;
        let expected_revision = i64::try_from(expected_revision)
            .map_err(|_| "authority checkpoint revision is out of range".to_string())?;
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or_else(|| "authority checkpoint revision overflow".to_string())?;
        let changed = self
            .connection
            .execute(
                "UPDATE authority_checkpoints SET revision = ?1, phase = ?2,
                     checkpoint = ?3, updated_at_unix_ms = ?4
                 WHERE operation_kind = ?5 AND operation_id = ?6 AND revision = ?7",
                params![
                    next_revision,
                    phase,
                    checkpoint,
                    unix_time_ms()?,
                    authority_kind,
                    operation_id,
                    expected_revision
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("authority checkpoint was missing or concurrently changed".into());
        }
        Ok(())
    }

    pub fn complete_effect_with_journal(
        &mut self,
        lease: &EffectLease,
        kind: JournalEntryKind,
        payload: &[u8],
        journal_outcome: &[u8],
        effect_outcome: &[u8],
    ) -> Result<i64, String> {
        self.ensure_owner()?;
        validate_blob("payload", payload)?;
        validate_blob("journal outcome", journal_outcome)?;
        validate_blob("effect outcome", effect_outcome)?;
        let now = unix_time_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM runtime_journal", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        if count >= MAX_JOURNAL_RECORDS {
            return Err(format!(
                "runtime journal record limit {MAX_JOURNAL_RECORDS} reached"
            ));
        }
        transaction
            .execute(
                "INSERT INTO runtime_journal
                     (kind, payload, outcome, created_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                params![kind.as_str(), payload, journal_outcome, now],
            )
            .map_err(|error| error.to_string())?;
        let changed = transaction
            .execute(
                "UPDATE runtime_effect_tasks SET state = 'completed', outcome = ?1,
                     lease_token = NULL, lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?2
                 WHERE effect_id = ?3 AND state = 'leased' AND lease_token = ?4
                   AND attempt = ?5 AND lease_expires_at_unix_ms > ?2",
                params![
                    effect_outcome,
                    now,
                    lease.effect_id,
                    lease.lease_token,
                    i64::from(lease.attempt)
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("effect lease was lost or expired".into());
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(now)
    }

    pub fn fail_effect(
        &mut self,
        lease: &EffectLease,
        error: &str,
        retry_after: Duration,
    ) -> Result<(), String> {
        self.finish_effect(lease, None, Some(error), retry_after)
    }

    pub fn reject_effect(&mut self, lease: &EffectLease, error: &str) -> Result<(), String> {
        self.ensure_owner()?;
        if error.len() > MAX_JOURNAL_PAYLOAD_BYTES {
            return Err("effect error is too large".into());
        }
        let now = unix_time_ms()?;
        let changed = self
            .connection
            .execute(
                "UPDATE runtime_effect_tasks SET state = 'failed', last_error = ?1,
                     lease_token = NULL, lease_expires_at_unix_ms = NULL,
                     updated_at_unix_ms = ?2
                 WHERE effect_id = ?3 AND state = 'leased' AND lease_token = ?4
                   AND attempt = ?5 AND lease_expires_at_unix_ms > ?2",
                params![
                    error,
                    now,
                    lease.effect_id,
                    lease.lease_token,
                    i64::from(lease.attempt)
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("effect lease was lost or expired".into());
        }
        Ok(())
    }

    fn finish_effect(
        &mut self,
        lease: &EffectLease,
        outcome: Option<&[u8]>,
        error: Option<&str>,
        retry_after: Duration,
    ) -> Result<(), String> {
        self.ensure_owner()?;
        if let Some(outcome) = outcome {
            validate_blob("effect outcome", outcome)?;
        }
        if error.is_some_and(|error| error.len() > MAX_JOURNAL_PAYLOAD_BYTES) {
            return Err("effect error is too large".into());
        }
        let retry_ms = i64::try_from(retry_after.as_millis())
            .map_err(|_| "effect retry delay is too large".to_string())?;
        let now = unix_time_ms()?;
        let available_at = now
            .checked_add(retry_ms)
            .ok_or_else(|| "effect retry timestamp overflow".to_string())?;
        let changed = if let Some(outcome) = outcome {
            self.connection.execute(
                "UPDATE runtime_effect_tasks SET state = 'completed', outcome = ?1,
                     lease_token = NULL, lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?2
                 WHERE effect_id = ?3 AND state = 'leased' AND lease_token = ?4
                   AND attempt = ?5 AND lease_expires_at_unix_ms > ?2",
                params![
                    outcome,
                    now,
                    lease.effect_id,
                    lease.lease_token,
                    i64::from(lease.attempt)
                ],
            )
        } else {
            self.connection.execute(
                "UPDATE runtime_effect_tasks SET
                     state = CASE WHEN attempt >= max_attempts THEN 'failed' ELSE 'ready' END,
                     last_error = ?1, available_at_unix_ms = ?2,
                     lease_token = NULL, lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?3
                 WHERE effect_id = ?4 AND state = 'leased' AND lease_token = ?5
                   AND attempt = ?6 AND lease_expires_at_unix_ms > ?3",
                params![
                    error,
                    available_at,
                    now,
                    lease.effect_id,
                    lease.lease_token,
                    i64::from(lease.attempt)
                ],
            )
        }
        .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("effect lease was lost or expired".into());
        }
        Ok(())
    }

    pub fn ensure_owner(&mut self) -> Result<(), String> {
        let expires_at = unix_time_ms()?
            .checked_add(OWNER_LEASE_DURATION_MS)
            .ok_or_else(|| "runtime owner lease timestamp overflow".to_string())?;
        let changed = self
            .connection
            .execute(
                "UPDATE runtime_owner SET lease_expires_at_unix_ms = ?1
                 WHERE id = 1 AND owner_token = ?2",
                params![expires_at, self.owner_token],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("runtime journal ownership lease was lost".into());
        }
        Ok(())
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        let _ = self.connection.execute(
            "DELETE FROM runtime_owner WHERE id = 1 AND owner_token = ?1",
            [&self.owner_token],
        );
    }
}

fn complete_leased_effect(
    transaction: &Transaction<'_>,
    lease: &EffectLease,
    outcome: &[u8],
    now: i64,
) -> Result<(), String> {
    let changed = transaction
        .execute(
            "UPDATE runtime_effect_tasks SET state = 'completed', outcome = ?1,
                 lease_token = NULL, lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?2
             WHERE effect_id = ?3 AND state = 'leased' AND lease_token = ?4
               AND attempt = ?5 AND lease_expires_at_unix_ms > ?2",
            params![
                outcome,
                now,
                lease.effect_id,
                lease.lease_token,
                i64::from(lease.attempt)
            ],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("effect lease was lost or expired".into());
    }
    Ok(())
}

fn validate_authority_phase(operation_kind: &str, phase: &str) -> Result<(), String> {
    let valid = match operation_kind {
        AUTHORITY_KIND_DAEMON_BOOTSTRAP => matches!(
            phase,
            "planned" | "deploying" | "bootstrapped" | "session_bound" | "failed"
        ),
        AUTHORITY_KIND_GEWYVERN_PROVISIONING => matches!(
            phase,
            "planned" | "installing" | "service_ready" | "runtime_registered" | "failed"
        ),
        AUTHORITY_KIND_GEWYVERN_RETIREMENT => matches!(
            phase,
            "planned" | "retiring_service" | "service_retired" | "runtime_unregistered" | "failed"
        ),
        _ => false,
    };
    valid
        .then_some(())
        .ok_or_else(|| "authority checkpoint kind or phase is invalid".to_string())
}

fn migrate_schema(connection: &mut Connection, from: i64) -> Result<i64, String> {
    match from {
        1 => migrate_schema_1_to_2(connection),
        2 => migrate_schema_2_to_3(connection),
        3 => migrate_schema_3_to_4(connection),
        4 => migrate_schema_4_to_5(connection),
        5 => migrate_schema_5_to_6(connection),
        6 => migrate_schema_6_to_7(connection),
        7 => migrate_schema_7_to_8(connection),
        8 => migrate_schema_8_to_9(connection),
        9 => migrate_schema_9_to_10(connection),
        10 => migrate_schema_10_to_11(connection),
        11 => migrate_schema_11_to_12(connection),
        12 => migrate_schema_12_to_13(connection),
        version => Err(format!(
            "no runtime journal migration from schema {version}"
        )),
    }
}

fn migrate_schema_12_to_13(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "DROP INDEX authority_checkpoints_by_kind_phase;
             CREATE TABLE authority_checkpoints_v13 (
                 operation_kind TEXT NOT NULL CHECK (
                     operation_kind IN (
                         'daemon_bootstrap', 'gewyvern_provisioning', 'gewyvern_retirement'
                     )
                 ),
                 operation_id TEXT NOT NULL,
                 revision INTEGER NOT NULL CHECK (revision >= 1),
                 phase TEXT NOT NULL,
                 checkpoint BLOB NOT NULL CHECK (length(checkpoint) <= 65536),
                 updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0),
                 PRIMARY KEY (operation_kind, operation_id),
                 CHECK (
                     (operation_kind = 'daemon_bootstrap' AND phase IN (
                         'planned', 'deploying', 'bootstrapped', 'session_bound', 'failed'
                     )) OR
                     (operation_kind = 'gewyvern_provisioning' AND phase IN (
                         'planned', 'installing', 'service_ready', 'runtime_registered', 'failed'
                     )) OR
                     (operation_kind = 'gewyvern_retirement' AND phase IN (
                         'planned', 'retiring_service', 'service_retired',
                         'runtime_unregistered', 'failed'
                     ))
                 )
             ) STRICT;
             INSERT INTO authority_checkpoints_v13
                 (operation_kind, operation_id, revision, phase, checkpoint, updated_at_unix_ms)
             SELECT operation_kind, operation_id, revision, phase, checkpoint,
                    updated_at_unix_ms
             FROM authority_checkpoints;
             DROP TABLE authority_checkpoints;
             ALTER TABLE authority_checkpoints_v13 RENAME TO authority_checkpoints;
             CREATE INDEX authority_checkpoints_by_kind_phase
                 ON authority_checkpoints
                    (operation_kind, phase, updated_at_unix_ms DESC);",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (13, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 13 WHERE key = 'schema_version' AND value = 12",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(13)
}

fn migrate_schema_11_to_12(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE authority_checkpoints (
                 operation_kind TEXT NOT NULL CHECK (
                     operation_kind IN ('daemon_bootstrap', 'gewyvern_provisioning')
                 ),
                 operation_id TEXT NOT NULL,
                 revision INTEGER NOT NULL CHECK (revision >= 1),
                 phase TEXT NOT NULL,
                 checkpoint BLOB NOT NULL CHECK (length(checkpoint) <= 65536),
                 updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0),
                 PRIMARY KEY (operation_kind, operation_id),
                 CHECK (
                     (operation_kind = 'daemon_bootstrap' AND phase IN (
                         'planned', 'deploying', 'bootstrapped', 'session_bound', 'failed'
                     )) OR
                     (operation_kind = 'gewyvern_provisioning' AND phase IN (
                         'planned', 'installing', 'service_ready', 'runtime_registered', 'failed'
                     ))
                 )
             ) STRICT;
             INSERT INTO authority_checkpoints
                 (operation_kind, operation_id, revision, phase, checkpoint, updated_at_unix_ms)
             SELECT 'daemon_bootstrap', bootstrap_id, revision, phase, checkpoint,
                    updated_at_unix_ms
             FROM bootstrap_handoffs;
             DROP INDEX bootstrap_handoffs_by_phase;
             DROP TABLE bootstrap_handoffs;
             CREATE INDEX authority_checkpoints_by_kind_phase
                 ON authority_checkpoints
                    (operation_kind, phase, updated_at_unix_ms DESC);",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (12, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 12 WHERE key = 'schema_version' AND value = 11",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(12)
}

fn migrate_schema_10_to_11(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE bootstrap_handoffs (
                 bootstrap_id TEXT PRIMARY KEY,
                 revision INTEGER NOT NULL CHECK (revision >= 1),
                 phase TEXT NOT NULL CHECK (
                     phase IN ('planned', 'deploying', 'bootstrapped', 'session_bound', 'failed')
                 ),
                 checkpoint BLOB NOT NULL CHECK (length(checkpoint) <= 65536),
                 updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0)
             ) STRICT;
             CREATE INDEX bootstrap_handoffs_by_phase
                 ON bootstrap_handoffs (phase, updated_at_unix_ms DESC);",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (11, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 11 WHERE key = 'schema_version' AND value = 10",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(11)
}

fn migrate_schema_9_to_10(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE orchestra_runs (
                 run_id TEXT PRIMARY KEY,
                 runtime_id TEXT NOT NULL,
                 request_id TEXT,
                 envelope BLOB NOT NULL CHECK (length(envelope) <= 1048576),
                 updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0)
             ) STRICT;
             CREATE INDEX orchestra_runs_by_runtime
                 ON orchestra_runs (runtime_id, updated_at_unix_ms DESC);
             CREATE UNIQUE INDEX orchestra_runs_by_runtime_request
                 ON orchestra_runs (runtime_id, request_id) WHERE request_id IS NOT NULL;
             CREATE TABLE orchestra_events (
                 event_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 run_id TEXT NOT NULL,
                 runtime_id TEXT NOT NULL,
                 event_type TEXT NOT NULL,
                 to_outcome TEXT NOT NULL,
                 recorded_at TEXT NOT NULL,
                 envelope BLOB NOT NULL CHECK (length(envelope) <= 1048576),
                 created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
                 UNIQUE (run_id, event_type, to_outcome, recorded_at),
                 FOREIGN KEY (run_id) REFERENCES orchestra_runs(run_id) ON DELETE CASCADE
             ) STRICT;
             CREATE INDEX orchestra_events_by_run
                 ON orchestra_events (runtime_id, run_id, event_id);",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (10, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 10 WHERE key = 'schema_version' AND value = 9",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(10)
}

fn migrate_schema_8_to_9(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (9, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 9 WHERE key = 'schema_version' AND value = 8",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(9)
}

fn migrate_schema_7_to_8(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE runtime_logs (
                 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                 runtime_id TEXT NOT NULL,
                 level TEXT NOT NULL CHECK (level IN ('trace', 'debug', 'info', 'warning', 'error')),
                 message TEXT NOT NULL CHECK (length(CAST(message AS BLOB)) <= 65536),
                 created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0)
             ) STRICT;
             CREATE INDEX runtime_logs_by_runtime_sequence
                 ON runtime_logs (runtime_id, sequence);",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (8, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 8 WHERE key = 'schema_version' AND value = 7",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(8)
}

fn migrate_schema_6_to_7(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (7, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 7 WHERE key = 'schema_version' AND value = 6",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(7)
}

fn migrate_schema_5_to_6(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE runtime_effect_tasks (
             effect_id TEXT PRIMARY KEY,
             kind TEXT NOT NULL,
             payload BLOB NOT NULL,
             state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'completed', 'failed')),
             attempt INTEGER NOT NULL CHECK (attempt >= 0),
             max_attempts INTEGER NOT NULL CHECK (max_attempts BETWEEN 1 AND 100),
             available_at_unix_ms INTEGER NOT NULL CHECK (available_at_unix_ms >= 0),
             lease_token TEXT,
             lease_expires_at_unix_ms INTEGER,
             outcome BLOB,
             last_error TEXT,
             created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
             updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0)
         ) STRICT;
         CREATE INDEX runtime_effect_claim
             ON runtime_effect_tasks (state, available_at_unix_ms, created_at_unix_ms);",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (6, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 6 WHERE key = 'schema_version' AND value = 5",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(6)
}

fn migrate_schema_4_to_5(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE runtime_owner (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 owner_token TEXT NOT NULL,
                 lease_expires_at_unix_ms INTEGER NOT NULL
                     CHECK (lease_expires_at_unix_ms >= 0)
             ) STRICT;",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (5, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 5 WHERE key = 'schema_version' AND value = 4",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(5)
}

fn migrate_schema_3_to_4(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "ALTER TABLE runtime_snapshots RENAME TO runtime_snapshots_v3;
             CREATE TABLE runtime_snapshots (
                 generation INTEGER PRIMARY KEY CHECK (generation >= 1),
                 domain_schema INTEGER NOT NULL CHECK (domain_schema >= 1),
                 through_sequence INTEGER NOT NULL CHECK (through_sequence >= 0),
                 payload BLOB NOT NULL,
                 checksum TEXT NOT NULL,
                 created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0)
             ) STRICT;
             INSERT INTO runtime_snapshots
                 (generation, domain_schema, through_sequence, payload, checksum, created_at_unix_ms)
             SELECT 1, domain_schema, through_sequence, payload, checksum, created_at_unix_ms
             FROM runtime_snapshots_v3;
             DROP TABLE runtime_snapshots_v3;",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (4, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 4 WHERE key = 'schema_version' AND value = 3",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(4)
}

fn migrate_schema_2_to_3(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE runtime_snapshots (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 domain_schema INTEGER NOT NULL CHECK (domain_schema >= 1),
                 through_sequence INTEGER NOT NULL CHECK (through_sequence >= 0),
                 payload BLOB NOT NULL,
                 checksum TEXT NOT NULL,
                 created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0)
             ) STRICT;",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (3, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 3 WHERE key = 'schema_version' AND value = 2",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(3)
}

fn migrate_schema_1_to_2(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "ALTER TABLE runtime_journal
                 ADD COLUMN created_at_unix_ms INTEGER NOT NULL DEFAULT 0
                 CHECK (created_at_unix_ms >= 0);
             CREATE INDEX runtime_journal_kind_sequence
                 ON runtime_journal (kind, sequence);
             CREATE TABLE runtime_schema_migrations (
                 version INTEGER PRIMARY KEY,
                 applied_at_unix_ms INTEGER NOT NULL CHECK (applied_at_unix_ms >= 0)
             ) STRICT;",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (1, 0), (2, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 2 WHERE key = 'schema_version' AND value = 1",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(2)
}

fn validate_current_schema(connection: &Connection) -> Result<(), String> {
    let (migration_count, first_migration, last_migration): (i64, i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(MIN(version), 0), COALESCE(MAX(version), 0)
             FROM runtime_schema_migrations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    if (migration_count, first_migration, last_migration) != (13, 1, 13) {
        return Err("invalid runtime journal schema 13 migration history".into());
    }
    let timestamp_column: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runtime_journal') WHERE name = 'created_at_unix_ms'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if timestamp_column != 1 {
        return Err("invalid runtime journal schema 13 timestamp column".into());
    }
    connection
        .query_row("SELECT COUNT(*) FROM runtime_snapshots", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    connection
        .query_row("SELECT COUNT(*) FROM runtime_owner", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    connection
        .query_row("SELECT COUNT(*) FROM runtime_effect_tasks", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    let effect_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runtime_effect_tasks')
             WHERE name IN (
                 'effect_id', 'kind', 'payload', 'state', 'attempt', 'max_attempts',
                 'available_at_unix_ms', 'lease_token', 'lease_expires_at_unix_ms',
                 'outcome', 'last_error', 'created_at_unix_ms', 'updated_at_unix_ms'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    if effect_columns != 13 {
        return Err("invalid runtime journal schema 13 effect columns".into());
    }
    let claim_index: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'runtime_effect_claim'
               AND tbl_name = 'runtime_effect_tasks'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    if claim_index != 1 {
        return Err("invalid runtime journal schema 13 effect claim index".into());
    }
    let unknown_journal_kinds: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM runtime_journal
             WHERE kind NOT IN (
                 'runtime_registration', 'command_plan', 'runtime_status_observation',
                 'runtime_capability_observation', 'runtime_unregistration'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    if unknown_journal_kinds != 0 {
        return Err("invalid runtime journal schema 13 journal kind".into());
    }
    let log_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runtime_logs')
             WHERE name IN ('sequence', 'runtime_id', 'level', 'message', 'created_at_unix_ms')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    if log_columns != 5 {
        return Err("invalid runtime journal schema 13 log columns".into());
    }
    let log_index: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'runtime_logs_by_runtime_sequence'
               AND tbl_name = 'runtime_logs'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    if log_index != 1 {
        return Err("invalid runtime journal schema 13 log index".into());
    }
    let orchestra_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name IN ('orchestra_runs', 'orchestra_events')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    if orchestra_tables != 2 {
        return Err("invalid runtime journal schema 13 Orchestra tables".into());
    }
    let orchestra_indexes: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index'
               AND name IN (
                   'orchestra_runs_by_runtime', 'orchestra_runs_by_runtime_request',
                   'orchestra_events_by_run')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    if orchestra_indexes != 3 {
        return Err("invalid runtime journal schema 13 Orchestra indexes".into());
    }
    let authority_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('authority_checkpoints')
             WHERE name IN (
                 'operation_kind', 'operation_id', 'revision', 'phase', 'checkpoint',
                 'updated_at_unix_ms'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    if authority_columns != 6 {
        return Err("invalid runtime journal schema 13 authority checkpoint columns".into());
    }
    let authority_index: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'authority_checkpoints_by_kind_phase'
               AND tbl_name = 'authority_checkpoints'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 13: {error}"))?;
    if authority_index != 1 {
        return Err("invalid runtime journal schema 13 authority checkpoint index".into());
    }
    Ok(())
}

fn log_level_label(level: RuntimeLogLevel) -> &'static str {
    match level {
        RuntimeLogLevel::Trace => "trace",
        RuntimeLogLevel::Debug => "debug",
        RuntimeLogLevel::Info => "info",
        RuntimeLogLevel::Warning => "warning",
        RuntimeLogLevel::Error => "error",
    }
}

fn parse_log_level(value: &str) -> Result<RuntimeLogLevel, String> {
    match value {
        "trace" => Ok(RuntimeLogLevel::Trace),
        "debug" => Ok(RuntimeLogLevel::Debug),
        "info" => Ok(RuntimeLogLevel::Info),
        "warning" => Ok(RuntimeLogLevel::Warning),
        "error" => Ok(RuntimeLogLevel::Error),
        _ => Err(format!("invalid persisted runtime log level '{value}'")),
    }
}

fn map_log_row(row: &Row<'_>) -> rusqlite::Result<(i64, String, String)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

fn collect_log_rows<F>(rows: MappedRows<'_, F>) -> Result<Vec<(i64, String, String)>, String>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<(i64, String, String)>,
{
    rows.map(|row| row.map_err(|error| error.to_string()))
        .collect()
}

fn acquire_owner(connection: &mut Connection, owner_token: &str) -> Result<(), String> {
    let now = unix_time_ms()?;
    let expires_at = now
        .checked_add(OWNER_LEASE_DURATION_MS)
        .ok_or_else(|| "runtime owner lease timestamp overflow".to_string())?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "INSERT INTO runtime_owner (id, owner_token, lease_expires_at_unix_ms)
             VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET
                 owner_token = excluded.owner_token,
                 lease_expires_at_unix_ms = excluded.lease_expires_at_unix_ms
             WHERE runtime_owner.lease_expires_at_unix_ms <= ?3",
            params![owner_token, expires_at, now],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal is owned by another live process".into());
    }
    transaction.commit().map_err(|error| error.to_string())
}

fn new_owner_token() -> Result<String, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let sequence = OWNER_TOKEN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(format!("{}-{nanos}-{sequence}", std::process::id()))
}

fn validate_scheduler_id(label: &str, value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'));
    valid
        .then_some(())
        .ok_or_else(|| format!("invalid {label}"))
}

fn validate_lease_duration(duration: Duration) -> Result<i64, String> {
    let millis = i64::try_from(duration.as_millis())
        .map_err(|_| "effect lease duration is too large".to_string())?;
    if !(1..=MAX_EFFECT_LEASE_MS).contains(&millis) {
        return Err(format!(
            "effect lease duration must be between 1 and {MAX_EFFECT_LEASE_MS} milliseconds"
        ));
    }
    Ok(millis)
}

pub(crate) fn unix_time_ms() -> Result<i64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    i64::try_from(millis).map_err(|_| "system time exceeds SQLite integer range".into())
}

fn validate_blob(label: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_JOURNAL_PAYLOAD_BYTES {
        return Err(format!(
            "runtime journal {label} exceeds {MAX_JOURNAL_PAYLOAD_BYTES} bytes"
        ));
    }
    Ok(())
}

fn validate_orchestra_blob(label: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > MAX_ORCHESTRA_ENVELOPE_BYTES {
        return Err(format!(
            "Orchestra {label} must contain between 1 and {MAX_ORCHESTRA_ENVELOPE_BYTES} bytes"
        ));
    }
    Ok(())
}

fn history_next_offset(has_more: bool, offset: i64, limit: u16) -> Option<u32> {
    has_more.then(|| {
        u32::try_from(offset)
            .unwrap_or_default()
            .saturating_add(u32::from(limit))
    })
}

fn validate_snapshot_blob(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_SNAPSHOT_PAYLOAD_BYTES {
        return Err(format!(
            "runtime snapshot exceeds {MAX_SNAPSHOT_PAYLOAD_BYTES} bytes"
        ));
    }
    Ok(())
}

pub(crate) fn snapshot_checksum(
    schema_version: u32,
    through_sequence: i64,
    bytes: &[u8],
) -> String {
    let hash = schema_version
        .to_le_bytes()
        .into_iter()
        .chain(through_sequence.to_le_bytes())
        .chain(bytes.iter().copied())
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    format!("fnv1a64:{hash:016x}")
}

fn reject_unsafe_path(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err("runtime journal path must not be a symbolic link".into())
        }
        Ok(metadata) if !metadata.is_file() => {
            Err("runtime journal path must be a regular file".into())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn create_private_file_if_missing(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn canonical_open_path(path: &Path) -> Result<std::path::PathBuf, String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| "runtime journal path must include a file name".to_string())?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent = match parent {
        Some(parent) => parent.canonicalize().map_err(|error| error.to_string())?,
        None => std::env::current_dir().map_err(|error| error.to_string())?,
    };
    Ok(parent.join(file_name))
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

use rusqlite::OptionalExtension;
