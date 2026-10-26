use super::*;

fn host_error() -> EffectError {
    EffectError {
        class: EffectErrorClass::Transient,
        code: "host_busy".into(),
        message: "temporary host failure".into(),
    }
}
fn invoke(
    vm: &mut Vm,
    lease: &DispatchLease,
    now: u64,
    error: EffectError,
    policy: &RetryPolicy,
    frozen: bool,
) -> Result<RetryDisposition, Fault> {
    if frozen {
        frozen_report_effect_error(vm, lease, now, error, policy)
    } else {
        vm.report_effect_error(lease, now, error, policy)
    }
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

#[test]
fn retry_entry_matches_frozen_vm_for_seventy_operations_and_competing_refusals() {
    let mut comparisons = 0;
    let mut baseline_completions = 0;
    for (original, reply) in crate::reply_contract_tests::cases() {
        for defect in 0..24 {
            let mut request = original.clone();
            if matches!(defect, 7 | 22) {
                request.continuation.deadline_at_ms = Some(if defect == 7 { 10 } else { 100 });
                request.budget = request.continuation.resource_budget();
            }
            let mut actual = Vm::new(1);
            let mut expected = Vm::new(1);
            actual.restore_request(request.clone()).unwrap();
            expected.restore_request(request.clone()).unwrap();
            let mut lease = actual.claim_effect(1, 100).unwrap().unwrap();
            assert_eq!(expected.claim_effect(1, 100).unwrap().unwrap(), lease);
            let mut policy = RetryPolicy::default();
            let mut error = host_error();
            let mut now = 12;
            if matches!(defect, 8 | 23) {
                assert_eq!(
                    actual.acknowledge_effect(&lease, 2, reply.clone()),
                    expected.acknowledge_effect(&lease, 2, reply.clone())
                );
            }
            if defect == 9 {
                assert_eq!(
                    actual.cancel_effect(&request.continuation, 2),
                    expected.cancel_effect(&request.continuation, 2)
                );
            }
            if matches!(defect, 10 | 20) {
                let previous =
                    frozen_report_effect_error(&mut actual, &lease, 2, host_error(), &policy)
                        .unwrap();
                assert_eq!(
                    frozen_report_effect_error(&mut expected, &lease, 2, host_error(), &policy)
                        .unwrap(),
                    previous
                );
                let RetryDisposition::Scheduled(schedule) = previous else {
                    panic!()
                };
                if defect == 20 {
                    lease = actual
                        .claim_effect(schedule.ready_at_ms, 100)
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        expected
                            .claim_effect(schedule.ready_at_ms, 100)
                            .unwrap()
                            .unwrap(),
                        lease
                    );
                    now = schedule.ready_at_ms + 1;
                    policy.max_retries = 1;
                }
            }
            if defect == 11 {
                assert_eq!(
                    actual.claim_effect(102, 100).unwrap().unwrap(),
                    expected.claim_effect(102, 100).unwrap().unwrap()
                );
                now = 103;
            }
            match defect {
                1 => error.class = EffectErrorClass::Permanent,
                2 => policy.max_retries = 0,
                3 => error.code.clear(),
                4 => policy.base_delay_ms = 0,
                5 | 7 | 8 | 9 | 23 => {
                    error.code.clear();
                    policy.base_delay_ms = 0;
                }
                6 => now = u64::MAX,
                12 => now = lease.lease_expires_at_ms,
                13 => {
                    lease.request.continuation.token =
                        ContinuationToken::from_wire("continuation-9001".into())
                }
                14 => lease.request.required_capability = "substituted.capability".into(),
                15 => lease.retry_count = u32::MAX,
                16 => {
                    error.class = EffectErrorClass::Permanent;
                    policy.base_delay_ms = 0;
                }
                17 => now = leselang_runtime_core::MAX_CLOCK_MS,
                18 => {
                    policy.base_delay_ms = 0;
                    now = lease.lease_expires_at_ms;
                }
                19 => lease.request.continuation.schema_version = 0,
                21 => error.message = "x".repeat(1025),
                _ => {}
            }
            if defect == 23 {
                lease.request.continuation.schema_version = 0;
            }
            let before = serde_json::to_vec(&(&lease, &policy)).unwrap();
            let want = invoke(&mut expected, &lease, now, error.clone(), &policy, true);
            let got = invoke(&mut actual, &lease, now, error, &policy, false);
            assert_eq!(
                got, want,
                "defect={defect}, type={:?}",
                original.continuation.result_type
            );
            assert_eq!(
                serde_json::to_vec(&got).unwrap(),
                serde_json::to_vec(&want).unwrap()
            );
            assert_eq!(serde_json::to_vec(&(&lease, &policy)).unwrap(), before);
            if defect < 3 {
                assert!(got.is_ok());
                assert_eq!(
                    matches!(got, Ok(RetryDisposition::Scheduled(_))),
                    defect == 0
                );
                baseline_completions += 1;
            }
            if matches!(defect, 3 | 5 | 21) {
                assert_eq!(got.as_ref().unwrap_err().code, "LSV2202");
            }
            if matches!(defect, 4 | 18) {
                assert_eq!(got.as_ref().unwrap_err().code, "LSV2201");
            }
            if defect == 17 {
                assert_eq!(got.as_ref().unwrap_err().code, "LSV2203");
            }
            if matches!(defect, 7 | 8 | 9 | 23) {
                assert!(matches!(got, Ok(RetryDisposition::Terminal(_))));
            }
            if defect == 22 {
                assert!(matches!(got, Ok(RetryDisposition::Scheduled(_))));
            }
            assert_state(&actual, &expected);
            assert_eq!(
                actual.claim_effect(1000, 100),
                expected.claim_effect(1000, 100)
            );
            assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
            assert_state(&actual, &expected);
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 1680);
    assert_eq!(baseline_completions, 210);
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn retry_entry_sqlite_matches_frozen_write_failures_peer_races_and_restart() {
    use rusqlite::{Connection, types::Value as SqlValue};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn snapshot(path: &std::path::Path) -> Vec<(String, Vec<Vec<SqlValue>>)> {
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
    let mut comparisons = 0;
    let mut observed_write_failures = BTreeSet::new();
    for source in [
        r#"ui.focus(node_id: "a")"#,
        r#"bind(r: ui.focus(node_id: "a"), body: 7)"#,
        r#"bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: "b"))"#,
        r#"bind(r: ui.focus(node_id: "a"), body: bind(s: ui.focus(node_id: "b"), body: 7))"#,
        r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"bind(r: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"bind(r: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: bind(s: ui.focus(node_id: "c"), body: 7))"#,
    ] {
        let program =
            leserpent_hir::lower(&leselang_syntax::parse(&format!("fn main() = {source}")))
                .unwrap();
        for route in 0..3 {
            for defect in 0..8 {
                let scratch = Scratch(std::env::temp_dir().join(format!(
                    "leselang-retry-entry-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                )));
                std::fs::create_dir(&scratch.0).unwrap();
                let actual_path = scratch.0.join("actual.sqlite3");
                let expected_path = scratch.0.join("expected.sqlite3");
                let mut actual = Vm::open_journal(&actual_path, 100).unwrap();
                let mut expected = Vm::open_journal(&expected_path, 100).unwrap();
                let start = |vm: &mut Vm| {
                    vm.start(
                        &program,
                        Principal::new("operator").unwrap(),
                        CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                        None,
                    )
                };
                assert_eq!(start(&mut actual), start(&mut expected));
                let mut peer_actual = Vm::open_journal(&actual_path, 1).unwrap();
                let mut peer_expected = Vm::open_journal(&expected_path, 1).unwrap();
                let lease = actual.claim_effect(1, 100).unwrap().unwrap();
                assert_eq!(expected.claim_effect(1, 100).unwrap().unwrap(), lease);
                let mut error = host_error();
                let mut policy = RetryPolicy::default();
                if route == 1 {
                    error.class = EffectErrorClass::Permanent;
                }
                if route == 2 {
                    policy.max_retries = 0;
                }
                let mut now = 12;
                match defect {
                    1..=3 => {
                        let table = ["vm_dispatches", "vm_effects", "vm_merge_groups"][defect - 1];
                        for path in [&actual_path, &expected_path] {
                            Connection::open(path).unwrap().execute_batch(&format!("CREATE TRIGGER reject_retry BEFORE UPDATE ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
                        }
                    }
                    4 => {
                        assert_eq!(
                            peer_actual.cancel_effect(&lease.request.continuation, 2),
                            peer_expected.cancel_effect(&lease.request.continuation, 2)
                        );
                    }
                    5 => {
                        let reply = EffectResult::Presentation(PresentationResult::Focus {
                            node_id: "a".into(),
                        });
                        assert_eq!(
                            peer_actual.acknowledge_effect(&lease, 2, reply.clone()),
                            peer_expected.acknowledge_effect(&lease, 2, reply)
                        );
                    }
                    6 => {
                        assert_eq!(
                            peer_actual.claim_effect(102, 100).unwrap().unwrap(),
                            peer_expected.claim_effect(102, 100).unwrap().unwrap()
                        );
                        now = 103;
                    }
                    7 => {
                        assert_eq!(
                            invoke(
                                &mut peer_actual,
                                &lease,
                                2,
                                host_error(),
                                &RetryPolicy::default(),
                                false
                            )
                            .unwrap(),
                            invoke(
                                &mut peer_expected,
                                &lease,
                                2,
                                host_error(),
                                &RetryPolicy::default(),
                                true
                            )
                            .unwrap()
                        );
                    }
                    _ => {}
                }
                let before = snapshot(&actual_path);
                let want = invoke(&mut expected, &lease, now, error.clone(), &policy, true);
                let got = invoke(&mut actual, &lease, now, error.clone(), &policy, false);
                assert_eq!(got, want, "{source}, route={route}, defect={defect}");
                assert_eq!(
                    serde_json::to_vec(&got).unwrap(),
                    serde_json::to_vec(&want).unwrap()
                );
                if matches!(defect, 0 | 4 | 5) {
                    assert!(got.is_ok(), "{got:?}");
                }
                if matches!(defect, 6 | 7) {
                    assert!(got.is_err(), "stale lease must not reschedule");
                }
                if (1..=3).contains(&defect) && got.is_err() {
                    observed_write_failures.insert(defect);
                    assert_eq!(
                        snapshot(&actual_path),
                        before,
                        "retry transaction must roll back"
                    );
                }
                assert_state(&actual, &expected);
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                for path in [&actual_path, &expected_path] {
                    Connection::open(path)
                        .unwrap()
                        .execute_batch("DROP TRIGGER IF EXISTS reject_retry")
                        .unwrap();
                }
                assert_eq!(
                    invoke(
                        &mut peer_actual,
                        &lease,
                        now + 1,
                        error.clone(),
                        &policy,
                        false
                    ),
                    invoke(&mut peer_expected, &lease, now + 1, error, &policy, true)
                );
                assert_state(&peer_actual, &peer_expected);
                drop((actual, expected, peer_actual, peer_expected));
                let mut actual = Vm::open_journal(&actual_path, 0).unwrap();
                let mut expected = Vm::open_journal(&expected_path, 0).unwrap();
                assert_state(&actual, &expected);
                let mut drained = 0;
                for now in 1000..1016 {
                    let next = actual.claim_effect(now, 100).unwrap();
                    assert_eq!(next, expected.claim_effect(now, 100).unwrap());
                    let Some(next) = next else { break };
                    let got = actual.cancel_effect(&next.request.continuation, now + 1);
                    assert_eq!(
                        got,
                        expected.cancel_effect(&next.request.continuation, now + 1)
                    );
                    assert!(!matches!(got, Step::Fault(_)), "{got:?}");
                    drained += 1;
                }
                assert!(drained < 16);
                assert_eq!(actual.pending_count(), 0);
                assert_state(&actual, &expected);
                assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                comparisons += 1;
            }
        }
    }
    assert_eq!(observed_write_failures, BTreeSet::from([1, 2, 3]));
    assert_eq!(comparisons, 192);
}

// Frozen complete retry entry before product-policy extraction.
fn frozen_report_effect_error(
    vm: &mut Vm,
    lease: &DispatchLease,
    now_ms: u64,
    error: EffectError,
    policy: &RetryPolicy,
) -> Result<RetryDisposition, Fault> {
    validate_scheduler_time(now_ms)?;
    vm.expire_due(now_ms)?;
    if let Some(completed) = vm.replay_completed(&lease.request.continuation.token)? {
        return Ok(RetryDisposition::Terminal(
            vm.visible_step(&lease.request.continuation.token, completed),
        ));
    }
    let schedule = leselang_command::retry_policy::plan_effect_retry(
        lease.retry_count,
        now_ms,
        &error,
        policy,
    )
    .map_err(effect_retry_fault)?;
    let disposition = match schedule {
        None => RetryDisposition::Terminal(Step::Failed(FailedEffect {
            error,
            retry_count: lease.retry_count,
        })),
        Some(point) => RetryDisposition::Scheduled(RetrySchedule {
            retry_count: point.retry_count,
            ready_at_ms: point.ready_at_ms,
            error,
        }),
    };
    let authoritative = vm.journal.report_error(lease, now_ms, &disposition)?;
    if let RetryDisposition::Terminal(step) = &authoritative {
        let image = &lease.request.continuation;
        vm.pending.remove(&image.token);
        vm.completed.insert(image.token.clone(), step.clone());
        return Ok(RetryDisposition::Terminal(
            vm.visible_step(&image.token, step.clone()),
        ));
    }
    Ok(authoritative)
}
