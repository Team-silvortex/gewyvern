use super::group_recovery_contract_tests::{Fixture, snapshot};
use super::*;
use leselang_runtime_core::{CheckpointCommitError, CheckpointError, CheckpointRecovery};

fn checkpoint_fault(error: CheckpointError) -> Fault {
    match error {
        CheckpointError::RecordLimit => {
            journal_fault("LSV4006", "journal recovery record limit exceeded")
        }
        CheckpointError::InvalidSequence => {
            journal_fault("LSV4007", "journal continuation sequence is invalid")
        }
        CheckpointError::DuplicateKey => {
            journal_fault("LSV4007", "duplicate journal continuation token")
        }
        CheckpointError::Closed => journal_fault("LSV4007", "journal recovery pass is closed"),
    }
}

fn all_tables(connection: &Connection) -> Vec<Vec<rusqlite::types::Value>> {
    let mut result = snapshot(connection);
    let mut statement = connection
        .prepare("SELECT * FROM vm_debugger_audit ORDER BY command_id")
        .unwrap();
    let columns = statement.column_count();
    result.extend(
        statement
            .query_map([], |row| {
                (0..columns)
                    .map(|i| row.get(i))
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
    );
    result
}

fn mutate(connection: &Connection, defect: usize) {
    connection
        .pragma_update(None, "ignore_check_constraints", true)
        .unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    let sql = match defect {
        0 => "",
        1 => "UPDATE vm_metadata SET next_sequence = 1;",
        2 => "UPDATE vm_metadata SET next_sequence = 9223372036854775807;",
        3 => "UPDATE vm_metadata SET next_sequence = 0;",
        4 => "UPDATE vm_dispatches SET request = x'7b';",
        5 => "UPDATE vm_merge_groups SET execution_order = 'invalid';",
        6 => "DROP INDEX vm_debugger_audit_token_idx;",
        7 => "DELETE FROM vm_metadata;",
        8 => {
            "UPDATE vm_metadata SET next_sequence = 1; CREATE TRIGGER reject_watermark BEFORE UPDATE ON vm_metadata BEGIN SELECT RAISE(ABORT, 'watermark refusal'); END;"
        }
        9 => {
            "UPDATE vm_metadata SET next_sequence = 1; CREATE TABLE recovery_parent(id INTEGER PRIMARY KEY); CREATE TABLE recovery_child(id INTEGER REFERENCES recovery_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER reject_commit AFTER UPDATE ON vm_metadata BEGIN INSERT INTO recovery_child VALUES(1); END;"
        }
        10 => {
            "UPDATE vm_dispatches SET request = x'7b'; UPDATE vm_merge_groups SET execution_order = 'invalid'; DROP INDEX vm_debugger_audit_token_idx;"
        }
        11 => {
            "UPDATE vm_merge_groups SET execution_order = 'invalid'; DROP INDEX vm_debugger_audit_token_idx;"
        }
        12 => "DROP INDEX vm_debugger_audit_token_idx; UPDATE vm_metadata SET next_sequence = -1;",
        13 => "UPDATE vm_debugger_audit SET observed_at_ms = observed_at_ms + 1;",
        14 => {
            "UPDATE vm_debugger_audit SET principal_id = ''; UPDATE vm_metadata SET next_sequence = -1;"
        }
        _ => unreachable!(),
    };
    connection.execute_batch(sql).unwrap();
}

#[test]
fn checkpoint_finalization_matches_frozen_load_faults_snapshots_and_atomic_writes() {
    let sources = [
        r#"fn main() = ui.focus(node_id: "a")"#,
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: 1)"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: 1)"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(a: ui.focus(node_id: "a"), body: ui.focus(node_id: field(value: a, name: "node_id")))"#,
        r#"fn main() = bind(a: ui.focus(node_id: "a"), body: bind(b: ui.focus(node_id: field(value: a, name: "node_id")), body: ui.focus(node_id: field(value: b, name: "node_id"))))"#,
    ];
    let cases = sources
        .iter()
        .flat_map(|source| (0..3).map(move |stage| (*source, stage, false)))
        .chain([(sources[0], 0, true)]);
    let mut comparisons = 0;
    for (source, stage, audited) in cases {
        for defect in 0..15 {
            let fixture = Fixture::new(source, stage);
            if audited {
                let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let image = vm.pending_continuations().pop().unwrap();
                vm.cancel_effect_audited(
                    &image,
                    30,
                    &DebuggerAuditContext {
                        command_id: crate::CommandId::new("audit-original").unwrap(),
                        idempotency_key: crate::IdempotencyKey::new("key-original").unwrap(),
                        principal: crate::Principal::new("operator").unwrap(),
                        origin: crate::CommandOrigin::Gui,
                        session_id: "original-session".into(),
                        expected_revision: crate::Revision(7),
                    },
                )
                .unwrap();
            }
            let reference_path = fixture.0.with_extension("reference.sqlite3");
            std::fs::copy(&fixture.0, &reference_path).unwrap();
            let mut original = Connection::open(&reference_path).unwrap();
            let mut actual = SqliteJournal {
                connection: Connection::open(&fixture.0).unwrap(),
            };
            mutate(&original, defect);
            mutate(&actual.connection, defect);
            let before = all_tables(&actual.connection);
            assert_eq!(all_tables(&original), before);
            let comparable = |s: JournalSnapshot| (s.pending, s.completed, s.next_sequence);
            let expected = frozen_load(&mut original).map(comparable);
            let result = actual.load().map(comparable);
            if defect == 0 {
                assert!(expected.is_ok(), "{source}, {stage}, {audited}");
            }
            if defect == 9 && source == sources[0] {
                let error = expected.as_ref().unwrap_err();
                assert_eq!(error.code, "LSV4005");
                assert!(
                    error.message.starts_with("failed to commit journal load"),
                    "{error:?}"
                );
                for connection in [&original, &actual.connection] {
                    assert_eq!(
                        connection
                            .query_row("SELECT COUNT(*) FROM recovery_child", [], |row| row
                                .get::<_, i64>(0))
                            .unwrap(),
                        0
                    );
                }
            }
            if expected.is_err() {
                assert_eq!(all_tables(&actual.connection), before);
            }
            assert_eq!(result, expected, "{source}, {stage}, {audited}, {defect}");
            assert_eq!(all_tables(&actual.connection), all_tables(&original));
            assert!(actual.connection.is_autocommit());
            assert!(original.is_autocommit());
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 420);
}

fn frozen_load(connection: &mut Connection) -> Result<JournalSnapshot, Fault> {
    // Validation and allocator repair must observe the same complete graph.
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| journal_error("LSV4005", "failed to lock journal load", error))?;
    let count = journal_record_count(&transaction)?;
    if count > MAX_JOURNAL_RECORDS {
        return Err(journal_fault(
            "LSV4006",
            format!("journal has {count} records, limit is {MAX_JOURNAL_RECORDS}"),
        ));
    }
    let total_bytes = journal_payload_bytes(&transaction)?;
    if total_bytes > MAX_JOURNAL_TOTAL_BYTES {
        return Err(journal_fault(
            "LSV4006",
            format!("journal payload is {total_bytes} bytes, limit is {MAX_JOURNAL_TOTAL_BYTES}"),
        ));
    }

    let mut statement = transaction
        .prepare(
            "SELECT token, state, image, deadline_at_ms, terminal_step
             FROM vm_effects ORDER BY token ASC",
        )
        .map_err(|error| journal_error("LSV4005", "failed to prepare journal load", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| journal_error("LSV4005", "failed to load journal", error))?;
    let mut recovery = CheckpointRecovery::new(MAX_JOURNAL_RECORDS, crate::MAX_EFFECT_SEQUENCE);

    while let Some(row) = rows
        .next()
        .map_err(|error| journal_error("LSV4005", "failed to read journal record", error))?
    {
        recovery
            .push(recover_effect_record(&transaction, row)?)
            .map_err(checkpoint_fault)?;
    }

    drop(rows);
    drop(statement);
    let (snapshot, next_sequence) = recovery
        .finish(Some(transaction), |transaction, snapshot| {
            let connection = transaction
                .as_ref()
                .ok_or_else(|| journal_fault("LSV4005", "journal load transaction is closed"))?;
            validate_dispatch_records(connection)?;
            let highest_sequence = snapshot
                .highest_sequence()
                .max(validate_merge_graph_records(connection)?);
            validate_debugger_audit_records(connection)?;
            let next_sequence = advance_sequence_watermark(connection, highest_sequence)?;
            let transaction = transaction
                .take()
                .ok_or_else(|| journal_fault("LSV4005", "journal load transaction is closed"))?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4005", "failed to commit journal load", error)
            })?;
            Ok(next_sequence)
        })
        .map_err(|error| match error {
            CheckpointCommitError::Host(error) => error,
            CheckpointCommitError::Closed => checkpoint_fault(CheckpointError::Closed),
        })?;
    let (pending, completed) = snapshot.into_parts();
    Ok(JournalSnapshot {
        pending,
        completed,
        next_sequence,
    })
}
