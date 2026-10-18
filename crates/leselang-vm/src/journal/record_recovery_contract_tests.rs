use super::group_recovery_contract_tests::{Fixture, snapshot};
use super::*;
use leselang_runtime_core::RecoveryPayload;

fn comparable(record: RecoveredEffectRecord) -> (ContinuationToken, u64, serde_json::Value) {
    let payload = match record.payload {
        RecoveryPayload::Pending(image) => serde_json::json!({"pending": image}),
        RecoveryPayload::Completed(step) => serde_json::json!({"completed": step}),
    };
    (record.key, record.sequence, payload)
}

#[test]
fn stored_effect_record_preserves_frozen_fault_order_and_original_tables() {
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
    let mut states = 0;
    let mut comparisons = 0;
    for source in sources {
        for stage in 0..3 {
            let fixture = Fixture::new(source, stage);
            let mut connection = Connection::open(&fixture.0).unwrap();
            connection
                .pragma_update(None, "foreign_keys", false)
                .unwrap();
            connection
                .pragma_update(None, "ignore_check_constraints", true)
                .unwrap();
            let ids = connection
                .prepare("SELECT rowid FROM vm_effects ORDER BY token")
                .unwrap()
                .query_map([], |row| row.get::<_, i64>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            for id in ids {
                states += 1;
                for defect in 0..16 {
                    let tx = connection.transaction().unwrap();
                    match defect {
                        0 => {}
                        1 => {
                            tx.execute(
                                "UPDATE vm_effects SET image = ?1 WHERE rowid = ?2",
                                params![b"{".as_slice(), id],
                            )
                            .unwrap();
                        }
                        2 => {
                            tx.execute(
                                "UPDATE vm_effects SET token = 'wrong' WHERE rowid = ?1",
                                [id],
                            )
                            .unwrap();
                        }
                        3 => {
                            tx.execute(
                                "UPDATE vm_effects SET deadline_at_ms = 7 WHERE rowid = ?1",
                                [id],
                            )
                            .unwrap();
                        }
                        4 => {
                            tx.execute(
                                "UPDATE vm_effects SET deadline_at_ms = -1 WHERE rowid = ?1",
                                [id],
                            )
                            .unwrap();
                        }
                        5 => {
                            tx.execute(
                                "UPDATE vm_effects SET state = 'wrong' WHERE rowid = ?1",
                                [id],
                            )
                            .unwrap();
                        }
                        6 => {
                            tx.execute(
                                "UPDATE vm_effects SET terminal_step = ?1 WHERE rowid = ?2",
                                params![b"{".as_slice(), id],
                            )
                            .unwrap();
                        }
                        7 => {
                            tx.execute("UPDATE vm_effects SET state = 'completed', terminal_step = NULL WHERE rowid = ?1", [id]).unwrap();
                        }
                        8 => {
                            tx.execute("UPDATE vm_effects SET state = 'completed', terminal_step = ?1 WHERE rowid = ?2", params![b"{".as_slice(), id]).unwrap();
                        }
                        9 => {
                            tx.execute("UPDATE vm_effects SET state = 'pending', terminal_step = ?1 WHERE rowid = ?2", params![b"{".as_slice(), id]).unwrap();
                        }
                        10 => {
                            tx.execute("UPDATE vm_effects SET deadline_at_ms = -1, state = 'wrong' WHERE rowid = ?1", [id]).unwrap();
                        }
                        11 => {
                            tx.execute("UPDATE vm_effects SET token = 'wrong', deadline_at_ms = -1 WHERE rowid = ?1", [id]).unwrap();
                        }
                        12 => {
                            tx.execute("UPDATE vm_effects SET image = ?1, token = 'wrong', state = 'wrong' WHERE rowid = ?2", params![b"{".as_slice(), id]).unwrap();
                        }
                        13 => {
                            tx.execute("DELETE FROM vm_merge_groups", []).unwrap();
                        }
                        14 => {
                            tx.execute("UPDATE vm_metadata SET next_sequence = 1", [])
                                .unwrap();
                        }
                        15 => {
                            tx.execute("DELETE FROM vm_merge_groups", []).unwrap();
                            tx.execute("UPDATE vm_effects SET state = 'wrong', terminal_step = ?1 WHERE rowid = ?2", params![b"{".as_slice(), id]).unwrap();
                        }
                        _ => unreachable!(),
                    }
                    let before = snapshot(&tx);
                    let mut statement = tx.prepare("SELECT token, state, image, deadline_at_ms, terminal_step FROM vm_effects WHERE rowid = ?1").unwrap();
                    let mut rows = statement.query([id]).unwrap();
                    let row = rows.next().unwrap().unwrap();
                    let expected = frozen(&tx, row).map(comparable);
                    if defect == 0 {
                        assert!(expected.is_ok(), "{source}, {stage}, {id}");
                    }
                    assert_eq!(
                        recover_effect_record(&tx, row).map(comparable),
                        expected,
                        "{source}, {stage}, {id}, {defect}"
                    );
                    assert_eq!(snapshot(&tx), before);
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(states, 49);
    assert_eq!(comparisons, 784);
}

fn frozen(
    connection: &Connection,
    row: &rusqlite::Row<'_>,
) -> Result<RecoveredEffectRecord, Fault> {
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
    let sequence = crate::canonical_continuation_sequence(&image.token)
        .ok_or_else(|| journal_fault("LSV4007", "journal continuation sequence is invalid"))?;
    if image.token.as_str() != token {
        return Err(journal_fault(
            "LSV4007",
            "journal token does not match continuation image",
        ));
    }
    let stored_deadline: Option<i64> = row
        .get(3)
        .map_err(|error| journal_error("LSV4005", "invalid journal effect deadline", error))?;
    let stored_deadline = stored_deadline
        .map(|value| {
            u64::try_from(value).map_err(|_| journal_fault("LSV4007", "effect deadline is invalid"))
        })
        .transpose()?;
    if stored_deadline != image.deadline_at_ms {
        return Err(journal_fault(
            "LSV4007",
            "journal deadline does not match continuation image",
        ));
    }
    group_binding::validate_owner(connection, &image)?;

    let (key, payload) = match state.as_str() {
        "pending" => {
            successor::validate_record(connection, &image, None)?;
            (image.token.clone(), RecoveryPayload::Pending(image))
        }
        "completed" => {
            let step_bytes: Vec<u8> = row.get(4).map_err(|error| {
                journal_error("LSV4005", "invalid completed journal record", error)
            })?;
            let step: Step = decode_bounded(&step_bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            validate_terminal_step(&step)?;
            successor::validate_record(connection, &image, Some(&step))?;
            (image.token, RecoveryPayload::Completed(step))
        }
        _ => return Err(journal_fault("LSV4007", "invalid journal record state")),
    };
    Ok(RecoveryRecord {
        key,
        sequence,
        payload,
    })
}
