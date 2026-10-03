use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{UiFocusNavigationDirection, UiSelectionState, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    DebuggerCancelResult, EffectOperation, EffectRequest, EffectResult, PresentationResult,
    RetentionPolicy, ScalarValue, Step, Value, Vm, decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};

const FLOW: &str = r#"fn main() = bind(prefix: "target-", body:
    bind(g: seq(move: ui.navigate_focus(node_id: "a", direction: "next"),
                state: ui.set_selection(node_id: "toggle", state: "selected")), body:
        bind(alias: g, body: bind(state_alias: member(value: alias, name: "state"), body:
            bind(target: concat(left: prefix, right: field(value: member(value: g, name: "move"), name: "focused_node_id")), body:
                bind(focused: ui.focus(node_id: target), body:
                    choose(when: and(left: field(value: state_alias, name: "selected"),
                        right: field(value: member(value: alias, name: "state"), name: "selected")),
                        then: concat(left: field(value: member(value: g, name: "move"), name: "focused_node_id"),
                            right: field(value: focused, name: "node_id")), otherwise: "no")))))))"#;
const ONE: &str = r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")),
    body: bind(r: ui.focus(node_id: "b"), body: eq(left: field(value: member(value: g, name: "a"), name: "node_id"), right: field(value: r, name: "node_id"))))"#;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-group-capture-{}-{}",
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
fn focus(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
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
fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
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
fn prepare(vm: &mut Vm) -> (EffectRequest, EffectRequest, EffectRequest) {
    let first = effect(start(vm, FLOW));
    let second = effect(vm.resume(&first.continuation, navigate("b")));
    let tail = effect(vm.resume(&second.continuation, selected()));
    (first, second, tail)
}

#[test]
fn one_capture_preserves_typed_group_and_member_aliases_in_both_journals() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
        let (first, second, tail) = prepare(&mut vm);
        for request in [&first, &second, &tail] {
            assert_eq!(request.continuation.schema_version, 8);
            assert_eq!(
                request.continuation.group_result,
                first.continuation.group_result
            );
        }
        assert!(first.continuation.result_binding.is_none());
        let binding = tail.continuation.result_binding.as_ref().unwrap();
        assert_eq!(
            binding
                .groups
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            ["g", "alias"]
        );
        assert_eq!(binding.results[0].name, "state_alias");
        assert_eq!(binding.results[0].result.projection_version, 2);
        assert_eq!(
            binding.groups[0].group.members[1].result.projection_version,
            2
        );
        assert_eq!(
            decode_continuation(&encode_continuation(&tail.continuation).unwrap()).unwrap(),
            tail.continuation
        );
        assert!(tail.budget.fuel_remaining < second.budget.fuel_remaining);
        assert_eq!(tail.budget.max_output_items, first.budget.max_output_items);
        assert_eq!(
            tail.continuation.expected_revision,
            first.continuation.expected_revision
        );
        let expected = done(ScalarValue::String("btarget-b".into()));
        assert_eq!(vm.resume(&tail.continuation, focus("target-b")), expected);
        for request in [&first, &second, &tail] {
            assert_eq!(vm.resume(&request.continuation, focus("changed")), expected);
        }
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
}

#[test]
fn restart_keeps_raw_tail_receipts_and_replays_committed_scalars_without_recalculation() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, FLOW));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let second = effect(vm.resume(&first.continuation, navigate("b")));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let tail = effect(vm.resume(&second.continuation, selected()));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.pending_continuations(),
        std::slice::from_ref(&tail.continuation)
    );
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    let expected = vm.acknowledge_effect(&lease, 2, focus("target-b"));
    assert_eq!(expected, done(ScalarValue::String("btarget-b".into())));
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    let raw: Vec<u8> = connection
        .query_row(
            "SELECT terminal_step FROM vm_effects WHERE token = ?1",
            [tail.continuation.token.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Step>(&raw).unwrap(),
        Step::Done(Value::UiFocus {
            node_id: "target-b".into()
        })
    );
    assert_eq!(plan(&connection)["order"], "group_capture");
    assert_eq!(count(&connection, "vm_effects"), 3);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume(&first.continuation, navigate("changed")),
        expected
    );
    assert_eq!(vm.resume(&tail.continuation, focus("changed")), expected);
}

#[test]
fn prefix_admission_and_final_scalar_completion_roll_back_at_every_write_boundary() {
    for finishing in [false, true] {
        let writes: &[(&str, &str)] = if finishing {
            &[
                ("vm_effects", "UPDATE"),
                ("vm_dispatches", "UPDATE"),
                ("vm_merge_groups", "UPDATE"),
            ]
        } else {
            &[
                ("vm_effects", "UPDATE"),
                ("vm_dispatches", "UPDATE"),
                ("vm_effects", "INSERT"),
                ("vm_dispatches", "INSERT"),
                ("vm_merge_groups", "UPDATE"),
                ("vm_merge_branches", "INSERT"),
            ]
        };
        for (table, operation) in writes {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 500).unwrap();
            let first = effect(start(&mut vm, ONE));
            let current = if finishing {
                effect(vm.resume(&first.continuation, focus("a")))
            } else {
                first.clone()
            };
            let connection = Connection::open(&path.0).unwrap();
            let before = plan(&connection);
            let records = count(&connection, "vm_effects");
            connection.execute_batch(&format!("CREATE TRIGGER reject_transition BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
            assert!(matches!(
                vm.resume(
                    &current.continuation,
                    focus(if finishing { "b" } else { "a" })
                ),
                Step::Fault(_)
            ));
            assert_eq!(
                vm.pending_continuations(),
                std::slice::from_ref(&current.continuation)
            );
            assert_eq!(count(&connection, "vm_effects"), records);
            assert_eq!(plan(&connection), before);
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            connection
                .execute_batch("DROP TRIGGER reject_transition")
                .unwrap();
            let next = vm.resume(
                &current.continuation,
                focus(if finishing { "b" } else { "a" }),
            );
            if finishing {
                assert_eq!(next, done(ScalarValue::Boolean(false)));
            } else {
                assert!(matches!(next, Step::Effect(_)));
            }
            Vm::open_journal(&path.0, 1).unwrap();
        }
    }
}

#[test]
fn concurrent_acknowledgements_share_one_capture_and_one_scalar_completion() {
    for _ in 0..3 {
        let path = JournalPath::new();
        let mut left = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut left, ONE));
        let mut right = Vm::open_journal(&path.0, 1).unwrap();
        let barrier = std::sync::Barrier::new(2);
        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                barrier.wait();
                left.resume(&first.continuation, focus("a"))
            });
            let b = scope.spawn(|| {
                barrier.wait();
                right.resume(&first.continuation, focus("a"))
            });
            (a.join().unwrap(), b.join().unwrap())
        });
        assert_eq!(a, b);
        let tail = effect(a);
        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                barrier.wait();
                left.resume(&tail.continuation, focus("b"))
            });
            let b = scope.spawn(|| {
                barrier.wait();
                right.resume(&tail.continuation, focus("wrong"))
            });
            (a.join().unwrap(), b.join().unwrap())
        });
        assert_eq!(a, b);
        assert!(a == done(ScalarValue::Boolean(false)) || matches!(a, Step::Fault(_)));
        assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 2);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&first.continuation, focus("changed")), a);
    }
}

#[test]
fn captured_group_images_cannot_restore_without_their_complete_journal() {
    let mut vm = Vm::new(500);
    let (first, second, tail) = prepare(&mut vm);
    for request in [first, second, tail] {
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
fn frame_corruption_schema_downgrades_and_unbounded_bodies_fail_closed() {
    for mutation in 0..12 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let (_, _, tail) = prepare(&mut vm);
        drop(vm);
        let mut request = serde_json::to_value(&tail).unwrap();
        match mutation {
            0 => request["continuation"]["schema_version"] = 7.into(),
            1 => request["continuation"]["group_result"] = serde_json::Value::Null,
            2 => request["continuation"]["result_binding"]["groups"] = serde_json::json!([]),
            3 => {
                request["continuation"]["result_binding"]["groups"][0]["group"]["members"][0]["name"] =
                    "wrong".into()
            }
            4 => {
                request["continuation"]["result_binding"]["groups"][0]["group"]["members"][0]["result"]
                    ["fields"][1]["value"]["value"] = "changed".into()
            }
            5 => {
                request["continuation"]["result_binding"]["groups"][0]["group"]["members"][0]["result"]
                    ["unknown"] = true.into()
            }
            6 => {
                request["continuation"]["result_binding"]["groups"][0]["group"]["members"][0]["result"]
                    ["projection_version"] = 99.into()
            }
            7 => {
                let group = &mut request["continuation"]["result_binding"]["groups"][1];
                let member = &mut group["group"]["members"][1]["result"];
                member["projection_version"] = 1.into();
                member["fields"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|field| field["field"] != "selected");
            }
            8 => {
                request["continuation"]["result_binding"]["body"] = serde_json::json!({"kind":"host", "effect":{"kind":"runtime_list", "filter":{"environment":null,"role":null}}})
            }
            9 => {
                request["continuation"]["fuel_remaining"] = 1000.into();
                request["budget"]["fuel_remaining"] = 1000.into();
            }
            10 => {
                request["continuation"]["result_binding"]["results"][0]["result"]["fields"][1]["value"]
                    ["value"] = false.into()
            }
            _ => {
                request["continuation"]["result_binding"]["groups"][1]["group"]["members"][0]["result"]
                    ["fields"][0]["value"]["value"] = "changed".into()
            }
        }
        let connection = Connection::open(&path.0).unwrap();
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
        assert!(Vm::open_journal(&path.0, 1).is_err(), "mutation {mutation}");
        if matches!(mutation, 0 | 1 | 5 | 6 | 7 | 8) {
            assert!(
                decode_continuation(&serde_json::to_vec(&request["continuation"]).unwrap())
                    .is_err(),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn pure_final_faults_are_committed_with_raw_tail_results_and_replayed_after_restart() {
    for (body, code) in [
        ("div(left: 1, right: 0)", "LSV1401"),
        (r#"parse_integer(value: "bad")"#, "LSV1408"),
        ("loop(n: 0, while: true, next: n, limit: 1)", "LSV1406"),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let source = format!(
            r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: {body}))"#
        );
        let first = effect(start(&mut vm, &source));
        let tail = effect(vm.resume(&first.continuation, focus("a")));
        let terminal = vm.resume(&tail.continuation, focus("b"));
        assert!(matches!(&terminal, Step::Fault(fault) if fault.code == code));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&first.continuation, focus("changed")), terminal);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
}

#[test]
fn a_conditional_capture_can_select_different_atomic_types_without_running_cold_work() {
    for choose_ui in [false, true] {
        let mut vm = Vm::new(500);
        let first = effect(start(
            &mut vm,
            &format!(
                r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: choose(when: {choose_ui}, then: bind(r: ui.focus(node_id: "b"), body: 1), otherwise: bind(r: runtime.list(), body: field(value: r, name: "count"))))"#
            ),
        ));
        let tail = effect(vm.resume(&first.continuation, focus("a")));
        let result = if choose_ui {
            focus("b")
        } else {
            EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
                revision: Revision(7),
                runtimes: vec![],
            })
        };
        assert_eq!(
            vm.resume(&tail.continuation, result),
            done(ScalarValue::Integer(u64::from(choose_ui)))
        );
    }
}

#[test]
fn leases_cancellation_and_deadlines_fence_both_capture_and_final_calculation() {
    for mode in ["lease", "cancel", "deadline", "wrong-result"] {
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
        let tail = effect(vm.resume_at(&first.continuation, 101, focus("a")));
        let terminal = match mode {
            "cancel" => vm.cancel_effect(&tail.continuation, 102),
            "deadline" => vm.resume_at(&tail.continuation, 200, focus("b")),
            "wrong-result" => vm.resume_at(&tail.continuation, 102, focus("wrong")),
            _ => {
                let stale = vm.claim_effect(102, 10).unwrap().unwrap();
                assert!(
                    matches!(vm.resume_at(&tail.continuation, 103, focus("b")), Step::Fault(fault) if fault.code == "LSV4024")
                );
                assert!(
                    matches!(vm.acknowledge_effect(&stale, 113, focus("b")), Step::Fault(fault) if fault.code == "LSV4023")
                );
                let active = vm.claim_effect(113, 50).unwrap().unwrap();
                vm.acknowledge_effect(&active, 114, focus("b"))
            }
        };
        assert!(!matches!(
            terminal,
            Step::Effect(_) | Step::Effects(_) | Step::Waiting(_)
        ));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 201, focus("changed")),
            terminal
        );
        assert!(vm.claim_effect(202, 50).unwrap().is_none());
    }
}

#[test]
fn oversized_group_projection_is_a_durable_fault_without_admitting_a_tail() {
    let mut body = r#"bind(r: ui.focus(node_id: "b"), body: 7)"#.to_string();
    for index in 0..10 {
        body = format!("bind(alias{index}: g, body: {body})");
    }
    let source = format!(
        r#"fn main() = bind(g: repeat(times: 63, body: ui.focus(node_id: "a")), body: {body})"#
    );
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100000).unwrap();
    let first = effect(start(&mut vm, &source));
    let mut step = Step::Effect(Box::new(first.clone()));
    for _ in 0..63 {
        step = vm.resume(&effect(step).continuation, focus("a"));
    }
    assert!(
        matches!(&step, Step::Fault(fault) if fault.code == "LSV3002"),
        "{step:?}"
    );
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 63);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&first.continuation, focus("changed")), step);
    assert!(vm.claim_effect(1, 100).unwrap().is_none());
}

#[test]
fn raw_tail_output_is_counted_before_scalar_projection_can_hide_an_overflow() {
    let mut control = leserpent_domain::InMemoryControlPlane::default();
    let runtime = control.register_runtime(
        leselang_host_contract::RuntimeId::new("runtime-a").unwrap(),
        "runtime",
        "http://runtime.local",
    );
    let inventory = |size| {
        EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
            revision: Revision(7),
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
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(
        &mut vm,
        r#"fn main() = bind(g: seq(a: runtime.list()), body: bind(r: runtime.list(), body: add(left: field(value: member(value: g, name: "a"), name: "count"), right: field(value: r, name: "count"))))"#,
    ));
    let tail = effect(vm.resume(&first.continuation, inventory(5000)));
    let terminal = vm.resume(&tail.continuation, inventory(5001));
    assert!(matches!(&terminal, Step::Fault(fault) if fault.code == "LSV2404"));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&first.continuation, inventory(0)), terminal);
}

#[test]
fn inherited_command_authority_still_requires_confirmed_correlated_acknowledgement() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: debugger.cancel(session_id: field(value: member(value: g, name: "a"), name: "node_id")), body: true))"#)).unwrap();
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
        matches!(vm.resume(&tail.continuation, result.clone()), Step::Fault(fault) if fault.code == "LSV2110")
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    assert_eq!(lease.request, tail);
    let terminal = vm.acknowledge_effect(&lease, 2, result);
    assert_eq!(terminal, done(ScalarValue::Boolean(true)));
    assert_eq!(vm.resume(&first.continuation, focus("ignored")), terminal);
    drop(vm);
    Vm::open_journal(&path.0, 1).unwrap();
}

#[test]
fn shared_fuel_counts_group_capture_restoration_and_final_calculation_without_refills() {
    let mut first_success = None;
    for fuel in 1..100 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
        let mut step = start(&mut vm, ONE);
        let mut root = None;
        if let Step::Effect(first) = step {
            root = Some(first.continuation.clone());
            step = vm.resume(&first.continuation, focus("a"));
        }
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        if let Step::Effect(tail) = step {
            step = vm.resume(&tail.continuation, focus("b"));
        }
        if matches!(step, Step::Done(_)) {
            first_success = Some(fuel);
            assert_eq!(step, done(ScalarValue::Boolean(false)));
            break;
        }
        assert!(
            matches!(&step, Step::Fault(fault) if fault.code == "LSV1001"),
            "{step:?}"
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        if let Some(root) = root {
            assert_eq!(vm.resume(&root, focus("changed")), step);
        }
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
    assert!(first_success.is_some());
}

#[test]
fn public_merging_cannot_evaluate_a_captured_group_outside_its_owned_journal() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, ONE));
    for appended in [false, true] {
        if appended {
            vm.resume(&first.continuation, focus("a"));
        }
        let saved = serde_json::from_value(plan(&Connection::open(&path.0).unwrap())).unwrap();
        assert_eq!(
            leselang_vm::merge_declared(&saved, vec![], 10000)
                .unwrap_err()
                .code,
            "LSV1409"
        );
    }
    let mut legacy = Vm::new(500);
    let request = effect(start(
        &mut legacy,
        r#"fn main() = bind(r: ui.focus(node_id: "a"), body: field(value: r, name: "node_id"))"#,
    ));
    assert_eq!(request.continuation.schema_version, 2);
    assert!(
        !serde_json::to_string(&request.continuation)
            .unwrap()
            .contains("\"groups\"")
    );
}

#[test]
fn compaction_protects_live_captures_and_removes_the_completed_whole_unit() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let (first, _, tail) = prepare(&mut vm);
    let policy = RetentionPolicy {
        max_completed_records: 1,
        max_delete_per_run: 10,
    };
    assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 0);
    vm.resume(&tail.continuation, focus("target-b"));
    let later = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    vm.resume(&later.continuation, focus("a"));
    assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 1);
    assert_eq!(vm.completed_count(), 1);
    assert_eq!(
        count(&Connection::open(&path.0).unwrap(), "vm_merge_groups"),
        0
    );
    assert!(matches!(
        vm.resume(&first.continuation, focus("ignored")),
        Step::Fault(_)
    ));
    Vm::open_journal(&path.0, 1).unwrap();
}
