use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{Effect, UiFocusNavigationDirection, UiSelectionState, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
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
            "leselang-group-conditional-{}-{}",
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

fn source(parallel: bool) -> String {
    let group = if parallel { "all" } else { "seq" };
    format!(
        r#"fn main() = bind(g: {group}(nav: ui.navigate_focus(node_id: "a", direction: "next"), flag: ui.assert_selection(node_id: "toggle", state: "selected")), body:
        bind(alias: g, body:
            choose(when: and(left: field(value: member(value: alias, name: "flag"), name: "selected"), right: eq(left: field(value: member(value: g, name: "nav"), name: "focused_node_id"), right: "stop")), then: 0, otherwise:
                bind(first: ui.navigate_focus(node_id: field(value: member(value: alias, name: "nav"), name: "node_id"), direction: "next"), body:
                    choose(when: eq(left: field(value: first, name: "focused_node_id"), right: "stop"), then: 1, otherwise:
                        bind(previous: first, body:
                            bind(second: ui.navigate_focus(node_id: field(value: previous, name: "focused_node_id"), direction: "next"), body:
                                choose(when: eq(left: field(value: second, name: "focused_node_id"), right: "stop"), then: 2, otherwise:
                                    bind(last: ui.assert_visible(node_id: field(value: second, name: "focused_node_id")), body:
                                        choose(when: eq(left: field(value: last, name: "node_id"), right: field(value: second, name: "focused_node_id")), then: 3, otherwise: 4))))))))))"#
    )
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
fn done(value: u64) -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Integer(value),
    })
}
fn navigate(node: &str, destination: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::NavigateFocus {
        node_id: node.into(),
        direction: UiFocusNavigationDirection::Next,
        focused_node_id: destination.into(),
    })
}
fn selected() -> EffectResult {
    EffectResult::Presentation(PresentationResult::AssertSelection {
        node_id: "toggle".into(),
        state: UiSelectionState::Selected,
    })
}
fn receipt(request: &EffectRequest, stop: bool) -> EffectResult {
    match &request.continuation.pending_effect {
        Effect::UiNavigateFocus { node_id, .. } => navigate(
            node_id,
            if stop {
                "stop"
            } else if node_id == "a" {
                "b"
            } else {
                "a"
            },
        ),
        Effect::UiAssertVisible { node_id } => {
            EffectResult::Presentation(PresentationResult::AssertVisible {
                node_id: node_id.clone(),
            })
        }
        other => panic!("unexpected effect: {other:?}"),
    }
}
fn prefix(vm: &mut Vm, parallel: bool, stop: bool) -> (Vec<EffectRequest>, Step) {
    match start(vm, &source(parallel)) {
        Step::Effects(batch) => {
            let requests = batch
                .branches
                .into_iter()
                .map(|branch| branch.request)
                .collect::<Vec<_>>();
            assert!(matches!(
                vm.resume(&requests[1].continuation, selected()),
                Step::Waiting(_)
            ));
            let next = vm.resume(&requests[0].continuation, receipt(&requests[0], stop));
            (requests, next)
        }
        Step::Effect(first) => {
            let second = effect(vm.resume(&first.continuation, receipt(&first, stop)));
            let next = vm.resume(&second.continuation, selected());
            (vec![*first, second], next)
        }
        other => panic!("{other:?}"),
    }
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

#[test]
fn early_exits_before_or_between_captures_work_for_both_prefixes_and_journals() {
    for parallel in [false, true] {
        for stop_at in 0..=3 {
            let path = JournalPath::new();
            for mut vm in [Vm::new(1000), Vm::open_journal(&path.0, 1000).unwrap()] {
                let (mut requests, mut step) = prefix(&mut vm, parallel, stop_at == 0);
                for index in 1..=stop_at {
                    let request = effect(step);
                    step = vm.resume(&request.continuation, receipt(&request, index == stop_at));
                    requests.push(request);
                }
                assert_eq!(step, done(stop_at));
                assert_eq!(requests.len(), 2 + stop_at as usize);
                for request in &requests {
                    assert_eq!(request.continuation.schema_version, 11);
                    assert_eq!(
                        request.continuation.group_result,
                        requests[0].continuation.group_result
                    );
                    assert_eq!(request.continuation.expected_revision, Some(Revision(7)));
                    assert_eq!(
                        request.budget.max_output_items,
                        requests[0].budget.max_output_items
                    );
                    assert_eq!(
                        decode_continuation(&encode_continuation(&request.continuation).unwrap())
                            .unwrap(),
                        request.continuation
                    );
                    assert_eq!(
                        vm.resume(&request.continuation, navigate("a", "changed")),
                        done(stop_at)
                    );
                }
                for pair in requests[2..].windows(2) {
                    assert!(pair[1].budget.fuel_remaining < pair[0].budget.fuel_remaining);
                }
                if stop_at > 0 {
                    let saved = requests
                        .last()
                        .unwrap()
                        .continuation
                        .result_binding
                        .as_ref()
                        .unwrap();
                    assert_eq!(
                        saved
                            .groups
                            .iter()
                            .map(|group| group.name.as_str())
                            .collect::<Vec<_>>(),
                        ["g", "alias"]
                    );
                    assert_eq!(
                        saved.groups[0].group.members[1].result.projection_version,
                        2
                    );
                    assert_eq!(saved.body.is_pure(), stop_at == 3);
                    assert!(saved.body.can_return_without_suspending());
                }
                assert_eq!(vm.pending_count(), 0);
            }
        }
    }
}

#[test]
fn every_exit_replays_after_restart_and_unused_reserved_slots_stay_rowless() {
    for parallel in [false, true] {
        for stop_at in 0..=3 {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
            let (prefix, mut step) = prefix(&mut vm, parallel, stop_at == 0);
            let connection = Connection::open(&path.0).unwrap();
            let saved = plan(&connection);
            assert_eq!(
                saved["order"],
                if parallel {
                    "parallel_group_conditional"
                } else {
                    "group_conditional"
                }
            );
            let mut reserved = vec![
                saved["result_binding"]["successor_sequence"]
                    .as_u64()
                    .unwrap(),
            ];
            reserved.extend(
                saved["result_binding"]["additional_successor_sequences"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|sequence| sequence.as_u64().unwrap()),
            );
            assert_eq!(reserved.len(), 3);
            for index in 1..=stop_at {
                let request = effect(step);
                drop(vm);
                vm = Vm::open_journal(&path.0, 1).unwrap();
                let lease = vm.claim_effect(1, 100).unwrap().unwrap();
                assert_eq!(lease.request, request);
                step = vm.acknowledge_effect(&lease, 2, receipt(&request, index == stop_at));
                let raw: Vec<u8> = connection
                    .query_row(
                        "SELECT terminal_step FROM vm_effects WHERE token = ?1",
                        [request.continuation.token.as_str()],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert!(matches!(
                    serde_json::from_slice::<Step>(&raw).unwrap(),
                    Step::Done(Value::UiNavigateFocus { .. } | Value::UiAssertVisible { .. })
                ));
            }
            assert_eq!(step, done(stop_at));
            assert_eq!(count(&connection, "vm_effects"), 2 + stop_at as i64);
            for (index, sequence) in reserved.iter().enumerate() {
                let exists: bool = connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM vm_effects WHERE token = ?1)",
                        [format!("continuation-{sequence}")],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(exists, index < stop_at as usize);
            }
            let unrelated = effect(start(
                &mut vm,
                r#"fn main() = ui.focus(node_id: "unrelated")"#,
            ));
            assert!(
                reserved
                    .iter()
                    .all(|sequence| unrelated.continuation.token.as_str()
                        != format!("continuation-{sequence}"))
            );
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            for request in prefix {
                assert_eq!(
                    vm.resume(&request.continuation, navigate("a", "changed")),
                    done(stop_at)
                );
            }
        }
    }
}

#[test]
fn parallel_early_exit_still_waits_for_every_successful_prefix_member() {
    let mut vm = Vm::new(1000);
    let Step::Effects(batch) = start(&mut vm, &source(true)) else {
        panic!()
    };
    let nav = &batch.branches[0].request;
    let flag = &batch.branches[1].request;
    let waiting = vm.resume(&nav.continuation, navigate("a", "stop"));
    assert!(matches!(waiting, Step::Waiting(_)));
    assert_eq!(vm.pending_count(), 1);
    assert_eq!(vm.resume(&nav.continuation, navigate("a", "a")), waiting);
    assert_eq!(vm.resume(&flag.continuation, selected()), done(0));
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn early_scalar_commits_and_later_admissions_are_atomic_at_every_write_boundary() {
    for parallel in [false, true] {
        for boundary in ["prefix-exit", "capture-exit", "capture-next"] {
            let writes: &[(&str, &str)] = if boundary == "capture-next" {
                &[
                    ("vm_effects", "UPDATE"),
                    ("vm_dispatches", "UPDATE"),
                    ("vm_effects", "INSERT"),
                    ("vm_dispatches", "INSERT"),
                    ("vm_merge_groups", "UPDATE"),
                    ("vm_merge_branches", "INSERT"),
                ]
            } else {
                &[
                    ("vm_effects", "UPDATE"),
                    ("vm_dispatches", "UPDATE"),
                    ("vm_merge_groups", "UPDATE"),
                ]
            };
            for (table, operation) in writes {
                let path = JournalPath::new();
                let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
                let (current, result, expected) = if boundary == "prefix-exit" {
                    match start(&mut vm, &source(parallel)) {
                        Step::Effects(batch) => {
                            assert!(matches!(
                                vm.resume(&batch.branches[1].request.continuation, selected()),
                                Step::Waiting(_)
                            ));
                            let current = batch.branches[0].request.clone();
                            let result = receipt(&current, true);
                            (current, result, done(0))
                        }
                        Step::Effect(first) => {
                            let current =
                                effect(vm.resume(&first.continuation, receipt(&first, true)));
                            (current, selected(), done(0))
                        }
                        other => panic!("{other:?}"),
                    }
                } else {
                    let (_, next) = prefix(&mut vm, parallel, false);
                    let current = effect(next);
                    let exiting = boundary == "capture-exit";
                    let result = receipt(&current, exiting);
                    (current, result, done(if exiting { 1 } else { 2 }))
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
                let mut step = vm.resume(&current.continuation, result);
                if boundary == "capture-next" {
                    let next = effect(step);
                    step = vm.resume(&next.continuation, receipt(&next, true));
                }
                assert_eq!(step, expected);
            }
        }
    }
}

#[test]
fn conflicting_valid_receipts_commit_only_one_exit_or_successor() {
    for parallel in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let (_, next) = prefix(&mut vm, parallel, false);
        let current = effect(next);
        drop(vm);
        let mut left = Vm::open_journal(&path.0, 1).unwrap();
        let mut right = Vm::open_journal(&path.0, 1).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let first = current.clone();
        let start = barrier.clone();
        let left = std::thread::spawn(move || {
            start.wait();
            left.resume(&first.continuation, receipt(&first, true))
        });
        let second = current.clone();
        let right = std::thread::spawn(move || {
            barrier.wait();
            right.resume(&second.continuation, receipt(&second, false))
        });
        let mut committed = left.join().unwrap();
        assert_eq!(right.join().unwrap(), committed);
        let connection = Connection::open(&path.0).unwrap();
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        if let Step::Effect(request) = committed {
            assert_eq!(count(&connection, "vm_effects"), 4);
            committed = vm.resume(&request.continuation, receipt(&request, true));
            assert_eq!(committed, done(2));
        } else {
            assert_eq!(count(&connection, "vm_effects"), 3);
            assert_eq!(committed, done(1));
        }
        assert_eq!(
            vm.resume(&current.continuation, receipt(&current, false)),
            committed
        );
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn recovery_rejects_schema_downgrades_forged_frames_and_reserved_id_reuse() {
    for mutation in 0..8 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let (_, next) = prefix(&mut vm, true, false);
        let first = effect(next);
        let second = effect(vm.resume(&first.continuation, receipt(&first, false)));
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        if mutation < 6 {
            let target = if mutation == 0 { &first } else { &second };
            let mut request = serde_json::to_value(target).unwrap();
            match mutation {
                0 | 1 => request["continuation"]["schema_version"] = 10.into(),
                2 => {
                    request["continuation"]["result_binding"]["results"][0]["result"]["fields"][0]
                        ["value"] =
                        serde_json::to_value(ScalarValue::String("forged".into())).unwrap()
                }
                3 => request["continuation"]["fuel_remaining"] = 1000.into(),
                4 => request["continuation"]["expected_revision"] = 99.into(),
                5 => request["continuation"]["group_result"] = "merge-999".into(),
                _ => unreachable!(),
            }
            connection
                .execute(
                    "UPDATE vm_effects SET image = ?2 WHERE token = ?1",
                    params![
                        target.continuation.token.as_str(),
                        serde_json::to_vec(&request["continuation"]).unwrap()
                    ],
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE vm_dispatches SET request = ?2 WHERE token = ?1",
                    params![
                        target.continuation.token.as_str(),
                        serde_json::to_vec(&request).unwrap()
                    ],
                )
                .unwrap();
        } else {
            let mut saved = plan(&connection);
            if mutation == 6 {
                saved["order"] = "parallel_group_dataflow".into();
            } else {
                saved["result_binding"]["additional_successor_sequences"][0] =
                    saved["result_binding"]["successor_sequence"].clone();
            }
            connection
                .execute(
                    "UPDATE vm_merge_groups SET plan = ?1",
                    [serde_json::to_vec(&saved).unwrap()],
                )
                .unwrap();
        }
        assert!(Vm::open_journal(&path.0, 1).is_err(), "mutation {mutation}");
    }
}

#[test]
fn recovery_rejects_scalar_terminals_before_a_mandatory_capture() {
    for after_capture in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let prefix = effect(start(
            &mut vm,
            r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: bind(s: ui.focus(node_id: "c"), body: choose(when: true, then: 1, otherwise: bind(t: ui.focus(node_id: "d"), body: 2)))))"#,
        ));
        let next = if after_capture {
            effect(vm.resume(
                &prefix.continuation,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "a".into(),
                }),
            ))
        } else {
            prefix
        };
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        let node = if after_capture { "b" } else { "a" };
        let raw = Step::Done(Value::UiFocus {
            node_id: node.into(),
        });
        connection
            .execute(
                "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
                params![
                    next.continuation.token.as_str(),
                    serde_json::to_vec(&raw).unwrap()
                ],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE vm_dispatches SET state = 'acknowledged' WHERE token = ?1",
                [next.continuation.token.as_str()],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE vm_merge_groups SET state = 'completed', terminal_step = ?1",
                [serde_json::to_vec(&done(1)).unwrap()],
            )
            .unwrap();
        assert!(Vm::open_journal(&path.0, 1).is_err());
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
fn scalar_early_exits_cannot_hide_cumulative_raw_item_or_byte_overflow() {
    for parallel in [false, true] {
        for after_capture in [false, true] {
            for large in [false, true] {
                let path = JournalPath::new();
                let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
                let group = if parallel { "all" } else { "seq" };
                let tail = if after_capture {
                    "bind(r: runtime.list(), body: choose(when: true, then: 1, otherwise: bind(s: runtime.list(), body: 2)))"
                } else {
                    "choose(when: true, then: 0, otherwise: bind(r: runtime.list(), body: 1))"
                };
                let mut step = start(
                    &mut vm,
                    &format!(
                        "fn main() = bind(g: {group}(a: runtime.list(), b: runtime.list()), body: {tail})"
                    ),
                );
                let first_result = inventory(if large { 1 } else { 5000 }, large);
                let last_result = inventory(if large { 1 } else { 5001 }, large);
                let first;
                match step {
                    Step::Effects(batch) => {
                        first = batch.branches[0].request.clone();
                        assert!(matches!(
                            vm.resume(&first.continuation, first_result),
                            Step::Waiting(_)
                        ));
                        step = vm.resume(
                            &batch.branches[1].request.continuation,
                            if after_capture {
                                inventory(0, false)
                            } else {
                                last_result.clone()
                            },
                        );
                    }
                    Step::Effect(request) => {
                        first = *request;
                        let second = effect(vm.resume(&first.continuation, first_result));
                        step = vm.resume(
                            &second.continuation,
                            if after_capture {
                                inventory(0, false)
                            } else {
                                last_result.clone()
                            },
                        );
                    }
                    other => panic!("{other:?}"),
                }
                if after_capture {
                    let request = effect(step);
                    step = vm.resume(&request.continuation, last_result);
                }
                assert!(
                    matches!(&step, Step::Fault(fault) if fault.code == if large { "LSV3002" } else { "LSV2404" }),
                    "{step:?}"
                );
                assert_eq!(
                    count(&Connection::open(&path.0).unwrap(), "vm_effects"),
                    if after_capture { 3 } else { 2 }
                );
                assert_eq!(vm.pending_count(), 0);
                drop(vm);
                let mut vm = Vm::open_journal(&path.0, 1).unwrap();
                assert_eq!(vm.resume(&first.continuation, inventory(0, false)), step);
                assert!(vm.claim_effect(1, 100).unwrap().is_none());
            }
        }
    }
}

#[test]
fn conditional_fuel_is_shared_and_restart_never_refills_the_flow() {
    let mut completed = 0;
    let mut exhausted = 0;
    for fuel in (1..=90).chain([1000]) {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
        let mut step = start(&mut vm, &source(false));
        let mut previous = fuel;
        while let Step::Effect(request) = step {
            assert!(request.budget.fuel_remaining <= previous);
            previous = request.budget.fuel_remaining;
            let result = if matches!(
                request.continuation.pending_effect,
                Effect::UiAssertSelection { .. }
            ) {
                selected()
            } else {
                receipt(&request, false)
            };
            drop(vm);
            vm = Vm::open_journal(&path.0, 1000).unwrap();
            step = vm.resume(&request.continuation, result);
        }
        match step {
            Step::Done(_) => {
                assert_eq!(step, done(3));
                completed += 1;
            }
            Step::Fault(ref fault) if fault.code == "LSV1001" => exhausted += 1,
            other => panic!("fuel {fuel}: {other:?}"),
        }
        drop(vm);
        assert!(Vm::open_journal(&path.0, 1).is_ok(), "fuel {fuel}");
    }
    assert!(completed > 0 && exhausted > 0);
}

#[test]
fn early_exit_does_not_recover_failed_cancelled_expired_or_unacknowledged_effects() {
    for parallel in [false, true] {
        for in_prefix in [false, true] {
            for mode in ["cancel", "deadline", "wrong-result", "lease"] {
                let path = JournalPath::new();
                let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
                let step = vm.start_timed(
                    &lower(&parse(&source(parallel))).unwrap(),
                    Principal::new("operator").unwrap(),
                    CapabilitySet::new(["ui.presentation"]),
                    Some(Revision(7)),
                    100,
                    100,
                );
                let current = match step {
                    Step::Effects(batch) => {
                        vm.resume_at(&batch.branches[1].request.continuation, 101, selected());
                        let nav = batch.branches[0].request.clone();
                        if in_prefix {
                            nav
                        } else {
                            effect(vm.resume_at(&nav.continuation, 101, receipt(&nav, false)))
                        }
                    }
                    Step::Effect(nav) => {
                        let flag =
                            effect(vm.resume_at(&nav.continuation, 101, receipt(&nav, in_prefix)));
                        if in_prefix {
                            flag
                        } else {
                            effect(vm.resume_at(&flag.continuation, 101, selected()))
                        }
                    }
                    other => panic!("{other:?}"),
                };
                let result = if matches!(
                    current.continuation.pending_effect,
                    Effect::UiAssertSelection { .. }
                ) {
                    selected()
                } else {
                    receipt(&current, true)
                };
                let terminal = match mode {
                    "cancel" => vm.cancel_effect(&current.continuation, 102),
                    "deadline" => vm.resume_at(&current.continuation, 200, result),
                    "wrong-result" => {
                        vm.resume_at(&current.continuation, 102, navigate("wrong", "stop"))
                    }
                    _ => {
                        let lease = vm.claim_effect(102, 10).unwrap().unwrap();
                        assert_eq!(lease.request, current);
                        assert!(
                            matches!(vm.resume_at(&current.continuation, 103, result.clone()), Step::Fault(fault) if fault.code == "LSV4024")
                        );
                        assert!(
                            matches!(vm.acknowledge_effect(&lease, 113, result.clone()), Step::Fault(fault) if fault.code == "LSV4023")
                        );
                        let lease = vm.claim_effect(113, 50).unwrap().unwrap();
                        let step = vm.acknowledge_effect(&lease, 114, result);
                        assert_eq!(step, done(if in_prefix { 0 } else { 1 }));
                        drop(vm);
                        assert!(Vm::open_journal(&path.0, 1).is_ok());
                        continue;
                    }
                };
                assert!(
                    !matches!(
                        terminal,
                        Step::Effect(_) | Step::Effects(_) | Step::Waiting(_) | Step::Done(_)
                    ),
                    "{terminal:?}"
                );
                drop(vm);
                let mut vm = Vm::open_journal(&path.0, 1).unwrap();
                assert_eq!(
                    vm.resume_at(&current.continuation, 201, navigate("a", "stop")),
                    terminal
                );
                assert!(vm.claim_effect(202, 50).unwrap().is_none());
            }
        }
    }
}

#[test]
fn early_exit_and_pure_preparation_faults_are_durable_without_new_work() {
    for after_capture in [false, true] {
        for (expression, code) in [
            ("div(left: 1, right: 0)", "LSV1401"),
            ("parse_integer(value: \"invalid\")", "LSV1408"),
        ] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
            let exit = format!(
                "choose(when: true, then: {expression}, otherwise: bind(s: runtime.list(), body: 0))"
            );
            let tail = if after_capture {
                format!("bind(r: runtime.list(), body: {exit})")
            } else {
                exit
            };
            let first = effect(start(
                &mut vm,
                &format!("fn main() = bind(g: seq(a: runtime.list()), body: {tail})"),
            ));
            let mut step = vm.resume(&first.continuation, inventory(0, false));
            if after_capture {
                step = vm.resume(&effect(step).continuation, inventory(0, false));
            }
            assert!(
                matches!(&step, Step::Fault(fault) if fault.code == code),
                "{step:?}"
            );
            assert_eq!(
                count(&Connection::open(&path.0).unwrap(), "vm_effects"),
                if after_capture { 2 } else { 1 }
            );
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            assert_eq!(vm.resume(&first.continuation, inventory(0, false)), step);
        }
    }
}

#[test]
fn early_exit_skips_cold_projection_growth_but_selected_growth_stays_bounded() {
    for exiting in [false, true] {
        let mut cold = r#"bind(s: ui.focus(node_id: "c"), body: 1)"#.to_string();
        for index in 0..10 {
            cold = format!("bind(alias{index}: g, body: {cold})");
        }
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100000).unwrap();
        let source = format!(
            r#"fn main() = bind(g: repeat(times: 62, body: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: choose(when: {exiting}, then: 0, otherwise: {cold})))"#
        );
        let prefix = effect(start(&mut vm, &source));
        let mut step = Step::Effect(Box::new(prefix.clone()));
        for _ in 0..62 {
            step = vm.resume(
                &effect(step).continuation,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "a".into(),
                }),
            );
        }
        let current = effect(step);
        let terminal = vm.resume(
            &current.continuation,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "b".into(),
            }),
        );
        if exiting {
            assert_eq!(terminal, done(0));
        } else {
            assert!(
                matches!(&terminal, Step::Fault(fault) if fault.code == "LSV3002"),
                "{terminal:?}"
            );
        }
        assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 63);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume(
                &prefix.continuation,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "ignored".into()
                })
            ),
            terminal
        );
    }
}

#[test]
fn live_conditional_groups_and_completed_early_exits_compact_as_owned_units() {
    for stop_at in [0, 1] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let (_, mut step) = prefix(&mut vm, true, stop_at == 0);
        let policy = RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 10,
        };
        if stop_at == 1 {
            assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 0);
            let request = effect(step);
            step = vm.resume(&request.continuation, receipt(&request, true));
        }
        assert_eq!(step, done(stop_at));
        let later = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "later")"#));
        vm.resume(
            &later.continuation,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "later".into(),
            }),
        );
        assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 1);
        let connection = Connection::open(&path.0).unwrap();
        assert_eq!(count(&connection, "vm_effects"), 1);
        assert_eq!(count(&connection, "vm_merge_groups"), 0);
        drop(vm);
        assert!(Vm::open_journal(&path.0, 1).is_ok());
    }
}

#[test]
fn owned_conditional_images_cannot_be_restored_as_detached_work() {
    let mut vm = Vm::new(1000);
    let (prefix, next) = prefix(&mut vm, false, false);
    let first = effect(next);
    let second = effect(vm.resume(&first.continuation, receipt(&first, false)));
    for request in prefix.into_iter().chain([first, second]) {
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

#[test]
fn cold_command_authority_is_required_even_if_the_initial_exit_is_selected() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: choose(when: true, then: 0, otherwise: bind(r: debugger.cancel(session_id: "s"), body: 1)))"#)).unwrap();
    assert!(matches!(
        start(
            &mut vm,
            &leselang_hir::canonical_source(&program.function.effect).unwrap()
        ),
        Step::Fault(_)
    ));
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 0);
    let first = effect(vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "debugger.control"]),
        Some(Revision(7)),
    ));
    assert_eq!(
        vm.resume(
            &first.continuation,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "a".into()
            })
        ),
        done(0)
    );
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 1);
}

#[test]
fn a_captured_command_still_requires_confirmation_and_correlated_lease_acknowledgement() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: choose(when: false, then: 0, otherwise: bind(r: debugger.cancel(session_id: "s"), body: choose(when: true, then: 1, otherwise: bind(t: ui.focus(node_id: "a"), body: 2)))))"#)).unwrap();
    let first = effect(vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "debugger.control"]),
        Some(Revision(7)),
    ));
    let current = effect(vm.resume(
        &first.continuation,
        EffectResult::Presentation(PresentationResult::Focus {
            node_id: "a".into(),
        }),
    ));
    assert_eq!(current.continuation.schema_version, 11);
    let leselang_vm::EffectOperation::Command(command) = &current.operation else {
        panic!()
    };
    assert_eq!(
        command.confirmation,
        leselang_host_contract::Confirmation::Confirmed
    );
    assert_eq!(command.expected_revision, Some(Revision(7)));
    let result = EffectResult::DebuggerCancel(leselang_vm::DebuggerCancelResult {
        command_id: command.command_id.clone(),
        session_id: "s".into(),
        observed_at_ms: 2,
    });
    assert!(
        matches!(vm.resume(&current.continuation, result.clone()), Step::Fault(fault) if fault.code == "LSV2110")
    );
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    assert_eq!(lease.request, current);
    assert_eq!(vm.acknowledge_effect(&lease, 2, result), done(1));
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 2);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume(&first.continuation, navigate("ignored", "stop")),
        done(1)
    );
}

#[test]
fn missing_member_receipts_cannot_be_forged_into_a_successful_scalar_exit() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
    let Step::Effects(batch) = start(&mut vm, &source(true)) else {
        panic!()
    };
    assert!(matches!(
        vm.resume(
            &batch.branches[0].request.continuation,
            navigate("a", "stop")
        ),
        Step::Waiting(_)
    ));
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    connection
        .execute(
            "UPDATE vm_merge_groups SET state = 'completed', terminal_step = ?1",
            [serde_json::to_vec(&done(0)).unwrap()],
        )
        .unwrap();
    assert!(Vm::open_journal(&path.0, 1).is_err());
}

#[test]
fn failed_parallel_prefix_never_takes_a_scalar_success_branch() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
    let Step::Effects(batch) = start(&mut vm, &source(true)) else {
        panic!()
    };
    vm.resume(
        &batch.branches[0].request.continuation,
        navigate("a", "stop"),
    );
    let step = vm.resume(
        &batch.branches[1].request.continuation,
        navigate("wrong", "stop"),
    );
    assert!(matches!(step, Step::Fault(_)));
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 2);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume(
            &batch.branches[0].request.continuation,
            navigate("a", "stop")
        ),
        step
    );
}
