use std::{cell::Cell, panic::AssertUnwindSafe, rc::Rc};

use leselang_runtime_core::{
    ArgumentTypeError, MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS, NamedArgumentError,
    NamedParameter, OperationCatalog, OperationCatalogError, OperationCatalogLimits,
    OperationSchema, OperationTypeError, OptionalStringValue, ScalarArgumentDomain,
    ScalarArgumentType, ScalarType, ScalarTypeSet, ScalarValue, StringListValue,
    check_argument_type,
};

struct Domain {
    types: ScalarTypeSet,
    type_reads: Cell<usize>,
    literal_reads: Cell<usize>,
    accepted: bool,
}

impl Domain {
    fn new(types: ScalarTypeSet, accepted: bool) -> Self {
        Self {
            types,
            type_reads: Cell::new(0),
            literal_reads: Cell::new(0),
            accepted,
        }
    }
}

impl ScalarArgumentDomain for Domain {
    fn scalar_types(&self) -> ScalarTypeSet {
        self.type_reads.set(self.type_reads.get() + 1);
        self.types
    }

    fn accepts_literal(&self, _: &ScalarValue) -> bool {
        self.literal_reads.set(self.literal_reads.get() + 1);
        self.accepted
    }
}

#[test]
fn every_scalar_alternative_matrix_matches_for_literals_and_dynamic_facts() {
    let values = [
        ScalarValue::Integer(4),
        ScalarValue::Boolean(false),
        ScalarValue::String("ready".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec!["a".into()])),
    ];
    for bits in 0u8..64 {
        let types =
            values
                .iter()
                .enumerate()
                .fold(ScalarTypeSet::empty(), |types, (index, value)| {
                    if bits & (1 << index) != 0 {
                        types.with(value.scalar_type())
                    } else {
                        types
                    }
                });
        for (index, value) in values.iter().enumerate() {
            let expected = if bits & (1 << index) != 0 {
                Ok(())
            } else {
                Err(ArgumentTypeError::TypeMismatch)
            };
            assert_eq!(
                check_argument_type(&types, ScalarArgumentType::literal(value)),
                expected
            );
            assert_eq!(
                check_argument_type(
                    &types,
                    ScalarArgumentType::expression(Some(value.scalar_type()), true)
                ),
                expected
            );
        }
    }
}

#[test]
fn pure_scalar_preflight_precedes_every_native_domain_method() {
    let domain = Domain::new(ScalarTypeSet::only(ScalarType::String), false);
    for (facts, expected) in [
        (
            ScalarArgumentType::expression(None, false),
            ArgumentTypeError::Impure,
        ),
        (
            ScalarArgumentType::expression(Some(ScalarType::String), false),
            ArgumentTypeError::Impure,
        ),
        (
            ScalarArgumentType::expression(None, true),
            ArgumentTypeError::NonScalar,
        ),
    ] {
        assert_eq!(check_argument_type(&domain, facts), Err(expected));
    }
    assert_eq!(domain.type_reads.get(), 0);
    assert_eq!(domain.literal_reads.get(), 0);
    assert_eq!(
        check_argument_type(
            &domain,
            ScalarArgumentType::expression(Some(ScalarType::Boolean), true)
        ),
        Err(ArgumentTypeError::TypeMismatch)
    );
    assert_eq!(domain.type_reads.get(), 1);
    assert_eq!(domain.literal_reads.get(), 0);
}

#[test]
fn accepted_type_then_literal_consistency_then_bounds_precede_domain_check() {
    let domain = Domain::new(ScalarTypeSet::only(ScalarType::String), false);
    let large = ScalarValue::String("private".repeat(600));
    let optional = ScalarValue::OptionalString(OptionalStringValue(None));
    for (facts, expected) in [
        (
            ScalarArgumentType::literal(&optional),
            ArgumentTypeError::TypeMismatch,
        ),
        (
            ScalarArgumentType {
                scalar_type: Some(ScalarType::Boolean),
                is_pure: true,
                literal: Some(&large),
            },
            ArgumentTypeError::TypeMismatch,
        ),
        (
            ScalarArgumentType {
                scalar_type: Some(ScalarType::String),
                is_pure: true,
                literal: Some(&optional),
            },
            ArgumentTypeError::InconsistentLiteralType,
        ),
        (
            ScalarArgumentType::literal(&large),
            ArgumentTypeError::UnboundedLiteral,
        ),
    ] {
        assert_eq!(check_argument_type(&domain, facts), Err(expected));
    }
    assert_eq!(domain.literal_reads.get(), 0);
    let small = ScalarValue::String("ready".into());
    assert_eq!(
        check_argument_type(&domain, ScalarArgumentType::literal(&small)),
        Err(ArgumentTypeError::InvalidLiteralDomain)
    );
    assert_eq!(domain.literal_reads.get(), 1);
}

#[test]
fn all_container_bounds_are_enforced_before_native_literal_inspection() {
    let values = [
        ScalarValue::String("a".repeat(MAX_SCALAR_STRING_BYTES + 1)),
        ScalarValue::OptionalString(OptionalStringValue(Some(
            "a".repeat(MAX_SCALAR_STRING_BYTES + 1),
        ))),
        ScalarValue::StringList(StringListValue(vec![
            String::new();
            MAX_STRING_LIST_ITEMS + 1
        ])),
        ScalarValue::StringList(StringListValue(vec![
            "a".repeat(MAX_SCALAR_STRING_BYTES),
            "b".into(),
        ])),
    ];
    for value in values {
        let domain = Domain::new(ScalarTypeSet::only(value.scalar_type()), true);
        assert_eq!(
            check_argument_type(&domain, ScalarArgumentType::literal(&value)),
            Err(ArgumentTypeError::UnboundedLiteral)
        );
        assert_eq!(domain.literal_reads.get(), 0);
    }
    for value in [
        ScalarValue::String("a".repeat(MAX_SCALAR_STRING_BYTES)),
        ScalarValue::OptionalString(OptionalStringValue(Some(
            "a".repeat(MAX_SCALAR_STRING_BYTES),
        ))),
        ScalarValue::StringList(StringListValue(vec![String::new(); MAX_STRING_LIST_ITEMS])),
    ] {
        let domain = Domain::new(ScalarTypeSet::only(value.scalar_type()), true);
        check_argument_type(&domain, ScalarArgumentType::literal(&value)).unwrap();
        assert_eq!(domain.literal_reads.get(), 1);
    }
}

#[test]
fn dynamic_type_success_is_not_a_value_domain_or_later_mutation_certificate() {
    let domain = Domain::new(ScalarTypeSet::only(ScalarType::String), false);
    check_argument_type(
        &domain,
        ScalarArgumentType::expression(Some(ScalarType::String), true),
    )
    .unwrap();
    assert_eq!(domain.literal_reads.get(), 0);
    let mut value = ScalarValue::String("later-invalid".into());
    assert_eq!(
        check_argument_type(&domain, ScalarArgumentType::literal(&value)),
        Err(ArgumentTypeError::InvalidLiteralDomain)
    );
    let types = ScalarTypeSet::only(ScalarType::String);
    check_argument_type(&types, ScalarArgumentType::literal(&value)).unwrap();
    value = ScalarValue::String("a".repeat(MAX_SCALAR_STRING_BYTES + 1));
    assert_eq!(
        check_argument_type(&types, ScalarArgumentType::literal(&value)),
        Err(ArgumentTypeError::UnboundedLiteral)
    );
}

#[test]
fn mismatched_fact_count_and_excess_names_precede_native_key_comparison() {
    struct NeverCompare;
    impl PartialEq for NeverCompare {
        fn eq(&self, _: &Self) -> bool {
            panic!("key comparison must not run")
        }
    }
    let parameters = [NamedParameter::required(
        NeverCompare,
        Domain::new(ScalarTypeSet::empty(), false),
    )];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    assert_eq!(
        schema.check_argument_types(&[NeverCompare], &[]),
        Err(OperationTypeError::ArgumentCountMismatch)
    );
    assert_eq!(
        schema.check_argument_types(
            &[NeverCompare, NeverCompare],
            &[ScalarArgumentType::expression(None, false); 2]
        ),
        Err(OperationTypeError::Names(
            NamedArgumentError::TooManyArguments
        ))
    );
    assert_eq!(parameters[0].domain.type_reads.get(), 0);
}

#[test]
fn complete_named_shape_precedes_all_argument_type_and_domain_checks() {
    let parameters = [
        NamedParameter::required("first", Domain::new(ScalarTypeSet::empty(), false)),
        NamedParameter::optional("last", Domain::new(ScalarTypeSet::empty(), false)),
    ];
    let schema = OperationSchema {
        key: "native",
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let invalid = ScalarArgumentType::expression(None, false);
    for (names, expected) in [
        (
            vec!["first", "first"],
            NamedArgumentError::DuplicateArgument { index: 1 },
        ),
        (
            vec!["first", "private-unknown"],
            NamedArgumentError::UnknownArgument { index: 1 },
        ),
        (
            vec!["last"],
            NamedArgumentError::MissingArgument { parameter_index: 0 },
        ),
    ] {
        assert_eq!(
            schema.check_argument_types(&names, &vec![invalid; names.len()]),
            Err(OperationTypeError::Names(expected))
        );
    }
    for parameter in &parameters {
        assert_eq!(parameter.domain.type_reads.get(), 0);
        assert_eq!(parameter.domain.literal_reads.get(), 0);
    }
    let duplicate = [
        NamedParameter::optional("same", Domain::new(ScalarTypeSet::empty(), false)),
        NamedParameter::optional("same", Domain::new(ScalarTypeSet::empty(), false)),
    ];
    let schema = OperationSchema {
        key: (),
        parameters: &duplicate,
        result: (),
        required_capability: (),
    };
    assert_eq!(
        schema.check_argument_types(&[], &[]),
        Err(OperationTypeError::Names(
            NamedArgumentError::DuplicateParameter { index: 1 }
        ))
    );
    assert_eq!(duplicate[0].domain.type_reads.get(), 0);
}

#[test]
fn declaration_order_reports_original_indices_and_skips_optional_parameters() {
    let parameters = [
        NamedParameter::optional("omitted", Domain::new(ScalarTypeSet::empty(), false)),
        NamedParameter::required(
            "target",
            Domain::new(ScalarTypeSet::only(ScalarType::String), true),
        ),
        NamedParameter::optional(
            "caption",
            Domain::new(ScalarTypeSet::only(ScalarType::String), false),
        ),
    ];
    let schema = OperationSchema {
        key: "caption",
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let text = ScalarValue::String("private".into());
    let facts = [
        ScalarArgumentType::literal(&text),
        ScalarArgumentType::expression(Some(ScalarType::Boolean), true),
    ];
    assert_eq!(
        schema.check_argument_types(&["caption", "target"], &facts),
        Err(OperationTypeError::Argument {
            parameter_index: 1,
            argument_index: 1,
            error: ArgumentTypeError::TypeMismatch
        })
    );
    assert_eq!(parameters[0].domain.type_reads.get(), 0);
    assert_eq!(parameters[2].domain.type_reads.get(), 0);
    assert_eq!(
        schema.check_argument_types(
            &["caption", "target"],
            &[ScalarArgumentType::literal(&text); 2]
        ),
        Err(OperationTypeError::Argument {
            parameter_index: 2,
            argument_index: 0,
            error: ArgumentTypeError::InvalidLiteralDomain
        })
    );
    assert_eq!(parameters[1].domain.literal_reads.get(), 1);
}

#[test]
fn required_presence_does_not_imply_non_null_and_omission_never_constructs_a_default() {
    let nullable = ScalarTypeSet::only(ScalarType::String)
        .with(ScalarType::None)
        .with(ScalarType::OptionalString);
    let parameters = [
        NamedParameter::required("value", nullable),
        NamedParameter::optional("extra", ScalarTypeSet::empty()),
    ];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    for value in [
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::OptionalString(OptionalStringValue(Some(String::new()))),
    ] {
        schema
            .check_argument_types(&["value"], &[ScalarArgumentType::literal(&value)])
            .unwrap();
    }
    assert_eq!(
        schema.check_argument_types(&[], &[]),
        Err(OperationTypeError::Names(
            NamedArgumentError::MissingArgument { parameter_index: 0 }
        ))
    );
    let empty: [NamedParameter<&str, ScalarTypeSet>; 0] = [];
    let schema = OperationSchema {
        key: (),
        parameters: &empty,
        result: (),
        required_capability: (),
    };
    schema.check_argument_types(&[], &[]).unwrap();
}

#[test]
fn unrelated_gui_and_device_catalogs_share_typed_signatures_without_product_types() {
    #[derive(PartialEq)]
    enum GuiParameter {
        Target,
        Caption,
    }
    let gui_parameters = [
        NamedParameter::required(
            GuiParameter::Target,
            ScalarTypeSet::only(ScalarType::String),
        ),
        NamedParameter::optional(
            GuiParameter::Caption,
            ScalarTypeSet::only(ScalarType::OptionalString),
        ),
    ];
    let gui_schemas = [OperationSchema {
        key: String::from("window.caption"),
        parameters: &gui_parameters,
        result: ScalarType::Boolean,
        required_capability: "view.edit",
    }];
    let limits = OperationCatalogLimits {
        max_operations: 1,
        max_parameters_per_operation: 2,
    };
    let gui = OperationCatalog::new(7, &gui_schemas, limits).unwrap();
    let schema = gui.authorize("window.caption", 7, &["view.edit"]).unwrap();
    let caption = ScalarValue::OptionalString(OptionalStringValue(None));
    schema
        .check_argument_types(
            &[GuiParameter::Caption, GuiParameter::Target],
            &[
                ScalarArgumentType::literal(&caption),
                ScalarArgumentType::expression(Some(ScalarType::String), true),
            ],
        )
        .unwrap();
    assert_eq!(schema.result, ScalarType::Boolean);

    struct PositionDomain(Rc<Cell<usize>>);
    impl ScalarArgumentDomain for PositionDomain {
        fn scalar_types(&self) -> ScalarTypeSet {
            ScalarTypeSet::only(ScalarType::Integer)
        }
        fn accepts_literal(&self, value: &ScalarValue) -> bool {
            self.0.set(self.0.get() + 1);
            matches!(value, ScalarValue::Integer(position) if *position <= 100)
        }
    }
    struct Receipt(Rc<()>);
    let reads = Rc::new(Cell::new(0));
    let device_parameters = [NamedParameter::required(
        42u16,
        PositionDomain(reads.clone()),
    )];
    let device_schemas = [OperationSchema {
        key: 17u32,
        parameters: &device_parameters,
        result: Receipt(Rc::new(())),
        required_capability: 31u8,
    }];
    let device = OperationCatalog::new(9, &device_schemas, limits).unwrap();
    let schema = device.authorize(&17, 9, &[31]).unwrap();
    schema
        .check_argument_types(
            &[42],
            &[ScalarArgumentType::literal(&ScalarValue::Integer(100))],
        )
        .unwrap();
    assert_eq!(reads.get(), 1);
    assert_eq!(
        schema.check_argument_types(
            &[42],
            &[ScalarArgumentType::literal(&ScalarValue::Integer(101))]
        ),
        Err(OperationTypeError::Argument {
            parameter_index: 0,
            argument_index: 0,
            error: ArgumentTypeError::InvalidLiteralDomain
        })
    );
    assert_eq!(
        device.authorize(&17, 7, &[31]).unwrap_err(),
        OperationCatalogError::UnsupportedVersion
    );
    assert_eq!(
        device.authorize(&17, 9, &[]).unwrap_err(),
        OperationCatalogError::CapabilityDenied
    );
    assert_eq!(reads.get(), 2);
    assert_eq!(Rc::strong_count(&schema.result.0), 1);
    assert!(std::ptr::eq(schema, &device_schemas[0]));
}

#[test]
fn diagnostics_contain_only_tags_and_positions_not_literals_or_native_metadata() {
    let value = ScalarValue::String("private-script-secret".into());
    let facts = ScalarArgumentType::literal(&value);
    let debug = format!("{facts:?}");
    assert!(debug.contains("has_literal: true"));
    assert!(!debug.contains("private"));
    let parameters = [NamedParameter::required(
        "private-parameter",
        ScalarTypeSet::empty(),
    )];
    let schema = OperationSchema {
        key: "private-operation",
        parameters: &parameters,
        result: (),
        required_capability: "private-capability",
    };
    let error = schema
        .check_argument_types(&["private-parameter"], &[facts])
        .unwrap_err();
    assert!(!format!("{error:?}: {error}").contains("private"));
    assert!(!format!("{schema:?}").contains("private"));
}

#[test]
fn native_domain_unwinding_propagates_without_consuming_values_or_granting_authority() {
    struct Panicking;
    impl ScalarArgumentDomain for Panicking {
        fn scalar_types(&self) -> ScalarTypeSet {
            ScalarTypeSet::only(ScalarType::String)
        }
        fn accepts_literal(&self, _: &ScalarValue) -> bool {
            panic!("trusted native domain unwind")
        }
    }
    let value = ScalarValue::String("still-owned".into());
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        check_argument_type(&Panicking, ScalarArgumentType::literal(&value))
    }));
    assert!(result.is_err());
    assert_eq!(value, ScalarValue::String("still-owned".into()));
    check_argument_type(
        &ScalarTypeSet::only(ScalarType::String),
        ScalarArgumentType::literal(&value),
    )
    .unwrap();
}
