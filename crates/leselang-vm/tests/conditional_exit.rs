use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{UiFocusNavigationDirection, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    CONDITIONAL_CONTINUATION_SCHEMA_VERSION, EffectRequest, EffectResult, PresentationResult,
    RetentionPolicy, ScalarValue, Step, Value, Vm, decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};

const FLOW: &str = r#"fn main() = bind(first: ui.navigate_focus(node_id: "a", direction: "next"), body:
    choose(when: eq(left: field(value: first, name: "focused_node_id"), right: "done"), then: false,
        otherwise: bind(second: ui.navigate_focus(node_id: field(value: first, name: "focused_node_id"), direction: "next"), body:
            choose(when: eq(left: field(value: second, name: "focused_node_id"), right: "done"), then: true,
                otherwise: bind(last: ui.focus(node_id: field(value: second, name: "focused_node_id")), body:
                    eq(left: field(value: first, name: "node_id"), right: field(value: last, name: "node_id")))))))"#;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-conditional-{}-{}",
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
fn start(vm: &mut Vm, source: &str) -> EffectRequest {
    effect(vm.start(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "runtime.read"]),
        Some(Revision(7)),
    ))
}
fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("expected effect: {other:?}"),
    }
}
fn navigate(node: &str, destination: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::NavigateFocus {
        node_id: node.into(),
        direction: UiFocusNavigationDirection::Next,
        focused_node_id: destination.into(),
    })
}
fn boolean(value: bool) -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(value),
    })
}
fn count(path: &JournalPath, table: &str) -> i64 {
    Connection::open(&path.0)
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn either_guard_can_complete_without_admitting_unselected_effects() {
    for stop_at in [1, 2, 3] {
        let path = JournalPath::new();
        for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
            let first = start(&mut vm, FLOW);
            assert_eq!(
                first.continuation.schema_version,
                CONDITIONAL_CONTINUATION_SCHEMA_VERSION
            );
            let mut tokens = vec![first.continuation.clone()];
            let mut step = vm.resume(
                &first.continuation,
                navigate("a", if stop_at == 1 { "done" } else { "b" }),
            );
            if stop_at > 1 {
                let second = effect(step);
                assert_eq!(
                    second.continuation.schema_version,
                    CONDITIONAL_CONTINUATION_SCHEMA_VERSION
                );
                assert!(second.budget.fuel_remaining < first.budget.fuel_remaining);
                assert_eq!(
                    second.continuation.expected_revision,
                    first.continuation.expected_revision
                );
                tokens.push(second.continuation.clone());
                step = vm.resume(
                    &second.continuation,
                    navigate("b", if stop_at == 2 { "done" } else { "a" }),
                );
            }
            if stop_at == 3 {
                let third = effect(step);
                assert_eq!(third.continuation.schema_version, 4);
                tokens.push(third.continuation.clone());
                step = vm.resume(
                    &third.continuation,
                    EffectResult::Presentation(PresentationResult::Focus {
                        node_id: "a".into(),
                    }),
                );
            }
            assert_eq!(step, boolean(stop_at != 1));
            assert_eq!(vm.pending_count(), 0);
            assert!(vm.claim_effect(1, 100).unwrap().is_none());
            for token in tokens {
                assert_eq!(vm.resume(&token, navigate("a", "changed")), step);
            }
        }
        assert_eq!(count(&path, "vm_effects"), stop_at);
        assert_eq!(count(&path, "vm_merge_groups"), i64::from(stop_at > 1));
    }
}

#[test]
fn conditional_choices_and_early_terminal_values_survive_every_restart() {
    for early in [true, false] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = start(&mut vm, FLOW);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        let second = effect(vm.resume(&first.continuation, navigate("a", "b")));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            effect(vm.resume(&first.continuation, navigate("a", "done"))),
            second
        );
        let mut step = vm.resume(
            &second.continuation,
            navigate("b", if early { "done" } else { "a" }),
        );
        if !early {
            let third = effect(step);
            drop(vm);
            vm = Vm::open_journal(&path.0, 1).unwrap();
            step = vm.resume(
                &third.continuation,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "a".into(),
                }),
            );
        }
        assert_eq!(step, boolean(true));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&first.continuation, navigate("a", "done")), step);
        assert_eq!(
            vm.resume(&second.continuation, navigate("b", "changed")),
            step
        );
        assert_eq!(vm.pending_count(), 0);
    }
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = start(&mut vm, FLOW);
    assert_eq!(
        vm.resume(&first.continuation, navigate("a", "done")),
        boolean(false)
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume(&first.continuation, navigate("a", "b")),
        boolean(false)
    );
}

#[test]
fn first_commit_wins_between_early_return_and_continuation_workers() {
    for early_wins in [true, false] {
        let path = JournalPath::new();
        let mut winner = Vm::open_journal(&path.0, 500).unwrap();
        let first = start(&mut winner, FLOW);
        let mut peer = Vm::open_journal(&path.0, 1).unwrap();
        let chosen = winner.resume(
            &first.continuation,
            navigate("a", if early_wins { "done" } else { "b" }),
        );
        assert_eq!(
            peer.resume(
                &first.continuation,
                navigate("a", if early_wins { "b" } else { "done" })
            ),
            chosen
        );
        if !early_wins {
            let second = effect(chosen);
            assert_eq!(
                peer.resume(&second.continuation, navigate("b", "done")),
                boolean(true)
            );
            assert_eq!(
                winner.resume(&first.continuation, navigate("a", "done")),
                boolean(true)
            );
        }
        assert_eq!(winner.pending_count(), 0);
        assert_eq!(peer.pending_count(), 0);
        assert_eq!(count(&path, "vm_effects"), if early_wins { 1 } else { 2 });
    }
}

#[test]
fn early_exit_rolls_back_with_its_group_when_any_terminal_write_fails() {
    for table in ["vm_effects", "vm_dispatches", "vm_merge_groups"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = start(&mut vm, FLOW);
        let second = effect(vm.resume(&first.continuation, navigate("a", "b")));
        let connection = Connection::open(&path.0).unwrap();
        connection.execute_batch(&format!(
            "CREATE TRIGGER fail_exit BEFORE UPDATE ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;"
        )).unwrap();
        assert!(matches!(
            vm.resume(&second.continuation, navigate("b", "done")),
            Step::Fault(_)
        ));
        drop(vm);
        connection.execute_batch("DROP TRIGGER fail_exit;").unwrap();
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.pending_continuations(),
            std::slice::from_ref(&second.continuation)
        );
        assert_eq!(
            vm.resume(&second.continuation, navigate("b", "done")),
            boolean(true)
        );
        drop(vm);
        assert_eq!(Vm::open_journal(&path.0, 1).unwrap().pending_count(), 0);
        assert_eq!(count(&path, "vm_effects"), 2);
    }
}

#[test]
fn conditional_frames_require_authority_and_reject_legacy_version_downgrades() {
    let mut vm = Vm::new(500);
    let first = start(&mut vm, FLOW);
    let second = effect(vm.resume(&first.continuation, navigate("a", "b")));
    for request in [first, second] {
        assert_eq!(
            decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap(),
            request.continuation
        );
        assert!(Vm::new(500).restore(request.continuation.clone()).is_err());
        for version in [1, 2, 3, 4] {
            let mut old = request.clone();
            old.continuation.schema_version = version;
            assert!(Vm::new(500).restore_request(old).is_err());
        }
        let mut restored = Vm::new(1);
        restored.restore_request(request.clone()).unwrap();
        let node = if request
            .continuation
            .result_binding
            .as_ref()
            .unwrap()
            .results
            .is_empty()
        {
            "a"
        } else {
            "b"
        };
        assert_eq!(
            restored.resume(&request.continuation, navigate(node, "done")),
            boolean(node == "b")
        );
    }
}

#[test]
fn recovery_rejects_scalar_child_links_wrong_terminal_types_and_orphaned_successors() {
    for corruption in ["scalar_parent", "wrong_scalar", "raw_terminal", "orphan"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = start(&mut vm, FLOW);
        let second = effect(vm.resume(&first.continuation, navigate("a", "b")));
        if corruption != "orphan" {
            assert_eq!(
                vm.resume(&second.continuation, navigate("b", "done")),
                boolean(true)
            );
        }
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        if corruption == "orphan" {
            connection
                .execute("DELETE FROM vm_merge_branches", [])
                .unwrap();
        } else {
            let (token, step) = match corruption {
                "scalar_parent" => (&first.continuation.token, boolean(false)),
                "wrong_scalar" => (
                    &second.continuation.token,
                    Step::Done(Value::Scalar {
                        value: ScalarValue::Integer(1),
                    }),
                ),
                "raw_terminal" => (
                    &second.continuation.token,
                    Step::Done(Value::UiNavigateFocus {
                        node_id: "b".into(),
                        direction: UiFocusNavigationDirection::Next,
                        focused_node_id: "done".into(),
                    }),
                ),
                _ => unreachable!(),
            };
            connection
                .execute(
                    "UPDATE vm_effects SET terminal_step = ?2 WHERE token = ?1",
                    params![token.as_str(), serde_json::to_vec(&step).unwrap()],
                )
                .unwrap();
        }
        assert!(Vm::open_journal(&path.0, 500).is_err(), "{corruption}");
    }
}

#[test]
fn recovery_cannot_skip_a_required_capture_before_a_later_conditional_exit() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = start(
        &mut vm,
        r#"fn main() = bind(a: ui.focus(node_id: "a"), body:
        bind(b: ui.focus(node_id: "b"), body: choose(when: true, then: true,
            otherwise: bind(c: ui.focus(node_id: "c"), body: false))))"#,
    );
    assert_eq!(
        first.continuation.schema_version,
        CONDITIONAL_CONTINUATION_SCHEMA_VERSION
    );
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    connection
        .execute(
            "UPDATE vm_effects SET state = 'completed', terminal_step = ?1",
            [serde_json::to_vec(&boolean(true)).unwrap()],
        )
        .unwrap();
    connection
        .execute("UPDATE vm_dispatches SET state = 'acknowledged'", [])
        .unwrap();
    assert!(Vm::open_journal(&path.0, 500).is_err());
}

#[test]
fn recovery_preserves_the_final_type_across_individually_valid_frames() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = start(&mut vm, FLOW);
    let mut second = effect(vm.resume(&first.continuation, navigate("a", "b")));
    drop(vm);
    second.continuation.schema_version = 4;
    second.continuation.result_binding.as_mut().unwrap().body =
        leselang_hir::computation::Computation::Literal {
            value: ScalarValue::Integer(1),
        };
    assert!(Vm::new(1).restore_request(second.clone()).is_ok());
    let connection = Connection::open(&path.0).unwrap();
    connection
        .execute(
            "UPDATE vm_effects SET image = ?2 WHERE token = ?1",
            params![
                second.continuation.token.as_str(),
                serde_json::to_vec(&second.continuation).unwrap()
            ],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE vm_dispatches SET request = ?2 WHERE token = ?1",
            params![
                second.continuation.token.as_str(),
                serde_json::to_vec(&second).unwrap()
            ],
        )
        .unwrap();
    assert!(Vm::open_journal(&path.0, 500).is_err());
}

#[test]
fn early_exit_compacts_as_a_whole_chain_and_old_dataflow_journals_migrate_unchanged() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let legacy_source = r#"fn main() = bind(a: ui.focus(node_id: "a"), body: bind(b: ui.focus(node_id: "a"), body: true))"#;
    let first = start(&mut vm, legacy_source);
    assert_eq!(first.continuation.schema_version, 4);
    let second = effect(vm.resume(
        &first.continuation,
        EffectResult::Presentation(PresentationResult::Focus {
            node_id: "a".into(),
        }),
    ));
    let bytes = encode_continuation(&second.continuation).unwrap();
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    connection
        .execute_batch("PRAGMA user_version = 9;")
        .unwrap();
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        leselang_vm::JOURNAL_SCHEMA_VERSION
    );
    assert_eq!(
        encode_continuation(&vm.pending_continuations()[0]).unwrap(),
        bytes
    );
    assert_eq!(
        vm.resume(
            &second.continuation,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "a".into()
            })
        ),
        boolean(true)
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = start(&mut vm, FLOW);
    let second = effect(vm.resume(&first.continuation, navigate("a", "b")));
    assert_eq!(
        vm.resume(&second.continuation, navigate("b", "done")),
        boolean(true)
    );
    let first = start(&mut vm, FLOW);
    assert_eq!(
        vm.resume(&first.continuation, navigate("a", "done")),
        boolean(false)
    );
    let report = vm
        .compact_journal(&RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 10,
        })
        .unwrap();
    assert_eq!(report.removed_records, 2);
    assert_eq!(count(&path, "vm_effects"), 1);
    assert_eq!(count(&path, "vm_merge_groups"), 0);
}

#[test]
fn concurrent_exit_and_successor_commits_choose_exactly_one_outcome() {
    for _ in 0..6 {
        let path = JournalPath::new();
        let mut left = Vm::open_journal(&path.0, 500).unwrap();
        let first = start(&mut left, FLOW);
        let mut right = Vm::open_journal(&path.0, 1).unwrap();
        let barrier = std::sync::Barrier::new(2);
        let (a, b) = std::thread::scope(|scope| {
            let left = scope.spawn(|| {
                barrier.wait();
                left.resume(&first.continuation, navigate("a", "done"))
            });
            let right = scope.spawn(|| {
                barrier.wait();
                right.resume(&first.continuation, navigate("a", "b"))
            });
            (left.join().unwrap(), right.join().unwrap())
        });
        assert_eq!(a, b);
        let mut restored = Vm::open_journal(&path.0, 1).unwrap();
        match a {
            Step::Done(_) => {
                assert_eq!(a, boolean(false));
                assert_eq!(count(&path, "vm_effects"), 1);
            }
            Step::Effect(request) => {
                assert_eq!(count(&path, "vm_effects"), 2);
                assert_eq!(
                    restored.resume(&request.continuation, navigate("b", "done")),
                    boolean(true)
                );
            }
            other => panic!("unexpected concurrent outcome: {other:?}"),
        }
    }
}

#[test]
fn cold_work_is_lazy_but_selected_arithmetic_failures_remain_terminal() {
    let source = r#"fn main() = bind(r: ui.navigate_focus(node_id: "a", direction: "next"), body:
        choose(when: eq(left: field(value: r, name: "focused_node_id"), right: "done"), then: true,
            otherwise: bind(s: ui.focus(node_id: "a"), body: eq(left: div(left: 1, right: 0), right: 0))))"#;
    for early in [true, false] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = start(&mut vm, source);
        let mut step = vm.resume(
            &first.continuation,
            navigate("a", if early { "done" } else { "b" }),
        );
        if early {
            assert_eq!(step, boolean(true));
        } else {
            let second = effect(step);
            step = vm.resume(
                &second.continuation,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "a".into(),
                }),
            );
            assert!(matches!(step, Step::Fault(_)));
        }
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume(&first.continuation, navigate("a", "changed")),
            step
        );
        assert_eq!(count(&path, "vm_effects"), if early { 1 } else { 2 });
    }
}

#[test]
fn conditional_exit_keeps_dispatch_leases_and_shared_fuel_fenced() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = start(&mut vm, FLOW);
    let first_lease = vm.claim_effect(1, 100).unwrap().unwrap();
    let second = effect(vm.acknowledge_effect(&first_lease, 2, navigate("a", "b")));
    let stale = vm.claim_effect(3, 10).unwrap().unwrap();
    assert!(
        matches!(vm.resume(&second.continuation, navigate("b", "done")), Step::Fault(f) if f.code == "LSV4024")
    );
    let active = vm.claim_effect(13, 100).unwrap().unwrap();
    assert!(
        matches!(vm.acknowledge_effect(&stale, 14, navigate("b", "done")), Step::Fault(f) if f.code == "LSV4022")
    );
    assert_eq!(
        vm.acknowledge_effect(&active, 15, navigate("b", "done")),
        boolean(true)
    );
    assert_eq!(
        vm.acknowledge_effect(&stale, 16, navigate("b", "a")),
        boolean(true)
    );
    drop(vm);
    assert_eq!(Vm::open_journal(&path.0, 1).unwrap().pending_count(), 0);

    let mut exhausted = first;
    exhausted.continuation.fuel_remaining = 1;
    exhausted.budget.fuel_remaining = 1;
    let mut vm = Vm::new(1_000_000);
    vm.restore_request(exhausted.clone()).unwrap();
    assert!(
        matches!(vm.resume(&exhausted.continuation, navigate("a", "done")), Step::Fault(f) if f.code == "LSV1001")
    );
}

#[test]
fn cancellation_and_deadline_win_over_an_early_return() {
    for deadline in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(vm.start_timed(
            &lower(&parse(FLOW)).unwrap(),
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
            100,
            100,
        ));
        let second = effect(vm.resume_at(&first.continuation, 110, navigate("a", "b")));
        assert_eq!(second.budget.deadline_at_ms, Some(200));
        let terminal = if deadline {
            vm.resume_at(&second.continuation, 200, navigate("b", "done"))
        } else {
            vm.cancel_effect(&second.continuation, 120)
        };
        assert!(matches!(terminal, Step::Cancelled(_)));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 210, navigate("a", "done")),
            terminal
        );
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(count(&path, "vm_effects"), 2);
    }
}
