use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    DispatchClaim, DispatchLease, EffectError, EffectErrorClass, EffectOperation, EffectRequest,
    EffectResult, MAX_DISPATCH_LEASE_MS, MergePlan, RetryDisposition, RetryPolicy, SchedulerLimits,
    Step, Vm, merge_declared,
};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-backpressure-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("flow.sqlite3"))
    }
    fn vm(&self, durable: bool, pending: usize, active: usize) -> Vm {
        let limits = limits(pending, active);
        if durable {
            Vm::open_journal_with_limits(&self.0, 10000, limits).unwrap()
        } else {
            Vm::new_with_limits(10000, limits).unwrap()
        }
    }
}
impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}
fn limits(pending: usize, active: usize) -> SchedulerLimits {
    SchedulerLimits {
        max_pending_dispatches: pending,
        max_active_leases: active,
    }
}
fn start(vm: &mut Vm, source: &str, now: u64, timeout: u64) -> Step {
    vm.start_timed(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        now,
        timeout,
    )
}
fn focus(label: &str) -> String {
    format!(r#"fn main() = bind(result: ui.focus(node_id: "{label}"), body: true)"#)
}
fn effect(step: Step) -> EffectRequest {
    let Step::Effect(request) = step else {
        panic!("{step:?}")
    };
    *request
}
fn fault(step: Step, code: &str) {
    assert!(
        matches!(step, Step::Fault(ref fault) if fault.code == code),
        "{step:?}"
    );
}
fn leased(claim: DispatchClaim) -> DispatchLease {
    let DispatchClaim::Leased(lease) = claim else {
        panic!("{claim:?}")
    };
    *lease
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}
fn acknowledge(vm: &mut Vm, lease: &DispatchLease, now: u64) -> Step {
    vm.acknowledge_effect(lease, now, receipt(&lease.request))
}

#[test]
fn scheduler_faults_do_not_alias_legacy_merge_validation_faults() {
    assert_eq!(MergePlan::new(["only"]).unwrap_err().code, "LSV2401");
    let plan = MergePlan::new(["left", "right"]).unwrap();
    assert_eq!(
        merge_declared(&plan, vec![], 100).unwrap_err().code,
        "LSV2402"
    );
    let mut vm = Vm::new_with_limits(10000, limits(1, 1)).unwrap();
    effect(start(&mut vm, &focus("existing"), 100, 1000));
    fault(start(&mut vm, &focus("blocked"), 100, 1000), "LSV2501");
}

#[test]
fn invalid_limits_fail_before_creating_a_journal_and_do_not_replace_valid_policy() {
    let path = JournalPath::new();
    let mut vm = path.vm(false, 2, 1);
    for invalid in [
        limits(0, 1),
        limits(1, 0),
        limits(10001, 1),
        limits(1, usize::MAX),
    ] {
        assert_eq!(
            Vm::open_journal_with_limits(&path.0, 10000, invalid)
                .err()
                .unwrap()
                .code,
            "LSV2500"
        );
        assert!(!path.0.exists());
        assert_eq!(
            vm.set_scheduler_limits(invalid).unwrap_err().code,
            "LSV2500"
        );
        assert_eq!(vm.scheduler_pressure(100).unwrap().limits, limits(2, 1));
    }
}

#[test]
fn invalid_claim_parameters_do_not_expire_pending_work_or_commit_durable_state() {
    for durable in [false, true] {
        for legacy in [false, true] {
            for (now_ms, lease_ms, code, message) in [
                (110, 0, "LSV4015", "dispatch lease must be between"),
                (
                    110,
                    MAX_DISPATCH_LEASE_MS + 1,
                    "LSV4015",
                    "dispatch lease must be between",
                ),
                (110, u64::MAX, "LSV4015", "dispatch lease must be between"),
                (
                    i64::MAX as u64,
                    1,
                    "LSV4015",
                    "dispatch lease expiration is out of range",
                ),
                (u64::MAX, 0, "LSV2011", "scheduler clock is out of range"),
            ] {
                let path = JournalPath::new();
                let mut vm = path.vm(durable, 2, 1);
                let request = effect(start(&mut vm, &focus("pending"), 100, 10));
                let before = vm.pending_continuations();
                let error = if legacy {
                    vm.claim_effect(now_ms, lease_ms).unwrap_err()
                } else {
                    vm.try_claim_effect(now_ms, lease_ms).unwrap_err()
                };
                assert_eq!(error.code, code);
                assert!(error.message.starts_with(message));
                assert_eq!(vm.pending_continuations(), before);
                assert_eq!(vm.completed_count(), 0);
                assert_eq!(vm.scheduler_pressure(110).unwrap().pending_dispatches, 1);
                if durable {
                    drop(vm);
                    vm = path.vm(true, 2, 1);
                    assert_eq!(vm.pending_continuations(), before);
                    assert_eq!(vm.completed_count(), 0);
                }
                assert_eq!(vm.try_claim_effect(120, 1).unwrap(), DispatchClaim::Idle);
                assert_eq!(vm.pending_count(), 0);
                assert!(matches!(
                    vm.cancel_effect(&request.continuation, 121),
                    Step::Cancelled(cancellation)
                        if cancellation.observed_at_ms == 120
                            && cancellation.reason == leselang_vm::CancellationReason::DeadlineExceeded
                ));
            }
        }
    }
}

#[test]
fn invalid_claim_does_not_cancel_a_leased_group_or_change_its_attempt() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 2, 1);
        effect(start(
            &mut vm,
            r#"fn main() = seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
            100,
            10,
        ));
        let lease = leased(vm.try_claim_effect(101, 50).unwrap());
        let before = vm.pending_continuations();
        let pressure = vm.scheduler_pressure(102).unwrap();
        assert_eq!(vm.try_claim_effect(110, 0).unwrap_err().code, "LSV4015");
        assert_eq!(vm.claim_effect(111, u64::MAX).unwrap_err().code, "LSV4015");
        assert_eq!(vm.pending_continuations(), before);
        assert_eq!(vm.completed_count(), 0);
        assert_eq!(vm.scheduler_pressure(102).unwrap(), pressure);
        if durable {
            let connection = Connection::open(&path.0).unwrap();
            let stored = connection.query_row(
                "SELECT state, attempt, lease_expires_at_ms FROM vm_dispatches WHERE token = ?1",
                [lease.request.continuation.token.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?, row.get::<_, Option<i64>>(2)?)),
            ).unwrap();
            assert_eq!(
                stored,
                (
                    "leased".into(),
                    lease.attempt,
                    Some(lease.lease_expires_at_ms as i64)
                )
            );
        }
        assert_eq!(vm.try_claim_effect(112, 1).unwrap(), DispatchClaim::Idle);
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn invalid_claim_parameters_are_rejected_before_waiting_for_a_sqlite_writer() {
    let path = JournalPath::new();
    let mut vm = path.vm(true, 1, 1);
    effect(start(&mut vm, &focus("pending"), 100, 10));
    let before = vm.pending_continuations();
    let connection = Connection::open(&path.0).unwrap();
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    let error = vm.try_claim_effect(110, 0).unwrap_err();
    connection.execute_batch("ROLLBACK").unwrap();
    assert_eq!(error.code, "LSV4015");
    assert_eq!(vm.pending_continuations(), before);
    assert_eq!(vm.completed_count(), 0);
}

#[test]
fn admission_rejection_has_no_effect_or_identity_and_completion_releases_capacity() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 1);
        let first = effect(start(&mut vm, &focus("first"), 100, 1000));
        fault(start(&mut vm, &focus("rejected"), 100, 1000), "LSV2501");
        assert_eq!(vm.scheduler_pressure(100).unwrap().pending_dispatches, 1);
        assert!(matches!(
            start(&mut vm, "fn main() = true", 100, 1000),
            Step::Done(_)
        ));
        let lease = leased(vm.try_claim_effect(101, 50).unwrap());
        assert_eq!(lease.request, first);
        assert!(matches!(acknowledge(&mut vm, &lease, 102), Step::Done(_)));
        let next = effect(start(&mut vm, &focus("next"), 103, 1000));
        assert_eq!(next.effect_id, "effect-2");
        assert_eq!(vm.scheduler_pressure(103).unwrap().pending_dispatches, 1);
    }
}

#[test]
fn oversized_batches_fail_before_allocation_and_busy_batches_leave_no_partial_graph() {
    let group = r#"fn main() = all(first: ui.focus(node_id: "first"), second: ui.focus(node_id: "second"))"#;
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 1);
        fault(start(&mut vm, group, 100, 1000), "LSV2503");
        let first = effect(start(&mut vm, &focus("existing"), 100, 1000));
        assert_eq!(first.effect_id, "effect-1");
        vm.set_scheduler_limits(limits(2, 1)).unwrap();
        fault(start(&mut vm, group, 100, 1000), "LSV2501");
        assert_eq!(vm.scheduler_pressure(100).unwrap().pending_dispatches, 1);
        if durable {
            assert_eq!(
                Connection::open(&path.0)
                    .unwrap()
                    .query_row("SELECT COUNT(*) FROM vm_merge_groups", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        let lease = leased(vm.try_claim_effect(101, 50).unwrap());
        assert!(matches!(acknowledge(&mut vm, &lease, 102), Step::Done(_)));
        assert!(matches!(start(&mut vm, group, 103, 1000), Step::Effects(_)));
        assert_eq!(vm.scheduler_pressure(103).unwrap().pending_dispatches, 2);
    }
}

#[test]
fn typed_claim_distinguishes_idle_from_backpressure_without_spending_attempts() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 2, 1);
        assert_eq!(vm.try_claim_effect(100, 50).unwrap(), DispatchClaim::Idle);
        let first = effect(start(&mut vm, &focus("first"), 100, 1000));
        let second = effect(start(&mut vm, &focus("second"), 100, 1000));
        let lease = leased(vm.try_claim_effect(101, 50).unwrap());
        assert_eq!(lease.request, first);
        for now in 102..105 {
            let DispatchClaim::Backpressured(pressure) = vm.try_claim_effect(now, 50).unwrap()
            else {
                panic!()
            };
            assert_eq!(pressure.pending_dispatches, 2);
            assert_eq!(pressure.active_leases, 1);
            assert_eq!(pressure.limits, limits(2, 1));
        }
        assert_eq!(vm.claim_effect(105, 50).unwrap_err().code, "LSV2502");
        assert!(matches!(acknowledge(&mut vm, &lease, 106), Step::Done(_)));
        let next = leased(vm.try_claim_effect(107, 50).unwrap());
        assert_eq!(next.request, second);
        assert_eq!(next.attempt, 1);
        assert!(matches!(acknowledge(&mut vm, &next, 108), Step::Done(_)));
        assert_eq!(vm.try_claim_effect(109, 50).unwrap(), DispatchClaim::Idle);
    }
}

#[test]
fn expired_leases_release_slots_without_weakening_old_attempt_fencing() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 2, 1);
        let first = effect(start(&mut vm, &focus("first"), 100, 1000));
        let second = effect(start(&mut vm, &focus("second"), 100, 1000));
        let old = leased(vm.try_claim_effect(101, 10).unwrap());
        assert!(matches!(
            vm.try_claim_effect(110, 50).unwrap(),
            DispatchClaim::Backpressured(_)
        ));
        let current = leased(vm.try_claim_effect(111, 50).unwrap());
        assert_eq!(current.request, second);
        fault(acknowledge(&mut vm, &old, 112), "LSV4023");
        assert!(matches!(acknowledge(&mut vm, &current, 113), Step::Done(_)));
        let retry = leased(vm.try_claim_effect(114, 50).unwrap());
        assert_eq!(retry.request, first);
        assert_eq!(retry.attempt, 2);
        fault(acknowledge(&mut vm, &old, 114), "LSV4022");
        assert!(matches!(acknowledge(&mut vm, &retry, 115), Step::Done(_)));
    }
}

#[test]
fn blocked_sequential_siblings_are_idle_not_lease_backpressure() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 2, 1);
        let source = r#"fn main() = bind(group: seq(first: ui.focus(node_id: "first"), second: ui.focus(node_id: "second")), body: true)"#;
        assert!(matches!(start(&mut vm, source, 100, 1000), Step::Effect(_)));
        let first = leased(vm.try_claim_effect(101, 50).unwrap());
        assert_eq!(vm.try_claim_effect(102, 50).unwrap(), DispatchClaim::Idle);
        assert_eq!(vm.scheduler_pressure(102).unwrap().active_leases, 1);
        assert!(matches!(acknowledge(&mut vm, &first, 103), Step::Effect(_)));
        let second = leased(vm.try_claim_effect(104, 50).unwrap());
        assert!(matches!(acknowledge(&mut vm, &second, 105), Step::Done(_)));
    }
}

#[test]
fn draining_an_admitted_group_and_tail_survives_lowered_admission_capacity() {
    for kind in ["seq", "all"] {
        for durable in [false, true] {
            let path = JournalPath::new();
            let mut vm = path.vm(durable, 2, 1);
            let source = format!(
                r#"fn main() = bind(group: {kind}(first: ui.assert_text(node_id: "a", expected: "ready"), second: ui.focus(node_id: "b")), body: bind(tail: ui.set_form_value(node_id: "form", field: "answer", value: field(value: member(value: group, name: "first"), name: "expected")), body: true))"#
            );
            assert!(matches!(
                start(&mut vm, &source, 100, 1000),
                Step::Effect(_) | Step::Effects(_)
            ));
            vm.set_scheduler_limits(limits(1, 1)).unwrap();
            fault(start(&mut vm, &focus("rejected"), 100, 1000), "LSV2501");
            for index in 0..3 {
                let now = 101 + index * 2;
                let lease = leased(vm.try_claim_effect(now, 50).unwrap());
                let progress = acknowledge(&mut vm, &lease, now + 1);
                if index == 2 {
                    assert!(matches!(progress, Step::Done(_)));
                } else {
                    assert!(matches!(progress, Step::Effect(_) | Step::Waiting(_)));
                }
            }
            assert_eq!(vm.scheduler_pressure(107).unwrap().pending_dispatches, 0);
            assert!(matches!(
                start(&mut vm, &focus("accepted"), 107, 1000),
                Step::Effect(_)
            ));
        }
    }
}

#[test]
fn timed_admission_reaps_deadlines_but_pressure_snapshots_are_read_only() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 1);
        let expired = effect(start(&mut vm, &focus("expired"), 100, 10));
        assert_eq!(vm.scheduler_pressure(110).unwrap().pending_dispatches, 1);
        assert!(matches!(
            start(&mut vm, &focus("healthy"), 110, 1000),
            Step::Effect(_)
        ));
        assert_eq!(vm.scheduler_pressure(110).unwrap().pending_dispatches, 1);
        assert!(matches!(
            vm.resume_at(&expired.continuation, 111, receipt(&expired)),
            Step::Cancelled(_)
        ));
    }
}

#[test]
fn semantic_retry_keeps_pending_capacity_but_releases_the_execution_slot() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 1);
        let request = effect(start(&mut vm, &focus("retrying"), 100, 1000));
        let lease = leased(vm.try_claim_effect(101, 50).unwrap());
        assert!(matches!(
            vm.report_effect_error(
                &lease,
                102,
                EffectError {
                    class: EffectErrorClass::Transient,
                    code: "busy".into(),
                    message: "host busy".into(),
                },
                &RetryPolicy::default()
            )
            .unwrap(),
            RetryDisposition::Scheduled(_)
        ));
        let pressure = vm.scheduler_pressure(102).unwrap();
        assert_eq!(
            (pressure.pending_dispatches, pressure.active_leases),
            (1, 0)
        );
        fault(start(&mut vm, &focus("rejected"), 102, 1000), "LSV2501");
        assert_eq!(vm.try_claim_effect(103, 50).unwrap(), DispatchClaim::Idle);
        assert!(matches!(
            vm.cancel_effect(&request.continuation, 104),
            Step::Cancelled(_)
        ));
        assert!(matches!(
            start(&mut vm, &focus("accepted"), 105, 1000),
            Step::Effect(_)
        ));
    }
}

#[test]
fn restore_request_is_bounded_but_duplicate_restore_is_idempotent_at_capacity() {
    let path = JournalPath::new();
    let mut source = Vm::default();
    let first = effect(start(&mut source, &focus("first"), 100, 1000));
    let second = effect(start(&mut source, &focus("second"), 100, 1000));
    for durable in [false, true] {
        let mut vm = path.vm(durable, 1, 1);
        vm.restore_request(first.clone()).unwrap();
        vm.restore_request(first.clone()).unwrap();
        assert_eq!(
            vm.restore_request(second.clone()).unwrap_err().code,
            "LSV2501"
        );
        assert_eq!(vm.scheduler_pressure(100).unwrap().pending_dispatches, 1);
    }
}

#[test]
fn concurrent_stale_workers_cannot_over_admit_shared_graph_capacity() {
    let path = JournalPath::new();
    let mut admitting = path.vm(true, 3, 2);
    let workers = (0..4).map(|_| path.vm(true, 3, 2)).collect::<Vec<_>>();
    effect(start(&mut admitting, &focus("existing"), 100, 1000));
    let source = r#"fn main() = all(first: ui.focus(node_id: "first"), second: ui.focus(node_id: "second"))"#;
    let barrier = std::sync::Barrier::new(4);
    let results = std::thread::scope(|scope| {
        let threads = workers
            .into_iter()
            .map(|mut worker| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    start(&mut worker, source, 100, 1000)
                })
            })
            .collect::<Vec<_>>();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        results
            .iter()
            .filter(|step| matches!(step, Step::Effects(_)))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|step| matches!(step, Step::Fault(fault) if fault.code == "LSV2501"))
            .count(),
        3
    );
    assert_eq!(
        admitting
            .scheduler_pressure(100)
            .unwrap()
            .pending_dispatches,
        3
    );
    let connection = Connection::open(&path.0).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM vm_merge_groups", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn concurrent_claim_capacity_is_global_and_survives_worker_restarts() {
    let path = JournalPath::new();
    let mut admitting = path.vm(true, 5, 2);
    let observer = path.vm(true, 5, 2);
    for index in 0..5 {
        effect(start(
            &mut admitting,
            &focus(&format!("node-{index}")),
            100,
            1000,
        ));
    }
    assert_eq!(observer.pending_count(), 0);
    assert_eq!(
        observer.scheduler_pressure(100).unwrap().pending_dispatches,
        5
    );
    for now in [101, 121] {
        let workers = (0..4).map(|_| path.vm(true, 5, 2)).collect::<Vec<_>>();
        let barrier = std::sync::Barrier::new(4);
        let claims = std::thread::scope(|scope| {
            let threads = workers
                .into_iter()
                .map(|mut worker| {
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        worker.try_claim_effect(now, 20).unwrap()
                    })
                })
                .collect::<Vec<_>>();
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(
            claims
                .iter()
                .filter(|claim| matches!(claim, DispatchClaim::Leased(_)))
                .count(),
            2
        );
        assert_eq!(
            claims
                .iter()
                .filter(|claim| matches!(claim, DispatchClaim::Backpressured(_)))
                .count(),
            2
        );
        let pressure = observer.scheduler_pressure(now).unwrap();
        assert_eq!(
            (pressure.pending_dispatches, pressure.active_leases),
            (5, 2)
        );
    }
}

#[test]
fn request_restore_enforces_capacity_without_a_vm_start_preflight() {
    let path = JournalPath::new();
    let observer = path.vm(true, 1, 1);
    let mut source = Vm::default();
    let workers = (0..4)
        .map(|index| {
            let request = effect(start(
                &mut source,
                &focus(&format!("node-{index}")),
                100,
                1000,
            ));
            (path.vm(true, 1, 1), request)
        })
        .collect::<Vec<_>>();
    let barrier = std::sync::Barrier::new(4);
    let results = std::thread::scope(|scope| {
        let threads = workers
            .into_iter()
            .map(|(mut worker, request)| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    worker.restore_request(request)
                })
            })
            .collect::<Vec<_>>();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(fault) if fault.code == "LSV2501"))
            .count(),
        3
    );
    assert_eq!(
        observer.scheduler_pressure(100).unwrap().pending_dispatches,
        1
    );
}

#[test]
fn journal_insert_failures_roll_back_admission_for_the_whole_batch() {
    let path = JournalPath::new();
    let mut vm = path.vm(true, 2, 1);
    let connection = Connection::open(&path.0).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_dispatch BEFORE INSERT ON vm_dispatches BEGIN SELECT RAISE(ABORT, 'test dispatch failure'); END;").unwrap();
    assert!(matches!(
        start(&mut vm, &focus("single"), 100, 1000),
        Step::Fault(_)
    ));
    connection.execute_batch("DROP TRIGGER reject_dispatch; CREATE TRIGGER reject_dispatch BEFORE INSERT ON vm_dispatches WHEN (SELECT COUNT(*) FROM vm_dispatches) > 0 BEGIN SELECT RAISE(ABORT, 'test second dispatch failure'); END;").unwrap();
    let source = r#"fn main() = all(first: ui.focus(node_id: "first"), second: ui.focus(node_id: "second"))"#;
    assert!(matches!(start(&mut vm, source, 100, 1000), Step::Fault(_)));
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.scheduler_pressure(100).unwrap().pending_dispatches, 0);
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM vm_merge_groups", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    connection
        .execute_batch("DROP TRIGGER reject_dispatch;")
        .unwrap();
    assert!(matches!(
        start(&mut vm, source, 100, 1000),
        Step::Effects(_)
    ));
    assert_eq!(vm.scheduler_pressure(100).unwrap().pending_dispatches, 2);
}
