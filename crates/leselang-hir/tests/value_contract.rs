use leselang_hir::computation::{
    BinaryOperator, Computation, MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS,
    OptionalStringValue, ScalarType, ScalarValue, StringListValue,
};
use leselang_hir::{Effect, canonical_source, lower};
use leselang_runtime_core as core;
use leselang_syntax::parse;

#[test]
fn existing_hir_imports_are_the_same_core_types_not_conversion_wrappers() {
    let value = core::ScalarValue::String("same allocation".into());
    let pointer = value.text().unwrap().as_ptr();
    let hir: ScalarValue = value;
    let core: core::ScalarValue = hir;
    assert_eq!(core.text().unwrap().as_ptr(), pointer);
    let ty: ScalarType = core::ScalarType::String;
    assert_eq!(ty, core.scalar_type());
    let list: StringListValue = core::StringListValue(vec!["".into()]);
    let optional: OptionalStringValue = core::OptionalStringValue(None);
    assert!(ScalarValue::StringList(list).is_bounded());
    assert_eq!(ScalarValue::OptionalString(optional).text(), None);
    assert_eq!(MAX_SCALAR_STRING_BYTES, core::MAX_SCALAR_STRING_BYTES);
    assert_eq!(MAX_STRING_LIST_ITEMS, core::MAX_STRING_LIST_ITEMS);
}

#[test]
fn unchecked_core_values_still_fail_hir_ingress_before_shortening_or_execution() {
    for value in [
        core::ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
        core::ScalarValue::OptionalString(core::OptionalStringValue(Some(
            "x".repeat(MAX_SCALAR_STRING_BYTES + 1),
        ))),
        core::ScalarValue::StringList(core::StringListValue(vec![
            String::new();
            MAX_STRING_LIST_ITEMS + 1
        ])),
    ] {
        let expression = Computation::Literal { value };
        assert!(expression.validate_structure().is_err());
        assert!(
            canonical_source(&Effect::Compute {
                expression: Box::new(expression)
            })
            .is_err()
        );
    }
}

fn literal_tree(depth: usize) -> Computation {
    if depth == 0 {
        return Computation::Literal {
            value: core::ScalarValue::StringList(core::StringListValue(vec![
                String::new();
                MAX_STRING_LIST_ITEMS
            ])),
        };
    }
    Computation::Binary {
        operator: BinaryOperator::Eq,
        left: Box::new(literal_tree(depth - 1)),
        right: Box::new(literal_tree(depth - 1)),
    }
}

#[test]
fn literal_constructor_node_cost_remains_hir_owned_after_value_extraction() {
    assert!(literal_tree(3).validate_structure().is_ok());
    assert!(literal_tree(4).validate_structure().is_err());
}

fn helper_tree(depth: usize) -> String {
    if depth == 0 {
        return "parts()".into();
    }
    format!(
        "eq(left: {}, right: {})",
        helper_tree(depth - 1),
        helper_tree(depth - 1)
    )
}

#[test]
fn helper_expansion_reserves_original_list_constructor_cost_before_constant_folding() {
    let entries = (0..MAX_STRING_LIST_ITEMS)
        .map(|index| format!("i{index}: \"\""))
        .collect::<Vec<_>>()
        .join(", ");
    for (depth, accepted) in [(3, true), (4, false)] {
        let source = format!(
            "fn parts() = strings({entries})\nfn main() = {}",
            helper_tree(depth)
        );
        assert_eq!(lower(&parse(&source)).is_ok(), accepted);
    }
}
