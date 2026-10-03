use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{UiSelectionState, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectRequest, EffectResult, PresentationResult, RetentionPolicy, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};

fn flow(parallel: bool) -> String {
    let group = if parallel { "all" } else { "seq" };
    format!(
        r#"fn main() = bind(g: {group}(a: ui.focus(node_id: "a"), flag: ui.assert_selection(node_id: "toggle", state: "selected")), body:
        bind(first: ui.focus(node_id: field(value: member(value: g, name: "a"), name: "node_id")), body:
            bind(alias: g, body: bind(previous: first, body:
                bind(second: ui.focus(node_id: concat(left: "next-", right: field(value: previous, name: "node_id"))), body:
                    and(left: field(value: member(value: alias, name: "flag"), name: "selected"),
                        right: eq(left: field(value: second, name: "node_id"), right: concat(left: "next-", right: field(value: member(value: g, name: "a"), name: "node_id")))))))))"#
    )
}

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-group-dataflow-{}-{}",
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
fn selected() -> EffectResult {
    EffectResult::Presentation(PresentationResult::AssertSelection {
        node_id: "toggle".into(),
        state: UiSelectionState::Selected,
    })
}
fn done(value: bool) -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(value),
    })
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
fn prepare(vm: &mut Vm, parallel: bool) -> (Vec<EffectRequest>, EffectRequest) {
    let step = start(vm, &flow(parallel));
    let prefix = if parallel {
        let Step::Effects(batch) = step else {
            panic!("{step:?}")
        };
        let prefix = batch
            .branches
            .into_iter()
            .map(|branch| branch.request)
            .collect::<Vec<_>>();
        assert!(matches!(
            vm.resume(&prefix[1].continuation, selected()),
            Step::Waiting(_)
        ));
        prefix
    } else {
        let first = effect(step);
        let second = effect(vm.resume(&first.continuation, focus("a")));
        let tail = effect(vm.resume(&second.continuation, selected()));
        return (vec![first, second], tail);
    };
    let tail = effect(vm.resume(&prefix[0].continuation, focus("a")));
    (prefix, tail)
}

#[test]
fn sequential_and_parallel_groups_drive_multiple_typed_captures_in_both_journals() {
    for parallel in [false, true] {
        let path = JournalPath::new();
        for mut vm in [Vm::new(1000), Vm::open_journal(&path.0, 1000).unwrap()] {
            let (prefix, first) = prepare(&mut vm, parallel);
            let second = effect(vm.resume(&first.continuation, focus("a")));
            let first_binding = first.continuation.result_binding.as_ref().unwrap();
            assert!(!first_binding.body.is_pure());
            let binding = second.continuation.result_binding.as_ref().unwrap();
            assert!(binding.body.is_pure());
            assert_eq!(
                binding
                    .groups
                    .iter()
                    .map(|group| group.name.as_str())
                    .collect::<Vec<_>>(),
                ["g", "alias"]
            );
            assert_eq!(
                binding
                    .results
                    .iter()
                    .map(|saved| saved.name.as_str())
                    .collect::<Vec<_>>(),
                ["first", "previous"]
            );
            assert_eq!(
                binding.groups[0].group.members[1].result.projection_version,
                2
            );
            for request in prefix.iter().chain([&first, &second]) {
                assert_eq!(request.continuation.schema_version, 10);
                assert_eq!(
                    request.continuation.group_result,
                    prefix[0].continuation.group_result
                );
                assert_eq!(request.continuation.expected_revision, Some(Revision(7)));
                assert_eq!(
                    request.budget.max_output_items,
                    prefix[0].budget.max_output_items
                );
                assert_eq!(
                    decode_continuation(&encode_continuation(&request.continuation).unwrap())
                        .unwrap(),
                    request.continuation
                );
            }
            assert!(second.budget.fuel_remaining < first.budget.fuel_remaining);
            assert_eq!(
                vm.resume(&first.continuation, focus("changed")),
                Step::Effect(Box::new(second.clone()))
            );
            assert_eq!(vm.resume(&second.continuation, focus("next-a")), done(true));
            for request in prefix.iter().chain([&first, &second]) {
                assert_eq!(
                    vm.resume(&request.continuation, focus("changed")),
                    done(true)
                );
            }
            assert_eq!(vm.pending_count(), 0);
        }
    }
}

#[test]
fn restart_after_every_capture_preserves_raw_receipts_and_shared_authority() {
    for parallel in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let (prefix, first) = prepare(&mut vm, parallel);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        let lease = vm.claim_effect(1, 100).unwrap().unwrap();
        assert_eq!(lease.request, first);
        let second = effect(vm.acknowledge_effect(&lease, 2, focus("a")));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.pending_continuations(),
            std::slice::from_ref(&second.continuation)
        );
        assert_eq!(vm.resume(&second.continuation, focus("next-a")), done(true));
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        assert_eq!(count(&connection, "vm_effects"), 4);
        let saved = plan(&connection);
        assert_eq!(
            saved["order"],
            if parallel {
                "parallel_group_dataflow"
            } else {
                "group_dataflow"
            }
        );
        assert_eq!(
            saved["result_binding"]["additional_successor_sequences"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        for (request, node) in [(&first, "a"), (&second, "next-a")] {
            let bytes: Vec<u8> = connection
                .query_row(
                    "SELECT terminal_step FROM vm_effects WHERE token = ?1",
                    [request.continuation.token.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                serde_json::from_slice::<Step>(&bytes).unwrap(),
                Step::Done(Value::UiFocus {
                    node_id: node.into()
                })
            );
        }
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        for request in prefix.iter().chain([&first, &second]) {
            assert_eq!(
                vm.resume(&request.continuation, focus("changed")),
                done(true)
            );
        }
    }
}

#[test]
fn every_successor_creation_and_final_commit_boundary_rolls_back_cleanly() {
    for parallel in [false, true] {
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
                let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
                let (_, first) = prepare(&mut vm, parallel);
                let current = if finishing {
                    effect(vm.resume(&first.continuation, focus("a")))
                } else {
                    first
                };
                let result = if finishing {
                    focus("next-a")
                } else {
                    focus("a")
                };
                let connection = Connection::open(&path.0).unwrap();
                let before = plan(&connection);
                let records = count(&connection, "vm_effects");
                connection.execute_batch(&format!("CREATE TRIGGER reject_transition BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
                assert!(matches!(
                    vm.resume(&current.continuation, result.clone()),
                    Step::Fault(_)
                ));
                assert_eq!(plan(&connection), before);
                assert_eq!(count(&connection, "vm_effects"), records);
                let state: String = connection
                    .query_row(
                        "SELECT state FROM vm_effects WHERE token = ?1",
                        [current.continuation.token.as_str()],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(state, "pending");
                drop(vm);
                connection
                    .execute_batch("DROP TRIGGER reject_transition")
                    .unwrap();
                let mut vm = Vm::open_journal(&path.0, 1).unwrap();
                let next = vm.resume(&current.continuation, result);
                if finishing {
                    assert_eq!(next, done(true));
                } else {
                    let next = effect(next);
                    assert_eq!(vm.resume(&next.continuation, focus("next-a")), done(true));
                }
            }
        }
    }
}

#[test]
fn reserved_identities_are_not_reused_by_interleaved_work_or_shorter_paths() {
    for selected_path in [true, false] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let source = format!(
            r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: choose(when: {selected_path},
            then: bind(r: ui.focus(node_id: "short"), body: true),
            otherwise: bind(r: ui.focus(node_id: "long"), body: bind(s: ui.focus(node_id: field(value: r, name: "node_id")), body: true))))"#
        );
        let prefix = effect(start(&mut vm, &source));
        let connection = Connection::open(&path.0).unwrap();
        let saved = plan(&connection);
        let reserved = saved["result_binding"]["additional_successor_sequences"][0]
            .as_u64()
            .unwrap();
        let unrelated = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "other")"#));
        assert_ne!(
            unrelated.continuation.token.as_str(),
            format!("continuation-{reserved}")
        );
        let first = effect(vm.resume(&prefix.continuation, focus("a")));
        let mut final_step = vm.resume(
            &first.continuation,
            focus(if selected_path { "short" } else { "long" }),
        );
        if !selected_path {
            let second = effect(final_step);
            assert_eq!(
                second.continuation.token.as_str(),
                format!("continuation-{reserved}")
            );
            final_step = vm.resume(&second.continuation, focus("long"));
        }
        assert_eq!(final_step, done(true));
        drop(vm);
        assert!(Vm::open_journal(&path.0, 1).is_ok());
    }
}

#[test]
fn group_capture_members_do_not_collide_with_reserved_successor_labels() {
    let mut vm = Vm::new(1000);
    let prefix = effect(start(
        &mut vm,
        r#"fn main() = bind(g: seq(successor: ui.focus(node_id: "a"), successor_1: ui.focus(node_id: "b")), body: bind(r: ui.focus(node_id: "c"), body: bind(s: ui.focus(node_id: field(value: r, name: "node_id")), body: true)))"#,
    ));
    let second = effect(vm.resume(&prefix.continuation, focus("a")));
    let first = effect(vm.resume(&second.continuation, focus("b")));
    let last = effect(vm.resume(&first.continuation, focus("c")));
    assert_eq!(vm.resume(&last.continuation, focus("c")), done(true));
}

#[test]
fn three_captures_keep_prior_frames_and_distinct_atomic_types_until_final_calculation() {
    for group in ["seq", "all"] {
        let source = format!(
            r#"fn main() = bind(g: {group}(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body:
            bind(r: ui.focus(node_id: field(value: member(value: g, name: "a"), name: "node_id")), body:
                bind(s: ui.focus(node_id: field(value: r, name: "node_id")), body:
                    bind(t: ui.assert_visible(node_id: field(value: s, name: "node_id")), body:
                        eq(left: field(value: t, name: "node_id"), right: field(value: member(value: g, name: "a"), name: "node_id"))))))"#
        );
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let first = match start(&mut vm, &source) {
            Step::Effect(prefix) => {
                let next = effect(vm.resume(&prefix.continuation, focus("a")));
                effect(vm.resume(&next.continuation, focus("b")))
            }
            Step::Effects(batch) => {
                vm.resume(&batch.branches[1].request.continuation, focus("b"));
                effect(vm.resume(&batch.branches[0].request.continuation, focus("a")))
            }
            other => panic!("{other:?}"),
        };
        let second = effect(vm.resume(&first.continuation, focus("a")));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        let third = effect(vm.resume(&second.continuation, focus("a")));
        assert_eq!(
            third
                .continuation
                .result_binding
                .as_ref()
                .unwrap()
                .results
                .iter()
                .map(|saved| saved.name.as_str())
                .collect::<Vec<_>>(),
            ["r", "s"]
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume(
                &third.continuation,
                EffectResult::Presentation(PresentationResult::AssertVisible {
                    node_id: "a".into()
                })
            ),
            done(true)
        );
        assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 5);
        drop(vm);
        assert!(Vm::open_journal(&path.0, 1).is_ok());
    }
}

#[test]
fn a_sixty_two_member_prefix_and_two_captures_use_exactly_sixty_four_graph_slots() {
    for group in ["seq", "all"] {
        let members = (0..62)
            .map(|index| format!("m{index}: runtime.list()"))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            r#"fn main() = bind(g: {group}({members}), body: bind(r: ui.focus(node_id: "a"), body: bind(s: ui.focus(node_id: "b"), body: true)))"#
        );
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100000).unwrap();
        let first = match start(&mut vm, &source) {
            Step::Effects(batch) => {
                for branch in batch.branches.iter().skip(1).rev() {
                    assert!(matches!(
                        vm.resume(&branch.request.continuation, inventory(0, false)),
                        Step::Waiting(_)
                    ));
                }
                effect(vm.resume(&batch.branches[0].request.continuation, inventory(0, false)))
            }
            mut step => {
                for _ in 0..62 {
                    step = vm.resume(&effect(step).continuation, inventory(0, false));
                }
                effect(step)
            }
        };
        let second = effect(vm.resume(&first.continuation, focus("a")));
        assert_eq!(vm.resume(&second.continuation, focus("b")), done(true));
        assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 64);
        drop(vm);
        assert!(Vm::open_journal(&path.0, 1).is_ok());
    }
}

#[test]
fn competing_valid_receipts_choose_one_result_driven_successor_transactionally() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
    let prefix = effect(start(
        &mut vm,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body:
        bind(r: ui.navigate_focus(node_id: "a", direction: "next"), body:
            bind(s: ui.focus(node_id: field(value: r, name: "focused_node_id")), body:
                eq(left: field(value: s, name: "node_id"), right: field(value: r, name: "focused_node_id")))))"#,
    ));
    let first = effect(vm.resume(&prefix.continuation, focus("a")));
    drop(vm);
    let mut left = Vm::open_journal(&path.0, 1).unwrap();
    let mut right = Vm::open_journal(&path.0, 1).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let one = first.clone();
    let start = barrier.clone();
    let navigate = |node: &str| {
        EffectResult::Presentation(PresentationResult::NavigateFocus {
            node_id: "a".into(),
            direction: leselang_hir::UiFocusNavigationDirection::Next,
            focused_node_id: node.into(),
        })
    };
    let left = std::thread::spawn(move || {
        start.wait();
        left.resume(&one.continuation, navigate("left"))
    });
    let right = std::thread::spawn(move || {
        barrier.wait();
        right.resume(&first.continuation, navigate("right"))
    });
    let next = effect(left.join().unwrap());
    assert_eq!(right.join().unwrap(), Step::Effect(Box::new(next.clone())));
    let leselang_hir::Effect::UiFocus { node_id } = &next.continuation.pending_effect else {
        panic!()
    };
    assert!(node_id == "left" || node_id == "right");
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&next.continuation, focus(node_id)), done(true));
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 3);
}

#[test]
fn conflicting_workers_share_one_successor_and_the_first_committed_receipt() {
    for parallel in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let (_, first) = prepare(&mut vm, parallel);
        drop(vm);
        let mut left = Vm::open_journal(&path.0, 1).unwrap();
        let mut right = Vm::open_journal(&path.0, 1).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let one = first.clone();
        let start = barrier.clone();
        let left = std::thread::spawn(move || {
            start.wait();
            left.resume(&one.continuation, focus("a"))
        });
        let right = std::thread::spawn(move || {
            barrier.wait();
            right.resume(&first.continuation, focus("a"))
        });
        let next = effect(left.join().unwrap());
        assert_eq!(right.join().unwrap(), Step::Effect(Box::new(next.clone())));
        let connection = Connection::open(&path.0).unwrap();
        assert_eq!(count(&connection, "vm_effects"), 4);
        let mut winner = Vm::open_journal(&path.0, 1).unwrap();
        let mut loser = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            winner.resume(&next.continuation, focus("next-a")),
            done(true)
        );
        assert_eq!(
            loser.resume(&next.continuation, focus("not-the-winner")),
            done(true)
        );
    }
}

#[test]
fn recovery_rejects_forged_frames_budgets_reservations_and_incomplete_prefixes() {
    for mutation in 0..9 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let (prefix, first) = prepare(&mut vm, true);
        let second = effect(vm.resume(&first.continuation, focus("a")));
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        if mutation >= 6 {
            let mut saved = plan(&connection);
            match mutation {
                6 => {
                    saved["result_binding"]["additional_successor_sequences"][0] =
                        saved["result_binding"]["successor_sequence"].clone()
                }
                7 => saved["result_binding"]["additional_successor_sequences"]
                    .as_array_mut()
                    .unwrap()
                    .push(999.into()),
                8 => {
                    connection
                        .execute(
                            "UPDATE vm_effects SET terminal_step = ?2 WHERE token = ?1",
                            params![
                                prefix[0].continuation.token.as_str(),
                                serde_json::to_vec(&Step::Fault(leselang_vm::Fault {
                                    code: "LSV1001".into(),
                                    message: "injected".into()
                                }))
                                .unwrap()
                            ],
                        )
                        .unwrap();
                }
                _ => unreachable!(),
            }
            connection
                .execute(
                    "UPDATE vm_merge_groups SET plan = ?1",
                    [serde_json::to_vec(&saved).unwrap()],
                )
                .unwrap();
        } else {
            let mut request = serde_json::to_value(&second).unwrap();
            match mutation {
                0 => {
                    request["continuation"]["result_binding"]["results"][0]["result"]["fields"][0]
                        ["value"] =
                        serde_json::to_value(ScalarValue::String("forged".into())).unwrap()
                }
                1 => {
                    request["continuation"]["result_binding"]["groups"][0]["group"]["members"][0]
                        ["result"]["fields"][0]["value"] =
                        serde_json::to_value(ScalarValue::String("forged".into())).unwrap()
                }
                2 => request["continuation"]["fuel_remaining"] = 1000.into(),
                3 => request["continuation"]["expected_revision"] = 99.into(),
                4 => request["continuation"]["schema_version"] = 9.into(),
                5 => request["continuation"]["result_binding"]["results"] = serde_json::json!([]),
                _ => unreachable!(),
            }
            connection
                .execute(
                    "UPDATE vm_effects SET image = ?2 WHERE token = ?1",
                    params![
                        second.continuation.token.as_str(),
                        serde_json::to_vec(&request["continuation"]).unwrap()
                    ],
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE vm_dispatches SET request = ?2 WHERE token = ?1",
                    params![
                        second.continuation.token.as_str(),
                        serde_json::to_vec(&request).unwrap()
                    ],
                )
                .unwrap();
        }
        assert!(Vm::open_journal(&path.0, 1).is_err(), "mutation {mutation}");
    }
}

#[test]
fn shared_fuel_exhaustion_is_durable_and_restart_cannot_refill_group_chains() {
    let source = r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: bind(s: ui.focus(node_id: "c"), body: true)))"#;
    let mut completed = 0;
    let mut exhausted = 0;
    for fuel in 1..80 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
        let mut step = start(&mut vm, source);
        let mut previous_fuel = fuel;
        for node in ["a", "b", "c"] {
            let Step::Effect(request) = step else { break };
            assert!(request.budget.fuel_remaining <= previous_fuel);
            previous_fuel = request.budget.fuel_remaining;
            drop(vm);
            vm = Vm::open_journal(&path.0, 1000).unwrap();
            step = vm.resume(&request.continuation, focus(node));
        }
        match step {
            Step::Done(_) => completed += 1,
            Step::Fault(ref fault) if fault.code == "LSV1001" => exhausted += 1,
            _ => panic!("fuel {fuel}: {step:?}"),
        }
        drop(vm);
        assert!(Vm::open_journal(&path.0, 1).is_ok(), "fuel {fuel}");
    }
    assert!(completed > 0 && exhausted > 0);
}

#[test]
fn live_group_chains_are_retained_and_completed_chains_are_pruned_as_one_unit() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
    let (_, first) = prepare(&mut vm, true);
    let second = effect(vm.resume(&first.continuation, focus("a")));
    let policy = RetentionPolicy {
        max_completed_records: 1,
        max_delete_per_run: 10,
    };
    let report = vm.compact_journal(&policy).unwrap();
    assert_eq!(report.removed_records, 0);
    assert_eq!(vm.resume(&second.continuation, focus("next-a")), done(true));
    let later = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "later")"#));
    vm.resume(&later.continuation, focus("later"));
    let report = vm.compact_journal(&policy).unwrap();
    assert_eq!(report.removed_records, 1);
    let connection = Connection::open(&path.0).unwrap();
    assert_eq!(count(&connection, "vm_effects"), 1);
    assert_eq!(count(&connection, "vm_merge_groups"), 0);
    drop(vm);
    assert!(Vm::open_journal(&path.0, 1).is_ok());
}

#[test]
fn detached_group_chain_images_cannot_bypass_their_owned_journal() {
    let mut vm = Vm::new(1000);
    let (_, first) = prepare(&mut vm, false);
    let second = effect(vm.resume(&first.continuation, focus("a")));
    for request in [first, second] {
        let mut detached = Vm::new(1000);
        assert_eq!(
            detached
                .restore(request.continuation.clone())
                .unwrap_err()
                .code,
            "LSV1409"
        );
        assert_eq!(
            detached.restore_request(request).unwrap_err().code,
            "LSV1409"
        );
    }
}

fn inventory(size: usize, large: bool) -> EffectResult {
    let mut control = leserpent_domain::InMemoryControlPlane::default();
    let mut runtime = control.register_runtime(
        leselang_host_contract::RuntimeId::new("seed").unwrap(),
        "runtime",
        "http://runtime.local",
    );
    if large {
        runtime.name = "x".repeat(5 * 1024 * 1024);
    }
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
}

#[test]
fn raw_cumulative_item_and_byte_limits_apply_before_every_successor_and_final_projection() {
    for parallel in [false, true] {
        for large in [false, true] {
            for finishing in [false, true] {
                let path = JournalPath::new();
                let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
                let group = if parallel { "all" } else { "seq" };
                let final_body = if finishing {
                    "true"
                } else {
                    "bind(t: runtime.list(), body: true)"
                };
                let source = format!(
                    "fn main() = bind(g: {group}(a: runtime.list(), b: runtime.list()), body: bind(r: runtime.list(), body: bind(s: runtime.list(), body: {final_body})))"
                );
                let (prefix, first) = match start(&mut vm, &source) {
                    Step::Effects(batch) => {
                        let first = batch.branches[0].request.clone();
                        vm.resume(&batch.branches[1].request.continuation, inventory(0, false));
                        let capture = effect(vm.resume(&first.continuation, inventory(0, false)));
                        (first, capture)
                    }
                    Step::Effect(first) => {
                        let second = effect(vm.resume(&first.continuation, inventory(0, false)));
                        let capture = effect(vm.resume(&second.continuation, inventory(0, false)));
                        (*first, capture)
                    }
                    other => panic!("{other:?}"),
                };
                let second = effect(vm.resume(
                    &first.continuation,
                    inventory(if large { 1 } else { 5000 }, large),
                ));
                let terminal = vm.resume(
                    &second.continuation,
                    inventory(if large { 1 } else { 5001 }, large),
                );
                assert!(
                    matches!(&terminal, Step::Fault(fault) if fault.code == if large { "LSV3002" } else { "LSV2404" }),
                    "{terminal:?}"
                );
                assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 4);
                assert_eq!(vm.pending_count(), 0);
                drop(vm);
                let mut vm = Vm::open_journal(&path.0, 1).unwrap();
                assert_eq!(
                    vm.resume(&prefix.continuation, inventory(0, false)),
                    terminal
                );
                assert!(vm.claim_effect(1, 100).unwrap().is_none());
            }
        }
    }
}

#[test]
fn preparation_and_final_calculation_faults_are_durable_without_creating_more_work() {
    for (tail, code, records) in [
        (
            r#"bind(r: ui.focus(node_id: "b"), body: bind(n: div(left: 1, right: 0), body: bind(s: ui.focus(node_id: to_string(value: n)), body: true)))"#,
            "LSV1401",
            2,
        ),
        (
            r#"bind(r: ui.focus(node_id: "b"), body: bind(s: ui.focus(node_id: concat(left: "", right: "")), body: true))"#,
            "LSV1404",
            2,
        ),
        (
            r#"bind(r: ui.focus(node_id: "b"), body: bind(s: ui.focus(node_id: "c"), body: div(left: 1, right: 0)))"#,
            "LSV1401",
            3,
        ),
        (
            r#"bind(r: ui.focus(node_id: "b"), body: bind(s: ui.focus(node_id: "c"), body: add(left: 18446744073709551615, right: 1)))"#,
            "LSV1401",
            3,
        ),
        (
            r#"bind(r: ui.focus(node_id: "b"), body: bind(s: ui.focus(node_id: "c"), body: parse_integer(value: "invalid")))"#,
            "LSV1408",
            3,
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let prefix = effect(start(
            &mut vm,
            &format!(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: {tail})"#),
        ));
        let first = effect(vm.resume(&prefix.continuation, focus("a")));
        let mut terminal = vm.resume(&first.continuation, focus("b"));
        if let Step::Effect(request) = terminal {
            terminal = vm.resume(&request.continuation, focus("c"));
        }
        assert!(
            matches!(&terminal, Step::Fault(fault) if fault.code == code),
            "{terminal:?}"
        );
        assert_eq!(
            count(&Connection::open(&path.0).unwrap(), "vm_effects"),
            records
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&prefix.continuation, focus("ignored")), terminal);
    }
}

#[test]
fn projection_growth_between_captures_is_bounded_before_the_next_effect_is_admitted() {
    let mut body = r#"bind(s: ui.focus(node_id: "c"), body: true)"#.to_string();
    for index in 0..10 {
        body = format!("bind(alias{index}: g, body: {body})");
    }
    let source = format!(
        r#"fn main() = bind(g: repeat(times: 62, body: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: {body}))"#
    );
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100000).unwrap();
    let prefix = effect(start(&mut vm, &source));
    let mut step = Step::Effect(Box::new(prefix.clone()));
    for _ in 0..62 {
        step = vm.resume(&effect(step).continuation, focus("a"));
    }
    let first = effect(step);
    let terminal = vm.resume(&first.continuation, focus("b"));
    assert!(
        matches!(&terminal, Step::Fault(fault) if fault.code == "LSV3002"),
        "{terminal:?}"
    );
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 63);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&prefix.continuation, focus("ignored")), terminal);
}

#[test]
fn leases_cancellation_and_deadlines_do_not_allow_unconfirmed_successors() {
    for mode in ["lease", "cancel", "deadline", "wrong-result"] {
        let source = r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: bind(s: ui.focus(node_id: "c"), body: true)))"#;
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let prefix = effect(vm.start_timed(
            &lower(&parse(source)).unwrap(),
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            Some(Revision(7)),
            100,
            100,
        ));
        let first = effect(vm.resume_at(&prefix.continuation, 101, focus("a")));
        let terminal = match mode {
            "cancel" => vm.cancel_effect(&first.continuation, 102),
            "deadline" => vm.resume_at(&first.continuation, 200, focus("b")),
            "wrong-result" => vm.resume_at(&first.continuation, 102, focus("wrong")),
            _ => {
                let lease = vm.claim_effect(102, 10).unwrap().unwrap();
                assert!(
                    matches!(vm.resume_at(&first.continuation, 103, focus("b")), Step::Fault(fault) if fault.code == "LSV4024")
                );
                assert!(
                    matches!(vm.acknowledge_effect(&lease, 113, focus("b")), Step::Fault(fault) if fault.code == "LSV4023")
                );
                let lease = vm.claim_effect(113, 50).unwrap().unwrap();
                let second = effect(vm.acknowledge_effect(&lease, 114, focus("b")));
                assert_eq!(
                    second.continuation.deadline_at_ms,
                    prefix.continuation.deadline_at_ms
                );
                vm.resume_at(&second.continuation, 200, focus("c"))
            }
        };
        assert!(!matches!(
            terminal,
            Step::Effect(_) | Step::Effects(_) | Step::Waiting(_)
        ));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&prefix.continuation, 201, focus("ignored")),
            terminal
        );
        assert!(vm.claim_effect(202, 50).unwrap().is_none());
    }
}

#[test]
fn later_commands_inherit_confirmation_authority_and_require_correlated_acknowledgement() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: bind(s: debugger.cancel(session_id: field(value: r, name: "node_id")), body: true)))"#)).unwrap();
    assert!(matches!(
        vm.start(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            Some(Revision(7))
        ),
        Step::Fault(_)
    ));
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 0);
    let prefix = effect(vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "debugger.control"]),
        Some(Revision(7)),
    ));
    let first = effect(vm.resume(&prefix.continuation, focus("a")));
    let command_request = effect(vm.resume(&first.continuation, focus("b")));
    let leselang_vm::EffectOperation::Command(command) = &command_request.operation else {
        panic!()
    };
    assert_eq!(
        command.confirmation,
        leselang_host_contract::Confirmation::Confirmed
    );
    assert_eq!(command.expected_revision, Some(Revision(7)));
    let result = EffectResult::DebuggerCancel(leselang_vm::DebuggerCancelResult {
        command_id: command.command_id.clone(),
        session_id: "b".into(),
        observed_at_ms: 2,
    });
    assert!(
        matches!(vm.resume(&command_request.continuation, result.clone()), Step::Fault(fault) if fault.code == "LSV2110")
    );
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    assert_eq!(lease.request, command_request);
    assert_eq!(vm.acknowledge_effect(&lease, 2, result), done(true));
    drop(vm);
    assert!(Vm::open_journal(&path.0, 1).is_ok());
}
