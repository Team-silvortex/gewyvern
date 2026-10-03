use leselang_hir::{Effect, lower};
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
            "leselang-host-functions-{}-{}",
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
fn fault(step: Step, code: &str) {
    assert!(
        matches!(step, Step::Fault(ref fault) if fault.code == code),
        "{step:?}"
    );
}
fn flow(skip: bool) -> String {
    format!(
        r#"fn host_ready(node: string, skip: boolean) = choose(when: skip, then: false, otherwise:
        bind(result: ui.assert_text(node_id: node, expected: "ready"), body: starts_with(left: field(value: result, name: "expected"), right: "ready")))
        fn main() = bind(ok: host_ready(node: "status", skip: {skip}), body:
            bind(written: ui.set_form_value(node_id: "form", field: "ready", value: to_string(value: ok)), body: ok))"#
    )
}
fn rows(path: &JournalPath) -> i64 {
    Connection::open(&path.0)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn effectful_function_results_return_to_the_caller_after_correlated_receipts() {
    for skip in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut vm, &flow(skip)));
        let current = if skip {
            first.clone()
        } else {
            assert!(
                matches!(&first.operation, EffectOperation::Presentation(envelope) if matches!(envelope.operation, PresentationOperation::AssertText { .. }))
            );
            effect(vm.resume_at(&first.continuation, 101, receipt(&first)))
        };
        assert!(
            matches!(&current.operation, EffectOperation::Presentation(envelope) if matches!(&envelope.operation, PresentationOperation::SetFormValue { value, .. } if value == if skip { "false" } else { "true" }))
        );
        assert_eq!(
            vm.resume_at(&current.continuation, 102, receipt(&current)),
            done(ScalarValue::Boolean(!skip))
        );
        assert_eq!(rows(&path), if skip { 1 } else { 2 });
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(
            vm.resume_at(&first.continuation, 103, receipt(&first)),
            done(ScalarValue::Boolean(!skip))
        );
    }
}

#[test]
fn durable_function_reentry_uses_expanded_hir_without_source_or_call_frames() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(&mut vm, &flow(false)));
    let wire = encode_continuation(&first.continuation).unwrap();
    assert!(!String::from_utf8_lossy(&wire).contains("host_ready"));
    assert_eq!(decode_continuation(&wire).unwrap(), first.continuation);
    assert!(first.continuation.schema_version <= 11);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    assert_ne!(first.effect_id, second.effect_id);
    let (
        EffectOperation::Presentation(first_envelope),
        EffectOperation::Presentation(second_envelope),
    ) = (&first.operation, &second.operation)
    else {
        panic!()
    };
    assert_eq!(first_envelope.principal, second_envelope.principal);
    assert_eq!(first_envelope.capabilities, second_envelope.capabilities);
    assert_eq!(second.continuation.expected_revision, Some(Revision(7)));
    assert_eq!(second.continuation.deadline_ms, 100);
    assert_eq!(second.continuation.deadline_at_ms, Some(200));
    assert!(second.continuation.fuel_remaining < first.continuation.fuel_remaining);
    assert_eq!(
        vm.resume_at(&first.continuation, 102, receipt(&first)),
        Step::Effect(Box::new(second.clone()))
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&second.continuation, 103, receipt(&second)),
        done(ScalarValue::Boolean(true))
    );
    assert_eq!(
        vm.resume_at(&first.continuation, 104, receipt(&first)),
        done(ScalarValue::Boolean(true))
    );
}

#[test]
fn multiple_function_calls_keep_caller_and_definition_locals_separate() {
    let source = r#"fn aggregate(node: string) = bind(result: ui.assert_text(node_id: node, expected: "2,3"), body:
        fold(total: 0, items: split(left: field(value: result, name: "expected"), right: ","), item: "part", next: add(left: total, right: parse_integer(value: part)), limit: 2))
        fn main() = bind(_lf0: "outer", body: bind(result: 7, body: bind(total: 8, body: bind(part: "outer", body:
            bind(first: aggregate(node: "a"), body: bind(second: aggregate(node: "b"), body: add(left: add(left: first, right: second), right: add(left: result, right: total))))))))"#;
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(&mut vm, source));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    assert!(
        matches!(&second.continuation.pending_effect, Effect::UiAssertText { node_id, .. } if node_id == "b")
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&second.continuation, 102, receipt(&second)),
        done(ScalarValue::Integer(25))
    );
    assert_eq!(
        vm.resume_at(&first.continuation, 103, receipt(&first)),
        done(ScalarValue::Integer(25))
    );
    let source = r#"fn identity(n: integer) = n
        fn work(n: integer) = bind(r: ui.focus(node_id: "a"), body: n)
        fn main() = work(n: fold(total: 0, items: strings(a: "x"), item: "_lf0", next: identity(n: total), limit: 1))"#;
    let mut vm = Vm::new(10000);
    let request = effect(start(&mut vm, source));
    assert_eq!(
        vm.resume_at(&request.continuation, 101, receipt(&request)),
        done(ScalarValue::Integer(0))
    );
}

#[test]
fn functions_return_atomic_receipts_and_nested_function_calls_in_normal_positions() {
    let source = r#"fn focus(node: string) = ui.focus(node_id: node)
        fn visible(node: string) = bind(r: focus(node: node), body: ui.assert_visible(node_id: field(value: r, name: "node_id")))
        fn main() = bind(receipt: visible(node: "a"), body: field(value: receipt, name: "node_id"))"#;
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, source));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    assert!(
        matches!(&second.operation, EffectOperation::Presentation(envelope) if matches!(&envelope.operation, PresentationOperation::AssertVisible { node_id } if node_id == "a"))
    );
    assert_eq!(
        vm.resume_at(&second.continuation, 102, receipt(&second)),
        done(ScalarValue::String("a".into()))
    );
}

#[test]
fn function_owned_groups_preserve_named_members_and_the_all_success_barrier() {
    for kind in ["seq", "all"] {
        let source = format!(
            r#"fn gathered(node: string) = bind(group: {kind}(first: ui.assert_text(node_id: node, expected: "x"), second: ui.assert_text(node_id: "b", expected: "y")), body: concat(left: field(value: member(value: group, name: "first"), name: "expected"), right: field(value: member(value: group, name: "second"), name: "expected")))
            fn main() = bind(value: gathered(node: "a"), body: bind(written: ui.set_form_value(node_id: "form", field: "value", value: value), body: value))"#
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
                assert_eq!(kind, "all");
                assert!(matches!(
                    vm.resume_at(
                        &batch.branches[1].request.continuation,
                        101,
                        receipt(&batch.branches[1].request)
                    ),
                    Step::Waiting(_)
                ));
                assert_eq!(rows(&path), 2);
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
            matches!(&tail.operation, EffectOperation::Presentation(envelope) if matches!(&envelope.operation, PresentationOperation::SetFormValue { value, .. } if value == "xy"))
        );
        assert_eq!(
            vm.resume_at(&tail.continuation, 103, receipt(&tail)),
            done(ScalarValue::String("xy".into()))
        );
    }
    let source = r#"fn rows(node: string) = seq(first: ui.focus(node_id: node), second: ui.assert_visible(node_id: node))
        fn main() = bind(group: rows(node: "a"), body: field(value: member(value: group, name: "second"), name: "node_id"))"#;
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, source));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    assert_eq!(
        vm.resume_at(&second.continuation, 102, receipt(&second)),
        done(ScalarValue::String("a".into()))
    );
}

#[test]
fn effectful_function_arguments_evaluate_once_in_signature_order_before_admission() {
    let source = r#"fn work(n: integer, unused: integer) = bind(r: ui.focus(node_id: "a"), body: add(left: n, right: n))
        fn main() = work(unused: parse_integer(value: "private"), n: div(left: 1, right: 0))"#;
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let step = start(&mut vm, source);
    assert!(!format!("{step:?}").contains("private"));
    fault(step, "LSV1401");
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(rows(&path), 0);
    assert_eq!(
        effect(start(&mut vm, "fn main() = ui.focus(node_id: \"a\")")).effect_id,
        "effect-1"
    );
    let source = r#"fn work(n: integer) = bind(r: ui.focus(node_id: "a"), body: add(left: n, right: n))
        fn main() = work(n: add(left: 2, right: 3))"#;
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, source));
    assert_eq!(
        vm.resume_at(&first.continuation, 101, receipt(&first)),
        done(ScalarValue::Integer(10))
    );
}

#[test]
fn lazy_unused_calls_allocate_no_identity_but_cold_calls_retain_capability_checks() {
    let source = r#"fn work(n: integer) = bind(r: ui.focus(node_id: "a"), body: n)
        fn main() = choose(when: true, then: 0, otherwise: work(n: div(left: 1, right: 0)))"#;
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let program = lower(&parse(source)).unwrap();
    fault(
        vm.start(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::default(),
            None,
        ),
        "LSH2001",
    );
    assert_eq!(start(&mut vm, source), done(ScalarValue::Integer(0)));
    assert_eq!(rows(&path), 0);
    assert_eq!(vm.pending_count(), 0);
    let unused = r#"fn work() = ui.focus(node_id: "a")
        fn main() = 0"#;
    assert_eq!(start(&mut vm, unused), done(ScalarValue::Integer(0)));
}

#[test]
fn original_host_domains_and_parameter_faults_still_prevent_partial_admission() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    for source in [
        r#"fn focus(node: string) = ui.focus(node_id: node)
            fn main() = focus(node: "bad node")"#,
        r#"fn write(text: string) = ui.set_form_value(node_id: "form", field: "value", value: text)
            fn main() = write(text: "a\nb")"#,
    ] {
        fault(start(&mut vm, source), "LSV1404");
    }
    assert_eq!(rows(&path), 0);
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(
        effect(start(&mut vm, "fn main() = ui.focus(node_id: \"a\")")).effect_id,
        "effect-1"
    );
}

#[test]
fn reentry_never_refills_function_fuel_or_bypasses_deadlines_and_cancellation() {
    let first = effect(start(&mut Vm::new(10000), &flow(false)));
    let mut exhausted = first.clone();
    exhausted.continuation.fuel_remaining = 1;
    exhausted.budget.fuel_remaining = 1;
    let mut vm = Vm::new(10000);
    vm.restore_request(exhausted.clone()).unwrap();
    fault(
        vm.resume_at(&exhausted.continuation, 101, receipt(&first)),
        "LSV1001",
    );
    for cancel in [false, true] {
        let mut vm = Vm::new(10000);
        vm.restore_request(first.clone()).unwrap();
        let terminal = if cancel {
            vm.cancel_effect(&first.continuation, 101)
        } else {
            vm.resume_at(&first.continuation, 200, receipt(&first))
        };
        assert!(matches!(terminal, Step::Cancelled(_)));
        assert_eq!(
            vm.resume_at(&first.continuation, 201, receipt(&first)),
            terminal
        );
    }
}

#[test]
fn host_errors_and_mismatched_receipts_never_execute_the_function_return_continuation() {
    for wrong in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut vm, &flow(false)));
        let terminal = if wrong {
            vm.resume_at(
                &first.continuation,
                101,
                EffectResult::Presentation(PresentationResult::AssertText {
                    node_id: "status".into(),
                    expected: "other".into(),
                }),
            )
        } else {
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
        };
        assert!(matches!(terminal, Step::Fault(_) | Step::Failed(_)));
        assert_eq!(rows(&path), 1);
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 102, receipt(&first)),
            terminal
        );
    }
}

#[test]
fn sql_transition_failures_roll_back_function_returns_and_caller_admission() {
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
            let first = effect(start(&mut vm, &flow(false)));
            let current = if finishing {
                effect(vm.resume_at(&first.continuation, 101, receipt(&first)))
            } else {
                first
            };
            let connection = Connection::open(&path.0).unwrap();
            let count = rows(&path);
            connection.execute_batch(&format!("CREATE TRIGGER reject_transition BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
            assert!(matches!(
                vm.resume_at(&current.continuation, 102, receipt(&current)),
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
            let mut step = vm.resume_at(&current.continuation, 103, receipt(&current));
            if !finishing {
                let next = effect(step);
                step = vm.resume_at(&next.continuation, 104, receipt(&next));
            }
            assert_eq!(step, done(ScalarValue::Boolean(true)));
        }
    }
}

#[test]
fn function_expansion_keeps_legacy_wire_and_rejects_fake_call_frames() {
    let first = effect(start(&mut Vm::new(10000), &flow(false)));
    let mut wire = serde_json::to_value(&first.continuation).unwrap();
    wire["result_binding"]["body"]["function"] = serde_json::json!("host_ready");
    assert!(decode_continuation(&serde_json::to_vec(&wire).unwrap()).is_err());
    let plain = lower(&parse(r#"fn main() = ui.focus(node_id: "a")"#)).unwrap();
    let unused = lower(&parse(
        r#"fn extra() = runtime.list()
        fn main() = ui.focus(node_id: "a")"#,
    ))
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&unused).unwrap(),
        serde_json::to_vec(&plain).unwrap()
    );
}
