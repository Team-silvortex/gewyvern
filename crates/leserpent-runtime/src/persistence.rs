use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{self, OpenOptions};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::types::Value;
use rusqlite::{
    Connection, MappedRows, OpenFlags, Row, Transaction, TransactionBehavior, params,
    params_from_iter,
};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use leserpent_domain::{RuntimeId, RuntimeLogLevel, RuntimeLogRecord};

use crate::{
    EFFECT_QUEUE_CAPACITY, EffectEnqueue, EffectQueueStats, MAX_EFFECT_ENQUEUE_BATCH,
    ORCHESTRA_DELETE_REPLAY_HORIZON, OrchestraDeleteReplayHorizon,
    RUNTIME_UNREGISTRATION_REPLAY_HORIZON, RuntimeUnregisterTarget, RuntimeUnregistration,
    RuntimeUnregistrationReplayHorizon,
};

pub(super) const ORCHESTRA_DELETE_REPLAY_HORIZON_PINNED_ERROR: &str =
    "Orchestra delete replay horizon is pinned by reconciliation audit";

const RUNTIME_JOURNAL_SCHEMA_VERSION: i64 = 21;
pub const AUTHORITY_KIND_DAEMON_BOOTSTRAP: &str = "daemon_bootstrap";
pub const AUTHORITY_KIND_DAEMON_RETIREMENT: &str = "daemon_retirement";
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
const MAX_ORCHESTRA_EVENTS_PER_RUN: usize = 3;
const MAX_ORCHESTRA_RUNS_PER_RUNTIME: usize = 32;
const MAX_RUNTIME_TARGET_BINDINGS: i64 = 4_096;
const MAX_RUNTIME_TARGET_REGISTRATION_INTENTS: i64 = 128;
const MAX_RUNTIME_TARGET_REGISTRATION_PAYLOAD_BYTES: usize = 16 * 1024;
const MAX_RUNTIME_TARGET_SECRET_GC_BATCH: usize = 128;
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
    pub created_at_unix_ms: i64,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrchestraEffectStatusRecord {
    pub kind: String,
    pub payload: Vec<u8>,
    pub state: String,
    pub attempt: u32,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrchestraEffectCancellationRecord {
    pub cancelled_effect_count: u32,
    pub replayed: bool,
}

pub struct OrchestraPersistenceRecord {
    pub run: Vec<u8>,
    pub event: Vec<u8>,
    pub event_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrchestraImportRecord {
    pub run_id: String,
    pub runtime_id: String,
    pub request_id: Option<String>,
    pub event_type: String,
    pub outcome: String,
    pub recorded_at: String,
    pub run: Vec<u8>,
    pub event: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlPlaneImportRecord {
    pub through_sequence: i64,
    pub saved_at_unix_ms: i64,
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

#[derive(Debug, Eq, PartialEq)]
pub struct OrchestraDeleteOperationRecord {
    pub generation: u64,
    pub request: Vec<u8>,
    pub deleted_runtime_count: u32,
    pub deleted_run_count: u64,
    pub deleted_event_count: u64,
    pub committed_at_unix_ms: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredOrchestraRunEnvelope {
    run_id: String,
    runtime_id: String,
    plan_id: String,
    outcome: String,
    executed_at: String,
    completed_at: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredOrchestraEventEnvelope {
    #[serde(default)]
    event_id: u64,
    run_id: String,
    runtime_id: String,
    event_type: String,
    from_outcome: Option<String>,
    to_outcome: String,
    summary: String,
    recorded_at: String,
}

struct ValidatedOrchestraRun {
    outcome: String,
    executed_at: OffsetDateTime,
    completed_at: Option<OffsetDateTime>,
    request_id: Option<String>,
}

struct ValidatedOrchestraEvent {
    event_id: u64,
    envelope_event_id: u64,
    run_id: String,
    runtime_id: String,
    event_type: String,
    envelope: Vec<u8>,
    from_outcome: Option<String>,
    to_outcome: String,
    recorded_at_text: String,
    recorded_at: OffsetDateTime,
    generation: i64,
}

struct OrchestraRetentionPlan {
    retained_run_ids: Vec<String>,
    evicted_run_ids: Vec<String>,
}

struct ValidatedRuntimeUnregistrationOperation {
    runtime_ids: Vec<String>,
    journal_sequences: Vec<i64>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RuntimeUnregistrationOperationRecord {
    pub generation: u64,
    pub request: Vec<u8>,
    pub deleted_runtime_count: u32,
    pub deleted_run_count: u64,
    pub deleted_event_count: u64,
    pub removed_at_unix_ms: i64,
}

pub struct RuntimeUnregistrationReceiptLookupRecord {
    pub operation: Option<RuntimeUnregistrationOperationRecord>,
    pub replay_horizon: RuntimeUnregistrationReplayHorizon,
}

pub struct AuthorityCheckpointRecord {
    pub revision: u64,
    pub phase: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityWriterClaimRecord {
    pub generation: u64,
    pub writer_id: String,
    pub replayed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTargetRegistrationAdmission {
    Prepared,
    PendingReplay,
    CommittedReplay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTargetRegistrationRecord {
    pub operation_id: String,
    pub runtime_id: String,
    pub secret_key: String,
    pub payload: Vec<u8>,
    pub recorded_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTargetBindingRecord {
    pub operation_id: String,
    pub runtime_id: String,
    pub secret_key: String,
    pub payload: Vec<u8>,
    pub updated_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTargetBindingCommit {
    pub binding: RuntimeTargetBindingRecord,
    pub replayed: bool,
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

    pub fn claim_authority_writer(
        &mut self,
        writer_id: &str,
    ) -> Result<AuthorityWriterClaimRecord, String> {
        validate_authority_writer_id(writer_id)?;
        self.ensure_owner()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let current: Option<(i64, String)> = transaction
            .query_row(
                "SELECT generation, writer_id
                 FROM authority_writer_fence WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some((generation, current_writer_id)) = current.as_ref()
            && current_writer_id == writer_id
        {
            let generation = u64::try_from(*generation)
                .map_err(|_| "authority writer generation is invalid".to_string())?;
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(AuthorityWriterClaimRecord {
                generation,
                writer_id: writer_id.to_string(),
                replayed: true,
            });
        }
        let generation = match current {
            Some((generation, _)) => generation
                .checked_add(1)
                .ok_or_else(|| "authority writer generation is exhausted".to_string())?,
            None => 1,
        };
        transaction
            .execute(
                "INSERT INTO authority_writer_fence (id, generation, writer_id)
                 VALUES (1, ?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET
                     generation = excluded.generation,
                     writer_id = excluded.writer_id",
                params![generation, writer_id],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(AuthorityWriterClaimRecord {
            generation: u64::try_from(generation)
                .map_err(|_| "authority writer generation is invalid".to_string())?,
            writer_id: writer_id.to_string(),
            replayed: false,
        })
    }

    pub fn authority_writer_fence(&mut self) -> Result<Option<(u64, String)>, String> {
        self.ensure_owner()?;
        self.connection
            .query_row(
                "SELECT generation, writer_id
                 FROM authority_writer_fence WHERE id = 1",
                [],
                |row| {
                    let generation = row.get::<_, i64>(0)?;
                    let writer_id = row.get::<_, String>(1)?;
                    Ok((generation, writer_id))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
            .map(|(generation, writer_id)| {
                Ok((
                    u64::try_from(generation)
                        .map_err(|_| "authority writer generation is invalid".to_string())?,
                    writer_id,
                ))
            })
            .transpose()
    }

    pub fn begin_runtime_target_registration(
        &mut self,
        operation_id: &str,
        runtime_id: &str,
        secret_key: &str,
        payload: &[u8],
    ) -> Result<RuntimeTargetRegistrationAdmission, String> {
        validate_runtime_target_registration(operation_id, runtime_id, secret_key, payload)?;
        self.ensure_owner()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;

        if let Some(existing) = load_runtime_target_registration_by_operation(
            &transaction,
            "runtime_target_registration_intents",
            operation_id,
        )? {
            require_runtime_target_registration_match(
                &existing,
                operation_id,
                runtime_id,
                secret_key,
                payload,
            )?;
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(RuntimeTargetRegistrationAdmission::PendingReplay);
        }
        if let Some(existing) = load_runtime_target_registration_by_operation(
            &transaction,
            "runtime_target_bindings",
            operation_id,
        )? {
            require_runtime_target_registration_match(
                &existing,
                operation_id,
                runtime_id,
                secret_key,
                payload,
            )?;
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(RuntimeTargetRegistrationAdmission::CommittedReplay);
        }
        let secret_key_references: i64 = transaction
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM runtime_target_registration_intents
                      WHERE secret_key = ?1) +
                     (SELECT COUNT(*) FROM runtime_target_bindings
                      WHERE secret_key = ?1) +
                     (SELECT COUNT(*) FROM runtime_target_secret_gc
                      WHERE secret_key = ?1)",
                [secret_key],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if secret_key_references != 0 {
            return Err("runtime target secret key is already reserved".into());
        }
        let runtime_pending: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM runtime_target_registration_intents
                 WHERE runtime_id = ?1",
                [runtime_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if runtime_pending != 0 {
            return Err("runtime target registration is already pending".into());
        }
        let pending_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM runtime_target_registration_intents",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if pending_count >= MAX_RUNTIME_TARGET_REGISTRATION_INTENTS {
            return Err("runtime target registration intent capacity reached".into());
        }
        let now = unix_time_ms()?;
        transaction
            .execute(
                "INSERT INTO runtime_target_registration_intents
                     (operation_id, runtime_id, secret_key, payload, created_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![operation_id, runtime_id, secret_key, payload, now],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(RuntimeTargetRegistrationAdmission::Prepared)
    }

    pub fn pending_runtime_target_registrations(
        &mut self,
    ) -> Result<Vec<RuntimeTargetRegistrationRecord>, String> {
        self.ensure_owner()?;
        load_runtime_target_registrations(
            &self.connection,
            "runtime_target_registration_intents",
            "created_at_unix_ms",
        )
    }

    pub fn runtime_target_bindings(&mut self) -> Result<Vec<RuntimeTargetBindingRecord>, String> {
        self.ensure_owner()?;
        load_runtime_target_registrations(
            &self.connection,
            "runtime_target_bindings",
            "updated_at_unix_ms",
        )
        .map(|records| {
            records
                .into_iter()
                .map(|record| RuntimeTargetBindingRecord {
                    operation_id: record.operation_id,
                    runtime_id: record.runtime_id,
                    secret_key: record.secret_key,
                    payload: record.payload,
                    updated_at_unix_ms: record.recorded_at_unix_ms,
                })
                .collect()
        })
    }

    pub fn commit_runtime_target_registration(
        &mut self,
        operation_id: &str,
    ) -> Result<RuntimeTargetBindingCommit, String> {
        validate_scheduler_id("runtime target registration operation ID", operation_id)?;
        self.ensure_owner()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let Some(intent) = load_runtime_target_registration_by_operation(
            &transaction,
            "runtime_target_registration_intents",
            operation_id,
        )?
        else {
            let binding = load_runtime_target_registration_by_operation(
                &transaction,
                "runtime_target_bindings",
                operation_id,
            )?
            .ok_or_else(|| "runtime target registration intent is missing".to_string())?;
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(RuntimeTargetBindingCommit {
                binding: RuntimeTargetBindingRecord {
                    operation_id: binding.operation_id,
                    runtime_id: binding.runtime_id,
                    secret_key: binding.secret_key,
                    payload: binding.payload,
                    updated_at_unix_ms: binding.recorded_at_unix_ms,
                },
                replayed: true,
            });
        };
        let existing_binding = load_runtime_target_registration_by_runtime(
            &transaction,
            "runtime_target_bindings",
            &intent.runtime_id,
        )?;
        if existing_binding.is_none() {
            let binding_count: i64 = transaction
                .query_row("SELECT COUNT(*) FROM runtime_target_bindings", [], |row| {
                    row.get(0)
                })
                .map_err(|error| error.to_string())?;
            if binding_count >= MAX_RUNTIME_TARGET_BINDINGS {
                return Err("runtime target binding capacity reached".into());
            }
        }
        let now = unix_time_ms()?;
        transaction
            .execute(
                "INSERT INTO runtime_target_bindings
                     (runtime_id, operation_id, secret_key, payload, updated_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(runtime_id) DO UPDATE SET
                     operation_id = excluded.operation_id,
                     secret_key = excluded.secret_key,
                     payload = excluded.payload,
                     updated_at_unix_ms = excluded.updated_at_unix_ms",
                params![
                    intent.runtime_id,
                    intent.operation_id,
                    intent.secret_key,
                    intent.payload,
                    now
                ],
            )
            .map_err(|error| error.to_string())?;
        let deleted = transaction
            .execute(
                "DELETE FROM runtime_target_registration_intents WHERE operation_id = ?1",
                [operation_id],
            )
            .map_err(|error| error.to_string())?;
        if deleted != 1 {
            return Err("runtime target registration intent changed during commit".into());
        }
        if let Some(previous) = existing_binding
            && previous.secret_key != intent.secret_key
        {
            queue_runtime_target_secret_gc(&transaction, &previous.secret_key, now)?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(RuntimeTargetBindingCommit {
            binding: RuntimeTargetBindingRecord {
                operation_id: intent.operation_id,
                runtime_id: intent.runtime_id,
                secret_key: intent.secret_key,
                payload: intent.payload,
                updated_at_unix_ms: now,
            },
            replayed: false,
        })
    }

    pub fn abort_runtime_target_registration(
        &mut self,
        operation_id: &str,
    ) -> Result<bool, String> {
        validate_scheduler_id("runtime target registration operation ID", operation_id)?;
        self.ensure_owner()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let Some(intent) = load_runtime_target_registration_by_operation(
            &transaction,
            "runtime_target_registration_intents",
            operation_id,
        )?
        else {
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(false);
        };
        let deleted = transaction
            .execute(
                "DELETE FROM runtime_target_registration_intents WHERE operation_id = ?1",
                [operation_id],
            )
            .map_err(|error| error.to_string())?;
        if deleted != 1 {
            return Err("runtime target registration intent changed during abort".into());
        }
        queue_runtime_target_secret_gc(&transaction, &intent.secret_key, unix_time_ms()?)?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub fn retire_runtime_target_binding(&mut self, runtime_id: &str) -> Result<bool, String> {
        validate_scheduler_id("runtime target binding runtime ID", runtime_id)?;
        self.ensure_owner()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let Some(binding) = load_runtime_target_registration_by_runtime(
            &transaction,
            "runtime_target_bindings",
            runtime_id,
        )?
        else {
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(false);
        };
        let deleted = transaction
            .execute(
                "DELETE FROM runtime_target_bindings WHERE runtime_id = ?1",
                [runtime_id],
            )
            .map_err(|error| error.to_string())?;
        if deleted != 1 {
            return Err("runtime target binding changed during retirement".into());
        }
        queue_runtime_target_secret_gc(&transaction, &binding.secret_key, unix_time_ms()?)?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub fn runtime_target_secret_gc_batch(&mut self) -> Result<Vec<String>, String> {
        self.ensure_owner()?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT secret_key FROM runtime_target_secret_gc
                 ORDER BY queued_at_unix_ms, secret_key LIMIT ?1",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([MAX_RUNTIME_TARGET_SECRET_GC_BATCH as i64], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| error.to_string())?;
        let keys = rows
            .map(|row| row.map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        for key in &keys {
            validate_scheduler_id("runtime target secret key", key)?;
        }
        Ok(keys)
    }

    pub fn acknowledge_runtime_target_secret_gc(
        &mut self,
        secret_key: &str,
    ) -> Result<bool, String> {
        validate_scheduler_id("runtime target secret key", secret_key)?;
        self.ensure_owner()?;
        self.connection
            .execute(
                "DELETE FROM runtime_target_secret_gc WHERE secret_key = ?1",
                [secret_key],
            )
            .map(|changed| changed == 1)
            .map_err(|error| error.to_string())
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
                "SELECT generation, domain_schema, through_sequence, payload, checksum,
                        created_at_unix_ms
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
                    row.get::<_, i64>(5)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut snapshots = Vec::new();
        let mut found = 0;
        for row in rows {
            found += 1;
            let (
                generation,
                schema_version,
                through_sequence,
                payload,
                checksum,
                created_at_unix_ms,
            ) = row.map_err(|error| error.to_string())?;
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
                created_at_unix_ms,
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
        evict_runtime_unregistration_replay_horizon(&transaction, 0)?;
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
            let protected_sequences =
                retained_runtime_unregistration_journal_sequences(&transaction)?;
            compact_runtime_journal(&transaction, boundary, &protected_sequences)?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(through_sequence)
    }

    pub fn replace_control_plane_state(
        &mut self,
        domain_schema: u32,
        payload: &[u8],
        orchestra_runs: &[OrchestraImportRecord],
        protected_binding_runtime_ids: &[String],
    ) -> Result<ControlPlaneImportRecord, String> {
        self.ensure_owner()?;
        validate_snapshot_blob(payload)?;
        validate_control_plane_import(orchestra_runs, protected_binding_runtime_ids)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        evict_runtime_unregistration_replay_horizon(&transaction, 0)?;

        let pending_journal: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM runtime_journal
                 WHERE outcome IS NULL AND terminal_error IS NULL AND kind = 'command_plan'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if pending_journal != 0 {
            return Err("control-plane import requires a fully terminal journal".into());
        }
        let pending_registrations: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM runtime_target_registration_intents",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if pending_registrations != 0 {
            return Err("control-plane import is blocked by registration recovery".into());
        }
        let active_effects: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM runtime_effect_tasks WHERE state IN ('ready', 'leased')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if active_effects != 0 {
            return Err("control-plane import is blocked by active side effects".into());
        }
        let incomplete_authority: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM authority_checkpoints
                 WHERE NOT (
                     (operation_kind = 'daemon_bootstrap' AND phase IN ('session_bound', 'failed')) OR
                     (operation_kind = 'daemon_retirement' AND phase IN ('service_retired', 'failed')) OR
                     (operation_kind = 'gewyvern_provisioning' AND phase IN ('runtime_registered', 'failed')) OR
                     (operation_kind = 'gewyvern_retirement' AND phase IN ('runtime_unregistered', 'failed'))
                 )",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if incomplete_authority != 0 {
            return Err(
                "control-plane import is blocked by incomplete authority operations".into(),
            );
        }

        let persisted_binding_runtime_ids = {
            let mut statement = transaction
                .prepare("SELECT runtime_id FROM runtime_target_bindings ORDER BY runtime_id")
                .map_err(|error| error.to_string())?;
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?
        };
        if persisted_binding_runtime_ids != protected_binding_runtime_ids {
            return Err("control-plane import would alter credential-bound runtime targets".into());
        }

        let through_sequence: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM runtime_journal",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let previous_generation: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(generation), 0) FROM runtime_snapshots",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let first_generation = previous_generation
            .checked_add(1)
            .ok_or_else(|| "runtime snapshot generation is exhausted".to_string())?;
        let second_generation = first_generation
            .checked_add(1)
            .ok_or_else(|| "runtime snapshot generation is exhausted".to_string())?;
        let saved_at_unix_ms = unix_time_ms()?;

        let previous_orchestra_generation: Option<i64> = transaction
            .query_row(
                "SELECT MAX(updated_at_unix_ms) FROM orchestra_runs",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let orchestra_generation = match previous_orchestra_generation {
            Some(generation) => generation
                .checked_add(1)
                .ok_or_else(|| "Orchestra import generation is exhausted".to_string())?
                .max(saved_at_unix_ms),
            None => saved_at_unix_ms,
        };
        transaction
            .execute("DELETE FROM orchestra_runs", [])
            .map_err(|error| error.to_string())?;
        transaction
            .execute("DELETE FROM runtime_logs", [])
            .map_err(|error| error.to_string())?;
        for (index, imported) in orchestra_runs.iter().enumerate() {
            let generation = orchestra_generation
                .checked_add(
                    i64::try_from(index)
                        .map_err(|_| "Orchestra import generation is out of range".to_string())?,
                )
                .ok_or_else(|| "Orchestra import generation is exhausted".to_string())?;
            transaction
                .execute(
                    "INSERT INTO orchestra_runs
                         (run_id, runtime_id, request_id, envelope, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        imported.run_id,
                        imported.runtime_id,
                        imported.request_id,
                        imported.run,
                        generation
                    ],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO orchestra_events
                         (run_id, runtime_id, event_type, to_outcome, recorded_at,
                          envelope, created_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        imported.run_id,
                        imported.runtime_id,
                        imported.event_type,
                        imported.outcome,
                        imported.recorded_at,
                        imported.event,
                        generation
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
        let (stored_runs, stored_events): (i64, i64) = transaction
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs),
                     (SELECT COUNT(*) FROM orchestra_events)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| error.to_string())?;
        let imported_count = i64::try_from(orchestra_runs.len())
            .map_err(|_| "Orchestra import count is out of range".to_string())?;
        if stored_runs != imported_count || stored_events != imported_count {
            return Err("control-plane import Orchestra replacement is inconsistent".into());
        }

        transaction
            .execute("DELETE FROM runtime_snapshots", [])
            .map_err(|error| error.to_string())?;
        for generation in [first_generation, second_generation] {
            transaction
                .execute(
                    "INSERT INTO runtime_snapshots
                         (generation, domain_schema, through_sequence, payload, checksum,
                          created_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        generation,
                        i64::from(domain_schema),
                        through_sequence,
                        payload,
                        snapshot_checksum(domain_schema, through_sequence, payload),
                        saved_at_unix_ms
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
        let protected_sequences = retained_runtime_unregistration_journal_sequences(&transaction)?;
        compact_runtime_journal(&transaction, through_sequence, &protected_sequences)?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(ControlPlaneImportRecord {
            through_sequence,
            saved_at_unix_ms,
        })
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

    pub fn orchestra_effect_status(
        &mut self,
        effect_id: &str,
    ) -> Result<Option<OrchestraEffectStatusRecord>, String> {
        self.effect_record(effect_id).map(|record| {
            record.map(|record| OrchestraEffectStatusRecord {
                kind: record.kind,
                payload: record.payload,
                state: record.state,
                attempt: record.attempt,
                last_error: record.last_error,
            })
        })
    }

    pub fn cancel_ready_orchestra_effects(
        &mut self,
        run_id: &str,
        command_id: &str,
        effect_ids: &[String],
    ) -> Result<OrchestraEffectCancellationRecord, String> {
        self.ensure_owner()?;
        validate_scheduler_id("Orchestra run_id", run_id)?;
        validate_scheduler_id("Orchestra cancellation command_id", command_id)?;
        if effect_ids.is_empty() || effect_ids.len() > 4 {
            return Err("Orchestra cancellation effect set is out of bounds".into());
        }
        let mut unique = HashSet::with_capacity(effect_ids.len());
        for effect_id in effect_ids {
            validate_scheduler_id("Orchestra effect_id", effect_id)?;
            if !unique.insert(effect_id.as_str()) {
                return Err("Orchestra cancellation effect set contains a duplicate".into());
            }
        }
        let marker_id = orchestra_cancel_marker_id(run_id)?;
        let marker_payload = command_id.as_bytes();
        validate_blob("Orchestra cancellation marker", marker_payload)?;
        let now = unix_time_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let existing_marker: Option<(String, Vec<u8>, String)> = transaction
            .query_row(
                "SELECT kind, payload, state FROM runtime_effect_tasks WHERE effect_id = ?1",
                [&marker_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        match existing_marker {
            Some((kind, payload, state))
                if kind == "leserpent.orchestra.cancel"
                    && payload == marker_payload
                    && state == "failed" =>
            {
                transaction.commit().map_err(|error| error.to_string())?;
                return Ok(OrchestraEffectCancellationRecord {
                    cancelled_effect_count: 0,
                    replayed: true,
                });
            }
            Some(_) => {
                return Err("Orchestra cancellation command identity was reused".into());
            }
            None => {}
        }

        let cancellation_error = format!("orchestra_cancelled:{command_id}");
        let mut cancelled = 0_u32;
        for effect_id in effect_ids {
            let changed = transaction
                .execute(
                    "UPDATE runtime_effect_tasks SET state = 'failed', last_error = ?1,
                         lease_token = NULL, lease_expires_at_unix_ms = NULL,
                         updated_at_unix_ms = ?2
                     WHERE effect_id = ?3 AND state = 'ready'",
                    params![cancellation_error, now, effect_id],
                )
                .map_err(|error| error.to_string())?;
            cancelled = cancelled
                .checked_add(
                    u32::try_from(changed)
                        .map_err(|_| "Orchestra cancellation count overflow".to_string())?,
                )
                .ok_or_else(|| "Orchestra cancellation count overflow".to_string())?;
        }
        if cancelled == 0 {
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(OrchestraEffectCancellationRecord {
                cancelled_effect_count: 0,
                replayed: false,
            });
        }
        transaction
            .execute(
                "INSERT INTO runtime_effect_tasks
                     (effect_id, kind, payload, state, attempt, max_attempts,
                      available_at_unix_ms, last_error, created_at_unix_ms,
                      updated_at_unix_ms)
                 VALUES (?1, 'leserpent.orchestra.cancel', ?2, 'failed', 0, 1,
                         ?3, ?4, ?3, ?3)",
                params![marker_id, marker_payload, now, cancellation_error],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(OrchestraEffectCancellationRecord {
            cancelled_effect_count: cancelled,
            replayed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn persist_orchestra_run_event(
        &mut self,
        run_id: &str,
        runtime_id: &str,
        request_id: Option<&str>,
        event_type: &str,
        from_outcome: Option<&str>,
        to_outcome: &str,
        run_outcome: &str,
        recorded_at: &str,
        run: &[u8],
        event: &[u8],
    ) -> Result<OrchestraPersistenceRecord, String> {
        self.persist_orchestra_run_event_inner(
            run_id,
            runtime_id,
            request_id,
            event_type,
            from_outcome,
            to_outcome,
            run_outcome,
            recorded_at,
            run,
            event,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn persist_orchestra_run_event_start(
        &mut self,
        run_id: &str,
        runtime_id: &str,
        request_id: &str,
        event_type: &str,
        to_outcome: &str,
        recorded_at: &str,
        run: &[u8],
        event: &[u8],
    ) -> Result<OrchestraPersistenceRecord, String> {
        self.persist_orchestra_run_event_inner(
            run_id,
            runtime_id,
            Some(request_id),
            event_type,
            None,
            to_outcome,
            to_outcome,
            recorded_at,
            run,
            event,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn persist_orchestra_run_event_inner(
        &mut self,
        run_id: &str,
        runtime_id: &str,
        request_id: Option<&str>,
        event_type: &str,
        from_outcome: Option<&str>,
        to_outcome: &str,
        run_outcome: &str,
        recorded_at: &str,
        run: &[u8],
        event: &[u8],
        require_idle_runtime: bool,
    ) -> Result<OrchestraPersistenceRecord, String> {
        self.ensure_owner()?;
        for (field, value) in [
            ("run_id", run_id),
            ("runtime_id", runtime_id),
            ("event_type", event_type),
        ] {
            validate_scheduler_id(field, value)?;
        }
        validate_orchestra_outcome("to_outcome", to_outcome)?;
        validate_orchestra_outcome("run_outcome", run_outcome)?;
        if let Some(from_outcome) = from_outcome {
            validate_orchestra_outcome("from_outcome", from_outcome)?;
        }
        if run_outcome != to_outcome {
            return Err("Orchestra run outcome does not match its event".into());
        }
        if let Some(request_id) = request_id {
            validate_scheduler_id("request_id", request_id)?;
        }
        let recorded_at_instant = validate_orchestra_recorded_at(recorded_at)?;
        validate_orchestra_blob("run envelope", run)?;
        validate_orchestra_blob("event envelope", event)?;
        let wall_clock_now = unix_time_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        validate_orchestra_append_envelopes(
            run_id,
            runtime_id,
            request_id,
            event_type,
            from_outcome,
            to_outcome,
            run_outcome,
            recorded_at,
            run,
            event,
        )?;
        let existing_run: Option<(String, Option<String>, Vec<u8>)> = transaction
            .query_row(
                "SELECT runtime_id, request_id, envelope
                 FROM orchestra_runs WHERE run_id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let mut event_batches = load_validated_orchestra_event_batches(&transaction, &[run_id])?;
        let retained_events = event_batches
            .remove(run_id)
            .ok_or_else(|| "Orchestra retained event history is inconsistent".to_string())?;
        if !event_batches.is_empty() {
            return Err("Orchestra retained event history is inconsistent".into());
        }
        match &existing_run {
            Some((existing_runtime, stored_request_id, stored_envelope)) => {
                if existing_runtime != runtime_id {
                    return Err("Orchestra run identity was reused for another runtime".into());
                }
                let retained_run = validate_retained_orchestra_run_row(
                    run_id,
                    existing_runtime,
                    stored_request_id.as_deref(),
                    stored_envelope,
                )?;
                validate_orchestra_event_history(
                    &retained_run,
                    &retained_events,
                    existing_runtime,
                    run_id,
                )?;
            }
            None if !retained_events.is_empty() => {
                return Err("Orchestra events exist without their retained run".into());
            }
            None => {}
        }
        if require_idle_runtime && existing_run.is_none() {
            let mut statement = transaction
                .prepare(
                    "SELECT run_id, request_id, envelope FROM orchestra_runs
                     WHERE runtime_id = ?1 AND run_id <> ?2",
                )
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map(params![runtime_id, run_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            for (retained_run_id, retained_request_id, retained_envelope) in rows {
                let retained = validate_retained_orchestra_run_row(
                    &retained_run_id,
                    runtime_id,
                    retained_request_id.as_deref(),
                    &retained_envelope,
                )?;
                if is_active_orchestra_outcome(&retained.outcome) {
                    return Err("Orchestra runtime already has an active run".into());
                }
            }
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
        let retention_plan = match existing_event {
            Some(existing) if existing != event => {
                return Err("Orchestra event identity was reused with different content".into());
            }
            Some(_)
                if existing_run
                    .as_ref()
                    .is_some_and(|(_, _, stored)| stored != run) =>
            {
                return Err("Orchestra event replay changed its run content".into());
            }
            Some(_) => {
                let plan = plan_orchestra_retention(&transaction, runtime_id, run_id)?;
                if !plan.evicted_run_ids.is_empty() {
                    return Err("Orchestra retained run set exceeds its bound".into());
                }
                plan
            }
            None => {
                match retained_events.last() {
                    None if from_outcome.is_some() => {
                        return Err("Orchestra origin event has a source outcome".into());
                    }
                    Some(_) if from_outcome.is_none() => {
                        return Err("Orchestra appended event is missing its source outcome".into());
                    }
                    Some(previous) => {
                        if from_outcome != Some(previous.to_outcome.as_str()) {
                            return Err(
                                "Orchestra appended event does not follow the previous outcome"
                                    .into(),
                            );
                        }
                        if recorded_at_instant < previous.recorded_at {
                            return Err(
                                "Orchestra appended event time precedes its predecessor".into()
                            );
                        }
                        if !is_valid_orchestra_transition(&previous.to_outcome, to_outcome) {
                            return Err("Orchestra appended event transition is invalid".into());
                        }
                    }
                    None => {}
                }
                let generation =
                    next_orchestra_generation(&transaction, runtime_id, wall_clock_now)?;
                transaction
                    .execute(
                        "INSERT INTO orchestra_runs (
                             run_id, runtime_id, request_id, envelope, updated_at_unix_ms)
                         VALUES (?1, ?2, ?3, ?4, ?5)
                         ON CONFLICT(run_id) DO UPDATE SET
                             request_id = excluded.request_id,
                             envelope = excluded.envelope,
                             updated_at_unix_ms = excluded.updated_at_unix_ms",
                        params![run_id, runtime_id, request_id, run, generation],
                    )
                    .map_err(|error| error.to_string())?;
                let plan = plan_orchestra_retention(&transaction, runtime_id, run_id)?;
                for evicted_run_id in &plan.evicted_run_ids {
                    transaction
                        .execute(
                            "DELETE FROM orchestra_runs
                             WHERE runtime_id = ?1 AND run_id = ?2",
                            params![runtime_id, evicted_run_id],
                        )
                        .map_err(|error| error.to_string())?;
                }
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
                            generation
                        ],
                    )
                    .map_err(|error| error.to_string())?;
                plan
            }
        };
        let receipt = load_validated_orchestra_persistence_record(
            &transaction,
            run_id,
            runtime_id,
            request_id,
            event_type,
            to_outcome,
            recorded_at,
            &retention_plan.retained_run_ids,
            &retention_plan.evicted_run_ids,
        )?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(receipt)
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
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|error| error.to_string())?;
        if let Some(run_id) = run_id {
            let stored_run: Option<(String, String, Option<String>, Vec<u8>)> = transaction
                .query_row(
                    "SELECT run_id, runtime_id, request_id, envelope
                     FROM orchestra_runs WHERE run_id = ?1",
                    [run_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            let Some((stored_run_id, stored_runtime_id, stored_request_id, run_envelope)) =
                stored_run
            else {
                transaction.commit().map_err(|error| error.to_string())?;
                return Ok(OrchestraHistoryRecord {
                    runs: Vec::new(),
                    events: Vec::new(),
                    next_offset: None,
                });
            };
            if stored_runtime_id != runtime_id.unwrap_or_default() {
                return Err("Orchestra history runtime identity is inconsistent".into());
            }
            let validated_run = validate_retained_orchestra_run_row(
                &stored_run_id,
                &stored_runtime_id,
                stored_request_id.as_deref(),
                &run_envelope,
            )?;
            let mut event_batches =
                load_validated_orchestra_event_batches(&transaction, &[run_id])?;
            let rows = event_batches
                .remove(run_id)
                .ok_or_else(|| "Orchestra event history identity is inconsistent".to_string())?;
            validate_orchestra_event_history(
                &validated_run,
                &rows,
                runtime_id.unwrap_or_default(),
                run_id,
            )?;
            let offset = usize::try_from(offset)
                .map_err(|_| "Orchestra history offset is invalid".to_string())?;
            let limit = usize::from(limit);
            let page_end = offset.saturating_add(limit).min(rows.len());
            let events = if offset >= rows.len() {
                Vec::new()
            } else {
                rows[offset..page_end]
                    .iter()
                    .map(|event| (event.event_id, event.envelope.clone()))
                    .collect()
            };
            let has_more = page_end < rows.len();
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(OrchestraHistoryRecord {
                runs: Vec::new(),
                events,
                next_offset: history_next_offset(
                    has_more,
                    i64::try_from(offset)
                        .map_err(|_| "Orchestra history offset is invalid".to_string())?,
                    u16::try_from(limit)
                        .map_err(|_| "Orchestra history limit is invalid".to_string())?,
                ),
            });
        }
        let rows = if let Some(runtime_id) = runtime_id {
            let mut statement = transaction
                .prepare(
                    "SELECT run_id, runtime_id, request_id, envelope FROM orchestra_runs
                     WHERE runtime_id = ?1
                     ORDER BY updated_at_unix_ms DESC, run_id ASC LIMIT ?2 OFFSET ?3",
                )
                .map_err(|error| error.to_string())?;
            statement
                .query_map(params![runtime_id, fetch, offset], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?
        } else {
            let mut statement = transaction
                .prepare(
                    "SELECT run_id, runtime_id, request_id, envelope FROM orchestra_runs
                     ORDER BY updated_at_unix_ms DESC, run_id ASC LIMIT ?1 OFFSET ?2",
                )
                .map_err(|error| error.to_string())?;
            statement
                .query_map(params![fetch, offset], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?
        };
        let mut validated_rows = rows
            .into_iter()
            .map(|(run_id, runtime_id, request_id, envelope)| {
                let run = validate_retained_orchestra_run_row(
                    &run_id,
                    &runtime_id,
                    request_id.as_deref(),
                    &envelope,
                )?;
                Ok((run_id, runtime_id, envelope, run))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let run_ids = validated_rows
            .iter()
            .map(|(run_id, _, _, _)| run_id.as_str())
            .collect::<Vec<_>>();
        let mut event_batches = load_validated_orchestra_event_batches(&transaction, &run_ids)?;
        for (run_id, runtime_id, _, run) in &validated_rows {
            let events = event_batches
                .remove(run_id)
                .ok_or_else(|| "Orchestra event history identity is inconsistent".to_string())?;
            validate_orchestra_event_history(run, &events, runtime_id, run_id)?;
        }
        if !event_batches.is_empty() {
            return Err("Orchestra event history identity is inconsistent".into());
        }
        let has_more = validated_rows.len() > usize::from(limit);
        validated_rows.truncate(usize::from(limit));
        let runs = validated_rows
            .into_iter()
            .map(|(_, _, envelope, _)| envelope)
            .collect();
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(OrchestraHistoryRecord {
            runs,
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
        let deleted = delete_orchestra_runtimes_in_transaction(&transaction, runtime_ids)?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(deleted)
    }

    pub fn delete_orchestra_runtimes_idempotent(
        &mut self,
        operation_id: &str,
        runtime_ids: &[String],
    ) -> Result<(OrchestraDeleteOperationRecord, bool), String> {
        self.ensure_owner()?;
        validate_scheduler_id("Orchestra delete operation_id", operation_id)?;
        let canonical_runtime_ids = canonical_orchestra_delete_runtime_ids(runtime_ids)?;
        let request = serde_json::to_vec(&canonical_runtime_ids)
            .map_err(|error| format!("failed to encode Orchestra delete request: {error}"))?;
        validate_blob("Orchestra delete request", &request)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        if let Some(record) = load_orchestra_delete_operation_record(&transaction, operation_id)? {
            if record.request != request {
                return Err("Orchestra delete operation idempotency conflict".into());
            }
            validate_orchestra_delete_replay_snapshot(&transaction, &record)?;
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok((record, true));
        }
        evict_orchestra_delete_replay_horizon(&transaction, 1)?;
        let generation = allocate_orchestra_delete_generation(&transaction)?;
        let committed_at_unix_ms = next_orchestra_delete_timestamp(&transaction)?;
        let deleted =
            delete_orchestra_runtimes_in_transaction(&transaction, &canonical_runtime_ids)?;
        let expected = OrchestraDeleteOperationRecord {
            generation,
            request,
            deleted_runtime_count: deleted.deleted_runtime_count,
            deleted_run_count: deleted.deleted_run_count,
            deleted_event_count: deleted.deleted_event_count,
            committed_at_unix_ms,
        };
        transaction
            .execute(
                "INSERT INTO orchestra_delete_operations
                     (operation_id, generation, request, deleted_runtime_count,
                      deleted_run_count, deleted_event_count, committed_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    operation_id,
                    i64::try_from(generation)
                        .map_err(|_| "Orchestra delete generation is out of range")?,
                    expected.request,
                    i64::from(expected.deleted_runtime_count),
                    i64::try_from(expected.deleted_run_count)
                        .map_err(|_| "deleted Orchestra run count is out of range")?,
                    i64::try_from(expected.deleted_event_count)
                        .map_err(|_| "deleted Orchestra event count is out of range")?,
                    committed_at_unix_ms,
                ],
            )
            .map_err(|error| error.to_string())?;
        let record = load_orchestra_delete_operation_record(&transaction, operation_id)?
            .ok_or_else(|| {
                "Orchestra delete operation post-write receipt is missing".to_string()
            })?;
        if record != expected {
            return Err("Orchestra delete operation post-write receipt is inconsistent".into());
        }
        let protected = transaction
            .execute(
                "UPDATE orchestra_delete_replay_horizon
                 SET protected_from_generation =
                     COALESCE(protected_from_generation, ?1)
                 WHERE id = 1",
                [i64::try_from(generation)
                    .map_err(|_| "Orchestra delete generation is out of range")?],
            )
            .map_err(|error| error.to_string())?;
        if protected != 1 {
            return Err("Orchestra delete replay protection is inconsistent".into());
        }
        validate_orchestra_delete_replay_snapshot(&transaction, &record)?;
        load_orchestra_delete_replay_horizon(&transaction)?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok((record, false))
    }

    pub fn orchestra_delete_replay_horizon(
        &mut self,
    ) -> Result<OrchestraDeleteReplayHorizon, String> {
        self.ensure_owner()?;
        load_orchestra_delete_replay_horizon(&self.connection)
    }

    pub fn checkpoint_orchestra_delete_replay_horizon(
        &mut self,
        minimum_retained_generation: u64,
        observed_through_generation: u64,
    ) -> Result<OrchestraDeleteReplayHorizon, String> {
        self.ensure_owner()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let horizon = load_orchestra_delete_replay_horizon(&transaction)?;
        if minimum_retained_generation == 0
            || observed_through_generation < minimum_retained_generation
            || horizon
                .newest_generation
                .is_none_or(|newest| observed_through_generation > newest)
            || minimum_retained_generation <= horizon.evicted_through_generation
            || horizon
                .protected_from_generation
                .is_some_and(|protected| minimum_retained_generation < protected)
        {
            return Err(
                "Orchestra delete replay checkpoint is outside the retained horizon".into(),
            );
        }
        let expected = observed_through_generation
            .checked_sub(minimum_retained_generation)
            .and_then(|span| span.checked_add(1))
            .ok_or_else(|| "Orchestra delete replay checkpoint is invalid".to_string())?;
        let retained: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM orchestra_delete_operations
                 WHERE generation BETWEEN ?1 AND ?2",
                params![
                    i64::try_from(minimum_retained_generation)
                        .map_err(|_| "Orchestra delete replay checkpoint is invalid")?,
                    i64::try_from(observed_through_generation)
                        .map_err(|_| "Orchestra delete replay checkpoint is invalid")?,
                ],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if u64::try_from(retained).ok() != Some(expected) {
            return Err("Orchestra delete replay checkpoint has a receipt gap".into());
        }
        let changed = transaction
            .execute(
                "UPDATE orchestra_delete_replay_horizon
                 SET protected_from_generation = ?1,
                     checkpointed_through_generation = ?2
                 WHERE id = 1
                   AND (protected_from_generation IS NULL
                        OR protected_from_generation <= ?1)",
                params![
                    i64::try_from(minimum_retained_generation)
                        .map_err(|_| "Orchestra delete replay checkpoint is invalid")?,
                    i64::try_from(observed_through_generation)
                        .map_err(|_| "Orchestra delete replay checkpoint is invalid")?,
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("Orchestra delete replay checkpoint conflicted".into());
        }
        compact_orchestra_delete_before_protected(&transaction)?;
        let checkpoint = load_orchestra_delete_replay_horizon(&transaction)?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(checkpoint)
    }

    pub fn runtime_unregistration_operation(
        &mut self,
        operation_id: &str,
    ) -> Result<Option<RuntimeUnregistrationOperationRecord>, String> {
        self.ensure_owner()?;
        validate_scheduler_id("runtime unregistration operation_id", operation_id)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        evict_runtime_unregistration_replay_horizon(&transaction, 0)?;
        let record = load_runtime_unregistration_operation_record(&transaction, operation_id)?;
        let Some(record) = record else {
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(None);
        };
        validate_runtime_unregistration_replay_snapshot(&transaction, &record)?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(Some(record))
    }

    pub fn runtime_unregistration_replay_horizon(
        &mut self,
    ) -> Result<RuntimeUnregistrationReplayHorizon, String> {
        self.ensure_owner()?;
        load_runtime_unregistration_replay_horizon(&self.connection)
    }

    pub fn runtime_unregistration_receipt_lookup(
        &mut self,
        operation_id: &str,
    ) -> Result<RuntimeUnregistrationReceiptLookupRecord, String> {
        self.ensure_owner()?;
        validate_scheduler_id("runtime unregistration operation_id", operation_id)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        evict_runtime_unregistration_replay_horizon(&transaction, 0)?;
        let operation = load_runtime_unregistration_operation_record(&transaction, operation_id)?;
        if let Some(record) = &operation {
            validate_runtime_unregistration_replay_snapshot(&transaction, record)?;
        }
        let replay_horizon = load_runtime_unregistration_replay_horizon(&transaction)?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(RuntimeUnregistrationReceiptLookupRecord {
            operation,
            replay_horizon,
        })
    }

    pub fn commit_runtime_unregistration_operation(
        &mut self,
        operation_id: &str,
        request: &[u8],
        runtime_ids: &[String],
        unregistrations: &[Vec<u8>],
    ) -> Result<RuntimeUnregistrationOperationRecord, String> {
        self.ensure_owner()?;
        validate_scheduler_id("runtime unregistration operation_id", operation_id)?;
        validate_blob("runtime unregistration request", request)?;
        if runtime_ids.is_empty()
            || runtime_ids.len() > 128
            || runtime_ids.len() != unregistrations.len()
        {
            return Err(
                "runtime unregistration must contain between 1 and 128 matching targets".into(),
            );
        }
        let mut unique = BTreeSet::new();
        for (runtime_id, payload) in runtime_ids.iter().zip(unregistrations) {
            validate_scheduler_id("runtime_id", runtime_id)?;
            validate_blob("runtime unregistration", payload)?;
            if !unique.insert(runtime_id.as_str()) {
                return Err("runtime unregistration contains a duplicate runtime ID".into());
            }
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        evict_runtime_unregistration_replay_horizon(&transaction, 1)?;
        let generation = allocate_runtime_unregistration_generation(&transaction)?;
        let removed_at_unix_ms = next_runtime_unregistration_timestamp(&transaction)?;
        let count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM runtime_journal", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        let target_count = i64::try_from(runtime_ids.len())
            .map_err(|_| "runtime unregistration target count is out of range".to_string())?;
        if count.saturating_add(target_count) > MAX_JOURNAL_RECORDS {
            return Err(format!(
                "runtime journal record limit {MAX_JOURNAL_RECORDS} reached"
            ));
        }
        for payload in unregistrations {
            transaction
                .execute(
                    "INSERT INTO runtime_journal
                         (kind, payload, created_at_unix_ms) VALUES (?1, ?2, ?3)",
                    params![
                        JournalEntryKind::RuntimeUnregistration.as_str(),
                        payload,
                        removed_at_unix_ms
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
        let deleted = delete_orchestra_runtimes_in_transaction(&transaction, runtime_ids)?;
        let expected_record = RuntimeUnregistrationOperationRecord {
            generation,
            request: request.to_vec(),
            deleted_runtime_count: deleted.deleted_runtime_count,
            deleted_run_count: deleted.deleted_run_count,
            deleted_event_count: deleted.deleted_event_count,
            removed_at_unix_ms,
        };
        transaction
            .execute(
                "INSERT INTO runtime_unregistration_operations
                     (operation_id, generation, request, deleted_runtime_count, deleted_run_count,
                      deleted_event_count, removed_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    operation_id,
                    i64::try_from(generation)
                        .map_err(|_| "runtime unregistration generation is out of range")?,
                    request,
                    i64::from(deleted.deleted_runtime_count),
                    i64::try_from(deleted.deleted_run_count)
                        .map_err(|_| "deleted run count is out of range".to_string())?,
                    i64::try_from(deleted.deleted_event_count)
                        .map_err(|_| "deleted event count is out of range".to_string())?,
                    removed_at_unix_ms
                ],
            )
            .map_err(|error| error.to_string())?;
        let record = load_runtime_unregistration_operation_record(&transaction, operation_id)?
            .ok_or_else(|| {
                "runtime unregistration operation post-write record is missing".to_string()
            })?;
        if record != expected_record {
            return Err(
                "runtime unregistration operation post-write record is inconsistent".into(),
            );
        }
        validate_runtime_unregistration_replay_snapshot(&transaction, &record)?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn enqueue_effect_batch(&mut self, effects: &[EffectEnqueue]) -> Result<u64, String> {
        self.ensure_owner()?;
        if effects.is_empty() || effects.len() > MAX_EFFECT_ENQUEUE_BATCH {
            return Err(format!(
                "effect enqueue batch must contain between 1 and {MAX_EFFECT_ENQUEUE_BATCH} tasks"
            ));
        }
        let mut batch_ids = HashSet::with_capacity(effects.len());
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
        {
            let mut existing_statement = transaction
                .prepare_cached(
                    "SELECT kind, payload, max_attempts FROM runtime_effect_tasks
                     WHERE effect_id = ?1",
                )
                .map_err(|error| error.to_string())?;
            for effect in effects {
                let existing: Option<(String, Vec<u8>, i64)> = existing_statement
                    .query_row([&effect.effect_id], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                    })
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
        }
        let new_count = i64::try_from(new_effects.len())
            .map_err(|_| "effect enqueue batch is too large".to_string())?;
        if active.saturating_add(new_count) > MAX_EFFECT_TASKS {
            return Err(format!(
                "runtime effect queue limit {MAX_EFFECT_TASKS} reached"
            ));
        }
        {
            let mut insert_statement = transaction
                .prepare_cached(
                    "INSERT INTO runtime_effect_tasks
                     (effect_id, kind, payload, state, attempt, max_attempts,
                      available_at_unix_ms, created_at_unix_ms, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, 'ready', 0, ?4, ?5, ?5, ?5)",
                )
                .map_err(|error| error.to_string())?;
            for effect in new_effects {
                insert_statement
                    .execute(params![
                        effect.effect_id,
                        effect.kind,
                        effect.payload,
                        i64::from(effect.max_attempts),
                        now
                    ])
                    .map_err(|error| error.to_string())?;
            }
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
        let journal_timestamp = next_runtime_unregistration_timestamp(&transaction)?;
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
                    journal_timestamp
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

fn delete_orchestra_runtimes_in_transaction(
    transaction: &Transaction<'_>,
    runtime_ids: &[String],
) -> Result<OrchestraDeleteRecord, String> {
    if runtime_ids.is_empty() || runtime_ids.len() > 128 {
        return Err("Orchestra deletion snapshot target set is invalid".into());
    }
    let placeholders = (1..=runtime_ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let parameters = || runtime_ids.iter().map(String::as_str);
    let run_metrics_query = format!(
        "SELECT COUNT(*), COUNT(DISTINCT runtime_id)
         FROM orchestra_runs WHERE runtime_id IN ({placeholders})"
    );
    let (run_count, runtime_count): (i64, i64) = transaction
        .query_row(&run_metrics_query, params_from_iter(parameters()), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|error| error.to_string())?;
    let run_count = u64::try_from(run_count)
        .map_err(|_| "Orchestra deletion run count is invalid".to_string())?;
    let runtime_count = u32::try_from(runtime_count)
        .map_err(|_| "Orchestra deletion runtime count is invalid".to_string())?;
    let maximum_run_count = runtime_ids
        .len()
        .checked_mul(MAX_ORCHESTRA_RUNS_PER_RUNTIME)
        .and_then(|count| u64::try_from(count).ok())
        .ok_or_else(|| "Orchestra deletion run bound is invalid".to_string())?;
    if run_count > maximum_run_count {
        return Err("Orchestra deletion run set exceeds its retention bound".into());
    }

    let event_count_query = format!(
        "SELECT COUNT(*) FROM orchestra_events AS event
         JOIN orchestra_runs AS run ON run.run_id = event.run_id
         WHERE run.runtime_id IN ({placeholders})"
    );
    let event_count: i64 = transaction
        .query_row(&event_count_query, params_from_iter(parameters()), |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    let event_count = u64::try_from(event_count)
        .map_err(|_| "Orchestra deletion event count is invalid".to_string())?;
    let maximum_event_count = run_count
        .checked_mul(
            u64::try_from(MAX_ORCHESTRA_EVENTS_PER_RUN)
                .map_err(|_| "Orchestra deletion event bound is invalid".to_string())?,
        )
        .ok_or_else(|| "Orchestra deletion event bound is invalid".to_string())?;
    if event_count > maximum_event_count {
        return Err("Orchestra deletion event set exceeds its state-machine bound".into());
    }

    let mismatched_event_query = format!(
        "SELECT COUNT(*) FROM orchestra_events AS event
         JOIN orchestra_runs AS run ON run.run_id = event.run_id
         WHERE (run.runtime_id IN ({placeholders}))
            != (event.runtime_id IN ({placeholders}))"
    );
    let mismatched_event_count: i64 = transaction
        .query_row(
            &mismatched_event_query,
            params_from_iter(parameters()),
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if mismatched_event_count != 0 {
        return Err("Orchestra deletion event ownership is inconsistent".into());
    }

    let changes_before: i64 = transaction
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let delete_query = format!("DELETE FROM orchestra_runs WHERE runtime_id IN ({placeholders})");
    let changed_runs = transaction
        .execute(&delete_query, params_from_iter(parameters()))
        .map_err(|error| error.to_string())?;

    let post_delete_query = format!(
        "SELECT
             (SELECT COUNT(*) FROM orchestra_runs
              WHERE runtime_id IN ({placeholders})),
             (SELECT COUNT(*) FROM orchestra_events
              WHERE runtime_id IN ({placeholders}))"
    );
    let (remaining_runs, remaining_events): (i64, i64) = transaction
        .query_row(&post_delete_query, params_from_iter(parameters()), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|error| error.to_string())?;
    let changes_after: i64 = transaction
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let observed_changes = changes_after
        .checked_sub(changes_before)
        .and_then(|changes| u64::try_from(changes).ok())
        .ok_or_else(|| "Orchestra deletion mutation count is invalid".to_string())?;
    let expected_changes = run_count
        .checked_add(event_count)
        .ok_or_else(|| "Orchestra deletion mutation count overflow".to_string())?;
    if u64::try_from(changed_runs).ok() != Some(run_count)
        || remaining_runs != 0
        || remaining_events != 0
        || observed_changes != expected_changes
    {
        return Err("Orchestra deletion post-write snapshot is inconsistent".into());
    }
    Ok(OrchestraDeleteRecord {
        deleted_runtime_count: runtime_count,
        deleted_run_count: run_count,
        deleted_event_count: event_count,
    })
}

fn canonical_orchestra_delete_runtime_ids(runtime_ids: &[String]) -> Result<Vec<String>, String> {
    if runtime_ids.is_empty() || runtime_ids.len() > 128 {
        return Err("Orchestra delete must contain between 1 and 128 runtime IDs".into());
    }
    let mut canonical = runtime_ids.to_vec();
    canonical.sort();
    for runtime_id in &canonical {
        validate_scheduler_id("runtime_id", runtime_id)?;
    }
    if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("Orchestra delete contains a duplicate runtime ID".into());
    }
    Ok(canonical)
}

fn load_orchestra_delete_operation_record(
    transaction: &Transaction<'_>,
    operation_id: &str,
) -> Result<Option<OrchestraDeleteOperationRecord>, String> {
    transaction
        .query_row(
            "SELECT generation, request, deleted_runtime_count, deleted_run_count,
                    deleted_event_count, committed_at_unix_ms
             FROM orchestra_delete_operations WHERE operation_id = ?1",
            [operation_id],
            |row| {
                let generation = row.get::<_, i64>(0)?;
                let deleted_runtime_count = row.get::<_, i64>(2)?;
                let deleted_run_count = row.get::<_, i64>(3)?;
                let deleted_event_count = row.get::<_, i64>(4)?;
                Ok((
                    generation,
                    row.get::<_, Vec<u8>>(1)?,
                    deleted_runtime_count,
                    deleted_run_count,
                    deleted_event_count,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .map(
            |(
                generation,
                request,
                deleted_runtime_count,
                deleted_run_count,
                deleted_event_count,
                committed_at_unix_ms,
            )| {
                Ok(OrchestraDeleteOperationRecord {
                    generation: u64::try_from(generation)
                        .map_err(|_| "Orchestra delete generation is invalid")?,
                    request,
                    deleted_runtime_count: u32::try_from(deleted_runtime_count)
                        .map_err(|_| "Orchestra delete runtime count is invalid")?,
                    deleted_run_count: u64::try_from(deleted_run_count)
                        .map_err(|_| "Orchestra delete run count is invalid")?,
                    deleted_event_count: u64::try_from(deleted_event_count)
                        .map_err(|_| "Orchestra delete event count is invalid")?,
                    committed_at_unix_ms,
                })
            },
        )
        .transpose()
}

fn load_orchestra_delete_replay_horizon(
    connection: &Connection,
) -> Result<OrchestraDeleteReplayHorizon, String> {
    let (retained, oldest_generation, newest_generation): (i64, Option<i64>, Option<i64>) =
        connection
            .query_row(
                "SELECT COUNT(*), MIN(generation), MAX(generation)
                 FROM orchestra_delete_operations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|error| error.to_string())?;
    let next_generation: i64 = connection
        .query_row(
            "SELECT next_generation FROM orchestra_delete_generation WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let (evicted_through_generation, protected_from_generation, checkpointed_through_generation): (
        i64,
        Option<i64>,
        Option<i64>,
    ) = connection
        .query_row(
            "SELECT evicted_through_generation, protected_from_generation,
                    checkpointed_through_generation
             FROM orchestra_delete_replay_horizon WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| error.to_string())?;
    let retained = u64::try_from(retained)
        .map_err(|_| "Orchestra delete replay horizon retained count is invalid")?;
    let capacity = u64::try_from(ORCHESTRA_DELETE_REPLAY_HORIZON)
        .map_err(|_| "Orchestra delete replay horizon capacity is invalid")?;
    let oldest_generation = oldest_generation
        .map(u64::try_from)
        .transpose()
        .map_err(|_| "Orchestra delete replay horizon oldest generation is invalid")?;
    let newest_generation = newest_generation
        .map(u64::try_from)
        .transpose()
        .map_err(|_| "Orchestra delete replay horizon newest generation is invalid")?;
    let next_generation = u64::try_from(next_generation)
        .map_err(|_| "Orchestra delete replay horizon next generation is invalid")?;
    let evicted_through_generation = u64::try_from(evicted_through_generation)
        .map_err(|_| "Orchestra delete replay horizon high-water mark is invalid")?;
    let protected_from_generation = protected_from_generation
        .map(u64::try_from)
        .transpose()
        .map_err(|_| "Orchestra delete replay horizon checkpoint is invalid")?;
    let checkpointed_through_generation = checkpointed_through_generation
        .map(u64::try_from)
        .transpose()
        .map_err(|_| "Orchestra delete replay horizon checkpoint high-water is invalid")?;
    let contiguous = match (oldest_generation, newest_generation) {
        (None, None) => {
            retained == 0
                && evicted_through_generation
                    .checked_add(1)
                    .is_some_and(|next| next == next_generation)
                && protected_from_generation.is_none()
                && checkpointed_through_generation.is_none()
        }
        (Some(oldest), Some(newest)) => {
            retained > 0
                && evicted_through_generation
                    .checked_add(1)
                    .is_some_and(|expected| expected == oldest)
                && newest
                    .checked_add(1)
                    .is_some_and(|expected| expected == next_generation)
                && newest
                    .checked_sub(oldest)
                    .and_then(|span| span.checked_add(1))
                    .is_some_and(|span| span == retained)
                && protected_from_generation
                    .is_some_and(|protected| protected >= oldest && protected <= newest)
                && checkpointed_through_generation.is_none_or(|checkpointed| {
                    protected_from_generation.is_some_and(|protected| {
                        checkpointed >= protected && checkpointed <= newest
                    })
                })
        }
        _ => false,
    };
    if retained > capacity
        || next_generation == 0
        || evicted_through_generation >= next_generation
        || !contiguous
    {
        return Err("Orchestra delete replay horizon metadata is inconsistent".into());
    }
    Ok(OrchestraDeleteReplayHorizon {
        capacity,
        retained,
        oldest_generation,
        newest_generation,
        next_generation,
        evicted_through_generation,
        protected_from_generation,
        checkpointed_through_generation,
    })
}

fn compact_orchestra_delete_before_protected(transaction: &Transaction<'_>) -> Result<u64, String> {
    let horizon = load_orchestra_delete_replay_horizon(transaction)?;
    let Some(protected_from_generation) = horizon.protected_from_generation else {
        return Ok(0);
    };
    let protected = i64::try_from(protected_from_generation)
        .map_err(|_| "Orchestra delete replay checkpoint is invalid".to_string())?;
    let (candidate_count, high_water): (i64, Option<i64>) = transaction
        .query_row(
            "SELECT COUNT(*), MAX(generation)
             FROM orchestra_delete_operations WHERE generation < ?1",
            [protected],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    if candidate_count == 0 {
        return Ok(0);
    }
    let candidate_operation_ids = {
        let mut statement = transaction
            .prepare(
                "SELECT operation_id FROM orchestra_delete_operations
                 WHERE generation < ?1 ORDER BY generation",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([protected], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    if i64::try_from(candidate_operation_ids.len()).ok() != Some(candidate_count) {
        return Err("Orchestra delete replay compaction plan is inconsistent".into());
    }
    for operation_id in candidate_operation_ids {
        validate_scheduler_id("Orchestra delete operation_id", &operation_id)?;
        let record = load_orchestra_delete_operation_record(transaction, &operation_id)?
            .ok_or_else(|| {
                "Orchestra delete replay compaction candidate disappeared".to_string()
            })?;
        validate_orchestra_delete_replay_snapshot(transaction, &record)?;
    }
    let high_water = high_water
        .ok_or_else(|| "Orchestra delete replay compaction plan is incomplete".to_string())?;
    let deleted = transaction
        .execute(
            "DELETE FROM orchestra_delete_operations WHERE generation < ?1",
            [protected],
        )
        .map_err(|error| error.to_string())?;
    let updated = transaction
        .execute(
            "UPDATE orchestra_delete_replay_horizon
             SET evicted_through_generation = ?1
             WHERE id = 1 AND evicted_through_generation < ?1",
            [high_water],
        )
        .map_err(|error| error.to_string())?;
    if i64::try_from(deleted).ok() != Some(candidate_count) || updated != 1 {
        return Err("Orchestra delete replay compaction is inconsistent".into());
    }
    load_orchestra_delete_replay_horizon(transaction)?;
    u64::try_from(deleted)
        .map_err(|_| "Orchestra delete replay compaction count is invalid".to_string())
}

fn evict_orchestra_delete_replay_horizon(
    transaction: &Transaction<'_>,
    incoming_operations: usize,
) -> Result<u64, String> {
    if incoming_operations > ORCHESTRA_DELETE_REPLAY_HORIZON {
        return Err("Orchestra delete replay horizon admission is invalid".into());
    }
    let compacted = compact_orchestra_delete_before_protected(transaction)?;
    let horizon = load_orchestra_delete_replay_horizon(transaction)?;
    let admitted = horizon
        .retained
        .checked_add(
            u64::try_from(incoming_operations)
                .map_err(|_| "Orchestra delete replay horizon admission is invalid")?,
        )
        .ok_or_else(|| "Orchestra delete replay horizon admission is invalid".to_string())?;
    if admitted > horizon.capacity {
        return Err(ORCHESTRA_DELETE_REPLAY_HORIZON_PINNED_ERROR.into());
    }
    Ok(compacted)
}

fn allocate_orchestra_delete_generation(transaction: &Transaction<'_>) -> Result<u64, String> {
    let next: i64 = transaction
        .query_row(
            "SELECT next_generation FROM orchestra_delete_generation WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE orchestra_delete_generation
             SET next_generation = next_generation + 1
             WHERE id = 1 AND next_generation = ?1",
            [next],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("Orchestra delete generation allocation conflicted".into());
    }
    u64::try_from(next).map_err(|_| "Orchestra delete generation is invalid".into())
}

fn next_orchestra_delete_timestamp(transaction: &Transaction<'_>) -> Result<i64, String> {
    let now = unix_time_ms()?;
    let previous: Option<i64> = transaction
        .query_row(
            "SELECT MAX(committed_at_unix_ms) FROM orchestra_delete_operations",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    previous.map_or(Ok(now), |value| {
        value
            .checked_add(1)
            .map(|next| now.max(next))
            .ok_or_else(|| "Orchestra delete timestamp overflow".to_string())
    })
}

fn validate_orchestra_delete_replay_snapshot(
    transaction: &Transaction<'_>,
    record: &OrchestraDeleteOperationRecord,
) -> Result<(), String> {
    if record.generation == 0 || record.committed_at_unix_ms < 0 {
        return Err("Orchestra delete receipt metadata is invalid".into());
    }
    let runtime_ids: Vec<String> = serde_json::from_slice(&record.request)
        .map_err(|_| "Orchestra delete receipt request is invalid".to_string())?;
    let canonical = canonical_orchestra_delete_runtime_ids(&runtime_ids)?;
    let canonical_request = serde_json::to_vec(&canonical)
        .map_err(|error| format!("failed to encode Orchestra delete request: {error}"))?;
    if canonical_request != record.request {
        return Err("Orchestra delete receipt request is not canonical".into());
    }
    let maximum_run_count = canonical
        .len()
        .checked_mul(MAX_ORCHESTRA_RUNS_PER_RUNTIME)
        .and_then(|count| u64::try_from(count).ok())
        .ok_or_else(|| "Orchestra delete receipt run bound is invalid".to_string())?;
    let maximum_event_count = maximum_run_count
        .checked_mul(
            u64::try_from(MAX_ORCHESTRA_EVENTS_PER_RUN)
                .map_err(|_| "Orchestra delete receipt event bound is invalid")?,
        )
        .ok_or_else(|| "Orchestra delete receipt event bound is invalid".to_string())?;
    if usize::try_from(record.deleted_runtime_count).ok() > Some(canonical.len())
        || record.deleted_run_count > maximum_run_count
        || record.deleted_event_count > maximum_event_count
    {
        return Err("Orchestra delete receipt counts exceed their bounds".into());
    }
    let placeholders = (1..=canonical.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "SELECT
             (SELECT COUNT(*) FROM orchestra_runs WHERE runtime_id IN ({placeholders})),
             (SELECT COUNT(*) FROM orchestra_events WHERE runtime_id IN ({placeholders}))"
    );
    let (remaining_runs, remaining_events): (i64, i64) = transaction
        .query_row(
            &query,
            params_from_iter(canonical.iter().map(String::as_str)),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    if remaining_runs != 0 || remaining_events != 0 {
        return Err("Orchestra delete receipt target tombstone is inconsistent".into());
    }
    Ok(())
}

fn load_runtime_unregistration_operation_record(
    transaction: &Transaction<'_>,
    operation_id: &str,
) -> Result<Option<RuntimeUnregistrationOperationRecord>, String> {
    transaction
        .query_row(
            "SELECT generation, request, deleted_runtime_count, deleted_run_count,
                    deleted_event_count, removed_at_unix_ms
             FROM runtime_unregistration_operations WHERE operation_id = ?1",
            [operation_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .map(
            |(
                generation,
                request,
                deleted_runtime_count,
                deleted_run_count,
                deleted_event_count,
                removed_at_unix_ms,
            )| {
                Ok(RuntimeUnregistrationOperationRecord {
                    generation: u64::try_from(generation)
                        .map_err(|_| "invalid runtime unregistration generation".to_string())?,
                    request,
                    deleted_runtime_count: u32::try_from(deleted_runtime_count)
                        .map_err(|_| "invalid deleted runtime count".to_string())?,
                    deleted_run_count: u64::try_from(deleted_run_count)
                        .map_err(|_| "invalid deleted run count".to_string())?,
                    deleted_event_count: u64::try_from(deleted_event_count)
                        .map_err(|_| "invalid deleted event count".to_string())?,
                    removed_at_unix_ms,
                })
            },
        )
        .transpose()
}

fn next_runtime_unregistration_timestamp(transaction: &Transaction<'_>) -> Result<i64, String> {
    let wall_time = unix_time_ms()?;
    let retained_maximum: Option<i64> = transaction
        .query_row(
            "SELECT MAX(created_at_unix_ms) FROM runtime_journal
             WHERE kind = ?1",
            [JournalEntryKind::RuntimeUnregistration.as_str()],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let Some(retained_maximum) = retained_maximum else {
        return Ok(wall_time);
    };
    let next_generation = retained_maximum
        .checked_add(1)
        .ok_or_else(|| "runtime unregistration timestamp is exhausted".to_string())?;
    Ok(wall_time.max(next_generation))
}

fn load_runtime_unregistration_replay_horizon(
    connection: &Connection,
) -> Result<RuntimeUnregistrationReplayHorizon, String> {
    let (retained, oldest_generation, newest_generation): (i64, Option<i64>, Option<i64>) =
        connection
            .query_row(
                "SELECT COUNT(*), MIN(generation), MAX(generation)
                 FROM runtime_unregistration_operations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|error| error.to_string())?;
    let (next_generation, evicted_through_generation): (i64, i64) = connection
        .query_row(
            "SELECT next_generation, evicted_through_generation
             FROM runtime_unregistration_replay_horizon WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    let retained = u64::try_from(retained)
        .map_err(|_| "runtime unregistration replay horizon retained count is invalid")?;
    let capacity = u64::try_from(RUNTIME_UNREGISTRATION_REPLAY_HORIZON)
        .map_err(|_| "runtime unregistration replay horizon capacity is invalid")?;
    let oldest_generation = oldest_generation
        .map(u64::try_from)
        .transpose()
        .map_err(|_| "runtime unregistration replay horizon oldest generation is invalid")?;
    let newest_generation = newest_generation
        .map(u64::try_from)
        .transpose()
        .map_err(|_| "runtime unregistration replay horizon newest generation is invalid")?;
    let next_generation = u64::try_from(next_generation)
        .map_err(|_| "runtime unregistration replay horizon next generation is invalid")?;
    let evicted_through_generation = u64::try_from(evicted_through_generation)
        .map_err(|_| "runtime unregistration replay horizon high-water mark is invalid")?;
    let contiguous = match (oldest_generation, newest_generation) {
        (None, None) => {
            retained == 0
                && evicted_through_generation
                    .checked_add(1)
                    .is_some_and(|next| next == next_generation)
        }
        (Some(oldest), Some(newest)) => {
            retained > 0
                && evicted_through_generation
                    .checked_add(1)
                    .is_some_and(|expected| expected == oldest)
                && newest
                    .checked_add(1)
                    .is_some_and(|expected| expected == next_generation)
                && newest
                    .checked_sub(oldest)
                    .and_then(|span| span.checked_add(1))
                    .is_some_and(|span| span == retained)
        }
        _ => false,
    };
    if retained > capacity
        || next_generation == 0
        || evicted_through_generation >= next_generation
        || !contiguous
    {
        return Err("runtime unregistration replay horizon metadata is inconsistent".into());
    }
    Ok(RuntimeUnregistrationReplayHorizon {
        capacity,
        retained,
        oldest_generation,
        newest_generation,
        next_generation,
        evicted_through_generation,
    })
}

fn allocate_runtime_unregistration_generation(
    transaction: &Transaction<'_>,
) -> Result<u64, String> {
    let horizon = load_runtime_unregistration_replay_horizon(transaction)?;
    let next = i64::try_from(horizon.next_generation)
        .map_err(|_| "runtime unregistration generation is exhausted".to_string())?;
    let following = next
        .checked_add(1)
        .ok_or_else(|| "runtime unregistration generation is exhausted".to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_unregistration_replay_horizon
             SET next_generation = ?1
             WHERE id = 1 AND next_generation = ?2",
            params![following, next],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime unregistration generation allocation is inconsistent".into());
    }
    u64::try_from(next).map_err(|_| "runtime unregistration generation is invalid".to_string())
}

fn evict_runtime_unregistration_replay_horizon(
    transaction: &Transaction<'_>,
    incoming_operations: usize,
) -> Result<u64, String> {
    if incoming_operations > RUNTIME_UNREGISTRATION_REPLAY_HORIZON {
        return Err("runtime unregistration replay horizon admission is invalid".into());
    }
    let operation_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM runtime_unregistration_operations",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !(0..=MAX_JOURNAL_RECORDS).contains(&operation_count) {
        return Err("runtime unregistration replay horizon size is invalid".into());
    }
    let incoming_operations = i64::try_from(incoming_operations)
        .map_err(|_| "runtime unregistration replay horizon admission is invalid".to_string())?;
    let replay_horizon = i64::try_from(RUNTIME_UNREGISTRATION_REPLAY_HORIZON)
        .map_err(|_| "runtime unregistration replay horizon is invalid".to_string())?;
    let eviction_count = operation_count
        .checked_add(incoming_operations)
        .and_then(|count| count.checked_sub(replay_horizon))
        .unwrap_or(0)
        .max(0);
    if eviction_count == 0 {
        return Ok(0);
    }
    let operations = {
        let mut statement = transaction
            .prepare(
                "SELECT operation_id, generation FROM runtime_unregistration_operations
                 ORDER BY generation ASC LIMIT ?1",
            )
            .map_err(|error| error.to_string())?;
        statement
            .query_map([eviction_count], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    if i64::try_from(operations.len()).ok() != Some(eviction_count) {
        return Err("runtime unregistration replay horizon plan is incomplete".into());
    }
    let (next_generation, evicted_through_generation): (i64, i64) = transaction
        .query_row(
            "SELECT next_generation, evicted_through_generation
             FROM runtime_unregistration_replay_horizon WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    if next_generation <= 0
        || evicted_through_generation < 0
        || evicted_through_generation >= next_generation
    {
        return Err("runtime unregistration replay horizon metadata is inconsistent".into());
    }
    let high_water = operations
        .last()
        .map(|(_, generation)| *generation)
        .ok_or_else(|| "runtime unregistration replay horizon plan is incomplete".to_string())?;
    if operations.iter().any(|(_, generation)| {
        *generation <= evicted_through_generation || *generation >= next_generation
    }) {
        return Err("runtime unregistration replay horizon generation is inconsistent".into());
    }

    let changes_before: i64 = transaction
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let mut deleted_operations = 0_usize;
    for (operation_id, generation) in &operations {
        let record = load_runtime_unregistration_operation_record(transaction, operation_id)?
            .ok_or_else(|| {
                "runtime unregistration replay horizon operation is missing".to_string()
            })?;
        if record.generation != u64::try_from(*generation).unwrap_or_default() {
            return Err("runtime unregistration replay horizon generation is inconsistent".into());
        }
        validate_runtime_unregistration_operation_journal(transaction, &record)?;
        deleted_operations = deleted_operations
            .checked_add(
                transaction
                    .execute(
                        "DELETE FROM runtime_unregistration_operations
                         WHERE operation_id = ?1 AND generation = ?2",
                        params![operation_id, generation],
                    )
                    .map_err(|error| error.to_string())?,
            )
            .ok_or_else(|| {
                "runtime unregistration replay horizon mutation count is invalid".to_string()
            })?;
    }
    let high_water_updated = transaction
        .execute(
            "UPDATE runtime_unregistration_replay_horizon
             SET evicted_through_generation = ?1
             WHERE id = 1 AND evicted_through_generation = ?2 AND next_generation > ?1",
            params![high_water, evicted_through_generation],
        )
        .map_err(|error| error.to_string())?;
    let remaining_operations: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM runtime_unregistration_operations
             WHERE generation <= ?1",
            [high_water],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let changes_after: i64 = transaction
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let expected_changes = operations
        .len()
        .checked_add(1)
        .ok_or("runtime unregistration replay horizon mutation count is invalid")?;
    let observed_changes = changes_after
        .checked_sub(changes_before)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| {
            "runtime unregistration replay horizon mutation count is invalid".to_string()
        })?;
    if deleted_operations != operations.len()
        || high_water_updated != 1
        || remaining_operations != 0
        || observed_changes != expected_changes
    {
        return Err("runtime unregistration replay horizon eviction is inconsistent".into());
    }
    load_runtime_unregistration_replay_horizon(transaction)?;
    u64::try_from(operations.len())
        .map_err(|_| "runtime unregistration replay horizon eviction count is invalid".to_string())
}

fn retained_runtime_unregistration_journal_sequences(
    transaction: &Transaction<'_>,
) -> Result<BTreeSet<i64>, String> {
    let operation_ids = {
        let mut statement = transaction
            .prepare(
                "SELECT operation_id FROM runtime_unregistration_operations
                 ORDER BY generation ASC",
            )
            .map_err(|error| error.to_string())?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    if operation_ids.len() > RUNTIME_UNREGISTRATION_REPLAY_HORIZON {
        return Err("runtime unregistration replay horizon exceeds its bound".into());
    }
    let mut protected_sequences = BTreeSet::new();
    for operation_id in operation_ids {
        let record = load_runtime_unregistration_operation_record(transaction, &operation_id)?
            .ok_or_else(|| {
                "runtime unregistration replay horizon operation is missing".to_string()
            })?;
        let validated = validate_runtime_unregistration_operation_journal(transaction, &record)?;
        for sequence in validated.journal_sequences {
            if !protected_sequences.insert(sequence) {
                return Err(
                    "runtime unregistration replay horizon journal ownership is inconsistent"
                        .into(),
                );
            }
        }
    }
    Ok(protected_sequences)
}

fn compact_runtime_journal(
    transaction: &Transaction<'_>,
    boundary: i64,
    protected_unregistration_sequences: &BTreeSet<i64>,
) -> Result<u64, String> {
    if boundary < 0 {
        return Err("runtime journal compaction boundary is invalid".into());
    }
    let compaction_limit = usize::try_from(MAX_COMPACTION_RECORDS)
        .map_err(|_| "runtime journal compaction bound is invalid".to_string())?;
    let mut candidates = Vec::with_capacity(compaction_limit);
    let mut cursor = 0_i64;
    while candidates.len() < compaction_limit {
        let rows = {
            let mut statement = transaction
                .prepare(
                    "SELECT sequence, kind FROM runtime_journal
                     WHERE sequence > ?1 AND sequence <= ?2
                     ORDER BY sequence ASC LIMIT ?3",
                )
                .map_err(|error| error.to_string())?;
            statement
                .query_map(params![cursor, boundary, MAX_COMPACTION_RECORDS], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?
        };
        let Some((last_sequence, _)) = rows.last() else {
            break;
        };
        cursor = *last_sequence;
        for (sequence, kind) in rows {
            if kind != JournalEntryKind::RuntimeUnregistration.as_str()
                || !protected_unregistration_sequences.contains(&sequence)
            {
                candidates.push(sequence);
                if candidates.len() == compaction_limit {
                    break;
                }
            }
        }
    }
    if candidates.is_empty() {
        return Ok(0);
    }

    let changes_before: i64 = transaction
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let mut deleted = 0_usize;
    let mut remaining = 0_i64;
    for chunk in candidates.chunks(900) {
        let placeholders = (1..=chunk.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let delete_query =
            format!("DELETE FROM runtime_journal WHERE sequence IN ({placeholders})");
        deleted = deleted
            .checked_add(
                transaction
                    .execute(&delete_query, params_from_iter(chunk.iter()))
                    .map_err(|error| error.to_string())?,
            )
            .ok_or_else(|| "runtime journal compaction count is invalid".to_string())?;
        let remaining_query =
            format!("SELECT COUNT(*) FROM runtime_journal WHERE sequence IN ({placeholders})");
        remaining = remaining
            .checked_add(
                transaction
                    .query_row(&remaining_query, params_from_iter(chunk.iter()), |row| {
                        row.get::<_, i64>(0)
                    })
                    .map_err(|error| error.to_string())?,
            )
            .ok_or_else(|| "runtime journal compaction count is invalid".to_string())?;
    }
    let changes_after: i64 = transaction
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let observed_changes = changes_after
        .checked_sub(changes_before)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| "runtime journal compaction mutation count is invalid".to_string())?;
    if deleted != candidates.len() || remaining != 0 || observed_changes != candidates.len() {
        return Err("runtime journal compaction post-write snapshot is inconsistent".into());
    }
    u64::try_from(deleted).map_err(|_| "runtime journal compaction count is invalid".to_string())
}

fn validate_runtime_unregistration_replay_snapshot(
    transaction: &Transaction<'_>,
    record: &RuntimeUnregistrationOperationRecord,
) -> Result<(), String> {
    let validated = validate_runtime_unregistration_operation_journal(transaction, record)?;
    let placeholders = (1..=validated.runtime_ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let parameters = || validated.runtime_ids.iter().map(String::as_str);
    let tombstone_query = format!(
        "SELECT
             (SELECT COUNT(*) FROM orchestra_runs
              WHERE runtime_id IN ({placeholders})),
             (SELECT COUNT(*) FROM orchestra_events
              WHERE runtime_id IN ({placeholders})),
             (SELECT COUNT(*) FROM orchestra_events AS event
              JOIN orchestra_runs AS run ON run.run_id = event.run_id
              WHERE run.runtime_id IN ({placeholders}))"
    );
    let (remaining_runs, remaining_events, remaining_parent_events): (i64, i64, i64) = transaction
        .query_row(&tombstone_query, params_from_iter(parameters()), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|error| error.to_string())?;
    if remaining_runs != 0 || remaining_events != 0 || remaining_parent_events != 0 {
        return Err("runtime unregistration Orchestra tombstone is inconsistent".into());
    }
    Ok(())
}

fn validate_runtime_unregistration_operation_journal(
    transaction: &Transaction<'_>,
    record: &RuntimeUnregistrationOperationRecord,
) -> Result<ValidatedRuntimeUnregistrationOperation, String> {
    validate_blob("runtime unregistration request", &record.request)?;
    let targets: Vec<RuntimeUnregisterTarget> = serde_json::from_slice(&record.request)
        .map_err(|_| "runtime unregistration operation request is invalid".to_string())?;
    if targets.is_empty() || targets.len() > 128 {
        return Err("runtime unregistration operation target set is invalid".into());
    }
    let canonical_request = serde_json::to_vec(&targets)
        .map_err(|error| format!("runtime unregistration request encoding failed: {error}"))?;
    if canonical_request != record.request {
        return Err("runtime unregistration operation request is not canonical".into());
    }
    let mut runtime_ids = BTreeSet::new();
    for target in &targets {
        if !runtime_ids.insert(target.runtime_id.as_str()) {
            return Err("runtime unregistration operation contains a duplicate runtime ID".into());
        }
    }

    let target_count = u64::try_from(targets.len())
        .map_err(|_| "runtime unregistration operation target count is invalid".to_string())?;
    let maximum_run_count = target_count
        .checked_mul(
            u64::try_from(MAX_ORCHESTRA_RUNS_PER_RUNTIME)
                .map_err(|_| "runtime unregistration operation run bound is invalid".to_string())?,
        )
        .ok_or_else(|| "runtime unregistration operation run bound is invalid".to_string())?;
    let maximum_event_count =
        record
            .deleted_run_count
            .checked_mul(u64::try_from(MAX_ORCHESTRA_EVENTS_PER_RUN).map_err(|_| {
                "runtime unregistration operation event bound is invalid".to_string()
            })?)
            .ok_or_else(|| "runtime unregistration operation event bound is invalid".to_string())?;
    if record.generation == 0
        || record.removed_at_unix_ms < 0
        || u64::from(record.deleted_runtime_count) > target_count
        || u64::from(record.deleted_runtime_count) > record.deleted_run_count
        || (record.deleted_run_count > 0 && record.deleted_runtime_count == 0)
        || record.deleted_run_count > maximum_run_count
        || record.deleted_event_count > maximum_event_count
    {
        return Err("runtime unregistration operation receipt is inconsistent".into());
    }

    let mut expected_tombstones = targets
        .iter()
        .map(|target| {
            serde_json::to_vec(&RuntimeUnregistration {
                runtime_id: target.runtime_id.as_str().to_string(),
            })
            .map_err(|error| format!("runtime unregistration tombstone encoding failed: {error}"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let payload_placeholders = (3..3 + expected_tombstones.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let limit_parameter = expected_tombstones
        .len()
        .checked_add(3)
        .ok_or_else(|| "runtime unregistration tombstone bound is invalid".to_string())?;
    let tombstone_limit = i64::try_from(
        expected_tombstones
            .len()
            .checked_add(1)
            .ok_or_else(|| "runtime unregistration tombstone bound is invalid".to_string())?,
    )
    .map_err(|_| "runtime unregistration tombstone bound is invalid".to_string())?;
    let journal_tombstone_query = format!(
        "SELECT sequence, payload, outcome, terminal_error
         FROM runtime_journal
         WHERE kind = ?1 AND created_at_unix_ms = ?2
           AND payload IN ({payload_placeholders})
         ORDER BY sequence ASC LIMIT ?{limit_parameter}"
    );
    let mut journal_parameters = Vec::with_capacity(expected_tombstones.len() + 3);
    journal_parameters.push(Value::Text(
        JournalEntryKind::RuntimeUnregistration.as_str().to_string(),
    ));
    journal_parameters.push(Value::Integer(record.removed_at_unix_ms));
    journal_parameters.extend(expected_tombstones.iter().cloned().map(Value::Blob));
    journal_parameters.push(Value::Integer(tombstone_limit));
    let mut statement = transaction
        .prepare(&journal_tombstone_query)
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params_from_iter(journal_parameters.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut observed_tombstones = Vec::new();
    let mut journal_sequences = Vec::new();
    for row in rows {
        let (sequence, payload, outcome, terminal_error) =
            row.map_err(|error| error.to_string())?;
        if sequence <= 0 || outcome.is_some() || terminal_error.is_some() {
            return Err("runtime unregistration journal tombstone is inconsistent".into());
        }
        observed_tombstones.push(payload);
        journal_sequences.push(sequence);
    }
    expected_tombstones.sort();
    observed_tombstones.sort();
    if observed_tombstones != expected_tombstones {
        return Err("runtime unregistration journal tombstone is inconsistent".into());
    }

    Ok(ValidatedRuntimeUnregistrationOperation {
        runtime_ids: runtime_ids.into_iter().map(str::to_string).collect(),
        journal_sequences,
    })
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
        AUTHORITY_KIND_DAEMON_RETIREMENT => matches!(
            phase,
            "planned" | "retiring_service" | "service_retired" | "failed"
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

fn validate_runtime_target_registration(
    operation_id: &str,
    runtime_id: &str,
    secret_key: &str,
    payload: &[u8],
) -> Result<(), String> {
    validate_scheduler_id("runtime target registration operation ID", operation_id)?;
    RuntimeId::new(runtime_id.to_string())
        .map_err(|_| "runtime target registration runtime ID is invalid".to_string())?;
    validate_scheduler_id("runtime target secret key", secret_key)?;
    if payload.is_empty() || payload.len() > MAX_RUNTIME_TARGET_REGISTRATION_PAYLOAD_BYTES {
        return Err("runtime target registration payload is invalid".into());
    }
    Ok(())
}

fn runtime_target_registration_timestamp_column(table: &str) -> Result<&'static str, String> {
    match table {
        "runtime_target_registration_intents" => Ok("created_at_unix_ms"),
        "runtime_target_bindings" => Ok("updated_at_unix_ms"),
        _ => Err("runtime target registration table is invalid".into()),
    }
}

fn map_runtime_target_registration(
    row: &Row<'_>,
) -> rusqlite::Result<RuntimeTargetRegistrationRecord> {
    Ok(RuntimeTargetRegistrationRecord {
        operation_id: row.get(0)?,
        runtime_id: row.get(1)?,
        secret_key: row.get(2)?,
        payload: row.get(3)?,
        recorded_at_unix_ms: row.get(4)?,
    })
}

fn validate_runtime_target_registration_record(
    record: RuntimeTargetRegistrationRecord,
) -> Result<RuntimeTargetRegistrationRecord, String> {
    validate_runtime_target_registration(
        &record.operation_id,
        &record.runtime_id,
        &record.secret_key,
        &record.payload,
    )?;
    if record.recorded_at_unix_ms < 0 {
        return Err("runtime target registration timestamp is invalid".into());
    }
    Ok(record)
}

fn load_runtime_target_registration(
    connection: &Connection,
    table: &str,
    predicate: &str,
    value: &str,
) -> Result<Option<RuntimeTargetRegistrationRecord>, String> {
    let timestamp = runtime_target_registration_timestamp_column(table)?;
    if !matches!(predicate, "operation_id" | "runtime_id") {
        return Err("runtime target registration predicate is invalid".into());
    }
    let query = format!(
        "SELECT operation_id, runtime_id, secret_key, payload, {timestamp}
         FROM {table} WHERE {predicate} = ?1"
    );
    connection
        .query_row(&query, [value], map_runtime_target_registration)
        .optional()
        .map_err(|error| error.to_string())?
        .map(validate_runtime_target_registration_record)
        .transpose()
}

fn load_runtime_target_registration_by_operation(
    connection: &Connection,
    table: &str,
    operation_id: &str,
) -> Result<Option<RuntimeTargetRegistrationRecord>, String> {
    load_runtime_target_registration(connection, table, "operation_id", operation_id)
}

fn load_runtime_target_registration_by_runtime(
    connection: &Connection,
    table: &str,
    runtime_id: &str,
) -> Result<Option<RuntimeTargetRegistrationRecord>, String> {
    load_runtime_target_registration(connection, table, "runtime_id", runtime_id)
}

fn load_runtime_target_registrations(
    connection: &Connection,
    table: &str,
    timestamp_column: &str,
) -> Result<Vec<RuntimeTargetRegistrationRecord>, String> {
    let expected_timestamp = runtime_target_registration_timestamp_column(table)?;
    if timestamp_column != expected_timestamp {
        return Err("runtime target registration timestamp column is invalid".into());
    }
    let query = format!(
        "SELECT operation_id, runtime_id, secret_key, payload, {timestamp_column}
         FROM {table} ORDER BY {timestamp_column}, runtime_id"
    );
    let mut statement = connection
        .prepare(&query)
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], map_runtime_target_registration)
        .map_err(|error| error.to_string())?;
    rows.map(|row| {
        row.map_err(|error| error.to_string())
            .and_then(validate_runtime_target_registration_record)
    })
    .collect()
}

fn require_runtime_target_registration_match(
    record: &RuntimeTargetRegistrationRecord,
    operation_id: &str,
    runtime_id: &str,
    secret_key: &str,
    payload: &[u8],
) -> Result<(), String> {
    if record.operation_id != operation_id
        || record.runtime_id != runtime_id
        || record.secret_key != secret_key
        || record.payload != payload
    {
        return Err("runtime target registration operation identity conflicts".into());
    }
    Ok(())
}

fn queue_runtime_target_secret_gc(
    connection: &Connection,
    secret_key: &str,
    queued_at_unix_ms: i64,
) -> Result<(), String> {
    validate_scheduler_id("runtime target secret key", secret_key)?;
    let live_references: i64 = connection
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM runtime_target_registration_intents
                  WHERE secret_key = ?1) +
                 (SELECT COUNT(*) FROM runtime_target_bindings
                  WHERE secret_key = ?1)",
            [secret_key],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if live_references != 0 {
        return Err("runtime target secret is still referenced".into());
    }
    connection
        .execute(
            "INSERT INTO runtime_target_secret_gc (secret_key, queued_at_unix_ms)
             VALUES (?1, ?2) ON CONFLICT(secret_key) DO NOTHING",
            params![secret_key, queued_at_unix_ms],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
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
        13 => migrate_schema_13_to_14(connection),
        14 => migrate_schema_14_to_15(connection),
        15 => migrate_schema_15_to_16(connection),
        16 => migrate_schema_16_to_17(connection),
        17 => migrate_schema_17_to_18(connection),
        18 => migrate_schema_18_to_19(connection),
        19 => migrate_schema_19_to_20(connection),
        20 => migrate_schema_20_to_21(connection),
        version => Err(format!(
            "no runtime journal migration from schema {version}"
        )),
    }
}

fn migrate_schema_20_to_21(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE runtime_target_registration_intents (
                 operation_id TEXT PRIMARY KEY CHECK (
                     length(operation_id) BETWEEN 1 AND 128
                 ),
                 runtime_id TEXT NOT NULL UNIQUE CHECK (
                     length(runtime_id) BETWEEN 1 AND 128
                 ),
                 secret_key TEXT NOT NULL CHECK (
                     length(secret_key) BETWEEN 1 AND 128
                 ),
                 payload BLOB NOT NULL CHECK (
                     length(payload) BETWEEN 1 AND 16384
                 ),
                 created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0)
             ) STRICT;
             CREATE UNIQUE INDEX runtime_target_registration_intents_secret_key
                 ON runtime_target_registration_intents (secret_key);
             CREATE TABLE runtime_target_bindings (
                 runtime_id TEXT PRIMARY KEY CHECK (
                     length(runtime_id) BETWEEN 1 AND 128
                 ),
                 operation_id TEXT NOT NULL UNIQUE CHECK (
                     length(operation_id) BETWEEN 1 AND 128
                 ),
                 secret_key TEXT NOT NULL CHECK (
                     length(secret_key) BETWEEN 1 AND 128
                 ),
                 payload BLOB NOT NULL CHECK (
                     length(payload) BETWEEN 1 AND 16384
                 ),
                 updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0)
             ) STRICT;
             CREATE UNIQUE INDEX runtime_target_bindings_secret_key
                 ON runtime_target_bindings (secret_key);
             CREATE TABLE runtime_target_secret_gc (
                 secret_key TEXT PRIMARY KEY CHECK (
                     length(secret_key) BETWEEN 1 AND 128
                 ),
                 queued_at_unix_ms INTEGER NOT NULL CHECK (queued_at_unix_ms >= 0)
             ) STRICT;",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (21, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 21 WHERE key = 'schema_version' AND value = 20",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(21)
}

fn migrate_schema_19_to_20(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "DROP INDEX authority_checkpoints_by_kind_phase;
             CREATE TABLE authority_checkpoints_v20 (
                 operation_kind TEXT NOT NULL CHECK (
                     operation_kind IN (
                         'daemon_bootstrap', 'daemon_retirement',
                         'gewyvern_provisioning', 'gewyvern_retirement'
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
                     (operation_kind = 'daemon_retirement' AND phase IN (
                         'planned', 'retiring_service', 'service_retired', 'failed'
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
             INSERT INTO authority_checkpoints_v20
                 (operation_kind, operation_id, revision, phase, checkpoint, updated_at_unix_ms)
             SELECT operation_kind, operation_id, revision, phase, checkpoint, updated_at_unix_ms
             FROM authority_checkpoints;
             DROP TABLE authority_checkpoints;
             ALTER TABLE authority_checkpoints_v20 RENAME TO authority_checkpoints;
             CREATE INDEX authority_checkpoints_by_kind_phase
                 ON authority_checkpoints
                    (operation_kind, phase, updated_at_unix_ms DESC);",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (20, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 20 WHERE key = 'schema_version' AND value = 19",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(20)
}

fn migrate_schema_18_to_19(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE authority_writer_fence (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 generation INTEGER NOT NULL CHECK (generation >= 1),
                 writer_id TEXT NOT NULL CHECK (
                     length(writer_id) = 32
                     AND writer_id NOT GLOB '*[^0-9A-Fa-f]*'
                 )
             ) STRICT;",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (19, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 19 WHERE key = 'schema_version' AND value = 18",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(19)
}

fn migrate_schema_17_to_18(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "ALTER TABLE orchestra_delete_replay_horizon
                 ADD COLUMN checkpointed_through_generation INTEGER
                 CHECK (checkpointed_through_generation >= 1);
             UPDATE orchestra_delete_replay_horizon
             SET checkpointed_through_generation = protected_from_generation;",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (18, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 18 WHERE key = 'schema_version' AND value = 17",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(18)
}

fn migrate_schema_16_to_17(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE orchestra_delete_replay_horizon (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 evicted_through_generation INTEGER NOT NULL
                     CHECK (evicted_through_generation >= 0),
                 protected_from_generation INTEGER
                     CHECK (protected_from_generation >= 1)
             ) STRICT;
             INSERT INTO orchestra_delete_replay_horizon
                 (id, evicted_through_generation, protected_from_generation)
             SELECT 1, 0, MIN(generation)
             FROM orchestra_delete_operations;",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (17, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 17 WHERE key = 'schema_version' AND value = 16",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(17)
}

fn migrate_schema_15_to_16(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE orchestra_delete_operations (
                 operation_id TEXT PRIMARY KEY,
                 generation INTEGER NOT NULL UNIQUE CHECK (generation >= 1),
                 request BLOB NOT NULL CHECK (length(request) <= 65536),
                 deleted_runtime_count INTEGER NOT NULL CHECK (deleted_runtime_count >= 0),
                 deleted_run_count INTEGER NOT NULL CHECK (deleted_run_count >= 0),
                 deleted_event_count INTEGER NOT NULL CHECK (deleted_event_count >= 0),
                 committed_at_unix_ms INTEGER NOT NULL CHECK (committed_at_unix_ms >= 0)
             ) STRICT;
             CREATE TABLE orchestra_delete_generation (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 next_generation INTEGER NOT NULL CHECK (next_generation >= 1)
             ) STRICT;
             INSERT INTO orchestra_delete_generation (id, next_generation) VALUES (1, 1);",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (16, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 16 WHERE key = 'schema_version' AND value = 15",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(16)
}

fn migrate_schema_14_to_15(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "ALTER TABLE runtime_unregistration_operations
                 RENAME TO runtime_unregistration_operations_v14;
             CREATE TABLE runtime_unregistration_operations (
                 operation_id TEXT PRIMARY KEY,
                 generation INTEGER NOT NULL CHECK (generation >= 1),
                 request BLOB NOT NULL CHECK (length(request) <= 65536),
                 deleted_runtime_count INTEGER NOT NULL CHECK (deleted_runtime_count >= 0),
                 deleted_run_count INTEGER NOT NULL CHECK (deleted_run_count >= 0),
                 deleted_event_count INTEGER NOT NULL CHECK (deleted_event_count >= 0),
                 removed_at_unix_ms INTEGER NOT NULL CHECK (removed_at_unix_ms >= 0)
             ) STRICT;
             INSERT INTO runtime_unregistration_operations
                 (operation_id, generation, request, deleted_runtime_count, deleted_run_count,
                  deleted_event_count, removed_at_unix_ms)
             SELECT operation_id, ROW_NUMBER() OVER (ORDER BY rowid ASC), request,
                    deleted_runtime_count, deleted_run_count, deleted_event_count,
                    removed_at_unix_ms
             FROM runtime_unregistration_operations_v14;
             DROP TABLE runtime_unregistration_operations_v14;
             CREATE UNIQUE INDEX runtime_unregistration_operations_by_generation
                 ON runtime_unregistration_operations (generation);
             CREATE TABLE runtime_unregistration_replay_horizon (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 next_generation INTEGER NOT NULL CHECK (next_generation >= 1),
                 evicted_through_generation INTEGER NOT NULL
                     CHECK (
                         evicted_through_generation >= 0
                         AND evicted_through_generation < next_generation
                     )
             ) STRICT;
             INSERT INTO runtime_unregistration_replay_horizon
                 (id, next_generation, evicted_through_generation)
             SELECT 1, COALESCE(MAX(generation), 0) + 1, 0
             FROM runtime_unregistration_operations;",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (15, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 15 WHERE key = 'schema_version' AND value = 14",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(15)
}

fn migrate_schema_13_to_14(connection: &mut Connection) -> Result<i64, String> {
    let applied_at = unix_time_ms()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch(
            "CREATE TABLE runtime_unregistration_operations (
                 operation_id TEXT PRIMARY KEY,
                 request BLOB NOT NULL CHECK (length(request) <= 65536),
                 deleted_runtime_count INTEGER NOT NULL CHECK (deleted_runtime_count >= 0),
                 deleted_run_count INTEGER NOT NULL CHECK (deleted_run_count >= 0),
                 deleted_event_count INTEGER NOT NULL CHECK (deleted_event_count >= 0),
                 removed_at_unix_ms INTEGER NOT NULL CHECK (removed_at_unix_ms >= 0)
             ) STRICT;",
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO runtime_schema_migrations (version, applied_at_unix_ms) VALUES (14, ?1)",
            [applied_at],
        )
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runtime_metadata SET value = 14 WHERE key = 'schema_version' AND value = 13",
            [],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("runtime journal schema changed during migration".into());
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(14)
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
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if (migration_count, first_migration, last_migration) != (21, 1, 21) {
        return Err("invalid runtime journal schema 21 migration history".into());
    }
    let timestamp_column: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runtime_journal') WHERE name = 'created_at_unix_ms'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if timestamp_column != 1 {
        return Err("invalid runtime journal schema 21 timestamp column".into());
    }
    connection
        .query_row("SELECT COUNT(*) FROM runtime_snapshots", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    connection
        .query_row("SELECT COUNT(*) FROM runtime_owner", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    connection
        .query_row("SELECT COUNT(*) FROM runtime_effect_tasks", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
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
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if effect_columns != 13 {
        return Err("invalid runtime journal schema 21 effect columns".into());
    }
    let claim_index: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'runtime_effect_claim'
               AND tbl_name = 'runtime_effect_tasks'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if claim_index != 1 {
        return Err("invalid runtime journal schema 21 effect claim index".into());
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
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if unknown_journal_kinds != 0 {
        return Err("invalid runtime journal schema 21 journal kind".into());
    }
    let log_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runtime_logs')
             WHERE name IN ('sequence', 'runtime_id', 'level', 'message', 'created_at_unix_ms')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if log_columns != 5 {
        return Err("invalid runtime journal schema 21 log columns".into());
    }
    let log_index: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'runtime_logs_by_runtime_sequence'
               AND tbl_name = 'runtime_logs'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if log_index != 1 {
        return Err("invalid runtime journal schema 21 log index".into());
    }
    let orchestra_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name IN ('orchestra_runs', 'orchestra_events')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if orchestra_tables != 2 {
        return Err("invalid runtime journal schema 21 Orchestra tables".into());
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
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if orchestra_indexes != 3 {
        return Err("invalid runtime journal schema 21 Orchestra indexes".into());
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
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if authority_columns != 6 {
        return Err("invalid runtime journal schema 21 authority checkpoint columns".into());
    }
    let authority_index: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'authority_checkpoints_by_kind_phase'
               AND tbl_name = 'authority_checkpoints'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if authority_index != 1 {
        return Err("invalid runtime journal schema 21 authority checkpoint index".into());
    }
    let daemon_retirement_constraint: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name = 'authority_checkpoints'
               AND sql LIKE '%daemon_retirement%'
               AND sql LIKE '%service_retired%'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if daemon_retirement_constraint != 1 {
        return Err(
            "invalid runtime journal schema 21 daemon retirement authority constraint".into(),
        );
    }
    let unregistration_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runtime_unregistration_operations')
             WHERE name IN (
                 'operation_id', 'generation', 'request', 'deleted_runtime_count', 'deleted_run_count',
                 'deleted_event_count', 'removed_at_unix_ms'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if unregistration_columns != 7 {
        return Err("invalid runtime journal schema 21 unregistration columns".into());
    }
    let unregistration_generation_index: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index'
               AND name = 'runtime_unregistration_operations_by_generation'
               AND tbl_name = 'runtime_unregistration_operations'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if unregistration_generation_index != 1 {
        return Err("invalid runtime journal schema 21 unregistration generation index".into());
    }
    load_runtime_unregistration_replay_horizon(connection)
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    let orchestra_delete_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('orchestra_delete_operations')
             WHERE name IN (
                 'operation_id', 'generation', 'request', 'deleted_runtime_count',
                 'deleted_run_count', 'deleted_event_count', 'committed_at_unix_ms'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if orchestra_delete_columns != 7 {
        return Err("invalid runtime journal schema 21 Orchestra delete columns".into());
    }
    let orchestra_delete_generation_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM orchestra_delete_generation
             WHERE id = 1 AND next_generation >= 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if orchestra_delete_generation_rows != 1 {
        return Err("invalid runtime journal schema 21 Orchestra delete generation".into());
    }
    load_orchestra_delete_replay_horizon(connection)
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    let authority_writer_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('authority_writer_fence')
             WHERE name IN ('id', 'generation', 'writer_id')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if authority_writer_columns != 3 {
        return Err("invalid runtime journal schema 21 authority writer fence".into());
    }
    let target_registration_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name IN (
                 'runtime_target_registration_intents',
                 'runtime_target_bindings',
                 'runtime_target_secret_gc'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if target_registration_tables != 3 {
        return Err("invalid runtime journal schema 21 target registration tables".into());
    }
    let intent_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runtime_target_registration_intents')
             WHERE name IN (
                 'operation_id', 'runtime_id', 'secret_key', 'payload', 'created_at_unix_ms'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    let binding_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runtime_target_bindings')
             WHERE name IN (
                 'runtime_id', 'operation_id', 'secret_key', 'payload', 'updated_at_unix_ms'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    let secret_gc_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runtime_target_secret_gc')
             WHERE name IN ('secret_key', 'queued_at_unix_ms')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if intent_columns != 5 || binding_columns != 5 || secret_gc_columns != 2 {
        return Err("invalid runtime journal schema 21 target registration columns".into());
    }
    let target_secret_indexes: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name IN (
                 'runtime_target_registration_intents_secret_key',
                 'runtime_target_bindings_secret_key'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if target_secret_indexes != 2 {
        return Err("invalid runtime journal schema 21 target secret indexes".into());
    }
    let target_secret_conflicts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (
                 SELECT secret_key FROM (
                     SELECT secret_key FROM runtime_target_registration_intents
                     UNION ALL
                     SELECT secret_key FROM runtime_target_bindings
                     UNION ALL
                     SELECT secret_key FROM runtime_target_secret_gc
                 ) GROUP BY secret_key HAVING COUNT(*) > 1
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 21: {error}"))?;
    if target_secret_conflicts != 0 {
        return Err("invalid runtime journal schema 21 target secret ownership".into());
    }
    Ok(())
}

fn validate_authority_writer_id(writer_id: &str) -> Result<(), String> {
    (writer_id.len() == 32 && writer_id.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then_some(())
        .ok_or_else(|| "authority writer ID must be 32 ASCII hexadecimal characters".to_string())
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

fn orchestra_cancel_marker_id(run_id: &str) -> Result<String, String> {
    let marker_id = format!("{run_id}.cancel");
    validate_scheduler_id("Orchestra cancellation marker ID", &marker_id)?;
    Ok(marker_id)
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

fn validate_control_plane_import(
    orchestra_runs: &[OrchestraImportRecord],
    protected_binding_runtime_ids: &[String],
) -> Result<(), String> {
    if orchestra_runs.len() > usize::try_from(MAX_RUNTIME_TARGET_BINDINGS).unwrap_or(usize::MAX) {
        return Err("control-plane import exceeds the Orchestra retention bound".into());
    }
    if protected_binding_runtime_ids
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err("control-plane import binding identities are not canonical".into());
    }
    for runtime_id in protected_binding_runtime_ids {
        RuntimeId::new(runtime_id.clone())
            .map_err(|_| "control-plane import binding identity is invalid".to_string())?;
    }

    let mut run_ids = HashSet::with_capacity(orchestra_runs.len());
    let mut folded_run_ids = HashSet::with_capacity(orchestra_runs.len());
    let mut runtime_counts = BTreeMap::<&str, usize>::new();
    let mut request_ids = HashSet::<(&str, &str)>::new();
    let future_limit = i128::from(unix_time_ms()?)
        .checked_mul(1_000_000)
        .and_then(|value| value.checked_add(5 * 60 * 1_000_000_000))
        .ok_or_else(|| "control-plane import timestamp bound is invalid".to_string())?;
    for imported in orchestra_runs {
        if !run_ids.insert(imported.run_id.as_str())
            || !folded_run_ids.insert(imported.run_id.to_ascii_lowercase())
        {
            return Err("control-plane import contains duplicate Orchestra run identities".into());
        }
        let runtime_count = runtime_counts
            .entry(imported.runtime_id.as_str())
            .or_default();
        *runtime_count = runtime_count
            .checked_add(1)
            .ok_or_else(|| "control-plane import Orchestra count overflow".to_string())?;
        if *runtime_count > MAX_ORCHESTRA_RUNS_PER_RUNTIME {
            return Err("control-plane import exceeds per-runtime Orchestra retention".into());
        }
        if let Some(request_id) = imported.request_id.as_deref()
            && !request_ids.insert((imported.runtime_id.as_str(), request_id))
        {
            return Err(
                "control-plane import contains duplicate Orchestra request identities".into(),
            );
        }
        if imported.event_type != "legacy_import" || is_active_orchestra_outcome(&imported.outcome)
        {
            return Err("control-plane import contains a non-terminal Orchestra run".into());
        }
        validate_orchestra_append_envelopes(
            &imported.run_id,
            &imported.runtime_id,
            imported.request_id.as_deref(),
            &imported.event_type,
            None,
            &imported.outcome,
            &imported.outcome,
            &imported.recorded_at,
            &imported.run,
            &imported.event,
        )?;
        let run =
            validate_orchestra_run_row(&imported.run_id, &imported.runtime_id, &imported.run)?;
        let recorded_at = validate_orchestra_recorded_at(&imported.recorded_at)?;
        if run.executed_at.unix_timestamp_nanos() > future_limit
            || run
                .completed_at
                .is_some_and(|timestamp| timestamp.unix_timestamp_nanos() > future_limit)
            || recorded_at.unix_timestamp_nanos() > future_limit
        {
            return Err("control-plane import contains a future Orchestra timestamp".into());
        }
    }
    Ok(())
}

fn load_validated_orchestra_event_batches(
    transaction: &Transaction<'_>,
    run_ids: &[&str],
) -> Result<BTreeMap<String, Vec<ValidatedOrchestraEvent>>, String> {
    if run_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut batches = run_ids
        .iter()
        .map(|run_id| ((*run_id).to_string(), Vec::new()))
        .collect::<BTreeMap<_, _>>();
    if batches.len() != run_ids.len() {
        return Err("Orchestra history contains duplicate run identities".into());
    }
    let expected_rows = run_ids
        .len()
        .checked_mul(MAX_ORCHESTRA_EVENTS_PER_RUN)
        .ok_or_else(|| "Orchestra event batch bound is invalid".to_string())?;
    let fetch_rows = expected_rows
        .checked_add(1)
        .ok_or_else(|| "Orchestra event batch bound is invalid".to_string())?;
    let placeholders = (0..run_ids.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "SELECT event_id, run_id, runtime_id, event_type, to_outcome,
                recorded_at, envelope, created_at_unix_ms
         FROM orchestra_events
         WHERE run_id IN ({placeholders})
         ORDER BY run_id ASC, event_id ASC
         LIMIT {fetch_rows}"
    );
    let mut statement = transaction
        .prepare(&query)
        .map_err(|error| error.to_string())?;
    let raw_rows = statement
        .query_map(params_from_iter(run_ids.iter().copied()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Vec<u8>>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    if raw_rows.len() > expected_rows {
        return Err("Orchestra event batch exceeds its state-machine bound".into());
    }
    for (
        event_id,
        event_run_id,
        event_runtime_id,
        event_type,
        to_outcome,
        recorded_at,
        envelope,
        generation,
    ) in raw_rows
    {
        let event = validate_orchestra_event_row(
            event_id,
            &event_run_id,
            &event_runtime_id,
            &event_type,
            &to_outcome,
            &recorded_at,
            envelope,
            generation,
        )?;
        let events = batches
            .get_mut(&event.run_id)
            .ok_or_else(|| "Orchestra event history identity is inconsistent".to_string())?;
        if events.len() >= MAX_ORCHESTRA_EVENTS_PER_RUN {
            return Err("Orchestra event history exceeds its state-machine bound".into());
        }
        events.push(event);
    }
    Ok(batches)
}

fn next_orchestra_generation(
    transaction: &Transaction<'_>,
    runtime_id: &str,
    wall_clock_now: i64,
) -> Result<i64, String> {
    let latest_generation: Option<i64> = transaction
        .query_row(
            "SELECT MAX(updated_at_unix_ms) FROM orchestra_runs WHERE runtime_id = ?1",
            [runtime_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    match latest_generation {
        Some(latest_generation) => latest_generation
            .checked_add(1)
            .map(|generation| generation.max(wall_clock_now))
            .ok_or_else(|| "Orchestra transaction generation overflow".to_string()),
        None => Ok(wall_clock_now),
    }
}

fn plan_orchestra_retention(
    transaction: &Transaction<'_>,
    runtime_id: &str,
    current_run_id: &str,
) -> Result<OrchestraRetentionPlan, String> {
    let fetch_limit = i64::try_from(MAX_ORCHESTRA_RUNS_PER_RUNTIME + 2)
        .map_err(|_| "Orchestra retained run bound is invalid".to_string())?;
    let mut statement = transaction
        .prepare(
            "SELECT run_id FROM orchestra_runs
             WHERE runtime_id = ?1
             ORDER BY updated_at_unix_ms DESC, run_id ASC LIMIT ?2",
        )
        .map_err(|error| error.to_string())?;
    let mut run_ids = statement
        .query_map(params![runtime_id, fetch_limit], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    if run_ids.len() > MAX_ORCHESTRA_RUNS_PER_RUNTIME + 1 {
        return Err("Orchestra retained run set exceeds its bounded append delta".into());
    }
    let evicted_run_ids = if run_ids.len() > MAX_ORCHESTRA_RUNS_PER_RUNTIME {
        run_ids.split_off(MAX_ORCHESTRA_RUNS_PER_RUNTIME)
    } else {
        Vec::new()
    };
    if !run_ids.iter().any(|run_id| run_id == current_run_id) {
        return Err("Orchestra current run was excluded from its retention window".into());
    }
    Ok(OrchestraRetentionPlan {
        retained_run_ids: run_ids,
        evicted_run_ids,
    })
}

#[allow(clippy::too_many_arguments)]
fn load_validated_orchestra_persistence_record(
    transaction: &Transaction<'_>,
    run_id: &str,
    runtime_id: &str,
    request_id: Option<&str>,
    event_type: &str,
    to_outcome: &str,
    recorded_at: &str,
    expected_retained_run_ids: &[String],
    evicted_run_ids: &[String],
) -> Result<OrchestraPersistenceRecord, String> {
    if expected_retained_run_ids.is_empty()
        || expected_retained_run_ids.len() > MAX_ORCHESTRA_RUNS_PER_RUNTIME
        || !expected_retained_run_ids
            .iter()
            .any(|retained_run_id| retained_run_id == run_id)
        || evicted_run_ids.len() > 1
    {
        return Err("Orchestra retention plan is inconsistent".into());
    }
    let fetch_limit = i64::try_from(MAX_ORCHESTRA_RUNS_PER_RUNTIME + 1)
        .map_err(|_| "Orchestra retained run bound is invalid".to_string())?;
    let mut statement = transaction
        .prepare(
            "SELECT run_id, runtime_id, request_id, envelope, updated_at_unix_ms
             FROM orchestra_runs WHERE runtime_id = ?1
             ORDER BY updated_at_unix_ms DESC, run_id ASC LIMIT ?2",
        )
        .map_err(|error| error.to_string())?;
    let retained_rows = statement
        .query_map(params![runtime_id, fetch_limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let retained_rows = retained_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    if retained_rows.len() != expected_retained_run_ids.len()
        || retained_rows
            .iter()
            .map(|(run_id, _, _, _, _)| run_id)
            .ne(expected_retained_run_ids.iter())
    {
        return Err("Orchestra post-append retention window is inconsistent".into());
    }

    let mut validated_runs = BTreeMap::new();
    for (stored_run_id, stored_runtime_id, stored_request_id, run_envelope, run_generation) in
        retained_rows
    {
        if stored_runtime_id != runtime_id || run_generation < 0 {
            return Err("Orchestra post-append retained run is inconsistent".into());
        }
        let run = validate_retained_orchestra_run_row(
            &stored_run_id,
            &stored_runtime_id,
            stored_request_id.as_deref(),
            &run_envelope,
        )?;
        if validated_runs
            .insert(stored_run_id, (run, run_envelope, run_generation))
            .is_some()
        {
            return Err("Orchestra post-append retention identities are inconsistent".into());
        }
    }
    let run_id_refs = expected_retained_run_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut event_batches = load_validated_orchestra_event_batches(transaction, &run_id_refs)?;
    let mut validated_event_count = 0_usize;
    let mut receipt = None;
    for retained_run_id in expected_retained_run_ids {
        let (run, run_envelope, run_generation) = validated_runs
            .get(retained_run_id)
            .ok_or_else(|| "Orchestra post-append retained run is missing".to_string())?;
        let events = event_batches
            .remove(retained_run_id)
            .ok_or_else(|| "Orchestra post-append event snapshot is inconsistent".to_string())?;
        validate_orchestra_event_history(run, &events, runtime_id, retained_run_id)?;
        validated_event_count = validated_event_count
            .checked_add(events.len())
            .ok_or_else(|| "Orchestra retained event count overflow".to_string())?;
        if retained_run_id == run_id {
            if run.request_id.as_deref() != request_id || run.outcome != to_outcome {
                return Err("Orchestra post-append run snapshot is inconsistent".into());
            }
            let target_event = events
                .iter()
                .find(|event| {
                    event.event_type == event_type
                        && event.to_outcome == to_outcome
                        && event.recorded_at_text == recorded_at
                })
                .ok_or_else(|| {
                    "Orchestra post-append event snapshot is inconsistent".to_string()
                })?;
            if target_event.generation != *run_generation {
                return Err("Orchestra post-append transaction generation is inconsistent".into());
            }
            receipt = Some(OrchestraPersistenceRecord {
                run: run_envelope.clone(),
                event: target_event.envelope.clone(),
                event_count: u64::try_from(events.len())
                    .map_err(|_| "invalid Orchestra event count".to_string())?,
            });
        }
    }
    if !event_batches.is_empty() {
        return Err("Orchestra post-append event snapshot is inconsistent".into());
    }
    let stored_event_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM orchestra_events WHERE runtime_id = ?1",
            [runtime_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if usize::try_from(stored_event_count).ok() != Some(validated_event_count) {
        return Err("Orchestra post-append runtime event set is inconsistent".into());
    }
    for evicted_run_id in evicted_run_ids {
        let (run_count, event_count): (i64, i64) = transaction
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM orchestra_runs WHERE run_id = ?1),
                     (SELECT COUNT(*) FROM orchestra_events WHERE run_id = ?1)",
                [evicted_run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| error.to_string())?;
        if run_count != 0 || event_count != 0 {
            return Err("Orchestra post-append eviction cascade is inconsistent".into());
        }
    }
    receipt.ok_or_else(|| "Orchestra post-append persistence receipt is missing".to_string())
}

fn validate_orchestra_run_row(
    run_id: &str,
    runtime_id: &str,
    envelope: &[u8],
) -> Result<ValidatedOrchestraRun, String> {
    validate_scheduler_id("run_id", run_id)?;
    validate_scheduler_id("runtime_id", runtime_id)?;
    validate_orchestra_blob("run envelope", envelope)?;
    let decoded: StoredOrchestraRunEnvelope = serde_json::from_slice(envelope)
        .map_err(|_| "Orchestra run envelope is invalid".to_string())?;
    validate_scheduler_id("run plan_id", &decoded.plan_id)?;
    validate_orchestra_outcome("run outcome", &decoded.outcome)?;
    if let Some(request_id) = &decoded.request_id {
        validate_scheduler_id("run request_id", request_id)?;
    }
    if decoded.run_id != run_id || decoded.runtime_id != runtime_id {
        return Err("Orchestra run row does not match its envelope".into());
    }
    let executed_at = validate_orchestra_timestamp("run executed_at", &decoded.executed_at)?;
    let completed_at = decoded
        .completed_at
        .as_deref()
        .map(|value| validate_orchestra_timestamp("run completed_at", value))
        .transpose()?;
    if completed_at.is_some_and(|completed_at| completed_at < executed_at)
        || is_active_orchestra_outcome(&decoded.outcome) && completed_at.is_some()
    {
        return Err("Orchestra run timestamps are inconsistent".into());
    }
    Ok(ValidatedOrchestraRun {
        outcome: decoded.outcome,
        executed_at,
        completed_at,
        request_id: decoded.request_id,
    })
}

fn validate_retained_orchestra_run_row(
    run_id: &str,
    runtime_id: &str,
    request_id: Option<&str>,
    envelope: &[u8],
) -> Result<ValidatedOrchestraRun, String> {
    let run = validate_orchestra_run_row(run_id, runtime_id, envelope)?;
    if run.request_id.as_deref() != request_id {
        return Err("Orchestra retained run request identity is inconsistent".into());
    }
    Ok(run)
}

#[allow(clippy::too_many_arguments)]
fn validate_orchestra_append_envelopes(
    run_id: &str,
    runtime_id: &str,
    request_id: Option<&str>,
    event_type: &str,
    from_outcome: Option<&str>,
    to_outcome: &str,
    run_outcome: &str,
    recorded_at: &str,
    run_envelope: &[u8],
    event_envelope: &[u8],
) -> Result<(), String> {
    let run = validate_orchestra_run_row(run_id, runtime_id, run_envelope)?;
    if run.outcome != run_outcome || run.request_id.as_deref() != request_id {
        return Err("Orchestra run append fields do not match its envelope".into());
    }
    let event = validate_orchestra_event_row(
        1,
        run_id,
        runtime_id,
        event_type,
        to_outcome,
        recorded_at,
        event_envelope.to_vec(),
        0,
    )?;
    if event.envelope_event_id != 0 || event.from_outcome.as_deref() != from_outcome {
        return Err("Orchestra event append fields do not match its envelope".into());
    }
    if event.recorded_at < run.executed_at
        || run
            .completed_at
            .is_some_and(|completed_at| event.recorded_at < completed_at)
    {
        return Err("Orchestra append timestamps are inconsistent".into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_orchestra_event_row(
    event_id: i64,
    run_id: &str,
    runtime_id: &str,
    event_type: &str,
    to_outcome: &str,
    recorded_at: &str,
    envelope: Vec<u8>,
    generation: i64,
) -> Result<ValidatedOrchestraEvent, String> {
    let event_id = u64::try_from(event_id)
        .ok()
        .filter(|event_id| *event_id > 0)
        .ok_or_else(|| "Orchestra event ID is invalid".to_string())?;
    validate_scheduler_id("event run_id", run_id)?;
    validate_scheduler_id("event runtime_id", runtime_id)?;
    validate_scheduler_id("event_type", event_type)?;
    validate_orchestra_outcome("event to_outcome", to_outcome)?;
    if generation < 0 {
        return Err("Orchestra event generation is invalid".into());
    }
    let recorded_at_instant = validate_orchestra_recorded_at(recorded_at)?;
    validate_orchestra_blob("event envelope", &envelope)?;
    let decoded: StoredOrchestraEventEnvelope = serde_json::from_slice(&envelope)
        .map_err(|_| "Orchestra event envelope is invalid".to_string())?;
    if decoded.event_id != 0 && decoded.event_id != event_id
        || decoded.run_id != run_id
        || decoded.runtime_id != runtime_id
        || decoded.event_type != event_type
        || decoded.to_outcome != to_outcome
        || decoded.recorded_at != recorded_at
    {
        return Err("Orchestra event row does not match its envelope".into());
    }
    if decoded
        .from_outcome
        .as_deref()
        .is_some_and(|outcome| !is_known_orchestra_outcome(outcome))
        || decoded.summary.len() > 1_024
        || decoded.summary != decoded.summary.trim()
        || decoded.summary.chars().any(char::is_control)
    {
        return Err("Orchestra event envelope metadata is invalid".into());
    }
    Ok(ValidatedOrchestraEvent {
        event_id,
        envelope_event_id: decoded.event_id,
        run_id: decoded.run_id,
        runtime_id: decoded.runtime_id,
        event_type: decoded.event_type,
        envelope,
        from_outcome: decoded.from_outcome,
        to_outcome: decoded.to_outcome,
        recorded_at_text: decoded.recorded_at,
        recorded_at: recorded_at_instant,
        generation,
    })
}

fn validate_orchestra_event_history(
    run: &ValidatedOrchestraRun,
    events: &[ValidatedOrchestraEvent],
    runtime_id: &str,
    run_id: &str,
) -> Result<(), String> {
    if events.is_empty() {
        return Err("Orchestra run is missing its event history".into());
    }
    let mut previous: Option<&ValidatedOrchestraEvent> = None;
    for event in events {
        if event.runtime_id != runtime_id || event.run_id != run_id {
            return Err("Orchestra event history identity is inconsistent".into());
        }
        match previous {
            None if event.from_outcome.is_some() => {
                return Err("Orchestra origin event has a source outcome".into());
            }
            Some(previous)
                if event.event_id <= previous.event_id
                    || event.recorded_at < previous.recorded_at
                    || event.from_outcome.as_deref() != Some(previous.to_outcome.as_str())
                    || !is_valid_orchestra_transition(&previous.to_outcome, &event.to_outcome) =>
            {
                return Err("Orchestra event history sequence is invalid".into());
            }
            _ => {}
        }
        previous = Some(event);
    }
    let Some(first) = events.first() else {
        return Err("Orchestra run is missing its event history".into());
    };
    let Some(last) = events.last() else {
        return Err("Orchestra run is missing its event history".into());
    };
    if first.recorded_at < run.executed_at
        || last.to_outcome != run.outcome
        || run
            .completed_at
            .is_some_and(|completed_at| last.recorded_at < completed_at)
    {
        return Err("Orchestra event history does not match its run".into());
    }
    Ok(())
}

fn validate_orchestra_outcome(label: &str, value: &str) -> Result<(), String> {
    is_known_orchestra_outcome(value)
        .then_some(())
        .ok_or_else(|| format!("invalid Orchestra {label}"))
}

fn validate_orchestra_recorded_at(value: &str) -> Result<OffsetDateTime, String> {
    validate_orchestra_timestamp("recorded_at", value)
}

fn validate_orchestra_timestamp(label: &str, value: &str) -> Result<OffsetDateTime, String> {
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return Err(format!("Orchestra {label} is invalid"));
    }
    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| format!("Orchestra {label} is invalid"))
}

fn is_known_orchestra_outcome(value: &str) -> bool {
    matches!(
        value,
        "queued" | "running" | "succeeded" | "degraded" | "failed" | "cancelled" | "ok"
    )
}

fn is_active_orchestra_outcome(value: &str) -> bool {
    matches!(value, "queued" | "running")
}

fn is_valid_orchestra_transition(current: &str, next: &str) -> bool {
    match current {
        "queued" => matches!(next, "running" | "cancelled" | "failed"),
        "running" => matches!(
            next,
            "succeeded" | "degraded" | "failed" | "cancelled" | "ok"
        ),
        _ => false,
    }
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
