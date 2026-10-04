use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{
    BinaryOperator, OptionalStringValue, ScalarValue, StringListValue, UnaryOperator, apply_binary,
    apply_unary,
};
use leselang_syntax::parse;
use leselang_vm::{Step, Value, Vm};

fn source(value: &ScalarValue) -> String {
    match value {
        ScalarValue::Integer(value) => value.to_string(),
        ScalarValue::Boolean(value) => value.to_string(),
        ScalarValue::String(value) => serde_json::to_string(value).unwrap(),
        ScalarValue::None => "none".into(),
        ScalarValue::OptionalString(value) => format!(
            "optional_string(value: {})",
            value.0.as_ref().map_or_else(
                || "none".into(),
                |value| serde_json::to_string(value).unwrap()
            )
        ),
        ScalarValue::StringList(value) => format!(
            "strings({})",
            value
                .0
                .iter()
                .enumerate()
                .map(|(index, value)| format!(
                    "item{index}: {}",
                    serde_json::to_string(value).unwrap()
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
fn start(vm: &mut Vm, expression: &str) -> Step {
    let program = lower(&parse(&format!("fn main() = {expression}"))).unwrap();
    assert!(program.function.required_capabilities.is_empty());
    vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::default(),
        None,
    )
}
fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}

#[test]
fn every_reference_operator_delegates_to_the_same_host_neutral_semantics() {
    use BinaryOperator::*;
    let mut vm = Vm::default();
    for operator in [
        Add, Sub, Mul, Div, Rem, Eq, Ne, Lt, Le, Gt, Ge, And, Or, Concat, ValueOr, Contains,
        StartsWith, EndsWith, CharAt, Split, Join, Append, ItemAt,
    ] {
        let (left, right) = match operator {
            And | Or => (ScalarValue::Boolean(true), ScalarValue::Boolean(false)),
            Eq | Ne | ValueOr => (
                ScalarValue::OptionalString(OptionalStringValue(Some("chosen".into()))),
                if operator == ValueOr {
                    ScalarValue::String("fallback".into())
                } else {
                    ScalarValue::OptionalString(OptionalStringValue(None))
                },
            ),
            Concat | Contains | StartsWith | EndsWith => (
                ScalarValue::String("ab".into()),
                ScalarValue::String("b".into()),
            ),
            CharAt => (
                ScalarValue::String("e\u{0301}\u{1f600}".into()),
                ScalarValue::Integer(1),
            ),
            Split => (
                ScalarValue::String("a,,b".into()),
                ScalarValue::String(",".into()),
            ),
            Join | Append | ItemAt => (
                ScalarValue::StringList(StringListValue(vec!["".into(), "a".into(), "a".into()])),
                if operator == ItemAt {
                    ScalarValue::Integer(1)
                } else {
                    ScalarValue::String(",".into())
                },
            ),
            _ => (ScalarValue::Integer(12), ScalarValue::Integer(3)),
        };
        let body = format!(
            "{}(left: {}, right: {})",
            operator.name(),
            source(&left),
            source(&right)
        );
        let expected = apply_binary(operator, left, right).unwrap();
        assert_eq!(start(&mut vm, &body), done(expected), "{body}");
        assert_eq!(vm.pending_count(), 0);
    }
    for (operator, value) in [
        (UnaryOperator::Not, ScalarValue::Boolean(false)),
        (
            UnaryOperator::Len,
            ScalarValue::String("e\u{0301}\u{1f600}".into()),
        ),
        (UnaryOperator::ToString, ScalarValue::Integer(u64::MAX)),
        (
            UnaryOperator::ParseInteger,
            ScalarValue::String("002".into()),
        ),
        (
            UnaryOperator::ParseBoolean,
            ScalarValue::String("false".into()),
        ),
        (UnaryOperator::OptionalString, ScalarValue::None),
        (
            UnaryOperator::HasValue,
            ScalarValue::OptionalString(OptionalStringValue(Some("".into()))),
        ),
    ] {
        let body = format!("{}(value: {})", operator.name(), source(&value));
        assert_eq!(
            start(&mut vm, &body),
            done(apply_unary(operator, value).unwrap())
        );
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn adapter_keeps_exact_legacy_fault_codes_messages_and_redaction() {
    let large = "x".repeat(4096);
    for (body, code, message) in [
        (
            "div(left: 1, right: 0)".into(),
            "LSV1401",
            "integer overflow, underflow, or division by zero",
        ),
        (
            r#"parse_integer(value: "private-token")"#.into(),
            "LSV1408",
            "invalid integer text: expected ASCII decimal within u64",
        ),
        (
            r#"parse_boolean(value: "private-token")"#.into(),
            "LSV1408",
            "invalid boolean text: expected true or false",
        ),
        (
            format!("concat(left: \"{large}\", right: \"x\")"),
            "LSV1403",
            "computed string exceeds 4096 bytes",
        ),
        (
            format!("append(left: strings(a: \"{large}\"), right: \"x\")"),
            "LSV1403",
            "computed string list exceeds 64 entries or 4096 bytes",
        ),
    ] {
        let mut vm = Vm::default();
        let Step::Fault(fault) = start(&mut vm, &body) else {
            panic!("expected scalar fault: {body}")
        };
        assert_eq!(
            (fault.code.as_str(), fault.message.as_str()),
            (code, message)
        );
        assert!(
            !serde_json::to_string(&fault)
                .unwrap()
                .contains("private-token")
        );
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn lazy_evaluation_and_exact_fuel_thresholds_stay_in_the_adapter() {
    for (body, expected, fuel) in [
        (
            r#"and(left: false, right: parse_boolean(value: "bad"))"#,
            ScalarValue::Boolean(false),
            2,
        ),
        (
            r#"or(left: true, right: parse_boolean(value: "bad"))"#,
            ScalarValue::Boolean(true),
            2,
        ),
        (
            r#"value_or(left: optional_string(value: "x"), right: to_string(value: div(left: 1, right: 0)))"#,
            ScalarValue::String("x".into()),
            4,
        ),
        (r#"to_string(value: 1)"#, ScalarValue::String("1".into()), 3),
        (
            r#"contains(left: "x", right: "x")"#,
            ScalarValue::Boolean(true),
            7,
        ),
        (
            r#"char_at(left: "x", right: 0)"#,
            ScalarValue::OptionalString(OptionalStringValue(Some("x".into()))),
            6,
        ),
        (
            r#"split(left: "x", right: ",")"#,
            ScalarValue::StringList(StringListValue(vec!["x".into()])),
            9,
        ),
        (
            r#"recover(value: div(left: 1, right: 0), fallback: 7)"#,
            ScalarValue::Integer(7),
            5,
        ),
    ] {
        assert_eq!(start(&mut Vm::new(fuel), body), done(expected), "{body}");
        assert!(
            matches!(start(&mut Vm::new(fuel - 1), body), Step::Fault(fault) if fault.code == "LSV1001"),
            "{body}"
        );
    }
    let large = "x".repeat(4096);
    let body =
        format!("recover(value: concat(left: \"{large}\", right: \"x\"), fallback: \"fallback\")");
    assert!(
        matches!(start(&mut Vm::default(), &body), Step::Fault(fault) if fault.code == "LSV1403")
    );
}
