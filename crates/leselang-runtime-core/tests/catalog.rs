use std::{borrow::Borrow, cell::Cell, panic::AssertUnwindSafe, rc::Rc};

use leselang_runtime_core::{
    CalculationFailure, NamedArgumentError, NamedParameter, OperationCatalog,
    OperationCatalogError, OperationCatalogLimits, OperationSchema, ScalarType, ScalarTypeSet,
    ScalarValue,
};

const LIMITS: OperationCatalogLimits = OperationCatalogLimits {
    max_operations: 3,
    max_parameters_per_operation: 3,
};

#[test]
fn unrelated_native_hosts_share_registration_lookup_and_declaration_order_binding() {
    #[derive(PartialEq)]
    enum GuiParameter {
        Target,
        Caption,
    }
    let gui_parameters = [
        NamedParameter::required(GuiParameter::Target, ScalarType::String),
        NamedParameter::optional(GuiParameter::Caption, ScalarType::OptionalString),
    ];
    let gui_schemas = [OperationSchema {
        key: String::from("window.caption"),
        parameters: &gui_parameters,
        result: ScalarType::Boolean,
        required_capability: "view.edit",
    }];
    let gui = OperationCatalog::new(7, &gui_schemas, LIMITS).unwrap();
    let schema = gui.authorize("window.caption", 7, &["view.edit"]).unwrap();
    let names = [GuiParameter::Caption, GuiParameter::Target];
    let bindings = schema.bind_arguments(&names).unwrap();
    assert_eq!(
        bindings.iter().map(|(_, index)| index).collect::<Vec<_>>(),
        [1, 0]
    );
    assert!(std::ptr::eq(schema, &gui_schemas[0]));

    struct DeviceDescriptor(Rc<String>);
    let device_parameters = [NamedParameter::required(
        42u16,
        DeviceDescriptor(Rc::new("native position domain".into())),
    )];
    let device_schemas = [OperationSchema {
        key: 17u32,
        parameters: &device_parameters,
        result: DeviceDescriptor(Rc::new("native receipt".into())),
        required_capability: 31u8,
    }];
    let device = OperationCatalog::new(9, &device_schemas, LIMITS).unwrap();
    let schema = device.authorize(&17, 9, &[31]).unwrap();
    let names = [42];
    let bindings = schema.bind_arguments(&names).unwrap();
    let (parameter, index) = bindings.iter().next().unwrap();
    assert!(std::ptr::eq(parameter, &device_parameters[0]));
    assert_eq!(index, 0);
    assert_eq!(parameter.domain.0.as_str(), "native position domain");
    assert_eq!(schema.result.0.as_str(), "native receipt");
    assert_eq!(Rc::strong_count(&schema.result.0), 1);
}

#[test]
fn empty_limits_are_explicit_deny_all_and_required_inputs_are_not_registration_inputs() {
    let none: [OperationSchema<'_, &str, &str, (), (), ()>; 0] = [];
    let deny_all = OperationCatalogLimits {
        max_operations: 0,
        max_parameters_per_operation: 0,
    };
    let catalog = OperationCatalog::new(1, &none, deny_all).unwrap();
    assert_eq!(catalog.version(), 1);
    assert_eq!(
        catalog.lookup("anything", 1).unwrap_err(),
        OperationCatalogError::UnknownOperation
    );
    let parameters = [NamedParameter::required("needed", ())];
    let schemas = [OperationSchema {
        key: "op",
        parameters: &parameters,
        result: (),
        required_capability: (),
    }];
    assert_eq!(
        OperationCatalog::new(1, &schemas, deny_all).unwrap_err(),
        OperationCatalogError::OperationLimit
    );
    assert_eq!(
        OperationCatalog::new(
            1,
            &schemas,
            OperationCatalogLimits {
                max_operations: 1,
                max_parameters_per_operation: 0,
            }
        )
        .unwrap_err(),
        OperationCatalogError::ParameterLimit { operation_index: 0 }
    );
    let catalog = OperationCatalog::new(1, &schemas, LIMITS).unwrap();
    assert_eq!(
        catalog
            .lookup("op", 1)
            .unwrap()
            .bind_arguments(&[])
            .unwrap_err(),
        NamedArgumentError::MissingArgument { parameter_index: 0 }
    );
}

#[test]
fn version_and_all_count_limits_precede_every_native_comparison() {
    struct NeverCompare;
    impl PartialEq for NeverCompare {
        fn eq(&self, _: &Self) -> bool {
            panic!("native comparison must not run")
        }
    }
    let small = [NamedParameter::optional(NeverCompare, ())];
    let large = [
        NamedParameter::optional(NeverCompare, ()),
        NamedParameter::optional(NeverCompare, ()),
    ];
    let schemas = [
        OperationSchema {
            key: NeverCompare,
            parameters: &small,
            result: (),
            required_capability: (),
        },
        OperationSchema {
            key: NeverCompare,
            parameters: &small,
            result: (),
            required_capability: (),
        },
        OperationSchema {
            key: NeverCompare,
            parameters: &large,
            result: (),
            required_capability: (),
        },
    ];
    let limits = OperationCatalogLimits {
        max_operations: 1,
        max_parameters_per_operation: 0,
    };
    assert_eq!(
        OperationCatalog::new(0, &schemas, limits).unwrap_err(),
        OperationCatalogError::InvalidVersion
    );
    assert_eq!(
        OperationCatalog::new(1, &schemas, limits).unwrap_err(),
        OperationCatalogError::OperationLimit
    );
    assert_eq!(
        OperationCatalog::new(
            1,
            &schemas,
            OperationCatalogLimits {
                max_operations: 3,
                max_parameters_per_operation: 1,
            }
        )
        .unwrap_err(),
        OperationCatalogError::ParameterLimit { operation_index: 2 }
    );
}

#[test]
fn duplicate_operations_precede_duplicate_parameters_and_report_first_positions() {
    let duplicate_parameters = [
        NamedParameter::optional("a", ()),
        NamedParameter::optional("a", ()),
    ];
    let schemas = [
        OperationSchema {
            key: "a",
            parameters: &duplicate_parameters,
            result: (),
            required_capability: (),
        },
        OperationSchema {
            key: "b",
            parameters: &[],
            result: (),
            required_capability: (),
        },
        OperationSchema {
            key: "a",
            parameters: &[],
            result: (),
            required_capability: (),
        },
    ];
    assert_eq!(
        OperationCatalog::new(1, &schemas, LIMITS).unwrap_err(),
        OperationCatalogError::DuplicateOperation { operation_index: 2 }
    );
    assert_eq!(
        OperationCatalog::new(1, &schemas[..2], LIMITS).unwrap_err(),
        OperationCatalogError::DuplicateParameter {
            operation_index: 0,
            parameter_index: 1
        }
    );
    let parameters = [
        NamedParameter::optional("a", ()),
        NamedParameter::optional("b", ()),
        NamedParameter::optional("b", ()),
    ];
    let schemas = [OperationSchema {
        key: "a",
        parameters: &parameters,
        result: (),
        required_capability: (),
    }];
    assert_eq!(
        OperationCatalog::new(1, &schemas, LIMITS).unwrap_err(),
        OperationCatalogError::DuplicateParameter {
            operation_index: 0,
            parameter_index: 2
        }
    );
}

#[test]
fn owned_string_keys_accept_borrowed_queries_without_normalization_or_buffer_changes() {
    let parameters = [NamedParameter::optional(String::from("caption"), ())];
    let schemas = [OperationSchema {
        key: String::from("Window.Caption"),
        parameters: &parameters,
        result: String::from("private result descriptor"),
        required_capability: String::from("view.edit"),
    }];
    let pointers = [
        schemas[0].key.as_ptr(),
        schemas[0].result.as_ptr(),
        schemas[0].required_capability.as_ptr(),
        parameters[0].name.as_ptr(),
    ];
    let catalog = OperationCatalog::new(1, &schemas, LIMITS).unwrap();
    assert!(std::ptr::eq(
        catalog.lookup("Window.Caption", 1).unwrap(),
        &schemas[0]
    ));
    for query in ["window.caption", "Window.Caption ", "", "unknown"] {
        assert_eq!(
            catalog.lookup(query, 1).unwrap_err(),
            OperationCatalogError::UnknownOperation
        );
    }
    assert_eq!(
        pointers,
        [
            schemas[0].key.as_ptr(),
            schemas[0].result.as_ptr(),
            schemas[0].required_capability.as_ptr(),
            parameters[0].name.as_ptr()
        ]
    );
}

#[test]
fn exact_version_precedes_native_borrow_and_unknown_operation_precedes_capability_comparison() {
    struct Key {
        value: String,
        calls: Rc<Cell<usize>>,
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.value == other.value
        }
    }
    impl Borrow<str> for Key {
        fn borrow(&self) -> &str {
            self.calls.set(self.calls.get() + 1);
            &self.value
        }
    }
    struct Capability;
    impl PartialEq for Capability {
        fn eq(&self, _: &Self) -> bool {
            panic!("capability comparison must not run")
        }
    }
    let calls = Rc::new(Cell::new(0));
    let schemas: [OperationSchema<'_, Key, &str, (), (), Capability>; 1] = [OperationSchema {
        key: Key {
            value: "op".into(),
            calls: calls.clone(),
        },
        parameters: &[],
        result: (),
        required_capability: Capability,
    }];
    let catalog = OperationCatalog::new(4, &schemas, LIMITS).unwrap();
    assert_eq!(
        catalog.authorize("op", 0, &[Capability]).unwrap_err(),
        OperationCatalogError::UnsupportedVersion
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(
        catalog.authorize("unknown", 4, &[Capability]).unwrap_err(),
        OperationCatalogError::UnknownOperation
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(
        catalog.authorize("op", 4, &[]).unwrap_err(),
        OperationCatalogError::CapabilityDenied
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn label_membership_is_exact_and_does_not_validate_argument_values_or_native_authority() {
    let parameters = [NamedParameter::required(
        "target",
        ScalarTypeSet::only(ScalarType::String),
    )];
    let schemas = [OperationSchema {
        key: "op",
        parameters: &parameters,
        result: ScalarType::Boolean,
        required_capability: "edit",
    }];
    let catalog = OperationCatalog::new(1, &schemas, LIMITS).unwrap();
    for granted in [&[][..], &["EDIT"][..], &["edit "][..], &["other"][..]] {
        assert_eq!(
            catalog.authorize("op", 1, granted).unwrap_err(),
            OperationCatalogError::CapabilityDenied
        );
    }
    let schema = catalog
        .authorize("op", 1, &["other", "edit", "edit"])
        .unwrap();
    let names = ["target"];
    let bindings = schema.bind_arguments(&names).unwrap();
    let (parameter, _) = bindings.iter().next().unwrap();
    assert!(parameter.domain.validate(&ScalarValue::None).is_err());
    // Label membership and argument shape are not resource authority or result validation.
    assert_eq!(schema.result, ScalarType::Boolean);
}

#[test]
fn metadata_debug_never_formats_native_keys_domains_results_or_capabilities() {
    #[derive(PartialEq)]
    struct Private;
    impl std::fmt::Debug for Private {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("private formatter")
        }
    }
    let parameters = [NamedParameter::required(Private, Private)];
    let schemas = [OperationSchema {
        key: Private,
        parameters: &parameters,
        result: Private,
        required_capability: Private,
    }];
    let catalog = OperationCatalog::new(2, &schemas, LIMITS).unwrap();
    assert_eq!(
        format!("{:?}", schemas[0]),
        "OperationSchema { parameters: 1 }"
    );
    assert_eq!(
        format!("{catalog:?}"),
        "OperationCatalog { version: 2, operations: 1 }"
    );
}

#[test]
fn native_operation_and_parameter_equality_unwinds_are_not_converted_to_catalog_outcomes() {
    struct Native;
    impl PartialEq for Native {
        fn eq(&self, _: &Self) -> bool {
            panic!("native comparison unwind")
        }
    }
    let schemas: [OperationSchema<'_, Native, &str, (), (), ()>; 2] = [
        OperationSchema {
            key: Native,
            parameters: &[],
            result: (),
            required_capability: (),
        },
        OperationSchema {
            key: Native,
            parameters: &[],
            result: (),
            required_capability: (),
        },
    ];
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| OperationCatalog::new(
            1, &schemas, LIMITS
        )))
        .is_err()
    );
    let parameters = [
        NamedParameter::optional(Native, ()),
        NamedParameter::optional(Native, ()),
    ];
    let schemas = [OperationSchema {
        key: "op",
        parameters: &parameters,
        result: (),
        required_capability: (),
    }];
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| OperationCatalog::new(
            1, &schemas, LIMITS
        )))
        .is_err()
    );
}

#[test]
fn native_borrow_and_capability_unwinds_are_not_caught_or_replayed() {
    #[derive(PartialEq)]
    struct Key;
    impl Borrow<str> for Key {
        fn borrow(&self) -> &str {
            panic!("native borrow unwind")
        }
    }
    let schemas: [OperationSchema<'_, Key, &str, (), (), ()>; 1] = [OperationSchema {
        key: Key,
        parameters: &[],
        result: (),
        required_capability: (),
    }];
    let catalog = OperationCatalog::new(1, &schemas, LIMITS).unwrap();
    assert!(std::panic::catch_unwind(AssertUnwindSafe(|| catalog.lookup("op", 1))).is_err());
    struct Capability(Rc<Cell<usize>>);
    impl PartialEq for Capability {
        fn eq(&self, _: &Self) -> bool {
            self.0.set(self.0.get() + 1);
            panic!("native capability unwind")
        }
    }
    let calls = Rc::new(Cell::new(0));
    let schemas: [OperationSchema<'_, &str, &str, (), (), Capability>; 1] = [OperationSchema {
        key: "op",
        parameters: &[],
        result: (),
        required_capability: Capability(calls.clone()),
    }];
    let catalog = OperationCatalog::new(1, &schemas, LIMITS).unwrap();
    let granted = [Capability(calls.clone())];
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| catalog.authorize("op", 1, &granted)))
            .is_err()
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn core_comparison_counts_follow_caller_bounded_metadata_and_grant_slices() {
    struct Counted(u8, Rc<Cell<usize>>);
    impl PartialEq for Counted {
        fn eq(&self, other: &Self) -> bool {
            self.1.set(self.1.get() + 1);
            self.0 == other.0
        }
    }
    let calls = Rc::new(Cell::new(0));
    let parameters: Vec<_> = (0..3)
        .map(|key| NamedParameter::optional(Counted(key, calls.clone()), ()))
        .collect();
    let schemas: Vec<_> = (0..3)
        .map(|key| OperationSchema {
            key: Counted(key, calls.clone()),
            parameters: &parameters,
            result: (),
            required_capability: Counted(9, calls.clone()),
        })
        .collect();
    let catalog = OperationCatalog::new(1, &schemas, LIMITS).unwrap();
    assert_eq!(calls.get(), 3 + 3 * 3);
    calls.set(0);
    assert_eq!(
        catalog.lookup(&Counted(8, calls.clone()), 1).unwrap_err(),
        OperationCatalogError::UnknownOperation
    );
    assert_eq!(calls.get(), 3);
    calls.set(0);
    let granted: Vec<_> = (0..5).map(|key| Counted(key, calls.clone())).collect();
    assert_eq!(
        catalog
            .authorize(&Counted(2, calls.clone()), 1, &granted)
            .unwrap_err(),
        OperationCatalogError::CapabilityDenied
    );
    assert_eq!(calls.get(), 3 + 5);
}

#[test]
fn borrowed_metadata_is_not_a_certificate_for_interior_mutability() {
    let parameters = [NamedParameter::optional("target", Cell::new(1))];
    let schemas = [OperationSchema {
        key: "op",
        parameters: &parameters,
        result: Cell::new(2),
        required_capability: "edit",
    }];
    let catalog = OperationCatalog::new(1, &schemas, LIMITS).unwrap();
    let schema = catalog.authorize("op", 1, &["edit"]).unwrap();
    parameters[0].domain.set(3);
    schema.result.set(4);
    assert_eq!(
        catalog.lookup("op", 1).unwrap().parameters[0].domain.get(),
        3
    );
    assert_eq!(schema.result.get(), 4);
}

#[test]
fn closed_payload_free_errors_remain_external_to_calculation_recovery() {
    let errors = [
        OperationCatalogError::InvalidVersion,
        OperationCatalogError::OperationLimit,
        OperationCatalogError::ParameterLimit { operation_index: 0 },
        OperationCatalogError::DuplicateOperation { operation_index: 1 },
        OperationCatalogError::DuplicateParameter {
            operation_index: 1,
            parameter_index: 2,
        },
        OperationCatalogError::UnsupportedVersion,
        OperationCatalogError::UnknownOperation,
        OperationCatalogError::CapabilityDenied,
    ];
    for error in errors {
        let message = error.to_string();
        assert!(!message.contains("private"));
        assert!(std::error::Error::source(&error).is_none());
        let failure: CalculationFailure<OperationCatalogError> = error.into();
        assert!(!failure.is_recoverable());
    }
}

#[test]
fn catalog_send_and_sync_follow_only_the_borrowed_native_metadata() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let schemas: [OperationSchema<'_, String, String, (), (), ()>; 1] = [OperationSchema {
        key: "op".into(),
        parameters: &[],
        result: (),
        required_capability: (),
    }];
    let catalog = OperationCatalog::new(1, &schemas, LIMITS).unwrap();
    assert_send_sync(&catalog);
    std::thread::scope(|scope| {
        scope
            .spawn(|| assert!(std::ptr::eq(catalog.lookup("op", 1).unwrap(), &schemas[0])))
            .join()
            .unwrap();
    });
}
