use super::*;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "leselang-schema-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy)]
enum Failure {
    Before,
    During,
    After,
}

struct Owner<'a> {
    connection: &'a Connection,
    calls: Cell<usize>,
    batches: RefCell<Vec<String>>,
    failure: Option<(usize, Failure)>,
}
impl<'a> Owner<'a> {
    fn new(connection: &'a Connection) -> Self {
        Self {
            connection,
            calls: Cell::new(0),
            batches: RefCell::new(Vec::new()),
            failure: None,
        }
    }
    fn execute_batch(&self, sql: &str) -> rusqlite::Result<()> {
        self.batches.borrow_mut().push(sql.to_owned());
        let index = self.calls.get();
        self.calls.set(index + 1);
        if let Some((at, phase)) = self.failure
            && index == at
        {
            let fail = "SELECT * FROM injected_missing_schema_table;";
            return match phase {
                Failure::Before => self.connection.execute_batch(fail),
                Failure::During if sql.contains("COMMIT;") => self
                    .connection
                    .execute_batch(&sql.replacen("COMMIT;", &format!("{fail} COMMIT;"), 1)),
                Failure::During => self.connection.execute_batch(fail),
                Failure::After => {
                    self.connection.execute_batch(sql)?;
                    self.connection.execute_batch(fail)
                }
            };
        }
        self.connection.execute_batch(sql)
    }
}

fn migrated(connection: &Owner<'_>, version: u32) -> Result<(), Fault> {
    leselang_command::reference_journal_schema::migrate(version, |sql, context| {
        connection
            .execute_batch(sql)
            .map_err(|error| journal_error("LSV4003", context, error))
    })
}

fn version(connection: &Connection) -> u32 {
    connection
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap()
}

type SchemaRow = (String, String, String, Option<String>);
fn schema(connection: &Connection) -> Vec<SchemaRow> {
    connection
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY type, name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}
fn compare_connections(actual: &Connection, expected: &Connection) {
    assert_eq!(version(actual), version(expected));
    assert_eq!(actual.is_autocommit(), expected.is_autocommit());
    assert_eq!(schema(actual), schema(expected));
    for (_, name, _, _) in schema(actual)
        .into_iter()
        .filter(|r| r.0 == "table" && r.1.starts_with("vm_"))
    {
        let rows = |connection: &Connection| {
            let mut query = connection
                .prepare(&format!("SELECT * FROM {name} ORDER BY rowid"))
                .unwrap();
            let count = query.column_count();
            query
                .query_map([], |row| {
                    (0..count)
                        .map(|i| row.get::<_, rusqlite::types::Value>(i))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        assert_eq!(rows(actual), rows(expected), "{name}");
    }
}

fn reply(request: &EffectRequest) -> crate::EffectResult {
    let crate::EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    crate::EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}

fn seed(path: &Path, target: u32) {
    let connection = Connection::open(path).unwrap();
    if target == 0 {
        return;
    }
    frozen_schema(&Owner::new(&connection), 0).unwrap();
    let mut vm = crate::Vm::default();
    for state in ["pending", "completed"] {
        let program = leselang_hir::lower(&leselang_syntax::parse(
            r#"fn main() = ui.focus(node_id: "target")"#,
        ))
        .unwrap();
        let Step::Effect(request) = vm.start(
            &program,
            leselang_host_contract::Principal::new("operator").unwrap(),
            leselang_host_contract::CapabilitySet::new(["ui.presentation"]),
            None,
        ) else {
            panic!()
        };
        let terminal = (state == "completed").then(|| {
            serde_json::to_vec(&vm.resume(&request.continuation, reply(&request))).unwrap()
        });
        connection
            .execute(
                "INSERT INTO vm_effects(token,state,image,terminal_step) VALUES (?1,?2,?3,?4)",
                params![
                    request.continuation.token.as_str(),
                    state,
                    serde_json::to_vec(&request.continuation).unwrap(),
                    terminal
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO vm_dispatches(token,request,state,attempt) VALUES (?1,?2,?3,0)",
                params![
                    request.continuation.token.as_str(),
                    serde_json::to_vec(&request).unwrap(),
                    if state == "pending" {
                        "ready"
                    } else {
                        "acknowledged"
                    }
                ],
            )
            .unwrap();
    }
    connection
        .execute_batch(
            "UPDATE vm_metadata SET next_sequence=3; DROP INDEX vm_dispatch_attempt_order_idx;",
        )
        .unwrap();
    // Build legacy shapes from the frozen schema, not the migrated adapter.
    if target < 7 {
        connection
            .execute_batch("ALTER TABLE vm_merge_groups DROP COLUMN execution_order;")
            .unwrap();
    }
    if target < 6 {
        connection
            .execute_batch("DROP TABLE vm_debugger_audit;")
            .unwrap();
    }
    if target < 5 {
        connection
            .execute_batch("DROP TABLE vm_merge_branches; DROP TABLE vm_merge_groups;")
            .unwrap();
    }
    if target < 4 {
        connection.execute_batch("DROP INDEX vm_dispatch_ready_idx; ALTER TABLE vm_dispatches DROP COLUMN ready_at_ms; ALTER TABLE vm_dispatches DROP COLUMN retry_count; ALTER TABLE vm_dispatches DROP COLUMN last_error;").unwrap();
    }
    if target < 3 {
        connection.execute_batch("DROP INDEX vm_effect_deadline_idx; ALTER TABLE vm_effects DROP COLUMN deadline_at_ms;").unwrap();
    }
    if target < 2 {
        connection
            .execute_batch("DROP TABLE vm_dispatches;")
            .unwrap();
    }
    connection
        .execute_batch(&format!("PRAGMA user_version={target};"))
        .unwrap();
}

fn compare_vms(actual: &crate::Vm, expected: &crate::Vm) {
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(
        actual.scheduler_pressure(100),
        expected.scheduler_pressure(100)
    );
}

fn frozen_open(path: &Path) -> crate::Vm {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    frozen_schema(&Owner::new(&connection), version(&connection)).unwrap();
    let mut journal = SqliteJournal { connection };
    let snapshot = journal.load().unwrap();
    crate::Vm {
        next_effect_id: snapshot.next_sequence,
        fuel_limit: 100,
        pending: snapshot.pending,
        completed: snapshot.completed,
        journal: Journal::Sqlite(journal),
        scheduler_limits: SchedulerLimits::default(),
    }
}

#[test]
fn schema_migration_all_supported_versions_match_frozen_rows_recovery_and_reopen() {
    let scratch = Scratch::new();
    for v in 0..=10 {
        let actual_path = scratch.path(&format!("{v}-actual"));
        let expected_path = scratch.path(&format!("{v}-expected"));
        seed(&actual_path, v);
        seed(&expected_path, v);
        let mut actual = crate::Vm::open_journal(&actual_path, 100).unwrap();
        let mut expected = frozen_open(&expected_path);
        compare_vms(&actual, &expected);
        compare_connections(
            &Connection::open(&actual_path).unwrap(),
            &Connection::open(&expected_path).unwrap(),
        );
        let a = actual.claim_effect(100, 50).unwrap();
        let e = expected.claim_effect(100, 50).unwrap();
        assert_eq!(a, e);
        assert_eq!(a.is_some(), v >= 2);
        if let (Some(a), Some(e)) = (a, e) {
            let got = actual.acknowledge_effect(&a, 101, reply(&a.request));
            let want = expected.acknowledge_effect(&e, 101, reply(&e.request));
            assert_eq!(got, want);
            assert_eq!(actual.acknowledge_effect(&a, 102, reply(&a.request)), got);
            assert_eq!(
                expected.acknowledge_effect(&e, 102, reply(&e.request)),
                want
            );
        }
        drop(actual);
        drop(expected);
        let mut actual = crate::Vm::open_journal(&actual_path, 100).unwrap();
        let mut expected = frozen_open(&expected_path);
        compare_vms(&actual, &expected);
        assert_eq!(actual.claim_effect(200, 50), expected.claim_effect(200, 50));
        compare_connections(
            &Connection::open(&actual_path).unwrap(),
            &Connection::open(&expected_path).unwrap(),
        );
    }
}

#[test]
fn schema_native_failures_preserve_each_commit_prefix_and_rollback_only_open_transaction() {
    let scratch = Scratch::new();
    let mut cases = 0;
    for v in 0..=10 {
        let mut batch_count = 0;
        leselang_command::reference_journal_schema::migrate(v, |_, _| {
            batch_count += 1;
            Ok(())
        })
        .unwrap();
        for fail_at in 0..batch_count {
            for phase in [Failure::Before, Failure::During, Failure::After] {
                let actual_path = scratch.path(&format!("{cases}-actual"));
                let expected_path = scratch.path(&format!("{cases}-expected"));
                seed(&actual_path, v);
                seed(&expected_path, v);
                {
                    let actual = Connection::open(&actual_path).unwrap();
                    let expected = Connection::open(&expected_path).unwrap();
                    for connection in [&actual, &expected] {
                        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA secure_delete=ON; PRAGMA trusted_schema=OFF; PRAGMA synchronous=FULL;").unwrap();
                    }
                    let a = Owner {
                        failure: Some((fail_at, phase)),
                        ..Owner::new(&actual)
                    };
                    let e = Owner {
                        failure: Some((fail_at, phase)),
                        ..Owner::new(&expected)
                    };
                    let got = migrated(&a, v).unwrap_err();
                    let want = frozen_schema(&e, v).unwrap_err();
                    assert_eq!(got, want);
                    assert_eq!(a.calls.get(), fail_at + 1);
                    assert_eq!(a.calls.get(), e.calls.get());
                    assert_eq!(*a.batches.borrow(), *e.batches.borrow());
                    compare_connections(&actual, &expected);
                }
                {
                    let actual = Connection::open(&actual_path).unwrap();
                    let expected = Connection::open(&expected_path).unwrap();
                    compare_connections(&actual, &expected);
                }
                // Reopening resumes from the committed version, never from a fake
                // all-or-nothing rollback or an in-memory progress watermark.
                let actual = crate::Vm::open_journal(&actual_path, 100).unwrap();
                let expected = frozen_open(&expected_path);
                compare_vms(&actual, &expected);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 180);
}

#[test]
fn schema_future_versions_refuse_without_schema_writes() {
    let scratch = Scratch::new();
    for v in [11, 99, i32::MAX as u32] {
        let path = scratch.path(&format!("unsupported-{v}"));
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(&format!("PRAGMA user_version={v};"))
                .unwrap();
        }
        let error = crate::Vm::open_journal(&path, 100).err().unwrap();
        let connection = Connection::open(&path).unwrap();
        let owner = Owner::new(&connection);
        assert_eq!(error, frozen_schema(&owner, v).unwrap_err());
        assert_eq!(owner.calls.get(), 0);
        assert_eq!(version(&connection), v);
        assert!(schema(&connection).is_empty());
    }
}

// Frozen schema lifecycle from 49648886, with only the connection owner injected.
// Shared file-safety configuration, storage engine and payload recovery are not
// independent implementations and are not part of this compatibility oracle.
fn frozen_schema(connection: &Owner<'_>, version: u32) -> Result<(), Fault> {
    const JOURNAL_SCHEMA_VERSION: u32 = 10;
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
    } else if !matches!(version, 4 | 5 | 6 | 7 | 8 | 9 | JOURNAL_SCHEMA_VERSION) {
        return Err(journal_fault(
            "LSV4004",
            format!(
                "unsupported journal version {version}, expected 1 through {JOURNAL_SCHEMA_VERSION}"
            ),
        ));
    }

    if version < 5 {
        connection
            .execute_batch(
                "BEGIN IMMEDIATE;
                 CREATE TABLE vm_merge_groups (
                   token TEXT PRIMARY KEY,
                   plan BLOB NOT NULL,
                   state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                   terminal_step BLOB,
                   CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                          (state = 'completed' AND terminal_step IS NOT NULL))
                 ) STRICT;
                 CREATE TABLE vm_merge_branches (
                   group_token TEXT NOT NULL
                     REFERENCES vm_merge_groups(token) ON DELETE CASCADE,
                   branch_token TEXT NOT NULL UNIQUE
                     REFERENCES vm_effects(token) ON DELETE CASCADE,
                   branch_name TEXT NOT NULL,
                   position INTEGER NOT NULL CHECK (position >= 0 AND position < 64),
                   PRIMARY KEY (group_token, branch_name),
                   UNIQUE (group_token, position)
                 ) STRICT;
                 CREATE INDEX vm_merge_branch_group_idx
                   ON vm_merge_branches(group_token, position);
                 PRAGMA user_version = 5;
                 COMMIT;",
            )
            .map_err(|error| journal_error("LSV4003", "failed to migrate merge journal", error))?;
    }

    if version < 6 {
        connection
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     CREATE TABLE vm_debugger_audit (
                       command_id TEXT PRIMARY KEY,
                       principal_id TEXT NOT NULL,
                       idempotency_key TEXT NOT NULL,
                       origin TEXT NOT NULL CHECK (origin IN ('gui', 'cli', 'leselang', 'model', 'compatibility_adapter')),
                       session_id TEXT NOT NULL,
                       expected_revision INTEGER NOT NULL CHECK (expected_revision >= 0),
                       observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
                       continuation_token TEXT NOT NULL
                         REFERENCES vm_effects(token) ON DELETE CASCADE,
                       UNIQUE (principal_id, idempotency_key)
                     ) STRICT;
                     CREATE INDEX vm_debugger_audit_token_idx
                       ON vm_debugger_audit(continuation_token);
                     PRAGMA user_version = 6;
                     COMMIT;",
                )
                .map_err(|error| {
                    journal_error("LSV4003", "failed to migrate debugger audit journal", error)
                })?;
    }

    if version < 7 {
        connection.execute_batch(
                "BEGIN IMMEDIATE;
                 ALTER TABLE vm_merge_groups ADD COLUMN execution_order TEXT NOT NULL DEFAULT 'parallel'
                   CHECK (execution_order IN ('parallel', 'sequential'));
                 PRAGMA user_version = 7;
                 COMMIT;"
            ).map_err(|error| journal_error("LSV4003", "failed to migrate control flow journal", error))?;
    }

    if version < 8 {
        connection
            .execute_batch("BEGIN IMMEDIATE; PRAGMA user_version = 8; COMMIT;")
            .map_err(|error| {
                journal_error("LSV4003", "failed to migrate result-chain journal", error)
            })?;
    }

    if version < 9 {
        connection
            .execute_batch("BEGIN IMMEDIATE; PRAGMA user_version = 9; COMMIT;")
            .map_err(|error| {
                journal_error("LSV4003", "failed to migrate dataflow journal", error)
            })?;
    }

    if version < 10 {
        connection
            .execute_batch("BEGIN IMMEDIATE; PRAGMA user_version = 10; COMMIT;")
            .map_err(|error| {
                journal_error(
                    "LSV4003",
                    "failed to migrate conditional-exit journal",
                    error,
                )
            })?;
    }

    // Canonical tokens share one prefix and have no leading zeroes, so length/text
    // order is numeric admission order without casts or a new persisted sequence.
    connection
        .execute_batch(
            "CREATE INDEX IF NOT EXISTS vm_dispatch_attempt_order_idx
                 ON vm_dispatches(attempt, length(token), token)
                 WHERE state IN ('ready', 'leased');",
        )
        .map_err(|error| journal_error("LSV4003", "failed to index dispatch ordering", error))?;
    Ok(())
}
