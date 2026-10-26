use super::*;

fn marker() -> Step {
    Step::Fault(Fault {
        code: "LSV1001".into(),
        message: "original-cache-value".into(),
    })
}

fn assert_state(actual: &Vm, expected: &Vm) {
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(
        actual.scheduler_pressure(12),
        expected.scheduler_pressure(12)
    );
}

fn compare_cache(actual: &mut Vm, expected: &mut Vm, token: &ContinuationToken) {
    let pointer = actual.completed.get(token).map(std::ptr::from_ref);
    let want = frozen_reconcile_completed(expected, token).map(|s| s.cloned());
    let got = actual.reconcile_completed(token);
    if let (Some(pointer), Ok(Some(cached))) = (pointer, &got) {
        assert_eq!(std::ptr::from_ref(*cached), pointer);
    }
    let got = got.map(|s| s.cloned());
    assert_eq!(got, want);
    assert_eq!(
        serde_json::to_vec(&got).unwrap(),
        serde_json::to_vec(&want).unwrap()
    );
    assert_state(actual, expected);
}

fn compare_visible(
    actual: &mut Vm,
    expected: &mut Vm,
    token: &ContinuationToken,
    original: Step,
) -> Step {
    let want = frozen_visible_step(expected, token, original.clone());
    let got = actual.visible_step(token, original);
    assert_eq!(got, want);
    assert_eq!(
        serde_json::to_vec(&got).unwrap(),
        serde_json::to_vec(&want).unwrap()
    );
    assert_state(actual, expected);
    got
}

#[test]
fn visibility_cache_matches_frozen_memory_for_seventy_operations_and_five_cache_states() {
    let mut comparisons = 0;
    for (request, _) in crate::reply_contract_tests::cases() {
        for state in 0..5 {
            let mut actual = Vm::new(100);
            let mut expected = Vm::new(100);
            for vm in [&mut actual, &mut expected] {
                match state {
                    0 => {}
                    1 => vm.restore(request.continuation.clone()).unwrap(),
                    _ => vm.restore_request(request.clone()).unwrap(),
                }
                if state >= 3 {
                    vm.journal
                        .record_completed(&request.continuation, &marker())
                        .unwrap();
                }
                if state == 4 {
                    // Existence-only checks must not replace this already cached winner.
                    vm.completed.insert(
                        request.continuation.token.clone(),
                        Step::Done(Value::UiFocus {
                            node_id: "cached-not-refreshed".into(),
                        }),
                    );
                }
            }
            compare_cache(&mut actual, &mut expected, &request.continuation.token);
            assert_eq!(
                actual.completed.contains_key(&request.continuation.token),
                state >= 3
            );
            assert_eq!(
                actual.pending.contains_key(&request.continuation.token),
                matches!(state, 1 | 2 | 4)
            );
            compare_cache(&mut actual, &mut expected, &request.continuation.token);
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 350);
}

const SOURCES: [&str; 6] = [
    r#"ui.focus(node_id: "a")"#,
    r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"), c: ui.focus(node_id: "c"))"#,
    r#"bind(r: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
    r#"bind(r: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
];

fn start_source(vm: &mut Vm, source: &str) -> Step {
    let program =
        leselang_hir::lower(&leselang_syntax::parse(&format!("fn main() = {source}"))).unwrap();
    vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
        None,
    )
}

fn terminal(lease: &DispatchLease, outcome: usize) -> Step {
    match outcome {
        0 => {
            let EffectOperation::Presentation(envelope) = &lease.request.operation else {
                panic!()
            };
            let PresentationOperation::Focus { node_id } = &envelope.operation else {
                panic!()
            };
            Step::Done(Value::UiFocus {
                node_id: node_id.clone(),
            })
        }
        1 => Step::Cancelled(Cancellation {
            continuation: lease.request.continuation.token.clone(),
            reason: CancellationReason::Requested,
            observed_at_ms: 2,
        }),
        _ => marker(),
    }
}

fn seed_local_cache(vm: &mut Vm, token: &ContinuationToken, state: usize) {
    match state {
        1 => vm.pending.clear(),
        2 => {
            vm.completed.insert(token.clone(), marker());
        }
        3 => {
            vm.pending.remove(token);
            vm.completed.insert(token.clone(), marker());
        }
        _ => {}
    }
}

#[test]
fn visibility_groups_match_frozen_memory_across_outcomes_and_local_cache_shapes() {
    let mut scenarios = 0;
    let mut seen = [false; 4];
    for source in SOURCES {
        for outcome in 0..3 {
            for cache in 0..4 {
                let mut actual = Vm::new(100);
                let mut expected = Vm::new(100);
                assert_eq!(
                    start_source(&mut actual, source),
                    start_source(&mut expected, source)
                );
                let mut settled = 0;
                for now in [1, 3, 5, 7, 9, 11] {
                    let lease = actual.claim_effect(now, 100).unwrap();
                    assert_eq!(lease, expected.claim_effect(now, 100).unwrap());
                    let Some(lease) = lease else { break };
                    let token = &lease.request.continuation.token;
                    let proposed = terminal(&lease, if settled == 0 { outcome } else { 0 });
                    let winner = actual
                        .journal
                        .acknowledge_dispatch(&lease, now + 1, &proposed)
                        .unwrap();
                    assert_eq!(
                        winner,
                        expected
                            .journal
                            .acknowledge_dispatch(&lease, now + 1, &proposed)
                            .unwrap()
                    );
                    let variant = match actual.journal.merge_progress(token).unwrap() {
                        MergeProgress::Standalone => 0,
                        MergeProgress::Next { .. } => 1,
                        MergeProgress::Pending { .. } => 2,
                        MergeProgress::Completed { .. } => 3,
                    };
                    seen[variant] = true;
                    seed_local_cache(&mut actual, token, cache);
                    seed_local_cache(&mut expected, token, cache);
                    compare_visible(&mut actual, &mut expected, token, winner.clone());
                    compare_visible(&mut actual, &mut expected, token, winner);
                    compare_cache(&mut actual, &mut expected, token);
                    settled += 1;
                }
                assert!((1..6).contains(&settled), "{source}");
                assert!(actual.claim_effect(100, 100).unwrap().is_none());
                assert_state(&actual, &expected);
                scenarios += 1;
            }
        }
    }
    assert_eq!(seen, [true; 4]);
    assert_eq!(scenarios, 72);
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
                "leselang-visibility-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
        fn db(&self) -> std::path::PathBuf {
            self.0.join("journal.sqlite3")
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
    fn visibility_sqlite_reconciles_peer_completion_retention_and_read_failures_without_writes() {
        for state in 0..9 {
            let scratch = Scratch::new();
            let path = scratch.db();
            let mut peer = Vm::open_journal(&path, 100).unwrap();
            let mut requests = Vec::new();
            for _ in 0..3 {
                let Step::Effect(request) = start_source(&mut peer, SOURCES[0]) else {
                    panic!()
                };
                requests.push(request);
            }
            let mut actual = Vm::open_journal(&path, 100).unwrap();
            let mut expected = Vm::open_journal(&path, 100).unwrap();
            let token = &requests[0].continuation.token;
            if state != 0 {
                for request in &requests {
                    peer.journal
                        .record_completed(&request.continuation, &marker())
                        .unwrap();
                }
            }
            if matches!(state, 2 | 4 | 8) {
                actual.completed.insert(token.clone(), marker());
                expected.completed.insert(token.clone(), marker());
            }
            if matches!(state, 3 | 4) {
                assert_eq!(
                    peer.compact_journal(&RetentionPolicy {
                        max_completed_records: 1,
                        max_delete_per_run: 10
                    })
                    .unwrap()
                    .removed_records,
                    2
                );
            }
            let db = Connection::open(&path).unwrap();
            match state {
                5 => {
                    db.execute(
                        "UPDATE vm_effects SET terminal_step = X'00' WHERE token=?1",
                        [token.as_str()],
                    )
                    .unwrap();
                }
                6 => {
                    // Corruption fixture only; production journal constraints remain enabled.
                    db.pragma_update(None, "ignore_check_constraints", true)
                        .unwrap();
                    db.execute(
                        "UPDATE vm_effects SET state='invalid' WHERE token=?1",
                        [token.as_str()],
                    )
                    .unwrap();
                }
                7 | 8 => {
                    db.execute_batch("ALTER TABLE vm_effects RENAME TO missing_effects")
                        .unwrap();
                }
                _ => {}
            }
            let before = snapshot(&path);
            compare_cache(&mut actual, &mut expected, token);
            assert_eq!(snapshot(&path), before);
            if state >= 5 {
                assert!(actual.reconcile_completed(token).is_err());
                assert!(actual.pending.contains_key(token));
            } else {
                assert_eq!(actual.completed.contains_key(token), matches!(state, 1 | 2));
                assert_eq!(
                    actual.pending.contains_key(token),
                    matches!(state, 0 | 2 | 4)
                );
                // Evicted cached completion does not also remove pending until the next read.
                compare_cache(&mut actual, &mut expected, token);
                if state == 4 {
                    assert!(!actual.pending.contains_key(token));
                }
                let reopened = Vm::open_journal(&path, 100).unwrap();
                assert_eq!(
                    reopened.completed.contains_key(token),
                    matches!(state, 1 | 2)
                );
            }
        }
    }

    #[test]
    fn visibility_sqlite_group_read_failures_do_not_mutate_cache_or_storage() {
        for defect in 0..4 {
            let scratch = Scratch::new();
            let path = scratch.db();
            let mut peer = Vm::open_journal(&path, 100).unwrap();
            start_source(&mut peer, SOURCES[1]);
            let mut actual = Vm::open_journal(&path, 100).unwrap();
            let mut expected = Vm::open_journal(&path, 100).unwrap();
            let lease = peer.claim_effect(1, 100).unwrap().unwrap();
            let token = &lease.request.continuation.token;
            peer.journal
                .acknowledge_dispatch(&lease, 2, &terminal(&lease, 0))
                .unwrap();
            if defect == 1 {
                let last = peer.claim_effect(3, 100).unwrap().unwrap();
                peer.journal
                    .acknowledge_dispatch(&last, 4, &terminal(&last, 0))
                    .unwrap();
            }
            let db = Connection::open(&path).unwrap();
            match defect {
                0 => db
                    .execute_batch("ALTER TABLE vm_merge_groups RENAME TO missing_groups")
                    .unwrap(),
                1 => {
                    db.execute("UPDATE vm_merge_groups SET terminal_step=X'00'", [])
                        .unwrap();
                }
                2 => {
                    db.execute(
                        "UPDATE vm_dispatches SET request=X'00' WHERE state='ready'",
                        [],
                    )
                    .unwrap();
                }
                _ => {
                    db.pragma_update(None, "ignore_check_constraints", true)
                        .unwrap();
                    db.execute("UPDATE vm_merge_groups SET state='invalid'", [])
                        .unwrap();
                }
            }
            let before = snapshot(&path);
            let pending = actual.pending.clone();
            let completed = actual.completed.clone();
            let Step::Fault(error) =
                compare_visible(&mut actual, &mut expected, token, terminal(&lease, 0))
            else {
                panic!()
            };
            assert!(
                matches!(error.code.as_str(), "LSV4007" | "LSV4033"),
                "{error:?}"
            );
            assert_eq!(actual.pending, pending);
            assert_eq!(actual.completed, completed);
            assert_eq!(snapshot(&path), before);
        }
    }

    #[test]
    fn visibility_sqlite_groups_keep_stale_owner_cache_parity_peer_successors_and_restart() {
        let mut scenarios = 0;
        for source in SOURCES {
            for outcome in 0..3 {
                for cache in 0..4 {
                    let scratch = Scratch::new();
                    let path = scratch.db();
                    let mut peer = Vm::open_journal(&path, 100).unwrap();
                    start_source(&mut peer, source);
                    let mut actual = Vm::open_journal(&path, 100).unwrap();
                    let mut expected = Vm::open_journal(&path, 100).unwrap();
                    let mut settled = 0;
                    for now in [1, 3, 5, 7, 9, 11] {
                        let Some(lease) = peer.claim_effect(now, 100).unwrap() else {
                            break;
                        };
                        let token = &lease.request.continuation.token;
                        let proposed = terminal(&lease, if settled == 0 { outcome } else { 0 });
                        let winner = peer
                            .journal
                            .acknowledge_dispatch(&lease, now + 1, &proposed)
                            .unwrap();
                        seed_local_cache(&mut actual, token, cache);
                        seed_local_cache(&mut expected, token, cache);
                        let before = snapshot(&path);
                        compare_visible(&mut actual, &mut expected, token, winner.clone());
                        compare_visible(&mut actual, &mut expected, token, winner);
                        compare_cache(&mut actual, &mut expected, token);
                        assert_eq!(snapshot(&path), before);
                        settled += 1;
                    }
                    assert!((1..6).contains(&settled), "{source}");
                    drop((peer, actual, expected));
                    let mut reopened = Vm::open_journal(&path, 100).unwrap();
                    assert!(reopened.claim_effect(100, 100).unwrap().is_none());
                    assert_eq!(reopened.pending_count(), 0);
                    scenarios += 1;
                }
            }
        }
        assert_eq!(scenarios, 72);
    }
}

// Frozen before visibility/cache policy extraction; native journal reads remain shared.
fn frozen_reconcile_completed<'a>(
    vm: &'a mut Vm,
    token: &ContinuationToken,
) -> Result<Option<&'a Step>, Fault> {
    if vm.completed.contains_key(token) {
        if !vm.journal.completed_exists(token)? {
            vm.completed.remove(token);
        }
    } else {
        match vm.journal.observe_effect(token)? {
            EffectObservation::Missing => {
                vm.pending.remove(token);
            }
            EffectObservation::Untracked | EffectObservation::Pending => {}
            EffectObservation::Completed(step) => {
                vm.pending.remove(token);
                vm.completed.insert(token.clone(), *step);
            }
        }
    }
    Ok(vm.completed.get(token))
}

fn frozen_visible_step(vm: &mut Vm, branch_token: &ContinuationToken, branch_step: Step) -> Step {
    match vm.journal.merge_progress(branch_token) {
        Ok(MergeProgress::Standalone) => branch_step,
        Ok(MergeProgress::Next {
            request,
            completed_tokens,
        }) => {
            for token in completed_tokens {
                vm.pending.remove(&token);
            }
            vm.pending.insert(
                request.continuation.token.clone(),
                request.continuation.clone(),
            );
            Step::Effect(request)
        }
        Ok(MergeProgress::Pending {
            merge_token,
            completed_branches,
            total_branches,
        }) => Step::Waiting(MergeWait {
            merge_token,
            completed_branches,
            total_branches,
        }),
        Ok(MergeProgress::Completed {
            merge_token,
            step,
            branch_tokens,
        }) => {
            let step = *step;
            for token in branch_tokens {
                if vm.pending.remove(&token).is_some() {
                    vm.completed.entry(token).or_insert_with(|| step.clone());
                }
            }
            debug_assert!(
                vm.journal
                    .merge_result(&merge_token)
                    .ok()
                    .flatten()
                    .is_some()
            );
            step
        }
        Err(error) => Step::Fault(error),
    }
}
