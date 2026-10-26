use super::group_recovery_contract_tests::Fixture;
use super::*;

const SOURCES: [&str; 4] = [
    r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
    r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
];
fn snapshot(c: &Connection) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
    let tables = c
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    tables
        .into_iter()
        .map(|name| {
            let mut q = c
                .prepare(&format!("SELECT * FROM {name} ORDER BY rowid"))
                .unwrap();
            let n = q.column_count();
            let rows = q
                .query_map([], |r| {
                    (0..n)
                        .map(|i| r.get(i))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            (name, rows)
        })
        .collect()
}
fn group_mutation(c: &Connection, kind: usize) {
    c.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
        .unwrap();
    match kind {
        0 => {}
        1 => c.execute_batch("UPDATE vm_merge_groups SET plan=x'7b';").unwrap(),
        2 => c.execute_batch(r#"UPDATE vm_merge_groups SET plan=CAST('{"order":"parallel","branches":[],"result_binding":null}' AS BLOB);"#).unwrap(),
        3 => c.execute_batch("UPDATE vm_merge_groups SET terminal_step=NULL;").unwrap(),
        4 => c.execute_batch("UPDATE vm_effects SET terminal_step=NULL;").unwrap(),
        5 => c.execute_batch("DELETE FROM vm_merge_branches WHERE position=0;").unwrap(),
        6 => c.execute_batch("UPDATE vm_effects SET state='pending' WHERE token=(SELECT branch_token FROM vm_merge_branches WHERE position=0);").unwrap(),
        7 => c.execute_batch("UPDATE vm_merge_branches SET position=position+100;").unwrap(),
        8 => c.execute_batch("DELETE FROM vm_dispatches;").unwrap(),
        9 => c.execute_batch("UPDATE vm_dispatches SET last_error=x'7b';").unwrap(),
        10 => c.execute_batch("CREATE TRIGGER refuse_branch BEFORE DELETE ON vm_effects WHEN OLD.token=(SELECT MAX(token) FROM vm_effects) BEGIN SELECT RAISE(ABORT,'last branch'); END;").unwrap(),
        11 => c.execute_batch("CREATE TRIGGER refuse_group BEFORE DELETE ON vm_merge_groups BEGIN SELECT RAISE(ABORT,'group'); END;").unwrap(),
        12 => c.execute_batch("CREATE TRIGGER skip_group BEFORE DELETE ON vm_merge_groups BEGIN SELECT RAISE(IGNORE); END;").unwrap(),
        13 => c.execute_batch("CREATE TRIGGER skip_branch BEFORE DELETE ON vm_effects WHEN OLD.token=(SELECT MAX(token) FROM vm_effects) BEGIN SELECT RAISE(IGNORE); END;").unwrap(),
        14 => c.execute_batch("UPDATE vm_merge_groups SET plan=x'7b'; DROP TABLE vm_dispatches;").unwrap(),
        15 => c.execute_batch("UPDATE vm_merge_groups SET plan=x'7b'; UPDATE vm_effects SET terminal_step=NULL;").unwrap(),
        _ => unreachable!(),
    }
    c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
        .unwrap();
}
#[test]
fn selected_group_cleanup_matches_frozen_errors_mutation_prefixes_and_outer_rollback() {
    let mut cases = 0;
    for source in SOURCES {
        for stage in 0..3 {
            for kind in 0..16 {
                let old_file = Fixture::new(source, stage);
                let new_file = Fixture::new(source, stage);
                let mut old = SqliteJournal::open(&old_file.0).unwrap();
                let mut new = SqliteJournal::open(&new_file.0).unwrap();
                let group: String = old
                    .connection
                    .query_row("SELECT token FROM vm_merge_groups", [], |r| r.get(0))
                    .unwrap();
                group_mutation(&old.connection, kind);
                group_mutation(&new.connection, kind);
                let before = snapshot(&new.connection);
                let old_tx = old
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .unwrap();
                let new_tx = new
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .unwrap();
                let expected = frozen_delete_completed_merge_group(&old_tx, &group);
                let actual = delete_completed_merge_group(&new_tx, &group);
                assert_eq!(actual, expected, "{source} stage={stage} kind={kind}");
                assert_eq!(snapshot(&new_tx), snapshot(&old_tx));
                assert!(!new_tx.is_autocommit());
                old_tx.rollback().unwrap();
                new_tx.rollback().unwrap();
                assert_eq!(snapshot(&new.connection), before);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 192);
}

fn read_mutation(c: &Connection, kind: usize) {
    c.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
        .unwrap();
    match kind {
        0 => {}
        1 => c.execute_batch("DELETE FROM vm_dispatches;").unwrap(),
        2 => c
            .execute_batch("UPDATE vm_dispatches SET last_error=x'7b';")
            .unwrap(),
        3 => c
            .execute_batch("UPDATE vm_effects SET image=x'7b';")
            .unwrap(),
        4 => c
            .execute_batch("UPDATE vm_effects SET terminal_step=NULL;")
            .unwrap(),
        5 => c.execute_batch("DROP TABLE vm_effects;").unwrap(),
        6 => c
            .execute_batch("UPDATE vm_effects SET state='pending';")
            .unwrap(),
        7 => c
            .execute_batch(
                "DELETE FROM vm_effects WHERE token=(SELECT MIN(token) FROM vm_effects);",
            )
            .unwrap(),
        _ => unreachable!(),
    }
    c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
        .unwrap();
}
#[test]
fn effect_size_and_stale_cache_projection_match_frozen_without_mutation_or_extra_admission() {
    let mut bytes_cases = 0;
    let mut cache_cases = 0;
    for source in SOURCES {
        for stage in 0..3 {
            for kind in 0..8 {
                let f = Fixture::new(source, stage);
                let j = SqliteJournal::open(&f.0).unwrap();
                let c = &j.connection;
                let tokens = c
                    .prepare("SELECT token FROM vm_effects ORDER BY rowid LIMIT 2")
                    .unwrap()
                    .query_map([], |r| r.get::<_, String>(0))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                let step = Step::Done(crate::Value::UiFocus {
                    node_id: String::new(),
                });
                let local = [tokens[0].as_str(), "missing", &tokens[0], &tokens[1], ""]
                    .map(|t| (ContinuationToken::from_wire(t.into()), step.clone()));
                let local_before = serde_json::to_vec(&local).unwrap();
                read_mutation(c, kind);
                let before = snapshot(c);
                let changes = c.total_changes();
                for token in [&tokens[0], &tokens[1], "missing"] {
                    assert_eq!(
                        completed_effect_logical_bytes(c, token),
                        frozen_completed_effect_logical_bytes(c, token),
                        "{source} stage={stage} kind={kind}"
                    );
                    bytes_cases += 1;
                }
                assert_eq!(
                    stale_local_completed(c, &local),
                    frozen_stale_local_completed(c, &local)
                );
                assert_eq!(snapshot(c), before);
                assert_eq!(c.total_changes(), changes);
                assert!(c.is_autocommit());
                assert_eq!(serde_json::to_vec(&local).unwrap(), local_before);
                cache_cases += 1;
            }
        }
    }
    assert_eq!(bytes_cases, 288);
    assert_eq!(cache_cases, 96);
}
#[test]
fn cleanup_borrows_uncommitted_state_and_never_rolls_back_or_commits_for_the_caller() {
    let f = Fixture::new(SOURCES[0], 2);
    let mut j = SqliteJournal::open(&f.0).unwrap();
    let group: String = j
        .connection
        .query_row("SELECT token FROM vm_merge_groups", [], |r| r.get(0))
        .unwrap();
    let before = snapshot(&j.connection);
    let tx = j
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute_batch("SAVEPOINT fixture; DELETE FROM vm_merge_branches WHERE position=0;")
        .unwrap();
    assert_eq!(
        delete_completed_merge_group(&tx, &group)
            .unwrap_err()
            .message,
        "completed merge group has an incomplete branch set"
    );
    tx.execute_batch("ROLLBACK TO fixture; RELEASE fixture; CREATE TEMP TRIGGER fail_last BEFORE DELETE ON vm_effects WHEN OLD.token=(SELECT MAX(token) FROM vm_effects) BEGIN SELECT RAISE(ABORT,'last branch'); END;").unwrap();
    assert!(
        delete_completed_merge_group(&tx, &group)
            .unwrap_err()
            .message
            .contains("failed to delete merge branch")
    );
    assert!(!tx.is_autocommit());
    assert_eq!(
        tx.query_row("SELECT COUNT(*) FROM vm_merge_groups", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        tx.query_row("SELECT COUNT(*) FROM vm_effects", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    tx.rollback().unwrap();
    assert_eq!(snapshot(&j.connection), before);
}

// Frozen at 63df912e, before compaction-record extraction; no migrated cleanup policy is shared.
#[cfg(feature = "sqlite-journal")]
pub(super) fn frozen_completed_effect_logical_bytes(
    connection: &Connection,
    token: &str,
) -> Result<usize, Fault> {
    let bytes: i64 = connection
        .query_row(
            "SELECT length(e.image) + length(e.terminal_step) +
                    COALESCE(length(d.request), 0) +
                    COALESCE(length(d.last_error), 0)
             FROM vm_effects e
             LEFT JOIN vm_dispatches d ON d.token = e.token
             WHERE e.token = ?1 AND e.state = 'completed'",
            [token],
            |row| row.get(0),
        )
        .map_err(|error| {
            journal_error("LSV4030", "failed to size completed journal record", error)
        })?;
    usize::try_from(bytes)
        .map_err(|_| journal_fault("LSV4007", "journal compaction size is invalid"))
}

#[cfg(feature = "sqlite-journal")]
pub(super) fn frozen_delete_completed_merge_group(
    transaction: &rusqlite::Transaction<'_>,
    group_token: &str,
) -> Result<usize, Fault> {
    let (group_bytes, plan_bytes): (i64, Vec<u8>) = transaction
        .query_row(
            "SELECT length(plan) + length(terminal_step), plan
             FROM vm_merge_groups WHERE token = ?1 AND state = 'completed'",
            [group_token],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| journal_error("LSV4030", "failed to size completed merge group", error))?;
    let branches = {
        let mut statement = transaction
            .prepare(
                "SELECT e.token,
                        length(e.image) + length(e.terminal_step) +
                        COALESCE(length(d.request), 0) +
                        COALESCE(length(d.last_error), 0),
                        e.state
                 FROM vm_merge_branches b
                 JOIN vm_effects e ON e.token = b.branch_token
                 LEFT JOIN vm_dispatches d ON d.token = e.token
                 WHERE b.group_token = ?1
                 ORDER BY b.position ASC",
            )
            .map_err(|error| {
                journal_error("LSV4030", "failed to prepare merge group deletion", error)
            })?;
        statement
            .query_map([group_token], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| {
                journal_error("LSV4030", "failed to scan merge group deletion", error)
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                journal_error("LSV4030", "failed to read merge group deletion", error)
            })?
    };
    let plan: MergePlan = decode_bounded(&plan_bytes, MAX_CONTINUATION_BYTES)?;
    validate_merge_plan(&plan)?;
    if branches.len() != plan.branches.len()
        || branches.iter().any(|(_, _, state)| state != "completed")
    {
        return Err(journal_fault(
            "LSV4030",
            "completed merge group has an incomplete branch set",
        ));
    }

    let mut reclaimed = usize::try_from(group_bytes)
        .map_err(|_| journal_fault("LSV4007", "merge group size is invalid"))?;
    for (_, bytes, _) in &branches {
        reclaimed = reclaimed.saturating_add(
            usize::try_from(*bytes)
                .map_err(|_| journal_fault("LSV4007", "merge branch size is invalid"))?,
        );
    }
    let deleted = transaction
        .execute(
            "DELETE FROM vm_merge_groups WHERE token = ?1 AND state = 'completed'",
            [group_token],
        )
        .map_err(|error| {
            journal_error("LSV4030", "failed to delete completed merge group", error)
        })?;
    if deleted != 1 {
        return Err(journal_fault(
            "LSV4030",
            "completed merge group changed during compaction",
        ));
    }
    for (token, _, _) in branches {
        let deleted = transaction
            .execute(
                "DELETE FROM vm_effects WHERE token = ?1 AND state = 'completed'",
                [&token],
            )
            .map_err(|error| journal_error("LSV4030", "failed to delete merge branch", error))?;
        if deleted != 1 {
            return Err(journal_fault(
                "LSV4030",
                "merge branch changed during compaction",
            ));
        }
    }
    Ok(reclaimed)
}

#[cfg(feature = "sqlite-journal")]
pub(super) fn frozen_stale_local_completed(
    connection: &Connection,
    local_completed: &[(ContinuationToken, Step)],
) -> Result<Vec<ContinuationToken>, Fault> {
    let mut statement = connection
        .prepare("SELECT token FROM vm_effects WHERE state = 'completed'")
        .map_err(|error| {
            journal_error(
                "LSV4030",
                "failed to prepare completed journal reconciliation",
                error,
            )
        })?;
    let persisted = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| {
            journal_error(
                "LSV4030",
                "failed to scan completed journal reconciliation",
                error,
            )
        })?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(|error| {
            journal_error(
                "LSV4030",
                "failed to read completed journal reconciliation",
                error,
            )
        })?;
    Ok(local_completed
        .iter()
        .filter(|(token, _)| !persisted.contains(token.as_str()))
        .map(|(token, _)| token.clone())
        .collect())
}
