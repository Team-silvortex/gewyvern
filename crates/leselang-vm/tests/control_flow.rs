use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectError, EffectErrorClass, EffectRequest, EffectResult, PresentationOperation,
    PresentationResult, RetentionPolicy, RetryDisposition, RetryPolicy, Step, Value, Vm,
};

const FLOW: &str = r#"fn main() = seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"), third: ui.focus(node_id: "c"))"#;
static NEXT: AtomicU64 = AtomicU64::new(1);

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "leselang-control-{}-{}",
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

fn start(vm: &mut Vm, source: &str) -> Step {
    vm.start(
        &lower(&parse(source)).unwrap(),
        Principal {
            id: "operator".into(),
        },
        CapabilitySet::new(["ui.presentation"]),
        None,
    )
}
fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("expected effect, got {other:?}"),
    }
}
fn result(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
}
fn node(request: &EffectRequest) -> &str {
    let leselang_vm::EffectOperation::Presentation(envelope) = &request.operation else {
        panic!("expected presentation")
    };
    let PresentationOperation::Focus { node_id } = &envelope.operation else {
        panic!("expected focus")
    };
    node_id
}

#[test]
fn sequential_dispatch_and_direct_reentry_agree_in_both_journals() {
    let path = JournalPath::new();
    for mut vm in [Vm::default(), Vm::open_journal(&path.0, 100).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        assert_eq!(node(&first), "a");
        let last = vm.pending_continuations().into_iter().find(|image| {
            matches!(&image.pending_effect, leselang_hir::Effect::UiFocus { node_id } if node_id == "c")
        }).unwrap();
        assert!(
            matches!(vm.resume(&last, result("c")), Step::Fault(fault) if fault.code == "LSV2410")
        );
        let lease = vm.claim_effect(1, 100).unwrap().unwrap();
        assert_eq!(node(&lease.request), "a");
        assert!(vm.claim_effect(2, 100).unwrap().is_none());
        let second = effect(vm.acknowledge_effect(&lease, 2, result("a")));
        assert_eq!(node(&second), "b");
        assert_eq!(
            second.budget.fuel_remaining + 1,
            first.budget.fuel_remaining
        );
        let third = effect(vm.resume(&second.continuation, result("b")));
        assert_eq!(node(&third), "c");
        assert_eq!(effect(vm.resume(&first.continuation, result("a"))), third);
        let done = vm.resume(&third.continuation, result("c"));
        let Step::Done(Value::Structured { fields }) = &done else {
            panic!("expected completed flow: {done:?}")
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["first", "second", "third"]
        );
        assert_eq!(vm.resume(&first.continuation, result("a")), done);
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(3, 100).unwrap().is_none());
        let extra = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "extra")"#));
        assert!(matches!(
            vm.resume(&extra.continuation, result("extra")),
            Step::Done(_)
        ));
        let report = vm
            .compact_journal(&RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: 1,
            })
            .unwrap();
        assert_eq!(report.removed_records, 1);
        assert_eq!(vm.completed_count(), 1);
    }
}

#[test]
fn sequential_journal_restart_and_competing_workers_never_release_two_steps() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let first = effect(start(&mut vm, FLOW));
    let lease = vm.claim_effect(1, 10).unwrap().unwrap();
    drop(vm);
    let mut recovered = Vm::open_journal(&path.0, 100).unwrap();
    let mut peer = Vm::open_journal(&path.0, 100).unwrap();
    assert!(recovered.claim_effect(5, 10).unwrap().is_none());
    let redelivery = recovered.claim_effect(11, 10).unwrap().unwrap();
    assert_eq!(redelivery.request, first);
    assert!(peer.claim_effect(12, 10).unwrap().is_none());
    assert!(matches!(
        peer.acknowledge_effect(&lease, 12, result("a")),
        Step::Fault(_)
    ));
    let second = effect(recovered.acknowledge_effect(&redelivery, 12, result("a")));
    let next_lease = peer.claim_effect(13, 10).unwrap().unwrap();
    assert_eq!(next_lease.request, second);
    let third = effect(peer.acknowledge_effect(&next_lease, 14, result("b")));
    drop(peer);
    drop(recovered);
    let mut recovered = Vm::open_journal(&path.0, 100).unwrap();
    assert_eq!(
        recovered.pending_continuations(),
        std::slice::from_ref(&third.continuation)
    );
    let done = recovered.resume(&third.continuation, result("c"));
    assert!(matches!(done, Step::Done(_)));
    drop(recovered);
    let mut recovered = Vm::open_journal(&path.0, 100).unwrap();
    assert_eq!(recovered.resume(&first.continuation, result("a")), done);
}

#[test]
fn failure_or_bad_result_stops_every_unexecuted_step() {
    for durable in [false, true] {
        for bad_result in [false, true] {
            let path = JournalPath::new();
            let mut vm = if durable {
                Vm::open_journal(&path.0, 100).unwrap()
            } else {
                Vm::default()
            };
            let first = effect(start(&mut vm, FLOW));
            let lease = vm.claim_effect(1, 100).unwrap().unwrap();
            let terminal = if bad_result {
                vm.acknowledge_effect(&lease, 2, result("wrong-target"))
            } else {
                let RetryDisposition::Terminal(step) = vm
                    .report_effect_error(
                        &lease,
                        2,
                        EffectError {
                            class: EffectErrorClass::Permanent,
                            code: "adapter_failed".into(),
                            message: "unavailable".into(),
                        },
                        &RetryPolicy::default(),
                    )
                    .unwrap()
                else {
                    panic!("expected permanent failure")
                };
                step
            };
            assert!(matches!(terminal, Step::Failed(_) | Step::Fault(_)));
            assert_eq!(vm.pending_count(), 0);
            assert!(vm.claim_effect(3, 100).unwrap().is_none());
            assert_eq!(vm.resume(&first.continuation, result("a")), terminal);
            if durable {
                drop(vm);
                let mut vm = Vm::open_journal(&path.0, 100).unwrap();
                assert_eq!(vm.pending_count(), 0);
                assert!(vm.claim_effect(4, 100).unwrap().is_none());
                assert_eq!(vm.resume(&first.continuation, result("a")), terminal);
            }
        }
    }
}

#[test]
fn retry_keeps_successors_blocked_and_cancellation_fences_the_whole_flow() {
    let path = JournalPath::new();
    for mut vm in [Vm::default(), Vm::open_journal(&path.0, 100).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        let lease = vm.claim_effect(1, 100).unwrap().unwrap();
        assert!(matches!(
            vm.report_effect_error(
                &lease,
                2,
                EffectError {
                    class: EffectErrorClass::Transient,
                    code: "busy".into(),
                    message: "try later".into()
                },
                &RetryPolicy::default()
            )
            .unwrap(),
            RetryDisposition::Scheduled(_)
        ));
        assert!(vm.claim_effect(100, 100).unwrap().is_none());
        let retry = vm.claim_effect(252, 100).unwrap().unwrap();
        assert_eq!(retry.request, first);
        let cancelled = vm.cancel_effect(&first.continuation, 253);
        assert!(matches!(cancelled, Step::Cancelled(_)));
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(1000, 100).unwrap().is_none());
        assert_eq!(vm.acknowledge_effect(&retry, 254, result("a")), cancelled);
    }
}

#[test]
fn bounded_repeat_has_distinct_effect_identities_and_one_shared_fuel_budget() {
    let source = r#"fn main() = repeat(times: 3, body: ui.focus(node_id: "a"))"#;
    let mut limited = Vm::new(2);
    assert!(matches!(start(&mut limited, source), Step::Fault(fault) if fault.code == "LSV1001"));
    assert_eq!(limited.pending_count(), 0);
    let mut vm = Vm::new(3);
    let mut step = start(&mut vm, source);
    let mut identities = std::collections::BTreeSet::new();
    for fuel in [2, 1, 0] {
        let request = effect(step);
        assert_eq!(request.budget.fuel_remaining, fuel);
        assert!(identities.insert(request.effect_id.clone()));
        step = vm.resume(&request.continuation, result("a"));
    }
    assert!(matches!(step, Step::Done(Value::Structured { fields }) if fields.len() == 3));
    let source = r#"fn main() = repeat(times: 1, body: ui.focus(node_id: "a"))"#;
    let request = effect(start(&mut vm, source));
    assert!(
        matches!(vm.resume(&request.continuation, result("a")), Step::Done(Value::Structured { fields }) if fields.len() == 1)
    );
}

#[test]
fn sequential_deadline_covers_the_entire_flow() {
    let path = JournalPath::new();
    for mut vm in [Vm::default(), Vm::open_journal(&path.0, 100).unwrap()] {
        let first = effect(vm.start_timed(
            &lower(&parse(FLOW)).unwrap(),
            Principal {
                id: "operator".into(),
            },
            CapabilitySet::new(["ui.presentation"]),
            None,
            100,
            20,
        ));
        let second = effect(vm.resume_at(&first.continuation, 119, result("a")));
        let step = vm.resume_at(&second.continuation, 120, result("b"));
        assert!(matches!(step, Step::Cancelled(_)));
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(120, 10).unwrap().is_none());
    }
}

#[test]
fn journal_order_metadata_cannot_be_downgraded_to_parallel() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    start(&mut vm, FLOW);
    drop(vm);
    let connection = rusqlite::Connection::open(&path.0).unwrap();
    connection
        .execute(
            "UPDATE vm_merge_groups SET execution_order = 'parallel'",
            [],
        )
        .unwrap();
    drop(connection);
    assert!(Vm::open_journal(&path.0, 100).is_err());
}

#[test]
fn old_journal_migrates_without_reinterpreting_parallel_execution() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let source = r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#;
    assert!(matches!(start(&mut vm, source), Step::Effects(_)));
    drop(vm);
    let connection = rusqlite::Connection::open(&path.0).unwrap();
    connection
        .execute_batch(
            "ALTER TABLE vm_merge_groups DROP COLUMN execution_order; PRAGMA user_version = 6;",
        )
        .unwrap();
    drop(connection);
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    assert!(vm.claim_effect(1, 100).unwrap().is_some());
    assert!(vm.claim_effect(2, 100).unwrap().is_some());
}

#[test]
fn commands_in_sequences_keep_authorization_revision_and_lease_fences() {
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::InMemoryControlPlane;
    let source = r#"fn main() = seq(refresh: runtime.refresh(runtime_id: "a"), focus: ui.focus(node_id: "a"))"#;
    let program = lower(&parse(source)).unwrap();
    let mut vm = Vm::default();
    let principal = Principal {
        id: "operator".into(),
    };
    let rejected = vm.start(
        &program,
        principal.clone(),
        CapabilitySet::new(["runtime.refresh"]),
        Some(Revision(1)),
    );
    assert!(matches!(rejected, Step::Fault(fault) if fault.code == "LSH2001"));
    assert_eq!(vm.pending_count(), 0);
    let first = effect(vm.start(
        &program,
        principal,
        CapabilitySet::new(["runtime.refresh", "ui.presentation"]),
        Some(Revision(1)),
    ));
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    let leselang_vm::EffectOperation::Command(command) = lease.request.operation.clone() else {
        panic!("expected command")
    };
    assert_eq!(command.expected_revision, Some(Revision(1)));
    let mut control = InMemoryControlPlane::default();
    control.register_runtime(RuntimeId::new("a").unwrap(), "A", "http://runtime-a");
    let command_result = EffectResult::Command(Box::new(control.execute(command).unwrap()));
    assert!(
        matches!(vm.resume(&first.continuation, command_result.clone()), Step::Fault(fault) if fault.code == "LSV2110")
    );
    let second = effect(vm.acknowledge_effect(&lease, 2, command_result));
    assert_eq!(node(&second), "a");
    assert_eq!(second.continuation.expected_revision, Some(Revision(1)));
}

#[test]
fn failed_sequence_finalization_rolls_back_predecessor_and_successor_states_together() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let first = effect(start(&mut vm, FLOW));
    let connection = rusqlite::Connection::open(&path.0).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER reject_sequence_stop BEFORE UPDATE ON vm_effects
         WHEN OLD.token = 'continuation-4' BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
        )
        .unwrap();
    assert!(matches!(
        vm.resume(&first.continuation, result("wrong")),
        Step::Fault(_)
    ));
    assert_eq!(vm.pending_count(), 3);
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM vm_effects WHERE state = 'completed'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    connection
        .execute_batch("DROP TRIGGER reject_sequence_stop;")
        .unwrap();
    assert!(matches!(
        vm.resume(&first.continuation, result("wrong")),
        Step::Fault(_)
    ));
    assert_eq!(vm.pending_count(), 0);
    assert!(vm.claim_effect(1, 100).unwrap().is_none());
}

#[test]
fn recovery_rejects_dispatch_or_failure_history_on_blocked_sequences() {
    for failed_predecessor in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100).unwrap();
        let first = effect(start(&mut vm, FLOW));
        drop(vm);
        let connection = rusqlite::Connection::open(&path.0).unwrap();
        if failed_predecessor {
            let step = Step::Fault(leselang_vm::Fault {
                code: "test_failure".into(),
                message: "failed".into(),
            });
            connection.execute("UPDATE vm_effects SET state = 'completed', terminal_step = ?1 WHERE token = ?2",
                rusqlite::params![serde_json::to_vec(&step).unwrap(), first.continuation.token.as_str()]).unwrap();
            connection
                .execute(
                    "UPDATE vm_dispatches SET state = 'acknowledged' WHERE token = ?1",
                    [first.continuation.token.as_str()],
                )
                .unwrap();
        } else {
            connection
                .execute(
                    "UPDATE vm_dispatches SET attempt = 1 WHERE token = 'continuation-4'",
                    [],
                )
                .unwrap();
        }
        drop(connection);
        assert!(Vm::open_journal(&path.0, 100).is_err());
    }
}
