use super::*;

const ORIGINS: [&str; 5] = ["gui", "cli", "leselang", "model", "compatibility_adapter"];
const RECORD_SQL: &str = "SELECT command_id, principal_id, origin, session_id, expected_revision, observed_at_ms FROM vm_debugger_audit";
const REPLAY_SQL: &str = "SELECT command_id, continuation_token, idempotency_key, principal_id, origin, session_id, expected_revision, observed_at_ms FROM vm_debugger_audit";

fn fixture(origin: &str) -> SqliteJournal {
    let connection = Connection::open_in_memory().unwrap();
    // This unconstrained table models raw row/type corruption, not a valid VM
    // checkpoint. Real cancellation/reopen coverage remains in the commit suite.
    connection.execute_batch("CREATE TABLE vm_debugger_audit(command_id, principal_id, idempotency_key, origin, session_id, expected_revision, observed_at_ms, continuation_token);").unwrap();
    connection.execute("INSERT INTO vm_debugger_audit VALUES ('audit-1','operator','key-1',?1,'session',7,42,'effect-1')", [origin]).unwrap();
    SqliteJournal { connection }
}

fn defect(connection: &Connection, kind: usize) {
    connection
        .execute_batch(match kind {
            0 => "",
            1 => "UPDATE vm_debugger_audit SET command_id='';",
            2 => "UPDATE vm_debugger_audit SET command_id=x'01';",
            3 => "UPDATE vm_debugger_audit SET principal_id='';",
            4 => "UPDATE vm_debugger_audit SET principal_id=x'01';",
            5 => "UPDATE vm_debugger_audit SET origin='GUI';",
            6 => "UPDATE vm_debugger_audit SET origin=x'01';",
            7 => "UPDATE vm_debugger_audit SET session_id='';",
            8 => "UPDATE vm_debugger_audit SET session_id=x'01';",
            9 => "UPDATE vm_debugger_audit SET expected_revision=-1;",
            10 => "UPDATE vm_debugger_audit SET expected_revision='bad integer';",
            11 => "UPDATE vm_debugger_audit SET observed_at_ms=-1;",
            12 => "UPDATE vm_debugger_audit SET observed_at_ms='bad integer';",
            13 => "UPDATE vm_debugger_audit SET continuation_token=x'01';",
            14 => "UPDATE vm_debugger_audit SET idempotency_key=x'01';",
            15 => "UPDATE vm_debugger_audit SET continuation_token='', idempotency_key='bad key';",
            16 => "DELETE FROM vm_debugger_audit;",
            _ => unreachable!(),
        })
        .unwrap();
}

fn native_error(error: rusqlite::Error) -> (String, String) {
    (format!("{error:?}"), error.to_string())
}
fn snapshot(connection: &Connection) -> Vec<Vec<rusqlite::types::Value>> {
    let mut query = connection
        .prepare("SELECT * FROM vm_debugger_audit ORDER BY rowid")
        .unwrap();
    query
        .query_map([], |row| {
            (0..8)
                .map(|i| row.get(i))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn compare_projections(connection: &Connection) {
    let before = snapshot(connection);
    let changes = connection.total_changes();
    let expected = connection
        .query_row(RECORD_SQL, [], frozen_decode_record)
        .map_err(native_error);
    let actual = connection
        .query_row(RECORD_SQL, [], audit_read::decode_record)
        .map_err(native_error);
    assert_eq!(actual, expected);
    let expected = connection
        .query_row(REPLAY_SQL, [], frozen_decode_replay)
        .map_err(native_error);
    let actual = connection
        .query_row(REPLAY_SQL, [], audit_read::decode_replay)
        .map_err(native_error);
    assert_eq!(actual, expected);
    assert_eq!(snapshot(connection), before);
    assert_eq!(connection.total_changes(), changes);
    assert!(connection.is_autocommit());
}

#[test]
fn both_audit_projections_match_frozen_types_conflicting_errors_and_read_only_behavior() {
    let mut cases = 0;
    for origin in ORIGINS {
        for kind in 0..17 {
            let journal = fixture(origin);
            defect(&journal.connection, kind);
            compare_projections(&journal.connection);
            cases += 2;
        }
        for first in [1, 2, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14] {
            for second in [1, 2, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14] {
                let journal = fixture(origin);
                defect(&journal.connection, first);
                defect(&journal.connection, second);
                compare_projections(&journal.connection);
                cases += 2;
            }
        }
    }
    assert_eq!(cases, 1610);
}

#[test]
fn native_lookup_keeps_optional_rows_or_selection_and_fault_contexts_without_writes() {
    let mut cases = 0;
    for origin in ORIGINS {
        for route in 0..4 {
            for broken in 0..9 {
                let mut journal = fixture(origin);
                let mut audit = DebuggerAuditContext {
                    command_id: crate::CommandId::new("audit-1").unwrap(),
                    idempotency_key: crate::IdempotencyKey::new("key-1").unwrap(),
                    principal: crate::Principal::new("operator").unwrap(),
                    origin: parse_command_origin(origin).unwrap(),
                    session_id: "session".into(),
                    expected_revision: crate::Revision(7),
                };
                if route == 1 || route == 2 {
                    audit.command_id = crate::CommandId::new("missing-command").unwrap();
                }
                if route == 2 {
                    audit.idempotency_key = crate::IdempotencyKey::new("missing-key").unwrap();
                }
                if route == 3 {
                    journal.connection.execute("INSERT INTO vm_debugger_audit VALUES ('audit-2','operator','key-2',?1,'other-session',8,43,'effect-2')", [origin]).unwrap();
                    audit.command_id = crate::CommandId::new("audit-2").unwrap();
                }
                match broken {
                    0 => {}
                    1 => defect(&journal.connection, 5),
                    2 => defect(&journal.connection, 4),
                    3 => defect(&journal.connection, 12),
                    4 => defect(&journal.connection, 9),
                    5 => defect(&journal.connection, 16),
                    6 => journal
                        .connection
                        .execute_batch("ALTER TABLE vm_debugger_audit DROP COLUMN session_id;")
                        .unwrap(),
                    7 => defect(&journal.connection, 2),
                    8 => journal
                        .connection
                        .execute_batch("DROP TABLE vm_debugger_audit;")
                        .unwrap(),
                    _ => unreachable!(),
                }
                let command = if route == 1 {
                    "audit-1"
                } else {
                    audit.command_id.as_str()
                };
                let before = journal.connection.total_changes();
                let result = journal.debugger_audit(command);
                assert_eq!(
                    result,
                    frozen_read(&journal.connection, command),
                    "read {route} {broken}"
                );
                if let Err(error) = result {
                    assert_eq!(error.code, "LSV4032");
                    assert!(
                        error
                            .message
                            .starts_with("failed to read debugger cancellation audit:")
                    );
                }
                let transaction = journal.connection.transaction().unwrap();
                let result = load_debugger_audit(&transaction, &audit);
                assert_eq!(
                    result,
                    frozen_load(&transaction, &audit),
                    "replay {route} {broken}"
                );
                if broken == 0 {
                    assert_eq!(result.as_ref().unwrap().is_some(), route != 2);
                }
                if let Err(error) = result {
                    assert_eq!(error.code, "LSV4032");
                    assert!(
                        error
                            .message
                            .starts_with("failed to load debugger cancellation audit:")
                    );
                }
                drop(transaction);
                assert_eq!(journal.connection.total_changes(), before);
                assert!(journal.connection.is_autocommit());
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 180);
}

// Frozen released projection and lookup bodies from 49648886. SQLite row reads
// and native fault formatting are shared, not the extracted projection policy.
fn parse_command_origin(value: &str) -> rusqlite::Result<leselang_host_contract::CommandOrigin> {
    use leselang_host_contract::CommandOrigin;
    Ok(match value {
        "gui" => CommandOrigin::Gui,
        "cli" => CommandOrigin::Cli,
        "leselang" => CommandOrigin::Leselang,
        "model" => CommandOrigin::Model,
        "compatibility_adapter" => CommandOrigin::CompatibilityAdapter,
        _ => return Err(rusqlite::Error::InvalidQuery),
    })
}
fn frozen_decode_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<DebuggerAuditRecord> {
    let command_id = crate::CommandId::new(row.get::<_, String>(0)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let principal_id = row.get::<_, String>(1)?;
    let origin = parse_command_origin(&row.get::<_, String>(2)?)?;
    let session_id = row.get::<_, String>(3)?;
    let expected_revision =
        u64::try_from(row.get::<_, i64>(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let observed_at_ms =
        u64::try_from(row.get::<_, i64>(5)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(DebuggerAuditRecord {
        command_id,
        principal_id,
        origin,
        session_id,
        expected_revision: crate::Revision(expected_revision),
        observed_at_ms,
    })
}
fn frozen_decode_replay(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<(String, String, String, DebuggerAuditRecord)> {
    let command_id = row.get::<_, String>(0)?;
    let token = row.get::<_, String>(1)?;
    let idempotency_key = row.get::<_, String>(2)?;
    let record = DebuggerAuditRecord {
        command_id: crate::CommandId::new(command_id.clone())
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        principal_id: row.get(3)?,
        origin: parse_command_origin(&row.get::<_, String>(4)?)?,
        session_id: row.get(5)?,
        expected_revision: crate::Revision(
            u64::try_from(row.get::<_, i64>(6)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ),
        observed_at_ms: u64::try_from(row.get::<_, i64>(7)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
    };
    Ok((command_id, token, idempotency_key, record))
}
fn frozen_load(
    transaction: &rusqlite::Transaction<'_>,
    audit: &DebuggerAuditContext,
) -> Result<Option<(String, String, String, DebuggerAuditRecord)>, Fault> {
    transaction
        .query_row(
            "SELECT command_id, continuation_token, idempotency_key,
                    principal_id, origin, session_id, expected_revision, observed_at_ms
             FROM vm_debugger_audit
             WHERE command_id = ?1 OR (principal_id = ?2 AND idempotency_key = ?3)",
            params![
                audit.command_id.as_str(),
                audit.principal.id.as_str(),
                audit.idempotency_key.as_str(),
            ],
            frozen_decode_replay,
        )
        .optional()
        .map_err(|error| {
            journal_error(
                "LSV4032",
                "failed to load debugger cancellation audit",
                error,
            )
        })
}
fn frozen_read(
    connection: &Connection,
    command_id: &str,
) -> Result<Option<DebuggerAuditRecord>, Fault> {
    connection
        .query_row(
            "SELECT command_id, principal_id, origin, session_id,
                        expected_revision, observed_at_ms
                 FROM vm_debugger_audit WHERE command_id = ?1",
            [command_id],
            frozen_decode_record,
        )
        .optional()
        .map_err(|error| {
            journal_error(
                "LSV4032",
                "failed to read debugger cancellation audit",
                error,
            )
        })
}
