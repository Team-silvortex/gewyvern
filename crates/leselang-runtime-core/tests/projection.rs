use std::cell::Cell;
use std::rc::Rc;

use leselang_runtime_core::{
    MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS, OptionalStringValue, ProjectionError,
    ScalarProjectionField, ScalarType, ScalarValue, StringListValue, validate_scalar_projection,
};
use serde::{Deserialize, Serialize};

fn field<Key>(key: Key, value: ScalarValue) -> ScalarProjectionField<Key> {
    ScalarProjectionField { field: key, value }
}

fn validate<Key: PartialEq>(
    fields: &[ScalarProjectionField<Key>],
    schema: &[(Key, ScalarType)],
) -> Result<(), ProjectionError> {
    validate_scalar_projection(fields, schema.iter().map(|(key, ty)| (key, *ty)))
}

fn values() -> [ScalarValue; 6] {
    [
        ScalarValue::Integer(u64::MAX),
        ScalarValue::Boolean(true),
        ScalarValue::String("caption".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec!["z".into(), "".into(), "z".into()])),
    ]
}

#[test]
fn two_unrelated_developer_schemas_use_the_same_scalar_validator() {
    #[derive(PartialEq)]
    enum EditorField {
        Caption,
        Enabled,
    }
    #[derive(PartialEq)]
    enum RobotField {
        Position,
        Note,
        Waypoints,
    }
    let editor = [
        field(EditorField::Caption, ScalarValue::String("ready".into())),
        field(EditorField::Enabled, ScalarValue::Boolean(true)),
    ];
    let editor_schema = [
        (EditorField::Caption, ScalarType::String),
        (EditorField::Enabled, ScalarType::Boolean),
    ];
    assert_eq!(validate(&editor, &editor_schema), Ok(()));
    let robot = [
        field(RobotField::Position, ScalarValue::Integer(42)),
        field(
            RobotField::Note,
            ScalarValue::OptionalString(OptionalStringValue(Some("".into()))),
        ),
        field(
            RobotField::Waypoints,
            ScalarValue::StringList(StringListValue(vec!["A".into(), "B".into()])),
        ),
    ];
    let robot_schema = [
        (RobotField::Position, ScalarType::Integer),
        (RobotField::Note, ScalarType::OptionalString),
        (RobotField::Waypoints, ScalarType::StringList),
    ];
    assert_eq!(validate(&robot, &robot_schema), Ok(()));
}

#[test]
fn exact_ordered_shape_rejects_missing_extra_reordered_duplicate_and_unknown_keys() {
    let schema = [(1, ScalarType::Integer), (2, ScalarType::Boolean)];
    let fields = [
        field(1, ScalarValue::Integer(0)),
        field(2, ScalarValue::Boolean(false)),
    ];
    assert_eq!(validate(&fields, &schema), Ok(()));
    assert_eq!(
        validate(&fields[..1], &schema),
        Err(ProjectionError::FieldCount)
    );
    assert_eq!(
        validate(&fields, &schema[..1]),
        Err(ProjectionError::FieldCount)
    );
    for malformed in [
        [
            field(2, ScalarValue::Boolean(false)),
            field(1, ScalarValue::Integer(0)),
        ],
        [
            field(1, ScalarValue::Integer(0)),
            field(1, ScalarValue::Boolean(false)),
        ],
        [
            field(1, ScalarValue::Integer(0)),
            field(3, ScalarValue::Boolean(false)),
        ],
    ] {
        assert_eq!(
            validate(&malformed, &schema),
            Err(ProjectionError::FieldKey)
        );
    }
    assert_eq!(validate::<u8>(&[], &[]), Ok(()));
    assert_eq!(validate(&fields, &[]), Err(ProjectionError::FieldCount));
    assert_eq!(validate(&[], &schema), Err(ProjectionError::FieldCount));
}

#[test]
fn all_six_types_have_closed_signatures_without_coercion() {
    for actual in values() {
        for expected in values() {
            let result = validate(
                &[field("value", actual.clone())],
                &[("value", expected.scalar_type())],
            );
            assert_eq!(
                result,
                if actual.scalar_type() == expected.scalar_type() {
                    Ok(())
                } else {
                    Err(ProjectionError::FieldType)
                }
            );
        }
    }
}

#[test]
fn text_optional_and_list_values_keep_byte_and_entry_bounds() {
    let maximum = "\u{754c}".repeat(MAX_SCALAR_STRING_BYTES / 3) + "x";
    assert_eq!(maximum.len(), MAX_SCALAR_STRING_BYTES);
    for (value, bounded) in [
        (ScalarValue::String(maximum.clone()), true),
        (ScalarValue::String(maximum.clone() + "x"), false),
        (ScalarValue::OptionalString(OptionalStringValue(None)), true),
        (
            ScalarValue::OptionalString(OptionalStringValue(Some("".into()))),
            true,
        ),
        (
            ScalarValue::OptionalString(OptionalStringValue(Some(maximum.clone()))),
            true,
        ),
        (
            ScalarValue::OptionalString(OptionalStringValue(Some(maximum.clone() + "x"))),
            false,
        ),
        (
            ScalarValue::StringList(StringListValue(vec![maximum.clone(), "".into()])),
            true,
        ),
        (
            ScalarValue::StringList(StringListValue(vec![maximum, "x".into()])),
            false,
        ),
        (
            ScalarValue::StringList(StringListValue(vec!["".into(); MAX_STRING_LIST_ITEMS])),
            true,
        ),
        (
            ScalarValue::StringList(StringListValue(vec!["".into(); MAX_STRING_LIST_ITEMS + 1])),
            false,
        ),
    ] {
        let ty = value.scalar_type();
        let stored = [field(0, value)];
        assert_eq!(
            validate(&stored, &[(0, ty)]),
            if bounded {
                Ok(())
            } else {
                Err(ProjectionError::UnboundedValue)
            }
        );
    }
}

#[test]
fn mutable_data_is_rechecked_and_failed_validation_never_truncates_it() {
    let schema = [("name", ScalarType::String)];
    let mut fields = [field("name", ScalarValue::String("ready".into()))];
    assert_eq!(validate(&fields, &schema), Ok(()));
    fields[0].value = ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1));
    let wire = serde_json::to_string(&fields).unwrap();
    let decoded: Vec<ScalarProjectionField<&str>> = serde_json::from_str(&wire).unwrap();
    assert_eq!(
        validate(&decoded, &schema),
        Err(ProjectionError::UnboundedValue)
    );
    assert_eq!(serde_json::to_string(&decoded).unwrap(), wire);
    assert_eq!(
        validate(&fields, &[("name", ScalarType::Boolean)]),
        Err(ProjectionError::FieldType)
    );
}

#[test]
fn opaque_thread_local_nonclone_keys_and_values_are_borrowed_without_conversion() {
    #[derive(PartialEq)]
    struct Key(Rc<usize>);
    let key = Rc::new(7);
    let schema = [(Key(Rc::clone(&key)), ScalarType::String)];
    let text = String::from("original buffer");
    let pointer = text.as_ptr();
    let fields = [field(Key(Rc::clone(&key)), ScalarValue::String(text))];
    assert_eq!(validate(&fields, &schema), Ok(()));
    assert_eq!(Rc::strong_count(&key), 3);
    assert_eq!(fields[0].value.text().unwrap().as_ptr(), pointer);
    assert_eq!(validate(&fields, &[]), Err(ProjectionError::FieldCount));
    assert_eq!(fields[0].value.text(), Some("original buffer"));
}

#[test]
fn nonclone_schema_is_not_collected_counted_or_queried_for_size_hints() {
    struct Schema<'a> {
        keys: &'a [u8],
        calls: &'a Cell<usize>,
    }
    impl<'a> Iterator for Schema<'a> {
        type Item = (&'a u8, ScalarType);
        fn next(&mut self) -> Option<Self::Item> {
            let count = self.calls.get();
            self.calls.set(count + 1);
            Some((&self.keys[count % self.keys.len()], ScalarType::Integer))
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            panic!("must not reserve or count schema")
        }
    }
    let calls = Cell::new(0);
    let keys = [1, 2, 3];
    let fields = [
        field(1, ScalarValue::Integer(1)),
        field(2, ScalarValue::Integer(2)),
    ];
    let schema = Schema {
        keys: &keys,
        calls: &calls,
    };
    assert_eq!(
        validate_scalar_projection(&fields, schema),
        Err(ProjectionError::FieldCount)
    );
    assert_eq!(calls.get(), fields.len() + 1);
}

#[test]
fn trusted_schema_callback_unwind_propagates_without_changing_fields() {
    let fields = [
        field(1, ScalarValue::String("private".into())),
        field(2, ScalarValue::String("secret".into())),
    ];
    let keys = [1, 2];
    let mut calls = 0;
    let before = serde_json::to_vec(&fields).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        validate_scalar_projection(
            &fields,
            std::iter::from_fn(|| {
                calls += 1;
                assert_eq!(calls, 1, "trusted host callback panic");
                Some((&keys[0], ScalarType::String))
            }),
        )
    }));
    assert!(result.is_err());
    assert_eq!(calls, 2);
    assert_eq!(serde_json::to_vec(&fields).unwrap(), before);
    assert_eq!(
        validate(&fields, &[(1, ScalarType::String), (2, ScalarType::String)]),
        Ok(())
    );
}

#[test]
fn generic_field_wire_is_closed_and_preserves_scalar_payloads() {
    #[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "snake_case")]
    enum Field {
        Caption,
    }
    for scalar in values() {
        let stored = field(Field::Caption, scalar);
        let wire = serde_json::to_string(&stored).unwrap();
        let restored: ScalarProjectionField<Field> = serde_json::from_str(&wire).unwrap();
        assert_eq!(restored, stored);
        assert_eq!(serde_json::to_string(&restored).unwrap(), wire);
    }
    for invalid in [
        r#"{"field":"caption","value":{"kind":"none"},"extra":true}"#,
        r#"{"field":"caption","field":"caption","value":{"kind":"none"}}"#,
        r#"{"field":"unknown","value":{"kind":"none"}}"#,
        r#"{"field":"caption"}"#,
        r#"{"field":"caption","value":{"kind":"optional_string"}}"#,
        r#"{"field":"caption","value":{"kind":"none","extra":true}}"#,
    ] {
        assert!(serde_json::from_str::<ScalarProjectionField<Field>>(invalid).is_err());
    }
    let stored = field(Field::Caption, ScalarValue::None);
    assert_eq!(
        serde_json::to_string(&stored).unwrap(),
        r#"{"field":"caption","value":{"kind":"none"}}"#
    );
}

#[test]
fn fixed_diagnostics_never_format_field_keys_or_payloads() {
    #[derive(PartialEq)]
    struct Key(&'static str);
    impl std::fmt::Debug for Key {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("private key")
        }
    }
    let fields = [field(Key("private"), ScalarValue::String("secret".into()))];
    let error = validate(&fields, &[(Key("other"), ScalarType::String)]).unwrap_err();
    assert_eq!(
        error.to_string(),
        "projection field key does not match schema"
    );
    assert_eq!(format!("{error:?}"), "FieldKey");
    for error in [
        ProjectionError::FieldCount,
        ProjectionError::FieldKey,
        ProjectionError::FieldType,
        ProjectionError::UnboundedValue,
    ] {
        let text = error.to_string();
        assert!(!text.contains("private") && !text.contains("secret"));
        assert!(text.len() < 64);
    }
}
