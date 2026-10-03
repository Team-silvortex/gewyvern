use leselang_hir::{HirProgram, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectError, EffectErrorClass, EffectOperation, EffectRequest, EffectResult, RetryDisposition,
    RetryPolicy, ScalarValue, Step, Value, Vm, decode_continuation, encode_continuation,
};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-concurrency-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("flow.sqlite3"))
    }
}
impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}
fn program(source: &str) -> HirProgram {
    lower(&parse(source)).unwrap()
}
fn start(vm: &mut Vm, program: &HirProgram, principal: &str, revision: u64, timeout: u64) -> Step {
    vm.start_timed(
        program,
        Principal::new(principal).unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(revision)),
        100,
        timeout,
    )
}
fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("{other:?}"),
    }
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}
fn done(value: bool) -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(value),
    })
}
fn flow() -> HirProgram {
    program(
        r#"fn read() = bind(result: ui.assert_text(node_id: "status", expected: "ready"), body: field(value: result, name: "expected"))
        fn main() = bind(answer: choose(when: true, then: read(), otherwise: "fallback"), body: bind(written: ui.set_form_value(node_id: "form", field: "answer", value: answer), body: true))"#,
    )
}

#[test]
fn long_lived_worker_claims_effects_admitted_after_its_snapshot() {
    let path = JournalPath::new();
    let mut admitting = Vm::open_journal(&path.0, 10000).unwrap();
    let mut worker = Vm::open_journal(&path.0, 1).unwrap();
    let first = effect(start(&mut admitting, &flow(), "operator", 7, 100));
    assert_eq!(worker.pending_count(), 0);
    let lease = worker.claim_effect(101, 50).unwrap().unwrap();
    assert_eq!(lease.request, first);
    let caller = effect(worker.acknowledge_effect(&lease, 102, receipt(&first)));
    let next = worker.claim_effect(103, 50).unwrap().unwrap();
    assert_eq!(next.request, caller);
    assert!(caller.continuation.fuel_remaining < first.continuation.fuel_remaining);
    assert_eq!(
        worker.acknowledge_effect(&next, 104, receipt(&caller)),
        done(true)
    );
    assert_eq!(
        admitting.resume_at(&first.continuation, 105, receipt(&first)),
        done(true)
    );
    assert_eq!(worker.pending_count(), 0);
}

#[test]
fn long_lived_workers_complete_sequential_and_parallel_groups_without_reopening() {
    for kind in ["seq", "all"] {
        let path = JournalPath::new();
        let mut admitting = Vm::open_journal(&path.0, 10000).unwrap();
        let mut left = Vm::open_journal(&path.0, 1).unwrap();
        let mut right = Vm::open_journal(&path.0, 1).unwrap();
        let mut tail = Vm::open_journal(&path.0, 1).unwrap();
        let source = format!(
            r#"fn read() = bind(group: {kind}(first: ui.assert_text(node_id: "a", expected: "re"), second: ui.assert_text(node_id: "b", expected: "ady")), body: concat(left: field(value: member(value: group, name: "first"), name: "expected"), right: field(value: member(value: group, name: "second"), name: "expected")))
            fn main() = bind(answer: choose(when: true, then: read(), otherwise: "fallback"), body: bind(written: ui.set_form_value(node_id: "form", field: "answer", value: answer), body: true))"#
        );
        let first = start(&mut admitting, &program(&source), "operator", 7, 100);
        assert!(matches!(first, Step::Effect(_) | Step::Effects(_)));
        let a = left.claim_effect(101, 50).unwrap().unwrap();
        let current = if kind == "seq" {
            assert!(right.claim_effect(101, 50).unwrap().is_none());
            let next = effect(left.acknowledge_effect(&a, 102, receipt(&a.request)));
            let b = right.claim_effect(103, 50).unwrap().unwrap();
            assert_eq!(b.request, next);
            effect(right.acknowledge_effect(&b, 104, receipt(&b.request)))
        } else {
            let b = right.claim_effect(101, 50).unwrap().unwrap();
            assert_ne!(a.request.effect_id, b.request.effect_id);
            assert!(matches!(
                right.acknowledge_effect(&b, 102, receipt(&b.request)),
                Step::Waiting(_)
            ));
            assert!(tail.claim_effect(103, 50).unwrap().is_none());
            effect(left.acknowledge_effect(&a, 104, receipt(&a.request)))
        };
        let lease = tail.claim_effect(105, 50).unwrap().unwrap();
        assert_eq!(lease.request, current);
        assert!(
            matches!(&current.operation, EffectOperation::Presentation(envelope) if matches!(&envelope.operation, leselang_vm::PresentationOperation::SetFormValue { value, .. } if value == "ready"))
        );
        assert_eq!(
            tail.acknowledge_effect(&lease, 106, receipt(&lease.request)),
            done(true)
        );
        assert_eq!(
            left.acknowledge_effect(&a, 107, receipt(&a.request)),
            done(true)
        );
        assert!(admitting.claim_effect(108, 50).unwrap().is_none());
        assert_eq!(
            Connection::open(&path.0)
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM vm_effects WHERE state = 'completed'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            3
        );
    }
}

#[test]
fn one_engine_interleaves_roots_without_sharing_locals_authority_or_budgets() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = if durable {
            Vm::open_journal(&path.0, 10000).unwrap()
        } else {
            Vm::new(10000)
        };
        let root = |label| {
            program(&format!(
                r#"fn read() = bind(result: ui.assert_text(node_id: "{label}", expected: "{label}"), body: field(value: result, name: "expected"))
            fn main() = bind(answer: choose(when: true, then: read(), otherwise: "fallback"), body: bind(written: ui.set_form_value(node_id: "form-{label}", field: "answer", value: answer), body: true))"#
            ))
        };
        let a = effect(start(&mut vm, &root("alpha"), "operator-a", 7, 50));
        let b = effect(start(&mut vm, &root("bravo"), "operator-b", 8, 100));
        assert_ne!(a.effect_id, b.effect_id);
        assert_eq!(a.continuation.fuel_remaining, b.continuation.fuel_remaining);
        let snapshot = encode_continuation(&b.continuation).unwrap();
        let mut forged = a.continuation.clone();
        forged.token = b.continuation.token.clone();
        assert!(
            matches!(vm.resume_at(&forged, 101, receipt(&a)), Step::Fault(fault) if fault.code == "LSV2005")
        );
        let cancelled = vm.cancel_effect(&a.continuation, 102);
        assert!(matches!(cancelled, Step::Cancelled(_)));
        assert_eq!(decode_continuation(&snapshot).unwrap(), b.continuation);
        let caller = effect(vm.resume_at(&b.continuation, 103, receipt(&b)));
        assert_eq!(caller.continuation.expected_revision, Some(Revision(8)));
        assert_eq!(caller.continuation.deadline_at_ms, Some(200));
        assert!(caller.continuation.fuel_remaining < b.continuation.fuel_remaining);
        assert!(
            matches!(&caller.operation, EffectOperation::Presentation(envelope) if envelope.principal == Principal::new("operator-b").unwrap() && matches!(&envelope.operation, leselang_vm::PresentationOperation::SetFormValue { node_id, value, .. } if node_id == "form-bravo" && value == "bravo"))
        );
        assert_eq!(
            vm.resume_at(&caller.continuation, 104, receipt(&caller)),
            done(true)
        );
        assert_eq!(vm.resume_at(&a.continuation, 105, receipt(&a)), cancelled);
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn independent_engines_move_to_distinct_host_threads_with_shared_immutable_hir() {
    fn assert_send<T: Send>() {}
    assert_send::<Vm>();
    let program = flow();
    let barrier = std::sync::Barrier::new(4);
    let first_ids = std::thread::scope(|scope| {
        let mut threads = Vec::new();
        for index in 0..4 {
            let mut vm = Vm::new(10000);
            let program = &program;
            let barrier = &barrier;
            threads.push(scope.spawn(move || {
                barrier.wait();
                let mut first_id = String::new();
                for _ in 0..16 {
                    let first = effect(start(
                        &mut vm,
                        program,
                        &format!("operator-{index}"),
                        7,
                        100,
                    ));
                    if first_id.is_empty() {
                        first_id = first.effect_id.clone();
                    }
                    let caller = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
                    assert_eq!(
                        vm.resume_at(&caller.continuation, 102, receipt(&caller)),
                        done(true)
                    );
                }
                assert_eq!(vm.pending_count(), 0);
                assert_eq!(vm.completed_count(), 32);
                first_id
            }));
        }
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    // Local IDs intentionally collide across isolated journals: the host must scope routing.
    assert_eq!(first_ids, vec!["effect-1"; 4]);
}

#[test]
fn expired_dispatch_reassignment_is_attempt_fenced_between_long_lived_workers() {
    let path = JournalPath::new();
    let mut admitting = Vm::open_journal(&path.0, 10000).unwrap();
    let mut left = Vm::open_journal(&path.0, 1).unwrap();
    let mut right = Vm::open_journal(&path.0, 1).unwrap();
    let program = program(r#"fn main() = bind(result: ui.focus(node_id: "a"), body: true)"#);
    let first = effect(start(&mut admitting, &program, "operator", 7, 100));
    let old = left.claim_effect(101, 10).unwrap().unwrap();
    assert!(right.claim_effect(110, 10).unwrap().is_none());
    let current = right.claim_effect(111, 10).unwrap().unwrap();
    assert_eq!(old.request, current.request);
    assert_eq!(current.request, first);
    assert_eq!(current.attempt, old.attempt + 1);
    assert!(
        matches!(left.acknowledge_effect(&old, 112, receipt(&old.request)), Step::Fault(fault) if fault.code == "LSV4022")
    );
    assert_eq!(
        right.acknowledge_effect(&current, 113, receipt(&current.request)),
        done(true)
    );
    assert_eq!(
        left.acknowledge_effect(&old, 114, receipt(&old.request)),
        done(true)
    );
}

#[test]
fn semantic_retry_does_not_block_an_unrelated_root_or_refill_execution_budget() {
    let path = JournalPath::new();
    let mut admitting = Vm::open_journal(&path.0, 10000).unwrap();
    let mut worker = Vm::open_journal(&path.0, 1).unwrap();
    for label in ["alpha", "bravo"] {
        let program = program(&format!(
            r#"fn main() = bind(result: ui.focus(node_id: "{label}"), body: true)"#
        ));
        effect(start(&mut admitting, &program, "operator", 7, 1000));
    }
    let first = worker.claim_effect(101, 50).unwrap().unwrap();
    let RetryDisposition::Scheduled(schedule) = worker
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
        panic!()
    };
    let unrelated = worker.claim_effect(103, 50).unwrap().unwrap();
    assert_ne!(first.request.effect_id, unrelated.request.effect_id);
    assert_eq!(
        worker.acknowledge_effect(&unrelated, 104, receipt(&unrelated.request)),
        done(true)
    );
    assert!(
        worker
            .claim_effect(schedule.ready_at_ms - 1, 50)
            .unwrap()
            .is_none()
    );
    let retry = worker
        .claim_effect(schedule.ready_at_ms, 50)
        .unwrap()
        .unwrap();
    assert_eq!(retry.request, first.request);
    assert_eq!(retry.retry_count, 1);
    assert_eq!(
        worker.acknowledge_effect(&retry, schedule.ready_at_ms + 1, receipt(&retry.request)),
        done(true)
    );
    assert!(
        worker
            .claim_effect(schedule.ready_at_ms + 2, 50)
            .unwrap()
            .is_none()
    );
}

#[test]
fn claim_rejects_a_dispatch_image_conflict_before_leasing_or_adopting_work() {
    let path = JournalPath::new();
    let mut admitting = Vm::open_journal(&path.0, 10000).unwrap();
    let mut worker = Vm::open_journal(&path.0, 1).unwrap();
    let first = effect(start(&mut admitting, &flow(), "operator", 7, 100));
    let mut changed = first.clone();
    changed.continuation.expected_revision = Some(Revision(8));
    let connection = Connection::open(&path.0).unwrap();
    connection
        .execute(
            "UPDATE vm_dispatches SET request = ?1 WHERE token = ?2",
            rusqlite::params![
                serde_json::to_vec(&changed).unwrap(),
                first.continuation.token.as_str()
            ],
        )
        .unwrap();
    assert_eq!(worker.claim_effect(101, 50).unwrap_err().code, "LSV4014");
    assert_eq!(worker.pending_count(), 0);
    assert_eq!(
        connection
            .query_row("SELECT state, attempt FROM vm_dispatches", [], |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?
            )))
            .unwrap(),
        ("ready".into(), 0)
    );
    connection
        .execute(
            "UPDATE vm_dispatches SET request = ?1 WHERE token = ?2",
            rusqlite::params![
                serde_json::to_vec(&first).unwrap(),
                first.continuation.token.as_str()
            ],
        )
        .unwrap();
    assert_eq!(
        worker.claim_effect(102, 50).unwrap().unwrap().request,
        first
    );
}

#[test]
fn simultaneous_workers_claim_distinct_members_of_multiple_roots() {
    let path = JournalPath::new();
    let mut admitting = Vm::open_journal(&path.0, 10000).unwrap();
    let workers = (0..4)
        .map(|_| Vm::open_journal(&path.0, 1).unwrap())
        .collect::<Vec<_>>();
    for label in ["alpha", "bravo"] {
        let source = format!(
            r#"fn main() = bind(group: all(first: ui.focus(node_id: "{label}-first"), second: ui.focus(node_id: "{label}-second")), body: true)"#
        );
        assert!(matches!(
            start(&mut admitting, &program(&source), "operator", 7, 100),
            Step::Effects(_)
        ));
    }
    let barrier = std::sync::Barrier::new(4);
    let completed = std::thread::scope(|scope| {
        let threads = workers
            .into_iter()
            .map(|mut worker| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    let lease = worker.claim_effect(101, 50).unwrap().unwrap();
                    let step = worker.acknowledge_effect(&lease, 102, receipt(&lease.request));
                    (lease.request.effect_id, step)
                })
            })
            .collect::<Vec<_>>();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        completed
            .iter()
            .map(|(id, _)| id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
    // A sibling can commit before visible progress is read, so both replies may be Done.
    assert!(
        completed
            .iter()
            .filter(|(_, step)| *step == done(true))
            .count()
            >= 2
    );
    assert!(
        completed
            .iter()
            .all(|(_, step)| matches!(step, Step::Waiting(_) | Step::Done(_)))
    );
    let mut recovered = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(recovered.pending_count(), 0);
    assert!(recovered.claim_effect(103, 50).unwrap().is_none());
    assert_eq!(
        Connection::open(&path.0)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM vm_merge_groups WHERE terminal_step IS NOT NULL",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
}
