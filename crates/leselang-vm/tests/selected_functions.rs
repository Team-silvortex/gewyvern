use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectError, EffectErrorClass, EffectOperation, EffectRequest, EffectResult,
    PresentationOperation, PresentationResult, RetryDisposition, RetryPolicy, ScalarValue, Step,
    Value, Vm, decode_continuation, encode_continuation,
};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-selected-functions-{}-{}",
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
fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}
fn rows(path: &JournalPath) -> i64 {
    Connection::open(&path.0)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row.get(0))
        .unwrap()
}
fn flow(single: bool, kind: &str) -> String {
    format!(
        r#"fn single() = bind(result: ui.assert_text(node_id: "single-only", expected: "ready"), body: field(value: result, name: "expected"))
        fn gathered() = bind(group: {kind}(first: ui.assert_text(node_id: "grouped-left", expected: "re"), second: ui.assert_text(node_id: "grouped-right", expected: "ady")), body: concat(left: field(value: member(value: group, name: "first"), name: "expected"), right: field(value: member(value: group, name: "second"), name: "expected")))
        fn main() = bind(answer: choose(when: {single}, then: single(), otherwise: gathered()), body:
            bind(written: ui.set_form_value(node_id: "form", field: "answer", value: answer), body: eq(left: field(value: written, name: "value"), right: "ready")))"#
    )
}

#[test]
fn selected_single_and_group_returns_survive_every_receipt_restart_and_replay() {
    for kind in ["seq", "all"] {
        for single in [false, true] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            let mut step = start(&mut vm, &flow(single, kind));
            let mut first = None;
            let mut previous_fuel = 10000;
            let mut time = 101;
            loop {
                let mut requests = match &step {
                    Step::Effect(request) => vec![request.as_ref().clone()],
                    Step::Effects(batch) => batch
                        .branches
                        .iter()
                        .map(|branch| branch.request.clone())
                        .collect(),
                    Step::Done(_) => break,
                    other => panic!("{other:?}"),
                };
                requests.reverse();
                for request in requests {
                    first.get_or_insert_with(|| request.clone());
                    let wire = encode_continuation(&request.continuation).unwrap();
                    assert_eq!(decode_continuation(&wire).unwrap(), request.continuation);
                    let text = String::from_utf8_lossy(&wire);
                    assert!(!text.contains("gathered") && !text.contains("choose"));
                    assert!(!text.contains(if single {
                        "grouped-left"
                    } else {
                        "single-only"
                    }));
                    assert!(request.continuation.schema_version <= 11);
                    assert_eq!(request.continuation.expected_revision, Some(Revision(7)));
                    assert_eq!(request.continuation.deadline_at_ms, Some(200));
                    assert!(request.continuation.fuel_remaining <= previous_fuel);
                    previous_fuel = request.continuation.fuel_remaining;
                    let EffectOperation::Presentation(envelope) = &request.operation else {
                        panic!()
                    };
                    assert_eq!(envelope.principal, Principal::new("operator").unwrap());
                    assert_eq!(
                        envelope.capabilities,
                        CapabilitySet::new(["ui.presentation"])
                    );
                    if let PresentationOperation::SetFormValue { value, .. } = &envelope.operation {
                        assert_eq!(value, "ready");
                        assert_eq!(rows(&path), if single { 2 } else { 3 });
                    }
                    drop(vm);
                    vm = Vm::open_journal(&path.0, 1).unwrap();
                    step = vm.resume_at(&request.continuation, time, receipt(&request));
                    assert_eq!(
                        vm.resume_at(&request.continuation, time, receipt(&request)),
                        step
                    );
                    time += 1;
                }
            }
            assert_eq!(step, done(ScalarValue::Boolean(true)));
            assert_eq!(vm.pending_count(), 0);
            assert_eq!(rows(&path), if single { 2 } else { 3 });
            let first = first.unwrap();
            assert_eq!(
                vm.resume_at(&first.continuation, time, receipt(&first)),
                step
            );
        }
    }
}

#[test]
fn selected_arguments_are_lazy_between_branches_but_eager_in_signature_order() {
    for selected in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let source = format!(
            r#"fn work(n: integer, unused: integer) = bind(result: ui.focus(node_id: "a"), body: n)
            fn main() = bind(answer: choose(when: {selected}, then: work(unused: parse_integer(value: "private"), n: div(left: 1, right: 0)), otherwise: 0), body: answer)"#
        );
        let program = lower(&parse(&source)).unwrap();
        assert!(
            matches!(vm.start(&program, Principal::new("operator").unwrap(), CapabilitySet::default(), None), Step::Fault(fault) if fault.code == "LSH2001")
        );
        let step = start(&mut vm, &source);
        if selected {
            assert!(!format!("{step:?}").contains("private"));
            assert!(matches!(step, Step::Fault(fault) if fault.code == "LSV1401"));
        } else {
            assert_eq!(step, done(ScalarValue::Integer(0)));
        }
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(rows(&path), 0);
        let unused_fault = source.replace("div(left: 1, right: 0)", "7");
        let step = start(&mut vm, &unused_fault);
        if selected {
            assert!(matches!(step, Step::Fault(fault) if fault.code == "LSV1408"));
        } else {
            assert_eq!(step, done(ScalarValue::Integer(0)));
        }
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(rows(&path), 0);
        assert_eq!(
            effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#)).effect_id,
            "effect-1"
        );
    }
    for selected in [false, true] {
        let source = format!(
            r#"fn work() = bind(scratch: "inner", body: bind(result: ui.focus(node_id: "a"), body: scratch))
            fn main() = bind(scratch: "outer", body: bind(answer: choose(when: {selected}, then: work(), otherwise: bind(scratch2: "fallback", body: scratch2)), body: bind(scratch2: "caller", body: concat(left: scratch, right: concat(left: answer, right: scratch2)))))"#
        );
        let mut vm = Vm::new(10000);
        let step = start(&mut vm, &source);
        let terminal = if selected {
            let request = effect(step);
            vm.resume_at(&request.continuation, 101, receipt(&request))
        } else {
            step
        };
        assert_eq!(
            terminal,
            done(ScalarValue::String(format!(
                "outer{}caller",
                if selected { "inner" } else { "fallback" }
            )))
        );
    }
}

#[test]
fn selected_bounded_lists_and_optional_empty_values_keep_their_data_types() {
    for selected in [false, true] {
        for (body, fallback, use_value, expected) in [
            (
                r#"split(left: field(value: result, name: "expected"), right: ",")"#,
                r#"strings(first: "fallback")"#,
                r#"join(left: answer, right: "|")"#,
                if selected { "x|y" } else { "fallback" },
            ),
            (
                r#"optional_string(value: "")"#,
                "optional_string(value: none)",
                r#"value_or(left: answer, right: "fallback")"#,
                if selected { "" } else { "fallback" },
            ),
        ] {
            let source = format!(
                r#"fn work() = bind(result: ui.assert_text(node_id: "a", expected: "x,y"), body: {body})
                fn main() = bind(answer: choose(otherwise: {fallback}, when: {selected}, then: work()), body: {use_value})"#
            );
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            let step = start(&mut vm, &source);
            let terminal = if selected {
                let request = effect(step);
                drop(vm);
                vm = Vm::open_journal(&path.0, 1).unwrap();
                vm.resume_at(&request.continuation, 101, receipt(&request))
            } else {
                step
            };
            assert_eq!(terminal, done(ScalarValue::String(expected.into())));
            assert_eq!(rows(&path), i64::from(selected));
        }
    }
}

#[test]
fn selected_return_transactions_roll_back_receipts_and_caller_admission_together() {
    for single in [false, true] {
        for finishing in [false, true] {
            let writes: &[(&str, &str)] = if finishing {
                &[("vm_effects", "UPDATE"), ("vm_dispatches", "UPDATE")]
            } else {
                &[
                    ("vm_effects", "UPDATE"),
                    ("vm_dispatches", "UPDATE"),
                    ("vm_effects", "INSERT"),
                    ("vm_dispatches", "INSERT"),
                ]
            };
            for (table, operation) in writes {
                let path = JournalPath::new();
                let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
                let first = effect(start(&mut vm, &flow(single, "seq")));
                let returning = if single {
                    first
                } else {
                    effect(vm.resume_at(&first.continuation, 101, receipt(&first)))
                };
                let current = if finishing {
                    effect(vm.resume_at(&returning.continuation, 102, receipt(&returning)))
                } else {
                    returning
                };
                let connection = Connection::open(&path.0).unwrap();
                let count = rows(&path);
                connection.execute_batch(&format!("CREATE TRIGGER reject_transition BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
                assert!(matches!(
                    vm.resume_at(&current.continuation, 103, receipt(&current)),
                    Step::Fault(_)
                ));
                assert_eq!(rows(&path), count);
                assert_eq!(
                    connection
                        .query_row(
                            "SELECT state FROM vm_effects WHERE token = ?1",
                            [current.continuation.token.as_str()],
                            |row| row.get::<_, String>(0)
                        )
                        .unwrap(),
                    "pending"
                );
                drop(vm);
                connection
                    .execute_batch("DROP TRIGGER reject_transition")
                    .unwrap();
                let mut vm = Vm::open_journal(&path.0, 1).unwrap();
                let mut step = vm.resume_at(&current.continuation, 104, receipt(&current));
                if !finishing {
                    let next = effect(step);
                    step = vm.resume_at(&next.continuation, 105, receipt(&next));
                }
                assert_eq!(step, done(ScalarValue::Boolean(true)));
                assert_eq!(
                    vm.resume_at(&current.continuation, 106, receipt(&current)),
                    step
                );
            }
        }
    }
}

#[test]
fn selected_functions_do_not_join_after_faults_cancellation_deadlines_or_exhausted_fuel() {
    let source = flow(true, "seq");
    let first = effect(start(&mut Vm::new(10000), &source));
    let mut exhausted = first.clone();
    exhausted.continuation.fuel_remaining = 1;
    exhausted.budget.fuel_remaining = 1;
    let mut vm = Vm::new(10000);
    vm.restore_request(exhausted.clone()).unwrap();
    assert!(
        matches!(vm.resume_at(&exhausted.continuation, 101, receipt(&first)), Step::Fault(fault) if fault.code == "LSV1001")
    );
    for mode in 0..4 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut vm, &source));
        let terminal = match mode {
            0 => vm.cancel_effect(&first.continuation, 101),
            1 => vm.resume_at(&first.continuation, 200, receipt(&first)),
            2 => vm.resume_at(
                &first.continuation,
                101,
                EffectResult::Presentation(PresentationResult::AssertText {
                    node_id: "single-only".into(),
                    expected: "wrong".into(),
                }),
            ),
            _ => {
                let lease = vm.claim_effect(101, 50).unwrap().unwrap();
                let RetryDisposition::Terminal(terminal) = vm
                    .report_effect_error(
                        &lease,
                        102,
                        EffectError {
                            class: EffectErrorClass::Permanent,
                            code: "host_unavailable".into(),
                            message: "host failure".into(),
                        },
                        &RetryPolicy::default(),
                    )
                    .unwrap()
                else {
                    panic!()
                };
                terminal
            }
        };
        assert!(matches!(
            terminal,
            Step::Cancelled(_) | Step::Fault(_) | Step::Failed(_)
        ));
        assert_eq!(rows(&path), 1);
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
fn simultaneous_workers_commit_one_selected_function_return_successor() {
    for _ in 0..3 {
        let path = JournalPath::new();
        let mut left = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut left, &flow(true, "seq")));
        let mut right = Vm::open_journal(&path.0, 1).unwrap();
        let barrier = std::sync::Barrier::new(2);
        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                barrier.wait();
                left.resume_at(&first.continuation, 101, receipt(&first))
            });
            let b = scope.spawn(|| {
                barrier.wait();
                right.resume_at(&first.continuation, 101, receipt(&first))
            });
            (a.join().unwrap(), b.join().unwrap())
        });
        assert_eq!(a, b);
        let caller = effect(a);
        assert_eq!(rows(&path), 2);
        assert_eq!(
            left.resume_at(&caller.continuation, 102, receipt(&caller)),
            done(ScalarValue::Boolean(true))
        );
        assert_eq!(
            right.resume_at(&caller.continuation, 103, receipt(&caller)),
            done(ScalarValue::Boolean(true))
        );
        assert_eq!(rows(&path), 2);
    }
}

#[test]
fn existing_prepared_captures_and_selected_group_wires_remain_unchanged() {
    for source in [
        r#"fn main() = bind(result: choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b")), body: field(value: result, name: "node_id"))"#,
        r#"fn main() = bind(group: choose(when: true, then: seq(first: ui.focus(node_id: "a")), otherwise: seq(first: ui.focus(node_id: "b"))), body: field(value: member(value: group, name: "first"), name: "node_id"))"#,
    ] {
        let extra = format!(
            "fn unused() = bind(result: runtime.list(), body: field(value: result, name: \"count\"))\n{source}"
        );
        assert_eq!(
            lower(&parse(source)).unwrap(),
            lower(&parse(&extra)).unwrap()
        );
        assert_eq!(
            start(&mut Vm::new(10000), source),
            start(&mut Vm::new(10000), &extra)
        );
    }
    let first = effect(start(&mut Vm::new(10000), &flow(true, "seq")));
    let mut wire = serde_json::to_value(&first.continuation).unwrap();
    wire["call_frame"] = serde_json::json!({ "function": "single" });
    assert!(decode_continuation(&serde_json::to_vec(&wire).unwrap()).is_err());
}
