use super::group_recovery_contract_tests::{Fixture, snapshot};
use super::*;

#[test]
fn dispatch_storage_field_errors_preserve_original_read_order_and_cold_lease() {
    let mut cases = 0;
    for stage in [0, 2] {
        let fixture = Fixture::new(r#"fn main() = ui.focus(node_id: "a")"#, stage);
        let connection = Connection::open(&fixture.0).unwrap();
        let before = snapshot(&connection);
        for mask in 0..1024 {
            let fields = [
                "d.token",
                "d.request",
                "d.state",
                "d.attempt",
                "d.lease_expires_at_ms",
                "d.ready_at_ms",
                "d.retry_count",
                "d.last_error",
                "e.state",
                "e.image",
            ]
            .into_iter()
            .enumerate()
            .map(|(index, field)| {
                if mask & (1 << index) == 0 {
                    field.to_string()
                } else if [3, 4, 5, 6, 7].contains(&index) {
                    "'wrong-type'".into()
                } else {
                    "NULL".into()
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
            let mut statement = connection.prepare(&format!(
                "SELECT {fields} FROM vm_dispatches d JOIN vm_effects e ON e.token = d.token ORDER BY d.token LIMIT 1"
            )).unwrap();
            let mut rows = statement.query([]).unwrap();
            let row = rows.next().unwrap().unwrap();
            let expected = frozen_row(row);
            assert_eq!(expected.is_ok(), mask == 0 || mask == 16);
            assert_eq!(
                recover_dispatch_record(row),
                expected,
                "stage {stage}, mask {mask}"
            );
            cases += 1;
        }
        assert_eq!(snapshot(&connection), before);
    }
    assert_eq!(cases, 2048);
}

#[test]
fn stored_dispatch_recovery_preserves_frozen_order_and_original_tables() {
    let sources = [
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: 1)"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: 1)"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(a: ui.focus(node_id: "a"), body: ui.focus(node_id: field(value: a, name: "node_id")))"#,
        r#"fn main() = bind(a: ui.focus(node_id: "a"), body: bind(b: ui.focus(node_id: field(value: a, name: "node_id")), body: ui.focus(node_id: field(value: b, name: "node_id"))))"#,
    ];
    let valid_error = serde_json::to_vec(&EffectError {
        class: crate::EffectErrorClass::Transient,
        code: "original_retry".into(),
        message: "original retry".into(),
    })
    .unwrap();
    let mut records = 0;
    let mut comparisons = 0;
    for source in sources {
        for stage in 0..3 {
            let fixture = Fixture::new(source, stage);
            let mut connection = Connection::open(&fixture.0).unwrap();
            connection
                .pragma_update(None, "ignore_check_constraints", true)
                .unwrap();
            let ids = connection
                .prepare("SELECT rowid FROM vm_dispatches ORDER BY token")
                .unwrap()
                .query_map([], |row| row.get::<_, i64>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            for id in ids {
                records += 1;
                for defect in 0..24 {
                    let tx = connection.transaction().unwrap();
                    let sql = match defect {
                        0 => None,
                        1 => Some("UPDATE vm_dispatches SET request = x'7b' WHERE rowid = ?1"),
                        2 => Some(
                            "UPDATE vm_effects SET image = x'7b' WHERE token = (SELECT token FROM vm_dispatches WHERE rowid = ?1)",
                        ),
                        3 => Some("UPDATE vm_dispatches SET attempt = -1 WHERE rowid = ?1"),
                        4 => Some(
                            "UPDATE vm_dispatches SET attempt = 9223372036854775807 WHERE rowid = ?1",
                        ),
                        5 => Some("UPDATE vm_dispatches SET ready_at_ms = -1 WHERE rowid = ?1"),
                        6 => Some("UPDATE vm_dispatches SET retry_count = -1 WHERE rowid = ?1"),
                        7 => Some(
                            "UPDATE vm_dispatches SET retry_count = 9223372036854775807 WHERE rowid = ?1",
                        ),
                        8 => Some(
                            "UPDATE vm_dispatches SET retry_count = 1, last_error = NULL WHERE rowid = ?1",
                        ),
                        9 => Some(
                            "UPDATE vm_dispatches SET retry_count = 0, last_error = x'7b' WHERE rowid = ?1",
                        ),
                        10 => Some(
                            "UPDATE vm_dispatches SET retry_count = 1, last_error = x'7b' WHERE rowid = ?1",
                        ),
                        11 => Some(
                            "UPDATE vm_dispatches SET state = CASE state WHEN 'acknowledged' THEN 'queued' ELSE 'acknowledged' END WHERE rowid = ?1",
                        ),
                        12 => Some(
                            "UPDATE vm_dispatches SET attempt = 0, ready_at_ms = 0, retry_count = 0, last_error = NULL WHERE rowid = ?1",
                        ),
                        13 => Some(
                            "UPDATE vm_dispatches SET ready_at_ms = 9223372036854775807 WHERE rowid = ?1",
                        ),
                        14 => Some(
                            "UPDATE vm_dispatches SET request = x'7b', attempt = -1 WHERE rowid = ?1",
                        ),
                        15 => Some(
                            "UPDATE vm_dispatches SET attempt = -1, ready_at_ms = -1, last_error = x'7b' WHERE rowid = ?1",
                        ),
                        16 => Some(
                            "UPDATE vm_dispatches SET ready_at_ms = -1, retry_count = 1, last_error = NULL WHERE rowid = ?1",
                        ),
                        17 => Some(
                            "UPDATE vm_dispatches SET retry_count = 0, last_error = x'7b', state = CASE state WHEN 'acknowledged' THEN 'queued' ELSE 'acknowledged' END WHERE rowid = ?1",
                        ),
                        18 => Some(
                            "UPDATE vm_dispatches SET retry_count = 1, last_error = x'7b', state = CASE state WHEN 'acknowledged' THEN 'queued' ELSE 'acknowledged' END WHERE rowid = ?1",
                        ),
                        19 => {
                            tx.execute(
                                "UPDATE vm_dispatches SET attempt = ?2 WHERE rowid = ?1",
                                params![id, MAX_DISPATCH_ATTEMPTS],
                            )
                            .unwrap();
                            None
                        }
                        20 | 21 => {
                            let retries = if defect == 20 {
                                1
                            } else {
                                MAX_SEMANTIC_RETRIES
                            };
                            tx.execute("UPDATE vm_dispatches SET retry_count = ?2, last_error = ?3 WHERE rowid = ?1", params![id, retries, valid_error]).unwrap();
                            None
                        }
                        22 | 23 => {
                            let bytes: Vec<u8> = tx
                                .query_row(
                                    "SELECT request FROM vm_dispatches WHERE rowid = ?1",
                                    [id],
                                    |row| row.get(0),
                                )
                                .unwrap();
                            let mut request: EffectRequest =
                                serde_json::from_slice(&bytes).unwrap();
                            request.continuation.token =
                                ContinuationToken::from_wire("continuation-999999".into());
                            tx.execute(
                                "UPDATE vm_dispatches SET request = ?2 WHERE rowid = ?1",
                                params![id, serde_json::to_vec(&request).unwrap()],
                            )
                            .unwrap();
                            if defect == 23 {
                                tx.execute(
                                    "UPDATE vm_dispatches SET attempt = -1 WHERE rowid = ?1",
                                    [id],
                                )
                                .unwrap();
                            }
                            None
                        }
                        _ => unreachable!(),
                    };
                    if let Some(sql) = sql {
                        tx.execute(sql, [id]).unwrap();
                    }
                    let before = snapshot(&tx);
                    let expected = frozen(&tx);
                    if [0, 12, 13, 19, 20, 21].contains(&defect) {
                        assert!(
                            expected.is_ok(),
                            "{source}, {stage}, {id}, {defect}: {expected:?}"
                        );
                    } else {
                        assert!(expected.is_err(), "{defect}");
                    }
                    assert_eq!(
                        validate_dispatch_records(&tx),
                        expected,
                        "{source}, {stage}, {id}, {defect}"
                    );
                    assert_eq!(snapshot(&tx), before);
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(records, 49);
    assert_eq!(comparisons, 1176);
}

#[test]
fn late_dispatch_refusal_blocks_snapshot_and_watermark_until_original_data_is_repaired() {
    for defect in 0..3 {
        let fixture = Fixture::new(
            r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
            0,
        );
        let mut journal = SqliteJournal {
            connection: Connection::open(&fixture.0).unwrap(),
        };
        journal
            .connection
            .pragma_update(None, "ignore_check_constraints", true)
            .unwrap();
        journal
            .connection
            .execute("UPDATE vm_metadata SET next_sequence = 1", [])
            .unwrap();
        let token: String = journal
            .connection
            .query_row(
                "SELECT token FROM vm_dispatches ORDER BY token DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let sql = match defect {
            0 => "UPDATE vm_dispatches SET attempt = -1 WHERE token = ?1",
            1 => "UPDATE vm_dispatches SET retry_count = 1, last_error = NULL WHERE token = ?1",
            _ => "UPDATE vm_dispatches SET state = 'acknowledged' WHERE token = ?1",
        };
        journal.connection.execute(sql, [&token]).unwrap();
        let expected = frozen(&journal.connection).unwrap_err();
        let before = snapshot(&journal.connection);
        assert_eq!(journal.load().err().unwrap(), expected);
        assert_eq!(snapshot(&journal.connection), before);
        journal.connection.execute("UPDATE vm_dispatches SET attempt = 0, retry_count = 0, state = 'ready' WHERE token = ?1", [&token]).unwrap();
        let recovered = journal.load().unwrap();
        assert_eq!(recovered.pending.len(), 2);
        assert!(recovered.completed.is_empty());
        assert!(recovered.next_sequence > 1);
    }
}

fn frozen(connection: &Connection) -> Result<(), Fault> {
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
        frozen_row(row)?;
    }
    Ok(())
}

fn frozen_row(row: &rusqlite::Row<'_>) -> Result<(), Fault> {
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
    let effect_image: Vec<u8> = row
        .get(9)
        .map_err(|error| journal_error("LSV4005", "invalid dispatch continuation image", error))?;
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
    Ok(())
}
