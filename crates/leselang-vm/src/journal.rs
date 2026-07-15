use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::limits::Limit;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::{
    ContinuationImage, ContinuationToken, Fault, MAX_CONTINUATION_BYTES, Step, Value,
    encode_json_capped, validate_image,
};

pub const JOURNAL_SCHEMA_VERSION: u32 = 1;
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
    Ephemeral,
    Sqlite(SqliteJournal),
}

pub(crate) struct SqliteJournal {
    connection: Connection,
}

struct JournalRecord {
    state: String,
    image: Vec<u8>,
    terminal_step: Option<Vec<u8>>,
}

impl Journal {
    pub fn ephemeral() -> Self {
        Self::Ephemeral
    }

    pub fn open(path: &Path) -> Result<(Self, JournalSnapshot), Fault> {
        let journal = SqliteJournal::open(path)?;
        let snapshot = journal.load()?;
        Ok((Self::Sqlite(journal), snapshot))
    }

    pub fn allocate_sequence(&mut self, local_next: u64) -> Result<u64, Fault> {
        match self {
            Self::Ephemeral => Ok(local_next),
            Self::Sqlite(journal) => journal.allocate_sequence(local_next),
        }
    }

    pub fn record_pending(&mut self, image: &ContinuationImage) -> Result<(), Fault> {
        match self {
            Self::Ephemeral => Ok(()),
            Self::Sqlite(journal) => journal.record_pending(image),
        }
    }

    pub fn record_completed(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
    ) -> Result<Step, Fault> {
        match self {
            Self::Ephemeral => Ok(step.clone()),
            Self::Sqlite(journal) => journal.record_completed(image, step),
        }
    }
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
                       terminal_step BLOB,
                       CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                              (state = 'completed' AND terminal_step IS NOT NULL))
                     ) STRICT;
                     PRAGMA user_version = 1;
                     COMMIT;",
                )
                .map_err(|error| journal_error("LSV4003", "failed to initialize journal", error))?;
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
            .prepare("SELECT token, state, image, terminal_step FROM vm_effects ORDER BY token ASC")
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

            match state.as_str() {
                "pending" => {
                    pending.insert(image.token.clone(), image);
                }
                "completed" => {
                    let step_bytes: Vec<u8> = row.get(3).map_err(|error| {
                        journal_error("LSV4005", "invalid completed journal record", error)
                    })?;
                    let step: Step = decode_bounded(&step_bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                    validate_terminal_step(&step)?;
                    completed.insert(image.token, step);
                }
                _ => return Err(journal_fault("LSV4007", "invalid journal record state")),
            }
        }

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

    fn record_pending(&mut self, image: &ContinuationImage) -> Result<(), Fault> {
        validate_image(image)?;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4010", "failed to lock journal", error))?;
        let existing = load_record(&transaction, image.token.as_str())?;
        if let Some(existing) = existing {
            if existing.state == "pending" && existing.image == image_bytes {
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
        ensure_growth(&transaction, image_bytes.len())?;
        transaction
            .execute(
                "INSERT INTO vm_effects(token, state, image, terminal_step)
                 VALUES (?1, 'pending', ?2, NULL)",
                params![image.token.as_str(), image_bytes],
            )
            .map_err(|error| journal_error("LSV4010", "failed to store pending effect", error))?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4010", "failed to commit pending effect", error))
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
        ensure_growth(&transaction, step_bytes.len())?;
        transaction
            .execute(
                "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
                params![image.token.as_str(), step_bytes],
            )
            .map_err(|error| journal_error("LSV4012", "failed to complete effect", error))?;
        transaction.commit().map_err(|error| {
            journal_error("LSV4012", "failed to commit completed effect", error)
        })?;
        Ok(step.clone())
    }
}

fn journal_payload_bytes(connection: &Connection) -> Result<usize, Fault> {
    let bytes: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(length(image) + COALESCE(length(terminal_step), 0)), 0)
             FROM vm_effects",
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
        Step::Fault(fault) if !fault.code.is_empty() && fault.code.len() <= 32 => Ok(()),
        Step::Done(_) => Err(journal_fault(
            "LSV4007",
            "completed journal output exceeds runtime limit",
        )),
        Step::Effect(_) | Step::Yield(_) => Err(journal_fault(
            "LSV4007",
            "journal completion must be a terminal step",
        )),
        Step::Fault(_) => Err(journal_fault(
            "LSV4007",
            "journal fault has an invalid diagnostic code",
        )),
    }
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
