use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};

use leselang_hir::{UiSelectionState, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision, RuntimeId};
use leselang_syntax::parse;
use leselang_vm::{
    BranchCompletion, BranchOutcome, DebuggerCancelResult, EffectOperation, EffectRequest,
    EffectResult, MergePlan, PresentationResult, RetentionPolicy, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation, merge_declared,
};
use rusqlite::{Connection, params};

const TAIL: &str = r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")),
    body: ui.focus(node_id: field(value: member(value: g, name: "b"), name: "node_id")))"#;
const CAPTURE: &str = r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")),
    body: bind(r: ui.focus(node_id: field(value: member(value: g, name: "b"), name: "node_id")),
        body: eq(left: field(value: member(value: g, name: "a"), name: "node_id"), right: field(value: r, name: "node_id"))))"#;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-parallel-tail-{}-{}",
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
fn batch(step: Step) -> Vec<EffectRequest> {
    match step {
        Step::Effects(batch) => batch
            .branches
            .into_iter()
            .map(|branch| branch.request)
            .collect(),
        other => panic!("expected parallel batch: {other:?}"),
    }
}
fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("expected successor: {other:?}"),
    }
}
fn focus(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
}
fn scalar(value: ScalarValue) -> Step {
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
fn prepare(vm: &mut Vm, source: &str) -> (Vec<EffectRequest>, EffectRequest) {
    let prefix = batch(start(vm, source));
    assert!(matches!(
        vm.resume(&prefix[1].continuation, focus("b")),
        Step::Waiting(_)
    ));
    let tail = effect(vm.resume(&prefix[0].continuation, focus("a")));
    (prefix, tail)
}

#[test]
fn every_prefix_order_waits_for_all_success_before_admitting_one_owned_successor() {
    for durable in [false, true] {
        for captured in [false, true] {
            for reversed in [false, true] {
                let path = JournalPath::new();
                let mut vm = if durable {
                    Vm::open_journal(&path.0, 500).unwrap()
                } else {
                    Vm::new(500)
                };
                let prefix = batch(start(&mut vm, if captured { CAPTURE } else { TAIL }));
                assert_eq!(prefix.len(), 2);
                assert_eq!(
                    prefix[0].budget.fuel_remaining,
                    prefix[1].budget.fuel_remaining
                );
                let owner = prefix[0].continuation.group_result.clone().unwrap();
                for request in &prefix {
                    assert_eq!(request.continuation.schema_version, 9);
                    assert_eq!(request.continuation.group_result.as_ref(), Some(&owner));
                    assert!(request.continuation.result_binding.is_none());
                }
                let first = usize::from(reversed);
                let last = 1 - first;
                let waiting = vm.resume(
                    &prefix[first].continuation,
                    focus(if first == 0 { "a" } else { "b" }),
                );
                assert!(
                    matches!(waiting, Step::Waiting(wait) if wait.completed_branches == 1 && wait.total_branches == 2)
                );
                assert_eq!(vm.merge_result(&owner).unwrap(), None);
                if durable {
                    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 2);
                }
                let tail = effect(vm.resume(
                    &prefix[last].continuation,
                    focus(if last == 0 { "a" } else { "b" }),
                ));
                assert_eq!(tail.continuation.schema_version, 9);
                assert_eq!(tail.continuation.group_result.as_ref(), Some(&owner));
                assert_eq!(tail.continuation.result_binding.is_some(), captured);
                assert!(tail.budget.fuel_remaining < prefix[0].budget.fuel_remaining);
                assert_eq!(
                    tail.budget.max_output_items,
                    prefix[0].budget.max_output_items
                );
                assert_eq!(tail.continuation.expected_revision, Some(Revision(7)));
                assert_eq!(
                    effect(vm.resume(&prefix[first].continuation, focus("changed"))),
                    tail
                );
                let expected = if captured {
                    scalar(ScalarValue::Boolean(false))
                } else {
                    Step::Done(Value::UiFocus {
                        node_id: "b".into(),
                    })
                };
                assert_eq!(vm.resume(&tail.continuation, focus("b")), expected);
                assert_eq!(vm.merge_result(&owner).unwrap(), Some(expected.clone()));
                for request in prefix.iter().chain(std::iter::once(&tail)) {
                    assert_eq!(vm.resume(&request.continuation, focus("ignored")), expected);
                }
                assert!(vm.claim_effect(1, 100).unwrap().is_none());
            }
        }
    }
}

#[test]
fn out_of_order_prefix_and_capture_restart_preserve_raw_receipts_and_saved_scalar() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let prefix = batch(start(&mut vm, CAPTURE));
    assert!(matches!(
        vm.resume(&prefix[1].continuation, focus("b")),
        Step::Waiting(_)
    ));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.pending_continuations(),
        std::slice::from_ref(&prefix[0].continuation)
    );
    let tail = effect(vm.resume(&prefix[0].continuation, focus("a")));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.pending_continuations(),
        std::slice::from_ref(&tail.continuation)
    );
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    let terminal = vm.acknowledge_effect(&lease, 2, focus("b"));
    assert_eq!(terminal, scalar(ScalarValue::Boolean(false)));
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    assert_eq!(plan(&connection)["order"], "parallel_group_capture");
    assert_eq!(count(&connection, "vm_effects"), 3);
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
            node_id: "b".into()
        })
    );
    let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
    assert_eq!(
        vm.resume(&prefix[0].continuation, focus("changed")),
        terminal
    );
    assert_eq!(vm.resume(&tail.continuation, focus("changed")), terminal);
}

#[test]
fn independent_prefix_leases_and_a_single_tail_lease_keep_correlation_fences() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let prefix = batch(start(&mut vm, CAPTURE));
    let left = vm.claim_effect(1, 100).unwrap().unwrap();
    let right = vm.claim_effect(1, 100).unwrap().unwrap();
    assert_ne!(
        left.request.continuation.token,
        right.request.continuation.token
    );
    assert!(vm.claim_effect(1, 100).unwrap().is_none());
    let result_for = |request: &EffectRequest| {
        if request.continuation == prefix[0].continuation {
            focus("a")
        } else {
            focus("b")
        }
    };
    assert!(matches!(
        vm.acknowledge_effect(&right, 2, result_for(&right.request)),
        Step::Waiting(_)
    ));
    let tail = effect(vm.acknowledge_effect(&left, 2, result_for(&left.request)));
    let stale = vm.claim_effect(3, 10).unwrap().unwrap();
    assert_eq!(stale.request, tail);
    assert!(
        matches!(vm.resume_at(&tail.continuation, 4, focus("b")), Step::Fault(fault) if fault.code == "LSV4024")
    );
    assert!(
        matches!(vm.acknowledge_effect(&stale, 14, focus("b")), Step::Fault(fault) if fault.code == "LSV4023")
    );
    let active = vm.claim_effect(14, 100).unwrap().unwrap();
    assert_eq!(
        vm.acknowledge_effect(&active, 15, focus("b")),
        scalar(ScalarValue::Boolean(false))
    );
    Vm::open_journal(&path.0, 1).unwrap();
}

#[test]
fn failed_or_cancelled_prefix_waits_for_other_members_and_never_creates_a_tail() {
    for durable in [false, true] {
        for cancel in [false, true] {
            let path = JournalPath::new();
            let mut vm = if durable {
                Vm::open_journal(&path.0, 500).unwrap()
            } else {
                Vm::new(500)
            };
            let prefix = batch(start(&mut vm, CAPTURE));
            let step = if cancel {
                vm.cancel_effect(&prefix[1].continuation, 1)
            } else {
                vm.resume(&prefix[1].continuation, focus("wrong"))
            };
            assert!(matches!(step, Step::Waiting(_)), "{step:?}");
            let terminal = vm.resume(&prefix[0].continuation, focus("a"));
            assert!(matches!(terminal, Step::Fault(_) | Step::Cancelled(_)));
            assert!(vm.claim_effect(2, 100).unwrap().is_none());
            assert_eq!(vm.resume(&prefix[1].continuation, focus("b")), terminal);
            if durable {
                assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 2);
                drop(vm);
                let mut vm = Vm::open_journal(&path.0, 1).unwrap();
                assert_eq!(
                    vm.resume(&prefix[0].continuation, focus("ignored")),
                    terminal
                );
            }
        }
    }
}

#[test]
fn prefix_admission_and_tail_completion_rollback_at_every_transactional_write() {
    for captured in [false, true] {
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
                let prefix = batch(start(&mut vm, if captured { CAPTURE } else { TAIL }));
                vm.resume(&prefix[1].continuation, focus("b"));
                let current = if finishing {
                    effect(vm.resume(&prefix[0].continuation, focus("a")))
                } else {
                    prefix[0].clone()
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
                assert_eq!(count(&connection, "vm_effects"), records);
                assert_eq!(plan(&connection), before);
                assert_eq!(
                    vm.pending_continuations(),
                    std::slice::from_ref(&current.continuation)
                );
                drop(vm);
                let mut vm = Vm::open_journal(&path.0, 1).unwrap();
                connection
                    .execute_batch("DROP TRIGGER reject_transition")
                    .unwrap();
                let step = vm.resume(
                    &current.continuation,
                    focus(if finishing { "b" } else { "a" }),
                );
                if finishing {
                    assert!(matches!(step, Step::Done(_)));
                } else {
                    assert!(matches!(step, Step::Effect(_)));
                }
                drop(vm);
                Vm::open_journal(&path.0, 1).unwrap();
            }
        }
    }
}

#[test]
fn concurrent_final_prefix_receipts_admit_one_tail_and_first_tail_commit_wins() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let prefix = batch(start(&mut vm, CAPTURE));
    drop(vm);
    let barrier = Arc::new(Barrier::new(2));
    let workers = (0..2)
        .map(|index| {
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            let request = prefix[index].clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                vm.resume(
                    &request.continuation,
                    focus(if index == 0 { "a" } else { "b" }),
                )
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    let tail = results
        .iter()
        .find_map(|step| {
            if let Step::Effect(request) = step {
                Some(request.as_ref().clone())
            } else {
                None
            }
        })
        .unwrap();
    for step in &results {
        match step {
            Step::Effect(request) => assert_eq!(request.as_ref(), &tail),
            Step::Waiting(wait) => {
                assert_eq!(wait.completed_branches, 1);
                assert_eq!(wait.total_branches, 2);
            }
            other => panic!("unexpected concurrent prefix outcome: {other:?}"),
        }
    }
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 3);
    let barrier = Arc::new(Barrier::new(2));
    let workers = ["b", "wrong"]
        .into_iter()
        .map(|node| {
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            let request = tail.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                vm.resume(&request.continuation, focus(node))
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results[0], results[1]);
    assert!(matches!(results[0], Step::Done(_) | Step::Fault(_)));
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume(&prefix[0].continuation, focus("ignored")),
        results[0]
    );
    assert!(vm.claim_effect(1, 100).unwrap().is_none());
}

fn inventory(size: usize) -> EffectResult {
    let mut control = leserpent_domain::InMemoryControlPlane::default();
    let runtime = control.register_runtime(
        RuntimeId::new("seed").unwrap(),
        "runtime",
        "http://runtime.local",
    );
    EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
        revision: Revision(7),
        runtimes: (0..size)
            .map(|index| {
                let mut runtime = runtime.clone();
                runtime.id = RuntimeId::new(format!("runtime-{index}")).unwrap();
                runtime
            })
            .collect(),
    })
}

fn large_inventory() -> EffectResult {
    let EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
        revision,
        mut runtimes,
    }) = inventory(1)
    else {
        panic!()
    };
    runtimes[0].name = "x".repeat(5 * 1024 * 1024);
    EffectResult::Query(leserpent_domain::QueryResult::RuntimeList { revision, runtimes })
}

#[test]
fn cumulative_raw_output_overflow_is_durable_before_tail_admission_or_scalar_projection() {
    for captured in [false, true] {
        for at_tail in [false, true] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 500).unwrap();
            let body = if captured {
                "bind(r: runtime.list(), body: 0)"
            } else {
                "runtime.list()"
            };
            let prefix = batch(start(
                &mut vm,
                &format!(
                    "fn main() = bind(g: all(a: runtime.list(), b: runtime.list()), body: {body})"
                ),
            ));
            assert!(matches!(
                vm.resume(&prefix[1].continuation, inventory(5000)),
                Step::Waiting(_)
            ));
            let mut terminal = vm.resume(
                &prefix[0].continuation,
                inventory(if at_tail { 0 } else { 5001 }),
            );
            if at_tail {
                terminal = vm.resume(&effect(terminal).continuation, inventory(5001));
            }
            assert!(
                matches!(&terminal, Step::Fault(fault) if fault.code == "LSV2404"),
                "{terminal:?}"
            );
            let connection = Connection::open(&path.0).unwrap();
            assert_eq!(
                count(&connection, "vm_effects"),
                if at_tail { 3 } else { 2 }
            );
            let pending: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM vm_effects WHERE state = 'pending'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(pending, 0);
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            assert_eq!(vm.resume(&prefix[0].continuation, inventory(0)), terminal);
            assert!(vm.claim_effect(1, 100).unwrap().is_none());
        }
    }
}

#[test]
fn merged_byte_limit_is_a_durable_fault_not_a_permanent_last_receipt_retry() {
    for at_tail in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let prefix = batch(start(
            &mut vm,
            "fn main() = bind(g: all(a: runtime.list(), b: runtime.list()), body: bind(r: runtime.list(), body: 0))",
        ));
        assert!(matches!(
            vm.resume(&prefix[1].continuation, large_inventory()),
            Step::Waiting(_)
        ));
        let mut terminal = vm.resume(
            &prefix[0].continuation,
            if at_tail {
                inventory(0)
            } else {
                large_inventory()
            },
        );
        if at_tail {
            terminal = vm.resume(&effect(terminal).continuation, large_inventory());
        }
        assert!(
            matches!(&terminal, Step::Fault(fault) if fault.code == "LSV3002"),
            "{terminal:?}"
        );
        assert_eq!(vm.pending_count(), 0);
        let pending: i64 = Connection::open(&path.0)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM vm_effects WHERE state = 'pending'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pending, 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&prefix[0].continuation, inventory(0)), terminal);
    }
}

#[test]
fn legacy_owned_group_byte_faults_commit_without_changing_old_schema_markers() {
    for (kind, body, schema) in [
        ("all", "0", 6),
        ("seq", "0", 6),
        ("seq", "runtime.list()", 7),
        ("seq", "bind(r: runtime.list(), body: 0)", 8),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let source = format!(
            "fn main() = bind(g: {kind}(a: runtime.list(), b: runtime.list()), body: {body})"
        );
        let (first, terminal) = match start(&mut vm, &source) {
            Step::Effect(first) => {
                let second = effect(vm.resume(&first.continuation, large_inventory()));
                let terminal = vm.resume(&second.continuation, large_inventory());
                (*first, terminal)
            }
            Step::Effects(batch) => {
                let first = batch.branches[0].request.clone();
                assert!(matches!(
                    vm.resume(&batch.branches[1].request.continuation, large_inventory()),
                    Step::Waiting(_)
                ));
                let terminal = vm.resume(&first.continuation, large_inventory());
                (first, terminal)
            }
            other => panic!("unexpected group start: {other:?}"),
        };
        assert_eq!(first.continuation.schema_version, schema);
        assert!(
            matches!(&terminal, Step::Fault(fault) if fault.code == "LSV3002"),
            "{terminal:?}"
        );
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 2);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&first.continuation, inventory(0)), terminal);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
}

#[test]
fn legacy_schema_eight_capture_byte_faults_preserve_the_raw_successor_receipt() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(
        &mut vm,
        "fn main() = bind(g: seq(a: runtime.list()), body: bind(r: runtime.list(), body: 0))",
    ));
    let tail = effect(vm.resume(&first.continuation, large_inventory()));
    assert_eq!(tail.continuation.schema_version, 8);
    let terminal = vm.resume(&tail.continuation, large_inventory());
    assert!(
        matches!(&terminal, Step::Fault(fault) if fault.code == "LSV3002"),
        "{terminal:?}"
    );
    assert_eq!(vm.pending_count(), 0);
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    let raw: Vec<u8> = connection
        .query_row(
            "SELECT terminal_step FROM vm_effects WHERE token = ?1",
            [tail.continuation.token.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(matches!(
        serde_json::from_slice::<Step>(&raw).unwrap(),
        Step::Done(Value::RuntimeList { .. })
    ));
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&tail.continuation, inventory(0)), terminal);
}

#[test]
fn pure_parallel_groups_commit_cumulative_item_faults_before_running_the_body() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let prefix = batch(start(
        &mut vm,
        "fn main() = bind(g: all(a: runtime.list(), b: runtime.list()), body: 0)",
    ));
    assert!(
        prefix
            .iter()
            .all(|request| request.continuation.schema_version == 6)
    );
    assert!(matches!(
        vm.resume(&prefix[1].continuation, inventory(5000)),
        Step::Waiting(_)
    ));
    let terminal = vm.resume(&prefix[0].continuation, inventory(5001));
    assert!(
        matches!(&terminal, Step::Fault(fault) if fault.code == "LSV2404"),
        "{terminal:?}"
    );
    assert_eq!(vm.pending_count(), 0);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&prefix[0].continuation, inventory(0)), terminal);
    assert!(vm.claim_effect(1, 100).unwrap().is_none());
}

#[test]
fn unbound_parallel_journal_groups_also_commit_item_and_byte_faults() {
    for bytes in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let prefix = batch(start(
            &mut vm,
            "fn main() = all(a: runtime.list(), b: runtime.list())",
        ));
        assert!(
            prefix
                .iter()
                .all(|request| request.continuation.schema_version == 1)
        );
        vm.resume(
            &prefix[1].continuation,
            if bytes {
                large_inventory()
            } else {
                inventory(5000)
            },
        );
        let terminal = vm.resume(
            &prefix[0].continuation,
            if bytes {
                large_inventory()
            } else {
                inventory(5001)
            },
        );
        assert!(
            matches!(&terminal, Step::Fault(fault) if fault.code == if bytes { "LSV3002" } else { "LSV2404" }),
            "{terminal:?}"
        );
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&prefix[0].continuation, inventory(0)), terminal);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
}

#[test]
fn capture_frames_preserve_boolean_member_aliases_and_reject_prefix_or_schema_corruption() {
    let source = r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.assert_selection(node_id: "toggle", state: "selected")), body: bind(alias: g, body: bind(state: member(value: alias, name: "b"), body: bind(r: ui.focus(node_id: "a"), body: and(left: field(value: state, name: "selected"), right: field(value: member(value: alias, name: "b"), name: "selected"))))))"#;
    for mutation in 0..8 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let prefix = batch(start(&mut vm, source));
        vm.resume(
            &prefix[1].continuation,
            EffectResult::Presentation(PresentationResult::AssertSelection {
                node_id: "toggle".into(),
                state: UiSelectionState::Selected,
            }),
        );
        let tail = effect(vm.resume(&prefix[0].continuation, focus("a")));
        let groups = &tail.continuation.result_binding.as_ref().unwrap().groups;
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[1].group.members[1].result.projection_version, 2);
        if mutation == 0 {
            assert_eq!(
                vm.resume(&tail.continuation, focus("a")),
                scalar(ScalarValue::Boolean(true))
            );
            continue;
        }
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        if mutation == 7 {
            let forged = serde_json::to_vec(&Step::Fault(leselang_vm::Fault {
                code: "host.failure".into(),
                message: "forged".into(),
            }))
            .unwrap();
            connection
                .execute(
                    "UPDATE vm_effects SET terminal_step = ?1 WHERE token = ?2",
                    params![forged, prefix[0].continuation.token.as_str()],
                )
                .unwrap();
        } else {
            let mut request = serde_json::to_value(&tail).unwrap();
            match mutation {
                1 => request["continuation"]["schema_version"] = 8.into(),
                2 => {
                    request["continuation"]["result_binding"]["groups"][0]["group"]["members"][0]
                        ["result"]["fields"][0]["value"]["value"] = "changed".into()
                }
                3 => {
                    request["continuation"]["result_binding"]["groups"][1]["group"]["members"][1]
                        ["result"]["projection_version"] = 99.into()
                }
                4 => request["continuation"]["group_result"] = serde_json::Value::Null,
                5 => {
                    request["continuation"]["fuel_remaining"] = 1000.into();
                    request["budget"]["fuel_remaining"] = 1000.into();
                }
                _ => {
                    request["continuation"]["result_binding"]["results"][0]["result"]["fields"][1]
                        ["value"]["value"] = false.into()
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
        assert!(Vm::open_journal(&path.0, 1).is_err(), "mutation {mutation}");
    }
}

#[test]
fn oversized_parallel_projection_fails_before_admitting_a_reserved_tail() {
    let members = (0..63)
        .map(|index| format!(r#"m{index}: ui.focus(node_id: "a")"#))
        .collect::<Vec<_>>()
        .join(", ");
    let mut body = r#"bind(r: ui.focus(node_id: "b"), body: 7)"#.to_string();
    for index in 0..10 {
        body = format!("bind(alias{index}: g, body: {body})");
    }
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100000).unwrap();
    let prefix = batch(start(
        &mut vm,
        &format!("fn main() = bind(g: all({members}), body: {body})"),
    ));
    let mut terminal = None;
    for (index, request) in prefix.iter().rev().enumerate() {
        let step = vm.resume(&request.continuation, focus("a"));
        if index < 62 {
            assert!(matches!(step, Step::Waiting(_)));
        } else {
            terminal = Some(step);
        }
    }
    let terminal = terminal.unwrap();
    assert!(
        matches!(&terminal, Step::Fault(fault) if fault.code == "LSV3002"),
        "{terminal:?}"
    );
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 63);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume(&prefix[0].continuation, focus("ignored")),
        terminal
    );
}

#[test]
fn preparation_and_final_calculation_faults_replay_without_extra_dispatches() {
    for (body, code, finishing) in [
        (
            r#"ui.focus(node_id: concat(left: "bad", right: " node"))"#,
            "LSV1404",
            false,
        ),
        (
            r#"bind(r: ui.focus(node_id: "b"), body: div(left: 1, right: 0))"#,
            "LSV1401",
            true,
        ),
        (
            r#"bind(r: ui.focus(node_id: "b"), body: parse_integer(value: "bad"))"#,
            "LSV1408",
            true,
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let prefix = batch(start(
            &mut vm,
            &format!(
                r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: {body})"#
            ),
        ));
        vm.resume(&prefix[1].continuation, focus("b"));
        let mut terminal = vm.resume(&prefix[0].continuation, focus("a"));
        if finishing {
            terminal = vm.resume(&effect(terminal).continuation, focus("b"));
        }
        assert!(
            matches!(&terminal, Step::Fault(fault) if fault.code == code),
            "{terminal:?}"
        );
        assert_eq!(
            count(&Connection::open(&path.0).unwrap(), "vm_effects"),
            if finishing { 3 } else { 2 }
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume(&prefix[0].continuation, focus("ignored")),
            terminal
        );
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
}

#[test]
fn conditional_capture_selects_only_one_atomic_type_under_the_original_deadline() {
    for choose_ui in [false, true] {
        for expired in [false, true] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 500).unwrap();
            let source = format!(
                r#"fn main() = bind(g: all(a: runtime.list(), b: runtime.list()), body: choose(when: {choose_ui}, then: bind(r: ui.focus(node_id: "b"), body: 1), otherwise: bind(r: runtime.list(), body: field(value: r, name: "count"))))"#
            );
            let prefix = batch(vm.start_timed(
                &lower(&parse(&source)).unwrap(),
                Principal::new("operator").unwrap(),
                CapabilitySet::new(["runtime.read", "ui.presentation"]),
                Some(Revision(7)),
                100,
                100,
            ));
            vm.resume_at(&prefix[1].continuation, 101, inventory(0));
            let tail = effect(vm.resume_at(&prefix[0].continuation, 199, inventory(0)));
            assert_eq!(tail.continuation.deadline_at_ms, Some(200));
            assert_eq!(
                tail.continuation.deadline_ms,
                prefix[0].continuation.deadline_ms
            );
            let result = if choose_ui { focus("b") } else { inventory(2) };
            let terminal =
                vm.resume_at(&tail.continuation, if expired { 200 } else { 199 }, result);
            if expired {
                assert!(matches!(terminal, Step::Cancelled(_)), "{terminal:?}");
            } else {
                assert_eq!(
                    terminal,
                    scalar(ScalarValue::Integer(if choose_ui { 1 } else { 2 }))
                );
            }
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            assert_eq!(
                vm.resume_at(&prefix[0].continuation, 201, inventory(0)),
                terminal
            );
        }
    }
}

#[test]
fn shared_parallel_budget_is_reserved_once_and_never_refilled_by_reopen() {
    for reopening_fuel in [1, 1000] {
        let mut reached_success = false;
        for fuel in 1..100 {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
            let mut step = start(&mut vm, CAPTURE);
            if let Step::Effects(_) = step {
                let prefix = batch(step);
                assert_eq!(
                    prefix[0].budget.fuel_remaining,
                    prefix[1].budget.fuel_remaining
                );
                vm.resume(&prefix[1].continuation, focus("b"));
                drop(vm);
                vm = Vm::open_journal(&path.0, reopening_fuel).unwrap();
                step = vm.resume(&prefix[0].continuation, focus("a"));
                if let Step::Effect(tail) = step {
                    assert!(tail.budget.fuel_remaining < prefix[0].budget.fuel_remaining);
                    drop(vm);
                    vm = Vm::open_journal(&path.0, reopening_fuel).unwrap();
                    step = vm.resume(&tail.continuation, focus("b"));
                }
            }
            if matches!(step, Step::Done(_)) {
                assert_eq!(step, scalar(ScalarValue::Boolean(false)));
                reached_success = true;
                break;
            }
            assert!(
                matches!(&step, Step::Fault(fault) if fault.code == "LSV1001"),
                "fuel {fuel}: {step:?}"
            );
        }
        assert!(reached_success);
    }
}

#[test]
fn uncaptured_tail_recovery_rejects_a_corrupted_unsuccessful_prefix() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let (prefix, _) = prepare(&mut vm, TAIL);
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    let fault = serde_json::to_vec(&Step::Fault(leselang_vm::Fault {
        code: "host.failure".into(),
        message: "forged".into(),
    }))
    .unwrap();
    connection
        .execute(
            "UPDATE vm_effects SET terminal_step = ?1 WHERE token = ?2",
            params![fault, prefix[0].continuation.token.as_str()],
        )
        .unwrap();
    assert!(Vm::open_journal(&path.0, 1).is_err());
}

#[test]
fn mutating_parallel_tail_keeps_confirmation_and_requires_a_correlated_lease() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let source = r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: bind(r: debugger.cancel(session_id: field(value: member(value: g, name: "a"), name: "node_id")), body: true))"#;
    let (_, tail) = prepare(&mut vm, source);
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
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    assert_eq!(lease.request, tail);
    assert_eq!(
        vm.acknowledge_effect(&lease, 2, result),
        scalar(ScalarValue::Boolean(true))
    );
    Vm::open_journal(&path.0, 1).unwrap();
}

#[test]
fn parallel_group_images_require_owned_recovery_and_old_group_bytes_remain_unchanged() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let (prefix, tail) = prepare(&mut vm, CAPTURE);
    for request in prefix.iter().chain(std::iter::once(&tail)) {
        assert_eq!(
            decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap(),
            request.continuation
        );
        assert_eq!(
            Vm::default()
                .restore(request.continuation.clone())
                .unwrap_err()
                .code,
            "LSV1409"
        );
        assert_eq!(
            Vm::default()
                .restore_request(request.clone())
                .unwrap_err()
                .code,
            "LSV1409"
        );
    }
    let plan: MergePlan =
        serde_json::from_value(plan(&Connection::open(&path.0).unwrap())).unwrap();
    let values = ["a", "b", "b"];
    let completions = plan
        .branches
        .iter()
        .zip(values)
        .map(|(name, node)| BranchCompletion {
            branch: name.clone(),
            outcome: BranchOutcome::Value(Value::UiFocus {
                node_id: node.into(),
            }),
        })
        .collect();
    assert_eq!(
        merge_declared(&plan, completions, 10000).unwrap_err().code,
        "LSV1409"
    );
    let mut old = Vm::new(500);
    let pure = batch(start(
        &mut old,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: true)"#,
    ));
    assert!(
        pure.iter()
            .all(|request| request.continuation.schema_version == 6)
    );
    assert!(
        pure.iter()
            .all(|request| request.continuation.result_binding.is_none())
    );
    old.resume(&pure[1].continuation, focus("b"));
    assert_eq!(
        old.resume(&pure[0].continuation, focus("a")),
        scalar(ScalarValue::Boolean(true))
    );
}

#[test]
fn live_parallel_prefix_and_tail_are_one_protected_retention_unit() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let prefix = batch(start(&mut vm, CAPTURE));
    let policy = RetentionPolicy {
        max_completed_records: 1,
        max_delete_per_run: 10,
    };
    vm.resume(&prefix[1].continuation, focus("b"));
    assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 0);
    let tail = effect(vm.resume(&prefix[0].continuation, focus("a")));
    assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 0);
    vm.resume(&tail.continuation, focus("b"));
    let later = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    vm.resume(&later.continuation, focus("a"));
    assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 1);
    assert_eq!(
        count(&Connection::open(&path.0).unwrap(), "vm_merge_groups"),
        0
    );
    assert_eq!(vm.completed_count(), 1);
    Vm::open_journal(&path.0, 1).unwrap();
}
