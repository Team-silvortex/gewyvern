use leselang_hir::Type;
use leselang_hir::computation::{BinaryOperator, Computation, UnaryOperator};
use leselang_runtime_core::{OptionalStringValue, ScalarValue, StringListValue};

fn samples() -> [ScalarValue; 6] {
    [
        ScalarValue::Integer(2),
        ScalarValue::Boolean(true),
        ScalarValue::String("2".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(Some("2".into()))),
        ScalarValue::StringList(StringListValue(vec!["2".into()])),
    ]
}
fn literal(value: ScalarValue) -> Box<Computation> {
    Box::new(Computation::Literal { value })
}

#[test]
fn old_operator_imports_are_core_types_and_nested_hir_wire_is_unchanged() {
    let binary: BinaryOperator = leselang_runtime_core::BinaryOperator::Add;
    let unary: UnaryOperator = leselang_runtime_core::UnaryOperator::Len;
    let cases = [
        (
            Computation::Binary {
                operator: binary,
                left: literal(ScalarValue::Integer(2)),
                right: literal(ScalarValue::Integer(3)),
            },
            r#"{"kind":"binary","operator":"add","left":{"kind":"literal","value":{"kind":"integer","value":2}},"right":{"kind":"literal","value":{"kind":"integer","value":3}}}"#,
        ),
        (
            Computation::Unary {
                operator: unary,
                value: literal(ScalarValue::String("x".into())),
            },
            r#"{"kind":"unary","operator":"len","value":{"kind":"literal","value":{"kind":"string","value":"x"}}}"#,
        ),
    ];
    for (expression, wire) in cases {
        assert_eq!(serde_json::to_string(&expression).unwrap(), wire);
        assert_eq!(
            serde_json::from_str::<Computation>(wire).unwrap(),
            expression
        );
    }
}

#[test]
fn hir_preflight_uses_shared_signatures_for_every_operand_type_combination() {
    for name in [
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
    ] {
        let operator = BinaryOperator::parse(name).unwrap();
        for left in samples() {
            for right in samples() {
                let expected = operator
                    .result_type(left.scalar_type(), right.scalar_type())
                    .map(Type::Scalar);
                let expression = Computation::Binary {
                    operator,
                    left: literal(left.clone()),
                    right: literal(right),
                };
                assert_eq!(
                    expression.validate_in_scope(&[]).ok(),
                    expected,
                    "{expression:?}"
                );
            }
        }
    }
    for name in [
        "not",
        "len",
        "to_string",
        "parse_integer",
        "parse_boolean",
        "optional_string",
        "has_value",
    ] {
        let operator = UnaryOperator::parse(name).unwrap();
        for value in samples() {
            let expected = operator.result_type(value.scalar_type()).map(Type::Scalar);
            // A local avoids the existing literal optional-constructor folding rule.
            let input_type = Type::Scalar(value.scalar_type());
            let expression = Computation::Unary {
                operator,
                value: Box::new(Computation::Local {
                    name: "operand".into(),
                }),
            };
            assert_eq!(
                expression
                    .validate_in_scope(&[("operand".into(), input_type)])
                    .ok(),
                expected,
                "{expression:?}"
            );
        }
    }
}
