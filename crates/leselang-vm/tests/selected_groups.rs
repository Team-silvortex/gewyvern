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
            "leselang-selected-group-{}-{}",
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
fn source(kind: &str, helper: bool, alternate: bool) -> String {
    let choice = format!(
        r#"choose(when: alternate,
        then: {kind}(first: ui.focus(node_id: "a1"), second: ui.assert_text(node_id: "a2", expected: "a")),
        otherwise: {kind}(first: ui.focus(node_id: "b1"), second: ui.assert_text(node_id: "b2", expected: concat(left: "", right: "b"))))"#
    );
    let body = r#"bind(written: ui.set_form_value(node_id: "form", field: "selected", value: concat(left: field(value: member(value: group, name: "first"), name: "node_id"), right: field(value: member(value: group, name: "second"), name: "expected"))), body: field(value: written, name: "value"))"#;
    if helper {
        format!(
            "fn rows(alternate: boolean) = {choice}\nfn main() = bind(group: rows(alternate: {alternate}), body: {body})"
        )
    } else {
        format!(
            "fn main() = bind(alternate: {alternate}, body: bind(group: {choice}, body: {body}))"
        )
    }
}

#[test]
fn selected_sequences_and_parallel_helpers_recover_only_the_resolved_branch() {
    for kind in ["seq", "all"] {
        for helper in [false, true] {
            for alternate in [false, true] {
                let path = JournalPath::new();
                let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
                let initial = start(&mut vm, &source(kind, helper, alternate));
                let requests = match initial {
                    Step::Effect(first) => vec![*first],
                    Step::Effects(batch) => batch
                        .branches
                        .into_iter()
                        .map(|branch| branch.request)
                        .collect(),
                    other => panic!("{other:?}"),
                };
                assert_eq!(rows(&path, "vm_effects"), 2);
                for request in &requests {
                    let wire = encode_continuation(&request.continuation).unwrap();
                    assert_eq!(decode_continuation(&wire).unwrap(), request.continuation);
                    let text = String::from_utf8(wire).unwrap();
                    assert!(
                        !text.contains("choose")
                            && !text.contains("concat")
                            && !text.contains("rows(")
                    );
                    let cold_node = if alternate { "b1" } else { "a1" };
                    assert!(!text.contains(cold_node));
                    assert_eq!(request.continuation.expected_revision, Some(Revision(7)));
                    assert_eq!(request.continuation.deadline_at_ms, Some(200));
                }
                let first = requests[0].clone();
                let tail = if kind == "seq" {
                    drop(vm);
                    vm = Vm::open_journal(&path.0, 1).unwrap();
                    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
                    assert_eq!(
                        second.budget.fuel_remaining,
                        first.budget.fuel_remaining - 1
                    );
                    drop(vm);
                    vm = Vm::open_journal(&path.0, 1).unwrap();
                    effect(vm.resume_at(&second.continuation, 102, receipt(&second)))
                } else {
                    let second = &requests[1];
                    assert!(matches!(
                        vm.resume_at(&second.continuation, 101, receipt(second)),
                        Step::Waiting(_)
                    ));
                    assert_eq!(rows(&path, "vm_effects"), 2);
                    drop(vm);
                    vm = Vm::open_journal(&path.0, 1).unwrap();
                    effect(vm.resume_at(&first.continuation, 102, receipt(&first)))
                };
                let expected = if alternate { "a1a" } else { "b1b" };
                assert!(
                    matches!(&tail.continuation.pending_effect, Effect::UiSetFormValue { value, .. } if value == expected)
                );
                assert!(tail.budget.fuel_remaining < first.budget.fuel_remaining);
                let terminal = Step::Done(Value::Scalar {
                    value: ScalarValue::String(expected.into()),
                });
                assert_eq!(
                    vm.resume_at(&tail.continuation, 103, receipt(&tail)),
                    terminal
                );
                assert_eq!(
                    vm.resume_at(&first.continuation, 104, receipt(&first)),
                    terminal
                );
                assert_eq!(vm.pending_count(), 0);
                drop(vm);
                let mut vm = Vm::open_journal(&path.0, 1).unwrap();
                assert_eq!(
                    vm.resume_at(&tail.continuation, 105, receipt(&tail)),
                    terminal
                );
            }
        }
    }
}

#[test]
fn selection_between_helper_calls_does_not_evaluate_cold_arguments() {
    let source = r#"fn rows(node: string, unused: integer) = seq(first: ui.focus(node_id: concat(left: "node-", right: node)), second: ui.assert_text(node_id: node, expected: "ready"))
        fn main() = bind(group: choose(when: true, then: rows(node: "a", unused: 1), otherwise: rows(node: "b", unused: div(left: 1, right: 0))), body: field(value: member(value: group, name: "first"), name: "node_id"))"#;
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, source));
    assert!(
        matches!(&first.continuation.pending_effect, Effect::UiFocus { node_id } if node_id == "node-a")
    );
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    assert_eq!(
        vm.resume_at(&second.continuation, 102, receipt(&second)),
        Step::Done(Value::Scalar {
            value: ScalarValue::String("node-a".into())
        })
    );
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    assert!(
        matches!(start(&mut vm, &source.replace("when: true", "when: false")), Step::Fault(ref fault) if fault.code == "LSV1401")
    );
    assert_eq!(rows(&path, "vm_effects"), 0);
}

#[test]
fn unselected_group_preparation_is_lazy_but_selected_faults_allocate_nothing() {
    for kind in ["seq", "all"] {
        let source = format!(
            r#"fn rows(alternate: boolean) = choose(when: alternate,
            then: {kind}(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b")),
            otherwise: {kind}(first: ui.focus(node_id: "cold"), second: ui.focus(node_id: to_string(value: div(left: 1, right: 0)))))
            fn main() = bind(group: rows(alternate: true), body: field(value: member(value: group, name: "first"), name: "node_id"))"#
        );
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        assert!(matches!(
            start(&mut vm, &source),
            Step::Effect(_) | Step::Effects(_)
        ));
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        assert!(
            matches!(start(&mut vm, &source.replace("rows(alternate: true)", "rows(alternate: false)")), Step::Fault(ref fault) if fault.code == "LSV1401")
        );
        assert_eq!(vm.pending_count(), 0);
        for table in [
            "vm_effects",
            "vm_dispatches",
            "vm_merge_groups",
            "vm_merge_branches",
        ] {
            assert_eq!(rows(&path, table), 0);
        }
        assert_eq!(
            effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#)).effect_id,
            "effect-1"
        );
    }
}

#[test]
fn selection_faults_and_missing_authority_fail_before_journal_admission() {
    for (fuel, guard, capabilities, code) in [
        (
            10000,
            "eq(left: div(left: 1, right: 0), right: 0)",
            vec!["ui.presentation"],
            "LSV1401",
        ),
        (1, "true", vec!["ui.presentation"], "LSV1001"),
        (10000, "true", vec![], "LSH2001"),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
        let source = format!(
            r#"fn main() = bind(group: choose(when: {guard}, then: seq(first: ui.focus(node_id: "a")), otherwise: seq(first: ui.focus(node_id: "b"))), body: true)"#
        );
        let step = vm.start(
            &lower(&parse(&source)).unwrap(),
            Principal::new("operator").unwrap(),
            CapabilitySet::new(capabilities),
            None,
        );
        assert!(
            matches!(&step, Step::Fault(fault) if fault.code == code),
            "{step:?}"
        );
        assert_eq!(rows(&path, "vm_effects"), 0);
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn selected_group_to_successor_transition_rolls_back_and_retries_without_reselection() {
    for kind in ["seq", "all"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let (first, last) = match start(&mut vm, &source(kind, true, false)) {
            Step::Effect(first) => {
                let last = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
                (*first, last)
            }
            Step::Effects(batch) => {
                let first = batch.branches[0].request.clone();
                assert!(matches!(
                    vm.resume_at(&first.continuation, 101, receipt(&first)),
                    Step::Waiting(_)
                ));
                (first, batch.branches[1].request.clone())
            }
            other => panic!("{other:?}"),
        };
        let connection = Connection::open(&path.0).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_tail BEFORE INSERT ON vm_effects BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
        assert!(matches!(
            vm.resume_at(&last.continuation, 102, receipt(&last)),
            Step::Fault(_)
        ));
        assert_eq!(rows(&path, "vm_effects"), 2);
        assert_eq!(vm.pending_count(), 1);
        connection
            .execute_batch("DROP TRIGGER reject_tail")
            .unwrap();
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        let tail = effect(vm.resume_at(&last.continuation, 103, receipt(&last)));
        assert!(
            matches!(&tail.continuation.pending_effect, Effect::UiSetFormValue { value, .. } if value == "b1b")
        );
        let terminal = vm.resume_at(&tail.continuation, 104, receipt(&tail));
        assert!(
            matches!(&terminal, Step::Done(Value::Scalar { value: ScalarValue::String(value) }) if value == "b1b")
        );
        assert_eq!(
            vm.resume_at(&first.continuation, 105, receipt(&first)),
            terminal
        );
    }
}

#[test]
fn selected_groups_retain_cancel_deadline_and_wrong_receipt_fences() {
    for mode in ["cancel", "deadline", "wrong-receipt"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut vm, &source("seq", false, true)));
        let terminal = match mode {
            "cancel" => vm.cancel_effect(&first.continuation, 101),
            "deadline" => vm.resume_at(&first.continuation, 200, receipt(&first)),
            _ => vm.resume_at(
                &first.continuation,
                101,
                EffectResult::Presentation(leselang_vm::PresentationResult::Focus {
                    node_id: "wrong".into(),
                }),
            ),
        };
        assert!(matches!(&terminal, Step::Cancelled(_) | Step::Fault(_)));
        assert_eq!(rows(&path, "vm_effects"), 2);
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 201, receipt(&first)),
            terminal
        );
    }
}
