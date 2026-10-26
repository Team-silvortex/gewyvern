use super::*;

fn marker() -> Step {
    Step::Fault(Fault {
        code: "LSV1001".into(),
        message: "original-local-result".into(),
    })
}

fn start_source(vm: &mut Vm, source: &str, timed: bool) -> Vec<EffectRequest> {
    let program =
        leselang_hir::lower(&leselang_syntax::parse(&format!("fn main() = {source}"))).unwrap();
    let principal = Principal::new("operator").unwrap();
    let capabilities = CapabilitySet::new([CAPABILITY_UI_PRESENTATION]);
    let step = if timed {
        vm.start_timed(&program, principal, capabilities, None, 0, 10)
    } else {
        vm.start(&program, principal, capabilities, None)
    };
    match step {
        Step::Effect(request) => vec![*request],
        Step::Effects(batch) => batch.branches.into_iter().map(|b| b.request).collect(),
        other => panic!("effect expected: {other:?}"),
    }
}

fn complete(vm: &mut Vm, request: &EffectRequest) {
    let winner = vm
        .journal
        .record_completed(&request.continuation, &marker())
        .unwrap();
    vm.pending.remove(&request.continuation.token);
    vm.completed
        .insert(request.continuation.token.clone(), winner);
}

fn retention_fixture(vm: &mut Vm) -> Vec<EffectRequest> {
    let mut requests = Vec::new();
    for _ in 0..3 {
        let request = start_source(vm, r#"ui.focus(node_id: "one")"#, false).remove(0);
        complete(vm, &request);
        requests.push(request);
    }
    let completed_group = start_source(
        vm,
        r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        false,
    );
    for request in &completed_group {
        complete(vm, request);
    }
    requests.extend(completed_group);
    let live_group = start_source(
        vm,
        r#"all(a: ui.focus(node_id: "live-a"), b: ui.focus(node_id: "live-b"))"#,
        false,
    );
    complete(vm, &live_group[0]);
    requests.extend(live_group);
    let newer = start_source(vm, r#"ui.focus(node_id: "newer")"#, false).remove(0);
    complete(vm, &newer);
    requests.push(newer);
    requests.extend(start_source(vm, r#"ui.focus(node_id: "pending")"#, false));
    assert_eq!(requests.len(), 9);
    requests
}

fn observation(vm: &Vm, token: &ContinuationToken) -> (u8, Option<Step>) {
    match vm.journal.observe_effect(token).unwrap() {
        EffectObservation::Untracked => (0, None),
        EffectObservation::Missing => (1, None),
        EffectObservation::Pending => (2, None),
        EffectObservation::Completed(step) => (3, Some(*step)),
    }
}

fn assert_state(actual: &Vm, expected: &Vm, requests: &[EffectRequest]) {
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(
        actual.scheduler_pressure(12),
        expected.scheduler_pressure(12)
    );
    for request in requests {
        assert_eq!(
            observation(actual, &request.continuation.token),
            observation(expected, &request.continuation.token)
        );
    }
}

#[test]
fn maintenance_retention_matches_frozen_memory_bounds_groups_and_sparse_caches() {
    let mut comparisons = 0;
    for retained in [0, 1, 2, 4, 10_000, 10_001, usize::MAX] {
        for batch in [0, 1, 2, 1_000, 1_001, usize::MAX] {
            for sparse in [false, true] {
                let mut actual = Vm::new(100);
                let mut expected = Vm::new(100);
                let requests = retention_fixture(&mut actual);
                assert_eq!(requests, retention_fixture(&mut expected));
                if sparse {
                    actual.completed.remove(&requests[3].continuation.token);
                    expected.completed.remove(&requests[3].continuation.token);
                }
                let pending = actual.pending.clone();
                let policy = RetentionPolicy {
                    max_completed_records: retained,
                    max_delete_per_run: batch,
                };
                let want = frozen_compact_journal(&mut expected, &policy);
                let got = actual.compact_journal(&policy);
                assert_eq!(got, want);
                assert_eq!(
                    serde_json::to_vec(&got).unwrap(),
                    serde_json::to_vec(&want).unwrap()
                );
                assert_state(&actual, &expected, &requests);
                assert_eq!(actual.pending, pending);
                assert!(
                    actual
                        .completed
                        .contains_key(&requests[5].continuation.token)
                );
                assert_eq!(observation(&actual, &requests[6].continuation.token).0, 2);
                if retained == 1 && batch == 1_000 {
                    let report = got.unwrap();
                    assert_eq!((report.removed_records, report.remaining_completed), (4, 1));
                    for request in &requests[3..5] {
                        assert!(!actual.completed.contains_key(&request.continuation.token));
                        assert_eq!(observation(&actual, &request.continuation.token).0, 0);
                    }
                }
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 84);
}

#[test]
fn maintenance_expiration_matches_frozen_memory_for_seventy_operations_and_six_states() {
    let mut comparisons = 0;
    for (original, _) in crate::reply_contract_tests::cases() {
        for state in 0..6 {
            let mut request = original.clone();
            request.continuation.deadline_at_ms = match state {
                0 => None,
                1 => Some(13),
                3 => Some(1),
                _ => Some(12),
            };
            request.budget = request.continuation.resource_budget();
            let mut actual = Vm::new(100);
            let mut expected = Vm::new(100);
            for vm in [&mut actual, &mut expected] {
                vm.restore_request(request.clone()).unwrap();
                if state == 4 {
                    complete(vm, &request);
                }
                if matches!(state, 2 | 3) {
                    vm.completed
                        .insert(request.continuation.token.clone(), marker());
                }
            }
            let now = if state == 5 { u64::MAX } else { 12 };
            let want = frozen_expire_due(&mut expected, now);
            let got = actual.expire_due(now);
            assert_eq!(got, want);
            assert_state(&actual, &expected, std::slice::from_ref(&request));
            assert_eq!(got.is_err(), state == 5);
            if matches!(state, 2 | 3) {
                assert_eq!(
                    actual.completed.get(&request.continuation.token),
                    Some(&Step::Cancelled(Cancellation {
                        continuation: request.continuation.token.clone(),
                        reason: CancellationReason::DeadlineExceeded,
                        observed_at_ms: 12,
                    }))
                );
                assert!(!actual.pending.contains_key(&request.continuation.token));
            }
            assert_eq!(
                actual.expire_due(now),
                frozen_expire_due(&mut expected, now)
            );
            assert_state(&actual, &expected, std::slice::from_ref(&request));
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 420);
}

#[cfg(feature = "sqlite-journal")]
mod durable {
    use super::*;
    use rusqlite::{Connection, types::Value as SqlValue};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "leselang-maintenance-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
        fn db(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn snapshot(path: &Path) -> Vec<(String, Vec<Vec<SqlValue>>)> {
        let db = Connection::open(path).unwrap();
        let tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vm_%' ORDER BY name").unwrap().query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        tables
            .into_iter()
            .map(|table| {
                let mut stmt = db
                    .prepare(&format!(
                        "SELECT * FROM \"{}\" ORDER BY rowid",
                        table.replace('"', "\"\"")
                    ))
                    .unwrap();
                let columns = stmt.column_count();
                let rows = stmt
                    .query_map([], |row| {
                        (0..columns)
                            .map(|i| row.get(i))
                            .collect::<Result<Vec<SqlValue>, _>>()
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                (table, rows)
            })
            .collect()
    }

    #[test]
    fn maintenance_retention_sqlite_preserves_peer_pruning_reports_rollback_and_restart() {
        let mut scenarios = 0;
        for batch in [1, 4] {
            for state in 0..6 {
                let scratch = Scratch::new();
                let a = scratch.db("actual.sqlite3");
                let e = scratch.db("expected.sqlite3");
                let mut actual = Vm::open_journal(&a, 100).unwrap();
                let mut expected = Vm::open_journal(&e, 100).unwrap();
                let requests = retention_fixture(&mut actual);
                assert_eq!(requests, retention_fixture(&mut expected));
                let policy = RetentionPolicy {
                    max_completed_records: match state {
                        1 | 4 => 10_000,
                        2 => 0,
                        _ => 1,
                    },
                    max_delete_per_run: batch,
                };
                if state == 4 {
                    for path in [&a, &e] {
                        let mut peer = Vm::open_journal(path, 100).unwrap();
                        assert_eq!(
                            peer.compact_journal(&RetentionPolicy {
                                max_completed_records: 1,
                                max_delete_per_run: 10
                            })
                            .unwrap()
                            .removed_records,
                            4
                        );
                    }
                }
                if state == 5 {
                    actual.completed.clear();
                    expected.completed.clear();
                }
                if state == 3 {
                    let failed_token = &requests[if batch == 1 { 0 } else { 4 }].continuation.token;
                    for path in [&a, &e] {
                        Connection::open(path).unwrap().execute_batch(&format!("CREATE TRIGGER refuse_compact BEFORE DELETE ON vm_effects WHEN OLD.token='{}' BEGIN SELECT RAISE(ABORT, 'injected compaction'); END;", failed_token.as_str())).unwrap();
                    }
                }
                let before = snapshot(&a);
                assert_eq!(before, snapshot(&e));
                let pending = actual.pending.clone();
                let completed = actual.completed.clone();
                let want = frozen_compact_journal(&mut expected, &policy);
                let got = actual.compact_journal(&policy);
                assert_eq!(got, want, "state={state}, batch={batch}");
                assert_eq!(
                    serde_json::to_vec(&got).unwrap(),
                    serde_json::to_vec(&want).unwrap()
                );
                assert_state(&actual, &expected, &requests);
                assert_eq!(snapshot(&a), snapshot(&e));
                assert_eq!(actual.pending, pending);
                if matches!(state, 2 | 3) {
                    assert!(got.is_err());
                    assert_eq!(actual.completed, completed);
                    assert_eq!(snapshot(&a), before);
                } else {
                    let report = got.unwrap();
                    if state == 4 {
                        assert_eq!(report.removed_records, 0);
                        assert_eq!(report.reclaimed_logical_bytes, 0);
                        assert!(actual.completed.len() < completed.len());
                    }
                }
                for path in [&a, &e] {
                    Connection::open(path)
                        .unwrap()
                        .execute_batch("DROP TRIGGER IF EXISTS refuse_compact")
                        .unwrap();
                }
                drop((actual, expected));
                let mut actual = Vm::open_journal(&a, 100).unwrap();
                let mut expected = Vm::open_journal(&e, 100).unwrap();
                assert_state(&actual, &expected, &requests);
                assert_eq!(actual.claim_effect(1, 100), expected.claim_effect(1, 100));
                assert_eq!(snapshot(&a), snapshot(&e));
                scenarios += 1;
            }
        }
        assert_eq!(scenarios, 12);
    }

    #[test]
    fn maintenance_expiration_sqlite_keeps_batch_commit_before_cache_and_restart_parity() {
        let mut scenarios = 0;
        for source in [
            r#"ui.focus(node_id: "a")"#,
            r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
            r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
            r#"bind(r: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
            r#"bind(r: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        ] {
            for state in 0..8 {
                let scratch = Scratch::new();
                let a = scratch.db("actual.sqlite3");
                let e = scratch.db("expected.sqlite3");
                let mut actual = Vm::open_journal(&a, 100).unwrap();
                let mut expected = Vm::open_journal(&e, 100).unwrap();
                let mut requests = start_source(&mut actual, source, true);
                assert_eq!(requests, start_source(&mut expected, source, true));
                let last = start_source(&mut actual, r#"ui.focus(node_id: "last")"#, true);
                assert_eq!(
                    last,
                    start_source(&mut expected, r#"ui.focus(node_id: "last")"#, true)
                );
                requests.extend(last);
                if state == 4 {
                    for path in [&a, &e] {
                        Vm::open_journal(path, 100).unwrap().expire_due(10).unwrap();
                    }
                }
                if matches!(state, 5 | 6) {
                    let table = if state == 5 {
                        "vm_effects"
                    } else {
                        "vm_merge_groups"
                    };
                    for path in [&a, &e] {
                        Connection::open(path).unwrap().execute_batch(&format!("CREATE TRIGGER refuse_expire BEFORE UPDATE ON {table} BEGIN SELECT RAISE(ABORT, 'injected expiry'); END;")).unwrap();
                    }
                }
                let mut saved_image = None;
                if state == 7 {
                    for path in [&a, &e] {
                        let db = Connection::open(path).unwrap();
                        let (token, bytes): (String, Vec<u8>) = db
                            .query_row(
                                "SELECT token,image FROM vm_effects ORDER BY token DESC LIMIT 1",
                                [],
                                |row| Ok((row.get(0)?, row.get(1)?)),
                            )
                            .unwrap();
                        db.execute("UPDATE vm_effects SET image=X'00' WHERE token=?1", [&token])
                            .unwrap();
                        saved_image = Some((token, bytes));
                    }
                }
                let now = match state {
                    0 => 9,
                    2 => 20,
                    3 => u64::MAX,
                    _ => 10,
                };
                let before = snapshot(&a);
                assert_eq!(before, snapshot(&e));
                let caches = (actual.pending.clone(), actual.completed.clone());
                let want = frozen_expire_due(&mut expected, now);
                let got = actual.expire_due(now);
                assert_eq!(got, want, "{source}, state={state}");
                assert_state(&actual, &expected, &requests);
                assert_eq!(snapshot(&a), snapshot(&e));
                let failure = matches!(state, 3 | 5 | 7)
                    || (state == 6 && source != r#"ui.focus(node_id: "a")"#);
                assert_eq!(got.is_err(), failure);
                if failure || matches!(state, 0 | 4) {
                    assert_eq!((actual.pending.clone(), actual.completed.clone()), caches);
                    assert_eq!(snapshot(&a), before);
                } else {
                    assert!(actual.pending.is_empty());
                    assert!(!actual.completed.is_empty());
                }
                for path in [&a, &e] {
                    let db = Connection::open(path).unwrap();
                    db.execute_batch("DROP TRIGGER IF EXISTS refuse_expire")
                        .unwrap();
                    if let Some((token, bytes)) = &saved_image {
                        db.execute(
                            "UPDATE vm_effects SET image=?1 WHERE token=?2",
                            rusqlite::params![bytes, token],
                        )
                        .unwrap();
                    }
                }
                drop((actual, expected));
                let mut actual = Vm::open_journal(&a, 100).unwrap();
                let mut expected = Vm::open_journal(&e, 100).unwrap();
                assert_eq!(actual.expire_due(30), frozen_expire_due(&mut expected, 30));
                assert_state(&actual, &expected, &requests);
                assert_eq!(snapshot(&a), snapshot(&e));
                assert!(actual.claim_effect(31, 100).unwrap().is_none());
                assert_eq!(actual.pending_count(), 0);
                scenarios += 1;
            }
        }
        assert_eq!(scenarios, 40);
    }
}

// Frozen pre-extraction coordination; native journal implementations remain shared.
fn frozen_compact_journal(
    vm: &mut Vm,
    policy: &RetentionPolicy,
) -> Result<CompactionReport, Fault> {
    validate_retention_policy(policy)?;
    let mut local_completed = vm
        .completed
        .iter()
        .map(|(token, step)| (token.clone(), step.clone()))
        .collect::<Vec<_>>();
    local_completed.sort_by(|(left, _), (right, _)| continuation_age_order(left, right));
    let compacted = vm.journal.compact_completed(&local_completed, policy)?;
    for token in &compacted.pruned_tokens {
        vm.completed.remove(token);
    }
    Ok(CompactionReport {
        removed_records: compacted.removed_records,
        remaining_completed: compacted.remaining_completed,
        reclaimed_logical_bytes: compacted.reclaimed_logical_bytes,
    })
}

fn frozen_expire_due(vm: &mut Vm, now_ms: u64) -> Result<(), Fault> {
    validate_scheduler_time(now_ms)?;
    let expired = vm.journal.expire_due(now_ms)?;
    for (image, step) in expired {
        vm.pending.remove(&image.token);
        vm.completed.insert(image.token, step);
    }
    Ok(())
}
