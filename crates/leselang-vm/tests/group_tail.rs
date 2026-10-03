use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{UiFocusNavigationDirection, UiSelectionState, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    DebuggerCancelResult, EffectError, EffectErrorClass, EffectOperation, EffectRequest,
    EffectResult, PresentationOperation, PresentationResult, RetentionPolicy, RetryDisposition,
    RetryPolicy, Step, Value, Vm, decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};

const FLOW: &str = r#"fn target(prefix: string, selected: boolean) = choose(
    when: selected, then: prefix, otherwise: "bad node")
fn main() = bind(prefix: "target-", body:
    bind(g: seq(move: ui.navigate_focus(node_id: "a", direction: "next"),
                state: ui.set_selection(node_id: "toggle", state: "selected")), body:
        bind(alias: g, body: ui.focus(node_id: target(
            prefix: concat(left: prefix, right: field(value: member(value: alias, name: "move"), name: "focused_node_id")),
            selected: field(value: member(value: g, name: "state"), name: "selected"))))))"#;
const ONE: &str = r#"fn main() = bind(g: seq(move: ui.navigate_focus(node_id: "a", direction: "next")),
    body: ui.focus(node_id: field(value: member(value: g, name: "move"), name: "focused_node_id")))"#;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-group-tail-{}-{}",
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
        CapabilitySet::new(["ui.presentation", "runtime.read", "debugger.control"]),
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
fn selected() -> EffectResult {
    EffectResult::Presentation(PresentationResult::SetSelection {
        node_id: "toggle".into(),
        state: UiSelectionState::Selected,
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
fn plan(connection: &Connection) -> serde_json::Value {
    let bytes: Vec<u8> = connection
        .query_row("SELECT plan FROM vm_merge_groups", [], |row| row.get(0))
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn write_plan(connection: &Connection, value: &serde_json::Value) {
    connection
        .execute(
            "UPDATE vm_merge_groups SET plan = ?1",
            [serde_json::to_vec(value).unwrap()],
        )
        .unwrap();
}

#[test]
fn the_whole_prefix_drives_one_tail_with_original_authority_and_budgets() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        let blocked = vm
            .pending_continuations()
            .into_iter()
            .find(|image| image.token != first.continuation.token)
            .unwrap();
        assert!(matches!(vm.resume(&blocked, selected()), Step::Fault(_)));
        assert_eq!(vm.pending_count(), 2);
        let second = effect(vm.resume(&first.continuation, navigate("winner")));
        assert_eq!(second.continuation, blocked);
        assert_eq!(vm.pending_count(), 1);
        let tail = effect(vm.resume(&second.continuation, selected()));
        assert_eq!(node(&tail), "target-winner");
        for request in [&first, &second, &tail] {
            assert_eq!(request.continuation.schema_version, 7);
            assert_eq!(
                request.continuation.group_result,
                first.continuation.group_result
            );
            assert!(request.continuation.result_binding.is_none());
            assert_eq!(
                decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap(),
                request.continuation
            );
        }
        assert!(tail.budget.fuel_remaining < second.budget.fuel_remaining);
        assert_eq!(tail.budget.max_output_items, first.budget.max_output_items);
        assert_eq!(tail.budget.deadline_ms, first.budget.deadline_ms);
        assert_eq!(
            tail.continuation.expected_revision,
            first.continuation.expected_revision
        );
        let (EffectOperation::Presentation(a), EffectOperation::Presentation(b)) =
            (&first.operation, &tail.operation)
        else {
            panic!()
        };
        assert_eq!(a.principal, b.principal);
        assert_eq!(a.capabilities, b.capabilities);
        assert_eq!(
            effect(vm.resume(&first.continuation, navigate("changed"))),
            tail
        );
        assert_eq!(effect(vm.resume(&second.continuation, selected())), tail);
        let done = vm.resume(&tail.continuation, focus("target-winner"));
        assert_eq!(
            done,
            Step::Done(Value::UiFocus {
                node_id: "target-winner".into()
            })
        );
        assert_eq!(vm.resume(&first.continuation, navigate("changed")), done);
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
}

#[test]
fn reservation_is_not_a_dispatch_and_restart_never_recalculates_the_tail() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, FLOW));
    let connection = Connection::open(&path.0).unwrap();
    let reserved = plan(&connection)["result_binding"]["successor_sequence"]
        .as_u64()
        .unwrap();
    assert_eq!(count(&connection, "vm_effects"), 2);
    assert_eq!(count(&connection, "vm_dispatches"), 2);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let second = effect(vm.resume(&first.continuation, navigate("winner")));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let tail = effect(vm.resume(&second.continuation, selected()));
    assert_eq!(
        tail.continuation.token.as_str(),
        format!("continuation-{reserved}")
    );
    assert_eq!(count(&connection, "vm_effects"), 3);
    assert_eq!(count(&connection, "vm_merge_groups"), 1);
    assert_eq!(
        plan(&connection)["branches"],
        serde_json::json!(["move", "state", "successor"])
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        effect(vm.resume(&first.continuation, navigate("changed"))),
        tail
    );
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    assert_eq!(lease.request, tail);
    let done = vm.acknowledge_effect(&lease, 2, focus("target-winner"));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&second.continuation, selected()), done);
    assert!(vm.claim_effect(3, 100).unwrap().is_none());
}

#[test]
fn competing_final_acknowledgements_append_exactly_one_tail() {
    for _ in 0..4 {
        let path = JournalPath::new();
        let mut left = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut left, ONE));
        let mut right = Vm::open_journal(&path.0, 1).unwrap();
        let barrier = std::sync::Barrier::new(2);
        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                barrier.wait();
                left.resume(&first.continuation, navigate("left"))
            });
            let b = scope.spawn(|| {
                barrier.wait();
                right.resume(&first.continuation, navigate("right"))
            });
            (a.join().unwrap(), b.join().unwrap())
        });
        assert_eq!(a, b);
        let tail = effect(a);
        assert!(matches!(node(&tail), "left" | "right"));
        assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 2);
        let mut recovered = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            effect(recovered.resume(&first.continuation, navigate("ignored"))),
            tail
        );
    }
}

#[test]
fn the_last_prefix_acknowledgement_and_tail_roll_back_at_every_write_boundary() {
    for (table, operation) in [
        ("vm_effects", "UPDATE"),
        ("vm_dispatches", "UPDATE"),
        ("vm_effects", "INSERT"),
        ("vm_dispatches", "INSERT"),
        ("vm_merge_groups", "UPDATE"),
        ("vm_merge_branches", "INSERT"),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut vm, FLOW));
        let second = effect(vm.resume(&first.continuation, navigate("winner")));
        let connection = Connection::open(&path.0).unwrap();
        let original = plan(&connection);
        connection.execute_batch(&format!("CREATE TRIGGER reject_transition BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
        assert!(matches!(
            vm.resume(&second.continuation, selected()),
            Step::Fault(_)
        ));
        assert_eq!(
            vm.pending_continuations(),
            std::slice::from_ref(&second.continuation)
        );
        assert_eq!(count(&connection, "vm_effects"), 2);
        assert_eq!(count(&connection, "vm_dispatches"), 2);
        assert_eq!(plan(&connection), original);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        connection
            .execute_batch("DROP TRIGGER reject_transition")
            .unwrap();
        let tail = effect(vm.resume(&second.continuation, selected()));
        assert_eq!(node(&tail), "target-winner");
        assert_eq!(count(&connection, "vm_effects"), 3);
    }
}

#[test]
fn leases_and_semantic_retries_fence_tail_admission() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
        let first = effect(start(&mut vm, ONE));
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
        assert!(matches!(
            vm.report_effect_error(
                &active,
                13,
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
        assert_eq!(vm.pending_count(), 1);
        let retry = vm.claim_effect(263, 100).unwrap().unwrap();
        let tail = effect(vm.acknowledge_effect(&retry, 264, navigate("active")));
        assert_eq!(
            effect(vm.acknowledge_effect(&stale, 265, navigate("changed"))),
            tail
        );
        assert_eq!(vm.claim_effect(265, 100).unwrap().unwrap().request, tail);
        assert!(vm.claim_effect(266, 100).unwrap().is_none());
    }
}

#[test]
fn preparation_faults_and_wrong_host_results_never_admit_a_tail() {
    for durable in [false, true] {
        for (source, result, code) in [
            (ONE.to_string(), focus("a"), "LSV2103"),
            (r#"fn main() = bind(g: seq(move: ui.navigate_focus(node_id: "a", direction: "next")), body: ui.focus(node_id: concat(left: field(value: member(value: g, name: "move"), name: "focused_node_id"), right: " invalid")))"#.into(), navigate("winner"), "LSV1404"),
            (r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: ui.focus(node_id: to_string(value: div(left: 1, right: 0))))"#.into(), focus("a"), "LSV1401"),
            (r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: ui.focus(node_id: to_string(value: parse_integer(value: "bad"))))"#.into(), focus("a"), "LSV1408"),
        ] {
            let path = JournalPath::new();
            let mut vm = if durable { Vm::open_journal(&path.0, 500).unwrap() } else { Vm::new(500) };
            let first = effect(start(&mut vm, &source));
            let terminal = vm.resume(&first.continuation, result);
            assert!(matches!(&terminal, Step::Fault(f) if f.code == code), "{terminal:?}");
            assert_eq!(vm.pending_count(), 0);
            assert!(vm.claim_effect(1, 100).unwrap().is_none());
            if durable {
                assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 1);
                drop(vm);
                let mut vm = Vm::open_journal(&path.0, 1).unwrap();
                assert_eq!(vm.resume(&first.continuation, navigate("changed")), terminal);
            }
        }
    }
}

#[test]
fn cancellation_deadlines_and_permanent_failures_seal_the_whole_unit() {
    for after_tail in [false, true] {
        for mode in ["cancel", "deadline", "host-error"] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 500).unwrap();
            let first = effect(vm.start_timed(
                &lower(&parse(ONE)).unwrap(),
                Principal::new("operator").unwrap(),
                CapabilitySet::new(["ui.presentation"]),
                Some(Revision(7)),
                100,
                100,
            ));
            let current = if after_tail {
                effect(vm.resume_at(&first.continuation, 101, navigate("winner")))
            } else {
                first.clone()
            };
            let terminal = match mode {
                "cancel" => vm.cancel_effect(&current.continuation, 102),
                "deadline" => vm.resume_at(
                    &current.continuation,
                    200,
                    if after_tail {
                        focus("winner")
                    } else {
                        navigate("winner")
                    },
                ),
                _ => {
                    let lease = vm.claim_effect(101, 50).unwrap().unwrap();
                    let RetryDisposition::Terminal(step) = vm
                        .report_effect_error(
                            &lease,
                            102,
                            EffectError {
                                class: EffectErrorClass::Permanent,
                                code: "rejected".into(),
                                message: "stop".into(),
                            },
                            &RetryPolicy::default(),
                        )
                        .unwrap()
                    else {
                        panic!()
                    };
                    step
                }
            };
            assert!(matches!(
                terminal,
                Step::Cancelled(_) | Step::Fault(_) | Step::Failed(_)
            ));
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            assert_eq!(
                vm.resume_at(&first.continuation, 201, navigate("changed")),
                terminal
            );
            assert_eq!(vm.pending_count(), 0);
            assert!(vm.claim_effect(202, 50).unwrap().is_none());
        }
    }
}

#[test]
fn every_group_tail_image_requires_its_complete_journal() {
    let mut vm = Vm::new(500);
    let first = effect(start(&mut vm, ONE));
    let tail = effect(vm.resume(&first.continuation, navigate("winner")));
    for request in [first, tail] {
        assert_eq!(
            Vm::default()
                .restore(request.continuation.clone())
                .unwrap_err()
                .code,
            "LSV1409"
        );
        assert_eq!(
            Vm::default().restore_request(request).unwrap_err().code,
            "LSV1409"
        );
    }
}

#[test]
fn corrupted_reservations_owners_budgets_and_schema_fail_closed() {
    for mutation in 0..10 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut vm, ONE));
        let tail = if mutation >= 5 {
            Some(effect(vm.resume(&first.continuation, navigate("winner"))))
        } else {
            None
        };
        if mutation == 9 {
            vm.resume(&tail.as_ref().unwrap().continuation, focus("winner"));
        }
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        let mut saved = plan(&connection);
        match mutation {
            0 => saved["result_binding"]["successor_sequence"] = serde_json::Value::Null,
            1 => saved["result_binding"]["successor_sequence"] = 1.into(),
            2 => saved["result_binding"]["successor_sequence"] = 999.into(),
            3 => {
                connection
                    .execute("UPDATE vm_metadata SET next_sequence = 1", [])
                    .unwrap();
            }
            4 => saved["order"] = "bound_sequential".into(),
            5 => saved["branches"][1] = "wrong".into(),
            6..=8 => {
                let tail = tail.unwrap();
                let mut request = serde_json::to_value(&tail).unwrap();
                match mutation {
                    6 => request["continuation"]["schema_version"] = 6.into(),
                    7 => request["continuation"]["group_result"] = "merge-999".into(),
                    _ => {
                        request["continuation"]["fuel_remaining"] =
                            first.continuation.fuel_remaining.into();
                        request["budget"]["fuel_remaining"] =
                            first.continuation.fuel_remaining.into();
                    }
                }
                connection
                    .execute(
                        "UPDATE vm_effects SET image = ?1 WHERE token = ?2",
                        params![
                            serde_json::to_vec(&request["continuation"]).unwrap(),
                            tail.continuation.token.as_str()
                        ],
                    )
                    .unwrap();
                connection
                    .execute(
                        "UPDATE vm_dispatches SET request = ?1 WHERE token = ?2",
                        params![
                            serde_json::to_vec(&request).unwrap(),
                            tail.continuation.token.as_str()
                        ],
                    )
                    .unwrap();
            }
            _ => {
                connection
                    .execute(
                        "UPDATE vm_merge_groups SET terminal_step = ?1",
                        [serde_json::to_vec(&Step::Done(Value::UiFocus {
                            node_id: "changed".into(),
                        }))
                        .unwrap()],
                    )
                    .unwrap();
            }
        }
        write_plan(&connection, &saved);
        assert!(Vm::open_journal(&path.0, 1).is_err(), "mutation {mutation}");
    }
}

#[test]
fn reservations_are_unique_and_cannot_alias_unrelated_effects_or_groups() {
    for alias in ["reservation", "effect", "group"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        start(&mut vm, ONE);
        let connection = Connection::open(&path.0).unwrap();
        let reserved = plan(&connection)["result_binding"]["successor_sequence"].clone();
        let other = if alias == "effect" {
            effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#))
        } else {
            effect(start(&mut vm, ONE))
        };
        drop(vm);
        let token = other
            .continuation
            .group_result
            .as_ref()
            .map(|token| token.as_str());
        if alias == "group" {
            let mut value = plan(&connection);
            value["result_binding"]["successor_sequence"] = token
                .unwrap()
                .strip_prefix("merge-")
                .unwrap()
                .parse::<u64>()
                .unwrap()
                .into();
            let bytes = serde_json::to_vec(&value).unwrap();
            connection
                .execute(
                    "UPDATE vm_merge_groups SET plan = ?1 WHERE token = ?2",
                    params![bytes, "merge-1"],
                )
                .unwrap();
        } else if let Some(token) = token {
            let bytes: Vec<u8> = connection
                .query_row(
                    "SELECT plan FROM vm_merge_groups WHERE token = ?1",
                    [token],
                    |row| row.get(0),
                )
                .unwrap();
            let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            value["result_binding"]["successor_sequence"] = reserved;
            connection
                .execute(
                    "UPDATE vm_merge_groups SET plan = ?1 WHERE token = ?2",
                    params![serde_json::to_vec(&value).unwrap(), token],
                )
                .unwrap();
        } else {
            let mut value = plan(&connection);
            value["result_binding"]["successor_sequence"] = other
                .continuation
                .token
                .as_str()
                .strip_prefix("continuation-")
                .unwrap()
                .parse::<u64>()
                .unwrap()
                .into();
            write_plan(&connection, &value);
        }
        assert!(Vm::open_journal(&path.0, 1).is_err());
    }
}

#[test]
fn repeat_computed_members_and_successor_name_collisions_stay_bounded() {
    for (group, member, size) in [
        (
            r#"seq(successor: ui.focus(node_id: "a"))"#.to_string(),
            "successor".to_string(),
            1,
        ),
        (
            r#"seq(a: ui.focus(node_id: concat(left: "", right: "a")))"#.to_string(),
            "a".to_string(),
            1,
        ),
        (
            r#"seq(nested: seq(a: ui.focus(node_id: "a")))"#.to_string(),
            "nested__a".to_string(),
            1,
        ),
        (
            r#"repeat(times: 2, body: ui.focus(node_id: "a"))"#.to_string(),
            "iteration_2".to_string(),
            2,
        ),
        (
            r#"repeat(times: 63, body: ui.focus(node_id: "a"))"#.to_string(),
            "iteration_63".to_string(),
            63,
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let mut step = start(
            &mut vm,
            &format!(
                r#"fn main() = bind(g: {group}, body: ui.focus(node_id: field(value: member(value: g, name: "{member}"), name: "node_id")))"#
            ),
        );
        for _ in 0..size {
            step = vm.resume(&effect(step).continuation, focus("a"));
        }
        let tail = effect(step);
        assert_eq!(node(&tail), "a");
        let saved = plan(&Connection::open(&path.0).unwrap());
        assert_eq!(saved["branches"].as_array().unwrap().len(), size + 1);
        assert_eq!(
            saved["branches"][size],
            if member == "successor" {
                "successor_1"
            } else {
                "successor"
            }
        );
        let done = vm.resume(&tail.continuation, focus("a"));
        assert!(matches!(done, Step::Done(Value::UiFocus { .. })));
        Vm::open_journal(&path.0, 1).unwrap();
    }
}

#[test]
fn live_groups_are_retained_and_completed_prefix_and_tail_are_pruned_together() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        let second = effect(vm.resume(&first.continuation, navigate("winner")));
        let policy = RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 10,
        };
        assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 0);
        let tail = effect(vm.resume(&second.continuation, selected()));
        assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 0);
        vm.resume(&tail.continuation, focus("target-winner"));
        let later = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
        vm.resume(&later.continuation, focus("a"));
        assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 1);
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 1);
    }
    let connection = Connection::open(&path.0).unwrap();
    for table in ["vm_effects", "vm_dispatches"] {
        assert_eq!(count(&connection, table), 1);
    }
    for table in ["vm_merge_groups", "vm_merge_branches"] {
        assert_eq!(count(&connection, table), 0);
    }
    Vm::open_journal(&path.0, 1).unwrap();
}

#[test]
fn cumulative_raw_outputs_and_revision_failures_stop_before_tail_projection() {
    let mut control = leserpent_domain::InMemoryControlPlane::default();
    let runtime = control.register_runtime(
        leselang_host_contract::RuntimeId::new("runtime-a").unwrap(),
        "runtime",
        "http://runtime.local",
    );
    let inventory = |revision, size| {
        EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
            revision: Revision(revision),
            runtimes: (0..size)
                .map(|index| {
                    let mut runtime = runtime.clone();
                    runtime.id =
                        leselang_host_contract::RuntimeId::new(format!("runtime-{index}")).unwrap();
                    runtime
                })
                .collect(),
        })
    };
    for bad_revision in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(
            &mut vm,
            r#"fn main() = bind(g: seq(a: runtime.list(), b: runtime.list()), body: ui.focus(node_id: to_string(value: field(value: member(value: g, name: "b"), name: "count"))))"#,
        ));
        let terminal = if bad_revision {
            vm.resume(&first.continuation, inventory(8, 0))
        } else {
            let second = effect(vm.resume(&first.continuation, inventory(7, 5001)));
            vm.resume(&second.continuation, inventory(7, 5001))
        };
        assert!(
            matches!(&terminal, Step::Fault(f) if f.code == if bad_revision { "LSV2101" } else { "LSV2404" })
        );
        assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 2);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&first.continuation, inventory(7, 0)), terminal);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
}

#[test]
fn mutation_tails_keep_confirmation_and_correlated_acknowledgements() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: debugger.cancel(session_id: field(value: member(value: g, name: "a"), name: "node_id")))"#)).unwrap();
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
    let tail = effect(vm.resume(&first.continuation, focus("a")));
    let EffectOperation::Command(command) = &tail.operation else {
        panic!()
    };
    assert_eq!(
        command.confirmation,
        leselang_host_contract::Confirmation::Confirmed
    );
    assert_eq!(command.expected_revision, Some(Revision(7)));
    let result = EffectResult::DebuggerCancel(DebuggerCancelResult {
        command_id: command.command_id.clone(),
        session_id: "a".into(),
        observed_at_ms: 2,
    });
    assert!(
        matches!(vm.resume(&tail.continuation, result.clone()), Step::Fault(f) if f.code == "LSV2110")
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    let done = vm.acknowledge_effect(&lease, 2, result);
    assert!(matches!(done, Step::Done(Value::DebuggerCancel { .. })));
    assert_eq!(vm.resume(&first.continuation, focus("changed")), done);
}

#[test]
fn lazy_atomic_choices_can_use_recovered_scalars_loops_and_member_aliases() {
    let source = r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body:
        bind(member_alias: member(value: g, name: "a"), body:
            choose(when: eq(left: field(value: member_alias, name: "node_id"), right: "a"),
                then: ui.focus(node_id: concat(
                    left: to_string(value: recover(value: parse_integer(value: "bad"), fallback: 3)),
                    right: to_string(value: loop(n: 0, while: lt(left: n, right: 2),
                        next: add(left: n, right: 1), limit: 2)))),
                otherwise: ui.focus(node_id: concat(left: "bad", right: " node")))))"#;
    let path = JournalPath::new();
    for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
        let first = effect(start(&mut vm, source));
        let tail = effect(vm.resume(&first.continuation, focus("a")));
        assert_eq!(node(&tail), "32");
        assert!(tail.budget.fuel_remaining < first.budget.fuel_remaining);
        assert_eq!(
            vm.resume(&tail.continuation, focus("32")),
            Step::Done(Value::UiFocus {
                node_id: "32".into()
            })
        );
    }
    Vm::open_journal(&path.0, 1).unwrap();
}

#[test]
fn public_merge_cannot_skip_the_transactional_tail_and_legacy_group_wire_stays_stable() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, ONE));
    let saved = serde_json::from_value(plan(&Connection::open(&path.0).unwrap())).unwrap();
    assert_eq!(
        leselang_vm::merge_declared(&saved, vec![], 10000)
            .unwrap_err()
            .code,
        "LSV1409"
    );
    let tail = effect(vm.resume(&first.continuation, navigate("winner")));
    assert_eq!(tail.continuation.schema_version, 7);
    let mut legacy = Vm::default();
    let first = effect(start(
        &mut legacy,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: true)"#,
    ));
    assert_eq!(first.continuation.schema_version, 6);
    assert!(
        !serde_json::to_string(&first.continuation)
            .unwrap()
            .contains("successor_sequence")
    );
}

#[test]
fn exhausted_shared_fuel_is_durable_and_restart_does_not_refill_it() {
    let mut first_success = None;
    for fuel in 1..60 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
        let mut step = start(&mut vm, ONE);
        let mut root = None;
        if let Step::Effect(first) = step {
            root = Some(first.continuation.clone());
            drop(vm);
            vm = Vm::open_journal(&path.0, 1).unwrap();
            step = vm.resume(&first.continuation, navigate("winner"));
        }
        if let Step::Effect(tail) = step {
            step = vm.resume(&tail.continuation, focus("winner"));
        }
        if matches!(step, Step::Done(_)) {
            first_success = Some(fuel);
            break;
        }
        assert!(
            matches!(&step, Step::Fault(f) if f.code == "LSV1001"),
            "{step:?}"
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        if let Some(root) = root {
            assert_eq!(vm.resume(&root, navigate("changed")), step);
        }
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
    assert!(first_success.is_some());
}
