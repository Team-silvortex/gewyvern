use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{UiFocusNavigationDirection, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    DebuggerCancelResult, EffectError, EffectErrorClass, EffectOperation, EffectRequest,
    EffectResult, PresentationOperation, PresentationResult, RetentionPolicy, RetryDisposition,
    RetryPolicy, Step, Value, Vm, decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};

const FLOW: &str = r#"fn main() = bind(prefix: "target-", body:
    bind(r: ui.navigate_focus(node_id: "a", direction: "next"), body:
        ui.focus(node_id: concat(left: prefix, right: field(value: r, name: "focused_node_id")))))"#;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-successor-{}-{}",
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
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "runtime.read"]),
        Some(Revision(7)),
    )
}
fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("expected effect: {other:?}"),
    }
}
fn navigate(destination: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::NavigateFocus {
        node_id: "a".into(),
        direction: UiFocusNavigationDirection::Next,
        focused_node_id: destination.into(),
    })
}
fn focus(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
}
fn node(request: &EffectRequest) -> &str {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!("not presentation")
    };
    let PresentationOperation::Focus { node_id } = &envelope.operation else {
        panic!("not focus")
    };
    node_id
}
fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn result_driven_successors_inherit_authority_and_budget_in_both_journals() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(100), Vm::open_journal(&path.0, 100).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        assert_eq!(first.continuation.schema_version, 3);
        assert_eq!(
            decode_continuation(&encode_continuation(&first.continuation).unwrap()).unwrap(),
            first.continuation
        );
        let next = effect(vm.resume(&first.continuation, navigate("b")));
        assert_eq!(node(&next), "target-b");
        assert_eq!(next.continuation.schema_version, 1);
        assert!(next.continuation.result_binding.is_none());
        assert!(next.budget.fuel_remaining < first.budget.fuel_remaining);
        assert_eq!(next.budget.deadline_ms, first.budget.deadline_ms);
        assert_eq!(next.budget.max_output_items, first.budget.max_output_items);
        assert_eq!(
            next.continuation.expected_revision,
            first.continuation.expected_revision
        );
        let (EffectOperation::Presentation(a), EffectOperation::Presentation(b)) =
            (&first.operation, &next.operation)
        else {
            panic!("presentation expected")
        };
        assert_eq!(a.principal, b.principal);
        assert_eq!(a.capabilities, b.capabilities);
        assert_ne!(first.effect_id, next.effect_id);
        assert_eq!(
            vm.pending_continuations(),
            std::slice::from_ref(&next.continuation)
        );
        assert_eq!(
            effect(vm.resume(&first.continuation, navigate("ignored"))),
            next
        );
        let done = vm.resume(&next.continuation, focus("target-b"));
        assert_eq!(
            done,
            Step::Done(Value::UiFocus {
                node_id: "target-b".into()
            })
        );
        assert_eq!(vm.resume(&first.continuation, navigate("ignored")), done);
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(1, 10).unwrap().is_none());
    }
}

#[test]
fn restart_and_competing_results_replay_the_first_committed_successor() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let first = effect(start(&mut vm, FLOW));
    drop(vm);
    let mut winner = Vm::open_journal(&path.0, 1).unwrap();
    let mut peer = Vm::open_journal(&path.0, 1).unwrap();
    let next = effect(winner.resume(&first.continuation, navigate("winner")));
    assert_eq!(
        effect(peer.resume(&first.continuation, navigate("loser"))),
        next
    );
    assert_eq!(
        peer.pending_continuations(),
        std::slice::from_ref(&next.continuation)
    );
    let connection = Connection::open(&path.0).unwrap();
    assert_eq!(count(&connection, "vm_effects"), 2);
    assert_eq!(count(&connection, "vm_merge_groups"), 1);
    drop(winner);
    drop(peer);
    let mut restored = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        restored.pending_continuations(),
        std::slice::from_ref(&next.continuation)
    );
    let lease = restored.claim_effect(1, 100).unwrap().unwrap();
    assert_eq!(lease.request, next);
    let done = restored.acknowledge_effect(&lease, 2, focus("target-winner"));
    assert!(matches!(done, Step::Done(_)));
    drop(restored);
    let mut restored = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        restored.resume(&first.continuation, navigate("changed")),
        done
    );
    assert!(restored.claim_effect(3, 100).unwrap().is_none());
}

#[test]
fn successor_admission_is_atomic_at_every_write_boundary() {
    for (table, operation) in [
        ("vm_merge_groups", "INSERT"),
        ("vm_effects", "UPDATE"),
        ("vm_dispatches", "UPDATE"),
        ("vm_effects", "INSERT"),
        ("vm_dispatches", "INSERT"),
        ("vm_merge_branches", "INSERT"),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100).unwrap();
        let first = effect(start(&mut vm, FLOW));
        let connection = Connection::open(&path.0).unwrap();
        connection.execute_batch(&format!("CREATE TRIGGER reject_transition BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
        assert!(matches!(
            vm.resume(&first.continuation, navigate("lost")),
            Step::Fault(_)
        ));
        assert_eq!(
            vm.pending_continuations(),
            std::slice::from_ref(&first.continuation)
        );
        assert_eq!(count(&connection, "vm_effects"), 1);
        assert_eq!(count(&connection, "vm_merge_groups"), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 100).unwrap();
        assert_eq!(
            vm.pending_continuations(),
            std::slice::from_ref(&first.continuation)
        );
        connection
            .execute_batch("DROP TRIGGER reject_transition;")
            .unwrap();
        let next = effect(vm.resume(&first.continuation, navigate("retry")));
        assert_eq!(node(&next), "target-retry");
        assert_eq!(count(&connection, "vm_effects"), 2);
    }
}

#[test]
fn leases_fence_successor_creation_and_duplicate_acknowledgements() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(100), Vm::open_journal(&path.0, 100).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        let stale = vm.claim_effect(1, 10).unwrap().unwrap();
        assert!(
            matches!(vm.resume(&first.continuation, navigate("unleased")), Step::Fault(f) if f.code == "LSV4024")
        );
        assert!(
            matches!(vm.acknowledge_effect(&stale, 12, navigate("expired")), Step::Fault(f) if f.code == "LSV4023")
        );
        let active = vm.claim_effect(12, 10).unwrap().unwrap();
        assert!(
            matches!(vm.acknowledge_effect(&stale, 13, navigate("stale")), Step::Fault(f) if f.code == "LSV4022")
        );
        let next = effect(vm.acknowledge_effect(&active, 13, navigate("active")));
        assert_eq!(node(&next), "target-active");
        assert_eq!(
            effect(vm.acknowledge_effect(&stale, 14, navigate("changed"))),
            next
        );
        assert_eq!(vm.claim_effect(14, 10).unwrap().unwrap().request, next);
        assert!(vm.claim_effect(15, 10).unwrap().is_none());
    }
}

#[test]
fn image_only_restore_cannot_mint_authority_for_a_successor() {
    let mut original = Vm::new(100);
    let request = effect(start(&mut original, FLOW));
    let mut vm = Vm::new(1);
    assert_eq!(
        vm.restore(request.continuation.clone()).unwrap_err().code,
        "LSV1407"
    );
    vm.restore_request(request.clone()).unwrap();
    assert_eq!(
        node(&effect(vm.resume(&request.continuation, navigate("b")))),
        "target-b"
    );
    let source = r#"fn main() = bind(r: runtime.list(), body: ui.focus(node_id: "a"))"#;
    let mut request = effect(start(&mut original, source));
    let EffectOperation::Query(envelope) = &mut request.operation else {
        panic!("query expected")
    };
    envelope.capabilities = CapabilitySet::new(["runtime.read"]);
    assert!(Vm::default().restore_request(request).is_err());
    let mut wrong_schema = effect(start(&mut original, FLOW)).continuation;
    wrong_schema.schema_version = 2;
    assert!(encode_continuation(&wrong_schema).is_err());
}

#[test]
fn bad_host_results_and_computation_faults_never_enqueue_a_successor() {
    for durable in [false, true] {
        for (source, result) in [
            (FLOW, focus("a")),
            (
                r#"fn main() = bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: concat(left: field(value: r, name: "node_id"), right: " invalid")))"#,
                focus("a"),
            ),
            (
                r#"fn main() = bind(r: ui.focus(node_id: "a"), body: choose(when: eq(left: div(left: 1, right: 0), right: 1), then: ui.focus(node_id: "b"), otherwise: ui.focus(node_id: "c")))"#,
                focus("a"),
            ),
        ] {
            let path = JournalPath::new();
            let mut vm = if durable {
                Vm::open_journal(&path.0, 100).unwrap()
            } else {
                Vm::new(100)
            };
            let first = effect(start(&mut vm, source));
            let terminal = vm.resume(&first.continuation, result);
            assert!(matches!(terminal, Step::Fault(_)), "{terminal:?}");
            assert_eq!(vm.pending_count(), 0);
            assert!(vm.claim_effect(1, 10).unwrap().is_none());
            assert_eq!(vm.resume(&first.continuation, navigate("retry")), terminal);
            if durable {
                drop(vm);
                let mut vm = Vm::open_journal(&path.0, 100).unwrap();
                assert_eq!(vm.resume(&first.continuation, navigate("retry")), terminal);
                assert_eq!(
                    count(&Connection::open(&path.0).unwrap(), "vm_merge_groups"),
                    0
                );
            }
        }
    }
}

#[test]
fn conditional_successor_is_lazy_and_shares_the_original_fuel() {
    let source = r#"fn main() = bind(r: ui.focus(node_id: "a"), body:
        choose(when: eq(left: field(value: r, name: "node_id"), right: "a"),
            then: ui.focus(node_id: "chosen"), otherwise: ui.focus(node_id: concat(left: "bad", right: " target"))))"#;
    let mut vm = Vm::new(100);
    let first = effect(start(&mut vm, source));
    assert_eq!(
        node(&effect(vm.resume(&first.continuation, focus("a")))),
        "chosen"
    );
    let mut request = effect(start(&mut vm, FLOW));
    request.continuation.fuel_remaining = 1;
    request.budget.fuel_remaining = 1;
    let mut restored = Vm::new(1000);
    restored.restore_request(request.clone()).unwrap();
    assert!(
        matches!(restored.resume(&request.continuation, navigate("b")), Step::Fault(f) if f.code == "LSV1001")
    );
    assert_eq!(restored.pending_count(), 0);
}

#[test]
fn cancellation_and_deadline_stop_the_current_chain_without_restarting_time() {
    for durable in [false, true] {
        for advance in [false, true] {
            let path = JournalPath::new();
            let mut vm = if durable {
                Vm::open_journal(&path.0, 100).unwrap()
            } else {
                Vm::new(100)
            };
            let first = effect(start(&mut vm, FLOW));
            let current = if advance {
                let next = effect(vm.resume(&first.continuation, navigate("b")));
                assert_eq!(effect(vm.cancel_effect(&first.continuation, 1)), next);
                next
            } else {
                first.clone()
            };
            let cancelled = vm.cancel_effect(&current.continuation, 2);
            assert!(matches!(cancelled, Step::Cancelled(_)));
            assert_eq!(vm.resume(&first.continuation, navigate("late")), cancelled);
            assert!(vm.claim_effect(3, 100).unwrap().is_none());
            if durable {
                drop(vm);
                let mut vm = Vm::open_journal(&path.0, 100).unwrap();
                assert_eq!(vm.resume(&first.continuation, navigate("late")), cancelled);
            }
        }
    }
    let mut vm = Vm::new(100);
    let first = effect(vm.start_timed(
        &lower(&parse(FLOW)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
        100,
        20,
    ));
    let next = effect(vm.resume_at(&first.continuation, 110, navigate("b")));
    assert_eq!(next.budget.deadline_at_ms, Some(120));
    assert!(matches!(
        vm.resume_at(&next.continuation, 120, focus("target-b")),
        Step::Cancelled(_)
    ));
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn retention_protects_active_chains_and_removes_completed_chains_as_one_unit() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(100), Vm::open_journal(&path.0, 100).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        let next = effect(vm.resume(&first.continuation, navigate("b")));
        let policy = RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 1,
        };
        assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 0);
        assert!(matches!(
            vm.resume(&next.continuation, focus("target-b")),
            Step::Done(_)
        ));
        let later = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "later")"#));
        assert!(matches!(
            vm.resume(&later.continuation, focus("later")),
            Step::Done(_)
        ));
        assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 1);
        assert_eq!(vm.completed_count(), 1);
    }
    let connection = Connection::open(&path.0).unwrap();
    for table in ["vm_merge_groups", "vm_merge_branches"] {
        assert_eq!(count(&connection, table), 0);
    }
    assert_eq!(count(&connection, "vm_effects"), 1);
    assert_eq!(count(&connection, "vm_dispatches"), 1);
    assert_eq!(Vm::open_journal(&path.0, 100).unwrap().pending_count(), 0);
}

#[test]
fn schema_seven_migrates_without_rewriting_old_continuations() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let atomic = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    let binding = effect(start(
        &mut vm,
        r#"fn main() = bind(r: ui.focus(node_id: "b"), body: true)"#,
    ));
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    connection
        .execute_batch("PRAGMA user_version = 7;")
        .unwrap();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        leselang_vm::JOURNAL_SCHEMA_VERSION
    );
    assert_eq!(
        vm.pending_continuations(),
        [atomic.continuation.clone(), binding.continuation.clone()]
    );
    assert!(matches!(
        vm.resume(&atomic.continuation, focus("a")),
        Step::Done(_)
    ));
    assert!(matches!(
        vm.resume(&binding.continuation, focus("b")),
        Step::Done(_)
    ));
}

#[test]
fn recovery_rejects_orphaned_and_authority_changed_result_chains() {
    for corruption in [
        "orphan",
        "missing_authority",
        "changed_principal",
        "changed_order",
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100).unwrap();
        let first = effect(start(&mut vm, FLOW));
        let mut next = effect(vm.resume(&first.continuation, navigate("b")));
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        match corruption {
            "orphan" => {
                connection
                    .execute("DELETE FROM vm_merge_branches", [])
                    .unwrap();
            }
            "missing_authority" => {
                connection
                    .execute(
                        "DELETE FROM vm_dispatches WHERE token = ?1",
                        [first.continuation.token.as_str()],
                    )
                    .unwrap();
            }
            "changed_principal" => {
                let EffectOperation::Presentation(envelope) = &mut next.operation else {
                    panic!("presentation expected")
                };
                envelope.principal = Principal::new("other").unwrap();
                connection
                    .execute(
                        "UPDATE vm_dispatches SET request = ?2 WHERE token = ?1",
                        params![
                            next.continuation.token.as_str(),
                            serde_json::to_vec(&next).unwrap()
                        ],
                    )
                    .unwrap();
            }
            "changed_order" => {
                connection.execute("UPDATE vm_merge_groups SET plan = ?1", [serde_json::to_vec(&serde_json::json!({"order":"sequential","branches":["result","successor"]})).unwrap()]).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(Vm::open_journal(&path.0, 100).is_err(), "{corruption}");
    }
}

#[test]
fn successor_commands_keep_confirmation_and_cannot_bypass_correlated_acknowledgement() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let program = lower(&parse(
        r#"fn main() = bind(r: ui.focus(node_id: "a"),
        body: debugger.cancel(session_id: field(value: r, name: "node_id")))"#,
    ))
    .unwrap();
    assert!(matches!(
        vm.start(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            Some(Revision(7))
        ),
        Step::Fault(_)
    ));
    assert_eq!(vm.pending_count(), 0);
    let first = effect(vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "debugger.control"]),
        Some(Revision(7)),
    ));
    let next = effect(vm.resume(&first.continuation, focus("a")));
    let EffectOperation::Command(command) = &next.operation else {
        panic!("command expected")
    };
    assert_eq!(
        command.confirmation,
        leselang_host_contract::Confirmation::Confirmed
    );
    assert_eq!(next.continuation.expected_revision, Some(Revision(7)));
    let result = EffectResult::DebuggerCancel(DebuggerCancelResult {
        command_id: command.command_id.clone(),
        session_id: "a".into(),
        observed_at_ms: 2,
    });
    assert!(
        matches!(vm.resume(&next.continuation, result.clone()), Step::Fault(f) if f.code == "LSV2110")
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    assert_eq!(lease.request, next);
    let done = vm.acknowledge_effect(&lease, 2, result);
    assert!(matches!(done, Step::Done(Value::DebuggerCancel { .. })));
    assert_eq!(vm.resume(&first.continuation, focus("ignored")), done);
}

#[test]
fn result_revision_and_output_caps_are_checked_before_tail_selection() {
    let source = r#"fn main() = bind(r: runtime.list(), body: ui.focus(node_id:
        choose(when: eq(left: loop(n: field(value: r, name: "count"),
            while: lt(left: n, right: 2), next: add(left: n, right: 1), limit: 2), right: 2),
            then: "ready", otherwise: "waiting")))"#;
    let result = |revision| {
        EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
            revision: Revision(revision),
            runtimes: vec![],
        })
    };
    let mut vm = Vm::new(100);
    let first = effect(start(&mut vm, source));
    assert!(
        matches!(vm.resume(&first.continuation, result(8)), Step::Fault(f) if f.code == "LSV2101")
    );
    assert_eq!(vm.pending_count(), 0);
    let first = effect(start(&mut vm, source));
    assert_eq!(
        node(&effect(vm.resume(&first.continuation, result(7)))),
        "ready"
    );
    let mut first = effect(start(&mut vm, source));
    first.continuation.max_output_items = 0;
    first.budget.max_output_items = 0;
    let mut restored = Vm::new(100);
    restored.restore_request(first.clone()).unwrap();
    assert!(
        matches!(restored.resume(&first.continuation, result(7)), Step::Fault(f) if f.code == "LSV2102")
    );
    assert_eq!(restored.pending_count(), 0);
}

#[test]
fn semantic_retry_does_not_release_or_recreate_the_successor() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(100), Vm::open_journal(&path.0, 100).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        let lease = vm.claim_effect(1, 100).unwrap().unwrap();
        assert!(matches!(
            vm.report_effect_error(
                &lease,
                2,
                EffectError {
                    class: EffectErrorClass::Transient,
                    code: "busy".into(),
                    message: "retry".into(),
                },
                &RetryPolicy::default()
            )
            .unwrap(),
            RetryDisposition::Scheduled(_)
        ));
        assert!(vm.claim_effect(100, 100).unwrap().is_none());
        assert_eq!(
            vm.pending_continuations(),
            std::slice::from_ref(&first.continuation)
        );
        let retry = vm.claim_effect(252, 100).unwrap().unwrap();
        assert_eq!(retry.request, first);
        let next = effect(vm.acknowledge_effect(&retry, 253, navigate("retried")));
        let lease = vm.claim_effect(254, 100).unwrap().unwrap();
        assert_eq!(lease.request, next);
        let RetryDisposition::Terminal(failed) = vm
            .report_effect_error(
                &lease,
                255,
                EffectError {
                    class: EffectErrorClass::Permanent,
                    code: "unavailable".into(),
                    message: "stop".into(),
                },
                &RetryPolicy::default(),
            )
            .unwrap()
        else {
            panic!("terminal failure expected")
        };
        assert!(matches!(failed, Step::Failed(_)));
        assert_eq!(vm.resume(&first.continuation, navigate("changed")), failed);
        assert!(vm.claim_effect(256, 100).unwrap().is_none());
    }
}
