use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{Effect, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation,
};
use rusqlite::Connection;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-prepared-group-{}-{}",
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
    vm.start_timed(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        100,
        100,
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
fn rows(path: &JournalPath, table: &str) -> i64 {
    Connection::open(&path.0)
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}
fn assert_empty(vm: &Vm, path: &JournalPath) {
    assert_eq!(vm.pending_count(), 0);
    for table in [
        "vm_effects",
        "vm_dispatches",
        "vm_merge_groups",
        "vm_merge_branches",
    ] {
        assert_eq!(rows(path, table), 0, "{table}");
    }
}
fn node(request: &EffectRequest) -> &str {
    let Effect::UiFocus { node_id } = &request.continuation.pending_effect else {
        panic!()
    };
    node_id
}

#[test]
fn sequence_helpers_are_fully_prepared_before_admission_and_restart_uses_only_resolved_effects() {
    let source = r#"fn select_node(node: string, alternate: boolean) = choose(when: alternate, then: ui.focus(node_id: "alternate"), otherwise: ui.focus(node_id: concat(left: "node-", right: node)))
        fn main() = seq(first: select_node(node: "a", alternate: false), again: repeat(times: 2, body: select_node(node: "b", alternate: true)))"#;
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(&mut vm, source));
    assert_eq!(node(&first), "node-a");
    assert_eq!(rows(&path, "vm_effects"), 3);
    let wire = encode_continuation(&first.continuation).unwrap();
    assert_eq!(decode_continuation(&wire).unwrap(), first.continuation);
    let wire = String::from_utf8(wire).unwrap();
    assert!(!wire.contains("select_node") && !wire.contains("choose") && !wire.contains("concat"));
    assert_eq!(first.continuation.expected_revision, Some(Revision(7)));
    assert_eq!(first.continuation.deadline_at_ms, Some(200));
    assert!(first.budget.fuel_remaining < 10000);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    assert_eq!(node(&second), "alternate");
    assert_eq!(
        second.budget.fuel_remaining,
        first.budget.fuel_remaining - 1
    );
    assert!(matches!(
        vm.resume_at(&second.continuation, 102, receipt(&second)),
        Step::Effect(_)
    ));
    let third = effect(vm.resume_at(&second.continuation, 103, receipt(&second)));
    assert_eq!(node(&third), "alternate");
    let terminal = vm.resume_at(&third.continuation, 104, receipt(&third));
    assert!(
        matches!(&terminal, Step::Done(Value::Structured { fields }) if fields.iter().map(|field| field.name.as_str()).collect::<Vec<_>>() == ["first", "again__iteration_1", "again__iteration_2"])
    );
    assert_eq!(
        vm.resume_at(&first.continuation, 105, receipt(&first)),
        terminal
    );
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn helper_member_receipts_preserve_named_group_projections_and_parallel_success_barriers() {
    for kind in ["seq", "all"] {
        let source = format!(
            r#"fn check(node: string) = ui.assert_text(node_id: node, expected: concat(left: "re", right: "ady"))
            fn main() = bind(group: {kind}(first: check(node: "a"), second: check(node: "b")), body:
                bind(written: ui.set_form_value(node_id: "form", field: "ready", value: concat(left: field(value: member(value: group, name: "first"), name: "expected"), right: field(value: member(value: group, name: "second"), name: "expected"))), body:
                    eq(left: field(value: written, name: "value"), right: "readyready")))"#
        );
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let tail = match start(&mut vm, &source) {
            Step::Effect(first) => {
                let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
                drop(vm);
                vm = Vm::open_journal(&path.0, 1).unwrap();
                effect(vm.resume_at(&second.continuation, 102, receipt(&second)))
            }
            Step::Effects(batch) => {
                assert!(matches!(
                    vm.resume_at(
                        &batch.branches[1].request.continuation,
                        101,
                        receipt(&batch.branches[1].request)
                    ),
                    Step::Waiting(_)
                ));
                assert_eq!(rows(&path, "vm_effects"), 2);
                drop(vm);
                vm = Vm::open_journal(&path.0, 1).unwrap();
                effect(vm.resume_at(
                    &batch.branches[0].request.continuation,
                    102,
                    receipt(&batch.branches[0].request),
                ))
            }
            other => panic!("{other:?}"),
        };
        assert!(
            matches!(&tail.continuation.pending_effect, Effect::UiSetFormValue { value, .. } if value == "readyready")
        );
        let terminal = Step::Done(Value::Scalar {
            value: ScalarValue::Boolean(true),
        });
        assert_eq!(
            vm.resume_at(&tail.continuation, 103, receipt(&tail)),
            terminal
        );
        assert_eq!(
            vm.resume_at(&tail.continuation, 104, receipt(&tail)),
            terminal
        );
    }
}

#[test]
fn later_helper_preparation_faults_leave_no_group_rows_or_used_identities() {
    for kind in ["seq", "all"] {
        for (body, argument, fuel, code) in [
            ("ui.focus(node_id: node)", "\"bad node\"", 10000, "LSV1404"),
            (
                "bind(unused: div(left: 1, right: 0), body: ui.focus(node_id: node))",
                "\"a\"",
                10000,
                "LSV1401",
            ),
            ("ui.focus(node_id: node)", "\"a\"", 4, "LSV1001"),
        ] {
            let source = format!(
                "fn work(node: string) = {body}\nfn main() = {kind}(first: ui.focus(node_id: \"a\"), second: work(node: {argument}))"
            );
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
            let step = start(&mut vm, &source);
            assert!(
                matches!(&step, Step::Fault(fault) if fault.code == code),
                "{step:?}"
            );
            assert_empty(&vm, &path);
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            assert_eq!(
                effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#)).effect_id,
                "effect-1"
            );
        }
    }
}

#[test]
fn signature_arguments_are_eager_once_in_declaration_order_even_when_unused() {
    let source = r#"fn work(n: integer, unused: integer) = ui.focus(node_id: to_string(value: n))
        fn main() = all(first: ui.focus(node_id: "a"), second: work(unused: parse_integer(value: "private"), n: div(left: 1, right: 0)))"#;
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let step = start(&mut vm, source);
    assert!(matches!(&step, Step::Fault(fault) if fault.code == "LSV1401"));
    assert!(!format!("{step:?}").contains("private"));
    assert_empty(&vm, &path);
    let source = r#"fn work(n: integer, unused: integer) = ui.focus(node_id: to_string(value: n))
        fn main() = seq(first: ui.focus(node_id: "a"), second: work(n: 1, unused: div(left: 1, right: 0)))"#;
    assert!(matches!(start(&mut vm, source), Step::Fault(_)));
    assert_empty(&vm, &path);
}

#[test]
fn helper_locals_remain_isolated_from_other_members_and_caller_locals() {
    let source = r#"fn focus(node: string) = bind(label: concat(left: node, right: "-inner"), body: ui.focus(node_id: label))
        fn main() = bind(_lf0: "outer", body: bind(label: "outer", body: seq(first: focus(node: label), second: ui.focus(node_id: label), third: focus(node: label))))"#;
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, source));
    assert_eq!(node(&first), "outer-inner");
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    assert_eq!(node(&second), "outer");
    let third = effect(vm.resume_at(&second.continuation, 102, receipt(&second)));
    assert_eq!(node(&third), "outer-inner");
    assert!(matches!(
        vm.resume_at(&third.continuation, 103, receipt(&third)),
        Step::Done(_)
    ));
}

#[test]
fn lazy_pure_selection_skips_cold_faults_but_never_skips_authority_checks() {
    let source = r#"fn focus() = choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: to_string(value: div(left: 1, right: 0))))
        fn read() = runtime.inspect(runtime_id: "a")
        fn main() = choose(when: true, then: seq(first: focus()), otherwise: seq(second: read()))"#;
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    assert!(matches!(start(&mut vm, source), Step::Fault(ref fault) if fault.code == "LSH2001"));
    assert_empty(&vm, &path);
    let first = effect(vm.start_timed(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "runtime.read"]),
        Some(Revision(7)),
        100,
        100,
    ));
    assert_eq!(node(&first), "a");
    assert_eq!(rows(&path, "vm_effects"), 1);
    assert!(matches!(
        vm.resume_at(&first.continuation, 101, receipt(&first)),
        Step::Done(_)
    ));
}

#[test]
fn failed_group_journal_admission_rolls_back_execution_rows_without_reusing_reserved_identities() {
    for table in [
        "vm_effects",
        "vm_dispatches",
        "vm_merge_groups",
        "vm_merge_branches",
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let connection = Connection::open(&path.0).unwrap();
        connection.execute_batch(&format!("CREATE TRIGGER reject_admission BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
        let source = r#"fn focus(node: string) = ui.focus(node_id: concat(left: node, right: "-prepared"))
            fn main() = all(first: focus(node: "a"), second: focus(node: "b"))"#;
        assert!(matches!(start(&mut vm, source), Step::Fault(_)));
        assert_empty(&vm, &path);
        // Admission rolls back execution rows, not the durable identity reservation fence.
        let next: i64 = connection
            .query_row(
                "SELECT next_sequence FROM vm_metadata WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next, 4);
        connection
            .execute_batch("DROP TRIGGER reject_admission")
            .unwrap();
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let Step::Effects(batch) = start(&mut vm, source) else {
            panic!()
        };
        assert_eq!(batch.branches[0].request.effect_id, "effect-5");
        assert_eq!(rows(&path, "vm_effects"), 2);
    }
}

#[test]
fn prepared_member_groups_cannot_refill_fuel_or_escape_deadline_and_cancel_fences() {
    let source = r#"fn focus(n: integer) = ui.focus(node_id: to_string(value: n))
        fn main() = repeat(times: 3, body: focus(n: add(left: 2, right: 3)))"#;
    let first = effect(start(&mut Vm::new(10000), source));
    let shorter = effect(start(
        &mut Vm::new(10000),
        &source.replace("times: 3", "times: 2"),
    ));
    assert!(shorter.budget.fuel_remaining - first.budget.fuel_remaining > 1);
    for cancelled in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut vm, source));
        let terminal = if cancelled {
            vm.cancel_effect(&first.continuation, 101)
        } else {
            vm.resume_at(&first.continuation, 200, receipt(&first))
        };
        assert!(matches!(&terminal, Step::Cancelled(_)));
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 201, receipt(&first)),
            terminal
        );
    }
}

#[test]
fn legacy_literal_group_wire_is_unchanged_and_forged_prepared_results_fail_before_admission() {
    let source =
        r#"fn main() = seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#;
    let plain = effect(start(&mut Vm::new(10000), source));
    let unused = format!("fn unused(node: string) = ui.focus(node_id: node)\n{source}");
    let unused = effect(start(&mut Vm::new(10000), &unused));
    assert_eq!(
        encode_continuation(&plain.continuation).unwrap(),
        encode_continuation(&unused.continuation).unwrap()
    );
    let mut program = lower(&parse(r#"fn main() = seq(first: bind(node: "a", body: ui.focus(node_id: node)), second: ui.focus(node_id: "b"))"#)).unwrap();
    let Effect::Compute { expression } = &mut program.function.effect else {
        panic!()
    };
    let leselang_hir::computation::Computation::Group { branches, .. } = expression.as_mut() else {
        panic!()
    };
    branches[0].result_type = leselang_hir::Type::UiAssertVisible;
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    assert!(
        matches!(vm.start(&program, Principal::new("operator").unwrap(), CapabilitySet::new(["ui.presentation"]), None), Step::Fault(ref fault) if fault.code == "LSH1405")
    );
    assert_empty(&vm, &path);
}
