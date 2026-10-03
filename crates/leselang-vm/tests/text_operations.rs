use leselang_hir::computation::{BinaryOperator, Computation, OptionalStringValue};
use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, PresentationOperation, ScalarValue, Step, Value,
    Vm, decode_continuation, encode_continuation,
};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-text-operations-{}-{}",
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
fn calculate(body: &str, fuel: u64) -> Step {
    start(&mut Vm::new(fuel), &format!("fn main() = {body}"))
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
fn optional(value: Option<&str>) -> ScalarValue {
    ScalarValue::OptionalString(OptionalStringValue(value.map(str::to_owned)))
}
fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    serde_json::from_value::<leselang_vm::PresentationResult>(
        serde_json::to_value(&envelope.operation).unwrap(),
    )
    .map(EffectResult::Presentation)
    .unwrap()
}
fn fault(step: Step, code: &str) {
    assert!(
        matches!(step, Step::Fault(ref fault) if fault.code == code),
        "{step:?}"
    );
}
fn flow(text: &str) -> String {
    format!(
        r#"fn ready(text: string) = and(left: starts_with(left: text, right: "ready"), right: and(left: contains(left: text, right: "ad"), right: ends_with(left: text, right: "!")))
        fn main() = bind(first: ui.assert_text(node_id: "status", expected: {}), body:
            bind(head: char_at(left: field(value: first, name: "expected"), right: 0), body:
                bind(last: ui.set_form_value(node_id: "form", field: "prefix", value: choose(when: ready(text: field(value: first, name: "expected")), then: value_or(left: head, right: "default"), otherwise: "default")), body: head)))"#,
        quote(text)
    )
}

#[test]
fn text_predicates_use_exact_case_sensitive_utf8_with_explicit_empty_patterns() {
    for (operation, text, pattern, expected) in [
        ("contains", "ready!", "ead", true),
        ("contains", "ready!", "READY", false),
        ("contains", "abc", "abcd", false),
        ("contains", "", "", true),
        ("starts_with", "abc", "", true),
        ("ends_with", "", "a", false),
        ("starts_with", "ready!", "ead", false),
        ("ends_with", "ready!", "dy!", true),
        ("contains", "\u{754c}\u{1f642}e\u{301}", "\u{1f642}e", true),
        ("starts_with", "\u{754c}\u{1f642}", "\u{754c}", true),
        ("ends_with", "e\u{301}", "\u{301}", true),
        ("contains", "e\u{301}", "\u{e9}", false),
        ("contains", "a\nb", "\n", true),
    ] {
        let body = format!(
            "{operation}(left: {}, right: {})",
            quote(text),
            quote(pattern)
        );
        assert_eq!(
            calculate(&body, 10000),
            done(ScalarValue::Boolean(expected)),
            "{body}"
        );
    }
}

#[test]
fn character_access_is_zero_based_unicode_scalar_access_not_bytes_or_graphemes() {
    let text = "\u{754c}\u{1f642}e\u{301}";
    for (index, expected) in [
        (0, Some("\u{754c}")),
        (1, Some("\u{1f642}")),
        (2, Some("e")),
        (3, Some("\u{301}")),
        (4, None),
        (u64::MAX, None),
    ] {
        assert_eq!(
            calculate(
                &format!("char_at(left: {}, right: {index})", quote(text)),
                1000
            ),
            done(optional(expected))
        );
    }
    assert_eq!(
        calculate(r#"char_at(left: "", right: 0)"#, 100),
        done(optional(None))
    );
    assert_eq!(
        calculate(
            r#"value_or(left: char_at(left: "", right: 0), right: "missing")"#,
            100
        ),
        done(ScalarValue::String("missing".into()))
    );
    assert_eq!(
        calculate(r#"has_value(value: char_at(left: "x", right: 0))"#, 100),
        done(ScalarValue::Boolean(true))
    );
    assert_eq!(
        calculate(
            r#"eq(left: char_at(left: "", right: 0), right: optional_string(value: ""))"#,
            100
        ),
        done(ScalarValue::Boolean(false))
    );
}

#[test]
fn text_access_composes_with_pure_helpers_and_bounded_iteration() {
    let source = format!(
        r#"fn at(text: string, index: integer) = char_at(left: text, right: index)
        fn main() = bind(original: {}, body: loop(text: "", while: lt(left: len(value: text), right: len(value: original)), next: concat(left: text, right: value_or(left: at(text: original, index: len(value: text)), right: "")), limit: 4))"#,
        quote("\u{754c}\u{1f642}e\u{301}")
    );
    assert_eq!(
        start(&mut Vm::new(10000), &source),
        done(ScalarValue::String("\u{754c}\u{1f642}e\u{301}".into()))
    );
    assert_eq!(
        calculate(
            r#"loop(index: 0, while: has_value(value: char_at(left: "abc", right: index)), next: add(left: index, right: 1), limit: 3)"#,
            1000
        ),
        done(ScalarValue::Integer(3))
    );
    fault(
        calculate(
            r#"loop(index: 0, while: has_value(value: char_at(left: "abc", right: index)), next: add(left: index, right: 1), limit: 2)"#,
            1000,
        ),
        "LSV1406",
    );
}

#[test]
fn binary_operands_are_eager_but_parent_branches_and_optional_defaults_stay_lazy() {
    for body in [
        r#"contains(left: "", right: to_string(value: parse_integer(value: "secret-invalid")))"#,
        r#"starts_with(left: "", right: to_string(value: parse_integer(value: "secret-invalid")))"#,
        r#"ends_with(left: "", right: to_string(value: parse_integer(value: "secret-invalid")))"#,
        r#"char_at(left: "", right: parse_integer(value: "secret-invalid"))"#,
    ] {
        let step = calculate(body, 1000);
        assert!(!format!("{step:?}").contains("secret-invalid"));
        fault(step, "LSV1408");
    }
    for body in [
        r#"contains(right: to_string(value: parse_integer(value: "bad")), left: to_string(value: div(left: 1, right: 0)))"#,
        r#"char_at(right: parse_integer(value: "bad"), left: to_string(value: div(left: 1, right: 0)))"#,
    ] {
        fault(calculate(body, 1000), "LSV1401");
    }
    assert_eq!(
        calculate(
            r#"choose(when: true, then: false, otherwise: contains(left: "", right: to_string(value: div(left: 1, right: 0))))"#,
            100
        ),
        done(ScalarValue::Boolean(false))
    );
    assert_eq!(
        calculate(
            r#"value_or(left: char_at(left: "x", right: 0), right: to_string(value: div(left: 1, right: 0)))"#,
            100
        ),
        done(ScalarValue::String("x".into()))
    );
    assert_eq!(
        calculate(
            r#"recover(value: char_at(left: "", right: 0), fallback: optional_string(value: "fallback"))"#,
            100
        ),
        done(optional(None))
    );
}

#[test]
fn scanning_and_character_materialization_share_the_original_fuel() {
    assert_eq!(
        calculate(r#"char_at(left: "x", right: 0)"#, 6),
        done(optional(Some("x")))
    );
    fault(calculate(r#"char_at(left: "x", right: 0)"#, 5), "LSV1001");
    assert_eq!(
        calculate(r#"char_at(left: "x", right: 1)"#, 5),
        done(optional(None))
    );
    assert_eq!(
        calculate(r#"contains(left: "x", right: "x")"#, 7),
        done(ScalarValue::Boolean(true))
    );
    fault(
        calculate(r#"contains(left: "x", right: "x")"#, 6),
        "LSV1001",
    );
    let maximum = quote(&"x".repeat(4096));
    // Input copying and the complete scan are charged even when the first scalar matches.
    assert_eq!(
        calculate(&format!("char_at(left: {maximum}, right: 0)"), 132),
        done(optional(Some("x")))
    );
    fault(
        calculate(&format!("char_at(left: {maximum}, right: 0)"), 131),
        "LSV1001",
    );
    assert_eq!(
        calculate(
            &format!("char_at(left: {maximum}, right: {})", u64::MAX),
            131
        ),
        done(optional(None))
    );
    fault(
        calculate(
            &format!(
                "recover(value: has_value(value: char_at(left: {maximum}, right: 0)), fallback: true)"
            ),
            10,
        ),
        "LSV1001",
    );
}

#[test]
fn saved_text_decisions_and_optional_character_locals_resume_and_replay_without_source() {
    for text in ["", "ready!", "other", "\u{754c}\u{1f642}"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut vm, &flow(text)));
        let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        let EffectOperation::Presentation(envelope) = &second.operation else {
            panic!()
        };
        assert!(
            matches!(&envelope.operation, PresentationOperation::SetFormValue { value, .. } if value == if text == "ready!" { "r" } else { "default" })
        );
        assert_eq!(second.continuation.expected_revision, Some(Revision(7)));
        assert_eq!(second.continuation.deadline_ms, 100);
        assert_eq!(second.continuation.deadline_at_ms, Some(200));
        assert!(second.continuation.fuel_remaining < first.continuation.fuel_remaining);
        assert_eq!(
            decode_continuation(&encode_continuation(&second.continuation).unwrap()).unwrap(),
            second.continuation
        );
        let binding = second.continuation.result_binding.as_ref().unwrap();
        assert!(binding.locals.iter().any(|local| {
            local.value
                == optional(
                    text.chars()
                        .next()
                        .map(|value| value.to_string())
                        .as_deref(),
                )
        }));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 102, receipt(&first)),
            Step::Effect(Box::new(second.clone()))
        );
        let result = done(optional(
            text.chars()
                .next()
                .map(|value| value.to_string())
                .as_deref(),
        ));
        assert_eq!(
            vm.resume_at(&second.continuation, 103, receipt(&second)),
            result
        );
        assert_eq!(
            vm.resume_at(&second.continuation, 104, receipt(&first)),
            result
        );
        assert_eq!(
            vm.resume_at(&first.continuation, 104, receipt(&first)),
            result
        );
    }
}

#[test]
fn named_group_text_decisions_keep_the_all_success_barrier_and_optional_tail() {
    for kind in ["seq", "all"] {
        let source = format!(
            r#"fn main() = bind(group: {kind}(one: ui.assert_text(node_id: "a", expected: "ready!"), two: ui.assert_text(node_id: "b", expected: "other")), body:
            choose(when: contains(left: field(value: member(value: group, name: "one"), name: "expected"), right: "ready"),
                then: bind(last: ui.set_form_value(node_id: "form", field: "prefix", value: value_or(left: char_at(left: field(value: member(value: group, name: "two"), name: "expected"), right: 0), right: "missing")), body: ends_with(left: field(value: last, name: "value"), right: "o")),
                otherwise: false))"#
        );
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let step = start(&mut vm, &source);
        let (first, third) = match step {
            Step::Effect(first) => {
                let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
                assert!(
                    matches!(&second.operation, EffectOperation::Presentation(envelope) if matches!(envelope.operation, PresentationOperation::AssertText { .. }))
                );
                drop(vm);
                vm = Vm::open_journal(&path.0, 1).unwrap();
                (
                    *first,
                    effect(vm.resume_at(&second.continuation, 102, receipt(&second))),
                )
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
                assert_eq!(
                    Connection::open(&path.0)
                        .unwrap()
                        .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                            .get::<_, i64>(0))
                        .unwrap(),
                    2
                );
                let first = batch.branches[0].request.clone();
                drop(vm);
                vm = Vm::open_journal(&path.0, 1).unwrap();
                let third = effect(vm.resume_at(&first.continuation, 102, receipt(&first)));
                (first, third)
            }
            other => panic!("{other:?}"),
        };
        assert!(
            matches!(&third.operation, EffectOperation::Presentation(envelope) if matches!(&envelope.operation, PresentationOperation::SetFormValue { value, .. } if value == "o"))
        );
        assert_eq!(
            vm.resume_at(&third.continuation, 103, receipt(&third)),
            done(ScalarValue::Boolean(true))
        );
        assert_eq!(
            vm.resume_at(&first.continuation, 104, receipt(&first)),
            done(ScalarValue::Boolean(true))
        );
    }
}

#[test]
fn character_outputs_cannot_bypass_host_validation_or_partial_group_admission() {
    let source = r#"fn main() = bind(first: ui.focus(node_id: "a"), body: ui.set_form_value(node_id: "form", field: "value", value: value_or(left: char_at(left: "a\n", right: 1), right: "")))"#;
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(&mut vm, source));
    fault(
        vm.resume_at(&first.continuation, 101, receipt(&first)),
        "LSV1404",
    );
    assert_eq!(
        Connection::open(&path.0)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    for kind in ["seq", "all"] {
        let source = format!(
            r#"fn main() = {kind}(valid: ui.focus(node_id: "a"), invalid: ui.set_form_value(node_id: "form", field: "value", value: value_or(left: char_at(left: "a\n", right: 1), right: "")))"#
        );
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        fault(start(&mut vm, &source), "LSV1404");
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(
            Connection::open(&path.0)
                .unwrap()
                .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}

#[test]
fn restored_text_bodies_reject_unknown_or_wrong_typed_operators_and_keep_deadlines() {
    let source = r#"fn main() = bind(first: ui.assert_text(node_id: "a", expected: "x"), body: char_at(left: field(value: first, name: "expected"), right: 0))"#;
    let first = effect(start(&mut Vm::new(1000), source));
    let wire = serde_json::to_value(&first.continuation).unwrap();
    for operator in ["string_regex", "contains", "add"] {
        let mut changed = wire.clone();
        changed["result_binding"]["body"]["operator"] = serde_json::json!(operator);
        assert!(
            decode_continuation(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "{operator}"
        );
    }
    let mut exhausted = first.clone();
    exhausted.continuation.fuel_remaining = 1;
    exhausted.budget.fuel_remaining = 1;
    let mut vm = Vm::new(1000);
    vm.restore_request(exhausted.clone()).unwrap();
    fault(
        vm.resume_at(&exhausted.continuation, 101, receipt(&first)),
        "LSV1001",
    );
    let mut vm = Vm::new(1000);
    vm.restore_request(first.clone()).unwrap();
    assert!(matches!(
        vm.resume_at(&first.continuation, 200, receipt(&first)),
        Step::Cancelled(_)
    ));
    let mut vm = Vm::new(1000);
    vm.restore_request(first.clone()).unwrap();
    assert!(matches!(
        vm.cancel_effect(&first.continuation, 101),
        Step::Cancelled(_)
    ));
    assert!(matches!(
        vm.resume_at(&first.continuation, 102, receipt(&first)),
        Step::Cancelled(_)
    ));
}

#[test]
fn mismatched_text_receipts_never_select_a_successor_and_replay_the_committed_fault() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(&mut vm, &flow("ready!")));
    let EffectResult::Presentation(result) = receipt(&first) else {
        panic!()
    };
    let mut wrong = serde_json::to_value(result).unwrap();
    wrong["expected"] = serde_json::json!("other");
    let terminal = vm.resume_at(
        &first.continuation,
        101,
        EffectResult::Presentation(serde_json::from_value(wrong).unwrap()),
    );
    assert!(matches!(&terminal, Step::Fault(_)));
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(
        Connection::open(&path.0)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&first.continuation, 102, receipt(&first)),
        terminal
    );
}

#[test]
fn sql_failures_roll_back_text_selected_successors_and_terminal_values() {
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
            let first = effect(start(&mut vm, &flow("ready!")));
            let current = if finishing {
                effect(vm.resume_at(&first.continuation, 101, receipt(&first)))
            } else {
                first
            };
            let connection = Connection::open(&path.0).unwrap();
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row.get(0))
                .unwrap();
            connection.execute_batch(&format!("CREATE TRIGGER reject_transition BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
            assert!(matches!(
                vm.resume_at(&current.continuation, 102, receipt(&current)),
                Step::Fault(_)
            ));
            assert_eq!(
                connection
                    .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                count
            );
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
            assert_eq!(step, done(optional(Some("r"))));
        }
    }
}

#[test]
fn text_operators_reuse_closed_binary_wire_without_widening_legacy_nodes() {
    for (operator, name, right) in [
        (
            BinaryOperator::Contains,
            "contains",
            ScalarValue::String("x".into()),
        ),
        (
            BinaryOperator::StartsWith,
            "starts_with",
            ScalarValue::String("x".into()),
        ),
        (
            BinaryOperator::EndsWith,
            "ends_with",
            ScalarValue::String("x".into()),
        ),
        (BinaryOperator::CharAt, "char_at", ScalarValue::Integer(0)),
    ] {
        let computation = Computation::Binary {
            operator,
            left: Box::new(Computation::Literal {
                value: ScalarValue::String("x".into()),
            }),
            right: Box::new(Computation::Literal { value: right }),
        };
        let mut wire = serde_json::to_value(&computation).unwrap();
        assert_eq!(wire["kind"], "binary");
        assert_eq!(wire["operator"], name);
        assert_eq!(
            serde_json::from_value::<Computation>(wire.clone()).unwrap(),
            computation
        );
        wire["operator"] = serde_json::json!("string_regex");
        assert!(serde_json::from_value::<Computation>(wire).is_err());
    }
    assert_eq!(
        serde_json::to_value(BinaryOperator::Concat).unwrap(),
        "concat"
    );
}
