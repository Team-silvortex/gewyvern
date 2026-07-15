use std::fs::{self, OpenOptions};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};

use crate::{EFFECT_QUEUE_CAPACITY, EffectQueueStats};

const RUNTIME_JOURNAL_SCHEMA_VERSION: i64 = 6;
const MAX_JOURNAL_RECORDS: i64 = 100_000;
const MAX_JOURNAL_PAYLOAD_BYTES: usize = 64 * 1024;
const MAX_SNAPSHOT_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
const MAX_COMPACTION_RECORDS: i64 = 1_000;
const OWNER_LEASE_DURATION_MS: i64 = 30_000;
const MAX_EFFECT_TASKS: i64 = EFFECT_QUEUE_CAPACITY as i64;
const MAX_EFFECT_LEASE_MS: i64 = 5 * 60 * 1_000;
static OWNER_TOKEN_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalEntryKind {
    RuntimeRegistration,
    CommandPlan,
    RuntimeStatusObservation,
}

impl JournalEntryKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeRegistration => "runtime_registration",
            Self::CommandPlan => "command_plan",
            Self::RuntimeStatusObservation => "runtime_status_observation",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "runtime_registration" => Ok(Self::RuntimeRegistration),
            "command_plan" => Ok(Self::CommandPlan),
            "runtime_status_observation" => Ok(Self::RuntimeStatusObservation),
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

    pub fn append(&mut self, kind: JournalEntryKind, payload: &[u8]) -> Result<i64, String> {
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
        self.connection
            .execute(
                "INSERT INTO runtime_journal (kind, payload, created_at_unix_ms) VALUES (?1, ?2, ?3)",
                params![kind.as_str(), payload, unix_time_ms()?],
            )
            .map_err(|error| error.to_string())?;
        Ok(self.connection.last_insert_rowid())
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
                "SELECT sequence, kind, payload, outcome, terminal_error
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
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut entries = Vec::new();
        for row in rows {
            let (sequence, kind, payload, outcome, terminal_error) =
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
            });
        }
        Ok(entries)
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
        self.ensure_owner()?;
        validate_scheduler_id("effect_id", effect_id)?;
        validate_scheduler_id("effect kind", kind)?;
        validate_blob("effect payload", payload)?;
        if max_attempts == 0 || max_attempts > 100 {
            return Err("effect max_attempts must be between 1 and 100".into());
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
        if active >= MAX_EFFECT_TASKS {
            return Err(format!(
                "runtime effect queue limit {MAX_EFFECT_TASKS} reached"
            ));
        }
        let changed = transaction
            .execute(
                "INSERT INTO runtime_effect_tasks
                     (effect_id, kind, payload, state, attempt, max_attempts,
                      available_at_unix_ms, created_at_unix_ms, updated_at_unix_ms)
                 VALUES (?1, ?2, ?3, 'ready', 0, ?4, ?5, ?5, ?5)
                 ON CONFLICT(effect_id) DO NOTHING",
                params![effect_id, kind, payload, i64::from(max_attempts), now],
            )
            .map_err(|error| error.to_string())?;
        if changed == 0 {
            let matches: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM runtime_effect_tasks
                     WHERE effect_id = ?1 AND kind = ?2 AND payload = ?3 AND max_attempts = ?4",
                    params![effect_id, kind, payload, i64::from(max_attempts)],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            if matches != 1 {
                return Err(format!(
                    "effect id '{effect_id}' was reused with different input"
                ));
            }
        }
        transaction.commit().map_err(|error| error.to_string())
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

    pub fn claim_effect(
        &mut self,
        worker_id: &str,
        lease_duration: Duration,
    ) -> Result<Option<EffectLease>, String> {
        self.ensure_owner()?;
        validate_scheduler_id("worker_id", worker_id)?;
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
        let candidate: Option<(String, String, Vec<u8>, i64)> = transaction
            .query_row(
                "SELECT effect_id, kind, payload, attempt FROM runtime_effect_tasks
                 WHERE attempt < max_attempts AND (
                     (state = 'ready' AND available_at_unix_ms <= ?1) OR
                     (state = 'leased' AND lease_expires_at_unix_ms <= ?1)
                 )
                 ORDER BY available_at_unix_ms, created_at_unix_ms, effect_id LIMIT 1",
                [now],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
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

    pub fn complete_effect_with_journal(
        &mut self,
        lease: &EffectLease,
        kind: JournalEntryKind,
        payload: &[u8],
        journal_outcome: &[u8],
        effect_outcome: &[u8],
    ) -> Result<(), String> {
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
        transaction.commit().map_err(|error| error.to_string())
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

fn migrate_schema(connection: &mut Connection, from: i64) -> Result<i64, String> {
    match from {
        1 => migrate_schema_1_to_2(connection),
        2 => migrate_schema_2_to_3(connection),
        3 => migrate_schema_3_to_4(connection),
        4 => migrate_schema_4_to_5(connection),
        5 => migrate_schema_5_to_6(connection),
        version => Err(format!(
            "no runtime journal migration from schema {version}"
        )),
    }
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
    let migration_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM runtime_schema_migrations WHERE version IN (1, 2, 3, 4, 5, 6)",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("invalid runtime journal schema 6: {error}"))?;
    if migration_count != 6 {
        return Err("invalid runtime journal schema 6 migration history".into());
    }
    let timestamp_column: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runtime_journal') WHERE name = 'created_at_unix_ms'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if timestamp_column != 1 {
        return Err("invalid runtime journal schema 6 timestamp column".into());
    }
    connection
        .query_row("SELECT COUNT(*) FROM runtime_snapshots", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("invalid runtime journal schema 6: {error}"))?;
    connection
        .query_row("SELECT COUNT(*) FROM runtime_owner", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("invalid runtime journal schema 6: {error}"))?;
    connection
        .query_row("SELECT COUNT(*) FROM runtime_effect_tasks", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("invalid runtime journal schema 6: {error}"))?;
    Ok(())
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

fn unix_time_ms() -> Result<i64, String> {
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

fn validate_snapshot_blob(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_SNAPSHOT_PAYLOAD_BYTES {
        return Err(format!(
            "runtime snapshot exceeds {MAX_SNAPSHOT_PAYLOAD_BYTES} bytes"
        ));
    }
    Ok(())
}

fn snapshot_checksum(schema_version: u32, through_sequence: i64, bytes: &[u8]) -> String {
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
