use super::*;

#[derive(Debug, PartialEq)]
enum Outcome {
    // Test-only boxing keeps observations small without changing optional wire bytes.
    Optional(Result<Option<Box<DispatchLease>>, Fault>),
    Structured(Result<DispatchClaim, Fault>),
}
impl Outcome {
    fn lease(&self) -> Option<&DispatchLease> {
        match self {
            Self::Optional(Ok(Some(lease))) => Some(lease),
            Self::Structured(Ok(DispatchClaim::Leased(lease))) => Some(lease),
            _ => None,
        }
    }
    fn error(&self) -> Option<&Fault> {
        match self {
            Self::Optional(Err(error)) | Self::Structured(Err(error)) => Some(error),
            _ => None,
        }
    }
    fn wire(&self) -> Vec<u8> {
        match self {
            Self::Optional(result) => serde_json::to_vec(result).unwrap(),
            Self::Structured(result) => serde_json::to_vec(result).unwrap(),
        }
    }
}
fn invoke(vm: &mut Vm, now: u64, duration: u64, structured: bool, frozen: bool) -> Outcome {
    match (structured, frozen) {
        (false, false) => Outcome::Optional(
            vm.claim_effect(now, duration)
                .map(|lease| lease.map(Box::new)),
        ),
        (false, true) => Outcome::Optional(
            frozen_claim_effect(vm, now, duration).map(|lease| lease.map(Box::new)),
        ),
        (true, false) => Outcome::Structured(vm.try_claim_effect(now, duration)),
        (true, true) => Outcome::Structured(frozen_try_claim_effect(vm, now, duration)),
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
fn retry_error() -> EffectError {
    EffectError {
        class: EffectErrorClass::Transient,
        code: "busy".into(),
        message: "try later".into(),
    }
}

#[test]
fn claim_entries_match_frozen_vm_for_seventy_operations_and_live_state_boundaries() {
    let mut comparisons = 0;
    let mut baseline_leases = 0;
    for (original, reply) in crate::reply_contract_tests::cases() {
        for structured in [false, true] {
            for defect in 0..20 {
                let mut request = original.clone();
                if matches!(defect, 7 | 8 | 18) {
                    request.continuation.deadline_at_ms = Some(10);
                    request.budget = request.continuation.resource_budget();
                }
                let limits = SchedulerLimits {
                    max_active_leases: if matches!(defect, 15 | 16) { 1 } else { 100 },
                    ..SchedulerLimits::default()
                };
                let mut actual = Vm::new_with_limits(1, limits).unwrap();
                let mut expected = Vm::new_with_limits(1, limits).unwrap();
                if defect != 1 {
                    actual.restore_request(request.clone()).unwrap();
                    expected.restore_request(request.clone()).unwrap();
                }
                let mut now = 12;
                let mut duration = 50;
                if matches!(defect, 9..=13 | 15 | 16) {
                    let lease = frozen_claim_effect(&mut actual, 1, 100).unwrap().unwrap();
                    assert_eq!(
                        frozen_claim_effect(&mut expected, 1, 100).unwrap().unwrap(),
                        lease
                    );
                    match defect {
                        10 => now = 102,
                        11 | 12 => {
                            assert_eq!(
                                actual
                                    .report_effect_error(
                                        &lease,
                                        2,
                                        retry_error(),
                                        &RetryPolicy::default()
                                    )
                                    .unwrap(),
                                expected
                                    .report_effect_error(
                                        &lease,
                                        2,
                                        retry_error(),
                                        &RetryPolicy::default()
                                    )
                                    .unwrap()
                            );
                            if defect == 12 {
                                now = 252;
                            }
                        }
                        13 => {
                            assert_eq!(
                                actual.acknowledge_effect(&lease, 2, reply.clone()),
                                expected.acknowledge_effect(&lease, 2, reply.clone())
                            );
                        }
                        15 => {
                            let mut other = request.clone();
                            other.continuation.token =
                                ContinuationToken::from_wire("continuation-9000".into());
                            other.effect_id = "effect-9000".into();
                            if let EffectOperation::Command(envelope) = &mut other.operation {
                                envelope.command_id =
                                    CommandId::new("leselang-command-9000").unwrap();
                                envelope.idempotency_key =
                                    IdempotencyKey::new("leselang-effect-9000").unwrap();
                            }
                            actual.restore_request(other.clone()).unwrap();
                            expected.restore_request(other).unwrap();
                        }
                        _ => {}
                    }
                }
                if defect == 14 {
                    assert_eq!(
                        actual.cancel_effect(&request.continuation, 2),
                        expected.cancel_effect(&request.continuation, 2)
                    );
                }
                if defect == 17 {
                    for now in 1..=u64::from(MAX_DISPATCH_ATTEMPTS) {
                        assert_eq!(
                            frozen_claim_effect(&mut actual, now, 1).unwrap().unwrap(),
                            frozen_claim_effect(&mut expected, now, 1).unwrap().unwrap()
                        );
                    }
                    now = u64::from(MAX_DISPATCH_ATTEMPTS) + 1;
                }
                match defect {
                    2 => {
                        actual.pending.remove(&request.continuation.token);
                        expected.pending.remove(&request.continuation.token);
                    }
                    3 | 19 => {
                        for vm in [&mut actual, &mut expected] {
                            let image = vm.pending.get_mut(&request.continuation.token).unwrap();
                            if defect == 3 {
                                image.fuel_remaining -= 1;
                            } else {
                                image.token =
                                    ContinuationToken::from_wire("continuation-9001".into());
                            }
                        }
                    }
                    4 => {
                        now = u64::MAX;
                        duration = 0;
                    }
                    5 | 18 => duration = 0,
                    6 => duration = MAX_DISPATCH_LEASE_MS + 1,
                    7 => {
                        now = leselang_runtime_core::MAX_CLOCK_MS;
                        duration = 1;
                    }
                    _ => {}
                }
                let cache_pointer = actual
                    .pending
                    .get(&request.continuation.token)
                    .map(|image| image.token.as_str().as_ptr());
                let before = serde_json::to_vec(&request).unwrap();
                let want = invoke(&mut expected, now, duration, structured, true);
                let got = invoke(&mut actual, now, duration, structured, false);
                assert_eq!(
                    got, want,
                    "defect={defect}, structured={structured}, type={:?}",
                    original.continuation.result_type
                );
                assert_eq!(got.wire(), want.wire());
                assert_eq!(serde_json::to_vec(&request).unwrap(), before);
                if matches!(defect, 0 | 2) {
                    assert_eq!(got.lease().unwrap().request, request);
                    baseline_leases += 1;
                }
                if defect == 0 {
                    assert_eq!(
                        actual
                            .pending
                            .get(&request.continuation.token)
                            .unwrap()
                            .token
                            .as_str()
                            .as_ptr(),
                        cache_pointer.unwrap()
                    );
                }
                if matches!(defect, 3 | 19) {
                    assert_eq!(got.error().unwrap().code, "LSV2005");
                    assert_eq!(actual.scheduler_pressure(now).unwrap().active_leases, 1);
                }
                if matches!(defect, 5 | 6 | 7 | 18) {
                    assert_eq!(got.error().unwrap().code, "LSV4015");
                    assert_eq!(actual.pending_count(), 1);
                }
                if defect == 15 {
                    if structured {
                        assert!(matches!(
                            got,
                            Outcome::Structured(Ok(DispatchClaim::Backpressured(_)))
                        ));
                    } else {
                        assert_eq!(got.error().unwrap().code, "LSV2502");
                    }
                }
                if matches!(defect, 1 | 8 | 9 | 11 | 13 | 14 | 16) {
                    assert!(matches!(
                        got,
                        Outcome::Optional(Ok(None)) | Outcome::Structured(Ok(DispatchClaim::Idle))
                    ));
                }
                if defect == 17 {
                    assert_eq!(got.error().unwrap().code, "LSV4017");
                }
                assert_state(&actual, &expected);
                assert_eq!(
                    invoke(&mut actual, 1000, 1, structured, false),
                    invoke(&mut expected, 1000, 1, structured, true)
                );
                assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                assert_state(&actual, &expected);
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 2800);
    assert_eq!(baseline_leases, 280);
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn claim_entries_sqlite_match_frozen_failures_peer_visibility_and_restart() {
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
        for structured in [false, true] {
            for defect in 0..11 {
                let scratch = Scratch(std::env::temp_dir().join(format!(
                    "leselang-claim-entry-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                )));
                std::fs::create_dir(&scratch.0).unwrap();
                let actual_path = scratch.0.join("actual.sqlite3");
                let expected_path = scratch.0.join("expected.sqlite3");
                let limits = SchedulerLimits {
                    max_active_leases: 1,
                    ..SchedulerLimits::default()
                };
                let mut actual = Vm::open_journal_with_limits(&actual_path, 100, limits).unwrap();
                let mut expected =
                    Vm::open_journal_with_limits(&expected_path, 100, limits).unwrap();
                let start = |vm: &mut Vm| {
                    if matches!(defect, 2 | 3) {
                        vm.start_timed(
                            &program,
                            Principal::new("operator").unwrap(),
                            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                            None,
                            0,
                            10,
                        )
                    } else {
                        vm.start(
                            &program,
                            Principal::new("operator").unwrap(),
                            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                            None,
                        )
                    }
                };
                if defect != 7 {
                    assert_eq!(start(&mut actual), start(&mut expected));
                }
                let mut peer_actual =
                    Vm::open_journal_with_limits(&actual_path, 100, limits).unwrap();
                let mut peer_expected =
                    Vm::open_journal_with_limits(&expected_path, 100, limits).unwrap();
                if defect == 7 {
                    assert_eq!(start(&mut peer_actual), start(&mut peer_expected));
                    assert_eq!(actual.pending_count(), 0);
                }
                let mut now = 12;
                if matches!(defect, 4..=6 | 9 | 10) {
                    let lease = frozen_claim_effect(&mut peer_actual, 1, 100)
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        frozen_claim_effect(&mut peer_expected, 1, 100)
                            .unwrap()
                            .unwrap(),
                        lease
                    );
                    match defect {
                        5 => {
                            assert_eq!(
                                peer_actual.cancel_effect(&lease.request.continuation, 2),
                                peer_expected.cancel_effect(&lease.request.continuation, 2)
                            );
                        }
                        6 => {
                            let reply = EffectResult::Presentation(PresentationResult::Focus {
                                node_id: "a".into(),
                            });
                            assert_eq!(
                                peer_actual.acknowledge_effect(&lease, 2, reply.clone()),
                                peer_expected.acknowledge_effect(&lease, 2, reply)
                            );
                        }
                        9 => {
                            assert_eq!(
                                peer_actual
                                    .report_effect_error(
                                        &lease,
                                        2,
                                        retry_error(),
                                        &RetryPolicy::default()
                                    )
                                    .unwrap(),
                                peer_expected
                                    .report_effect_error(
                                        &lease,
                                        2,
                                        retry_error(),
                                        &RetryPolicy::default()
                                    )
                                    .unwrap()
                            );
                        }
                        10 => now = 102,
                        _ => {}
                    }
                }
                if (1..=3).contains(&defect) {
                    let table = ["vm_dispatches", "vm_effects", "vm_merge_groups"][defect - 1];
                    for path in [&actual_path, &expected_path] {
                        Connection::open(path).unwrap().execute_batch(&format!("CREATE TRIGGER reject_claim BEFORE UPDATE ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
                    }
                }
                if defect == 8 {
                    for vm in [&mut actual, &mut expected] {
                        for image in vm.pending.values_mut() {
                            image.fuel_remaining -= 1;
                        }
                    }
                }
                let before = snapshot(&actual_path);
                let want = invoke(&mut expected, now, 50, structured, true);
                let got = invoke(&mut actual, now, 50, structured, false);
                assert_eq!(
                    got, want,
                    "{source}, structured={structured}, defect={defect}"
                );
                assert_eq!(got.wire(), want.wire());
                if matches!(defect, 0 | 7 | 10) {
                    assert!(got.lease().is_some(), "{got:?}");
                }
                if (1..=3).contains(&defect) && got.error().is_some() {
                    observed_write_failures.insert(defect);
                    assert_eq!(
                        snapshot(&actual_path),
                        before,
                        "native transaction rollback"
                    );
                }
                if defect == 8 {
                    assert_eq!(got.error().unwrap().code, "LSV2005");
                    assert_eq!(actual.scheduler_pressure(now).unwrap().active_leases, 1);
                    assert_ne!(
                        snapshot(&actual_path),
                        before,
                        "lease is committed before local conflict"
                    );
                }
                assert_state(&actual, &expected);
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                for path in [&actual_path, &expected_path] {
                    Connection::open(path)
                        .unwrap()
                        .execute_batch("DROP TRIGGER IF EXISTS reject_claim")
                        .unwrap();
                }
                assert_eq!(
                    invoke(&mut peer_actual, now + 1, 50, structured, false),
                    invoke(&mut peer_expected, now + 1, 50, structured, true)
                );
                assert_state(&peer_actual, &peer_expected);
                drop((actual, expected, peer_actual, peer_expected));
                let mut actual = Vm::open_journal_with_limits(&actual_path, 0, limits).unwrap();
                let mut expected = Vm::open_journal_with_limits(&expected_path, 0, limits).unwrap();
                assert_state(&actual, &expected);
                let mut drained = 0;
                for now in 1000..1016 {
                    let got = invoke(&mut actual, now, 1, structured, false);
                    let want = invoke(&mut expected, now, 1, structured, true);
                    assert_eq!(got, want);
                    assert!(got.error().is_none(), "{got:?}");
                    let Some(lease) = got.lease() else { break };
                    let result = actual.cancel_effect(&lease.request.continuation, now);
                    assert_eq!(
                        result,
                        expected.cancel_effect(&lease.request.continuation, now)
                    );
                    assert!(!matches!(result, Step::Fault(_)), "{result:?}");
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
    assert_eq!(comparisons, 176);
}

// Frozen complete claim entries before product-policy extraction.
fn frozen_claim_effect(
    vm: &mut Vm,
    now_ms: u64,
    lease_ms: u64,
) -> Result<Option<DispatchLease>, Fault> {
    match frozen_try_claim_effect(vm, now_ms, lease_ms)? {
        DispatchClaim::Leased(lease) => Ok(Some(*lease)),
        DispatchClaim::Idle => Ok(None),
        DispatchClaim::Backpressured(pressure) => Err(pressure.lease_fault()),
    }
}
fn frozen_try_claim_effect(
    vm: &mut Vm,
    now_ms: u64,
    lease_ms: u64,
) -> Result<DispatchClaim, Fault> {
    validate_scheduler_time(now_ms)?;
    journal::validate_lease_clock(now_ms, lease_ms)?;
    vm.expire_due(now_ms)?;
    let claim = vm
        .journal
        .claim_dispatch(now_ms, lease_ms, vm.scheduler_limits)?;
    if let DispatchClaim::Leased(lease) = &claim {
        let image = &lease.request.continuation;
        match vm.pending.entry(image.token.clone()) {
            std::collections::btree_map::Entry::Occupied(entry) => {
                if entry.get() != image {
                    return Err(Fault {
                        code: "LSV2005".into(),
                        message: "claimed continuation conflicts with local pending state".into(),
                    });
                }
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                // Another engine may have admitted this request after our startup snapshot.
                entry.insert(image.clone());
            }
        }
    }
    Ok(claim)
}
