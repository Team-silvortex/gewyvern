use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_command::{
    StructuredField, Value, ValueBoundsError, ValueValidationLimits, validate_value_items,
};
use leselang_host_contract::Revision;
use leserpent_hir::computation::{ScalarValue, StringListValue};

const LIMITS: ValueValidationLimits = ValueValidationLimits {
    max_depth: 4,
    max_fields: 4,
    max_query_items: 4,
};

fn leaf(valid: bool) -> Value {
    Value::UiFocus {
        node_id: if valid { "a" } else { "private invalid node" }.into(),
    }
}
fn group(fields: Vec<(&str, Value)>) -> Value {
    Value::Structured {
        fields: fields
            .into_iter()
            .map(|(name, value)| StructuredField {
                name: name.into(),
                value: Box::new(value),
            })
            .collect(),
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

#[test]
fn value_tree_policy_preserves_depth_width_and_zero_item_boundaries() {
    let empty_query = Value::RuntimeList {
        revision: Revision(7),
        runtimes: vec![],
    };
    let empty_list = Value::Scalar {
        value: ScalarValue::StringList(StringListValue(vec![])),
    };
    let value = group(vec![
        ("a", empty_query),
        ("b", empty_list),
        (
            "c",
            Value::Scalar {
                value: ScalarValue::StringList(StringListValue(vec!["a".into(), "b".into()])),
            },
        ),
        ("d", group(vec![("a", leaf(true))])),
    ]);
    let before = serde_json::to_vec(&value).unwrap();
    assert_eq!(validate_value_items(&value, 2, LIMITS, valid_name), Ok(4));
    assert_eq!(
        validate_value_items(&value, 3, LIMITS, valid_name),
        Err(ValueBoundsError::Nesting)
    );
    for width in [0, 1, 3, 4, 5] {
        let limits = ValueValidationLimits {
            max_fields: width,
            ..LIMITS
        };
        assert_eq!(
            validate_value_items(&value, 0, limits, valid_name),
            if width >= 4 {
                Ok(4)
            } else {
                Err(ValueBoundsError::Value)
            }
        );
    }
    assert_eq!(
        validate_value_items(&group(vec![]), 0, LIMITS, valid_name),
        Err(ValueBoundsError::Value)
    );
    assert_eq!(
        validate_value_items(&leaf(false), usize::MAX, LIMITS, valid_name),
        Err(ValueBoundsError::Nesting)
    );
    assert_eq!(serde_json::to_vec(&value).unwrap(), before);
}

#[test]
fn depth_offset_overflow_refuses_after_names_without_clamping() {
    let limits = ValueValidationLimits {
        max_depth: usize::MAX,
        ..LIMITS
    };
    assert_eq!(
        validate_value_items(&leaf(true), usize::MAX, limits, valid_name),
        Ok(1)
    );
    let value = group(vec![("a", leaf(true))]);
    assert_eq!(
        validate_value_items(&value, usize::MAX - 1, limits, valid_name),
        Ok(1)
    );
    let mut visited = vec![];
    assert_eq!(
        validate_value_items(&value, usize::MAX, limits, |name| {
            visited.push(name.to_owned());
            true
        }),
        Err(ValueBoundsError::Nesting)
    );
    assert_eq!(visited, ["a"]);
    assert_eq!(
        validate_value_items(&value, usize::MAX, limits, |_| false),
        Err(ValueBoundsError::Fields)
    );
}

#[test]
fn declaration_order_and_local_duplicate_scopes_preserve_refusal_precedence() {
    for (value, depth, expected, visited) in [
        (
            group(vec![("a", leaf(false)), ("a", leaf(true))]),
            0,
            Err(ValueBoundsError::Value),
            vec!["a"],
        ),
        (
            group(vec![("a", leaf(true)), ("a", leaf(false))]),
            0,
            Err(ValueBoundsError::Fields),
            vec!["a", "a"],
        ),
        (
            group(vec![("bad name", leaf(false))]),
            4,
            Err(ValueBoundsError::Fields),
            vec!["bad name"],
        ),
        (
            group(vec![("a", leaf(false))]),
            4,
            Err(ValueBoundsError::Nesting),
            vec!["a"],
        ),
        (
            group(vec![
                ("a", group(vec![("x", leaf(true))])),
                ("b", group(vec![("x", leaf(true))])),
            ]),
            0,
            Ok(2),
            vec!["a", "x", "b", "x"],
        ),
        (
            group(vec![("a", leaf(false))]),
            5,
            Err(ValueBoundsError::Nesting),
            vec![],
        ),
    ] {
        let before = serde_json::to_vec(&value).unwrap();
        let mut names = vec![];
        assert_eq!(
            validate_value_items(&value, depth, LIMITS, |name| {
                names.push(name.to_owned());
                valid_name(name)
            }),
            expected
        );
        assert_eq!(names, visited);
        assert_eq!(serde_json::to_vec(&value).unwrap(), before);
    }
}

#[test]
fn borrowed_names_payloads_and_native_unwind_do_not_mutate_input_or_retry_hooks() {
    let value = group(vec![("a", leaf(true)), ("b", leaf(true))]);
    let Value::Structured { fields } = &value else {
        panic!()
    };
    let addresses = fields
        .iter()
        .map(|field| field.name.as_ptr())
        .collect::<Vec<_>>();
    let before = serde_json::to_vec(&value).unwrap();
    let mut calls = vec![];
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = validate_value_items(&value, 0, LIMITS, |name| {
                calls.push(name.as_ptr());
                assert!(calls.len() < 2, "injected native name hook unwind");
                true
            });
        }))
        .is_err()
    );
    assert_eq!(calls, addresses);
    assert_eq!(serde_json::to_vec(&value).unwrap(), before);
    assert_eq!(validate_value_items(&value, 0, LIMITS, valid_name), Ok(2));
    for error in [
        ValueBoundsError::Nesting,
        ValueBoundsError::Fields,
        ValueBoundsError::Value,
    ] {
        assert!(!format!("{error}: {error:?}").contains("private invalid node"));
    }
}

#[test]
fn deep_explicit_policy_uses_no_recursive_rust_stack() {
    // Teardown also moves children out iteratively; Value's derived Drop is not this API.
    let mut value = leaf(true);
    for _ in 0..4096 {
        value = group(vec![("a", value)]);
    }
    let limits = ValueValidationLimits {
        max_depth: 4096,
        max_fields: 1,
        max_query_items: 0,
    };
    assert_eq!(validate_value_items(&value, 0, limits, valid_name), Ok(1));
    assert_eq!(
        validate_value_items(&value, 1, limits, valid_name),
        Err(ValueBoundsError::Nesting)
    );
    let mut calls = 0;
    assert_eq!(
        validate_value_items(
            &value,
            0,
            ValueValidationLimits {
                max_fields: 0,
                ..limits
            },
            |_| {
                calls += 1;
                true
            }
        ),
        Err(ValueBoundsError::Value)
    );
    assert_eq!(calls, 0);
    while let Value::Structured { mut fields } = value {
        value = *fields.pop().unwrap().value;
    }
}
