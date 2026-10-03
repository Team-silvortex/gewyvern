use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    DispatchLease, EffectError, EffectErrorClass, EffectOperation, EffectRequest, EffectResult,
    MAX_DISPATCH_ATTEMPTS, RetryDisposition, RetryPolicy, ScalarValue, Step, Value, Vm,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-dispatch-scheduling-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("flow.sqlite3"))
    }

    fn vm(&self, durable: bool) -> Vm {
        if durable {
            Vm::open_journal(&self.0, 10000).unwrap()
        } else {
            Vm::new(10000)
        }
    }
}
impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn start_source(vm: &mut Vm, source: &str, timeout: u64) -> Step {
    vm.start_timed(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        100,
        timeout,
    )
}

fn admit(vm: &mut Vm, label: &str) -> EffectRequest {
    let Step::Effect(request) = start_source(
        vm,
        &format!(r#"fn main() = bind(result: ui.focus(node_id: "{label}"), body: true)"#),
        5000,
    ) else {
        panic!("expected one pending effect")
    };
    *request
}

fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!("expected a presentation")
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}

fn done() -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(true),
    })
}

fn acknowledge(vm: &mut Vm, lease: &DispatchLease, now_ms: u64) {
    assert_eq!(
        vm.acknowledge_effect(lease, now_ms, receipt(&lease.request)),
        done()
    );
}

#[test]
fn equally_attempted_dispatches_use_numeric_admission_order() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        let requests = (0..12)
            .map(|index| admit(&mut vm, &format!("node-{index}")))
            .collect::<Vec<_>>();
        for (index, request) in requests.iter().enumerate() {
            if durable && index == 6 {
                drop(vm);
                vm = path.vm(true);
            }
            let lease = vm.claim_effect(101, 50).unwrap().unwrap();
            assert_eq!(&lease.request, request);
            assert_eq!(lease.attempt, 1);
            acknowledge(&mut vm, &lease, 102);
        }
        assert!(vm.claim_effect(103, 50).unwrap().is_none());
    }
}

#[test]
fn fixed_ready_cohort_advances_in_attempt_rounds_even_when_all_leases_expire() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        let requests = (0..12)
            .map(|index| admit(&mut vm, &format!("node-{index}")))
            .collect::<Vec<_>>();
        for round in 0..3 {
            if durable {
                drop(vm);
                vm = path.vm(true);
            }
            for (index, request) in requests.iter().enumerate() {
                // Every preceding lease is expired at this claim, not just at round boundaries.
                let now = 101 + u64::from(round) * requests.len() as u64 + index as u64;
                let lease = vm.claim_effect(now, 1).unwrap().unwrap();
                assert_eq!(&lease.request, request);
                assert_eq!(lease.attempt, round + 1);
                assert_eq!(lease.retry_count, 0);
            }
        }
        for request in &requests {
            let lease = vm.claim_effect(137, 50).unwrap().unwrap();
            assert_eq!(&lease.request, request);
            assert_eq!(lease.attempt, 4);
            acknowledge(&mut vm, &lease, 138);
        }
        assert!(vm.claim_effect(139, 50).unwrap().is_none());
    }
}

#[test]
fn delivery_attempt_exhaustion_does_not_poison_unrelated_ready_work() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        let abandoned = admit(&mut vm, "abandoned");
        for attempt in 1..=MAX_DISPATCH_ATTEMPTS {
            let lease = vm
                .claim_effect(100 + u64::from(attempt), 1)
                .unwrap()
                .unwrap();
            assert_eq!(lease.request, abandoned);
            assert_eq!(lease.attempt, attempt);
        }
        let healthy = admit(&mut vm, "healthy");
        if durable {
            drop(vm);
            vm = path.vm(true);
        }
        let now = 101 + u64::from(MAX_DISPATCH_ATTEMPTS);
        let lease = vm.claim_effect(now, 50).unwrap().unwrap();
        assert_eq!(lease.request, healthy);
        assert_eq!(lease.attempt, 1);
        acknowledge(&mut vm, &lease, now);
        assert_eq!(vm.claim_effect(now + 1, 50).unwrap_err().code, "LSV4017");
        assert!(matches!(
            vm.cancel_effect(&abandoned.continuation, now + 1),
            Step::Cancelled(_)
        ));
        assert!(vm.claim_effect(now + 2, 50).unwrap().is_none());
    }
}

#[test]
fn retry_not_before_clock_and_attempt_priority_survive_recovery() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        let retrying = admit(&mut vm, "retrying");
        let first = vm.claim_effect(101, 50).unwrap().unwrap();
        let RetryDisposition::Scheduled(schedule) = vm
            .report_effect_error(
                &first,
                102,
                EffectError {
                    class: EffectErrorClass::Transient,
                    code: "temporarily_unavailable".into(),
                    message: "temporary host failure".into(),
                },
                &RetryPolicy::default(),
            )
            .unwrap()
        else {
            panic!("expected a scheduled retry")
        };
        assert!(
            vm.claim_effect(schedule.ready_at_ms - 1, 50)
                .unwrap()
                .is_none()
        );
        let healthy = admit(&mut vm, "healthy");
        if durable {
            drop(vm);
            vm = path.vm(true);
        }
        let fresh = vm.claim_effect(schedule.ready_at_ms, 50).unwrap().unwrap();
        assert_eq!(fresh.request, healthy);
        acknowledge(&mut vm, &fresh, schedule.ready_at_ms);
        let retry = vm.claim_effect(schedule.ready_at_ms, 50).unwrap().unwrap();
        assert_eq!(retry.request, retrying);
        assert_eq!(retry.attempt, 2);
        assert_eq!(retry.retry_count, 1);
        acknowledge(&mut vm, &retry, schedule.ready_at_ms);
    }
}

#[test]
fn attempt_priority_never_bypasses_sequential_predecessors() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        assert!(matches!(
            start_source(
                &mut vm,
                r#"fn main() = bind(group: seq(first: ui.focus(node_id: "first"), second: ui.focus(node_id: "second")), body: true)"#,
                5000,
            ),
            Step::Effect(_)
        ));
        let old = vm.claim_effect(101, 10).unwrap().unwrap();
        let healthy = admit(&mut vm, "healthy");
        let independent = vm.claim_effect(102, 50).unwrap().unwrap();
        assert_eq!(independent.request, healthy);
        acknowledge(&mut vm, &independent, 102);
        assert!(vm.claim_effect(110, 50).unwrap().is_none());
        let current = vm.claim_effect(111, 50).unwrap().unwrap();
        assert_eq!(current.request, old.request);
        assert_eq!(current.attempt, 2);
        assert!(
            matches!(vm.acknowledge_effect(&old, 112, receipt(&old.request)), Step::Fault(fault) if fault.code == "LSV4022")
        );
        let Step::Effect(next) = vm.acknowledge_effect(&current, 112, receipt(&current.request))
        else {
            panic!("expected the sequential successor")
        };
        let tail = vm.claim_effect(113, 50).unwrap().unwrap();
        assert_eq!(tail.request, *next);
        assert_eq!(tail.attempt, 1);
        acknowledge(&mut vm, &tail, 114);
    }
}

#[test]
fn expired_and_cancelled_unattempted_roots_do_not_hold_the_queue() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        assert!(matches!(
            start_source(
                &mut vm,
                r#"fn main() = bind(result: ui.focus(node_id: "expired"), body: true)"#,
                1,
            ),
            Step::Effect(_)
        ));
        let cancelled = admit(&mut vm, "cancelled");
        let healthy = admit(&mut vm, "healthy");
        assert!(matches!(
            vm.cancel_effect(&cancelled.continuation, 100),
            Step::Cancelled(_)
        ));
        if durable {
            drop(vm);
            vm = path.vm(true);
        }
        let lease = vm.claim_effect(101, 50).unwrap().unwrap();
        assert_eq!(lease.request, healthy);
        acknowledge(&mut vm, &lease, 102);
        assert!(vm.claim_effect(103, 50).unwrap().is_none());
    }
}

#[test]
fn simultaneous_shared_workers_keep_attempt_rounds_across_restarts() {
    let path = JournalPath::new();
    let mut admitting = path.vm(true);
    let expected_ids = (0..8)
        .map(|index| admit(&mut admitting, &format!("node-{index}")).effect_id)
        .collect::<std::collections::BTreeSet<_>>();
    for round in 1..=3 {
        let workers = (0..8).map(|_| path.vm(true)).collect::<Vec<_>>();
        let barrier = std::sync::Barrier::new(8);
        let leases = std::thread::scope(|scope| {
            let threads = workers
                .into_iter()
                .map(|mut worker| {
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        worker
                            .claim_effect(100 + u64::from(round), 1)
                            .unwrap()
                            .unwrap()
                    })
                })
                .collect::<Vec<_>>();
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(
            leases
                .iter()
                .map(|lease| lease.request.effect_id.clone())
                .collect::<std::collections::BTreeSet<_>>(),
            expected_ids
        );
        assert!(leases.iter().all(|lease| lease.attempt == round));
        if round == 3 {
            for lease in leases {
                acknowledge(&mut admitting, &lease, 103);
            }
        }
    }
    assert!(admitting.claim_effect(104, 50).unwrap().is_none());
}

#[test]
fn existing_schema_ten_journals_gain_order_index_without_rewriting_requests() {
    let path = JournalPath::new();
    let mut vm = path.vm(true);
    let request = admit(&mut vm, "existing");
    let lease = vm.claim_effect(101, 1).unwrap().unwrap();
    drop(vm);
    let connection = rusqlite::Connection::open(&path.0).unwrap();
    let snapshot = || {
        connection
            .query_row(
                "SELECT request, attempt, lease_expires_at_ms FROM vm_dispatches",
                [],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    let before = snapshot();
    connection
        .execute_batch("DROP INDEX vm_dispatch_attempt_order_idx;")
        .unwrap();
    let mut worker = path.vm(true);
    assert_eq!(snapshot(), before);
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        leselang_vm::JOURNAL_SCHEMA_VERSION
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'vm_dispatch_attempt_order_idx'",
                [],
                |row| row.get::<_, u32>(0),
            )
            .unwrap(),
        1
    );
    let recovered = worker.claim_effect(102, 50).unwrap().unwrap();
    assert_eq!(recovered.request, request);
    assert_eq!(recovered.attempt, lease.attempt + 1);
    acknowledge(&mut worker, &recovered, 103);
}

#[test]
fn numeric_order_remains_exact_at_large_sequence_and_digit_boundaries() {
    for sequence in [999_u64, 999_999_999_999_999_999, i64::MAX as u64 - 20] {
        let path = JournalPath::new();
        drop(path.vm(true));
        rusqlite::Connection::open(&path.0)
            .unwrap()
            .execute(
                "UPDATE vm_metadata SET next_sequence = ?1",
                [i64::try_from(sequence).unwrap()],
            )
            .unwrap();
        let mut vm = path.vm(true);
        let requests = (0..12)
            .map(|index| admit(&mut vm, &format!("node-{index}")))
            .collect::<Vec<_>>();
        drop(vm);
        let mut worker = path.vm(true);
        for (index, request) in requests.iter().enumerate() {
            assert_eq!(
                request.continuation.token.as_str(),
                format!("continuation-{}", sequence + index as u64)
            );
            let lease = worker.claim_effect(101, 50).unwrap().unwrap();
            assert_eq!(&lease.request, request);
            acknowledge(&mut worker, &lease, 102);
        }
    }
}
