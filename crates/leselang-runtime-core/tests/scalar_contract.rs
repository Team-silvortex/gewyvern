use leselang_runtime_core::{
    CalculationFailure, Fuel, NamedArgumentError, NamedParameter, OptionalStringValue,
    ScalarContractError, ScalarType, ScalarTypeSet, ScalarValue, StringListValue,
    validate_named_arguments,
};

const TYPES: [ScalarType; 6] = [
    ScalarType::Integer,
    ScalarType::Boolean,
    ScalarType::String,
    ScalarType::None,
    ScalarType::OptionalString,
    ScalarType::StringList,
];

fn samples() -> [ScalarValue; 6] {
    [
        ScalarValue::Integer(u64::MAX),
        ScalarValue::Boolean(false),
        ScalarValue::String("false".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec![])),
    ]
}

#[test]
fn const_contracts_have_explicit_deny_all_single_and_union_semantics() {
    const EMPTY: ScalarTypeSet = ScalarTypeSet::empty();
    const TEXT: ScalarTypeSet = ScalarTypeSet::only(ScalarType::String);
    const NULLABLE: ScalarTypeSet = TEXT.with(ScalarType::None);
    const PRESENT: bool = NULLABLE.contains(ScalarType::String);
    assert_eq!(EMPTY.iter().count(), 0);
    assert_eq!(TEXT.iter().collect::<Vec<_>>(), [ScalarType::String]);
    assert!(std::hint::black_box(PRESENT));
    assert!(!NULLABLE.contains(ScalarType::OptionalString));
    for value in samples() {
        assert_eq!(
            EMPTY.validate(&value),
            Err(ScalarContractError::TypeMismatch)
        );
    }
}

#[test]
fn every_subset_checks_all_six_types_without_coercion_or_implicit_acceptance() {
    let values = samples();
    for bits in 0u8..64 {
        let mut contract = ScalarTypeSet::empty();
        let mut expected = Vec::new();
        for (index, ty) in TYPES.into_iter().enumerate() {
            if bits & (1 << index) != 0 {
                contract = contract.with(ty);
                expected.push(ty);
            }
        }
        assert_eq!(contract.iter().collect::<Vec<_>>(), expected);
        for (index, value) in values.iter().enumerate() {
            let accepted = bits & (1 << index) != 0;
            assert_eq!(contract.contains(value.scalar_type()), accepted);
            assert_eq!(
                contract.validate(value),
                if accepted {
                    Ok(())
                } else {
                    Err(ScalarContractError::TypeMismatch)
                }
            );
        }
    }
}

#[test]
fn canonical_iteration_and_debug_ignore_insertion_order_and_duplicate_tags() {
    let contract = TYPES
        .into_iter()
        .rev()
        .fold(ScalarTypeSet::empty(), |set, ty| set.with(ty).with(ty));
    assert_eq!(contract.iter().collect::<Vec<_>>(), TYPES);
    let mut iter = contract.iter();
    assert_eq!(iter.next(), Some(ScalarType::Integer));
    assert_eq!(iter.next_back(), Some(ScalarType::StringList));
    assert_eq!(iter.collect::<Vec<_>>(), TYPES[1..5]);
    let mut empty = ScalarTypeSet::empty().iter();
    for _ in 0..10 {
        assert_eq!(empty.next(), None);
        assert_eq!(empty.next_back(), None);
    }
    assert_eq!(
        format!(
            "{:?}",
            ScalarTypeSet::only(ScalarType::None).with(ScalarType::String)
        ),
        "ScalarTypeSet {String, None}"
    );
}

#[test]
fn wrong_type_precedes_bounds_even_for_unchecked_oversized_values() {
    for value in [
        ScalarValue::String("x".repeat(4097)),
        ScalarValue::OptionalString(OptionalStringValue(Some("x".repeat(4097)))),
        ScalarValue::StringList(StringListValue(vec![String::new(); 65])),
    ] {
        assert_eq!(
            ScalarTypeSet::only(ScalarType::Integer).validate(&value),
            Err(ScalarContractError::TypeMismatch)
        );
        assert_eq!(
            ScalarTypeSet::only(value.scalar_type()).validate(&value),
            Err(ScalarContractError::UnboundedValue)
        );
    }
}

#[test]
fn plain_text_bounds_are_utf8_bytes_not_character_counts() {
    let contract = ScalarTypeSet::only(ScalarType::String);
    for text in [
        String::new(),
        "x".repeat(4096),
        "\u{754c}".repeat(1365) + "x",
    ] {
        contract.validate(&ScalarValue::String(text)).unwrap();
    }
    for text in ["x".repeat(4097), "\u{754c}".repeat(1366)] {
        assert_eq!(
            contract.validate(&ScalarValue::String(text)),
            Err(ScalarContractError::UnboundedValue)
        );
    }
}

#[test]
fn none_absent_optional_and_present_empty_text_remain_distinct_types_and_values() {
    let optional = ScalarTypeSet::only(ScalarType::OptionalString);
    assert_eq!(
        optional.validate(&ScalarValue::None),
        Err(ScalarContractError::TypeMismatch)
    );
    assert_eq!(
        optional.validate(&ScalarValue::String(String::new())),
        Err(ScalarContractError::TypeMismatch)
    );
    for value in [None, Some(String::new()), Some("x".repeat(4096))] {
        optional
            .validate(&ScalarValue::OptionalString(OptionalStringValue(value)))
            .unwrap();
    }
    assert_eq!(
        optional.validate(&ScalarValue::OptionalString(OptionalStringValue(Some(
            "x".repeat(4097)
        )))),
        Err(ScalarContractError::UnboundedValue)
    );
    let alternatives = optional.with(ScalarType::None).with(ScalarType::String);
    for value in [ScalarValue::None, ScalarValue::String(String::new())] {
        alternatives.validate(&value).unwrap();
    }
}

#[test]
fn lists_keep_count_and_aggregate_utf8_byte_limits_including_empty_entries() {
    let contract = ScalarTypeSet::only(ScalarType::StringList);
    for items in [
        vec![],
        vec![String::new(); 64],
        vec!["x".repeat(64); 64],
        vec!["\u{754c}".repeat(1365), "x".into()],
    ] {
        contract
            .validate(&ScalarValue::StringList(StringListValue(items)))
            .unwrap();
    }
    for items in [
        vec![String::new(); 65],
        vec!["x".repeat(65); 64],
        vec!["\u{754c}".repeat(1365), "xx".into()],
    ] {
        assert_eq!(
            contract.validate(&ScalarValue::StringList(StringListValue(items))),
            Err(ScalarContractError::UnboundedValue)
        );
    }
}

#[test]
fn preflight_borrows_buffers_without_mutation_reordering_or_wire_changes() {
    for value in [
        ScalarValue::String("private".repeat(586)),
        ScalarValue::OptionalString(OptionalStringValue(Some("private".into()))),
        ScalarValue::StringList(StringListValue(vec!["last".into(), "first".into()])),
    ] {
        let wire = serde_json::to_vec(&value).unwrap();
        let pointer = value.text().map(str::as_ptr);
        let list_pointer = match &value {
            ScalarValue::StringList(items) => Some((items.0.as_ptr(), items.0[0].as_ptr())),
            _ => None,
        };
        let contract = ScalarTypeSet::only(value.scalar_type());
        for _ in 0..3 {
            assert_eq!(contract.validate(&value).is_ok(), value.is_bounded());
            assert_eq!(
                ScalarTypeSet::empty().validate(&value),
                Err(ScalarContractError::TypeMismatch)
            );
            assert_eq!(serde_json::to_vec(&value).unwrap(), wire);
            assert_eq!(value.text().map(str::as_ptr), pointer);
            if let ScalarValue::StringList(items) = &value {
                assert_eq!(Some((items.0.as_ptr(), items.0[0].as_ptr())), list_pointer);
            }
        }
    }
}

#[test]
fn success_does_not_certify_later_value_or_descriptor_mutations() {
    let mut contract = ScalarTypeSet::only(ScalarType::String);
    let mut value = ScalarValue::String("ok".into());
    contract.validate(&value).unwrap();
    let ScalarValue::String(text) = &mut value else {
        unreachable!()
    };
    text.push_str(&"x".repeat(4095));
    assert_eq!(
        contract.validate(&value),
        Err(ScalarContractError::UnboundedValue)
    );
    value = ScalarValue::None;
    assert_eq!(
        contract.validate(&value),
        Err(ScalarContractError::TypeMismatch)
    );
    contract = contract.with(ScalarType::None);
    contract.validate(&value).unwrap();
    contract = ScalarTypeSet::empty();
    assert_eq!(
        contract.validate(&value),
        Err(ScalarContractError::TypeMismatch)
    );
}

#[test]
fn unrelated_host_schemas_share_type_preflight_but_own_keys_presence_and_domains() {
    #[derive(PartialEq)]
    enum GuiKey {
        Caption,
    }
    let gui = [NamedParameter::required(
        GuiKey::Caption,
        ScalarTypeSet::only(ScalarType::String).with(ScalarType::None),
    )];
    validate_named_arguments(&[GuiKey::Caption], &gui).unwrap();
    gui[0].domain.validate(&ScalarValue::None).unwrap();
    assert_eq!(
        validate_named_arguments(&[], &gui),
        Err(NamedArgumentError::MissingArgument { parameter_index: 0 })
    );

    struct DeviceDomain {
        types: ScalarTypeSet,
        maximum: u64,
    }
    let device = [NamedParameter::required(
        17u32,
        DeviceDomain {
            types: ScalarTypeSet::only(ScalarType::Integer),
            maximum: 10,
        },
    )];
    validate_named_arguments(&[17], &device).unwrap();
    let value = ScalarValue::Integer(11);
    device[0].domain.types.validate(&value).unwrap();
    let ScalarValue::Integer(position) = value else {
        unreachable!()
    };
    assert!(position > device[0].domain.maximum);
    assert_eq!(
        device[0]
            .domain
            .types
            .validate(&ScalarValue::String("9".into())),
        Err(ScalarContractError::TypeMismatch)
    );
}

#[test]
fn closed_errors_are_payload_free_and_do_not_enter_calculation_recovery() {
    for (error, message) in [
        (
            ScalarContractError::TypeMismatch,
            "scalar type is not accepted by contract",
        ),
        (
            ScalarContractError::UnboundedValue,
            "scalar exceeds language bounds",
        ),
    ] {
        assert_eq!(error.to_string(), message);
        assert!(!format!("{error:?}: {error}").contains("private"));
        let failure: CalculationFailure<ScalarContractError> = error.into();
        assert!(!failure.is_recoverable());
        assert!(matches!(failure, CalculationFailure::External(actual) if actual == error));
    }
}

#[test]
fn copying_type_metadata_neither_grants_nor_charges_host_execution_fuel() {
    let mut fuel = Fuel::new(7);
    fuel.charge(2).unwrap();
    let contract = ScalarTypeSet::only(ScalarType::String);
    let copied = contract;
    for descriptor in [contract, copied, copied.with(ScalarType::None)] {
        descriptor
            .validate(&ScalarValue::String("ready".into()))
            .unwrap();
        assert_eq!(
            descriptor.validate(&ScalarValue::Boolean(true)),
            Err(ScalarContractError::TypeMismatch)
        );
        assert_eq!(fuel.remaining(), 5);
    }
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ScalarTypeSet>();
    assert_send_sync::<ScalarContractError>();
}
