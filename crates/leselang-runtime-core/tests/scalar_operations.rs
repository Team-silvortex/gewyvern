use leselang_runtime_core::{
    BinaryOperator, OptionalStringValue, ScalarError, ScalarType, ScalarValue, StringListValue,
    UnaryOperator, apply_binary, apply_unary,
};

const BINARY: [BinaryOperator; 23] = [
    BinaryOperator::Add,
    BinaryOperator::Sub,
    BinaryOperator::Mul,
    BinaryOperator::Div,
    BinaryOperator::Rem,
    BinaryOperator::Eq,
    BinaryOperator::Ne,
    BinaryOperator::Lt,
    BinaryOperator::Le,
    BinaryOperator::Gt,
    BinaryOperator::Ge,
    BinaryOperator::And,
    BinaryOperator::Or,
    BinaryOperator::Concat,
    BinaryOperator::ValueOr,
    BinaryOperator::Contains,
    BinaryOperator::StartsWith,
    BinaryOperator::EndsWith,
    BinaryOperator::CharAt,
    BinaryOperator::Split,
    BinaryOperator::Join,
    BinaryOperator::Append,
    BinaryOperator::ItemAt,
];
const UNARY: [UnaryOperator; 7] = [
    UnaryOperator::Not,
    UnaryOperator::Len,
    UnaryOperator::ToString,
    UnaryOperator::ParseInteger,
    UnaryOperator::ParseBoolean,
    UnaryOperator::OptionalString,
    UnaryOperator::HasValue,
];

fn text(value: &str) -> ScalarValue {
    ScalarValue::String(value.into())
}
fn optional(value: Option<&str>) -> ScalarValue {
    ScalarValue::OptionalString(OptionalStringValue(value.map(str::to_owned)))
}
fn list(values: &[&str]) -> ScalarValue {
    ScalarValue::StringList(StringListValue(
        values.iter().map(|value| (*value).into()).collect(),
    ))
}
fn samples() -> [ScalarValue; 6] {
    [
        ScalarValue::Integer(2),
        ScalarValue::Boolean(true),
        text("2"),
        ScalarValue::None,
        optional(Some("2")),
        list(&["2"]),
    ]
}

#[test]
fn all_operator_names_and_closed_serde_tags_are_exact() {
    let names = [
        "add",
        "sub",
        "mul",
        "div",
        "rem",
        "eq",
        "ne",
        "lt",
        "le",
        "gt",
        "ge",
        "and",
        "or",
        "concat",
        "value_or",
        "contains",
        "starts_with",
        "ends_with",
        "char_at",
        "split",
        "join",
        "append",
        "item_at",
    ];
    for (operator, name) in BINARY.into_iter().zip(names) {
        let wire = format!("\"{name}\"");
        assert_eq!(operator.name(), name);
        assert_eq!(BinaryOperator::parse(name), Some(operator));
        assert_eq!(serde_json::to_string(&operator).unwrap(), wire);
        assert_eq!(
            serde_json::from_str::<BinaryOperator>(&wire).unwrap(),
            operator
        );
    }
    for (operator, name) in UNARY.into_iter().zip([
        "not",
        "len",
        "to_string",
        "parse_integer",
        "parse_boolean",
        "optional_string",
        "has_value",
    ]) {
        let wire = format!("\"{name}\"");
        assert_eq!(operator.name(), name);
        assert_eq!(UnaryOperator::parse(name), Some(operator));
        assert_eq!(serde_json::to_string(&operator).unwrap(), wire);
        assert_eq!(
            serde_json::from_str::<UnaryOperator>(&wire).unwrap(),
            operator
        );
    }
    for name in [
        "",
        "ADD",
        " add",
        "add ",
        "ui.focus",
        "__proto__",
        "value-or",
        "parseInteger",
    ] {
        assert_eq!(BinaryOperator::parse(name), None);
        assert_eq!(UnaryOperator::parse(name), None);
        let wire = serde_json::to_string(name).unwrap();
        assert!(serde_json::from_str::<BinaryOperator>(&wire).is_err());
        assert!(serde_json::from_str::<UnaryOperator>(&wire).is_err());
    }
}

#[test]
fn unary_signatures_and_execution_agree_for_every_scalar_type() {
    use ScalarType::{Boolean, Integer, None, OptionalString, String, StringList};
    for operator in UNARY {
        for value in samples() {
            let input = value.scalar_type();
            let expected = match operator {
                UnaryOperator::Not if input == Boolean => Some(Boolean),
                UnaryOperator::Len if matches!(input, String | StringList) => Some(Integer),
                UnaryOperator::ToString if matches!(input, Integer | Boolean | String) => {
                    Some(String)
                }
                UnaryOperator::ParseInteger if input == String => Some(Integer),
                UnaryOperator::ParseBoolean if input == String => Some(Boolean),
                UnaryOperator::OptionalString if matches!(input, String | None) => {
                    Some(OptionalString)
                }
                UnaryOperator::HasValue if input == OptionalString => Some(Boolean),
                _ => Option::None,
            };
            assert_eq!(
                operator.result_type(input),
                expected,
                "{operator:?}, {input:?}"
            );
            match (expected, apply_unary(operator, value)) {
                (Option::None, Err(ScalarError::TypeMismatch)) => {}
                (Some(expected), Ok(value)) => {
                    assert_eq!(value.scalar_type(), expected);
                    assert!(value.is_bounded());
                }
                (Some(Boolean), Err(ScalarError::InvalidBooleanText))
                    if operator == UnaryOperator::ParseBoolean => {}
                unexpected => panic!("{operator:?}, {input:?}: {unexpected:?}"),
            }
        }
    }
}

#[test]
fn binary_signatures_and_execution_agree_for_every_scalar_type_pair() {
    use BinaryOperator::*;
    use ScalarType::{Boolean, Integer, OptionalString, String, StringList};
    for operator in BINARY {
        for left in samples() {
            for right in samples() {
                let inputs = (left.scalar_type(), right.scalar_type());
                let expected = match (operator, inputs) {
                    (Eq | Ne, (left, right)) if left == right => Some(Boolean),
                    (Add | Sub | Mul | Div | Rem, (Integer, Integer)) => Some(Integer),
                    (Lt | Le | Gt | Ge, (Integer, Integer)) => Some(Boolean),
                    (And | Or, (Boolean, Boolean)) => Some(Boolean),
                    (Concat, (String, String))
                    | (ValueOr, (OptionalString, String))
                    | (Join, (StringList, String)) => Some(String),
                    (Contains | StartsWith | EndsWith, (String, String)) => Some(Boolean),
                    (CharAt, (String, Integer)) | (ItemAt, (StringList, Integer)) => {
                        Some(OptionalString)
                    }
                    (Split, (String, String)) | (Append, (StringList, String)) => Some(StringList),
                    _ => None,
                };
                assert_eq!(
                    operator.result_type(inputs.0, inputs.1),
                    expected,
                    "{operator:?}, {inputs:?}"
                );
                match (expected, apply_binary(operator, left.clone(), right)) {
                    (None, Err(ScalarError::TypeMismatch)) => {}
                    (Some(expected), Ok(value)) => {
                        assert_eq!(value.scalar_type(), expected);
                        assert!(value.is_bounded());
                    }
                    unexpected => panic!("{operator:?}, {inputs:?}: {unexpected:?}"),
                }
            }
        }
    }
}

#[test]
fn full_width_arithmetic_is_checked_and_never_wraps() {
    use BinaryOperator::{Add, Div, Mul, Rem, Sub};
    for left in [0_u64, 1, 2, 63, u64::MAX / 2, u64::MAX - 1, u64::MAX] {
        for right in [0_u64, 1, 2, 63, u64::MAX / 2, u64::MAX - 1, u64::MAX] {
            for (operator, expected) in [
                (Add, left.checked_add(right)),
                (Sub, left.checked_sub(right)),
                (Mul, left.checked_mul(right)),
                (Div, left.checked_div(right)),
                (Rem, left.checked_rem(right)),
            ] {
                assert_eq!(
                    apply_binary(
                        operator,
                        ScalarValue::Integer(left),
                        ScalarValue::Integer(right)
                    ),
                    expected
                        .map(ScalarValue::Integer)
                        .ok_or(ScalarError::IntegerArithmetic),
                    "{left}, {operator:?}, {right}"
                );
            }
        }
    }
}

#[test]
fn strict_conversions_do_not_coerce_or_echo_input() {
    for (input, expected) in [("0", 0), ("0002", 2), ("18446744073709551615", u64::MAX)] {
        assert_eq!(
            apply_unary(UnaryOperator::ParseInteger, text(input)),
            Ok(ScalarValue::Integer(expected))
        );
    }
    for input in [
        "",
        "+1",
        "-1",
        " 1",
        "1\n",
        "1.0",
        "0x10",
        "1_000",
        "\u{0661}",
        "18446744073709551616",
        "private-token",
    ] {
        assert_eq!(
            apply_unary(UnaryOperator::ParseInteger, text(input)),
            Err(ScalarError::InvalidIntegerText)
        );
    }
    for (input, expected) in [("true", true), ("false", false)] {
        assert_eq!(
            apply_unary(UnaryOperator::ParseBoolean, text(input)),
            Ok(ScalarValue::Boolean(expected))
        );
    }
    for input in ["TRUE", "True", "false ", "1", "", "private-token"] {
        assert_eq!(
            apply_unary(UnaryOperator::ParseBoolean, text(input)),
            Err(ScalarError::InvalidBooleanText)
        );
    }
    for failure in [
        ScalarError::TypeMismatch,
        ScalarError::UnboundedOperand,
        ScalarError::IntegerArithmetic,
        ScalarError::InvalidIntegerText,
        ScalarError::InvalidBooleanText,
        ScalarError::StringLimit,
        ScalarError::StringListLimit,
    ] {
        assert!(!format!("{failure:?}: {failure}").contains("private-token"));
        let failure: &dyn std::error::Error = &failure;
        assert!(failure.source().is_none());
    }
}

#[test]
fn text_inspection_uses_exact_case_and_unicode_scalar_indices() {
    let source = "e\u{0301}\u{1f600}";
    assert_eq!(
        apply_unary(UnaryOperator::Len, text(source)),
        Ok(ScalarValue::Integer(3))
    );
    for (index, expected) in [
        (0, Some("e")),
        (1, Some("\u{0301}")),
        (2, Some("\u{1f600}")),
        (3, None),
        (u64::MAX, None),
    ] {
        assert_eq!(
            apply_binary(
                BinaryOperator::CharAt,
                text(source),
                ScalarValue::Integer(index)
            ),
            Ok(optional(expected))
        );
    }
    for (operator, needle, expected) in [
        (BinaryOperator::Contains, "\u{0301}", true),
        (BinaryOperator::StartsWith, "e", true),
        (BinaryOperator::StartsWith, "E", false),
        (BinaryOperator::EndsWith, "\u{1f600}", true),
        (BinaryOperator::Contains, "", true),
    ] {
        assert_eq!(
            apply_binary(operator, text(source), text(needle)),
            Ok(ScalarValue::Boolean(expected))
        );
    }
}

#[test]
fn optional_and_equality_semantics_preserve_absence_order_and_empty_data() {
    assert_eq!(
        apply_unary(UnaryOperator::OptionalString, ScalarValue::None),
        Ok(optional(None))
    );
    assert_eq!(
        apply_unary(UnaryOperator::OptionalString, text("")),
        Ok(optional(Some("")))
    );
    for (value, has_value, chosen) in [
        (optional(None), false, "fallback"),
        (optional(Some("")), true, ""),
        (optional(Some("x")), true, "x"),
    ] {
        assert_eq!(
            apply_unary(UnaryOperator::HasValue, value.clone()),
            Ok(ScalarValue::Boolean(has_value))
        );
        assert_eq!(
            apply_binary(BinaryOperator::ValueOr, value, text("fallback")),
            Ok(text(chosen))
        );
    }
    for (left, right, equal) in [
        (optional(None), optional(Some("")), false),
        (list(&["", "a", "a"]), list(&["", "a", "a"]), true),
        (list(&["a", "b"]), list(&["b", "a"]), false),
        (ScalarValue::None, ScalarValue::None, true),
    ] {
        assert_eq!(
            apply_binary(BinaryOperator::Eq, left.clone(), right.clone()),
            Ok(ScalarValue::Boolean(equal))
        );
        assert_eq!(
            apply_binary(BinaryOperator::Ne, left, right),
            Ok(ScalarValue::Boolean(!equal))
        );
    }
    assert_eq!(
        apply_binary(BinaryOperator::Eq, ScalarValue::None, optional(None)),
        Err(ScalarError::TypeMismatch)
    );
}

#[test]
fn bounded_operands_are_validated_even_when_results_would_be_small() {
    for value in [
        text(&"x".repeat(4097)),
        optional(Some(&"x".repeat(4097))),
        ScalarValue::StringList(StringListValue(vec!["".into(); 65])),
        list(&[&"x".repeat(4097)]),
    ] {
        let unary = match value.scalar_type() {
            ScalarType::OptionalString => UnaryOperator::HasValue,
            _ => UnaryOperator::Len,
        };
        assert_eq!(
            apply_unary(unary, value.clone()),
            Err(ScalarError::UnboundedOperand)
        );
        assert_eq!(
            apply_binary(BinaryOperator::Eq, value.clone(), value),
            Err(ScalarError::UnboundedOperand)
        );
    }
    assert_eq!(
        apply_binary(
            BinaryOperator::CharAt,
            text(&"x".repeat(4097)),
            ScalarValue::Integer(u64::MAX)
        ),
        Err(ScalarError::UnboundedOperand)
    );
    assert_eq!(
        apply_binary(
            BinaryOperator::ItemAt,
            ScalarValue::StringList(StringListValue(vec!["".into(); 65])),
            ScalarValue::Integer(u64::MAX)
        ),
        Err(ScalarError::UnboundedOperand)
    );
    assert_eq!(
        apply_binary(
            BinaryOperator::ValueOr,
            optional(Some("chosen")),
            text(&"x".repeat(4097))
        ),
        Err(ScalarError::UnboundedOperand)
    );
    assert_eq!(
        apply_binary(
            BinaryOperator::And,
            ScalarValue::Boolean(false),
            text("not boolean")
        ),
        Err(ScalarError::TypeMismatch)
    );
    assert_eq!(
        apply_binary(
            BinaryOperator::Or,
            ScalarValue::Boolean(true),
            ScalarValue::None
        ),
        Err(ScalarError::TypeMismatch)
    );
}

#[test]
fn concatenation_and_join_check_output_bytes_before_materializing() {
    let maximum = "\u{1f600}".repeat(1024);
    assert_eq!(
        apply_binary(BinaryOperator::Concat, text(&maximum), text("")),
        Ok(text(&maximum))
    );
    assert_eq!(
        apply_binary(BinaryOperator::Concat, text(&maximum), text("x")),
        Err(ScalarError::StringLimit)
    );
    assert_eq!(
        apply_binary(BinaryOperator::Join, list(&[&maximum, ""]), text("")),
        Ok(text(&maximum))
    );
    assert_eq!(
        apply_binary(BinaryOperator::Join, list(&[&maximum, ""]), text("x")),
        Err(ScalarError::StringLimit)
    );
    assert_eq!(
        apply_binary(BinaryOperator::Join, list(&[]), text(&maximum)),
        Ok(text(""))
    );
    assert_eq!(
        apply_binary(BinaryOperator::Join, list(&["one"]), text(&maximum)),
        Ok(text("one"))
    );
    assert_eq!(
        apply_binary(
            BinaryOperator::Join,
            list(&["", "", ""]),
            text(&"x".repeat(2048))
        ),
        Ok(text(&"x".repeat(4096)))
    );
    assert_eq!(
        apply_binary(
            BinaryOperator::Join,
            list(&["", "", ""]),
            text(&"x".repeat(2049))
        ),
        Err(ScalarError::StringLimit)
    );
}

#[test]
fn splitting_and_append_enforce_count_and_aggregate_bytes() {
    assert_eq!(
        apply_binary(BinaryOperator::Split, text("a,,a,"), text(",")),
        Ok(list(&["a", "", "a", ""]))
    );
    assert_eq!(
        apply_binary(BinaryOperator::Split, text("\u{1f600}"), text("")),
        Ok(list(&["", "\u{1f600}", ""]))
    );
    assert_eq!(
        apply_binary(BinaryOperator::Split, text(&"x".repeat(62)), text("")),
        Ok(ScalarValue::StringList(StringListValue(
            std::iter::once("".into())
                .chain(std::iter::repeat_n("x".into(), 62))
                .chain(std::iter::once("".into()))
                .collect()
        )))
    );
    assert_eq!(
        apply_binary(BinaryOperator::Split, text(&"x".repeat(63)), text("")),
        Err(ScalarError::StringListLimit)
    );
    let maximum = "x".repeat(4096);
    assert_eq!(
        apply_binary(BinaryOperator::Append, list(&[&maximum]), text("")),
        Ok(list(&[&maximum, ""]))
    );
    assert_eq!(
        apply_binary(BinaryOperator::Append, list(&[&maximum]), text("x")),
        Err(ScalarError::StringListLimit)
    );
    assert_eq!(
        apply_binary(
            BinaryOperator::Append,
            ScalarValue::StringList(StringListValue(vec!["".into(); 63])),
            text("")
        ),
        Ok(ScalarValue::StringList(StringListValue(vec![
            "".into();
            64
        ])))
    );
    assert_eq!(
        apply_binary(
            BinaryOperator::Append,
            ScalarValue::StringList(StringListValue(vec!["".into(); 64])),
            text("")
        ),
        Err(ScalarError::StringListLimit)
    );
    for (index, expected) in [(0, Some("")), (1, Some("a")), (2, None), (u64::MAX, None)] {
        assert_eq!(
            apply_binary(
                BinaryOperator::ItemAt,
                list(&["", "a"]),
                ScalarValue::Integer(index)
            ),
            Ok(optional(expected))
        );
    }
}

#[test]
fn owned_pass_through_operations_do_not_clone_text_buffers() {
    for operator in [UnaryOperator::ToString, UnaryOperator::OptionalString] {
        let original = std::string::String::from("owned-text");
        let pointer = original.as_ptr();
        let result = apply_unary(operator, ScalarValue::String(original)).unwrap();
        assert_eq!(result.text().unwrap().as_ptr(), pointer);
    }
    let original = std::string::String::from("chosen-text");
    let pointer = original.as_ptr();
    let result = apply_binary(
        BinaryOperator::ValueOr,
        ScalarValue::OptionalString(OptionalStringValue(Some(original))),
        text("fallback"),
    )
    .unwrap();
    assert_eq!(result.text().unwrap().as_ptr(), pointer);
}
