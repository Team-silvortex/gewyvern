use super::group_recovery_contract_tests::{Fixture, snapshot};
use super::*;

const ORIGINS: [&str; 5] = ["gui", "cli", "leselang", "model", "compatibility_adapter"];

fn terminal(token: &str, reason: CancellationReason, time: u64) -> Vec<u8> {
    serde_json::to_vec(&Step::Cancelled(Cancellation {
        continuation: ContinuationToken::from_wire(token.into()),
        reason,
        observed_at_ms: time,
    }))
    .unwrap()
}

fn raw_fixture(origin: &str) -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    // Deliberately omit constraints to probe persisted corruption and lazy row
    // decoding. These fixture rows are not a valid checkpoint or a dispatch grant.
    connection.execute_batch("CREATE TABLE vm_effects(token, state, terminal_step);
        CREATE TABLE vm_debugger_audit(command_id, principal_id, idempotency_key, origin, session_id, expected_revision, observed_at_ms, continuation_token);
        CREATE INDEX vm_debugger_audit_token_idx ON vm_debugger_audit(continuation_token);").unwrap();
    connection
        .execute(
            "INSERT INTO vm_effects VALUES ('effect-1','completed',?1)",
            [terminal("effect-1", CancellationReason::Requested, 42)],
        )
        .unwrap();
    connection.execute("INSERT INTO vm_debugger_audit VALUES ('audit-1','operator','key-1',?1,'session-1',7,42,'effect-1')", [origin]).unwrap();
    connection
}

fn defect(connection: &Connection, defect: usize) {
    let sql = match defect {
        0 => "",
        1 => "UPDATE vm_debugger_audit SET command_id='';",
        2 => "UPDATE vm_debugger_audit SET command_id=x'01';",
        3 => "UPDATE vm_debugger_audit SET principal_id='bad principal';",
        4 => "UPDATE vm_debugger_audit SET principal_id=x'01';",
        5 => "UPDATE vm_debugger_audit SET idempotency_key='';",
        6 => "UPDATE vm_debugger_audit SET idempotency_key=x'01';",
        7 => "UPDATE vm_debugger_audit SET origin='GUI';",
        8 => "UPDATE vm_debugger_audit SET origin=x'01';",
        9 => "UPDATE vm_debugger_audit SET session_id='bad session';",
        10 => "UPDATE vm_debugger_audit SET session_id=x'01';",
        11 => "UPDATE vm_debugger_audit SET expected_revision=-1;",
        12 => "UPDATE vm_debugger_audit SET expected_revision='bad integer';",
        13 => "UPDATE vm_debugger_audit SET observed_at_ms=-1;",
        14 => "UPDATE vm_debugger_audit SET observed_at_ms='bad integer';",
        15 => {
            "UPDATE vm_debugger_audit SET continuation_token='bad token'; UPDATE vm_effects SET token='bad token';"
        }
        16 => {
            "UPDATE vm_debugger_audit SET continuation_token=x'01'; UPDATE vm_effects SET token=x'01';"
        }
        17 => "UPDATE vm_effects SET state='pending';",
        18 => "UPDATE vm_effects SET state=x'01';",
        19 => "UPDATE vm_effects SET terminal_step=x'7b';",
        20 => "UPDATE vm_effects SET terminal_step=NULL;",
        21 | 22 => {
            connection
                .execute(
                    "UPDATE vm_effects SET terminal_step=?1",
                    [terminal(
                        "effect-1",
                        if defect == 21 {
                            CancellationReason::Requested
                        } else {
                            CancellationReason::DeadlineExceeded
                        },
                        if defect == 21 { 43 } else { 42 },
                    )],
                )
                .unwrap();
            return;
        }
        23 => "DELETE FROM vm_effects;",
        24 => "DROP INDEX vm_debugger_audit_token_idx;",
        25 => "INSERT INTO vm_effects SELECT * FROM vm_effects;",
        26 => "DELETE FROM vm_debugger_audit;",
        27 => {
            "INSERT INTO vm_debugger_audit SELECT 'audit-2',principal_id,'key-2',origin,session_id,expected_revision,observed_at_ms,continuation_token FROM vm_debugger_audit;"
        }
        _ => unreachable!(),
    };
    connection.execute_batch(sql).unwrap();
}

fn rows(connection: &Connection, table: &str) -> Vec<Vec<rusqlite::types::Value>> {
    let mut query = connection
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .unwrap();
    let columns = query.column_count();
    query
        .query_map([], |row| {
            (0..columns)
                .map(|i| row.get(i))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn compare_read_only(connection: &Connection) -> Result<(), Fault> {
    let before = (
        rows(connection, "vm_effects"),
        rows(connection, "vm_debugger_audit"),
        connection.total_changes(),
    );
    let expected = frozen_validate_audits(connection);
    let actual = validate_debugger_audit_records(connection);
    assert_eq!(actual, expected);
    assert_eq!(
        before,
        (
            rows(connection, "vm_effects"),
            rows(connection, "vm_debugger_audit"),
            connection.total_changes()
        )
    );
    assert!(connection.is_autocommit());
    actual
}

#[test]
fn audit_recovery_matches_frozen_origins_bad_rows_conflicting_faults_and_join_counts() {
    let mut cases = 0;
    for origin in ORIGINS {
        for broken in 0..28 {
            let connection = raw_fixture(origin);
            defect(&connection, broken);
            let result = compare_read_only(&connection);
            assert_eq!(
                result.is_ok(),
                matches!(broken, 0 | 26 | 27),
                "{origin} {broken}: {result:?}"
            );
            cases += 1;
        }
        for first in [1, 3, 5, 7, 9, 11, 13, 15, 17, 19] {
            for second in [1, 3, 5, 7, 9, 11, 13, 15, 17, 19] {
                let connection = raw_fixture(origin);
                defect(&connection, first);
                defect(&connection, second);
                assert!(compare_read_only(&connection).is_err());
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 640);
}

#[test]
fn audit_recovery_keeps_native_query_errors_and_schema_before_row_order() {
    for sql in [
        "DROP TABLE vm_debugger_audit; CREATE TABLE dummy(token); CREATE INDEX vm_debugger_audit_token_idx ON dummy(token);",
        "ALTER TABLE vm_debugger_audit DROP COLUMN session_id;",
        "ALTER TABLE vm_effects DROP COLUMN state;",
    ] {
        let connection = raw_fixture("gui");
        connection.execute_batch(sql).unwrap();
        let changes = connection.total_changes();
        let expected = frozen_validate_audits(&connection).unwrap_err();
        assert_eq!(
            validate_debugger_audit_records(&connection).unwrap_err(),
            expected
        );
        assert_eq!(expected.code, "LSV4005");
        assert_eq!(connection.total_changes(), changes);
    }
    let connection = raw_fixture("gui");
    connection
        .execute_batch(
            "UPDATE vm_debugger_audit SET command_id=''; DROP INDEX vm_debugger_audit_token_idx;",
        )
        .unwrap();
    assert_eq!(
        compare_read_only(&connection).unwrap_err().message,
        "debugger audit schema is incomplete"
    );
}

#[test]
fn audit_recovery_keeps_payload_bounds_and_does_not_repair_terminal_bytes() {
    for bytes in [
        vec![b' '; MAX_JOURNAL_ENTRY_BYTES + 1],
        terminal("effect-2", CancellationReason::Requested, 42),
        serde_json::to_vec(&Step::Fault(Fault {
            code: "HOST".into(),
            message: "not a cancellation".into(),
        }))
        .unwrap(),
    ] {
        let connection = raw_fixture("gui");
        connection
            .execute("UPDATE vm_effects SET terminal_step=?1", [&bytes])
            .unwrap();
        assert!(compare_read_only(&connection).is_err());
    }
}

fn all_tables(connection: &Connection) -> Vec<Vec<rusqlite::types::Value>> {
    let mut values = snapshot(connection);
    values.extend(rows(connection, "vm_debugger_audit"));
    values
}

#[test]
fn audit_recovery_blocks_watermark_publication_then_reopens_with_original_receipt() {
    let mut cases = 0;
    for label in ORIGINS {
        for broken in 0..7 {
            let fixture = Fixture::new(r#"fn main() = ui.focus(node_id: "a")"#, 0);
            let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
            let image = vm.pending_continuations().pop().unwrap();
            let audit = DebuggerAuditContext {
                command_id: crate::CommandId::new("audit-original").unwrap(),
                idempotency_key: crate::IdempotencyKey::new("key-original").unwrap(),
                principal: crate::Principal::new("operator").unwrap(),
                origin: leselang_command::debugger_audit::parse_command_origin(label).unwrap(),
                session_id: "session-original".into(),
                expected_revision: crate::Revision(7),
            };
            let receipt = vm.cancel_effect_audited(&image, 30, &audit).unwrap();
            drop(vm);
            let mut journal = SqliteJournal {
                connection: Connection::open(&fixture.0).unwrap(),
            };
            // Corrupt only this fixture, then restore enforcement before load.
            journal
                .connection
                .execute_batch(
                    "PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON; UPDATE vm_metadata SET next_sequence=1;",
                )
                .unwrap();
            journal
                .connection
                .execute_batch(match broken {
                    0 => "",
                    1 => "UPDATE vm_debugger_audit SET observed_at_ms=31;",
                    2 => "UPDATE vm_debugger_audit SET principal_id='bad principal';",
                    3 => "UPDATE vm_debugger_audit SET origin='unknown';",
                    4 => "UPDATE vm_debugger_audit SET continuation_token='effect-999';",
                    5 => "DROP INDEX vm_debugger_audit_token_idx;",
                    6 => "UPDATE vm_debugger_audit SET command_id='';",
                    _ => unreachable!(),
                })
                .unwrap();
            journal
                .connection
                .execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
                .unwrap();
            let before = all_tables(&journal.connection);
            let expected = frozen_validate_audits(&journal.connection);
            let actual = journal.load();
            assert_eq!(actual.as_ref().map(|_| ()).map_err(Clone::clone), expected);
            assert!(journal.connection.is_autocommit());
            if broken == 0 {
                let snapshot = actual.unwrap();
                assert_eq!(snapshot.next_sequence, 2);
                assert!(snapshot.pending.is_empty());
                assert_eq!(snapshot.completed.len(), 1);
            } else {
                assert_eq!(all_tables(&journal.connection), before);
            }
            journal.connection.execute_batch("DELETE FROM vm_debugger_audit; CREATE INDEX IF NOT EXISTS vm_debugger_audit_token_idx ON vm_debugger_audit(continuation_token);").unwrap();
            journal.connection.execute(
                "INSERT INTO vm_debugger_audit(command_id,principal_id,idempotency_key,origin,session_id,expected_revision,observed_at_ms,continuation_token)
                 VALUES (?1,?2,?3,?4,?5,7,30,?6)",
                params![audit.command_id.as_str(), audit.principal.id, audit.idempotency_key.as_str(), label, audit.session_id, image.token.as_str()],
            ).unwrap();
            drop(journal);
            let mut reopened = crate::Vm::open_journal(&fixture.0, 500).unwrap();
            assert_eq!(
                reopened.debugger_audit(&audit.command_id).unwrap(),
                Some(receipt.clone())
            );
            assert_eq!(
                reopened.cancel_effect_audited(&image, 200, &audit).unwrap(),
                receipt
            );
            assert!(reopened.claim_effect(201, 50).unwrap().is_none());
            assert_eq!(reopened.next_effect_id, 2);
            cases += 1;
        }
    }
    assert_eq!(cases, 35);
}

// Frozen pre-extraction audit pass from 49648886. SQLite reads and the unchanged
// bounded decoder/identity primitives remain shared, not the migrated policy.
fn frozen_validate_audits(connection: &Connection) -> Result<(), Fault> {
    let index_count: u32 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'vm_debugger_audit_token_idx'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| {
            journal_error("LSV4005", "failed to validate debugger audit schema", error)
        })?;
    if index_count != 1 {
        return Err(journal_fault(
            "LSV4007",
            "debugger audit schema is incomplete",
        ));
    }

    let audit_count: u32 = connection
        .query_row("SELECT COUNT(*) FROM vm_debugger_audit", [], |row| {
            row.get(0)
        })
        .map_err(|error| journal_error("LSV4005", "failed to count debugger audits", error))?;

    let mut statement = connection
        .prepare(
            "SELECT a.command_id, a.principal_id, a.idempotency_key, a.origin,
                    a.session_id, a.expected_revision, a.observed_at_ms,
                    a.continuation_token, e.state, e.terminal_step
             FROM vm_debugger_audit a
             JOIN vm_effects e ON e.token = a.continuation_token
             ORDER BY a.command_id ASC",
        )
        .map_err(|error| {
            journal_error("LSV4005", "failed to prepare debugger audit load", error)
        })?;
    let mut rows = statement
        .query([])
        .map_err(|error| journal_error("LSV4005", "failed to load debugger audits", error))?;
    let mut validated_count = 0_u32;
    while let Some(row) = rows
        .next()
        .map_err(|error| journal_error("LSV4005", "failed to read debugger audit", error))?
    {
        let command_id =
            crate::CommandId::new(row.get::<_, String>(0).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit command", error)
            })?)
            .map_err(|_| journal_fault("LSV4007", "debugger audit command is invalid"))?;
        let principal_id = row
            .get::<_, String>(1)
            .map_err(|error| journal_error("LSV4005", "invalid debugger audit principal", error))?;
        let idempotency_key =
            crate::IdempotencyKey::new(row.get::<_, String>(2).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit idempotency key", error)
            })?)
            .map_err(|_| journal_fault("LSV4007", "debugger audit idempotency key is invalid"))?;
        let origin =
            parse_command_origin(&row.get::<_, String>(3).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit origin", error)
            })?)
            .map_err(|error| journal_error("LSV4007", "debugger audit origin is invalid", error))?;
        let session_id = row
            .get::<_, String>(4)
            .map_err(|error| journal_error("LSV4005", "invalid debugger audit session", error))?;
        let expected_revision =
            u64::try_from(row.get::<_, i64>(5).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit revision", error)
            })?)
            .map_err(|_| journal_fault("LSV4007", "debugger audit revision is invalid"))?;
        let observed_at_ms =
            u64::try_from(row.get::<_, i64>(6).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit clock", error)
            })?)
            .map_err(|_| journal_fault("LSV4007", "debugger audit clock is invalid"))?;
        let token = ContinuationToken::from_wire(row.get::<_, String>(7).map_err(|error| {
            journal_error("LSV4005", "invalid debugger audit continuation", error)
        })?);
        let context = DebuggerAuditContext {
            command_id,
            idempotency_key,
            principal: crate::Principal { id: principal_id },
            origin,
            session_id,
            expected_revision: crate::Revision(expected_revision),
        };
        crate::validate_debugger_audit(&context)
            .map_err(|_| journal_fault("LSV4007", "debugger audit identity is invalid"))?;
        if !valid_continuation_token(&token)
            || row.get::<_, String>(8).map_err(|error| {
                journal_error("LSV4005", "invalid debugger audit effect state", error)
            })? != "completed"
        {
            return Err(journal_fault(
                "LSV4007",
                "debugger audit continuation is invalid",
            ));
        }
        let terminal_bytes = row.get::<_, Vec<u8>>(9).map_err(|error| {
            journal_error("LSV4005", "invalid debugger audit terminal step", error)
        })?;
        let terminal: Step = decode_bounded(&terminal_bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        if !matches!(
            terminal,
            Step::Cancelled(Cancellation {
                continuation,
                reason: CancellationReason::Requested,
                observed_at_ms: terminal_observed_at,
            }) if continuation == token && terminal_observed_at == observed_at_ms
        ) {
            return Err(journal_fault(
                "LSV4007",
                "debugger audit does not match its terminal cancellation",
            ));
        }
        validated_count += 1;
    }
    if validated_count != audit_count {
        return Err(journal_fault(
            "LSV4007",
            "debugger audit references a missing continuation",
        ));
    }
    Ok(())
}
