use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{UiFocusNavigationDirection, lower};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectRequest, EffectResult, PresentationResult, RetentionPolicy, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-group-binding-{}-{}",
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
fn start(vm: &mut Vm, expression: &str) -> Step {
    let program = lower(&parse(&format!("fn main() = {expression}"))).unwrap();
    assert_eq!(
        serde_json::from_slice::<leselang_hir::HirProgram>(&serde_json::to_vec(&program).unwrap())
            .unwrap(),
        program
    );
    vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "runtime.read"]),
        None,
    )
}
fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("expected effect: {other:?}"),
    }
}
fn focus(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
}
fn navigate(target: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::NavigateFocus {
        node_id: "a".into(),
        direction: UiFocusNavigationDirection::Next,
        focused_node_id: target.into(),
    })
}
fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}
fn plan(connection: &Connection) -> serde_json::Value {
    let bytes: Vec<u8> = connection
        .query_row("SELECT plan FROM vm_merge_groups", [], |row| row.get(0))
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn write_plan(connection: &Connection, plan: &serde_json::Value) {
    connection
        .execute(
            "UPDATE vm_merge_groups SET plan = ?1",
            [serde_json::to_vec(plan).unwrap()],
        )
        .unwrap();
}
const FLOW: &str = r#"bind(prefix: "target-", body:
    bind(g: seq(first: ui.focus(node_id: concat(left: prefix, right: "a")), second: ui.navigate_focus(node_id: "a", direction: "next")), body:
        bind(alias: g, body: concat(left: field(value: member(value: alias, name: "first"), name: "node_id"),
            right: field(value: member(value: g, name: "second"), name: "focused_node_id")))))"#;

#[test]
fn named_sequence_members_and_aliases_complete_as_one_scalar_in_both_journals() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        let blocked = vm
            .pending_continuations()
            .into_iter()
            .find(|image| image.token != first.continuation.token)
            .unwrap();
        assert!(matches!(
            vm.resume(&blocked, navigate("wrong-order")),
            Step::Fault(_)
        ));
        assert_eq!(vm.pending_count(), 2);
        let second = effect(vm.resume(&first.continuation, focus("target-a")));
        assert_eq!(second.continuation, blocked);
        assert_eq!(first.continuation.schema_version, 6);
        assert_eq!(second.continuation.schema_version, 6);
        assert_eq!(
            first.continuation.group_result,
            second.continuation.group_result
        );
        assert!(first.continuation.result_binding.is_none());
        assert_eq!(
            first.budget.fuel_remaining - 1,
            second.budget.fuel_remaining
        );
        let result = done(ScalarValue::String("target-aZ".into()));
        assert_eq!(vm.resume(&second.continuation, navigate("Z")), result);
        for request in [&first, &second] {
            assert_eq!(
                vm.resume(&request.continuation, navigate("ignored")),
                result
            );
        }
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
}

#[test]
fn parallel_completion_order_does_not_change_member_names_or_final_value() {
    for reverse in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100).unwrap();
        let Step::Effects(batch) = start(
            &mut vm,
            r#"bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: concat(left: field(value: member(value: g, name: "a"), name: "node_id"), right: field(value: member(value: g, name: "b"), name: "node_id")))"#,
        ) else {
            panic!()
        };
        let (first, last) = if reverse {
            (&batch.branches[1], &batch.branches[0])
        } else {
            (&batch.branches[0], &batch.branches[1])
        };
        assert_eq!(
            first.request.budget.fuel_remaining,
            last.request.budget.fuel_remaining
        );
        assert!(matches!(
            vm.resume(&first.request.continuation, focus(&first.branch)),
            Step::Waiting(_)
        ));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        let result = done(ScalarValue::String("ab".into()));
        assert_eq!(
            vm.resume(&last.request.continuation, focus(&last.branch)),
            result
        );
        assert_eq!(vm.merge_result(&batch.merge_token).unwrap(), Some(result));
        assert_eq!(
            plan(&Connection::open(&path.0).unwrap())["order"],
            "bound_parallel"
        );
    }
}

#[test]
fn repeat_names_and_single_member_sequences_project_without_raw_group_escape() {
    for (group, name, count) in [
        (
            r#"repeat(times: 2, body: ui.focus(node_id: "a"))"#,
            "iteration_2",
            2,
        ),
        (r#"seq(only: ui.focus(node_id: "a"))"#, "only", 1),
        (
            r#"seq(nested: seq(a: ui.focus(node_id: "a")))"#,
            "nested__a",
            1,
        ),
    ] {
        let source = format!(
            r#"bind(g: {group}, body: eq(left: field(value: member(value: g, name: "{name}"), name: "node_id"), right: "a"))"#
        );
        let mut vm = Vm::default();
        let mut step = start(&mut vm, &source);
        for _ in 0..count {
            step = vm.resume(&effect(step).continuation, focus("a"));
        }
        assert_eq!(step, done(ScalarValue::Boolean(true)));
    }
}

#[test]
fn group_restart_replays_committed_members_and_committed_scalar_without_reexecution() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, FLOW));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let second = effect(vm.resume(&first.continuation, focus("target-a")));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        effect(vm.resume(&first.continuation, navigate("ignored"))),
        second
    );
    let result = vm.resume(&second.continuation, navigate("b"));
    assert_eq!(result, done(ScalarValue::String("target-ab".into())));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&first.continuation, navigate("changed")), result);
    assert_eq!(vm.resume(&second.continuation, navigate("changed")), result);
}

#[test]
fn group_images_are_bounded_versioned_and_never_restored_without_their_whole_journal() {
    let mut vm = Vm::default();
    let first = effect(start(&mut vm, FLOW));
    let bytes = encode_continuation(&first.continuation).unwrap();
    assert_eq!(decode_continuation(&bytes).unwrap(), first.continuation);
    assert_eq!(
        Vm::default()
            .restore(first.continuation.clone())
            .unwrap_err()
            .code,
        "LSV1409"
    );
    assert_eq!(
        Vm::default()
            .restore_request(first.clone())
            .unwrap_err()
            .code,
        "LSV1409"
    );
    for owner in [
        "merge-0",
        "merge-01",
        "continuation-1",
        "merge-18446744073709551615",
    ] {
        let mut wire = serde_json::to_value(&first.continuation).unwrap();
        wire["group_result"] = owner.into();
        assert!(decode_continuation(&serde_json::to_vec(&wire).unwrap()).is_err());
    }
    for (key, value) in [
        ("schema_version", serde_json::json!(1)),
        ("group_result", serde_json::Value::Null),
    ] {
        let mut wire = serde_json::to_value(&first.continuation).unwrap();
        wire[key] = value;
        assert!(decode_continuation(&serde_json::to_vec(&wire).unwrap()).is_err());
    }
}

#[test]
fn eager_argument_preflight_and_permissions_fail_before_any_group_dispatch() {
    let mut vm = Vm::default();
    let bad = r#"bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: to_string(value: div(left: 1, right: 0)))), body: true)"#;
    assert!(matches!(start(&mut vm, bad), Step::Fault(fault) if fault.code == "LSV1401"));
    let program = lower(&parse(
        r#"fn main() = bind(g: all(a: runtime.list(), b: ui.focus(node_id: "a")), body: true)"#,
    ))
    .unwrap();
    assert!(matches!(
        vm.start(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["runtime.read"]),
            None
        ),
        Step::Fault(_)
    ));
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(
        effect(start(&mut vm, r#"ui.focus(node_id: "a")"#)).effect_id,
        "effect-1"
    );
}

#[test]
fn body_faults_commit_atomically_and_recovery_does_not_replenish_shared_fuel() {
    let source = r#"bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: recover(value: div(left: 1, right: 0), fallback: 7))"#;
    let mut first_success = None;
    for fuel in 1..25 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
        let mut step = start(&mut vm, source);
        for node in ["a", "b"] {
            if let Step::Effect(request) = step {
                step = vm.resume(&request.continuation, focus(node));
            }
        }
        match step {
            Step::Done(_) => {
                assert_eq!(step, done(ScalarValue::Integer(7)));
                first_success = Some(fuel);
                break;
            }
            Step::Fault(ref fault) => assert_eq!(fault.code, "LSV1001"),
            other => panic!("unexpected outcome {other:?}"),
        }
        Vm::open_journal(&path.0, 1).unwrap();
    }
    assert_eq!(first_success, Some(9));
    let oversized = format!(
        r#"bind(s: "{}", body: concat(left: s, right: s))"#,
        "x".repeat(2049)
    );
    for (body, code) in [
        ("div(left: 1, right: 0)", "LSV1401"),
        (r#"parse_integer(value: "bad")"#, "LSV1408"),
        ("loop(n: 0, while: true, next: n, limit: 1)", "LSV1406"),
        (oversized.as_str(), "LSV1403"),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let request = effect(start(
            &mut vm,
            &format!(r#"bind(g: seq(a: ui.focus(node_id: "a")), body: {body})"#),
        ));
        let terminal = vm.resume(&request.continuation, focus("a"));
        assert!(matches!(&terminal, Step::Fault(fault) if fault.code == code));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&request.continuation, focus("changed")), terminal);
    }
}

#[test]
fn final_member_and_scalar_or_fault_roll_back_together_on_journal_write_failure() {
    for body in ["7", "div(left: 1, right: 0)"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100).unwrap();
        let request = effect(start(
            &mut vm,
            &format!(r#"bind(g: seq(a: ui.focus(node_id: "a")), body: {body})"#),
        ));
        let connection = Connection::open(&path.0).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_group BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
        assert!(matches!(
            vm.resume(&request.continuation, focus("a")),
            Step::Fault(_)
        ));
        let state: String = connection
            .query_row("SELECT state FROM vm_effects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(state, "pending");
        assert_eq!(vm.pending_count(), 1);
        connection
            .execute_batch("DROP TRIGGER reject_group")
            .unwrap();
        let terminal = vm.resume(&request.continuation, focus("a"));
        if body == "7" {
            assert_eq!(terminal, done(ScalarValue::Integer(7)));
        } else {
            assert!(matches!(terminal, Step::Fault(ref fault) if fault.code == "LSV1401"));
        }
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&request.continuation, focus("ignored")), terminal);
    }
}

#[test]
fn competing_final_acknowledgements_share_exactly_one_committed_result() {
    for _ in 0..4 {
        let path = JournalPath::new();
        let mut left = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut left, FLOW));
        let second = effect(left.resume(&first.continuation, focus("target-a")));
        let mut right = Vm::open_journal(&path.0, 1).unwrap();
        let barrier = std::sync::Barrier::new(2);
        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                barrier.wait();
                left.resume(&second.continuation, navigate("left"))
            });
            let b = scope.spawn(|| {
                barrier.wait();
                right.resume(&second.continuation, navigate("right"))
            });
            (a.join().unwrap(), b.join().unwrap())
        });
        assert_eq!(a, b);
        assert!(
            a == done(ScalarValue::String("target-aleft".into()))
                || a == done(ScalarValue::String("target-aright".into()))
        );
        let mut reopened = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(reopened.resume(&first.continuation, navigate("ignored")), a);
    }
}

#[test]
fn damaged_group_plan_owner_request_and_terminal_fail_closed_on_reopen() {
    for mutation in 0..10 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut vm, FLOW));
        if mutation >= 8 {
            let second = effect(vm.resume(&first.continuation, focus("target-a")));
            vm.resume(&second.continuation, navigate("b"));
        }
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        let mut saved = plan(&connection);
        match mutation {
            0 => {
                saved["result_binding"] = serde_json::Value::Null;
                write_plan(&connection, &saved);
            }
            1 => {
                saved["order"] = "sequential".into();
                write_plan(&connection, &saved);
            }
            2 => {
                saved["result_binding"]["fuel_remaining"] = 1000.into();
                write_plan(&connection, &saved);
            }
            3 => {
                saved["result_binding"]["pending"]["steps"][0]["name"] = "wrong".into();
                write_plan(&connection, &saved);
            }
            4 => {
                saved["result_binding"]["unknown"] = true.into();
                write_plan(&connection, &saved);
            }
            5 => {
                connection
                    .execute(
                        "DELETE FROM vm_merge_branches WHERE branch_token = ?1",
                        [first.continuation.token.as_str()],
                    )
                    .unwrap();
            }
            6 => {
                connection
                    .execute(
                        "DELETE FROM vm_dispatches WHERE token = ?1",
                        [first.continuation.token.as_str()],
                    )
                    .unwrap();
            }
            7 => {
                let mut image = serde_json::to_value(&first.continuation).unwrap();
                image["group_result"] = "merge-999".into();
                connection
                    .execute(
                        "UPDATE vm_effects SET image = ?1 WHERE token = ?2",
                        params![
                            serde_json::to_vec(&image).unwrap(),
                            first.continuation.token.as_str()
                        ],
                    )
                    .unwrap();
            }
            8 => {
                connection
                    .execute(
                        "UPDATE vm_merge_groups SET terminal_step = ?1",
                        [serde_json::to_vec(&done(ScalarValue::Boolean(true))).unwrap()],
                    )
                    .unwrap();
            }
            _ => {
                connection
                    .execute(
                        "UPDATE vm_merge_groups SET terminal_step = ?1",
                        [
                            serde_json::to_vec(&Step::Done(Value::Structured { fields: vec![] }))
                                .unwrap(),
                        ],
                    )
                    .unwrap();
            }
        }
        assert!(Vm::open_journal(&path.0, 1).is_err(), "mutation {mutation}");
    }
}

#[test]
fn compaction_removes_the_whole_calculated_group_and_preserves_legacy_work() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, FLOW));
    let second = effect(vm.resume(&first.continuation, focus("target-a")));
    vm.resume(&second.continuation, navigate("b"));
    let legacy = effect(start(&mut vm, r#"ui.focus(node_id: "a")"#));
    assert_eq!(legacy.continuation.schema_version, 1);
    assert!(
        !serde_json::to_string(&legacy.continuation)
            .unwrap()
            .contains("group_result")
    );
    vm.resume(&legacy.continuation, focus("a"));
    let report = vm
        .compact_journal(&RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 10,
        })
        .unwrap();
    assert_eq!(report.removed_records, 1);
    let connection = Connection::open(&path.0).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM vm_merge_groups", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    Vm::open_journal(&path.0, 1).unwrap();
}

#[test]
fn single_step_group_retention_uses_the_declared_plan_instead_of_a_two_step_minimum() {
    for source in [
        r#"seq(a: ui.focus(node_id: "a"))"#,
        r#"bind(g: seq(a: ui.focus(node_id: "a")), body: true)"#,
    ] {
        let path = JournalPath::new();
        for mut vm in [Vm::default(), Vm::open_journal(&path.0, 100).unwrap()] {
            let first = effect(start(&mut vm, source));
            vm.resume(&first.continuation, focus("a"));
            let later = effect(start(&mut vm, r#"ui.focus(node_id: "a")"#));
            vm.resume(&later.continuation, focus("a"));
            assert_eq!(
                vm.compact_journal(&RetentionPolicy {
                    max_completed_records: 1,
                    max_delete_per_run: 1
                })
                .unwrap()
                .removed_records,
                1
            );
            assert_eq!(vm.completed_count(), 1);
        }
        Vm::open_journal(&path.0, 1).unwrap();
    }
}

#[test]
fn both_group_orders_reserve_all_member_fuel_before_pure_body_execution() {
    for order in ["seq", "all"] {
        let source = format!(
            r#"bind(g: {order}(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: true)"#
        );
        for fuel in 3..=5 {
            let mut vm = Vm::new(fuel);
            let step = start(&mut vm, &source);
            if fuel == 3 {
                assert!(matches!(step, Step::Fault(ref fault) if fault.code == "LSV1001"));
                assert_eq!(vm.pending_count(), 0);
                assert_eq!(
                    effect(start(&mut vm, r#"ui.focus(node_id: "a")"#)).effect_id,
                    "effect-1"
                );
                continue;
            }
            let terminal = match step {
                Step::Effect(first) => {
                    let last = effect(vm.resume(&first.continuation, focus("a")));
                    vm.resume(&last.continuation, focus("b"))
                }
                Step::Effects(batch) => {
                    assert!(matches!(
                        vm.resume(&batch.branches[0].request.continuation, focus("a")),
                        Step::Waiting(_)
                    ));
                    vm.resume(&batch.branches[1].request.continuation, focus("b"))
                }
                other => panic!("unexpected group start: {other:?}"),
            };
            if fuel == 4 {
                assert!(matches!(terminal, Step::Fault(fault) if fault.code == "LSV1001"));
            } else {
                assert_eq!(terminal, done(ScalarValue::Boolean(true)));
            }
        }
    }
}

#[test]
fn cancellation_deadlines_and_host_failures_never_enter_the_group_fallback() {
    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: recover(value: div(left: 1, right: 0), fallback: 7))"#)).unwrap();
    for mode in ["cancel", "deadline", "wrong-result", "host-error"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100).unwrap();
        let first = effect(vm.start_timed(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
            100,
            100,
        ));
        let terminal = match mode {
            "cancel" => vm.cancel_effect(&first.continuation, 102),
            "deadline" => vm.resume_at(&first.continuation, 200, focus("a")),
            "wrong-result" => vm.resume_at(&first.continuation, 102, focus("wrong")),
            _ => {
                let lease = vm.claim_effect(101, 50).unwrap().unwrap();
                let disposition = vm
                    .report_effect_error(
                        &lease,
                        102,
                        leselang_vm::EffectError {
                            class: leselang_vm::EffectErrorClass::Permanent,
                            code: "LSV1408".into(),
                            message: "host rejection".into(),
                        },
                        &leselang_vm::RetryPolicy::default(),
                    )
                    .unwrap();
                let leselang_vm::RetryDisposition::Terminal(step) = disposition else {
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
        assert_eq!(vm.resume_at(&first.continuation, 201, focus("a")), terminal);
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(202, 50).unwrap().is_none());
    }
}

#[test]
fn public_merging_checks_raw_member_type_and_total_budget_before_projection() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    start(&mut vm, FLOW);
    let saved: leselang_vm::MergePlan =
        serde_json::from_value(plan(&Connection::open(&path.0).unwrap())).unwrap();
    let completions = |first| {
        vec![
            leselang_vm::BranchCompletion {
                branch: "first".into(),
                outcome: leselang_vm::BranchOutcome::Value(first),
            },
            leselang_vm::BranchCompletion {
                branch: "second".into(),
                outcome: leselang_vm::BranchOutcome::Value(Value::UiNavigateFocus {
                    node_id: "a".into(),
                    direction: UiFocusNavigationDirection::Next,
                    focused_node_id: "b".into(),
                }),
            },
        ]
    };
    assert_eq!(
        leselang_vm::merge_declared(
            &saved,
            completions(Value::UiActivate {
                node_id: "target-a".into()
            }),
            2
        )
        .unwrap_err()
        .code,
        "LSV1409"
    );
    assert_eq!(
        leselang_vm::merge_declared(
            &saved,
            completions(Value::UiFocus {
                node_id: "target-a".into()
            }),
            1
        )
        .unwrap_err()
        .code,
        "LSV2404"
    );
    assert_eq!(
        leselang_vm::merge_declared(
            &saved,
            completions(Value::UiFocus {
                node_id: "target-a".into()
            }),
            2
        )
        .unwrap(),
        done(ScalarValue::String("target-ab".into()))
    );
}

#[test]
fn oversized_group_recovery_metadata_is_rejected_before_allocating_effects() {
    let source = format!(
        r#"bind(g: repeat(times: 64, body: ui.assert_text(node_id: "a", expected: "{}")), body: true)"#,
        "x".repeat(1024)
    );
    let mut vm = Vm::new(10000);
    assert!(matches!(start(&mut vm, &source), Step::Fault(fault) if fault.code == "LSV3002"));
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(
        effect(start(&mut vm, r#"ui.focus(node_id: "a")"#)).effect_id,
        "effect-1"
    );
}

#[test]
fn cumulative_output_failure_is_durable_even_when_the_last_member_exhausts_the_limit() {
    let mut control = leserpent_domain::InMemoryControlPlane::default();
    let runtime = control.register_runtime(
        leselang_host_contract::RuntimeId::new("runtime-a").unwrap(),
        "runtime",
        "http://runtime.local",
    );
    let inventory = || {
        EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
            revision: leselang_host_contract::Revision(1),
            runtimes: (0..5001)
                .map(|index| {
                    let mut runtime = runtime.clone();
                    runtime.id =
                        leselang_host_contract::RuntimeId::new(format!("runtime-{index}")).unwrap();
                    runtime
                })
                .collect(),
        })
    };
    for tail in ["", ", third: runtime.list()"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100).unwrap();
        let first = effect(start(
            &mut vm,
            &format!("bind(g: seq(first: runtime.list(), second: runtime.list(){tail}), body: 7)"),
        ));
        let second = effect(vm.resume(&first.continuation, inventory()));
        let terminal = vm.resume(&second.continuation, inventory());
        assert!(matches!(&terminal, Step::Fault(fault) if fault.code == "LSV2404"));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&first.continuation, inventory()), terminal);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
}
