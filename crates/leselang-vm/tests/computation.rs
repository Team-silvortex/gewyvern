use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::computation::ScalarType;
use leselang_hir::{Effect, Type, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectError, EffectErrorClass, EffectOperation, EffectRequest, EffectResult,
    PresentationResult, RetryDisposition, RetryPolicy, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation,
};

fn start(vm: &mut Vm, expression: &str) -> Step {
    vm.start(
        &lower(&parse(&format!("fn main() = {expression}"))).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    )
}

fn done(expression: &str, expected: ScalarValue) {
    let mut vm = Vm::default();
    assert_eq!(
        start(&mut vm, expression),
        Step::Done(Value::Scalar { value: expected })
    );
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 0);
}

fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("expected effect, got {other:?}"),
    }
}

fn result(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
}

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-computation-{}-{}",
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

#[test]
fn arithmetic_comparison_boolean_string_and_binding_results_are_typed() {
    use ScalarValue::*;
    for (expression, expected) in [
        ("add(left: 2, right: 3)", Integer(5)),
        ("sub(left: 7, right: 3)", Integer(4)),
        ("mul(left: 7, right: 3)", Integer(21)),
        ("div(left: 7, right: 3)", Integer(2)),
        ("rem(left: 7, right: 3)", Integer(1)),
        ("lt(left: 1, right: 2)", Boolean(true)),
        ("le(left: 2, right: 2)", Boolean(true)),
        ("gt(left: 1, right: 2)", Boolean(false)),
        ("ge(left: 2, right: 2)", Boolean(true)),
        ("eq(left: none, right: none)", Boolean(true)),
        ("ne(left: \"a\", right: \"b\")", Boolean(true)),
        ("and(left: true, right: false)", Boolean(false)),
        ("or(left: false, right: true)", Boolean(true)),
        ("not(value: false)", Boolean(true)),
        (
            "concat(left: \"hello\", right: \" world\")",
            String("hello world".into()),
        ),
        ("len(value: \"\u{754c}\u{9762}\u{1f642}\")", Integer(3)),
        (
            "bind(n: add(left: 2, right: 3), body: bind(m: mul(left: n, right: 2), body: choose(when: eq(left: m, right: 10), then: m, otherwise: 0)))",
            Integer(10),
        ),
        ("choose(when: false, then: 1, otherwise: 2)", Integer(2)),
        ("none", None),
    ] {
        done(expression, expected);
    }
}

#[test]
fn unchosen_branches_and_short_circuited_operands_do_not_execute() {
    done(
        "choose(when: true, then: 7, otherwise: div(left: 1, right: 0))",
        ScalarValue::Integer(7),
    );
    done(
        "and(left: false, right: eq(left: div(left: 1, right: 0), right: 0))",
        ScalarValue::Boolean(false),
    );
    done(
        "or(left: true, right: eq(left: div(left: 1, right: 0), right: 0))",
        ScalarValue::Boolean(true),
    );
    assert_eq!(
        start(
            &mut Vm::new(3),
            "choose(when: true, then: 7, otherwise: div(left: 1, right: 0))"
        ),
        Step::Done(Value::Scalar {
            value: ScalarValue::Integer(7)
        })
    );
    assert!(
        matches!(start(&mut Vm::new(2), "choose(when: true, then: 7, otherwise: 0)"), Step::Fault(fault) if fault.code == "LSV1001")
    );
}

#[test]
fn arithmetic_failures_leave_no_pending_work_or_consumed_effect_identity() {
    for expression in [
        "add(left: 18446744073709551615, right: 1)",
        "sub(left: 0, right: 1)",
        "mul(left: 18446744073709551615, right: 2)",
        "div(left: 1, right: 0)",
        "rem(left: 1, right: 0)",
    ] {
        let mut vm = Vm::default();
        assert!(
            matches!(start(&mut vm, expression), Step::Fault(fault) if fault.code == "LSV1401")
        );
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(
            effect(start(&mut vm, "ui.focus(node_id: \"a\")")).effect_id,
            "effect-1"
        );
    }
}

#[test]
fn pure_programs_need_no_host_capability_and_do_not_change_later_start_budgets() {
    let mut vm = Vm::new(20);
    let program = lower(&parse(
        "fn main() = bind(n: 2, body: add(left: n, right: 3))",
    ))
    .unwrap();
    assert_eq!(
        vm.start(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::default(),
            None
        ),
        Step::Done(Value::Scalar {
            value: ScalarValue::Integer(5)
        })
    );
    assert_eq!(
        effect(start(&mut vm, "ui.focus(node_id: \"a\")"))
            .budget
            .fuel_remaining,
        19
    );
}

#[test]
fn string_work_and_output_size_are_bounded() {
    let large = "x".repeat(4096);
    done(
        &format!("len(value: \"{large}\")"),
        ScalarValue::Integer(4096),
    );
    assert!(
        matches!(start(&mut Vm::default(), &format!("concat(left: \"{large}\", right: \"x\")")), Step::Fault(fault) if fault.code == "LSV1403")
    );
    assert!(
        matches!(start(&mut Vm::new(1), "\"hello\""), Step::Fault(fault) if fault.code == "LSV1001")
    );
    done(
        &format!(
            "concat(left: \"{}\", right: \"{}\")",
            "a".repeat(2048),
            "b".repeat(2048)
        ),
        ScalarValue::String(format!("{}{}", "a".repeat(2048), "b".repeat(2048))),
    );
}

#[test]
fn both_branch_authority_and_forged_result_metadata_are_checked_before_selection() {
    let mut program = lower(&parse(r#"fn main() = choose(when: true, then: seq(read: runtime.list()), otherwise: seq(mutate: runtime.refresh(runtime_id: "a")))"#)).unwrap();
    let mut vm = Vm::default();
    assert!(
        matches!(vm.start(&program, Principal::new("operator").unwrap(), CapabilitySet::new(["runtime.read"]), None), Step::Fault(fault) if fault.code == "LSH2001")
    );
    assert_eq!(vm.pending_count(), 0);
    program = lower(&parse("fn main() = 42")).unwrap();
    program.function.result_type = Type::Scalar(ScalarType::Boolean);
    assert!(
        matches!(vm.start(&program, Principal::new("operator").unwrap(), CapabilitySet::default(), None), Step::Fault(fault) if fault.code == "LSH1405")
    );
}

#[test]
fn selected_sequence_persists_only_chosen_effects_and_remaining_budget() {
    let path = JournalPath::new();
    let source = r#"bind(ready: true, body: choose(when: ready,
        then: seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b")),
        otherwise: seq(unselected: ui.focus(node_id: "never"))))"#;
    let mut vm = Vm::open_journal(&path.0, 20).unwrap();
    let first = effect(start(&mut vm, source));
    assert_eq!(first.budget.fuel_remaining, 14);
    assert_eq!(vm.pending_count(), 2);
    assert!(vm.pending_continuations().iter().all(
        |image| !matches!(&image.pending_effect, Effect::UiFocus { node_id } if node_id == "never")
    ));
    assert_eq!(
        decode_continuation(&encode_continuation(&first.continuation).unwrap()).unwrap(),
        first.continuation
    );
    let lease = vm.claim_effect(1, 10).unwrap().unwrap();
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 999).unwrap();
    let second = effect(vm.acknowledge_effect(&lease, 2, result("a")));
    assert_eq!(second.budget.fuel_remaining, 13);
    assert!(
        matches!(&second.continuation.pending_effect, Effect::UiFocus { node_id } if node_id == "b")
    );
    let finished = vm.resume(&second.continuation, result("b"));
    assert!(matches!(&finished, Step::Done(Value::Structured { fields }) if fields.len() == 2));
    assert_eq!(vm.pending_count(), 0);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 999).unwrap();
    assert_eq!(vm.resume(&first.continuation, result("a")), finished);
    assert!(vm.claim_effect(3, 10).unwrap().is_none());
}

#[test]
fn computation_cannot_reset_host_fuel_deadline_or_cancel_fences() {
    let expression =
        r#"choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b"))"#;
    let mut vm = Vm::new(3);
    assert!(matches!(start(&mut vm, expression), Step::Fault(fault) if fault.code == "LSV1001"));
    assert_eq!(vm.pending_count(), 0);
    let program = lower(&parse(&format!("fn main() = {expression}"))).unwrap();
    let mut vm = Vm::new(4);
    let request = effect(vm.start_timed(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
        100,
        50,
    ));
    assert_eq!(request.budget.fuel_remaining, 0);
    assert_eq!(request.budget.deadline_at_ms, Some(150));
    let cancelled = vm.cancel_effect(&request.continuation, 101);
    assert!(matches!(cancelled, Step::Cancelled(_)));
    assert_eq!(
        vm.resume_at(&request.continuation, 102, result("a")),
        cancelled
    );
    let mut forged = request.continuation;
    forged.pending_effect = program.function.effect;
    assert!(encode_continuation(&forged).is_err());
}

#[test]
fn computational_frontend_can_select_existing_parallel_groups_without_new_wire() {
    let mut vm = Vm::new(12);
    let step = start(
        &mut vm,
        r#"choose(when: false,
        then: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")),
        otherwise: all(c: ui.focus(node_id: "c"), d: ui.focus(node_id: "d")))"#,
    );
    let Step::Effects(batch) = step else {
        panic!("expected parallel host batch")
    };
    assert_eq!(
        batch
            .branches
            .iter()
            .map(|branch| branch.branch.as_str())
            .collect::<Vec<_>>(),
        ["c", "d"]
    );
    assert!(
        batch
            .branches
            .iter()
            .all(|branch| branch.request.budget.fuel_remaining == 8)
    );
}

#[test]
fn computation_wire_rejects_unknown_fields_and_preserves_scalar_results() {
    use leselang_hir::computation::Computation;
    let mut wire = serde_json::to_value(Computation::Literal {
        value: ScalarValue::Boolean(true),
    })
    .unwrap();
    wire["extra"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Computation>(wire).is_err());
    for value in [
        ScalarValue::Integer(u64::MAX),
        ScalarValue::Boolean(false),
        ScalarValue::String("text".into()),
        ScalarValue::None,
    ] {
        let step = Step::Done(Value::Scalar { value });
        assert_eq!(
            serde_json::from_slice::<Step>(&serde_json::to_vec(&step).unwrap()).unwrap(),
            step
        );
    }
}

#[test]
fn computed_arguments_resolve_to_the_same_typed_operation_as_literals() {
    for (computed, literal) in [
        (
            r#"bind(node: concat(left: "runtime-", right: "a"), body: ui.focus(node_id: node))"#,
            r#"ui.focus(node_id: "runtime-a")"#,
        ),
        (
            r#"ui.navigate_focus(node_id: concat(left: "runtime-", right: "a"), direction: choose(when: false, then: "last", otherwise: "next"))"#,
            r#"ui.navigate_focus(node_id: "runtime-a", direction: "next")"#,
        ),
        (
            r#"bind(empty: none, body: ui.wait_form_field_placeholder(node_id: "a", field: "target", expected: empty))"#,
            r#"ui.wait_form_field_placeholder(node_id: "a", field: "target", expected: none)"#,
        ),
        (
            r#"ui.assert_child_count(node_id: "a", count: concat(left: "1", right: "2"))"#,
            r#"ui.assert_child_count(node_id: "a", count: "12")"#,
        ),
        (
            r#"ui.focus(node_id: choose(when: true, then: "a", otherwise: bind(n: div(left: 1, right: 0), body: "b")))"#,
            r#"ui.focus(node_id: "a")"#,
        ),
    ] {
        let mut vm = Vm::default();
        let resolved = effect(start(&mut vm, computed));
        let static_request = effect(start(&mut Vm::default(), literal));
        assert_eq!(resolved.operation, static_request.operation);
        assert_eq!(
            resolved.continuation.pending_effect,
            static_request.continuation.pending_effect
        );
        assert_eq!(
            resolved.continuation.result_type,
            static_request.continuation.result_type
        );
        assert!(resolved.budget.fuel_remaining < static_request.budget.fuel_remaining);
    }
}

#[test]
fn computed_text_is_data_not_an_executable_source_fragment() {
    let text = r#""), injected: runtime.deploy(runtime_id: "other", pipeline_kind: "run")"#;
    let source = format!(
        "ui.assert_text(node_id: \"a\", expected: concat(left: {}, right: \"\"))",
        serde_json::to_string(text).unwrap()
    );
    let mut vm = Vm::default();
    let request = effect(start(&mut vm, &source));
    assert_eq!(request.required_capability, "ui.presentation");
    assert_eq!(
        request.continuation.pending_effect,
        Effect::UiAssertText {
            node_id: "a".into(),
            expected: text.into()
        }
    );
    assert_eq!(vm.pending_count(), 1);
}

#[test]
fn invalid_computed_values_leave_no_journal_rows_or_effect_ids() {
    for (source, code) in [
        (
            r#"ui.focus(node_id: concat(left: "bad", right: " node"))"#.to_string(),
            "LSV1404",
        ),
        (
            r#"ui.navigate_focus(node_id: "a", direction: concat(left: "un", right: "known"))"#
                .into(),
            "LSV1404",
        ),
        (
            r#"ui.assert_child_count(node_id: "a", count: concat(left: "409", right: "7"))"#.into(),
            "LSV1404",
        ),
        (
            format!(
                r#"ui.focus(node_id: concat(left: "{}", right: "x"))"#,
                "a".repeat(128)
            ),
            "LSV1404",
        ),
        (
            format!(
                r#"ui.assert_text(node_id: "a", expected: concat(left: "{}", right: "x"))"#,
                "a".repeat(1024)
            ),
            "LSV1404",
        ),
        (
            r#"ui.focus(node_id: bind(n: div(left: 1, right: 0), body: "a"))"#.into(),
            "LSV1401",
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        let Step::Fault(failure) = start(&mut vm, &source) else {
            panic!("expected fault for {source}");
        };
        assert_eq!(failure.code, code);
        assert!(!failure.message.contains("bad node"));
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 0);
        drop(vm);
        let connection = rusqlite::Connection::open(&path.0).unwrap();
        for table in ["vm_effects", "vm_dispatches"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0);
        }
        drop(connection);
        let mut vm = Vm::open_journal(&path.0, 1000).unwrap();
        assert_eq!(
            effect(start(&mut vm, r#"ui.focus(node_id: "a")"#)).effect_id,
            "effect-1"
        );
    }
}

#[test]
fn computed_argument_fuel_is_charged_before_allocation_and_preserved_on_recovery() {
    let source = r#"bind(node: "a", body: ui.focus(node_id: node))"#;
    // bind + literal/string copy + call + local/string copy + argument materialization = 7.
    let mut exhausted = Vm::new(7);
    assert!(matches!(start(&mut exhausted, source), Step::Fault(fault) if fault.code == "LSV1001"));
    assert_eq!(exhausted.pending_count(), 0);
    assert_eq!(
        effect(start(&mut exhausted, r#"ui.focus(node_id: "a")"#)).effect_id,
        "effect-1"
    );
    let path = JournalPath::new();
    let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
    let mut vm = Vm::open_journal(&path.0, 8).unwrap();
    let request = effect(vm.start_timed(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
        100,
        50,
    ));
    assert_eq!(request.budget.fuel_remaining, 0);
    assert_eq!(request.budget.deadline_at_ms, Some(150));
    assert_eq!(
        decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap(),
        request.continuation
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let recovered = vm.claim_effect(101, 10).unwrap().unwrap();
    assert_eq!(recovered.request, request);
    let cancelled = vm.cancel_effect(&request.continuation, 102);
    assert!(matches!(cancelled, Step::Cancelled(_)));
    assert_eq!(
        vm.acknowledge_effect(&recovered, 103, result("a")),
        cancelled
    );
    let mut forged = request.continuation;
    forged.pending_effect = program.function.effect;
    assert!(encode_continuation(&forged).is_err());
}

#[test]
fn resolved_mutation_parameters_identity_and_revision_survive_retry_restart() {
    let path = JournalPath::new();
    let program = lower(&parse(r#"fn main() = bind(node: concat(left: "runtime-", right: "a"), body: runtime.deploy(target: concat(left: "/tmp/", right: "demo.gewy"), pipeline_kind: "run", runtime_id: node))"#)).unwrap();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let principal = Principal::new("operator").unwrap();
    assert!(
        matches!(vm.start(&program, principal.clone(), CapabilitySet::default(), Some(Revision(7))), Step::Fault(fault) if fault.code == "LSH2001")
    );
    assert_eq!(vm.pending_count(), 0);
    let request = effect(vm.start(
        &program,
        principal,
        CapabilitySet::new(["runtime.deploy"]),
        Some(Revision(7)),
    ));
    assert!(
        matches!(&request.continuation.pending_effect, Effect::RuntimeDeploy { runtime_id, pipeline_kind, target } if runtime_id.as_str() == "runtime-a" && pipeline_kind == "run" && target.as_deref() == Some("/tmp/demo.gewy"))
    );
    let first = vm.claim_effect(1, 100).unwrap().unwrap();
    assert!(matches!(
        vm.report_effect_error(
            &first,
            2,
            EffectError {
                class: EffectErrorClass::Transient,
                code: "busy".into(),
                message: "try later".into()
            },
            &RetryPolicy::default()
        )
        .unwrap(),
        RetryDisposition::Scheduled(_)
    ));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert!(vm.claim_effect(251, 100).unwrap().is_none());
    let retried = vm.claim_effect(252, 100).unwrap().unwrap();
    assert_eq!(retried.request, request);
    assert_eq!(retried.retry_count, 1);
    let EffectOperation::Command(command) = &retried.request.operation else {
        panic!("expected mutation")
    };
    assert_eq!(command.expected_revision, Some(Revision(7)));
    assert_eq!(command.idempotency_key.as_str(), "leselang-effect-1");
}

#[test]
fn computed_call_wire_cannot_change_the_operation_or_add_hidden_arguments() {
    use leselang_hir::computation::Computation;
    let program = lower(&parse(
        r#"fn main() = ui.focus(node_id: concat(left: "a", right: "b"))"#,
    ))
    .unwrap();
    let Effect::Compute { expression } = program.function.effect else {
        panic!("expected call")
    };
    let wire = serde_json::to_value(expression.as_ref()).unwrap();
    assert_eq!(
        serde_json::from_value::<Computation>(wire.clone()).unwrap(),
        *expression
    );
    for unknown in ["runtime.exec", "ui.focus(node_id: injected)"] {
        let mut forged = wire.clone();
        forged["operation"] = serde_json::json!(unknown);
        assert!(serde_json::from_value::<Computation>(forged).is_err());
    }
    let mut forged = wire;
    forged["arguments"][0]["extra"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Computation>(forged).is_err());
}
